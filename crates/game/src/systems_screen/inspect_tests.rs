use super::*;
use crate::format::DASH;
use crate::test_rocket::rocket;

fn setup() -> (std::sync::Arc<sim::craft::Craft>, ShipState, Vec<PartFacts>) {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let state = ShipState::initial(def);
    let colours = vec![[0.5; 3]; craft.parts.len()];
    let facts =
        super::super::rules::facts(&craft.parts, &colours, def, &state, None, &sim::systems::Knowledge::perfect());
    (craft, state, facts)
}

fn titles(groups: &[Group]) -> Vec<&str> {
    groups.iter().map(|g| g.title.as_str()).collect()
}

fn value<'a>(groups: &'a [Group], title: &str, label: &str) -> &'a str {
    let g = groups.iter().find(|g| g.title == title).unwrap_or_else(|| panic!("{title}"));
    g.value(label).unwrap_or_else(|| panic!("{title}: {label}"))
}

#[test]
fn a_tank_part_shows_its_structure_both_compartments_condition_and_ratings() {
    let (craft, state, facts) = setup();
    let def = craft.ship.as_deref().unwrap();
    let id = def.part_named("tank").unwrap();
    let g = details(def, &state, &facts, Selection::Part(id), &Known::default());
    // The one layout: name and type, structure, its functions, then its wear and ratings.
    let want = ["PART tank", "STRUCTURE", "TANK tank:fuel", "TANK tank:ox", "CONDITION", "RATINGS", "COMPONENTS · 1"];
    assert_eq!(titles(&g), want);
    assert_eq!(value(&g, "STRUCTURE", "joint"), "separation · separation system");
    // Known (perfect knowledge here) and unloaded on the pad's state.
    assert_eq!(value(&g, "STRUCTURE", "joint load"), "0 % / 100 %");
    assert_eq!(value(&g, "TANK tank:ox", "fluid"), "LOX");
    assert!(value(&g, "TANK tank:fuel", "liquid").ends_with("(90.0 %)"));
    // Loaded at 3 bar: the gas law on the state gives it back, against its burst.
    let ox = g.iter().find(|g| g.title == "TANK tank:ox").unwrap();
    let row = ox.rows.iter().find(|r| r.label == "ullage pressure").unwrap();
    assert!(row.value.starts_with("3.00 bar / ") && row.value.ends_with(" bar"), "{}", row.value);
    assert_eq!(row.level, crate::format::Level::Normal, "well under its burst");
    assert!(row.hover.is_some(), "the term is explained");
    assert_eq!(value(&g, "CONDITION", "integrity"), "100 %");
    assert!(value(&g, "RATINGS", "burst pressure").ends_with("bar"), "derived from the tank");
}

#[test]
fn an_engine_and_a_leg_show_their_function_and_commands() {
    let (craft, mut state, facts) = setup();
    let def = craft.ship.as_deref().unwrap();
    let engine = def.part_named("engine#2").unwrap();
    let g = details(def, &state, &facts, Selection::Part(engine), &Known::default());
    assert_eq!(value(&g, "ENGINE engine#2", "status"), "Off");
    assert_eq!(value(&g, "ENGINE engine#2", "ignitions left"), "3");
    assert_eq!(value(&g, "ENGINE engine#2", "throttle range"), "40–100 %");
    let cmds = |state: &ShipState, sel| commands(def, state, sel).into_iter().map(|c| c.0).collect::<Vec<_>>();
    assert_eq!(cmds(&state, Selection::Part(engine)), ["IGNITE engine#2"]);
    crate::test_rocket::running(def, &mut state, "engine#2");
    assert_eq!(cmds(&state, Selection::Part(engine)), ["SHUT DOWN engine#2"]);
    let leg = def.part_named("leg#1").unwrap();
    assert_eq!(cmds(&state, Selection::Part(leg)), ["DEPLOY leg#1"]);
    let g = details(def, &state, &facts, Selection::Part(leg), &Known::default());
    assert_eq!(value(&g, "MECHANISM leg#1", "angle"), "0°");
    crate::test_rocket::deployed(def, &mut state, "leg#1");
    assert!(cmds(&state, Selection::Part(leg)).is_empty(), "a leg that cannot retract");
    // The tank's separation joint, the capsule's nothing of its own.
    assert_eq!(cmds(&state, Selection::Part(def.part_named("tank").unwrap())), ["SEPARATE separation"]);
    assert!(cmds(&state, Selection::Part(def.part_named("capsule").unwrap())).is_empty());
}

#[test]
fn components_show_their_equipment_and_switches() {
    let (craft, state, facts) = setup();
    let def = craft.ship.as_deref().unwrap();
    let index = |id: &str| def.components.iter().position(|c| c.id == id).unwrap();
    // (component, its function's group)
    let table = [
        ("bottle", "BOTTLE bottle"),
        ("rcs#1", "THRUSTERS rcs#1"),
        ("computer", "COMPUTER computer"),
        ("seat", "SEAT seat"),
    ];
    for (id, function) in table {
        let g = details(def, &state, &facts, Selection::Component(index(id)), &Known::default());
        let t = titles(&g);
        assert_eq!((t[0], *t.last().unwrap()), (format!("COMPONENT {id}").as_str(), function), "{t:?}");
    }
    let g = details(def, &state, &facts, Selection::Component(index("bottle")), &Known::default());
    assert_eq!(value(&g, "BOTTLE bottle", "gas"), "nitrogen");
    assert!(value(&g, "BOTTLE bottle", "pressure").starts_with("300 bar / "), "the card's rule (finding 49)");
    // Its contents of its full load, as the RCS menu reads its feed
    // (finding 48: `bottle_contents`), exact in debug mode; through a
    // transducer to 0.41 bar, the gas law's kilograms of that.
    let j = def.bottles.iter().position(|b| b.id == "bottle").unwrap();
    let full = def.bottle_gas(&def.bottles[j]);
    let exact = crate::format::amount(Some(sim::systems::Reading { value: full, precision: 0.0 }), full);
    assert_eq!(value(&g, "BOTTLE bottle", "contents"), exact);
    let mut k = sim::systems::Knowledge::default();
    k.learn(Quantity::BottlePressure(j as u32), 4.1e4);
    let r = bottle_contents(def, &state, &k, j).unwrap();
    assert!(r.precision > 0.0 && r.precision < 1e-2 * full, "{r:?} of {full} kg");
    let g = details(def, &state, &facts, Selection::Component(index("bottle")), &Known::from(k));
    assert_eq!(value(&g, "BOTTLE bottle", "contents"), crate::format::amount(Some(r), full));
    let g = details(def, &state, &facts, Selection::Component(index("computer")), &Known::default());
    assert_eq!(value(&g, "COMPONENT computer", "power"), "50 W · on");
    assert_eq!(value(&g, "COMPUTER computer", "commands"), "engine#1, engine#2, engine#3, engine#4, rcs#1, rcs#2");
    let cmds = |i| commands(def, &state, Selection::Component(i)).into_iter().map(|c| c.0).collect::<Vec<_>>();
    assert_eq!(cmds(index("computer")), ["POWER computer off"]);
    assert_eq!(cmds(index("rcs#2")), ["RCS rcs#2 off"]);
    assert!(details(def, &state, &facts, Selection::Component(999), &Known::default()).is_empty());
    assert!(commands(def, &state, Selection::Part(999)).is_empty());
}

/// A name selects a part by its id, else a component (MCP's `shoot
/// {part}`: the battery and bottle inspectors had no way in).
#[test]
fn a_name_selects_a_part_or_a_component() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let bottle = def.components.iter().position(|c| c.id == "bottle").unwrap();
    let table = [
        ("tank", Some(Selection::Part(def.part_named("tank").unwrap()))),
        ("bottle", Some(Selection::Component(bottle))),
        ("nothing", None),
    ];
    for (name, want) in table {
        assert_eq!(named(def, name), want, "{name}");
    }
}

/// What the craft does not measure is a dash (D106); design data stays.
#[test]
fn values_without_a_sensor_read_as_a_dash() {
    let (craft, state, facts) = setup();
    let def = craft.ship.as_deref().unwrap();
    let blind = Known::from(sim::systems::Knowledge::default());
    let tank = details(def, &state, &facts, Selection::Part(def.part_named("tank").unwrap()), &blind);
    for label in ["liquid temperature", "ullage temperature"] {
        assert_eq!(value(&tank, "TANK tank:ox", label), DASH, "{label}");
    }
    assert!(value(&tank, "TANK tank:ox", "liquid").starts_with("—/"), "against the design's capacity");
    assert!(value(&tank, "TANK tank:ox", "ullage pressure").starts_with("— / "), "against the design's burst");
    assert_eq!(value(&tank, "TANK tank:ox", "fluid"), "LOX");
    assert!(value(&tank, "TANK tank:ox", "operating").ends_with("bar"), "the design's");
    let bottle = def.components.iter().position(|c| c.id == "bottle").unwrap();
    let g = details(def, &state, &facts, Selection::Component(bottle), &blind);
    assert!(value(&g, "BOTTLE bottle", "contents").starts_with("—/"), "against its full load");
    assert!(value(&g, "BOTTLE bottle", "pressure").starts_with("— / "));
    // A pressure transducer to 0.1 bar: one decimal.
    let mut k = sim::systems::Knowledge::default();
    k.learn(Quantity::TankPressure(1), 1e4);
    let g = details(def, &state, &facts, Selection::Part(def.part_named("tank").unwrap()), &Known::from(k));
    assert!(value(&g, "TANK tank:ox", "ullage pressure").starts_with("3.0 bar / "));
}

/// Ignitions left, one rule for the screen and the card: a hypergolic
/// engine is unlimited on the pad too; a limited one's count stands, the
/// pad's supply said while clamped; a group by its range.
#[test]
fn ignitions_left_reads_one_way() {
    let max = u32::MAX;
    let table: [(&[u32], bool, &str); 9] = [
        (&[3], false, "3"),
        (&[3], true, "3 (the pad lights it)"),
        (&[0], false, "0"),
        (&[max], false, "unlimited (hypergolic)"),
        (&[max], true, "unlimited (hypergolic)"),
        (&[1, 1, 1], false, "1 each"),
        (&[1, 1, 1], true, "1 each (the pad lights them)"),
        (&[1, 3], false, "1–3"),
        (&[2, max], true, "2 to unlimited (the pad lights them)"),
    ];
    for (left, clamped, want) in table {
        assert_eq!(ignitions_left(left, clamped), want, "{left:?} {clamped}");
    }
}
