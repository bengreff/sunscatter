//! Materials (D101, `docs/design/ship-model.md` §3.1): what parts are made
//! of. Generators ([`crate::catalog`]) derive a part's mass, skin and
//! strength from them; the structure and heat models read them.
//!
//! The table is data: `data/materials.ron` ([`MATERIALS_FILE`]), each
//! material with its sources and the fields still to verify (given from
//! memory, for the data workstream to check against the cited sources).
//! Finishes (paints, coatings, solar cells) change a surface's optics and
//! add an areal mass; they are listed beside the materials.
//!
//! Strength is a table against temperature, interpolated linearly and held
//! at its ends; above the destruction temperature a material carries
//! nothing. Everything here is a pure function of the table.

use crate::body::DataError;
use crate::craft::Skin;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The materials file inside `data/`.
pub const MATERIALS_FILE: &str = "materials.ron";

/// `data/materials.ron`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialsFile {
    pub materials: Vec<Material>,
    #[serde(default)]
    pub finishes: Vec<Finish>,
}

/// What kind of solid a material is: decides how it fails (a metal yields
/// before it breaks, a composite or glass breaks at its strength, an
/// ablator recedes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialKind {
    Metal,
    Composite,
    /// A sandwich core (honeycomb, foam): carries shear and spacing, not skin loads.
    Core,
    Ablator,
    Glass,
    Fabric,
}

/// Strength at one temperature.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Strength {
    /// K.
    pub k: f64,
    /// Yield (0.2 % offset; a brittle material's equals its ultimate), Pa.
    pub yield_pa: f64,
    /// Ultimate tensile strength, Pa.
    pub ultimate_pa: f64,
}

/// An ablator's data (thermal protection that chars and recedes).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ablation {
    /// kg/m³.
    pub virgin_density: f64,
    pub char_density: f64,
    /// Where pyrolysis starts (K).
    pub pyrolysis_k: f64,
    /// Effective heat of ablation Q* (J/kg of material lost).
    pub effective_heat: f64,
    /// The char's conductivity (W/(m·K)).
    pub char_conductivity: f64,
}

/// One material.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub id: String,
    pub name: String,
    pub kind: MaterialKind,
    /// kg/m³.
    pub density: f64,
    /// Young's modulus (Pa).
    pub modulus: f64,
    /// Against temperature, ascending.
    pub strength: Vec<Strength>,
    /// W/(m·K).
    pub conductivity: f64,
    /// J/(kg·K).
    pub specific_heat: f64,
    /// Hemispherical total emissivity and solar absorptivity of the bare
    /// surface (a finish overrides them).
    pub emissivity: f64,
    pub absorptivity: f64,
    /// Highest temperature at which it keeps its rated properties (K).
    pub service_k: f64,
    /// Where it is destroyed at once (melting, decomposition; an ablator's
    /// surface limit) (K).
    pub destruction_k: f64,
    #[serde(default)]
    pub ablation: Option<Ablation>,
    /// The S–N curve's exponent m (cycles ∝ S^-m; Miner's rule).
    #[serde(default)]
    pub fatigue_exponent: Option<f64>,
    /// USD per kg of mill product or prepreg (not the part's cost).
    pub cost_per_kg: f64,
    /// Year of first flight use.
    pub era: i32,
    pub sources: Vec<String>,
    /// Fields given from memory, to check against the sources.
    #[serde(default)]
    pub verify: Vec<String>,
}

/// A surface finish: paint, coating, solar cells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finish {
    pub id: String,
    pub name: String,
    pub emissivity: f64,
    pub absorptivity: f64,
    /// Added mass per area (kg/m²).
    pub areal_mass: f64,
    pub sources: Vec<String>,
    #[serde(default)]
    pub verify: Vec<String>,
}

/// The fields a `verify` list may name.
const MATERIAL_FIELDS: [&str; 15] = [
    "density",
    "modulus",
    "strength",
    "conductivity",
    "specific_heat",
    "emissivity",
    "absorptivity",
    "service_k",
    "destruction_k",
    "ablation",
    "fatigue_exponent",
    "cost_per_kg",
    "era",
    "kind",
    "name",
];
const FINISH_FIELDS: [&str; 3] = ["emissivity", "absorptivity", "areal_mass"];

impl Material {
    /// Yield and ultimate strength (Pa) at `t` (K): the table interpolated
    /// linearly, held at its ends; zero at and above the destruction
    /// temperature.
    pub fn strength_at(&self, t: f64) -> (f64, f64) {
        if t >= self.destruction_k {
            return (0.0, 0.0);
        }
        let s = &self.strength;
        let first = s[0];
        if t <= first.k {
            return (first.yield_pa, first.ultimate_pa);
        }
        for w in s.windows(2) {
            let (a, b) = (w[0], w[1]);
            if t <= b.k {
                let f = (t - a.k) / (b.k - a.k);
                return (
                    a.yield_pa + (b.yield_pa - a.yield_pa) * f,
                    a.ultimate_pa + (b.ultimate_pa - a.ultimate_pa) * f,
                );
            }
        }
        let last = s[s.len() - 1];
        (last.yield_pa, last.ultimate_pa)
    }

    /// A skin of `thickness` (m) of this material, with `finish` (optics
    /// and its areal mass) if any; its limit is the destruction temperature.
    pub fn skin(&self, thickness: f64, finish: Option<&Finish>) -> Skin {
        Skin {
            areal_mass: self.density * thickness + finish.map_or(0.0, |f| f.areal_mass),
            specific_heat: self.specific_heat,
            emissivity: finish.map_or(self.emissivity, |f| f.emissivity),
            conductivity: self.conductivity,
            thickness,
            max_k: Some(self.destruction_k),
            coupling: None,
        }
    }

    fn validate(&self) -> Result<(), String> {
        let what = &self.id;
        for (field, x) in [
            ("density", self.density),
            ("modulus", self.modulus),
            ("conductivity", self.conductivity),
            ("specific_heat", self.specific_heat),
            ("service_k", self.service_k),
            ("destruction_k", self.destruction_k),
        ] {
            if !(x.is_finite() && x > 0.0) {
                return Err(format!("material {what:?}: {field} must be positive, got {x}"));
            }
        }
        for (field, x) in [("emissivity", self.emissivity), ("absorptivity", self.absorptivity)] {
            if !(x > 0.0 && x <= 1.0) {
                return Err(format!("material {what:?}: {field} must be in (0, 1], got {x}"));
            }
        }
        if self.service_k > self.destruction_k {
            return Err(format!("material {what:?}: service_k above destruction_k"));
        }
        if self.strength.is_empty() {
            return Err(format!("material {what:?}: strength needs at least one temperature"));
        }
        let mut last = 0.0;
        for s in &self.strength {
            let ok = s.k > last && s.yield_pa >= 0.0 && s.ultimate_pa >= s.yield_pa && s.ultimate_pa.is_finite();
            if !ok {
                return Err(format!("material {what:?}: strength needs rising temperatures and 0 ≤ yield ≤ ultimate"));
            }
            last = s.k;
        }
        if !(self.cost_per_kg.is_finite() && self.cost_per_kg >= 0.0) {
            return Err(format!("material {what:?}: cost_per_kg must be non-negative"));
        }
        if let Some(m) = self.fatigue_exponent.filter(|m| !(m.is_finite() && *m > 0.0)) {
            return Err(format!("material {what:?}: fatigue_exponent must be positive, got {m}"));
        }
        if let Some(a) = &self.ablation {
            let ok = [a.virgin_density, a.char_density, a.pyrolysis_k, a.effective_heat, a.char_conductivity]
                .iter()
                .all(|x| x.is_finite() && *x > 0.0);
            if !ok || a.char_density > a.virgin_density {
                return Err(format!("material {what:?}: ablation needs positive values and char ≤ virgin density"));
            }
        }
        if self.sources.is_empty() {
            return Err(format!("material {what:?}: sources must not be empty"));
        }
        known_fields(what, &self.verify, &MATERIAL_FIELDS)
    }
}

fn known_fields(what: &str, verify: &[String], fields: &[&str]) -> Result<(), String> {
    match verify.iter().find(|v| !fields.contains(&v.as_str())) {
        Some(v) => Err(format!("{what:?}: verify names an unknown field {v:?}")),
        None => Ok(()),
    }
}

impl Finish {
    fn validate(&self) -> Result<(), String> {
        let what = &self.id;
        for (field, x) in [("emissivity", self.emissivity), ("absorptivity", self.absorptivity)] {
            if !(x > 0.0 && x <= 1.0) {
                return Err(format!("finish {what:?}: {field} must be in (0, 1], got {x}"));
            }
        }
        if !(self.areal_mass.is_finite() && self.areal_mass >= 0.0) {
            return Err(format!("finish {what:?}: areal_mass must be non-negative"));
        }
        if self.sources.is_empty() {
            return Err(format!("finish {what:?}: sources must not be empty"));
        }
        known_fields(what, &self.verify, &FINISH_FIELDS)
    }
}

/// The materials and finishes by id.
#[derive(Clone, Debug, PartialEq)]
pub struct Materials {
    materials: BTreeMap<String, Material>,
    finishes: BTreeMap<String, Finish>,
}

impl Materials {
    /// Parses and validates the materials file's text.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: MaterialsFile = ron::from_str(text).map_err(|e| e.to_string())?;
        let mut materials = BTreeMap::new();
        for m in file.materials {
            m.validate()?;
            let id = m.id.clone();
            if materials.insert(id.clone(), m).is_some() {
                return Err(format!("material {id:?} is listed twice"));
            }
        }
        let mut finishes = BTreeMap::new();
        for f in file.finishes {
            f.validate()?;
            let id = f.id.clone();
            if finishes.insert(id.clone(), f).is_some() {
                return Err(format!("finish {id:?} is listed twice"));
            }
        }
        Ok(Materials { materials, finishes })
    }

    /// Loads `path`.
    pub fn load(path: &Path) -> Result<Self, DataError> {
        let err = |message: String| DataError { path: path.to_path_buf(), message };
        let text = std::fs::read_to_string(path).map_err(|e| err(e.to_string()))?;
        Self::parse(&text).map_err(err)
    }

    /// The material `id`, or why there is none.
    pub fn get(&self, id: &str) -> Result<&Material, String> {
        self.materials.get(id).ok_or_else(|| format!("no material {id:?} in {MATERIALS_FILE}"))
    }

    /// The finish `id`, or why there is none.
    pub fn finish(&self, id: &str) -> Result<&Finish, String> {
        self.finishes.get(id).ok_or_else(|| format!("no finish {id:?} in {MATERIALS_FILE}"))
    }

    /// Every material, by id.
    pub fn iter(&self) -> impl Iterator<Item = &Material> {
        self.materials.values()
    }
}

/// `data/materials.ron` of this source tree.
pub fn default_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data").join(MATERIALS_FILE)
}

/// The materials of [`default_path`], loaded once per process. Panics if
/// the file is missing or invalid.
pub fn materials() -> &'static Materials {
    static TABLE: OnceLock<Materials> = OnceLock::new();
    TABLE.get_or_init(|| Materials::load(&default_path()).unwrap_or_else(|e| panic!("{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_loads_with_the_falcon_9_and_dragon_set() {
        let m = materials();
        for id in [
            "al-li-2195",
            "al-li-2198",
            "al-2219",
            "al-7075",
            "al-6061",
            "ti-6al-4v",
            "inconel-718",
            "copper-narloy-z",
            "niobium-c103",
            "stainless-301",
            "cfrp",
            "al-honeycomb",
            "kevlar-49",
            "pica-x",
            "silicone-ablator",
            "fused-silica",
        ] {
            let mat = m.get(id).unwrap();
            assert!(mat.service_k <= mat.destruction_k, "{id}");
        }
        assert!(m.get("pica-x").unwrap().ablation.is_some());
        for id in ["white-paint", "black-paint", "silicide-coating", "solar-cells"] {
            m.finish(id).unwrap();
        }
        assert!(m.get("unobtainium").is_err());
    }

    /// Strength between, at and beyond the table's points; nothing at the
    /// destruction temperature.
    #[test]
    fn strength_interpolates_and_vanishes_at_destruction() {
        let mat = Material {
            id: "t".into(),
            name: "t".into(),
            kind: MaterialKind::Metal,
            density: 1.0,
            modulus: 1.0,
            strength: vec![
                Strength { k: 100.0, yield_pa: 500.0, ultimate_pa: 600.0 },
                Strength { k: 300.0, yield_pa: 300.0, ultimate_pa: 400.0 },
            ],
            conductivity: 1.0,
            specific_heat: 1.0,
            emissivity: 0.5,
            absorptivity: 0.5,
            service_k: 400.0,
            destruction_k: 800.0,
            ablation: None,
            fatigue_exponent: None,
            cost_per_kg: 1.0,
            era: 2000,
            sources: vec!["test".into()],
            verify: vec![],
        };
        // (K, yield, ultimate)
        let cases = [
            (50.0, 500.0, 600.0),
            (100.0, 500.0, 600.0),
            (200.0, 400.0, 500.0),
            (300.0, 300.0, 400.0),
            (700.0, 300.0, 400.0),
            (800.0, 0.0, 0.0),
        ];
        for (t, y, u) in cases {
            assert_eq!(mat.strength_at(t), (y, u), "{t} K");
        }
        let skin = mat.skin(0.002, None);
        assert_eq!((skin.areal_mass, skin.thickness, skin.max_k), (0.002, 0.002, Some(800.0)));
    }

    #[test]
    fn invalid_tables_are_refused() {
        let good = std::fs::read_to_string(default_path()).unwrap();
        assert!(Materials::parse(&good).is_ok());
        let first = good.find("density:").unwrap();
        let bad_density = format!("{}density: -1.0,{}", &good[..first], good[first..].split_once(',').unwrap().1);
        assert!(Materials::parse(&bad_density).unwrap_err().contains("density"));
        let verify = good.replacen("verify: [", "verify: [\"colour\", ", 1);
        assert!(Materials::parse(&verify).unwrap_err().contains("colour"));
    }
}
