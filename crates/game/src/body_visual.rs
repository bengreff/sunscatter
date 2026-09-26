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
    /// Tint applied to the colour map under water (multiplier).
    pub tint: [f32; 3],
    /// Water mask (8-bit sea fraction, baked by `asset-tool`), relative to
    /// the body dir. Without one no water is drawn.
    #[serde(default)]
    pub mask: Option<String>,
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
            for file in [def.color_map, mask].into_iter().flatten() {
                assert!(super::body_dir(name).join(&file).exists(), "{name}: {file}");
            }
        }
    }
}
