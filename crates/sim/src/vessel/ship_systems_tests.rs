//! The ship systems in flight (ship-model.md §5, §11): orders at epochs,
//! the Falcon 9's pad (held ticks, the clamps' load, the release at full
//! thrust, a pad abort, a manual release), and the same orders in frames
//! and in one jump.

use super::*;
use crate::craft::command::ShipCommand;
use crate::craft::state::EngineStatus;
use crate::craft::{craft_of, registry, CraftId};
use crate::ephem::Ephemeris;
use crate::sol;
use crate::systems::propulsion::{self, Stop};
use crate::systems::OrderKind;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

fn full() -> Controls {
    Controls { throttle: 1.0, sas: true, ..Default::default() }
}

fn falcon9(w: &World) -> Vessel {
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    Vessel::landed_at(w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft)
}

fn rocket(w: &World) -> Vessel {
    let id = CraftId("test-rocket".into());
    registry::register(&id, crate::craft::build_tests::built(crate::craft::build_tests::CRAFT).unwrap().def);
    Vessel::landed_at(w, VesselId(1), "Earth", 28.6, -80.6, t0(), &craft_of(&id))
}

fn statuses(v: &Vessel) -> Vec<EngineStatus> {
    v.ship().unwrap().engines.iter().map(|e| e.status).collect()
}

/// The real pad's rule: the sequence's ignition at T and its liftoff at
/// T + 0.5 s (orders at epochs): the clamps hold while the nine Merlins
/// spool up over their 2 s start, carrying thrust less weight, and let go
/// at the first tick all nine deliver their rated thrust.
#[test]
fn the_clamps_release_only_when_all_nine_run_at_rated_thrust() {
    let w = world();
    let mut v = falcon9(&w);
    let ignition = t0().add_seconds(1.0);
    v.order_at(ignition, OrderKind::Stage).unwrap();
    v.order_at(ignition.add_seconds(0.5), OrderKind::Stage).unwrap();
    v.advance(&w, ignition.add_seconds(1.0), &full(), usize::MAX);
    assert!(v.clamped() && matches!(v.phase, Phase::Landed { .. }));
    assert!(statuses(&v)[1..10].iter().all(|s| *s == EngineStatus::Starting), "the Merlins: {:?}", statuses(&v));
    assert!(v.ship().unwrap().runtime.release_armed);
    // Half way: the clamps hold the stage down against less than its
    // weight: no load yet pulls on them (they push up, the vehicle rests).
    let hold = |v: &Vessel| {
        let def = v.ship_def().unwrap();
        let j = def.joints.iter().position(|j| j.name == "hold-down clamps").unwrap();
        v.ship().unwrap().runtime.joints[j].clone()
    };
    let up = |v: &Vessel| {
        let (_, r, _) = v.state(&w);
        v.attitude.q.inverse() * r.normalize()
    };
    let mid = hold(&v);
    assert!(mid.force.dot(up(&v)) < 0.0, "the vehicle still weighs on the mount: {:?}", mid.force);
    v.advance(&w, ignition.add_seconds(1.98), &full(), usize::MAX);
    assert!(v.clamped(), "not before full thrust");
    let at_full = hold(&v);
    // At full thrust: 9 × (914 − 101.3·0.681) kN ≈ 7.6 MN against ~5.5 MN.
    v.advance(&w, ignition.add_seconds(2.1), &full(), usize::MAX);
    assert!(!v.clamped(), "released at full thrust");
    // In the tick that ends 2 s after ignition (events carry a tick's start).
    let released = v.ship_events().iter().find(|e| e.text.starts_with("Liftoff")).unwrap().at;
    let after = released.seconds_since(ignition);
    assert!((1.97..2.0).contains(&after), "released in the tick from {after} s after ignition");
    assert!(v.plan_breakup().is_some(), "the mount separates");
    println!("clamp load at full thrust {:.2} MN (ratio {:.3})", at_full.force.length() / 1e6, at_full.ratio);
    assert!(at_full.force.dot(up(&v)) > 1.5e6, "pulled up by the engines: {:?}", at_full.force);
    assert!(at_full.ratio > 0.0 && at_full.ratio < 1.0);
}

/// A Merlin that fails during the start aborts the launch: every engine
/// shut down, the clamps kept; by hand, the clamps let go at once.
#[test]
fn a_pad_abort_and_a_manual_release() {
    let w = world();
    let mut v = falcon9(&w);
    v.next_stage(&w).unwrap();
    v.next_stage(&w).unwrap();
    v.advance(&w, t0().add_seconds(1.0), &full(), usize::MAX);
    let def = v.ship_def().unwrap();
    let e3 = def.engines.iter().position(|e| e.id == "s1.merlin.3").unwrap();
    propulsion::stop(&def, v.ship.as_mut().unwrap(), e3, Stop::Fail("forced".into()), v.time);
    v.advance(&w, t0().add_seconds(3.0), &full(), usize::MAX);
    assert!(v.clamped(), "held");
    let abort = v.ship_events().iter().find(|e| e.text.starts_with("Pad abort")).expect("an abort");
    assert!(abort.alarm && abort.text.contains("s1.merlin.3"), "{}", abort.text);
    assert!(!v.ship().unwrap().runtime.release_armed);
    assert!(statuses(&v)
        .iter()
        .all(|s| matches!(s, EngineStatus::ShuttingDown | EngineStatus::Off | EngineStatus::Failed)));
    v.advance(&w, t0().add_seconds(5.0), &full(), usize::MAX);
    assert!(statuses(&v).iter().all(|s| matches!(s, EngineStatus::Off | EngineStatus::Failed)));
    // The player's choice: released by hand, at once, engines or not.
    v.command(&w, &ShipCommand::ReleaseClamps).unwrap();
    assert!(!v.clamped() && v.plan_breakup().is_some());
}

/// Orders at epochs (an ignition, a throttle, a shutdown) give the same
/// flight in 60 fps frames as in one jump; an order sent with a light
/// delay acts at its arrival's tick.
#[test]
fn orders_at_epochs_fly_the_same_in_frames_and_in_one_jump() {
    let w = world();
    let mut v = rocket(&w);
    let go = t0().add_seconds(0.37);
    v.order_at(go, OrderKind::Stage).unwrap();
    v.order_at(go.add_seconds(2.5), OrderKind::Command(ShipCommand::Throttle("engine#*".into(), 0.6))).unwrap();
    // From mission control 1.28 s away (the Moon's light time).
    v.order_at(go.add_seconds(4.0 + 1.28), OrderKind::Command(ShipCommand::Shutdown("engine#*".into()))).unwrap();
    let mut frames = v.clone();
    let end = t0().add_seconds(8.0);
    v.advance(&w, end, &full(), usize::MAX);
    let mut t = frames.time;
    while t < end {
        let next = t.add_seconds(1.0 / 60.0);
        t = if next < end { next } else { end };
        frames.advance(&w, t, &full(), usize::MAX);
    }
    assert_eq!(frames, v);
    let events: Vec<_> = v.ship_events().iter().map(|e| (e.at.seconds_since(go), e.text.clone())).collect();
    assert!(events[0].0 == 0.0 && events[0].1 == "Stage: Ignition", "{events:?}");
    let s = v.ship().unwrap();
    assert!(s.engines.iter().all(|e| e.status == EngineStatus::Off), "shut down by the delayed order");
    assert!(s.engines.iter().all(|e| e.throttle == 0.6));
}

/// The engines' draw is the tanks' loss (the invariant): ten seconds after
/// liftoff at full thrust the nine Merlins have drawn 9 × 299.7 kg/s from
/// the first stage's tanks by their mixture ratio, nothing else; at that
/// rate its 411 t last ~152 s (falcon9.md §4.3: "152 s at full flow empties
/// the stage").
#[test]
fn the_first_stage_draws_its_engines_flow_and_nothing_else() {
    let w = world();
    let mut v = falcon9(&w);
    v.order_at(t0(), OrderKind::Stage).unwrap();
    v.order_at(t0(), OrderKind::Stage).unwrap();
    v.advance(&w, t0().add_seconds(2.1), &full(), usize::MAX);
    let plan = v.plan_breakup().expect("the mount separates");
    let mut ids = VesselIds::default();
    ids.allocate();
    v.split(&w, &plan, &plan.crafts(), &mut ids);
    let def = v.ship_def().unwrap();
    let tank = |id: &str| def.tanks.iter().position(|t| t.id == id).unwrap();
    let (lox, rp1) = (tank("s1.tank:lox"), tank("s1.tank:rp1"));
    let before = v.ship().unwrap().clone();
    let start = v.time;
    v.advance(&w, start.add_seconds(10.0), &full(), usize::MAX);
    let s = v.ship().unwrap();
    let drawn = |i: usize| before.tanks[i].liquid - s.tanks[i].liquid;
    let flow = 9.0 * 914.0e3 / (311.0 * G0);
    let seconds = v.time.seconds_since(start);
    println!("{:.0} kg in {seconds:.2} s: {:.1} kg/s", drawn(lox) + drawn(rp1), (drawn(lox) + drawn(rp1)) / seconds);
    assert!(((drawn(lox) + drawn(rp1)) / (flow * seconds) - 1.0).abs() < 0.005);
    assert!((drawn(lox) / drawn(rp1) - 2.33).abs() < 1e-6, "by the mixture ratio");
    let others: f64 = (0..def.tanks.len()).filter(|&i| i != lox && i != rp1).map(|i| drawn(i).abs()).sum();
    assert!(others < 1.0, "the other tanks keep theirs: {others} kg");
    let load = before.tanks[lox].liquid + before.tanks[rp1].liquid;
    assert!((load / flow - 152.0).abs() < 3.0, "a full-flow burn of {} s", load / flow);
}

/// A diagnostic flight (run by hand): the Falcon 9 from the pad at full
/// throttle, held vertical, printed every 10 s.
#[test]
#[ignore]
fn falcon9_ascent_profile() {
    let w = world();
    let mut v = falcon9(&w);
    v.order_at(t0(), OrderKind::Stage).unwrap();
    v.order_at(t0(), OrderKind::Stage).unwrap();
    v.advance(&w, t0().add_seconds(2.1), &full(), usize::MAX);
    let plan = v.plan_breakup().expect("the mount separates");
    let mut ids = VesselIds::default();
    ids.allocate();
    v.split(&w, &plan, &plan.crafts(), &mut ids);
    let def = v.ship_def().unwrap();
    let s1: Vec<usize> = (0..def.tanks.len()).filter(|&i| def.tanks[i].id.starts_with("s1.")).collect();
    let s1_load: f64 = s1.iter().map(|&i| v.ship().unwrap().tanks[i].liquid).sum();
    let start = v.time;
    for k in 1..=16 {
        v.advance(&w, start.add_seconds(10.0 * k as f64), &full(), usize::MAX);
        let (anchor, r, vel) = v.state(&w);
        let alt = crate::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0;
        let s = v.ship().unwrap();
        let left: f64 = s1.iter().map(|&i| s.tanks[i].liquid).sum();
        let worst = s.runtime.joints.iter().map(|j| j.ratio).fold(0.0, f64::max);
        println!(
            "T+{:>3.0} s alt {:>7.0} m speed {:>6.1} m/s mass {:>7.0} kg s1 {:>5.1} % drawn {:>7.0} kg worst joint {:.2} accel {:.1}",
            v.time.seconds_since(start),
            alt,
            vel.length(),
            v.mass(),
            100.0 * left / s1_load,
            s1_load - left,
            worst,
            s.runtime.accel.length()
        );
        if v.plan_breakup().is_some() {
            println!("broke up: {:?}", v.ship_events().iter().rev().take(3).collect::<Vec<_>>());
            break;
        }
    }
}

/// The second stage with Dragon in orbit after its first burn (tanks at
/// 10 %, ullages held at 3.5 bar), coasting 20 minutes: its propellant
/// floats. Commanded to start, the MVac's start sequence fires the two
/// nitrogen pods' aft nozzles until the propellant is settled, then lights
/// and runs; commanded twice (start now), it lights into floating
/// propellant and the start is aborted by gas ingestion, its ignition spent.
#[test]
fn a_restart_after_a_coast_settles_first_and_ingests_gas_when_forced() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let (r, vel) = crate::kepler::Elements { a: 6_578_137.0, e: 0.0, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }
        .to_state(earth.gm);
    let craft = craft_of(&CraftId("falcon9-crew#0-11".into()));
    let fresh = || {
        let mut v = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, vel, &craft);
        let def = v.ship_def().unwrap();
        let s = v.ship.as_mut().unwrap();
        for (t, st) in def.tanks.iter().zip(s.tanks.iter_mut()).filter(|(t, _)| t.id.starts_with("s2.")) {
            st.liquid *= 0.1;
            let g = crate::systems::plumbing::geometry(t, st);
            st.ullage_moles = crate::fluids::moles(t.load.pressure, g.ullage, st.ullage_k);
        }
        v.propellant = s.consumables(&def);
        v
    };
    let ignite = ShipCommand::Ignite("s2".into());
    let mvac = |v: &Vessel| v.ship_def().unwrap().engines.iter().position(|e| e.id == "s2.mvac").unwrap();
    // Settled by the start sequence.
    let mut v = fresh();
    let coast = t0().add_seconds(1_200.0);
    v.advance(&w, coast, &Controls::default(), usize::MAX);
    let i = mvac(&v);
    assert!(!crate::systems::propulsion::feeds_settled(&v.ship_def().unwrap(), v.ship().unwrap(), i), "floating");
    let n2 = |v: &Vessel| v.ship().unwrap().bottles.iter().map(|b| b.moles).sum::<f64>();
    let gas0 = n2(&v);
    v.order_at(coast, OrderKind::Command(ignite.clone())).unwrap();
    v.advance(&w, coast.add_seconds(30.0), &full(), usize::MAX);
    let events: Vec<String> = v.ship_events().iter().map(|e| e.text.clone()).collect();
    println!("{events:#?}");
    assert!(events.iter().any(|e| e.contains("settled after")), "{events:?}");
    assert_eq!(v.ship().unwrap().engines[i].status, EngineStatus::Running);
    assert_eq!(v.ship().unwrap().engines[i].ignitions, 2, "one charge spent");
    assert!(n2(&v) < gas0, "the pods fired");
    // Forced: lit into floating propellant.
    let mut v = fresh();
    v.advance(&w, coast, &Controls::default(), usize::MAX);
    v.order_at(coast, OrderKind::Command(ignite.clone())).unwrap();
    v.order_at(coast, OrderKind::Command(ignite)).unwrap();
    v.advance(&w, coast.add_seconds(5.0), &full(), usize::MAX);
    let s = v.ship().unwrap();
    let events: Vec<String> = v.ship_events().iter().map(|e| e.text.clone()).collect();
    assert!(events.iter().any(|e| e.contains("start aborted: gas ingestion")), "{events:?}");
    assert_eq!((s.engines[i].status, s.engines[i].ignitions), (EngineStatus::Off, 2));
}

/// A diagnostic gravity turn (run by hand): pitched by the pilot at T+12 s
/// for 4 s, then held surface-prograde with SAS, printed every 5 s: angle of
/// attack, the worst joint, the gimbal's deflection.
#[test]
#[ignore]
fn falcon9_gravity_turn_profile() {
    use super::hold::{HoldMode, SpeedReference};
    let w = world();
    let mut v = falcon9(&w);
    v.order_at(t0(), OrderKind::Stage).unwrap();
    v.order_at(t0(), OrderKind::Stage).unwrap();
    v.advance(&w, t0().add_seconds(2.1), &full(), usize::MAX);
    let plan = v.plan_breakup().expect("the mount separates");
    let mut ids = VesselIds::default();
    ids.allocate();
    v.split(&w, &plan, &plan.crafts(), &mut ids);
    let start = v.time;
    let earth = w.find("Earth").unwrap().node;
    let mut worst_all: f64 = 0.0;
    for k in 1..=40 {
        let t = start.add_seconds(2.5 * k as f64);
        let s = t.seconds_since(start);
        let controls = if (10.0..12.5).contains(&s) {
            Controls { throttle: 1.0, sas: false, rotate: DVec3::new(0.04, 0.0, 0.0), ..Default::default() }
        } else if s >= 12.5 {
            Controls {
                throttle: 1.0,
                sas: true,
                hold: HoldMode::Prograde,
                speed: SpeedReference::Surface,
                ..Default::default()
            }
        } else {
            full()
        };
        v.advance(&w, t, &controls, usize::MAX);
        if v.plan_breakup().is_some() {
            println!("broke up at T+{s}: {:?}", v.ship_events().iter().rev().take(2).collect::<Vec<_>>());
            break;
        }
        let (anchor, r, vel) = v.state(&w);
        let rot = w.source(earth).and_then(|s| s.physical.as_ref()).unwrap().rotation;
        let surf = vel - rot.omega(v.time).raw().cross(r);
        let nose = v.attitude.nose();
        let alpha = crate::math::acos(nose.dot(surf.normalize()).clamp(-1.0, 1.0)).to_degrees();
        let pitch = crate::math::acos(nose.dot(r.normalize()).clamp(-1.0, 1.0)).to_degrees();
        let st = v.ship().unwrap();
        let worst = st.runtime.joints.iter().map(|j| j.ratio).fold(0.0, f64::max);
        worst_all = worst_all.max(worst);
        let gimbal =
            st.runtime.thrust_dir.map_or(0.0, |d| crate::math::acos(d.normalize().z.clamp(-1.0, 1.0)).to_degrees());
        let alt = crate::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0;
        println!(
            "T+{s:>5.1} alt {alt:>6.0} m surf {:>6.1} m/s pitch {pitch:>5.1}° α {alpha:>5.2}° gimbal {gimbal:>4.2}° ω {:.4} worst joint {worst:.2}",
            surf.length(),
            v.attitude.omega.length()
        );
    }
    println!("worst joint over the flight {worst_all:.2}");
}

/// The booster (`falcon9-crew#12-31`) at `alt` m over the sea, falling at
/// `speed` m/s engines first (nose up), its first-stage tanks at `fill`.
fn booster(w: &World, alt: f64, speed: f64, fill: f64) -> Vessel {
    booster_as(w, CraftId("falcon9-crew#12-31".into()), alt, speed, fill)
}

/// As [`booster`], in configuration `id` (its mechanisms at their deployed
/// angles where the id says so).
fn booster_as(w: &World, id: CraftId, alt: f64, speed: f64, fill: f64) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let craft = craft_of(&id);
    let deg = crate::math::PI / 180.0;
    let fixed = p.ground_point(28.0 * deg, -75.0 * deg, alt);
    let rot = p.rotation;
    let r = rot.to_inertial(fixed, t0()).raw();
    let up = r.normalize();
    let v = rot.omega(t0()).raw().cross(r) - up * speed;
    let mut b = Vessel::coasting(w, VesselId(2), t0(), earth.node, r, v, &craft);
    b.set_attitude(Attitude { q: super::attitude::quat_z_to(up), omega: DVec3::ZERO });
    let def = b.ship_def().unwrap();
    let s = b.ship.as_mut().unwrap();
    for (t, st) in def.tanks.iter().zip(s.tanks.iter_mut()) {
        st.liquid *= fill;
        let g = crate::systems::plumbing::geometry(t, st);
        st.ullage_moles = crate::fluids::moles(t.load.pressure, g.ullage, st.ullage_k);
    }
    for (d, m) in def.mechanisms.iter().zip(s.mechanisms.iter_mut()) {
        if id.deployed().contains(&d.part) {
            (m.angle, m.target) = (d.item.deployed_deg, d.item.deployed_deg);
        }
    }
    b.propellant = s.consumables(&def);
    b
}

/// A diagnostic descent (run by hand): the booster at 12 km falling at
/// 300 m/s engines first, grid fins deployed or not, SAS holding surface
/// retrograde after a 5° upset; printed every second.
#[test]
#[ignore]
fn booster_grid_fin_descent_profile() {
    use super::hold::{HoldMode, SpeedReference};
    let w = world();
    for deploy in [false, true] {
        let mut v = booster(&w, 12_000.0, 300.0, 0.05);
        v.set_attitude(Attitude {
            q: v.attitude.q * super::attitude::quat_from_rotvec(DVec3::X * 0.09),
            omega: DVec3::ZERO,
        });
        if deploy {
            v.command(&w, &ShipCommand::Deploy("s1.gridfin#*".into())).unwrap();
        }
        let hold =
            Controls { sas: true, hold: HoldMode::Retrograde, speed: SpeedReference::Surface, ..Default::default() };
        let mut ids = VesselIds::default();
        ids.allocate();
        ids.allocate();
        let earth = w.find("Earth").unwrap().node;
        let start = v.time;
        for k in 1..=30 {
            v.advance(&w, start.add_seconds(k as f64), &hold, usize::MAX);
            if let Some(plan) = v.plan_breakup() {
                v.split(&w, &plan, &plan.crafts(), &mut ids);
            }
            let (anchor, r, vel) = v.state(&w);
            let rot = w.source(earth).and_then(|s| s.physical.as_ref()).unwrap().rotation;
            let surf = vel - rot.omega(v.time).raw().cross(r);
            let off = crate::math::acos((-v.attitude.nose()).dot(surf.normalize()).clamp(-1.0, 1.0)).to_degrees();
            let alt = crate::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0;
            let fins = v.ship().unwrap().runtime.fins.iter().map(|d| d.to_degrees()).collect::<Vec<_>>();
            if k % 3 == 0 {
                println!(
                    "fins {deploy} T+{k} alt {alt:.0} speed {:.0} off-retrograde {off:.2}° ω {:.4} fins {fins:.1?}",
                    surf.length(),
                    v.attitude.omega.length()
                );
            }
        }
    }
}

/// The booster's landing burn on one engine (falcon9.md §4.4): from 2 km
/// falling at 150 m/s engines first, grid fins and legs out, the centre
/// Merlin lit by the "Booster: one engine" function and throttled by a
/// simple hoverslam rule (the deceleration that stops it at the ground,
/// within the engine's 40–100 %), SAS on retrograde, then holding upright:
/// it touches down on its four legs, intact, and comes to rest. It cannot
/// hover (40 % of a Merlin is more than its weight: falcon9.md §4.4), so
/// the engine is cut at touchdown. Ignored until the structure's leg joints
/// carry a landing (their ring-bending ultimate of ~128 kN snaps them at
/// touchdown; the dynamics land it at 1.1 m/s).
#[test]
#[ignore]
fn the_booster_lands_on_one_engine() {
    use super::hold::{HoldMode, SpeedReference};
    let w = world();
    let id = CraftId("falcon9-crew#12-31".into()).with_deployed(&[13, 14, 15, 16, 28, 29, 30, 31]);
    let mut v = booster_as(&w, id, 2_000.0, 150.0, 0.03);
    let earth = w.find("Earth").unwrap();
    let rot = earth.physical.as_ref().unwrap().rotation;
    let g = 9.81;
    let mut lit = false;
    let mut t = v.time;
    let mut touchdown = None;
    for _ in 0..1200 {
        t = t.add_seconds(0.1);
        let (anchor, r, vel) = v.state(&w);
        let surface = vel - rot.omega(v.time).raw().cross(r);
        let up = r.normalize();
        let vz = surface.dot(up);
        let h = crate::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0 - v.contact_height();
        let mass = v.mass();
        // The thrust the engine gives at full here (sea level ~845 kN).
        let full = 845.0e3;
        let need = (vz * vz / (2.0 * (h - 1.0).max(0.5)) + g) * mass / full;
        if !lit && need > 0.85 {
            v.command(&w, &ShipCommand::Select("s1.one".into())).unwrap();
            v.command(&w, &ShipCommand::Ignite("s1.one".into())).unwrap();
            lit = true;
        }
        let touching = matches!(v.phase, Phase::Landed { .. }) || (h < 0.3 && vz.abs() < 3.0);
        if lit && touching && touchdown.is_none() {
            touchdown = Some((vz, h));
            v.command(&w, &ShipCommand::Shutdown("s1.one".into())).unwrap();
        }
        let hold = if surface.length() > 20.0 { HoldMode::Retrograde } else { HoldMode::Stability };
        let controls = Controls {
            throttle: need.clamp(0.0, 1.0),
            sas: true,
            hold,
            speed: SpeedReference::Surface,
            ..Default::default()
        };
        v.advance(&w, t, &controls, usize::MAX);
        if matches!(v.phase, Phase::Landed { .. } | Phase::Crashed { .. }) || v.plan_breakup().is_some() {
            break;
        }
    }
    let t0b = v.ship().unwrap().runtime.launch.unwrap();
    let events: Vec<String> =
        v.ship_events().iter().map(|e| format!("{:.2} {}", e.at.seconds_since(t0b), e.text)).collect();
    println!(
        "touchdown {touchdown:?}, {:?}, destroyed {:?}, events {events:#?}",
        std::mem::discriminant(&v.phase),
        v.destruction()
    );
    assert!(lit, "the engine lit");
    assert!(matches!(v.phase, Phase::Landed { .. }), "at rest, intact");
    assert!(v.destruction().is_none() && v.plan_breakup().is_none());
    let def = v.ship_def().unwrap();
    let e9 = def.engines.iter().position(|e| e.id == "s1.merlin.9").unwrap();
    assert_eq!(v.ship().unwrap().engines[e9].ignitions, 2, "one relight used");
}

/// Boil-off over a day on rails (D081; ship-model.md §5.10): the second
/// stage with Dragon coasting a day in a 400 km orbit. Its densified LOX
/// warms in the sunlight its wetted wall absorbs (and stays below its
/// saturation at 4 bar: it does not boil, falcon9.md §3.6); the same stage
/// with its LOX already at saturation at its relief pressure boils off what
/// the sunlight brings, vented. Advanced in one jump or in uneven frames,
/// the vessel is the same bits.
#[test]
fn a_day_on_rails_warms_and_boils_off_the_same_in_any_frames() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let (r, vel) =
        crate::kepler::Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }
            .to_state(earth.gm);
    let craft = craft_of(&CraftId("falcon9-crew#0-11".into()));
    let def = craft.ship.clone().unwrap();
    let lox = def.tanks.iter().position(|t| t.id == "s2.tank:lox").unwrap();
    let day = t0().add_seconds(86_400.0);
    for saturated in [false, true] {
        let mut v = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, vel, &craft);
        if saturated {
            let s = v.ship.as_mut().unwrap();
            let l = crate::systems::plumbing::liquid_of(&def.tanks[lox]).unwrap();
            s.tanks[lox].liquid_k = l.saturation_temperature(def.tanks[lox].pressure);
            s.tanks[lox].ullage_moles = 0.0;
            v.propellant = s.consumables(&def);
        }
        let (k0, m0) = (v.ship().unwrap().tanks[lox].liquid_k, v.mass());
        let mut frames = v.clone();
        v.advance(&w, day, &Controls::default(), usize::MAX);
        let mut t = frames.time;
        let mut k = 0;
        while t < day {
            k += 1;
            t = t.add_seconds(if k % 3 == 0 { 777.7 } else { 1_234.5 });
            if t > day {
                t = day;
            }
            frames.advance(&w, t, &Controls::default(), usize::MAX);
        }
        assert_eq!(frames, v, "saturated {saturated}");
        let s = v.ship().unwrap();
        let (dk, vented) = (s.tanks[lox].liquid_k - k0, s.runtime.tanks[lox].vented);
        println!(
            "saturated {saturated}: LOX {k0:.1} K → {:+.2} K, vented {vented:.1} kg, mass {:+.1} kg",
            dk,
            v.mass() - m0
        );
        if saturated {
            assert!(vented > 1.0 && dk.abs() < 0.5, "boils at its saturation: {vented} kg");
            let all: f64 = s.runtime.tanks.iter().map(|t| t.vented).sum();
            assert!((m0 - v.mass() - all).abs() < 1e-6 * m0, "the vessel loses what its tanks vent: {all} kg");
        } else {
            // It vents only a little pressurant as the warming liquid
            // squeezes its ullage.
            assert!(dk > 0.1 && vented < 5.0, "warms, does not boil: {dk} K, {vented} kg");
        }
    }
}

/// The same flight at 1× (60 fps frames) and in one jump, bit for bit, with
/// a failure firing identically (D105 §6.3, rule 4): the Falcon 9 from the
/// pad by its sequence's orders, one Merlin's in-flight-shutdown fate made
/// near (its 2.2e-3/h hazard reaches a threshold of 1e-6 in ~1.6 s of
/// running, 3e-6 in ~4.9 s). Near, it fails while the engines spool up on
/// the pad: the release is aborted, the clamps held. Later, it shuts down
/// benignly after liftoff and the stack flies on eight (engine out). Both
/// at the same tick in frames and in one jump.
#[test]
fn a_failure_fires_the_same_in_frames_and_in_one_jump() {
    let w = world();
    for (threshold, airborne) in [(1e-6, false), (3e-6, true)] {
        let mut v = falcon9(&w);
        let s = v.ship.as_mut().unwrap();
        let fate =
            s.fates.iter_mut().find(|f| f.owner == "s1.merlin.3" && f.mode == "in-flight shutdown").expect("its fate");
        fate.threshold = threshold;
        v.order_at(t0().add_seconds(0.5), OrderKind::Stage).unwrap();
        v.order_at(t0().add_seconds(0.5), OrderKind::Stage).unwrap();
        let mut frames = v.clone();
        let end = t0().add_seconds(8.0);
        let mut ids = VesselIds::default();
        ids.allocate();
        let fly = |v: &mut Vessel, until: Epoch, ids: &mut VesselIds| loop {
            v.advance(&w, until, &full(), usize::MAX);
            match v.plan_breakup() {
                Some(plan) => {
                    v.split(&w, &plan, &plan.crafts(), ids);
                }
                None => return,
            }
        };
        fly(&mut v, end, &mut ids.clone());
        let mut t = frames.time;
        while t < end {
            let next = t.add_seconds(1.0 / 60.0);
            t = if next < end { next } else { end };
            fly(&mut frames, t, &mut ids.clone());
        }
        assert_eq!(frames, v);
        let def = v.ship_def().unwrap();
        let e3 = def.engines.iter().position(|e| e.id == "s1.merlin.3").unwrap();
        let s = v.ship().unwrap();
        let events: Vec<String> = v.ship_events().iter().map(|e| e.text.clone()).collect();
        assert_eq!(s.engines[e3].status, EngineStatus::Failed, "{events:?}");
        let running = (def.engines.iter().zip(&s.engines))
            .filter(|(d, e)| d.id.starts_with("s1.merlin") && e.status == EngineStatus::Running)
            .count();
        if airborne {
            assert!(!v.clamped() && running == 8, "engine out: the other eight run ({running}); {events:?}");
        } else {
            assert!(v.clamped() && running == 0, "a pad abort: {events:?}");
            assert!(events.iter().any(|e| e.starts_with("Pad abort: s1.merlin.3")));
        }
    }
}
