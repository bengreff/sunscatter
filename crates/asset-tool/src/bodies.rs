//! Earth and Moon: heightmaps, colour maps, roughness and (Earth) the water mask.

use std::path::{Path, PathBuf};

use tiff::decoder::DecodingResult;

use crate::color::{linear_lut, resample_rgb, write_jpeg, write_preview};
use crate::grid::{encode_height, pixel_of, write_height_png, HeightMap};
use crate::resample::resample_area;
use crate::source::{check_global_geotiff, fetch, read_tiff, EXTERNAL};
use crate::water::{coverage, ocean_mask, write_mask_png, SEED_DEPTH_M};

const ETOPO_URL: &str = "https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/ETOPO_2022_v1_60s_N90W180_surface.tif";
const BMNG_URL: &str = "https://assets.science.nasa.gov/content/dam/science/esd/eo/images/bmng/bmng-base/july/world.200407.3x21600x10800_geo.tif";
const LDEM_URL: &str = "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/ldem_64_uint.tif";
const LROC_URL: &str = "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/lroc_color_16bit_srgb_8k.tif";

/// Default output widths (height = width / 2). See the A3 notes in the plan for why.
const EARTH_HEIGHT_W: usize = 8192;
const EARTH_COLOR_W: usize = 8192;
/// Matches the colour map, so coastlines in both agree.
const EARTH_WATER_W: usize = 8192;
const MOON_HEIGHT_W: usize = 4096;
const MOON_COLOR_W: usize = 8192;
const JPEG_QUALITY: u8 = 90;
const MOON_RADIUS_M: f64 = 1_737_400.0;

fn out_dir(body: &str) -> PathBuf {
    let dir = Path::new("data/bodies").join(body);
    std::fs::create_dir_all(&dir).expect("create data/bodies/<body>");
    dir
}

fn preview_path(name: &str) -> PathBuf {
    let dir = Path::new(EXTERNAL).join("previews");
    std::fs::create_dir_all(&dir).expect("create previews dir");
    dir.join(name)
}

fn report_size(path: &Path, w: usize, h: usize) {
    let bytes = std::fs::metadata(path).expect("stat output").len();
    println!("  wrote {} ({w}×{h}, {:.1} MB)", path.display(), bytes as f64 / 1e6);
}

/// Area-averages a height grid (metres, via `row`) and writes `height.png`.
fn bake_heights(
    body: &str,
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    previews: bool,
    row: impl FnMut(usize, &mut [f32]),
) {
    let dst_h = dst_w / 2;
    println!("  resampling heights {src_w}×{src_h} → {dst_w}×{dst_h}");
    let heights = resample_area(src_w, src_h, 1, dst_w, dst_h, row);
    assert!(heights.iter().all(|h| h.is_finite()), "non-finite height");
    let (lo, hi) = heights.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    println!("  range {lo:.0} m .. {hi:.0} m");
    let encoded: Vec<u16> = heights.iter().map(|&h| encode_height(h)).collect();
    let path = out_dir(body).join("height.png");
    write_height_png(&path, dst_w, dst_h, &encoded);
    report_size(&path, dst_w, dst_h);
    if previews {
        let gray: Vec<u8> = heights.iter().flat_map(|&h| [((h - lo) / (hi - lo) * 255.0).round() as u8; 3]).collect();
        write_preview(&preview_path(&format!("{body}_height.png")), dst_w, dst_h, &gray, 1024);
    }
}

fn bake_color<T: Copy + Into<u32>>(
    body: &str,
    src: &[T],
    src_w: usize,
    src_h: usize,
    bits: u32,
    dst_w: usize,
    previews: bool,
) {
    let dst_h = dst_w / 2;
    println!("  resampling colour {src_w}×{src_h} → {dst_w}×{dst_h}");
    let rgb = resample_rgb(src, src_w, src_h, &linear_lut(bits), dst_w, dst_h);
    let path = out_dir(body).join("color.jpg");
    write_jpeg(&path, dst_w, dst_h, &rgb, JPEG_QUALITY);
    report_size(&path, dst_w, dst_h);
    if previews {
        write_preview(&preview_path(&format!("{body}_color.png")), dst_w, dst_h, &rgb, 1024);
    }
}

/// One of a body's baked maps.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Map {
    Height,
    Color,
    Water,
    /// Derived from the committed heightmap, after it is baked.
    Roughness,
}

/// Command-line options for the body bakes.
#[derive(Clone, Copy, Default)]
pub struct Opts {
    /// Override the heightmap width.
    pub height_w: Option<usize>,
    /// Override the colour map width.
    pub color_w: Option<usize>,
    /// Override the water mask width.
    pub water_w: Option<usize>,
    /// Bake only this map.
    pub only: Option<Map>,
    /// Write small previews to `data/external/previews/`.
    pub previews: bool,
}

impl Opts {
    fn wants(&self, m: Map) -> bool {
        self.only.is_none_or(|o| o == m)
    }
}

pub fn earth(o: Opts) {
    if o.wants(Map::Height) || o.wants(Map::Water) {
        earth_heights_and_water(o);
    }
    if o.wants(Map::Color) {
        earth_color(o);
    }
    if o.wants(Map::Roughness) {
        crate::roughness::bake("earth");
    }
}

pub fn moon(o: Opts) {
    if o.wants(Map::Height) {
        moon_heights(o);
    }
    if o.wants(Map::Color) {
        moon_color(o);
    }
    if o.wants(Map::Roughness) {
        crate::roughness::bake("moon");
    }
}

/// Heights and the water mask both come from ETOPO, read once.
fn earth_heights_and_water(o: Opts) {
    println!("Earth heights and water (ETOPO 2022 60″ surface)");
    let path = fetch(ETOPO_URL, "ETOPO_2022_v1_60s_N90W180_surface.tif");
    check_global_geotiff(&path);
    let t = read_tiff(&path);
    let DecodingResult::F32(src) = t.data else { panic!("ETOPO: expected float32 samples") };
    if o.wants(Map::Height) {
        bake_heights("earth", t.w, t.h, o.height_w.unwrap_or(EARTH_HEIGHT_W), o.previews, |j, buf| {
            buf.copy_from_slice(&src[j * t.w..(j + 1) * t.w]);
        });
    }
    if o.wants(Map::Water) {
        // ETOPO heights are relative to mean sea level, Earth's sea level (body.ron).
        bake_water("earth", &src, t.w, t.h, 0.0, o.water_w.unwrap_or(EARTH_WATER_W), o.previews);
    }
}

/// Flood-fills the open sea at source resolution and writes `water.png` (see
/// [`crate::water`]).
fn bake_water(body: &str, heights: &[f32], w: usize, h: usize, sea: f32, dst_w: usize, previews: bool) {
    let dst_h = dst_w / 2;
    println!("  flood-filling the sea from cells below {sea} − {SEED_DEPTH_M} m");
    let mask = ocean_mask(heights, w, h, sea, SEED_DEPTH_M);
    let below = heights.iter().filter(|&&z| z < sea).count();
    let sea_cells = mask.iter().filter(|&&m| m == 1).count();
    println!(
        "  sea {:.1} % of cells; {} cells below sea level left as land",
        100.0 * sea_cells as f64 / mask.len() as f64,
        below - sea_cells
    );
    println!("  resampling water {w}×{h} → {dst_w}×{dst_h}");
    let cov = coverage(&mask, w, h, dst_w, dst_h);
    let path = out_dir(body).join("water.png");
    write_mask_png(&path, dst_w, dst_h, &cov);
    report_size(&path, dst_w, dst_h);
    if previews {
        let rgb: Vec<u8> = cov.iter().flat_map(|&c| [c; 3]).collect();
        write_preview(&preview_path(&format!("{body}_water.png")), dst_w, dst_h, &rgb, 1024);
    }
}

fn earth_color(o: Opts) {
    println!("Earth colour (Blue Marble NG, July 2004, base)");
    let path = fetch(BMNG_URL, "world.200407.3x21600x10800_geo.tif");
    check_global_geotiff(&path);
    let t = read_tiff(&path);
    let DecodingResult::U8(src) = t.data else { panic!("BMNG: expected 8-bit RGB") };
    bake_color("earth", &src, t.w, t.h, 8, o.color_w.unwrap_or(EARTH_COLOR_W), o.previews);
}

fn moon_heights(o: Opts) {
    println!("Moon heights (LOLA LDEM_64, CGI Moon Kit uint16)");
    let path = fetch(LDEM_URL, "ldem_64_uint.tif");
    let t = read_tiff(&path);
    assert_eq!((t.w, t.h), (23040, 11520), "unexpected LDEM_64 size");
    let DecodingResult::U16(src) = t.data else { panic!("LDEM: expected uint16 samples") };
    // Kit encoding: half-metres above a 1,727,400 m sphere, i.e. 10 km below the
    // 1,737,400 m reference, so height = v/2 − 10,000 m.
    bake_heights("moon", t.w, t.h, o.height_w.unwrap_or(MOON_HEIGHT_W), o.previews, |j, buf| {
        for (b, &v) in buf.iter_mut().zip(&src[j * t.w..(j + 1) * t.w]) {
            *b = f32::from(v) * 0.5 - 10_000.0;
        }
    });
}

fn moon_color(o: Opts) {
    println!("Moon colour (LROC WAC, CGI Moon Kit 2025 colour map, 16-bit sRGB)");
    let path = fetch(LROC_URL, "lroc_color_16bit_srgb_8k.tif");
    let t = read_tiff(&path);
    assert_eq!(t.w, 2 * t.h, "unexpected LROC aspect");
    match t.data {
        DecodingResult::U16(src) => {
            bake_color("moon", &src, t.w, t.h, 16, o.color_w.unwrap_or(MOON_COLOR_W), o.previews)
        }
        DecodingResult::U8(src) => bake_color("moon", &src, t.w, t.h, 8, o.color_w.unwrap_or(MOON_COLOR_W), o.previews),
        _ => panic!("LROC: expected 8- or 16-bit RGB"),
    }
}

/// Prints spot heights (and water fractions) from the committed maps.
pub fn check() {
    let water = Path::new("data/bodies/earth/water.png").exists().then(|| {
        let img = image::open("data/bodies/earth/water.png").expect("open water.png").into_luma8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        (w, h, img.into_raw())
    });
    let earth = Path::new("data/bodies/earth/height.png");
    if earth.exists() {
        let m = HeightMap::load(earth);
        println!("Earth height.png {}×{}", m.w, m.h);
        for (name, lat, lon) in [
            ("Mount Everest", 27.988, 86.925),
            ("Challenger Deep", 11.37, 142.59),
            ("KSC LC-39A", 28.608, -80.604),
            ("Dead Sea", 31.5, 35.5),
            ("Amsterdam", 52.37, 4.9),
            ("Caspian Sea", 42.0, 51.0),
            ("Black Sea", 43.0, 34.0),
            ("Lake Superior", 47.7, -87.5),
            ("North Sea", 55.0, 3.0),
        ] {
            let (lo, hi) = m.min_max_near(lon, lat, 1);
            let sea = water.as_ref().map_or(String::new(), |(w, h, d)| {
                let (i, j) = pixel_of(lon, lat, *w, *h);
                format!(", water {:.2}", f32::from(d[j * w + i]) / 255.0)
            });
            println!("  {name:<16} {:>6} m (3×3 range {lo} .. {hi}){sea}", m.sample(lon, lat));
        }
    }
    let moon = Path::new("data/bodies/moon/height.png");
    if moon.exists() {
        let m = HeightMap::load(moon);
        println!("Moon height.png {}×{}", m.w, m.h);
        let (lat, lon) = (-43.31f64, -11.36f64);
        let km_per_deg_lon = MOON_RADIUS_M / 1000.0 * lat.to_radians().cos() * std::f64::consts::PI / 180.0;
        print!("  Tycho E-W profile (km from centre: m):");
        for d in (-60..=60).step_by(10) {
            print!(" {d}:{}", m.sample(lon + f64::from(d) / km_per_deg_lon, lat));
        }
        println!();
        for (name, lat, lon) in [("Mare Tranquillitatis", 8.5, 31.4), ("Apollo 11 site", 0.674, 23.473)] {
            println!("  {name:<20} {:>6} m", m.sample(lon, lat));
        }
    }
}
