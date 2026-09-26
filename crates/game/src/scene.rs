//! Scene: bodies, ships, the near-ground patch and the trajectory line, all
//! placed camera-relative from f64 simulation state every frame.
//!
//! Minimal graphics on purpose (shaded spheres, one sun light); the visual
//! direction is still to be decided.

use crate::camera::CameraRig;
use crate::hud::{PlotFrame, UiState};
use crate::state::{Prediction, SimState};
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::GlobalAmbientLight;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use glam::{DMat3, DQuat, DVec3};
use sim::body::BodyPhysical;
use sim::frame::{NodeId, Vec3 as FVec3};
use sim::time::Epoch;
use sim::vessel::Segment;
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
) {
    let sphere = meshes.add(Sphere::new(1.0).mesh().uv(256, 128));
    for src in &sim.world.sources {
        let (color, emissive) = match src.name.as_str() {
            "Earth" => (Color::srgb(0.18, 0.32, 0.55), LinearRgba::BLACK),
            "Moon" => (Color::srgb(0.55, 0.55, 0.53), LinearRgba::BLACK),
            "Sun" => (Color::srgb(1.0, 0.95, 0.8), LinearRgba::rgb(40.0, 36.0, 28.0)),
            _ => continue,
        };
        let mat =
            materials.add(StandardMaterial { base_color: color, emissive, perceptual_roughness: 0.9, ..default() });
        commands.spawn((
            BodyVisual(src.node),
            Mesh3d(sphere.clone()),
            MeshMaterial3d(mat),
            Transform::IDENTITY,
            NoFrustumCulling,
        ));
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
    commands.spawn((SunLight, DirectionalLight { illuminance: 12_000.0, ..default() }, Transform::IDENTITY));
    commands.insert_resource(GlobalAmbientLight { brightness: 30.0, ..default() });
    commands.insert_resource(Assets3d {
        ship_mesh: meshes.add(Cuboid::new(3.0, 3.0, 10.0)),
        ship_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.8, 0.82), ..default() }),
        active_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.85, 0.6), ..default() }),
    });
}

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
    mut bodies: Query<(&BodyVisual, &mut Transform), Without<SunLight>>,
    mut light: Query<&mut Transform, With<SunLight>>,
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
        if p.name == "Sun" {
            if let Ok(mut lt) = light.single_mut() {
                *lt = Transform::IDENTITY.looking_to(-pos.normalize().as_vec3(), Vec3::Z);
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

/// Earth–Moon rotating basis at `t` (x → Moon, z → orbit normal).
fn rotating_basis(world: &World, t: Epoch) -> DMat3 {
    let (earth, moon) = (world.find("Earth").map(|s| s.node), world.find("Moon").map(|s| s.node));
    let (Some(e), Some(m)) = (earth, moon) else { return DMat3::IDENTITY };
    let k = world.eph.relative(m, e, t);
    let x = k.r.normalize();
    let z = k.r.cross(k.v).normalize();
    DMat3::from_cols(x, z.cross(x), z)
}

/// Number of points used to draw a trajectory.
const TRAJECTORY_POINTS: usize = 600;

/// Draws the future part of the stored segment (coasting) or the background
/// prediction (powered), resampled at uniform times with the segment's own
/// interpolation, so the line starts exactly at the ship and stays smooth.
pub fn draw_trajectory(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    pred: Res<Prediction>,
    ui: Res<UiState>,
    mut gizmos: Gizmos,
) {
    let seg: Option<&Segment> = sim.ship().segment().or(pred.segment.as_ref());
    let Some(seg) = seg else { return };
    let Some(earth) = sim.world.find("Earth").map(|s| s.node) else { return };
    let t_start = sim.clock.seconds_since(seg.t0).max(seg.samples.first().map_or(0.0, |s| s.s.t));
    let t_end = seg.computed_until();
    if t_end <= t_start {
        return;
    }
    let earth_now = sim.world.snapshot(sim.clock).relative_r(earth, rig.anchor);
    let basis_now = match ui.plot_frame {
        PlotFrame::EarthInertial => DMat3::IDENTITY,
        PlotFrame::EarthMoonRotating => rotating_basis(&sim.world, sim.clock),
    };
    // While powered the prediction was computed a moment ago; join it to the
    // ship's current position so the line always starts at the ship.
    let (ship_anchor, ship_r, _) = sim.ship().state(&sim.world);
    let ship_now = sim.world.snapshot(sim.clock).relative_r(ship_anchor, rig.anchor) + ship_r - rig.cam_pos;
    let resampled = (0..TRAJECTORY_POINTS).filter_map(|k| {
        let t = t_start + (t_end - t_start) * k as f64 / (TRAJECTORY_POINTS - 1) as f64;
        let (anchor, r, _) = seg.eval(t)?;
        let epoch = seg.t0.add_seconds(t);
        let rel_earth = r + sim.world.eph.relative(anchor, earth, epoch).r;
        let plotted = match ui.plot_frame {
            PlotFrame::EarthInertial => rel_earth,
            PlotFrame::EarthMoonRotating => basis_now * (rotating_basis(&sim.world, epoch).transpose() * rel_earth),
        };
        Some((plotted + earth_now - rig.cam_pos).as_vec3())
    });
    gizmos.linestrip(std::iter::once(ship_now.as_vec3()).chain(resampled), Color::srgb(1.0, 0.85, 0.2));
}
