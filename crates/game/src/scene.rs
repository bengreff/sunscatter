//! Scene: self-luminous bodies (stars) as spheres, ships, the sun light and
//! the per-body visual definitions, all placed camera-relative from f64
//! simulation state every frame. Surfaces are drawn by `terrain`.

use crate::atmosphere;
use crate::body_visual::{self, BodyVisualDef};
use crate::camera::CameraRig;
use crate::state::SimState;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::atmosphere::ScatteringMedium;
use bevy::light::{CascadeShadowConfigBuilder, GlobalAmbientLight, NotShadowCaster};
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

#[derive(Resource)]
pub struct Assets3d {
    ship_mesh: Handle<Mesh>,
    ship_mat: Handle<StandardMaterial>,
    active_mat: Handle<StandardMaterial>,
}

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
        // Bodies with a surface are drawn by the terrain system.
        if def.emissive.is_none() {
            defs.defs.push((src.node, def));
            continue;
        }
        commands.spawn((
            BodyVisual(src.node),
            // A light source must not shadow what it lights.
            NotShadowCaster,
            Mesh3d(sphere.clone()),
            MeshMaterial3d(mat),
            Transform::IDENTITY,
            NoFrustumCulling,
        ));
        defs.defs.push((src.node, def));
    }
    // Shadows are for ships and nearby ground: cascades out to a few km.
    let cascades = CascadeShadowConfigBuilder {
        num_cascades: 3,
        first_cascade_far_bound: 150.0,
        maximum_distance: 4000.0,
        ..default()
    }
    .build();
    commands.spawn((SunLight, DirectionalLight { illuminance: 128_000.0, ..default() }, cascades, Transform::IDENTITY));
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
        let (anchor, r, _) = vessel.state_at(&sim.world, sim.clock);
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
