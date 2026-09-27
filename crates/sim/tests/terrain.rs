//! Terrain physics (D047): landing on terrain versus the solid sea, and the
//! shipped heightmaps sampled deterministically.

use glam::DVec3;
use sim::body::BodyPhysical;
use sim::ephem::Ephemeris;
use sim::frame::{BodyFixed, Vec3};
use sim::sol;
use sim::terrain::Heightmap;
use sim::time::Epoch;
use sim::vessel::{Controls, Phase, Vessel, VesselId};
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
    let earth = w.sources.iter_mut().find(|s| s.name == "Earth").unwrap().physical.as_mut().unwrap();
    earth.terrain = Some(Arc::new(Heightmap::from_vec(width, height, data)));
    earth.detail = None;
    w
}

fn earth(w: &World) -> &BodyPhysical {
    w.find("Earth").unwrap().physical.as_ref().unwrap()
}

/// Drops the test craft under parachute from `above_ground` metres over the
/// surface at (lat, lon) and returns where it came to rest.
fn parachute_drop(w: &World, lat: f64, lon: f64, above_ground: f64) -> Vessel {
    let e = earth(w);
    let t = t0();
    let fixed = e.ground_point(lat * DEG, lon * DEG, above_ground);
    let r = e.rotation.to_inertial(fixed, t).raw();
    let v = e.rotation.omega(t).raw().cross(r);
    let node = w.find("Earth").unwrap().node;
    let mut ship = Vessel::coasting(w, VesselId(1), t, node, r, v, sim::craft::test_craft());
    // Debug mode: at its full mass the test craft's parachute lands it
    // faster than its impact limit (this test is about where it lands).
    ship.set_debug(w, true);
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
    let contact = sim::craft::test_craft().params().contact_height(16_000.0);
    // Resting on its gear: the feet sink m·g / 4k ≈ 12 cm on Earth.
    let resting = |ground: f64, h: f64| h < ground + contact && h > ground + contact - 0.2;
    // Plateau: comes to rest on the terrain, 3 km above the ellipsoid.
    let ship = parachute_drop(&w, 20.0, 10.0, 4_000.0);
    assert!(resting(3_000.0, landed_height(&w, &ship)), "{}", landed_height(&w, &ship));
    // Sea: the floor is at −4 km, but the ocean is solid at sea level (D037).
    let ship = parachute_drop(&w, -20.0, 10.0, 4_000.0);
    assert!(resting(0.0, landed_height(&w, &ship)), "{}", landed_height(&w, &ship));
}

#[test]
fn vessels_start_on_the_terrain() {
    let w = plateau_world();
    let contact = sim::craft::test_craft().params().contact_height(16_000.0);
    let high = Vessel::landed_at(&w, VesselId(1), "Earth", 30.0, 0.0, t0(), sim::craft::test_craft());
    assert!((landed_height(&w, &high) - (3_000.0 + contact)).abs() < 1e-6);
    let sea = Vessel::landed_at(&w, VesselId(1), "Earth", -30.0, 0.0, t0(), sim::craft::test_craft());
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
    let mut ship = Vessel::coasting(&w, VesselId(1), t, w.find("Earth").unwrap().node, r, v, sim::craft::test_craft());
    ship.advance(&w, t.add_seconds(600.0), &Controls::default(), usize::MAX);
    match ship.phase {
        // A hull or foot point hit the plateau: the centre of mass is
        // within the craft's reach of it (the fall is flown live with its
        // aerodynamics, so it may hit tilted), not near the ellipsoid.
        Phase::Crashed { fixed, .. } => {
            let h = e.altitude(fixed) - 3_000.0;
            let reach = ship.craft.contact_reach(ship.propellant());
            assert!(h > ship.contact_height() - 1.0 && h < reach, "{h} m");
        }
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
        let ship = Vessel::landed_at(&w, VesselId(1), "Earth", 27.9881, 86.925, t0(), sim::craft::test_craft());
        let Phase::Landed { fixed, .. } = ship.phase else { unreachable!() };
        let surface = e.surface_height_latlon(27.9881 * DEG, 86.925 * DEG);
        assert!((e.altitude(fixed) - surface - ship.contact_height()).abs() < 1.0);
        assert!((surface - everest).abs() < 500.0, "detail stays near the base: {surface} vs {everest}");
    }
    if let Some(m) = shipped(&w, "Moon") {
        let tycho = height(&m, -43.31, -11.36);
        println!("Tycho centre {tycho:.0} m");
        assert!(tycho < -1_000.0, "{tycho}");
    }
}

/// Pseudo-random directions (xorshift, fixed seed).
fn directions(n: usize) -> Vec<DVec3> {
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    (0..n).map(|_| DVec3::new(next(), next(), next())).collect()
}

/// Cross-platform determinism on the committed data (CI: macOS and Windows):
/// the whole surface, base (bicubic) + detail + solid sea.
#[test]
fn shipped_heightmap_samples_match_golden_hash() {
    let w = world();
    let Some(e) = shipped(&w, "Earth") else { return };
    assert!(e.detail.is_some(), "Earth ships with terrain detail");
    let mut bytes = Vec::new();
    for dir in directions(100_000) {
        bytes.extend_from_slice(&e.surface_height(Vec3::from_raw(dir)).to_bits().to_le_bytes());
    }
    let hash = sim::ephem::fnv1a64(&bytes);
    println!("Earth surface sample hash: {hash:#018x}");
    assert_eq!(hash, EARTH_GOLDEN, "terrain sampling, detail or data changed (or differs on this platform)");
}

// Base + procedural detail (D059); was the bicubic heightmap alone.
const EARTH_GOLDEN: u64 = 0xe9f34b76e0848548;

#[test]
fn detail_is_zero_over_water_and_present_on_rough_land() {
    let w = world();
    let Some(e) = shipped(&w, "Earth") else { return };
    let map = e.terrain.as_ref().unwrap();
    let mut rough_land = 0;
    for dir in directions(20_000) {
        let (lat, lon) = sim::terrain::lat_lon(dir);
        let base = map.sample(lat, lon);
        let detail = e.terrain_height(dir) - base;
        if base <= 0.0 {
            assert_eq!(detail, 0.0, "detail over water at {dir}");
            assert_eq!(e.surface_height(Vec3::from_raw(dir)), 0.0);
        } else if base > 2_000.0 && detail.abs() > 1.0 {
            rough_land += 1;
        }
    }
    assert!(rough_land > 100, "{rough_land}");
}

/// No steps: heights of directions 1e-10 rad apart (0.6 mm on Earth) differ
/// by < 1 mm, across the antimeridian, at the poles and at random places.
/// (1e-9 rad is 6 mm, where real steep detail legitimately rises more.)
#[test]
fn shipped_surfaces_are_continuous() {
    let w = world();
    for body in ["Earth", "Moon"] {
        let Some(p) = shipped(&w, body) else { continue };
        let eps = 1e-10;
        let mut pairs = vec![
            (DVec3::new(-1.0, eps, 0.3), DVec3::new(-1.0, -eps, 0.3)),
            (DVec3::new(-1.0, eps, -0.6), DVec3::new(-1.0, -eps, -0.6)),
            (DVec3::new(eps, 0.0, 1.0), DVec3::new(-eps, 0.0, 1.0)),
            (DVec3::new(0.0, eps, 1.0), DVec3::new(0.0, -eps, 1.0)),
            (DVec3::new(eps, eps, -1.0), DVec3::new(-eps, -eps, -1.0)),
        ];
        for a in directions(5_000) {
            let a = a.normalize();
            pairs.push((a, (a + a.any_orthonormal_vector() * eps).normalize()));
        }
        for (a, b) in pairs {
            let d = (p.terrain_height(a) - p.terrain_height(b)).abs();
            assert!(d < 1e-3, "{body} {a} vs {b}: step {d} m");
        }
    }
}

/// D062: every shipped body's rails floor follows the rule (the data cannot
/// drift from it when the maps or the atmosphere change).
#[test]
fn rails_floors_follow_the_rule() {
    let w = world();
    for name in ["Earth", "Moon"] {
        let Some(p) = shipped(&w, name) else { return };
        let max_terrain = p.terrain.as_ref().map_or(0.0, |t| f64::from(*t.data().iter().max().unwrap()));
        let rule = sim::body::rails_floor_rule(p.atmosphere.as_ref().map(|a| a.top), max_terrain);
        assert_eq!(p.rails_floor, rule, "{name}");
        assert!(p.rails_floor > 0.0);
    }
}
