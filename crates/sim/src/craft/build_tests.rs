//! The build against a small craft of catalog parts: placement by mounts,
//! patterns, components, layers, validation errors, mass properties with
//! each tank's own fill, several engines, partition and determinism.

use super::assembly::AssemblyFile;
use super::build::{build, Assembly};
use super::mass::{parallel_axis, solid_cylinder, MassProps};
use super::state::{EngineStatus, ShipState};
use super::{CellOptions, Craft, CraftId, Resolution};
use crate::catalog::{ron_options, Catalog, Ctx};
use crate::systems::{command, propulsion};
use crate::time::Epoch;
use glam::{DMat3, DVec3};

pub(crate) use super::small::ROCKET as CRAFT;

pub(crate) fn catalog() -> &'static Catalog {
    super::small::catalog()
}

pub(crate) fn file(text: &str) -> AssemblyFile {
    ron_options().from_str(text).unwrap()
}

pub(crate) fn built(text: &str) -> Result<Assembly, Vec<String>> {
    build(&file(text), catalog(), &Ctx::data())
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

/// The vessel's mass properties with each tank at its own fill (D104 §5.1,
/// F2) against a hand sum: the dry parts, each tank's liquid a solid
/// cylinder settled on its base (its height from its density at its
/// temperature), the bottle's gas at its centre, each ullage's gas at the
/// middle of the length the liquid leaves.
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
        let (slugs, points) = crate::systems::plumbing::contents(&ship, &state);
        let props = craft.mass.with_slugs(&slugs, &points);
        let mut bodies = vec![dry];
        let area = |t: &crate::craft::ship::TankDef| std::f64::consts::PI * t.region.radius * t.region.radius;
        let density =
            |t: &crate::craft::ship::TankDef, k: f64| crate::fluids::fluids().liquid(&t.fluid).unwrap().density(k);
        for (t, s) in ship.tanks.iter().zip(&state.tanks) {
            if s.liquid > 0.0 {
                let h = s.liquid / (density(t, s.liquid_k) * area(t));
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
        // Each ullage's pressurant over its liquid, up to the far dome.
        for (t, s) in ship.tanks.iter().zip(&state.tanks) {
            let mass = s.ullage_moles * crate::fluids::fluids().gas(&t.load.pressurant).unwrap().molar_mass;
            let (h, length) = (s.liquid / (density(t, s.liquid_k) * area(t)), t.volume / area(t));
            let com = t.region.base + t.region.axis.normalize() * (0.5 * (h + length));
            bodies.push(MassProps { mass, com, inertia: DMat3::ZERO });
        }
        let mass: f64 = bodies.iter().map(|b| b.mass).sum();
        let com = bodies.iter().map(|b| b.com * b.mass).sum::<DVec3>() / mass;
        let inertia = bodies.iter().fold(DMat3::ZERO, |i, b| i + b.inertia + parallel_axis(b.mass, b.com - com));
        assert!((props.mass - mass).abs() < 1e-9 * mass, "{ox} {fuel}");
        assert!(close(props.com, com, 1e-12), "{ox} {fuel}: {} vs {com}", props.com);
        assert!((props.inertia - inertia).x_axis.length() < 1e-9 * inertia.x_axis.length());
    }
}

/// A start's time on the systems' clock.
fn t0() -> Epoch {
    Epoch::new(1_000_000_000, 0.0)
}

/// The lit engines at their held level for the pilot's `throttle` (their
/// starts done: `propulsion`'s step ramps them there).
fn at_level(ship: &super::ship::ShipDef, state: &mut ShipState, throttle: f64) {
    for (e, d) in state.engines.iter_mut().zip(&ship.engines) {
        if matches!(e.status, EngineStatus::Starting | EngineStatus::Running) {
            e.status = EngineStatus::Running;
            e.level = propulsion::setting(&d.data, throttle, e.throttle);
        }
    }
}

/// Four engines on a ring, one off: three quarters of the thrust, the
/// weighted mount off the axis by a third of the ring radius (a moment
/// about the centre of mass the gimbal must hold), each tank drawn by the
/// mixture ratio (`systems::propulsion::output`, what the vessel flies).
#[test]
fn several_engines_and_one_off() {
    let a = built(CRAFT).unwrap();
    let ship = a.def.ship.clone().unwrap();
    let mut state = ShipState::initial(&ship);
    assert!(state.engines.iter().all(|e| e.selected), "the sequence's first ignition selects them");
    crate::systems::ship_os::sequence::next_stage(&ship, &mut state, t0()).unwrap();
    assert!(state.engines.iter().all(|e| e.status == EngineStatus::Starting && e.ignitions == 2));
    at_level(&ship, &mut state, 1.0);
    let all = propulsion::output(&ship, &state, 0.0, 0.02, false);
    assert!((all.thrust - 400.0e3).abs() < 1e-6);
    assert!(close(all.pos, DVec3::new(0.0, 0.0, -4.8), 1e-12));
    let cx = command::Context { at: t0(), sequence: false };
    command::apply(&ship, &mut state, &super::command::ShipCommand::Shutdown("engine#3".into()), cx).unwrap();
    let off = ship.engines_of("engine#3")[0];
    assert_eq!(state.engines[off].status, EngineStatus::ShuttingDown);
    // Its tail-off done.
    (state.engines[off].status, state.engines[off].level) = (EngineStatus::Off, 0.0);
    let three = propulsion::output(&ship, &state, 0.0, 0.02, false);
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
    at_level(&ship, &mut state, 0.1);
    let low = propulsion::output(&ship, &state, 0.0, 0.02, false);
    assert!((low.thrust - 0.4 * 300.0e3).abs() < 1e-6);
    state.tanks[0].liquid = 0.0;
    assert_eq!(propulsion::output(&ship, &state, 0.0, 0.02, false).thrust, 0.0);
    assert!(propulsion::output(&ship, &state, 0.0, 0.02, true).thrust > 0.0, "debug mode");
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
    let (su, sl) = (state.partition(&upper, &ship, &du, true), state.partition(&lower, &ship, &dl, false));
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
    // Each piece keeps the whole program and sees the commands that name
    // what it has (`craft::program`): the capsule none, the tank both
    // stages less the separation (its joint is gone).
    let mut flown = state.clone();
    crate::systems::ship_os::sequence::next_stage(&ship, &mut flown, t0()).unwrap();
    crate::systems::ship_os::sequence::next_stage(&ship, &mut flown, t0()).unwrap();
    let (fu, fl) = (flown.partition(&upper, &ship, &du, true), flown.partition(&lower, &ship, &dl, false));
    for p in [&fu, &fl] {
        assert!((&p.stages, &p.functions, p.sequence) == (&flown.stages, &flown.functions, 2));
    }
    use super::program::{view, Shown};
    assert!(view(&du, &fu.stages).is_empty() && view(&du, &fu.functions).is_empty());
    let shown = |index: usize, actions: Vec<usize>| Shown { index, actions };
    assert_eq!(view(&dl, &fl.stages), [shown(0, vec![0]), shown(1, vec![0])]);
    assert_eq!(view(&dl, &fl.functions), [shown(0, vec![0])]);
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
    assert_eq!(crate::systems::ship_os::sequence::next_stage(&ship, &mut state, t0()).unwrap().0, "Ignition");
    let (name, effects) = crate::systems::ship_os::sequence::next_stage(&ship, &mut state, t0()).unwrap();
    assert_eq!((name.as_str(), effects), ("Separation", vec![super::state::Effect::Separate(1)]));
    assert!(crate::systems::ship_os::sequence::next_stage(&ship, &mut state, t0()).is_none());
}
