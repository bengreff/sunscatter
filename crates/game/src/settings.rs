//! Graphics settings (D048): five tiers, each a preset over individual
//! feature toggles and quality knobs. Every toggle can be changed on its own;
//! changing anything applies live (no restart).
//!
//! Minimal is the default and the only tier developed (D073): terrain
//! relief (the drawn ground is the physical one), crisp surface zones
//! (crisp-surfaces.md), the physical sky (the atmosphere raymarched from
//! outside it, lookup tables inside), stars, shadows and the simple ground
//! pattern (terrain detail off); no bloom, flare, glint, planetshine or sky
//! light.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Tier {
    #[default]
    Minimal,
    Low,
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
    /// Surface zones (D073, crisp-surfaces.md): crisp coasts and zone edges
    /// at every distance, each zone one procedural texture, instead of the
    /// magnified colour map.
    pub zones: bool,
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
    /// Planetshine (Earthshine on the Moon, moonlight on Earth; D055).
    pub earthshine: bool,
    /// Sky light from the atmosphere's environment map (ambient and
    /// reflections). Regenerated and filtered every frame: ~3 ms at 1600×900,
    /// ~12 ms on a Retina window. Off: a daylight ambient stand-in.
    pub sky_light: bool,
    /// Haze strength: the aerial perspective on the ground and ships, as a
    /// multiple of the physical air density along the view ray (1 =
    /// physical). Not part of the tiers.
    pub haze: f32,
    /// Exposure follows the starlight at the camera (D087): a day side far
    /// from the Sun reads like Earth's. Off: the fixed exposure of 1 AU.
    /// Not part of the tiers.
    pub starlight_exposure: bool,
}

impl GraphicsSettings {
    pub fn preset(tier: Tier) -> Self {
        let base = GraphicsSettings {
            tier: Some(tier),
            terrain: false,
            terrain_error_px: 8.0,
            texture_size: 2048,
            detail: false,
            zones: false,
            atmosphere: AtmosphereQuality::Off,
            ocean_glint: false,
            star_magnitude: 0.0,
            bloom: false,
            flare: false,
            shadows: false,
            msaa: MsaaLevel::Off,
            earthshine: false,
            sky_light: false,
            haze: 1.0,
            starlight_exposure: true,
        };
        match tier {
            Tier::Minimal => GraphicsSettings {
                terrain: true,
                zones: true,
                // Adaptive: lookup tables inside an atmosphere, raymarched
                // from outside, where the tables' aerial perspective washes
                // the ground out (`atmosphere::mode`).
                atmosphere: AtmosphereQuality::Raymarched,
                star_magnitude: 5.0,
                shadows: true,
                msaa: MsaaLevel::X2,
                ..base
            },
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
            Tier::Ultra => GraphicsSettings {
                terrain_error_px: 1.0,
                star_magnitude: 8.0,
                sky_light: true,
                ..Self::preset(Tier::High)
            },
        }
        .with_tier(tier)
    }

    /// Tier `tier` with this settings' choices that are not part of the
    /// tiers (haze, exposure) kept.
    pub fn with_preset(&self, tier: Tier) -> Self {
        GraphicsSettings { haze: self.haze, starlight_exposure: self.starlight_exposure, ..Self::preset(tier) }
    }

    fn with_tier(self, tier: Tier) -> Self {
        GraphicsSettings { tier: Some(tier), ..self }
    }

    /// The tier this matches exactly, if any (after a toggle changes).
    pub fn matching_tier(&self) -> Option<Tier> {
        Tier::ALL.into_iter().find(|&t| {
            let p = Self::preset(t);
            let own =
                GraphicsSettings { tier: None, haze: self.haze, starlight_exposure: self.starlight_exposure, ..p };
            own == GraphicsSettings { tier: None, ..*self }
        })
    }
}

impl Default for GraphicsSettings {
    /// The default tier, or `SUNSCATTER_TIER=<name>` if set; the haze from
    /// `SUNSCATTER_HAZE=<strength>` if set (for demo comparisons).
    fn default() -> Self {
        let from_env = std::env::var("SUNSCATTER_TIER")
            .ok()
            .and_then(|name| Tier::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(&name)));
        let haze = std::env::var("SUNSCATTER_HAZE").ok().and_then(|h| h.parse().ok()).unwrap_or(1.0);
        GraphicsSettings { haze, ..Self::preset(from_env.unwrap_or_default()) }
    }
}

/// Camera and light state that follows the settings.
pub fn apply(
    settings: Res<GraphicsSettings>,
    mut commands: Commands,
    cams: Query<Entity, With<crate::camera::MainCamera>>,
    mut lights: Query<(&mut DirectionalLight, &crate::lighting::StarLight)>,
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
    // Shadow maps (ships, nearby ground) from the brightest star only.
    for (mut l, slot) in &mut lights {
        l.shadow_maps_enabled = s.shadows && slot.0 == 0;
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
        let hazy = GraphicsSettings { haze: 0.5, ..GraphicsSettings::preset(Tier::Low) };
        assert_eq!(hazy.matching_tier(), Some(Tier::Low), "haze is not part of the tiers");
        assert_eq!(hazy.with_preset(Tier::High).haze, 0.5);
        let fixed = GraphicsSettings { starlight_exposure: false, ..GraphicsSettings::preset(Tier::Medium) };
        assert_eq!(fixed.matching_tier(), Some(Tier::Medium), "the exposure rule is not part of the tiers");
        assert!(!fixed.with_preset(Tier::Minimal).starlight_exposure);
        assert!(GraphicsSettings::preset(Tier::Minimal).starlight_exposure, "on by default (D087)");
        let custom = GraphicsSettings { bloom: false, ..GraphicsSettings::preset(Tier::Ultra) };
        assert_eq!(custom.matching_tier(), None);
    }

    #[test]
    fn minimal_is_the_default_and_draws_what_physics_uses() {
        let m = GraphicsSettings::preset(Tier::Minimal);
        assert_eq!(Tier::default(), Tier::Minimal);
        // Relief on (the drawn ground is the physical ground), crisp zones,
        // the physical sky (raymarched from orbit), stars and shadows; the
        // costly effects off.
        assert!(m.terrain && m.zones && m.shadows && m.star_magnitude >= 5.0);
        assert_eq!(m.atmosphere, AtmosphereQuality::Raymarched);
        assert!(!(m.detail || m.bloom || m.flare || m.ocean_glint || m.earthshine || m.sky_light));
        assert_ne!(m.msaa, MsaaLevel::X4);
    }

    #[test]
    fn higher_tiers_are_unchanged_by_the_zones() {
        // D073: only Minimal is developed; the others keep their presets
        // (zones off) as before 2026-10-25.
        let atmos = [AtmosphereQuality::Lut, AtmosphereQuality::Raymarched, AtmosphereQuality::Raymarched];
        for (t, a) in [Tier::Low, Tier::Medium, Tier::High].into_iter().zip(atmos) {
            let p = GraphicsSettings::preset(t);
            assert!(!p.zones, "{t:?}");
            assert_eq!(p.atmosphere, a, "{t:?}");
        }
        let low = GraphicsSettings::preset(Tier::Low);
        assert_eq!((low.texture_size, low.msaa, low.shadows), (4096, MsaaLevel::X2, false));
        assert!(!GraphicsSettings::preset(Tier::Ultra).zones);
    }
}
