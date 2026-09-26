//! Vessel attitude: orientation (body → inertial) and angular velocity.
//!
//! Rotation persists through coasts (torque-free, constant angular velocity for
//! the prototype's symmetric block). Quaternions are built with `sim::math`
//! trig, never glam's `from_axis_angle` (platform trig; see `math`).

use crate::math;
use glam::{DQuat, DVec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attitude {
    /// Rotation from vessel body axes to inertial axes. Body +Z is the nose.
    pub q: DQuat,
    /// Angular velocity (inertial axes, rad/s).
    pub omega: DVec3,
}

/// Quaternion for a rotation vector (axis × angle).
pub fn quat_from_rotvec(v: DVec3) -> DQuat {
    let angle = v.length();
    if angle < 1e-300 {
        return DQuat::IDENTITY;
    }
    let axis = v / angle;
    let (s, c) = (math::sin(0.5 * angle), math::cos(0.5 * angle));
    DQuat::from_xyzw(axis.x * s, axis.y * s, axis.z * s, c)
}

/// Quaternion rotating +Z onto `dir` (unit), with a deterministic roll.
pub fn quat_z_to(dir: DVec3) -> DQuat {
    let axis = DVec3::Z.cross(dir);
    let s = axis.length();
    let c = DVec3::Z.dot(dir);
    if s < 1e-12 {
        return if c > 0.0 { DQuat::IDENTITY } else { DQuat::from_xyzw(1.0, 0.0, 0.0, 0.0) };
    }
    quat_from_rotvec(axis / s * math::atan2(s, c))
}

impl Attitude {
    pub fn nose(&self) -> DVec3 {
        self.q * DVec3::Z
    }

    /// Torque-free propagation by `dt` seconds (constant angular velocity).
    pub fn propagate_free(&self, dt: f64) -> Attitude {
        Attitude { q: (quat_from_rotvec(self.omega * dt) * self.q).normalize(), omega: self.omega }
    }

    /// One control tick: `input` is the commanded angular acceleration
    /// direction in body axes (components in [-1, 1]); with no input and SAS
    /// on, angular velocity is damped towards zero.
    pub fn control_tick(&mut self, input: DVec3, sas: bool, max_accel: f64, dt: f64) {
        if input != DVec3::ZERO {
            self.omega += (self.q * input.clamp(DVec3::splat(-1.0), DVec3::splat(1.0))) * (max_accel * dt);
        } else if sas {
            let w = self.omega.length();
            let dw = max_accel * dt;
            self.omega = if w <= dw { DVec3::ZERO } else { self.omega * ((w - dw) / w) };
        }
        *self = self.propagate_free(dt);
    }

    /// True when a control tick would change anything (input, or SAS still
    /// damping).
    pub fn needs_ticks(&self, input: DVec3, sas: bool) -> bool {
        input != DVec3::ZERO || (sas && self.omega != DVec3::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_rotation_is_additive_in_time() {
        let a = Attitude { q: DQuat::IDENTITY, omega: DVec3::new(0.0, 0.3, 0.1) };
        let once = a.propagate_free(10.0);
        let twice = a.propagate_free(4.0).propagate_free(6.0);
        assert!((once.nose() - twice.nose()).length() < 1e-12);
    }

    #[test]
    fn sas_stops_rotation_in_finite_ticks() {
        let mut a = Attitude { q: DQuat::IDENTITY, omega: DVec3::new(0.2, 0.0, 0.0) };
        let mut ticks = 0;
        while a.needs_ticks(DVec3::ZERO, true) {
            a.control_tick(DVec3::ZERO, true, 0.5, 0.02);
            ticks += 1;
        }
        assert_eq!(a.omega, DVec3::ZERO);
        assert!(ticks <= 21, "{ticks}");
    }

    #[test]
    fn z_to_points_the_nose() {
        for d in [DVec3::X, DVec3::new(0.3, -0.4, 0.866).normalize(), -DVec3::Z, DVec3::Z] {
            assert!((quat_z_to(d) * DVec3::Z - d).length() < 1e-12);
        }
    }
}
