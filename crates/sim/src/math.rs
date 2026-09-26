//! Deterministic math.
//!
//! All transcendental functions used by the simulation go through this module.
//! They are backed by the pure-Rust `libm` crate, so they return identical bits
//! on every platform; `std`'s versions call the platform math library, which
//! differs between macOS and Windows. Basic IEEE operations (`+ - * /`,
//! `sqrt`) are already exact and deterministic. Never use `mul_add` in sim code.
//!
//! Also do not call glam's trig-based helpers (`from_axis_angle`, `slerp`, ...)
//! inside the simulation: they use `std` trig. Use [`rotate_z`] and friends.

use glam::DVec3;

pub const TAU: f64 = std::f64::consts::TAU;
pub const PI: f64 = std::f64::consts::PI;

#[inline]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}
#[inline]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}
#[inline]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}
#[inline]
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}
#[inline]
pub fn acos(x: f64) -> f64 {
    libm::acos(x)
}
#[inline]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}
#[inline]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}
#[inline]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}
#[inline]
pub fn sinh(x: f64) -> f64 {
    libm::sinh(x)
}
#[inline]
pub fn cosh(x: f64) -> f64 {
    libm::cosh(x)
}
#[inline]
pub fn atanh(x: f64) -> f64 {
    libm::atanh(x)
}
#[inline]
pub fn cbrt(x: f64) -> f64 {
    libm::cbrt(x)
}
#[inline]
pub fn powi(x: f64, n: i32) -> f64 {
    // Repeated multiplication: deterministic, unlike `f64::powi`.
    let mut r = 1.0;
    for _ in 0..n.unsigned_abs() {
        r *= x;
    }
    if n < 0 {
        1.0 / r
    } else {
        r
    }
}

/// Square root clamped at zero (guards tiny negative round-off).
#[inline]
pub fn sqrt_pos(x: f64) -> f64 {
    x.max(0.0).sqrt()
}

/// Wraps an angle into `[0, TAU)`.
#[inline]
pub fn wrap_tau(x: f64) -> f64 {
    // fmod is exact in IEEE arithmetic; use libm's to avoid the platform library.
    let mut r = libm::fmod(x, TAU);
    if r < 0.0 {
        r += TAU;
    }
    if r >= TAU {
        0.0
    } else {
        r
    }
}

/// Wraps an angle into `[-PI, PI)`.
#[inline]
pub fn wrap_pi(x: f64) -> f64 {
    wrap_tau(x + PI) - PI
}

/// Rotates `v` about the +z axis by `angle` radians.
#[inline]
pub fn rotate_z(v: DVec3, angle: f64) -> DVec3 {
    let (s, c) = (sin(angle), cos(angle));
    DVec3::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z)
}

/// Rotates `v` about the +x axis by `angle` radians.
#[inline]
pub fn rotate_x(v: DVec3, angle: f64) -> DVec3 {
    let (s, c) = (sin(angle), cos(angle));
    DVec3::new(v.x, c * v.y - s * v.z, s * v.y + c * v.z)
}

/// Rotates `v` about the unit axis `k` by `angle` radians (Rodrigues).
#[inline]
pub fn rotate_axis(v: DVec3, k: DVec3, angle: f64) -> DVec3 {
    let (s, c) = (sin(angle), cos(angle));
    v * c + k.cross(v) * s + k * (k.dot(v) * (1.0 - c))
}

/// Compensated (Kahan–Babuška/Neumaier) accumulator for a 3-vector.
///
/// Integrators add many small increments to a large position; plain summation
/// random-walks by ~√N ulp. The compensation term is part of the integrator
/// state and must be stored with it so chunked integration stays bit-identical.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Compensated {
    pub sum: DVec3,
    pub comp: DVec3,
}

impl Compensated {
    pub fn new(value: DVec3) -> Self {
        Self { sum: value, comp: DVec3::ZERO }
    }

    /// Adds `delta`, tracking the rounding error.
    #[inline]
    pub fn add(&mut self, delta: DVec3) {
        self.sum = DVec3::new(
            neumaier(self.sum.x, delta.x, &mut self.comp.x),
            neumaier(self.sum.y, delta.y, &mut self.comp.y),
            neumaier(self.sum.z, delta.z, &mut self.comp.z),
        );
    }

    /// The best estimate of the accumulated value.
    #[inline]
    pub fn value(&self) -> DVec3 {
        self.sum + self.comp
    }
}

#[inline]
fn neumaier(sum: f64, x: f64, comp: &mut f64) -> f64 {
    let t = sum + x;
    if sum.abs() >= x.abs() {
        *comp += (sum - t) + x;
    } else {
        *comp += (x - t) + sum;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compensated_sum_beats_naive() {
        let big = DVec3::splat(1.5e11);
        let mut naive = big;
        let mut comp = Compensated::new(big);
        for _ in 0..1_000_000 {
            naive += DVec3::splat(1.0e-3);
            comp.add(DVec3::splat(1.0e-3));
        }
        let exact = 1.5e11 + 1000.0;
        assert!((comp.value().x - exact).abs() < 1e-4);
        assert!((naive.x - exact).abs() > (comp.value().x - exact).abs());
    }

    #[test]
    fn wrap_handles_huge_angles() {
        let a = 1.0e10_f64;
        let w = wrap_tau(a);
        assert!((0.0..TAU).contains(&w));
        assert!((sin(w) - sin(a)).abs() < 1e-6);
        assert!((-PI..PI).contains(&wrap_pi(-7.0)));
    }

    #[test]
    fn rotations_agree() {
        let v = DVec3::new(1.0, 2.0, 3.0);
        let a = rotate_z(v, 0.7);
        let b = rotate_axis(v, DVec3::Z, 0.7);
        assert!((a - b).length() < 1e-14);
        assert!((rotate_x(v, 0.3) - rotate_axis(v, DVec3::X, 0.3)).length() < 1e-14);
    }

    #[test]
    fn powi_matches_repeated_multiplication() {
        assert_eq!(powi(2.0, 10), 1024.0);
        assert_eq!(powi(2.0, -2), 0.25);
    }
}
