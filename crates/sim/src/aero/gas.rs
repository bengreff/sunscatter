//! The gas of an atmosphere (D048 data-driven, D085): what the
//! aerodynamics and heating need of it beyond γ and the molar mass (both
//! data of the atmosphere): viscosity, Prandtl number and the equilibrium
//! normal-shock density ratio at entry speeds. An atmosphere names its gas
//! (`gas` in `body.ron`, Earth air by default).
//!
//! * **Viscosity:** Sutherland's law μ = β·T^1.5/(T + S). Air: the US 1976
//!   constants (β 1.458e-6, S 110.4 K). CO₂ and N₂: White, *Viscous Fluid
//!   Flow*, Table 1-2 (μ₀ 1.370e-5 and 1.663e-5 Pa·s at 273 K, S 222 and
//!   107 K; as reproduced in the COMSOL CFD manual), β = μ₀(273 + S)/273^1.5.
//!   Against NIST's data CO₂ is 0.4 % low at 317 K, 2.8 % at 517 K, 5.9 % at
//!   1,117 K; N₂ exact at 300 K, 3.7 % low at 1,000 K.
//! * **Prandtl number:** air 0.72; CO₂ 0.74 (NIST at 0.01 MPa: 0.757 at
//!   317 K to 0.716 at 1,117 K); N₂ 0.72 (0.705–0.742 at 300–1,000 K). The
//!   recovery factors √Pr and ∛Pr and Colburn's Pr^−⅔ follow.
//! * **Shock density ratio** ρ₂/ρ₁ against the flight speed (for the
//!   Newtonian stagnation pressure, [`Gas::real_gas_gamma`]): air from Hunt
//!   & Souders (NASA SP-3093, [`super::air::shock_density_ratio`]); CO₂
//!   (Mars, Venus) from Miller & Wilder, NASA SP-3100 (1976, NTRS
//!   19770009349), Table I, the moving-shock density ratio into CO₂ at
//!   300 K and 20 Pa (at 100 Pa within 6 %); N₂ (Titan) takes air's: no
//!   published equilibrium-N₂ table was found, and an equilibrium N₂/N/N⁺
//!   calculation (rigid rotor, harmonic oscillator; computed for this
//!   comparison at Huygens' peak-heating free stream) is within 13 % of air's
//!   at 4–7 km/s, under 0.5 % in Cp,max ≈ 2 − ρ₁/ρ₂.

use super::air;

/// The gas of an atmosphere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Gas {
    /// Earth air.
    #[default]
    Air,
    /// Mostly carbon dioxide with a few per cent N₂ and Ar (Mars, Venus).
    CarbonDioxide,
    /// Mostly nitrogen with a few per cent CH₄ (Titan).
    Nitrogen,
}

/// The properties of one gas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Properties {
    /// Sutherland's law: μ = beta·T^1.5/(T + s).
    pub beta: f64,
    pub s: f64,
    pub prandtl: f64,
    /// √Pr and ∛Pr (the recovery factors, rounded as is usual for air).
    pub laminar_recovery: f64,
    pub turbulent_recovery: f64,
    /// Colburn's analogy factor Pr^−⅔.
    pub colburn: f64,
    /// Equilibrium normal-shock density ratio against speed (m/s).
    shock: &'static [(f64, f64)],
}

impl Properties {
    /// Dynamic viscosity (Pa·s) at `temperature` (K).
    pub fn viscosity(&self, temperature: f64) -> f64 {
        let t = temperature.max(1.0);
        self.beta * t * t.sqrt() / (t + self.s)
    }
}

const AIR: Properties = Properties {
    beta: 1.458e-6,
    s: 110.4,
    prandtl: 0.72,
    laminar_recovery: 0.85,
    turbulent_recovery: 0.89,
    colburn: 1.244_8,
    shock: &air::SHOCK_DENSITY_RATIO,
};

/// CO₂: ρ₂/ρ₁ by NASA SP-3100, Table I (p₁ 20 Pa, T₁ 300 K). It peaks near
/// 5 km/s (CO₂ dissociated) and dips past 6 km/s as CO and O go.
const CO2_SHOCK: [(f64, f64); 15] = [
    (1_000.0, 6.10),
    (2_000.0, 10.50),
    (3_000.0, 16.27),
    (4_000.0, 20.67),
    (5_000.0, 22.59),
    (5_200.0, 22.62),
    (6_000.0, 18.70),
    (6_400.0, 17.38),
    (7_000.0, 18.12),
    (8_000.0, 19.58),
    (9_200.0, 20.47),
    (10_000.0, 19.56),
    (11_000.0, 17.90),
    (12_000.0, 17.35),
    (13_000.0, 17.12),
];

const CO2: Properties = Properties {
    beta: 1.503_425e-6,
    s: 222.0,
    prandtl: 0.74,
    laminar_recovery: 0.860_2,
    turbulent_recovery: 0.904_5,
    colburn: 1.222_3,
    shock: &CO2_SHOCK,
};

const N2: Properties = Properties {
    beta: 1.400_980e-6,
    s: 107.0,
    prandtl: 0.72,
    laminar_recovery: 0.85,
    turbulent_recovery: 0.89,
    colburn: 1.244_8,
    shock: &air::SHOCK_DENSITY_RATIO,
};

impl Gas {
    /// The gas's properties.
    pub fn properties(self) -> &'static Properties {
        match self {
            Gas::Air => &AIR,
            Gas::CarbonDioxide => &CO2,
            Gas::Nitrogen => &N2,
        }
    }

    /// Dynamic viscosity (Pa·s) at `temperature` (K).
    pub fn viscosity(self, temperature: f64) -> f64 {
        self.properties().viscosity(temperature)
    }

    /// Equilibrium normal-shock density ratio at `speed` (m/s): the gas's
    /// table, linear between entries, held at its ends.
    pub fn shock_density_ratio(self, speed: f64) -> f64 {
        let t = self.properties().shock;
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
    /// `speed` (m/s): the γ whose strong-shock density ratio (γ+1)/(γ−1) is
    /// the gas's equilibrium ratio, ε = ρ₁/ρ₂, γ_eff = (1+ε)/(1−ε), never
    /// above the gas's own `gamma`.
    pub fn real_gas_gamma(self, gamma: f64, speed: f64) -> f64 {
        let eps = 1.0 / self.shock_density_ratio(speed);
        ((1.0 + eps) / (1.0 - eps)).min(gamma)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math;

    #[test]
    fn viscosity_is_sutherland_s_with_white_s_constants() {
        // At 273 K each gas's μ₀ (White, Table 1-2); air exactly the US 1976 law.
        assert!((Gas::CarbonDioxide.viscosity(273.0) / 1.370e-5 - 1.0).abs() < 1e-6);
        assert!((Gas::Nitrogen.viscosity(273.0) / 1.663e-5 - 1.0).abs() < 1e-6);
        // Air: Sutherland's law with the US 1976 constants, Table I:
        // 1.7894e-5 at sea level, 1.4216e-5 at 216.65 K.
        assert!((Gas::Air.viscosity(288.15) / 1.7894e-5 - 1.0).abs() < 1e-4);
        assert!((Gas::Air.viscosity(216.65) / 1.4216e-5 - 1.0).abs() < 1e-4);
        // The factors follow the Prandtl numbers.
        for g in [Gas::Air, Gas::CarbonDioxide, Gas::Nitrogen] {
            let p = g.properties();
            // Air's conventional 0.85 and 0.89 are rounded (√0.72 = 0.849, ∛0.72 = 0.896).
            assert!((p.laminar_recovery / p.prandtl.sqrt() - 1.0).abs() < 1e-2, "{g:?}");
            assert!((p.turbulent_recovery / math::cbrt(p.prandtl) - 1.0).abs() < 1e-2, "{g:?}");
            assert!((p.colburn / math::exp(-2.0 / 3.0 * math::ln(p.prandtl)) - 1.0).abs() < 1e-4, "{g:?}");
        }
    }

    #[test]
    fn co2_compresses_more_than_air_behind_a_strong_shock() {
        // SP-3100 Table I at 20 Pa: 16.27 at 3 km/s, 22.59 at 5, 18.70 at 6.
        let co2 = Gas::CarbonDioxide;
        assert_eq!(co2.shock_density_ratio(3000.0), 16.27);
        assert_eq!(co2.shock_density_ratio(5000.0), 22.59);
        assert!((co2.shock_density_ratio(5500.0) - (22.62 + (18.70 - 22.62) * 0.375)).abs() < 1e-12);
        // At Mars entry speeds CO₂'s Cp,max is above air's (2 − ε for a
        // strong shock); at low speed both are their perfect gases.
        let cp = |g: Gas, v: f64, gamma: f64| air::cp_max(g.real_gas_gamma(gamma, v), 30.0);
        assert!(cp(co2, 6000.0, 1.29) > cp(Gas::Air, 6000.0, 1.4));
        assert!((cp(co2, 6000.0, 1.29) - (2.0 - 1.0 / 18.70)).abs() < 0.02);
        assert_eq!(co2.real_gas_gamma(1.29, 500.0), 1.29);
        // Nitrogen takes air's table.
        assert_eq!(Gas::Nitrogen.shock_density_ratio(5126.0), Gas::Air.shock_density_ratio(5126.0));
    }
}
