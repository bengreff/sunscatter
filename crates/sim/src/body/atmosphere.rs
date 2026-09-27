//! A body's atmosphere: density, pressure, temperature and composition by
//! altitude, and the air's properties the aerodynamics and heating need
//! (`sim::aero::air`, `sim::thermal`). Heights are above the reference
//! ellipsoid.
//!
//! Two models:
//! * **Table** (Earth: U.S. Standard Atmosphere 1976 to 1,000 km, generated
//!   by `tools/us76_table.py`): rows of (altitude, temperature, pressure,
//!   density, mean molar mass); temperature and molar mass linear between
//!   rows, pressure and density log-linear (each interval an exponential
//!   with its own scale height). Below the first row the first row's
//!   values; above the last, the last interval's exponentials continued.
//! * **Exponential** (bodies without a table): ρ₀·e^(−h/H), isothermal at
//!   the temperature the scale height implies (T = H·g₀·M/R).
//!
//! Flight uses the air only below `top` (density zero above it: the live
//! boundary of vessels and the rails floor, D062); [`Atmosphere::profile`]
//! reads the model at any height.

use crate::aero::air;
use crate::math;

/// Boltzmann constant (J/K).
pub const BOLTZMANN: f64 = 1.380_649e-23;
/// Effective collision diameter of Earth air molecules (m; US 1976, used
/// for its mean free path).
pub const EARTH_AIR_COLLISION_DIAMETER: f64 = 3.65e-10;

/// The atmosphere of a body. The air fields default to Earth air.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Atmosphere {
    /// Exponential model: sea-level density (kg/m³).
    #[serde(default)]
    pub rho0: f64,
    /// Exponential model: scale height (m).
    #[serde(default)]
    pub scale_height: f64,
    /// Altitude above which flight sees no air (m).
    pub top: f64,
    /// Exponential model: sea-level pressure (Pa); falls with the same
    /// scale height (engines' back pressure). Zero if not given.
    #[serde(default)]
    pub p0: f64,
    /// Ratio of specific heats.
    #[serde(default = "air_defaults::gamma")]
    pub gamma: f64,
    /// Exponential model: mean molar mass (kg/mol).
    #[serde(default = "air_defaults::molar_mass")]
    pub molar_mass: f64,
    /// Exponential model: mean free path at `rho0` (m); λ ∝ 1/ρ.
    #[serde(default = "air_defaults::mean_free_path")]
    pub mean_free_path: f64,
    /// Table model: effective collision diameter of the molecules (m), for
    /// the mean free path λ = k·T / (√2·π·d²·p).
    #[serde(default = "air_defaults::collision_diameter")]
    pub collision_diameter: f64,
    /// Sutton–Graves stagnation heating constant (SI, `thermal::sutton_graves`).
    #[serde(default = "air_defaults::sutton_graves_k")]
    pub sutton_graves_k: f64,
    /// The tabulated profile; the exponential model without it.
    #[serde(default)]
    pub table: Option<AtmosphereTable>,
}

/// Earth air: the defaults of [`Atmosphere`]'s air fields.
mod air_defaults {
    pub fn gamma() -> f64 {
        crate::aero::air::EARTH_AIR_GAMMA
    }
    pub fn molar_mass() -> f64 {
        crate::aero::air::EARTH_AIR_MOLAR_MASS
    }
    pub fn mean_free_path() -> f64 {
        crate::aero::air::EARTH_MEAN_FREE_PATH_SL
    }
    pub fn collision_diameter() -> f64 {
        super::EARTH_AIR_COLLISION_DIAMETER
    }
    pub fn sutton_graves_k() -> f64 {
        crate::thermal::heating::SUTTON_GRAVES_EARTH
    }
}

/// The air at one height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirState {
    /// kg/m³.
    pub rho: f64,
    /// Pa.
    pub pressure: f64,
    /// Kinetic temperature (K).
    pub temperature: f64,
    /// Mean molar mass (kg/mol).
    pub molar_mass: f64,
    /// Mean free path (m).
    pub mean_free_path: f64,
}

impl AirState {
    /// Speed of sound (m/s) for the ratio of specific heats `gamma`.
    pub fn speed_of_sound(&self, gamma: f64) -> f64 {
        air::speed_of_sound(gamma, self.temperature, self.molar_mass)
    }
}

/// One row as written in the data: (altitude m, temperature K, pressure Pa,
/// density kg/m³, mean molar mass kg/mol).
pub type TableRow = (f64, f64, f64, f64, f64);

/// A tabulated atmosphere (rows by increasing altitude, at least two).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "Vec<TableRow>", into = "Vec<TableRow>")]
pub struct AtmosphereTable {
    rows: Vec<TableRow>,
    /// ln p and ln ρ per row.
    ln: Vec<(f64, f64)>,
}

impl TryFrom<Vec<TableRow>> for AtmosphereTable {
    type Error = String;

    fn try_from(rows: Vec<TableRow>) -> Result<Self, String> {
        if rows.len() < 2 {
            return Err("an atmosphere table needs at least two rows".into());
        }
        for (i, r) in rows.iter().enumerate() {
            if !(r.1 > 0.0 && r.2 > 0.0 && r.3 > 0.0 && r.4 > 0.0) {
                return Err(format!("atmosphere table row {i}: values must be positive"));
            }
            if i > 0 && r.0 <= rows[i - 1].0 {
                return Err(format!("atmosphere table row {i}: altitudes must increase"));
            }
        }
        let ln = rows.iter().map(|r| (math::ln(r.2), math::ln(r.3))).collect();
        Ok(AtmosphereTable { rows, ln })
    }
}

impl From<AtmosphereTable> for Vec<TableRow> {
    fn from(t: AtmosphereTable) -> Self {
        t.rows
    }
}

impl AtmosphereTable {
    pub fn rows(&self) -> &[TableRow] {
        &self.rows
    }

    /// The interval of `altitude` and the fraction along it (clamped below
    /// the first row, extrapolated above the last).
    fn locate(&self, altitude: f64) -> (usize, f64) {
        let n = self.rows.len();
        let i = self.rows.partition_point(|r| r.0 <= altitude).clamp(1, n - 1) - 1;
        let (a, b) = (self.rows[i].0, self.rows[i + 1].0);
        (i, ((altitude - a) / (b - a)).max(0.0))
    }

    fn log_linear(&self, i: usize, f: f64, pick: fn(&(f64, f64)) -> f64) -> f64 {
        let (a, b) = (pick(&self.ln[i]), pick(&self.ln[i + 1]));
        math::exp(a + (b - a) * f)
    }

    pub fn density(&self, altitude: f64) -> f64 {
        let (i, f) = self.locate(altitude);
        self.log_linear(i, f, |l| l.1)
    }

    pub fn pressure(&self, altitude: f64) -> f64 {
        let (i, f) = self.locate(altitude);
        self.log_linear(i, f, |l| l.0)
    }

    /// Temperature and molar mass (linear; clamped to the end rows).
    fn temperature_and_molar_mass(&self, altitude: f64) -> (f64, f64) {
        let (i, f) = self.locate(altitude);
        let f = f.min(1.0);
        let (a, b) = (self.rows[i], self.rows[i + 1]);
        (a.1 + (b.1 - a.1) * f, a.4 + (b.4 - a.4) * f)
    }
}

impl Atmosphere {
    /// Ambient pressure (Pa) at `altitude` (zero at and above `top`).
    pub fn pressure(&self, altitude: f64) -> f64 {
        if altitude >= self.top {
            0.0
        } else if let Some(t) = &self.table {
            t.pressure(altitude)
        } else {
            self.p0 * math::exp(-altitude.max(0.0) / self.scale_height)
        }
    }

    /// Density (kg/m³) at `altitude` (zero at and above `top`).
    pub fn density(&self, altitude: f64) -> f64 {
        if altitude >= self.top {
            0.0
        } else if let Some(t) = &self.table {
            t.density(altitude)
        } else {
            self.rho0 * math::exp(-altitude.max(0.0) / self.scale_height)
        }
    }

    /// The model's air at `altitude`, ignoring `top`. `g0` (m/s², the
    /// body's surface gravity) sets the exponential model's temperature.
    pub fn profile(&self, altitude: f64, g0: f64) -> AirState {
        match &self.table {
            Some(t) => {
                let (temperature, molar_mass) = t.temperature_and_molar_mass(altitude);
                let pressure = t.pressure(altitude);
                let d = self.collision_diameter;
                let mean_free_path = BOLTZMANN * temperature / (std::f64::consts::SQRT_2 * math::PI * d * d * pressure);
                AirState { rho: t.density(altitude), pressure, temperature, molar_mass, mean_free_path }
            }
            None => {
                let e = math::exp(-altitude.max(0.0) / self.scale_height);
                let rho = self.rho0 * e;
                AirState {
                    rho,
                    pressure: self.p0 * e,
                    temperature: air::isothermal_temperature(self.scale_height, g0, self.molar_mass),
                    molar_mass: self.molar_mass,
                    mean_free_path: air::mean_free_path(self.mean_free_path, self.rho0, rho),
                }
            }
        }
    }

    /// The air flight sees at `altitude`: `None` at and above `top` or in
    /// vacuum.
    pub fn air(&self, altitude: f64, g0: f64) -> Option<AirState> {
        if altitude >= self.top {
            return None;
        }
        Some(self.profile(altitude, g0)).filter(|s| s.rho > 0.0)
    }
}

#[cfg(test)]
mod tests;
