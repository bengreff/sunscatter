//! Relativity (D012, D088, motion model §6, realism-1 §4a): proper time.
//!
//! Each vessel carries δ = τ − t, its proper time minus coordinate time t.
//! The coordinate time is TDB, the ephemeris' time scale (`sim::time`). A
//! clock's rate against TCB (the barycentric coordinate time) is
//! `1 − (U + v²/2)/c²` (weak field, slow motion), with `U = Σ GMᵢ/rᵢ > 0`
//! over every gravity source (point masses; a source cut from the forces
//! still deepens the potential) and `v` the velocity relative to the
//! barycentre (through the frame tree); TDB runs at `1 − L_B` against TCB
//! (IAU 2006 Resolution B3). So `dδ/dt = (L_B − (U + v²/2)/c²) / (1 − L_B)`:
//! a clock on the geoid keeps TT, which matches TDB on average, so its δ
//! has no drift, only TDB − TT's periodic terms ([`tdb_minus_tt`]). Without
//! L_B every clock lost 1.34 ms a day against the game clock. The rate is
//! evaluated with the forces ([`crate::forces::ForceContext::accel_rate`])
//! and integrated as an extra component of the vessel's state, outside
//! error control, so trajectories do not change.

use crate::math;
use crate::time::Epoch;

/// Speed of light (m/s).
pub const C: f64 = 299_792_458.0;

/// TDB's rate against TCB is `1 − L_B` (IAU 2006 Resolution B3, a defining
/// constant): `L_B = L_C + L_G − L_C·L_G`, the mean rate of a clock on the
/// geoid (TT) against TCB.
pub const L_B: f64 = 1.550_519_768e-8;

/// `dδ/dt` against TDB for a clock in potential `u = Σ GM/r` (> 0, m²/s²)
/// moving at barycentric speed² `v2` (m²/s²): the clock's rate against TCB
/// over TDB's. Deeper in a well or faster is always slower; a clock on the
/// geoid averages zero.
pub fn proper_time_rate(u: f64, v2: f64) -> f64 {
    (L_B - (u + 0.5 * v2) / (C * C)) / (1.0 - L_B)
}

/// TDB − TT at the geocentre (s): the leading terms of the Fairhead &
/// Bretagnon series as given in USNO Circular 179 (Kaplan 2005) eq. 2.6,
/// accurate to about 10 µs from 1600 to 2200 (the same terms lead SOFA's
/// `iauDtdb`). The site clock (D088) starts from it so that it reads TT,
/// and the tests check the integrated clocks against it.
pub fn tdb_minus_tt(t: Epoch) -> f64 {
    // Julian centuries from J2000 (TDB for TT: 1.7 ms changes nothing).
    let c = t.days_since_j2000() / 36_525.0;
    let terms = [
        (1_656.674_564e-6, 628.307_584_999_1, 6.240_054_195),
        (22.417_471e-6, 575.338_488_489_7, 4.296_977_442),
        (13.839_792e-6, 1_256.615_169_998_3, 6.196_904_410),
        (4.770_086e-6, 52.969_096_509_5, 0.444_401_603),
        (4.676_740e-6, 606.977_675_455_3, 4.021_195_093),
        (2.256_707e-6, 21.329_909_543_8, 5.543_113_262),
    ];
    let periodic: f64 = terms.iter().map(|&(a, w, p)| a * math::sin(w * c + p)).sum();
    // The mixed term (its amplitude grows with time).
    periodic + 10.215_672_4e-6 * c * math::sin(628.307_584_999_1 * c + 4.249_032_005)
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
        let against_tdb = |x: f64| (L_B - x) / (1.0 - L_B);
        // (U, v²) → rate
        let cases = [
            // Far from everything, at rest: TCB's rate, faster than TDB's.
            (0.0, 0.0, against_tdb(0.0)),
            // Earth's surface potential alone.
            (gm_earth / 6_378_137.0, 0.0, against_tdb(gm_earth / 6_378_137.0 / c2)),
            // Orbital speed alone: v²/2c².
            (0.0, 7_800.0 * 7_800.0, against_tdb(0.5 * 7_800.0 * 7_800.0 / c2)),
            // Both add.
            (1e7, 1e7, against_tdb(1.5e7 / c2)),
            // A clock whose U + v²/2 is L_B c² keeps TDB's rate.
            (L_B * c2, 0.0, 0.0),
        ];
        for (u, v2, expected) in cases {
            let r = proper_time_rate(u, v2);
            assert!((r - expected).abs() <= 1e-15 * expected.abs(), "{u} {v2}: {r} vs {expected}");
        }
        // The sign that is easy to get wrong: a clock deeper in the well
        // (larger U) runs slower, so δ decreases faster.
        assert!(proper_time_rate(gm_earth / 6.4e6, 0.0) < proper_time_rate(gm_earth / 2.66e7, 0.0));
        // The geoid (W0 = 62,636,856 m²/s², IERS 2010) plus the Sun's
        // potential and Earth's orbital speed at 1 AU is TT's rate: within
        // 1e-11 of TDB's (the eccentric orbit's annual term is ±3.3e-10).
        let sun = 1.327_124_400_41e20 / 1.495_978_707e11 + 0.5 * 29_784.7f64.powi(2);
        assert!(proper_time_rate(62_636_856.0 + sun, 0.0).abs() < 1e-11);
        // GPS: at 20,200 km altitude the clock gains ≈ 38.6 µs/day over one
        // on the equator.
        let (r_gps, r_eq) = (6_378_137.0 + 20_200_000.0, 6_378_137.0);
        let ground = proper_time_rate(gm_earth / r_eq, 465.1 * 465.1);
        let gps = proper_time_rate(gm_earth / r_gps, gm_earth / r_gps);
        let gain = (gps - ground) * 86_400.0 * 1e6;
        assert!((gain - 38.6).abs() < 0.1, "{gain} µs/day");
    }

    /// TDB − TT is the 1.66 ms annual term: largest in early April and
    /// October, when Earth's speed passes its mean on the way from
    /// perihelion (early January) to aphelion and back.
    #[test]
    fn tdb_minus_tt_is_the_annual_term() {
        let start = Epoch::from_calendar(2030, 1, 1, 0, 0, 0.0);
        let year: Vec<(f64, u32)> =
            (0..365).map(|d| (tdb_minus_tt(start.add_seconds(f64::from(d) * 86_400.0)), d)).collect();
        let max = year.iter().copied().max_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        let min = year.iter().copied().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        assert!((max.0 - 1.66e-3).abs() < 0.05e-3 && (85..100).contains(&max.1), "{max:?}");
        assert!((min.0 + 1.66e-3).abs() < 0.05e-3 && (270..285).contains(&min.1), "{min:?}");
    }

    #[test]
    fn gauss3_integrates_quintics_exactly() {
        let f = |x: f64| 1.0 + x - 3.0 * x * x + 2.0 * x * x * x * x * x;
        // ∫₀¹ = 1 + 1/2 − 1 + 1/3
        let got: f64 = GAUSS3.iter().map(|&(x, w)| w * f(x)).sum();
        assert!((got - (1.0 + 0.5 - 1.0 + 1.0 / 3.0)).abs() < 1e-15, "{got}");
    }
}
