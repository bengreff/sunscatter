//! Downloading sources into `data/external/` and reading (Geo)TIFFs.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

use tiff::decoder::{Decoder, DecodingResult, Limits};
use tiff::tags::Tag;

pub const EXTERNAL: &str = "data/external";

/// Returns `data/external/<name>`, downloading `url` there with `curl` first if the
/// file is missing. Partial downloads go to `<name>.part` and are renamed on success.
pub fn fetch(url: &str, name: &str) -> PathBuf {
    let dir = Path::new(EXTERNAL);
    std::fs::create_dir_all(dir).expect("create data/external");
    let path = dir.join(name);
    if path.exists() {
        println!("  have {}", path.display());
        return path;
    }
    println!("  downloading {url}");
    let part = dir.join(format!("{name}.part"));
    let status = Command::new("curl")
        .args(["-fL", "--retry", "3", "--progress-bar", "-o"])
        .arg(&part)
        .arg(url)
        .status()
        .expect("run curl (is it installed?)");
    assert!(status.success(), "download failed: {url}");
    std::fs::rename(&part, &path).expect("rename download");
    path
}

pub struct Tiff {
    pub w: usize,
    pub h: usize,
    pub data: DecodingResult,
}

/// Reads the first image of a TIFF, printing its size, colour type and GeoTIFF
/// georeferencing tags (pixel scale and tie point) when present.
pub fn read_tiff(path: &Path) -> Tiff {
    println!("  reading {}", path.display());
    let file = File::open(path).unwrap_or_else(|e| panic!("open {}: {e}", path.display()));
    let mut dec = Decoder::new(BufReader::new(file)).expect("TIFF header").with_limits(Limits::unlimited());
    let (w, h) = dec.dimensions().expect("TIFF dimensions");
    let color = dec.colortype().expect("TIFF colour type");
    let scale = dec.get_tag_f64_vec(Tag::ModelPixelScaleTag).ok();
    let tie = dec.get_tag_f64_vec(Tag::ModelTiepointTag).ok();
    println!("    {w}×{h} {color:?} pixel scale {scale:?} tie point {tie:?}");
    let data = dec.read_image().expect("decode TIFF");
    Tiff { w: w as usize, h: h as usize, data }
}

/// Checks GeoTIFF georeferencing for a full-globe, pixel-area-registered grid whose
/// top-left corner is (−180°, 90°), i.e. our grid convention.
pub fn check_global_geotiff(path: &Path) {
    let file = File::open(path).expect("open TIFF");
    let mut dec = Decoder::new(BufReader::new(file)).expect("TIFF header");
    let (w, h) = dec.dimensions().expect("TIFF dimensions");
    let scale = dec.get_tag_f64_vec(Tag::ModelPixelScaleTag).expect("GeoTIFF pixel scale");
    let tie = dec.get_tag_f64_vec(Tag::ModelTiepointTag).expect("GeoTIFF tie point");
    let close = |a: f64, b: f64| (a - b).abs() < 1e-6;
    assert!(close(scale[0] * f64::from(w), 360.0) && close(scale[1] * f64::from(h), 180.0), "not a global grid");
    // Tie point maps raster (0, 0) — the top-left corner of the top-left pixel in the
    // default PixelIsArea raster space — to model (lon, lat).
    assert!(close(tie[0], 0.0) && close(tie[1], 0.0), "unexpected tie point raster position");
    assert!(close(tie[3], -180.0) && close(tie[4], 90.0), "grid does not start at (−180°, 90°): {tie:?}");
}
