//! Terrain rendering (A4): a chunked cube-sphere quadtree per body, generic
//! for any body with visual data. Chunks are built on background tasks under
//! a budget; a node splits only once all four children are ready, so the
//! surface never has holes. Chunk transforms are camera-relative, computed
//! from f64 every frame.

mod lod;
mod material;
pub mod mesh;
pub mod noise;
pub mod water;
pub mod zones;

pub use lod::update;

use crate::body_visual::{self, BodyVisualDef};
use crate::look::{Parts, COLOR_MAP_WIDTH};
use crate::scene::BodyDefs;
use crate::state::SimState;
use bevy::asset::embedded_asset;
use bevy::pbr::ExtendedMaterial;
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};
use glam::{DMat3, DVec3};
use material::TerrainExt;
pub use material::{TerrainFrame, TerrainMaterial, TerrainParams};
use mesh::{ChunkData, ChunkKey, HeightFn, Shape};

use sim::frame::NodeId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Marks a terrain chunk entity, with its triangle count (the profiler's).
#[derive(Component)]
pub struct TerrainChunk {
    pub triangles: u32,
}

/// Chunks built on the background threads, and the time they took (ns),
/// since the last [`take_build_stats`] (the profiler's).
static BUILT: AtomicU64 = AtomicU64::new(0);
static BUILD_NS: AtomicU64 = AtomicU64::new(0);

/// Builds a chunk, counting it for the profiler.
fn build_counted(key: ChunkKey, shape: &Shape) -> ChunkData {
    let start = std::time::Instant::now();
    let d = mesh::build(key, shape);
    BUILT.fetch_add(1, Ordering::Relaxed);
    BUILD_NS.fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
    d
}

/// (chunks built, ns spent) since the last call.
pub fn take_build_stats() -> (u64, u64) {
    (BUILT.swap(0, Ordering::Relaxed), BUILD_NS.swap(0, Ordering::Relaxed))
}

/// Deepest quadtree level (Earth: ~5 m between vertices).
const MAX_LEVEL: u8 = 19;
/// Chunk builds started per frame, and in flight at once.
const SPAWNS_PER_FRAME: usize = 16;
const MAX_IN_FLIGHT: usize = 64;
/// Tessellation sag limit (m) for bodies with an atmosphere (see `lod`).
const MAX_SAG_ATMOSPHERE: f64 = 50.0;
/// Unused chunks are dropped after this many frames.
const EVICT_AFTER: u32 = 240;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "terrain.wgsl");
        // The loading screen waits mostly for the terrain (`Terrain::loading`).
        app.add_plugins((MaterialPlugin::<TerrainMaterial>::default(), crate::scene::loading::LoadingPlugin));
    }
}

struct Chunk {
    entity: Entity,
    center: DVec3,
    radius: f64,
    max_height: f64,
    last_used: u32,
    /// Frame the chunk was installed. Its entity is spawned through
    /// `Commands`, so it can only be shown from the next frame.
    installed: u32,
}

pub struct TerrainBody {
    node: NodeId,
    name: String,
    /// Heightmap resolution (m per sample at the equator).
    height_res: f64,
    /// Largest allowed sag of a chunk's flat triangles below the ellipsoid (m).
    max_sag: f64,
    shape: Shape,
    /// Heights from data (left out only by the benchmark, `Parts::relief`).
    heights: Option<HeightFn>,
    material: Handle<TerrainMaterial>,
    /// This frame's data for the shader, filled by `lod::update`,
    /// `lighting::update_terrain` and `clouds::update`, uploaded by
    /// [`upload_frames`] into `frame_buffer` (the material's storage buffer).
    pub frame: TerrainFrame,
    frame_buffer: Handle<ShaderBuffer>,
    /// Some chunk of it is in sight this frame (`lod::chunk_in_sight`).
    in_sight: bool,
    chunks: HashMap<ChunkKey, Chunk>,
    pending: HashMap<ChunkKey, Task<ChunkData>>,
    color_task: Option<Task<Maps>>,
    /// Colour-map width currently loaded or loading.
    color_width: u32,
    /// Water-mask width currently loaded or loading (with zones on, at
    /// least the zone map's, so coasts and zone edges match).
    water_width: u32,
    /// Surface zones: what loading them needs (taken when they start
    /// loading), the load, and the zone map's width (from its header).
    zone_inputs: Option<zones::ZoneInputs>,
    zone_task: Option<Task<Option<zones::ZoneTextures>>>,
    zone_width: Option<u32>,
    /// Elevation shown (`Parts::relief` when the chunks were built).
    displaced: bool,
    def: BodyVisualDef,
}

#[derive(Resource, Default)]
pub struct Terrain {
    pub bodies: Vec<TerrainBody>,
    frame: u32,
    /// Chunks drawn last frame, those in sight (for the overlay).
    pub drawn: usize,
}

impl Terrain {
    /// Each terrain body and its material.
    pub fn materials(&self) -> impl Iterator<Item = (NodeId, &Handle<TerrainMaterial>)> {
        self.bodies.iter().map(|b| (b.node, &b.material))
    }

    /// Each terrain body's frame data (lighting, fog), to fill this frame.
    pub fn frames_mut(&mut self) -> impl Iterator<Item = (NodeId, &mut TerrainFrame)> {
        self.bodies.iter_mut().map(|b| (b.node, &mut b.frame))
    }

    /// Chunks being built and surface maps (colour, water, zones) loading,
    /// over every body: what the loading screen waits for
    /// (`loading::Counts`).
    pub fn loading(&self) -> (usize, usize) {
        let chunks = self.bodies.iter().map(|b| b.pending.len()).sum();
        let maps =
            self.bodies.iter().map(|b| usize::from(b.color_task.is_some()) + usize::from(b.zone_task.is_some())).sum();
        (chunks, maps)
    }

    /// Chunks or surface maps are still being built.
    pub fn busy(&self) -> bool {
        self.bodies.iter().any(|b| !b.pending.is_empty() || b.color_task.is_some() || b.zone_task.is_some())
    }
}

/// Terrain heights for a body, from the simulation (one owner of the
/// sampling math, so the rendered surface is the physical one).
///
/// Heights include the procedural sub-sample detail (D059), so relief is
/// unresolved (and chunks keep splitting for it) down to the detail's
/// shortest wavelength, not only the heightmap's spacing.
fn height_fn(sim: &SimState, node: NodeId) -> Option<(HeightFn, f64)> {
    let p = sim.world.source(node)?.physical.clone()?;
    let map = p.terrain.as_ref()?;
    let map_res = std::f64::consts::TAU * p.radius_eq / map.width() as f64;
    let res = p.detail.as_ref().map_or(map_res, |d| {
        let k = d.params.lacunarity.powi(d.params.octaves.saturating_sub(1) as i32);
        d.params.wavelength / k
    });
    Some((Arc::new(move |dir| p.terrain_height(dir)), res))
}

fn v4(a: [f32; 3], w: f32) -> Vec4 {
    Vec4::new(a[0], a[1], a[2], w)
}

pub fn setup(
    mut commands: Commands,
    sim: Res<SimState>,
    defs: Res<BodyDefs>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut terrain = Terrain::default();
    let noise = images.add(noise::image());
    for (node, def) in &defs.defs {
        if def.emissive.is_some() {
            continue;
        }
        let Some(p) = sim.world.source(*node).and_then(|s| s.physical.as_ref()) else { continue };
        let detail = def.detail.as_ref();
        // Shallow water needs the sea floor (the heightmap) and a sea level.
        let shallow = def
            .ocean
            .as_ref()
            .filter(|o| o.shallow_depth_m.is_some() && p.terrain.is_some() && p.sea_level.is_some())
            .and_then(|o| o.shallow_albedo)
            .map_or(Vec4::ZERO, |a| v4(a, 1.0));
        // Hazy atmospheres light the ground through their haze (Titan, Venus).
        let haze =
            def.atmosphere.as_ref().filter(|a| a.raymarch_inside).map(|a| crate::lighting::haze_light(a, p.radius_eq));
        let params = TerrainParams {
            shape: Vec4::new(p.radius_eq as f32, p.radius_polar as f32, 0.0, detail.map_or(0.0, |d| d.strength)),
            rock: detail.map_or(Vec4::ZERO, |d| v4(d.rock_color, d.snow_line_m.unwrap_or(-1.0))),
            snow: detail.map_or(v4([1.0; 3], def.roughness), |d| v4(d.snow_color, def.roughness)),
            ocean: def.ocean.as_ref().map_or(Vec4::ONE, |o| v4(o.albedo, 1.0)),
            base: v4(def.base_color, 1.0),
            shallow,
            clouds: def.clouds.map_or(Vec4::ZERO, |c| Vec4::new(c.scale_m, c.warp, c.stretch.max(1.0), c.contrast)),
            haze: haze.map_or(Vec4::ZERO, |h| v4(h.tau, h.r_over_h)),
            haze_diffuse: haze.map_or(Vec4::ZERO, |h| v4(h.diffuse, 0.0)),
            ..default()
        };
        let frame = TerrainFrame { ambient: Vec4::ONE, ..default() };
        let frame_buffer = buffers.add(ShaderBuffer::from(frame));
        let material = materials.add(ExtendedMaterial {
            base: StandardMaterial { perceptual_roughness: def.roughness, reflectance: 0.3, ..default() },
            extension: TerrainExt {
                params,
                color_map: None,
                water_map: None,
                zone_map: None,
                zone_far: None,
                ring_profile: None,
                frame: frame_buffer.clone(),
                noise: Some(noise.clone()),
            },
        });
        let shape = Shape {
            radius_eq: p.radius_eq,
            radius_polar: p.radius_polar,
            height: None,
            sea_level: p.sea_level,
            detail_scale: f64::from(detail.map_or(50.0, |d| d.scale_m)),
        };
        let heights = height_fn(&sim, *node);
        let dir = body_visual::body_dir(&sim.world.eph.node(*node).name);
        let zone_width = def.zones.as_ref().and_then(|z| image::image_dimensions(dir.join(&z.map)).ok()).map(|d| d.0);
        let zone_inputs = def.zones.clone().map(|z| zones::ZoneInputs {
            dir,
            def: z,
            color_map: def.color_map.clone(),
            ocean: def.ocean.clone(),
            heights: p.terrain.clone(),
            sea_level: p.sea_level,
            base_color: def.base_color,
        });
        terrain.bodies.push(TerrainBody {
            node: *node,
            name: sim.world.eph.node(*node).name.clone(),
            height_res: heights.as_ref().map_or(f64::INFINITY, |h| h.1),
            max_sag: if def.atmosphere.is_some() { MAX_SAG_ATMOSPHERE } else { f64::INFINITY },
            shape,
            heights: heights.map(|h| h.0),
            material,
            frame,
            frame_buffer,
            in_sight: true,
            chunks: HashMap::new(),
            pending: HashMap::new(),
            color_task: None,
            color_width: 0,
            water_width: 0,
            zone_inputs,
            zone_task: None,
            zone_width,
            displaced: false,
            def: def.clone(),
        });
    }
    commands.insert_resource(terrain);
}

fn body_matrix(sim: &SimState, node: NodeId) -> DMat3 {
    let p = sim.world.source(node).and_then(|s| s.physical.as_ref()).expect("terrain body is physical");
    crate::scene::body_matrix(p, sim.clock)
}

/// Writes the frame data of each body in sight into its storage buffer
/// (the material itself is left alone: see `TerrainFrame`; a body out of
/// sight gets its data the frame it comes into sight).
pub fn upload_frames(terrain: Res<Terrain>, mut buffers: ResMut<Assets<ShaderBuffer>>) {
    for body in terrain.bodies.iter().filter(|b| b.in_sight) {
        if let Some(mut b) = buffers.get_mut(&body.frame_buffer) {
            b.set_data(body.frame);
        }
    }
}

/// A body's surface maps, loaded together (each only if its width changed).
struct Maps {
    color: Option<Image>,
    water: Option<Image>,
}

/// Starts the colour-map and water-mask loads (again when the benchmark
/// turns the zones off or on: the water mask is at least the zone map's
/// width with zones), the zones' the first time they are wanted, and
/// installs finished ones.
pub fn update_textures(
    parts: Res<Parts>,
    mut terrain: ResMut<Terrain>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    for body in &mut terrain.bodies {
        if parts.zones {
            if let Some(inputs) = body.zone_inputs.take() {
                body.zone_task = Some(AsyncComputeTaskPool::get().spawn(async move { zones::load(inputs) }));
            }
        }
        if let Some(task) = body.zone_task.as_mut() {
            if let Some(loaded) = check_ready(task) {
                body.zone_task = None;
                if let (Some(z), Some(mut mat)) = (loaded, materials.get_mut(&body.material)) {
                    let p = &mut mat.extension.params;
                    p.zone_color = z.palette.color;
                    p.zone_pattern = z.palette.pattern;
                    p.zones.y = zones::warp_wavelength(body.shape.radius_eq, z.width) as f32;
                    mat.extension.zone_map = Some(images.add(z.map));
                    mat.extension.zone_far = Some(images.add(z.far));
                }
            }
        }
        let color = body.def.color_map.clone();
        let water = body.def.ocean.as_ref().and_then(|o| o.mask.clone());
        if color.is_none() && water.is_none() {
            continue;
        }
        let color_w = COLOR_MAP_WIDTH;
        let water_w = if parts.zones { color_w.max(body.zone_width.unwrap_or(0)) } else { color_w };
        if (body.color_width, body.water_width) != (color_w, water_w) && body.color_task.is_none() {
            let color = color.filter(|_| body.color_width != color_w);
            let water = water.filter(|_| body.water_width != water_w);
            (body.color_width, body.water_width) = (color_w, water_w);
            let dir = body_visual::body_dir(&body.name);
            body.color_task = Some(AsyncComputeTaskPool::get().spawn(async move {
                Maps {
                    color: color.and_then(|f| material::load_color_map(dir.join(f), color_w)),
                    water: water.and_then(|f| water::load_water_map(&dir.join(f), water_w)),
                }
            }));
        }
        if let Some(task) = body.color_task.as_mut() {
            if let Some(maps) = check_ready(task) {
                body.color_task = None;
                let Some(mut mat) = materials.get_mut(&body.material) else { continue };
                if let Some(img) = maps.color {
                    mat.extension.color_map = Some(images.add(img));
                    mat.extension.params.flags.z = 1;
                }
                if let Some(img) = maps.water {
                    mat.extension.water_map = Some(images.add(img));
                    mat.extension.params.flags.w = 1;
                }
            }
        }
    }
}
