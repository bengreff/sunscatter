//! The lattice of the terrain shader's value noise as a 3D texture
//! (`terrain.wgsl`: `lattice_noise2`): two random channels per cell, read
//! with the hardware's trilinear filter at a coordinate smoothstepped within
//! its cell, which is the same C1 value noise as hashing the cell's 8
//! corners, at one fetch instead of 8 integer hashes (the zones' warp and
//! pattern took ~9.6 ms of a 38 ms frame at 3024×1964, 2026-10-26). The
//! lattice repeats every [`SIZE`] cells; the shader turns each octave by a
//! fixed rotation so the octaves' periods never line up, and keeps the
//! hashed noise for each sum's first, coarsest octave.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// Cells along each side (2 bytes each: 4 MiB).
pub const SIZE: u32 = 128;

/// The integer lattice hash of `terrain.wgsl` (`hash3`).
pub fn hash3(x: i32, y: i32, z: i32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343)
        ^ (y as u32).wrapping_mul(0xd816_3841)
        ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h = (h ^ (h >> 15)).wrapping_mul(0x2c1b_3c6d);
    h = (h ^ (h >> 12)).wrapping_mul(0x297a_2d39);
    h ^ (h >> 15)
}

/// The two values (0..=255) of lattice cell (x, y, z).
pub fn cell(x: u32, y: u32, z: u32) -> [u8; 2] {
    let h = hash3(x as i32, y as i32, z as i32);
    [(h >> 8) as u8, (h >> 24) as u8]
}

/// The lattice texture: `SIZE`³ cells, RG8, repeating, filtered linearly.
pub fn image() -> Image {
    let mut data = Vec::with_capacity((SIZE * SIZE * SIZE * 2) as usize);
    for z in 0..SIZE {
        for y in 0..SIZE {
            for x in 0..SIZE {
                data.extend_from_slice(&cell(x, y, z));
            }
        }
    }
    let mut image = Image::new(
        Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: SIZE },
        TextureDimension::D3,
        data,
        TextureFormat::Rg8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lattice_is_uniform_and_its_channels_independent() {
        let n = 32u32;
        let (mut sum, mut diff, mut count) = ([0.0f64; 2], 0.0f64, 0.0);
        let mut seen = [[false; 256]; 2];
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    let [a, b] = cell(x, y, z);
                    sum[0] += f64::from(a);
                    sum[1] += f64::from(b);
                    diff += (f64::from(a) - f64::from(b)).abs();
                    seen[0][a as usize] = true;
                    seen[1][b as usize] = true;
                    count += 1.0;
                }
            }
        }
        // Means near 127.5, every value taken, the channels unrelated (the
        // mean |a − b| of two independent uniform bytes is ~85).
        for c in 0..2 {
            assert!((sum[c] / count - 127.5).abs() < 2.0, "channel {c}: {}", sum[c] / count);
            assert!(seen[c].iter().all(|&s| s), "channel {c} misses values");
        }
        assert!((diff / count - 85.3).abs() < 2.0, "{}", diff / count);
        // Neighbouring cells differ (no structure along an axis).
        assert_ne!(cell(0, 0, 0), cell(1, 0, 0));
        assert_eq!(cell(5, 6, 7), cell(5, 6, 7));
        assert_eq!(image().texture_descriptor.size.depth_or_array_layers, SIZE);
    }
}
