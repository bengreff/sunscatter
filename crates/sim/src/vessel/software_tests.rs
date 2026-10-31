//! The flight software in flight (F3, `systems::flight`): the Falcon 9
//! Crew's SAS without its IMUs, its first stage's computer unpowered, a
//! lost engine controller on the pad, debug mode, and the landing
//! prediction without navigation.

use super::*;
use crate::craft::state::EngineStatus;
use crate::craft::state::Lost;
use crate::craft::{craft_of, CraftId};
use crate::ephem::Ephemeris;
use crate::sol;
use crate::systems::flight::Sas;
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

/// Flies `v` to `until` with `controls`, applying its breakups (the pad's
/// mount left behind) and keeping its own piece.
fn fly(v: &mut Vessel, w: &World, until: Epoch, controls: &Controls) {
    let mut ids = VesselIds::default();
    ids.allocate();
    loop {
        v.advance(w, until, controls, usize::MAX);
        match v.plan_breakup() {
            Some(plan) => {
                v.split(w, &plan, &plan.crafts(), &mut ids);
            }
            None => return,
        }
    }
}

/// The Falcon 9 Crew 4 s after liftoff (released 2 s after ignition).
fn airborne(w: &World, debug: bool) -> Vessel {
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    let mut v = Vessel::landed_at(w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft);
    v.debug = debug;
    v.next_stage(w).unwrap();
    v.next_stage(w).unwrap();
    fly(&mut v, w, t0().add_seconds(6.0), &full());
    assert!(!v.clamped() && matches!(v.phase, Phase::Powered { .. }), "flying");
    assert!(v.time.seconds_since(t0()) > 5.9, "at {} s", v.time.seconds_since(t0()));
    v
}

fn lose(v: &mut Vessel, select: impl Fn(&str) -> bool) {
    let def = v.ship_def().unwrap();
    let state = v.ship.as_mut().unwrap();
    for s in def.sensors.iter().filter(|s| select(&s.id)) {
        state.lost.push(Lost { part: s.part, owner: s.id.clone(), why: "test".into() });
    }
}

/// The pitch and yaw rate left 3 s after a 0.02 rad/s pitch upset.
fn rate_after_upset(v: &mut Vessel, w: &World) -> f64 {
    v.attitude.omega += v.attitude.q * DVec3::new(0.02, 0.0, 0.0);
    let t = v.time.add_seconds(3.0);
    fly(v, w, t, &full());
    let body = v.attitude.q.inverse() * v.attitude.omega;
    body.truncate().length()
}

/// With every IMU lost SAS does not hold (no attitude reference; Dragon's
/// star trackers give no rates): an upset stays. With them it is damped;
/// in debug mode (the truth view) too.
#[test]
fn without_its_imus_sas_does_not_hold() {
    let w = world();
    // (debug, IMUs lost, status, damped)
    let table =
        [(false, false, Sas::Holding, true), (false, true, Sas::NoAttitude, false), (true, true, Sas::Holding, true)];
    for (debug, lost, status, damped) in table {
        let mut v = airborne(&w, debug);
        if lost {
            lose(&mut v, |id| id.contains(".imu#"));
        }
        assert_eq!(v.sas_status(&full()), status, "debug {debug}, lost {lost}");
        let rate = rate_after_upset(&mut v, &w);
        println!("debug {debug}, IMUs lost {lost}: {rate:.5} rad/s after 3 s");
        assert_eq!(rate < 0.005, damped, "debug {debug}, lost {lost}: {rate} rad/s");
        if !damped {
            assert!(rate > 0.015, "left to itself: {rate}");
        }
    }
    assert_eq!(Sas::NoAttitude.text(), "no attitude reference");
}

/// Without GNSS the holds that need a direction hold the attitude instead,
/// and there is no landing prediction; debug mode predicts still.
#[test]
fn without_gnss_the_direction_holds_hold_the_attitude() {
    let w = world();
    let mut v = airborne(&w, false);
    let prograde = Controls { hold: HoldMode::Prograde, speed: SpeedReference::Surface, ..full() };
    let start = |v: &Vessel| crate::landing::LandingStart::of_vessel(&w, v, 1.0, false, AssumedSurface);
    assert!(start(&v).is_some());
    lose(&mut v, |id| id.contains(".gps#"));
    assert_eq!(v.sas_status(&prograde), Sas::NoNavigation);
    assert!(start(&v).is_none(), "no navigation solution: no prediction");
    v.debug = true;
    assert_eq!(v.sas_status(&prograde), Sas::Holding);
    assert!(start(&v).is_some());
}

use crate::landing::AssumedAttitude::SurfaceRetrograde as AssumedSurface;

/// The first stage's computer unpowered in flight (its batteries drained):
/// the nine Merlins are safed by their own controllers (E); the second
/// stage's computer still works.
#[test]
fn a_stage_computer_unpowered_safes_its_engines() {
    let w = world();
    let mut v = airborne(&w, false);
    let def = v.ship_def().unwrap();
    let state = v.ship.as_mut().unwrap();
    for (b, d) in state.batteries.iter_mut().zip(&def.batteries) {
        if d.id.starts_with("s1.") {
            b.charge = 0.0;
        }
    }
    let t = v.time.add_seconds(0.1);
    fly(&mut v, &w, t, &full());
    let s = v.ship().unwrap();
    assert!(s.unpowered.iter().any(|l| l.owner == "s1.computer"), "the bus is dead");
    for (d, e) in def.engines.iter().zip(&s.engines).filter(|(d, _)| d.id.starts_with("s1.merlin")) {
        assert!(matches!(e.status, EngineStatus::ShuttingDown | EngineStatus::Off), "{}: {:?}", d.id, e.status);
    }
    let safed = v.ship_events().iter().filter(|e| e.text.contains("safed by its controller")).count();
    assert_eq!(safed, 9, "{:?}", v.ship_events().iter().map(|e| &e.text).collect::<Vec<_>>());
    assert!(v.flight_health().computing());
}

/// A Merlin's controller lost during the start: its thrust reading never
/// reaches the computer, and the launch commit aborts (the clamps held);
/// in debug mode (the truth view) the release goes ahead.
#[test]
fn a_lost_controller_on_the_pad_aborts_the_launch() {
    let w = world();
    for debug in [false, true] {
        let craft = craft_of(&CraftId("falcon9-crew".into()));
        let mut v = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft);
        v.debug = debug;
        v.next_stage(&w).unwrap();
        v.next_stage(&w).unwrap();
        v.advance(&w, t0().add_seconds(1.0), &full(), usize::MAX);
        let def = v.ship_def().unwrap();
        let e5 = def.engines.iter().find(|e| e.owner == "s1.merlin.5").unwrap();
        let lost = Lost { part: e5.part, owner: e5.owner.clone(), why: "controller lost".into() };
        v.ship.as_mut().unwrap().lost.push(lost);
        v.advance(&w, t0().add_seconds(3.0), &full(), usize::MAX);
        let abort = v.ship_events().iter().find(|e| e.text.starts_with("Pad abort"));
        if debug {
            assert!(abort.is_none() && !v.clamped(), "debug: released");
        } else {
            let abort = abort.expect("a pad abort");
            assert!(abort.alarm && abort.text.contains("s1.merlin.5 no thrust reading"), "{}", abort.text);
            assert!(v.clamped() && !v.ship().unwrap().release_armed);
        }
    }
}
