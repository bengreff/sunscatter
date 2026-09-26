//! Procedural terrain detail below the heightmap's resolution (D059).
//!
//! `detail(dir)` is fractal gradient noise ([`super::noise`]) on the 3D point
//! `dir × radius`, summed over octaves from [`DetailParams::wavelength`]
//! (about the heightmap's sample spacing) down to a few metres. Each octave's
//! amplitude is `amplitude × roughness × gainᵏ`, where roughness (m) is the
//! baked RMS residual of the real data near that point ([`Roughness`]): flat
//! plains stay flat, rough ground gets detail. Rough ground (above
//! `ridge_threshold`) blends from plain fBm to ridged octaves (`(1 − |n|)²`,
//! centred), which gives crests and gullies instead of lumps.
//!
//! Over water (the body has a sea level and the base height is at or below it)
//! the detail is zero; it fades in over [`COAST_FADE_M`] above sea level so the
//! surface stays continuous at the shore.
//!
//! **Roughness file contract** (`roughness.png`, baked by `asset-tool`): 8-bit
//! grayscale, the heightmap's grid convention (pixel-centred equirectangular,
//! `w = 2h`, row 0 north), `rms_m = value² / 64`.

use super::noise;
use super::smoothstep;
use crate::math;
use glam::DVec3;
use std::fmt;

/// Detail fades in over this height above sea level (m).
pub const COAST_FADE_M: f64 = 10.0;
/// Octaves with an amplitude below this (m) are skipped (they would move
/// the surface by less than 0.1 mm).
const MIN_AMPLITUDE_M: f64 = 1e-4;
/// Mean of `(1 − |n|)²` over the noise, so ridged octaves add no net height.
const RIDGE_MEAN: f64 = 0.632;
/// Rotation applied between octaves (orthonormal, exact in binary to 1 ulp),
/// so octave lattices do not line up.
const OCTAVE_ROTATION: [[f64; 3]; 3] = [[0.0, 0.8, 0.6], [-0.8, 0.36, -0.48], [-0.6, -0.48, 0.64]];

/// Detail parameters, body data (`terrain_detail` in `body.ron`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DetailParams {
    /// Number of octaves.
    pub octaves: u32,
    /// Wavelength of the first octave (m), about the heightmap sample spacing.
    pub wavelength: f64,
    /// Wavelength ratio between octaves (> 1).
    pub lacunarity: f64,
    /// Amplitude ratio between octaves (< 1).
    pub gain: f64,
    /// First-octave amplitude per metre of roughness.
    pub amplitude: f64,
    /// Roughness (m) at which detail is half fBm, half ridged; the blend runs
    /// from half to one and a half times this.
    pub ridge_threshold: f64,
    /// Selects an independent noise field.
    #[serde(default)]
    pub seed: u64,
}

/// Baked roughness (RMS residual, m) on an equirectangular grid.
#[derive(Clone, PartialEq)]
pub struct Roughness {
    width: usize,
    height: usize,
    data: Vec<u8>,
    /// Mean roughness (m) of the north and south rows, for the pole blend.
    pole_means: [f64; 2],
}

impl fmt::Debug for Roughness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Roughness({}×{})", self.width, self.height)
    }
}

/// Decodes a stored roughness byte to metres.
pub fn decode_roughness(v: u8) -> f64 {
    let v = f64::from(v);
    v * v / 64.0
}

impl Roughness {
    /// Builds a map from row-major stored bytes. Panics on a size mismatch or an empty grid.
    pub fn from_vec(width: usize, height: usize, data: Vec<u8>) -> Self {
        assert!(width > 0 && height > 0, "empty roughness map");
        assert_eq!(data.len(), width * height, "roughness map size");
        let mean = |j: usize| data[j * width..(j + 1) * width].iter().map(|&v| decode_roughness(v)).sum::<f64>();
        let pole_means = [mean(0) / width as f64, mean(height - 1) / width as f64];
        Roughness { width, height, data, pole_means }
    }

    /// A map of one value everywhere (tests, tools).
    pub fn uniform(value: u8) -> Self {
        Self::from_vec(2, 1, vec![value; 2])
    }

    /// Decodes the 8-bit grayscale PNG of the file contract.
    pub fn from_png(bytes: &[u8]) -> Result<Self, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::IDENTITY);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let info = reader.info();
        if info.color_type != png::ColorType::Grayscale || info.bit_depth != png::BitDepth::Eight {
            return Err(format!("expected 8-bit grayscale, found {:?} {:?}", info.color_type, info.bit_depth));
        }
        let (width, height) = (info.width as usize, info.height as usize);
        let mut buf = vec![0u8; reader.output_buffer_size().ok_or("image too large")?];
        let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        buf.truncate(frame.buffer_size());
        if buf.len() != width * height || width == 0 {
            return Err(format!("unexpected pixel data size {}", buf.len()));
        }
        Ok(Self::from_vec(width, height, buf))
    }

    /// Roughness (m) at latitude/longitude (rad): bilinear between texel
    /// centres, longitude wraps, blended to the polar row's mean at the poles.
    pub fn sample(&self, lat: f64, lon: f64) -> f64 {
        let (w, h) = (self.width as f64, self.height as f64);
        let u = (lon + math::PI) / math::TAU * w - 0.5;
        let v = (math::PI / 2.0 - lat) / math::PI * h - 0.5;
        let (u0, v0) = (u.floor(), v.floor());
        let (fu, fv) = (u - u0, v - v0);
        let (u0, v0) = (u0 as i64, v0 as i64);
        let c = |d: i64| (u0 + d).rem_euclid(self.width as i64) as usize;
        let r = |d: i64| (v0 + d).clamp(0, self.height as i64 - 1) as usize;
        let px = |c: usize, r: usize| decode_roughness(self.data[r * self.width + c]);
        let top = px(c(0), r(0)) * (1.0 - fu) + px(c(1), r(0)) * fu;
        let bottom = px(c(0), r(1)) * (1.0 - fu) + px(c(1), r(1)) * fu;
        let value = top * (1.0 - fv) + bottom * fv;
        let (pole, dist) = if v < 0.0 { (0, v + 0.5) } else { (1, h - 0.5 - v) };
        if dist < 0.5 {
            let mean = self.pole_means[pole];
            return mean + (value - mean) * smoothstep(dist / 0.5);
        }
        value
    }
}

/// Everything a body needs for detail: its parameters and its roughness map.
#[derive(Clone, Debug, PartialEq)]
pub struct Detail {
    pub params: DetailParams,
    pub roughness: std::sync::Arc<Roughness>,
}

impl Detail {
    /// Detail height (m) at unit direction `dir` (latitude/longitude `lat`,
    /// `lon` of the same direction, rad) on a body of reference radius
    /// `radius` (m). `water_weight` ∈ [0, 1] scales it (see [`coast_weight`]).
    pub fn height(&self, dir: DVec3, lat: f64, lon: f64, radius: f64, water_weight: f64) -> f64 {
        if water_weight <= 0.0 {
            return 0.0;
        }
        let rough = self.roughness.sample(lat, lon);
        water_weight * fractal(&self.params, dir * radius, rough)
    }
}

/// How much detail a point with base height `base` gets, for a body with
/// sea level `sea`: 0 at or below the sea, 1 from [`COAST_FADE_M`] above it.
pub fn coast_weight(base: f64, sea: Option<f64>) -> f64 {
    match sea {
        Some(sea) => smoothstep((base - sea) / COAST_FADE_M),
        None => 1.0,
    }
}

/// The fractal sum at body-fixed point `p` (m) for local roughness `rough` (m).
pub fn fractal(params: &DetailParams, p: DVec3, rough: f64) -> f64 {
    let mut amp = params.amplitude * rough;
    if amp < MIN_AMPLITUDE_M {
        return 0.0;
    }
    let ridge = if params.ridge_threshold > 0.0 { smoothstep(rough / params.ridge_threshold - 0.5) } else { 1.0 };
    let mut q = p / params.wavelength;
    let mut sum = 0.0;
    for k in 0..params.octaves {
        if amp < MIN_AMPLITUDE_M {
            break;
        }
        let n = noise::gradient(q, params.seed.wrapping_add(u64::from(k)));
        let a = 1.0 - n.abs();
        let ridged = a * a - RIDGE_MEAN;
        sum += amp * (n + (ridged - n) * ridge);
        amp *= params.gain;
        q = rotate(q) * params.lacunarity;
    }
    sum
}

#[inline]
fn rotate(q: DVec3) -> DVec3 {
    let m = OCTAVE_ROTATION;
    DVec3::new(
        m[0][0] * q.x + m[0][1] * q.y + m[0][2] * q.z,
        m[1][0] * q.x + m[1][1] * q.y + m[1][2] * q.z,
        m[2][0] * q.x + m[2][1] * q.y + m[2][2] * q.z,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn params() -> DetailParams {
        DetailParams {
            octaves: 12,
            wavelength: 4_900.0,
            lacunarity: 2.0,
            gain: 0.5,
            amplitude: 1.0,
            ridge_threshold: 100.0,
            seed: 0,
        }
    }

    /// RMS of the detail over many points of an Earth-sized sphere.
    fn rms(p: &DetailParams, rough: f64) -> f64 {
        let n = 4_000;
        let mut s2 = 0.0;
        for k in 0..n {
            let t = k as f64;
            let dir = DVec3::new(t * 0.7133, t * 0.3119 + 1.0, t * 0.1571 - 2.0).normalize();
            let h = fractal(p, dir * 6.378e6, rough);
            s2 += h * h;
        }
        (s2 / n as f64).sqrt()
    }

    #[test]
    fn amplitude_grows_with_roughness() {
        // (roughness m, detail RMS lower and upper bound m)
        let table = [(0.0, 0.0, 0.0), (1.0, 0.05, 1.0), (10.0, 0.5, 10.0), (100.0, 5.0, 100.0), (500.0, 25.0, 500.0)];
        let mut last = -1.0;
        for (rough, lo, hi) in table {
            let r = rms(&params(), rough);
            println!("roughness {rough} m → detail RMS {r:.3} m");
            assert!(r >= lo && r <= hi, "roughness {rough}: {r}");
            assert!(r > last || rough == 0.0);
            last = r;
        }
        // Without ridges the RMS is proportional to roughness (same field, scaled).
        let smooth = DetailParams { ridge_threshold: 1e9, ..params() };
        let (a, b) = (rms(&smooth, 2.0), rms(&smooth, 20.0));
        assert!((b / a - 10.0).abs() < 1e-6, "{a} {b}");
    }

    #[test]
    fn ridged_octaves_have_no_net_height() {
        let ridged = DetailParams { ridge_threshold: 1e-9, octaves: 1, ..params() };
        let n = 40_000;
        let mean = (0..n)
            .map(|k| {
                let t = k as f64;
                fractal(&ridged, DVec3::new(t * 713.3, t * 311.9 + 1.0, t * 157.1 - 2.0), 1.0)
            })
            .sum::<f64>()
            / n as f64;
        assert!(mean.abs() < 0.01, "{mean}");
    }

    #[test]
    fn coast_weight_is_zero_over_water() {
        for (base, sea, w) in [
            (-100.0, Some(0.0), 0.0),
            (0.0, Some(0.0), 0.0),
            (COAST_FADE_M, Some(0.0), 1.0),
            (5_000.0, Some(0.0), 1.0),
            (-100.0, None, 1.0),
            (100.0, Some(100.0), 0.0),
        ] {
            assert_eq!(coast_weight(base, sea), w, "{base} {sea:?}");
        }
        let d = Detail { params: params(), roughness: std::sync::Arc::new(Roughness::uniform(200)) };
        assert_eq!(d.height(DVec3::X, 0.0, 0.0, 6.4e6, 0.0), 0.0);
        assert_ne!(d.height(DVec3::new(0.6, 0.0, 0.8), 0.9, 0.0, 6.4e6, 1.0), 0.0);
    }

    #[test]
    fn roughness_decodes_and_samples() {
        assert_eq!(decode_roughness(8), 1.0);
        assert_eq!(decode_roughness(255), 1016.015625);
        // 4×2 map: bilinear between texel centres; poles are the row means.
        let m = Roughness::from_vec(4, 2, vec![8, 16, 8, 16, 0, 0, 0, 0]);
        let lon = |i: f64| -math::PI + (i + 0.5) * math::TAU / 4.0;
        let lat0 = math::PI / 2.0 - 0.5 * math::PI / 2.0;
        assert!((m.sample(lat0, lon(0.0)) - 1.0).abs() < 1e-12);
        assert!((m.sample(lat0, lon(1.0)) - 4.0).abs() < 1e-12);
        assert!((m.sample(lat0, lon(0.5)) - 2.5).abs() < 1e-12);
        assert!((m.sample(math::PI / 2.0, 1.234) - 2.5).abs() < 1e-12);
        assert_eq!(m.sample(-math::PI / 2.0, 0.3), 0.0);
    }
}
