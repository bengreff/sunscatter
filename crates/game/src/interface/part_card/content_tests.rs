//! Part cards on the Falcon 9: an engine's, a tank's as the craft measures
//! it, the interstage's separation and the pad's clamps with their
//! confirmed commands, the sensors the dashes name, what cannot be undone.

use super::*;
use sim::systems::data;

fn falcon9() -> (ShipDef, ShipState) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = (**sim::craft::load_definition(&path).unwrap().ship.as_ref().unwrap()).clone();
    let state = ShipState::initial(&def);
    (def, state)
}

fn part(def: &ShipDef, id: &str) -> u32 {
    def.part_named(id).unwrap()
}

fn value<'a>(card: &'a Card, heading: &str, label: &str) -> &'a Value {
    let s = card.sections.iter().find(|s| s.heading.starts_with(heading)).unwrap_or_else(|| panic!("{heading}"));
    &s.lines.iter().find(|l| l.label == label).unwrap_or_else(|| panic!("{heading} {label}")).value
}

#[test]
fn an_engine_card_shows_its_state_thrust_and_commands() {
    let (def, mut state) = falcon9();
    let k = Knowledge::perfect();
    let thrusts = vec![0.0; def.engines.len()];
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &thrusts, kelvin: &[], chute: None };
    let mvac = part(&def, "s2.mvac");
    let card = card(&live, mvac).unwrap();
    assert_eq!(card.title, "s2.mvac");
    assert_eq!(value(&card, "ENGINE", "state"), &Value::Shown("off".into()));
    assert!(matches!(value(&card, "ENGINE", "thrust"), Value::Shown(t) if t.starts_with("0 N of ")), "{card:?}");
    let commands: Vec<(&ShipCommand, bool)> = card.commands.iter().map(|c| (&c.command, c.confirm)).collect();
    assert_eq!(commands, [(&ShipCommand::Ignite("s2.mvac".into()), false)]);
    state.engines[def.engines.iter().position(|e| e.id == "s2.mvac").unwrap()].status = EngineStatus::Running;
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &thrusts, kelvin: &[], chute: None };
    let card = super::card(&live, mvac).unwrap();
    assert_eq!(card.commands[0].command, ShipCommand::Shutdown("s2.mvac".into()));
}

#[test]
fn a_tank_card_shows_what_the_craft_measures() {
    let (def, state) = falcon9();
    let k = data::knowledge(&def, &state, false);
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &[], kelvin: &[], chute: None };
    let s1 = card(&live, part(&def, "s1.tank")).unwrap();
    assert!(matches!(value(&s1, "TANK s1.tank:lox", "contents"), Value::Shown(t) if t.ends_with(" t")), "{s1:?}");
    // Dragon's tanks have no level sensor (falcon9.md §5.2).
    let t = def.tanks.iter().position(|t| t.id.starts_with("dragon")).unwrap();
    let dragon = card(&live, def.tanks[t].part).unwrap();
    let heading = format!("TANK {}", def.tanks[t].id);
    assert_eq!(value(&dragon, &heading, "contents"), &Value::Unknown("a level sensor in the tank: none fitted".into()));
}

#[test]
fn separations_and_clamps_are_confirmed() {
    let (def, state) = falcon9();
    let k = Knowledge::perfect();
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &[], kelvin: &[], chute: None };
    let interstage = card(&live, part(&def, "s1.interstage")).unwrap();
    assert_eq!(value(&interstage, "SEPARATION", "state"), &Value::Shown("attached".into()));
    let sep = interstage.commands.iter().find(|c| c.command == ShipCommand::Separate("stage separation".into()));
    assert!(sep.is_some_and(|c| c.confirm), "{:?}", interstage.commands);
    let pad = card(&live, part(&def, "pad.mount")).unwrap();
    assert_eq!(value(&pad, "CLAMPS", "state"), &Value::Shown("holding".into()));
    assert!(pad.commands.iter().any(|c| c.command == ShipCommand::ReleaseClamps && c.confirm));
}

#[test]
fn a_dash_names_the_sensor_that_would_measure_it() {
    let (def, _) = falcon9();
    let tank = |id: &str| def.tanks.iter().position(|t| t.id == id).unwrap() as u32;
    let dragon = def.tanks.iter().position(|t| t.id.starts_with("dragon")).unwrap() as u32;
    // (quantity, words)
    let table = [
        (
            Quantity::TankLevel(tank("s1.tank:lox")),
            "s1.lox-level (not working: switched off, failed, unpowered or not linked)",
        ),
        (Quantity::TankLevel(dragon), "a level sensor in the tank: none fitted"),
        (Quantity::EngineThrust(0), "the engine's own controller (lost with its part)"),
        (Quantity::CabinO2, "a cabin air sensor (none modelled yet)"),
        (Quantity::Velocity, "a sensor the craft does not carry"),
    ];
    for (q, want) in table {
        assert_eq!(sensor_for(&def, q), want, "{q:?}");
    }
}

#[test]
fn what_cannot_be_undone_is_confirmed() {
    let (mut def, _) = falcon9();
    let leg = def.mechanisms.iter().position(|m| m.id.starts_with("s1.leg")).unwrap();
    // (command, retractable legs, confirm)
    let table = [
        (ShipCommand::Separate("stage separation".into()), true, true),
        (ShipCommand::ReleaseClamps, true, true),
        (ShipCommand::ArmChutes, true, true),
        (ShipCommand::Ignite("s1".into()), true, false),
        (ShipCommand::Deploy("s1.leg#*".into()), false, true),
        (ShipCommand::Deploy("s1.leg#*".into()), true, false),
    ];
    for (c, retractable, want) in table {
        for m in def.mechanisms.iter_mut().skip(leg).filter(|m| m.id.starts_with("s1.leg")) {
            m.item.retractable = retractable;
        }
        assert_eq!(irreversible(&def, &c), want, "{c:?} retractable {retractable}");
    }
    use crate::interface::stage_bar::icons::Look;
    let words: Vec<&str> =
        [Look::Idle, Look::Armed, Look::Running, Look::Failed, Look::Spent].map(chute_words).to_vec();
    assert_eq!(words, ["stowed", "armed", "open", "torn", "cut away"]);
}

/// A part's temperature against its rating colours by the one rule
/// (`format::limit_level`), only where it is measured.
#[test]
fn a_hot_part_reads_amber_then_red() {
    let (def, state) = falcon9();
    let mvac = part(&def, "s2.mvac");
    let rated = def.parts.iter().find(|p| p.part == mvac).unwrap().ratings.rated_k.expect("rated");
    // (temperature as a share of its rating, knowledge, level)
    let table = [
        (0.5, Knowledge::perfect(), format::Level::Normal),
        (0.9, Knowledge::perfect(), format::Level::Warn),
        (1.1, Knowledge::perfect(), format::Level::Over),
        (1.1, Knowledge::default(), format::Level::Normal),
    ];
    for (share, k, want) in table {
        let kelvin = [(mvac, share * rated)];
        let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &[], kelvin: &kelvin, chute: None };
        let card = card(&live, mvac).unwrap();
        let s = card.sections.iter().find(|s| s.heading.starts_with("PART")).unwrap();
        let line = s.lines.iter().find(|l| l.label == "temperature").unwrap();
        assert_eq!(line.level, want, "{share} {}", k.perfect);
    }
}
