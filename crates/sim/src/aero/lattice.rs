//! Lattice (grid) fins (ship-model.md §5.4): a grid of small cells the flow
//! passes through, not a panel. Their normal force on the fin's frontal
//! (cell) area is C_N = C_N,max·tanh(C_Nα(M)·α/C_N,max), α the incidence of
//! the flow on the cells' axis plus the fin's deflection.
//!
//! C_Nα against Mach (per radian, on the cells' frontal area): from the
//! wind-tunnel tests of Hiroshima & Tatsumi (ICAS 2004, "Grid pattern
//! effects on aerodynamic characteristics of grid fins", Fig. 7: a square
//! grid fin's C_Nα 0.095, 0.083 and 0.078 per degree at Mach 0.5, 1.2 and
//! 2.0 on the body's 324.3 mm², i.e. 4.7, 4.1 and 3.9 per radian on the
//! fin's 375 mm² of cells); the **transonic choking dip** (the cells' flow
//! chokes, part of it spills round the fin: Washington & Miller, AIAA
//! 93-0035, the drag's maximum at Mach 0.9) at Mach 0.95 to 0.65 of the
//! subsonic slope (E: the same tests' choked hexagonal fin kept 0.76 of its
//! subsonic slope at Mach 0.8); beyond Mach 2 the slope falls as 1/√(M²−1)
//! (supersonic linear theory, E). C_N,max 1.2: the tests' normal force at
//! 30° of incidence (Fig. 8, ~1.3–1.4 on the body area). The cells'
//! frontal faces' drag is the bake's (a Newtonian plate overstates a
//! lattice's axial force: ~1.5–2× its measured transonic peak, E).

use crate::math;

/// The largest normal-force coefficient (on the cells' frontal area).
pub const CN_MAX: f64 = 1.2;

/// (Mach, C_Nα per radian on the cells' frontal area); see the module.
const SLOPE: [(f64, f64); 9] =
    [(0.0, 4.7), (0.5, 4.7), (0.8, 4.7), (0.95, 3.1), (1.2, 4.1), (2.0, 3.9), (3.0, 2.4), (5.0, 1.38), (8.0, 0.85)];

/// Normal-force slope (per radian, on the cells' frontal area) at `mach`.
pub fn cn_alpha(mach: f64) -> f64 {
    let m = mach.max(0.0);
    match SLOPE.iter().position(|&(x, _)| x > m) {
        None => {
            let (x, y) = SLOPE[SLOPE.len() - 1];
            // Supersonic theory beyond the table: ∝ 1/√(M²−1).
            y * math::sqrt_pos(x * x - 1.0) / math::sqrt_pos(m * m - 1.0)
        }
        Some(0) => SLOPE[0].1,
        Some(i) => {
            let ((x0, y0), (x1, y1)) = (SLOPE[i - 1], SLOPE[i]);
            y0 + (y1 - y0) * (m - x0) / (x1 - x0)
        }
    }
}

fn tanh(x: f64) -> f64 {
    let e = math::exp(-2.0 * x.abs());
    ((1.0 - e) / (1.0 + e)).copysign(x)
}

/// Normal-force coefficient at incidence `alpha` (rad, cells' axis to the
/// flow, deflection included) and `mach`.
pub fn normal(alpha: f64, mach: f64) -> f64 {
    CN_MAX * tanh(cn_alpha(mach) * alpha / CN_MAX)
}

/// Its slope at `alpha` (per radian): what a deflection changes.
pub fn slope(alpha: f64, mach: f64) -> f64 {
    let t = tanh(cn_alpha(mach) * alpha / CN_MAX);
    cn_alpha(mach) * (1.0 - t * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Against the tests (Hiroshima & Tatsumi 2004 on the fin's own area):
    /// 4.7, 4.1, 3.9 per radian at Mach 0.5, 1.2, 2.0; the choking dip;
    /// the fall beyond Mach 2; saturation at large incidence.
    #[test]
    fn the_slope_table() {
        // (Mach, per radian)
        let cases = [(0.3, 4.7), (0.5, 4.7), (0.95, 3.1), (1.2, 4.1), (2.0, 3.9), (8.0, 0.85)];
        for (m, want) in cases {
            assert!((cn_alpha(m) - want).abs() < 1e-12, "{m}: {}", cn_alpha(m));
        }
        assert!(cn_alpha(0.95) < 0.7 * cn_alpha(0.5), "the transonic dip");
        assert!((cn_alpha(10.0) / cn_alpha(8.0) - (63.0f64).sqrt() / (99.0f64).sqrt()).abs() < 1e-12);
        // Linear at small incidence, saturating at large.
        let a = 2f64.to_radians();
        assert!((normal(a, 0.5) / (4.7 * a) - 1.0).abs() < 0.01);
        assert!(normal(0.5, 0.5) < CN_MAX && normal(0.5, 0.5) > 0.95 * CN_MAX);
        assert_eq!(normal(-0.1, 1.5), -normal(0.1, 1.5));
        assert!((slope(0.0, 0.5) - 4.7).abs() < 1e-12 && slope(0.5, 0.5) < 0.5);
    }
}
