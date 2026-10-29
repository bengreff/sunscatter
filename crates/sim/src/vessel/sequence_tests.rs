//! The stage sequence on the pad and in flight (D111, D114): the sequence
//! never passes a clamp release that did not let go (a pad abort, the
//! engines stopping, a split), a function never moves it, and editing the
//! stages keeps those already run.

use super::*;
use crate::craft::state::EngineStatus;
use crate::craft::{craft_of, CraftId};
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

/// The index of the stage named `name`.
fn stage(v: &Vessel, name: &str) -> u32 {
    v.ship().unwrap().stages.iter().position(|s| s.name == name).unwrap() as u32
}

fn merlins(v: &Vessel) -> Vec<EngineStatus> {
    let def = v.ship_def().unwrap();
    (def.engines.iter().zip(&v.ship().unwrap().engines))
        .filter(|(d, _)| d.id.starts_with("s1.merlin"))
        .map(|(_, e)| e.status)
        .collect()
}

/// After a pad abort the clamps' stage is next again: Space would run
/// Liftoff, not MECO and the stage separation on the pad (review,
/// 2026-10-27). AGAIN on Ignition relights the Merlins from the pad's
/// supply (D114), Liftoff re-arms the release, and the clamps let go at
/// full thrust.
#[test]
fn after_a_pad_abort_liftoff_is_next_again() {
    let w = world();
    let mut v = falcon9(&w);
    let liftoff = stage(&v, "Liftoff");
    v.next_stage(&w).unwrap();
    v.next_stage(&w).unwrap();
    v.advance(&w, t0().add_seconds(1.0), &full(), usize::MAX);
    let def = v.ship_def().unwrap();
    let e3 = def.engines.iter().position(|e| e.id == "s1.merlin.3").unwrap();
    propulsion::stop(&def, v.ship.as_mut().unwrap(), e3, Stop::Abort("forced".into()), v.time);
    v.advance(&w, t0().add_seconds(6.0), &full(), usize::MAX);
    assert!(v.clamped() && v.ship_events().iter().any(|e| e.text.starts_with("Pad abort")));
    assert!(merlins(&v).iter().all(|s| *s == EngineStatus::Off), "{:?}", merlins(&v));
    assert_eq!(v.ship().unwrap().sequence, liftoff, "Liftoff is next again");
    let t = v.time;
    v.order_at(t, OrderKind::Again(stage(&v, "Ignition"))).unwrap();
    v.advance(&w, t.add_seconds(0.5), &full(), usize::MAX);
    assert!(merlins(&v).iter().all(|s| *s == EngineStatus::Starting), "relit: {:?}", merlins(&v));
    v.order_at(v.time, OrderKind::Stage).unwrap();
    v.advance(&w, t.add_seconds(1.0), &full(), usize::MAX);
    let s = v.ship().unwrap();
    assert!(s.runtime.release_armed && s.sequence == liftoff + 1);
    v.advance(&w, t.add_seconds(3.0), &full(), usize::MAX);
    assert!(!v.clamped(), "released at full thrust: {:?}", v.ship_events().last());
}

/// The "Launch abort" function while the release is armed on the pad: the
/// function does not move the sequence; the separation leaves the stack
/// clamped with its release disarmed (an event says so) and Liftoff next
/// again, and the capsule, held by nothing, is not clamped.
#[test]
fn a_launch_abort_on_the_pad_leaves_liftoff_next() {
    let w = world();
    let mut v = falcon9(&w);
    v.next_stage(&w).unwrap();
    v.next_stage(&w).unwrap();
    v.advance(&w, t0().add_seconds(1.0), &full(), usize::MAX);
    let sequence = v.ship().unwrap().sequence;
    let at = v.time;
    v.order_at(at, OrderKind::Function("Launch abort".into())).unwrap();
    v.advance(&w, at.add_seconds(0.1), &full(), usize::MAX);
    assert_eq!(v.ship().unwrap().sequence, sequence, "a function never moves the sequence");
    let plan = v.plan_breakup().expect("the capsule separates");
    let mut ids = VesselIds::default();
    ids.allocate();
    let mut pieces = v.split(&w, &plan, &plan.crafts(), &mut ids);
    pieces.push(v);
    let (held, free): (Vec<&Vessel>, Vec<&Vessel>) = pieces.iter().partition(|p| p.clamped());
    assert_eq!(held.len(), 1, "the stack stays on the pad");
    let stack = held[0];
    let s = stack.ship().unwrap();
    assert!(!s.runtime.release_armed);
    assert_eq!(s.stages[s.sequence as usize].name, "Liftoff", "Liftoff is next again");
    assert!(s.runtime.events.iter().any(|e| e.text.starts_with("Clamps: release disarmed")), "{:?}", s.runtime.events);
    for p in free {
        assert!(!p.ship().unwrap().clamped, "{:?}: nothing holds it", p.craft.id);
    }
}
