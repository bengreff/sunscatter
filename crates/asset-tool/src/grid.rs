//! The committed map format: equirectangular grids and the 16-bit height encoding.
//!
//! Grid convention (shared by `height.png` and `color.jpg`), for a `w × h` image with
//! `w = 2h`:
//! - column `i` is centred on longitude `−180° + (i + 0.5)·360°/w` (east-positive), so
//!   the left edge is −180° and 0° is at the image centre;
//! - row `j` is centred on latitude `+90° − (j + 0.5)·180°/h`, row 0 at the north edge.
//!
//! Height encoding: 16-bit grayscale PNG, `u16 = round(height_m) + 32768`, i.e. signed
//! metres at 1 m resolution, range −32768..=32767 m.

use std::path::Path;

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

pub const HEIGHT_OFFSET: i32 = 32768;

/// Encodes a height in metres as the stored `u16` (clamped to the representable range).
pub fn encode_height(h_m: f32) -> u16 {
    let v = h_m.round() as i32 + HEIGHT_OFFSET;
    v.clamp(0, i32::from(u16::MAX)) as u16
}

/// Decodes a stored `u16` back to signed metres.
pub fn decode_height(v: u16) -> i32 {
    i32::from(v) - HEIGHT_OFFSET
}

/// Longitude and latitude (degrees) of the centre of pixel `(i, j)`.
#[cfg_attr(not(test), allow(dead_code))] // The documented convention; tests pin it.
pub fn pixel_center(i: usize, j: usize, w: usize, h: usize) -> (f64, f64) {
    let lon = -180.0 + (i as f64 + 0.5) * 360.0 / w as f64;
    let lat = 90.0 - (j as f64 + 0.5) * 180.0 / h as f64;
    (lon, lat)
}

/// The pixel containing `(lon, lat)` in degrees (longitude wraps, latitude clamps).
pub fn pixel_of(lon: f64, lat: f64, w: usize, h: usize) -> (usize, usize) {
    let x = ((lon + 180.0) / 360.0).rem_euclid(1.0) * w as f64;
    let y = (90.0 - lat) / 180.0 * h as f64;
    ((x.floor() as usize).min(w - 1), (y.floor().max(0.0) as usize).min(h - 1))
}

/// Writes a 16-bit grayscale PNG with maximum deflate compression.
pub fn write_height_png(path: &Path, w: usize, h: usize, data: &[u16]) {
    assert_eq!(data.len(), w * h);
    let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    let enc =
        PngEncoder::new_with_quality(std::io::BufWriter::new(file), CompressionType::Level(9), FilterType::Adaptive);
    enc.write_image(&bytes, w as u32, h as u32, ExtendedColorType::L16).expect("encode PNG");
}

/// A decoded committed heightmap, for spot checks.
pub struct HeightMap {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u16>,
}

impl HeightMap {
    pub fn load(path: &Path) -> Self {
        let img = image::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display())).into_luma16();
        let (w, h) = (img.width() as usize, img.height() as usize);
        Self { w, h, data: img.into_raw() }
    }

    pub fn at(&self, i: usize, j: usize) -> i32 {
        decode_height(self.data[j * self.w + i])
    }

    /// Height of the pixel containing `(lon, lat)`.
    pub fn sample(&self, lon: f64, lat: f64) -> i32 {
        let (i, j) = pixel_of(lon, lat, self.w, self.h);
        self.at(i, j)
    }

    /// Minimum and maximum within `r` pixels of the pixel containing `(lon, lat)`.
    pub fn min_max_near(&self, lon: f64, lat: f64, r: isize) -> (i32, i32) {
        let (i0, j0) = pixel_of(lon, lat, self.w, self.h);
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for dj in -r..=r {
            let j = (j0 as isize + dj).clamp(0, self.h as isize - 1) as usize;
            for di in -r..=r {
                let i = (i0 as isize + di).rem_euclid(self.w as isize) as usize;
                let v = self.at(i, j);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        (lo, hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_encoding_round_trips() {
        for h in [-11000.0f32, -1.4, -0.5, 0.0, 0.49, 1.0, 8848.7, 32767.0, -32768.0] {
            assert_eq!(decode_height(encode_height(h)), h.round() as i32);
        }
        assert_eq!(encode_height(0.0), 32768);
        assert_eq!(encode_height(1e6), u16::MAX);
        assert_eq!(encode_height(-1e6), 0);
    }

    #[test]
    fn grid_convention() {
        let (w, h) = (8, 4);
        assert_eq!(pixel_center(0, 0, w, h), (-157.5, 67.5));
        assert_eq!(pixel_center(7, 3, w, h), (157.5, -67.5));
        // Centre of the image straddles lon 0 / lat 0.
        assert_eq!(pixel_center(4, 2, w, h), (22.5, -22.5));
        for j in 0..h {
            for i in 0..w {
                let (lon, lat) = pixel_center(i, j, w, h);
                assert_eq!(pixel_of(lon, lat, w, h), (i, j));
            }
        }
        assert_eq!(pixel_of(-180.0, 90.0, w, h), (0, 0));
        assert_eq!(pixel_of(180.0, -90.0, w, h), (0, 3)); // wraps to the left edge
        assert_eq!(pixel_of(-80.6, 28.6, 8192, 4096).0, 2261);
    }

    #[test]
    fn png_round_trip() {
        let dir = std::env::temp_dir().join(format!("asset-tool-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("h.png");
        let data: Vec<u16> =
            [-10994.0f32, -1.0, 0.0, 3.0, 8848.0, 400.0, -4000.0, 12.0].iter().map(|&h| encode_height(h)).collect();
        write_height_png(&path, 4, 2, &data);
        let map = HeightMap::load(&path);
        assert_eq!((map.w, map.h), (4, 2));
        assert_eq!(map.data, data);
        assert_eq!(map.at(0, 0), -10994);
        assert_eq!(map.at(0, 1), 8848);
        assert_eq!(map.at(3, 1), 12);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
