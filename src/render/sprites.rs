//! Sprite atlas — loads part/engine/plume PNGs, packs into a single GPU texture,
//! and provides part ID → UV rect mapping.

// On wasm we use the build-time atlas, so the runtime packer + cache helpers
// (build_sprite_atlas, shelf_pack, *_parallel, mtime probes, etc.) are
// intentionally compiled but unreachable. Silence the dead-code chorus there.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
// Filesystem metadata (mtime) returns std::time::SystemTime; use it directly
// rather than web_time::SystemTime so std::fs::Metadata interop type-checks. This
// code path is desktop-only — wasm uses an embedded prebuilt atlas (Phase 3).
use std::time::SystemTime;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use serde::{Serialize, Deserialize};

/// UV rectangle in the atlas (normalized 0–1 coordinates)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpriteRect {
    pub u_min: f32,
    pub v_min: f32,
    pub u_max: f32,
    pub v_max: f32,
    /// Original image pixel width (for aspect ratio preservation)
    #[serde(default)]
    pub pixel_width: u32,
    /// Original image pixel height (for aspect ratio preservation)
    #[serde(default)]
    pub pixel_height: u32,
}

/// 4-frame plume animation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlumeAnimation {
    pub frames: [SpriteRect; 4],
}

/// Serializable cache metadata for the sprite atlas
#[derive(Serialize, Deserialize)]
struct SpriteAtlasMetadata {
    atlas_width: u32,
    atlas_height: u32,
    parts: HashMap<String, SpriteRect>,
    plumes: HashMap<String, PlumeAnimation>,
}

const CACHE_DIR: &str = "data/cache";
const CACHE_ATLAS_PNG: &str = "data/cache/sprite_atlas.png";
const CACHE_ATLAS_RON: &str = "data/cache/sprite_atlas.ron";

/// Sprite atlas: single GPU texture + lookup tables
pub struct SpriteAtlas {
    pub bind_group: wgpu::BindGroup,
    pub parts: HashMap<String, SpriteRect>,
    pub plumes: HashMap<String, PlumeAnimation>,
}

/// Entry for atlas packing (before GPU upload)
struct SpriteEntry {
    id: String,
    image: image::RgbaImage,
    width: u32,
    height: u32,
    /// "part", "engine", or "plume:<propellant>:<frame>"
    kind: String,
}

/// Packed position result
struct PackedSprite {
    id: String,
    kind: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Shelf-based atlas packer
fn shelf_pack(entries: &mut Vec<SpriteEntry>, atlas_width: u32) -> (Vec<PackedSprite>, u32) {
    // Sort by height descending for better shelf packing
    entries.sort_by(|a, b| b.height.cmp(&a.height));

    let mut packed = Vec::new();
    let mut shelf_y: u32 = 0;
    let mut shelf_height: u32 = 0;
    let mut cursor_x: u32 = 0;
    let padding = 1u32; // 1px padding between sprites to avoid bleeding

    for entry in entries.iter() {
        let w = entry.width + padding;
        let h = entry.height + padding;

        // Does it fit in the current shelf?
        if cursor_x + w > atlas_width {
            // Start new shelf
            shelf_y += shelf_height;
            shelf_height = 0;
            cursor_x = 0;
        }

        packed.push(PackedSprite {
            id: entry.id.clone(),
            kind: entry.kind.clone(),
            x: cursor_x,
            y: shelf_y,
            width: entry.width,
            height: entry.height,
        });

        cursor_x += w;
        shelf_height = shelf_height.max(h);
    }

    let total_height = shelf_y + shelf_height;
    // Round to a multiple of 256 for friendly GPU buffer alignment; do NOT
    // round to next power-of-2. wgpu has no POT requirement for D2 textures,
    // and the POT-round used to double atlas_height (e.g. 8300 → 16384,
    // quadrupling RGBA memory and atlas-cache PNG decode time on startup).
    let atlas_height = ((total_height + 255) / 256) * 256;
    (packed, atlas_height)
}

/// Get the newest modification time of any file in the given directories
fn newest_sprite_mtime(dirs: &[&Path]) -> Option<SystemTime> {
    let mut newest: Option<SystemTime> = None;
    for dir in dirs {
        let Ok(read_dir) = std::fs::read_dir(dir) else { continue };
        for entry in read_dir.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(mtime) = meta.modified() else { continue };
            newest = Some(newest.map_or(mtime, |n: SystemTime| n.max(mtime)));
        }
    }
    newest
}

/// Check if cached atlas is fresh (exists and newer than all source sprites)
fn is_cache_fresh(sprite_dir: &Path) -> bool {
    let png_path = Path::new(CACHE_ATLAS_PNG);
    let ron_path = Path::new(CACHE_ATLAS_RON);

    let (Ok(png_meta), Ok(ron_meta)) = (png_path.metadata(), ron_path.metadata()) else {
        return false;
    };
    let (Ok(png_mtime), Ok(ron_mtime)) = (png_meta.modified(), ron_meta.modified()) else {
        return false;
    };
    let cache_mtime = png_mtime.min(ron_mtime);

    let source_dirs = [
        sprite_dir.join("engines"),
        sprite_dir.join("parts"),
        sprite_dir.join("plumes"),
    ];
    let source_refs: Vec<&Path> = source_dirs.iter().map(|p| p.as_path()).collect();

    if let Some(newest_source) = newest_sprite_mtime(&source_refs) {
        cache_mtime >= newest_source
    } else {
        false // no source files = can't validate cache
    }
}

/// Try to load the atlas from cache, returning atlas data + metadata if fresh
fn load_from_cache() -> Option<(Vec<u8>, SpriteAtlasMetadata)> {
    let ron_data = std::fs::read_to_string(CACHE_ATLAS_RON).ok()?;
    let metadata: SpriteAtlasMetadata = ron::from_str(&ron_data).ok()?;

    // Invalidate cache if SpriteRect is missing pixel dimensions (old format)
    if metadata.parts.values().any(|r| r.pixel_width == 0) {
        log::info!("Cache missing sprite pixel dimensions, rebuilding");
        return None;
    }

    let img = image::open(CACHE_ATLAS_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    if w != metadata.atlas_width || h != metadata.atlas_height {
        log::warn!("Cache atlas dimensions mismatch, rebuilding");
        return None;
    }

    Some((img.into_raw(), metadata))
}

/// Save atlas data + metadata to cache
fn save_to_cache(atlas_data: &[u8], metadata: &SpriteAtlasMetadata) {
    if let Err(e) = std::fs::create_dir_all(CACHE_DIR) {
        log::warn!("Failed to create cache dir: {}", e);
        return;
    }

    let img: image::RgbaImage = image::RgbaImage::from_raw(
        metadata.atlas_width, metadata.atlas_height, atlas_data.to_vec(),
    ).unwrap();
    if let Err(e) = img.save(CACHE_ATLAS_PNG) {
        log::warn!("Failed to save atlas cache PNG: {}", e);
        return;
    }

    let ron_str = ron::ser::to_string_pretty(metadata, ron::ser::PrettyConfig::default())
        .unwrap_or_default();
    if let Err(e) = std::fs::write(CACHE_ATLAS_RON, ron_str) {
        log::warn!("Failed to save atlas cache RON: {}", e);
    } else {
        log::info!("Saved sprite atlas cache to {}", CACHE_DIR);
    }
}

/// Load all sprites and create the atlas. On wasm this decodes a build-time
/// pre-packed atlas from `OUT_DIR`; on desktop it walks `data/sprites/` and
/// packs at runtime (with a `data/cache/` fallthrough cache).
pub fn load_sprite_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> SpriteAtlas {
    #[cfg(target_arch = "wasm32")]
    {
        return load_sprite_atlas_embedded(device, queue);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let sprite_dir = Path::new("data/sprites");

        // Try loading from cache first
        let (atlas_data, atlas_width, atlas_height, parts, plumes) = if is_cache_fresh(sprite_dir) {
            if let Some((data, metadata)) = load_from_cache() {
                log::info!(
                    "Loaded sprite atlas from cache ({}x{}, {} parts, {} plume types)",
                    metadata.atlas_width, metadata.atlas_height,
                    metadata.parts.len(), metadata.plumes.len()
                );
                (data, metadata.atlas_width, metadata.atlas_height, metadata.parts, metadata.plumes)
            } else {
                log::info!("Cache files corrupted, rebuilding sprite atlas");
                build_sprite_atlas(sprite_dir, device)
            }
        } else {
            build_sprite_atlas(sprite_dir, device)
        };

        if parts.is_empty() && plumes.is_empty() {
            log::warn!("No sprites loaded, creating dummy sprite atlas");
            return create_dummy_atlas(device, queue);
        }

        upload_atlas_to_gpu(device, queue, &atlas_data, atlas_width, atlas_height, parts, plumes)
    }
}

/// Decode the build-time atlas embedded in the wasm binary and upload to GPU.
#[cfg(target_arch = "wasm32")]
fn load_sprite_atlas_embedded(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> SpriteAtlas {
    const ATLAS_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sprite_atlas.png"));
    const ATLAS_RON: &str = include_str!(concat!(env!("OUT_DIR"), "/sprite_atlas.ron"));

    let img = match image::load_from_memory(ATLAS_PNG) {
        Ok(i) => i.into_rgba8(),
        Err(e) => {
            log::error!("Failed to decode embedded sprite atlas: {}", e);
            return create_dummy_atlas(device, queue);
        }
    };
    let metadata: SpriteAtlasMetadata = match ron::from_str(ATLAS_RON) {
        Ok(m) => m,
        Err(e) => {
            log::error!("Failed to parse embedded sprite atlas metadata: {}", e);
            return create_dummy_atlas(device, queue);
        }
    };

    let (w, h) = img.dimensions();
    if w != metadata.atlas_width || h != metadata.atlas_height {
        log::error!(
            "Embedded atlas dimensions {}x{} disagree with metadata {}x{}",
            w, h, metadata.atlas_width, metadata.atlas_height
        );
        return create_dummy_atlas(device, queue);
    }

    log::info!(
        "Loaded embedded sprite atlas ({}x{}, {} parts, {} plume types)",
        metadata.atlas_width, metadata.atlas_height,
        metadata.parts.len(), metadata.plumes.len()
    );

    upload_atlas_to_gpu(
        device,
        queue,
        &img.into_raw(),
        metadata.atlas_width,
        metadata.atlas_height,
        metadata.parts,
        metadata.plumes,
    )
}

/// Build the sprite atlas from source PNGs: decode, pack, blit, cache, return raw data + metadata
fn build_sprite_atlas(
    sprite_dir: &Path,
    device: &wgpu::Device,
) -> (Vec<u8>, u32, u32, HashMap<String, SpriteRect>, HashMap<String, PlumeAnimation>) {
    // Collect all file paths first (fast, sequential filesystem reads)
    let engine_paths = collect_sprite_paths(&sprite_dir.join("engines"), |_| true);
    let part_paths = collect_sprite_paths(&sprite_dir.join("parts"), |_| true);
    let plume_paths = collect_plume_paths(&sprite_dir.join("plumes"));

    // Decode all PNGs in parallel using rayon
    let mut entries: Vec<SpriteEntry> = Vec::new();
    entries.extend(decode_sprites_parallel(engine_paths, "engine"));
    entries.extend(decode_sprites_parallel(part_paths, "part"));
    entries.extend(decode_plume_sprites_parallel(plume_paths));

    if entries.is_empty() {
        return (Vec::new(), 0, 0, HashMap::new(), HashMap::new());
    }

    log::info!("Packing {} sprites into atlas", entries.len());

    let max_dim = device.limits().max_texture_dimension_2d;
    let atlas_width = max_dim;

    // Downscale any sprite whose width or height exceeds atlas_width.
    // This handles very large parts (e.g. interstellar engines at 4x resolution)
    // that would overflow the atlas blit or fail to pack.
    for entry in entries.iter_mut() {
        if entry.width > atlas_width || entry.height > atlas_width {
            let scale = (atlas_width as f64 / entry.width.max(entry.height) as f64).min(1.0);
            let new_w = ((entry.width as f64 * scale) as u32).max(1);
            let new_h = ((entry.height as f64 * scale) as u32).max(1);
            log::info!(
                "Downscaling oversized sprite '{}' from {}x{} to {}x{}",
                entry.id, entry.width, entry.height, new_w, new_h
            );
            entry.image = image::imageops::resize(
                &entry.image, new_w, new_h, image::imageops::FilterType::Lanczos3,
            );
            entry.width = new_w;
            entry.height = new_h;
        }
    }

    // Tiered pre-scaling: keep small/medium sprites at full resolution for
    // crisp detail; scale down large sprites and sphere tanks to fit within
    // the GPU texture limit without the iterative halving that used to
    // destroy quality.
    for entry in entries.iter_mut() {
        let is_sphere = entry.id.starts_with("tank_sphere_")
            || entry.id.starts_with("tank_am_sphere_");
        let max_dim_px = entry.width.max(entry.height);
        let scale: f64 = if is_sphere {
            0.25
        } else if max_dim_px > 2160 {
            0.4
        } else {
            1.0
        };
        if scale < 1.0 {
            let new_w = ((entry.width as f64 * scale) as u32).max(1);
            let new_h = ((entry.height as f64 * scale) as u32).max(1);
            log::info!(
                "Pre-scaling '{}' from {}x{} to {}x{} ({:.0}%)",
                entry.id, entry.width, entry.height, new_w, new_h, scale * 100.0
            );
            entry.image = image::imageops::resize(
                &entry.image, new_w, new_h, image::imageops::FilterType::Lanczos3,
            );
            entry.width = new_w;
            entry.height = new_h;
        }
    }

    let (mut packed, mut atlas_height) = shelf_pack(&mut entries, atlas_width);

    // Safety-net halving: if the atlas still exceeds the GPU texture limit
    // after pre-scaling, halve all sprites with Lanczos3 until it fits.
    // With the tiered pre-scale above this loop should rarely fire.
    let target_height = max_dim;
    loop {
        if atlas_height <= target_height { break; }
        let mut halved_any = false;
        for entry in entries.iter_mut() {
            let new_w = (entry.width / 2).max(1);
            let new_h = (entry.height / 2).max(1);
            if new_w < 8 || new_h < 8 {
                continue;
            }
            entry.image = image::imageops::resize(
                &entry.image, new_w, new_h, image::imageops::FilterType::Lanczos3,
            );
            entry.width = new_w;
            entry.height = new_h;
            halved_any = true;
        }
        if !halved_any {
            log::info!(
                "Sprite atlas {}x{} > target {} but no more sprites halve-able; \
                 keeping current size",
                atlas_width, atlas_height, target_height
            );
            break;
        }
        log::info!(
            "Sprite atlas {}x{} > target {}; halved sprites and repacking",
            atlas_width, atlas_height, target_height
        );
        let result = shelf_pack(&mut entries, atlas_width);
        packed = result.0;
        atlas_height = result.1;
    }

    log::info!("Sprite atlas: {}x{}", atlas_width, atlas_height);

    // Create RGBA buffer and blit sprites
    let mut atlas_data = vec![0u8; (atlas_width * atlas_height * 4) as usize];

    let image_map: HashMap<String, &image::RgbaImage> = entries.iter()
        .map(|e| (e.id.clone(), &e.image))
        .collect();

    for p in &packed {
        let img = image_map[&p.id];
        for row in 0..p.height {
            let src_offset = (row * p.width * 4) as usize;
            let dst_offset = ((p.y + row) * atlas_width * 4 + p.x * 4) as usize;
            let row_bytes = (p.width * 4) as usize;
            atlas_data[dst_offset..dst_offset + row_bytes]
                .copy_from_slice(&img.as_raw()[src_offset..src_offset + row_bytes]);
        }
    }

    // Build lookup tables
    let mut parts: HashMap<String, SpriteRect> = HashMap::new();
    let mut plume_frames: HashMap<String, Vec<(usize, SpriteRect)>> = HashMap::new();

    let aw = atlas_width as f32;
    let ah = atlas_height as f32;

    for p in &packed {
        let rect = SpriteRect {
            u_min: p.x as f32 / aw,
            v_min: p.y as f32 / ah,
            u_max: (p.x + p.width) as f32 / aw,
            v_max: (p.y + p.height) as f32 / ah,
            pixel_width: p.width,
            pixel_height: p.height,
        };

        if p.kind.starts_with("plume:") {
            let parts_str: Vec<&str> = p.kind.splitn(3, ':').collect();
            if parts_str.len() == 3 {
                let propellant = parts_str[1].to_string();
                let frame: usize = parts_str[2].parse().unwrap_or(0);
                plume_frames.entry(propellant).or_default().push((frame, rect));
            }
        } else {
            parts.insert(p.id.clone(), rect);
        }
    }

    let mut plumes: HashMap<String, PlumeAnimation> = HashMap::new();
    for (propellant, mut frames) in plume_frames {
        frames.sort_by_key(|(idx, _)| *idx);
        if frames.len() >= 4 {
            let dummy = SpriteRect { u_min: 0.0, v_min: 0.0, u_max: 0.0, v_max: 0.0, pixel_width: 0, pixel_height: 0 };
            let mut anim_frames = [dummy.clone(), dummy.clone(), dummy.clone(), dummy];
            for (idx, rect) in frames.into_iter().take(4) {
                anim_frames[idx] = rect;
            }
            plumes.insert(propellant, PlumeAnimation { frames: anim_frames });
        }
    }

    // Save to cache for next launch
    let metadata = SpriteAtlasMetadata {
        atlas_width,
        atlas_height,
        parts: parts.clone(),
        plumes: plumes.clone(),
    };
    save_to_cache(&atlas_data, &metadata);

    log::info!("Sprite atlas built: {} parts, {} plume types", parts.len(), plumes.len());

    (atlas_data, atlas_width, atlas_height, parts, plumes)
}

/// Upload pre-built atlas data to GPU and create SpriteAtlas
fn upload_atlas_to_gpu(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    atlas_data: &[u8],
    atlas_width: u32,
    atlas_height: u32,
    parts: HashMap<String, SpriteRect>,
    plumes: HashMap<String, PlumeAnimation>,
) -> SpriteAtlas {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Sprite Atlas"),
        size: wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        atlas_data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4 * atlas_width),
            rows_per_image: Some(atlas_height),
        },
        wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Sprite Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let bind_group_layout = create_sprite_bind_group_layout(device);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Sprite Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    SpriteAtlas { bind_group, parts, plumes }
}

/// Create the bind group layout for sprite atlas (group 2)
pub fn create_sprite_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
        label: Some("sprite_bind_group_layout"),
    })
}

/// Collect PNG paths from a directory, applying a stem filter
fn collect_sprite_paths(
    dir: &Path,
    filter: impl Fn(&str) -> bool,
) -> Vec<(String, PathBuf)> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        log::warn!("Cannot read sprite dir: {}", dir.display());
        return Vec::new();
    };

    read_dir.flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("png") {
                return None;
            }
            let stem = path.file_stem().and_then(|s| s.to_str())?.to_string();
            if !filter(&stem) {
                return None;
            }
            Some((stem, path))
        })
        .collect()
}

/// Max dimension for any individual sprite during decode.
/// Sprites larger than this are immediately downscaled to limit memory and decode time.
/// The final atlas is 16384 wide; individual sprites rarely need more than 4096px.
const MAX_SPRITE_DIM: u32 = 4096;

/// Downscale an image if its largest dimension exceeds MAX_SPRITE_DIM
fn cap_sprite_size(id: &str, rgba: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = rgba.dimensions();
    if w <= MAX_SPRITE_DIM && h <= MAX_SPRITE_DIM {
        return rgba;
    }
    let scale = MAX_SPRITE_DIM as f64 / w.max(h) as f64;
    let new_w = ((w as f64 * scale) as u32).max(1);
    let new_h = ((h as f64 * scale) as u32).max(1);
    log::info!(
        "Downscaling sprite '{}' from {}x{} to {}x{} (cap {})",
        id, w, h, new_w, new_h, MAX_SPRITE_DIM
    );
    image::imageops::resize(&rgba, new_w, new_h, image::imageops::FilterType::Lanczos3)
}

/// Decode sprite PNGs in parallel and return SpriteEntries
fn decode_sprites_parallel(
    paths: Vec<(String, PathBuf)>,
    kind: &str,
) -> Vec<SpriteEntry> {
    let kind = kind.to_string();
    let decode_one = move |(stem, path): (String, PathBuf)| -> Option<SpriteEntry> {
        let mut reader = match image::ImageReader::open(&path) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Failed to load sprite {}: {}", path.display(), e);
                return None;
            }
        };
        reader.no_limits();
        match reader.decode() {
            Ok(img) => {
                let rgba = cap_sprite_size(&stem, img.into_rgba8());
                let (w, h) = rgba.dimensions();
                Some(SpriteEntry {
                    id: stem,
                    image: rgba,
                    width: w,
                    height: h,
                    kind: kind.clone(),
                })
            }
            Err(e) => {
                log::warn!("Failed to decode sprite {}: {}", path.display(), e);
                None
            }
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    return paths.into_par_iter().filter_map(decode_one).collect();
    #[cfg(target_arch = "wasm32")]
    return paths.into_iter().filter_map(decode_one).collect();
}

/// Collect plume sprite paths and their parsed metadata
fn collect_plume_paths(dir: &Path) -> Vec<(String, PathBuf, String)> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        log::warn!("Cannot read plume dir: {}", dir.display());
        return Vec::new();
    };

    read_dir.flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("png") {
                return None;
            }
            let stem = path.file_stem().and_then(|s| s.to_str())?.to_string();

            // Parse "kerolox_frame2" -> propellant="kerolox", frame=2
            let idx = stem.rfind("_frame")?;
            let propellant = &stem[..idx];
            let frame_str = &stem[idx + 6..];
            let frame: usize = frame_str.parse().ok()?;
            let kind = format!("plume:{}:{}", propellant, frame);
            Some((stem, path, kind))
        })
        .collect()
}

/// Decode plume sprites in parallel
fn decode_plume_sprites_parallel(
    paths: Vec<(String, PathBuf, String)>,
) -> Vec<SpriteEntry> {
    let decode_one = |(stem, path, kind): (String, PathBuf, String)| -> Option<SpriteEntry> {
        let mut reader = match image::ImageReader::open(&path) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Failed to load plume sprite {}: {}", path.display(), e);
                return None;
            }
        };
        reader.no_limits();
        match reader.decode() {
            Ok(img) => {
                let rgba = cap_sprite_size(&stem, img.into_rgba8());
                let (w, h) = rgba.dimensions();
                Some(SpriteEntry {
                    id: stem,
                    image: rgba,
                    width: w,
                    height: h,
                    kind,
                })
            }
            Err(e) => {
                log::warn!("Failed to decode plume sprite {}: {}", path.display(), e);
                None
            }
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    return paths.into_par_iter().filter_map(decode_one).collect();
    #[cfg(target_arch = "wasm32")]
    return paths.into_iter().filter_map(decode_one).collect();
}

/// Create a dummy 1x1 atlas when no sprites are found
fn create_dummy_atlas(device: &wgpu::Device, queue: &wgpu::Queue) -> SpriteAtlas {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Dummy Sprite Atlas"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0u8; 4],
        wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Dummy Sprite Sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let layout = create_sprite_bind_group_layout(device);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Dummy Sprite Bind Group"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
        ],
    });

    SpriteAtlas {
        bind_group,
        parts: HashMap::new(),
        plumes: HashMap::new(),
    }
}
