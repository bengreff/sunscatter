//! Cost of the 10 km bubble (D082; `docs/technical/proximity.md`): two
//! stress craft of `parts` parts touching in low orbit, held live by a
//! test craft 2 km away burning at its least throttle. Release only:
//!
//! `cargo run -p sim --release --example bench_bubble [parts=300] [seconds=4]`
//!
//! Prints µs per 20 ms tick: one stress craft live alone (the baseline),
//! the bubble's ticks while the two touch (joint substeps with contact),
//! and one call of the pair's contact forces at the touching pose.
use glam::DVec3;
use sim::craft::{registry, stress, test_craft};
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::proximity::{self, Body};
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{bubble, Controls, Phase, Vessel, VesselId, TICK};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str, d: f64| {
        args.iter().find_map(|a| a.strip_prefix(&format!("{k}=")).and_then(|v| v.parse().ok())).unwrap_or(d)
    };
    let (parts, seconds) = (arg("parts", 300.0) as usize, arg("seconds", 4.0));
    let w = world();
    let t0 = sol::sol_epoch().add_seconds(86_400.0);
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    let along = (v - r * (v.dot(r) / r.length_squared())).normalize();
    let id = stress::id(parts);
    let craft = registry::craft_of(&id);
    let hull = proximity::hull_of(&id);
    println!(
        "stress-{parts}: {} primitives, {} hull points, radius {:.1} m",
        hull.shapes.len(),
        hull.points.len(),
        hull.radius
    );
    let a = Vessel::coasting(&w, VesselId(1), t0, earth.node, r, v, &craft);
    // Where B first touches A along track (both at A's attitude).
    let props = a.mass_props();
    let body = |at: DVec3| Body {
        hull: &hull,
        r: at,
        v: DVec3::ZERO,
        q: a.attitude.q,
        omega: DVec3::ZERO,
        com: props.com,
        mass: props.mass,
        inertia: props.inertia,
    };
    let mut d = 2.0 * hull.radius;
    while proximity::contact::touches(&body(DVec3::ZERO), &body(along * d)).is_empty() && d > 0.0 {
        d -= 0.01;
    }
    println!("touching at {d:.2} m between centres of mass");
    let t = Instant::now();
    let reps = 200;
    for _ in 0..reps {
        std::hint::black_box(proximity::pair_forces(
            &body(DVec3::ZERO),
            &body(along * (d - 0.01)),
            0.5,
            0.05,
            TICK / 10.0,
        ));
    }
    let f = proximity::pair_forces(&body(DVec3::ZERO), &body(along * (d - 0.01)), 0.5, 0.05, TICK / 10.0);
    println!(
        "pair forces 1 cm deep: {} points touching, {:.0} µs a call (10 a tick)",
        f.count,
        t.elapsed().as_secs_f64() * 1e6 / reps as f64
    );

    let (ba, bb) = (body(DVec3::ZERO), body(along * (d - 0.01)));
    let t = Instant::now();
    let mut near = proximity::near_points(&ba, &bb, 0.03);
    for _ in 1..reps {
        near = std::hint::black_box(proximity::near_points(&ba, &bb, 0.03));
    }
    let find = t.elapsed().as_secs_f64() * 1e6 / reps as f64;
    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(proximity::pair_forces_near(&ba, &bb, &near, 0.5, 0.05, TICK / 10.0));
    }
    println!(
        "within 3 cm: {} + {} points, found in {find:.0} µs (once a tick); their forces {:.1} µs a substep",
        near.a.len(),
        near.b.len(),
        t.elapsed().as_secs_f64() * 1e6 / reps as f64
    );
    // Baseline: one stress craft live alone (held by a bubble flag through
    // the bubble pass with a leader far away is the same tick).
    let leader = |at: DVec3| Vessel::coasting(&w, VesselId(3), t0, earth.node, at, v, test_craft());
    let ctl = |i: usize| Controls { sas: true, throttle: if i == 2 { 0.1 } else { 0.0 }, ..Default::default() };
    let tick_times = |fleet: &mut Vec<Vessel>, touching: &dyn Fn(&[Vessel]) -> bool| {
        let (mut on, mut off) = (Vec::new(), Vec::new());
        let start = fleet[0].time;
        let ticks = (seconds / TICK) as usize;
        for k in 1..=ticks {
            let target: Epoch = start.add_seconds(TICK * k as f64);
            let t = Instant::now();
            bubble::advance(&w, fleet, &ctl, target);
            for (i, v) in fleet.iter_mut().enumerate() {
                v.advance(&w, target, &ctl(i), usize::MAX / 4);
            }
            let us = t.elapsed().as_secs_f64() * 1e6;
            if touching(fleet) {
                on.push(us);
            } else {
                off.push(us);
            }
        }
        (on, off)
    };
    let far = Vessel::coasting(&w, VesselId(2), t0, earth.node, r + along * 5_000.0, v, &craft);
    let mut alone = vec![a.clone(), far, leader(r - along * 2_000.0)];
    let (_, solo) = tick_times(&mut alone, &|_| false);
    assert!(alone.iter().all(|v| v.in_bubble() && matches!(v.phase, Phase::Powered { .. })));
    // The pair, B closing at 2 cm/s from 1 cm short of touching.
    let b = Vessel::coasting(&w, VesselId(2), t0, earth.node, r + along * (d + 0.01), v - along * 0.02, &craft);
    let mut pair = vec![a, b, leader(r - along * 2_000.0)];
    let (on, off) = tick_times(&mut pair, &|f: &[Vessel]| f[0].bubble_state().since_contact == Some(0));
    println!(
        "µs per tick (bubble pass + fleet, three vessels): two stress craft and the leader apart {:.0}; \
         touching {:.0} ({} ticks); not touching but together {:.0} ({} ticks)",
        median(solo),
        median(on.clone()),
        on.len(),
        median(off.clone()),
        off.len()
    );
}
