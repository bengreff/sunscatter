use super::*;
use crate::test_rocket::rocket;
use glam::DVec3;
use sim::systems::estimate::Aboard;
use sim::systems::{Knowledge, Reading};

fn facts_of(def: &ShipDef, state: &ShipState, known: &Knowledge) -> Vec<PartFacts> {
    let craft = rocket();
    let colours = vec![[0.5; 3]; craft.parts.len()];
    super::super::rules::facts(&craft.parts, &colours, def, state, None, known)
}

fn group<'a>(groups: &'a [Group], title: &str) -> &'a Group {
    groups.iter().find(|g| g.title.starts_with(title)).unwrap_or_else(|| panic!("{title}: {groups:#?}"))
}

/// Every layer summarises the test rocket; a blind craft shows dashes and
/// counts what nothing measures.
#[test]
fn each_layer_summarises_the_craft() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let state = ShipState::initial(def);
    let all = Knowledge::perfect();
    let facts = facts_of(def, &state, &all);
    let known = Known::from(all);
    let x = Inputs {
        def,
        state: &state,
        facts: &facts,
        known: &known,
        aboard: Aboard { mass: Reading { value: 12_345.0, precision: 0.0 }, dry: 4_000.0 },
        com: 2.5,
        power: None,
        accel: DVec3::ZERO,
    };
    let s = summary(Layer::Structural, &x);
    assert_eq!(group(&s, "STRUCTURE").value("mass"), Some("12.3 t"));
    assert_eq!(group(&s, "STRUCTURE").value("g-limit"), Some("none: not accelerating"));
    // The g-load is the one given (the navball's), not the state's last
    // live tick's: 2 g along the nose, the crew's limit.
    let thrusting = Inputs { accel: DVec3::Z * 2.0 * crate::navball::rules::G0, ..x };
    let s2 = summary(Layer::Structural, &thrusting);
    let row = group(&s2, "STRUCTURE").value("g-limit").unwrap_or_default().to_string();
    assert!(row.starts_with("2.0 g / "), "{row}");
    assert!(group(&s2, "STRUCTURE").value("set by").is_some_and(|b| b.starts_with("crew")), "{s2:#?}");
    assert_eq!(group(&s, "STRUCTURE").value("dry mass"), Some("4000 kg"));
    assert_eq!(group(&s, "STRUCTURE").value("centre of mass"), Some("2.5 m above the base"));
    // Unloaded on the pad's state: every rated joint at 0 %, linked to its
    // part; the legs counted, with no load shown (no capacity, D106).
    let joints = group(&s, "JOINTS");
    assert_eq!(joints.value("separation"), Some("0 % / 100 %"));
    let (counted, loads): (Vec<_>, Vec<_>) = joints.rows.iter().partition(|r| r.label == "no capacity");
    assert_eq!(counted.len(), 1);
    assert!(loads.iter().all(|r| r.open.is_some() && r.level == Level::Normal && r.value.ends_with("/ 100 %")));
    let p = summary(Layer::Plumbing, &x);
    let tanks = group(&p, "TANKS");
    assert_eq!(tanks.rows.len(), def.tanks.len());
    assert!(tanks.rows[0].value.contains(" / "), "{:?}", tanks.rows[0]);
    // A row per owner a line starts from.
    let mut owners: Vec<&str> = def.lines.iter().map(|l| l.from.split(':').next().unwrap()).collect();
    owners.sort();
    owners.dedup();
    assert_eq!(group(&p, "LINES").rows.len(), owners.len());
    assert!(group(&p, "LINES").rows.iter().all(|r| r.value.starts_with("→ ")));
    assert_eq!(group(&p, "FLOWS").value("engines"), Some("none burning"));
    let d = summary(Layer::Data, &x);
    assert_eq!(group(&d, "WHAT THE CRAFT MEASURES").value(""), Some("debug mode: everything"));
    let n = facts.len();
    assert_eq!(group(&d, "WHAT THE CRAFT MEASURES").value("part temperatures"), Some(format!("{n} of {n}").as_str()));
    // The pad's parts are not the craft's (the thermal and health layers
    // count the same parts).
    let mut on_pad = facts.clone();
    on_pad[0].ground = true;
    let d = summary(Layer::Data, &Inputs { facts: &on_pad, ..x });
    let want = format!("{} of {}", n - 1, n - 1);
    assert_eq!(group(&d, "WHAT THE CRAFT MEASURES").value("part temperatures"), Some(want.as_str()));
    assert!(group(&d, "COMPUTERS").rows[0].open.is_some(), "a computer opens its component");
    // Blind: dashes and counts.
    let blind = Known::from(Knowledge::default());
    let facts = facts_of(def, &state, &Knowledge::default());
    // The mass to its precision (every tank from empty to full: the
    // staging bar's rule, finding 41), the centre of mass a dash.
    let wide = Aboard { mass: Reading { value: 12_345.0, precision: 6_000.0 }, ..x.aboard };
    let x = Inputs { known: &blind, facts: &facts, aboard: wide, ..x };
    let s = summary(Layer::Structural, &x);
    assert_eq!(group(&s, "STRUCTURE").value("mass"), Some("12 t"));
    assert_eq!(group(&s, "STRUCTURE").value("centre of mass"), Some(DASH));
    assert_eq!(group(&s, "STRUCTURE").value("g-limit"), Some(DASH));
    let joints = group(&s, "JOINTS");
    // The joints the craft gives a capacity are not measured; the rest (the
    // legs, hung by their hinges) have nothing to measure against.
    let rated = def.joints.iter().filter(|j| sim::systems::structure::rated(def, j)).count();
    assert!(rated > 0 && rated < def.joints.len());
    assert_eq!(joints.rows.len(), 2);
    assert_eq!(joints.value("not measured"), Some(format!("{rated} joints (no strain gauge)").as_str()));
    let unrated = format!("{} joints (no allowable given, or hung by a hinge)", def.joints.len() - rated);
    assert_eq!(joints.value("no capacity"), Some(unrated.as_str()));
    let measured = group(&summary(Layer::Data, &x), "WHAT THE CRAFT MEASURES").value("tank levels").map(String::from);
    assert_eq!(measured, Some(format!("0 of {}", def.tanks.len())));
}

/// The parts nearest their limits come first, coloured by the one rule;
/// the parts without a sensor are counted.
#[test]
fn the_hottest_and_the_worn_lead() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let state = ShipState::initial(def);
    let part = |id: u32, name: &str, skin: Option<(f64, f64)>, damage: f64| PartFacts {
        id,
        name: name.into(),
        skin,
        damage,
        integrity: 1.0,
        ..PartFacts::default()
    };
    let facts = vec![
        part(1, "cool", Some((300.0, 600.0)), 0.0),
        part(2, "hot", Some((500.0, 600.0)), 0.9),
        part(3, "over", Some((650.0, 600.0)), 0.3),
        part(4, "blind", None, 0.0),
    ];
    let known = Known::default();
    let x = Inputs {
        def,
        state: &state,
        facts: &facts,
        known: &known,
        aboard: Aboard { mass: Reading { value: 0.0, precision: 0.0 }, dry: 0.0 },
        com: 0.0,
        power: None,
        accel: DVec3::ZERO,
    };
    let t = summary(Layer::Thermal, &x);
    let g = group(&t, "NEAREST");
    // (label, value, level)
    let want = [
        ("over", "650 K / 600 K", Level::Over),
        ("hot", "500 K / 600 K", Level::Warn),
        ("cool", "300 K / 600 K", Level::Normal),
        ("not measured", "1 parts (no temperature sensor)", Level::Normal),
    ];
    let got: Vec<(&str, &str, Level)> = g.rows.iter().map(|r| (r.label.as_str(), r.value.as_str(), r.level)).collect();
    assert_eq!(got, want);
    let h = summary(Layer::Health, &x);
    let g = group(&h, "CONDITION");
    assert_eq!(g.value("sound"), Some("2 of 4 parts"));
    assert_eq!((g.rows[1].label.as_str(), g.rows[1].level), ("hot", Level::Warn));
    assert_eq!((g.rows[2].label.as_str(), g.rows[2].level), ("over", Level::Normal));
}

/// The electrical summary is the Power menu's: buses, batteries (low
/// amber, empty red), what is without power (red), named by their ids as
/// on every layer (the menu writes them in capitals).
#[test]
fn electrical_is_the_power_menus() {
    use crate::interface::menus::power::{sections, Battery, PowerData};
    let cell = |charge: f64| Battery { id: "s2.battery#1".into(), charge, capacity: 3600.0e3, known: true };
    let p = PowerData {
        buses: vec![sim::systems::power::BusRun {
            name: "dragon".into(),
            volts: 28.0,
            load: 410.0,
            solar: 0.0,
            ground: true,
        }],
        bus_names: vec!["dragon".into()],
        batteries: vec![cell(3600.0e3), cell(360.0e3), cell(0.0)],
        unpowered: vec![("heater".into(), "shed".into())],
    };
    let g = electrical(Some(&p));
    assert_eq!(group(&g, "BUSES").value("dragon"), Some("28.0 V · 410 W · ground"));
    assert_eq!(group(&g, "BATTERIES").rows[0].label, "s2.battery#1");
    assert_eq!(group(&g, "WITHOUT POWER").rows[0].label, "heater");
    let menu = sections(Some(&p));
    assert_eq!(menu[0].value("DRAGON"), Some("28.0 V · 410 W · ground"), "the menu's capitals");
    let levels: Vec<Level> = group(&g, "BATTERIES").rows.iter().map(|r| r.level).collect();
    assert_eq!(levels, [Level::Normal, Level::Warn, Level::Over]);
    assert_eq!(group(&g, "WITHOUT POWER").rows[0].level, Level::Over);
    assert_eq!(group(&electrical(None), "BUSES").value(""), Some("no electrical system"));
}
