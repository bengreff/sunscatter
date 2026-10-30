//! The giant planets' colour maps (agent SKY; sources and choices in
//! docs/technical/solar-visuals.md#giants): `asset-tool giants [jupiter saturn uranus neptune]`.
//!
//! All four from the Hubble OPAL global maps of 2025 (Simon et al., DOI 10.17909/T9G593),
//! three filters per planet as red, green and blue:
//! - Jupiter: the map in full (3600×1800, 0.1°), uncovered pixels and rows from the
//!   latitude profile.
//! - Saturn, Uranus, Neptune: zonal profiles (median of each latitude row; their maps are
//!   coarser and only partly covered, and their cloud features change from year to year:
//!   the bands are what stays). Rows the map does not cover are filled from the nearest
//!   covered rows (interpolated between them, extended at the ends).
//!
//! Every map is calibrated to the planet's measured colour: the area mean of each channel is
//! the Lambert albedo 1.5 p of the geometric albedo p in the Sloan r', Johnson V and Sloan g'
//! bands (Mallama et al. 2017, Icarus 282, 19, table 7). Where that would push the
//! brightest clouds over 1, the contrast is reduced by a power law until they fit
//! ([`fit_contrast`]).
//!
//! Inputs in `data/external/giants/` (see the doc for the download URLs).

use crate::color::{linear_to_srgb, write_jpeg, write_preview};
use std::path::{Path, PathBuf};

/// The zonal maps' size (narrow: one profile repeated; the renderer's
/// width cap then keeps every latitude row).
const ZONAL_W: usize = 64;
const ZONAL_H: usize = 2048;
/// The share of a latitude row that must be covered for its median to count.
const MIN_COVER: f64 = 0.5;
/// Profile cleaning: rows within `EDGE_DEG` of the map's coverage edge
/// are dropped (the limb, seen at grazing angles, is dark), and rows more
/// than `SPIKE` (fraction) off the median of the rows within `SPIKE_DEG`
/// (the rings across Saturn's equator, edge-on in 2025).
const EDGE_DEG: f64 = 15.0;
const SPIKE_DEG: f64 = 3.0;
const SPIKE: f32 = 0.15;
/// Percentile of the brightest channel kept under [`MAX_LINEAR`] by the contrast fit.
const TOP_PERCENTILE: f64 = 0.995;
const MAX_LINEAR: f32 = 0.98;

/// Geometric albedos (Mallama et al. 2017, table 7): Sloan r', Johnson V, Sloan g'.
fn albedo(name: &str) -> [f32; 3] {
    match name {
        "jupiter" => [0.575, 0.538, 0.509],
        "saturn" => [0.618, 0.499, 0.436],
        "uranus" => [0.355, 0.488, 0.559],
        "neptune" => [0.307, 0.442, 0.533],
        _ => panic!("not a giant: {name}"),
    }
}

/// The OPAL maps used for a planet (red, green, blue filters).
fn opal(name: &str) -> [&'static str; 3] {
    match name {
        "jupiter" => [
            "hlsp_opal_hst_wfc3-uvis_jupiter-2025a_f631n_v1_globalmap.fits",
            "hlsp_opal_hst_wfc3-uvis_jupiter-2025a_f502n_v1_globalmap.fits",
            "hlsp_opal_hst_wfc3-uvis_jupiter-2025a_f467m_v1_globalmap.fits",
        ],
        "saturn" => [
            "hlsp_opal_hst_wfc3-uvis_saturn-2025a_f631n_v1_globalmap.fits",
            "hlsp_opal_hst_wfc3-uvis_saturn-2025a_f502n_v1_globalmap.fits",
            "hlsp_opal_hst_wfc3-uvis_saturn-2025a_f467m_v1_globalmap.fits",
        ],
        "uranus" => [
            "hlsp_opal_hst_wfc3-uvis_uranus-2025a_f657n_v1_globalmap.fits",
            "hlsp_opal_hst_wfc3-uvis_uranus-2025a_f547m_v1_globalmap.fits",
            "hlsp_opal_hst_wfc3-uvis_uranus-2025a_f467m_v1_globalmap.fits",
        ],
        "neptune" => [
            "hlsp_opal_hst_wfc3-uvis_neptune-2025b_f657n_v1_globalmap.fits",
            "hlsp_opal_hst_wfc3-uvis_neptune-2025b_f547m_v1_globalmap.fits",
            "hlsp_opal_hst_wfc3-uvis_neptune-2025b_f467m_v1_globalmap.fits",
        ],
        _ => panic!("no OPAL maps for {name}"),
    }
}

fn external() -> PathBuf {
    PathBuf::from(format!("{}/../../data/external/giants", env!("CARGO_MANIFEST_DIR")))
}

fn body_dir(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../data/bodies/{name}", env!("CARGO_MANIFEST_DIR")))
}

pub fn run(names: &[String]) {
    let all = ["jupiter", "saturn", "uranus", "neptune"];
    let list: Vec<&str> = if names.is_empty() { all.to_vec() } else { names.iter().map(String::as_str).collect() };
    for name in list {
        println!("{name}");
        let (w, h, mut lin) = if name == "jupiter" { jupiter() } else { zonal(name) };
        let (gains, gamma) = fit_contrast(&mut lin, w, h, albedo(name));
        println!("  calibrated: gains {gains:.3?}, contrast exponent {gamma:.3}");
        let mean = area_mean(&lin, w, h);
        println!("  area mean (linear) {mean:.3?}; target {:.3?}", albedo(name).map(|p| 1.5 * p));
        let rgb: Vec<u8> = lin.iter().flat_map(|c| c.map(|v| (linear_to_srgb(v) * 255.0).round() as u8)).collect();
        let out = body_dir(name).join("color.jpg");
        write_jpeg(&out, w, h, &rgb, 92);
        println!("  wrote {} ({} kB)", out.display(), std::fs::metadata(&out).map_or(0, |m| m.len() / 1000));
        let previews = external().join("previews");
        std::fs::create_dir_all(&previews).ok();
        let pw = if w == ZONAL_W { ZONAL_W } else { 1024 };
        write_preview(&previews.join(format!("{name}_color.png")), w, h, &rgb, pw);
    }
}

/// Jupiter: the OPAL map in full (3600×1800, pixel centres on the grid
/// convention's), each channel's uncovered pixels and the rows dropped by
/// [`clean_profile`] filled with its row profile.
fn jupiter() -> (usize, usize, Vec<[f32; 3]>) {
    let mut out: Vec<[f32; 3]> = Vec::new();
    let (mut w, mut h) = (0, 0);
    for (k, f) in opal("jupiter").iter().enumerate() {
        let (fw, fh, data) = read_fits(&external().join(f));
        (w, h) = (fw, fh);
        let rows = clean_profile(&row_medians(&data, w, h, MIN_COVER), EDGE_DEG, SPIKE_DEG, SPIKE);
        println!("  {f}: {w}×{h}, {} of {h} rows covered", rows.iter().flatten().count());
        let profile = fill_gaps(&rows);
        out.resize(w * h, [0.0; 3]);
        for j in 0..h {
            for i in 0..w {
                let v = data[j * w + i];
                let ok = rows[j].is_some() && v.is_finite() && v > 0.0;
                out[j * w + i][k] = if ok { v } else { profile[j] };
            }
        }
    }
    (w, h, out)
}

/// A zonal map from three OPAL filters: each channel's row profile,
/// resampled to [`ZONAL_H`] rows and repeated across [`ZONAL_W`] columns.
fn zonal(name: &str) -> (usize, usize, Vec<[f32; 3]>) {
    let profiles: Vec<Vec<f32>> = opal(name)
        .iter()
        .map(|f| {
            let (w, h, data) = read_fits(&external().join(f));
            let rows = clean_profile(&row_medians(&data, w, h, MIN_COVER), EDGE_DEG, SPIKE_DEG, SPIKE);
            let covered = rows.iter().filter(|r| r.is_some()).count();
            println!("  {f}: {w}×{h}, {covered} of {h} rows covered");
            fill_gaps(&rows)
        })
        .collect();
    let n = profiles[0].len();
    let mut out = Vec::with_capacity(ZONAL_W * ZONAL_H);
    for j in 0..ZONAL_H {
        // Source rows span the poles edge to edge (centres at 90° − j·180°/(n − 1)).
        let y = (j as f64 + 0.5) / ZONAL_H as f64 * (n - 1) as f64;
        let (j0, f) = ((y.floor() as usize).min(n - 2), y.fract() as f32);
        let c: [f32; 3] = std::array::from_fn(|k| profiles[k][j0] + (profiles[k][j0 + 1] - profiles[k][j0]) * f);
        out.extend(std::iter::repeat_n(c, ZONAL_W));
    }
    (ZONAL_W, ZONAL_H, out)
}

/// Reads a 2-D FITS image of 32-bit floats (BITPIX −32), the first HDU.
pub fn read_fits(path: &Path) -> (usize, usize, Vec<f32>) {
    let raw = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let (w, h, start) = fits_header(&raw).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let data = raw[start..start + 4 * w * h].as_chunks::<4>().0.iter().map(|b| f32::from_be_bytes(*b)).collect();
    (w, h, data)
}

/// The image size and the data's byte offset from a FITS header.
fn fits_header(raw: &[u8]) -> Result<(usize, usize, usize), String> {
    let (mut w, mut h, mut bitpix) = (None, None, None);
    for (i, card) in raw.chunks(80).enumerate() {
        let text = std::str::from_utf8(card).map_err(|_| "header is not ASCII")?;
        let key = text.get(..8).unwrap_or("").trim();
        let value = || text.get(10..).unwrap_or("").split('/').next().unwrap_or("").trim().parse::<i64>().ok();
        match key {
            "BITPIX" => bitpix = value(),
            "NAXIS1" => w = value(),
            "NAXIS2" => h = value(),
            "END" => {
                if bitpix != Some(-32) {
                    return Err(format!("BITPIX {bitpix:?}, expected -32"));
                }
                let start = (i + 1) * 80;
                let (w, h) = (w.ok_or("no NAXIS1")? as usize, h.ok_or("no NAXIS2")? as usize);
                let start = start.div_ceil(2880) * 2880;
                return if raw.len() >= start + 4 * w * h { Ok((w, h, start)) } else { Err("truncated".into()) };
            }
            _ => {}
        }
    }
    Err("no END card".into())
}

/// The median of each row's covered pixels (finite and positive), or
/// `None` where less than `min_cover` of the row is covered. Row 0 is the
/// map's top (north).
pub fn row_medians(data: &[f32], w: usize, h: usize, min_cover: f64) -> Vec<Option<f32>> {
    (0..h)
        .map(|j| {
            let mut v: Vec<f32> =
                data[j * w..(j + 1) * w].iter().copied().filter(|x| x.is_finite() && *x > 0.0).collect();
            if (v.len() as f64) < min_cover * w as f64 {
                return None;
            }
            v.sort_by(f32::total_cmp);
            Some(v[v.len() / 2])
        })
        .collect()
}

/// Drops a profile's rows near its coverage edges and its spikes (see
/// [`EDGE_DEG`]); `rows` span the poles edge to edge.
pub fn clean_profile(rows: &[Option<f32>], edge_deg: f64, spike_deg: f64, spike: f32) -> Vec<Option<f32>> {
    let n = rows.len();
    let per_deg = (n - 1) as f64 / 180.0;
    let (edge, half) = ((edge_deg * per_deg).round() as usize, (spike_deg * per_deg).round().max(1.0) as usize);
    // Distance (rows) to the nearest uncovered row; the poles are not edges.
    let near_gap = |j: usize| {
        let lo = j.saturating_sub(edge);
        let hi = (j + edge).min(n - 1);
        rows[lo..=hi].iter().any(Option::is_none)
    };
    (0..n)
        .map(|j| {
            let v = rows[j]?;
            if near_gap(j) {
                return None;
            }
            let mut around: Vec<f32> =
                rows[j.saturating_sub(half)..=(j + half).min(n - 1)].iter().flatten().copied().collect();
            around.sort_by(f32::total_cmp);
            let m = around[around.len() / 2];
            ((v - m).abs() <= spike * m).then_some(v)
        })
        .collect()
}

/// Fills missing rows: linearly between the nearest covered rows, and with
/// the nearest covered row beyond the first and last.
pub fn fill_gaps(rows: &[Option<f32>]) -> Vec<f32> {
    let known: Vec<(usize, f32)> = rows.iter().enumerate().filter_map(|(j, r)| r.map(|v| (j, v))).collect();
    assert!(!known.is_empty(), "no covered rows");
    (0..rows.len())
        .map(|j| match known.binary_search_by_key(&j, |k| k.0) {
            Ok(i) => known[i].1,
            Err(0) => known[0].1,
            Err(i) if i == known.len() => known[i - 1].1,
            Err(i) => {
                let ((j0, v0), (j1, v1)) = (known[i - 1], known[i]);
                v0 + (v1 - v0) * (j - j0) as f32 / (j1 - j0) as f32
            }
        })
        .collect()
}

/// The area-weighted (cos latitude) mean of a `w × h` grid-convention map.
pub fn area_mean(lin: &[[f32; 3]], w: usize, h: usize) -> [f32; 3] {
    let mut sum = [0.0f64; 3];
    let mut weight = 0.0;
    for j in 0..h {
        let lat = (90.0 - (j as f64 + 0.5) * 180.0 / h as f64).to_radians();
        let c = lat.cos();
        for p in &lin[j * w..(j + 1) * w] {
            for k in 0..3 {
                sum[k] += c * f64::from(p[k]);
            }
            weight += c;
        }
    }
    sum.map(|s| (s / weight) as f32)
}

/// Calibrates a map in place to the Lambert albedo 1.5 p per channel (its
/// area mean), reducing its contrast by an exponent γ ≤ 1 (each channel
/// relative to its mean, raised to γ) just enough that the brightest
/// [`TOP_PERCENTILE`] stays under [`MAX_LINEAR`]. Returns the final gains
/// (relative to the input) at the mean, and γ.
pub fn fit_contrast(lin: &mut [[f32; 3]], w: usize, h: usize, p: [f32; 3]) -> ([f32; 3], f32) {
    let target = p.map(|v| 1.5 * v);
    let mean = area_mean(lin, w, h);
    let rel: Vec<[f32; 3]> = lin.iter().map(|c| std::array::from_fn(|k| c[k] / mean[k].max(1e-9))).collect();
    let apply = |gamma: f32| -> Vec<[f32; 3]> {
        let powed: Vec<[f32; 3]> = rel.iter().map(|c| c.map(|v| v.powf(gamma))).collect();
        let m = area_mean(&powed, w, h);
        powed.iter().map(|c| std::array::from_fn(|k| c[k] / m[k] * target[k])).collect()
    };
    let top = |m: &[[f32; 3]]| {
        let mut v: Vec<f32> = m.iter().map(|c| c[0].max(c[1]).max(c[2])).collect();
        v.sort_by(f32::total_cmp);
        v[((v.len() - 1) as f64 * TOP_PERCENTILE) as usize]
    };
    let mut gamma = 1.0;
    if top(&apply(1.0)) > MAX_LINEAR {
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..30 {
            let mid = 0.5 * (lo + hi);
            if top(&apply(mid)) > MAX_LINEAR {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        gamma = lo;
    }
    let out = apply(gamma);
    for (c, o) in lin.iter_mut().zip(out) {
        *c = o.map(|v| v.clamp(0.0, 1.0));
    }
    (std::array::from_fn(|k| target[k] / mean[k]), gamma)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaps_are_interpolated_inside_and_extended_outside() {
        // (rows, filled)
        let cases: [(&[Option<f32>], &[f32]); 3] = [
            (&[None, Some(1.0), None, None, Some(4.0), None], &[1.0, 1.0, 2.0, 3.0, 4.0, 4.0]),
            (&[Some(2.0), Some(3.0)], &[2.0, 3.0]),
            (&[None, None, Some(5.0)], &[5.0, 5.0, 5.0]),
        ];
        for (rows, want) in cases {
            assert_eq!(fill_gaps(rows), want, "{rows:?}");
        }
    }

    #[test]
    fn cleaning_drops_coverage_edges_and_spikes() {
        // 19 rows (10° apart): uncovered at the north pole, a spike at the
        // equator; edges of 10° and a spike window of 20°.
        let mut rows: Vec<Option<f32>> = (0..19).map(|_| Some(1.0)).collect();
        rows[0] = None;
        rows[9] = Some(1.5);
        rows[10] = Some(1.1);
        let clean = clean_profile(&rows, 10.0, 20.0, 0.15);
        assert_eq!(clean[1], None, "next to the uncovered pole");
        assert_eq!(clean[2], Some(1.0));
        assert_eq!(clean[9], None, "the spike");
        assert_eq!(clean[10], Some(1.1), "within 15 %");
        assert_eq!(clean[18], Some(1.0), "a covered pole is not an edge");
    }

    #[test]
    fn row_medians_skip_uncovered_pixels_and_thin_rows() {
        let nan = f32::NAN;
        // Three rows of four: covered, half covered, one pixel covered.
        let data = [1.0, 9.0, 2.0, 3.0, 5.0, 0.0, nan, 7.0, 4.0, 0.0, 0.0, -1.0];
        assert_eq!(row_medians(&data, 4, 3, 0.5), vec![Some(3.0), Some(7.0), None]);
    }

    #[test]
    fn calibration_hits_the_albedo_and_fits_under_one() {
        // A zonal map with a bright band: its mean becomes 1.5 p per channel.
        let (w, h) = (4, 64);
        let mut lin: Vec<[f32; 3]> =
            (0..w * h).map(|i| if (i / w) % 8 < 2 { [0.9, 0.8, 0.7] } else { [0.3, 0.25, 0.2] }).collect();
        let p = [0.3, 0.25, 0.2];
        let (_, gamma) = fit_contrast(&mut lin, w, h, p);
        assert_eq!(gamma, 1.0, "no clipping, contrast kept");
        let m = area_mean(&lin, w, h);
        assert!(m.iter().zip(p).all(|(m, p)| (m - 1.5 * p).abs() < 1e-3), "{m:?}");
        // A bright target forces the contrast down, still at the albedo.
        let mut bright = lin.clone();
        let (_, gamma) = fit_contrast(&mut bright, w, h, [0.6, 0.55, 0.5]);
        assert!(gamma < 1.0 && gamma > 0.0, "{gamma}");
        assert!(bright.iter().all(|c| c.iter().all(|&v| v <= MAX_LINEAR + 1e-3)));
        let m = area_mean(&bright, w, h);
        assert!((m[0] - 0.9).abs() < 2e-3, "{m:?}");
    }

    #[test]
    fn fits_header_finds_the_image() {
        let mut raw = Vec::new();
        for card in [
            "SIMPLE  =                    T",
            "BITPIX  =                  -32 / bits",
            "NAXIS   =                    2",
            "NAXIS1  =                    3",
            "NAXIS2  =                    2",
            "END",
        ] {
            raw.extend(format!("{card:<80}").bytes());
        }
        raw.resize(2880, b' ');
        for v in [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0] {
            raw.extend(v.to_be_bytes());
        }
        assert_eq!(fits_header(&raw), Ok((3, 2, 2880)));
        raw.truncate(2890);
        assert!(fits_header(&raw).is_err(), "truncated data");
    }
}
