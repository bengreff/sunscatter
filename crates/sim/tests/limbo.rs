//! Limbo (`docs/technical/limbo.md`, D081): craft on rails under continuous
//! forces (thin-air drag, radiation pressure) evolve to the same
//! bits whatever the warp: one call, game frames at 1×, 1,000× and
//! 1,000,000× (the game's budgets and look-ahead), and across a save.

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, HoldMode, Phase, Vessel, VesselId, VesselIds};
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
