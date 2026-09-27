//! Proper time (D012, realism-1 §4a): the ship clock δ = τ − t against the
//! GPS numbers, and its determinism (chunked = single pass, save/load).

use sim::craft::test_craft;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, Vessel, VesselId, VesselIds};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(4.0 * 86_400.0)
}

/// A GPS-like vessel: circular, 20,200 km above the equator's radius, 55°.
fn gps(w: &World) -> (Vessel, f64) {
    let earth = w.find("Earth").unwrap();
    let a = 6_378_137.0 + 20_200_000.0;
    let (r, v) =
        Elements { a, e: 0.0, i: 55f64.to_radians(), raan: 0.3, argp: 0.0, mean_anomaly: 0.0 }.to_state(earth.gm);
    let period = std::f64::consts::TAU * (a * a * a / earth.gm).sqrt();
    (Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, test_craft()), period)
}

fn equator(w: &World) -> Vessel {
    Vessel::landed_at(w, VesselId(2), "Earth", 0.0, 0.0, t0(), test_craft())
}

#[test]
fn a_gps_clock_gains_38_6_microseconds_a_day_over_the_equator() {
    let w = world();
    let (mut sat, period) = gps(&w);
    let mut ground = equator(&w);
    // Two orbits (about a sidereal day): the satellite and the ground point
    // both come back, so the periodic terms of the barycentric motion
    // (v_Earth · r / c², tens of µs) cancel.
    let end = t0().add_seconds(2.0 * period);
    sat.advance(&w, end, &Controls::default(), usize::MAX);
    ground.advance(&w, end, &Controls::default(), usize::MAX);
    assert_eq!((sat.time, ground.time), (end, end));
    let gain = (sat.proper_time_offset() - ground.proper_time_offset()) / (2.0 * period) * 86_400.0 * 1e6;
    println!(
        "GPS gain {gain:.3} µs/day; ground {:.1} µs/day",
        ground.proper_time_offset() / (2.0 * period) * 86_400.0 * 1e6
    );
    assert!((gain / 38.6 - 1.0).abs() < 0.01, "{gain} µs/day");
    // Both clocks run slow against coordinate (barycentric) time: the Sun's
    // potential and Earth's orbital speed dominate.
    assert!(sat.proper_time_offset() < 0.0 && ground.proper_time_offset() < 0.0);
}

/// Advances in frames of `dt` (the last one ending at `end`).
fn frames(w: &World, v: &mut Vessel, end: Epoch, dt: f64) {
    let mut t = v.time;
    while t < end {
        t = t.add_seconds(dt);
        if t > end {
            t = end;
        }
        v.advance(w, t, &Controls::default(), usize::MAX);
    }
}

#[test]
fn chunked_equals_single_pass() {
    let w = world();
    let (sat, _) = gps(&w);
    let ground = equator(&w);
    for ship in [sat, ground] {
        let end = t0().add_seconds(86_400.0);
        let mut one = ship.clone();
        one.advance(&w, end, &Controls::default(), usize::MAX);
        for dt in [1.0 / 60.0 * 1e3, 777.7, 5_000.0] {
            let mut chunked = ship.clone();
            frames(&w, &mut chunked, end, dt);
            assert_eq!(chunked.proper_time_offset().to_bits(), one.proper_time_offset().to_bits(), "frames of {dt} s");
            assert_eq!(chunked, one);
        }
    }
}

#[test]
fn live_ticks_carry_the_clock() {
    // Thrusting from the pad and in orbit: every tick adds its increment.
    let w = world();
    let full = Controls { throttle: 1.0, sas: true, ..Controls::default() };
    let mut pad = equator(&w);
    pad.set_debug(&w, true);
    let (mut sat, _) = gps(&w);
    for ship in [&mut pad, &mut sat] {
        let mut single = ship.clone();
        let before = ship.proper_time_offset();
        let start = ship.time;
        for k in 1..=600 {
            ship.advance(&w, start.add_seconds(k as f64 / 60.0), &full, usize::MAX);
        }
        single.advance(&w, start.add_seconds(10.0), &full, usize::MAX);
        assert_eq!(*ship, single);
        // About 10 s at a rate of −1.5e-8 (the Sun's potential and Earth's
        // orbital speed): −0.15 µs.
        let d = ship.proper_time_offset() - before;
        assert!(d < -1.4e-7 && d > -1.7e-7, "{d} s in 10 s");
    }
}

#[test]
fn save_and_load_preserve_the_clock() {
    let w = world();
    let (mut sat, _) = gps(&w);
    let mut ground = equator(&w);
    let mid = t0().add_seconds(40_000.3);
    sat.advance(&w, mid, &Controls::default(), usize::MAX);
    ground.advance(&w, mid, &Controls::default(), usize::MAX);
    let mut ids = VesselIds::default();
    ids.allocate();
    ids.allocate();
    let fleet = [sat, ground];
    let text = SaveGame::capture(&w, mid, &fleet, ids, 0, Controls::default()).to_ron();
    let loaded = SaveGame::from_ron(&text).unwrap().vessels;
    let end = t0().add_seconds(86_400.0);
    for (mut before, mut after) in fleet.into_iter().zip(loaded) {
        assert_eq!(after.proper_time_offset().to_bits(), before.proper_time_offset().to_bits());
        assert!(after.proper_time_offset() != 0.0);
        before.advance(&w, end, &Controls::default(), usize::MAX);
        after.advance(&w, end, &Controls::default(), usize::MAX);
        assert_eq!(after, before);
    }
}
