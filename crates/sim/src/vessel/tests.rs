//! Vessel unit tests that need private state (segment horizons, the attitude
//! lattice). Behaviour tests live in `tests/vessel_dynamics.rs`.

use super::*;
use crate::ephem::{Ephemeris, PAST_END_CALLS};
use crate::kepler::Elements;
use crate::sol;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

/// A 400 km orbit about Earth.
fn leo(w: &World) -> (NodeId, DVec3, DVec3) {
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    (earth.node, r, v)
}

#[test]
fn a_coast_started_near_the_ephemeris_end_stops_at_the_end() {
    let w = world();
    let (earth, r, v) = leo(&w);
    let t0 = w.end().add_seconds(-6.0 * 3600.0);
    PAST_END_CALLS.with(|c| c.set(0));
    let mut ship = Vessel::coasting(&w, t0, earth, r, v, VesselParams::block());
    let reached = ship.advance(&w, w.end().add_seconds(86_400.0), &Controls::default(), usize::MAX);
    assert!(reached.seconds_since(w.end()).abs() < 1e-6, "stopped {} s from the end", reached.seconds_since(w.end()));
    assert!(reached <= w.end());
    let end = ship.segment().unwrap().end.unwrap();
    assert_eq!(end.kind, EndKind::EphemerisEnd);
    // Advancing again stays put, and a coast started at the end is empty.
    assert_eq!(ship.advance(&w, w.end().add_seconds(10.0), &Controls::default(), usize::MAX), reached);
    let mut at_end = Segment::new(
        &w,
        w.end(),
        CoastStart {
            anchor: earth,
            r,
            v,
            drag: None,
            contact_height: 0.0,
            horizon: COAST_HORIZON,
            fixed_anchor: false,
        },
    );
    at_end.extend(&w, 10);
    assert_eq!(at_end.end, Some(SegmentEnd { t: 0.0, kind: EndKind::EphemerisEnd }));
    assert_eq!(PAST_END_CALLS.with(|c| c.get()), 0, "the ephemeris was evaluated past its end");
}

#[test]
fn a_segment_ended_at_its_horizon_is_continued() {
    let w = world();
    let (earth, r, v) = leo(&w);
    let t0 = sol::sol_epoch().add_seconds(86_400.0);
    let mut ship = Vessel::coasting(&w, t0, earth, r, v, VesselParams::block());
    let start =
        CoastStart { anchor: earth, r, v, drag: None, contact_height: 0.0, horizon: 100.0, fixed_anchor: false };
    ship.phase = Phase::Coasting { segment: Box::new(Segment::new(&w, t0, start)) };
    let target = t0.add_seconds(1_000.0);
    assert_eq!(ship.advance(&w, target, &Controls::default(), usize::MAX), target);
    let seg = ship.segment().unwrap();
    assert!(seg.t0.seconds_since(t0) >= 100.0 && seg.end.is_none(), "a new segment continues the coast");
}

/// Coasts one ship at 60 fps and another in one jump over 10 s; returns both.
fn attitude_after_frames_and_jump(controls: Controls, omega0: DVec3) -> (Vessel, Vessel) {
    let w = world();
    let (earth, r, v) = leo(&w);
    let t0 = sol::sol_epoch().add_seconds(86_400.0);
    let mut frames = Vessel::coasting(&w, t0, earth, r, v, VesselParams::block());
    frames.attitude.omega = omega0;
    let mut jump = frames.clone();
    for k in 1..=600 {
        frames.advance(&w, t0.add_seconds(k as f64 / 60.0), &controls, usize::MAX);
    }
    jump.advance(&w, t0.add_seconds(10.0), &controls, usize::MAX);
    assert_eq!(frames.time, jump.time);
    (frames, jump)
}

#[test]
fn coasting_rotation_input_is_independent_of_frame_rate() {
    let controls = Controls { rotate: DVec3::new(0.3, -1.0, 0.5), ..Controls::default() };
    let (frames, jump) = attitude_after_frames_and_jump(controls, DVec3::ZERO);
    // 10 s of input at 0.5 rad/s² per axis (500 ticks, one partial tick left).
    let spin = frames.attitude.omega.length();
    assert!((spin - 0.5 * 10.0 * DVec3::new(0.3, -1.0, 0.5).length()).abs() < 0.02, "spin {spin} rad/s");
    assert_eq!(frames.attitude, jump.attitude);
}

#[test]
fn coasting_sas_is_independent_of_frame_rate() {
    let controls = Controls { sas: true, ..Controls::default() };
    let (frames, jump) = attitude_after_frames_and_jump(controls, DVec3::new(0.3, 0.1, -0.2));
    assert_eq!(frames.attitude.omega, DVec3::ZERO, "SAS stopped the rotation");
    assert_eq!(frames.attitude, jump.attitude);
}
