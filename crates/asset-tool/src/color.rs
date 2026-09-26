//! sRGB colour maps: linear-light resampling, JPEG output and small previews.

use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};

use crate::resample::resample_area;

pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(l: f32) -> f32 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// Lookup table from an `bits`-bit sRGB code value to linear light.
pub fn linear_lut(bits: u32) -> Vec<f32> {
    let max = ((1u32 << bits) - 1) as f32;
    (0..1u32 << bits).map(|v| srgb_to_linear(v as f32 / max)).collect()
}

fn to_u8(linear: f32) -> u8 {
    (linear_to_srgb(linear) * 255.0).round() as u8
}

/// Resamples an interleaved RGB image (any integer depth, via `lut`) to `dst_w × dst_h`
/// with an area average in linear light, returning 8-bit sRGB.
pub fn resample_rgb<T: Copy + Into<u32>>(
    src: &[T],
    src_w: usize,
    src_h: usize,
    lut: &[f32],
    dst_w: usize,
    dst_h: usize,
) -> Vec<u8> {
    assert_eq!(src.len(), src_w * src_h * 3);
    let out = resample_area(src_w, src_h, 3, dst_w, dst_h, |j, buf| {
        for (b, &v) in buf.iter_mut().zip(&src[j * src_w * 3..(j + 1) * src_w * 3]) {
            *b = lut[v.into() as usize];
        }
    });
    out.into_iter().map(to_u8).collect()
}

pub fn write_jpeg(path: &Path, w: usize, h: usize, rgb: &[u8], quality: u8) {
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("create {}: {e}", path.display()));
    JpegEncoder::new_with_quality(std::io::BufWriter::new(file), quality)
        .write_image(rgb, w as u32, h as u32, ExtendedColorType::Rgb8)
        .expect("encode JPEG");
}

/// Writes a small PNG preview (area-averaged in linear light) for eyeballing.
pub fn write_preview(path: &Path, w: usize, h: usize, rgb: &[u8], preview_w: usize) {
    let ph = preview_w * h / w;
    let small = resample_rgb(rgb, w, h, &linear_lut(8), preview_w, ph);
    image::save_buffer(path, &small, preview_w as u32, ph as u32, ExtendedColorType::Rgb8).expect("write preview");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_round_trip() {
        for v in 0..=255u32 {
            let c = v as f32 / 255.0;
            assert_eq!((linear_to_srgb(srgb_to_linear(c)) * 255.0).round() as u32, v);
        }
    }

    #[test]
    fn averaging_happens_in_linear_light() {
        // Black and white average to linear 0.5, which is sRGB ~188, not 128.
        let src: Vec<u8> = vec![0, 0, 0, 255, 255, 255];
        let out = resample_rgb(&src, 2, 1, &linear_lut(8), 1, 1);
        assert_eq!(out, vec![188, 188, 188]);
    }
}
