//! Terrain heights (D047): the committed int16 heightmap, sampled
//! deterministically. The same sampling serves altitude, contact, landing and
//! (in the game) rendering, so the drawn surface is the physical one.
//!
//! **File contract** (`data/bodies/<body>/height.png`): 16-bit grayscale PNG,
//! `u16 = height_m + 32768`, equirectangular with `width = 2 × height`.
//! Pixel centres: column `i` ↔ longitude `−180° + (i + 0.5)·360°/W`
//! (east-positive), row `j` ↔ latitude `+90° − (j + 0.5)·180°/H` (row 0 is
//! north). Longitude wraps; latitude clamps at the poles. Heights are relative
//! to the body's reference surface (sea level for Earth, including negative
//! bathymetry).
//!
//! **Sampling** is Catmull-Rom bicubic between pixel centres, so heights and
//! slopes are continuous (bilinear creases show as facets up close). Within
//! half a row of a pole the value blends (smoothstep) to the mean of the polar
//! row, so the surface is single-valued at the pole itself.
//!
//! Determinism: latitude/longitude come from [`crate::math`] (libm) trig; the
//! lookup and the blends are plain IEEE arithmetic on integer data.

use crate::math;
use glam::DVec3;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Offset between stored `u16` values and heights in metres.
pub const HEIGHT_OFFSET: i32 = 32_768;

/// An equirectangular grid of heights (m), row 0 north, column 0 at −180°.
#[derive(Clone, PartialEq, Eq)]
pub struct Heightmap {
    width: usize,
    height: usize,
    data: Vec<i16>,
    /// Sums of the first (north) and last (south) rows, for the pole blend.
    pole_sums: [i64; 2],
}

impl fmt::Debug for Heightmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Heightmap({}×{})", self.width, self.height)
    }
}

impl Heightmap {
    /// Builds a heightmap from row-major heights (m). Panics if the size
    /// does not match or the grid is empty.
    pub fn from_vec(width: usize, height: usize, data: Vec<i16>) -> Self {
        assert!(width > 0 && height > 0, "empty heightmap");
        assert_eq!(data.len(), width * height, "heightmap size");
        let row_sum = |j: usize| data[j * width..(j + 1) * width].iter().map(|&h| i64::from(h)).sum();
        let pole_sums = [row_sum(0), row_sum(height - 1)];
        Heightmap { width, height, data, pole_sums }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Row-major heights (m).
    pub fn data(&self) -> &[i16] {
        &self.data
    }

    /// Height at pixel `(column, row)` (m).
    pub fn pixel(&self, column: usize, row: usize) -> i16 {
        self.data[row * self.width + column]
    }

    /// Decodes the 16-bit grayscale PNG of the file contract.
    pub fn from_png(bytes: &[u8]) -> Result<Self, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::IDENTITY);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let info = reader.info();
        if info.color_type != png::ColorType::Grayscale || info.bit_depth != png::BitDepth::Sixteen {
            return Err(format!("expected 16-bit grayscale, found {:?} {:?}", info.color_type, info.bit_depth));
        }
        let (width, height) = (info.width as usize, info.height as usize);
        let mut buf = vec![0u8; reader.output_buffer_size().ok_or("image too large")?];
        let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let bytes = &buf[..frame.buffer_size()];
        if bytes.len() != width * height * 2 {
            return Err(format!("unexpected pixel data size {}", bytes.len()));
        }
        // PNG stores 16-bit samples big-endian.
        let data = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&b| (i32::from(u16::from_be_bytes(b)) - HEIGHT_OFFSET) as i16)
            .collect();
        if width == 0 || height == 0 {
            return Err("empty image".into());
        }
        Ok(Heightmap::from_vec(width, height, data))
    }

    /// Encodes to the file contract (for tools and tests).
    pub fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width as u32, self.height as u32);
            enc.set_color(png::ColorType::Grayscale);
            enc.set_depth(png::BitDepth::Sixteen);
            let mut w = enc.write_header().expect("png header");
            let bytes: Vec<u8> =
                self.data.iter().flat_map(|&h| ((i32::from(h) + HEIGHT_OFFSET) as u16).to_be_bytes()).collect();
            w.write_image_data(&bytes).expect("png data");
        }
        out
    }

    /// Height (m) at latitude/longitude (rad): Catmull-Rom bicubic between
    /// pixel centres, blended to the polar-row mean at the poles.
    pub fn sample(&self, lat: f64, lon: f64) -> f64 {
        let (w, h) = (self.width as f64, self.height as f64);
        // Continuous pixel coordinates with pixel centres at integers.
        let u = (lon + math::PI) / math::TAU * w - 0.5;
        let v = (math::PI / 2.0 - lat) / math::PI * h - 0.5;
        let (u0, v0) = (u.floor(), v.floor());
        let (wu, wv) = (catmull_rom(u - u0), catmull_rom(v - v0));
        let (u0, v0) = (u0 as i64, v0 as i64);
        let (wi, hi) = (self.width as i64, self.height as i64);
        let mut sum = 0.0;
        for (dj, &wr) in wv.iter().enumerate() {
            let r = (v0 - 1 + dj as i64).clamp(0, hi - 1) as usize;
            let row = &self.data[r * self.width..(r + 1) * self.width];
            let mut acc = 0.0;
            for (di, &wc) in wu.iter().enumerate() {
                let c = (u0 - 1 + di as i64).rem_euclid(wi) as usize;
                acc += wc * f64::from(row[c]);
            }
            sum += wr * acc;
        }
        // Pole blend: v = −0.5 is the north pole, v = h − 0.5 the south pole.
        let (pole, dist) = if v < 0.0 { (0, v + 0.5) } else { (1, h - 0.5 - v) };
        if dist < 0.5 {
            let mean = self.pole_sums[pole] as f64 / w;
            return mean + (sum - mean) * smoothstep(dist / 0.5);
        }
        sum
    }

    /// Height (m) in the direction of a body-fixed vector (any length > 0).
    pub fn height_at(&self, dir: DVec3) -> f64 {
        let (lat, lon) = lat_lon(dir);
        self.sample(lat, lon)
    }
}

/// Catmull-Rom weights of the four samples around fraction `t` ∈ [0, 1).
/// They sum to 1 and reproduce linear data exactly.
fn catmull_rom(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [0.5 * (-t3 + 2.0 * t2 - t), 0.5 * (3.0 * t3 - 5.0 * t2 + 2.0), 0.5 * (-3.0 * t3 + 4.0 * t2 + t), 0.5 * (t3 - t2)]
}

/// `3x² − 2x³` on [0, 1], clamped outside.
pub(crate) fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Geocentric latitude and longitude (rad) of a body-fixed direction.
pub fn lat_lon(dir: DVec3) -> (f64, f64) {
    let horizontal = (dir.x * dir.x + dir.y * dir.y).sqrt();
    (math::atan2(dir.z, horizontal), math::atan2(dir.y, dir.x))
}

/// Loads a heightmap file, once per path per process (the result is shared:
/// Earth's grid is tens of MB and worlds are cloned for background work).
pub fn load_shared(path: &Path) -> Result<Arc<Heightmap>, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Heightmap>>>> = OnceLock::new();
    let key = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
    let cache = CACHE.get_or_init(Default::default);
    // Holding the lock while decoding makes concurrent loaders wait for one decode.
    let mut map = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(h) = map.get(&key) {
        return Ok(h.clone());
    }
    let bytes = std::fs::read(&key).map_err(|e| format!("{}: {e}", path.display()))?;
    let h = Arc::new(Heightmap::from_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))?);
    map.insert(key, h.clone());
    Ok(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEG: f64 = math::PI / 180.0;

    /// A synthetic grid with structure at every scale (integer formula only).
    pub(crate) fn synthetic(width: usize) -> Heightmap {
        let height = width / 2;
        let data = (0..width * height)
            .map(|k| {
                let (i, j) = ((k % width) as i64, (k / width) as i64);
                ((i * 7_919 + j * 104_729 + i * j * 31) % 20_001 - 10_000) as i16
            })
            .collect();
        Heightmap::from_vec(width, height, data)
    }

    #[test]
    fn pixel_centres_follow_the_contract() {
        let m = synthetic(64);
        for (i, j) in [(0, 0), (5, 7), (63, 31), (32, 16)] {
            let lon = -math::PI + (i as f64 + 0.5) * math::TAU / 64.0;
            let lat = math::PI / 2.0 - (j as f64 + 0.5) * math::PI / 32.0;
            assert!((m.sample(lat, lon) - f64::from(m.pixel(i, j))).abs() < 1e-6, "({i}, {j})");
        }
    }

    #[test]
    fn longitude_wraps_and_the_poles_are_single_valued() {
        let m = synthetic(64);
        let p = |i: usize| f64::from(m.pixel(i, 10));
        // Exactly on the antimeridian: Catmull-Rom halfway between the last and first columns.
        let mid = (-p(62) + 9.0 * p(63) + 9.0 * p(0) - p(1)) / 16.0;
        let lat = math::PI / 2.0 - 10.5 * math::PI / 32.0;
        assert!((m.sample(lat, math::PI) - mid).abs() < 1e-9);
        assert!((m.sample(lat, -math::PI) - mid).abs() < 1e-9);
        // At the poles: the polar row's mean, whatever the longitude.
        let mean = |j: usize| (0..64).map(|i| f64::from(m.pixel(i, j))).sum::<f64>() / 64.0;
        for lon in [-3.0, -1.0, 0.0, 0.5, 2.9] {
            assert!((m.sample(90.0 * DEG, lon) - mean(0)).abs() < 1e-9);
            assert!((m.sample(-90.0 * DEG, lon) - mean(31)).abs() < 1e-9);
        }
    }

    #[test]
    fn bicubic_reproduces_a_linear_ramp() {
        // h = 3·column − 5·row + 7 (away from the wrap seam, where a ramp cannot be periodic).
        let (w, h) = (64usize, 32usize);
        let data = (0..w * h).map(|k| (3 * (k % w) as i64 - 5 * (k / w) as i64 + 7) as i16).collect();
        let m = Heightmap::from_vec(w, h, data);
        for (u, v) in [(10.0, 5.0), (10.25, 5.5), (30.9, 20.1), (40.5, 12.75), (2.0, 2.0), (60.99, 28.9)] {
            let lon = -math::PI + (u + 0.5) * math::TAU / w as f64;
            let lat = math::PI / 2.0 - (v + 0.5) * math::PI / h as f64;
            let expected = 3.0 * u - 5.0 * v + 7.0;
            assert!((m.sample(lat, lon) - expected).abs() < 1e-9, "({u}, {v}): {} vs {expected}", m.sample(lat, lon));
        }
    }

    #[test]
    fn catmull_rom_weights_sum_to_one() {
        for t in [0.0, 0.1, 0.5, 0.77, 0.999] {
            let w = catmull_rom(t);
            assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-15);
            // Linear precision: Σ wᵢ·(i − 1) = t.
            assert!((w[2] + 2.0 * w[3] - w[0] - t).abs() < 1e-15);
        }
        assert_eq!(catmull_rom(0.0), [0.0, 1.0, 0.0, 0.0]);
    }

    /// Heights of directions 1e-10 rad apart differ by < 1 mm: across the
    /// antimeridian, at and around the poles, and at random places.
    #[test]
    fn heights_are_continuous_across_the_seam_and_the_poles() {
        // Random ±500 m per pixel: rougher than any real map.
        let m = synthetic(256);
        let m = Heightmap::from_vec(256, 128, m.data.iter().map(|&h| h / 20).collect());
        let eps = 1e-10;
        let mut pairs = vec![
            (DVec3::new(-1.0, eps, 0.3), DVec3::new(-1.0, -eps, 0.3)),
            (DVec3::new(eps, 0.0, 1.0), DVec3::new(-eps, 0.0, 1.0)),
            (DVec3::new(0.0, eps, 1.0), DVec3::new(0.0, -eps, 1.0)),
            (DVec3::new(eps, eps, -1.0), DVec3::new(-eps, -eps, -1.0)),
            (DVec3::new(1e-3, 0.0, 1.0), DVec3::new(1e-3, eps, 1.0)),
        ];
        let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
        for _ in 0..1000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let a = DVec3::new(
                (x % 1000) as f64 - 500.0,
                ((x >> 20) % 1000) as f64 - 500.0,
                ((x >> 40) % 1000) as f64 - 500.0,
            )
            .normalize();
            let b = (a + a.any_orthonormal_vector() * eps).normalize();
            pairs.push((a, b));
        }
        for (a, b) in pairs {
            let d = (m.height_at(a) - m.height_at(b)).abs();
            assert!(d < 1e-3, "{a} vs {b}: step {d} m");
        }
    }

    #[test]
    fn png_round_trips_the_contract() {
        let mut data = synthetic(32).data;
        data[0] = i16::MIN;
        data[1] = i16::MAX;
        data[2] = 0;
        let m = Heightmap::from_vec(32, 16, data);
        let png = m.to_png();
        assert_eq!(Heightmap::from_png(&png).unwrap(), m);
        // Stored value is height + 32768, big-endian.
        let decoded = {
            let mut d = png::Decoder::new(std::io::Cursor::new(&png[..]));
            d.set_transformations(png::Transformations::IDENTITY);
            let mut r = d.read_info().unwrap();
            let mut buf = vec![0u8; r.output_buffer_size().unwrap()];
            r.next_frame(&mut buf).unwrap();
            buf
        };
        assert_eq!(&decoded[4..6], &[0x80, 0x00]);
    }

    /// Cross-platform determinism of sampling (CI: macOS and Windows).
    #[test]
    fn synthetic_samples_match_golden_hash() {
        let hash = sample_hash(&synthetic(512), 100_000);
        println!("terrain sample golden hash: {hash:#018x}");
        assert_eq!(hash, SYNTHETIC_GOLDEN);
    }

    /// FNV-1a over the bits of heights at pseudo-random directions.
    pub(crate) fn sample_hash(m: &Heightmap, n: usize) -> u64 {
        let mut x: u64 = 0x2545_f491_4f6c_dd1d;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        };
        let mut bytes = Vec::with_capacity(n * 8);
        for _ in 0..n {
            let dir = DVec3::new(next(), next(), next());
            bytes.extend_from_slice(&m.height_at(dir).to_bits().to_le_bytes());
        }
        crate::ephem::fnv1a64(&bytes)
    }

    // Bicubic sampling with pole blend (D059).
    const SYNTHETIC_GOLDEN: u64 = 0x55cfb00e38564bef;
}
