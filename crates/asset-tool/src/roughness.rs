//! Roughness maps (D059): how rough the real terrain is just above the
//! heightmap's resolution, which scales the simulation's procedural detail
//! below it (`sim::terrain::detail`).
//!
//! Method, from the committed `height.png`:
//! 1. smooth the heights with a separable 5-tap binomial filter `[1 4 6 4 1]/16`
//!    (longitude wraps, latitude clamps);
//! 2. the residual is height − smoothed;
//! 3. each output texel is the RMS residual of a [`BLOCK`]×[`BLOCK`] block of samples.
//!
//! Output: `roughness.png`, 8-bit grayscale, the grid convention of
//! [`crate::grid`] at `1/BLOCK` of the heightmap's width. **Encoding:**
//! `rms_m = value² / 64` (square-root encoding: 1/64 m steps near zero, about
//! 8 m steps near the top, maximum 1,016 m at 255). `sim::terrain` decodes it.

use std::path::Path;

use crate::grid::HeightMap;
use crate::water::write_mask_png;

/// Heightmap samples per roughness texel along each axis.
pub const BLOCK: usize = 4;

/// Encodes an RMS residual (m) as the stored byte.
pub fn encode(rms_m: f64) -> u8 {
    (8.0 * rms_m.max(0.0).sqrt()).round().min(255.0) as u8
}

/// Decodes a stored byte to metres (the inverse of [`encode`] on its grid).
#[cfg_attr(not(test), allow(dead_code))] // The documented contract; tests pin it.
pub fn decode(v: u8) -> f64 {
    let v = f64::from(v);
    v * v / 64.0
}

/// Separable `[1 4 6 4 1]/16` smoothing of a `w × h` grid (longitude wraps, latitude clamps).
fn smooth(z: &[f32], w: usize, h: usize) -> Vec<f32> {
    const K: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];
    let mut tmp = vec![0.0f32; w * h];
    for j in 0..h {
        for i in 0..w {
            let s: f32 = (0..5).map(|k| K[k] * z[j * w + (i + w + k - 2) % w]).sum();
            tmp[j * w + i] = s / 16.0;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for j in 0..h {
        for i in 0..w {
            let s: f32 = (0..5).map(|k| K[k] * tmp[(j + k).saturating_sub(2).min(h - 1) * w + i]).sum();
            out[j * w + i] = s / 16.0;
        }
    }
    out
}

/// Roughness texels (RMS residual, m) of a `w × h` height grid; `w` and `h`
/// must be multiples of [`BLOCK`].
pub fn rms_blocks(z: &[f32], w: usize, h: usize) -> Vec<f64> {
    assert!(w.is_multiple_of(BLOCK) && h.is_multiple_of(BLOCK), "grid not a multiple of {BLOCK}");
    let s = smooth(z, w, h);
    let (bw, bh) = (w / BLOCK, h / BLOCK);
    let mut out = vec![0.0; bw * bh];
    for (k, o) in out.iter_mut().enumerate() {
        let (bi, bj) = (k % bw, k / bw);
        let mut sum = 0.0f64;
        for j in bj * BLOCK..(bj + 1) * BLOCK {
            for i in bi * BLOCK..(bi + 1) * BLOCK {
                let d = f64::from(z[j * w + i] - s[j * w + i]);
                sum += d * d;
            }
        }
        *o = (sum / (BLOCK * BLOCK) as f64).sqrt();
    }
    out
}

/// Bakes `data/bodies/<body>/roughness.png` from the committed `height.png`.
pub fn bake(body: &str) {
    let dir = Path::new("data/bodies").join(body);
    println!("{body} roughness (from height.png)");
    let m = HeightMap::load(&dir.join("height.png"));
    let z: Vec<f32> = (0..m.w * m.h).map(|k| m.at(k % m.w, k / m.w) as f32).collect();
    let rms = rms_blocks(&z, m.w, m.h);
    let (w, h) = (m.w / BLOCK, m.h / BLOCK);
    let bytes: Vec<u8> = rms.iter().map(|&r| encode(r)).collect();
    let mut sorted = rms.clone();
    sorted.sort_by(f64::total_cmp);
    let pct = |p: f64| sorted[((sorted.len() - 1) as f64 * p) as usize];
    println!("  RMS residual: median {:.1} m, 99 % {:.1} m, max {:.1} m", pct(0.5), pct(0.99), pct(1.0));
    let path = dir.join("roughness.png");
    write_mask_png(&path, w, h, &bytes);
    let size = std::fs::metadata(&path).expect("stat output").len();
    println!("  wrote {} ({w}×{h}, {:.2} MB)", path.display(), size as f64 / 1e6);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_round_trips_on_its_grid() {
        for v in 0..=255u8 {
            assert_eq!(encode(decode(v)), v);
        }
        assert_eq!(decode(8), 1.0);
        assert_eq!(decode(255), 255.0 * 255.0 / 64.0);
        assert_eq!(encode(1e6), 255);
        assert_eq!(encode(-1.0), 0);
    }

    #[test]
    fn flat_and_linear_terrain_is_smooth() {
        let (w, h) = (16, 8);
        let flat = vec![120.0f32; w * h];
        assert!(rms_blocks(&flat, w, h).iter().all(|&r| r == 0.0));
        // A north-south ramp (latitude does not wrap): smooth except at the clamped edges.
        let ramp: Vec<f32> = (0..w * h).map(|k| (k / w) as f32 * 10.0).collect();
        let r = rms_blocks(&ramp, w, h);
        assert!(r[(w / BLOCK)..(w / BLOCK) * (h / BLOCK - 1)].iter().all(|&r| r == 0.0), "{r:?}");
    }

    #[test]
    fn rough_blocks_are_rough() {
        // Checkerboard 0/100 m in columns 0..8, flat elsewhere.
        let (w, h) = (32, 8);
        let z: Vec<f32> = (0..w * h).map(|k| if k % w < 8 && (k % w + k / w) % 2 == 0 { 100.0 } else { 0.0 }).collect();
        let r = rms_blocks(&z, w, h);
        assert!(r[0] > 30.0 && r[1] > 30.0, "{r:?}");
        // Far from the rough columns (the filter reaches 2 samples, wrapping): exactly smooth.
        assert!(r[3..=6].iter().all(|&x| x == 0.0) && r[2] < r[0] / 5.0, "{r:?}");
    }
}
