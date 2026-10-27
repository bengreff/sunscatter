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

/// The real timeline (falcon9.md §4.3) flown by hand as the owner will:
/// the sequence's orders at their epochs (ignition, liftoff at full thrust,
/// MECO at T+145 s, stage separation 3 s later, the second stage's
/// ignition 8 s after MECO), a pitch kick of the pilot's at T+10 s (4 % of
/// the gimbal for 2.5 s), then SAS holding surface prograde: a gravity turn
/// through max-Q with the angle of attack under a degree and every joint
/// under its ultimate load (the gimbal's actuators at their rate, SAS tuned
/// to the stack). The pushers and MECO's seconds of coasting leave the
/// second stage's propellant on its outlets: the MVac lights at once and
/// runs, without debug mode; the booster leaves with its nine engines.
#[test]
fn a_gravity_turn_to_meco_and_the_mvac_lights_like_the_real_one() {
    use sim::vessel::{HoldMode, SpeedReference};
    let w = world();
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    let mut v = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft);
    let mut ids = VesselIds::default();
    ids.allocate();
    let go = t0().add_seconds(1.0);
    let liftoff = go.add_seconds(2.0);
    let meco = liftoff.add_seconds(145.0);
    for at in [go, go, meco, meco.add_seconds(3.0), meco.add_seconds(8.0)] {
        v.order_at(at, OrderKind::Stage).unwrap();
    }
    let up = Controls { throttle: 1.0, sas: true, ..Default::default() };
    let kick = Controls { throttle: 1.0, rotate: glam::DVec3::new(0.04, 0.0, 0.0), ..Default::default() };
    let prograde = Controls { hold: HoldMode::Prograde, speed: SpeedReference::Surface, ..up };
    let mut pieces = fly(&w, &mut v, liftoff.add_seconds(10.0), &up, &mut ids);
    pieces.extend(fly(&w, &mut v, liftoff.add_seconds(12.5), &kick, &mut ids));
    let earth = w.find("Earth").unwrap();
    let rot = earth.physical.as_ref().unwrap().rotation;
    let (mut worst, mut alpha_max) = (0.0f64, 0.0f64);
    let mut t = liftoff.add_seconds(12.5);
    while t < meco {
        t = t.add_seconds(2.5);
        pieces.extend(fly(&w, &mut v, t, &prograde, &mut ids));
        let (_, r, vel) = v.state(&w);
        let surface = vel - rot.omega(v.time).raw().cross(r);
        if t.seconds_since(liftoff) > 15.0 && surface.length() > 50.0 {
            let c = v.attitude.nose().dot(surface.normalize()).clamp(-1.0, 1.0);
            alpha_max = alpha_max.max(sim::math::acos(c).to_degrees());
        }
        worst = v.ship().unwrap().runtime.joints.iter().map(|j| j.ratio).fold(worst, f64::max);
    }
    let (anchor, r, vel) = v.state(&w);
    let alt = sim::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0;
    let surface = vel - rot.omega(v.time).raw().cross(r);
    println!(
        "MECO: {:.1} km, {:.0} m/s over the ground; α ≤ {alpha_max:.2}°, joints ≤ {worst:.2}",
        alt / 1e3,
        surface.length()
    );
    assert!(alpha_max < 1.0, "angle of attack {alpha_max}°");
    assert!(worst < 1.0, "a joint at {worst}");
    assert!(alt > 50e3 && surface.length() > 1_500.0, "{alt} m, {} m/s", surface.length());
    pieces.extend(fly(&w, &mut v, meco.add_seconds(14.0), &prograde, &mut ids));
    let events: Vec<String> = v.ship_events().iter().map(|e| e.text.clone()).collect();
    println!("{events:#?}");
    assert!(
        !events.iter().any(|e| e.contains("ingestion") || e.contains("abandoned") || e.contains("buckled")),
        "{events:?}"
    );
    let def = v.ship_def().unwrap();
    let mvac = def.engines.iter().position(|e| e.id == "s2.mvac").expect("the upper stage is the vessel");
    assert_eq!(v.ship().unwrap().engines[mvac].status, EngineStatus::Running);
    let thrust = v.engine_thrusts(1.0, 0.0).unwrap()[mvac];
    assert!((thrust - 981.0e3).abs() < 1.0, "{thrust}");
    let booster = pieces.iter_mut().find(|p| p.ship().is_some_and(|s| s.engines.len() == 9)).expect("the booster");
    assert!(booster.controllable(), "the booster flies on");
}
