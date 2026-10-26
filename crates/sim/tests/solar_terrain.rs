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
    ("Mercury", 0x86e4674effb8a8fb),
    ("Mars", 0x9a96c8e7420f2c0a),
    ("Io", 0x4459ca49f80b87ba),
    ("Europa", 0x63dbb3c0633ceff2),
    ("Ganymede", 0x805f8f0f51ae5d98),
    ("Callisto", 0x6c5fa8ece4a75d7c),
    ("Ceres", 0x066e50fa09dde3ed),
    ("Mimas", 0x5951e8f2fed99d96),
    ("Enceladus", 0x343132a28ee1e28a),
    ("Tethys", 0x60441854fc6cbf06),
    ("Dione", 0x0bd326c8b217a795),
    ("Rhea", 0x838eaa676c7851cd),
    ("Hyperion", 0x43b5dd863113f992),
    ("Iapetus", 0xb67dd00b4976e534),
    ("Phoebe", 0x0eb8f1a097e2e51c),
    ("Miranda", 0x33b37bf4c3fb3bde),
    ("Ariel", 0x3141ac1c04eba7ed),
    ("Umbriel", 0xdbc72ff21a52227e),
    ("Titania", 0x520451d5ab18d3f2),
    ("Oberon", 0x1e250132e7692c60),
    ("Triton", 0x52b98bdae2be78cc),
    ("Proteus", 0x7282cc788e8af6c4),
    ("Nereid", 0x7f031697ad5ac777),
    ("Pluto", 0x7441be1aa8cffebb),
    ("Charon", 0x1a87efcb88ac9daa),
    ("Eris", 0xd21301b895ff0f10),
    ("Dysnomia", 0x5f6487c9c4327ec5),
    ("Haumea", 0xbe93353621b3816d),
    ("Hiiaka", 0xe35da1601c799999),
    ("Makemake", 0x85368dd2628629ce),
    ("Gonggong", 0xc46e05808563057d),
    ("Quaoar", 0x676a3970e36b4618),
    ("Sedna", 0xf9457ac8a5f58aaf),
    ("Orcus", 0xe56a3a106debe69b),
    ("Vanth", 0x6166286c926fd04f),
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
