//! Offline asset pipeline.
//!
//! For wasm targets we pre-pack the sprite atlas at compile time so the runtime
//! can `include_bytes!` it instead of walking the filesystem and decoding 185
//! PNGs in serial in the browser.
//!
//! The packer mirrors the runtime logic in `src/render/sprites.rs` but caps the
//! atlas at 4096px (a width + height that's safe across WebGL2, Safari iOS, and
//! WebGPU). On native builds this entire script is a no-op.

use std::collections::HashMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const ATLAS_MAX_DIM: u32 = 4096;
const MAX_SPRITE_DIM: u32 = 4096;
const PADDING: u32 = 1;

fn main() {
    println!("cargo:rerun-if-changed=data/sprites");
    println!("cargo:rerun-if-changed=data/textures/bodies");
    println!("cargo:rerun-if-changed=build.rs");

    let target = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target != "wasm32" {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR set by cargo"));

    let sprite_dir = Path::new("data/sprites");
    if sprite_dir.exists() {
        pack_atlas(sprite_dir, &out_dir);
    } else {
        eprintln!("data/sprites missing; skipping wasm atlas pack");
    }

    let bodies_dir = Path::new("data/textures/bodies");
    if bodies_dir.exists() {
        bake_body_colors(bodies_dir, &out_dir);
    } else {
        eprintln!("data/textures/bodies missing; skipping body color bake");
    }
}

/// Walk every body texture PNG, compute its average disc color (mirrors the
/// runtime logic in `src/render/textures.rs::compute_average_disc_color`),
/// and emit a `body_colors.ron` map. The wasm runtime embeds this so that
/// pre-fetch flat-color rendering matches what the desktop's `BodyTextureMap`
/// would have produced from the actual PNG.
fn bake_body_colors(bodies_dir: &Path, out_dir: &Path) {
    let Ok(rd) = fs::read_dir(bodies_dir) else { return };

    let mut entries: Vec<(String, [f32; 4])> = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "png" | "jpg" | "jpeg") { continue; }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_lowercase(),
            None => continue,
        };
        match image::open(&path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                entries.push((stem, average_disc_color(&rgba)));
            }
            Err(e) => eprintln!("bake_body_colors decode {}: {}", path.display(), e),
        }
    }

    // Stable ordering for reproducible builds.
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    let _ = writeln!(out, "{{");
    for (name, [r, g, b, a]) in &entries {
        let _ = writeln!(
            out, "    \"{}\": ({}, {}, {}, {}),", name, r, g, b, a,
        );
    }
    let _ = writeln!(out, "}}");

    let path = out_dir.join("body_colors.ron");
    fs::write(&path, out).unwrap_or_else(|e| panic!("write {}: {}", path.display(), e));
    eprintln!("baked {} body disc colors -> {}", entries.len(), path.display());
}

fn average_disc_color(img: &image::RgbaImage) -> [f32; 4] {
    let (w, h) = img.dimensions();
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let disc_r = (w as f64 / 2.0) - 4.0;

    let mut r_sum = 0.0f64;
    let mut g_sum = 0.0f64;
    let mut b_sum = 0.0f64;
    let mut count = 0u64;

    for y in (0..h).step_by(4) {
        for x in (0..w).step_by(4) {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            if dx * dx + dy * dy <= disc_r * disc_r {
                let p = img.get_pixel(x, y);
                r_sum += p[0] as f64;
                g_sum += p[1] as f64;
                b_sum += p[2] as f64;
                count += 1;
            }
        }
    }
    if count == 0 { return [0.5, 0.5, 0.5, 1.0]; }

    let mut r = (r_sum / count as f64 / 255.0) as f32;
    let mut g = (g_sum / count as f64 / 255.0) as f32;
    let mut b = (b_sum / count as f64 / 255.0) as f32;

    let max_ch = r.max(g).max(b);
    if max_ch > 0.0 && max_ch < 0.45 {
        let scale = 0.45 / max_ch;
        r *= scale;
        g *= scale;
        b *= scale;
    }
    [r, g, b, 1.0]
}

struct SpriteEntry {
    id: String,
    image: image::RgbaImage,
    width: u32,
    height: u32,
    /// "part" / "engine" / "plume:<propellant>:<frame>"
    kind: String,
}

struct PackedSprite {
    id: String,
    kind: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn pack_atlas(sprite_dir: &Path, out_dir: &Path) {
    let mut entries = Vec::new();
    entries.extend(collect_dir(&sprite_dir.join("engines"), "engine"));
    entries.extend(collect_dir(&sprite_dir.join("parts"), "part"));
    entries.extend(collect_plumes(&sprite_dir.join("plumes")));
    if entries.is_empty() {
        eprintln!("no sprites found under data/sprites");
        return;
    }

    // Cap individual sprite size before packing.
    for e in entries.iter_mut() {
        if e.width > MAX_SPRITE_DIM || e.height > MAX_SPRITE_DIM {
            let scale = MAX_SPRITE_DIM as f64 / e.width.max(e.height) as f64;
            let nw = ((e.width as f64 * scale) as u32).max(1);
            let nh = ((e.height as f64 * scale) as u32).max(1);
            e.image = image::imageops::resize(&e.image, nw, nh, image::imageops::FilterType::Lanczos3);
            e.width = nw;
            e.height = nh;
        }
    }

    let atlas_width = ATLAS_MAX_DIM;
    let (mut packed, mut atlas_height) = shelf_pack(&mut entries, atlas_width);
    while atlas_height > ATLAS_MAX_DIM {
        eprintln!("atlas {}x{} exceeds {}, downscaling 50%", atlas_width, atlas_height, ATLAS_MAX_DIM);
        for e in entries.iter_mut() {
            let nw = (e.width / 2).max(1);
            let nh = (e.height / 2).max(1);
            e.image = image::imageops::resize(&e.image, nw, nh, image::imageops::FilterType::Lanczos3);
            e.width = nw;
            e.height = nh;
        }
        let r = shelf_pack(&mut entries, atlas_width);
        packed = r.0;
        atlas_height = r.1;
    }

    let mut atlas = image::RgbaImage::new(atlas_width, atlas_height);
    let by_id: HashMap<String, &image::RgbaImage> =
        entries.iter().map(|e| (e.id.clone(), &e.image)).collect();
    for p in &packed {
        let img = by_id[&p.id];
        for y in 0..p.height {
            for x in 0..p.width {
                let px = img.get_pixel(x, y);
                atlas.put_pixel(p.x + x, p.y + y, *px);
            }
        }
    }

    let png_path = out_dir.join("sprite_atlas.png");
    atlas.save(&png_path).unwrap_or_else(|e| panic!("save {}: {}", png_path.display(), e));

    let ron = build_metadata_ron(&packed, atlas_width, atlas_height);
    let ron_path = out_dir.join("sprite_atlas.ron");
    fs::write(&ron_path, ron).unwrap_or_else(|e| panic!("write {}: {}", ron_path.display(), e));

    eprintln!(
        "packed atlas {}x{} ({} sprites) -> {}",
        atlas_width, atlas_height, packed.len(), png_path.display()
    );
}

fn collect_dir(dir: &Path, kind: &str) -> Vec<SpriteEntry> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") { continue; }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        match image::open(&path) {
            Ok(img) => {
                let rgba = img.into_rgba8();
                let (w, h) = rgba.dimensions();
                out.push(SpriteEntry { id: stem, image: rgba, width: w, height: h, kind: kind.to_string() });
            }
            Err(e) => eprintln!("decode {}: {}", path.display(), e),
        }
    }
    out
}

fn collect_plumes(dir: &Path) -> Vec<SpriteEntry> {
    let Ok(rd) = fs::read_dir(dir) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") { continue; }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let Some(idx) = stem.rfind("_frame") else { continue };
        let propellant = &stem[..idx];
        let Ok(frame) = stem[idx + 6..].parse::<usize>() else { continue };
        let kind = format!("plume:{}:{}", propellant, frame);
        match image::open(&path) {
            Ok(img) => {
                let rgba = img.into_rgba8();
                let (w, h) = rgba.dimensions();
                out.push(SpriteEntry { id: stem, image: rgba, width: w, height: h, kind });
            }
            Err(e) => eprintln!("decode plume {}: {}", path.display(), e),
        }
    }
    out
}

fn shelf_pack(entries: &mut Vec<SpriteEntry>, atlas_width: u32) -> (Vec<PackedSprite>, u32) {
    entries.sort_by(|a, b| b.height.cmp(&a.height));
    let mut packed = Vec::new();
    let mut shelf_y = 0u32;
    let mut shelf_height = 0u32;
    let mut cursor_x = 0u32;
    for e in entries.iter() {
        let w = e.width + PADDING;
        let h = e.height + PADDING;
        if cursor_x + w > atlas_width {
            shelf_y += shelf_height;
            shelf_height = 0;
            cursor_x = 0;
        }
        packed.push(PackedSprite {
            id: e.id.clone(),
            kind: e.kind.clone(),
            x: cursor_x,
            y: shelf_y,
            width: e.width,
            height: e.height,
        });
        cursor_x += w;
        shelf_height = shelf_height.max(h);
    }
    let total = shelf_y + shelf_height;
    let mut p = 1u32;
    while p < total { p *= 2; }
    (packed, p)
}

fn build_metadata_ron(packed: &[PackedSprite], aw: u32, ah: u32) -> String {
    let aw_f = aw as f32;
    let ah_f = ah as f32;

    let mut parts = Vec::new();
    let mut plume_frames: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    for p in packed {
        let u_min = p.x as f32 / aw_f;
        let v_min = p.y as f32 / ah_f;
        let u_max = (p.x + p.width) as f32 / aw_f;
        let v_max = (p.y + p.height) as f32 / ah_f;
        let rect = format!(
            "(u_min: {}, v_min: {}, u_max: {}, v_max: {}, pixel_width: {}, pixel_height: {})",
            u_min, v_min, u_max, v_max, p.width, p.height
        );
        if let Some(rest) = p.kind.strip_prefix("plume:") {
            let mut split = rest.splitn(2, ':');
            let propellant = split.next().unwrap_or("").to_string();
            let frame = split.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
            plume_frames.entry(propellant).or_default().push((frame, rect));
        } else {
            parts.push((p.id.clone(), rect));
        }
    }

    parts.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = String::new();
    let _ = writeln!(out, "(");
    let _ = writeln!(out, "    atlas_width: {},", aw);
    let _ = writeln!(out, "    atlas_height: {},", ah);
    let _ = writeln!(out, "    parts: {{");
    for (id, rect) in &parts {
        let _ = writeln!(out, "        \"{}\": {},", id, rect);
    }
    let _ = writeln!(out, "    }},");
    let _ = writeln!(out, "    plumes: {{");
    let mut plumes_sorted: Vec<_> = plume_frames.into_iter().collect();
    plumes_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (propellant, mut frames) in plumes_sorted {
        if frames.len() < 4 { continue; }
        frames.sort_by_key(|(idx, _)| *idx);
        // `frames: [SpriteRect; 4]` serializes as a RON tuple, not a sequence,
        // so emit `frames: ((...), (...), (...), (...))` not `frames: [...]`.
        let _ = writeln!(out, "        \"{}\": (frames: (", propellant);
        for (_, rect) in frames.iter().take(4) {
            let _ = writeln!(out, "            {},", rect);
        }
        let _ = writeln!(out, "        )),");
    }
    let _ = writeln!(out, "    }},");
    let _ = writeln!(out, ")");
    out
}
