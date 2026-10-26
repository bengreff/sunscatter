//! Global equirectangular sources other than Earth's and the Moon's: USGS
//! Astrogeology GeoTIFF mosaics and DEMs, and raw PDS grids. Each is read
//! into a [`Grid`] whose rows come out in our grid convention
//! ([`crate::grid`]): column 0 at −180°, east-positive, row 0 at the north.
//!
//! USGS products give their extent as GeoTIFF tags in degrees or, more
//! often, in metres of an equidistant projection on a sphere: the sphere's
//! radius is then `scale·w / 2π`, and the left edge's longitude is the tie
//! point's `x / radius`. Grids starting at 0° (`clon180`) are rolled by half
//! their width. Only full-globe grids (360° × 180°) are accepted.
//!
//! Masked resampling: source pixels equal to a no-data value do not count in
//! the area average ([`resample_masked`]); output pixels with no valid source
//! are filled from coarser averages ([`fill_gaps`], push-pull), so large
//! unmapped areas (Pluto's far side) become smooth.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use tiff::decoder::{Decoder, DecodingResult, Limits};
use tiff::tags::Tag;
use tiff::ColorType;

use crate::resample::resample_area;

/// Decoded samples, interleaved by channel.
pub enum Samples {
    U8(Vec<u8>),
    U16(Vec<u16>),
    I16(Vec<i16>),
    F32(Vec<f32>),
}

/// A full-globe grid, `w × h` pixels of `channels` samples.
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub channels: usize,
    pub data: Samples,
    /// Source column of our column 0 (the roll that puts −180° first).
    pub shift: usize,
}

impl Grid {
    /// Fills `buf` (`w · channels`) with row `j` as raw sample values, rolled
    /// so column 0 is centred just east of −180°.
    pub fn row(&self, j: usize, buf: &mut [f32]) {
        let (w, c) = (self.w, self.channels);
        assert_eq!(buf.len(), w * c);
        let start = j * w * c;
        let split = self.shift * c;
        let put = |buf: &mut [f32], f: &dyn Fn(usize) -> f32| {
            for (k, b) in buf.iter_mut().enumerate() {
                let src = if k < (w * c - split) { split + k } else { k - (w * c - split) };
                *b = f(start + src);
            }
        };
        match &self.data {
            Samples::U8(d) => put(buf, &|i| f32::from(d[i])),
            Samples::U16(d) => put(buf, &|i| f32::from(d[i])),
            Samples::I16(d) => put(buf, &|i| f32::from(d[i])),
            Samples::F32(d) => put(buf, &|i| d[i]),
        }
    }
}

/// The column shift that puts −180° at column 0, for a grid of width `w`
/// whose left edge is at `left_deg`, rounded to a whole column (grids with
/// an extra column are a fraction of a pixel off; that is reported).
pub fn shift_for(left_deg: f64, w: usize) -> usize {
    let cols = (-180.0 - left_deg) / 360.0 * w as f64;
    let k = cols.round();
    if (cols - k).abs() > 1e-3 {
        println!("    left edge {left_deg}° is {:.2} px off a pixel boundary: rounded", cols - k);
    }
    (k as i64).rem_euclid(w as i64) as usize
}

/// The central meridian (degrees) of a projected GeoTIFF: GeoKey 3088
/// (ProjCenterLong) or 3080 (ProjNatOriginLong), 0 if neither is present.
fn central_meridian<R: std::io::Read + std::io::Seek>(dec: &mut Decoder<R>) -> f64 {
    let Ok(keys) = dec.get_tag_u16_vec(Tag::GeoKeyDirectoryTag) else { return 0.0 };
    let doubles = dec.get_tag_f64_vec(Tag::GeoDoubleParamsTag).unwrap_or_default();
    keys.get(4..)
        .unwrap_or_default()
        .as_chunks::<4>()
        .0
        .iter()
        .find(|k| (k[0] == 3088 || k[0] == 3080) && k[1] == 34736)
        .and_then(|k| doubles.get(usize::from(k[3])).copied())
        .unwrap_or(0.0)
}

/// Reads a full-globe GeoTIFF (see the module docs for the georeferencing).
pub fn read_geotiff(path: &Path) -> Grid {
    println!("  reading {}", path.display());
    let file = File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut dec = Decoder::new(BufReader::new(file)).expect("TIFF header").with_limits(Limits::unlimited());
    let (w, h) = dec.dimensions().expect("TIFF dimensions");
    let (w, h) = (w as usize, h as usize);
    let color = dec.colortype().expect("TIFF colour type");
    let channels = match color {
        ColorType::Gray(_) => 1,
        ColorType::RGB(_) => 3,
        ColorType::RGBA(_) => 4,
        ColorType::Multiband { num_samples, .. } => usize::from(num_samples),
        c => panic!("{}: unsupported colour type {c:?}", path.display()),
    };
    let scale = dec.get_tag_f64_vec(Tag::ModelPixelScaleTag).expect("GeoTIFF pixel scale");
    let tie = dec.get_tag_f64_vec(Tag::ModelTiepointTag).expect("GeoTIFF tie point");
    assert!(tie[0].abs() < 1e-9 && tie[1].abs() < 1e-9, "tie point not at raster (0, 0): {tie:?}");
    // Within two pixels of the globe (some USGS grids carry an extra column).
    let near = |a: f64, b: f64, px: f64| (a - b).abs() <= 2.0 * px;
    let left_deg = if near(scale[0] * w as f64, 360.0, scale[0]) {
        assert!(near(scale[1] * h as f64, 180.0, scale[1]), "{}: not a global grid", path.display());
        tie[3]
    } else {
        let radius = scale[0] * w as f64 / std::f64::consts::TAU;
        assert!(
            near(scale[1] * h as f64, std::f64::consts::PI * radius, scale[1]),
            "{}: not a global grid (scale {scale:?})",
            path.display()
        );
        central_meridian(&mut dec) + tie[3] / radius * 180.0 / std::f64::consts::PI
    };
    let shift = shift_for(left_deg, w);
    println!("    {w}×{h} {color:?}, left edge {left_deg:.3}°, roll {shift} columns");
    let planar = dec.get_tag_u32(Tag::PlanarConfiguration).unwrap_or(1) == 2;
    if planar && channels > 1 {
        return Grid { w, h, channels, data: Samples::U8(read_planar_u8(&mut dec, w, h, channels)), shift };
    }
    let data = match dec.read_image().expect("decode TIFF") {
        DecodingResult::U8(d) => Samples::U8(d),
        DecodingResult::U16(d) => Samples::U16(d),
        DecodingResult::I16(d) => Samples::I16(d),
        DecodingResult::F32(d) => Samples::F32(d),
        _ => panic!("{}: unsupported sample type", path.display()),
    };
    Grid { w, h, channels, data, shift }
}

/// Reads a strip-organised planar (band-sequential) 8-bit TIFF into
/// interleaved samples (the decoder's `read_image` returns only the first
/// plane of such files).
fn read_planar_u8<R: std::io::Read + std::io::Seek>(dec: &mut Decoder<R>, w: usize, h: usize, c: usize) -> Vec<u8> {
    assert!(matches!(dec.get_chunk_type(), tiff::decoder::ChunkType::Strip), "planar tiled TIFFs are not supported");
    let n = dec.strip_count().expect("strip count") as usize;
    let per_plane = n / c;
    let rows = dec.chunk_dimensions().1 as usize;
    let mut out = vec![0u8; w * h * c];
    for i in 0..n {
        let (plane, strip) = (i / per_plane, i % per_plane);
        let DecodingResult::U8(d) = dec.read_chunk(i as u32).expect("read strip") else {
            panic!("planar: expected 8-bit samples")
        };
        for (k, &v) in d.iter().enumerate() {
            let (y, x) = (strip * rows + k / w, k % w);
            if y < h {
                out[(y * w + x) * c + plane] = v;
            }
        }
    }
    out
}

/// Reads a raw PDS grid of big-endian `i16` (MOLA MEGDRs) whose left edge is
/// at `left_deg`.
pub fn read_pds_i16_msb(path: &Path, w: usize, h: usize, left_deg: f64) -> Grid {
    println!("  reading {}", path.display());
    let mut bytes = Vec::new();
    File::open(path).and_then(|mut f| f.read_to_end(&mut bytes)).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(bytes.len(), w * h * 2, "{}: unexpected size", path.display());
    let data = bytes.as_chunks::<2>().0.iter().map(|&b| i16::from_be_bytes(b)).collect();
    Grid { w, h, channels: 1, data: Samples::I16(data), shift: shift_for(left_deg, w) }
}

/// Area-averages `grid` to `dst_w × dst_h`, mapping each sample through
/// `value` (`None`: no data). Returns per channel the mean of the valid
/// samples, `NaN` where an output pixel has none.
pub fn resample_masked(grid: &Grid, dst_w: usize, dst_h: usize, value: impl Fn(f32) -> Option<f32>) -> Vec<f32> {
    let c = grid.channels;
    let mut raw = vec![0.0; grid.w * c];
    // Per channel: value·valid and valid.
    let sums = resample_area(grid.w, grid.h, 2 * c, dst_w, dst_h, |j, buf| {
        grid.row(j, &mut raw);
        for (k, &v) in raw.iter().enumerate() {
            let (a, b) = value(v).map_or((0.0, 0.0), |x| (x, 1.0));
            buf[2 * k] = a;
            buf[2 * k + 1] = b;
        }
    });
    sums.as_chunks::<2>().0.iter().map(|p| if p[1] > 1e-6 { p[0] / p[1] } else { f32::NAN }).collect()
}

/// Fills `NaN` pixels of a `w × h` grid with `channels` interleaved channels
/// by push-pull: averages of the valid pixels at successively halved
/// resolutions, read back bilinearly. Valid pixels are unchanged. If no pixel
/// is valid, everything becomes `fallback`.
pub fn fill_gaps(data: &mut [f32], w: usize, h: usize, channels: usize, fallback: f32) {
    if !data.iter().any(|v| v.is_nan()) {
        return;
    }
    if w <= 2 || h <= 1 {
        let c = channels;
        for ch in 0..c {
            let valid: Vec<f32> = data.iter().skip(ch).step_by(c).copied().filter(|v| !v.is_nan()).collect();
            let mean = if valid.is_empty() { fallback } else { valid.iter().sum::<f32>() / valid.len() as f32 };
            data.iter_mut().skip(ch).step_by(c).filter(|v| v.is_nan()).for_each(|v| *v = mean);
        }
        return;
    }
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut coarse = vec![f32::NAN; cw * ch * channels];
    for j in 0..ch {
        for i in 0..cw {
            for c in 0..channels {
                let (mut s, mut n) = (0.0, 0.0);
                for (dj, di) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    let (y, x) = ((2 * j + dj).min(h - 1), (2 * i + di) % w);
                    let v = data[(y * w + x) * channels + c];
                    if !v.is_nan() {
                        s += v;
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    coarse[(j * cw + i) * channels + c] = s / n;
                }
            }
        }
    }
    fill_gaps(&mut coarse, cw, ch, channels, fallback);
    for j in 0..h {
        // Coarse pixel centres sit at fine coordinates 2k + 0.5.
        let y = ((j as f32 - 0.5) / 2.0).clamp(0.0, (ch - 1) as f32);
        let (y0, fy) = (y.floor() as usize, y.fract());
        let y1 = (y0 + 1).min(ch - 1);
        for i in 0..w {
            let x = (i as f32 - 0.5) / 2.0;
            let x0f = x.floor();
            let fx = x - x0f;
            let x0 = (x0f as i64).rem_euclid(cw as i64) as usize;
            let x1 = (x0 + 1) % cw;
            for c in 0..channels {
                let k = (j * w + i) * channels + c;
                if data[k].is_nan() {
                    let at = |yy: usize, xx: usize| coarse[(yy * cw + xx) * channels + c];
                    let top = at(y0, x0) * (1.0 - fx) + at(y0, x1) * fx;
                    let bottom = at(y1, x0) * (1.0 - fx) + at(y1, x1) * fx;
                    data[k] = top * (1.0 - fy) + bottom * fy;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_puts_minus_180_first() {
        assert_eq!(shift_for(-180.0, 8), 0);
        // A 0..360° grid: −180° (= 180°) is half-way along.
        assert_eq!(shift_for(0.0, 8), 4);
        assert_eq!(shift_for(-90.0, 8), 6);
        let g = Grid { w: 4, h: 1, channels: 1, data: Samples::U8(vec![0, 1, 2, 3]), shift: shift_for(0.0, 4) };
        let mut row = vec![0.0; 4];
        g.row(0, &mut row);
        assert_eq!(row, vec![2.0, 3.0, 0.0, 1.0]);
        let rgb = Grid { w: 2, h: 1, channels: 3, data: Samples::U8(vec![1, 2, 3, 4, 5, 6]), shift: 1 };
        let mut row = vec![0.0; 6];
        rgb.row(0, &mut row);
        assert_eq!(row, vec![4.0, 5.0, 6.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn masked_resampling_ignores_no_data() {
        let g = Grid { w: 4, h: 2, channels: 1, data: Samples::I16(vec![10, -1, 30, -1, -1, -1, 50, 70]), shift: 0 };
        let out = resample_masked(&g, 2, 1, |v| (v >= 0.0).then_some(v));
        assert_eq!(out[0], 10.0);
        assert_eq!(out[1], 50.0);
        let none = resample_masked(&g, 4, 2, |v| (v >= 0.0).then_some(v));
        assert!(none[1].is_nan() && none[0] == 10.0);
    }

    #[test]
    fn gaps_fill_smoothly_and_keep_valid_pixels() {
        let (w, h) = (16, 8);
        let mut d: Vec<f32> = (0..w * h).map(|k| if k % w < 8 { (k % w) as f32 } else { f32::NAN }).collect();
        let before = d.clone();
        fill_gaps(&mut d, w, h, 1, 0.0);
        assert!(d.iter().all(|v| v.is_finite()));
        for (a, b) in before.iter().zip(&d) {
            if a.is_finite() {
                assert_eq!(a, b);
            }
        }
        // The filled values stay within the valid range.
        assert!(d.iter().all(|&v| (0.0..=7.0).contains(&v)), "{d:?}");
        let mut all_nan = vec![f32::NAN; 6];
        fill_gaps(&mut all_nan, 3, 2, 1, 5.0);
        assert!(all_nan.iter().all(|&v| v == 5.0));
    }
}
