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
    g.rows.iter().find(|r| r.0 == label).map(|r| r.1.as_str()).unwrap_or_else(|| panic!("{title}: {label}"))
}

#[test]
fn a_tank_part_shows_its_joint_ratings_condition_and_both_compartments() {
    let (craft, state, facts) = setup();
    let def = craft.ship.as_deref().unwrap();
    let id = def.part_named("tank").unwrap();
    let g = details(def, &state, &facts, Selection::Part(id), &Known::default());
    assert_eq!(titles(&g), ["PART tank", "JOINT separation", "RATINGS", "CONDITION", "TANK tank:fuel", "TANK tank:ox"]);
    assert_eq!(value(&g, "JOINT separation", "kind"), "separation system");
    // Known (perfect knowledge here) and unloaded on the pad's state.
    assert_eq!(value(&g, "JOINT separation", "load"), "0 % of its ultimate");
    assert_eq!(value(&g, "TANK tank:ox", "fluid"), "LOX");
    assert!(value(&g, "TANK tank:fuel", "liquid").ends_with("(90.0 %)"));
    // Loaded at 3 bar: the gas law on the state gives it back.
    assert_eq!(value(&g, "TANK tank:ox", "ullage pressure"), "3.00 bar");
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
    state.apply(def, &ShipCommand::Ignite("engine#2".into())).unwrap();
    assert_eq!(cmds(&state, Selection::Part(engine)), ["SHUT DOWN engine#2"]);
    let leg = def.part_named("leg#1").unwrap();
    assert_eq!(cmds(&state, Selection::Part(leg)), ["DEPLOY leg#1"]);
    let g = details(def, &state, &facts, Selection::Part(leg), &Known::default());
    assert_eq!(value(&g, "MECHANISM leg#1", "angle"), "0°");
    state.apply(def, &ShipCommand::Deploy("leg#1".into())).unwrap();
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
    assert!(value(&g, "BOTTLE bottle", "pressure").starts_with("300.00 bar"));
    let g = details(def, &state, &facts, Selection::Component(index("computer")), &Known::default());
    assert_eq!(value(&g, "COMPONENT computer", "power"), "50 W · on");
    assert_eq!(value(&g, "COMPUTER computer", "commands"), "engine#1, engine#2, engine#3, engine#4, rcs#1, rcs#2");
    let cmds = |i| commands(def, &state, Selection::Component(i)).into_iter().map(|c| c.0).collect::<Vec<_>>();
    assert_eq!(cmds(index("computer")), ["POWER computer off"]);
    assert_eq!(cmds(index("rcs#2")), ["RCS rcs#2 off"]);
    assert!(details(def, &state, &facts, Selection::Component(999), &Known::default()).is_empty());
    assert!(commands(def, &state, Selection::Part(999)).is_empty());
}

/// What the craft does not measure is a dash (D106); design data stays.
#[test]
fn values_without_a_sensor_read_as_a_dash() {
    let (craft, state, facts) = setup();
    let def = craft.ship.as_deref().unwrap();
    let blind = Known::from(sim::systems::Knowledge::default());
    let tank = details(def, &state, &facts, Selection::Part(def.part_named("tank").unwrap()), &blind);
    for label in ["liquid", "liquid temperature", "ullage pressure", "ullage temperature"] {
        assert_eq!(value(&tank, "TANK tank:ox", label), DASH, "{label}");
    }
    assert_eq!(value(&tank, "TANK tank:ox", "fluid"), "LOX");
    assert!(value(&tank, "TANK tank:ox", "operating · burst").ends_with("bar"), "the design's");
    let bottle = def.components.iter().position(|c| c.id == "bottle").unwrap();
    let g = details(def, &state, &facts, Selection::Component(bottle), &blind);
    assert_eq!(value(&g, "BOTTLE bottle", "contents"), DASH);
    assert!(value(&g, "BOTTLE bottle", "pressure").starts_with("— (rated"));
    // A pressure transducer to 0.1 bar: one decimal.
    let mut k = sim::systems::Knowledge::default();
    k.learn(Quantity::TankPressure(1), 1e4);
    let g = details(def, &state, &facts, Selection::Part(def.part_named("tank").unwrap()), &Known::from(k));
    assert_eq!(value(&g, "TANK tank:ox", "ullage pressure"), "3.0 bar");
}
