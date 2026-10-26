//! The bubble in flight (D082): scenarios in low Earth orbit with the test
//! craft. Vessel 0 (A) and 1 (B) coast side by side, B drifting onto A at
//! 0.5 m/s; vessel 2 (C), 2 km behind, burns at its least throttle, so it
//! leads a bubble that holds A and B live.

use super::*;
use crate::craft::test_craft;
use crate::ephem::Ephemeris;
use crate::kepler::Elements;
use crate::sol;
use crate::vessel::VesselId;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(86_400.0)
}

/// A 400 km orbit about Earth and its along-track unit vector.
fn leo(w: &World) -> (NodeId, DVec3, DVec3, DVec3) {
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    let along = (v - r * (v.dot(r) / r.length_squared())).normalize();
    (earth.node, r, v, along)
}

/// A and B `gap` m apart along track (B ahead, closing at `closing` m/s),
/// and C 2 km behind A.
fn scene(w: &World, gap: f64, closing: f64) -> Vec<Vessel> {
    let (earth, r, v, along) = leo(w);
    let craft = test_craft();
    let a = Vessel::coasting(w, VesselId(1), t0(), earth, r, v, craft);
    let b = Vessel::coasting(w, VesselId(2), t0(), earth, r + along * gap, v - along * closing, craft);
    let c = Vessel::coasting(w, VesselId(3), t0(), earth, r - along * 2_000.0, v, craft);
    vec![a, b, c]
}

/// C burns at its least throttle (while `burning`); everyone holds its
/// attitude (the game's passive controls).
fn controls(burning: bool) -> impl Fn(usize) -> Controls {
    move |i| Controls { sas: true, throttle: if i == 2 && burning { 0.1 } else { 0.0 }, ..Default::default() }
}

/// One frame of the game: the bubble first, then every vessel.
fn frame(w: &World, fleet: &mut [Vessel], ctl: &dyn Fn(usize) -> Controls, target: Epoch) {
    advance(w, fleet, ctl, target);
    for (i, v) in fleet.iter_mut().enumerate() {
        v.advance(w, target, &ctl(i), COAST_BUDGET);
    }
}

/// Frames of `dt` from the fleet's time for `span` s; calls `each` after
/// every frame.
fn run(
    w: &World,
    fleet: &mut [Vessel],
    ctl: &dyn Fn(usize) -> Controls,
    dt: f64,
    span: f64,
    each: &mut dyn FnMut(&[Vessel]),
) {
    let start = fleet[0].time;
    let frames = (span / dt).round() as usize;
    for k in 1..=frames {
        frame(w, fleet, ctl, start.add_seconds(dt * k as f64));
        each(fleet);
    }
}

fn state(w: &World, v: &Vessel) -> (DVec3, DVec3) {
    let (_, r, vel) = v.state(w);
    (r, vel)
}

/// Two craft drifting together at 0.5 m/s inside a bubble collide and
/// separate: both join at the first epoch, touch, come apart with the
/// approach reversed, and survive (0.5 m/s is far under the 8 m/s limit).
#[test]
fn two_craft_in_a_bubble_collide_and_separate() {
    let w = world();
    let mut fleet = scene(&w, 12.0, 0.5);
    let ctl = controls(true);
    let (mut touched, mut first) = (0, None);
    let mut ticks = 0;
    run(&w, &mut fleet, &ctl, 1.0 / 60.0, 30.0, &mut |f| {
        ticks += 1;
        if f[0].bubble.since_contact == Some(0) {
            touched += 1;
            first.get_or_insert(f[0].time);
        }
    });
    for v in &fleet {
        assert!(v.in_bubble(), "{:?} is a member", v.id());
        assert!(matches!(v.phase, Phase::Powered { .. }));
        assert_eq!(v.destruction(), None);
    }
    assert_eq!(fleet[0].time, fleet[1].time, "members tick in lockstep");
    let ((ra, va), (rb, vb)) = (state(&w, &fleet[0]), state(&w, &fleet[1]));
    let line = (rb - ra).normalize();
    let closing = (vb - va).dot(line);
    eprintln!(
        "touched in {touched} of {ticks} frames from {:.2} s; separating at {closing:.3} m/s, {:.2} m apart",
        first.map_or(f64::NAN, |t| t.seconds_since(t0())),
        (rb - ra).length()
    );
    assert!(touched > 3, "they touched");
    assert!(closing > 0.05 && closing < 0.4, "separating at {closing} m/s");
}

/// Frames of 1/60 s and one call over the same span give the same bits:
/// the bubble's joins, ticks and contacts are on its lattice.
#[test]
fn a_bubble_in_frames_equals_one_call() {
    let w = world();
    let ctl = controls(true);
    let mut frames = scene(&w, 12.0, 0.5);
    run(&w, &mut frames, &ctl, 1.0 / 60.0, 14.0, &mut |_| {});
    let mut once = scene(&w, 12.0, 0.5);
    run(&w, &mut once, &ctl, 14.0, 14.0, &mut |_| {});
    for (a, b) in frames.iter().zip(&once) {
        assert_eq!(a.time, b.time);
        assert_eq!(a.phase, b.phase, "{:?}", a.id());
        assert_eq!(a.attitude, b.attitude);
        assert_eq!(a.bubble, b.bubble);
        assert_eq!(a.thermal, b.thermal);
    }
}

/// Saved and loaded mid-contact (A and B touching), the bubble continues
/// bit for bit: membership, contact counters and the lattice are in the
/// vessels.
#[test]
fn save_and_load_mid_contact_continue_bit_for_bit() {
    let w = world();
    let ctl = controls(true);
    let mut fleet = scene(&w, 12.0, 0.5);
    let start = fleet[0].time;
    let mut k = 0;
    while fleet[0].bubble.since_contact != Some(0) {
        k += 1;
        assert!(k < 60 * 40, "no contact");
        frame(&w, &mut fleet, &ctl, start.add_seconds(k as f64 / 60.0));
    }
    let mut ids = crate::vessel::VesselIds::default();
    while ids.issued(VesselId(1)) && !ids.issued(VesselId(3)) || !ids.issued(VesselId(1)) {
        ids.allocate();
    }
    let save = crate::save::SaveGame::capture(&w, fleet[0].time, &fleet, ids, 0, Controls::default());
    let mut loaded = crate::save::SaveGame::from_ron(&save.to_ron()).expect("loads").vessels;
    assert_eq!(loaded, fleet, "the save round-trips");
    let resume = fleet[0].time;
    for f in 1..=120 {
        let target = resume.add_seconds(f as f64 / 60.0);
        frame(&w, &mut fleet, &ctl, target);
        frame(&w, &mut loaded, &ctl, target);
    }
    assert_eq!(loaded, fleet, "continued alike");
    assert!(fleet[0].bubble.since_contact.is_some());
}

/// The difference a bubble makes to a craft that never touches anything:
/// B 5 km from the leader, flown live for a minute, against B coasting
/// alone: under a millimetre. It is the difference between live ticks and
/// a coast, not the bubble's: a vessel held live alone differs from its
/// coast by the same bits, and the coast from a bare (unperturbed) coast
/// by nothing measurable here (the radiation pressure and thin air the
/// live ticks leave out are ~1e-8 m/s²). Measured: 0.72 mm and 3.9e-5 m/s
/// at 400 km, 0.28 mm and 2.2e-5 m/s at 2,000 km, a steady ~4e-7 m/s²
/// between the live tick's force evaluation and the coast's (reported in
/// `docs/features/proximity.md`).
#[test]
fn a_minute_in_a_bubble_moves_a_craft_by_under_a_millimetre() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    for (a, most) in [(6_778_137.0, 1e-3), (8_378_137.0, 5e-4)] {
        let el = Elements { a, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
        let (r, v) = el.to_state(earth.gm);
        let along = (v - r * (v.dot(r) / r.length_squared())).normalize();
        let craft = test_craft();
        let b = || Vessel::coasting(&w, VesselId(2), t0(), earth.node, r + along * 5_000.0, v, craft);
        let leader = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, craft);
        let ctl = |i: usize| Controls { sas: true, throttle: if i == 0 { 0.1 } else { 0.0 }, ..Default::default() };
        let mut with = vec![leader, b()];
        let mut alone = vec![b()];
        let mut joined = None;
        run(&w, &mut with, &ctl, 0.5, 60.0, &mut |f| {
            if f[1].in_bubble() {
                joined.get_or_insert(f[1].time);
            }
        });
        run(&w, &mut alone, &|_| Controls { sas: true, ..Default::default() }, 0.5, 60.0, &mut |_| {});
        assert!(joined.is_some_and(|t| t.seconds_since(t0()) <= 0.5), "B joins in the first frame");
        assert!(matches!(with[1].phase, Phase::Powered { .. }) && matches!(alone[0].phase, Phase::Coasting { .. }));
        let t = with[1].time;
        let (ra, va) = state(&w, &with[1]);
        let (_, rb, vb) = alone[0].state_at(&w, t);
        let (dr, dv) = ((ra - rb).length(), (va - vb).length());
        eprintln!("a = {:.0} km, 60 s in the bubble: {:.4} mm, {dv:.2e} m/s from the coast", a / 1e3, dr * 1e3);
        assert!(dr < most, "{dr} m");
    }
}

/// When the leader stops burning, its members leave at that epoch and
/// coast on (and the leader with them); nothing is live afterwards.
#[test]
fn members_return_to_coasts_when_the_leader_does() {
    let w = world();
    let mut fleet = scene(&w, 200.0, 0.0);
    run(&w, &mut fleet, &controls(true), 1.0 / 60.0, 2.0, &mut |_| {});
    assert!(fleet.iter().all(|v| v.in_bubble() && matches!(v.phase, Phase::Powered { .. })));
    run(&w, &mut fleet, &controls(false), 1.0 / 60.0, 1.0, &mut |_| {});
    for v in &fleet {
        assert!(!v.in_bubble(), "{:?} left", v.id());
        assert!(matches!(v.phase, Phase::Coasting { .. }), "{:?} coasts", v.id());
    }
    let before = fleet.clone();
    let target = fleet[0].time.add_seconds(10.0);
    let out = advance(&w, &mut fleet, &controls(false), target);
    assert_eq!(out, Advance::default(), "no bubble without a leader");
    assert_eq!(fleet, before, "the bubble pass touched nothing");
}
