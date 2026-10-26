//! Three-dimensional relief of the boundary layer (Mangler; D085): on a
//! body whose cross-section grows along the flow (a cone, a nose) the
//! layer spreads sideways and stays thinner than a flat plate's at the
//! same run length, so it rubs and heats more: on a sharp cone √3 times a
//! plate's laminar heating (Tauber, NASA TP-2914, eq. 47, the Mangler
//! transformation) and ~1.15 times its turbulent heating (eq. 59).
//!
//! The axisymmetric analogue: a layer of run length x over a body whose
//! wetted perimeter is P(x) is a plate's of run length x_eff = ∫₀ˣ P^k dx /
//! P(x)^k, k = 2 laminar (Mangler), 1.25 turbulent (the ⅕-power law's;
//! (9/4)^⅕ = 1.18 on a cone). Baked per flow direction: P(x) at
//! [`STATIONS`] slabs along it from the attached faces (Σ A·√(1 − (n·d)²)
//! per slab over its width, each face shared between the two nearest slab
//! middles), and the ratios x_eff/x at the slabs' middles
//! (1 on a plate or a cylinder along the flow, ⅓ and 1/2.25 on a cone; above
//! 1 where the body narrows, the layer thickening), within [`RATIO_MIN`]
//! and [`RATIO_MAX`]. At run time each cell takes the ratios at its
//! fraction of the run, interpolated like the exposure between the grid's
//! directions. At incidence the windward and leeward rays share one
//! perimeter (the cross-section's), an approximation.

use glam::DVec3;

/// Slabs along the flow.
pub const STATIONS: usize = 32;
/// Bounds of x_eff/x (a body closing to a point would send it to ∞).
pub const RATIO_MIN: f64 = 0.2;
pub const RATIO_MAX: f64 = 3.0;
/// Faces turned downstream beyond this (n·d above) are separated.
const SEPARATED: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// The ratios (laminar, turbulent) at each slab for triangles (centroid,
/// unit normal, area) along `d` over `lo..hi` (positions along `d`).
pub fn ratios(tris: &[(DVec3, DVec3, f64)], d: DVec3, lo: f64, hi: f64) -> [(f32, f32); STATIONS] {
    let dx = ((hi - lo) / STATIONS as f64).max(1e-9);
    let mut perimeter = [0.0; STATIONS];
    for &(c, n, a) in tris {
        let nd = n.dot(d);
        if nd > SEPARATED {
            continue;
        }
        // Shared between the two nearest slab middles (no aliasing between
        // the mesh's rows and the slabs).
        let p = a * (1.0 - nd * nd).max(0.0).sqrt() / dx;
        let u = ((c.dot(d) - lo) / dx - 0.5).clamp(0.0, (STATIONS - 1) as f64);
        let k = (u.floor() as usize).min(STATIONS - 2);
        let f = u - k as f64;
        perimeter[k] += p * (1.0 - f);
        perimeter[k + 1] += p * f;
    }
    // P linear between the slab middles (from its value extrapolated to
    // the front, at least 0): ∫P² exact, ∫P^1.25 by trapezoids.
    let pow = |p: f64| if p > 0.0 { crate::math::exp(1.25 * crate::math::ln(p)) } else { 0.0 };
    let front = (2.0 * perimeter[0] - perimeter[1]).max(0.0);
    let mut out = [(1.0f32, 1.0f32); STATIONS];
    let (mut prev, mut step) = (front, 0.5 * dx);
    let (mut lam, mut turb) = (0.0, 0.0);
    for (j, &p) in perimeter.iter().enumerate() {
        lam += step * (prev * prev + prev * p + p * p) / 3.0;
        turb += step * 0.5 * (pow(prev) + pow(p));
        let x = (j as f64 + 0.5) * dx;
        let ratio = |sum: f64, here: f64| if here > 0.0 { (sum / (here * x)).clamp(RATIO_MIN, RATIO_MAX) } else { 1.0 };
        out[j] = (ratio(lam, p * p) as f32, ratio(turb, pow(p)) as f32);
        (prev, step) = (p, dx);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cone (apex at the origin, half-angle δ, along +Z) and a cylinder,
    /// as rings of thin triangles' (centroid, normal, area).
    fn rings(radius: impl Fn(f64) -> f64, slope: f64, length: f64) -> Vec<(DVec3, DVec3, f64)> {
        let (n_ax, n_az) = (400, 64);
        let mut tris = Vec::new();
        for i in 0..n_ax {
            let z = (i as f64 + 0.5) * length / n_ax as f64;
            let r = radius(z);
            for k in 0..n_az {
                let a = (k as f64 + 0.5) * crate::math::TAU / n_az as f64;
                let (s, c) = (crate::math::sin(a), crate::math::cos(a));
                let normal = DVec3::new(c, s, -slope).normalize();
                let area = crate::math::TAU * r / n_az as f64 * length / n_ax as f64 * (1.0 + slope * slope).sqrt();
                tris.push((DVec3::new(r * c, s * r, z), normal, area));
            }
        }
        tris
    }

    #[test]
    fn a_cone_has_mangler_s_third_and_a_cylinder_a_plate_s_run() {
        // The flow along −Z… the cone's apex upstream: d = +Z from the apex.
        let t = 0.2f64.tan();
        let cone = rings(|z| z * t, t, 3.0);
        let r = ratios(&cone, DVec3::Z, 0.0, 3.0);
        for &(lam, turb) in &r[4..] {
            assert!((f64::from(lam) * 3.0 - 1.0).abs() < 0.02, "{lam}");
            assert!((f64::from(turb) * 2.25 - 1.0).abs() < 0.02, "{turb}");
        }
        let cylinder = rings(|_| 0.5, 0.0, 5.0);
        for &(lam, turb) in &ratios(&cylinder, DVec3::Z, 0.0, 5.0) {
            assert!((f64::from(lam) - 1.0).abs() < 1e-6 && (f64::from(turb) - 1.0).abs() < 1e-6);
        }
        // Laminar heating ∝ 1/√x_eff: √3 a plate's on the cone (eq. 47);
        // turbulent ∝ x_eff^−⅕: 2.25^⅕ = 1.176 (Tauber's eq. 59: 1.15).
        assert!((1.0 / f64::from(r[16].0).sqrt() / 3f64.sqrt() - 1.0).abs() < 0.01);
        let turb = crate::math::exp(-0.2 * crate::math::ln(f64::from(r[16].1)));
        assert!((turb / 1.15 - 1.0).abs() < 0.03, "{turb}");
    }
}
