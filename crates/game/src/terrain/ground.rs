//! Ground textures: tiling colour and normal maps (CC0, ambientCG; see
//! `data/textures/terrain/LICENSE.md`) that give terrain contrast close up.
//! The shader picks layers per fragment from the colour map, slope, snow
//! line and coast, and uses each texture *relative to its mean colour*, so
//! the satellite colours stay right and only the fine detail comes from the
//! texture. Loaded once into two mipmapped 2D texture arrays.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use std::path::{Path, PathBuf};

/// The layers, in array order (the shader indexes them by these numbers).
pub const LAYERS: [&str; 5] = ["grass", "soil", "sand", "rock", "snow"];

/// Texture size used (px); the files are this size.
const SIZE: u32 = 1024;

pub struct GroundTextures {
    pub color: Image,
    pub normal: Image,
    /// Mean linear colour of each colour layer (the shader divides by it).
    pub means: [Vec4; 5],
}

fn dir() -> PathBuf {
    PathBuf::from(format!("{}/../../data/textures/terrain", env!("CARGO_MANIFEST_DIR")))
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Box-filters an RGBA8 level to half size. With `srgb`, averages in linear
/// light.
fn halve(level: &[u8], size: u32, srgb: bool) -> Vec<u8> {
    let (s, h) = (size as usize, (size / 2) as usize);
    let lin = |v: u8| if srgb { srgb_to_linear(f32::from(v) / 255.0) } else { f32::from(v) / 255.0 };
    let enc = |v: f32| {
        let v = if srgb {
            if v <= 0.003_130_8 {
                v * 12.92
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }
        } else {
            v
        };
        (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
    };
    let mut out = Vec::with_capacity(h * h * 4);
    for y in 0..h {
        for x in 0..h {
            for c in 0..4 {
                let p = |dx: usize, dy: usize| lin(level[((2 * y + dy) * s + 2 * x + dx) * 4 + c]);
                out.push(enc((p(0, 0) + p(1, 0) + p(0, 1) + p(1, 1)) * 0.25));
            }
        }
    }
    out
}

/// One layer's full mip chain (RGBA8, base first).
fn mip_chain(path: &Path, srgb: bool) -> Option<Vec<u8>> {
    let img = match image::open(path) {
        Ok(i) => i.resize_exact(SIZE, SIZE, image::imageops::FilterType::Triangle).to_rgba8(),
        Err(e) => {
            error!("ground texture {}: {e}", path.display());
            return None;
        }
    };
    let mut level = img.into_raw();
    let mut size = SIZE;
    let mut chain = level.clone();
    while size > 1 {
        level = halve(&level, size, srgb);
        size /= 2;
        chain.extend_from_slice(&level);
    }
    Some(chain)
}

fn array_image(layers: Vec<Vec<u8>>, format: TextureFormat) -> Image {
    let mips = SIZE.ilog2() + 1;
    // Layer-major: each layer's whole mip chain in turn (Bevy's default order).
    let mut image = Image::new_uninit(
        Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: layers.len() as u32 },
        TextureDimension::D2,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(layers.concat());
    image.texture_descriptor.mip_level_count = mips;
    image.texture_view_descriptor =
        Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::D2Array), ..default() });
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    image
}

/// Mean linear colour of a layer (from its 1×1 mip).
fn mean(chain: &[u8]) -> Vec4 {
    let n = chain.len();
    let px = &chain[n - 4..];
    Vec4::new(
        srgb_to_linear(f32::from(px[0]) / 255.0),
        srgb_to_linear(f32::from(px[1]) / 255.0),
        srgb_to_linear(f32::from(px[2]) / 255.0),
        1.0,
    )
}

/// Loads every layer; `None` (and an error logged) if a file is missing.
pub fn load() -> Option<GroundTextures> {
    let d = dir();
    let mut colors = Vec::new();
    let mut normals = Vec::new();
    let mut means = [Vec4::ONE; 5];
    for (i, name) in LAYERS.iter().enumerate() {
        let c = mip_chain(&d.join(format!("{name}_color.jpg")), true)?;
        means[i] = mean(&c);
        colors.push(c);
        normals.push(mip_chain(&d.join(format!("{name}_normal.jpg")), false)?);
    }
    Some(GroundTextures {
        color: array_image(colors, TextureFormat::Rgba8UnormSrgb),
        normal: array_image(normals, TextureFormat::Rgba8Unorm),
        means,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halving_averages_and_keeps_linear_light_for_colour() {
        // 2x2 of black and white: linear mean 0.5, which is ~188 in sRGB.
        let level = [0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255];
        assert_eq!(halve(&level, 2, false)[0], 128);
        let srgb = halve(&level, 2, true)[0];
        assert!((186..=189).contains(&srgb), "{srgb}");
    }

    #[test]
    fn shipped_layers_load_with_full_mip_chains() {
        let g = load().expect("ground textures");
        let expected: usize = (0..=SIZE.ilog2()).map(|m| ((SIZE >> m) * (SIZE >> m) * 4) as usize).sum();
        assert_eq!(g.color.data.as_ref().unwrap().len(), expected * LAYERS.len());
        assert_eq!(g.normal.texture_descriptor.size.depth_or_array_layers, 5);
        // Grass is green on average, snow bright.
        assert!(g.means[0].y > g.means[0].x && g.means[0].y > g.means[0].z);
        assert!(g.means[4].x > 0.4);
    }
}
