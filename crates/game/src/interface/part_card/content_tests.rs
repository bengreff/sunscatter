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
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &thrusts, temps: &[], chute: None };
    let mvac = part(&def, "s2.mvac");
    let card = card(&live, mvac).unwrap();
    assert_eq!(card.title, "s2.mvac");
    assert_eq!(value(&card, "ENGINE", "state"), &Value::Shown("off".into()));
    assert!(matches!(value(&card, "ENGINE", "thrust"), Value::Shown(t) if t.starts_with("0 N of ")), "{card:?}");
    let commands: Vec<(&ShipCommand, bool)> = card.commands.iter().map(|c| (&c.command, c.confirm)).collect();
    assert_eq!(commands, [(&ShipCommand::Ignite("s2.mvac".into()), false)]);
    state.engines[def.engines.iter().position(|e| e.id == "s2.mvac").unwrap()].status = EngineStatus::Running;
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &thrusts, temps: &[], chute: None };
    let card = super::card(&live, mvac).unwrap();
    assert_eq!(card.commands[0].command, ShipCommand::Shutdown("s2.mvac".into()));
}

#[test]
fn a_tank_card_shows_what_the_craft_measures() {
    let (def, state) = falcon9();
    let k = data::knowledge(&def, &state, false);
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &[], temps: &[], chute: None };
    let s1 = card(&live, part(&def, "s1.tank")).unwrap();
    assert!(matches!(value(&s1, "TANK s1.tank:lox", "contents"), Value::Shown(t) if t.ends_with(" t")), "{s1:?}");
    // Dragon's tanks have no level sensor (falcon9.md §5.2): their total
    // on the capsule's card is a dash naming it.
    let t = def.tanks.iter().position(|t| t.id.starts_with("dragon")).unwrap();
    let dragon = card(&live, def.tanks[t].part).unwrap();
    let fluid = def.tanks[t].fluid.as_str();
    assert_eq!(value(&dragon, "TANKS", fluid), &Value::Unknown("a level sensor in the tank: none fitted".into()));
}

/// A battery's charge by the one rule (finding 47, `format::charge`): the
/// card's batteries read what their sensors give, to their 1 %, as the
/// Power menu and the systems screen do.
#[test]
fn a_batterys_charge_reads_by_the_one_rule() {
    let (def, mut state) = falcon9();
    let s2: Vec<usize> = (0..def.batteries.len()).filter(|&j| def.batteries[j].id.starts_with("s2.battery")).collect();
    assert_eq!(s2.len(), 2, "two of 2 kWh in the second stage's tank");
    for &j in &s2 {
        state.batteries[j].charge = 0.7 * def.batteries[j].item.capacity;
    }
    let k = data::knowledge(&def, &state, false);
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &[], temps: &[], chute: None };
    let card = card(&live, def.batteries[s2[0]].part).unwrap();
    let r = |j: usize| k.read(Quantity::BatteryCharge(j as u32), state.batteries[j].charge).unwrap();
    // A sum by `Reading`'s one rule: the precisions add (the estimate's
    // worst case), never the largest of them.
    let sum = r(s2[0]) + r(s2[1]);
    assert_eq!(sum.precision, 2.0 * r(s2[0]).precision);
    let want = format::charge(Some(sum), 4000.0 * 3600.0);
    assert_eq!(want, "2800/4000 Wh (70 %)", "to the sensors' 20 Wh each, added: 1 % of the two, as one battery");
    assert_eq!(value(&card, "BATTERIES", "charge"), &Value::Shown(want));
    assert_eq!(format::charge(Some(r(s2[0])), 7.2e6), "1400/2000 Wh (70 %)", "one of them, to 1 %");
}

/// The capsule's card is the part and its totals (owner, 2026-10-29: "those
/// components should not be listed"): one section per kind of what its
/// components carry, none per component, and none of their commands.
#[test]
fn the_capsules_card_sums_what_it_carries() {
    let (def, state) = falcon9();
    let k = Knowledge::perfect();
    let thrusts = vec![0.0; def.engines.len()];
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &thrusts, temps: &[], chute: None };
    let capsule = part(&def, "dragon.capsule");
    let card = card(&live, capsule).unwrap();
    let headings: Vec<&str> = card.sections.iter().map(|s| s.heading.as_str()).collect();
    let on = |f: &dyn Fn(usize) -> bool, len: usize| (0..len).filter(|&i| f(i)).count();
    let engines = on(&|i| def.engines[i].part == capsule, def.engines.len());
    let tanks = on(&|i| def.tanks[i].part == capsule, def.tanks.len());
    let thrusters = on(&|i| def.thrusters[i].part == capsule, def.thrusters.len());
    assert_eq!(engines, 8, "four SuperDraco pods of two");
    let want = [format!("ENGINES · {engines}"), format!("TANKS · {tanks}"), format!("RCS · {thrusters} blocks")];
    for w in &want {
        assert!(headings.contains(&w.as_str()), "{w} in {headings:?}");
    }
    for one in ["ENGINE ", "TANK ", "BOTTLE ", "BATTERY ", "RCS dragon", "VALVE ", "MECHANISM "] {
        assert!(headings.iter().all(|h| !h.starts_with(one)), "{one} listed: {headings:?}");
    }
    assert_eq!(value(&card, "ENGINES", "state"), &Value::Shown("8 off".into()));
    assert_eq!(
        value(&card, "BATTERIES", "charge"),
        &Value::Shown("10.0/10.0 kWh (100.0 %)".into()),
        "four of 2.5 kWh, exact"
    );
    assert!(card.sections.last().unwrap().heading.starts_with("PART"));
    assert!(card.commands.iter().all(|c| !matches!(c.command, ShipCommand::Rcs(..) | ShipCommand::Ignite(..))));
    // A part's own items keep their sections (the booster's tank's two).
    let s1 = super::card(&live, part(&def, "s1.tank")).unwrap();
    assert!(s1.sections.iter().any(|s| s.heading.starts_with("TANK s1.tank:lox")));
}

#[test]
fn counts_read_in_the_order_met() {
    use crate::interface::part_card::inside::counts;
    // (words, text)
    let table: [(&[&str], &str); 3] =
        [(&["off"; 8], "8 off"), (&["running", "off", "running"], "2 running · 1 off"), (&[], "")];
    for (words, want) in table {
        assert_eq!(counts(words.iter().copied()), want);
    }
}

#[test]
fn separations_and_clamps_are_confirmed() {
    let (def, state) = falcon9();
    let k = Knowledge::perfect();
    let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &[], temps: &[], chute: None };
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

/// A part's skin and interior against their own limits, as the systems
/// screen shows them, colour by the one rule (`format::limit_level`), only
/// where they are measured; a part without interior nodes has no interior
/// line.
#[test]
fn a_hot_part_reads_amber_then_red() {
    let (def, state) = falcon9();
    let mvac = part(&def, "s2.mvac");
    let limit = 1_800.0;
    // (skin temperature as a share of its limit, knowledge, level, value)
    let table = [
        (0.5, Knowledge::perfect(), format::Level::Normal, "900 K / 1800 K"),
        (0.9, Knowledge::perfect(), format::Level::Warn, "1620 K / 1800 K"),
        (1.1, Knowledge::perfect(), format::Level::Over, "1980 K / 1800 K"),
        (1.1, Knowledge::default(), format::Level::Normal, ""),
    ];
    for (share, k, want, text) in table {
        let temps = [(mvac, (Some((share * limit, limit)), None))];
        let live = Live { def: &def, state: &state, knowledge: &k, thrusts: &[], temps: &temps, chute: None };
        let card = card(&live, mvac).unwrap();
        let s = card.sections.iter().find(|s| s.heading.starts_with("PART")).unwrap();
        let line = s.lines.iter().find(|l| l.label == "skin").unwrap();
        assert_eq!(line.level, want, "{share} {}", k.perfect);
        if let Value::Shown(v) = &line.value {
            assert_eq!(v, text, "{share}");
        }
        assert!(s.lines.iter().all(|l| l.label != "interior" && l.label != "temperature"), "{share}");
    }
}
