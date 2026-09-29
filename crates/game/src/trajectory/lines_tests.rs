use super::*;
use crate::state::SimState;

/// A test ship's coast over `revs` revolutions (the segment, its start).
fn leo_coast(revs: f64) -> (SimState, Segment) {
    let mut sim = SimState::new();
    sim.spawn_test_ships(1);
    let world = sim.world.clone();
    let until = sim.clock.add_seconds(revs * 5_560.0);
    let i = sim.fleet.len() - 1;
    sim.fleet[i].extend_coast(&world, until, 200_000);
    let seg = sim.fleet[i].trajectory().expect("coasting").segments[0].clone();
    (sim, seg)
}

#[test]
fn samples_turn_by_at_most_max_turn_and_cover_the_span() {
    let (_, seg) = leo_coast(1.2);
    let t1 = 5_560.0;
    let times = sample_times(&seg, 10.0, t1);
    assert_eq!(times.first().map(|p| p.0), Some(10.0));
    assert_eq!(times.last().map(|p| p.0), Some(t1));
    assert!(times.windows(2).all(|w| w[1].0 > w[0].0));
    for w in times.windows(2) {
        assert!(turn(w[0].1, w[1].1) <= MAX_TURN * 1.01, "{w:?}");
    }
    // One revolution turns 2π: about 2π / MAX_TURN points, far fewer than
    // the 600 uniform points drawn before.
    let expected = std::f64::consts::TAU / MAX_TURN;
    let n = times.len() as f64;
    assert!(n > expected * 0.9 && n < expected * 1.5 + 5.0, "{n} vs {expected}");
}

#[test]
fn a_straight_path_keeps_its_ends_and_the_clock_limit() {
    // A coast far from everything turns little: points only every MAX_DT.
    let mut sim = SimState::new();
    let earth = sim.world.find("Earth").unwrap().node;
    let id = sim.vessel_ids.allocate();
    let r = DVec3::new(3.0e9, 0.0, 0.0);
    let v = DVec3::new(0.0, 12_000.0, 0.0);
    let vessel = sim::vessel::Vessel::coasting(&sim.world, id, sim.clock, earth, r, v, sim::craft::test_craft());
    sim.fleet.push(vessel);
    let world = sim.world.clone();
    let until = sim.clock.add_seconds(10.0 * 3_600.0);
    let i = sim.fleet.len() - 1;
    sim.fleet[i].extend_coast(&world, until, 200_000);
    let seg = &sim.fleet[i].trajectory().unwrap().segments[0];
    let times = sample_times(seg, 0.0, 10.0 * 3_600.0);
    assert!(times.len() >= 11 && times.len() <= 25, "{}", times.len());
    assert!(times.windows(2).all(|w| w[1].0 - w[0].0 <= MAX_DT + 1e-6));
}

#[test]
fn frustum_culls_only_pieces_wholly_outside_one_side() {
    let f = Frustum { right: Vec3::X, up: Vec3::Y, forward: Vec3::NEG_Z, tan_x: 1.0, tan_y: 0.5 };
    let cases = [
        // (a, b, culled)
        (Vec3::new(0.0, 0.0, -10.0), Vec3::new(1.0, 1.0, -10.0), false),
        (Vec3::new(20.0, 0.0, -10.0), Vec3::new(30.0, 0.0, -10.0), true),
        // Crossing the view from left to right: kept.
        (Vec3::new(-20.0, 0.0, -10.0), Vec3::new(20.0, 0.0, -10.0), false),
        // Above the top plane (tan_y 0.5): culled.
        (Vec3::new(0.0, 6.0, -10.0), Vec3::new(3.0, 8.0, -10.0), true),
        // Behind the camera: outside every side plane. Through it: kept (the
        // near clip cuts it).
        (Vec3::new(-1.0, 0.0, 10.0), Vec3::new(1.0, 0.0, 10.0), true),
        (Vec3::new(0.0, 0.0, 10.0), Vec3::new(0.0, 0.0, -10.0), false),
    ];
    for (a, b, culled) in cases {
        assert_eq!(f.culls(a, b), culled, "{a} {b}");
    }
}

#[test]
fn chord_error_grows_with_turn_and_nearness() {
    let a = DVec3::new(0.0, 0.0, -1_000.0);
    let b = DVec3::new(100.0, 0.0, -1_000.0);
    // 100 m chord turning 0.08 rad at 1 km: sagitta 1 m, 1 mrad, 1 px at
    // a focal length of 1000 px.
    assert!((chord_error_px(a, b, 0.08, 1_000.0) - 1.0).abs() < 1e-3);
    assert!(chord_error_px(a * 10.0, b + a * 9.0, 0.08, 1_000.0) < 0.2);
    assert_eq!(chord_error_px(a, b, 0.0, 1_000.0), 0.0);
}

/// Timing of the old uniform resampling against the adaptive one (run with
/// `--ignored --nocapture`).
#[test]
#[ignore]
fn bench_line_sampling() {
    let (sim, seg) = leo_coast(1.0);
    let rig = CameraRig::default();
    let plotter = Plotter::new(&sim, &rig, crate::hud::PlotFrame::EarthInertial).unwrap();
    let t1 = seg.computed_until();
    let reps = 200;
    let t = std::time::Instant::now();
    let mut n_old = 0;
    for _ in 0..reps {
        n_old = (0..600)
            .filter_map(|k| {
                let tt = t1 * f64::from(k) / 599.0;
                let (a, r, _) = seg.eval(tt)?;
                Some(plotter.plot(a, r, seg.t0.add_seconds(tt)))
            })
            .count();
    }
    let old = t.elapsed().as_secs_f64() / f64::from(reps);
    let f = Frustum { right: Vec3::X, up: Vec3::Y, forward: Vec3::NEG_Z, tan_x: 1e6, tan_y: 1e6 };
    let viewer = Viewer { plotter: &plotter, frustum: f, focal: 1_000.0 };
    let t = std::time::Instant::now();
    let mut n_new = 0;
    for _ in 0..reps {
        n_new = segment_runs(&seg, 0.0, t1, &viewer, None).1;
    }
    let new = t.elapsed().as_secs_f64() / f64::from(reps);
    println!("one LEO revolution: uniform {n_old} pts {:.1} µs; adaptive {n_new} pts {:.1} µs", old * 1e6, new * 1e6);
}
