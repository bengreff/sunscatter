//! Air properties the aerodynamics needs (realism-1 §5a): speed of sound,
//! mean free path, Knudsen number, the stagnation pressure coefficient.
//! Their inputs come from [`crate::body::Atmosphere`] (a table, or the
//! isothermal exponential model: a scale height H means T = H·g₀·M / R).

use crate::math;

/// Universal gas constant (J/(mol·K)).
pub const GAS_CONSTANT: f64 = 8.314_462_618;
/// Mean molar mass of Earth air (kg/mol, US 1976 sea level).
pub const EARTH_AIR_MOLAR_MASS: f64 = 0.028_964_4;
/// Ratio of specific heats of Earth air.
pub const EARTH_AIR_GAMMA: f64 = 1.4;
/// Mean free path of Earth air at sea level (m, US 1976: 6.63e-8).
pub const EARTH_MEAN_FREE_PATH_SL: f64 = 6.6e-8;
/// Standard gravity (m/s²).
pub const G0: f64 = 9.806_65;

/// Temperature (K) of an isothermal atmosphere with scale height `h` (m),
/// surface gravity `g0` (m/s²) and molar mass `molar_mass` (kg/mol).
pub fn isothermal_temperature(h: f64, g0: f64, molar_mass: f64) -> f64 {
    h * g0 * molar_mass / GAS_CONSTANT
}

/// Dynamic viscosity of air (Pa·s) at `temperature` (K), Sutherland's law
/// with the US 1976 constants: μ = β·T^1.5 / (T + S), β = 1.458e-6,
/// S = 110.4 K (1.789e-5 at 288.15 K).
pub fn sutherland_viscosity(temperature: f64) -> f64 {
    let t = temperature.max(1.0);
    1.458e-6 * t * t.sqrt() / (t + 110.4)
}

/// Speed of sound (m/s): √(γ·R·T / M).
pub fn speed_of_sound(gamma: f64, temperature: f64, molar_mass: f64) -> f64 {
    (gamma * GAS_CONSTANT * temperature / molar_mass).max(0.0).sqrt()
}

/// Mean free path (m) at density `rho`, scaled from `lambda0` at `rho0`
/// (λ ∝ 1/ρ at fixed composition). Infinite in vacuum.
pub fn mean_free_path(lambda0: f64, rho0: f64, rho: f64) -> f64 {
    if rho > 0.0 {
        lambda0 * rho0 / rho
    } else {
        f64::INFINITY
    }
}

/// Knudsen number λ / L.
pub fn knudsen(mean_free_path: f64, length: f64) -> f64 {
    mean_free_path / length
}

/// Density ratio ρ₂/ρ₁ across a normal shock in equilibrium air against
/// the flight speed (m/s): dissociation and ionisation soak up the energy,
/// and the gas behind the shock is far denser than a γ = 1.4 gas's 6.
/// Hunt & Souders, *Normal- and Oblique-Shock Flow Parameters in
/// Equilibrium Air*, NASA SP-3093 (1975), the 90° rows of Table VIII
/// (53.34 km) to 7.9 km/s, Table IX (60.96 km) to 9.8 km/s and Table X
/// (68.58 km) above, each at the altitude where an entry flies that fast.
/// The ratio peaks near 8.5 km/s (oxygen fully dissociated) and dips
/// before nitrogen goes; it grows ~5 % per 8 km of altitude, which is left
/// out (Cp,max ≈ 2 − ρ₁/ρ₂ moves by under 0.5 %). 6 at 2 km/s is the
/// perfect-gas strong-shock limit, joining the table at 2.4 km/s.
const SHOCK_DENSITY_RATIO: [(f64, f64); 17] = [
    (2_000.0, 6.0),
    (2_438.4, 7.208),
    (3_048.0, 9.196),
    (3_657.6, 10.785),
    (4_267.2, 10.899),
    (4_876.8, 11.660),
    (5_486.4, 12.873),
    (6_096.0, 14.076),
    (6_705.6, 15.147),
    (7_315.2, 16.032),
    (7_924.8, 16.671),
    (8_534.4, 17.596),
    (9_144.0, 16.929),
    (9_753.6, 16.022),
    (10_363.2, 16.399),
    (10_972.8, 16.623),
    (11_582.4, 16.897),
];

/// Equilibrium-air normal-shock density ratio at `speed` (m/s): the
/// table, linear between entries, 6 below 2 km/s, the last value above.
pub fn shock_density_ratio(speed: f64) -> f64 {
    let t = &SHOCK_DENSITY_RATIO;
    if speed <= t[0].0 {
        return t[0].1;
    }
    let k = t.partition_point(|e| e.0 <= speed);
    if k >= t.len() {
        return t[t.len() - 1].1;
    }
    let (a, b) = (t[k - 1], t[k]);
    a.1 + (b.1 - a.1) * (speed - a.0) / (b.0 - a.0)
}

/// The effective ratio of specific heats for the stagnation pressure at
/// entry speeds: the γ whose strong-shock density ratio (γ+1)/(γ−1) is
/// equilibrium air's at `speed`, ε = ρ₁/ρ₂, γ_eff = (1+ε)/(1−ε), never
/// above the gas's own `gamma` (a perfect gas below ~2 km/s).
pub fn real_gas_gamma(gamma: f64, speed: f64) -> f64 {
    let eps = 1.0 / shock_density_ratio(speed);
    ((1.0 + eps) / (1.0 - eps)).min(gamma)
}

/// Stagnation pressure coefficient behind a normal shock (Rayleigh pitot
/// formula), Cp,max = (p₀₂/p₁ − 1) / (½γM²), for M ≥ 1 (clamped):
/// p₀₂/p₁ = [(γ+1)²M² / (4γM² − 2(γ−1))]^(γ/(γ−1)) · (1 − γ + 2γM²)/(γ+1).
/// Anderson, *Modern Compressible Flow*, eq. 3.79; 1.839 for γ = 1.4, M → ∞.
pub fn cp_max(gamma: f64, mach: f64) -> f64 {
    let m2 = mach.max(1.0) * mach.max(1.0);
    let g = gamma;
    let base = (g + 1.0) * (g + 1.0) * m2 / (4.0 * g * m2 - 2.0 * (g - 1.0));
    let ratio = math::exp(g / (g - 1.0) * math::ln(base)) * (1.0 - g + 2.0 * g * m2) / (g + 1.0);
    (ratio - 1.0) * 2.0 / (g * m2)
}

/// The limit of [`cp_max`] as M → ∞: [(γ+1)²/(4γ)]^(γ/(γ−1)) · 4/(γ+1).
pub fn cp_max_limit(gamma: f64) -> f64 {
    let g = gamma;
    math::exp(g / (g - 1.0) * math::ln((g + 1.0) * (g + 1.0) / (4.0 * g))) * 4.0 / (g + 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_exponential_atmosphere_air() {
        // Earth's body.ron: H = 7200 m.
        let t = isothermal_temperature(7200.0, G0, EARTH_AIR_MOLAR_MASS);
        assert!((t - 246.0).abs() < 1.0, "{t}");
        // a² = γ·H·g₀ for the isothermal atmosphere.
        let a = speed_of_sound(EARTH_AIR_GAMMA, t, EARTH_AIR_MOLAR_MASS);
        assert!((a * a / (1.4 * 7200.0 * G0) - 1.0).abs() < 1e-12, "{a}");
        // US 1976 sea level (288.15 K): 340.29 m/s.
        let a_sl = speed_of_sound(1.4, 288.15, EARTH_AIR_MOLAR_MASS);
        assert!((a_sl - 340.29).abs() < 0.05, "{a_sl}");
    }

    #[test]
    fn mean_free_path_scales_inversely_with_density() {
        let cases = [(1.225, 6.6e-8), (1.225e-3, 6.6e-5), (1.225e-9, 6.6e1)];
        for (rho, want) in cases {
            let l = mean_free_path(EARTH_MEAN_FREE_PATH_SL, 1.225, rho);
            assert!((l / want - 1.0).abs() < 1e-12, "{rho}: {l}");
        }
        assert_eq!(mean_free_path(6.6e-8, 1.225, 0.0), f64::INFINITY);
        assert!((knudsen(0.5, 5.0) - 0.1).abs() < 1e-15);
    }

    #[test]
    fn sutherland_matches_us_1976() {
        // US 1976 Table I: 1.7894e-5 at sea level, 1.4216e-5 at 216.65 K.
        assert!((sutherland_viscosity(288.15) / 1.7894e-5 - 1.0).abs() < 1e-4);
        assert!((sutherland_viscosity(216.65) / 1.4216e-5 - 1.0).abs() < 1e-4);
    }

    #[test]
    fn real_gas_stagnation_pressure_at_entry_speeds() {
        // At lunar-return speed Cp,max ≈ 1.9–2.0 (≈ 2 − ρ₁/ρ₂ for a strong
        // shock; Anderson, Hypersonic and High-Temperature Gas Dynamics,
        // §3.2 and ch. 14), against 1.84 for a perfect gas.
        let cp = |v: f64, m: f64| cp_max(real_gas_gamma(EARTH_AIR_GAMMA, v), m);
        let lunar = cp(11_000.0, 36.0);
        assert!(lunar > 1.9 && lunar < 2.0, "{lunar}");
        assert!((lunar - (2.0 - 1.0 / 16.62)).abs() < 0.01, "{lunar}");
        // SP-3093's normal-shock rows: 7.2084 at 2.4384 km/s, 16.671 at
        // 7.9248 (53.34 km), 17.596 at 8.5344 (60.96 km, the peak).
        assert_eq!(shock_density_ratio(2_438.4), 7.208);
        assert_eq!(shock_density_ratio(7_924.8), 16.671);
        assert_eq!(shock_density_ratio(8_534.4), 17.596);
        // Low speed: the perfect gas.
        assert_eq!(real_gas_gamma(1.4, 1_000.0), 1.4);
        assert!((cp(1_500.0, 5.0) - cp_max(1.4, 5.0)).abs() < 1e-15);
        // Continuous, within 1.9–2.0 above 3.6 km/s (it dips past the peak).
        let mut last = cp(1_000.0, 1_000.0 / 300.0);
        for k in 1..200 {
            let v = 1_000.0 + 100.0 * k as f64;
            let c = cp(v, v / 300.0);
            assert!((c - last).abs() < 0.03, "{v}");
            assert!(v < 3_600.0 || (c > 1.9 && c < 2.0), "{v}: {c}");
            last = c;
        }
    }

    #[test]
    fn rayleigh_pitot_values() {
        // (γ, M, Cp,max) from the normal-shock tables (p₀₂/p₁ at M 2: 5.640;
        // M 10: 129.2).
        let cases =
            [(1.4, 1.0, 2.0 * (1.8929 - 1.0) / 1.4), (1.4, 2.0, 2.0 * 4.640 / 5.6), (1.4, 10.0, 2.0 * 128.2 / 140.0)];
        for (g, m, want) in cases {
            let c = cp_max(g, m);
            assert!((c / want - 1.0).abs() < 2e-3, "M {m}: {c} vs {want}");
        }
        assert!((cp_max_limit(1.4) - 1.8394).abs() < 1e-4);
        assert!((cp_max(1.4, 1e6) / cp_max_limit(1.4) - 1.0).abs() < 1e-9);
    }
}
