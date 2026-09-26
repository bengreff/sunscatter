//! Quadtree LOD selection, chunk building and placement.

use super::mesh::{self, ChunkData, ChunkKey, GRID};
use super::{body_matrix, Chunk, Terrain, TerrainBody, TerrainChunk, TerrainMaterial};
use super::{EVICT_AFTER, MAX_IN_FLIGHT, MAX_LEVEL, SPAWNS_PER_FRAME};
use crate::camera::{CameraRig, MainCamera};
use crate::map;
use crate::settings::GraphicsSettings;
use crate::state::SimState;
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool};
use glam::{DMat3, DQuat, DVec3};

/// Geometric error per metre of vertex spacing: always (curvature and
/// close-up smoothness), plus more while the spacing is coarser than the
/// heightmap (unresolved relief).
const ERROR_BASE: f64 = 0.01;
const ERROR_RELIEF: f64 = 0.1;

fn to_mesh(d: ChunkData) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, d.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, d.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, d.detail)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, d.colors)
        .with_inserted_indices(Indices::U32(d.indices))
}

/// Vertex spacing (m) of a chunk at `level`.
fn spacing(body: &TerrainBody, level: u8) -> f64 {
    body.shape.radius_eq * std::f64::consts::FRAC_PI_2 / f64::from(1u32 << level) / (GRID - 1) as f64
}

type ChunkParts = (&'static mut Transform, &'static mut Visibility);
type ChunkFilter = (With<TerrainChunk>, Without<MainCamera>);

struct Ctx {
    cam_fixed: DVec3,
    focal: f64,
    threshold: f64,
    horizon: f64,
    frame: u32,
}

impl TerrainBody {
    fn request(&mut self, key: ChunkKey, budget: &mut usize) {
        if self.pending.contains_key(&key) || self.chunks.contains_key(&key) {
            return;
        }
        if *budget == 0 || self.pending.len() >= MAX_IN_FLIGHT {
            return;
        }
        *budget -= 1;
        let shape = self.shape.clone();
        self.pending.insert(key, AsyncComputeTaskPool::get().spawn(async move { mesh::build(key, &shape) }));
    }

    fn wants_split(&self, key: ChunkKey, c: &Chunk, ctx: &Ctx) -> bool {
        if key.level >= MAX_LEVEL {
            return false;
        }
        let d = (ctx.cam_fixed - c.center).length();
        if d - c.radius > ctx.horizon {
            return false;
        }
        let s = spacing(self, key.level);
        let relief = if self.displaced && s > self.height_res { ERROR_RELIEF } else { 0.0 };
        let err = s * (ERROR_BASE + relief) + c.max_height.min(s) * relief;
        err / (d - c.radius).max(1.0) * ctx.focal > ctx.threshold
    }

    /// Selects the chunks to draw this frame.
    fn select(&mut self, ctx: &Ctx, budget: &mut usize) -> Vec<ChunkKey> {
        let mut out = Vec::new();
        let mut stack: Vec<ChunkKey> = ChunkKey::roots().to_vec();
        while let Some(key) = stack.pop() {
            let Some(c) = self.chunks.get_mut(&key) else { continue };
            c.last_used = ctx.frame;
            let c = &self.chunks[&key];
            if self.wants_split(key, c, ctx) {
                let children = key.children();
                if children.iter().all(|k| self.chunks.contains_key(k)) {
                    stack.extend(children);
                    continue;
                }
                for k in children {
                    self.request(k, budget);
                }
            }
            out.push(key);
        }
        out
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    settings: Res<GraphicsSettings>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    mut terrain: ResMut<Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
    mut chunks_q: Query<ChunkParts, ChunkFilter>,
) {
    let Some(view) = map::view(&cam) else { return };
    terrain.frame += 1;
    let frame = terrain.frame;
    let snap = sim.world.snapshot(sim.clock);
    let mut budget = SPAWNS_PER_FRAME;
    let mut drawn = 0;
    for body in &mut terrain.bodies {
        if body.displaced != settings.terrain || body.chunks.is_empty() && body.pending.is_empty() {
            for (_, c) in body.chunks.drain() {
                commands.entity(c.entity).despawn();
            }
            body.pending.clear();
            body.displaced = settings.terrain;
            body.shape.height = if settings.terrain { body.heights.clone() } else { None };
            for key in ChunkKey::roots() {
                let shape = body.shape.clone();
                body.pending.insert(key, AsyncComputeTaskPool::get().spawn(async move { mesh::build(key, &shape) }));
            }
        }
        // Install finished chunks.
        let done: Vec<(ChunkKey, ChunkData)> =
            body.pending.iter_mut().filter_map(|(k, t)| check_ready(t).map(|d| (*k, d))).collect();
        for (k, d) in done {
            body.pending.remove(&k);
            let (center, radius, max_height) = (d.center, d.radius, d.max_height);
            let entity = commands
                .spawn((
                    TerrainChunk,
                    // Planet-sized casters would stretch the shadow cascades
                    // over thousands of km; the terrain only receives.
                    NotShadowCaster,
                    Mesh3d(meshes.add(to_mesh(d))),
                    MeshMaterial3d(body.material.clone()),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                ))
                .id();
            body.chunks.insert(k, Chunk { entity, center, radius, max_height, last_used: frame });
        }

        let m: DMat3 = body_matrix(&sim, body.node);
        let centre = snap.relative_r(body.node, rig.anchor) - rig.cam_pos;
        let cam_fixed = m.transpose() * -centre;
        let h = cam_fixed.length();
        let r_min = body.shape.radius_polar - 11_000.0;
        let r_max = body.shape.radius_eq + 9_000.0;
        let horizon = (h * h - r_min * r_min).max(0.0).sqrt() + (r_max * r_max - r_min * r_min).sqrt();
        let ctx =
            Ctx { cam_fixed, focal: view.focal(), threshold: f64::from(settings.terrain_error_px), horizon, frame };
        let selected = body.select(&ctx, &mut budget);
        drawn += selected.len();

        let rot = DQuat::from_mat3(&m).as_quat();
        for key in &selected {
            let c = &body.chunks[key];
            if let Ok((mut t, mut vis)) = chunks_q.get_mut(c.entity) {
                *t = Transform { translation: (m * c.center + centre).as_vec3(), rotation: rot, scale: Vec3::ONE };
                *vis = Visibility::Visible;
            }
        }
        let evict: Vec<ChunkKey> = body
            .chunks
            .iter()
            .filter(|(k, c)| k.level > 0 && c.last_used + EVICT_AFTER < frame)
            .map(|(k, _)| *k)
            .collect();
        for (k, c) in &body.chunks {
            if c.last_used != frame || !selected.contains(k) {
                if let Ok((_, mut vis)) = chunks_q.get_mut(c.entity) {
                    *vis = Visibility::Hidden;
                }
            }
        }
        for k in evict {
            if let Some(c) = body.chunks.remove(&k) {
                commands.entity(c.entity).despawn();
            }
        }
        if let Some(mut mat) = materials.get_mut(&body.material) {
            let p = &mut mat.extension.params;
            p.fixed_x = m.x_axis.as_vec3().extend(0.0);
            p.fixed_y = m.y_axis.as_vec3().extend(0.0);
            p.fixed_z = m.z_axis.as_vec3().extend(0.0);
            p.center = centre.as_vec3().extend(0.0);
            p.flags.x = u32::from(settings.detail);
            p.flags.y = u32::from(settings.ocean_glint);
        }
    }
    terrain.drawn = drawn;
}
