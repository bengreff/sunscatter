//! Planets and moons with global maps beyond Earth and the Moon (LANDS,
//! 2026-10-26; `docs/features/solar-visuals.md`): heights from a global DEM
//! and colour from a global mosaic, baked like the Moon's into
//! `data/bodies/<body>/` (`height.png`, `color.jpg`), then roughness and
//! zones from the committed maps. Every body is one [`Spec`] in [`SPECS`];
//! bodies without a DEM get procedural relief instead ([`crate::relief`]).
//!
//! **Heights.** A DEM's samples are metres above a reference sphere (or
//! radii minus its radius, as MOLA's MEGDRs; heights above a geoid would need
//! the geoid added first). The bake area-averages them (no-data samples
//! excluded, gaps filled smoothly by push-pull,
//! [`crate::global`]), turns each output pixel into a radius and subtracts
//! the radius of the body's reference spheroid from its `body.ron`
//! (`radius_eq`, `radius_polar`, geocentric latitude, as
//! `sim::body::BodyPhysical::ellipsoid_radius`), so the committed heights are
//! what the simulation adds to its spheroid: the drawn and the physical
//! surface are the measured one whatever datum the source used.
//!
//! **Colour.** 8-bit mosaics are taken as sRGB-coded and averaged in linear
//! light; zero is no data where the mosaic says so. Grey mosaics are written
//! grey: a zone's colour comes from its sourced albedo, the map only
//! classifies and tints (`docs/features/crisp-surfaces.md`).

use std::path::Path;

use serde::Deserialize;

use crate::bodies::{out_dir, preview_path, report_size, Map, Opts, JPEG_QUALITY};
use crate::color::{linear_lut, linear_to_srgb, write_jpeg, write_preview};
use crate::global::{fill_gaps, read_geotiff, read_pds_i16_msb, resample_masked};
use crate::grid::{encode_height, write_height_png};
use crate::relief::{generate, Relief};
use crate::source::fetch;
pub use crate::specs::SPECS;

pub(crate) const USGS: &str = "https://asc-pds-services.s3.us-west-2.amazonaws.com/mosaic/";

/// A global DEM: `height = sample · scale + offset` above a sphere of
/// radius `sphere_m`, for raw samples within `valid` (others are no data).
/// A GeoTIFF, or with `raw = (w, h, left edge °)` a raw big-endian `i16`
/// PDS grid (MOLA MEGDRs).
pub struct HeightSource {
    pub url: &'static str,
    pub file: &'static str,
    pub scale: f64,
    pub offset: f64,
    pub valid: (f32, f32),
    pub sphere_m: f64,
    pub raw: Option<(usize, usize, f64)>,
}

/// A global colour (or grey) mosaic of 8-bit sRGB-coded samples. `bands`:
/// the source channels used as R, G, B (one channel repeated for a grey
/// mosaic).
pub struct ColorSource {
    pub url: &'static str,
    pub file: &'static str,
    pub bands: [usize; 3],
    /// Samples equal to zero (in every used band) are no data.
    pub zero_is_no_data: bool,
}

/// Where a body's heights come from.
pub enum Heights {
    /// A measured global DEM.
    Dem(HeightSource),
    /// Procedural relief from the body's statistics ([`crate::relief`]).
    Procedural(Relief),
}

/// Where a body's colour map comes from.
pub enum Colors {
    /// A global mosaic.
    Mosaic(ColorSource),
    /// The body's measured colour (sRGB), brightened around fresh craters
    /// (from the procedural relief).
    Procedural([u8; 3]),
}

/// One body's sources and output widths (height = width / 2).
pub struct Spec {
    pub body: &'static str,
    pub height: Heights,
    pub color: Colors,
    /// Metres per stored height unit (`height_scale` in `body.ron`; 1
    /// unless the relief exceeds ±32,767 m).
    pub height_scale: f64,
    pub height_w: usize,
    pub color_w: usize,
    pub zones_w: usize,
}

/// The source URL: absolute, or a file name in the USGS mosaic bucket.
fn url(u: &str) -> String {
    if u.starts_with("https://") {
        u.to_string()
    } else {
        format!("{USGS}{u}")
    }
}

/// The local copy of a source, downloaded once. `zip#member` names a file
/// inside a zip archive, extracted next to it.
fn source_path(u: &str, file: &str) -> std::path::PathBuf {
    let Some((zip, member)) = u.split_once('#') else { return fetch(&url(u), file) };
    let name = member.rsplit('/').next().expect("member name");
    let path = Path::new(crate::source::EXTERNAL).join(name);
    if !path.exists() {
        let archive = fetch(&url(zip), zip.rsplit('/').next().expect("zip name"));
        let status = std::process::Command::new("unzip")
            .args(["-o", "-j", "-q"])
            .arg(&archive)
            .arg(member)
            .arg("-d")
            .arg(crate::source::EXTERNAL)
            .status()
            .expect("run unzip");
        assert!(status.success(), "unzip {member}");
    }
    path
}

/// The reference spheroid from a body's `body.ron`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spheroid {
    pub a: f64,
    pub c: f64,
}

impl Spheroid {
    /// Radius (m) at planetocentric latitude `lat_deg`: the formula of
    /// `sim::body::BodyPhysical::ellipsoid_radius`.
    pub fn radius(&self, lat_deg: f64) -> f64 {
        let s = lat_deg.to_radians().sin();
        let s2 = s * s;
        (self.a * self.c) / (self.c * self.c * (1.0 - s2) + self.a * self.a * s2).sqrt()
    }
}

#[derive(Deserialize)]
struct BodyFile {
    physical: PhysicalFile,
}

#[derive(Deserialize)]
struct PhysicalFile {
    radius_eq: f64,
    radius_polar: f64,
}

/// Reads the reference spheroid of `data/bodies/<body>/body.ron`.
pub fn spheroid(body: &str) -> Spheroid {
    spheroid_in(Path::new("data/bodies"), body)
}

/// [`spheroid`] from a bodies directory `dir`.
pub fn spheroid_in(dir: &Path, body: &str) -> Spheroid {
    let path = dir.join(body).join("body.ron");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let f: BodyFile = ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Spheroid { a: f.physical.radius_eq, c: f.physical.radius_polar }
}

/// Latitude (degrees) of the centre of row `j` of `h`.
fn row_lat(j: usize, h: usize) -> f64 {
    90.0 - (j as f64 + 0.5) * 180.0 / h as f64
}

/// Converts heights above a sphere of radius `sphere_m` on a `w × h` grid
/// into heights above the body's spheroid.
pub fn to_spheroid(heights: &mut [f32], w: usize, h: usize, sphere_m: f64, s: Spheroid) {
    for j in 0..h {
        let offset = sphere_m - s.radius(row_lat(j, h));
        for v in &mut heights[j * w..(j + 1) * w] {
            *v = (f64::from(*v) + offset) as f32;
        }
    }
}

/// Bakes the maps `o` asks for, for every body in `bodies` (all specs if empty).
pub fn run(bodies: &[String], o: Opts) {
    for spec in SPECS.iter().filter(|s| bodies.is_empty() || bodies.iter().any(|b| b == s.body)) {
        bake(spec, o);
    }
}

fn bake(spec: &Spec, o: Opts) {
    let body = spec.body;
    let (hw, cw) = (o.height_w.unwrap_or(spec.height_w), o.color_w.unwrap_or(spec.color_w));
    // The procedural relief also gives the procedural colour its fresh craters.
    let relief = match &spec.height {
        Heights::Procedural(r) if o.wants(Map::Height) || o.wants(Map::Color) => {
            println!("{body} procedural relief");
            Some(generate(r, spheroid(body), hw, hw / 2))
        }
        _ => None,
    };
    if o.wants(Map::Height) {
        match (&spec.height, &relief) {
            (Heights::Dem(hs), _) => bake_height(body, hs, hw, spec.height_scale, o.previews),
            (Heights::Procedural(_), Some(r)) => {
                write_heights(body, &r.heights, hw, hw / 2, spec.height_scale, o.previews)
            }
            _ => {}
        }
    }
    if o.wants(Map::Color) {
        match &spec.color {
            Colors::Mosaic(cs) => bake_color(body, cs, cw, o.previews),
            Colors::Procedural(srgb) => {
                let r = relief.as_ref().expect("a procedural colour needs procedural relief");
                procedural_color(body, *srgb, &r.brightness, hw, cw, o.previews);
            }
        }
    }
    if o.wants(Map::Roughness) && out_dir(body).join("height.png").exists() {
        crate::roughness::bake(body);
    }
    if o.wants(Map::Zones) {
        let preview = o.previews.then(|| preview_path(&format!("{body}_zones.png")));
        crate::zones::bake(body, o.zones_w.unwrap_or(spec.zones_w), preview.as_deref());
    }
}

/// Writes `color.jpg` (`cw` wide) as the body's colour times the relief's
/// brightness factor (a `hw`-wide grid), area-averaged.
fn procedural_color(body: &str, srgb: [u8; 3], brightness: &[f32], hw: usize, cw: usize, previews: bool) {
    let lut = linear_lut(8);
    let base = srgb.map(|c| lut[usize::from(c)]);
    let f = crate::resample::resample_area(hw, hw / 2, 1, cw, cw / 2, |j, buf| {
        buf.copy_from_slice(&brightness[j * hw..(j + 1) * hw]);
    });
    let rgb: Vec<u8> =
        f.iter().flat_map(|&k| base.map(|b| (linear_to_srgb((b * k).clamp(0.0, 1.0)) * 255.0).round() as u8)).collect();
    let path = out_dir(body).join("color.jpg");
    write_jpeg(&path, cw, cw / 2, &rgb, JPEG_QUALITY);
    report_size(&path, cw, cw / 2);
    if previews {
        write_preview(&preview_path(&format!("{body}_color.png")), cw, cw / 2, &rgb, 1024);
    }
}

fn bake_height(body: &str, hs: &HeightSource, w: usize, scale: f64, previews: bool) {
    println!("{body} heights ({})", hs.file);
    let h = w / 2;
    let path = source_path(hs.url, hs.file);
    let grid = match hs.raw {
        Some((gw, gh, left)) => read_pds_i16_msb(&path, gw, gh, left),
        None => read_geotiff(&path),
    };
    let (lo, hi) = hs.valid;
    println!("  resampling heights {}×{} → {w}×{h}", grid.w, grid.h);
    let mut heights =
        resample_masked(&grid, w, h, |v| (v >= lo && v <= hi).then(|| (f64::from(v) * hs.scale + hs.offset) as f32));
    drop(grid);
    let gaps = heights.iter().filter(|v| v.is_nan()).count();
    if gaps > 0 {
        println!("  {:.1} % of the map has no data: filled smoothly", 100.0 * gaps as f64 / heights.len() as f64);
    }
    fill_gaps(&mut heights, w, h, 1, 0.0);
    to_spheroid(&mut heights, w, h, hs.sphere_m, spheroid(body));
    write_heights(body, &heights, w, h, scale, previews);
}

/// Writes `height.png` (`scale` metres per step) and an optional preview.
pub fn write_heights(body: &str, heights: &[f32], w: usize, h: usize, scale: f64, previews: bool) {
    let (lo, hi) = heights.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    println!("  range {lo:.0} m .. {hi:.0} m above the spheroid ({scale} m per step)");
    let s = scale as f32;
    assert!(lo / s > -32768.0 && hi / s < 32767.0, "heights out of the 16-bit range: raise height_scale");
    let encoded: Vec<u16> = heights.iter().map(|&v| encode_height(v / s)).collect();
    let path = out_dir(body).join("height.png");
    write_height_png(&path, w, h, &encoded);
    report_size(&path, w, h);
    if previews {
        let gray: Vec<u8> = heights.iter().flat_map(|&v| [((v - lo) / (hi - lo) * 255.0).round() as u8; 3]).collect();
        write_preview(&preview_path(&format!("{body}_height.png")), w, h, &gray, 1024);
    }
}

fn bake_color(body: &str, cs: &ColorSource, w: usize, previews: bool) {
    println!("{body} colour ({})", cs.file);
    let h = w / 2;
    let grid = read_geotiff(&source_path(cs.url, cs.file));
    let lut = linear_lut(8);
    let lin = |v: f32| lut[(v.clamp(0.0, 255.0)) as usize];
    println!("  resampling colour {}×{} ({} channels) → {w}×{h}", grid.w, grid.h, grid.channels);
    let mut px = resample_masked(&grid, w, h, |v| (!(cs.zero_is_no_data && v == 0.0)).then(|| lin(v)));
    let c = grid.channels;
    drop(grid);
    fill_gaps(&mut px, w, h, c, 0.0);
    let rgb: Vec<u8> = px
        .chunks_exact(c)
        .flat_map(|p| cs.bands.map(|b| (linear_to_srgb(p[b].clamp(0.0, 1.0)) * 255.0).round() as u8))
        .collect();
    let path = out_dir(body).join("color.jpg");
    write_jpeg(&path, w, h, &rgb, JPEG_QUALITY);
    report_size(&path, w, h);
    if previews {
        write_preview(&preview_path(&format!("{body}_color.png")), w, h, &rgb, 1024);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spheroid_radius_matches_the_axes() {
        let s = Spheroid { a: 3_396_190.0, c: 3_376_200.0 };
        assert_eq!(s.radius(0.0), 3_396_190.0);
        assert!((s.radius(90.0) - 3_376_200.0).abs() < 1e-6);
        assert!((s.radius(-90.0) - 3_376_200.0).abs() < 1e-6);
        assert!(s.radius(45.0) < s.a && s.radius(45.0) > s.c);
    }

    #[test]
    fn datums_become_heights_above_the_spheroid() {
        let s = Spheroid { a: 1000.0, c: 900.0 };
        let (w, h) = (4, 2);
        let lat = row_lat(0, h);
        // A sphere of 1000 m with 10 m of relief.
        let mut z = vec![10.0f32; w * h];
        to_spheroid(&mut z, w, h, 1000.0, s);
        assert!((f64::from(z[0]) - (1010.0 - s.radius(lat))).abs() < 1e-3);
        // Heights that reach the sphere everywhere: the sphere above the spheroid.
        let mut z = vec![0.0f32; w * h];
        to_spheroid(&mut z, w, h, 1000.0, s);
        assert!(z
            .iter()
            .enumerate()
            .all(|(k, &v)| (f64::from(v) - (1000.0 - s.radius(row_lat(k / w, h)))).abs() < 1e-3));
    }

    #[test]
    fn every_spec_names_a_body_with_data() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bodies");
        for spec in SPECS {
            let s = spheroid_in(&dir, spec.body);
            assert!(s.a >= s.c && s.c > 0.0, "{}", spec.body);
            for w in [spec.height_w, spec.color_w, spec.zones_w] {
                assert!(w % 2 == 0 && w >= 256, "{}: width {w}", spec.body);
            }
        }
    }

    #[derive(Deserialize)]
    struct Visual {
        #[serde(default)]
        color_map: Option<String>,
        #[serde(default)]
        zones: Option<VisualZones>,
    }

    #[derive(Deserialize)]
    struct VisualZones {
        map: String,
        list: Vec<VisualZone>,
    }

    #[derive(Deserialize)]
    struct VisualZone {
        name: String,
        #[serde(default)]
        albedo: Option<[f32; 3]>,
        #[serde(default)]
        water: bool,
    }

    #[derive(Deserialize)]
    struct Body {
        physical: BodyMaps,
    }

    #[derive(Deserialize)]
    struct BodyMaps {
        #[serde(default)]
        heightmap: Option<String>,
        #[serde(default)]
        roughness: Option<String>,
    }

    /// Every body's committed maps: the referenced files exist, maps are
    /// equirectangular (`w = 2h`), zone maps index their lists, land zones
    /// have a reflectance in (0, 1), and every baked spec's maps are there.
    #[test]
    fn shipped_maps_are_complete_and_consistent() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bodies");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let d = entry.unwrap().path();
            let name = d.file_name().unwrap().to_string_lossy().to_string();
            let Ok(text) = std::fs::read_to_string(d.join("visual.ron")) else { continue };
            let v: Visual = ron::from_str(&text).unwrap_or_else(|e| panic!("{name}/visual.ron: {e}"));
            let b: Body = ron::from_str(&std::fs::read_to_string(d.join("body.ron")).unwrap())
                .unwrap_or_else(|e| panic!("{name}/body.ron: {e}"));
            // A colour map without zones may be any image (e.g. a giant planet's bands).
            let zoned_color = v.zones.as_ref().and(v.color_map.as_ref());
            for f in [zoned_color, b.physical.heightmap.as_ref(), b.physical.roughness.as_ref()].into_iter().flatten() {
                let (w, h) = image::image_dimensions(d.join(f)).unwrap_or_else(|e| panic!("{name}/{f}: {e}"));
                assert_eq!(w, 2 * h, "{name}/{f}");
            }
            if let Some(z) = &v.zones {
                assert!(v.color_map.is_some(), "{name}: zones need a colour map");
                assert!(!z.list.is_empty() && z.list.len() < 16, "{name}: 1..=15 zones");
                for zone in z.list.iter().filter(|z| !z.water) {
                    let a = zone.albedo.unwrap_or_else(|| panic!("{name}: {} has no albedo", zone.name));
                    assert!(a.iter().all(|&x| x > 0.0 && x < 1.0), "{name}: {}", zone.name);
                }
                let img = image::open(d.join(&z.map)).unwrap().into_luma8();
                assert_eq!(img.width(), 2 * img.height(), "{name}/{}", z.map);
                let max = img.as_raw().iter().copied().max().unwrap();
                assert!(usize::from(max) <= z.list.len(), "{name}: zone index {max} past the list");
            }
            checked += 1;
        }
        assert!(checked > 40, "every body directory: {checked}");
        for spec in SPECS {
            let d = dir.join(spec.body);
            for f in ["height.png", "color.jpg", "roughness.png"] {
                assert!(d.join(f).exists(), "{}: {f} not baked", spec.body);
            }
        }
    }
}
