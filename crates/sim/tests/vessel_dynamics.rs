//! Vessel dynamics against the shipped Solar System: orbit physics, anchor
//! invariance, warp invariance, chunked == single-pass, and landing.

use glam::DVec3;
use sim::chute::CanopyState;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{CoastStart, Controls, Destruction, EndKind, Phase, Segment, SegmentEnd, Vessel, VesselId};
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
    Segment::new(
        w,
        t0(),
        CoastStart { anchor, r, v, drag: None, contact_height: 0.0, horizon: 1e9, fixed_anchor, proper_time: 0.0 },
    )
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
    let params = sim::craft::test_craft();
    let mut slow = Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, params);
    let mut fast = Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, params);
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

/// A non-finite state ends the segment (it used to retry the step forever).
#[test]
fn nan_initial_state_ends_the_segment_instead_of_hanging() {
    let w = world();
    let (earth, mu) = earth(&w);
    let (r, v) = leo(mu);
    let mut seg = coast(&w, earth, DVec3::new(f64::NAN, r.y, r.z), v, false);
    seg.extend(&w, 10);
    assert_eq!(seg.end, Some(SegmentEnd { t: 0.0, kind: EndKind::Failed }));
    // A vessel in that state stops at its time; advancing returns.
    let mut vessel =
        Vessel::coasting(&w, VesselId(1), t0(), earth, DVec3::new(f64::NAN, r.y, r.z), v, sim::craft::test_craft());
    assert_eq!(vessel.advance(&w, t0().add_seconds(60.0), &Controls::default(), usize::MAX), t0());
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

/// A vessel `height` m above Cape Canaveral falling at `down` m/s
/// relative to the ground.
fn above_the_cape(w: &World, height: f64, down: f64, propellant: Option<f64>) -> Vessel {
    let (earth, _) = earth(w);
    let e = sim::body::earth();
    let t = t0();
    let deg = std::f64::consts::PI / 180.0;
    let fixed = e.surface_point(28.6 * deg, -80.6 * deg, height);
    let r = e.rotation.to_inertial(fixed, t).raw();
    let v = e.rotation.omega(t).raw().cross(r) - r.normalize() * down;
    let mut vessel = Vessel::coasting(w, VesselId(1), t, earth, r, v, sim::craft::test_craft());
    if let Some(p) = propellant {
        vessel.set_propellant(w, p);
    }
    // In the atmosphere it is flown live from the start.
    assert!(matches!(vessel.phase, Phase::Powered { .. }), "{:?}", vessel.phase);
    vessel
}

/// Speed relative to the ground (m/s).
fn ground_speed(w: &World, vessel: &Vessel) -> f64 {
    let (_, r, v) = vessel.state(w);
    (v - sim::body::earth().rotation.omega(vessel.time).raw().cross(r)).length()
}

/// Height above the ground (m).
fn height(w: &World, vessel: &Vessel) -> f64 {
    let (anchor, r, _) = vessel.state(w);
    let earth = w.find("Earth").unwrap().node;
    sim::forces::altitude_above(w, &w.snapshot(vessel.time), anchor, earth, r).0
}

/// The parachutes' drag force now (N): q·Cd·A of the open canopies.
fn chute_load(w: &World, vessel: &Vessel) -> f64 {
    let (anchor, r, v) = vessel.state(w);
    let snap = w.snapshot(vessel.time);
    let air = sim::vessel::air_at(w, &snap, anchor, r, v).unwrap();
    0.5 * air.rho * air.wind.length_squared() * vessel.chute.cd_area(&vessel.craft.chute)
}

#[test]
fn a_full_craft_tears_its_mains_and_crashes_but_lands_in_debug_mode() {
    let w = world();
    let controls = Controls { chute: true, sas: true, ..Default::default() };
    // At its full 20 t the drogues hold, but the mains, sized for 5 t,
    // break as they open towards full (D079); the craft hits at its
    // drogue-less hull speed.
    let mut vessel = above_the_cape(&w, 8_000.0, 0.0, None);
    let mut debug = vessel.clone();
    let t = vessel.time;
    let mut clock = t;
    let mut main_failed = false;
    for _ in 0..20_000 {
        clock = clock.add_seconds(0.1);
        vessel.advance(&w, clock, &controls, usize::MAX);
        main_failed |= vessel.chute.main == CanopyState::Failed;
        assert_ne!(vessel.chute.drogue, CanopyState::Failed);
        if !matches!(vessel.phase, Phase::Powered { .. }) {
            break;
        }
    }
    assert!(main_failed);
    match vessel.phase {
        Phase::Crashed { cause: Destruction::Impact { speed, .. }, .. } => assert!(speed > 30.0, "{speed} m/s"),
        other => panic!("expected a crash, got {other:?}"),
    }
    // Debug mode: unbreakable canopies and infinite impact tolerance; it
    // reaches the ground at its terminal speed under the full main,
    // √(2mg / ρ·CdA) ≈ 14 m/s.
    debug.set_debug(&w, true);
    let terminal =
        (2.0 * debug.mass() * 9.80 / (1.225 * (debug.craft.cd_area + debug.craft.chute.main.cd_area))).sqrt();
    let mut touchdown = 0.0;
    for _ in 0..20_000 {
        debug.advance(&w, debug.time.add_seconds(0.1), &controls, usize::MAX);
        if height(&w, &debug) < 20.0 || !matches!(debug.phase, Phase::Powered { .. }) {
            break;
        }
        touchdown = ground_speed(&w, &debug);
    }
    assert!((touchdown / terminal - 1.0).abs() < 0.05, "{touchdown} vs {terminal} m/s");
    debug.advance(&w, t.add_seconds(3_600.0), &controls, usize::MAX);
    assert!(matches!(debug.phase, Phase::Landed { .. }), "{:?}", debug.phase);
}

#[test]
fn a_nominal_descent_opens_drogues_then_reefed_mains_within_their_design_loads() {
    let w = world();
    // A 5 t craft (dry plus a 1 t reserve) at 7.3 km (Apollo's 24,000 ft,
    // TN D-7437) falling at 110 m/s (q ≈ 3.4 kPa, inside Apollo's 2.5 to
    // 5.5 kPa drogue qualification range), the command given at once.
    let mut vessel = above_the_cape(&w, 7_300.0, 110.0, Some(1_000.0));
    let controls = Controls { chute: true, sas: true, ..Default::default() };
    let weight = vessel.mass() * 9.80;
    let (mut drogue_peak, mut main_peak, mut drogue_q, mut main_q) = (0.0f64, 0.0f64, None, None);
    for _ in 0..200_000 {
        vessel.advance(&w, vessel.time.add_seconds(0.02), &controls, usize::MAX);
        if !matches!(vessel.phase, Phase::Powered { .. }) {
            break;
        }
        let load = chute_load(&w, &vessel);
        match (vessel.chute.drogue, vessel.chute.main) {
            (CanopyState::Open { age, .. }, _) => {
                drogue_peak = drogue_peak.max(load);
                if age < 0.03 {
                    let (anchor, r, v) = vessel.state(&w);
                    let air = sim::vessel::air_at(&w, &w.snapshot(vessel.time), anchor, r, v).unwrap();
                    drogue_q = Some(0.5 * air.rho * air.wind.length_squared());
                }
            }
            (_, CanopyState::Open { age, .. }) => {
                main_peak = main_peak.max(load);
                if age < 0.03 {
                    let (anchor, r, v) = vessel.state(&w);
                    let air = sim::vessel::air_at(&w, &w.snapshot(vessel.time), anchor, r, v).unwrap();
                    main_q = Some(0.5 * air.rho * air.wind.length_squared());
                }
            }
            _ => {}
        }
    }
    let c = &vessel.craft.chute;
    let (d, m) = (c.drogue.as_ref().unwrap(), &c.main);
    println!(
        "drogue at q {:.0} Pa, peak {:.0} kN ({:.2} g); main at q {:.0} Pa, peak {:.0} kN ({:.2} g); {:?}",
        drogue_q.unwrap(),
        drogue_peak / 1e3,
        drogue_peak / weight,
        main_q.unwrap(),
        main_peak / 1e3,
        main_peak / weight,
        vessel.phase
    );
    assert!(matches!(vessel.phase, Phase::Landed { .. }), "{:?}", vessel.phase);
    // Each canopy opened under its design q, and each peak between the
    // weight (it decelerates the craft) and Apollo's design limit loads
    // (2 × 17,200 lb drogues, 4 × 23,800 lb mains; TN D-7437). In g the
    // light craft feels more than Apollo's 2.9 g main design (37,500 lb on
    // 13,000 lb): four mains on 5 t against three on 5.9 t.
    assert!(drogue_q.unwrap() < d.deploy_max_q && main_q.unwrap() < m.deploy_max_q);
    assert!(drogue_peak < 2.0 * 76.5e3 && drogue_peak > weight, "{drogue_peak}");
    assert!(main_peak < 4.0 * 105.9e3 && main_peak > weight, "{main_peak}");
    assert!(drogue_peak < d.max_load && main_peak < m.max_load);
}

#[test]
fn a_drogue_fired_far_above_its_design_q_tears_and_the_mains_still_land_a_light_craft() {
    let w = world();
    // 350 m/s at 10 km (q ≈ 25 kPa, 4.6 × the drogue's 5.5 kPa).
    let mut vessel = above_the_cape(&w, 10_000.0, 350.0, Some(1_000.0));
    let controls = Controls { chute: true, sas: true, ..Default::default() };
    vessel.advance(&w, vessel.time.add_seconds(1.0), &controls, usize::MAX);
    assert_eq!(vessel.chute.drogue, CanopyState::Failed);
    vessel.advance(&w, vessel.time.add_seconds(1_800.0), &controls, usize::MAX);
    assert!(matches!(vessel.phase, Phase::Landed { .. }), "{:?}", vessel.phase);
}

#[test]
fn a_light_craft_lands_intact_under_its_parachute() {
    let w = world();
    let controls = Controls { chute: true, sas: true, ..Default::default() };
    // Dry 4 t plus a 1 t reserve: the landing mass the chute is sized
    // for (~7 m/s at sea level, under the gear's 8 m/s).
    let mut vessel = above_the_cape(&w, 3_000.0, 0.0, Some(1_000.0));
    let terminal =
        (2.0 * vessel.mass() * 9.80 / (1.225 * (vessel.craft.cd_area + vessel.craft.chute.main.cd_area))).sqrt();
    assert!(terminal > 6.0 && terminal < 7.5, "{terminal} m/s");
    vessel.advance(&w, vessel.time.add_seconds(1_800.0), &controls, usize::MAX);
    assert!(matches!(vessel.phase, Phase::Landed { .. }), "{:?}", vessel.phase);
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
    let mut vessel = Vessel::coasting(&w, VesselId(1), t, earth, r, v, sim::craft::test_craft());
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
        CoastStart {
            anchor: earth,
            r,
            v,
            drag: None,
            contact_height: 0.0,
            horizon: 1e9,
            fixed_anchor: false,
            proper_time: 0.0,
        },
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

const COAST_GOLDEN: u64 = 0x0d0bdc25e979d651;

/// A powered vessel trails the clock by up to a tick; shown at the clock
/// (`state_at`), it lands within millimetres of where the next tick puts it,
/// so the ground no longer jumps by v·dt each frame while thrusting.
#[test]
fn powered_vessel_shown_at_the_clock_is_continuous() {
    let w = world();
    let mut ship = Vessel::landed_at(&w, VesselId(1), "Earth", 28.5, -80.6, t0(), sim::craft::test_craft());
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
    let ship = Vessel::landed_at(&w, VesselId(1), "Earth", 0.0, 0.0, t0(), sim::craft::test_craft());
    let (_, r0, v0) = ship.state(&w);
    let (_, r1, _) = ship.state_at(&w, t0().add_seconds(0.01));
    assert!(((r1 - r0) - v0 * 0.01).length() < 1e-4);
}
