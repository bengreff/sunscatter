//! Rigid-body ground contact scenarios (realism-1 §3e, D066): slopes the
//! craft holds or tips on, touchdowns it survives or not, and the rest
//! state (save/load, warp, frame rate).
//!
//! Slopes are synthetic: the Moon's heightmap is replaced by a plane rising
//! to the north through the equator (no procedural detail).

use glam::{DMat3, DQuat, DVec3};
use sim::craft::{test_craft, ContactKind};
use sim::ephem::Ephemeris;
use sim::frame::{NodeId, Vec3};
use sim::save::SaveGame;
use sim::sol;
use sim::terrain::Heightmap;
use sim::time::Epoch;
use sim::vessel::{Attitude, Controls, Phase, Vessel, VesselId, VesselIds};
use sim::world::World;
use std::sync::Arc;

const DEG: f64 = std::f64::consts::PI / 180.0;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(5.0 * 86_400.0)
}

/// The Moon with a plane rising to the north at `deg` through the equator
/// (flat beyond ±30 km of height).
fn slope_world(deg: f64) -> World {
    let mut w = world();
    let moon = w.sources.iter_mut().find(|s| s.name == "Moon").unwrap().physical.as_mut().unwrap();
    let (width, height) = (2000usize, 1000usize);
    let tan = libm::tan(deg * DEG);
    let data: Vec<i16> = (0..width * height)
        .map(|k| {
            let lat = std::f64::consts::FRAC_PI_2 - ((k / width) as f64 + 0.5) * std::f64::consts::PI / height as f64;
            libm::round((tan * moon.radius_eq * lat).clamp(-30_000.0, 30_000.0)) as i16
        })
        .collect();
    moon.terrain = Some(Arc::new(Heightmap::from_vec(width, height, data)));
    moon.detail = None;
    w
}

/// Body → inertial rotation of `body` at `t`.
fn body_axes(w: &World, body: &str, t: Epoch) -> DMat3 {
    let rot = w.find(body).unwrap().physical.as_ref().unwrap().rotation;
    let col = |v: DVec3| rot.to_inertial(Vec3::from_raw(v), t).raw();
    DMat3::from_cols(col(DVec3::X), col(DVec3::Y), col(DVec3::Z))
}

/// Local east, north and up (body-fixed) at latitude/longitude (deg).
fn enu(lat: f64, lon: f64) -> (DVec3, DVec3, DVec3) {
    let (la, lo) = (lat * DEG, lon * DEG);
    let up = DVec3::new(libm::cos(la) * libm::cos(lo), libm::cos(la) * libm::sin(lo), libm::sin(la));
    let east = DVec3::Z.cross(up).normalize();
    (east, up.cross(east), up)
}

/// How to drop the craft.
struct Drop {
    body: &'static str,
    lat: f64,
    lon: f64,
    /// Ground slope rising to the north (deg), the craft aligned with it.
    slope: f64,
    /// Nose direction relative to the ground normal: `true` upside down.
    inverted: bool,
    /// Yaw of the feet (deg): 45 puts two feet downhill.
    yaw: f64,
    /// Gap between the lowest feet (or the nose, inverted) and the ground (m).
    gap: f64,
    /// Speed towards the ground (m/s).
    speed: f64,
    propellant: f64,
    debug: bool,
}

impl Default for Drop {
    fn default() -> Self {
        Drop {
            body: "Moon",
            lat: 0.0,
            lon: 20.0,
            slope: 0.0,
            inverted: false,
            yaw: 0.0,
            gap: 0.02,
            speed: 0.0,
            propellant: 16_000.0,
            debug: false,
        }
    }
}

/// The ground normal of a plane rising to the north at `slope` (deg).
fn normal(d: &Drop) -> DVec3 {
    let (_, north, up) = enu(d.lat, d.lon);
    (up - north * libm::tan(d.slope * DEG)).normalize()
}

fn drop(w: &World, d: &Drop) -> Vessel {
    let src = w.find(d.body).unwrap();
    let p = src.physical.as_ref().unwrap();
    let mut ship = Vessel::coasting(w, VesselId(1), t0(), src.node, DVec3::X * 1e9, DVec3::ZERO, test_craft());
    ship.set_propellant(w, d.propellant);
    let props = ship.mass_props();
    // Craft axes (body-fixed frame of the body): z along the normal, the
    // feet diagonal between +X and +Y downhill for yaw 45°.
    let n = normal(d);
    let (_, north, _) = enu(d.lat, d.lon);
    let down_hill = (-north - n * (-north).dot(n)).normalize();
    let e2 = n.cross(down_hill);
    let (c, s) = (libm::cos((d.yaw - 45.0) * DEG), libm::sin((d.yaw - 45.0) * DEG));
    let diag = down_hill * c + e2 * s;
    let side = n.cross(diag);
    let (x, y) = ((diag - side) * std::f64::consts::FRAC_1_SQRT_2, (diag + side) * std::f64::consts::FRAC_1_SQRT_2);
    let (x, y, z) = if d.inverted { (x, -y, -n) } else { (x, y, n) };
    let att_fixed = DQuat::from_mat3(&DMat3::from_cols(x, y, z)).normalize();
    // Height of the centre of mass above the ground along the normal.
    let reach = if d.inverted {
        test_craft().surface.positions.iter().map(|q| q.z).fold(f64::MIN, f64::max) - props.com.z
    } else {
        props.com.z - ship.craft.bottom_z
    };
    let ground = p.ground_point(d.lat * DEG, d.lon * DEG, 0.0).raw();
    let fixed = ground + n * (reach + d.gap);
    let b2i = body_axes(w, d.body, t0());
    let omega = p.rotation.omega(t0()).raw();
    let r = b2i * fixed;
    let v = omega.cross(r) - b2i * n * d.speed;
    let mut ship = Vessel::coasting(w, VesselId(1), t0(), src.node, r, v, test_craft());
    ship.set_propellant(w, d.propellant);
    ship.set_debug(w, d.debug);
    ship.set_attitude(Attitude { q: (DQuat::from_mat3(&b2i) * att_fixed).normalize(), omega });
    ship
}

/// Advances at 60 fps for `seconds` (stopping early once landed or crashed
/// if `until_still`).
fn fly(w: &World, ship: &mut Vessel, seconds: f64, controls: &Controls, until_still: bool) {
    let start = ship.time;
    for k in 1..=(seconds * 60.0) as usize {
        ship.advance(w, start.add_seconds(k as f64 / 60.0), controls, usize::MAX);
        if until_still && matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. }) {
            return;
        }
    }
}

/// Angle (deg) between the craft's nose and `dir` (body-fixed).
fn tilt_from(w: &World, ship: &Vessel, body: &str, dir: DVec3) -> f64 {
    let b2i = body_axes(w, body, ship.time);
    let nose = b2i.transpose() * ship.attitude.nose();
    libm::acos(nose.dot(dir).clamp(-1.0, 1.0)) / DEG
}

/// Body-fixed position of the centre of mass.
fn fixed_of(w: &World, ship: &Vessel, body: &str) -> DVec3 {
    let (_, r, _) = ship.state(w);
    body_axes(w, body, ship.time).transpose() * r
}

#[test]
fn a_two_degree_slope_holds() {
    let w = slope_world(2.0);
    let d = Drop { slope: 2.0, ..Drop::default() };
    let mut ship = drop(&w, &d);
    let start = fixed_of(&w, &ship, "Moon");
    fly(&w, &mut ship, 30.0, &Controls::default(), true);
    let Phase::Landed { fixed, att_fixed, .. } = ship.phase else { panic!("not at rest: {:?}", ship.phase) };
    let n = normal(&d);
    // Settled on its feet (a few cm) without sliding, tilted with the slope.
    let moved = fixed.raw() - start;
    assert!((moved - n * moved.dot(n)).length() < 0.02, "slid {} m", moved.length());
    assert!(moved.dot(n) < 0.0 && moved.dot(n) > -0.1, "sank {} m", -moved.dot(n));
    let nose = att_fixed * DVec3::Z;
    let (_, _, up) = enu(d.lat, d.lon);
    assert!((libm::acos(nose.dot(up)) / DEG - 2.0).abs() < 0.05, "{}°", libm::acos(nose.dot(up)) / DEG);
    assert!(tilt_from(&w, &ship, "Moon", n) < 0.05);
}

/// Worst tilt from the slope normal (deg) over `seconds`, and the vessel.
fn tilt_on_slope(deg: f64, d: &Drop, seconds: usize) -> (f64, Vessel) {
    let w = slope_world(deg);
    let d = Drop { slope: deg, ..*d };
    let mut ship = drop(&w, &d);
    let mut worst: f64 = 0.0;
    for _ in 0..seconds {
        fly(&w, &mut ship, 1.0, &Controls::default(), false);
        worst = worst.max(tilt_from(&w, &ship, "Moon", normal(&d)));
    }
    (worst, ship)
}

#[test]
fn a_steep_slope_tips_the_craft_over() {
    // Empty tank (highest centre of mass), two feet downhill: it tips once
    // the centre of mass is beyond the downhill edge of the feet, at
    // atan(2.12 m / 3.35 m) ≈ 32°; it would slide at atan(0.8) ≈ 39°. So a
    // 30° slope holds, just, and 35° tips it over. Debug mode, so falling
    // over does not destroy it.
    let d = Drop { yaw: 45.0, propellant: 0.0, debug: true, ..Drop::default() };
    let ship = drop(&world(), &d);
    let edge = 3.0 * std::f64::consts::FRAC_1_SQRT_2;
    let tip = libm::atan(edge / (ship.mass_props().com.z - ship.craft.bottom_z)) / DEG;
    assert!(tip > 31.0 && tip < 34.0, "tip angle {tip}°");
    let (worst, ship) = tilt_on_slope(35.0, &d, 20);
    assert!(worst > 60.0, "35°: tilted at most {worst}° from the slope normal ({:?})", ship.phase);
    let (worst, ship) = tilt_on_slope(30.0, &d, 20);
    assert!(worst < 2.0 && matches!(ship.phase, Phase::Landed { .. }), "30°: {worst}°, {:?}", ship.phase);
    let (worst, ship) = tilt_on_slope(2.0, &d, 20);
    assert!(worst < 0.5 && matches!(ship.phase, Phase::Landed { .. }), "2°: {worst}°, {:?}", ship.phase);
}

#[test]
fn three_metres_per_second_on_the_gear_survives() {
    // Earth, over the (solid, flat) Atlantic: the hardest gravity.
    let w = world();
    let d = Drop { body: "Earth", lat: 0.0, lon: -25.0, speed: 3.0, gap: 0.05, ..Drop::default() };
    let mut ship = drop(&w, &d);
    fly(&w, &mut ship, 30.0, &Controls::default(), true);
    assert!(matches!(ship.phase, Phase::Landed { .. }), "{:?}", ship.phase);
    assert!(tilt_from(&w, &ship, "Earth", normal(&d)) < 0.1);
}

#[test]
fn twelve_metres_per_second_on_the_hull_destroys_the_craft() {
    let w = slope_world(0.0);
    let d = Drop { inverted: true, speed: 12.0, gap: 0.01, ..Drop::default() };
    let mut ship = drop(&w, &d);
    fly(&w, &mut ship, 5.0, &Controls::default(), true);
    let Phase::Crashed { speed, point, .. } = ship.phase else { panic!("{:?}", ship.phase) };
    assert_eq!(ship.craft.contacts[point as usize].kind, ContactKind::Hull);
    assert!((speed - 12.0).abs() < 0.1, "{speed} m/s");
    // Debug mode: infinite impact tolerance.
    let mut debug = drop(&w, &Drop { debug: true, ..d });
    fly(&w, &mut debug, 2.0, &Controls::default(), true);
    assert!(!matches!(debug.phase, Phase::Crashed { .. }));
    // On the gear at 12 m/s the feet bottom out still too fast.
    let mut gear = drop(&w, &Drop { inverted: false, ..d });
    fly(&w, &mut gear, 5.0, &Controls::default(), true);
    let Phase::Crashed { point, speed, .. } = gear.phase else { panic!("{:?}", gear.phase) };
    println!("gear at 12 m/s: point {point} ({:?}) at {speed} m/s", gear.craft.contacts[point as usize].kind);
}

/// Lands on the 2° slope, saving and loading every `save_every` frames.
fn land_with_saves(w: &World, save_every: Option<usize>) -> Vessel {
    let mut ship = drop(w, &Drop { slope: 2.0, speed: 1.0, ..Drop::default() });
    let start = ship.time;
    for k in 1..=(20.0 * 60.0) as usize {
        ship.advance(w, start.add_seconds(k as f64 / 60.0), &Controls::default(), usize::MAX);
        if save_every.is_some_and(|n| k % n == 0) {
            let mut ids = VesselIds::default();
            ids.allocate();
            let text =
                SaveGame::capture(w, ship.time, std::slice::from_ref(&ship), ids, 0, Controls::default()).to_ron();
            ship = SaveGame::from_ron(&text).unwrap().vessels.remove(0);
        }
    }
    ship
}

#[test]
fn the_resting_pose_is_identical_after_save_and_load() {
    let w = slope_world(2.0);
    let plain = land_with_saves(&w, None);
    assert!(matches!(plain.phase, Phase::Landed { .. }));
    // Saved and loaded every 7 frames while bouncing, settling and resting.
    assert_eq!(land_with_saves(&w, Some(7)), plain);
}

#[test]
fn sixty_fps_and_one_jump_land_identically() {
    let w = slope_world(2.0);
    let mut frames = drop(&w, &Drop { slope: 2.0, speed: 2.0, ..Drop::default() });
    let mut jump = frames.clone();
    let end = frames.time.add_seconds(20.0);
    fly(&w, &mut frames, 20.0, &Controls::default(), false);
    jump.advance(&w, end, &Controls::default(), usize::MAX);
    assert!(matches!(frames.phase, Phase::Landed { .. }));
    assert_eq!(frames, jump);
}

#[test]
fn landed_for_a_day_at_a_million_x_never_wakes() {
    let w = slope_world(2.0);
    let mut ship = drop(&w, &Drop { slope: 2.0, ..Drop::default() });
    fly(&w, &mut ship, 20.0, &Controls::default(), true);
    let Phase::Landed { body, fixed, att_fixed } = ship.phase.clone() else { panic!("{:?}", ship.phase) };
    let controls = Controls { sas: true, ..Controls::default() };
    let start = ship.time;
    let mut t = start;
    while t.seconds_since(start) < 86_400.0 {
        t = t.add_seconds(1e6 / 60.0);
        ship.advance(&w, t, &controls, usize::MAX);
        assert_eq!(ship.phase, Phase::Landed { body, fixed, att_fixed });
    }
    // Rotation input wakes it; it settles again where it was.
    let turn = Controls { rotate: DVec3::Z * 0.1, ..controls };
    ship.advance(&w, t.add_seconds(0.1), &turn, usize::MAX);
    assert!(matches!(ship.phase, Phase::Powered { .. }));
    let _: NodeId = body;
}
