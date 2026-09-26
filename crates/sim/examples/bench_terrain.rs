//! Cost of the physical surface (D059 budget: `surface_height` ≤ 2 µs per call,
//! a 33×33 chunk ≤ 2 ms). `cargo run -p sim --release --example bench_terrain`
//!
//! 2026-09-26, reference Mac: Earth 0.56 µs global, 1.36 µs on rough land
//! (base alone 0.12 µs); a 33×33 chunk 0.44 ms; Moon 1.3 µs.
use glam::DVec3;
use sim::body::BodyPhysical;
use sim::ephem::Ephemeris;
use sim::frame::Vec3;
use sim::math;
use sim::sol;
use sim::world::World;
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

const DEG: f64 = std::f64::consts::PI / 180.0;

/// Mean µs per `surface_height` call over `dirs` (after one warm-up pass).
fn per_call(p: &BodyPhysical, dirs: &[DVec3]) -> f64 {
    let run = || dirs.iter().map(|&d| p.surface_height(Vec3::from_raw(d))).sum::<f64>();
    black_box(run());
    let clock = Instant::now();
    let reps = 5;
    for _ in 0..reps {
        black_box(run());
    }
    clock.elapsed().as_secs_f64() / (reps * dirs.len()) as f64 * 1e6
}

fn dir(lat: f64, lon: f64) -> DVec3 {
    let (lat, lon) = (lat * DEG, lon * DEG);
    DVec3::new(math::cos(lat) * math::cos(lon), math::cos(lat) * math::sin(lon), math::sin(lat))
}

fn main() {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let w = World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()));
    let n = 200_000;
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    let global: Vec<DVec3> = (0..n).map(|_| DVec3::new(next() - 0.5, next() - 0.5, next() - 0.5)).collect();
    // Himalaya box: rough land, every octave active (the worst case).
    let rough: Vec<DVec3> = (0..n).map(|_| dir(27.0 + 3.0 * next(), 83.0 + 6.0 * next())).collect();
    // A 33×33 chunk grid, 2 m spacing, near Everest.
    let chunk: Vec<DVec3> =
        (0..33 * 33).map(|k| dir(27.9881 + (k / 33) as f64 * 1.8e-5, 86.925 + (k % 33) as f64 * 2.0e-5)).collect();
    for body in ["Earth", "Moon"] {
        let p = w.find(body).unwrap().physical.clone().unwrap();
        let base_only = BodyPhysical { detail: None, ..p.clone() };
        println!(
            "{body}: surface_height global {:.3} µs, rough land {:.3} µs (base only {:.3} µs)",
            per_call(&p, &global),
            per_call(&p, &rough),
            per_call(&base_only, &global),
        );
        if body == "Earth" {
            let us = per_call(&p, &chunk);
            println!("{body}: 33×33 chunk at Everest {:.3} ms ({us:.3} µs per vertex)", us * 1089.0 / 1e3);
        }
    }
}
