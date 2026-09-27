//! Two-body orbital elements and Kepler's equation.
//!
//! This is the *one* Kepler solver in the codebase (lesson: v0.1 had two that
//! disagreed by ~106 ly). Everything that needs conic math calls into here.
//!
//! Elements are classical: semi-major axis `a` (negative for hyperbolic orbits),
//! eccentricity `e`, inclination `i`, right ascension of the ascending node
//! `raan`, argument of periapsis `argp`, and mean anomaly `mean_anomaly`.
//! Degenerate cases use fixed conventions so that `from_state`/`to_state` round
//! trip: equatorial orbits take `raan = 0`; circular orbits take `argp = 0`.

pub mod lambert;

use crate::math::{self, TAU};
use glam::DVec3;
use serde::{Deserialize, Serialize};

const DEGENERATE_E: f64 = 1e-11;
const DEGENERATE_N: f64 = 1e-11;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Elements {
    pub a: f64,
    pub e: f64,
    pub i: f64,
    pub raan: f64,
    pub argp: f64,
    pub mean_anomaly: f64,
}

impl Elements {
    /// Elements from a position/velocity relative to a central body of
    /// gravitational parameter `mu`.
    pub fn from_state(r: DVec3, v: DVec3, mu: f64) -> Self {
        let rm = r.length();
        let h = r.cross(v);
        let hm = h.length();
        let h_hat = h / hm;
        let e_vec = (r * (v.length_squared() - mu / rm) - v * r.dot(v)) / mu;
        let e = e_vec.length();
        let energy = 0.5 * v.length_squared() - mu / rm;
        let a = -mu / (2.0 * energy);
        let i = math::acos((h.z / hm).clamp(-1.0, 1.0));

        let n = DVec3::Z.cross(h);
        let (raan, n_hat) = if n.length() / hm > DEGENERATE_N {
            let n_hat = n.normalize();
            (math::wrap_tau(math::atan2(n.y, n.x)), n_hat)
        } else {
            (0.0, DVec3::X)
        };

        let (argp, nu) = if e > DEGENERATE_E {
            let argp = math::atan2(n_hat.cross(e_vec).dot(h_hat), n_hat.dot(e_vec));
            let nu = math::atan2(e_vec.cross(r).dot(h_hat), e_vec.dot(r));
            (math::wrap_tau(argp), nu)
        } else {
            (0.0, math::atan2(n_hat.cross(r).dot(h_hat), n_hat.dot(r)))
        };

        Elements { a, e, i, raan, argp, mean_anomaly: mean_from_true(nu, e) }
    }

    /// Position and velocity relative to the central body.
    pub fn to_state(&self, mu: f64) -> (DVec3, DVec3) {
        let nu = true_from_mean(self.mean_anomaly, self.e);
        let p = self.a * (1.0 - self.e * self.e);
        let (s, c) = (math::sin(nu), math::cos(nu));
        let rm = p / (1.0 + self.e * c);
        let r_pf = DVec3::new(rm * c, rm * s, 0.0);
        let k = math::sqrt_pos(mu / p);
        let v_pf = DVec3::new(-k * s, k * (self.e + c), 0.0);
        (self.perifocal_to_inertial(r_pf), self.perifocal_to_inertial(v_pf))
    }

    /// Rotates a perifocal vector into the reference frame.
    fn perifocal_to_inertial(&self, v: DVec3) -> DVec3 {
        let v = math::rotate_z(v, self.argp);
        let v = math::rotate_x(v, self.i);
        math::rotate_z(v, self.raan)
    }

    /// Mean motion (rad/s).
    pub fn mean_motion(&self, mu: f64) -> f64 {
        math::sqrt_pos(mu / (self.a.abs() * self.a.abs() * self.a.abs()))
    }

    /// Orbital period (s); infinite for unbound orbits.
    pub fn period(&self, mu: f64) -> f64 {
        if self.e < 1.0 {
            TAU / self.mean_motion(mu)
        } else {
            f64::INFINITY
        }
    }

    pub fn periapsis(&self) -> f64 {
        self.a * (1.0 - self.e)
    }

    /// Apoapsis radius; infinite for unbound orbits.
    pub fn apoapsis(&self) -> f64 {
        if self.e < 1.0 {
            self.a * (1.0 + self.e)
        } else {
            f64::INFINITY
        }
    }
}

/// Mean anomaly from true anomaly.
pub fn mean_from_true(nu: f64, e: f64) -> f64 {
    if e < 1.0 {
        let big_e = math::atan2((1.0 - e * e).sqrt() * math::sin(nu), e + math::cos(nu));
        math::wrap_tau(big_e - e * math::sin(big_e))
    } else {
        let t = ((e - 1.0) / (e + 1.0)).sqrt() * math::tan(nu / 2.0);
        let h = 2.0 * math::atanh(t);
        e * math::sinh(h) - h
    }
}

/// True anomaly from mean anomaly.
pub fn true_from_mean(m: f64, e: f64) -> f64 {
    if e < 1.0 {
        let big_e = solve_elliptic(m, e);
        math::atan2((1.0 - e * e).sqrt() * math::sin(big_e), math::cos(big_e) - e)
    } else {
        let h = solve_hyperbolic(m, e);
        2.0 * math::atan2(((e + 1.0) / (e - 1.0)).sqrt() * math::sinh(h / 2.0), math::cosh(h / 2.0))
    }
}

/// Solves `E - e sin E = M` for the eccentric anomaly.
pub fn solve_elliptic(m: f64, e: f64) -> f64 {
    let m = math::wrap_pi(m);
    let mut big_e = if e < 0.8 { m + e * math::sin(m) } else { m.signum() * math::PI };
    for _ in 0..60 {
        let f = big_e - e * math::sin(big_e) - m;
        let fp = 1.0 - e * math::cos(big_e);
        let step = f / fp;
        big_e -= step;
        if step.abs() < 1e-15 {
            break;
        }
    }
    big_e
}

/// Solves `e sinh H - H = M` for the hyperbolic anomaly.
pub fn solve_hyperbolic(m: f64, e: f64) -> f64 {
    // Both estimates bound the root from above in their regimes (cubic for
    // near-parabolic, logarithmic for large M); f is convex, so Newton then
    // converges monotonically.
    let guess = math::cbrt(6.0 * m.abs()).min(math::ln(2.0 * m.abs() / e + 1.8));
    let mut h = m.signum() * guess;
    for _ in 0..100 {
        let f = e * math::sinh(h) - h - m;
        let fp = e * math::cosh(h) - 1.0;
        let step = f / fp;
        h -= step;
        if step.abs() < 1e-15 * (1.0 + h.abs()) {
            break;
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    const MU_EARTH: f64 = 3.986_004_418e14;

    fn assert_round_trip(el: Elements) {
        let (r, v) = el.to_state(MU_EARTH);
        let back = Elements::from_state(r, v, MU_EARTH);
        let (r2, v2) = back.to_state(MU_EARTH);
        let dr = (r - r2).length() / r.length();
        let dv = (v - v2).length() / v.length();
        assert!(dr < 1e-12 && dv < 1e-12, "{el:?} -> {back:?}: dr={dr:e} dv={dv:e}");
    }

    #[test]
    fn round_trips_across_orbit_types() {
        let base = Elements { a: 7.0e6, e: 0.1, i: 0.9, raan: 1.2, argp: 2.3, mean_anomaly: 0.4 };
        assert_round_trip(base);
        assert_round_trip(Elements { e: 0.0, ..base });
        assert_round_trip(Elements { i: 0.0, ..base });
        assert_round_trip(Elements { e: 0.0, i: 0.0, ..base });
        assert_round_trip(Elements { i: math::PI, ..base });
        assert_round_trip(Elements { e: 0.97, ..base });
        assert_round_trip(Elements { a: -2.0e7, e: 1.5, mean_anomaly: 3.0, ..base });
        assert_round_trip(Elements { a: -1.0e8, e: 1.001, mean_anomaly: -0.2, ..base });
    }

    #[test]
    fn round_trip_many_pseudorandom_orbits() {
        let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..2000 {
            let e = next() * 0.99;
            let el = Elements {
                a: 6.6e6 + next() * 4.0e8,
                e,
                i: next() * math::PI,
                raan: next() * TAU,
                argp: next() * TAU,
                mean_anomaly: next() * TAU,
            };
            assert_round_trip(el);
        }
    }

    #[test]
    fn kepler_solver_converges_for_high_eccentricity() {
        for &e in &[0.0, 0.5, 0.9, 0.999] {
            for k in 0..64 {
                let m = -math::PI + k as f64 * TAU / 64.0;
                let big_e = solve_elliptic(m, e);
                let resid = big_e - e * math::sin(big_e) - math::wrap_pi(m);
                assert!(resid.abs() < 1e-13, "e={e} m={m} resid={resid}");
            }
        }
    }

    #[test]
    fn leo_period_is_about_92_minutes() {
        let el = Elements { a: 6.778e6, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, mean_anomaly: 0.0 };
        let minutes = el.period(MU_EARTH) / 60.0;
        assert!((minutes - 92.6).abs() < 0.2, "{minutes}");
    }
}
