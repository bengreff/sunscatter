//! Per-body visual definitions, loaded from `data/bodies/<body>/visual.ron`
//! (D048: every visual is data-driven). Physical data lives next to it in
//! `body.ron` and is read by the sim; rendering code never matches on names.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub struct BodyVisualDef {
    /// Colour when no colour map is loaded (linear-ish sRGB triple).
    pub base_color: [f32; 3],
    /// Equirectangular colour map (sRGB JPEG/PNG), relative to the body dir.
    #[serde(default)]
    pub color_map: Option<String>,
    /// Surface roughness (0 = mirror, 1 = matte).
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    #[serde(default)]
    pub ocean: Option<OceanDef>,
    #[serde(default)]
    pub detail: Option<DetailDef>,
    /// Surface zones drawn crisp from any distance (D073, crisp-surfaces.md).
    #[serde(default)]
    pub zones: Option<ZonesDef>,
    #[serde(default)]
    pub atmosphere: Option<AtmosphereDef>,
    /// Self-luminous bodies (stars).
    #[serde(default)]
    pub emissive: Option<EmissiveDef>,
    /// Bond albedo (for light reflected onto nearby objects).
    #[serde(default = "default_albedo")]
    pub albedo: f32,
    /// Icon colour in map mode.
    #[serde(default = "default_icon")]
    pub icon_color: [f32; 3],
}

fn default_roughness() -> f32 {
    0.9
}

fn default_albedo() -> f32 {
    0.3
}

fn default_icon() -> [f32; 3] {
    [0.8, 0.8, 0.8]
}

/// Water at sea level (the physics treats it as solid ground, D037).
#[derive(Clone, Debug, Deserialize)]
pub struct OceanDef {
    pub roughness: f32,
    /// Linear albedo of open water (water-leaving reflectance, without the
    /// surface's Fresnel reflection, which the lighting adds). The colour
    /// map under water is raised to at least this: Blue Marble's open ocean
    /// is ~0.002, far darker than real water, so the haze swamped it.
    pub albedo: [f32; 3],
    /// Water mask (8-bit sea fraction, baked by `asset-tool`), relative to
    /// the body dir. Without one no water is drawn.
    #[serde(default)]
    pub mask: Option<String>,
    /// With zones: the water-leaving reflectance over a bright shallow
    /// bottom; water is drawn albedo + (shallow − albedo)·exp(−depth /
    /// `shallow_depth_m`), the depth from the heightmap.
    #[serde(default)]
    pub shallow_albedo: Option<[f32; 3]>,
    #[serde(default)]
    pub shallow_depth_m: Option<f32>,
}

/// Surface zones: a zone map baked by `asset-tool zones` (which reads the
/// classification fields from here) and how each zone is drawn.
#[derive(Clone, Debug, Deserialize)]
pub struct ZonesDef {
    /// The zone map (8-bit index: 0 sea, 1..=N the list), relative to the body dir.
    pub map: String,
    /// How much of the colour map's variation survives inside a zone, as an
    /// exponent of satellite / source (0: flat zone colours).
    #[serde(default)]
    pub tint: f32,
    pub list: Vec<ZoneDef>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ZoneDef {
    #[allow(dead_code)] // For people and asset-tool's report.
    pub name: String,
    /// The zone's colour in the colour map (sRGB), the classifier's centre.
    pub source: [u8; 3],
    /// The colour it is drawn with (sRGB); default: `source`.
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// Classifier constraints (read by `asset-tool`).
    #[serde(default)]
    #[allow(dead_code)]
    pub min_abs_lat: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub max_abs_lat: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub min_height_m: Option<f32>,
    #[serde(default)]
    #[allow(dead_code)]
    pub max_height_m: Option<f32>,
    /// Drawn as water (lakes above sea level, which the sea mask does not hold).
    #[serde(default)]
    pub water: bool,
    #[serde(default)]
    pub pattern: PatternDef,
}

/// A zone's procedural texture: brightness × (1 + contrast × fBm), with
/// octaves from `scale_m` down, each fading out by the pixel footprint.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct PatternDef {
    pub scale_m: f32,
    pub contrast: f32,
}

impl Default for PatternDef {
    /// Flat (no pattern).
    fn default() -> Self {
        PatternDef { scale_m: 1000.0, contrast: 0.0 }
    }
}

/// Close-range procedural detail layered over the colour map.
#[derive(Clone, Debug, Deserialize)]
pub struct DetailDef {
    /// Base feature size (m).
    pub scale_m: f32,
    /// Brightness variation (0..1).
    pub strength: f32,
    /// Distance (m) over which the layer fades out (default 400 × scale).
    #[serde(default)]
    pub fade_m: Option<f32>,
    /// Colour on steep slopes.
    pub rock_color: [f32; 3],
    /// Snow above this height (m) at the equator, lower towards the poles.
    #[serde(default)]
    pub snow_line_m: Option<f32>,
    #[serde(default = "default_snow")]
    pub snow_color: [f32; 3],
}

fn default_snow() -> [f32; 3] {
    [0.9, 0.92, 0.95]
}

/// Scattering parameters for a Hillaire-style atmosphere, in absolute units
/// (m, m⁻¹) so they are independent of the renderer's normalisations.
#[derive(Clone, Debug, Deserialize)]
pub struct AtmosphereDef {
    /// Height of the top of the atmosphere above the mean radius (m).
    pub thickness_m: f32,
    pub rayleigh_scattering: [f32; 3],
    pub rayleigh_scale_height_m: f32,
    pub mie_scattering: [f32; 3],
    pub mie_absorption: [f32; 3],
    pub mie_scale_height_m: f32,
    pub mie_asymmetry: f32,
    /// Ozone-like absorbing layer (tent profile), optional.
    #[serde(default)]
    pub ozone: Option<OzoneDef>,
    pub ground_albedo: [f32; 3],
}

#[derive(Clone, Debug, Deserialize)]
pub struct OzoneDef {
    pub absorption: [f32; 3],
    pub center_m: f32,
    pub width_m: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EmissiveDef {
    pub color: [f32; 3],
    /// Surface luminance used for rendering (cd/m², artistic: bright enough
    /// for bloom, not the physical 1.6e9).
    pub luminance: f32,
    /// Luminosity (W) of the light this body casts (D055).
    pub luminosity_w: f64,
    /// Luminous efficacy of its spectrum (lm/W).
    pub luminous_efficacy: f64,
}

/// Directory holding per-body data.
pub fn bodies_dir() -> PathBuf {
    PathBuf::from(format!("{}/../../data/bodies", env!("CARGO_MANIFEST_DIR")))
}

/// Directory for one body (lowercase name).
pub fn body_dir(name: &str) -> PathBuf {
    bodies_dir().join(name.to_lowercase())
}

/// Loads a body's visual definition, if it has one.
pub fn load(name: &str) -> Option<BodyVisualDef> {
    let path = body_dir(name).join("visual.ron");
    let text = std::fs::read_to_string(&path).ok()?;
    match ron::from_str(&text) {
        Ok(def) => Some(def),
        Err(e) => {
            bevy::log::error!("{}: {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shipped_definitions_parse() {
        for name in ["Earth", "Moon", "Sun"] {
            assert!(super::load(name).is_some(), "{name}");
        }
        // Every referenced map exists.
        for name in ["Earth", "Moon", "Sun"] {
            let def = super::load(name).unwrap();
            let mask = def.ocean.and_then(|o| o.mask);
            let zones = def.zones.map(|z| z.map);
            for file in [def.color_map, mask, zones].into_iter().flatten() {
                assert!(super::body_dir(name).join(&file).exists(), "{name}: {file}");
            }
        }
    }

    #[test]
    fn shipped_zones_parse_and_fit_the_palette() {
        for name in ["Earth", "Moon"] {
            let def = super::load(name).unwrap();
            // The zone colours are sampled with the colour map's sampler.
            assert!(def.color_map.is_some(), "{name}: zones need a colour map");
            let z = def.zones.unwrap_or_else(|| panic!("{name} has zones"));
            assert!(!z.list.is_empty() && z.list.len() < crate::terrain::zones::MAX_ZONES, "{name}");
            assert!((0.0..=1.0).contains(&z.tint), "{name}");
            assert!(z.list.iter().all(|d| d.pattern.scale_m > 0.0 && (0.0..1.0).contains(&d.pattern.contrast)));
        }
        let ocean = super::load("Earth").unwrap().ocean.unwrap();
        assert!(ocean.shallow_albedo.is_some() && ocean.shallow_depth_m.is_some_and(|d| d > 0.0));
    }
}
