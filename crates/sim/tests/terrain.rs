//! Terrain physics (D047): landing on terrain versus the solid sea, and the
//! shipped heightmaps sampled deterministically.

use glam::DVec3;
use sim::body::BodyPhysical;
use sim::ephem::Ephemeris;
use sim::frame::{BodyFixed, Vec3};
use sim::sol;
use sim::terrain::Heightmap;
use sim::time::Epoch;
use sim::vessel::{Controls, Phase, Vessel, VesselParams};
use sim::world::World;
use std::sync::Arc;

const DEG: f64 = std::f64::consts::PI / 180.0;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

/// Earth with synthetic terrain: a 3,000 m plateau north of the equator and
/// a −4,000 m sea floor south of it.
fn plateau_world() -> World {
    let mut w = world();
    let (width, height) = (64, 32);
    let data = (0..width * height).map(|k| if k / width < height / 2 { 3_000 } else { -4_000 }).collect();
    let earth = w.sources.iter_mut().find(|s| s.name == "Earth").unwrap();
    earth.physical.as_mut().unwrap().terrain = Some(Arc::new(Heightmap::from_vec(width, height, data)));
    w
}

fn earth(w: &World) -> &BodyPhysical {
    w.find("Earth").unwrap().physical.as_ref().unwrap()
}

/// Drops the block under parachute from `above_ground` metres over the
/// surface at (lat, lon) and returns where it came to rest.
fn parachute_drop(w: &World, lat: f64, lon: f64, above_ground: f64) -> Vessel {
    let e = earth(w);
    let t = t0();
    let fixed = e.ground_point(lat * DEG, lon * DEG, above_ground);
    let r = e.rotation.to_inertial(fixed, t).raw();
    let v = e.rotation.omega(t).raw().cross(r);
    let node = w.find("Earth").unwrap().node;
    let mut ship = Vessel::coasting(w, t, node, r, v, VesselParams::block());
    let controls = Controls { chute: true, sas: true, ..Default::default() };
    ship.advance(w, t.add_seconds(3_600.0), &controls, usize::MAX);
    ship
}

fn landed_height(w: &World, ship: &Vessel) -> f64 {
    match &ship.phase {
        Phase::Landed { fixed, .. } => earth(w).altitude(*fixed),
        other => panic!("expected a landing, got {other:?}"),
    }
}

#[test]
fn landing_on_a_mountain_versus_the_sea() {
    let w = plateau_world();
    let contact = VesselParams::block().contact_height;
    // Plateau: comes to rest on the terrain, 3 km above the ellipsoid.
    let ship = parachute_drop(&w, 20.0, 10.0, 4_000.0);
    assert!((landed_height(&w, &ship) - (3_000.0 + contact)).abs() < 1e-6, "{}", landed_height(&w, &ship));
    // Sea: the floor is at −4 km, but the ocean is solid at sea level (D037).
    let ship = parachute_drop(&w, -20.0, 10.0, 4_000.0);
    assert!((landed_height(&w, &ship) - contact).abs() < 1e-6, "{}", landed_height(&w, &ship));
}

#[test]
fn vessels_start_on_the_terrain() {
    let w = plateau_world();
    let contact = VesselParams::block().contact_height;
    let high = Vessel::landed_at(&w, "Earth", 30.0, 0.0, t0(), VesselParams::block());
    assert!((landed_height(&w, &high) - (3_000.0 + contact)).abs() < 1e-6);
    let sea = Vessel::landed_at(&w, "Earth", -30.0, 0.0, t0(), VesselParams::block());
    assert!((landed_height(&w, &sea) - contact).abs() < 1e-6);
    // Standing still is not contact: the vessel stays landed.
    let mut v = high.clone();
    v.advance(&w, t0().add_seconds(60.0), &Controls::default(), usize::MAX);
    assert!(matches!(v.phase, Phase::Landed { .. }));
    // Without an ocean (the Moon), the surface is the terrain itself.
    let mut moon = earth(&w).clone();
    moon.sea_level = None;
    let p: Vec3<BodyFixed> = moon.surface_point(-30.0 * DEG, 0.0, 0.0);
    assert!((moon.surface_height(p) + 4_000.0).abs() < 1e-9);
}

#[test]
fn falling_onto_a_mountain_is_detected_at_its_height() {
    // Free fall, no parachute: the crash is at the plateau, not the ellipsoid.
    let w = plateau_world();
    let e = earth(&w);
    let t = t0();
    let fixed = e.ground_point(25.0 * DEG, 40.0 * DEG, 2_000.0);
    let r = e.rotation.to_inertial(fixed, t).raw();
    let v = e.rotation.omega(t).raw().cross(r);
    let mut ship = Vessel::coasting(&w, t, w.find("Earth").unwrap().node, r, v, VesselParams::block());
    ship.advance(&w, t.add_seconds(600.0), &Controls::default(), usize::MAX);
    match ship.phase {
        Phase::Crashed { fixed, .. } => assert!((e.altitude(fixed) - 3_005.0).abs() < 1e-6),
        other => panic!("expected a crash, got {other:?}"),
    }
}

/// Real data (skipped when the heightmaps are not present).
fn shipped(w: &World, body: &str) -> Option<BodyPhysical> {
    let p = w.find(body).unwrap().physical.clone().unwrap();
    if p.terrain.is_none() {
        println!("no {body} heightmap: skipping");
        return None;
    }
    Some(p)
}

fn height(p: &BodyPhysical, lat: f64, lon: f64) -> f64 {
    p.terrain.as_ref().unwrap().sample(lat * DEG, lon * DEG)
}

#[test]
fn shipped_heightmaps_have_the_expected_landmarks() {
    let w = world();
    if let Some(e) = shipped(&w, "Earth") {
        let everest = height(&e, 27.9881, 86.925);
        let deep = height(&e, 11.3733, 142.5917);
        let pad = height(&e, 28.6082, -80.6041);
        println!("Everest {everest:.0} m, Challenger Deep {deep:.0} m, LC-39A {pad:.1} m");
        assert!(everest > 5_000.0, "{everest}");
        assert!(deep < -9_000.0, "{deep}");
        assert!(pad.abs() < 20.0, "{pad}");
        assert_eq!(e.surface_height_latlon(11.3733 * DEG, 142.5917 * DEG), 0.0, "sea is solid at 0");
        let ship = Vessel::landed_at(&w, "Earth", 27.9881, 86.925, t0(), VesselParams::block());
        let Phase::Landed { fixed, .. } = ship.phase else { unreachable!() };
        assert!((e.altitude(fixed) - everest - 5.0).abs() < 1.0);
    }
    if let Some(m) = shipped(&w, "Moon") {
        let tycho = height(&m, -43.31, -11.36);
        println!("Tycho centre {tycho:.0} m");
        assert!(tycho < -1_000.0, "{tycho}");
    }
}

/// Cross-platform determinism on the committed data (CI: macOS and Windows).
#[test]
fn shipped_heightmap_samples_match_golden_hash() {
    let w = world();
    let Some(e) = shipped(&w, "Earth") else { return };
    let map = e.terrain.as_ref().unwrap();
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    let mut bytes = Vec::new();
    for _ in 0..100_000 {
        let dir = DVec3::new(next(), next(), next());
        bytes.extend_from_slice(&map.height_at(dir).to_bits().to_le_bytes());
    }
    let hash = sim::ephem::fnv1a64(&bytes);
    println!("Earth terrain sample hash: {hash:#018x}");
    assert_eq!(hash, EARTH_GOLDEN, "terrain sampling or data changed (or differs on this platform)");
}

// Catmull-Rom bicubic sampling with the pole blend (D059), was bilinear.
const EARTH_GOLDEN: u64 = 0x581153ae2859922d;
