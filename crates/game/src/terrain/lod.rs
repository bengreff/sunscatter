//! Quadtree LOD selection, chunk building and placement.

use super::material::{ATTRIBUTE_MORPH, MORPH_TAG_SCALE};
use super::mesh::{ChunkData, ChunkKey, GRID};
use super::{body_matrix, Chunk, Terrain, TerrainBody, TerrainChunk, TerrainMaterial};
use super::{EVICT_AFTER, MAX_IN_FLIGHT, MAX_LEVEL, SPAWNS_PER_FRAME};
use crate::camera::{CameraRig, MainCamera};
use crate::look::{Parts, TERRAIN_ERROR_PX};
use crate::map;
use crate::state::SimState;
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, MeshTag, PrimitiveTopology};
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
        .with_inserted_attribute(ATTRIBUTE_MORPH, d.morph)
        .with_inserted_indices(Indices::U32(d.indices))
}

/// Geomorphing: a chunk that has just split in (split measure 1) is drawn
/// at its parent's shape, and reaches its own at this measure (the last 20%
/// of the distance range, as distance ∝ 1 / measure).
const MORPH_FULL_AT: f64 = 1.25;

/// A child's morph factor (1 = parent shape, 0 = own shape) from its
/// parent's split measure; `None` (split for other reasons) never morphs.
pub fn morph_factor(parent_measure: Option<f64>) -> f32 {
    parent_measure.map_or(0.0, |m| ((MORPH_FULL_AT - m) / (MORPH_FULL_AT - 1.0)).clamp(0.0, 1.0) as f32)
}

/// Whether a chunk installed at frame `installed` can be drawn at `frame`.
/// Its entity is spawned through `Commands` and only exists from the next
/// frame: switching to children installed this frame hid the parent a frame
/// before they could be shown, a one-frame hole through which the
/// atmosphere drew its grey ground (the owner's "whitish shapes" when
/// zooming).
pub fn shown_by(installed: u32, frame: u32) -> bool {
    installed < frame
}

/// What the camera sees, for culling chunks on the CPU: its forward
/// direction and the cosine of the half-angle of a cone holding the whole
/// view (the half-diagonal of the field of view, with a margin).
#[derive(Clone, Copy, Debug)]
pub struct Sight {
    pub forward: DVec3,
    pub cos_half: f64,
    pub sin_half: f64,
}

impl Sight {
    /// For a vertical field of view `fov` (rad) and aspect (width over
    /// height), with a 10 % margin.
    pub fn new(forward: DVec3, fov: f64, aspect: f64) -> Self {
        let t = (fov / 2.0).tan();
        let half = (t * (1.0 + aspect * aspect).sqrt()).atan() * 1.1;
        let half = half.min(std::f64::consts::FRAC_PI_2);
        Sight { forward, cos_half: half.cos(), sin_half: half.sin() }
    }
}

/// Whether a chunk's bounding sphere (camera-relative `centre`, `radius`)
/// can show: in the view cone and not wholly beyond the horizon (`horizon`:
/// the farthest distance at which the body's surface can be seen). The
/// others are hidden and not moved: their transforms changing every frame
/// cost Bevy work for nothing (1,590 chunks selected, 149 in view, from
/// 185 km, 2026-10-26).
pub fn chunk_in_sight(centre: DVec3, radius: f64, horizon: f64, sight: &Sight) -> bool {
    let d = centre.length();
    if d <= radius {
        return true;
    }
    if d - radius > horizon {
        return false;
    }
    // The sphere meets the cone when its centre is within the cone widened
    // by the sphere's angular radius.
    let cos_c = centre.dot(sight.forward) / d;
    let sin_r = (radius / d).min(1.0);
    let cos_r = (1.0 - sin_r * sin_r).sqrt();
    // cos(half + r) = cos half cos r − sin half sin r.
    cos_c >= sight.cos_half * cos_r - sight.sin_half * sin_r
}

/// Why a chunk splits: `No`, `Always` (tessellation sag, not the camera),
/// or `Measure(m)` with m > 1 the screen error over the threshold.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Split {
    No,
    Always,
    Measure(f64),
}

/// Vertex spacing (m) of a chunk at `level`.
fn spacing(body: &TerrainBody, level: u8) -> f64 {
    body.shape.radius_eq * std::f64::consts::FRAC_PI_2 / f64::from(1u32 << level) / (GRID - 1) as f64
}

type ChunkParts = (&'static mut Transform, &'static mut Visibility, &'static mut MeshTag);
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
        self.pending.insert(key, AsyncComputeTaskPool::get().spawn(async move { super::build_counted(key, &shape) }));
    }

    fn wants_split(&self, key: ChunkKey, c: &Chunk, ctx: &Ctx) -> Split {
        if key.level >= MAX_LEVEL {
            return Split::No;
        }
        let s = spacing(self, key.level);
        // Flat triangles sag below the true surface; an atmosphere's
        // aerial perspective depends strongly on the depth it ends at, so
        // keep the sag small wherever the body is drawn, including chunks
        // judged past the horizon: a coarse chunk there sagged hundreds of km
        // and its neighbours' skirts stood above it as walls near the limb.
        if s * s / (8.0 * self.shape.radius_eq) > self.max_sag {
            return Split::Always;
        }
        let d = (ctx.cam_fixed - c.center).length();
        if d - c.radius > ctx.horizon {
            return Split::No;
        }
        let relief = if self.displaced && s > self.height_res { ERROR_RELIEF } else { 0.0 };
        let err = s * (ERROR_BASE + relief) + c.max_height.min(s) * relief;
        let m = err / (d - c.radius).max(1.0) * ctx.focal / ctx.threshold;
        if m > 1.0 {
            Split::Measure(m)
        } else {
            Split::No
        }
    }

    /// Selects the chunks to draw this frame, each with its morph factor.
    fn select(&mut self, ctx: &Ctx, budget: &mut usize) -> Vec<(ChunkKey, f32)> {
        let mut out = Vec::new();
        let mut stack: Vec<(ChunkKey, f32)> = ChunkKey::roots().iter().map(|&k| (k, 0.0)).collect();
        while let Some((key, morph)) = stack.pop() {
            let Some(c) = self.chunks.get_mut(&key) else { continue };
            c.last_used = ctx.frame;
            let c = &self.chunks[&key];
            let split = self.wants_split(key, c, ctx);
            if split != Split::No {
                let children = key.children();
                if children.iter().all(|k| self.chunks.get(k).is_some_and(|c| shown_by(c.installed, ctx.frame))) {
                    let m = morph_factor(match split {
                        Split::Measure(m) => Some(m),
                        _ => None,
                    });
                    stack.extend(children.iter().map(|&k| (k, m)));
                    continue;
                }
                for k in children {
                    self.request(k, budget);
                }
            }
            out.push((key, morph));
        }
        out
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    parts: Res<Parts>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    mut terrain: ResMut<Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Assets<TerrainMaterial>>,
    mut chunks_q: Query<ChunkParts, ChunkFilter>,
) {
    let Some(view) = map::view(&cam) else { return };
    let Ok((_, cam_t, _)) = cam.single() else { return };
    let sight = Sight::new(cam_t.forward().as_vec3().as_dvec3(), view.fov(), view.aspect());
    terrain.frame += 1;
    let frame = terrain.frame;
    let snap = sim.world.snapshot(sim.clock);
    let mut budget = SPAWNS_PER_FRAME;
    let mut drawn = 0;
    for body in &mut terrain.bodies {
        if body.displaced != parts.relief || body.chunks.is_empty() && body.pending.is_empty() {
            for (_, c) in body.chunks.drain() {
                commands.entity(c.entity).despawn();
            }
            body.pending.clear();
            body.displaced = parts.relief;
            body.shape.height = if parts.relief { body.heights.clone() } else { None };
            for key in ChunkKey::roots() {
                let shape = body.shape.clone();
                body.pending
                    .insert(key, AsyncComputeTaskPool::get().spawn(async move { super::build_counted(key, &shape) }));
            }
        }
        // Install finished chunks.
        let done: Vec<(ChunkKey, ChunkData)> =
            body.pending.iter_mut().filter_map(|(k, t)| check_ready(t).map(|d| (*k, d))).collect();
        for (k, d) in done {
            body.pending.remove(&k);
            let (center, radius, max_height) = (d.center, d.radius, d.max_height);
            let triangles = (d.indices.len() / 3) as u32;
            let entity = commands
                .spawn((
                    TerrainChunk { triangles },
                    // Planet-sized casters would stretch the shadow cascades
                    // over thousands of km; the terrain only receives.
                    NotShadowCaster,
                    Mesh3d(meshes.add(to_mesh(d))),
                    MeshMaterial3d(body.material.clone()),
                    Transform::IDENTITY,
                    Visibility::Hidden,
                    MeshTag(0),
                ))
                .id();
            body.chunks.insert(k, Chunk { entity, center, radius, max_height, last_used: frame, installed: frame });
        }

        let m: DMat3 = body_matrix(&sim, body.node);
        let centre = snap.relative_r(body.node, rig.anchor) - rig.cam_pos;
        let cam_fixed = m.transpose() * -centre;
        let h = cam_fixed.length();
        let r_min = body.shape.radius_polar - 11_000.0;
        let r_max = body.shape.radius_eq + 9_000.0;
        let horizon = (h * h - r_min * r_min).max(0.0).sqrt() + (r_max * r_max - r_min * r_min).sqrt();
        let ctx = Ctx { cam_fixed, focal: view.focal(), threshold: f64::from(TERRAIN_ERROR_PX), horizon, frame };
        let selected = body.select(&ctx, &mut budget);

        let rot = DQuat::from_mat3(&m).as_quat();
        let mut shown = 0;
        for (key, morph) in &selected {
            let c = &body.chunks[key];
            let at = m * c.center + centre;
            if !chunk_in_sight(at, c.radius, horizon, &sight) {
                continue;
            }
            shown += 1;
            if let Ok((mut t, mut vis, mut tag)) = chunks_q.get_mut(c.entity) {
                *t = Transform { translation: at.as_vec3(), rotation: rot, scale: Vec3::ONE };
                vis.set_if_neq(Visibility::Visible);
                let q = (morph * MORPH_TAG_SCALE) as u32;
                if tag.0 != q {
                    tag.0 = q;
                }
            }
        }
        let selected: std::collections::HashSet<ChunkKey> = selected.into_iter().map(|(k, _)| k).collect();
        let evict: Vec<ChunkKey> = body
            .chunks
            .iter()
            .filter(|(k, c)| k.level > 0 && c.last_used + EVICT_AFTER < frame)
            .map(|(k, _)| *k)
            .collect();
        drawn += shown;
        body.in_sight = shown > 0;
        for (k, c) in &body.chunks {
            let in_sight = || chunk_in_sight(m * c.center + centre, c.radius, horizon, &sight);
            if c.last_used != frame || !selected.contains(k) || !in_sight() {
                if let Ok((_, mut vis, _)) = chunks_q.get_mut(c.entity) {
                    vis.set_if_neq(Visibility::Hidden);
                }
            }
        }
        for k in evict {
            if let Some(c) = body.chunks.remove(&k) {
                commands.entity(c.entity).despawn();
            }
        }
        // The zone colours use the colour map's sampler, so both must be in
        // (read only: a modified material is prepared again).
        let zones_loaded = materials
            .get(&body.material)
            .is_some_and(|m| m.extension.zone_map.is_some() && m.extension.color_map.is_some());
        let f = &mut body.frame;
        f.fixed_x = m.x_axis.as_vec3().extend(0.0);
        f.fixed_y = m.y_axis.as_vec3().extend(0.0);
        f.fixed_z = m.z_axis.as_vec3().extend(0.0);
        f.center = centre.as_vec3().extend(0.0);
        // Zones draw once loaded (`update_textures`).
        f.flags.x = if parts.zones && zones_loaded { 1.0 } else { 0.0 };
        // The warp and the zones' pattern (off only in the benchmark).
        f.flags.y = if parts.zone_warp { 1.0 } else { 0.0 };
        f.flags.z = if parts.zone_pattern { 1.0 } else { 0.0 };
    }
    terrain.drawn = drawn;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_out_of_sight_are_culled() {
        // Looking along −z with a 45° vertical field of view, 16:9.
        let sight = Sight::new(DVec3::NEG_Z, std::f64::consts::FRAC_PI_4, 16.0 / 9.0);
        // (case, centre, radius, horizon, in sight)
        let cases = [
            ("ahead", DVec3::new(0.0, 0.0, -1000.0), 10.0, 1e9, true),
            ("behind", DVec3::new(0.0, 0.0, 1000.0), 10.0, 1e9, false),
            ("in the corner", DVec3::new(0.6, 0.3, -1.0).normalize() * 1000.0, 10.0, 1e9, true),
            ("just outside the side, but large", DVec3::new(1.2, 0.0, -1.0).normalize() * 1000.0, 600.0, 1e9, true),
            ("well outside the side", DVec3::new(3.0, 0.0, -1.0).normalize() * 1000.0, 10.0, 1e9, false),
            ("beyond the horizon", DVec3::new(0.0, 0.0, -5000.0), 10.0, 1000.0, false),
            ("around the camera", DVec3::new(0.0, 0.0, 50.0), 100.0, 10.0, true),
        ];
        for (case, c, r, horizon, want) in cases {
            assert_eq!(chunk_in_sight(c, r, horizon, &sight), want, "{case}");
        }
    }

    #[test]
    fn a_chunk_is_drawn_from_the_frame_after_it_is_installed() {
        assert!(!shown_by(10, 10));
        assert!(shown_by(10, 11));
    }

    #[test]
    fn children_start_at_the_parent_shape_and_finish_morphing_at_80_percent_distance() {
        // (parent split measure, expected morph factor)
        let cases = [(Some(1.0), 1.0), (Some(1.125), 0.5), (Some(1.25), 0.0), (Some(3.0), 0.0), (None, 0.0)];
        for (m, expected) in cases {
            assert!((morph_factor(m) - expected).abs() < 1e-6, "{m:?}");
        }
    }
}
