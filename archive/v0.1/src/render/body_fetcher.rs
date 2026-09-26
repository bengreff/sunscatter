//! Lazy body-texture fetcher for the wasm build.
//!
//! Body textures total ~186 MB, far too large to ship inside the wasm. Instead
//! we pre-allocate a fixed-capacity texture array on startup, and per-frame the
//! app can call [`BodyTextureFetcher::request_fetch`] when a body's on-screen
//! radius gets large enough that flat-disc shading looks bad. Each fetch
//! `gloo_net::http::Request::get`s the PNG, decodes it on the wasm thread,
//! and queues the decoded image for GPU upload. [`BodyTextureFetcher::drain_uploads`]
//! is called once per frame to write completed images into a free layer of the
//! texture array and update [`BodyTextureMap`] so the next draw uses them.
//!
//! Out of layers? We just stop accepting new fetches — there are typically only
//! 5–10 bodies on screen at any zoom level. Phase 7 can add LRU eviction.
//!
//! No-op on native (the desktop `load_body_textures_native` already eager-loads
//! everything).

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen_futures::spawn_local;

use super::textures::{BodyTextureMap, TEXTURE_SIZE};

#[derive(Clone, Copy, PartialEq, Eq)]
enum FetchState {
    Loading,
    Loaded(u32), // layer index
    Failed,
}

/// Image decoded off-fetch, waiting for a free moment to upload to GPU.
struct PendingUpload {
    texture_name: String,
    image: image::RgbaImage,
}

pub struct BodyTextureFetcher {
    /// Capacity of the texture array (number of layers).
    capacity: u32,
    /// Next free layer index.
    next_layer: u32,
    /// Per-name fetch state, keyed by lowercase texture stem.
    state: HashMap<String, FetchState>,
    /// Decoded images awaiting GPU upload. Shared with spawn_local closures.
    pending: Rc<RefCell<Vec<PendingUpload>>>,
}

impl BodyTextureFetcher {
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            next_layer: 0,
            state: HashMap::new(),
            pending: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Kick off fetches for every name in `body_names`, lowercased + underscored
    /// (e.g. "Earth" → "earth"). Used at startup to eagerly load the real
    /// solar-system bodies so they appear textured immediately in every game
    /// mode, not just flight mode above the zoom threshold.
    pub fn prefetch(&mut self, body_names: &[String]) {
        for name in body_names {
            if name.is_empty() { continue; }
            let key = name.to_lowercase().replace(' ', "_");
            self.request_fetch(&key);
        }
    }

    /// Kick off a fetch for `texture_name` if we haven't already attempted it
    /// and we still have a free layer. `texture_name` must already be the
    /// canonical lowercase, underscored stem (e.g. "earth", "milky_way").
    pub fn request_fetch(&mut self, texture_name: &str) {
        if self.state.contains_key(texture_name) {
            return;
        }
        if self.next_layer >= self.capacity {
            // Out of layers; mark as failed so we don't keep checking.
            self.state.insert(texture_name.to_string(), FetchState::Failed);
            log::warn!(
                "Body texture pool full ({} layers); skipping fetch for '{}'",
                self.capacity, texture_name
            );
            return;
        }
        self.state.insert(texture_name.to_string(), FetchState::Loading);

        let url = format!("textures/bodies/{}.png", texture_name);
        let pending = self.pending.clone();
        let name = texture_name.to_string();
        spawn_local(async move {
            match fetch_and_decode(&url).await {
                Ok(img) => {
                    log::info!("Fetched body texture: {} ({}x{})", name, img.width(), img.height());
                    pending.borrow_mut().push(PendingUpload { texture_name: name, image: img });
                }
                Err(e) => {
                    // Most catalog planets won't have textures on the server;
                    // log as info, not warn.
                    log::info!("No texture for body '{}': {}", name, e);
                    // Use a sentinel marker so drain_uploads can flip the state.
                    pending.borrow_mut().push(PendingUpload {
                        texture_name: name,
                        image: image::RgbaImage::new(0, 0),
                    });
                }
            }
        });
    }

    /// Apply any decoded images to the GPU texture and update the body texture
    /// map. Call once per frame.
    pub fn drain_uploads(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        body_texture_map: &mut BodyTextureMap,
    ) {
        let _ = device;
        let drained: Vec<PendingUpload> = self.pending.borrow_mut().drain(..).collect();
        for upload in drained {
            // 0×0 sentinel = fetch failed; mark and skip.
            if upload.image.width() == 0 {
                self.state.insert(upload.texture_name, FetchState::Failed);
                continue;
            }

            // Resize to TEXTURE_SIZE if needed.
            let img = if upload.image.width() != TEXTURE_SIZE || upload.image.height() != TEXTURE_SIZE {
                image::imageops::resize(
                    &upload.image,
                    TEXTURE_SIZE,
                    TEXTURE_SIZE,
                    image::imageops::FilterType::Triangle,
                )
            } else {
                upload.image
            };

            let layer = self.next_layer;
            self.next_layer += 1;

            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
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

            self.state.insert(upload.texture_name.clone(), FetchState::Loaded(layer));
            body_texture_map.set_name_layer(&upload.texture_name, layer);
            log::info!("Uploaded body texture '{}' to layer {}", upload.texture_name, layer);

            if upload.texture_name == "milky_way" {
                body_texture_map.set_galaxy_layer(layer);
            }
        }
    }
}

async fn fetch_and_decode(url: &str) -> Result<image::RgbaImage, String> {
    let resp = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|e| format!("network: {}", e))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.binary().await.map_err(|e| format!("body: {}", e))?;
    let img = image::load_from_memory(&bytes).map_err(|e| format!("decode: {}", e))?;
    Ok(img.into_rgba8())
}
