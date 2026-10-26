//! Stability at scale (D081; `docs/features/scale-performance.md`): 50
//! craft flown for simulated years at 1,000,000x in the game's frames
//! (`Vessel::advance_frame`, the clock held at the slowest vessel; physics
//! warp while one is live), the vessels advanced in parallel. Every month
//! of simulated time: no NaN or infinity anywhere in a vessel (state,
//! attitude, temperatures, propellant, systems), the stored trajectories'
//! samples (memory) bounded, no panic. Then save/load: the fleet captured
//! to RON and read back equals the original, and both flown on agree bit
//! for bit.
//!
//! The fleet: 10 limbo craft (four 0.5 N ion spirals from 700-850 km, three
//! held prograde at 450-500 km decaying in thin air, three tumbling at
//! 20,000 km under radiation pressure) and 40 quiet ones holding attitude
//! (15 stress craft of 300 parts at 600-1,300 km, 20 test craft at
//! 600-2,500 km, four on eccentric orbits from 2,000 to 60,000 km, one on the pad).
//! Stress craft come from the craft registry (`stress-300`), so the save
//! reloads them.
//!
//! `cargo run -p sim --release --example long_run [years=2] [threads=7] [save_days=10]`
use glam::DVec3;
use sim::craft::{registry, CraftId};
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{
    Attitude, BurnEnd, BurnLaw, Controls, DirectionLaw, FlightPlan, HoldMode, Phase, PlannedBurn, Vessel, VesselId,
    VesselIds,
};
use sim::world::World;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const RE: f64 = 6_378_137.0;
const DAY: f64 = 86_400.0;
/// Budget units per advance call (the game's chunk).
const CHUNK: usize = 2_000;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn fleet(w: &World, t0: Epoch) -> (Vec<Vessel>, Vec<Controls>, VesselIds) {
    let earth = w.find("Earth").unwrap();
    let big = registry::craft_of(&CraftId("stress-300".into()));
    let small = registry::craft_of(&CraftId(sim::craft::TEST_CRAFT.into()));
    let mut ids = VesselIds::default();
    let held = Controls { sas: true, hold: HoldMode::Prograde, ..Controls::default() };
    let still = Controls { sas: true, ..Controls::default() };
    let (mut ships, mut controls) = (Vec::new(), Vec::new());
    for k in 0..50 {
        let craft = match k {
            0..=9 => [&small, &big][k % 2],
            10..=24 => &big,
            _ => &small,
        };
        let (h, e, a_far) = match k {
            0..=3 => (700e3 + 50e3 * k as f64, 0.001, None),
            4..=6 => (450e3 + 25e3 * (k - 4) as f64, 0.001, None),
            7..=9 => (20_000e3, 0.01, None),
            10..=24 => (600e3 + 50e3 * (k - 10) as f64, 0.001, None),
            45..=48 => (0.0, 0.0, Some(RE + 2_000e3 + 250e3 * (k - 45) as f64)),
            _ => (600e3 + 100e3 * (k - 25) as f64, 0.002, None),
        };
        let id = ids.allocate();
        if k == 49 {
            ships.push(Vessel::landed_at(w, id, "Earth", 28.6082, -80.66, t0, craft));
            controls.push(still);
            continue;
        }
        let el = match a_far {
            // Perigee `rp`, apogee 60,000 km (the Moon lowers a perigee of a
            // few hundred kilometres into the air within weeks).
            Some(rp) => {
                let ra = 6.0e7;
                Elements {
                    a: 0.5 * (rp + ra),
                    e: (ra - rp) / (ra + rp),
                    i: 0.4,
                    raan: k as f64,
                    argp: 1.0,
                    mean_anomaly: 2.0,
                }
            }
            None => Elements {
                a: RE + h,
                e,
                i: 0.1 + 0.03 * k as f64,
                raan: 0.37 * k as f64,
                argp: 0.0,
                mean_anomaly: 0.7 * k as f64,
            },
        };
        let (r, v) = el.to_state(earth.gm);
        let mut s = Vessel::coasting(w, id, t0, earth.node, r, v, craft);
        s.systems.power.spec.tracking = k % 2 == 0;
        s.systems.consumables.boil_off = 1e-6;
        let c = match k {
            0..=3 => {
                s.set_propellant(w, 2_000.0);
                let law =
                    BurnLaw::from_isp(0.5, 3_000.0, DirectionLaw::Tracking { reference: earth.node, axes: DVec3::X });
                let burn = PlannedBurn { t_start: t0.add_seconds(600.0), law, end: BurnEnd::Duration(365.0 * DAY) };
                s.set_plan(w, FlightPlan { burns: vec![burn] }).unwrap();
                held
            }
            4..=6 => held,
            7..=9 => {
                s.set_attitude(Attitude { q: s.attitude.q, omega: DVec3::new(0.01, 0.003, 0.02) });
                Controls::default()
            }
            _ => still,
        };
        ships.push(s);
        controls.push(c);
    }
    (ships, controls, ids)
}

/// Every vessel towards `target` on `threads` threads (a queue of vessels,
/// each advanced by one thread to the target in chunks); the clock after.
fn frame(w: &World, ships: &mut [Vessel], controls: &[Controls], target: Epoch, threads: usize) -> Epoch {
    let queue = Mutex::new(ships.iter_mut().zip(controls).enumerate());
    let reached = Mutex::new(vec![target; controls.len()]);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let next = queue.lock().unwrap().next();
                let Some((i, (v, c))) = next else { break };
                let mut r = v.time;
                loop {
                    let before = r;
                    r = v.advance_frame(w, target, c, CHUNK).0;
                    let coasting = matches!(v.phase, Phase::Coasting { .. });
                    if r >= target || r == before || !coasting {
                        break;
                    }
                }
                let finished = matches!(&v.phase, Phase::Coasting { trajectory } if trajectory.finished());
                reached.lock().unwrap()[i] = if finished { target } else { r };
            });
        }
    });
    let reached = reached.into_inner().unwrap();
    let mut clock = target;
    for r in reached {
        if r.seconds_since(target) < -sim::vessel::TICK && r < clock {
            clock = r;
        }
    }
    clock
}

/// The first non-finite number in a vessel, if any.
fn non_finite(w: &World, v: &Vessel) -> Option<String> {
    let (_, r, vel) = v.state(w);
    let th = v.thermal();
    let checks = [
        ("position", r.is_finite()),
        ("velocity", vel.is_finite()),
        ("attitude", v.attitude.q.is_finite() && v.attitude.omega.is_finite()),
        ("skin", th.state.skin.iter().all(|t| t.is_finite() && *t > 0.0)),
        ("nodes", th.state.nodes.iter().all(|t| t.is_finite() && *t > 0.0)),
        ("mass", v.mass().is_finite() && v.mass() > 0.0),
        ("battery", v.systems.power.charge.is_finite()),
    ];
    checks.iter().find(|c| !c.1).map(|c| c.0.to_string())
}

/// The fleet's saved state (MB of RON): every vessel's growable state
/// (trajectory samples, burns, systems), the measure of bounded memory
/// that does not depend on the machine (resident memory on a loaded Mac
/// swings by hundreds of MB with compression).
fn state_mb(ships: &[Vessel]) -> f64 {
    ships.iter().map(|v| ron::to_string(v).map_or(0, |s| s.len())).sum::<usize>() as f64 / 1e6
}

fn samples(v: &Vessel) -> usize {
    v.trajectory().map_or(0, |t| t.segments.iter().map(|s| s.samples.len()).sum())
}

/// Flies the fleet from `clock` to `end` in 1,000,000x frames (physics warp
/// while a vessel is live), checking every `check` seconds; returns the clock.
fn fly(
    w: &World,
    ships: &mut [Vessel],
    controls: &[Controls],
    mut clock: Epoch,
    end: Epoch,
    threads: usize,
    check: f64,
) -> Epoch {
    let start = Instant::now();
    let mut next_check = clock.add_seconds(check);
    let (mut frames, mut capped, mut max_samples) = (0usize, 0usize, 0usize);
    let mut was_live = false;
    while clock < end {
        let live_now = ships.iter().position(|v| matches!(v.phase, Phase::Powered { .. }));
        let live = live_now.is_some();
        if let (Some(i), false) = (live_now, was_live) {
            let km = (ships[i].state(w).1.length() - RE) / 1e3;
            println!("  vessel {i} live at day {:.3}, {km:.0} km", clock.seconds_since(sol::sol_epoch()) / DAY);
        }
        was_live = live;
        let warp = if live { 4.0 } else { 1e6 };
        capped += usize::from(live);
        let target = clock.add_seconds(warp / 60.0);
        let target = if target > end { end } else { target };
        let next = frame(w, ships, controls, target, threads);
        assert!(next >= clock, "the clock went back");
        clock = next;
        frames += 1;
        if clock >= next_check || clock >= end {
            next_check = clock.add_seconds(check);
            for (i, v) in ships.iter().enumerate() {
                if let Some(what) = non_finite(w, v) {
                    panic!("vessel {i}: non-finite {what} at {clock:?}");
                }
            }
            let counts: Vec<usize> = ships.iter().map(samples).collect();
            let total: usize = counts.iter().sum();
            let most = counts.iter().copied().max().unwrap_or(0);
            max_samples = max_samples.max(most);
            let phases = |f: fn(&Phase) -> bool| ships.iter().filter(|v| f(&v.phase)).count();
            let km = |i: usize| (ships[i].state(w).1.length() - RE) / 1e3;
            println!(
                "day {:>6.1}: {frames} frames ({capped} at 4x), {:.0} s wall, {:.2} MB of state; samples {total} (most {most}); live {}, landed {}, crashed {}; spiral #0 at {:.0} km, decaying #4 at {:.0} km, {:.0} kg propellant",
                clock.seconds_since(sol::sol_epoch()) / DAY,
                start.elapsed().as_secs_f64(),
                state_mb(ships),
                phases(|p| matches!(p, Phase::Powered { .. })),
                phases(|p| matches!(p, Phase::Landed { .. })),
                phases(|p| matches!(p, Phase::Crashed { .. })),
                km(0),
                km(4),
                ships[0].propellant(),
            );
        }
    }
    println!("max samples in one vessel: {max_samples}");
    clock
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |key: &str| args.iter().find_map(|a| a.strip_prefix(key).map(str::to_owned));
    let years: f64 = value("years=").map_or(2.0, |s| s.parse().unwrap());
    let threads: usize = value("threads=").map_or(7, |s| s.parse().unwrap());
    let save_days: f64 = value("save_days=").map_or(10.0, |s| s.parse().unwrap());
    let w = world();
    let t0 = sol::sol_epoch().add_seconds(DAY);
    let built = Instant::now();
    let (mut ships, controls, ids) = fleet(&w, t0);
    println!(
        "50 craft built in {:.1} s; flying {years} years at 1,000,000x on {threads} threads",
        built.elapsed().as_secs_f64()
    );
    let end = t0.add_seconds(years * 365.25 * DAY);
    let clock = fly(&w, &mut ships, &controls, t0, end, threads, 30.0 * DAY);
    // Save and load at scale: the same bits, and the same bits flown on.
    let c = Instant::now();
    let save = SaveGame::capture(&w, clock, &ships, ids, 0, controls[0]);
    let text = save.to_ron();
    let loaded = SaveGame::from_ron(&text).expect("the save reads back");
    loaded.check_world(&w).expect("same ephemeris");
    println!("save: {:.1} MB of RON, written and read in {:.1} s", text.len() as f64 / 1e6, c.elapsed().as_secs_f64());
    assert_eq!(loaded.vessels, ships, "the loaded fleet equals the saved one");
    let mut reloaded = loaded.vessels;
    let on = clock.add_seconds(save_days * DAY);
    let a = fly(&w, &mut ships, &controls, clock, on, threads, save_days * DAY);
    let b = fly(&w, &mut reloaded, &controls, clock, on, threads, save_days * DAY);
    assert_eq!(a, b, "the clocks");
    for (i, (x, y)) in ships.iter().zip(&reloaded).enumerate() {
        assert_eq!(x.time, y.time, "vessel {i}: time");
        assert_eq!(x.state(&w), y.state(&w), "vessel {i}: state");
        assert_eq!(x.thermal(), y.thermal(), "vessel {i}: temperatures");
        assert_eq!(x.attitude, y.attitude, "vessel {i}: attitude");
    }
    println!(
        "saved and reloaded fleets flown {save_days} more days: the same bits; ids {:?}",
        VesselId(ids_next(&reloaded))
    );
}

fn ids_next(v: &[Vessel]) -> u64 {
    v.iter().map(|s| s.id().0).max().unwrap_or(0) + 1
}
