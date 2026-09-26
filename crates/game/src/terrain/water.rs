//! The water mask: the sea fraction of the surface in each direction, baked
//! by `asset-tool` (`water.png`) and looked up per fragment in `terrain.wgsl`
//! from the body-fixed direction, like the colour map. Being a function of
//! direction only, it does not depend on which LOD chunk draws the point.
//!
//! [`WaterMask::coverage`] is the CPU mirror of the shader lookup (same uv,
//! bilinear between texel centres, longitude wrapping, latitude clamped), for
//! tests and any CPU user.

use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use glam::DVec3;
use std::f64::consts::PI;
use std::path::Path;

/// An equirectangular sea-fraction grid (0 land … 1 sea), row 0 at the north
/// edge, column 0 starting at −180° (the asset-tool grid convention).
#[derive(Clone, Debug)]
pub struct WaterMask {
    pub w: usize,
    pub h: usize,
    pub data: Vec<f32>,
}

/// Texture coordinates of a body-fixed unit direction (as in the shader).
#[cfg_attr(not(test), allow(dead_code))] // The shader's lookup on the CPU; tests pin it.
pub fn uv(dir: DVec3) -> (f64, f64) {
    let lat = dir.z.clamp(-1.0, 1.0).asin();
    let lon = dir.y.atan2(dir.x);
    ((lon + PI) / (2.0 * PI), (0.5 * PI - lat) / PI)
}

impl WaterMask {
    pub fn load(path: &Path) -> Option<Self> {
        let img = match image::open(path) {
            Ok(i) => i.into_luma8(),
            Err(e) => {
                error!("water mask {}: {e}", path.display());
                return None;
            }
        };
        let (w, h) = (img.width() as usize, img.height() as usize);
        Some(Self { w, h, data: img.into_raw().into_iter().map(|v| f32::from(v) / 255.0).collect() })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn at(&self, i: i64, j: i64) -> f64 {
        let i = i.rem_euclid(self.w as i64) as usize;
        let j = j.clamp(0, self.h as i64 - 1) as usize;
        f64::from(self.data[j * self.w + i])
    }

    /// Sea fraction at a body-fixed unit direction (bilinear, base level).
    #[cfg_attr(not(test), allow(dead_code))] // The shader's lookup on the CPU; tests pin it.
    pub fn coverage(&self, dir: DVec3) -> f64 {
        let (u, v) = uv(dir);
        let x = u * self.w as f64 - 0.5;
        let y = v * self.h as f64 - 0.5;
        let (i, j) = (x.floor(), y.floor());
        let (fx, fy) = (x - i, y - j);
        let (i, j) = (i as i64, j as i64);
        let top = self.at(i, j) * (1.0 - fx) + self.at(i + 1, j) * fx;
        let bottom = self.at(i, j + 1) * (1.0 - fx) + self.at(i + 1, j + 1) * fx;
        top * (1.0 - fy) + bottom * fy
    }

    /// Box-halves until the width is at most `max_width`.
    pub fn downsample_to(mut self, max_width: usize) -> Self {
        while self.w > max_width && self.w > 1 && self.h > 1 {
            self = self.halve();
        }
        self
    }

    fn halve(&self) -> Self {
        let (w, h) = ((self.w / 2).max(1), (self.h / 2).max(1));
        let mut data = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let mut acc = 0.0;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    acc += self.data[(y * 2 + dy).min(self.h - 1) * self.w + (x * 2 + dx).min(self.w - 1)];
                }
                data.push(acc / 4.0);
            }
        }
        Self { w, h, data }
    }

    /// A mipmapped single-channel texture with the colour map's sampler.
    pub fn to_image(&self) -> Image {
        let mut level = self.clone();
        let mut bytes = Vec::new();
        let mut mips = 0;
        loop {
            bytes.extend(level.data.iter().map(|&c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
            mips += 1;
            if level.w == 1 && level.h == 1 {
                break;
            }
            level = level.halve();
        }
        // `Image::new` expects the base level only; the data holds the whole chain.
        let mut image = Image::new_uninit(
            Extent3d { width: self.w as u32, height: self.h as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            TextureFormat::R8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.data = Some(bytes);
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
        image
    }
}

/// Loads a water mask as a texture no wider than `max_width`.
pub fn load_water_map(path: &Path, max_width: u32) -> Option<Image> {
    Some(WaterMask::load(path)?.downsample_to(max_width as usize).to_image())
}

#[cfg(test)]
mod tests {
    use super::super::mesh::{build, direction, ChunkKey, Shape, GRID};
    use super::*;
    use std::sync::Arc;

    /// A mask with a sharp, jagged coastline through longitude 0 (sea to the
    /// east), so any change of the looked-up direction would show.
    fn coast_mask() -> WaterMask {
        let (w, h) = (256, 128);
        let data = (0..w * h).map(|k| f32::from(u8::from(k % w + (k / w) % 3 >= w / 2))).collect();
        WaterMask { w, h, data }
    }

    fn earthlike() -> Shape {
        // Rugged terrain around sea level: under the old per-vertex rule each
        // level sampled different points and flipped between land and water.
        let heights = Arc::new(|d: DVec3| 300.0 * (d.x * 900.0).sin() * (d.y * 700.0).cos() - 20.0);
        Shape {
            radius_eq: 6.378e6,
            radius_polar: 6.357e6,
            height: Some(heights),
            sea_level: Some(0.0),
            detail_scale: 40.0,
        }
    }

    /// The direction the shader looks up for the point at chunk grid
    /// coordinates `(s, t)` ∈ [0, 1]²: the flat triangle's position
    /// (interpolated between mesh vertices), relative to the body centre.
    fn fragment_dir(key: ChunkKey, shape: &Shape, s: f64, t: f64) -> DVec3 {
        let c = build(key, shape);
        let n = (GRID - 1) as f64;
        let (x, y) = (s * n, t * n);
        let (i, j) = ((x.floor() as usize).min(GRID - 2), (y.floor() as usize).min(GRID - 2));
        let (fx, fy) = (x - i as f64, y - j as f64);
        let p = |i: usize, j: usize| DVec3::from_array(c.positions[j * GRID + i].map(f64::from)) + c.center;
        // Bilinear over the quad; within a triangle this is within the quad's
        // flatness of the rasterised point, far below a mask texel.
        let top = p(i, j) * (1.0 - fx) + p(i + 1, j) * fx;
        let bottom = p(i, j + 1) * (1.0 - fx) + p(i + 1, j + 1) * fx;
        (top * (1.0 - fy) + bottom * fy).normalize()
    }

    #[test]
    fn lookup_is_independent_of_lod_level() {
        let mask = coast_mask();
        let shape = earthlike();
        // A point on face 0 near the synthetic coastline, drawn by one chunk at
        // each level from 2 to 14.
        let (a, b) = (0.005, 0.004_1);
        let exact = mask.coverage(direction(0, a, b));
        assert!(exact > 0.05 && exact < 0.95, "test point should sit on the coast: {exact}");
        for level in 2..=14u8 {
            let n = f64::from(1u32 << level);
            let (fa, fb) = ((a + 1.0) / 2.0 * n, (b + 1.0) / 2.0 * n);
            let key = ChunkKey { face: 0, level, x: fa.floor() as u32, y: fb.floor() as u32 };
            let got = mask.coverage(fragment_dir(key, &shape, fa.fract(), fb.fract()));
            // Only the flat triangles' tiny tangential offsets differ between
            // levels; the old per-vertex flag flipped between 0 and 1 here.
            assert!((got - exact).abs() < 0.01, "level {level}: {got} vs {exact}");
        }
    }

    #[test]
    fn uv_follows_the_grid_convention() {
        let close = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12;
        assert!(close(uv(DVec3::X), (0.5, 0.5))); // lon 0, lat 0: image centre
        assert!(close(uv(DVec3::Y), (0.75, 0.5))); // 90° E
        assert!(close(uv(DVec3::Z), (0.5, 0.0))); // north pole: top row
                                                  // Texel centres give back their stored values.
        let m = WaterMask { w: 4, h: 2, data: vec![0.0, 0.25, 0.5, 1.0, 0.1, 0.2, 0.3, 0.4] };
        let lon = (-180.0f64 + 2.5 * 90.0).to_radians();
        let lat = 45.0f64.to_radians();
        let dir = DVec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin());
        assert!((m.coverage(dir) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn coverage_wraps_across_the_date_line() {
        let m = WaterMask { w: 4, h: 1, data: vec![1.0, 0.0, 0.0, 0.0] };
        // At −180° we are halfway between the last and first texel centres.
        assert!((m.coverage(-DVec3::X) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn mips_average() {
        let m = WaterMask { w: 4, h: 2, data: vec![1.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0] };
        let img = m.clone().to_image();
        assert_eq!(img.texture_descriptor.mip_level_count, 3);
        assert_eq!(img.data.as_ref().unwrap().len(), 8 + 2 + 1);
        assert_eq!(m.downsample_to(2).data, vec![0.75, 0.0]);
    }
}
