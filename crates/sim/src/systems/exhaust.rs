//! The exhaust seed (D109; technical/exhaust.md): each engine's state at its
//! nozzle exit, as a pure function of its data and its flow level, and the
//! jet it makes in the local air. What the exhaust is drawn from; the
//! trajectory physics never reads it.
//!
//! * **Exit state.** The published thrust and Isp fix the effective exhaust
//!   velocity `c = F_vac/ṁ = vₑ + pₑ·Aₑ/ṁ`. In an isentropic nozzle of
//!   expansion ratio ε the exit Mach `Mₑ` follows from ε and γ alone (the
//!   area–Mach relation), and `pₑ·Aₑ/(ṁ·vₑ) = 1/(γ·Mₑ²)`, so
//!   `vₑ = c/(1 + 1/(γ·Mₑ²))`, `ρₑ = ṁ/(vₑ·Aₑ)`, `pₑ = ρₑ·vₑ²/(γ·Mₑ²)`,
//!   consistent with the thrust the trajectory flies (`F_vac = ṁ·vₑ +
//!   pₑ·Aₑ`, exactly). γ and the molar mass are the propellant pair's
//!   (`data/propellants.ron`, handbook placeholders); the chamber
//!   temperature they imply is checked against the handbook's.
//! * **Exit temperature.** Not the single-γ (frozen) figure: the gas
//!   recombines as it expands (shifting equilibrium), which keeps it
//!   hotter (LOX/RP-1 at ε 16: ~1,800 K, not 1,330 K). From the pair's
//!   chamber temperature down the pressure ratio,
//!   `Tₑ = T_c·(pₑ/p_c)^((γₛ−1)/γₛ)`, with `γₛ` fitted to CEA-method runs
//!   and `p_c/pₑ` the isentropic ratio at `Mₑ`.
//! * **Throttle.** As the steady map (`propulsion::steady`, an ideal
//!   nozzle): the flow and the exit pressure and density scale with the
//!   level, the velocity and temperature do not.
//! * **The jet in air** ([`jet`]): expanded isentropically from the exit to
//!   the ambient pressure, the fully expanded Mach `Mⱼ` and radius, the
//!   Prandtl–Meyer turning at the lip (outward when underexpanded, inward
//!   when overexpanded; continuous through matched) and the shock-cell
//!   length (Prandtl–Pack, `λ = 1.22·Dⱼ·√(Mⱼ² − 1)`).
//!
//! CRUCIBLE (the owner's engine testbench) will give each engine's exit
//! state directly; it replaces [`exit_state`]'s inputs, never [`ExitState`].

use crate::body::DataError;
use crate::catalog::EngineData;
use crate::math;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The propellant table's file in `data/`.
pub const PROPELLANTS_FILE: &str = "propellants.ron";
/// Universal gas constant (J/(kmol·K)).
pub const R_UNIVERSAL: f64 = 8_314.462_618;
/// The expansion ratio of an engine whose throat is not given (the test
/// craft's): a sea-level booster's, Merlin 1D's 16.
pub const DEFAULT_EXPANSION: f64 = 16.0;
/// The lowest ambient pressure the jet is expanded to (Pa): vacuum's jet
/// is the limit of a continuous function, never a separate case.
pub const VACUUM_FLOOR: f64 = 1e-6;
/// Pack's constant of the shock-cell length (Powell 2010, Prandtl's 1.306
/// refined).
pub const PACK: f64 = 1.22;

/// What a propellant pair's exhaust is (one row of `propellants.ron`).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Products {
    /// Ratio of specific heats of the expanding gas.
    pub gamma: f64,
    /// Mean molar mass of the products (g/mol).
    pub molar_mass: f64,
    /// The handbook's chamber temperature (K) at its mixture ratio: a
    /// check of γ, and where the exit temperature starts.
    pub chamber_temperature: f64,
    /// The exponent of temperature against pressure through the nozzle in
    /// shifting equilibrium (`T ∝ p^((γₛ−1)/γₛ)`), fitted to CEA-method
    /// runs: what sets the exit temperature.
    pub shifting_gamma: f64,
    pub mixture_ratio: f64,
    /// Oxidizer to fuel mass ratio that burns the fuel completely.
    pub stoichiometric_ratio: f64,
    /// Mole fraction of water at the exit.
    pub water: f64,
    /// Mass fractions of condensed particles.
    pub soot: f64,
    pub alumina: f64,
    pub source: String,
}

/// The propellant pairs by `"oxidizer/fuel"`.
#[derive(Clone, Debug, PartialEq)]
pub struct Propellants(BTreeMap<String, Products>);

impl Propellants {
    pub fn parse(text: &str) -> Result<Self, String> {
        let rows: BTreeMap<String, Products> = ron::from_str(text).map_err(|e| e.to_string())?;
        for (k, p) in &rows {
            let ok = p.gamma > 1.0 && p.shifting_gamma > 1.0 && p.molar_mass > 0.0 && p.stoichiometric_ratio > 0.0;
            let fractions = [p.water, p.soot, p.alumina].iter().all(|f| (0.0..=1.0).contains(f));
            if !ok || !fractions {
                return Err(format!(
                    "{k}: needs gamma and shifting_gamma > 1, molar mass and ratio > 0, fractions in [0, 1]"
                ));
            }
        }
        Ok(Propellants(rows))
    }

    pub fn load(path: &Path) -> Result<Self, DataError> {
        let err = |message: String| DataError { path: path.to_path_buf(), message };
        Self::parse(&std::fs::read_to_string(path).map_err(|e| err(e.to_string()))?).map_err(err)
    }

    /// The pair burning `oxidizer` and `fuel`.
    pub fn pair(&self, oxidizer: &str, fuel: &str) -> Option<&Products> {
        self.0.get(&format!("{oxidizer}/{fuel}"))
    }
}

pub fn default_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").join(PROPELLANTS_FILE)
}

/// The table of [`default_path`], loaded once per process. Panics if the
/// file is missing or invalid.
pub fn propellants() -> &'static Propellants {
    static TABLE: OnceLock<Propellants> = OnceLock::new();
    TABLE.get_or_init(|| Propellants::load(&default_path()).unwrap_or_else(|e| panic!("{e}")))
}

/// What the seed needs of an engine at full flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NozzleSpec {
    /// Vacuum thrust (N) and mass flow (kg/s) at full flow.
    pub thrust_vac: f64,
    pub mdot_full: f64,
    /// Exit area (m²) and expansion ratio (exit over throat area).
    pub exit_area: f64,
    pub expansion: f64,
}

impl NozzleSpec {
    /// An engine of a craft of parts: its exit area as the steady map's
    /// (`propulsion::exit_area`), its throat's from the geometry.
    pub fn of(d: &EngineData) -> Self {
        let exit_area = super::propulsion::exit_area(d);
        let expansion = if d.throat_area > 0.0 { exit_area / d.throat_area } else { DEFAULT_EXPANSION };
        NozzleSpec { thrust_vac: d.thrust_vac, mdot_full: super::propulsion::full_flow(d), exit_area, expansion }
    }

    /// The test craft's engine (no throat given: [`DEFAULT_EXPANSION`]).
    pub fn of_engine(e: &crate::craft::engine::Engine) -> Self {
        NozzleSpec {
            thrust_vac: e.thrust_vac,
            mdot_full: e.mdot_max(),
            exit_area: e.exit_area,
            expansion: DEFAULT_EXPANSION,
        }
    }

    /// The flow level (share of full flow) that gives `thrust` (N) at
    /// ambient pressure `p` (Pa): the steady map inverted.
    pub fn level(&self, thrust: f64, p: f64) -> f64 {
        if thrust > 0.0 && self.thrust_vac > 0.0 {
            ((thrust + p * self.exit_area) / self.thrust_vac).max(0.0)
        } else {
            0.0
        }
    }
}

/// What the exhaust is made of, for its light.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Composition {
    /// Mole fraction of water.
    pub water: f64,
    /// Mass fractions of soot and alumina particles.
    pub soot: f64,
    pub alumina: f64,
    /// Share of the fuel left unburned (a fuel-rich mixture), what can
    /// afterburn in air: `1 − MR/MR_stoich`, at least 0.
    pub unburned: f64,
}

/// An engine's exhaust at its nozzle exit (the seed).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ExitState {
    /// Mass flow (kg/s).
    pub mdot: f64,
    /// Velocity (m/s), pressure (Pa), temperature (K), density (kg/m³).
    pub velocity: f64,
    pub pressure: f64,
    pub temperature: f64,
    pub density: f64,
    pub gamma: f64,
    /// Mean molar mass (g/mol).
    pub molar_mass: f64,
    pub mach: f64,
    /// Exit radius (m).
    pub radius: f64,
    pub composition: Composition,
}

/// Area ratio `A/A*` of isentropic flow at Mach `m`.
pub fn area_ratio(m: f64, gamma: f64) -> f64 {
    let k = (gamma + 1.0) / (2.0 * (gamma - 1.0));
    math::pow(2.0 / (gamma + 1.0) * (1.0 + 0.5 * (gamma - 1.0) * m * m), k) / m
}

/// The supersonic Mach of area ratio `ratio` (≥ 1; bisection in log Mach,
/// where the ratio rises monotonically).
pub fn supersonic_mach(ratio: f64, gamma: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, math::ln(1e4));
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if area_ratio(math::exp(mid), gamma) < ratio {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    math::exp(0.5 * (lo + hi))
}

/// The Prandtl–Meyer angle (rad) at Mach `m` ≥ 1.
pub fn prandtl_meyer(m: f64, gamma: f64) -> f64 {
    let g = (gamma + 1.0) / (gamma - 1.0);
    let b = (m * m - 1.0).max(0.0).sqrt();
    g.sqrt() * math::atan2(b / g.sqrt(), 1.0) - math::atan2(b, 1.0)
}

/// The exhaust at the exit of nozzle `n` burning `products` at flow
/// `level` (share of full flow; zero: none) with oxidizer-to-fuel mass
/// ratio `mixture_ratio`.
pub fn exit_state(n: &NozzleSpec, products: &Products, mixture_ratio: f64, level: f64) -> ExitState {
    let gamma = products.gamma;
    let mach = supersonic_mach(n.expansion.max(1.0), gamma);
    let c = n.thrust_vac / n.mdot_full;
    let velocity = c / (1.0 + 1.0 / (gamma * mach * mach));
    let mdot = n.mdot_full * level.max(0.0);
    let density = mdot / (velocity * n.exit_area);
    let pressure = density * velocity * velocity / (gamma * mach * mach);
    let stagnation = 1.0 + 0.5 * (gamma - 1.0) * mach * mach;
    let gs = products.shifting_gamma;
    let temperature = products.chamber_temperature * math::pow(stagnation, -gamma / (gamma - 1.0) * (gs - 1.0) / gs);
    let composition = Composition {
        water: products.water,
        soot: products.soot,
        alumina: products.alumina,
        unburned: (1.0 - mixture_ratio / products.stoichiometric_ratio).max(0.0),
    };
    let radius = (n.exit_area / math::PI).sqrt();
    ExitState {
        mdot,
        velocity,
        pressure,
        temperature,
        density,
        gamma,
        molar_mass: products.molar_mass,
        mach,
        radius,
        composition,
    }
}

/// The chamber temperature (K) an exit state's velocity implies through
/// its single γ (isentropic, at rest in the chamber; the frozen exit
/// temperature `vₑ²/(γ·R·Mₑ²)` times the stagnation ratio): the check of
/// γ and the molar mass against the handbook.
pub fn chamber_temperature(s: &ExitState) -> f64 {
    let frozen = s.velocity * s.velocity / (s.gamma * R_UNIVERSAL / s.molar_mass * s.mach * s.mach);
    frozen * (1.0 + 0.5 * (s.gamma - 1.0) * s.mach * s.mach)
}

/// The jet an exit state makes in air at pressure `ambient` (Pa).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jet {
    /// Exit over ambient pressure (the nozzle pressure ratio; > 1:
    /// underexpanded).
    pub pressure_ratio: f64,
    /// Mach and radius (m) of the jet fully expanded to the ambient.
    pub mach: f64,
    pub radius: f64,
    /// Turning of the flow at the lip (rad; outward > 0).
    pub turn: f64,
    /// Shock-cell length (m).
    pub cell: f64,
}

pub fn jet(s: &ExitState, ambient: f64) -> Jet {
    let g = s.gamma;
    let pressure_ratio = s.pressure / ambient.max(VACUUM_FLOOR);
    let stagnation = 1.0 + 0.5 * (g - 1.0) * s.mach * s.mach;
    let expanded = stagnation * math::pow(pressure_ratio, (g - 1.0) / g);
    let mach = ((expanded - 1.0) / (0.5 * (g - 1.0))).max(1.0).sqrt();
    let radius = s.radius * (area_ratio(mach, g) / area_ratio(s.mach, g)).sqrt();
    let turn = prandtl_meyer(mach, g) - prandtl_meyer(s.mach, g);
    let cell = PACK * 2.0 * radius * (mach * mach - 1.0).sqrt();
    Jet { pressure_ratio, mach, radius, turn, cell }
}

#[cfg(test)]
#[path = "exhaust_tests.rs"]
mod tests;
