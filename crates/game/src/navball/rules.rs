//! Navball rules: pure functions of state (rule 8), table-tested below.
//!
//! All vectors share one set of inertial axes (the vessel's anchor frame);
//! only directions and differences matter. Ship body axes follow the
//! controls in `state::read_controls`: +Z is the nose, +X the right side
//! (D yaws the nose towards it), −Y the top (W pitches the nose down, towards
//! +Y).

use glam::{DQuat, DVec3};
use sim::kepler::Elements;
use std::f64::consts::{PI, TAU};

/// Below this altitude the navball shows surface mode, above it orbit mode.
pub const SURFACE_MODE_BELOW: f64 = 36_000.0;
/// Standard gravity for the g-load readout (m/s²).
pub const G0: f64 = 9.806_65;
/// Below this speed (m/s) there is no meaningful prograde.
const MIN_SPEED: f64 = 0.1;

/// Which velocity the markers and the speed readout use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Relative to the rotating surface of the reference body.
    #[default]
    Surface,
    /// Relative to the reference body's centre (inertial axes).
    Orbit,
    /// Relative to the target.
    Target,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Mode::Surface => "SURFACE",
            Mode::Orbit => "ORBIT",
            Mode::Target => "TARGET",
        }
    }

    /// The next mode when the mode label is clicked (target only with a target).
    pub fn next(self, has_target: bool) -> Mode {
        match self {
            Mode::Surface => Mode::Orbit,
            Mode::Orbit if has_target => Mode::Target,
            Mode::Orbit | Mode::Target => Mode::Surface,
        }
    }
}

/// The mode to show. Target mode (chosen by clicking) holds while a target
/// exists; a locked surface/orbit mode holds; otherwise surface below 36 km
/// and orbit above, as in KSP.
pub fn auto_mode(current: Mode, locked: bool, altitude: f64, has_target: bool) -> Mode {
    match current {
        Mode::Target if has_target => Mode::Target,
        Mode::Surface | Mode::Orbit if locked => current,
        _ if altitude < SURFACE_MODE_BELOW => Mode::Surface,
        _ => Mode::Orbit,
    }
}

/// The local horizon frame at a point: up (away from the body's centre),
/// north (towards the body's pole, in the horizontal plane) and east.
#[derive(Clone, Copy, Debug)]
pub struct Local {
    pub up: DVec3,
    pub north: DVec3,
    pub east: DVec3,
}

/// Local frame at `r` (relative to the body's centre) for a body with spin
/// axis `pole` (unit, inertial). At the poles, north is an arbitrary
/// horizontal direction.
pub fn local_frame(r: DVec3, pole: DVec3) -> Local {
    let up = r.normalize();
    let mut north = pole - up * pole.dot(up);
    if north.length_squared() < 1e-20 {
        north = up.any_orthonormal_vector();
    }
    let north = north.normalize();
    Local { up, north, east: north.cross(up) }
}

impl Local {
    /// Direction at `elevation` above the horizon and `azimuth` from north
    /// towards east (radians).
    pub fn direction(&self, elevation: f64, azimuth: f64) -> DVec3 {
        let (se, ce) = elevation.sin_cos();
        let (sa, ca) = azimuth.sin_cos();
        (self.north * ca + self.east * sa) * ce + self.up * se
    }
}

/// The ship's axes in inertial space.
#[derive(Clone, Copy, Debug)]
pub struct Ship {
    pub forward: DVec3,
    pub up: DVec3,
    pub right: DVec3,
}

/// Ship axes from the attitude quaternion (body → inertial).
pub fn ship_axes(q: DQuat) -> Ship {
    Ship { forward: q * DVec3::Z, up: q * DVec3::NEG_Y, right: q * DVec3::X }
}

/// Heading (0–360°, from north through east), pitch (−90–90°) and roll
/// (−180–180°, positive right wing down), in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Angles {
    pub heading: f64,
    pub pitch: f64,
    pub roll: f64,
}

pub fn attitude_angles(ship: &Ship, local: &Local) -> Angles {
    let f = ship.forward;
    let h = f - local.up * f.dot(local.up);
    // atan2, not asin: well conditioned near the vertical.
    let pitch = f.dot(local.up).atan2(h.length());
    // With the nose (near) vertical, the heading is where the belly (nose
    // up) or the top (nose down) points: continuous through the vertical.
    let h = if h.length() > 1e-3 { h } else { ship.up * -pitch.signum() };
    let heading = h.dot(local.east).atan2(h.dot(local.north)).to_degrees().rem_euclid(360.0);
    let (s, c) = (-ship.right.dot(local.up), ship.up.dot(local.up));
    let roll = if s.hypot(c) > 1e-9 { s.atan2(c).to_degrees() } else { 0.0 };
    Angles { heading, pitch: pitch.to_degrees(), roll }
}

/// Marker directions (unit vectors). `None` when undefined (no velocity).
#[derive(Clone, Copy, Debug, Default)]
pub struct Markers {
    pub prograde: Option<DVec3>,
    pub normal: Option<DVec3>,
    pub radial_out: Option<DVec3>,
    pub target: Option<DVec3>,
}

/// Markers for velocity `v` in the chosen frame at position `r` (relative to
/// the reference body), with the target at `target_rel` relative to the ship.
/// Prograde is `v` normalised; normal is along `r × v`; radial out is
/// prograde × normal (in the plane of `r` and `v`, perpendicular to
/// prograde, pointing away from the body). Retro/anti markers are negations.
pub fn markers(r: DVec3, v: DVec3, target_rel: Option<DVec3>) -> Markers {
    let prograde = (v.length() > MIN_SPEED).then(|| v.normalize());
    let normal = prograde.and_then(|p| {
        let n = r.cross(p);
        (n.length() > 1e-9 * r.length()).then(|| n.normalize())
    });
    let radial_out = prograde.zip(normal).map(|(p, n)| p.cross(n));
    let target = target_rel.filter(|t| t.length() > 0.0).map(DVec3::normalize);
    Markers { prograde, normal, radial_out, target }
}

/// Angle of attack (positive nose above the velocity) and sideslip (positive
/// moving to the right) in degrees, for air-relative velocity `v_air`.
pub fn aoa_sideslip(ship: &Ship, v_air: DVec3) -> Option<(f64, f64)> {
    let speed = v_air.length();
    if speed < 1.0 {
        return None;
    }
    let aoa = (-v_air.dot(ship.up)).atan2(v_air.dot(ship.forward)).to_degrees();
    let slip = (v_air.dot(ship.right) / speed).clamp(-1.0, 1.0).asin().to_degrees();
    Some((aoa, slip))
}

/// Vertical speed: the rate of change of distance from the body's centre.
pub fn vertical_speed(r: DVec3, v: DVec3) -> f64 {
    v.dot(r.normalize())
}

/// g-load from the proper (non-gravitational) acceleration.
pub fn g_load(proper_accel: DVec3) -> f64 {
    proper_accel.length() / G0
}

/// Where direction `d` appears on the ball, viewed from behind the ship: `x`
/// right and `y` up on the unit disc, and depth (> 0: front hemisphere). The
/// ball's centre is the nose; up on screen is the ship's top.
pub fn ball_project(ship: &Ship, d: DVec3) -> (f64, f64, f64) {
    (d.dot(ship.right), d.dot(ship.up), d.dot(ship.forward))
}

/// The direction shown at disc point (`x`, `y`) of the front hemisphere.
pub fn ball_unproject(ship: &Ship, x: f64, y: f64) -> DVec3 {
    let z = (1.0 - x * x - y * y).max(0.0).sqrt();
    ship.right * x + ship.up * y + ship.forward * z
}

/// Times (s) to the next apoapsis and periapsis of an osculating orbit;
/// `None` when there is none ahead (no apoapsis when unbound, no periapsis
/// after it on an escape).
pub fn time_to_apsides(el: &Elements, mu: f64) -> (Option<f64>, Option<f64>) {
    let n = el.mean_motion(mu);
    if n.is_nan() || n <= 0.0 {
        return (None, None);
    }
    let m = el.mean_anomaly;
    if el.e < 1.0 {
        let ap = (PI - m).rem_euclid(TAU) / n;
        let pe = (-m).rem_euclid(TAU) / n;
        (Some(ap), Some(pe))
    } else {
        (None, (m < 0.0).then(|| -m / n))
    }
}

/// The altitude shown above the ball, with its label: above the ground in
/// surface mode, above sea level (the reference ellipsoid) otherwise.
pub fn mode_altitude(mode: Mode, above_terrain: f64, above_sea_level: f64) -> (&'static str, f64) {
    match mode {
        Mode::Surface => ("ALT TERRAIN", above_terrain),
        Mode::Orbit | Mode::Target => ("ALT SEA", above_sea_level),
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
