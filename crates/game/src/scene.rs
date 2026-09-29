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
            defs.light_source = Some((src.node, crate::lighting::luminous_power(e.luminosity_w, e.luminous_efficacy)));
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
    commands.insert_resource(GlobalAmbientLight { brightness: 0.0, ..default() });
    commands.insert_resource(defs);
    commands.insert_resource(Assets3d { ship_mesh: meshes.add(craft_mesh(&sim::craft::test_craft().mesh)) });
}

/// A Bevy mesh from a craft's render mesh (body axes, +Z the nose).
fn craft_mesh(m: &sim::craft::RenderMesh) -> Mesh {
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone())
        .with_inserted_indices(bevy::mesh::Indices::U32(m.indices.clone()))
}

/// Visual definitions of the bodies that have one, and which body lights
/// the scene.
#[derive(Resource, Default)]
pub struct BodyDefs {
    pub defs: Vec<(NodeId, BodyVisualDef)>,
    /// The star lighting the scene and its luminous power (lm).
    pub light_source: Option<(NodeId, f64)>,
}

impl BodyDefs {
    pub fn get(&self, node: NodeId) -> Option<&BodyVisualDef> {
        self.defs.iter().find(|(n, _)| *n == node).map(|(_, d)| d)
    }
}

fn physical(world: &World, node: NodeId) -> Option<&BodyPhysical> {
    world.source(node).and_then(|s| s.physical.as_ref())
}

/// Body-fixed → inertial rotation as a matrix (sim trig, evaluated once).
pub fn body_matrix(p: &BodyPhysical, t: Epoch) -> DMat3 {
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
        if let Some((_, lm)) = defs.light_source.filter(|(n, _)| *n == body.0) {
            if let Ok((mut lt, mut l)) = light.single_mut() {
                // The shared light carries the flux at the camera; each lit
                // body rescales it to its own distance (lighting.rs).
                let dir = -pos.normalize().as_vec3();
                *lt = Transform::IDENTITY.looking_to(dir, light_up(dir));
                l.illuminance = crate::lighting::flux(lm, pos.length()) as f32;
            }
        }
    }
}

/// An up vector for a light shining along `dir` (unit): +Z unless the
/// light shines (nearly) along it, then +X, so the orientation is never
/// degenerate.
pub fn light_up(dir: Vec3) -> Vec3 {
    if dir.cross(Vec3::Z).length() > 1e-3 {
        Vec3::Z
    } else {
        Vec3::X
    }
}

/// Ship base colours (sRGB) for other vessels and the active one.
const SHIP_COLOR: [f32; 3] = [0.8, 0.8, 0.82];
const ACTIVE_COLOR: [f32; 3] = [0.95, 0.85, 0.6];

#[allow(clippy::too_many_arguments)]
pub fn update_ships(
    mut commands: Commands,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    assets: Res<Assets3d>,
    defs: Res<BodyDefs>,
    settings: Res<crate::settings::GraphicsSettings>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ships: Query<(Entity, &ShipVisual, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
) {
    use crate::lighting::{object_light, Sphere};
    let snap = sim.world.snapshot(sim.clock);
    let rel = |n: NodeId| snap.relative_r(n, rig.anchor) - rig.cam_pos;
    let star = defs.light_source.map(|(n, lm)| (rel(n), physical(&sim.world, n).map_or(0.0, |p| p.radius_eq), lm));
    let bodies: Vec<Sphere> = sim
        .world
        .surfaces()
        .filter_map(|s| {
            let p = s.physical.as_ref()?;
            let albedo = defs.get(s.node).map_or(0.3, |d| f64::from(d.albedo));
            Some(Sphere { centre: rel(s.node), radius: p.radius_eq, albedo })
        })
        .collect();
    let mut seen = vec![false; sim.fleet.len()];
    for (entity, ship, mut t, mat) in &mut ships {
        let Some(vessel) = sim.fleet.get(ship.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        seen[ship.0] = true;
        let (anchor, r, _) = vessel.state_at(&sim.world, sim.clock);
        let pos = snap.relative(anchor, rig.anchor).r + r - rig.cam_pos;
        // The vessel's position is its centre of mass; the mesh is in the
        // craft's body axes.
        let q = vessel.attitude.q;
        let origin = pos - q * vessel.mass_props().com;
        *t = Transform { translation: origin.as_vec3(), rotation: q.as_quat(), scale: Vec3::ONE };
        // Per-ship light (D055): the shared sunlight dimmed where a body
        // hides the Sun, plus planetshine as a diffuse glow where it does
        // (in sunlight the sky light and sunlight already dominate, and a
        // uniform glow would light every face).
        let (visible, shine) = star.map_or((1.0, 0.0), |(c, radius, lm)| object_light(pos, c, radius, lm, &bodies));
        let shine = if settings.earthshine { shine * (1.0 - visible) } else { 0.0 };
        let [r0, g0, b0] = if ship.0 == sim.active { ACTIVE_COLOR } else { SHIP_COLOR };
        let LinearRgba { red: r0, green: g0, blue: b0, .. } = Color::srgb(r0, g0, b0).to_linear();
        let k = visible as f32;
        let glow = (shine / std::f64::consts::PI) as f32;
        if let Some(m) = materials.get(&mat.0) {
            let want_base = LinearRgba::rgb(r0 * k, g0 * k, b0 * k);
            let want_glow = LinearRgba::rgb(r0 * glow, g0 * glow, b0 * glow);
            let base = m.base_color.to_linear();
            let close =
                |a: LinearRgba, b: LinearRgba| (a.red - b.red).abs() + (a.green - b.green).abs() < 1e-3 * (1.0 + b.red);
            if !close(base, want_base) || !close(m.emissive, want_glow) {
                if let Some(mut m) = materials.get_mut(&mat.0) {
                    m.base_color = want_base.into();
                    m.emissive = want_glow;
                }
            }
        }
    }
    for (i, _) in seen.iter().enumerate().filter(|(_, s)| !**s) {
        // Each ship gets its own material: its light depends on where it is.
        let [r0, g0, b0] = SHIP_COLOR;
        let material = materials.add(StandardMaterial { base_color: Color::srgb(r0, g0, b0), ..default() });
        commands.spawn((
            ShipVisual(i),
            Mesh3d(assets.ship_mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sun_light_has_a_valid_up_in_every_direction() {
        // (direction, expected up)
        let cases = [
            (Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X),
            (-Vec3::Z, Vec3::X),
            (Vec3::new(0.0, 1e-5, 1.0).normalize(), Vec3::X),
        ];
        for (dir, up) in cases {
            assert_eq!(light_up(dir), up, "{dir:?}");
            let t = Transform::IDENTITY.looking_to(dir, light_up(dir));
            assert!((t.forward().as_vec3() - dir).length() < 1e-5 && t.rotation.is_finite());
        }
    }
}
