//! A craft of parts' attitude control within its real actuators' limits
//! (ship-model.md §5.3): the gimbal actuators' rate, SAS's hold tuned to
//! the craft's own angular acceleration along the error (a 70 m stack's
//! weak roll authority must not slow its pitch), and slews to far targets
//! capped (`vessel::ship::turn_cap`). The test craft keeps its ideal
//! actuators (`Actuators::limits` is `None`).

use super::{Gimbal, HOLD_OMEGA};
use crate::math;
use glam::{DMat3, DVec3};

/// The limits a real craft's attitude control lives within (a craft of
/// parts; ship-model.md §5.3): the gimbal actuators' rate, and SAS tuned to
/// the craft's own authority so a 70 m stack is not swung at full gimbal
/// for every small error (no bang-bang).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// The thrust direction the gimbal holds now (body axes, unit) and how
    /// far it may turn this tick (rad: the actuators' rate × dt).
    pub gimbal_from: DVec3,
    pub gimbal_step: f64,
    /// The fastest turn SAS commands (rad/s).
    pub max_rate: f64,
}

/// A hold stays linear (never saturating) for errors under this (rad).
pub(super) const LINEAR_ERROR: f64 = 0.05;

/// The largest angular acceleration (rad/s²) the authority `auth` (per
/// body axis) gives about the direction of `v` (zero vector: none).
pub(super) fn alpha_along(inertia: &DMat3, auth: DVec3, v: DVec3) -> f64 {
    if v.length() == 0.0 {
        return 0.0;
    }
    let need = (*inertia * v.normalize()).abs();
    let a = DVec3::select(need.cmpgt(DVec3::splat(1e-12)), auth / need, DVec3::splat(f64::INFINITY));
    let a = a.min_element();
    if a.is_finite() {
        a
    } else {
        0.0
    }
}

/// A craft's hold frequency (rad/s): the test craft's 1 rad/s, a craft of
/// parts' lower where its angular acceleration `alpha` could not hold an
/// error of [`LINEAR_ERROR`] without saturating.
pub(super) fn hold_omega(limits: Option<&Limits>, alpha: f64) -> f64 {
    match limits {
        Some(_) if alpha > 0.0 => HOLD_OMEGA.min((alpha / LINEAR_ERROR).sqrt()),
        _ => HOLD_OMEGA,
    }
}

/// [`Gimbal::deflect`]'s direction for torque `tau`, the actuators turning
/// the thrust from `from` (body axes, unit) by at most `step` (rad)
/// towards it; the direction and the torque it gives.
pub(super) fn deflect_from(g: &Gimbal, tau: DVec3, from: DVec3, step: f64) -> (DVec3, DVec3) {
    let (want, _) = g.deflect(tau);
    let c = from.dot(want).clamp(-1.0, 1.0);
    let angle = math::acos(c);
    let dir = if angle <= step || angle == 0.0 {
        want
    } else {
        let perp = (want - from * c).normalize();
        from * math::cos(step) + perp * math::sin(step)
    };
    (dir, g.lever.cross(dir * g.thrust))
}

/// Whether a stability hold has no target yet (it is damping the rotation).
pub(super) fn target_free(control: &super::AttitudeControl, sas: bool) -> bool {
    sas && control.hold.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gimbal's actuators turn the thrust at most their step a tick,
    /// towards the deflection asked; a small ask is reached at once.
    #[test]
    fn the_gimbal_turns_at_its_rate() {
        let g =
            Gimbal { lever: DVec3::new(0.0, 0.0, -27.0), dir: DVec3::Z, thrust: 7.6e6, max_angle: 5f64.to_radians() };
        let step = 20f64.to_radians() * 0.02;
        let (full, _) = g.deflect(DVec3::new(1e9, 0.0, 0.0));
        let (first, torque) = deflect_from(&g, DVec3::new(1e9, 0.0, 0.0), DVec3::Z, step);
        assert!((math::acos(first.dot(DVec3::Z)) - step).abs() < 1e-12, "one step");
        assert!((torque - g.lever.cross(first * g.thrust)).length() < 1e-6);
        let mut dir = DVec3::Z;
        let mut ticks = 0;
        while (dir - full).length() > 1e-12 {
            dir = deflect_from(&g, DVec3::new(1e9, 0.0, 0.0), dir, step).0;
            ticks += 1;
        }
        assert_eq!(ticks, 13, "5° at 20°/s: 0.25 s");
        let (small, _) = g.deflect(DVec3::new(1e5, 0.0, 0.0));
        assert_eq!(deflect_from(&g, DVec3::new(1e5, 0.0, 0.0), DVec3::Z, step).0, small);
    }

    /// The angular acceleration along an error: a stack's pitch authority
    /// about pitch, unthrottled by its weak roll.
    #[test]
    fn alpha_along_an_axis() {
        let inertia = DMat3::from_diagonal(DVec3::new(2e8, 2e8, 1e6));
        let auth = DVec3::new(18e6, 18e6, 640.0);
        assert!((alpha_along(&inertia, auth, DVec3::X) - 0.09).abs() < 1e-12);
        assert!((alpha_along(&inertia, auth, DVec3::Z) - 6.4e-4).abs() < 1e-12);
        assert_eq!(alpha_along(&inertia, auth, DVec3::ZERO), 0.0);
        assert_eq!(hold_omega(Some(&Limits { gimbal_from: DVec3::Z, gimbal_step: 0.1, max_rate: 0.1 }), 0.09), 1.0);
        assert!(
            (hold_omega(Some(&Limits { gimbal_from: DVec3::Z, gimbal_step: 0.1, max_rate: 0.1 }), 0.0125) - 0.5).abs()
                < 1e-12
        );
    }
}
