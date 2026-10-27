//! Ships' lighting (D055, task 3 of 2026-10-26): every drawn star at the
//! ship's own position with its eclipses, planetshine from the significant
//! reflectors as lit discs (`lighting::rules::shine_disc`: right in the
//! near field, where a ship in low orbit sees the lit Earth fill most of
//! half its sky and its sides away from the Sun are lit by it), and the
//! starlight without a light slot. A material over Bevy's standard one
//! (`ship.wgsl`), whose per-frame data lives in a storage buffer per ship
//! so the material itself is never modified per frame.

use crate::lighting::rules::{ObjectLight, MAX_SHINES, MAX_STARS};
use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use glam::DVec3;

pub type ShipMaterial = ExtendedMaterial<StandardMaterial, ShipExt>;

const SHADER: &str = "embedded://game/ship.wgsl";

/// One ship's light this frame (`ship.wgsl`'s `ShipLight`).
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType, Reflect)]
pub struct ShipLight {
    /// Per light slot: xyz the direction to its star from the camera (the
    /// slot's Bevy light's), w the share of that light the ship receives
    /// (its flux here over its flux at the camera, eclipses included).
    pub stars: [Vec4; MAX_STARS],
    /// Per planetshine source: xyz the direction to the lit disc's centre,
    /// w sin² of its angular radius.
    pub shine_dir: [Vec4; MAX_SHINES],
    /// rgb: its mean radiance (lux per steradian).
    pub shine_radiance: [Vec4; MAX_SHINES],
    /// x: starlight without a light slot, sphere-averaged (lux).
    pub ambient: Vec4,
}

/// The shader data from a ship's [`ObjectLight`], with the directions to
/// the slotted stars from the camera (`star_dirs`, in slot order).
pub fn ship_light(light: &ObjectLight, star_dirs: &[DVec3]) -> ShipLight {
    let mut s = ShipLight::default();
    for (k, (dir, scale)) in star_dirs.iter().zip(&light.scale).enumerate().take(MAX_STARS) {
        s.stars[k] = dir.as_vec3().extend(*scale as f32);
    }
    for (m, d) in light.shines.iter().enumerate().take(MAX_SHINES) {
        s.shine_dir[m] = d.dir.as_vec3().extend(d.sin2 as f32);
        let [r, g, b] = d.radiance.map(|c| c as f32);
        s.shine_radiance[m] = Vec4::new(r, g, b, 1.0);
    }
    s.ambient.x = light.ambient as f32;
    s
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct ShipExt {
    /// This frame's [`ShipLight`].
    #[storage(100, read_only)]
    pub light: Handle<ShaderBuffer>,
}

impl MaterialExtension for ShipExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub struct ShipLightPlugin;

impl Plugin for ShipLightPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "ship.wgsl");
        app.add_plugins(MaterialPlugin::<ShipMaterial>::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lighting::rules::ShineDisc;

    #[test]
    fn the_shader_data_holds_every_slot_and_source() {
        let light = ObjectLight {
            scale: vec![1.0, 0.25],
            shines: vec![ShineDisc { dir: DVec3::NEG_Z, sin2: 0.88, radiance: [3.0, 4.0, 5.0] }],
            ambient: 0.5,
        };
        let s = ship_light(&light, &[DVec3::X, DVec3::Y]);
        assert_eq!(s.stars[0], Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(s.stars[1], Vec4::new(0.0, 1.0, 0.0, 0.25));
        assert_eq!(s.stars[2].w, 0.0, "no star in the slot");
        assert!((s.shine_dir[0] - Vec4::new(0.0, 0.0, -1.0, 0.88)).length() < 1e-6);
        assert_eq!(s.shine_radiance[0], Vec4::new(3.0, 4.0, 5.0, 1.0));
        assert_eq!(s.shine_radiance[1].w, 0.0, "one source only");
        assert_eq!(s.ambient.x, 0.5);
    }

    #[test]
    fn the_shader_mirrors_the_disc_irradiance() {
        let wgsl = include_str!("ship.wgsl");
        assert!(wgsl.contains("fn disc_irradiance(cos_n: f32, sin2: f32) -> f32"));
        assert!(wgsl.contains(&format!("const MAX_STARS: i32 = {MAX_STARS};")));
        assert!(wgsl.contains(&format!("const MAX_SHINES: i32 = {MAX_SHINES};")));
    }
}
