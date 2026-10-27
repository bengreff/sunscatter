//! A craft of catalog parts in flight (ship-model.md §5.1, §5.3): the
//! small test rocket of `craft::build_tests` (a capsule on a two-compartment
//! tank, four engines on a thrust structure, four legs, cold-gas thrusters)
//! lifts off its pad on its sequence's ignition drawing each tank by the
//! mixture ratio, separates at its joint with momentum conserved and its
//! ship state partitioned, saves and reloads bit for bit, and flies the
//! same in frames and in one jump.

use super::*;
use crate::craft::state::{EngineStatus, ShipState};
use crate::craft::{craft_of, registry, CraftId};
use crate::ephem::Ephemeris;
use crate::kepler::Elements;
use crate::save::SaveGame;
use crate::sol;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

/// The test rocket, registered once for the process.
fn rocket() -> Arc<crate::craft::Craft> {
    let id = CraftId("test-rocket".into());
    registry::register(&id, crate::craft::build_tests::built(crate::craft::build_tests::CRAFT).unwrap().def);
    craft_of(&id)
}

fn on_the_pad(w: &World) -> Vessel {
    Vessel::landed_at(w, VesselId(1), "Earth", 28.6, -80.6, t0(), &rocket())
}

fn full(throttle: f64) -> Controls {
    Controls { throttle, sas: true, ..Default::default() }
}

/// On the pad: tanks at their loading, nothing running, standing on its
/// lowest points. The sequence's ignition starts the four engines; at full
/// throttle it lifts off, each tank drained by its share of the mixture
/// ratio, the vessel's propellant the ship's consumables, its centre of
/// mass moving as the tanks drain unequally.
#[test]
fn a_craft_of_parts_lifts_off_on_its_engines() {
    let w = world();
    let mut v = on_the_pad(&w);
    assert!(matches!(v.phase, Phase::Landed { .. }), "it stands on the pad");
    let def = v.ship_def().unwrap();
    let start = v.ship().unwrap().clone();
    assert_eq!(start, ShipState::initial(&def));
    assert!((v.propellant() - start.consumables(&def)).abs() < 1e-9);
    let com0 = v.mass_props().com;
    assert!(!v.engine_running(&full(1.0)), "engines off");
    let (name, _) = v.next_stage(&w).unwrap();
    assert_eq!(name, "Ignition");
    assert!(v.ship().unwrap().engines.iter().all(|e| e.status == EngineStatus::Running));
    // Each engine's thrust for its flame, from the same rule as the tick.
    let each = v.engine_thrusts(1.0, 0.0).unwrap();
    assert_eq!(each, vec![100.0e3; 4]);
    // The sequence and functions are the vessel's: edited, checked.
    use crate::craft::command::{ShipCommand, Stage};
    let stage = |action: ShipCommand| Stage { name: "x".into(), key: None, actions: vec![action], pinned: true };
    let bad = v.set_functions(vec![stage(ShipCommand::Separate("no such joint".into()))]).unwrap_err();
    assert!(bad[0].contains("names no joint"), "{bad:?}");
    v.set_functions(vec![stage(ShipCommand::Deploy("leg#*".into()))]).unwrap();
    assert!(v.ship().unwrap().functions[0].pinned);
    let (height0, m0) = (v.contact_height(), v.mass());
    v.advance(&w, t0().add_seconds(5.0), &full(1.0), usize::MAX);
    let s = v.ship().unwrap();
    let (fuel, ox) = (start.tanks[0].liquid - s.tanks[0].liquid, start.tanks[1].liquid - s.tanks[1].liquid);
    assert!(fuel > 0.0 && (ox / fuel - 2.4).abs() < 1e-9, "by the mixture ratio: {ox} / {fuel}");
    // Four engines' flow at full throttle for the ticks since ignition.
    let mdot = 4.0 * 100.0e3 / (320.0 * G0);
    assert!(((fuel + ox) / mdot - 5.0).abs() < 0.03, "{} s of burn", (fuel + ox) / mdot);
    let spent = m0 - v.mass();
    assert!(spent >= fuel + ox - 1e-6 && spent < fuel + ox + 1.0, "the thrusters' gas at most: {spent}");
    assert!((v.propellant() - s.consumables(&def)).abs() < 1e-9);
    let (anchor, r, _) = v.state(&w);
    let up = crate::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0;
    assert!(up > height0 + 10.0, "it climbs: {up} m");
    assert!((v.mass_props().com - com0).length() > 1e-3, "the centre of mass moves");
}

/// The sequence's separation fires the joint: the breakup the game
/// applies; the capsule stays the vessel (its control part is the lowest),
/// the tank and engines leave with every tank, engine and bottle, mass and
/// momentum conserved.
#[test]
fn a_separation_partitions_the_ship_and_conserves_momentum() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let (r, vel) =
        Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    let mut v = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, vel, &rocket());
    v.set_attitude(Attitude { q: v.attitude.q, omega: DVec3::new(0.0, 0.01, 0.002) });
    v.advance(&w, t0().add_seconds(10.0), &Controls::default(), usize::MAX);
    v.next_stage(&w).unwrap();
    v.next_stage(&w).unwrap();
    let plan = v.plan_breakup().expect("a pending separation");
    assert_eq!(plan.pieces, vec![vec![0], (1..11).collect::<Vec<u32>>()]);
    let before = v.clone();
    let state = before.ship().unwrap().clone();
    let props = before.mass_props();
    let (_, r0, v0) = before.state(&w);
    let mut ids = VesselIds::default();
    ids.allocate();
    let pieces = v.split(&w, &plan, &plan.crafts(), &mut ids);
    assert_eq!(pieces.len(), 1);
    let all = [&v, &pieces[0]];
    let mass: f64 = all.iter().map(|p| p.mass_props().mass).sum();
    assert!((mass - props.mass).abs() < 1e-9 * mass);
    let (mut com, mut momentum) = (DVec3::ZERO, DVec3::ZERO);
    for p in all {
        let (_, r, vel) = p.state(&w);
        com += (r - r0) * p.mass_props().mass;
        momentum += (vel - v0) * p.mass_props().mass;
    }
    assert!((com / mass).length() < 1e-6, "{}", com / mass);
    assert!(momentum.length() < 1e-6 * mass, "{momentum}");
    let (capsule, tank) = (v.ship().unwrap(), pieces[0].ship().unwrap());
    assert_eq!((capsule.tanks.len(), tank.tanks.len()), (0, 2));
    assert_eq!((capsule.engines.len(), tank.engines.len()), (0, 4));
    assert_eq!(tank.tanks, state.tanks);
    assert!(v.craft.engine.thrust_vac == 0.0 && pieces[0].craft.engine.thrust_vac == 400.0e3);
    assert!(v.crew() == 1 && pieces[0].crew() == 0);
}

/// Saved and loaded (the craft rebuilt from its id), the vessel is the same
/// and flies on the same bits; 60 fps frames equal one jump.
#[test]
fn saves_round_trip_and_frames_equal_one_jump() {
    let w = world();
    let mut v = on_the_pad(&w);
    v.next_stage(&w).unwrap();
    v.advance(&w, t0().add_seconds(1.0), &full(0.7), usize::MAX);
    let mut ids = VesselIds::default();
    assert_eq!(ids.allocate(), v.id());
    let text = SaveGame::capture(&w, v.time, std::slice::from_ref(&v), ids, 0, full(0.7)).to_ron();
    let mut loaded = SaveGame::from_ron(&text).unwrap().vessels.remove(0);
    assert_eq!(loaded, v);
    let mut frames = v.clone();
    let end = t0().add_seconds(4.0);
    v.advance(&w, end, &full(0.7), usize::MAX);
    loaded.advance(&w, end, &full(0.7), usize::MAX);
    assert_eq!(loaded, v);
    let mut t = frames.time;
    while t < end {
        let next = t.add_seconds(1.0 / 60.0);
        t = if next < end { next } else { end };
        frames.advance(&w, t, &full(0.7), usize::MAX);
    }
    assert_eq!(frames, v);
}
