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
/// lowest points. The sequence's ignition starts the four engines (a 1 s
/// start transient, `systems::propulsion`); at full throttle it lifts off, each tank drained by its share of the mixture
/// ratio, the vessel's propellant the ship's consumables, its centre of
/// mass moving as the tanks drain unequally.
#[test]
fn a_craft_of_parts_lifts_off_on_its_engines() {
    let w = world();
    let mut v = on_the_pad(&w);
    assert!(matches!(v.phase, Phase::Landed { .. }), "it stands on the pad");
    let def = v.ship_def().unwrap();
    let start = v.ship().unwrap().clone();
    let mut loaded = ShipState::initial(&def);
    loaded.runtime.launch = Some(t0());
    assert_eq!(start, loaded);
    assert!((v.propellant() - start.consumables(&def)).abs() < 1e-9);
    let com0 = v.mass_props().com;
    let (height0, m0) = (v.contact_height(), v.mass());
    assert!(!v.engine_running(&full(1.0)), "engines off");
    let (name, _) = v.next_stage(&w).unwrap();
    assert_eq!(name, "Ignition");
    assert!(v.ship().unwrap().engines.iter().all(|e| e.status == EngineStatus::Starting));
    assert_eq!(v.engine_thrusts(1.0, 0.0).unwrap(), vec![0.0; 4], "nothing yet");
    v.advance(&w, t0().add_seconds(0.5), &full(1.0), usize::MAX);
    let half = v.engine_thrusts(1.0, 0.0).unwrap()[0];
    assert!(half > 40.0e3 && half < 60.0e3, "half way up its start: {half}");
    v.advance(&w, t0().add_seconds(1.05), &full(1.0), usize::MAX);
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
    v.advance(&w, t0().add_seconds(5.0), &full(1.0), usize::MAX);
    let s = v.ship().unwrap();
    let (fuel, ox) = (start.tanks[0].liquid - s.tanks[0].liquid, start.tanks[1].liquid - s.tanks[1].liquid);
    // By the mixture ratio, but for the LOX vapour its ullage vents: loaded
    // at 3 bar of helium over LOX at 90 K (1 bar of vapour), the tank sits
    // over its 3 bar relief pressure (`systems::plumbing`).
    assert!(fuel > 0.0 && (ox / fuel - 2.4).abs() < 0.01, "by the mixture ratio: {ox} / {fuel}");
    // Four engines' flow at full throttle for the ticks since ignition, less
    // half the 1 s start (the smooth step's mean) and its first tick.
    let mdot = 4.0 * 100.0e3 / (320.0 * G0);
    assert!(((fuel + ox) / mdot - 4.5).abs() < 0.05, "{} s of burn", (fuel + ox) / mdot);
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

/// The Falcon 9 Crew on LC-39A: it stands on the launch mount's posts,
/// fuelled; the sequence's ignition starts the nine Merlins and the clamps
/// hold it welded to the ground; their release leaves the mount as ground
/// (no vessel) and at full throttle (7.6 MN against ~5.5 MN of weight) it
/// lifts off, the booster's LOX and RP-1 going down by the mixture ratio.
#[test]
fn the_falcon_9_stands_on_the_pad_and_lifts_off() {
    let w = world();
    let t = std::time::Instant::now();
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    craft.design();
    let (cells, nodes) = (craft.cells.cells.len(), craft.design().nodes());
    println!("built in {:.1} s: {} parts, {cells} cells, {nodes} nodes", t.elapsed().as_secs_f64(), craft.parts.len());
    let mut v = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft);
    assert!(matches!(v.phase, Phase::Landed { .. }), "it stands on the pad");
    assert!(v.clamped());
    let def = v.ship_def().unwrap();
    let pad: f64 = v.craft.design().parts.iter().filter(|p| p.name.starts_with("pad.")).map(|p| p.dry_mass).sum();
    println!("mass {:.0} kg with the mount's {pad:.0} kg, centre of mass {:.2} m up", v.mass(), v.contact_height());
    assert!(((v.mass() - pad) / 560_200.0 - 1.0).abs() < 0.01);
    assert_eq!(v.craft.engine.thrust_vac, 9.0 * 914.0e3, "the readouts' engine: the nine Merlins");
    assert_eq!(v.next_stage(&w).unwrap().0, "Ignition");
    // Lit from the ground's supply: E1, E5 and E9 keep their three relights.
    let merlins = |x: &Vessel| -> Vec<u32> {
        let def = x.ship_def().unwrap();
        (def.engines.iter().zip(&x.ship().unwrap().engines))
            .filter(|(d, _)| d.owner.starts_with("s1.merlin"))
            .map(|(_, e)| e.ignitions)
            .collect()
    };
    assert_eq!(merlins(&v), [3, 3, 0, 0, 0, 3, 0, 0, 0], "E9, E1…E8");
    let loaded = v.propellant();
    v.advance(&w, t0().add_seconds(2.0), &full(1.0), usize::MAX);
    assert!(matches!(v.phase, Phase::Landed { .. }), "held by the clamps");
    assert!(v.propellant() < loaded - 2_000.0, "the engines burn on the pad: {}", loaded - v.propellant());
    // Liftoff, the real pad's rule: armed, the clamps let go at the first
    // tick all nine deliver their rated thrust; the mount stays as ground.
    assert_eq!(v.next_stage(&w).unwrap().0, "Liftoff");
    assert!(v.clamped(), "armed, not yet released");
    v.advance(&w, v.time.add_seconds(0.1), &full(1.0), usize::MAX);
    assert!(!v.clamped());
    let plan = v.plan_breakup().expect("the mount separates");
    assert_eq!(plan.pieces.len(), 2);
    let mut ids = VesselIds::default();
    ids.allocate();
    assert!(v.split(&w, &plan, &plan.crafts(), &mut ids).is_empty(), "the mount is no vessel");
    assert!(v.craft.design().parts.iter().all(|p| !p.name.starts_with("pad.")));
    let start = v.ship().unwrap().clone();
    let t = std::time::Instant::now();
    let end = v.time.add_seconds(4.0);
    v.advance(&w, end, &full(1.0), usize::MAX);
    println!("4 s of flight in {:.2} s", t.elapsed().as_secs_f64());
    let s = v.ship().unwrap();
    let drop = |i: usize| start.tanks[i].liquid - s.tanks[i].liquid;
    let def = v.ship_def().unwrap_or(def);
    let lox = def.tanks.iter().position(|t| t.id == "s1.tank:lox").unwrap();
    let rp1 = def.tanks.iter().position(|t| t.id == "s1.tank:rp1").unwrap();
    assert!((drop(lox) / drop(rp1) - 2.33).abs() < 1e-9);
    let others: f64 = (0..def.tanks.len()).filter(|&i| i != lox && i != rp1).map(drop).sum();
    assert_eq!(others, 0.0, "the upper stage and Dragon keep theirs");
    let (anchor, r, _) = v.state(&w);
    let up = crate::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0;
    println!("{up:.1} m up after 4 s");
    assert!(up > v.contact_height() + 5.0, "{up}");
    // MECO and stage separation: the booster leaves as a vessel with its
    // engines, fins, legs and functions; the upper stack keeps the rest.
    assert_eq!(v.next_stage(&w).unwrap().0, "MECO");
    assert_eq!(v.next_stage(&w).unwrap().0, "Stage separation");
    let plan = v.plan_breakup().expect("the stages separate");
    let mass = v.mass();
    let (_, r0, v0) = v.state(&w);
    let pieces = v.split(&w, &plan, &plan.crafts(), &mut ids);
    assert_eq!(pieces.len(), 1);
    let booster = &pieces[0];
    let total = v.mass() + booster.mass();
    assert!((total - mass).abs() < 1e-6 * mass, "{total} vs {mass}");
    let momentum = (v.state(&w).2 - v0) * v.mass() + (booster.state(&w).2 - v0) * booster.mass();
    let com = (v.state(&w).1 - r0) * v.mass() + (booster.state(&w).1 - r0) * booster.mass();
    assert!(momentum.length() < 1e-6 * mass && com.length() < 1e-6 * mass);
    let names = |x: &Vessel| x.ship().unwrap().functions.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
    assert!(names(booster).contains(&"Landing legs".to_string()) && !names(booster).contains(&"Nosecone".to_string()));
    assert!(names(&v).contains(&"Nosecone".to_string()) && !names(&v).contains(&"Grid fins".to_string()));
    assert_eq!(booster.ship().unwrap().engines.len(), 9);
    assert!(booster.controllable(), "the booster can be flown");
    assert_eq!(v.craft.engine.thrust_vac, 981.0e3, "the readouts' engine: the MVac");
    assert_eq!(v.next_stage(&w).unwrap().0, "Second stage ignition");
    // Saved and loaded, both pieces are the same.
    let fleet = vec![v.clone(), booster.clone()];
    let text = SaveGame::capture(&w, v.time, &fleet, ids, 0, full(1.0)).to_ron();
    assert_eq!(SaveGame::from_ron(&text).unwrap().vessels, fleet);
}

/// Deploying the legs in flight gives the vessel its legs-out shape (a
/// craft of its own, `base#parts@legs`): the feet are contact points, the
/// centre of mass and the angular momentum stay, the ship state is kept.
#[test]
fn deploying_the_legs_reshapes_the_vessel() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let (r, vel) =
        Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    let mut v = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, vel, &rocket());
    v.set_attitude(Attitude { q: v.attitude.q, omega: DVec3::new(0.02, 0.0, 0.01) });
    v.advance(&w, t0().add_seconds(5.0), &Controls::default(), usize::MAX);
    use crate::craft::command::ShipCommand;
    v.command(&w, &ShipCommand::Deploy("leg#*".into())).unwrap();
    assert!(v.plan_breakup().is_none(), "the legs take their time");
    v.advance(&w, v.time.add_seconds(10.0), &Controls::default(), usize::MAX);
    let plan = v.plan_breakup().expect("a new shape to build");
    assert_eq!(plan.pieces.len(), 1);
    assert!(plan.ids[0].0.starts_with("test-rocket#0-10@"), "{}", plan.ids[0].0);
    let before = v.clone();
    let spin = |x: &Vessel| {
        let rot = glam::DMat3::from_quat(x.attitude.q);
        rot * x.mass_props().inertia * rot.transpose() * x.attitude.omega
    };
    assert!(v.split(&w, &plan, &plan.crafts(), &mut VesselIds::default()).is_empty());
    assert_eq!(v.craft.id, plan.ids[0]);
    let feet = v.craft.contacts.iter().filter(|c| c.kind == crate::craft::ContactKind::Foot).count();
    assert_eq!(feet, 4);
    let ((_, r0, v0), (_, r1, v1)) = (before.state(&w), v.state(&w));
    assert!((r1 - r0).length() < 1e-9 && (v1 - v0).length() < 1e-12, "the centre of mass stays");
    assert!((spin(&v) - spin(&before)).length() < 1e-9 * spin(&before).length(), "angular momentum stays");
    assert!(v.mass_props().inertia.x_axis.x > before.mass_props().inertia.x_axis.x, "legs out: more inertia");
    assert_eq!(v.ship(), before.ship());
    assert_eq!(v.deployed_parts(), plan.ids[0].deployed());
}

/// Stage separation at MECO (~75 km, ~2 km/s): the pushers part the stages
/// at ~0.5 m/s, the MVac starts with its nozzle still in the interstage,
/// and the two fly on in one bubble: the pieces of the split do not touch
/// while they overlap, neither is wrecked, and they are hundreds of metres
/// apart after ten seconds.
#[test]
fn the_falcon_9_stages_separate_without_touching() {
    use crate::craft::command::ShipCommand;
    let w = world();
    let whole = crate::craft::registry::definition(&CraftId("falcon9-crew".into())).unwrap();
    let flying: Vec<u32> = whole.parts.iter().filter(|p| !p.name.starts_with("pad.")).map(|p| p.id).collect();
    let craft = craft_of(&CraftId::piece("falcon9-crew", &flying));
    let earth = w.find("Earth").unwrap();
    let up = DVec3::new(0.3, -0.5, 0.81).normalize();
    let east = DVec3::Z.cross(up).normalize();
    let r = up * (6_378_137.0 + 75_000.0);
    let vel = (east * 0.866 + up * 0.5) * 2_000.0;
    let mut v = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, vel, &craft);
    v.set_attitude(Attitude { q: crate::vessel::quat_z_to(vel.normalize()), omega: DVec3::ZERO });
    // At MECO the booster keeps ~7 % of its load for its return (falcon9.md §4.3).
    let def = v.ship_def().unwrap();
    let ship = v.ship.as_mut().unwrap();
    for (t, d) in ship.tanks.iter_mut().zip(&def.tanks) {
        if d.id.starts_with("s1.") {
            t.liquid *= 0.07;
        }
    }
    v.propellant = ship.consumables(&def);
    v.command(&w, &ShipCommand::Separate("stage separation".into())).unwrap();
    let plan = v.plan_breakup().unwrap();
    let mut ids = VesselIds::default();
    ids.allocate();
    let booster = v.split(&w, &plan, &plan.crafts(), &mut ids).remove(0);
    let rel = booster.state(&w).2 - v.state(&w).2;
    println!("parting at {:.2} m/s", rel.length());
    assert!((rel.length() - 0.5).abs() < 0.15, "{}", rel.length());
    assert!(rel.dot(v.attitude.q * DVec3::Z) < 0.0, "the booster pushed back");
    v.command(&w, &ShipCommand::Select("s2".into())).unwrap();
    v.command(&w, &ShipCommand::Ignite("s2".into())).unwrap();
    let mut fleet = vec![v, booster];
    let controls = |i: usize| Controls { throttle: if i == 0 { 1.0 } else { 0.0 }, sas: true, ..Default::default() };
    let start = fleet[0].time;
    for k in 1..=100 {
        let target = start.add_seconds(0.1 * f64::from(k));
        crate::vessel::bubble::advance(&w, &mut fleet, &controls, target);
        for (i, x) in fleet.iter_mut().enumerate() {
            x.advance(&w, target, &controls(i), usize::MAX);
        }
    }
    for x in &fleet {
        assert!(x.destruction().is_none() && x.breakup().is_none(), "{:?}", x.destruction());
    }
    let gap = (fleet[0].state(&w).1 - fleet[1].state(&w).1).length();
    println!("{gap:.0} m apart after 10 s");
    assert!(gap > 200.0, "{gap}");
    assert!(fleet[0].clear_of.is_empty() && fleet[1].clear_of.is_empty(), "clear of each other");
}

/// The pilot's RCS mode (D111): off, the rotation keys do not fire the
/// thrusters (no torque without them on the pad) and the translation keys
/// nothing; SAS's holds still use them. On, both do. The RCS menu's
/// authority is the same thrusters' (`rcs::authority`).
#[test]
fn the_pilots_rcs_mode_gates_the_thrusters() {
    use crate::systems::rcs::{self, RcsMode};
    let w = world();
    let mut v = on_the_pad(&w);
    let def = v.ship_def().unwrap();
    for i in 0..def.thrusters.len() {
        v.ship.as_mut().unwrap().thrusters[i].enabled = true;
    }
    let props = v.mass_props();
    let dir = (rcs::DIRECTIONS.iter().position(|d| rcs::can_push(&def, v.ship().unwrap(), *d)))
        .expect("a direction the thrusters push");
    let tick = |c: Controls| {
        let c = rcs::pilot(&c);
        let (_, act, ship) = v.actuators(&c, (0.0, None), 0.02, &props);
        (act.torque, ship.unwrap().push.force)
    };
    let on = RcsMode { on: true, ..RcsMode::default() };
    let rotate = Controls { rotate: DVec3::X, ..Controls::default() };
    let translate = Controls { translate: rcs::DIRECTIONS[dir], ..Controls::default() };
    let sas = Controls { sas: true, ..Controls::default() };
    assert_eq!(tick(rotate).0, DVec3::ZERO, "off: the keys do not fire them");
    assert!(tick(Controls { rcs: on, ..rotate }).0.x > 0.0, "on: they do");
    assert!(tick(sas).0.x > 0.0, "SAS's holds use them");
    assert_eq!(tick(translate).1, DVec3::ZERO);
    assert!(tick(Controls { rcs: on, ..translate }).1.dot(rcs::DIRECTIONS[dir]) > 0.0);
    let a = v.rcs_authority().unwrap();
    assert!(a.translate[dir].0 > 0.0 && a.rotate.x > 0.0, "{a:?}");
}
