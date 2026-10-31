//! Cost of ten limbo craft (`docs/technical/limbo.md`, D081) in the game's
//! frame loop (release): perturbed coasts under thin-air drag (low orbits,
//! held prograde), the same higher up, radiation pressure (a tumbling craft
//! high up), each with its systems lattice and
//! thermal lattice, advanced as `crates/game/src/state.rs` does: every
//! vessel to the clock plus dt·warp with the game's budget, the clock held
//! at the slowest, then look-ahead for one vessel per frame.
//! `cargo run -p sim --release --example bench_limbo`
use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::vessel::{Attitude, Controls, HoldMode, Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

const RE: f64 = 6_378_137.0;
/// The game's per-frame budgets.
const COAST_STEPS_PER_FRAME: usize = 1_200;
const LOOKAHEAD_STEPS_PER_FRAME: usize = 1_500;
const LOOKAHEAD: f64 = 6_000.0;
/// Simulated time before measuring (s): holds settled,
/// temperatures settled (30 days).
const WARM: f64 = 30.0 * 86_400.0;

/// Ten small rockets (`craft::small`): four decaying in low orbit, four
/// held prograde at 900-1,050 km, two tumbling high up under sunlight. (The
/// four used to spiral out on 0.5 N ion burns: the open-loop burn law on
/// rails went with the made-up craft, D121; low thrust returns with D029.)
fn fleet(w: &World) -> Vec<(Vessel, Controls)> {
    let earth = w.find("Earth").unwrap();
    let t0 = sol::sol_epoch().add_seconds(86_400.0);
    let craft = sim::craft::small::rocket_ref();
    let mut out = Vec::new();
    for k in 0..10 {
        let (h, e) = match k {
            0..=3 => (250_000.0 + 40_000.0 * k as f64, 0.001),
            4..=7 => (700_000.0 + 50_000.0 * k as f64, 0.0005),
            _ => (20_000_000.0, 0.01),
        };
        let el = Elements { a: RE + h, e, i: 0.2 * k as f64, raan: 0.5 * k as f64, argp: 0.0, mean_anomaly: k as f64 };
        let (r, v) = el.to_state(earth.gm);
        let mut s = Vessel::coasting(w, VesselId(k as u64 + 1), t0, earth.node, r, v, craft);
        let controls = match k {
            0..=7 => Controls { sas: true, hold: HoldMode::Prograde, ..Controls::default() },
            _ => {
                s.set_attitude(Attitude { q: s.attitude.q, omega: DVec3::new(0.01, 0.003, 0.02) });
                Controls::default()
            }
        };
        out.push((s, controls));
    }
    out
}

/// The game's frame loop at `warp` for `frames` frames of 1/60 s.
fn game(w: &World, warp: f64, frames: usize) {
    let mut ships = fleet(w);
    let mut clock = ships[0].0.time;
    let start = clock;
    for (s, c) in &mut ships {
        // Settle onto the holds and let the temperatures
        // settle (a fresh craft starts at a uniform 290 K) first.
        s.advance(w, clock.add_seconds(WARM), c, usize::MAX);
        s.extend_coast(w, clock.add_seconds(WARM + LOOKAHEAD), usize::MAX);
    }
    clock = clock.add_seconds(WARM);
    let n = ships.len();
    let mut per_vessel = vec![0.0f64; n];
    let (mut worst, mut ahead, mut limited) = (0.0f64, 0.0, 0);
    for frame in 0..frames {
        let target = clock.add_seconds(warp / 60.0);
        let mut reached = target;
        for (i, (s, c)) in ships.iter_mut().enumerate() {
            let t = Instant::now();
            let r = s.advance(w, target, c, COAST_STEPS_PER_FRAME);
            let dt = t.elapsed().as_secs_f64();
            per_vessel[i] += dt;
            worst = worst.max(dt);
            if r < reached {
                reached = r;
            }
        }
        limited += usize::from(reached < target);
        clock = reached;
        let t = Instant::now();
        let (s, _) = &mut ships[frame % n];
        s.extend_coast(w, clock.add_seconds(LOOKAHEAD), LOOKAHEAD_STEPS_PER_FRAME);
        ahead += t.elapsed().as_secs_f64();
    }
    let f = frames as f64;
    let mean = per_vessel.iter().sum::<f64>() / (f * n as f64);
    let by_kind = |r: std::ops::Range<usize>| per_vessel[r.clone()].iter().sum::<f64>() / (f * r.len() as f64) * 1e6;
    println!(
        "{warp:>9.0}x: {:.0} µs per vessel per frame (drag {:.0}, low thrust {:.0}, light {:.0}; slowest call {:.2} ms), \
         all ten {:.2} ms/frame + look-ahead {:.2} ms, compute-limited {limited}/{frames}, reached {:.0}x",
        mean * 1e6,
        by_kind(0..4),
        by_kind(4..8),
        by_kind(8..10),
        worst * 1e3,
        mean * n as f64 * 1e3,
        ahead / f * 1e3,
        clock.seconds_since(start.add_seconds(WARM)) / (f / 60.0),
    );
}

/// Where a 1,000,000x frame's time goes for one craft of each kind: the
/// coast integration alone (integrated ahead first), then the rest of
/// `advance` (attitude, thermal and systems lattices).
fn breakdown(w: &World) {
    let frame = 1e6 / 60.0;
    // `BENCH_LIMBO_ONLY=<k>`: craft k alone for 600 frames (profiling).
    let only = std::env::var("BENCH_LIMBO_ONLY").ok().and_then(|s| s.parse::<usize>().ok());
    let kinds = match only {
        Some(k) => vec![(k, "chosen")],
        None => vec![(0, "drag"), (4, "held high"), (8, "light")],
    };
    for (i, kind) in kinds {
        let (mut s, c) = fleet(w).swap_remove(i);
        s.advance(w, s.time.add_seconds(WARM), &c, usize::MAX);
        let start = s.time;
        if only.is_some() {
            let (_, r, _) = s.state(w);
            let phase = std::mem::discriminant(&s.phase);
            println!("  after warming: {:.0} km, {phase:?}, {:.0} kg", (r.length() - RE) / 1e3, s.mass());
        }
        let frames = if only.is_some() { 600 } else { 60 };
        let t = Instant::now();
        s.extend_coast(w, start.add_seconds(frame * frames as f64 + 10.0), usize::MAX);
        let integrate = t.elapsed().as_secs_f64() / frames as f64;
        let thermal0 = s.thermal().clone();
        let t = Instant::now();
        for k in 1..=frames {
            s.advance(w, start.add_seconds(frame * k as f64), &c, usize::MAX);
        }
        let rest = t.elapsed().as_secs_f64() / frames as f64;
        let solves = thermal0 != *s.thermal();
        println!(
            "  {kind}: integration {:.0} µs/frame, attitude + thermal + systems {:.0} µs/frame (temperatures moved: {solves})",
            integrate * 1e6,
            rest * 1e6
        );
    }
}

/// Integration cost per step and steps per orbit of a low coast, bare and
/// perturbed (drag, light and the pointing at every stage).
fn steps(w: &World) {
    let (s, c) = fleet(w).swap_remove(0);
    let mut s = s;
    s.advance(w, s.time.add_seconds(900.0), &c, usize::MAX);
    let seg = s.segment().unwrap().clone();
    let start = seg.initial();
    let bare = sim::vessel::Segment::new(w, seg.t0, sim::vessel::CoastStart { drag: None, ..start });
    let period = 2.0 * std::f64::consts::PI * ((RE + 250_000.0f64).powi(3) / w.find("Earth").unwrap().gm).sqrt();
    for (what, mut seg) in [("bare", bare), ("perturbed", seg)] {
        let n = 5_000;
        let t = Instant::now();
        seg.extend(w, n);
        let dt = t.elapsed().as_secs_f64();
        let orbits = (seg.computed_until() - seg.samples[0].s.t) / period;
        println!(
            "  {what} coast at 250 km: {:.2} µs/step, {:.0} steps/orbit",
            dt / n as f64 * 1e6,
            seg.samples.len() as f64 / orbits
        );
    }
}

fn main() {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let w = World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()));
    if std::env::var("BENCH_LIMBO_ONLY").is_ok() {
        breakdown(&w);
        return;
    }
    steps(&w);
    println!("one craft of each kind at 1,000,000x:");
    breakdown(&w);
    println!("10 limbo craft, the game's frame loop (dt 1/60 s, one thread):");
    for (warp, frames) in [(1.0, 600), (1_000.0, 600), (1e6, 300)] {
        game(&w, warp, frames);
    }
}
