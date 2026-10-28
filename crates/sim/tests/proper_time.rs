//! Proper time (D012, D088, realism-1 §4a): the ship clock δ = τ − t
//! (t TDB) against the GPS and ISS numbers, a site's clock against TT, and
//! determinism (chunked = single pass, save/load).

use sim::craft::test_craft;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::relativity::tdb_minus_tt;
use sim::save::SaveGame;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, SiteClock, Vessel, VesselId, VesselIds};
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
    let per_day = |d: f64| d / (2.0 * period) * 86_400.0 * 1e6;
    let gain = per_day(sat.proper_time_offset() - ground.proper_time_offset());
    // The ground clock keeps TT: it follows TT − TDB's annual term (−29
    // µs/day in early January) within the topocentric terms (~2 µs) and
    // the point-mass potential's missing J2 (33 ns/day at the equator).
    let tt = per_day(tdb_minus_tt(t0()) - tdb_minus_tt(end));
    let ground_rate = per_day(ground.proper_time_offset());
    println!("GPS gain {gain:.3} µs/day; ground {ground_rate:.2} µs/day, TT {tt:.2}");
    assert!((gain / 38.6 - 1.0).abs() < 0.01, "{gain} µs/day");
    assert!((ground_rate - tt).abs() < 3.0, "{ground_rate} vs {tt} µs/day");
}

/// A clock at the launch site (Kennedy, 28.6° N) set to TT reads TT through
/// a year: the integrated rule matches the IAU series (geocentric, ~10 µs)
/// within 6.4 µs, the site's own diurnal term (v_Earth · r_site / c², ±1.8
/// µs) included, over a swing of 3.3 ms. Without L_B it lost 0.49 s.
#[test]
fn the_site_clock_reads_tt_through_a_year() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let fixed = p.surface_point(28.6082f64.to_radians(), (-80.66f64).to_radians(), 0.0);
    let start = sol::sol_epoch();
    let mut site = SiteClock::reading_tt(earth.node, fixed, start);
    let mut worst: f64 = 0.0;
    for day in (0..=365).step_by(5) {
        let t = start.add_seconds(f64::from(day) * 86_400.0);
        let tt = -tdb_minus_tt(t);
        worst = worst.max((site.offset_at(&w, t) - tt).abs());
    }
    println!("site clock − TT: at most {:.2} µs over a year", worst * 1e6);
    assert!(worst < 10e-6, "{worst} s");
    // Any order of calls gives the same bits (a pure function of time).
    let mid = start.add_seconds(100.3 * 86_400.0);
    let mut fresh = SiteClock::reading_tt(earth.node, fixed, start);
    assert_eq!(fresh.offset_at(&w, mid).to_bits(), site.offset_at(&w, mid).to_bits());
}

/// An ISS-like clock (400 km, 51.6°) loses ≈ 25 µs a day against the
/// launch site's: −28.3 from its speed, +3.5 from the shallower potential,
/// +0.1 for the site's own rotation.
#[test]
fn an_iss_clock_loses_25_microseconds_a_day_against_the_site() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let p = earth.physical.as_ref().unwrap();
    let fixed = p.surface_point(28.6082f64.to_radians(), (-80.66f64).to_radians(), 0.0);
    let a = 6_371_000.0 + 400_000.0;
    let el = Elements { a, e: 0.0, i: 51.6f64.to_radians(), raan: 0.3, argp: 0.0, mean_anomaly: 0.0 };
    let (r, v) = el.to_state(earth.gm);
    let period = std::f64::consts::TAU * (a * a * a / earth.gm).sqrt();
    let mut site = SiteClock::reading_tt(earth.node, fixed, t0());
    let mut iss = Vessel::coasting(&w, VesselId(3), t0(), earth.node, r, v, test_craft());
    iss.set_clock_offset(&w, site.offset_at(&w, t0()));
    // 16 orbits, about a day: the orbital periodic terms come back.
    let end = t0().add_seconds(16.0 * period);
    iss.advance(&w, end, &Controls::default(), usize::MAX);
    let loss = (iss.proper_time_offset() - site.offset_at(&w, end)) / (16.0 * period) * 86_400.0 * 1e6;
    println!("ISS against the site: {loss:.2} µs/day");
    assert!((loss + 24.7).abs() < 0.6, "{loss} µs/day");
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
        // About 10 s against TDB: TT's rate on the pad (−29 µs/day in
        // early January) and, in orbit, GPS's gain plus the periodic term of
        // its speed along Earth's (±1.3e-9): tens of nanoseconds at most.
        let d = ship.proper_time_offset() - before;
        assert!(d != 0.0 && d.abs() < 2e-8, "{d} s in 10 s");
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
    let text = SaveGame::capture(&w, mid, &fleet, ids, 0).to_ron();
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
