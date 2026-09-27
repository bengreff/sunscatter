//! Lambert's problem (two-body, single revolution): the velocities that take
//! a body from `r1` to `r2` in time `dt` about a point mass `mu`.
//!
//! A planning first guess only (D008): the N-body trajectory is then
//! integrated from it. Universal-variable formulation (Bate, Mueller &
//! White; Curtis, Algorithm 5.2) solved by bisection on z, which is slower
//! than Newton's method but cannot diverge, and is deterministic.

use crate::math;
use glam::DVec3;

/// Bisection iterations on z (the bracket shrinks to f64 resolution).
const ITERATIONS: usize = 200;

/// Stumpff functions C(z) and S(z).
pub fn stumpff(z: f64) -> (f64, f64) {
    if z.abs() < 1e-3 {
        // Series: C = 1/2 − z/24 + z²/720, S = 1/6 − z/120 + z²/5040.
        (0.5 - z / 24.0 + z * z / 720.0, 1.0 / 6.0 - z / 120.0 + z * z / 5040.0)
    } else if z > 0.0 {
        let s = z.sqrt();
        ((1.0 - math::cos(s)) / z, (s - math::sin(s)) / (s * s * s))
    } else {
        let s = (-z).sqrt();
        ((math::cosh(s) - 1.0) / -z, (math::sinh(s) - s) / (s * s * s))
    }
}

/// Departure and arrival velocities for a transfer from `r1` to `r2` in
/// `dt` seconds about `mu`, moving in the sense of `normal` (the transfer's
/// angular momentum points to the same side as `normal`). `None` if no
/// single-revolution solution exists (e.g. a 180° transfer, whose plane is
/// undefined).
pub fn lambert(r1: DVec3, r2: DVec3, dt: f64, mu: f64, normal: DVec3) -> Option<(DVec3, DVec3)> {
    let (n1, n2) = (r1.length(), r2.length());
    let cross = r1.cross(r2);
    let cos_dth = (r1.dot(r2) / (n1 * n2)).clamp(-1.0, 1.0);
    let mut dth = math::acos(cos_dth);
    if cross.dot(normal) < 0.0 {
        dth = math::TAU - dth;
    }
    let sin_dth = math::sin(dth);
    if sin_dth.abs() < 1e-9 || dt <= 0.0 {
        return None;
    }
    let a = sin_dth * (n1 * n2 / (1.0 - cos_dth)).sqrt();
    let y = |z: f64| {
        let (c, s) = stumpff(z);
        n1 + n2 + a * (z * s - 1.0) / c.sqrt()
    };
    let f = |z: f64| {
        let (c, s) = stumpff(z);
        let yz = y(z);
        let q = yz / c;
        q * q.sqrt() * s + a * yz.sqrt() - mu.sqrt() * dt
    };
    // Bracket: scan z over the single-revolution range (−(2π)², (2π)²) for
    // the first rise of F through zero where y > 0 (F is not monotonic for
    // transfers longer than 180°, A < 0).
    let limit = 4.0 * math::PI * math::PI;
    let ok = |z: f64| y(z) > 0.0 && f(z).is_finite();
    let steps = 800;
    let z_at = |k: usize| -limit + 2.0 * limit * k as f64 / steps as f64;
    let (mut lo, mut hi) = (f64::NAN, f64::NAN);
    for k in 0..steps {
        let (za, zb) = (z_at(k), z_at(k + 1).min(limit - 1e-9));
        if ok(za) && ok(zb) && f(za) <= 0.0 && f(zb) > 0.0 {
            (lo, hi) = (za, zb);
            break;
        }
    }
    if lo.is_nan() {
        return None;
    }
    for _ in 0..ITERATIONS {
        let mid = 0.5 * (lo + hi);
        if f(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let z = 0.5 * (lo + hi);
    let yz = y(z);
    let ff = 1.0 - yz / n1;
    let g = a * (yz / mu).sqrt();
    let gdot = 1.0 - yz / n2;
    Some(((r2 - r1 * ff) / g, (r2 * gdot - r1) / g))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kepler::Elements;

    const MU: f64 = 3.986_004_418e14;

    /// Propagates a two-body state by `dt` through the elements.
    fn propagate(r: DVec3, v: DVec3, dt: f64) -> (DVec3, DVec3) {
        let mut el = Elements::from_state(r, v, MU);
        el.mean_anomaly += el.mean_motion(MU) * dt;
        el.to_state(MU)
    }

    #[test]
    fn stumpff_is_continuous_at_zero() {
        for z in [-1e-3, 1e-3] {
            let (c0, s0) = stumpff(z * 0.999);
            let (c1, s1) = stumpff(z * 1.001);
            assert!((c0 - c1).abs() < 1e-6 && (s0 - s1).abs() < 1e-6);
        }
    }

    #[test]
    fn recovers_the_velocity_of_known_elliptic_and_hyperbolic_arcs() {
        // (r, v, transfer time): a LEO arc, a transfer ellipse, a hyperbola.
        let cases = [
            (DVec3::new(7.0e6, 0.0, 0.0), DVec3::new(0.0, 7.5e3, 0.5e3), 1500.0),
            (DVec3::new(6.7e6, 1.0e6, 0.0), DVec3::new(-1.0e3, 10.5e3, 1.0e3), 20_000.0),
            (DVec3::new(7.0e6, 0.0, 0.0), DVec3::new(0.0, 12.0e3, 0.0), 3000.0),
        ];
        for (r1, v1, dt) in cases {
            let (r2, v2) = propagate(r1, v1, dt);
            let (l1, l2) = lambert(r1, r2, dt, MU, r1.cross(v1)).expect("a solution");
            assert!((l1 - v1).length() < 1e-3, "v1 {l1} vs {v1}");
            assert!((l2 - v2).length() < 1e-3, "v2 {l2} vs {v2}");
        }
    }

    #[test]
    fn the_sense_of_motion_picks_the_long_or_short_way() {
        let r1 = DVec3::new(7.0e6, 0.0, 0.0);
        let r2 = DVec3::new(0.0, 8.0e6, 0.0);
        let (short, _) = lambert(r1, r2, 2000.0, MU, DVec3::Z).unwrap();
        let (long, _) = lambert(r1, r2, 6000.0, MU, -DVec3::Z).unwrap();
        assert!(short.y > 0.0, "counter-clockwise");
        assert!(long.y < 0.0, "clockwise the long way");
        assert!(lambert(r1, -r1 * 1.1, 3000.0, MU, DVec3::Z).is_none(), "180°: plane undefined");
    }
}
