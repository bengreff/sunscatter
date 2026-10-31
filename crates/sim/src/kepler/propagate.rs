//! Two-body propagation of a state by `dt`, for any conic (universal
//! variables; Curtis, Algorithms 3.3 and 3.4): what flight software uses as
//! its model of an orbit (the maneuver program's target, `systems::maneuver`).
//! The physics never calls it: trajectories are integrated (D008).
//!
//! Newton's method on the universal Kepler equation from Curtis's first
//! guess, a fixed number of iterations at most: deterministic.

use super::lambert::stumpff;
use glam::DVec3;

/// Newton iterations at most.
const ITERATIONS: usize = 60;

/// The state `(r, v)` relative to a point mass `mu` carried `dt` seconds
/// (either sign) along its conic. `None` when the equation does not
/// converge or the numbers are not finite (a radial or degenerate state).
pub fn propagate(r0: DVec3, v0: DVec3, mu: f64, dt: f64) -> Option<(DVec3, DVec3)> {
    let r0n = r0.length();
    if !(r0n > 0.0 && mu > 0.0 && dt.is_finite()) {
        return None;
    }
    if dt == 0.0 {
        return Some((r0, v0));
    }
    let sqrt_mu = mu.sqrt();
    let vr0 = r0.dot(v0) / r0n;
    // The reciprocal of the semi-major axis (negative: hyperbolic).
    let alpha = 2.0 / r0n - v0.length_squared() / mu;
    let mut x = sqrt_mu * alpha.abs() * dt;
    if alpha.abs() < 1e-12 || !x.is_finite() || x == 0.0 {
        x = sqrt_mu * dt / r0n;
    }
    let tolerance = 1e-12 * (1.0 + crate::math::cbrt(sqrt_mu * dt.abs()));
    let mut converged = false;
    for _ in 0..ITERATIONS {
        let z = alpha * x * x;
        let (c, s) = stumpff(z);
        let f = r0n * vr0 / sqrt_mu * x * x * c + (1.0 - alpha * r0n) * x * x * x * s + r0n * x - sqrt_mu * dt;
        let df = r0n * vr0 / sqrt_mu * x * (1.0 - z * s) + (1.0 - alpha * r0n) * x * x * c + r0n;
        if !(f.is_finite() && df.is_finite()) || df == 0.0 {
            return None;
        }
        let step = f / df;
        x -= step;
        if step.abs() <= tolerance {
            converged = true;
            break;
        }
    }
    if !converged {
        return None;
    }
    let z = alpha * x * x;
    let (c, s) = stumpff(z);
    let f = 1.0 - x * x / r0n * c;
    let g = dt - x * x * x / sqrt_mu * s;
    let r = r0 * f + v0 * g;
    let rn = r.length();
    let fdot = sqrt_mu / (rn * r0n) * (z * s - 1.0) * x;
    let gdot = 1.0 - x * x / rn * c;
    let v = r0 * fdot + v0 * gdot;
    (r.is_finite() && v.is_finite() && rn > 0.0).then_some((r, v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kepler::Elements;

    const MU: f64 = 3.986_004_418e14;

    #[test]
    fn an_ellipse_matches_the_mean_anomaly_advance() {
        // (elements, dt) → the state Elements gives at M + n·dt.
        let cases = [
            (Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.3 }, 1_200.0),
            (Elements { a: 24_000_000.0, e: 0.7, i: 0.3, raan: 2.0, argp: 1.5, mean_anomaly: 3.0 }, 30_000.0),
            (Elements { a: 7_000_000.0, e: 0.1, i: 0.0, raan: 0.0, argp: 0.0, mean_anomaly: 1.0 }, -2_000.0),
        ];
        for (el, dt) in cases {
            let (r0, v0) = el.to_state(MU);
            let (r, v) = propagate(r0, v0, MU, dt).expect("converges");
            let later = Elements { mean_anomaly: el.mean_anomaly + el.mean_motion(MU) * dt, ..el };
            let (rw, vw) = later.to_state(MU);
            assert!((r - rw).length() < 1e-3, "{el:?} {dt}: {} m", (r - rw).length());
            assert!((v - vw).length() < 1e-6, "{el:?} {dt}: {} m/s", (v - vw).length());
        }
    }

    #[test]
    fn there_and_back_returns_to_the_start() {
        // Ellipse, near-parabola and hyperbola: forward then back.
        let r0 = DVec3::new(7.0e6, 0.0, 0.0);
        let escape = (2.0 * MU / r0.length()).sqrt();
        for (speed, dt) in [(7_600.0, 4_000.0), (escape * 0.999_999, 3_000.0), (escape * 1.3, 20_000.0)] {
            let v0 = DVec3::new(300.0, speed, 100.0);
            let (r, v) = propagate(r0, v0, MU, dt).expect("forward");
            let (rb, vb) = propagate(r, v, MU, -dt).expect("back");
            assert!((rb - r0).length() < 1e-2, "{speed}: {} m", (rb - r0).length());
            assert!((vb - v0).length() < 1e-5, "{speed}: {} m/s", (vb - v0).length());
            // Energy and angular momentum are kept.
            let energy = |r: DVec3, v: DVec3| 0.5 * v.length_squared() - MU / r.length();
            assert!((energy(r, v) - energy(r0, v0)).abs() < 1e-6 * energy(r0, v0).abs().max(1.0));
            assert!((r.cross(v) - r0.cross(v0)).length() < 1e-6 * r0.cross(v0).length());
        }
    }

    #[test]
    fn degenerate_inputs_have_no_answer() {
        assert_eq!(propagate(DVec3::ZERO, DVec3::Y, MU, 10.0), None);
        assert_eq!(propagate(DVec3::X * 7e6, DVec3::Y * 7e3, MU, f64::NAN), None);
        assert_eq!(propagate(DVec3::X * 7e6, DVec3::Y * 7e3, MU, 0.0), Some((DVec3::X * 7e6, DVec3::Y * 7e3)));
    }
}
