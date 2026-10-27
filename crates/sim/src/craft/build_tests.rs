//! The build against a small craft of catalog parts: placement by mounts,
//! patterns, components, layers, validation errors, mass properties with
//! each tank's own fill, several engines, partition and determinism.

use super::assembly::AssemblyFile;
use super::build::{build, Assembly};
use super::engine::ship_thrust;
use super::mass::{parallel_axis, solid_cylinder, MassProps};
use super::state::{EngineStatus, ShipState};
use super::{CellOptions, Craft, CraftId, Resolution};
use crate::catalog::{ron_options, Catalog, Ctx, Entry};
use glam::{DMat3, DVec3};

pub(crate) const ENTRIES: &[&str] = &[
    r#"(id: "t-capsule", name: "Test capsule", era: 2020,
        generator: Revolution(profile: [(0.0, 1.0), (1.5, 0.4)], wall: Solid(material: "al-6061", thickness: 0.003),
            inner_mass: 300.0, control: true))"#,
    r#"(id: "t-tank", name: "Test tank", era: 2020,
        generator: Tank(radius: 1.0, barrel: 3.0, domes: Elliptical(0.707), compartments: [("fuel", "RP-1"), ("ox", "LOX")],
            split: 0.4, skirt_low: 0.75, skirt_high: 0.75, material: "al-2219", pressure: 3.0e5, safety_factor: 1.4,
            min_gauge: 0.001))"#,
    r#"(id: "t-thrust", name: "Test thrust structure", era: 2020,
        generator: ThrustStructure(radius: 1.0, length: 0.5, wall: Solid(material: "al-7075", thickness: 0.005),
            ring: 4, ring_radius: 0.6, centre: false, engine_z: 0.2,
            radial: [(name: "leg", z: 0.25, count: 4, start_deg: 0.0)]))"#,
    r#"(id: "t-engine", name: "Test engine", era: 2020,
        generator: Engine(cycle: GasGenerator, oxidizer: "LOX", fuel: "RP-1", mixture_ratio: 2.4,
            performance: (thrust_vac: 100.0e3, isp_vac: 320.0, isp_sl: 280.0), throttle_min: 0.4, gimbal_deg: 5.0,
            ignitions: 3, igniter: TeaTeb, start_time: 1.0, throat_diameter: 0.1, exit_diameter: 0.4,
            nozzle_length: 0.6, powerhead_length: 0.4, powerhead_radius: 0.15, nozzle_material: "inconel-718",
            nozzle_thickness: 0.002, thrust_to_weight: 100.0))"#,
    r#"(id: "t-leg", name: "Test leg", era: 2020,
        generator: Leg(length: 2.0, depth: 0.15, width: 0.2, wall: Solid(material: "al-7075", thickness: 0.003),
            foot_radius: 0.25, foot_mass: 10.0, deployed_deg: 120.0, deploy_rate_dps: 30.0, actuator: Pneumatic,
            stroke: 0.3))"#,
    r#"(id: "t-bottle", name: "Test bottle", era: 2020,
        generator: Component(envelope: Sphere(0.2), equipment: Bottle(gas: "nitrogen", pressure: 3.0e7,
            temperature_k: 293.0, safety_factor: 1.5, performance: 2.5e5)))"#,
    r#"(id: "t-rcs", name: "Test thrusters", era: 2020,
        generator: Component(envelope: Box(0.3, 0.3, 0.3), equipment: Thrusters(thrust: 400.0, isp: 70.0,
            propellant: ColdGas("nitrogen"), enabled: true, mass: 20.0,
            nozzles: [(pos: (0.0, 0.0, 0.0), dir: (0.0, 1.0, 0.0)), (pos: (0.0, 0.0, 0.0), dir: (0.0, -1.0, 0.0))])))"#,
    r#"(id: "t-computer", name: "Test computer", era: 2020, published: { "dry_mass": (12.0, "test") },
        generator: Component(envelope: Box(0.3, 0.2, 0.1), power: 50.0, control: true,
            equipment: Computer(channels: 3, mass: 10.0)))"#,
    r#"(id: "t-seat", name: "Test seat", era: 2020,
        generator: Component(envelope: Box(0.5, 0.5, 0.5), equipment: Seat(crew: 1, mass: 30.0)))"#,
];

pub(crate) const CRAFT: &str = r#"(
    name: "Test rocket", crew: 1,
    parts: [
        (id: "capsule", entry: "t-capsule"),
        (id: "tank", entry: "t-tank", mount: (on: "capsule", at: "bottom", via: "top")),
        (id: "thrust", entry: "t-thrust", mount: (on: "tank", at: "bottom", via: "top")),
        (id: "engine", entry: "t-engine", pattern: List(4), mount: (on: "thrust", at: "engine_#", via: "mount")),
        (id: "leg", entry: "t-leg", pattern: List(4), mount: (on: "thrust", at: "leg_#", via: "root")),
    ],
    components: [
        (id: "bottle", entry: "t-bottle", in: "tank", pos: (0.0, 0.0, 3.0)),
        (id: "rcs", entry: "t-rcs", pattern: Radial(count: 2, start_deg: 0.0), in: "capsule", pos: (0.7, 0.0, 0.3)),
        (id: "computer", entry: "t-computer", in: "capsule", pos: (0.0, 0.0, 0.6)),
        (id: "seat", entry: "t-seat", in: "capsule", pos: (0.0, 0.0, 0.3)),
    ],
    joints: [(name: "separation", between: ("capsule", "tank"),
              kind: Separation(system: Clamp, impulse: 500.0, strength: 0.8))],
    plumbing: [
        (from: "tank:ox.outlet", to: "engine#*:ox_in", diameter: 0.05, length: 2.0),
        (from: "tank:fuel.outlet", to: "engine#*:fuel_in", diameter: 0.05, length: 1.0),
        (from: "bottle:out", to: "rcs#*:in", diameter: 0.01, length: 3.0),
    ],
    electrical: [(name: "bus", voltage: 28.0, members: ["computer", "rcs#*"])],
    data: [(from: "computer", to: ["engine#*", "rcs#*"])],
    loading: [
        (tank: "tank:ox", temperature_k: 90.0, pressure: 3.0e5, pressurant: "helium"),
        (tank: "tank:fuel", fill: 0.9, temperature_k: 293.0, pressure: 3.0e5, pressurant: "helium"),
    ],
    groups: { "main": ["engine#*"], "pair": ["engine#1", "engine#3"] },
    sequence: [
        (name: "Ignition", actions: [Ignite("main")]),
        (name: "Separation", actions: [Shutdown("main"), Separate("separation")]),
    ],
    functions: [(name: "Legs", key: "L", actions: [Deploy("leg#*")])],
    aero: (cd0: 0.4),
    thermal: (skin_max_k: 1100.0, internal_max_k: 400.0, node_size: 0.5, interior_specific_heat: 900.0,
              propellant_specific_heat: 2000.0, interior_conductivity: 5.0, internal_coupling: 2.0),
    impact: (max_speed: 8.0),
    contact: (foot: (stiffness: 1.0e5, damping: 1.0e4, stroke: 0.3, friction: 0.8),
              hull: (stiffness: 1.0e6, damping: 1.0e5, friction: 0.5), stick_speed: 0.05),
    antenna: (gain_dbi: 3.0, power_w: 5.0),
)"#;

pub(crate) fn catalog() -> Catalog {
    Catalog::of(ENTRIES.iter().map(|t| Entry::parse(t).unwrap())).unwrap()
}

pub(crate) fn file(text: &str) -> AssemblyFile {
    ron_options().from_str(text).unwrap()
}

pub(crate) fn built(text: &str) -> Result<Assembly, Vec<String>> {
    build(&file(text), &catalog(), &Ctx::data())
}

fn close(a: DVec3, b: DVec3, tol: f64) -> bool {
    (a - b).length() < tol
}

/// The parts in file order, copies numbered; stacked below the root along
/// −Z; engines on the ring; components in their parts; masses summed.
#[test]
fn a_craft_of_catalog_parts_builds() {
    let a = built(CRAFT).unwrap();
    let def = &a.def;
    let ship = def.ship.as_ref().unwrap();
    let names: Vec<&str> = def.parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names[..5], ["capsule", "tank", "thrust", "engine#1", "engine#2"]);
    assert_eq!(def.parts.len(), 11);
    assert_eq!(def.parts[3].parent, Some(2));
    // Tank: 0.75 + 3 + 0.75 = 4.5 m under the capsule; the thrust structure 0.5 m more.
    assert!(close(ship.parts[1].pose.pos, DVec3::new(0.0, 0.0, -4.5), 1e-12));
    assert!(close(ship.parts[2].pose.pos, DVec3::new(0.0, 0.0, -5.0), 1e-12));
    // Engine 1 on the ring at +X, its gimbal point 0.2 m up in the structure.
    let e1 = &ship.engines[0];
    assert!(close(e1.data.pos, DVec3::new(0.6, 0.0, -4.8), 1e-12), "{}", e1.data.pos);
    assert!(close(e1.data.dir, DVec3::Z, 1e-15));
    assert_eq!((e1.feeds[0].len(), e1.feeds[1].len()), (1, 1));
    assert_eq!(ship.tanks[e1.feeds[0][0] as usize].fluid, "LOX");
    // The two thruster blocks mirror each other about the axis.
    assert!(close(ship.thrusters[1].nozzles[0].pos, DVec3::new(-0.7, 0.0, 0.3), 1e-12));
    assert_eq!(ship.thrusters[0].feed, super::ship::Feed::Bottles(vec![0]));
    // Masses: parts and their components.
    let comps: f64 = ship.components.iter().map(|c| c.mass).sum();
    let parts: f64 = ship.parts.iter().map(|p| p.mass).sum();
    assert!((def.spec.dry_mass - parts - comps).abs() < 1e-9);
    assert_eq!(def.parts[0].crew, 1);
    assert!(def.parts[0].control);
    // The computer's published mass, reported against the derived.
    assert!(a.report.unmodelled.iter().any(|u| u.0 == "computer" && (u.1 - 2.0).abs() < 1e-12));
    // The separation joint, a share of the weaker wall.
    let j = ship.joints.iter().find(|j| j.name == "separation").unwrap();
    assert_eq!((j.parent, j.child), (0, 1));
    // Loading: LOX full at 90 K, RP-1 at 90 %.
    let lox = ship.tanks.iter().find(|t| t.id == "tank:ox").unwrap();
    let rho = crate::fluids::fluids().liquid("LOX").unwrap().density(90.0);
    assert!((lox.capacity - lox.volume * 0.97 * rho).abs() < 1e-6);
    // The bottle displaces LOX: the compartment's volume less the sphere.
    let (r, b) = (1.0f64, 3.0f64);
    let head = 2.0 / 3.0 * crate::math::PI * r * r * 0.707;
    let sphere = 4.0 / 3.0 * crate::math::PI * 0.008;
    let raw = crate::math::PI * r * r * 0.6 * b + 2.0 * head;
    assert!((lox.volume - (raw - sphere)).abs() < 1e-9, "{} vs {}", lox.volume, raw - sphere);
    assert!(a.report.z_range.0 < -5.0 && a.report.z_range.1 > 1.4);
}

/// Every error names what is wrong (each case one edit of the craft).
#[test]
fn the_build_refuses_with_named_errors() {
    // (edit from, edit to, words in an error)
    let cases = [
        // An engine mount is not a stack mount.
        (
            r#"(id: "tank", entry: "t-tank", mount: (on: "capsule", at: "bottom", via: "top"))"#,
            r#"(id: "tank", entry: "t-tank", mount: (on: "capsule", at: "bottom", via: "nope"))"#,
            "no mount",
        ),
        (r#"at: "engine_#", via: "mount""#, r#"at: "leg_#", via: "mount""#, "cannot attach"),
        (
            r#"(id: "tank", entry: "t-tank", mount"#,
            r#"(id: "tank", entry: "t-tank", set: {"radius": 1.2}, mount"#,
            "fit",
        ),
        (
            r#"(id: "thrust", entry: "t-thrust", mount: (on: "tank", at: "bottom", via: "top")),"#,
            r#"(id: "thrust", entry: "t-thrust"),"#,
            "touching is not attaching",
        ),
        (
            r#"pattern: List(4), mount: (on: "thrust", at: "leg_#""#,
            r#"pattern: List(4), mount: (on: "thrust", at: "leg_1""#,
            "already taken",
        ),
        (r#"(from: "tank:fuel.outlet", to: "engine#*:fuel_in", diameter: 0.05, length: 1.0),"#, "", "no feed"),
        (r#"to: "engine#*:fuel_in""#, r#"to: "engine#*:ox_in""#, "cannot flow"),
        (r#"(tank: "tank:ox", temperature_k: 90.0"#, r#"(tank: "tank:lox", temperature_k: 90.0"#, "no such tank"),
        (r#"temperature_k: 90.0"#, r#"temperature_k: 200.0"#, "between"),
        (r#"pos: (0.0, 0.0, 0.6)"#, r#"pos: (0.0, 0.0, 2.6)"#, "not inside"),
        (r#"Separate("separation")"#, r#"Separate("seam")"#, "names no joint"),
        (r#"Deploy("leg#*")"#, r#"Deploy("engine#*")"#, "names no mechanism"),
        (r#""pair": ["engine#1", "engine#3"]"#, r#""pair": ["engine#9"]"#, "names no engine"),
        (r#"(id: "bottle", entry: "t-bottle""#, r#"(id: "bottle", entry: "t-tank""#, "is a part"),
        (
            r#"(id: "tank", entry: "t-tank", mount"#,
            r#"(id: "tank", entry: "t-tank", set: {"colour": 1.0}, mount"#,
            "colour",
        ),
    ];
    for (from, to, words) in cases {
        let text = CRAFT.replacen(from, to, 1);
        assert_ne!(text, CRAFT, "edit {from:?} did not apply");
        let errors = built(&text).expect_err(words);
        assert!(errors.iter().any(|e| e.contains(words)), "{words}: {errors:?}");
    }
    assert!(built(CRAFT).is_ok());
}

/// The vessel's mass properties with each tank at its own fill (D104 §5.1)
/// against a hand sum: the dry parts, each tank's liquid a solid cylinder
/// from its base, the bottle's gas at its centre.
#[test]
fn mass_properties_of_a_draining_multi_tank_craft() {
    let a = built(CRAFT).unwrap();
    let craft =
        Craft::from_def(CraftId("t".into()), a.def.clone(), &Resolution::default(), &CellOptions::for_parts(11));
    let ship = craft.ship.clone().unwrap();
    let mut state = ShipState::initial(&ship);
    let dry = MassProps::sum(&craft.part_mass);
    assert!((dry.mass - craft.mass.dry_mass).abs() < 1e-9 && close(dry.com, craft.mass.dry_com, 1e-12));
    for (ox, fuel) in [(1.0, 1.0), (0.5, 0.9), (0.1, 0.0), (0.0, 0.0)] {
        state.tanks[1].liquid = ship.tanks[1].capacity * ox;
        state.tanks[0].liquid = ship.tanks[0].capacity * fuel;
        let (slugs, points) = state.contents(&ship);
        let props = craft.mass.with_contents(&slugs, &points);
        let mut bodies = vec![dry];
        for (t, s) in ship.tanks.iter().zip(&state.tanks) {
            if s.liquid > 0.0 {
                let h = t.region.axis.length() * s.liquid / t.capacity;
                let d = t.region.axis.normalize();
                bodies.push(MassProps {
                    mass: s.liquid,
                    com: t.region.base + d * (h / 2.0),
                    inertia: solid_cylinder(s.liquid, t.region.radius, h, d),
                });
            }
        }
        let gas = state.gas(&ship);
        bodies.push(MassProps { mass: gas, com: ship.bottles[0].pos, inertia: DMat3::ZERO });
        let mass: f64 = bodies.iter().map(|b| b.mass).sum();
        let com = bodies.iter().map(|b| b.com * b.mass).sum::<DVec3>() / mass;
        let inertia = bodies.iter().fold(DMat3::ZERO, |i, b| i + b.inertia + parallel_axis(b.mass, b.com - com));
        assert!((props.mass - mass).abs() < 1e-9 * mass, "{ox} {fuel}");
        assert!(close(props.com, com, 1e-12), "{ox} {fuel}: {} vs {com}", props.com);
        assert!((props.inertia - inertia).x_axis.length() < 1e-9 * inertia.x_axis.length());
    }
}

/// Four engines on a ring, one off: three quarters of the thrust, the
/// weighted mount off the axis by a third of the ring radius (a moment
/// about the centre of mass the gimbal must hold), each tank drawn by the
/// mixture ratio.
#[test]
fn several_engines_and_one_off() {
    let a = built(CRAFT).unwrap();
    let ship = a.def.ship.clone().unwrap();
    let mut state = ShipState::initial(&ship);
    assert!(state.engines.iter().all(|e| e.selected), "the sequence's first ignition selects them");
    state.next_stage(&ship).unwrap();
    assert!(state.engines.iter().all(|e| e.status == EngineStatus::Running && e.ignitions == 2));
    let all = ship_thrust(&ship, &state, 1.0, 0.0, 0.02, false);
    assert!((all.thrust - 400.0e3).abs() < 1e-6);
    assert!(close(all.pos, DVec3::new(0.0, 0.0, -4.8), 1e-12));
    state.apply(&ship, &super::command::ShipCommand::Shutdown("engine#3".into())).unwrap();
    let three = ship_thrust(&ship, &state, 1.0, 0.0, 0.02, false);
    assert!((three.thrust - 300.0e3).abs() < 1e-6);
    // Engines at +X, +Y, −Y (engine 3 at −X off): the mount moves 0.2 m to +X.
    assert!(close(three.pos, DVec3::new(0.2, 0.0, -4.8), 1e-12), "{}", three.pos);
    let com = DVec3::new(0.0, 0.0, -2.0);
    let moment = (three.pos - com).cross(three.dir * three.thrust);
    assert!((moment.y + 0.2 * 300.0e3).abs() < 1e-6 && moment.x.abs() < 1e-6, "{moment}");
    let mdot = 300.0e3 / (320.0 * crate::vessel::G0);
    let ox = three.draws[1];
    assert!((ox - mdot * 2.4 / 3.4).abs() < 1e-9 && (three.draws[0] - mdot / 3.4).abs() < 1e-9);
    // Throttle within each engine's range; empty tanks give nothing.
    let low = ship_thrust(&ship, &state, 0.1, 0.0, 0.02, false);
    assert!((low.thrust - 0.4 * 300.0e3).abs() < 1e-6);
    state.tanks[0].liquid = 0.0;
    assert_eq!(ship_thrust(&ship, &state, 1.0, 0.0, 0.02, false).thrust, 0.0);
    assert!(ship_thrust(&ship, &state, 1.0, 0.0, 0.02, true).thrust > 0.0, "debug mode");
}

/// The pieces' definitions and states partition the craft's: every item
/// once, in order; a piece of a piece is the piece.
#[test]
fn ship_definitions_and_states_partition_by_part() {
    let a = built(CRAFT).unwrap();
    let ship = a.def.ship.clone().unwrap();
    let state = ShipState::initial(&ship);
    let upper: Vec<u32> = vec![0];
    let lower: Vec<u32> = (1..11).collect();
    let (du, dl) = (ship.subset(&upper), ship.subset(&lower));
    let (su, sl) = (state.partition(&upper, &du), state.partition(&lower, &dl));
    assert_eq!(du.tanks.len() + dl.tanks.len(), ship.tanks.len());
    assert_eq!((su.tanks.len(), sl.tanks.len()), (du.tanks.len(), dl.tanks.len()));
    assert_eq!((su.engines.len(), sl.engines.len()), (0, 4));
    assert_eq!((du.thrusters.len(), su.thrusters.len()), (2, 2));
    // The thrusters lose their bottle (it stays in the tank's piece).
    assert_eq!(du.thrusters[0].feed, super::ship::Feed::Bottles(vec![]));
    assert_eq!(dl.engines[0].feeds, ship.engines[0].feeds);
    assert!(su.cabin.is_some() && sl.cabin.is_none());
    assert_eq!(su.joints.len() + sl.joints.len() + 1, state.joints.len(), "the cut joint goes");
    let consumables = state.consumables(&ship);
    assert!((su.consumables(&du) + sl.consumables(&dl) - consumables).abs() < 1e-9);
    let piece = a.def.subset(&[0]);
    assert_eq!(piece.ship.as_deref(), Some(&du));
    assert_eq!(piece.parts.len(), 1);
    // Each piece keeps the commands that name what it has: the capsule
    // none, the tank both stages less the separation (its joint is gone),
    // and the sequence's position counts the stages kept before it.
    let mut flown = state.clone();
    flown.next_stage(&ship).unwrap();
    flown.next_stage(&ship).unwrap();
    let (fu, fl) = (flown.partition(&upper, &du), flown.partition(&lower, &dl));
    assert!(fu.stages.is_empty() && fu.functions.is_empty() && fu.sequence == 0);
    let names: Vec<(&str, usize)> = fl.stages.iter().map(|s| (s.name.as_str(), s.actions.len())).collect();
    assert_eq!(names, [("Ignition", 1), ("Separation", 1)]);
    assert_eq!((fl.sequence, fl.functions.len()), (2, 1));
}

/// Built twice, the same bits (definition and craft); the commands of the
/// sequence apply in order.
#[test]
fn the_build_is_deterministic() {
    let (a, b) = (built(CRAFT).unwrap(), built(CRAFT).unwrap());
    assert_eq!(a.def, b.def);
    let opts = CellOptions::for_parts(11);
    let ca = Craft::from_def(CraftId("t".into()), a.def, &Resolution::default(), &opts);
    let cb = Craft::from_def(CraftId("t".into()), b.def, &Resolution::default(), &opts);
    assert_eq!(ca, cb);
    let ship = ca.ship.clone().unwrap();
    let mut state = ShipState::initial(&ship);
    assert_eq!(state.next_stage(&ship).unwrap().0, "Ignition");
    let (name, effects) = state.next_stage(&ship).unwrap();
    assert_eq!((name.as_str(), effects), ("Separation", vec![super::state::Effect::Separate(1)]));
    assert!(state.next_stage(&ship).is_none());
}
