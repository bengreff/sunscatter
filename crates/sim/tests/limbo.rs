//! Limbo (`docs/technical/limbo.md`, D081): craft on rails under continuous
//! forces (low thrust, thin-air drag, radiation pressure) evolve to the same
//! bits whatever the warp: one call, game frames at 1×, 1,000× and
//! 1,000,000× (the game's budgets and look-ahead), and across a save.

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{
    BurnEnd, BurnLaw, Controls, DirectionLaw, FlightPlan, HoldMode, Phase, PlannedBurn, SegmentKind, Vessel, VesselId,
    VesselIds,
};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

const RE: f64 = 6_378_137.0;
const DAY: f64 = 86_400.0;

/// The game's per-frame budgets (`crates/game/src/state.rs`).
const COAST_STEPS_PER_FRAME: usize = 1_200;
const LOOKAHEAD_STEPS_PER_FRAME: usize = 1_500;

/// The small rocket of parts (`sim::craft::small`) coasting on `el`
/// (Earth), every tank at `fill`.
fn ship(w: &World, el: Elements, fill: f64) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (r, v) = el.to_state(earth.gm);
    let mut s = Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, &sim::craft::small::rocket());
    s.set_fill(w, fill);
    assert!(matches!(s.phase, Phase::Coasting { .. }));
    s
}

/// The test craft coasting on `el` with `propellant` kg: its planned
/// low-thrust burns wait for planned burns on craft of parts (D126).
fn test_ship(w: &World, el: Elements, propellant: f64) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (r, v) = el.to_state(earth.gm);
    let mut s = Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, sim::craft::test_craft());
    s.set_propellant(w, propellant);
    assert!(matches!(s.phase, Phase::Coasting { .. }));
    s
}

fn circular(height: f64, i: f64) -> Elements {
    Elements { a: RE + height, e: 0.0005, i, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }
}

fn sas(hold: HoldMode) -> Controls {
    Controls { sas: true, hold, ..Controls::default() }
}

/// Flies `v` from its time to `end` in frames of `dt` as the game does:
/// each frame advances with the game's budget (the clock held back where
/// the vessel falls behind), then looks a low orbit ahead.
fn fly(w: &World, v: &mut Vessel, end: Epoch, dt: f64, controls: &Controls) {
    let mut clock = v.time;
    while clock < end {
        let target = clock.add_seconds(dt);
        let target = if target > end { end } else { target };
        clock = v.advance(w, target, controls, COAST_STEPS_PER_FRAME);
        v.extend_coast(w, clock.add_seconds(6_000.0), LOOKAHEAD_STEPS_PER_FRAME);
    }
}

/// Two vessels flown to the same time are the same bits: state, attitude,
/// mass, temperatures and (extended alike) the stored trajectory.
fn assert_same(w: &World, a: &mut Vessel, b: &mut Vessel, what: &str) {
    assert_eq!(a.time, b.time, "{what}: time");
    assert_eq!(a.state(w), b.state(w), "{what}: state");
    assert_eq!(a.attitude, b.attitude, "{what}: attitude");
    assert_eq!(a.mass(), b.mass(), "{what}: mass");
    assert_eq!(a.thermal(), b.thermal(), "{what}: temperatures");
    if let (Some(ta), Some(tb)) = (a.trajectory(), b.trajectory()) {
        let until = if ta.computed_until() > tb.computed_until() { ta.computed_until() } else { tb.computed_until() };
        a.extend_coast(w, until, usize::MAX);
        b.extend_coast(w, until, usize::MAX);
    }
    assert_eq!(a, b, "{what}: vessel");
}

/// Flies a copy of `start` over `span` in one call and in frames of each
/// `dt`, and checks they agree bit for bit. Returns the one-call vessel.
fn same_at_every_warp(w: &World, start: &Vessel, span: f64, frames: &[f64], controls: &Controls) -> Vessel {
    let end = start.time.add_seconds(span);
    let mut jump = start.clone();
    assert_eq!(jump.advance(w, end, controls, usize::MAX), end);
    for &dt in frames {
        let mut framed = start.clone();
        fly(w, &mut framed, end, dt, controls);
        assert_same(w, &mut framed, &mut jump.clone(), &format!("frames of {dt} s"));
    }
    jump
}

/// A low-thrust burn: `thrust` N at Isp 3,000 s (an ion engine's) along
/// the orbital prograde of Earth from `at` for `days`.
fn ion_burn(w: &World, at: Epoch, thrust: f64, days: f64) -> PlannedBurn {
    let earth = w.find("Earth").unwrap().node;
    let direction = DirectionLaw::Tracking { reference: earth, axes: DVec3::X };
    PlannedBurn { t_start: at, law: BurnLaw::from_isp(thrust, 3_000.0, direction), end: BurnEnd::Duration(days * DAY) }
}

#[test]
fn a_month_of_low_thrust_is_the_same_at_every_warp() {
    let w = world();
    let mut s = test_ship(&w, circular(700_000.0, 0.9), 2_000.0);
    let burn = ion_burn(&w, t0().add_seconds(600.0), 0.5, 30.0);
    s.set_plan(&w, FlightPlan { burns: vec![burn] }).unwrap();
    let controls = sas(HoldMode::Prograde);
    // 1x for the turn and the first minute of thrust; then the month at
    // 1,000x (its first day) and 1,000,000x.
    let mut lead = s.clone();
    lead.advance(&w, t0().add_seconds(540.0), &controls, usize::MAX);
    same_at_every_warp(&w, &lead, 120.0, &[1.0 / 60.0], &controls);
    same_at_every_warp(&w, &s, DAY, &[1_000.0 / 60.0], &controls);
    let month = same_at_every_warp(&w, &s, 31.0 * DAY, &[1e6 / 60.0], &controls);
    // Mid-burn it is a perturbed segment: continuous thrust, the attitude
    // known (held on the burn's prograde law), not ticks.
    let mut mid = s.clone();
    mid.advance(&w, t0().add_seconds(5.0 * DAY), &controls, usize::MAX);
    let seg = mid.segment().unwrap();
    let limbo = seg.limbo().expect("a perturbed segment");
    assert!(matches!(seg.kind, SegmentKind::Burn(_)) && limbo.thrust.is_some() && limbo.pointing.is_known());
    let (_, _, v) = mid.state(&w);
    assert!((mid.attitude.nose() - v.normalize()).length() < 1e-9, "engine on the prograde");
    // The burn ran on rails as a perturbed segment, draining exactly
    // ṁ·t = F/(Isp·g0)·30 days.
    let drained = 2_000.0 - month.propellant();
    let expected = 0.5 / (3_000.0 * sim::vessel::G0) * 30.0 * DAY;
    assert!((drained - expected).abs() < 1e-9 * expected, "{drained} vs {expected}");
    assert!(matches!(month.phase, Phase::Coasting { .. }));
    println!("a month of 0.5 N: {drained:.3} kg of propellant");
}

#[test]
fn a_year_of_drag_decay_and_systems_is_the_same_at_every_warp() {
    let w = world();
    // 350 km: the small rocket full (~14 t, held prograde, end on) loses
    // tens of km in a year (lower, boiling off its LOX, it would reach the
    // live boundary within the year). Its systems run on their lattice: its
    // body-mounted cells charge its battery when the Sun is on their side
    // and it carries the loads in eclipse; its LOX boils off.
    let s = ship(&w, circular(350_000.0, 0.9), 1.0);
    let controls = sas(HoldMode::Prograde);
    same_at_every_warp(&w, &s, 60.0, &[1.0 / 60.0], &controls);
    same_at_every_warp(&w, &s, 2.0 * DAY, &[1_000.0 / 60.0], &controls);
    let year = same_at_every_warp(&w, &s, 365.0 * DAY, &[1e6 / 60.0], &controls);
    let earth = w.find("Earth").unwrap();
    let (_, r, v) = year.state(&w);
    let a = Elements::from_state(r, v, earth.gm).a;
    let (charge, boiled) = (year.ship().unwrap().batteries[0].charge, s.propellant() - year.propellant());
    println!(
        "a year at 350 km: semi-major axis {:.1} km lower; battery {:.2} MJ; {boiled:.0} kg boiled off",
        (RE + 350_000.0 - a) / 1e3,
        charge / 1e6,
    );
    assert!(a < RE + 350_000.0 - 5_000.0, "{a}");
    // Every lattice point of the year was stepped; the LOX boiled off
    // (the tanks' own law, D119) and the cells kept the battery up.
    assert_eq!(year.systems.epoch, Some(year.time));
    assert!(boiled > 10.0, "{boiled} kg");
    assert!(charge > 0.0, "{charge} J");
}

#[test]
fn systems_continue_bit_for_bit_after_a_save_between_lattice_points() {
    let w = world();
    let mut s = ship(&w, circular(400_000.0, 0.9), 0.5);
    let controls = sas(HoldMode::Stability);
    let mid = t0().add_seconds(5_000.0 + 12.345);
    fly(&w, &mut s, mid, 1.0 / 60.0 * 600.0, &controls);
    assert!(s.systems.epoch.unwrap() < mid, "between lattice points");
    let mut ids = VesselIds::default();
    ids.allocate();
    let text = SaveGame::capture(&w, mid, std::slice::from_ref(&s), ids, 0).to_ron();
    let mut loaded = SaveGame::from_ron(&text).unwrap().vessels.remove(0);
    assert_eq!(loaded, s);
    let end = t0().add_seconds(2.0 * DAY);
    fly(&w, &mut s, end, 1e3 / 60.0, &controls);
    fly(&w, &mut loaded, end, 1e6 / 60.0, &controls);
    assert_same(&w, &mut loaded, &mut s, "after load");
}

#[test]
fn a_year_of_radiation_pressure_is_the_same_at_every_warp() {
    let w = world();
    // 20,000 km: no air; light, eclipses twice a year. Stability hold and
    // torque-free tumbling.
    let el = Elements { a: RE + 20_000_000.0, e: 0.01, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
    let s = ship(&w, el, 0.05);
    for controls in [sas(HoldMode::Stability), Controls::default()] {
        let mut spun = s.clone();
        spun.set_attitude(sim::vessel::Attitude { q: spun.attitude.q, omega: DVec3::new(0.01, 0.002, 0.03) });
        same_at_every_warp(&w, &spun, 60.0, &[1.0 / 60.0], &controls);
        same_at_every_warp(&w, &spun, 3.0 * DAY, &[1_000.0 / 60.0], &controls);
        same_at_every_warp(&w, &spun, 365.0 * DAY, &[1e6 / 60.0], &controls);
    }
}

#[test]
fn a_low_thrust_burn_continues_bit_for_bit_after_save_and_load() {
    let w = world();
    let mut s = test_ship(&w, circular(700_000.0, 0.9), 2_000.0);
    let burn = ion_burn(&w, t0().add_seconds(600.0), 0.5, 10.0);
    s.set_plan(&w, FlightPlan { burns: vec![burn] }).unwrap();
    let controls = sas(HoldMode::Prograde);
    let mid = t0().add_seconds(3.3 * DAY);
    fly(&w, &mut s, mid, 1e6 / 60.0, &controls);
    assert!(matches!(s.segment().unwrap().kind, SegmentKind::Burn(_)), "mid-burn");
    let mut ids = VesselIds::default();
    ids.allocate();
    let text = SaveGame::capture(&w, mid, std::slice::from_ref(&s), ids, 0).to_ron();
    let mut loaded = SaveGame::from_ron(&text).unwrap().vessels.remove(0);
    assert_eq!(loaded, s);
    let end = t0().add_seconds(12.0 * DAY);
    fly(&w, &mut s, end, 1e6 / 60.0, &controls);
    fly(&w, &mut loaded, end, 1e6 / 60.0, &controls);
    assert_same(&w, &mut loaded, &mut s, "after load");
}
