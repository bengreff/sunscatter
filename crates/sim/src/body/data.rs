//! Body definitions as data: `data/bodies/<body>/body.ron`.
//!
//! The file is one RON struct with a `physical: (...)` section, read here.
//! Other sections (the game's `visual: (...)`) are ignored by the simulation.
//! Angles are stored in degrees and rates in IAU units for readability; they
//! are converted on load with the same arithmetic as the former Rust
//! constants (`x * DEG`, `x * DEG / CENTURY`, `x * DEG / 86_400`), so the
//! resulting f64 values are bit-identical (tested below).

use super::{Atmosphere, BodyPhysical, Rotation};
use crate::math;
use crate::world::AnchorZone;
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

const DEG: f64 = math::PI / 180.0;
/// Seconds per Julian century (IAU `T` unit).
const CENTURY: f64 = 36_525.0 * 86_400.0;

/// File name of a body definition inside its directory.
pub const BODY_FILE: &str = "body.ron";

/// Everything the simulation reads from a body file.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyDef {
    pub physical: BodyPhysical,
    /// Anchor-zone precision policy (none: never a preferred anchor).
    pub anchor_zone: Option<AnchorZone>,
    /// The body's 1PN field acts on vessels.
    pub relativistic: bool,
    /// Resolved path of the heightmap (16-bit PNG), if the body has one.
    pub heightmap: Option<PathBuf>,
}

/// An error reading body data.
#[derive(Clone, Debug, PartialEq)]
pub struct DataError {
    pub path: PathBuf,
    pub message: String,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for DataError {}

/// The file's top level. Unknown sections (e.g. `visual`) are ignored.
#[derive(Deserialize)]
struct BodyFile {
    physical: PhysicalFile,
}

#[derive(Deserialize)]
struct PhysicalFile {
    name: String,
    radius_eq: f64,
    radius_polar: f64,
    #[serde(default)]
    j2: f64,
    rotation: RotationFile,
    #[serde(default)]
    atmosphere: Option<Atmosphere>,
    #[serde(default)]
    anchor_zone: Option<AnchorZone>,
    #[serde(default)]
    relativistic: bool,
    solid: bool,
    /// Path relative to the body's directory.
    #[serde(default)]
    heightmap: Option<String>,
    #[serde(default)]
    sea_level: Option<f64>,
}

#[derive(Deserialize)]
struct RotationFile {
    ra_deg: f64,
    dec_deg: f64,
    #[serde(default)]
    ra_rate_deg_per_century: f64,
    #[serde(default)]
    dec_rate_deg_per_century: f64,
    w0_deg: f64,
    w_rate_deg_per_day: f64,
}

impl RotationFile {
    fn to_rotation(&self) -> Rotation {
        Rotation {
            ra: self.ra_deg * DEG,
            dec: self.dec_deg * DEG,
            ra_rate: self.ra_rate_deg_per_century * DEG / CENTURY,
            dec_rate: self.dec_rate_deg_per_century * DEG / CENTURY,
            w0: self.w0_deg * DEG,
            w_rate: self.w_rate_deg_per_day * DEG / 86_400.0,
        }
    }
}

/// Parses a body file's text. `dir` resolves the heightmap path and labels errors.
pub fn parse_body(text: &str, dir: &Path) -> Result<BodyDef, DataError> {
    let err = |message: String| DataError { path: dir.join(BODY_FILE), message };
    let file: BodyFile = ron::from_str(text).map_err(|e| err(e.to_string()))?;
    let p = file.physical;
    if !(p.radius_eq > 0.0 && p.radius_polar > 0.0) {
        return Err(err("radii must be positive".into()));
    }
    Ok(BodyDef {
        physical: BodyPhysical {
            name: p.name,
            radius_eq: p.radius_eq,
            radius_polar: p.radius_polar,
            j2: p.j2,
            rotation: p.rotation.to_rotation(),
            atmosphere: p.atmosphere,
            solid: p.solid,
            sea_level: p.sea_level,
            terrain: None,
        },
        anchor_zone: p.anchor_zone,
        relativistic: p.relativistic,
        heightmap: p.heightmap.map(|h| dir.join(h)),
    })
}

impl BodyDef {
    /// Loads the heightmap into `physical.terrain` (shared per process, see
    /// [`crate::terrain::load_shared`]). A missing file is not an error: the
    /// body keeps its smooth ellipsoid and a warning is printed. A file that
    /// exists but cannot be decoded is an error.
    pub fn load_terrain(&mut self) -> Result<(), DataError> {
        let Some(path) = &self.heightmap else { return Ok(()) };
        if !path.exists() {
            eprintln!("warning: {} not found; {} uses its smooth ellipsoid", path.display(), self.physical.name);
            return Ok(());
        }
        let map = crate::terrain::load_shared(path).map_err(|message| DataError { path: path.clone(), message })?;
        self.physical.terrain = Some(map);
        Ok(())
    }
}

/// Loads `<dir>/body.ron` (without terrain; see [`BodyDef::load_terrain`]).
pub fn load_body(dir: &Path) -> Result<BodyDef, DataError> {
    let path = dir.join(BODY_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| DataError { path, message: e.to_string() })?;
    parse_body(&text, dir)
}

/// `data/bodies` of this source tree (development builds and tests).
pub fn default_bodies_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bodies")
}

/// Directory name of a body: its ephemeris name in lower case.
pub fn body_dir_name(name: &str) -> String {
    name.to_lowercase()
}

/// Loads a body from [`default_bodies_dir`]; panics if it is missing.
pub(crate) fn load_default(dir_name: &str) -> BodyDef {
    load_body(&default_bodies_dir().join(dir_name)).unwrap_or_else(|e| panic!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Rust constants these files replaced, kept to prove bit-identity.
    fn legacy(name: &str) -> BodyPhysical {
        let (radius_eq, radius_polar, j2, rotation, atmosphere, solid, sea_level) = match name {
            "earth" => (
                6_378_137.0,
                6_356_752.314_245,
                1.082_626_68e-3,
                Rotation {
                    ra: 0.0,
                    dec: 90.0 * DEG,
                    ra_rate: -0.641 * DEG / CENTURY,
                    dec_rate: -0.557 * DEG / CENTURY,
                    w0: 190.147 * DEG,
                    w_rate: 360.985_623_5 * DEG / 86_400.0,
                },
                Some(Atmosphere { rho0: 1.225, scale_height: 7_200.0, top: 150_000.0 }),
                true,
                Some(0.0),
            ),
            "moon" => (
                1_737_400.0,
                1_737_400.0,
                2.033e-4,
                Rotation {
                    ra: 269.9949 * DEG,
                    dec: 66.5392 * DEG,
                    ra_rate: 0.0031 * DEG / CENTURY,
                    dec_rate: 0.0130 * DEG / CENTURY,
                    w0: 38.3213 * DEG,
                    w_rate: 13.176_358_15 * DEG / 86_400.0,
                },
                None,
                true,
                None,
            ),
            _ => (
                695_700_000.0,
                695_700_000.0,
                0.0,
                Rotation {
                    ra: 286.13 * DEG,
                    dec: 63.87 * DEG,
                    ra_rate: 0.0,
                    dec_rate: 0.0,
                    w0: 84.176 * DEG,
                    w_rate: 14.1844 * DEG / 86_400.0,
                },
                None,
                false,
                None,
            ),
        };
        let name = match name {
            "earth" => "Earth",
            "moon" => "Moon",
            _ => "Sun",
        };
        BodyPhysical {
            name: name.into(),
            radius_eq,
            radius_polar,
            j2,
            rotation,
            atmosphere,
            solid,
            sea_level,
            terrain: None,
        }
    }

    fn bits(p: &BodyPhysical) -> Vec<u64> {
        let r = &p.rotation;
        let mut v = vec![p.radius_eq, p.radius_polar, p.j2, r.ra, r.dec, r.ra_rate, r.dec_rate, r.w0, r.w_rate];
        if let Some(a) = p.atmosphere {
            v.extend([a.rho0, a.scale_height, a.top]);
        }
        v.into_iter().map(f64::to_bits).collect()
    }

    #[test]
    fn shipped_bodies_are_bit_identical_to_the_former_constants() {
        for name in ["earth", "moon", "sun"] {
            let def = load_default(name);
            assert_eq!(bits(&def.physical), bits(&legacy(name)), "{name}");
            assert_eq!(def.physical, legacy(name), "{name}");
        }
        let earth = load_default("earth");
        assert_eq!(earth.anchor_zone, Some(AnchorZone { enter: 1.4e9, exit: 1.6e9 }));
        assert_eq!(load_default("moon").anchor_zone, Some(AnchorZone { enter: 6.0e7, exit: 7.0e7 }));
        assert!(load_default("sun").relativistic && !earth.relativistic);
        assert!(earth.heightmap.unwrap().ends_with("earth/height.png"));
    }

    #[test]
    fn unknown_sections_are_ignored_and_optional_fields_default() {
        let text = r#"(
            physical: (
                name: "Rock", radius_eq: 1000.0, radius_polar: 900.0,
                rotation: (ra_deg: 10.0, dec_deg: 20.0, w0_deg: 0.0, w_rate_deg_per_day: 1.0),
                solid: true,
                future_field: 3,
            ),
            visual: (colour: "grey", detail: [1, 2, 3]),
        )"#;
        let def = parse_body(text, Path::new("rock")).unwrap();
        assert_eq!(def.physical.name, "Rock");
        assert_eq!(def.physical.j2, 0.0);
        assert!(def.physical.atmosphere.is_none() && def.heightmap.is_none() && def.anchor_zone.is_none());
        assert_eq!(def.physical.rotation.dec, 20.0 * DEG);
    }

    #[test]
    fn bad_files_report_their_path() {
        let e = parse_body("(physical: (name: 3))", Path::new("x")).unwrap_err();
        let expected = Path::new("x").join(BODY_FILE).display().to_string();
        assert!(e.to_string().starts_with(&expected), "{e}");
        assert!(load_body(Path::new("/nonexistent/body")).is_err());
    }
}
