//! Vessel dynamics against the shipped Solar System: orbit physics, anchor
//! invariance, warp invariance, chunked == single-pass, and landing.

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{CoastStart, Controls, Phase, Segment, Vessel, VesselParams};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn earth(w: &World) -> (NodeId, f64) {
    let s = w.find("Earth").unwrap();
    (s.node, s.gm)
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

/// ISS-like orbit: 400 km circular, 51.6°.
fn leo(mu: f64) -> (DVec3, DVec3) {
    let deg = std::f64::consts::PI / 180.0;
    Elements { a: 6_778_137.0, e: 0.0005, i: 51.6 * deg, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(mu)
}

fn coast(w: &World, anchor: NodeId, r: DVec3, v: DVec3, fixed_anchor: bool) -> Segment {
    Segment::new(w, t0(), CoastStart { anchor, r, v, drag: None, contact_height: 0.0, horizon: 1e9, fixed_anchor })
}

fn extend_to(w: &World, seg: &mut Segment, t: f64) {
    while seg.computed_until() < t && !seg.finished() {
        seg.extend(w, 64);
    }
}

#[test]
fn leo_period_and_j2_nodal_regression_match_theory() {
    let w = world();
    let (earth, mu) = earth(&w);
    let (r, v) = leo(mu);
    let mut seg = coast(&w, earth, r, v, false);
    let days = 3.0;
    extend_to(&w, &mut seg, days * 86_400.0);
    let el0 = Elements::from_state(r, v, mu);
    let (anchor, r1, v1) = seg.eval(days * 86_400.0).unwrap();
    assert_eq!(anchor, earth);
    let el1 = Elements::from_state(r1, v1, mu);
    // Analytic J2 nodal regression: dΩ/dt = −1.5 n J2 (R/p)² cos i.
    let e = sim::body::earth();
    let n = el0.mean_motion(mu);
    let p = el0.a * (1.0 - el0.e * el0.e);
    let rate = -1.5 * n * e.j2 * (e.radius_eq / p).powi(2) * el0.i.cos();
    let expected = rate * days * 86_400.0;
    let measured = sim::math::wrap_pi(el1.raan - el0.raan);
    println!("nodal regression over {days} d: measured {measured:.5} rad, J2 theory {expected:.5} rad");
    assert!((measured - expected).abs() < 0.02 * expected.abs(), "measured {measured}, expected {expected}");
    println!("steps for {days} days of LEO: {}", seg.samples.len());
}

#[test]
fn anchor_choice_does_not_change_the_trajectory() {
    // Same physical initial state, integrated about Earth and about the
    // Earth–Moon barycenter (both keep offsets small), must agree closely.
    let w = world();
    let (earth, mu) = earth(&w);
    let emb = w.eph.find("EMB").unwrap();
    let (r, v) = leo(mu);
    let shift = w.eph.relative(earth, emb, t0());
    let mut about_earth = coast(&w, earth, r, v, true);
    let mut about_emb = coast(&w, emb, r + shift.r, v + shift.v, true);
    let span = 6.0 * 3600.0;
    extend_to(&w, &mut about_earth, span);
    extend_to(&w, &mut about_emb, span);
    for k in 1..=12 {
        let t = span * k as f64 / 12.0;
        let (_, ra, va) = about_earth.eval(t).unwrap();
        let (_, rb, vb) = about_emb.eval(t).unwrap();
        let back = w.eph.relative(emb, earth, t0().add_seconds(t));
        let dr = (ra - (rb + back.r)).length();
        let dv = (va - (vb + back.v)).length();
        assert!(dr < 0.05 && dv < 5e-5, "t={t}: dr={dr} m dv={dv} m/s");
    }
}

#[test]
fn barycentric_anchor_agrees_within_its_precision() {
    // Anchoring at the Solar System barycenter is physically identical but
    // costs precision (|r| ~ 1.5e11 m): the agreement is metres, not mm.
    let w = world();
    let (earth, mu) = earth(&w);
    let root = w.eph.root();
    let (r, v) = leo(mu);
    let shift = w.eph.relative(earth, root, t0());
    let mut local = coast(&w, earth, r, v, true);
    let mut bary = coast(&w, root, r + shift.r, v + shift.v, true);
    let span = 3.0 * 3600.0;
    extend_to(&w, &mut local, span);
    extend_to(&w, &mut bary, span);
    let (_, ra, _) = local.eval(span).unwrap();
    let (_, rb, _) = bary.eval(span).unwrap();
    let back = w.eph.relative(root, earth, t0().add_seconds(span));
    let dr = (ra - (rb + back.r)).length();
    println!("Earth-anchored vs barycentric after 3 h: {dr:.3} m");
    assert!(dr < 50.0, "{dr}");
}

#[test]
fn chunked_extension_is_bit_identical() {
    let w = world();
    let (earth, mu) = earth(&w);
    let (r, v) = leo(mu);
    let mut one = coast(&w, earth, r, v, false);
    one.extend(&w, 2000);
    let mut many = coast(&w, earth, r, v, false);
    for chunk in [1usize, 7, 100, 3, 1889] {
        many.extend(&w, chunk);
    }
    assert_eq!(one.samples, many.samples);
}

#[test]
fn pruning_passed_samples_does_not_change_evaluation() {
    let w = world();
    let (earth, mu) = earth(&w);
    let (r, v) = leo(mu);
    let mut full = coast(&w, earth, r, v, false);
    full.extend(&w, 500);
    let mut pruned = full.clone();
    let cut = full.computed_until() * 0.4;
    pruned.prune_before(cut);
    assert!(pruned.samples.len() < full.samples.len());
    for k in 0..=100 {
        let t = cut + (full.computed_until() - cut) * k as f64 / 100.0;
        assert_eq!(full.eval(t), pruned.eval(t), "t = {t}");
    }
}

#[test]
fn time_warp_does_not_change_the_state() {
    // Advancing a coasting vessel in many small frames or one huge jump must
    // give bit-identical states at the same time.
    let w = world();
    let (earth, mu) = earth(&w);
    let (r, v) = leo(mu);
    let params = VesselParams::block();
    let mut slow = Vessel::coasting(&w, t0(), earth, r, v, params);
    let mut fast = Vessel::coasting(&w, t0(), earth, r, v, params);
    let controls = Controls::default();
    let end = t0().add_seconds(20_000.0);
    let mut t = t0();
    while t < end {
        t = t.add_seconds(1.0 / 60.0 * 4.0); // 4x warp at 60 fps
        slow.advance(&w, t.min_epoch(end), &controls, usize::MAX);
    }
    fast.advance(&w, end, &controls, usize::MAX);
    assert_eq!(slow.time, fast.time);
    assert_eq!(slow.state(&w), fast.state(&w));
}

trait MinEpoch {
    fn min_epoch(self, other: Epoch) -> Epoch;
}
impl MinEpoch for Epoch {
    fn min_epoch(self, other: Epoch) -> Epoch {
        if self < other {
            self
        } else {
            other
        }
    }
}

#[test]
fn parachute_descent_lands_safely() {
    let w = world();
    let (earth, _) = earth(&w);
    let e = sim::body::earth();
    let t = t0();
    // 8 km above Cape Canaveral, at rest relative to the ground.
    let deg = std::f64::consts::PI / 180.0;
    let fixed = e.surface_point(28.6 * deg, -80.6 * deg, 8_000.0);
    let r = e.rotation.to_inertial(fixed, t).raw();
    let v = e.rotation.omega(t).raw().cross(r);
    let mut vessel = Vessel::coasting(&w, t, earth, r, v, VesselParams::block());
    let controls = Controls { chute: true, sas: true, ..Default::default() };
    vessel.advance(&w, t.add_seconds(3_600.0), &controls, usize::MAX);
    match vessel.phase {
        Phase::Landed { .. } => {}
        other => panic!("expected a safe landing, got {other:?}"),
    }
}

#[test]
fn falling_without_parachute_crashes() {
    let w = world();
    let (earth, _) = earth(&w);
    let e = sim::body::earth();
    let t = t0();
    let deg = std::f64::consts::PI / 180.0;
    let fixed = e.surface_point(0.0, 10.0 * deg, 3_000.0);
    let r = e.rotation.to_inertial(fixed, t).raw();
    let v = e.rotation.omega(t).raw().cross(r);
    let mut vessel = Vessel::coasting(&w, t, earth, r, v, VesselParams::block());
    vessel.advance(&w, t.add_seconds(600.0), &Controls::default(), usize::MAX);
    assert!(matches!(vessel.phase, Phase::Crashed { .. }), "{:?}", vessel.phase);
}

/// Cross-platform determinism of vessel coasts: CI runs this on macOS
/// (ARM64) and Windows (x86-64). If it changes intentionally, update the hash.
#[test]
fn coast_segment_matches_golden_hash() {
    let w = world();
    let (earth, mu) = earth(&w);
    let (r, v) = leo(mu);
    let mut seg = Segment::new(
        &w,
        t0(),
        CoastStart { anchor: earth, r, v, drag: None, contact_height: 0.0, horizon: 1e9, fixed_anchor: false },
    );
    seg.extend(&w, 2000);
    let mut bytes = Vec::new();
    for s in &seg.samples {
        bytes.extend_from_slice(&s.anchor.0.to_le_bytes());
        for x in [s.s.t, s.s.r.x, s.s.r.y, s.s.r.z, s.s.v.x, s.s.v.y, s.s.v.z] {
            bytes.extend_from_slice(&x.to_bits().to_le_bytes());
        }
    }
    let hash = sim::ephem::fnv1a64(&bytes);
    println!("coast golden hash: {hash:#018x}");
    assert_eq!(hash, COAST_GOLDEN, "coast output changed (or differs on this platform)");
}

const COAST_GOLDEN: u64 = 0xa1972777eba62ac1;

/// A powered vessel trails the clock by up to a tick; shown at the clock
/// (`state_at`), it lands within millimetres of where the next tick puts it,
/// so the ground no longer jumps by v·dt each frame while thrusting.
#[test]
fn powered_vessel_shown_at_the_clock_is_continuous() {
    let w = world();
    let mut ship = Vessel::landed_at(&w, "Earth", 28.5, -80.6, t0(), VesselParams::block());
    let full = Controls { throttle: 1.0, sas: true, ..Controls::default() };
    let mut clock = t0();
    // 20 s of climb, advanced by uneven frames as the game does.
    for k in 0..1500 {
        clock = clock.add_seconds(if k % 3 == 0 { 0.007 } else { 0.0163 });
        ship.advance(&w, clock, &full, usize::MAX);
    }
    assert!(matches!(ship.phase, Phase::Powered { .. }));
    assert_eq!(ship.state_at(&w, ship.time), ship.state(&w));
    let mut worst = 0.0f64;
    let mut speed = 0.0f64;
    for _ in 0..200 {
        let next = ship.time.add_seconds(sim::vessel::TICK);
        let (anchor, shown, _) = ship.state_at(&w, next);
        ship.advance(&w, next, &full, usize::MAX);
        let (a2, actual, v) = ship.state(&w);
        assert_eq!((anchor, ship.time), (a2, next));
        worst = worst.max((shown - actual).length());
        speed = v.length();
    }
    // Showing the tick state instead would be off by v·TICK (metres).
    assert!(speed * sim::vessel::TICK > 1.0, "speed {speed}");
    assert!(worst < 1e-3, "worst gap {worst} m");
}

/// Landed vessels are shown at the clock too (the body turns under them).
#[test]
fn landed_vessel_state_at_follows_the_rotation() {
    let w = world();
    let ship = Vessel::landed_at(&w, "Earth", 0.0, 0.0, t0(), VesselParams::block());
    let (_, r0, v0) = ship.state(&w);
    let (_, r1, _) = ship.state_at(&w, t0().add_seconds(0.01));
    assert!(((r1 - r0) - v0 * 0.01).length() < 1e-4);
}
