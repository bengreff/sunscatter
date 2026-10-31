//! Generators against hand calculations; entries, overrides and reports.

use super::*;
use crate::math;
use std::collections::BTreeMap;

fn ctx() -> Ctx<'static> {
    Ctx::data()
}

fn tank(compartments: &[(&str, &str)]) -> TankParams {
    TankParams {
        radius: 1.5,
        barrel: 6.0,
        domes: Dome::Hemispherical,
        compartments: compartments.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        split: 0.25,
        skirt_low: 1.5,
        skirt_high: 1.5,
        material: "al-2219".into(),
        finish: None,
        pressure: 3.0e5,
        safety_factor: 1.4,
        axial_load: 0.0,
        min_gauge: 1.0e-3,
        design_k: 295.0,
        ullage: 0.03,
        bulkhead_insulation: 0.0,
    }
}

fn entry(generator: Generator) -> Entry {
    Entry {
        id: "t".into(),
        name: "t".into(),
        generator,
        published: BTreeMap::new(),
        unmodelled: None,
        tolerance: None,
        ratings: Ratings::default(),
        failures: Vec::new(),
        era: 2020,
        cost: None,
        provisional: false,
    }
}

/// A one-compartment tank: volume πr²b + 4/3·πr³ (two hemispheres); the
/// barrel's hoop thickness SF·p·r/σᵤ (2219-T87 at 295 K: 434 MPa), the
/// heads' half of it; masses wall area × thickness × density.
#[test]
fn a_tank_against_a_hand_calculation() {
    let g = tank(&[("lox", "LOX")]).generate(&ctx()).unwrap();
    let (r, b) = (1.5, 6.0);
    let t = g.tanks[0].clone();
    let volume = math::PI * r * r * b + 4.0 / 3.0 * math::PI * r * r * r;
    assert!((t.volume - volume).abs() < 1e-9, "{} vs {volume}", t.volume);
    let hoop = 1.4 * 3.0e5 * r / 434.0e6;
    let rho = 2840.0;
    let barrel = math::TAU * r * b * hoop * rho;
    // The heads' crown thickness (hoop / 2) is under the 1 mm minimum gauge.
    assert!(hoop > 1e-3 && hoop / 2.0 < 1e-3);
    let domes = 2.0 * math::TAU * r * r * 1e-3 * rho;
    let skirts = math::TAU * r * 3.0 * 1.0e-3 * rho;
    let want = [("barrel", barrel), ("skirts", skirts), ("domes", domes)];
    for (name, mass) in want {
        let got = g.derived.iter().find(|d| d.0 == name).unwrap().1;
        assert!((got / mass - 1.0).abs() < 1e-12, "{name}: {got} vs {mass}");
    }
    assert!((g.mass - (barrel + skirts + domes)).abs() < 1e-9);
    // The liquid fills from the lower dome's pole, the domes inside the skirts.
    assert!((t.base.z - 0.0).abs() < 1e-12 && t.burst == 1.4 * 3.0e5);
    assert_eq!(g.primitives.len(), 1, "domes inside the skirts");
    assert_eq!(g.mounts[1].pos.z, 9.0);
    let names: Vec<&str> = g.ports.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["lox.outlet", "lox.press", "lox.vent"]);
}

/// A common bulkhead: the compartments share the volume; the bulkhead adds
/// its head; an axial load thickens the barrel by buckling.
#[test]
fn a_common_bulkhead_and_an_axial_load() {
    let one = tank(&[("all", "LOX")]).generate(&ctx()).unwrap();
    let two = tank(&[("rp1", "RP-1"), ("lox", "LOX")]).generate(&ctx()).unwrap();
    let total: f64 = two.tanks.iter().map(|t| t.volume).sum();
    assert!((total - one.tanks[0].volume).abs() < 1e-9);
    let r = 1.5;
    assert!((two.tanks[0].volume - math::PI * r * r * 6.0 * 0.25).abs() < 1e-9);
    assert!(two.derived.iter().any(|d| d.0 == "common bulkhead"));
    assert!(two.tanks[1].base.z > two.tanks[0].base.z);
    let mut loaded = tank(&[("all", "LOX")]);
    loaded.axial_load = 5.0e6;
    let heavy = loaded.generate(&ctx()).unwrap();
    // t² = SF·N/(2π·γ·0.605·E): 5 MN on 2219 (73.1 GPa).
    let t = (1.4 * 5.0e6 / (math::TAU * structure::KNOCKDOWN * 0.605 * 73.1e9)).sqrt();
    let barrel = heavy.derived.iter().find(|d| d.0 == "barrel").unwrap().1;
    assert!((barrel / (math::TAU * r * 6.0 * t * 2840.0) - 1.0).abs() < 1e-12);
}

/// A sandwich shell: the frustum's lateral area times the wall's areal
/// mass (two CFRP faces and an aluminium honeycomb core).
#[test]
fn a_shell_against_a_hand_calculation() {
    let p = ShellParams {
        radius_low: 1.83,
        radius_high: 1.83,
        length: 6.5,
        wall: Wall::Sandwich {
            face: "cfrp".into(),
            face_thickness: 1.5e-3,
            core: "al-honeycomb".into(),
            core_thickness: 0.025,
        },
        finish: Some("black-paint".into()),
        radial: vec![structure::RadialSet { name: "fin".into(), z: 5.0, count: 4, start_deg: 45.0 }],
    };
    let g = p.generate(&ctx()).unwrap();
    let areal = 2.0 * 1580.0 * 1.5e-3 + 50.0 * 0.025;
    assert!((g.mass - math::TAU * 1.83 * 6.5 * areal).abs() < 1e-9);
    let fins: Vec<&MountPoint> = g.mounts.iter().filter(|m| m.kind == MountKind::Radial).collect();
    assert_eq!(fins.len(), 4);
    assert_eq!(fins[0].name, "fin_1");
    let s = std::f64::consts::FRAC_1_SQRT_2 * 1.83;
    assert!((fins[0].pos - glam::DVec3::new(s, s, 5.0)).length() < 1e-12);
}

fn merlin() -> EngineParams {
    EngineParams {
        cycle: Cycle::GasGenerator,
        oxidizer: "LOX".into(),
        fuel: "RP-1".into(),
        mixture_ratio: 2.36,
        performance: None,
        throttle_min: 0.4,
        throttle_max: 1.0,
        gimbal_deg: 5.0,
        gimbal_rate_dps: 20.0,
        ignitions: 3,
        igniter: Igniter::TeaTeb,
        start_time: 1.5,
        shutdown_impulse: 0.0,
        npsp_ox: 0.0,
        npsp_fuel: 0.0,
        nozzle_heat_fraction: 0.0,
        throat_diameter: 0.25,
        exit_diameter: 0.92,
        nozzle_length: 1.6,
        powerhead_length: 1.0,
        powerhead_radius: 0.35,
        nozzle_material: "inconel-718".into(),
        nozzle_thickness: 3.0e-3,
        extension: None,
        thrust_to_weight: 180.0,
    }
}

/// An engine needs its published performance (no engine designer yet,
/// D104); its mass derives from the cycle's thrust-to-weight and a
/// published mass replaces it, the gap reported.
#[test]
fn an_engine_from_published_data() {
    let mut e = entry(Generator::Engine(merlin()));
    assert!(e.build(&ctx(), &BTreeMap::new()).unwrap_err().contains("published thrust_vac"));
    let src = |v: f64| Sourced(v, "test".into());
    e.published.insert("thrust_vac".into(), src(914.0e3));
    e.published.insert("isp_vac".into(), src(311.0));
    e.published.insert("thrust_sl".into(), src(845.0e3));
    let built = e.build(&ctx(), &BTreeMap::new()).unwrap();
    let Function::Engine(d) = &built.generated.functions[0] else { panic!() };
    assert!((d.isp_sl - 311.0 * 845.0 / 914.0).abs() < 1e-9);
    assert!((built.generated.mass - 914.0e3 / (crate::vessel::G0 * 180.0)).abs() < 1e-9);
    assert!(built.overrides.iter().all(|o| o.derived.is_none() && !o.contradicts));
    let nozzles: Vec<_> = built.generated.primitives.iter().filter(|p| p.nozzle).collect();
    assert_eq!(nozzles.len(), 1);
    // A published dry mass: the gap is unmodelled mass with its reason.
    e.published.insert("dry_mass".into(), src(470.0));
    e.unmodelled = Some("turbopump and valves heavier than the class".into());
    let built = e.build(&ctx(), &BTreeMap::new()).unwrap();
    let derived = 914.0e3 / (crate::vessel::G0 * 180.0);
    assert_eq!(built.generated.mass, 470.0);
    let (gap, reason) = built.unmodelled.clone().unwrap();
    assert!((gap - (470.0 - derived)).abs() < 1e-9 && reason.contains("turbopump"));
    assert!(built.overrides.iter().any(|o| o.key == "dry_mass" && o.contradicts));
    // Unverified performance is the derived value a published one is checked against.
    let mut own = merlin();
    own.performance = Some(Performance { thrust_vac: 800.0e3, isp_vac: 311.0, isp_sl: 282.0 });
    e.generator = Generator::Engine(own);
    let built = e.build(&ctx(), &BTreeMap::new()).unwrap();
    let thrust = built.overrides.iter().find(|o| o.key == "thrust_vac").unwrap();
    assert_eq!((thrust.derived, thrust.contradicts), (Some(800.0e3), true));
}

/// An extension: two nozzle solids, only the radiatively cooled one is
/// the nozzle (D077).
#[test]
fn an_engine_with_an_extension() {
    let mut p = merlin();
    p.performance = Some(Performance { thrust_vac: 981.0e3, isp_vac: 348.0, isp_sl: 100.0 });
    p.extension = Some(engine::Extension {
        material: "niobium-c103".into(),
        thickness: 1e-3,
        finish: Some("silicide-coating".into()),
        start: 0.3,
    });
    let g = p.generate(&ctx(), &BTreeMap::new()).unwrap();
    assert_eq!(g.primitives.len(), 3);
    let ext: Vec<_> = g.primitives.iter().filter(|p| p.nozzle).collect();
    assert_eq!(ext.len(), 1);
    assert_eq!(ext[0].skin.unwrap().max_k, Some(2620.0));
}

/// Components: a bottle's mass from burst × volume / performance; a
/// battery's from capacity / specific energy (a published capacity
/// replaces the derived one); thruster nozzles normalised.
#[test]
fn components_against_hand_calculations() {
    let bottle = ComponentParams {
        envelope: Envelope::Sphere(0.3),
        equipment: Equipment::Bottle {
            gas: "helium".into(),
            pressure: 30.0e6,
            temperature_k: 80.0,
            safety_factor: 1.5,
            performance: 2.5e5,
        },
        power: 0.0,
        priority: None,
        control: false,
        specific_heat: 900.0,
        max_k: 400.0,
    };
    let g = bottle.generate(&ctx(), &BTreeMap::new()).unwrap();
    let v = 4.0 / 3.0 * math::PI * 0.027;
    assert!((g.mass - 45.0e6 * v / 2.5e5).abs() < 1e-6, "{}", g.mass);
    assert!(matches!(&g.functions[0], Function::Bottle(b) if (b.volume - v).abs() < 1e-12));
    let battery = ComponentParams {
        envelope: Envelope::Box(glam::DVec3::new(0.4, 0.3, 0.2)),
        equipment: Equipment::Battery {
            capacity_wh: 2000.0,
            voltage: 28.0,
            specific_energy: 125.0,
            internal_resistance: 0.05,
            max_power: 2000.0,
        },
        power: 0.0,
        priority: None,
        control: false,
        specific_heat: 900.0,
        max_k: 330.0,
    };
    let mut e = entry(Generator::Component(battery));
    e.published.insert("capacity_wh".into(), Sourced(2100.0, "test".into()));
    let b = e.build(&ctx(), &BTreeMap::new()).unwrap();
    assert!((b.generated.mass - 16.0).abs() < 1e-12);
    assert!(matches!(b.generated.functions[0], Function::Battery(x) if x.capacity == 2100.0 * 3600.0));
    let o = &b.overrides[0];
    assert_eq!((o.derived, o.contradicts), (Some(2000.0), false));
}

/// Parameters change by name; a wrong name or a value of the wrong kind is
/// refused; a published key the generator does not allow is refused.
#[test]
fn settable_parameters_and_allowed_overrides() {
    let mut e = entry(Generator::Tank(tank(&[("lox", "LOX")])));
    let base = e.build(&ctx(), &BTreeMap::new()).unwrap().generated.tanks[0].volume;
    let longer = BTreeMap::from([("barrel".to_string(), 8.0)]);
    let v = e.build(&ctx(), &longer).unwrap().generated.tanks[0].volume;
    assert!((v - base - math::PI * 1.5 * 1.5 * 2.0).abs() < 1e-9);
    let wrong = BTreeMap::from([("colour".to_string(), 1.0)]);
    assert!(e.build(&ctx(), &wrong).unwrap_err().contains("colour"));
    let mut engine = entry(Generator::Engine(merlin()));
    assert!(engine.generator.set("ignitions", 2.5).is_err());
    engine.generator.set("ignitions", 2.0).unwrap();
    e.published.insert("thrust_vac".into(), Sourced(1.0, "x".into()));
    assert!(e.check().unwrap_err().contains("thrust_vac"));
}

/// An entry's text: newtype variants unwrapped, `Some` implicit.
#[test]
fn entries_parse_from_ron() {
    let text = r#"(
        id: "test-shell", name: "Test shell", era: 2010, provisional: true,
        generator: Shell(
            radius_low: 1.0, radius_high: 0.5, length: 2.0,
            wall: Solid(material: "al-6061", thickness: 0.002),
            finish: "white-paint",
        ),
        published: { "dry_mass": (60.0, "a test") },
        unmodelled: "rings and fasteners",
        ratings: (reliability: 9.0, vibration_grms: 14.1),
    )"#;
    let e = Entry::parse(text).unwrap();
    assert!(matches!(&e.generator, Generator::Shell(s) if s.finish.as_deref() == Some("white-paint")));
    let b = e.build(&ctx(), &BTreeMap::new()).unwrap();
    assert_eq!(b.generated.mass, 60.0);
    assert_eq!(b.ratings.reliability, Some(9.0));
    assert_eq!(b.ratings.rated_k, Some(420.0), "from the material");
    assert!(Entry::parse(&text.replace("length: 2.0", "length: 2.0, colour: 1.0")).is_err());
}

/// A component's priority on its bus (D125): its equipment's unless its
/// entry names one. (equipment, the entry's, the component's.)
#[test]
fn a_components_priority_is_its_equipments_unless_named() {
    let docking = r#"Docking(standard: "IDSS", diameter: 1.2, mass: 300.0)"#;
    let computer = "Computer(channels: 3, mass: 20.0)";
    let sensor = r#"Sensor(quantity: "attitude", accuracy: 0.01, rate_hz: 50.0, mass: 5.0)"#;
    let table = [
        (docking, "", Priority::NonEssential),
        (docking, "priority: Some(Critical),", Priority::Critical),
        (computer, "", Priority::Critical),
        (sensor, "", Priority::Essential),
        (sensor, "priority: Some(NonEssential),", Priority::NonEssential),
    ];
    for (equipment, named, want) in table {
        let text = format!(
            r#"(id: "t", name: "t", era: 2020, provisional: true,
                generator: Component(envelope: Sphere(0.2), power: 10.0, {named} equipment: {equipment}))"#
        );
        let e = Entry::parse(&text).unwrap();
        let b = e.build(&ctx(), &BTreeMap::new()).unwrap();
        assert_eq!(b.generated.priority, want, "{equipment} {named}");
    }
}
