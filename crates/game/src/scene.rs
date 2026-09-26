//! Scene: bodies, ships, the near-ground patch and the trajectory line, all
//! placed camera-relative from f64 simulation state every frame.
//!
//! Minimal graphics on purpose (shaded spheres, one sun light); the visual
//! direction is still to be decided.

use crate::atmosphere;
use crate::body_visual::{self, BodyVisualDef};
use crate::camera::CameraRig;
use crate::state::SimState;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::GlobalAmbientLight;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use glam::{DMat3, DQuat, DVec3};
use sim::body::BodyPhysical;
use sim::frame::{NodeId, Vec3 as FVec3};
use sim::time::Epoch;
use sim::world::World;

#[derive(Component)]
pub struct BodyVisual(pub NodeId);

#[derive(Component)]
pub struct ShipVisual(pub usize);

#[derive(Component)]
pub struct SunLight;

#[derive(Component)]
pub struct GroundPatch;

#[derive(Resource)]
pub struct Assets3d {
    ship_mesh: Handle<Mesh>,
    ship_mat: Handle<StandardMaterial>,
    active_mat: Handle<StandardMaterial>,
}

/// Patch resolution (vertices per side).
const PATCH_N: usize = 97;

pub fn setup(
    mut commands: Commands,
    sim: Res<SimState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut media: ResMut<Assets<ScatteringMedium>>,
) {
    let sphere = meshes.add(Sphere::new(1.0).mesh().uv(256, 128));
    let mut defs = BodyDefs::default();
    for src in &sim.world.sources {
        let (Some(def), Some(p)) = (body_visual::load(&src.name), src.physical.as_ref()) else { continue };
        let [r, g, b] = def.base_color;
        let emissive = def.emissive.as_ref().map_or(LinearRgba::BLACK, |e| {
            let [r, g, b] = e.color;
            LinearRgba::rgb(r, g, b) * e.luminance
        });
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgb(r, g, b),
            emissive,
            perceptual_roughness: def.roughness,
            ..default()
        });
        if let Some(a) = &def.atmosphere {
            let mean = (2.0 * p.radius_eq + p.radius_polar) / 3.0;
            atmosphere::spawn(&mut commands, &mut media, src.node, mean, a);
        }
        if let Some(e) = &def.emissive {
            defs.light_source = Some((src.node, e.illuminance_1au));
        }
        commands.spawn((
            BodyVisual(src.node),
            Mesh3d(sphere.clone()),
            MeshMaterial3d(mat),
            Transform::IDENTITY,
            NoFrustumCulling,
        ));
        defs.defs.push((src.node, def));
    }
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.32, 0.55),
        perceptual_roughness: 0.9,
        ..default()
    });
    let patch = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
    commands.spawn((
        GroundPatch,
        Mesh3d(patch),
        MeshMaterial3d(ground_mat),
        Transform::IDENTITY,
        NoFrustumCulling,
        Visibility::Hidden,
    ));
    commands.spawn((SunLight, DirectionalLight { illuminance: 128_000.0, ..default() }, Transform::IDENTITY));
    commands.insert_resource(GlobalAmbientLight { brightness: 30.0, ..default() });
    commands.insert_resource(defs);
    commands.insert_resource(Assets3d {
        ship_mesh: meshes.add(Cuboid::new(3.0, 3.0, 10.0)),
        ship_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.8, 0.82), ..default() }),
        active_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.85, 0.6), ..default() }),
    });
}

/// Visual definitions of the bodies that have one, and which body lights
/// the scene (with its illuminance at 1 AU).
#[derive(Resource, Default)]
pub struct BodyDefs {
    pub defs: Vec<(NodeId, BodyVisualDef)>,
    pub light_source: Option<(NodeId, f32)>,
}

impl BodyDefs {
    pub fn get(&self, node: NodeId) -> Option<&BodyVisualDef> {
        self.defs.iter().find(|(n, _)| *n == node).map(|(_, d)| d)
    }
}

/// One astronomical unit (m).
const AU: f64 = 1.495_978_707e11;

fn physical(world: &World, node: NodeId) -> Option<&BodyPhysical> {
    world.source(node).and_then(|s| s.physical.as_ref())
}

/// Body-fixed → inertial rotation as a matrix (sim trig, evaluated once).
fn body_matrix(p: &BodyPhysical, t: Epoch) -> DMat3 {
    let col = |v: DVec3| p.rotation.to_inertial(FVec3::from_raw(v), t).raw();
    DMat3::from_cols(col(DVec3::X), col(DVec3::Y), col(DVec3::Z))
}

pub fn update_bodies(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    mut bodies: Query<(&BodyVisual, &mut Transform), Without<SunLight>>,
    mut light: Query<(&mut Transform, &mut DirectionalLight), With<SunLight>>,
) {
    let snap = sim.world.snapshot(sim.clock);
    for (body, mut t) in &mut bodies {
        let Some(p) = physical(&sim.world, body.0) else { continue };
        let pos = snap.relative(body.0, rig.anchor).r - rig.cam_pos;
        // Mesh poles are along +Y; body-fixed poles along +Z.
        let q = DQuat::from_mat3(&body_matrix(p, sim.clock)) * DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2);
        *t = Transform {
            translation: pos.as_vec3(),
            rotation: q.as_quat(),
            scale: Vec3::new(p.radius_eq as f32, p.radius_polar as f32, p.radius_eq as f32),
        };
        if let Some((_, lux)) = defs.light_source.filter(|(n, _)| *n == body.0) {
            if let Ok((mut lt, mut l)) = light.single_mut() {
                *lt = Transform::IDENTITY.looking_to(-pos.normalize().as_vec3(), Vec3::Z);
                l.illuminance = lux * (AU / pos.length()).powi(2) as f32;
            }
        }
    }
}

pub fn update_ships(
    mut commands: Commands,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    assets: Res<Assets3d>,
    mut ships: Query<(Entity, &ShipVisual, &mut Transform, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let snap = sim.world.snapshot(sim.clock);
    let mut seen = vec![false; sim.fleet.len()];
    for (entity, ship, mut t, mut mat) in &mut ships {
        let Some(vessel) = sim.fleet.get(ship.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        seen[ship.0] = true;
        let (anchor, r, _) = vessel.state(&sim.world);
        let pos = snap.relative(anchor, rig.anchor).r + r - rig.cam_pos;
        *t = Transform { translation: pos.as_vec3(), rotation: vessel.attitude.q.as_quat(), scale: Vec3::ONE };
        mat.0 = if ship.0 == sim.active { assets.active_mat.clone() } else { assets.ship_mat.clone() };
    }
    for (i, _) in seen.iter().enumerate().filter(|(_, s)| !**s) {
        commands.spawn((
            ShipVisual(i),
            Mesh3d(assets.ship_mesh.clone()),
            MeshMaterial3d(assets.ship_mat.clone()),
            Transform::IDENTITY,
        ));
    }
}

/// A high-resolution patch of the true ellipsoid under the camera, computed in
/// f64. The sphere mesh is an inscribed polyhedron (hundreds of metres below
/// the surface between vertices); the patch covers it where it matters.
pub fn update_ground(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut patch: Query<(&Mesh3d, &mut Visibility), With<GroundPatch>>,
) {
    let Ok((mesh3d, mut vis)) = patch.single_mut() else { return };
    let snap = sim.world.snapshot(sim.clock);
    let nearest = sim
        .world
        .surfaces()
        .filter_map(|s| {
            let p = s.physical.as_ref()?;
            let center = snap.relative(s.node, rig.anchor).r;
            Some((p, center, (rig.cam_pos - center).length() - p.radius_eq))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((p, center, alt)) = nearest else { return };
    if alt > 600_000.0 {
        *vis = Visibility::Hidden;
        return;
    }
    *vis = Visibility::Visible;
    let m = body_matrix(p, sim.clock);
    let sub = (m.transpose() * (rig.cam_pos - center)).normalize(); // body-fixed direction
    let east = DVec3::Z.cross(sub).try_normalize().unwrap_or(DVec3::X);
    let north = sub.cross(east);
    let half = (alt.max(0.0) * 25.0).clamp(20_000.0, 2_000_000.0);
    let mut positions = Vec::with_capacity(PATCH_N * PATCH_N);
    let mut normals = Vec::with_capacity(PATCH_N * PATCH_N);
    for j in 0..PATCH_N {
        for i in 0..PATCH_N {
            // Quadratic spacing: dense under the camera, sparse at the rim.
            let s = |k: usize| {
                let u = 2.0 * k as f64 / (PATCH_N - 1) as f64 - 1.0;
                u * u.abs() * half
            };
            let dir = (sub + (east * s(i) + north * s(j)) / p.radius_eq).normalize();
            let lat = dir.z.asin();
            let lon = dir.y.atan2(dir.x);
            let fixed = p.surface_point(lat, lon, 0.0).raw();
            positions.push((m * fixed + center - rig.cam_pos).as_vec3().to_array());
            normals.push((m * dir).as_vec3().to_array());
        }
    }
    let mut indices = Vec::with_capacity((PATCH_N - 1) * (PATCH_N - 1) * 6);
    for j in 0..PATCH_N - 1 {
        for i in 0..PATCH_N - 1 {
            let a = (j * PATCH_N + i) as u32;
            let (b, c, d) = (a + 1, a + PATCH_N as u32, a + PATCH_N as u32 + 1);
            indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    if let Some(mut mesh) = meshes.get_mut(&mesh3d.0) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_indices(Indices::U32(indices));
    }
}
