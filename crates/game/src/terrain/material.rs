//! The terrain material (StandardMaterial plus our fragment shader) and the
//! colour-map loader (decoded, resized to the settings cap and mipmapped on
//! a background task; the water mask's is in `water`, the zones' in `zones`).

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexFormat};
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, TextureDimension,
    TextureFormat,
};
use bevy::shader::ShaderDefVal;
use bevy::shader::ShaderRef;
use std::path::PathBuf;

use super::zones::MAX_ZONES;

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExt>;

pub const SHADER: &str = "embedded://game/terrain/terrain.wgsl";

/// Per-vertex offset to the parent level's surface (geomorphing, see
/// `mesh::parent_offsets`); shader location 8.
pub const ATTRIBUTE_MORPH: MeshVertexAttribute =
    MeshVertexAttribute::new("TerrainMorph", 0x5375_6e73_6361_7401, VertexFormat::Float32x3);

/// A chunk's morph factor travels in its `MeshTag` as factor × this.
pub const MORPH_TAG_SCALE: f32 = 1000.0;

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct TerrainParams {
    pub fixed_x: Vec4,
    pub fixed_y: Vec4,
    pub fixed_z: Vec4,
    pub center: Vec4,
    pub shape: Vec4,
    pub rock: Vec4,
    pub snow: Vec4,
    pub ocean: Vec4,
    pub flags: UVec4,
    pub base: Vec4,
    /// Lighting (D055, `lighting::LightUniform`): the drawn stars, their
    /// light at this body, the bodies that can eclipse them, planetshine.
    pub starlight: crate::lighting::LightUniform,
    /// Ground textures (`ground.rs`): x > 0 when loaded; y, z: fine and
    /// coarse tile sizes in detail-uv units.
    pub ground: Vec4,
    /// Mean linear colour of each ground layer.
    pub ground_means: [Vec4; 5],
    /// Surface zones (`zones`): x 1 when drawn, y the warp's first
    /// wavelength (m).
    pub zones: Vec4,
    /// rgb: shallow-water albedo (linear), w 1 when set.
    pub shallow: Vec4,
    /// Zone palette (`zones::Palette`): rgb drawn colour, w 1 for water.
    pub zone_color: [Vec4; MAX_ZONES],
    /// Zone pattern: x scale (m), y contrast.
    pub zone_pattern: [Vec4; MAX_ZONES],
    /// x: the share of the global ambient light this body receives
    /// (`sky::ambient_share`: the daylight sky stand-in lights only the body
    /// whose air the camera is in).
    pub ambient: Vec4,
    /// Cloud detail on a colour map (giants, `CloudsDef`): x the first
    /// wavelength (m, 0 = off), y the warp's amplitude as a fraction of it,
    /// z the east-west stretch, w the tone's contrast.
    pub clouds: Vec4,
    /// Under a hazy atmosphere (`lighting::HazeLight`): rgb the vertical
    /// optical depth, w the radius over the scale height (0 = no haze).
    pub haze: Vec4,
    /// rgb: the diffuse fraction for an overhead Sun (w unused).
    pub haze_diffuse: Vec4,
    /// Rings (`rings`): x inner, y outer radius (m), z the profile's bins (0: none).
    pub rings: Vec4,
    /// Under an opaque deck (`clouds`): rgb the gas's extinction at the
    /// datum (m⁻¹), w its scale height (m).
    pub fog: Vec4,
    /// rgb: the sky's radiance the gas scatters towards the camera
    /// (lux/sr); w 1 when the camera is below the deck (0: no fog).
    pub fog_light: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct TerrainExt {
    #[uniform(100)]
    pub params: TerrainParams,
    #[texture(101)]
    #[sampler(102)]
    pub color_map: Option<Handle<Image>>,
    /// Sea fraction (`water::WaterMask`), looked up per fragment.
    #[texture(103)]
    #[sampler(104)]
    pub water_map: Option<Handle<Image>>,
    /// Ground layers (colour, normal), as 2D texture arrays.
    #[texture(105, dimension = "2d_array")]
    #[sampler(106)]
    pub ground_color: Option<Handle<Image>>,
    #[texture(107, dimension = "2d_array")]
    #[sampler(108)]
    pub ground_normal: Option<Handle<Image>>,
    /// Zone indices (`zones::ZoneMap`, sea filled), read with `textureLoad`.
    /// No samplers of their own: the fragment stage allows 16, and High's
    /// view bindings plus these would be 17 (a crash, 2026-10-25).
    #[texture(109)]
    pub zone_map: Option<Handle<Image>>,
    /// Zone colours and sea-floor shallowness, mipmapped
    /// (`zones::far_image`), sampled with `color_map`'s sampler.
    #[texture(110)]
    pub zone_far: Option<Handle<Image>>,
    /// The rings' profile (`rings`: rgb albedo, a optical depth per bin),
    /// read with `textureLoad` for their shadow on the planet.
    #[texture(111)]
    pub ring_profile: Option<Handle<Image>>,
}

impl MaterialExtension for TerrainExt {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    /// The main pass uses our vertex shader, which reads the morph offset.
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let prepass = descriptor
            .vertex
            .shader_defs
            .iter()
            .any(|d| matches!(d, ShaderDefVal::Bool(n, true) if n == "PREPASS_PIPELINE"));
        if !prepass {
            descriptor.vertex.buffers = vec![layout.0.get_layout(&[
                Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
                Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
                Mesh::ATTRIBUTE_COLOR.at_shader_location(5),
                ATTRIBUTE_MORPH.at_shader_location(8),
            ])?];
        }
        Ok(())
    }

    fn deferred_fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Decodes an sRGB colour map, box-downsamples it until its width is at most
/// `max_width`, and builds the full mip chain (averaging in linear light).
pub fn load_color_map(path: PathBuf, max_width: u32) -> Option<Image> {
    let img = match image::open(&path) {
        Ok(i) => i.to_rgb8(),
        Err(e) => {
            error!("colour map {}: {e}", path.display());
            return None;
        }
    };
    let (mut w, mut h) = img.dimensions();
    let lut: Vec<f32> = (0..256).map(|v| srgb_to_linear(v as f32 / 255.0)).collect();
    let mut level: Vec<[f32; 3]> =
        img.pixels().map(|p| [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize]]).collect();
    while w > max_width && w > 1 && h > 1 {
        (level, w, h) = halve(&level, w, h);
    }
    let (w0, h0) = (w, h);
    let mut data = Vec::new();
    let mut mips = 0;
    loop {
        data.extend(level.iter().flat_map(|c| {
            let [r, g, b] = c.map(|x| (linear_to_srgb(x) * 255.0 + 0.5) as u8);
            [r, g, b, 255]
        }));
        mips += 1;
        if w == 1 && h == 1 {
            break;
        }
        (level, w, h) = halve(&level, w, h);
    }
    let mut image = Image::new(
        Extent3d { width: w0, height: h0, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = mips;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    Some(image)
}

fn halve(src: &[[f32; 3]], w: u32, h: u32) -> (Vec<[f32; 3]>, u32, u32) {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = Vec::with_capacity((nw * nh) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let mut acc = [0.0f32; 3];
            let mut n = 0.0;
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                let c = src[(sy * w + sx) as usize];
                for k in 0..3 {
                    acc[k] += c[k];
                }
                n += 1.0;
            }
            out.push(acc.map(|v| v / n));
        }
    }
    (out, nw, nh)
}

pub(super) fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
