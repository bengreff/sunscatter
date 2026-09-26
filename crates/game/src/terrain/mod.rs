//! Terrain rendering (A4): a chunked cube-sphere quadtree per body, generic
//! for any body with visual data. Chunks are built on background tasks under
//! a budget; a node splits only once all four children are ready, so the
//! surface never has holes. Chunk transforms are camera-relative, computed
//! from f64 every frame.

mod lod;
mod material;
pub mod mesh;

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
}

pub struct TerrainBody {
    node: NodeId,
    name: String,
    /// Heightmap resolution (m per sample at the equator).
    height_res: f64,
    shape: Shape,
    /// Heights from data; used when terrain elevation is enabled.
    heights: Option<HeightFn>,
    material: Handle<TerrainMaterial>,
    chunks: HashMap<ChunkKey, Chunk>,
    pending: HashMap<ChunkKey, Task<ChunkData>>,
    color_task: Option<Task<Option<Image>>>,
    /// Colour-map width currently loaded or loading.
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
    /// Chunks or colour maps are still being built.
    pub fn busy(&self) -> bool {
        self.bodies.iter().any(|b| !b.pending.is_empty() || b.color_task.is_some())
    }
}

/// Terrain heights for a body, from the simulation (one owner of the
/// sampling math, so the rendered surface is the physical one).
fn height_fn(sim: &SimState, node: NodeId) -> Option<(HeightFn, f64)> {
    let p = sim.world.source(node)?.physical.as_ref()?;
    let map = p.terrain.clone()?;
    let res = std::f64::consts::TAU * p.radius_eq / map.width() as f64;
    Some((Arc::new(move |dir| map.height_at(dir)), res))
}

fn v4(a: [f32; 3], w: f32) -> Vec4 {
    Vec4::new(a[0], a[1], a[2], w)
}

pub fn setup(
    mut commands: Commands,
    sim: Res<SimState>,
    defs: Res<BodyDefs>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let mut terrain = Terrain::default();
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
                detail.map_or(1.0, |d| d.scale_m * 400.0),
                detail.map_or(0.0, |d| d.strength),
            ),
            rock: detail.map_or(Vec4::ZERO, |d| v4(d.rock_color, d.snow_line_m.unwrap_or(-1.0))),
            snow: detail.map_or(v4([1.0; 3], def.roughness), |d| v4(d.snow_color, def.roughness)),
            ocean: def.ocean.as_ref().map_or(Vec4::ONE, |o| v4(o.tint, o.roughness)),
            base: v4(def.base_color, 1.0),
            ..default()
        };
        let material = materials.add(ExtendedMaterial {
            base: StandardMaterial { perceptual_roughness: def.roughness, reflectance: 0.3, ..default() },
            extension: TerrainExt { params, color_map: None },
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

/// Starts colour-map loads when the texture size setting changes, and
/// installs finished ones.
pub fn update_textures(
    settings: Res<GraphicsSettings>,
    mut terrain: ResMut<Terrain>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    for body in &mut terrain.bodies {
        let Some(file) = body.def.color_map.clone() else { continue };
        if body.color_width != settings.texture_size && body.color_task.is_none() {
            body.color_width = settings.texture_size;
            let path = body_visual::body_dir(&body.name).join(file);
            let width = settings.texture_size;
            body.color_task =
                Some(AsyncComputeTaskPool::get().spawn(async move { material::load_color_map(path, width) }));
        }
        if let Some(task) = body.color_task.as_mut() {
            if let Some(result) = check_ready(task) {
                body.color_task = None;
                if let (Some(img), Some(mut mat)) = (result, materials.get_mut(&body.material)) {
                    mat.extension.color_map = Some(images.add(img));
                    mat.extension.params.flags.z = 1;
                }
            }
        }
    }
}
