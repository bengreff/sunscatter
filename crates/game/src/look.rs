//! The one look (D073): simple, crisp, functional, the same for every
//! player, with no graphics settings. What it draws: terrain relief (the
//! drawn ground is the physical one), crisp surface zones, the physical sky
//! (the atmosphere from lookup tables inside it, raymarched from outside),
//! stars, shadows, planetshine, rings, cloud decks, a sun flare, at the
//! window's full resolution with 2× MSAA.
//!
//! [`Parts`] switches each part off for the benchmark only (`bench`: what
//! each part costs); it is never saved or shown as a setting.

use bevy::prelude::*;

/// Terrain LOD target: the largest geometric error, in pixels.
pub const TERRAIN_ERROR_PX: f32 = 8.0;
/// Colour maps are loaded at most this wide: with zones, they only give the
/// zones' tint and the giants' cloud colours (crisp-surfaces.md).
pub const COLOR_MAP_WIDTH: u32 = 2048;
/// The faintest star drawn (visual magnitude).
pub const STAR_MAGNITUDE: f32 = 5.0;

/// The look's parts, all on; the benchmark turns one off at a time.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parts {
    /// Terrain relief (off: smooth ellipsoids).
    pub relief: bool,
    /// Surface zones (off: the colour map).
    pub zones: bool,
    /// The zones' domain warp (crisp, fractal edges) and their pattern.
    pub zone_warp: bool,
    pub zone_pattern: bool,
    /// Atmospheres.
    pub atmosphere: bool,
    /// Raymarching the atmosphere from outside it (off: lookup tables).
    pub raymarch: bool,
    pub shadows: bool,
    pub msaa: bool,
    pub stars: bool,
    pub planetshine: bool,
    pub flare: bool,
    /// The 3D view at the resolution the policy picks (`render_scale`);
    /// off: at half of it (the benchmark's row for the policy's floor).
    pub native: bool,
}

impl Default for Parts {
    fn default() -> Self {
        Parts {
            relief: true,
            zones: true,
            zone_warp: true,
            zone_pattern: true,
            atmosphere: true,
            raymarch: true,
            shadows: true,
            msaa: true,
            stars: true,
            planetshine: true,
            flare: true,
            native: true,
        }
    }
}

impl Parts {
    /// Each part's name and the look without it (the benchmark's list).
    pub fn each_off() -> Vec<(&'static str, Parts)> {
        let on = Parts::default();
        vec![
            ("relief", Parts { relief: false, ..on }),
            ("zones", Parts { zones: false, ..on }),
            ("zones' warp", Parts { zone_warp: false, ..on }),
            ("zones' pattern", Parts { zone_pattern: false, ..on }),
            ("atmosphere", Parts { atmosphere: false, ..on }),
            ("raymarching from outside", Parts { raymarch: false, ..on }),
            ("shadows", Parts { shadows: false, ..on }),
            ("MSAA", Parts { msaa: false, ..on }),
            ("stars", Parts { stars: false, ..on }),
            ("planetshine", Parts { planetshine: false, ..on }),
            ("flare", Parts { flare: false, ..on }),
            ("full resolution (half)", Parts { native: false, ..on }),
        ]
    }
}

/// The camera's MSAA and the brightest star's shadow maps, from [`Parts`]
/// (once at start, and whenever the benchmark changes them).
pub fn apply(
    parts: Res<Parts>,
    mut commands: Commands,
    cams: Query<Entity, With<crate::camera::MainCamera>>,
    mut lights: Query<(&mut DirectionalLight, Option<&crate::lighting::StarLight>)>,
    mut done: Local<bool>,
) {
    if *done && !parts.is_changed() {
        return;
    }
    *done = true;
    for cam in &cams {
        commands.entity(cam).insert(if parts.msaa { Msaa::Sample2 } else { Msaa::Off });
    }
    // Shadow maps from the brightest star only: its light's (the nearby
    // ground) and the craft shadow light's (craft, `lighting::shadows`).
    for (mut l, slot) in &mut lights {
        l.shadow_maps_enabled = parts.shadows && slot.is_none_or(|s| s.0 == 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_benchmark_turns_off_one_part_at_a_time() {
        let on = Parts::default();
        let parts = Parts::each_off();
        assert_eq!(parts.len(), 12);
        for (name, p) in &parts {
            // Exactly one part differs from the look.
            let flags = |p: &Parts| {
                [
                    p.relief,
                    p.zones,
                    p.zone_warp,
                    p.zone_pattern,
                    p.atmosphere,
                    p.raymarch,
                    p.shadows,
                    p.msaa,
                    p.stars,
                    p.planetshine,
                    p.flare,
                    p.native,
                ]
            };
            let off = flags(p).iter().zip(flags(&on)).filter(|(a, b)| **a != *b).count();
            assert_eq!(off, 1, "{name}");
        }
        let names: std::collections::HashSet<_> = parts.iter().map(|p| p.0).collect();
        assert_eq!(names.len(), parts.len(), "names are unique");
    }
}
