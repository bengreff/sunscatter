//! The terrain material (StandardMaterial plus our fragment shader) and the
//! colour-map loader (the water mask's is in `water`) (decoded, resized to the settings cap and mipmapped on
//! a background task).

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use std::path::PathBuf;

pub type TerrainMaterial = ExtendedMaterial<StandardMaterial, TerrainExt>;

pub const SHADER: &str = "embedded://game/terrain/terrain.wgsl";

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
    /// Lighting (D055, `lighting.rs`). xyz: the star's centre relative to
    /// the camera, w: its radius (m).
    pub star: Vec4,
    /// The body that can eclipse the star here: centre, radius (w = 0: none).
    pub occluder: Vec4,
    /// Planetshine: the reflecting body's centre, w: illuminance (lux) at
    /// this body's centre.
    pub shine: Vec4,
    /// rgb: planetshine colour; w: this body's sunlight relative to the
    /// shared light's (flux at the body / flux at the camera).
    pub light: Vec4,
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
}

impl MaterialExtension for TerrainExt {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
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

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
