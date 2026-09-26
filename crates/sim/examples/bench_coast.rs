//! Rough timing of coast integration (release): steps per second in LEO.
//! `cargo run -p sim --release --example bench_coast`
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::vessel::{CoastStart, Segment};
use sim::world::World;
use std::sync::Arc;

fn main() {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let w = World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()));
    let earth = w.find("Earth").unwrap().clone();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 0.0, argp: 0.0, mean_anomaly: 0.0 }.to_state(earth.gm);
    let t0 = sol::sol_epoch();
    let start =
        CoastStart { anchor: earth.node, r, v, drag: None, contact_height: 5.0, horizon: 1e9, fixed_anchor: false };
    let mut seg = Segment::new(&w, t0, start);
    let n = 20_000;
    let clock = std::time::Instant::now();
    seg.extend(&w, n);
    let dt = clock.elapsed().as_secs_f64();
    let days = seg.computed_until() / 86_400.0;
    println!(
        "{n} steps in {:.1} ms: {:.2} µs/step, {:.0} steps/orbit, {:.1} days covered",
        dt * 1e3,
        dt / n as f64 * 1e6,
        n as f64 / (days * 86_400.0 / 5_553.0),
        days
    );
    let clock = std::time::Instant::now();
    let mut acc = 0.0;
    for k in 0..100_000 {
        let snap = w.snapshot(t0.add_seconds(k as f64));
        acc += snap.relative(earth.node, w.eph.root()).r.x;
    }
    println!("snapshot + relative: {:.2} µs ({acc:e})", clock.elapsed().as_secs_f64() / 100_000.0 * 1e6);
}
