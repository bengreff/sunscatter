//! Fluids (D104, `docs/technical/ship-model.md` §3.2): propellants,
//! pressurants and the gases of a cabin. Tanks, bottles, engines and the
//! plumbing read them.
//!
//! The table is data: `data/fluids.ron` ([`FLUIDS_FILE`]), each fluid with
//! its sources and the fields still to verify. A **liquid** has its
//! saturated density and vapour pressure against temperature, its specific
//! and latent heat and its freezing, boiling and critical points; it may
//! name the gas its vapour is. A **gas** is ideal: molar mass, γ and cₚ.
//!
//! Density is interpolated linearly in temperature; vapour pressure as ln p
//! linear in 1/T (Clausius–Clapeyron between the table's points). Both are
//! held at the table's ends. All logarithms and exponentials go through
//! [`crate::math`] (rule 2).

use crate::body::DataError;
use crate::math;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The fluids file inside `data/`.
pub const FLUIDS_FILE: &str = "fluids.ron";
/// The molar gas constant (J/(mol·K), CODATA 2018, exact).
pub const R_GAS: f64 = 8.314_462_618;

/// `data/fluids.ron`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FluidsFile {
    pub liquids: Vec<Liquid>,
    pub gases: Vec<Gas>,
}

/// A liquid (propellant, coolant, water).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Liquid {
    pub id: String,
    pub name: String,
    /// kg/mol.
    pub molar_mass: f64,
    /// The gas its vapour is (an id in `gases`), if listed.
    #[serde(default)]
    pub vapour: Option<String>,
    /// Saturated liquid density against temperature: (K, kg/m³), ascending.
    pub density: Vec<(f64, f64)>,
    /// Saturation (vapour) pressure against temperature: (K, Pa), ascending.
    pub vapour_pressure: Vec<(f64, f64)>,
    /// J/(kg·K).
    pub specific_heat: f64,
    /// Heat of vaporisation at the normal boiling point (J/kg).
    pub latent_heat: f64,
    /// K.
    pub freezing_k: f64,
    /// Normal boiling point (at 101,325 Pa), K.
    pub boiling_k: f64,
    pub critical_k: f64,
    pub sources: Vec<String>,
    #[serde(default)]
    pub verify: Vec<String>,
}

/// An ideal gas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gas {
    pub id: String,
    pub name: String,
    /// kg/mol.
    pub molar_mass: f64,
    /// Ratio of specific heats cₚ/cᵥ (near room temperature).
    pub gamma: f64,
    /// J/(kg·K).
    pub cp: f64,
    pub sources: Vec<String>,
    #[serde(default)]
    pub verify: Vec<String>,
}

const LIQUID_FIELDS: [&str; 9] = [
    "molar_mass",
    "density",
    "vapour_pressure",
    "specific_heat",
    "latent_heat",
    "freezing_k",
    "boiling_k",
    "critical_k",
    "vapour",
];
const GAS_FIELDS: [&str; 3] = ["molar_mass", "gamma", "cp"];

/// Linear interpolation in a rising table of (x, y), held at its ends.
fn interpolate(table: &[(f64, f64)], x: f64, y_of: impl Fn(f64) -> f64, inv: impl Fn(f64) -> f64) -> f64 {
    let (first, last) = (table[0], table[table.len() - 1]);
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    let i = table.partition_point(|p| p.0 <= x).max(1) - 1;
    let (a, b) = (table[i], table[i + 1]);
    let f = (x - a.0) / (b.0 - a.0);
    inv(y_of(a.1) + (y_of(b.1) - y_of(a.1)) * f)
}

impl Liquid {
    /// Saturated liquid density (kg/m³) at `t` (K).
    pub fn density(&self, t: f64) -> f64 {
        interpolate(&self.density, t, |y| y, |y| y)
    }

    /// Vapour pressure (Pa) at `t` (K): ln p linear in 1/T between points.
    pub fn vapour_pressure(&self, t: f64) -> f64 {
        let inv: Vec<(f64, f64)> = self.vapour_pressure.iter().rev().map(|&(k, p)| (1.0 / k, p)).collect();
        interpolate(&inv, 1.0 / t, math::ln, math::exp)
    }

    /// The temperature (K) at which the vapour pressure is `p` (Pa): the
    /// boiling point at that pressure (held at the table's ends).
    pub fn saturation_temperature(&self, p: f64) -> f64 {
        let by_p: Vec<(f64, f64)> = self.vapour_pressure.iter().map(|&(k, p)| (math::ln(p), 1.0 / k)).collect();
        1.0 / interpolate(&by_p, math::ln(p.max(f64::MIN_POSITIVE)), |y| y, |y| y)
    }

    fn validate(&self) -> Result<(), String> {
        let what = &self.id;
        for (field, x) in [
            ("molar_mass", self.molar_mass),
            ("specific_heat", self.specific_heat),
            ("latent_heat", self.latent_heat),
            ("freezing_k", self.freezing_k),
            ("boiling_k", self.boiling_k),
            ("critical_k", self.critical_k),
        ] {
            if !(x.is_finite() && x > 0.0) {
                return Err(format!("liquid {what:?}: {field} must be positive, got {x}"));
            }
        }
        if !(self.freezing_k < self.boiling_k && self.boiling_k < self.critical_k) {
            return Err(format!("liquid {what:?}: needs freezing_k < boiling_k < critical_k"));
        }
        for (field, table) in [("density", &self.density), ("vapour_pressure", &self.vapour_pressure)] {
            let rising = table.windows(2).all(|w| w[1].0 > w[0].0);
            let positive = table.iter().all(|&(k, y)| k > 0.0 && y > 0.0 && y.is_finite());
            if table.len() < 2 || !rising || !positive {
                return Err(format!("liquid {what:?}: {field} needs two or more rising, positive points"));
            }
        }
        if self.vapour_pressure.windows(2).any(|w| w[1].1 <= w[0].1) {
            return Err(format!("liquid {what:?}: vapour_pressure must rise with temperature"));
        }
        if self.sources.is_empty() {
            return Err(format!("liquid {what:?}: sources must not be empty"));
        }
        match self.verify.iter().find(|v| !LIQUID_FIELDS.contains(&v.as_str())) {
            Some(v) => Err(format!("liquid {what:?}: verify names an unknown field {v:?}")),
            None => Ok(()),
        }
    }
}

impl Gas {
    /// The specific gas constant R/M (J/(kg·K)).
    pub fn r(&self) -> f64 {
        R_GAS / self.molar_mass
    }

    /// cᵥ (J/(kg·K)).
    pub fn cv(&self) -> f64 {
        self.cp / self.gamma
    }

    /// Density (kg/m³) at pressure `p` (Pa) and temperature `t` (K).
    pub fn density(&self, p: f64, t: f64) -> f64 {
        p / (self.r() * t)
    }

    fn validate(&self) -> Result<(), String> {
        let what = &self.id;
        if !(self.molar_mass > 0.0 && self.cp > 0.0 && self.gamma > 1.0 && self.gamma < 1.7) {
            return Err(format!("gas {what:?}: needs molar_mass > 0, cp > 0 and γ in (1, 1.7)"));
        }
        if self.sources.is_empty() {
            return Err(format!("gas {what:?}: sources must not be empty"));
        }
        match self.verify.iter().find(|v| !GAS_FIELDS.contains(&v.as_str())) {
            Some(v) => Err(format!("gas {what:?}: verify names an unknown field {v:?}")),
            None => Ok(()),
        }
    }
}

/// Moles of an ideal gas at `p` (Pa) in `v` (m³) at `t` (K).
pub fn moles(p: f64, v: f64, t: f64) -> f64 {
    p * v / (R_GAS * t)
}

/// Pressure (Pa) of `n` moles of ideal gas in `v` (m³) at `t` (K).
pub fn pressure(n: f64, v: f64, t: f64) -> f64 {
    n * R_GAS * t / v
}

/// The liquids and gases by id.
#[derive(Clone, Debug, PartialEq)]
pub struct Fluids {
    liquids: BTreeMap<String, Liquid>,
    gases: BTreeMap<String, Gas>,
}

impl Fluids {
    /// Parses and validates the fluids file's text.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: FluidsFile = ron::from_str(text).map_err(|e| e.to_string())?;
        let mut gases = BTreeMap::new();
        for g in file.gases {
            g.validate()?;
            let id = g.id.clone();
            if gases.insert(id.clone(), g).is_some() {
                return Err(format!("gas {id:?} is listed twice"));
            }
        }
        let mut liquids = BTreeMap::new();
        for l in file.liquids {
            l.validate()?;
            if let Some(v) = l.vapour.as_ref().filter(|v| !gases.contains_key(*v)) {
                return Err(format!("liquid {:?}: its vapour {v:?} is not a listed gas", l.id));
            }
            let id = l.id.clone();
            if gases.contains_key(&id) || liquids.insert(id.clone(), l).is_some() {
                return Err(format!("fluid {id:?} is listed twice"));
            }
        }
        Ok(Fluids { liquids, gases })
    }

    /// Loads `path`.
    pub fn load(path: &Path) -> Result<Self, DataError> {
        let err = |message: String| DataError { path: path.to_path_buf(), message };
        let text = std::fs::read_to_string(path).map_err(|e| err(e.to_string()))?;
        Self::parse(&text).map_err(err)
    }

    /// The liquid `id`, or why there is none.
    pub fn liquid(&self, id: &str) -> Result<&Liquid, String> {
        self.liquids.get(id).ok_or_else(|| format!("no liquid {id:?} in {FLUIDS_FILE}"))
    }

    /// The gas `id`, or why there is none.
    pub fn gas(&self, id: &str) -> Result<&Gas, String> {
        self.gases.get(id).ok_or_else(|| format!("no gas {id:?} in {FLUIDS_FILE}"))
    }

    /// Whether `id` names a liquid or a gas.
    pub fn knows(&self, id: &str) -> bool {
        self.liquids.contains_key(id) || self.gases.contains_key(id)
    }
}

/// `data/fluids.ron` of this source tree.
pub fn default_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").join(FLUIDS_FILE)
}

/// The fluids of [`default_path`], loaded once per process. Panics if the
/// file is missing or invalid.
pub fn fluids() -> &'static Fluids {
    static TABLE: OnceLock<Fluids> = OnceLock::new();
    TABLE.get_or_init(|| Fluids::load(&default_path()).unwrap_or_else(|e| panic!("{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_the_falcon_9_and_dragon_fluids() {
        let f = fluids();
        for id in ["LOX", "RP-1", "MMH", "NTO", "water"] {
            f.liquid(id).unwrap();
        }
        for id in ["helium", "nitrogen", "oxygen", "CO2", "water-vapour"] {
            f.gas(id).unwrap();
        }
        assert_eq!(f.liquid("LOX").unwrap().vapour.as_deref(), Some("oxygen"));
        assert!(f.liquid("helium").is_err() && f.gas("LOX").is_err() && f.knows("helium"));
    }

    /// Each liquid boils at its normal boiling point at one atmosphere
    /// (within the table's interpolation), and the saturation temperature
    /// inverts the vapour pressure.
    #[test]
    fn vapour_pressure_and_boiling_points() {
        let f = fluids();
        for id in ["LOX", "MMH", "NTO", "water"] {
            let l = f.liquid(id).unwrap();
            let t = l.saturation_temperature(101_325.0);
            assert!((t - l.boiling_k).abs() < 0.02 * l.boiling_k, "{id}: boils at {t} K, not {}", l.boiling_k);
            for k in [l.freezing_k + 5.0, l.boiling_k, l.boiling_k + 20.0] {
                let back = l.saturation_temperature(l.vapour_pressure(k));
                let (lo, hi) = (l.vapour_pressure[0].0, l.vapour_pressure[l.vapour_pressure.len() - 1].0);
                if k > lo && k < hi {
                    assert!((back - k).abs() < 1e-9 * k, "{id} at {k} K: {back}");
                }
            }
        }
        // NIST: water's saturation pressure at 20 °C is 2.339 kPa (a table point).
        assert!((f.liquid("water").unwrap().vapour_pressure(293.15) - 2339.0).abs() < 5.0);
    }

    /// Densified LOX (Falcon 9 loads it near 66 K) is denser than at its
    /// boiling point; densities interpolate between points and hold at the
    /// ends.
    #[test]
    fn densities() {
        let lox = fluids().liquid("LOX").unwrap();
        assert!((lox.density(90.0) - 1142.0).abs() < 1.0);
        let dense = lox.density(66.5);
        assert!(dense > 1240.0 && dense < 1260.0, "{dense}");
        assert_eq!(lox.density(1.0), lox.density[0].1);
        let rp1 = fluids().liquid("RP-1").unwrap();
        assert!(rp1.density(266.0) > rp1.density(293.0));
    }

    /// Ideal gas: helium at 300 K and one atmosphere, and the mole round trip.
    #[test]
    fn ideal_gases() {
        let he = fluids().gas("helium").unwrap();
        assert!((he.density(101_325.0, 300.0) - 0.1626).abs() < 1e-3, "{}", he.density(101_325.0, 300.0));
        let n = moles(30e6, 0.1, 80.0);
        assert!((pressure(n, 0.1, 80.0) / 30e6 - 1.0).abs() < 1e-15);
        assert!((he.cv() - he.cp / he.gamma).abs() < 1e-12);
    }

    #[test]
    fn invalid_tables_are_refused() {
        let good = std::fs::read_to_string(default_path()).unwrap();
        assert!(Fluids::parse(&good).is_ok());
        let vapour = good.replacen("vapour: Some(\"oxygen\")", "vapour: Some(\"ozone\")", 1);
        assert!(Fluids::parse(&vapour).unwrap_err().contains("ozone"));
        let gamma = good.replacen("gamma: 1.667", "gamma: 0.9", 1);
        assert!(Fluids::parse(&gamma).unwrap_err().contains("γ"));
    }
}
