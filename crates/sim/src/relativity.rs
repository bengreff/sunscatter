//! Relativity (D012, motion model §6, realism-1 §4a): proper time.
//!
//! Each vessel carries δ = τ − t, its proper time minus coordinate time,
//! integrated as `dδ/dt = −U/c² − v²/(2c²)` (weak field, slow motion) with
//! `U = Σ GMᵢ/rᵢ > 0` over every gravity source (point masses; a source cut
//! from the forces still deepens the potential) and `v` the velocity
//! relative to the barycentre (through the frame tree). The rate is
//! evaluated with the forces ([`crate::forces::ForceContext::accel_rate`])
//! and integrated as an extra component of the vessel's state, outside
//! error control, so trajectories do not change.

/// Speed of light (m/s).
pub const C: f64 = 299_792_458.0;

/// `dδ/dt` for a clock in potential `u = Σ GM/r` (> 0, m²/s²) moving at
/// barycentric speed² `v2` (m²/s²): negative (clocks run slow in a well and
/// in motion).
pub fn proper_time_rate(u: f64, v2: f64) -> f64 {
    -(u + 0.5 * v2) / (C * C)
}

/// Nodes (fractions of the interval) and weights of 3-point Gauss–Legendre
/// quadrature on [0, 1] (exact for polynomials of degree 5).
pub const GAUSS3: [(f64, f64); 3] =
    [(0.5 - 0.387_298_334_620_741_7, 5.0 / 18.0), (0.5, 8.0 / 18.0), (0.5 + 0.387_298_334_620_741_7, 5.0 / 18.0)];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proper_time_rate_table() {
        let gm_earth = 3.986_004_418e14;
        let c2 = C * C;
        // (U, v²) → rate
        let cases = [
            (0.0, 0.0, 0.0),
            // Earth's surface potential: slower by 6.96e-10 (60 µs/day).
            (gm_earth / 6_378_137.0, 0.0, -gm_earth / 6_378_137.0 / c2),
            // Orbital speed alone: v²/2c².
            (0.0, 7_800.0 * 7_800.0, -0.5 * 7_800.0 * 7_800.0 / c2),
            // Both add; deeper or faster is always slower.
            (1e7, 1e7, -1.5e7 / c2),
        ];
        for (u, v2, expected) in cases {
            let r = proper_time_rate(u, v2);
            assert!((r - expected).abs() <= 1e-15 * expected.abs(), "{u} {v2}: {r} vs {expected}");
        }
        // The sign that is easy to get wrong: a clock deeper in the well
        // (larger U) runs slower, so δ decreases faster.
        assert!(proper_time_rate(gm_earth / 6.4e6, 0.0) < proper_time_rate(gm_earth / 2.66e7, 0.0));
        assert!(proper_time_rate(gm_earth / 6.4e6, 0.0) < 0.0);
        // GPS: at 20,200 km altitude the clock gains ≈ 38.6 µs/day over one
        // on the equator.
        let (r_gps, r_eq) = (6_378_137.0 + 20_200_000.0, 6_378_137.0);
        let ground = proper_time_rate(gm_earth / r_eq, 465.1 * 465.1);
        let gps = proper_time_rate(gm_earth / r_gps, gm_earth / r_gps);
        let gain = (gps - ground) * 86_400.0 * 1e6;
        assert!((gain - 38.6).abs() < 0.1, "{gain} µs/day");
    }

    #[test]
    fn gauss3_integrates_quintics_exactly() {
        let f = |x: f64| 1.0 + x - 3.0 * x * x + 2.0 * x * x * x * x * x;
        // ∫₀¹ = 1 + 1/2 − 1 + 1/3
        let got: f64 = GAUSS3.iter().map(|&(x, w)| w * f(x)).sum();
        assert!((got - (1.0 + 0.5 - 1.0 + 1.0 / 3.0)).abs() < 1e-15, "{got}");
    }
}
