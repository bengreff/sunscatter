//! Cost of a coasting fleet at high warp (release): 11 test craft in low
//! Earth orbits (eclipses), advanced by `STEP` of simulated time per
//! "frame". The coasts are integrated ahead first (the game's look-ahead),
//! so the frames measure the per-vessel work of `Vessel::advance`
//! (attitude, the coast thermal lattice, pruning); then the same with the
//! integration inside the frames.
//! Last, the game's frame loop (`crates/game/src/state.rs` `advance`): every
//! vessel advanced to the clock plus `dt·warp` with the game's budgets, the
//! clock held at the slowest vessel, then one orbit of look-ahead for the
//! active vessel and one other, at 1x, 1000x and 1e6x (dt = 1/60 s).
//! `cargo run -p sim --release --example bench_fleet [game]` (`game`: only
//! the frame loop).
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::vessel::{Controls, Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

const VESSELS: usize = 11;
const STEP: f64 = 1e6;
const FRAMES: usize = 3;

fn fleet(w: &World) -> Vec<Vessel> {
    let earth = w.find("Earth").unwrap();
    let t0 = sol::sol_epoch().add_seconds(86_400.0);
    (0..VESSELS)
        .map(|k| {
            let a = 6_378_137.0 + 300_000.0 + 20_000.0 * k as f64;
            let el =
                Elements { a, e: 0.001, i: 0.1 * k as f64, raan: 0.5 * k as f64, argp: 0.0, mean_anomaly: k as f64 };
            let (r, v) = el.to_state(earth.gm);
            Vessel::coasting(w, VesselId(k as u64 + 1), t0, earth.node, r, v, sim::craft::test_craft())
        })
        .collect()
}

fn run(w: &World, ahead: bool, budget: usize) {
    let mut ships = fleet(w);
    let controls = Controls { sas: true, ..Default::default() };
    let mut target = ships[0].time;
    for frame in 0..FRAMES {
        target = target.add_seconds(STEP);
        if ahead {
            let clock = Instant::now();
            for s in &mut ships {
                s.extend_coast(w, target, usize::MAX);
            }
            println!("  frame {frame}: look-ahead integration {:.1} ms", clock.elapsed().as_secs_f64() * 1e3);
        }
        let clock = Instant::now();
        let mut behind = 0.0f64;
        for s in &mut ships {
            let reached = s.advance(w, target, &controls, budget);
            behind = behind.max(target.seconds_since(reached));
        }
        let ms = clock.elapsed().as_secs_f64() * 1e3;
        let skin: f64 = ships.iter().map(|s| s.max_skin_temperature()).sum::<f64>() / VESSELS as f64;
        println!("  frame {frame}: advance {ms:.2} ms, furthest behind {behind:.0} s, mean hottest skin {skin:.1} K");
    }
}

/// The game's per-frame budgets (`crates/game/src/state.rs`).
const COAST_STEPS_PER_FRAME: usize = 1_200;
const LOOKAHEAD_STEPS_PER_FRAME: usize = 1_500;
const OTHER_LOOKAHEAD_STEPS_PER_FRAME: usize = 300;
/// Look-ahead span (s): about one low orbit, as the drawn line.
const LOOKAHEAD: f64 = 6_000.0;

/// The game's frame loop at `warp` for `frames` frames of 1/60 s. Prints
/// the mean sim time per frame (all vessels in turn: the game spreads
/// the advances over its task pool) and the simulated time reached.
fn game(w: &World, warp: f64, frames: usize) {
    let mut ships = fleet(w);
    let controls = Controls { sas: true, ..Default::default() };
    let mut clock = ships[0].time;
    let start = clock;
    // Warm up the look-ahead as the game does before warping.
    for s in &mut ships {
        s.extend_coast(w, clock.add_seconds(LOOKAHEAD), usize::MAX);
    }
    let (mut advance, mut ahead, mut worst, mut limited) = (0.0, 0.0, 0.0f64, 0);
    for frame in 0..frames {
        let target = clock.add_seconds(warp / 60.0);
        let t = Instant::now();
        let mut reached = target;
        let mut slowest = 0.0f64;
        for s in &mut ships {
            let v = Instant::now();
            let r = s.advance(w, target, &controls, COAST_STEPS_PER_FRAME);
            slowest = slowest.max(v.elapsed().as_secs_f64());
            if r < reached {
                reached = r;
            }
        }
        advance += t.elapsed().as_secs_f64();
        worst = worst.max(slowest);
        limited += usize::from(reached < target);
        clock = reached;
        let t = Instant::now();
        ships[0].extend_coast(w, clock.add_seconds(LOOKAHEAD), LOOKAHEAD_STEPS_PER_FRAME);
        let other = 1 + frame % (VESSELS - 1);
        ships[other].extend_coast(w, clock.add_seconds(LOOKAHEAD), OTHER_LOOKAHEAD_STEPS_PER_FRAME);
        ahead += t.elapsed().as_secs_f64();
    }
    let n = frames as f64;
    println!(
        "  {warp:>9.0}x: advance {:.3} ms/frame (slowest vessel up to {:.2} ms), look-ahead {:.3} ms/frame, \
         compute-limited {limited}/{frames}, reached {:.0} s ({:.0}x)",
        advance / n * 1e3,
        worst * 1e3,
        ahead / n * 1e3,
        clock.seconds_since(start),
        clock.seconds_since(start) / (n / 60.0),
    );
}

fn main() {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let w = World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()));
    let game_only = std::env::args().any(|a| a == "game");
    if !game_only {
        stress(&w);
    }
    println!("{VESSELS} vessels, the game's frame loop (dt 1/60 s):");
    for (warp, frames) in [(1.0, 600), (1_000.0, 600), (1e6, 240)] {
        game(&w, warp, frames);
    }
}

fn stress(w: &World) {
    // The coast sunlight (orbit average) per lattice point.
    let ships = fleet(w);
    let (anchor, r, v) = ships[0].state(w);
    let clock = Instant::now();
    let mut acc = 0.0;
    for k in 0..10_000 {
        acc += sim::vessel::coast_sunlight(w, ships[0].time.add_seconds(k as f64 * 600.0), anchor, r, v).x;
    }
    println!("coast sunlight: {:.2} µs ({acc:e})", clock.elapsed().as_secs_f64() / 10_000.0 * 1e6);
    println!("{VESSELS} vessels, {STEP:.0} s per frame, coasts integrated ahead:");
    run(w, true, usize::MAX);
    println!("{VESSELS} vessels, {STEP:.0} s per frame, integration in the frame (1,200 steps per vessel):");
    run(w, false, 1_200);
}
