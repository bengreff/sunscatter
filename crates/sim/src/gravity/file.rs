//! A body's gravity field as data: `data/bodies/<body>/gravity.ron` (D051,
//! D090), written by `ephem-tool gravity` from the published model and read
//! here.
//!
//! ```text
//! (
//!     model: "GRGM1200A",
//!     radius: 1738000.0,           // reference radius of the coefficients (m)
//!     gm: Some(4.9028001224453e12), // the model's GM (m³/s²); None: the source's
//!     normalized: true,            // fully normalized (geodesy 4π); false: unnormalized
//!     frame: Some(((..), (..), (..))), // rows: body-fixed (body.ron) → the model's frame
//!     coefficients: [(2, 0, -9.0884339347424e-5, 0.0), ...], // (n, m, C, S)
//! )
//! ```

use super::{normalization, Coefficient, Field};
use glam::{DMat3, DVec3};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// File name of a body's gravity field inside its directory.
pub const GRAVITY_FILE: &str = "gravity.ron";

#[derive(Deserialize)]
struct FieldFile {
    model: String,
    radius: f64,
    #[serde(default)]
    gm: Option<f64>,
    normalized: bool,
    #[serde(default)]
    frame: Option<[[f64; 3]; 3]>,
    coefficients: Vec<(usize, usize, f64, f64)>,
}

/// Parses a gravity file's text.
pub fn parse(text: &str) -> Result<Field, String> {
    let f: FieldFile = ron::from_str(text).map_err(|e| e.to_string())?;
    let coefficients: Vec<Coefficient> = f
        .coefficients
        .iter()
        .map(|&(n, m, c, s)| {
            let scale = if f.normalized || m > n { 1.0 } else { 1.0 / normalization(n, m) };
            Coefficient { n, m, c: c * scale, s: s * scale }
        })
        .collect();
    let frame = match f.frame {
        Some(rows) => {
            let m = DMat3::from_cols(DVec3::from(rows[0]), DVec3::from(rows[1]), DVec3::from(rows[2])).transpose();
            if (m * m.transpose() - DMat3::IDENTITY).to_cols_array().iter().any(|e| e.abs() > 1e-12) {
                return Err("frame is not a rotation".into());
            }
            Some(m)
        }
        None => None,
    };
    Field::new(&f.model, f.radius, f.gm, &coefficients, frame)
}

/// Loads `dir/gravity.ron` if it exists (`Ok(None)` if not), once per
/// path per process (worlds are built often; the Moon's file is large).
pub fn load(dir: &Path) -> Result<Option<Arc<Field>>, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Field>>>> = OnceLock::new();
    let path = dir.join(GRAVITY_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let key = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(f) = cache.get(&key) {
        return Ok(Some(f.clone()));
    }
    let text = std::fs::read_to_string(&key).map_err(|e| format!("{}: {e}", path.display()))?;
    let field = Arc::new(parse(&text).map_err(|e| format!("{}: {e}", path.display()))?);
    cache.insert(key, field.clone());
    Ok(Some(field))
}
