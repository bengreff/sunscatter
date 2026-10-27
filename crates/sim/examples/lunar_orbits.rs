//! How long low lunar orbits last through the Moon's field (D090): an
//! orbit given by periapsis and apoapsis altitude (km) and inclination to
//! the lunar equator (deg), started 2030 + 3k days with its node over body-
//! fixed longitude 45k° and its periapsis over the north (ω 90°), for k =
//! 0..8, flown for DAYS through the full force model (every source, the
//! shipped fields, steps resolving them). Prints when each hits the ground
//! and its lowest altitude above the reference sphere per 5 days.
//! `cargo run -p sim --release --example lunar_orbits -- PERI APO INC DAYS`
//! (PFS-2: 91 130 169.28 60; PFS-1: 103 136 151.28 400; circular 100 km at
//! each inclination: 100 100 INC 150; `docs/features/solar-system.md`).
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::vessel::{CoastStart, EndKind, Segment};
use sim::world::World;
use std::sync::Arc;

fn main() {
    let args: Vec<f64> = std::env::args().skip(1).map(|a| a.parse().expect("a number")).collect();
    let [peri, apo, inc, days] = args[..] else {
        eprintln!("usage: lunar_orbits PERI_KM APO_KM INC_DEG DAYS");
        std::process::exit(2);
    };
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let w = World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()));
    let moon = w.find("Moon").unwrap();
    let p = moon.physical.as_ref().unwrap();
    let deg = sim::math::PI / 180.0;
    for k in 0..8 {
        let node = 45.0 * k as f64;
        let t0 = sol::sol_epoch().add_seconds(86_400.0 * 3.0 * k as f64);
        let a = p.radius_eq + 0.5 * (peri + apo) * 1e3;
        let e = (apo - peri) * 1e3 / (2.0 * a);
        let el = Elements { a, e, i: inc * deg, raan: node * deg, argp: 90.0 * deg, mean_anomaly: 0.0 };
        let (r, v) = el.to_state(moon.gm);
        let m = p.rotation.matrix(t0);
        let (r, v) = (m * r, m * v);
        let horizon = days * 86_400.0;
        let start = CoastStart {
            anchor: moon.node,
            r,
            v,
            drag: None,
            contact_height: 0.0,
            horizon,
            fixed_anchor: true,
            proper_time: 0.0,
        };
        let mut seg = Segment::new(&w, t0, start);
        let clock = std::time::Instant::now();
        seg.extend_until(&w, horizon, usize::MAX);
        let crash = seg.end.filter(|e| matches!(e.kind, EndKind::Surface { .. })).map(|e| e.t / 86_400.0);
        let mut lows: Vec<f64> = Vec::new();
        for s in &seg.samples {
            let b = (s.s.t / (5.0 * 86_400.0)) as usize;
            if lows.len() <= b {
                lows.resize(b + 1, f64::INFINITY);
            }
            lows[b] = lows[b].min((s.s.r.length() - p.radius_eq) / 1e3);
        }
        let lows: Vec<String> = lows.iter().map(|x| format!("{x:.0}")).collect();
        let fate = crash.map_or_else(|| format!("up after {days} days"), |d| format!("crashed after {d:.1} days"));
        println!(
            "node {node:5.1}°: {fate}, {} steps, {:.1} s; lowest km per 5 days: {}",
            seg.samples.len(),
            clock.elapsed().as_secs_f64(),
            lows.join(" ")
        );
    }
}
