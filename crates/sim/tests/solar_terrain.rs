//! The Solar System's solid surfaces (LANDS, 2026-10-26;
//! `docs/features/solar-visuals.md`): every body that declares a heightmap
//! loads it with its procedural detail, known features sit at their
//! measured heights, the rails floors follow D062's rule, and sampling is
//! bit-identical across platforms (golden hashes per body).

use glam::DVec3;
use sim::body::BodyPhysical;
use sim::ephem::Ephemeris;
use sim::frame::Vec3;
use sim::sol;
use sim::world::World;
use std::sync::Arc;

const DEG: f64 = std::f64::consts::PI / 180.0;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn body(w: &World, name: &str) -> BodyPhysical {
    w.find(name).unwrap().physical.clone().unwrap()
}

/// Height of the heightmap (no detail) at latitude/longitude in degrees east.
fn base(p: &BodyPhysical, lat: f64, lon: f64) -> f64 {
    p.terrain.as_ref().unwrap().sample(lat * DEG, lon * DEG)
}

/// Bodies whose maps this test pins (the solid bodies with a heightmap,
/// besides Earth and the Moon, which `terrain.rs` covers).
const BODIES: [&str; 35] = [
    "Mercury",
    "Mars",
    "Io",
    "Europa",
    "Ganymede",
    "Callisto",
    "Ceres",
    "Mimas",
    "Enceladus",
    "Tethys",
    "Dione",
    "Rhea",
    "Hyperion",
    "Iapetus",
    "Phoebe",
    "Miranda",
    "Ariel",
    "Umbriel",
    "Titania",
    "Oberon",
    "Triton",
    "Proteus",
    "Nereid",
    "Pluto",
    "Charon",
    "Eris",
    "Dysnomia",
    "Haumea",
    "Hiiaka",
    "Makemake",
    "Gonggong",
    "Quaoar",
    "Sedna",
    "Orcus",
    "Vanth",
];

fn shipped(w: &World) -> Vec<(&'static str, BodyPhysical)> {
    BODIES
        .iter()
        .filter_map(|&n| {
            let name = if n == "Hiiaka" { "Hi'iaka" } else { n };
            let p = body(w, name);
            p.terrain.is_some().then_some((n, p))
        })
        .collect()
}

#[test]
fn heightmaps_load_with_their_detail() {
    let w = world();
    let all = shipped(&w);
    for (name, p) in &all {
        assert!(p.solid, "{name}");
        assert!(p.detail.is_some(), "{name}: heightmap without terrain detail");
    }
    println!("{} bodies with terrain", all.len());
}

/// D062: the rails floor of an airless body is its highest terrain plus
/// 2 km, rounded up to a kilometre (the atmosphere's top with one).
#[test]
fn rails_floors_follow_the_rule() {
    let w = world();
    for (name, p) in shipped(&w) {
        let t = p.terrain.as_ref().unwrap();
        let max = f64::from(*t.data().iter().max().unwrap()) * t.scale();
        let rule = sim::body::rails_floor_rule(p.atmosphere.as_ref().map(|a| a.top), max);
        assert_eq!(p.rails_floor, rule, "{name}");
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

/// Cross-platform determinism (CI: macOS and Windows): the whole surface,
/// heightmap (bicubic) plus detail, at 2,000 directions per body.
#[test]
fn surfaces_match_golden_hashes() {
    let w = world();
    let mut failed = Vec::new();
    for (name, p) in shipped(&w) {
        let mut bytes = Vec::new();
        for dir in directions(2_000) {
            bytes.extend_from_slice(&p.surface_height(Vec3::from_raw(dir)).to_bits().to_le_bytes());
        }
        let hash = sim::ephem::fnv1a64(&bytes);
        let golden = GOLDEN.iter().find(|g| g.0 == name).map(|g| g.1);
        println!("(\"{name}\", {hash:#018x}),");
        if golden != Some(hash) {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "surface sampling, detail or data changed (or differs here): {failed:?}");
}

// Base + procedural detail, 2,000 directions; regenerate (printed by this
// test) when a body's maps or terrain parameters change on purpose.
const GOLDEN: &[(&str, u64)] = &[
    ("Mercury", 0xc23224e3b746c2bc),
    ("Mars", 0x9803d40ea9d5b0f3),
    ("Io", 0x4459ca49f80b87ba),
    ("Europa", 0x9887d88ed3bee3e8),
    ("Ganymede", 0x96d90aafabf52ec9),
    ("Callisto", 0x7e65a743650384d1),
    ("Ceres", 0xf84d047d37ff734b),
    ("Mimas", 0xb1f04858edc9a5b3),
    ("Enceladus", 0xbb5fded83e0076bd),
    ("Tethys", 0xf0699de43f9c0f27),
    ("Dione", 0x09ebd7027392fcbf),
    ("Rhea", 0xed70e45a08def698),
    ("Hyperion", 0xf2f6f79b0837c7d9),
    ("Iapetus", 0xcd1ac2409e762ae1),
    ("Phoebe", 0x0e83c1f0e6d64c15),
    ("Miranda", 0x18824854369eb73c),
    ("Ariel", 0xd347ec2040d1c382),
    ("Umbriel", 0x6ddc33123f5256b2),
    ("Titania", 0x40302e1a3d92f2a6),
    ("Oberon", 0xfb5711fe7443ea57),
    ("Triton", 0x5a4718930ba9ce2c),
    ("Proteus", 0x100b9f225d34b69a),
    ("Nereid", 0x3e6bc4907924acc1),
    ("Pluto", 0xa36b341f4aade44b),
    ("Charon", 0x4b25af4a7fb6320f),
    ("Eris", 0x74267c1196c76060),
    ("Dysnomia", 0xcfd891db23193b48),
    ("Haumea", 0xe1dc5e0eccd009b6),
    ("Hiiaka", 0x1e8f89717b46bff8),
    ("Makemake", 0x93dcd5771ec1f046),
    ("Gonggong", 0x76009353b1154559),
    ("Quaoar", 0xc06ec74563381bdc),
    ("Sedna", 0x04adb0ffd0408bba),
    ("Orcus", 0xdefbf780f54ba4a2),
    ("Vanth", 0xead3c81ac8558b59),
];

/// Known features at their measured heights above the body's reference
/// spheroid (`body.ron`; the sources' own datums differ: MOLA's areoid puts
/// Olympus Mons at 21.2 km and the Hellas floor at −8.2 km).
#[test]
fn landmarks_sit_at_their_measured_heights() {
    let w = world();
    // (body, feature, lat, lon east, lowest, highest) in metres.
    let table: [(&str, &str, f64, f64, f64, f64); 4] = [
        ("Mars", "Olympus Mons summit", 17.34, -133.03, 22_000.0, 23_500.0),
        ("Mars", "Hellas Planitia, deepest floor", -32.84, 62.22, -8_000.0, -7_000.0),
        ("Mercury", "Rachmaninoff basin floor", 27.64, 57.0, -6_500.0, -5_000.0),
        ("Mercury", "Caloris Planitia", 30.0, 162.0, -2_500.0, 0.0),
    ];
    for (name, feature, lat, lon, lo, hi) in table {
        let p = body(&w, name);
        if p.terrain.is_none() {
            println!("no {name} heightmap: skipping");
            continue;
        }
        let h = base(&p, lat, lon);
        println!("{name} {feature}: {h:.0} m");
        assert!((lo..=hi).contains(&h), "{name} {feature}: {h:.0} m, expected {lo}..{hi}");
    }
}
