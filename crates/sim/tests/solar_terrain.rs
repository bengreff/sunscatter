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
const BODIES: [&str; 37] = [
    "Mercury",
    "Venus",
    "Titan",
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
    ("Mercury", 0x93a1f0228621cd2b),
    ("Venus", 0xb1930e228f811eb8),
    ("Titan", 0x22da9dd8aa11f878),
    ("Mars", 0x9a96c8e7420f2c0a),
    ("Io", 0xb8196ba6b803372d),
    ("Europa", 0x1345978025925c60),
    ("Ganymede", 0x425cae0733d6a875),
    ("Callisto", 0x78b66183e9bae787),
    ("Ceres", 0x441b3556d8689807),
    ("Mimas", 0x55504db77ae1d07b),
    ("Enceladus", 0x51f8ff0a4206ddda),
    ("Tethys", 0xe6baa306cc57fadc),
    ("Dione", 0xe76d61b4102f11cf),
    ("Rhea", 0x8d2d4fda24c7e119),
    ("Hyperion", 0xdd2817bdb5622693),
    ("Iapetus", 0xb0cf82d5bd9d8527),
    ("Phoebe", 0x0fc112aca9edc752),
    ("Miranda", 0x6b570d4b87238a29),
    ("Ariel", 0xf3cb6eb85d46a6b1),
    ("Umbriel", 0x1909f2ed71e8b4cd),
    ("Titania", 0x902cc0e02b6c7809),
    ("Oberon", 0xb9af9fea71bd5e47),
    ("Triton", 0x52b98bdae2be78cc),
    ("Proteus", 0x6e0e8912a16e527b),
    ("Nereid", 0x2f031db56b805e45),
    ("Pluto", 0xb119eb46ba2d64d6),
    ("Charon", 0x0c7cd92e91b0696f),
    ("Eris", 0x2ea4d37a075f6edb),
    ("Dysnomia", 0x4b8b83ffb9686b7c),
    ("Haumea", 0xfe43feaa4759f6c7),
    ("Hiiaka", 0x29bae9af82126ce8),
    ("Makemake", 0x552584dfc4ff3020),
    ("Gonggong", 0x7a3f090d7cd32983),
    ("Quaoar", 0x4df462efee797f58),
    ("Sedna", 0x89db7969780fafa7),
    ("Orcus", 0xc7f0109760a71e15),
    ("Vanth", 0xc59405b25cf6e436),
];

/// Known features at their measured heights above the body's reference
/// spheroid (`body.ron`; the sources' own datums differ: MOLA's areoid puts
/// Olympus Mons at 21.2 km and the Hellas floor at −8.2 km).
#[test]
fn landmarks_sit_at_their_measured_heights() {
    let w = world();
    // (body, feature, lat, lon east, lowest, highest) in metres.
    let table: [(&str, &str, f64, f64, f64, f64); 5] = [
        ("Mars", "Olympus Mons summit", 17.34, -133.03, 22_000.0, 23_500.0),
        ("Mars", "Hellas Planitia, deepest floor", -32.84, 62.22, -8_000.0, -7_000.0),
        ("Mercury", "Rachmaninoff basin floor", 27.64, 57.0, -6_500.0, -5_000.0),
        ("Mercury", "Caloris Planitia", 30.0, 162.0, -2_500.0, 0.0),
        // Magellan: ~11 km above the 6,051 km sphere, 10.2 km above body.ron's 6,051.8 km; smoothed at 9.3 km.
        ("Venus", "Maxwell Montes summit", 65.2, 3.3, 8_500.0, 11_000.0),
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
