use super::*;
use bevy::tasks::TaskPool;
use sim::kepler::Elements;
use sim::sol;

fn t(s: f64) -> Epoch {
    sol::sol_epoch().add_seconds(s)
}

#[test]
fn the_vessel_furthest_behind_goes_first() {
    let mut q = Queue::new([t(5.0), t(2.0), t(2.0), t(9.0)].into_iter(), t(10.0));
    // (picked, then finished at, more)
    assert_eq!(q.pick(), Some(1), "the earliest, lowest index on ties");
    assert_eq!(q.pick(), Some(2), "the busy one is not picked again");
    q.finish(1, t(2.0), t(8.0), 100, true);
    assert_eq!(q.pick(), Some(0));
    q.finish(2, t(2.0), t(2.0), 0, true);
    assert_eq!(q.slot[2], Slot::Done, "no progress: left for the next frame");
    q.finish(0, t(5.0), t(10.0), 50, false);
    assert_eq!(q.slot[0], Slot::Done, "at the target");
    assert_eq!(q.pick(), Some(1), "8 s before 9 s");
    assert_eq!(q.pick(), Some(3));
    assert_eq!(q.pick(), None);
}

#[test]
fn a_vessel_that_keeps_spending_without_moving_is_capped() {
    let mut q = Queue::new([t(0.0)].into_iter(), t(10.0));
    for _ in 0..MAX_CHUNKS {
        assert_eq!(q.pick(), Some(0));
        q.finish(0, t(0.0), t(0.0), 10, true);
    }
    assert_eq!(q.pick(), None);
}

#[test]
fn a_vessel_at_or_past_the_target_is_never_queued() {
    let mut q = Queue::new([t(12.0), t(3.0), t(10.0)].into_iter(), t(10.0));
    assert_eq!(q.slot, [Slot::Done, Slot::Ready, Slot::Done]);
    assert_eq!(q.pick(), Some(1));
    assert_eq!(q.pick(), None);
}

/// A frame's target can fall below vessels that finished first (after a
/// compute-limited frame, a pause or a warp drop): they wait, never rewound
/// (review of 2026-10-26, PERF 1: a coast went back in time, and one whose
/// segment, a burn, started after the target panicked). Frames whose
/// targets shrink and grow end at the same bits as one jump.
#[test]
fn a_shrinking_target_never_moves_a_vessel_back() {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let w = world();
    let controls = |_: usize| Controls { sas: true, ..Default::default() };
    let mut fleet = leo_fleet(&w, 3);
    let engine = sim::craft::test_craft().params().engine;
    let burn = sim::vessel::PlannedBurn::delta_v_with(t(2_000.0), glam::DVec3::X * 50.0, &engine, None);
    fleet[1].set_plan(&w, sim::vessel::FlightPlan { burns: vec![burn] }).expect("plan");
    let end = t(4_000.0);
    let mut reference = fleet.clone();
    for (i, v) in reference.iter_mut().enumerate() {
        v.advance(&w, end, &controls(i), usize::MAX);
    }
    // Vessel 1 ahead of the others (it finished first), then in its burn.
    fleet[1].advance(&w, t(1_000.0), &controls(1), usize::MAX);
    for target in [400.0, 1_500.0, 1_200.0, 2_010.0, 1_990.0, 3_000.0, 2_500.0, 4_000.0] {
        let before: Vec<Epoch> = fleet.iter().map(|v| v.time).collect();
        let adv = advance_fleet(&w, &mut fleet, &controls, t(target), None);
        for (k, v) in fleet.iter().enumerate() {
            assert!(v.time >= before[k], "vessel {k} went back from the target {target} s");
            assert_eq!(adv.reached[k], v.time);
        }
    }
    for (a, b) in fleet.iter().zip(&reference) {
        assert_eq!(a.time, b.time);
        assert_eq!(a.state(&w), b.state(&w), "vessel {:?}", a.id());
        assert_eq!(a.mass(), b.mass());
    }
}

#[test]
fn only_a_lagging_live_vessel_holds_the_clock() {
    let target = t(100.0);
    // (seconds behind the target, stopped, holds)
    let cases = [(0.0, false, false), (0.5 * TICK, false, false), (10.0, false, true), (10.0, true, false)];
    for (behind, stopped, holds) in cases {
        assert_eq!(holds_clock(target.add_seconds(-behind), target, stopped), holds, "{behind} {stopped}");
    }
    let adv =
        FleetAdvance { reached: vec![t(100.0), t(40.0), t(10.0), t(99.999)], stopped: vec![false, false, true, false] };
    assert_eq!(clock_after(&adv, target), t(40.0), "the slowest that has not stopped");
}

#[test]
fn the_achieved_warp_follows_the_rate_over_half_a_second() {
    // (last, simulated, real, expected)
    let cases =
        [(1e6, 16_667.0, 1.0 / 60.0, 1e6), (1e6, 0.0, 1.0 / 60.0, 1e6 * (1.0 - 1.0 / 30.0)), (5.0, 1.0, 0.0, 5.0)];
    for (last, sim, real, want) in cases {
        let got = achieved_warp(last, sim, real);
        assert!((got / want - 1.0).abs() < 1e-3, "{last} {sim} {real}: {got} vs {want}");
    }
    let mut w = 1.0;
    for _ in 0..60 {
        w = achieved_warp(w, 1_000.0 / 60.0, 1.0 / 60.0);
    }
    assert!((w / 1_000.0 - 1.0).abs() < 0.15, "a second at 1,000x: {w}");
}

#[test]
fn a_frame_asks_a_little_more_than_the_clock_moved() {
    // (wanted, limited, last, span)
    let cases = [
        (16_667.0, false, 600.0, 1_200.0),
        (16_667.0, false, 16_000.0, 16_667.0),
        (16_667.0, true, 600.0, 750.0),
        (16_667.0, true, 16_000.0, 16_667.0),
        (16_667.0, true, 0.0, 16_667.0),
        (0.0167, true, 1.0, 0.0167),
        (0.0167, false, 0.0167, 0.0167),
    ];
    for (wanted, limited, last, want) in cases {
        assert_eq!(frame_span(wanted, limited, last), want, "{wanted} {limited} {last}");
    }
    // From 1x to 1,000,000x in twenty frames.
    let (mut last, mut frames) = (1.0 / 60.0, 0);
    while last < 1e6 / 60.0 {
        last = frame_span(1e6 / 60.0, false, last);
        frames += 1;
    }
    assert_eq!(frames, 20);
}

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let eph = sim::ephem::Ephemeris::from_bytes(&std::fs::read(path).expect("ephemeris")).expect("valid");
    World::sol(std::sync::Arc::new(eph))
}

fn leo_fleet(w: &World, n: usize) -> Vec<Vessel> {
    let earth = w.find("Earth").expect("Earth");
    (0..n)
        .map(|k| {
            let el = Elements {
                a: 6_378_137.0 + 300e3 + 20e3 * k as f64,
                e: 0.001,
                i: 0.3 * k as f64,
                raan: 0.5 * k as f64,
                argp: 0.0,
                mean_anomaly: k as f64,
            };
            let (r, v) = el.to_state(earth.gm);
            let id = sim::vessel::VesselId(k as u64 + 1);
            Vessel::coasting(w, id, t(0.0), earth.node, r, v, sim::craft::test_craft())
        })
        .collect()
}

/// The deadline and the threads change how far the vessels get in a frame,
/// never their motion: a fleet advanced in 1,000,000x frames cut short by a
/// tight deadline (the clock held back) ends at the same bits as each
/// vessel advanced in one call.
#[test]
fn deadlines_and_threads_change_the_clock_not_the_motion() {
    ComputeTaskPool::get_or_init(TaskPool::default);
    let w = world();
    let controls = |i: usize| Controls { sas: i.is_multiple_of(2), ..Default::default() };
    let end = t(2.0 * 86_400.0);
    let mut reference = leo_fleet(&w, 4);
    for (i, v) in reference.iter_mut().enumerate() {
        v.advance(&w, end, &controls(i), usize::MAX);
    }
    let mut fleet = leo_fleet(&w, 4);
    let mut clock = t(0.0);
    let (mut frames, mut limited) = (0, 0);
    while clock < end {
        let target = clock.add_seconds(1e6 / 60.0);
        let target = if target > end { end } else { target };
        let deadline = Instant::now() + std::time::Duration::from_micros(300);
        let adv = advance_fleet(&w, &mut fleet, &controls, target, Some(deadline));
        let next = clock_after(&adv, target);
        limited += usize::from(next < target);
        assert!(next >= clock, "the clock never goes back");
        clock = next;
        frames += 1;
        assert!(frames < 100_000, "stuck at {clock:?}");
    }
    assert!(limited > 0, "the deadline held the clock back");
    for (a, b) in fleet.iter().zip(&reference) {
        assert_eq!(a.time, b.time);
        assert_eq!(a.state(&w), b.state(&w), "vessel {:?}", a.id());
        assert_eq!(a.thermal(), b.thermal());
    }
}

/// The frame loop with 50 craft (`docs/technical/scale-performance.md`):
/// BENCH's fleet (`sim/examples/bench_scale.rs`): ten limbo stand-ins
/// tumbling (half 300-part stress craft, two on lunar transfers), 40 quiet
/// craft holding attitude (20 test craft and 15 300-part craft in low
/// orbit, four on lunar transfers, a test craft on the pad), vessel 0 (300
/// parts) active; settled 30 days first. Each frame as `state::advance`:
/// the fleet within `SIM_FRAME_TIME` on 7 threads (the game's compute
/// pool), then the look-ahead of the active vessel and one other. Release:
/// `cargo test -p game --release fleet_frames -- --ignored --nocapture`.
#[test]
#[ignore]
fn fleet_frames() {
    use crate::state::LOOKAHEAD_STEPS_PER_FRAME;
    let threads = std::env::var("FLEET_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(7);
    ComputeTaskPool::get_or_init(|| bevy::tasks::TaskPoolBuilder::new().num_threads(threads).build());
    let w = world();
    let big = (*sim::craft::stress::craft(300)).clone();
    let _ = big.design();
    let small = sim::craft::test_craft();
    let earth = w.find("Earth").expect("Earth").clone();
    let re = 6_378_137.0;
    let orbit = |craft: &sim::craft::Craft, k: usize, h: f64| {
        let el = Elements {
            a: re + h,
            e: 0.001,
            i: 0.1 + 0.02 * k as f64,
            raan: 0.37 * k as f64,
            argp: 0.0,
            mean_anomaly: 0.74 * k as f64,
        };
        let (r, v) = el.to_state(earth.gm);
        Vessel::coasting(&w, sim::vessel::VesselId(k as u64 + 1), t(86_400.0), earth.node, r, v, craft)
    };
    let mut fleet = Vec::new();
    let mut sas = Vec::new();
    for k in 0..50 {
        let limbo = k < 10;
        let craft = if limbo { [&big, small][k % 2] } else { [small, &big][usize::from((30..45).contains(&k))] };
        let mut v = match k {
            8 | 9 | 45..=48 => orbit(if k < 10 { craft } else { [small, &big][k % 2] }, k, 20_000e3),
            49 => Vessel::landed_at(&w, sim::vessel::VesselId(50), "Earth", 28.6082, -80.66, t(86_400.0), small),
            _ => orbit(craft, k, 300e3 + 5e3 * (k % 40) as f64),
        };
        if limbo {
            let (_, r, _) = v.state(&w);
            v.set_attitude(sim::vessel::Attitude {
                q: sim::vessel::quat_z_to(r.normalize()),
                omega: glam::DVec3::new(0.01, 0.02, 0.0),
            });
        }
        sas.push(!limbo);
        fleet.push(v);
    }
    let controls = move |i: usize| Controls { sas: sas[i], ..Default::default() };
    let settle = Instant::now();
    let warm = t(86_400.0 + 30.0 * 86_400.0);
    while fleet.iter().any(|v| v.time < warm && !stopped(v)) {
        advance_fleet(&w, &mut fleet, &controls, warm, None);
    }
    println!("settled 30 days on {threads} threads in {:.1} s", settle.elapsed().as_secs_f64());
    // Calibration: µs per budget unit in 1,000,000x chunks, per vessel.
    let mut ratios = Vec::new();
    for (i, v) in fleet.iter().enumerate() {
        let mut v = v.clone();
        let (mut us, mut units) = (0.0, 0usize);
        let mut at = warm;
        for _ in 0..30 {
            at = at.add_seconds(1e6 / 60.0);
            while v.time < at && !stopped(&v) && matches!(v.phase, Phase::Coasting { .. }) {
                let c = Instant::now();
                let (_, spent) = v.advance_frame(&w, at, &controls(i), CHUNK_UNITS);
                us += c.elapsed().as_secs_f64() * 1e6;
                units += spent;
            }
        }
        if units > 0 {
            ratios.push((us / units as f64, i, us / 30.0));
        }
    }
    ratios.sort_by(|a, b| a.0.total_cmp(&b.0));
    let pick = |q: f64| ratios[((ratios.len() - 1) as f64 * q) as usize];
    for (name, q) in [("min", 0.0), ("median", 0.5), ("p90", 0.9), ("max", 1.0)] {
        let (r, i, per) = pick(q);
        println!("  {name}: vessel {i} {r:.2} µs per unit ({per:.0} µs per 1,000,000x frame)");
    }
    for (warp, frames) in [(1.0, 600), (1_000.0, 600), (1e6, 240)] {
        let mut ships = fleet.clone();
        let mut clock = warm;
        let start = clock;
        let (mut mean, mut worst, mut limited, mut ahead) = (0.0, 0.0f64, 0, 0.0);
        let (mut over, mut lead, mut last, mut was_limited) = (0, 0.0f64, 1.0 / 60.0, false);
        let wall = Instant::now();
        for frame in 0..frames {
            // As `state::advance`: a compute-limited frame asks a quarter
            // more than the clock moved.
            let target = clock.add_seconds(frame_span(warp / 60.0, was_limited, last));
            let t0 = Instant::now();
            let deadline = t0 + std::time::Duration::from_secs_f64(SIM_FRAME_TIME);
            let adv = advance_fleet(&w, &mut ships, &controls, target, Some(deadline));
            let next = clock_after(&adv, target);
            was_limited = next < target;
            limited += usize::from(was_limited);
            last = next.seconds_since(clock);
            clock = next;
            // How far a vessel is drawn ahead of the clock (at its own time).
            lead = ships.iter().filter(|v| !stopped(v)).map(|v| v.time.seconds_since(clock)).fold(lead, f64::max);
            let t1 = Instant::now();
            ships[0].extend_coast(&w, clock.add_seconds(6_000.0), LOOKAHEAD_STEPS_PER_FRAME);
            let other = 1 + frame % 49;
            ships[other].extend_coast(&w, clock.add_seconds(6_000.0), 300);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            ahead += t1.elapsed().as_secs_f64() * 1e3 / frames as f64;
            mean += ms / frames as f64;
            worst = worst.max(ms);
            over += usize::from(ms > 16.7);
        }
        let simulated = clock.seconds_since(start);
        println!(
            "{warp:>9.0}x: frame {mean:.2} ms mean ({worst:.1} worst, {over} over 16.7 ms), look-ahead {ahead:.2} ms, compute-limited {limited}/{frames}; per 1/60 s frame {:.0}x, per wall second {:.0}x; most ahead of the clock {lead:.1} s",
            simulated / (frames as f64 / 60.0),
            simulated / wall.elapsed().as_secs_f64(),
        );
    }
}

/// Game-side per-frame costs with 50 vessels in low orbit, every one
/// tracked and in map view (the worst case; `docs/technical/scale-performance.md`):
/// the dominance radii, each vessel's line end (`trajectory::vessel_line_end`,
/// drawn lines and the look-ahead), the map's per-vessel orbit sizes and the
/// map-view rule over every body and vessel. Release:
/// `cargo test -p game --release game_side_costs -- --ignored --nocapture`.
#[test]
#[ignore]
fn game_side_costs() {
    use crate::map_view::{classify, Object, ObjectId};
    use crate::relations::{vessel_orbit, Dominance};
    use crate::trajectory::{settings::OrbitSettings, vessel_line_end};
    let w = world();
    let mut fleet = leo_fleet(&w, 50);
    let now = t(0.0);
    for v in &mut fleet {
        v.extend_coast(&w, now.add_seconds(12_000.0), usize::MAX);
    }
    let orbits = OrbitSettings::default();
    let reps = 20;
    let time = |f: &mut dyn FnMut()| {
        let c = Instant::now();
        for _ in 0..reps {
            f();
        }
        c.elapsed().as_secs_f64() * 1e3 / reps as f64
    };
    let mut dom = Dominance::new(&w.eph, now);
    let new_ms = time(&mut || dom = Dominance::new(&w.eph, now));
    let mut ends = 0.0;
    let lines_ms = time(&mut || {
        for v in &fleet {
            let seg = v.trajectory().unwrap().last();
            ends += vessel_line_end(&w, seg, now.seconds_since(seg.t0), &orbits).0;
        }
    });
    let mut sizes = 0.0;
    let orbit_ms = time(&mut || {
        for v in &fleet {
            sizes += vessel_orbit(&w, &dom, now, v).map_or(0.0, |o| o.radius());
        }
    });
    let objects: Vec<Object> = (0..44 + 50)
        .map(|k| Object {
            id: ObjectId::Vessel(sim::vessel::VesselId(k)),
            radius: 10.0,
            orbit_radius: Some(7e6),
            mass: k as f64,
            screen: Some(glam::Vec2::new((k * 7 % 800) as f32, (k * 13 % 600) as f32)),
            depth: 1e7 + k as f64,
            disc_px: 0.5,
            pinned: false,
            flown: false,
        })
        .collect();
    let mut shown = 0;
    let classify_ms = time(&mut || {
        shown += classify(&objects, 1e-5, 1e8, |_| crate::map_view::Visibility { in_map: true, icon: true, line: true })
            .iter()
            .filter(|v| v.icon)
            .count()
    });
    println!(
        "50 vessels: Dominance::new {new_ms:.3} ms; line ends of all 50 {lines_ms:.2} ms ({:.3} each); map orbit sizes {orbit_ms:.3} ms; map-view rule over 94 objects {classify_ms:.3} ms",
        lines_ms / 50.0
    );
    std::hint::black_box((ends, sizes, shown));
}
