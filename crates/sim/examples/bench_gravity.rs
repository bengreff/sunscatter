//! What the gravity fields cost a coast (D090, release): µs per DOP853 step,
//! steps per day and force evaluations per step for orbits near the bodies
//! with fields (LEO, GEO, low lunar orbits, Mars) and far from them (a
//! heliocentric arc), and the degree each includes.
//! `cargo run -p sim --release --example bench_gravity`
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::vessel::{CoastStart, Segment};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let w = World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()));
    let t0 = sol::sol_epoch();
    let circular = |body: &str, alt_km: f64, inc_deg: f64| {
        let s = w.find(body).unwrap();
        let p = s.physical.as_ref().unwrap();
        let a = p.radius_eq + alt_km * 1e3;
        let el = Elements { a, e: 0.0, i: inc_deg * sim::math::PI / 180.0, raan: 0.3, argp: 0.0, mean_anomaly: 0.0 };
        let (r, v) = el.to_state(s.gm);
        (s.node, r, v)
    };
    let sun = w.find("Sun").unwrap();
    let helio = {
        let el = Elements { a: 2.0e11, e: 0.1, i: 0.05, raan: 1.0, argp: 0.5, mean_anomaly: 2.0 };
        let (r, v) = el.to_state(sun.gm);
        (sun.node, r, v)
    };
    let cases = [
        ("LEO 400 km, 51.6 deg", circular("Earth", 400.0, 51.6), 1.0),
        ("LEO 200 km, 28.5 deg", circular("Earth", 200.0, 28.5), 1.0),
        ("GEO", circular("Earth", 35_786.0, 0.05), 5.0),
        ("LLO 100 km polar", circular("Moon", 100.0, 89.0), 1.0),
        ("LLO 50 km polar", circular("Moon", 50.0, 89.0), 1.0),
        ("Mars 250 km polar", circular("Mars", 250.0, 93.0), 1.0),
        ("heliocentric, 1.3 AU", helio, 60.0),
    ];
    for (name, (anchor, r, v), days) in cases {
        let start = CoastStart {
            anchor,
            r,
            v,
            drag: None,
            contact_height: 5.0,
            horizon: days * 86_400.0,
            fixed_anchor: false,
            proper_time: 0.0,
        };
        // Warm the ephemeris tables, then time a fresh segment.
        Segment::new(&w, t0, start).extend(&w, 50);
        let mut seg = Segment::new(&w, t0, start);
        let clock = Instant::now();
        let steps = seg.extend_until(&w, days * 86_400.0, usize::MAX);
        let dt = clock.elapsed().as_secs_f64();
        let degrees: Vec<String> = seg
            .active_sources()
            .0
            .iter()
            .filter(|a| a.degree > 0)
            .map(|a| format!("{} {}", w.sources[a.source].name, a.degree))
            .collect();
        println!(
            "{name:24} {:7.2} us/step, {:7.0} steps/day ({} steps, {:.1} days); fields: {}",
            dt / steps as f64 * 1e6,
            steps as f64 / (seg.computed_until() / 86_400.0),
            steps,
            seg.computed_until() / 86_400.0,
            degrees.join(", ")
        );
    }
}
