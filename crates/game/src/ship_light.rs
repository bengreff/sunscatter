//! Ships' lighting (D055, D110): each craft lit as one, at its own
//! position, never the camera's: every drawn star with its eclipses and the
//! air on the line to it (an orbital sunset reddens and darkens the whole
//! craft at once), planetshine from the significant reflectors as patches
//! of what the craft sees of them (`lighting::rules::shine_patches`: the
//! lit area, its light and its angle to the craft), the sky light of the
//! craft's own air,
//! the starlight without a light slot, and shadows from the craft's own
//! cascade (`lighting::shadows`). A material over Bevy's standard one
//! (`ship.wgsl`), whose per-frame data lives in a storage buffer per ship
//! so the material itself is never modified per frame.

use crate::lighting::rules::{ObjectLight, MAX_SHINES, MAX_STARS, SHINE_PATCHES};
use bevy::asset::embedded_asset;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::{ShaderDefVal, ShaderRef};
use glam::DVec3;

pub type ShipMaterial = ExtendedMaterial<StandardMaterial, ShipExt>;

const SHADER: &str = "embedded://game/ship.wgsl";

/// One ship's light this frame (`ship.wgsl`'s `ShipLight`).
#[derive(Clone, Copy, Debug, PartialEq, ShaderType, Reflect)]
pub struct ShipLight {
    /// Per light slot: xyz the direction to its star from the camera (the
    /// slot's Bevy light's), w the share of that light the ship receives
    /// (its flux here over its flux at the camera, eclipses included).
    pub stars: [Vec4; MAX_STARS],
    /// Per light slot: rgb the transmittance of the air between the ship
    /// and the star.
    pub star_air: [Vec4; MAX_STARS],
    /// Planetshine: per reflector, its patches (xyz: the direction scaled
    /// by the irradiance a face turned to it takes, lux;
    /// `lighting::rules::ShinePatches`).
    pub shine: [Vec4; MAX_SHINES * SHINE_PATCHES],
    /// Per reflector: rgb the colour of its light (luminance 1); w 0 when
    /// there is none.
    pub shine_tint: [Vec4; MAX_SHINES],
    /// x: starlight without a light slot, sphere-averaged (lux); y: the
    /// sky's radiance around the ship (Bevy's ambient units,
    /// `sky::sky_radiance_at`); z: the ship's cascade of the craft shadow
    /// light (`lighting::shadows`; −1: none this frame).
    pub ambient: Vec4,
}

impl Default for ShipLight {
    fn default() -> Self {
        ShipLight {
            stars: [Vec4::ZERO; MAX_STARS],
            star_air: [Vec4::ONE; MAX_STARS],
            shine: [Vec4::ZERO; MAX_SHINES * SHINE_PATCHES],
            shine_tint: [Vec4::ZERO; MAX_SHINES],
            ambient: Vec4::new(0.0, 0.0, -1.0, 0.0),
        }
    }
}

/// The shader data from a ship's [`ObjectLight`], with the directions to
/// the slotted stars from the camera (`star_dirs`, in slot order) and the
/// sky's radiance around it.
pub fn ship_light(light: &ObjectLight, star_dirs: &[DVec3], sky: f32) -> ShipLight {
    let mut s = ShipLight::default();
    for (k, (dir, scale)) in star_dirs.iter().zip(&light.scale).enumerate().take(MAX_STARS) {
        s.stars[k] = dir.as_vec3().extend(*scale as f32);
        let [r, g, b] = light.air.get(k).copied().unwrap_or([1.0; 3]).map(|c| c as f32);
        s.star_air[k] = Vec4::new(r, g, b, 1.0);
    }
    for (m, sh) in light.shines.iter().enumerate().take(MAX_SHINES) {
        for (p, v) in sh.patches.iter().enumerate() {
            s.shine[m * SHINE_PATCHES + p] = v.as_vec3().extend(0.0);
        }
        let [r, g, b] = sh.tint.map(|c| c as f32);
        s.shine_tint[m] = Vec4::new(r, g, b, 1.0);
    }
    s.ambient = Vec4::new(light.ambient as f32, sky, -1.0, 0.0);
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

    /// Bevy's atmosphere dims and reddens a directional light per fragment
    /// with the air above that fragment and hides it below the horizon at
    /// that fragment (its "atmosphere occlusion"); a craft takes the air on
    /// its own line to the star, once for all of it (`ShipLight::star_air`).
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(f) = descriptor.fragment.as_mut() {
            f.shader_defs.retain(|d| !matches!(d, ShaderDefVal::Bool(name, _) if name == "ATMOSPHERE"));
        }
        Ok(())
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
    use crate::lighting::rules::ShinePatches;

    #[test]
    fn the_shader_data_holds_every_slot_and_patch() {
        let mut patches = [DVec3::ZERO; SHINE_PATCHES];
        patches[0] = DVec3::new(0.0, 0.0, -3.0);
        patches[SHINE_PATCHES - 1] = DVec3::X;
        let light = ObjectLight {
            scale: vec![1.0, 0.25],
            air: vec![[0.9, 0.5, 0.1], [1.0; 3]],
            shines: vec![ShinePatches { patches, tint: [1.1, 1.0, 0.8] }],
            ambient: 0.5,
        };
        let s = ship_light(&light, &[DVec3::X, DVec3::Y], 7.0);
        assert_eq!(s.stars[0], Vec4::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(s.stars[1], Vec4::new(0.0, 1.0, 0.0, 0.25));
        assert_eq!(s.stars[2].w, 0.0, "no star in the slot");
        assert_eq!(s.star_air[0], Vec4::new(0.9, 0.5, 0.1, 1.0));
        assert_eq!(s.shine[0], Vec4::new(0.0, 0.0, -3.0, 0.0));
        assert_eq!(s.shine[SHINE_PATCHES - 1], Vec4::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!(s.shine_tint[0], Vec4::new(1.1, 1.0, 0.8, 1.0));
        assert_eq!(s.shine_tint[1].w, 0.0, "one reflector only");
        assert_eq!(s.ambient, Vec4::new(0.5, 7.0, -1.0, 0.0), "no shadow cascade until the craft are grouped");
    }

    #[test]
    fn the_shader_mirrors_the_data() {
        let wgsl = include_str!("ship.wgsl");
        assert!(wgsl.contains(&format!("const MAX_STARS: i32 = {MAX_STARS};")));
        assert!(wgsl.contains(&format!("const MAX_SHINES: i32 = {MAX_SHINES};")));
        assert!(wgsl.contains(&format!("const SHINE_PATCHES: i32 = {SHINE_PATCHES};")));
        assert!(wgsl.contains(&format!("shine: array<vec4<f32>, {}>", MAX_SHINES * SHINE_PATCHES)));
    }
}
