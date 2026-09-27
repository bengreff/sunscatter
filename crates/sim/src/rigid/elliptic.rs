//! Elliptic functions for the torque-free rigid body ([`super::free`]):
//! Jacobi sn/cn/dn, the complete and incomplete integrals of the first kind,
//! and Gauss–Legendre quadrature. Parameter `m = k²` in [0, 1).
//!
//! Deterministic: square roots and `sim::math` trig only.

use crate::math;

/// Arithmetic–geometric mean.
fn agm(mut a: f64, mut b: f64) -> f64 {
    for _ in 0..64 {
        if (a - b).abs() <= 1e-16 * a {
            break;
        }
        (a, b) = (0.5 * (a + b), (a * b).sqrt());
    }
    a
}

/// Complete elliptic integral of the first kind K(m).
pub fn complete_k(m: f64) -> f64 {
    math::PI / (2.0 * agm(1.0, (1.0 - m).sqrt()))
}

/// Carlson's symmetric integral R_F(x, y, z) (duplication, Numerical
/// Recipes §6.12; relative error below 1e-16).
pub fn carlson_rf(x: f64, y: f64, z: f64) -> f64 {
    const ERRTOL: f64 = 0.0025;
    let (mut x, mut y, mut z) = (x, y, z);
    let mut ave;
    let (mut dx, mut dy, mut dz);
    loop {
        let (sx, sy, sz) = (x.sqrt(), y.sqrt(), z.sqrt());
        let lambda = sx * (sy + sz) + sy * sz;
        x = 0.25 * (x + lambda);
        y = 0.25 * (y + lambda);
        z = 0.25 * (z + lambda);
        ave = (x + y + z) / 3.0;
        dx = (ave - x) / ave;
        dy = (ave - y) / ave;
        dz = (ave - z) / ave;
        if dx.abs().max(dy.abs()).max(dz.abs()) <= ERRTOL {
            break;
        }
    }
    let e2 = dx * dy - dz * dz;
    let e3 = dx * dy * dz;
    (1.0 + (e2 / 24.0 - 0.1 - 3.0 / 44.0 * e3) * e2 + e3 / 14.0) / ave.sqrt()
}

/// Incomplete elliptic integral of the first kind F(φ | m), for any φ
/// (the inverse of the Jacobi amplitude).
pub fn incomplete_f(phi: f64, m: f64) -> f64 {
    let n = libm::round(phi / math::PI);
    let r = phi - n * math::PI;
    let (s, c) = (math::sin(r), math::cos(r));
    s * carlson_rf(c * c, 1.0 - m * s * s, 1.0) + 2.0 * n * complete_k(m)
}

/// Jacobi elliptic functions (sn, cn, dn)(u | m), by the descending Landen
/// transformation (Abramowitz & Stegun 16.4). `u` is first reduced to one
/// period, 4K.
pub fn sn_cn_dn(u: f64, m: f64) -> (f64, f64, f64) {
    if m == 0.0 {
        return (math::sin(u), math::cos(u), 1.0);
    }
    let period = 4.0 * complete_k(m);
    let u = u - period * libm::floor(u / period);
    let mut a = [0.0f64; 17];
    let mut c = [0.0f64; 17];
    a[0] = 1.0;
    c[0] = m.sqrt();
    let mut b = (1.0 - m).sqrt();
    let mut n = 0;
    while c[n].abs() > 1e-16 && n < 16 {
        a[n + 1] = 0.5 * (a[n] + b);
        c[n + 1] = 0.5 * (a[n] - b);
        b = (a[n] * b).sqrt();
        n += 1;
    }
    let mut phi = math::powi(2.0, n as i32) * a[n] * u;
    for i in (1..=n).rev() {
        phi = 0.5 * (phi + math::asin((c[i] / a[i] * math::sin(phi)).clamp(-1.0, 1.0)));
    }
    let (sn, cn) = (math::sin(phi), math::cos(phi));
    (sn, cn, (1.0 - m * sn * sn).max(0.0).sqrt())
}

/// 8-point Gauss–Legendre nodes (positive half) and weights on [−1, 1].
const GL8: [(f64, f64); 4] = [
    (0.183_434_642_495_649_8, 0.362_683_783_378_362),
    (0.525_532_409_916_329, 0.313_706_645_877_887_3),
    (0.796_666_477_413_626_7, 0.222_381_034_453_374_5),
    (0.960_289_856_497_536_3, 0.101_228_536_290_376_3),
];

/// ∫ f over [a, b] with `panels` equal panels of 8-point Gauss–Legendre.
pub fn gauss_legendre(f: impl Fn(f64) -> f64, a: f64, b: f64, panels: usize) -> f64 {
    let panels = panels.max(1);
    let h = (b - a) / panels as f64;
    let mut total = 0.0;
    for p in 0..panels {
        let mid = a + h * (p as f64 + 0.5);
        let half = 0.5 * h;
        let mut s = 0.0;
        for &(x, w) in &GL8 {
            s += w * (f(mid - half * x) + f(mid + half * x));
        }
        total += s * half;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_integral_table() {
        // Reference values (A&S table 17.1).
        for (m, k) in [(0.0, std::f64::consts::FRAC_PI_2), (0.5, 1.854_074_677_301_372), (0.9, 2.578_092_113_348_173)] {
            assert!((complete_k(m) - k).abs() < 1e-14, "K({m}) = {}", complete_k(m));
        }
    }

    #[test]
    fn jacobi_functions_invert_the_integral_and_keep_their_identities() {
        for m in [0.0, 1e-9, 0.1, 0.5, 0.9, 0.999] {
            for phi in [-3.0, -1.2, 0.0, 0.3, 1.5, 2.9, 7.0] {
                let u = incomplete_f(phi, m);
                let (sn, cn, dn) = sn_cn_dn(u, m);
                assert!((sn - math::sin(phi)).abs() < 1e-12 && (cn - math::cos(phi)).abs() < 1e-12, "m {m} φ {phi}");
                assert!((dn * dn + m * sn * sn - 1.0).abs() < 1e-14);
            }
            // Period 4K for sn and cn, and quarter-period values.
            let k = complete_k(m);
            let (s1, c1, d1) = sn_cn_dn(k, m);
            assert!((s1 - 1.0).abs() < 1e-12 && c1.abs() < 1e-7 && (d1 - (1.0 - m).sqrt()).abs() < 1e-7, "m {m}");
            let (a, b) = (sn_cn_dn(0.7, m), sn_cn_dn(0.7 + 4.0 * k * 3.0, m));
            assert!((a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12);
        }
    }

    #[test]
    fn jacobi_derivatives_match() {
        // d sn/du = cn·dn: a check of all three functions together.
        for m in [0.2, 0.8] {
            for u in [0.1, 1.3, 2.9, 5.5] {
                let h = 1e-6;
                let d = (sn_cn_dn(u + h, m).0 - sn_cn_dn(u - h, m).0) / (2.0 * h);
                let (_, cn, dn) = sn_cn_dn(u, m);
                assert!((d - cn * dn).abs() < 1e-8, "m {m} u {u}");
            }
        }
    }

    #[test]
    fn quadrature_is_exact_for_smooth_functions() {
        let i = gauss_legendre(math::cos, 0.0, 2.0, 2);
        assert!((i - math::sin(2.0)).abs() < 1e-15);
        let p = gauss_legendre(|x| math::powi(x, 15), -1.0, 2.0, 1);
        assert!((p - (math::powi(2.0, 16) - 1.0) / 16.0).abs() < 1e-9);
    }
}
