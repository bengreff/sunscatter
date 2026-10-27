//! The Falcon 9 Crew flown by its own sequence (ship-model.md §11's
//! scenarios): orders at the real timeline's epochs, applied as the game
//! applies them (breakups split as they come).

use sim::craft::state::EngineStatus;
use sim::craft::{craft_of, CraftId};
use sim::ephem::Ephemeris;
use sim::sol;
use sim::systems::OrderKind;
use sim::time::Epoch;
use sim::vessel::{Controls, Vessel, VesselId, VesselIds};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

/// Flies `v` to `until` applying its breakups as the game does; returns the
/// pieces that left it.
fn fly(w: &World, v: &mut Vessel, until: Epoch, controls: &Controls, ids: &mut VesselIds) -> Vec<Vessel> {
    let mut pieces = Vec::new();
    loop {
        v.advance(w, until, controls, usize::MAX);
        match v.plan_breakup() {
            Some(plan) => pieces.extend(v.split(w, &plan, &plan.crafts(), ids)),
            None => return pieces,
        }
    }
}

/// The real timeline (falcon9.md §4.3, held vertical): ignition, liftoff at
/// full thrust, MECO at T+145 s, stage separation 3 s later, the second
/// stage's ignition 8 s after MECO. The pushers and MECO's few seconds of
/// coasting leave the second stage's propellant on its outlets: the MVac
/// lights at once and runs, without debug mode; the booster leaves with its
/// nine engines.
#[test]
fn the_mvac_lights_after_meco_like_the_real_one() {
    let w = world();
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    let mut v = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft);
    let mut ids = VesselIds::default();
    ids.allocate();
    let go = t0().add_seconds(1.0);
    let meco = go.add_seconds(2.0 + 145.0);
    for at in [go, go, meco, meco.add_seconds(3.0), meco.add_seconds(8.0)] {
        v.order_at(at, OrderKind::Stage).unwrap();
    }
    let controls = Controls { throttle: 1.0, sas: true, ..Default::default() };
    let mut pieces = fly(&w, &mut v, meco.add_seconds(14.0), &controls, &mut ids);
    let events: Vec<String> = v.ship_events().iter().map(|e| e.text.clone()).collect();
    println!("{events:#?}");
    assert!(!events.iter().any(|e| e.contains("ingestion") || e.contains("abandoned")), "{events:?}");
    let def = v.ship_def().unwrap();
    let mvac = def.engines.iter().position(|e| e.id == "s2.mvac").expect("the upper stage is the vessel");
    assert_eq!(v.ship().unwrap().engines[mvac].status, EngineStatus::Running);
    let thrust = v.engine_thrusts(1.0, 0.0).unwrap()[mvac];
    assert!((thrust - 981.0e3).abs() < 1.0, "{thrust}");
    let booster = pieces.iter_mut().find(|p| p.ship().is_some_and(|s| s.engines.len() == 9)).expect("the booster");
    assert!(booster.controllable(), "the booster flies on");
}
