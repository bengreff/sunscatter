//! A vessel that goes live inside a rails-warp frame stops there
//! (`Vessel::advance_frame`, D081; `docs/technical/scale-performance.md`):
//! the game then caps the warp instead of flying thousands of live ticks in
//! one frame (BENCH's lunar transfer that crashed inside one 1,000,000x
//! frame took 16-18 s). The motion is the same as one jump.

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, Phase, Vessel, VesselId, LIVE_STOP, TICK};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(86_400.0)
}

/// The small rocket of parts (D121) 30 km above the Moon's surface,
/// falling at 500 m/s with 300 m/s across: it reaches the ground about a
/// minute later.
fn falling_onto_the_moon(w: &World) -> Vessel {
    let moon = w.find("Moon").unwrap();
    let rm = moon.physical.as_ref().unwrap().radius_eq;
    let r = DVec3::new(rm + 30_000.0, 0.0, 0.0);
    let v = DVec3::new(-500.0, 300.0, 0.0);
    let mut s = Vessel::coasting(w, VesselId(1), t0(), moon.node, r, v, &sim::craft::small::rocket());
    s.set_debug(w, true);
    s
}

#[test]
fn a_coast_onto_a_surface_stops_where_it_goes_live_in_a_rails_frame() {
    let w = world();
    let controls = Controls { sas: true, ..Controls::default() };
    let start = falling_onto_the_moon(&w);
    assert!(matches!(start.phase, Phase::Coasting { .. }));
    // One 1,000,000x frame: stops at the hand-over, live, far short of the
    // target, at once.
    let mut framed = start.clone();
    let target = t0().add_seconds(1e6 / 60.0);
    let clock = Instant::now();
    let (reached, _) = framed.advance_frame(&w, target, &controls, usize::MAX);
    let wall = clock.elapsed().as_secs_f64();
    assert!(matches!(framed.phase, Phase::Powered { .. }), "{:?}", framed.phase);
    assert!(target.seconds_since(reached) > LIVE_STOP, "stopped at the hand-over");
    assert!(reached.seconds_since(t0()) < 120.0, "{} s", reached.seconds_since(t0()));
    assert!(wall < 1.0, "the frame took {wall} s");
    // Then flown in physics-warp frames (4x, as the game caps it) to the
    // ground and on: the same bits as one jump.
    let end = t0().add_seconds(300.0);
    let mut clock = reached;
    while clock < end {
        let target = clock.add_seconds(4.0 / 60.0);
        let target = if target > end { end } else { target };
        let (r, _) = framed.advance_frame(&w, target, &controls, 2_000);
        // A warp stop (D128) holds the frame at its epoch; the game takes it
        // and the next frame goes on.
        let stop = framed.take_warp_stop();
        assert!(stop.is_some() || target.seconds_since(r) <= TICK, "a physics-warp frame flies through");
        clock = if stop.is_some() { r } else { target };
    }
    let mut jump = start.clone();
    jump.advance(&w, end, &controls, usize::MAX);
    // On the ground: at rest, or (a slender hull lying on its side has no
    // support polygon to rest on, `contact::holds`) lying within its reach
    // of the surface.
    let moon = w.find("Moon").unwrap().physical.as_ref().unwrap();
    let (_, r, _) = jump.state(&w);
    let h = moon.altitude_above_surface(moon.rotation.to_fixed(sim::frame::Vec3::from_raw(r), jump.time));
    let com = jump.mass_props().com;
    let reach = jump.craft.contacts.iter().map(|c| (c.pos - com).length()).fold(0.0, f64::max);
    let down = matches!(jump.phase, Phase::Landed { .. } | Phase::Crashed { .. }) || h < reach;
    assert!(down, "on the ground: {h} m up, {:?}", jump.phase);
    assert_eq!(framed.time, jump.time);
    assert_eq!(framed.phase, jump.phase);
    assert_eq!(framed.state(&w), jump.state(&w));
}

#[test]
fn an_engine_starting_in_a_long_frame_stops_there_and_a_short_one_flies_on() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let r = 6_378_137.0 + 400e3;
    let mut s = Vessel::coasting(
        &w,
        VesselId(2),
        t0(),
        earth.node,
        DVec3::new(r, 0.0, 0.0),
        DVec3::new(0.0, (earth.gm / r).sqrt(), 0.0),
        &sim::craft::small::rocket(),
    );
    // Its engines start through the sequence (D100).
    assert_eq!(s.next_stage(&w).unwrap().0, "Ignition");
    let burn = Controls { throttle: 1.0, sas: true, ..Controls::default() };
    // A long frame (the throttle would cap the warp; a frame already asked
    // for stops where the engine starts).
    let mut long = s.clone();
    let (reached, _) = long.advance_frame(&w, t0().add_seconds(16_667.0), &burn, usize::MAX);
    assert!(matches!(long.phase, Phase::Powered { .. }));
    assert_eq!(reached, t0(), "stopped where the engine started");
    // A physics-warp frame flies the burn.
    let mut short = s.clone();
    let target = t0().add_seconds(4.0 / 60.0);
    let (reached, _) = short.advance_frame(&w, target, &burn, usize::MAX);
    assert!(matches!(short.phase, Phase::Powered { .. }));
    assert!(target.seconds_since(reached) <= TICK);
}
