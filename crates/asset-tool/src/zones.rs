//! Surface zones (D073, `docs/features/crisp-surfaces.md`): each pixel of a
//! body's committed maps is classified into one zone of the list in its
//! `visual.ron` (`zones` block), so the renderer can draw every zone as one
//! consistent procedural texture with crisp edges instead of a magnified
//! photograph.
//!
//! Method, from the committed `color.jpg`, `water.png` (if the body has an
//! ocean mask) and `height.png`:
//! 1. each output pixel takes an area average of the colour (in linear
//!    light), the sea fraction and the height (see [`crate::resample`]);
//! 2. sea (fraction ≥ 0.5) becomes index 0, so the zone map's coast is the
//!    water mask's;
//! 3. land takes the zone whose `source` colour is nearest in CIELAB, among
//!    the zones whose constraints (latitude, height) hold ([`nearest`]);
//! 4. a 3×3 majority filter removes speckle ([`majority`]); it never moves
//!    the coast.
//!
//! Output: `zones.png`, 8-bit grayscale, the zone index per pixel (0 sea,
//! 1..=N the list order), the grid convention of [`crate::grid`].
//!
//! The data format is owned by `game::body_visual` (`ZonesDef`); this reads
//! only the fields the classification needs.

use std::path::Path;

use serde::Deserialize;

use crate::color::{linear_lut, srgb_to_linear};
use crate::grid::{decode_height, pixel_center, HeightMap};
use crate::resample::resample_area;
use crate::water::write_mask_png;

/// Default output width (height = width / 2): ~10 km per texel on Earth,
/// ~2.7 km on the Moon.
pub const ZONES_W: usize = 4096;

/// A land pixel is sea at or above this sea fraction.
const SEA_FRACTION: f32 = 0.5;

/// The parts of `visual.ron` the classification reads.
#[derive(Deserialize)]
struct VisualFile {
    #[serde(default)]
    color_map: Option<String>,
    #[serde(default)]
    ocean: Option<OceanFile>,
    #[serde(default)]
    zones: Option<ZonesFile>,
}

#[derive(Deserialize)]
struct OceanFile {
    #[serde(default)]
    mask: Option<String>,
}

#[derive(Deserialize)]
struct ZonesFile {
    map: String,
    list: Vec<ZoneDef>,
}

/// One zone's classification fields.
#[derive(Clone, Debug, Deserialize)]
pub struct ZoneDef {
    pub name: String,
    /// The zone's colour in the colour map (sRGB): the classifier's centre.
    pub source: [u8; 3],
    #[serde(default)]
    pub min_abs_lat: Option<f64>,
    #[serde(default)]
    pub max_abs_lat: Option<f64>,
    #[serde(default)]
    pub min_height_m: Option<f64>,
    #[serde(default)]
    pub max_height_m: Option<f64>,
}

impl ZoneDef {
    /// Whether this zone is allowed at `|latitude|` (degrees) and height (m).
    pub fn allows(&self, abs_lat: f64, height_m: f64) -> bool {
        self.min_abs_lat.is_none_or(|v| abs_lat >= v)
            && self.max_abs_lat.is_none_or(|v| abs_lat <= v)
            && self.min_height_m.is_none_or(|v| height_m >= v)
            && self.max_height_m.is_none_or(|v| height_m <= v)
    }
}

/// CIELAB (D65 white) of a linear-light sRGB colour.
pub fn lab_of_linear(c: [f32; 3]) -> [f64; 3] {
    let [r, g, b] = c.map(f64::from);
    let x = (0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47;
    let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
    let z = (0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b) / 1.088_83;
    let d = 6.0 / 29.0;
    let f = |t: f64| if t > d * d * d { t.cbrt() } else { t / (3.0 * d * d) + 4.0 / 29.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// CIELAB of an 8-bit sRGB colour.
pub fn lab_of_srgb8(c: [u8; 3]) -> [f64; 3] {
    lab_of_linear(c.map(|v| srgb_to_linear(f32::from(v) / 255.0)))
}

/// A zone ready for classification: its definition and source colour in CIELAB.
pub struct Zone {
    pub def: ZoneDef,
    pub lab: [f64; 3],
}

impl Zone {
    pub fn new(def: ZoneDef) -> Self {
        let lab = lab_of_srgb8(def.source);
        Self { def, lab }
    }
}

/// The land zone (1-based index into `zones`) for a colour (CIELAB) at
/// `|latitude|` (degrees) and height (m): the nearest source colour among the
/// zones whose constraints hold; ties go to the earlier zone. If no zone's
/// constraints hold, the nearest of all.
pub fn nearest(zones: &[Zone], lab: [f64; 3], abs_lat: f64, height_m: f64) -> u8 {
    let dist = |z: &Zone| (0..3).map(|k| (lab[k] - z.lab[k]).powi(2)).sum::<f64>();
    let best = |allowed: &dyn Fn(&Zone) -> bool| {
        zones
            .iter()
            .enumerate()
            .filter(|(_, z)| allowed(z))
            .fold(None::<(usize, f64)>, |acc, (i, z)| {
                let d = dist(z);
                match acc {
                    Some((_, best)) if best <= d => acc,
                    _ => Some((i, d)),
                }
            })
            .map(|(i, _)| i)
    };
    let i = best(&|z| z.def.allows(abs_lat, height_m)).or_else(|| best(&|_| true)).expect("at least one zone");
    u8::try_from(i + 1).expect("at most 255 zones")
}

/// Zone indices of a `w × h` grid: `rgb` is linear light (3 per pixel),
/// `sea` the sea fraction (none: all land), `height` metres.
pub fn classify(rgb: &[f32], sea: Option<&[f32]>, height: &[f32], w: usize, h: usize, zones: &[Zone]) -> Vec<u8> {
    assert_eq!(rgb.len(), w * h * 3);
    assert_eq!(height.len(), w * h);
    (0..w * h)
        .map(|k| {
            if sea.is_some_and(|s| s[k] >= SEA_FRACTION) {
                return 0;
            }
            let (_, lat) = pixel_center(k % w, k / w, w, h);
            let c = [rgb[3 * k], rgb[3 * k + 1], rgb[3 * k + 2]];
            nearest(zones, lab_of_linear(c), lat.abs(), f64::from(height[k]))
        })
        .collect()
}

/// 3×3 majority filter (longitude wraps; rows beyond the poles are left
/// out). Sea (0) stays sea and land never becomes sea, so the coast does
/// not move. A land pixel takes the land zone most common among its land
/// neighbours (itself included); it keeps its own zone when that is tied
/// for the most, and other ties go to the lower index.
pub fn majority(idx: &[u8], w: usize, h: usize) -> Vec<u8> {
    assert_eq!(idx.len(), w * h);
    let mut out = Vec::with_capacity(w * h);
    let mut counts = [0u8; 256];
    for j in 0..h {
        for i in 0..w {
            let me = idx[j * w + i];
            if me == 0 {
                out.push(0);
                continue;
            }
            counts.fill(0);
            for dj in -1isize..=1 {
                let jj = j as isize + dj;
                if jj < 0 || jj >= h as isize {
                    continue;
                }
                for di in -1isize..=1 {
                    let ii = (i as isize + di).rem_euclid(w as isize) as usize;
                    let v = idx[jj as usize * w + ii];
                    if v != 0 {
                        counts[usize::from(v)] += 1;
                    }
                }
            }
            let top = *counts.iter().max().expect("non-empty");
            let pick = if counts[usize::from(me)] == top {
                me
            } else {
                counts.iter().position(|&c| c == top).expect("a maximum") as u8
            };
            out.push(pick);
        }
    }
    out
}

/// Area fractions (pixels weighted by cos latitude) of the sea, and of the
/// land in each zone (index 1..=n), of a `w`-wide grid.
pub fn fractions(idx: &[u8], w: usize, n: usize) -> (f64, Vec<f64>) {
    let h = idx.len() / w;
    let mut area = vec![0.0f64; n + 1];
    for (j, row) in idx.chunks(w).enumerate() {
        let (_, lat) = pixel_center(0, j, w, h);
        let a = lat.to_radians().cos();
        for &v in row {
            area[usize::from(v)] += a;
        }
    }
    let total: f64 = area.iter().sum();
    let land = (total - area[0]).max(f64::MIN_POSITIVE);
    (area[0] / total, area[1..].iter().map(|&a| a / land).collect())
}

/// Bakes `data/bodies/<body>/zones.png` (at width `w`) if the body's
/// `visual.ron` has a `zones` block. With `preview`, also writes a preview
/// in the zones' source colours (sea dark blue) to `preview`.
pub fn bake(body: &str, w: usize, preview: Option<&Path>) {
    let dir = Path::new("data/bodies").join(body);
    let text = std::fs::read_to_string(dir.join("visual.ron")).expect("read visual.ron");
    let visual: VisualFile = ron::from_str(&text).unwrap_or_else(|e| panic!("{body}/visual.ron: {e}"));
    let Some(zdef) = visual.zones else {
        println!("{body}: no zones block in visual.ron, skipped");
        return;
    };
    println!("{body} zones ({} zones, from the committed maps)", zdef.list.len());
    let h = w / 2;
    let color = image::open(dir.join(visual.color_map.as_deref().expect("zones need a colour map")))
        .expect("open colour map")
        .into_rgb8();
    let (cw, ch) = (color.width() as usize, color.height() as usize);
    let lut = linear_lut(8);
    let raw = color.into_raw();
    let rgb = resample_area(cw, ch, 3, w, h, |j, buf| {
        for (b, &v) in buf.iter_mut().zip(&raw[j * cw * 3..(j + 1) * cw * 3]) {
            *b = lut[usize::from(v)];
        }
    });
    drop(raw);
    let sea = visual.ocean.and_then(|o| o.mask).map(|m| {
        let img = image::open(dir.join(m)).expect("open water mask").into_luma8();
        let (mw, mh) = (img.width() as usize, img.height() as usize);
        let data = img.into_raw();
        resample_area(mw, mh, 1, w, h, |j, buf| {
            for (b, &v) in buf.iter_mut().zip(&data[j * mw..(j + 1) * mw]) {
                *b = f32::from(v) / 255.0;
            }
        })
    });
    let hm = HeightMap::load(&dir.join("height.png"));
    let height = resample_area(hm.w, hm.h, 1, w, h, |j, buf| {
        for (b, &v) in buf.iter_mut().zip(&hm.data[j * hm.w..(j + 1) * hm.w]) {
            *b = decode_height(v) as f32;
        }
    });
    let zones: Vec<Zone> = zdef.list.into_iter().map(Zone::new).collect();
    let idx = majority(&classify(&rgb, sea.as_deref(), &height, w, h, &zones), w, h);
    let (sea_frac, land) = fractions(&idx, w, zones.len());
    println!("  sea {:.1} % of the area; land by zone (area):", 100.0 * sea_frac);
    for (z, f) in zones.iter().zip(&land) {
        println!("    {:<16} {:5.1} %", z.def.name, 100.0 * f);
    }
    let path = dir.join(&zdef.map);
    write_mask_png(&path, w, h, &idx);
    let size = std::fs::metadata(&path).expect("stat output").len();
    println!("  wrote {} ({w}×{h}, {:.2} MB)", path.display(), size as f64 / 1e6);
    if let Some(p) = preview {
        // Every other pixel (nearest), so zone edges stay as they are.
        let (pw, ph) = (w / 2, h / 2);
        let px: Vec<u8> = (0..pw * ph)
            .flat_map(|k| {
                let v = idx[(k / pw) * 2 * w + (k % pw) * 2];
                if v == 0 {
                    [8, 16, 48]
                } else {
                    zones[usize::from(v) - 1].def.source
                }
            })
            .collect();
        image::save_buffer(p, &px, pw as u32, ph as u32, image::ExtendedColorType::Rgb8).expect("write preview");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(name: &str, source: [u8; 3]) -> ZoneDef {
        ZoneDef {
            name: name.into(),
            source,
            min_abs_lat: None,
            max_abs_lat: None,
            min_height_m: None,
            max_height_m: None,
        }
    }

    fn zones() -> Vec<Zone> {
        vec![
            Zone::new(ZoneDef { min_abs_lat: Some(55.0), ..zone("ice", [237, 239, 240]) }),
            Zone::new(zone("forest", [31, 43, 15])),
            Zone::new(zone("desert", [181, 154, 118])),
            Zone::new(ZoneDef { max_height_m: Some(1000.0), ..zone("salt", [209, 213, 215]) }),
            Zone::new(ZoneDef { min_height_m: Some(2000.0), ..zone("snow", [230, 232, 235]) }),
        ]
    }

    #[test]
    fn lab_reference_values() {
        let close = |a: [f64; 3], b: [f64; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 0.05);
        assert!(close(lab_of_srgb8([255, 255, 255]), [100.0, 0.0, 0.0]), "{:?}", lab_of_srgb8([255, 255, 255]));
        assert!(close(lab_of_srgb8([0, 0, 0]), [0.0, 0.0, 0.0]));
        // sRGB red and mid grey (standard conversions).
        assert!(close(lab_of_srgb8([255, 0, 0]), [53.24, 80.09, 67.20]), "{:?}", lab_of_srgb8([255, 0, 0]));
        assert!(close(lab_of_srgb8([128, 128, 128]), [53.59, 0.0, 0.0]), "{:?}", lab_of_srgb8([128, 128, 128]));
    }

    #[test]
    fn nearest_zone_with_constraints() {
        let z = zones();
        let lab = lab_of_srgb8;
        // (colour, |lat|, height, expected zone)
        let cases = [
            ([236, 238, 240], 70.0, 0.0, 1),    // ice at high latitude
            ([236, 238, 240], 20.0, 0.0, 4),    // bright and low: not ice, the salt pan
            ([236, 238, 240], 30.0, 3000.0, 5), // bright and high: snow
            ([212, 214, 214], 20.0, 3600.0, 5), // a salt pan above 1 km: salt is not allowed
            ([30, 45, 16], 10.0, 200.0, 2),     // forest
            ([185, 150, 110], 25.0, 300.0, 3),  // desert
            ([185, 150, 110], 80.0, 300.0, 3),  // constraints never force a far zone
        ];
        for (c, lat, height, want) in cases {
            assert_eq!(nearest(&z, lab(c), lat, height), want, "{c:?} at {lat}°, {height} m");
        }
        // No zone allowed: the nearest of all.
        let only = vec![Zone::new(ZoneDef { min_abs_lat: Some(80.0), ..zone("ice", [237, 239, 240]) })];
        assert_eq!(nearest(&only, lab([30, 45, 16]), 0.0, 0.0), 1);
        // Ties go to the earlier zone.
        let twins = vec![Zone::new(zone("a", [100, 100, 100])), Zone::new(zone("b", [100, 100, 100]))];
        assert_eq!(nearest(&twins, lab([100, 100, 100]), 0.0, 0.0), 1);
    }

    #[test]
    fn sea_is_zero() {
        let z = zones();
        let lin = |c: [u8; 3]| c.map(|v| srgb_to_linear(f32::from(v) / 255.0));
        let (w, h) = (4, 2);
        let rgb: Vec<f32> = [[30, 45, 16], [30, 45, 16], [185, 150, 110], [5, 10, 30]]
            .iter()
            .cycle()
            .take(8)
            .flat_map(|&c| lin(c))
            .collect();
        let sea = [0.0, 0.49, 0.5, 1.0, 0.2, 0.0, 0.0, 1.0];
        let height = [100.0; 8];
        let idx = classify(&rgb, Some(&sea), &height, w, h, &z);
        assert_eq!(idx, vec![2, 2, 0, 0, 2, 2, 3, 0]);
        // Without a mask everything is land.
        assert!(classify(&rgb, None, &height, w, h, &z).iter().all(|&v| v != 0));
    }

    /// Parses a grid of digits (zone indices).
    fn grid(rows: &[&str]) -> (Vec<u8>, usize, usize) {
        let w = rows[0].len();
        (rows.iter().flat_map(|r| r.bytes().map(|b| b - b'0')).collect(), w, rows.len())
    }

    fn render(idx: &[u8], w: usize) -> Vec<String> {
        idx.chunks(w).map(|r| r.iter().map(|v| char::from(b'0' + v)).collect()).collect()
    }

    #[test]
    fn majority_filter() {
        // (input, expected)
        let cases: [(&[&str], &[&str]); 6] = [
            // A lone speckle is removed.
            (&["22222", "22322", "22222"], &["22222", "22222", "22222"]),
            // A 2-pixel-wide band survives (6 of 9 inside; at the poles' edge
            // rows a 3-3 tie, which keeps the centre).
            (&["222222", "333333", "333333", "222222"], &["222222", "333333", "333333", "222222"]),
            // Sea stays sea and land never becomes sea, even when outvoted.
            (&["00000", "00200", "00000"], &["00000", "00200", "00000"]),
            // Ties: the centre keeps its zone (the 3s at 4 against 4); a centre
            // outvoted by a tie takes the lower index (the 4 becomes 2).
            (&["232", "343", "232"], &["333", "323", "333"]),
            // Longitude wraps: a one-pixel line at the left edge is outvoted
            // by the right edge (without wrapping it would be a tie and stay).
            (&["2333", "2333", "2333"], &["3333", "3333", "3333"]),
            // A zone two pixels wide stays.
            (&["2233", "2233", "2233"], &["2233", "2233", "2233"]),
        ];
        for (input, want) in cases {
            let (idx, w, h) = grid(input);
            assert_eq!(render(&majority(&idx, w, h), w), want, "{input:?}");
        }
    }

    #[test]
    fn fractions_are_by_area() {
        // Two rows at ±45°: equal weights.
        let (sea, land) = fractions(&[0, 1, 1, 2, 0, 0, 1, 2], 4, 2);
        assert!((sea - 3.0 / 8.0).abs() < 1e-12);
        assert!((land[0] - 0.6).abs() < 1e-12 && (land[1] - 0.4).abs() < 1e-12, "{land:?}");
        // Four rows: the polar rows (±67.5°) weigh cos 67.5° / cos 22.5° of the others.
        let (sea, _) = fractions(&[0, 1, 1, 0], 1, 1);
        let (p, e) = (67.5f64.to_radians().cos(), 22.5f64.to_radians().cos());
        assert!((sea - p / (p + e)).abs() < 1e-12, "{sea}");
    }

    #[test]
    fn output_is_deterministic() {
        let z = zones();
        let (w, h) = (64, 32);
        let rgb: Vec<f32> = (0..w * h * 3).map(|k| ((k * 7919) % 1000) as f32 / 1000.0).collect();
        let sea: Vec<f32> = (0..w * h).map(|k| ((k * 31) % 100) as f32 / 100.0).collect();
        let height: Vec<f32> = (0..w * h).map(|k| ((k * 13) % 4000) as f32).collect();
        let a = majority(&classify(&rgb, Some(&sea), &height, w, h, &z), w, h);
        let b = majority(&classify(&rgb, Some(&sea), &height, w, h, &z), w, h);
        assert_eq!(a, b);
        let (s, land) = fractions(&a, w, z.len());
        assert!((s + land.iter().sum::<f64>() * (1.0 - s) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn shipped_zone_lists_parse() {
        for body in ["earth", "moon"] {
            let path = format!("{}/../../data/bodies/{body}/visual.ron", env!("CARGO_MANIFEST_DIR"));
            let v: VisualFile = ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let z = v.zones.unwrap_or_else(|| panic!("{body} has zones"));
            assert!(!z.list.is_empty() && z.list.len() < 16, "{body}: 1..=15 zones (the shader's palette)");
        }
    }
}
