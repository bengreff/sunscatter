//! The closed-form zonal terms J2, J4 and J6 the ephemeris generator uses
//! between bodies (`gen::nbody`, with reaction). Their arithmetic is frozen:
//! the shipped ephemeris regenerates from it bit for bit (the golden tests).
//! Vessels use [`super::Field`], which these match to rounding
//! (`gravity::tests::closed_forms_match_the_field`).

use glam::DVec3;

/// Acceleration at `r` (relative to the oblate body's centre) from its J2 term.
pub fn j2_accel(r: DVec3, gm: f64, j2: f64, radius: f64, pole: DVec3) -> DVec3 {
    let d2 = r.length_squared();
    let d = d2.sqrt();
    let z = r.dot(pole);
    let k = -1.5 * j2 * gm * radius * radius / (d2 * d2 * d);
    let zr2 = 5.0 * z * z / d2;
    (r * (1.0 - zr2) + pole * (2.0 * z)) * k
}

/// Acceleration at `r` (relative to the oblate body's centre) from its J4
/// and J6 terms: the gradient of the potential's zonal part
/// `V_n = −(μ/r) J_n (R/r)^n P_n(z/r)` (with `V = μ/r + Σ V_n`, `a = ∇V`;
/// with [`j2_accel`], the zonal field to degree 6).
pub fn j4_j6_accel(r: DVec3, gm: f64, j4: f64, j6: f64, radius: f64, pole: DVec3) -> DVec3 {
    let d2 = r.length_squared();
    let d = d2.sqrt();
    let s = r.dot(pole) / d; // sine of the latitude
    let s2 = s * s;
    let q = radius * radius / d2;
    // a_n = ∇V_n = u J_n q^(n/2) [(n+1) P_n(s) r̂ − P_n'(s) (p̂ − s r̂)],
    // u = μ/r² (for n = 2 this is j2_accel: inward at the equator).
    let p4 = (35.0 * s2 * s2 - 30.0 * s2 + 3.0) / 8.0;
    let dp4 = (140.0 * s2 * s - 60.0 * s) / 8.0;
    let p6 = (231.0 * s2 * s2 * s2 - 315.0 * s2 * s2 + 105.0 * s2 - 5.0) / 16.0;
    let dp6 = (1386.0 * s2 * s2 * s - 1260.0 * s2 * s + 210.0 * s) / 16.0;
    let (c4, c6) = (j4 * q * q, j6 * q * q * q);
    let radial = 5.0 * c4 * p4 + 7.0 * c6 * p6;
    let polar = -(c4 * dp4 + c6 * dp6);
    let u = gm / d2;
    let rhat = r / d;
    (rhat * radial + (pole - rhat * s) * polar) * u
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j2_matches_textbook_components() {
        // On the equator J2 pulls inward by 1.5 J2 μ R²/r⁴; over the pole it
        // pushes outward by 3 J2 μ R²/r⁴ (less mass under the poles).
        let (gm, j2, rad) = (3.986e14, 1.0826e-3, 6.378e6);
        let r = 7.0e6;
        let eq = j2_accel(DVec3::new(r, 0.0, 0.0), gm, j2, rad, DVec3::Z);
        let pol = j2_accel(DVec3::new(0.0, 0.0, r), gm, j2, rad, DVec3::Z);
        let base = j2 * gm * rad * rad / r.powi(4);
        assert!((eq.x + 1.5 * base).abs() < 1e-12 * base.abs().max(1.0));
        assert!((pol.z - 3.0 * base).abs() < 1e-12 * base.abs().max(1.0));
    }

    /// J2, J4 and J6 are the gradient of the zonal potential
    /// `V_n = −(μ/r) J_n (R/r)^n P_n(z/r)` (acceleration `∇V`, the sign under
    /// which J2 pulls inward at the equator), by finite differences: one
    /// convention for all three, so J4 and J6 cannot have J2's sign flipped.
    #[test]
    fn zonal_terms_are_the_gradient_of_the_zonal_potential() {
        let (gm, rad, j2, j4, j6) = (1.266_865_3e17, 7.1492e7, 1.4697e-2, -5.866e-4, 3.42e-5);
        let pole = DVec3::new(0.1, -0.2, 1.0).normalize();
        let potential = |r: DVec3, jn: [f64; 3]| {
            let d = r.length();
            let s = r.dot(pole) / d;
            let p2 = (3.0 * s * s - 1.0) / 2.0;
            let p4 = (35.0 * s.powi(4) - 30.0 * s * s + 3.0) / 8.0;
            let p6 = (231.0 * s.powi(6) - 315.0 * s.powi(4) + 105.0 * s * s - 5.0) / 16.0;
            let q = rad / d;
            -(gm / d) * (jn[0] * q.powi(2) * p2 + jn[1] * q.powi(4) * p4 + jn[2] * q.powi(6) * p6)
        };
        let grad = |r: DVec3, jn: [f64; 3]| {
            let h = 1.0;
            let f = |dr: DVec3| potential(r + dr * h, jn) - potential(r - dr * h, jn);
            DVec3::new(f(DVec3::X), f(DVec3::Y), f(DVec3::Z)) / (2.0 * h)
        };
        for r in [DVec3::new(4.2e8, 1.0e7, 3.0e7), DVec3::new(1.0e8, -5.0e7, 6.0e7), DVec3::new(0.0, 0.0, 9.0e7)] {
            let a2 = j2_accel(r, gm, j2, rad, pole);
            let g2 = grad(r, [j2, 0.0, 0.0]);
            assert!((a2 - g2).length() < 1e-6 * g2.length(), "J2 {a2} vs {g2}");
            let a46 = j4_j6_accel(r, gm, j4, j6, rad, pole);
            let g46 = grad(r, [0.0, j4, j6]);
            assert!((a46 - g46).length() < 1e-5 * g46.length(), "J4/J6 {a46} vs {g46}");
        }
    }
}
