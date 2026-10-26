//! Body definitions as data: `data/bodies/<body>/body.ron`.
//!
//! The file is one RON struct with a `physical: (...)` section, read here.
//! Other sections (the game's `visual: (...)`) are ignored by the simulation.
//! Angles are stored in degrees and rates in IAU units for readability; they
//! are converted on load with the same arithmetic as the former Rust
//! constants (`x * DEG`, `x * DEG / CENTURY`, `x * DEG / 86_400`), so the
//! resulting f64 values are bit-identical (tested below).

use super::{Atmosphere, BodyPhysical, Libration, Rotation};
use crate::math;
use crate::terrain::{Detail, DetailParams};
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
    /// Resolved path of the roughness map (8-bit PNG, D059), if any.
    pub roughness: Option<PathBuf>,
    /// Procedural detail parameters (D059); detail needs the roughness map too.
    pub terrain_detail: Option<DetailParams>,
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
    /// Path relative to the body's directory.
    #[serde(default)]
    roughness: Option<String>,
    #[serde(default)]
    terrain_detail: Option<DetailParams>,
    /// Rails warp is disabled below this altitude (m, D062).
    #[serde(default)]
    rails_floor: f64,
    /// Radiated power of a star (W).
    #[serde(default)]
    luminosity: f64,
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
    /// Periodic terms, each (phase deg, rate deg/day, ra amplitude deg, dec
    /// amplitude deg, W amplitude deg) on `θ = phase + rate · d` (d days
    /// from J2000): ra += a sin θ, dec += b cos θ, W += c sin θ.
    #[serde(default)]
    librations: Vec<(f64, f64, f64, f64, f64)>,
}

impl RotationFile {
    fn to_rotation(&self) -> Result<Rotation, String> {
        if self.librations.len() > super::MAX_LIBRATIONS {
            return Err(format!("at most {} periodic rotation terms", super::MAX_LIBRATIONS));
        }
        let mut librations = Rotation::NO_LIBRATIONS;
        for (l, &(phase, rate, ra, dec, w)) in librations.iter_mut().zip(&self.librations) {
            *l =
                Libration { phase: phase * DEG, rate: rate * DEG / 86_400.0, ra: ra * DEG, dec: dec * DEG, w: w * DEG };
        }
        Ok(Rotation {
            ra: self.ra_deg * DEG,
            dec: self.dec_deg * DEG,
            ra_rate: self.ra_rate_deg_per_century * DEG / CENTURY,
            dec_rate: self.dec_rate_deg_per_century * DEG / CENTURY,
            w0: self.w0_deg * DEG,
            w_rate: self.w_rate_deg_per_day * DEG / 86_400.0,
            librations,
            n_librations: self.librations.len(),
        })
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
    if let Some(a) = p.atmosphere.as_ref().filter(|a| a.table.is_none()) {
        if !(a.rho0 > 0.0 && a.scale_height > 0.0) {
            return Err(err("an atmosphere without a table needs rho0 and scale_height".into()));
        }
    }
    Ok(BodyDef {
        physical: BodyPhysical {
            name: p.name,
            radius_eq: p.radius_eq,
            radius_polar: p.radius_polar,
            j2: p.j2,
            rotation: p.rotation.to_rotation().map_err(err)?,
            atmosphere: p.atmosphere,
            solid: p.solid,
            sea_level: p.sea_level,
            terrain: None,
            detail: None,
            rails_floor: p.rails_floor,
            luminosity: p.luminosity,
        },
        anchor_zone: p.anchor_zone,
        relativistic: p.relativistic,
        heightmap: p.heightmap.map(|h| dir.join(h)),
        roughness: p.roughness.map(|r| dir.join(r)),
        terrain_detail: p.terrain_detail,
    })
}

impl BodyDef {
    /// Loads the heightmap into `physical.terrain` and, with detail
    /// parameters, the roughness map into `physical.detail` (shared per
    /// process, see [`crate::terrain::load_shared`]). A missing file is not
    /// an error: the body keeps its smooth ellipsoid (or has no detail) and a
    /// warning is printed. A file that exists but cannot be decoded is an error.
    pub fn load_terrain(&mut self) -> Result<(), DataError> {
        if let Some(path) = &self.heightmap {
            if path.exists() {
                let map =
                    crate::terrain::load_shared(path).map_err(|message| DataError { path: path.clone(), message })?;
                self.physical.terrain = Some(map);
            } else {
                eprintln!("warning: {} not found; {} uses its smooth ellipsoid", path.display(), self.physical.name);
            }
        }
        if let (Some(params), Some(path)) = (self.terrain_detail, &self.roughness) {
            if path.exists() {
                let roughness = crate::terrain::load_shared_roughness(path)
                    .map_err(|message| DataError { path: path.clone(), message })?;
                self.physical.detail = Some(Detail { params, roughness });
            } else {
                eprintln!("warning: {} not found; {} has no terrain detail", path.display(), self.physical.name);
            }
        }
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

/// Directory name of a body: its ephemeris name in lower case, letters and
/// digits only ("Hi'iaka" lives in `hiiaka`).
pub fn body_dir_name(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
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
                    librations: Rotation::NO_LIBRATIONS,
                    n_librations: 0,
                },
                None, // replaced by the US 1976 table (tested in `body::atmosphere`)
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
                    librations: Rotation::NO_LIBRATIONS,
                    n_librations: 0,
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
                    librations: Rotation::NO_LIBRATIONS,
                    n_librations: 0,
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
            detail: None,
            rails_floor: 0.0,
            luminosity: if name == "Sun" { 3.828e26 } else { 0.0 },
        }
    }

    fn bits(p: &BodyPhysical) -> Vec<u64> {
        let r = &p.rotation;
        let v = vec![p.radius_eq, p.radius_polar, p.j2, r.ra, r.dec, r.ra_rate, r.dec_rate, r.w0, r.w_rate];
        v.into_iter().map(f64::to_bits).collect()
    }

    #[test]
    fn shipped_bodies_are_bit_identical_to_the_former_constants() {
        for name in ["earth", "moon", "sun"] {
            let def = load_default(name);
            assert_eq!(bits(&def.physical), bits(&legacy(name)), "{name}");
            let floor = def.physical.rails_floor;
            let atmosphere = def.physical.atmosphere.clone();
            // The Moon's file now also carries the IAU periodic terms (below).
            let r = def.physical.rotation;
            let rotation = Rotation { librations: r.librations, n_librations: r.n_librations, ..legacy(name).rotation };
            let expected = BodyPhysical { rails_floor: floor, atmosphere, rotation, ..legacy(name) };
            assert_eq!(def.physical, expected, "{name}");
        }
        let earth = load_default("earth");
        assert_eq!(earth.anchor_zone, Some(AnchorZone { enter: 1.4e9, exit: 1.6e9 }));
        assert_eq!(load_default("moon").anchor_zone, Some(AnchorZone { enter: 6.0e7, exit: 7.0e7 }));
        assert!(load_default("sun").relativistic && !earth.relativistic);
        assert!(earth.heightmap.unwrap().ends_with("earth/height.png"));
        assert!(earth.roughness.unwrap().ends_with("earth/roughness.png"));
        assert_eq!(earth.terrain_detail.map(|d| d.octaves), Some(12));
        assert!(earth.physical.detail.is_none(), "loaded by load_terrain only");
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
        assert!(def.roughness.is_none() && def.terrain_detail.is_none());
        assert_eq!(def.physical.rotation.dec, 20.0 * DEG);
    }

    /// Venus, Mars and Titan carry their sourced surface values; density falls
    /// monotonically to where it is negligible at the table's end (no wall
    /// where drag stops: under 1e-13 kg/m³, q under 1e-5 Pa at 10 km/s);
    /// `top` (live flight) sits where it is ~1e-9 kg/m³ like Earth's 150 km,
    /// with the rails floor there (D062).
    #[test]
    fn the_new_atmospheres_load_with_their_surface_values_and_fade_out() {
        // (body, surface pressure Pa, surface density kg/m³)
        for (name, p0, rho0) in [("venus", 92.1e5, 65.5), ("mars", 699.0, 0.0150), ("titan", 1.467e5, 5.17)] {
            let def = load_default(name);
            let atm = def.physical.atmosphere.as_ref().unwrap_or_else(|| panic!("{name} has air"));
            let table = atm.table.as_ref().unwrap_or_else(|| panic!("{name} has a table"));
            assert!((atm.pressure(0.0) / p0 - 1.0).abs() < 0.01, "{name}: {}", atm.pressure(0.0));
            assert!((atm.density(0.0) / rho0 - 1.0).abs() < 0.01, "{name}: {}", atm.density(0.0));
            let rows = table.rows();
            assert!(rows.windows(2).all(|w| w[1].3 < w[0].3), "{name}: density falls row by row");
            let last = rows.last().unwrap();
            assert!(last.3 < 1e-13, "{name}: {} kg/m³ where the table (and drag) ends", last.3);
            let at_top = table.density(atm.top);
            assert!((1e-10..1e-8).contains(&at_top), "{name}: {at_top} kg/m³ at the top");
            assert_eq!(def.physical.rails_floor, atm.top, "{name}: rails floor at the top (D062)");
        }
    }

    /// The Moon's and Triton's rotation against the full IAU WGCCRE 2015
    /// series (Archinal et al. 2018, every periodic term), over 2030-2080:
    /// the files carry the terms of at least 0.01 deg.
    #[test]
    fn periodic_rotation_terms_follow_the_iau_series() {
        fn sind(x: f64) -> f64 {
            (x * DEG).sin()
        }
        fn cosd(x: f64) -> f64 {
            (x * DEG).cos()
        }
        let moon = |d: f64| {
            let t = d / 36_525.0;
            let e = [
                125.045 - 0.0529921 * d,
                250.089 - 0.1059842 * d,
                260.008 + 13.0120009 * d,
                176.625 + 13.3407154 * d,
                357.529 + 0.9856003 * d,
                311.589 + 26.4057084 * d,
                134.963 + 13.0649930 * d,
                276.617 + 0.3287146 * d,
                34.226 + 1.7484877 * d,
                15.134 - 0.1589763 * d,
                119.743 + 0.0036096 * d,
                239.961 + 0.1643573 * d,
                25.053 + 12.9590088 * d,
            ];
            let ra = 269.9949 + 0.0031 * t - 3.8787 * sind(e[0]) - 0.1204 * sind(e[1]) + 0.0700 * sind(e[2])
                - 0.0172 * sind(e[3])
                + 0.0072 * sind(e[5])
                - 0.0052 * sind(e[9])
                + 0.0043 * sind(e[12]);
            let dec = 66.5392 + 0.0130 * t + 1.5419 * cosd(e[0]) + 0.0239 * cosd(e[1]) - 0.0278 * cosd(e[2])
                + 0.0068 * cosd(e[3])
                - 0.0029 * cosd(e[5])
                + 0.0009 * cosd(e[6])
                + 0.0008 * cosd(e[9])
                - 0.0009 * cosd(e[12]);
            let w = 38.3213 + 13.17635815 * d - 1.4e-12 * d * d + 3.5610 * sind(e[0]) + 0.1208 * sind(e[1])
                - 0.0642 * sind(e[2])
                + 0.0158 * sind(e[3])
                + 0.0252 * sind(e[4])
                - 0.0066 * sind(e[5])
                - 0.0047 * sind(e[6])
                - 0.0046 * sind(e[7])
                + 0.0028 * sind(e[8])
                + 0.0052 * sind(e[9])
                + 0.0040 * sind(e[10])
                + 0.0019 * sind(e[11])
                - 0.0044 * sind(e[12]);
            (ra, dec, Some(w))
        };
        let triton = |d: f64| {
            let n7 = 177.85 + 52.316 * d / 36_525.0;
            let ra_k = [-32.35, -6.28, -2.08, -0.74, -0.28, -0.11, -0.07, -0.02, -0.01];
            let dec_k = [22.55, 2.10, 0.55, 0.16, 0.05, 0.02, 0.01];
            let ra = 299.36 + ra_k.iter().enumerate().map(|(k, a)| a * sind((k + 1) as f64 * n7)).sum::<f64>();
            let dec = 41.17 + dec_k.iter().enumerate().map(|(k, a)| a * cosd((k + 1) as f64 * n7)).sum::<f64>();
            (ra, dec, None)
        };
        type Series = dyn Fn(f64) -> (f64, f64, Option<f64>);
        let cases: [(&str, &Series, f64, f64); 2] = [("moon", &moon, 0.01, 0.03), ("triton", &triton, 0.001, 0.0)];
        for (name, iau, pole_tol, w_tol) in cases {
            let rot = load_default(name).physical.rotation;
            assert!(rot.n_librations > 0, "{name} carries periodic terms");
            for year in [2030, 2041, 2055, 2068, 2080] {
                let t = crate::time::Epoch::from_calendar(year, 6, 1, 0, 0, 0.0);
                let d = t.to_seconds_f64() / 86_400.0;
                let (ra, dec, w) = iau(d);
                let unit = |ra: f64, dec: f64| glam::DVec3::new(cosd(dec) * cosd(ra), cosd(dec) * sind(ra), sind(dec));
                let angle = rot.pole(t).dot(unit(ra, dec)).clamp(-1.0, 1.0).acos() / DEG;
                assert!(angle < pole_tol, "{name} {year}: pole {angle} deg off the IAU series");
                if let Some(w) = w {
                    let dw = math::wrap_pi(rot.angle(t) - math::wrap_tau(w * DEG)) / DEG;
                    assert!(dw.abs() < w_tol, "{name} {year}: W {dw} deg off");
                }
            }
        }
    }

    #[test]
    fn directory_names_are_lower_case_letters_and_digits() {
        assert_eq!(body_dir_name("Earth"), "earth");
        assert_eq!(body_dir_name("Hi'iaka"), "hiiaka");
        assert_eq!(body_dir_name("Eris barycenter"), "erisbarycenter");
    }

    #[test]
    fn bad_files_report_their_path() {
        let e = parse_body("(physical: (name: 3))", Path::new("x")).unwrap_err();
        let expected = Path::new("x").join(BODY_FILE).display().to_string();
        assert!(e.to_string().starts_with(&expected), "{e}");
        assert!(load_body(Path::new("/nonexistent/body")).is_err());
    }
}
