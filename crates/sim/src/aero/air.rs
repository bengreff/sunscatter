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
