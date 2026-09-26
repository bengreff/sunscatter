//! Earth and Moon: heightmaps and colour maps.

use std::path::{Path, PathBuf};

use tiff::decoder::DecodingResult;

use crate::color::{linear_lut, resample_rgb, write_jpeg, write_preview};
use crate::grid::{encode_height, write_height_png, HeightMap};
use crate::resample::resample_area;
use crate::source::{check_global_geotiff, fetch, read_tiff, EXTERNAL};

const ETOPO_URL: &str = "https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/60s/60s_surface_elev_gtif/ETOPO_2022_v1_60s_N90W180_surface.tif";
const BMNG_URL: &str = "https://assets.science.nasa.gov/content/dam/science/esd/eo/images/bmng/bmng-base/july/world.200407.3x21600x10800_geo.tif";
const LDEM_URL: &str = "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/ldem_64_uint.tif";
const LROC_URL: &str = "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/lroc_color_16bit_srgb_8k.tif";

/// Default output widths (height = width / 2). See the A3 notes in the plan for why.
const EARTH_HEIGHT_W: usize = 8192;
const EARTH_COLOR_W: usize = 8192;
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

/// Command-line options for the body bakes.
#[derive(Clone, Copy, Default)]
pub struct Opts {
    /// Override the heightmap width.
    pub height_w: Option<usize>,
    /// Override the colour map width.
    pub color_w: Option<usize>,
    /// Bake only the heights (`Some(true)`) or only the colours (`Some(false)`).
    pub only_heights: Option<bool>,
    /// Write small previews to `data/external/previews/`.
    pub previews: bool,
}

pub fn earth(o: Opts) {
    if o.only_heights != Some(false) {
        earth_heights(o);
    }
    if o.only_heights != Some(true) {
        earth_color(o);
    }
}

pub fn moon(o: Opts) {
    if o.only_heights != Some(false) {
        moon_heights(o);
    }
    if o.only_heights != Some(true) {
        moon_color(o);
    }
}

fn earth_heights(o: Opts) {
    println!("Earth heights (ETOPO 2022 60″ surface)");
    let path = fetch(ETOPO_URL, "ETOPO_2022_v1_60s_N90W180_surface.tif");
    check_global_geotiff(&path);
    let t = read_tiff(&path);
    let DecodingResult::F32(src) = t.data else { panic!("ETOPO: expected float32 samples") };
    bake_heights("earth", t.w, t.h, o.height_w.unwrap_or(EARTH_HEIGHT_W), o.previews, |j, buf| {
        buf.copy_from_slice(&src[j * t.w..(j + 1) * t.w]);
    });
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

/// Prints spot heights from the committed maps.
pub fn check() {
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
        ] {
            let (lo, hi) = m.min_max_near(lon, lat, 1);
            println!("  {name:<16} {:>6} m (3×3 range {lo} .. {hi})", m.sample(lon, lat));
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
