//! Landing prediction tests: free fall against the analytic drop, the
//! adaptive prediction against a brute-force fixed-tick integration of the
//! same model on the rotating Earth, the parachute's terminal velocity, a
//! braking solution flown, determinism and anchor invariance.

use super::model::{Model, Run};
use super::*;
use crate::craft::{test_craft, CraftParams};
use crate::ephem::Ephemeris;
use crate::forces::ActiveSources;
use crate::sol;
use crate::vessel::{air_at, quat_z_to, AeroTick, TICK};
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn craft() -> CraftParams {
    test_craft().params()
}

/// A start `height` metres (centre of mass) above the ground at `lat`,
/// `lon` (rad) of `body`, moving `(east, north, up)` m/s relative to the ground.
fn start_over(w: &World, body: &str, (lat, lon, height): (f64, f64, f64), (e, n, u): (f64, f64, f64)) -> LandingStart {
    let src = w.find(body).unwrap();
    let p = src.physical.as_ref().unwrap();
    let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
    let fixed = p.ground_point(lat, lon, height);
    let r = p.rotation.to_inertial(fixed, t).raw();
    let to_i = |v: DVec3| p.rotation.to_inertial(Vec3::from_raw(v), t).raw();
    let up = r.normalize();
    let east = to_i(DVec3::new(-crate::math::sin(lon), crate::math::cos(lon), 0.0));
    let north = up.cross(east);
    let v = p.rotation.omega(t).raw().cross(r) + east * e + north * n + up * u;
    LandingStart {
        t,
        anchor: src.node,
        r,
        v,
        propellant: 2_000.0,
        infinite: false,
        throttle: 0.0,
        chute: Default::default(),
        attitude: AssumedAttitude::SurfaceRetrograde,
        body: src.node,
    }
}

fn limits() -> Limits {
    Limits { horizon: 3_600.0, max_steps: MAX_STEPS }
}

/// Fixed-step RK4 of the model at the live tick from `run` (the gravity
/// sources chosen at the start) while `go(t, r, v)` holds; returns the
/// last two ticks' states `(t, r, v)`.
type State = (f64, DVec3, DVec3);
fn brute(world: &World, model: &Model, run: &Run, until: f64, go: impl Fn(&State) -> bool) -> (State, State) {
    let active = ActiveSources::select(world, &world.snapshot(run.t0), run.anchor, run.r);
    let acc = |t: f64, r: DVec3, v: DVec3| model.accel(&world.snapshot(run.t0.add_seconds(t)), run, &active, t, r, v);
    let (mut prev, mut s) = ((0.0, run.r, run.v), (0.0, run.r, run.v));
    while go(&s) && s.0 < until {
        let (t, r, v) = s;
        let h = TICK.min(until - t);
        let k1 = (v, acc(t, r, v));
        let k2 = (v + k1.1 * (h / 2.0), acc(t + h / 2.0, r + k1.0 * (h / 2.0), v + k1.1 * (h / 2.0)));
        let k3 = (v + k2.1 * (h / 2.0), acc(t + h / 2.0, r + k2.0 * (h / 2.0), v + k2.1 * (h / 2.0)));
        let k4 = (v + k3.1 * h, acc(t + h, r + k3.0 * h, v + k3.1 * h));
        prev = s;
        s = (
            t + h,
            r + (k1.0 + k2.0 * 2.0 + k3.0 * 2.0 + k4.0) * (h / 6.0),
            v + (k1.1 + k2.1 * 2.0 + k3.1 * 2.0 + k4.1) * (h / 6.0),
        );
    }
    (prev, s)
}

/// Linear interpolation between two ticks where `f` crosses zero.
fn crossing(a: &State, b: &State, fa: f64, fb: f64) -> State {
    let x = fa / (fa - fb);
    (a.0 + (b.0 - a.0) * x, a.1 + (b.1 - a.1) * x, a.2 + (b.2 - a.2) * x)
}

#[test]
fn a_vertical_drop_on_the_moon_is_free_fall() {
    let w = world();
    let c = craft();
    let start = start_over(&w, "Moon", (0.3, 1.0, 1_000.0), (0.0, 0.0, 0.0));
    let h = clearance(&w, &c, &start);
    let d = predict_impact(&w, &c, &start, limits());
    let hit = d.impact.expect("it lands");
    let src = w.find("Moon").unwrap();
    let r0 = start.r.length();
    // Uniform gravity at the mean height of the fall (the drop is 0.06 % of
    // the radius; the rotation and the Earth's tide move it by centimetres).
    let g = src.gm / (r0 - h / 2.0).powi(2);
    let t = hit.t.seconds_since(start.t);
    assert!((t / (2.0 * h / g).sqrt() - 1.0).abs() < 2e-4, "{t} s from {h} m");
    assert!((hit.speed / (2.0 * g * h).sqrt() - 1.0).abs() < 2e-4, "{}", hit.speed);
    assert!(hit.v_horizontal < 0.05 && hit.v_vertical < 0.0, "{hit:?}");
    // Straight down: the ground point is the one under the start.
    let p = src.physical.as_ref().unwrap();
    let under = p.rotation.to_fixed(Vec3::from_raw(start.r), start.t).raw();
    let (lat, lon) = crate::terrain::lat_lon(under);
    assert!((lat - hit.lat).abs() * p.radius_eq < 0.2 && (lon - hit.lon).abs() * p.radius_eq < 0.2);
}

#[test]
fn the_impact_on_the_rotating_earth_matches_a_brute_force_tick_integration() {
    let w = world();
    let c = craft();
    // 30 km up over the Atlantic, 1.5 km/s east and falling at 200 m/s,
    // with the aerodynamics of the retrograde attitude all the way down.
    let start = start_over(&w, "Earth", (0.5, -1.2, 30_000.0), (1_500.0, 100.0, -200.0));
    let t0 = std::time::Instant::now();
    let d = predict_impact(&w, &c, &start, limits());
    let cost = t0.elapsed();
    let hit = d.impact.expect("it lands");
    let model = Model::new(&w, &c, &start);
    let run = Run::from_start(&start, 0.0);
    let clear = |s: &State| model.clearance_exact(&run, s.0, s.1, s.2);
    let (a, b) = brute(&w, &model, &run, 3_600.0, |s| clear(s) > 0.0);
    let s = crossing(&a, &b, clear(&a), clear(&b));
    let brute_hit = model.impact(start.t.add_seconds(s.0), start.anchor, start.body, s.1, s.2);
    let dt = hit.t.seconds_since(brute_hit.t);
    let miss = (hit.ground.raw() - brute_hit.ground.raw()).length();
    eprintln!("prediction {cost:?}; {} s of flight; dt {dt} s, miss {miss} m", hit.t.seconds_since(start.t));
    assert!(dt.abs() < 0.01, "{dt}");
    assert!(miss < 1.0, "{miss} m");
    assert!((hit.speed - brute_hit.speed).abs() < 0.05, "{} vs {}", hit.speed, brute_hit.speed);
    // The marker: the ground point carried by the rotation. At impact it is
    // under the vessel; a minute later the Earth has carried it ~20 km.
    let under = (hit.ground_at(&w, hit.t, start.anchor) - s.1).length();
    assert!(under < 30.0, "{under}");
    let moved =
        (hit.ground_at(&w, hit.t.add_seconds(60.0), start.anchor) - hit.ground_at(&w, hit.t, start.anchor)).length();
    assert!(moved > 15_000.0 && moved < 30_000.0, "{moved}");
}

#[test]
fn under_the_parachute_the_impact_speed_is_the_terminal_velocity() {
    let w = world();
    let c = craft();
    // Over the sea (a flat surface at sea level), 3 km up, falling slowly.
    let mut start = start_over(&w, "Earth", (0.5, -1.2, 3_000.0), (0.0, 0.0, -10.0));
    // Armed below the main's height: the prediction flies the full main.
    start.chute.armed = true;
    let d = predict_impact(&w, &c, &start, limits());
    let hit = d.impact.expect("it lands");
    let last = d.samples.last().unwrap();
    let snap = w.snapshot(hit.t);
    let air = air_at(&w, &snap, start.anchor, last.r, last.v).unwrap();
    let props = c.mass.at(start.propellant);
    let q = quat_z_to(last.r.normalize()) * quat_z_to(c.engine.mount_dir).inverse();
    let tick = AeroTick::new(c.design(), &air, props.com, Some((c.chute.main.cd_area, c.chute.mount))).unwrap();
    let (cd_area, _) = tick.drag_and_lift(q, props.mass);
    let g = w.find("Earth").unwrap().gm / last.r.length_squared();
    let terminal = (2.0 * props.mass * g / (air.rho * cd_area)).sqrt();
    assert!((hit.speed / terminal - 1.0).abs() < 0.01, "{} vs {terminal}", hit.speed);
    assert!(hit.v_horizontal < 0.1, "{}", hit.v_horizontal);
}

#[test]
fn the_braking_solution_stops_at_the_margin_when_flown() {
    let w = world();
    let c = craft();
    // 8 km over the Moon, 150 m/s across and 80 m/s down.
    let start = start_over(&w, "Moon", (0.2, 0.4, 8_000.0), (150.0, 0.0, -80.0));
    let d = predict_impact(&w, &c, &start, limits());
    let margin = 10.0;
    let b = braking_solution(&w, &c, &d, margin).expect("it can stop");
    let ti = b.t_ignite.seconds_since(start.t);
    assert!(ti > 0.0 && b.t_ignite < d.impact.unwrap().t, "{ti}");
    assert!((b.stop_height - margin).abs() < 1.0, "{}", b.stop_height);
    // Fly it with fixed ticks: coast to the ignition, then full thrust
    // until the surface-relative speed stops falling.
    let model = Model::new(&w, &c, &start);
    let coast = Run::from_start(&start, 0.0);
    let (_, s) = brute(&w, &model, &coast, ti, |_| true);
    let burn =
        Run { t0: b.t_ignite, anchor: start.anchor, r: s.1, v: s.2, propellant: start.propellant, throttle: 1.0 };
    let v_srf = |s: &State| {
        let snap = w.snapshot(burn.t0.add_seconds(s.0));
        s.2 - crate::contact::Ground::new(&w, &snap, start.anchor, start.body).velocity(s.1)
    };
    let v0 = v_srf(&(0.0, burn.r, burn.v));
    // Stop within a tick of zero speed (1 m/s at 50 m/s²: centimetres).
    let (_, stop) = brute(&w, &model, &burn, 3_600.0, |s| v_srf(s).dot(v0) > 0.0 && v_srf(s).length() > 1.0);
    let height = model.clearance_exact(&burn, stop.0, stop.1, stop.2);
    assert!((height - margin).abs() < 1.5, "flown: stopped {height} m up");
    assert!(v_srf(&stop).length() < 1.0, "{}", v_srf(&stop).length());
    assert!((b.t_stop.seconds_since(burn.t0.add_seconds(stop.0))).abs() < 0.2);
}

#[test]
fn a_craft_already_touching_the_ground_predicts_without_panicking() {
    // The centre of mass at ground level: the feet are already below it
    // (seen in the demo at the end of a parachute descent).
    let w = world();
    let c = craft();
    for u in [-5.0, 0.0] {
        let start = start_over(&w, "Moon", (0.2, 0.4, 0.0), (0.0, 0.0, u));
        let d = predict_impact(&w, &c, &start, limits());
        let hit = d.impact.expect("an impact at once");
        assert!(hit.t.seconds_since(start.t) < 1.0, "{}", hit.t.seconds_since(start.t));
        let _ = braking_solution(&w, &c, &d, 10.0);
    }
}

#[test]
fn a_craft_that_cannot_stop_has_no_braking_solution() {
    let w = world();
    let c = craft();
    let start = start_over(&w, "Moon", (0.2, 0.4, 300.0), (0.0, 0.0, -200.0));
    let d = predict_impact(&w, &c, &start, limits());
    assert!(d.impact.is_some());
    assert_eq!(braking_solution(&w, &c, &d, 10.0), None);
}

#[test]
fn predictions_are_deterministic_and_independent_of_the_anchor() {
    let w = world();
    let c = craft();
    let start = start_over(&w, "Moon", (0.3, 1.0, 2_000.0), (300.0, 0.0, -20.0));
    let a = predict_impact(&w, &c, &start, limits());
    assert_eq!(a, predict_impact(&w, &c, &start, limits()));
    // The same state relative to the Earth.
    let earth = w.find("Earth").unwrap().node;
    let k = w.snapshot(start.t).relative(start.anchor, earth);
    let from_earth = LandingStart { anchor: earth, r: start.r + k.r, v: start.v + k.v, ..start };
    let b = predict_impact(&w, &c, &from_earth, limits());
    let (ha, hb) = (a.impact.unwrap(), b.impact.unwrap());
    assert!(ha.t.seconds_since(hb.t).abs() < 1e-3);
    assert!((ha.ground.raw() - hb.ground.raw()).length() < 0.05);
}

#[test]
fn twr_uses_thrust_at_ambient_pressure() {
    let c = craft();
    let m = c.mass.dry_mass + 1_000.0;
    let vac = twr(&c, 1_000.0, 0.0, 1.62);
    assert!((vac - c.engine.thrust_vac / (m * 1.62)).abs() < 1e-12);
    assert!(twr(&c, 1_000.0, 101_325.0, 1.62) < vac);
}

#[test]
fn a_deorbit_from_low_orbit_lands_within_half_an_orbit_and_a_low_lunar_orbit_does_not() {
    let w = world();
    let c = craft();
    // 200 km over the Earth, 120 m/s short of circular (ground-relative
    // speed from the inertial circular speed): periapsis inside the air.
    let earth = w.find("Earth").unwrap();
    let r = earth.physical.as_ref().unwrap().radius_eq + 200_000.0;
    let v_circ = (earth.gm / r).sqrt();
    let omega_r = 7.292e-5 * r * libm::cos(0.1);
    let start = start_over(&w, "Earth", (0.1, 0.0, 200_000.0), (v_circ - 120.0 - omega_r, 0.0, 0.0));
    let t0 = std::time::Instant::now();
    let d = predict_impact(&w, &c, &start, Limits::two_orbits(&w, &start));
    let cost = t0.elapsed();
    let hit = d.impact.expect("it comes down");
    let dt = hit.t.seconds_since(start.t);
    eprintln!("deorbit: {cost:?}, {} steps, {dt} s, {hit:?}", d.samples.len());
    assert!(dt > 600.0 && dt < 3_000.0, "{dt}");
    // 30 km over the Moon, circular: two orbits and no impact.
    let moon = w.find("Moon").unwrap();
    let r = moon.physical.as_ref().unwrap().radius_eq + 30_000.0;
    let start = start_over(&w, "Moon", (0.1, 0.0, 30_000.0), ((moon.gm / r).sqrt(), 0.0, 0.0));
    let t0 = std::time::Instant::now();
    let d = predict_impact(&w, &c, &start, Limits::two_orbits(&w, &start));
    eprintln!("low lunar orbit: {:?}, {} steps over {} s", t0.elapsed(), d.samples.len(), d.duration());
    assert_eq!(d.impact, None);
    assert!(d.duration() > 10_000.0);
}
