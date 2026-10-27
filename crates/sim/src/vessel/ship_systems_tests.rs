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
