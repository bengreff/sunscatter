//! Limbo regime changes (`docs/technical/limbo.md`, review of 2026-10-26):
//! a perturbed coast restarts where its attitude regime changes, and those
//! epochs never depend on the frames. A change of the hold's automatic
//! reference body gives the same bits in one call and in frames at 1×,
//! 1,000× and 1,000,000×; the reference never follows the anchor; frames
//! whose budgets run out anywhere give one call's bits.

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::pointing::Rule;
use sim::vessel::{Controls, EndKind, HoldMode, Phase, SpeedReference, Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

/// Flies `v` to `end` in frames of `dt` with the game's budget and
/// look-ahead.
fn fly(w: &World, v: &mut Vessel, end: Epoch, dt: f64, controls: &Controls) {
    let mut clock = v.time;
    while clock < end {
        let target = clock.add_seconds(dt);
        let target = if target > end { end } else { target };
        clock = v.advance(w, target, controls, 1_200);
        v.extend_coast(w, clock.add_seconds(6_000.0), 1_500);
    }
}

/// The same bits: time, state, attitude, temperatures and the stored
/// trajectory extended alike.
fn assert_same(w: &World, a: &Vessel, b: &Vessel, what: &str) {
    let (mut a, mut b) = (a.clone(), b.clone());
    assert_eq!(a.time, b.time, "{what}: time");
    assert_eq!(a.state(w), b.state(w), "{what}: state");
    assert_eq!(a.attitude, b.attitude, "{what}: attitude");
    if let (Some(ta), Some(tb)) = (a.trajectory(), b.trajectory()) {
        let until = if ta.computed_until() > tb.computed_until() { ta.computed_until() } else { tb.computed_until() };
        a.extend_coast(w, until, usize::MAX);
        b.extend_coast(w, until, usize::MAX);
    }
    assert_eq!(a, b, "{what}: vessel");
}

/// Leaving Earth's Laplace sphere (~924,000 km) held prograde, retrograde
/// or surface-prograde with no reference named: the automatic reference
/// turns from Earth to the Sun at an integrator step of the coast (an
/// `EndKind::Regime` end), so one call and frames at 1,000× and 1,000,000×
/// (and 1× across the change) agree.
#[test]
fn the_automatic_reference_changes_at_the_same_epoch_at_every_frame_rate() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let sun = w.find("Sun").unwrap().node;
    let r = DVec3::new(8.9e8, 1.0e7, 2.0e7);
    let v = DVec3::new(1_400.0, 250.0, 0.0);
    let holds = [
        (HoldMode::Prograde, SpeedReference::Orbit),
        (HoldMode::Retrograde, SpeedReference::Orbit),
        (HoldMode::Prograde, SpeedReference::Surface),
    ];
    for (hold, speed) in holds {
        let controls = Controls { sas: true, hold, speed, ..Controls::default() };
        let what = format!("{hold:?} {speed:?}");
        let mut s = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, &sim::craft::small::rocket());
        assert!(matches!(s.phase, Phase::Coasting { .. }));
        // Settled in Earth's frame first.
        fly(&w, &mut s, t0().add_seconds(600.0), 100.0 / 60.0, &controls);
        let rule = s.segment().unwrap().limbo().unwrap().pointing.rule();
        assert!(matches!(rule, Rule::Track(t) if t.auto && t.reference == earth.node), "{what}: {rule:?}");
        // Where the reference changes.
        let mut probe = s.clone();
        probe.extend_coast(&w, s.time.add_seconds(60_000.0), usize::MAX);
        let traj = probe.trajectory().unwrap();
        let change = (traj.segments.iter())
            .find_map(|g| g.end.filter(|e| e.kind == EndKind::Regime).map(|e| g.t0.add_seconds(e.t)))
            .expect("the reference changes within the span");
        let end = change.add_seconds(3_000.0);
        let mut jump = s.clone();
        jump.advance(&w, end, &controls, usize::MAX);
        for dt in [1_000.0 / 60.0, 1e6 / 60.0] {
            let mut framed = s.clone();
            fly(&w, &mut framed, end, dt, &controls);
            assert_same(&w, &framed, &jump, &format!("{what}, frames of {dt} s"));
        }
        let mut before = s.clone();
        before.advance(&w, change.add_seconds(-20.0), &controls, usize::MAX);
        let (mut framed, mut once) = (before.clone(), before.clone());
        let end = change.add_seconds(120.0);
        fly(&w, &mut framed, end, 1.0 / 60.0, &controls);
        once.advance(&w, end, &controls, usize::MAX);
        assert_same(&w, &framed, &once, &format!("{what}, 1x frames across the change"));
        // After the change (and the turn) it holds in the Sun's frame.
        let rule = jump.segment().unwrap().limbo().unwrap().pointing.rule();
        assert!(matches!(rule, Rule::Track(t) if t.auto && t.reference == sun), "{what}: {rule:?}");
    }
}

/// A prograde hold with no reference named, far outside Earth's sphere
/// where the coast re-anchors from Earth: the automatic reference is the
/// Sun whatever the anchor (rule 1), so frames at 1,000× give one call's
/// bits.
#[test]
fn the_automatic_reference_does_not_follow_the_anchor() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let r = DVec3::new(1.59e9, 1.0e7, 0.0);
    let v = DVec3::new(3_000.0, 300.0, 0.0);
    let controls = Controls { sas: true, hold: HoldMode::Prograde, ..Controls::default() };
    let mut s = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, &sim::craft::small::rocket());
    fly(&w, &mut s, t0().add_seconds(600.0), 100.0 / 60.0, &controls);
    let end = s.time.add_seconds(40_000.0);
    let mut jump = s.clone();
    jump.advance(&w, end, &controls, usize::MAX);
    let mut framed = s.clone();
    fly(&w, &mut framed, end, 1_000.0 / 60.0, &controls);
    assert_ne!(jump.state(&w).0, earth.node, "re-anchored away from Earth");
    assert_same(&w, &framed, &jump, "frames of 1,000x");
    let sun = w.find("Sun").unwrap().node;
    let rule = jump.segment().unwrap().limbo().unwrap().pointing.rule();
    assert!(matches!(rule, Rule::Track(t) if t.auto && t.reference == sun), "{rule:?}");
}

/// Flies `v` to `end` in frames of `dt` with `budget` units a frame (the
/// look-ahead as tight), so calls stop wherever the work runs out.
fn fly_tight(w: &World, v: &mut Vessel, end: Epoch, dt: f64, controls: &Controls, budget: usize) {
    let mut clock = v.time;
    while clock < end {
        let target = clock.add_seconds(dt);
        let target = if target > end { end } else { target };
        clock = v.advance(w, target, controls, budget);
        v.extend_coast(w, clock.add_seconds(6_000.0), budget / 10);
    }
}

/// A month held prograde at 700 km in frames whose budgets run out
/// anywhere (one coast step a frame at 1x; a few at 1,000x and
/// 1,000,000x) gives one call's bits.
#[test]
fn a_month_held_prograde_with_tight_budgets_is_one_calls() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let controls = Controls { sas: true, hold: HoldMode::Prograde, ..Controls::default() };
    let el = Elements { a: 6_378_137.0 + 700_000.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
    let (r, v) = el.to_state(earth.gm);
    let s = Vessel::coasting(&w, VesselId(1), t0(), earth.node, r, v, &sim::craft::small::rocket());
    let mut lead = s.clone();
    lead.advance(&w, t0().add_seconds(540.0), &controls, usize::MAX);
    let cases =
        [(&lead, 120.0, 1.0 / 60.0, 10), (&s, 86_400.0, 1_000.0 / 60.0, 30), (&s, 31.0 * 86_400.0, 1e6 / 60.0, 60)];
    for (start, span, dt, budget) in cases {
        let end = start.time.add_seconds(span);
        let mut once = start.clone();
        assert_eq!(once.advance(&w, end, &controls, usize::MAX), end);
        let mut framed = start.clone();
        fly_tight(&w, &mut framed, end, dt, &controls, budget);
        assert_same(&w, &framed, &once, &format!("frames of {dt} s, {budget} units"));
    }
}
