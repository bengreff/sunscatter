//! Offline asset baking: terrain heights, colour maps, water masks and the star catalogue (D046).
//!
//! ```text
//! cargo run -p asset-tool --release -- <earth|moon|stars|roughness|all|check> [options]
//! cargo run -p asset-tool --release -- inspect <file.tif>
//! ```
//!
//! Run from the repository root. Sources are downloaded with `curl` into
//! `data/external/` (gitignored) unless already present, then baked into committed files:
//!
//! | Output | Source |
//! |---|---|
//! | `data/bodies/earth/height.png` | NOAA ETOPO 2022, 60″ surface elevation GeoTIFF |
//! | `data/bodies/earth/water.png` | ETOPO 2022 as above: cells below sea level flood-filled from the open ocean (see [`water`]) |
//! | `data/bodies/earth/color.jpg` | NASA Blue Marble Next Generation, July 2004, base (no relief shading) |
//! | `data/bodies/<body>/roughness.png` | the committed `height.png` (see [`roughness`]); Earth and Moon |
//! | `data/bodies/moon/height.png` | LRO LOLA LDEM_64 via the NASA SVS CGI Moon Kit (`ldem_64_uint.tif`) |
//! | `data/bodies/moon/color.jpg` | LROC WAC Hapke-normalised mosaic via the CGI Moon Kit (2025 colour map) |
//! | `data/stars/bsc5.ron` | Yale Bright Star Catalogue, 5th revised ed. (VizieR V/50) |
//!
//! Map format (see [`grid`]): equirectangular, `w = 2h`, left edge −180°, row 0 at the
//! north edge, pixel-centred. Heights are 16-bit grayscale PNG, `u16 = round(m) + 32768`.
//! Colours are sRGB JPEG. Water masks are 8-bit grayscale PNG, the sea fraction of
//! each pixel (0 land, 255 sea). All resampling is an area average (colour in linear light).
//!
//! Options: `--height-width N` / `--color-width N` / `--water-width N` override the
//! output widths (the defaults are the committed resolutions), `--only
//! height|color|water|roughness` bakes one map, and
//! `roughness` bakes only the roughness maps (from the committed heightmaps, no downloads);
//! `--only roughness` does the same for one body.
//! `--previews` writes small PNG previews to `data/external/previews/`.
//!
//! `check` prints spot heights (Everest, Challenger Deep, LC-39A, Tycho) from the
//! committed files.

mod bodies;
mod color;
mod grid;
mod resample;
mod roughness;
mod source;
mod stars;
mod water;

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let int =
        |name: &str| flag(name).map(|w| w.parse::<usize>().unwrap_or_else(|_| panic!("{name} must be an integer")));
    let opts = bodies::Opts {
        height_w: int("--height-width"),
        color_w: int("--color-width"),
        water_w: int("--water-width"),
        only: flag("--only").map(|o| match o.as_str() {
            "height" => bodies::Map::Height,
            "color" => bodies::Map::Color,
            "water" => bodies::Map::Water,
            "roughness" => bodies::Map::Roughness,
            _ => panic!("--only must be height, color, water or roughness"),
        }),
        previews: args.iter().any(|a| a == "--previews"),
    };
    match args.first().map(String::as_str) {
        Some("earth") => bodies::earth(opts),
        Some("moon") => bodies::moon(opts),
        Some("stars") => stars::run(),
        Some("roughness") => {
            roughness::bake("earth");
            roughness::bake("moon");
        }
        Some("all") => {
            bodies::earth(opts);
            bodies::moon(opts);
            stars::run();
            bodies::check();
        }
        Some("check") => bodies::check(),
        Some("inspect") => {
            let t = source::read_tiff(Path::new(args.get(1).expect("inspect needs a file")));
            println!("    decoded {}×{}", t.w, t.h);
        }
        _ => {
            eprintln!("usage: asset-tool <earth|moon|stars|roughness|all|check> [--height-width N] [--color-width N] [--water-width N] [--only height|color|water|roughness] [--previews] | inspect <file>");
            std::process::exit(2);
        }
    }
}
