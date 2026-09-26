//! A landed vessel with no throttle stays landed, whatever the frame rate or
//! warp (the owner saw Time to Ap flicker on the pad: a phase flip would show
//! it for a frame).

use sim::ephem::Ephemeris;
use sim::sol;
use sim::vessel::{Controls, Phase, Vessel, VesselId, VesselParams};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

#[test]
fn landed_vessel_without_throttle_never_leaves_the_ground() {
    let w = world();
    let start = sol::sol_epoch().add_seconds(86_400.0);
    for (dt, sas) in [(1.0 / 60.0, true), (1.0 / 60.0, false), (1.0 / 60.0 * 1e3, true), (1.0 / 60.0 * 1e6, true)] {
        let mut ship = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6082, -80.6041, start, VesselParams::block());
        let controls = Controls { sas, ..Default::default() };
        let mut t = start;
        for frame in 0..10_000 {
            t = t.add_seconds(dt);
            ship.advance(&w, t, &controls, 1_000);
            assert!(matches!(ship.phase, Phase::Landed { .. }), "dt {dt}: left the ground at frame {frame}");
        }
    }
}
