//! Graphics settings (D048): five tiers, each a preset over individual
//! feature toggles and quality knobs. Every toggle can be changed on its own;
//! changing anything applies live (no restart).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Tier {
    Minimal,
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

impl Tier {
    pub const ALL: [Tier; 5] = [Tier::Minimal, Tier::Low, Tier::Medium, Tier::High, Tier::Ultra];

    pub fn name(self) -> &'static str {
        match self {
            Tier::Minimal => "Minimal",
            Tier::Low => "Low",
            Tier::Medium => "Medium",
            Tier::High => "High",
            Tier::Ultra => "Ultra",
        }
    }
}

/// How the atmosphere is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtmosphereQuality {
    Off,
    /// Lookup tables only (cheap; less accurate from far away).
    Lut,
    /// Per-pixel raymarching (accurate from orbit).
    Raymarched,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MsaaLevel {
    Off,
    X2,
    X4,
}

/// Individual graphics features. Tiers are presets of this struct.
/// Fields missing from a settings file take the default tier's values.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphicsSettings {
    pub tier: Option<Tier>,
    /// Displaced terrain LOD (otherwise a smooth ellipsoid).
    pub terrain: bool,
    /// Terrain LOD target: maximum geometric error in pixels.
    pub terrain_error_px: f32,
    /// Colour-map texture size cap (longest side, pixels).
    pub texture_size: u32,
    /// Close-range procedural detail layer on terrain.
    pub detail: bool,
    pub atmosphere: AtmosphereQuality,
    /// Sun glint on Earth's oceans.
    pub ocean_glint: bool,
    /// Faintest star magnitude drawn (0 = no stars).
    pub star_magnitude: f32,
    pub bloom: bool,
    /// Lens flare and glare from the Sun.
    pub flare: bool,
    pub shadows: bool,
    pub msaa: MsaaLevel,
    /// Earthshine on the Moon and ships.
    pub earthshine: bool,
}

impl GraphicsSettings {
    pub fn preset(tier: Tier) -> Self {
        let base = GraphicsSettings {
            tier: Some(tier),
            terrain: false,
            terrain_error_px: 8.0,
            texture_size: 2048,
            detail: false,
            atmosphere: AtmosphereQuality::Off,
            ocean_glint: false,
            star_magnitude: 0.0,
            bloom: false,
            flare: false,
            shadows: false,
            msaa: MsaaLevel::Off,
            earthshine: false,
        };
        match tier {
            Tier::Minimal => base,
            Tier::Low => GraphicsSettings {
                terrain: true,
                texture_size: 4096,
                atmosphere: AtmosphereQuality::Lut,
                star_magnitude: 5.0,
                msaa: MsaaLevel::X2,
                ..base
            },
            Tier::Medium => GraphicsSettings {
                terrain: true,
                terrain_error_px: 4.0,
                texture_size: 8192,
                detail: true,
                atmosphere: AtmosphereQuality::Raymarched,
                star_magnitude: 6.0,
                bloom: true,
                msaa: MsaaLevel::X4,
                ..base
            },
            Tier::High => GraphicsSettings {
                terrain: true,
                terrain_error_px: 2.0,
                texture_size: 8192,
                detail: true,
                atmosphere: AtmosphereQuality::Raymarched,
                ocean_glint: true,
                star_magnitude: 6.5,
                bloom: true,
                flare: true,
                shadows: true,
                msaa: MsaaLevel::X4,
                earthshine: true,
                ..base
            },
            Tier::Ultra => GraphicsSettings { terrain_error_px: 1.0, star_magnitude: 8.0, ..Self::preset(Tier::High) },
        }
        .with_tier(tier)
    }

    fn with_tier(self, tier: Tier) -> Self {
        GraphicsSettings { tier: Some(tier), ..self }
    }

    /// The tier this matches exactly, if any (after a toggle changes).
    pub fn matching_tier(&self) -> Option<Tier> {
        Tier::ALL.into_iter().find(|&t| {
            let p = Self::preset(t);
            GraphicsSettings { tier: None, ..p } == GraphicsSettings { tier: None, ..*self }
        })
    }
}

impl Default for GraphicsSettings {
    /// The default tier, or `SUNSCATTER_TIER=<name>` if set.
    fn default() -> Self {
        let from_env = std::env::var("SUNSCATTER_TIER")
            .ok()
            .and_then(|name| Tier::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(&name)));
        Self::preset(from_env.unwrap_or_default())
    }
}

/// Camera and light state that follows the settings.
pub fn apply(
    settings: Res<GraphicsSettings>,
    mut commands: Commands,
    cams: Query<Entity, With<crate::camera::MainCamera>>,
    mut lights: Query<&mut DirectionalLight, With<crate::scene::SunLight>>,
) {
    if !settings.is_changed() {
        return;
    }
    use bevy::post_process::bloom::Bloom;
    let s = *settings;
    for cam in &cams {
        let mut e = commands.entity(cam);
        e.insert(match s.msaa {
            MsaaLevel::Off => Msaa::Off,
            MsaaLevel::X2 => Msaa::Sample2,
            MsaaLevel::X4 => Msaa::Sample4,
        });
        if s.bloom {
            e.insert(Bloom { intensity: 0.12, max_mip_dimension: 256, ..Bloom::NATURAL });
        } else {
            e.remove::<Bloom>();
        }
    }
    for mut l in &mut lights {
        l.shadow_maps_enabled = s.shadows;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match_their_tier() {
        for t in Tier::ALL {
            assert_eq!(GraphicsSettings::preset(t).matching_tier(), Some(t));
        }
        let custom = GraphicsSettings { bloom: false, ..GraphicsSettings::preset(Tier::Ultra) };
        assert_eq!(custom.matching_tier(), None);
    }
}
