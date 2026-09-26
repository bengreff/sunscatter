//! Body texture loading — loads pre-baked polar projection PNGs from data/textures/bodies/,
//! builds a texture_2d_array + sampler, and provides a mapping from body index → layer index.

// On wasm we ship without bundled body textures (Phase 6 lazy-fetches them); the
// disc-bleed / average-color helpers and `load_body_textures_native` are dead
// code there. Silence the warnings to keep the wasm build output clean.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports))]

use std::collections::HashMap;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

pub const TEXTURE_SIZE: u32 = 1024;

/// On wasm we pre-allocate this many layers in the body texture array so the
/// fetcher (see `body_fetcher.rs`) can claim slots without rebuilding the
/// bind group on every fetch. 32 × 1024² × RGBA = 128 MB of VRAM in the
/// worst case — fine on desktop browsers and modern phones.
#[cfg(target_arch = "wasm32")]
pub const WASM_BODY_TEXTURE_CAPACITY: u32 = 32;

/// Bleed disc-edge colors outward so GPU bilinear filtering at the body edge blends with
/// valid colors instead of the black pixels outside the polar projection disc.
fn bleed_disc_edges(img: &mut image::RgbaImage) {
    let (w, h) = img.dimensions();
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let disc_r = w as f64 / 2.0;
    let margin = 4.0; // covers AA zone + bilinear sampling radius

    for y in 0..h {
        for x in 0..w {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist >= disc_r - margin && dist > 0.0 {
                // Sample from a clean interior point at the same angle
                let scale = (disc_r - margin - 1.0) / dist;
                let src_x = (cx + dx * scale).round() as u32;
                let src_y = (cy + dy * scale).round() as u32;
                if src_x < w && src_y < h {
                    let color = *img.get_pixel(src_x, src_y);
                    img.put_pixel(x, y, color);
                }
            }
        }
    }
}

/// Compute average disc color for a polar-projection texture image.
/// Samples every 4th pixel within the disc (radius = size/2 - 4px),
/// then boosts brightness so minimum max-channel >= 0.45 for visibility.
fn compute_average_disc_color(img: &image::RgbaImage) -> [f32; 4] {
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

    if count == 0 {
        return [0.5, 0.5, 0.5, 1.0];
    }

    let mut r = (r_sum / count as f64 / 255.0) as f32;
    let mut g = (g_sum / count as f64 / 255.0) as f32;
    let mut b = (b_sum / count as f64 / 255.0) as f32;

    // Boost brightness so the max channel is at least 0.45
    let max_ch = r.max(g).max(b);
    if max_ch > 0.0 && max_ch < 0.45 {
        let scale = 0.45 / max_ch;
        r *= scale;
        g *= scale;
        b *= scale;
    }

    [r, g, b, 1.0]
}

/// Maps body index to texture array layer index. None means no texture for that body.
pub struct BodyTextureMap {
    layers: HashMap<usize, u32>,
    /// Lowercase texture name → layer index (for all loaded textures)
    name_layers: HashMap<String, u32>,
    /// Lowercase texture name → average disc color
    name_colors: HashMap<String, [f32; 4]>,
    pub galaxy_layer: Option<u32>,
}

impl BodyTextureMap {
    pub fn layer_for_body(&self, body_index: usize) -> Option<u32> {
        self.layers.get(&body_index).copied()
    }

    /// Look up average disc color by texture name (lowercase, underscored).
    pub fn color_for_name(&self, name: &str) -> Option<[f32; 4]> {
        self.name_colors.get(name).copied()
    }

    /// Register a synthetic body index to use an existing named texture.
    /// Looks up `name` (lowercased, spaces → underscores) in the name_layers map
    /// and inserts body_index → layer into the layers map.
    pub fn register_body_index(&mut self, body_index: usize, name: &str) {
        let key = name.to_lowercase().replace(' ', "_");
        if let Some(&layer) = self.name_layers.get(&key) {
            self.layers.insert(body_index, layer);
        }
    }

    /// Remove all dynamic body_index → layer entries at or above `from_index`.
    /// Call this before re-injecting catalog planets to avoid stale mappings.
    pub fn clear_dynamic_entries(&mut self, from_index: usize) {
        self.layers.retain(|&idx, _| idx < from_index);
    }

    /// Insert (or update) a name → layer mapping. Used by the wasm body
    /// fetcher when a lazily-loaded texture is uploaded into a new layer.
    pub fn set_name_layer(&mut self, name: &str, layer: u32) {
        self.name_layers.insert(name.to_string(), layer);
    }

    /// Insert (or update) a name → average disc color mapping. Populated at
    /// startup on wasm from the build-time-baked body_colors.ron.
    pub fn set_name_color(&mut self, name: &str, color: [f32; 4]) {
        self.name_colors.insert(name.to_string(), color);
    }

    pub fn set_galaxy_layer(&mut self, layer: u32) {
        self.galaxy_layer = Some(layer);
    }
}

/// Load body textures from `data/textures/bodies/<name>.{png,jpg}` and create a texture array.
/// Loads ALL textures in the directory — real bodies matched by index, extras by name only.
/// Textures should be pre-baked 1024x1024 north-pole polar projections.
/// Returns the texture view, sampler, and body→layer mapping.
/// If no textures are found, creates a 1-layer dummy texture so the bind group is always valid.
pub fn load_body_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    body_names: &[String],
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler, BodyTextureMap) {
    // On wasm we ship without bundled body textures (~186 MB on disk). The
    // `body_fetcher` module then lazily fills layers as the user zooms in.
    #[cfg(target_arch = "wasm32")]
    {
        let _ = body_names;
        create_empty_body_textures(device, queue)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        load_body_textures_native(device, queue, body_names)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_body_textures_native(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    body_names: &[String],
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler, BodyTextureMap) {
    let texture_dir = Path::new("data/textures/bodies");

    // Build a set of real body name stems so we can identify extras
    let mut real_body_stems: std::collections::HashSet<String> = std::collections::HashSet::new();
    // Collect paths for real body textures first (body_index → path)
    let mut body_paths: Vec<(usize, std::path::PathBuf, String)> = Vec::new(); // (body_idx, path, stem)
    for (i, name) in body_names.iter().enumerate() {
        let lower = name.to_lowercase().replace(' ', "_");
        let path = ["png", "jpg", "jpeg"].iter()
            .map(|ext| texture_dir.join(format!("{}.{}", lower, ext)))
            .find(|p| p.exists());
        if let Some(path) = path {
            real_body_stems.insert(lower.clone());
            body_paths.push((i, path, lower));
        }
    }

    // Scan directory for ALL extra textures not matched to real bodies
    let mut extra_paths: Vec<(std::path::PathBuf, String)> = Vec::new(); // (path, stem)
    if let Ok(entries) = std::fs::read_dir(texture_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() { continue; }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "png" | "jpg" | "jpeg") { continue; }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if stem.is_empty() { continue; }
            if real_body_stems.contains(&stem) { continue; }
            extra_paths.push((path, stem));
        }
    }
    // Sort extras alphabetically for deterministic layer ordering
    extra_paths.sort_by(|a, b| a.1.cmp(&b.1));

    // Assign sentinel body indices to extras (usize::MAX - 1 downward won't collide)
    // We use usize::MAX - 1 - i so they sort after real bodies but before galaxy (usize::MAX)
    let extra_start = body_paths.len();
    for (path, stem) in extra_paths {
        body_paths.push((usize::MAX / 2 + body_paths.len(), path, stem));
    }

    // Add galaxy texture path
    let galaxy_path = Path::new("data/textures/milky_way.png");
    if galaxy_path.exists() {
        body_paths.push((usize::MAX, galaxy_path.to_path_buf(), String::new()));
    }

    // Cap total textures to the device's max texture array layer count
    let max_layers = device.limits().max_texture_array_layers as usize;
    if body_paths.len() > max_layers {
        log::warn!("Texture count {} exceeds device max_texture_array_layers {}, dropping extras",
            body_paths.len(), max_layers);
        body_paths.truncate(max_layers);
    }

    log::info!("Loading {} body textures ({} real, {} extra)",
        body_paths.len() - if galaxy_path.exists() { 1 } else { 0 },
        extra_start.min(body_paths.len()),
        body_paths.len().saturating_sub(extra_start).saturating_sub(if galaxy_path.exists() { 1 } else { 0 }));

    // Decode, resize, bleed, and compute average color — parallel on native, serial on wasm.
    let decode_one = |(body_idx, path, stem): (usize, std::path::PathBuf, String)| -> Option<(usize, image::RgbaImage, String, [f32; 4])> {
        match image::open(&path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let mut final_img = if w != TEXTURE_SIZE || h != TEXTURE_SIZE {
                    image::imageops::resize(&rgba, TEXTURE_SIZE, TEXTURE_SIZE, image::imageops::FilterType::Triangle)
                } else {
                    rgba
                };
                let avg_color = if body_idx != usize::MAX {
                    bleed_disc_edges(&mut final_img);
                    compute_average_disc_color(&final_img)
                } else {
                    [0.0; 4]
                };
                log::info!("Loaded texture: {}", path.display());
                Some((body_idx, final_img, stem, avg_color))
            }
            Err(e) => {
                log::warn!("Failed to load texture {}: {}", path.display(), e);
                None
            }
        }
    };
    #[cfg(not(target_arch = "wasm32"))]
    let decoded: Vec<(usize, image::RgbaImage, String, [f32; 4])> =
        body_paths.into_par_iter().filter_map(decode_one).collect();
    #[cfg(target_arch = "wasm32")]
    let decoded: Vec<(usize, image::RgbaImage, String, [f32; 4])> =
        body_paths.into_iter().filter_map(decode_one).collect();

    // Sort to get deterministic layer ordering (real bodies first by index, extras, galaxy last)
    let mut images = decoded;
    images.sort_by_key(|(idx, _, _, _)| *idx);

    // Find galaxy layer index
    let mut galaxy_layer_idx: Option<u32> = None;
    for (layer_idx, (body_idx, _, _, _)) in images.iter().enumerate() {
        if *body_idx == usize::MAX {
            galaxy_layer_idx = Some(layer_idx as u32);
        }
    }
    if galaxy_layer_idx.is_some() {
        log::info!("Galaxy texture at layer {}", galaxy_layer_idx.unwrap());
    }

    let layer_count = images.len().max(1) as u32;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Body Texture Array"),
        size: wgpu::Extent3d {
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
            depth_or_array_layers: layer_count,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let mut layers = HashMap::new();
    let mut name_layers = HashMap::new();
    let mut name_colors = HashMap::new();

    for (layer_idx, (body_idx, img, stem, avg_color)) in images.iter().enumerate() {
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer_idx as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            img.as_raw(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * TEXTURE_SIZE),
                rows_per_image: Some(TEXTURE_SIZE),
            },
            wgpu::Extent3d {
                width: TEXTURE_SIZE,
                height: TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );
        // Map real body indices to layers (extras get name-only mapping)
        if *body_idx < usize::MAX / 2 {
            layers.insert(*body_idx, layer_idx as u32);
        }
        // Map all non-galaxy textures by name
        if !stem.is_empty() {
            name_layers.insert(stem.clone(), layer_idx as u32);
            name_colors.insert(stem.clone(), *avg_color);
        }
    }

    log::info!("Texture array: {} layers, {} name mappings", layer_count, name_layers.len());

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Body Texture Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (texture, view, sampler, BodyTextureMap { layers, name_layers, name_colors, galaxy_layer: galaxy_layer_idx })
}

/// Pre-allocate the body texture array used by the wasm lazy-fetch path. Also
/// callable on native as a fallback (e.g. when `data/textures/bodies/` is
/// missing) — desktop builds don't normally invoke it.
pub fn create_empty_body_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler, BodyTextureMap) {
    #[cfg(target_arch = "wasm32")]
    const LAYERS: u32 = WASM_BODY_TEXTURE_CAPACITY;
    // Native fallback (only reached if a desktop build invokes this function
    // explicitly): keep 2 layers — enough to satisfy the GLES heuristic.
    #[cfg(not(target_arch = "wasm32"))]
    const LAYERS: u32 = 2;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Body Texture Array (lazy)"),
        size: wgpu::Extent3d {
            width: TEXTURE_SIZE,
            height: TEXTURE_SIZE,
            depth_or_array_layers: LAYERS,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Initialize all layers with transparent black so the bind group is valid
    // even before any fetched textures land.
    let blank = vec![0u8; (TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize];
    for layer in 0..LAYERS {
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                aspect: wgpu::TextureAspect::All,
            },
            &blank,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * TEXTURE_SIZE),
                rows_per_image: Some(TEXTURE_SIZE),
            },
            wgpu::Extent3d {
                width: TEXTURE_SIZE,
                height: TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );
    }

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Body Texture Sampler (lazy)"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let map = BodyTextureMap {
        layers: HashMap::new(),
        name_layers: HashMap::new(),
        name_colors: HashMap::new(),
        galaxy_layer: None,
    };

    // Seed the name → average-color table from the build-time bake so that
    // bodies render in their actual disc color before any texture has loaded.
    #[cfg(target_arch = "wasm32")]
    let map = {
        let mut map = map;
        const BODY_COLORS_RON: &str = include_str!(concat!(env!("OUT_DIR"), "/body_colors.ron"));
        match ron::from_str::<HashMap<String, [f32; 4]>>(BODY_COLORS_RON) {
            Ok(colors) => {
                for (name, color) in colors {
                    map.name_colors.insert(name, color);
                }
                log::info!("Seeded {} body disc colors from build-time bake", map.name_colors.len());
            }
            Err(e) => log::warn!("Failed to parse embedded body_colors.ron: {}", e),
        }
        map
    };

    (texture, view, sampler, map)
}
