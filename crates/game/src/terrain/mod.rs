//! Terrain rendering (A4): a chunked cube-sphere quadtree per body, generic
//! for any body with visual data. Chunks are built on background tasks under
//! a budget; a node splits only once all four children are ready, so the
//! surface never has holes. Chunk transforms are camera-relative, computed
//! from f64 every frame.

mod ground;
mod lod;
mod material;
pub mod mesh;
pub mod water;

pub use lod::update;

use crate::body_visual::{self, BodyVisualDef};
use crate::scene::BodyDefs;
use crate::settings::GraphicsSettings;
use crate::state::SimState;
use bevy::asset::embedded_asset;
use bevy::pbr::ExtendedMaterial;
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};
use glam::{DMat3, DVec3};
pub use material::TerrainMaterial;
use material::{TerrainExt, TerrainParams};
use mesh::{ChunkData, ChunkKey, HeightFn, Shape};

use sim::frame::{NodeId, Vec3 as FVec3};
use std::collections::HashMap;
use std::sync::Arc;

/// Marks a terrain chunk entity.
#[derive(Component)]
pub struct TerrainChunk;

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
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default());
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
    /// Heights from data; used when terrain elevation is enabled.
    heights: Option<HeightFn>,
    material: Handle<TerrainMaterial>,
    chunks: HashMap<ChunkKey, Chunk>,
    pending: HashMap<ChunkKey, Task<ChunkData>>,
    color_task: Option<Task<Maps>>,
    /// Colour-map (and water-mask) width currently loaded or loading.
    color_width: u32,
    /// Elevation shown (the settings value when chunks were built).
    displaced: bool,
    def: BodyVisualDef,
}

#[derive(Resource, Default)]
pub struct Terrain {
    pub bodies: Vec<TerrainBody>,
    frame: u32,
    /// Chunks drawn last frame (for the overlay).
    pub drawn: usize,
}

impl Terrain {
    /// Each terrain body and its material (for per-body lighting).
    pub fn materials(&self) -> impl Iterator<Item = (NodeId, &Handle<TerrainMaterial>)> {
        self.bodies.iter().map(|b| (b.node, &b.material))
    }

    /// Chunks or colour maps are still being built.
    pub fn busy(&self) -> bool {
        self.bodies.iter().any(|b| !b.pending.is_empty() || b.color_task.is_some())
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
    mut images: ResMut<Assets<Image>>,
) {
    let mut terrain = Terrain::default();
    let ground = ground::load().map(|g| (images.add(g.color), images.add(g.normal), g.means));
    for (node, def) in &defs.defs {
        if def.emissive.is_some() {
            continue;
        }
        let Some(p) = sim.world.source(*node).and_then(|s| s.physical.as_ref()) else { continue };
        let detail = def.detail.as_ref();
        let params = TerrainParams {
            shape: Vec4::new(
                p.radius_eq as f32,
                p.radius_polar as f32,
                detail.map_or(1.0, |d| d.fade_m.unwrap_or(d.scale_m * 400.0)),
                detail.map_or(0.0, |d| d.strength),
            ),
            rock: detail.map_or(Vec4::ZERO, |d| v4(d.rock_color, d.snow_line_m.unwrap_or(-1.0))),
            snow: detail.map_or(v4([1.0; 3], def.roughness), |d| v4(d.snow_color, def.roughness)),
            ocean: def.ocean.as_ref().map_or(Vec4::ONE, |o| v4(o.albedo, o.roughness)),
            base: v4(def.base_color, 1.0),
            // Full sunlight until lighting.rs fills in the real values.
            light: Vec4::ONE,
            // Tiles of 5 m and 40 m for a 40 m detail scale (they divide the
            // detail uv's 256-unit wrap, so there is no seam).
            ground: Vec4::new(if ground.is_some() { 1.0 } else { 0.0 }, 0.125, 1.0, 0.0),
            ground_means: ground.as_ref().map_or([Vec4::ONE; 5], |g| g.2),
            ..default()
        };
        let material = materials.add(ExtendedMaterial {
            base: StandardMaterial { perceptual_roughness: def.roughness, reflectance: 0.3, ..default() },
            extension: TerrainExt {
                params,
                color_map: None,
                water_map: None,
                ground_color: ground.as_ref().map(|g| g.0.clone()),
                ground_normal: ground.as_ref().map(|g| g.1.clone()),
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
        terrain.bodies.push(TerrainBody {
            node: *node,
            name: sim.world.eph.node(*node).name.clone(),
            height_res: heights.as_ref().map_or(f64::INFINITY, |h| h.1),
            max_sag: if def.atmosphere.is_some() { MAX_SAG_ATMOSPHERE } else { f64::INFINITY },
            shape,
            heights: heights.map(|h| h.0),
            material,
            chunks: HashMap::new(),
            pending: HashMap::new(),
            color_task: None,
            color_width: 0,
            displaced: false,
            def: def.clone(),
        });
    }
    commands.insert_resource(terrain);
}

fn body_matrix(sim: &SimState, node: NodeId) -> DMat3 {
    let p = sim.world.source(node).and_then(|s| s.physical.as_ref()).expect("terrain body is physical");
    let col = |v: DVec3| p.rotation.to_inertial(FVec3::from_raw(v), sim.clock).raw();
    DMat3::from_cols(col(DVec3::X), col(DVec3::Y), col(DVec3::Z))
}

/// A body's surface maps, loaded together at the texture size setting.
struct Maps {
    color: Option<Image>,
    water: Option<Image>,
}

/// Starts colour-map and water-mask loads when the texture size setting
/// changes, and installs finished ones.
pub fn update_textures(
    settings: Res<GraphicsSettings>,
    mut terrain: ResMut<Terrain>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    for body in &mut terrain.bodies {
        let color = body.def.color_map.clone();
        let water = body.def.ocean.as_ref().and_then(|o| o.mask.clone());
        if color.is_none() && water.is_none() {
            continue;
        }
        if body.color_width != settings.texture_size && body.color_task.is_none() {
            body.color_width = settings.texture_size;
            let dir = body_visual::body_dir(&body.name);
            let width = settings.texture_size;
            body.color_task = Some(AsyncComputeTaskPool::get().spawn(async move {
                Maps {
                    color: color.and_then(|f| material::load_color_map(dir.join(f), width)),
                    water: water.and_then(|f| water::load_water_map(&dir.join(f), width)),
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
