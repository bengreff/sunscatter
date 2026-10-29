//! Scene: self-luminous bodies (stars) as spheres, ships, the sun light and
//! the per-body visual definitions, all placed camera-relative from f64
//! simulation state every frame. Surfaces are drawn by `terrain`.

pub mod exhaust;
pub mod loading;
pub mod parts;
pub mod pick;
pub mod ships;

pub use ships::{ShipModels, ShipVisual};

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
pub struct SunLight;

/// Vessel `.0`'s deployed parachute canopy (D073: a functional mark).
#[derive(Component)]
pub struct CanopyVisual(pub usize);

#[derive(Resource)]
pub struct Assets3d {
    canopy_mesh: Handle<Mesh>,
    canopy_material: Handle<StandardMaterial>,
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
            defs.stars.push(crate::lighting::StarDef {
                node: src.node,
                lm: crate::lighting::luminous_power(e.luminosity_w, e.luminous_efficacy),
                tint: e.temperature_k.map_or([1.0; 3], crate::lighting::color::star_tint),
            });
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
    // One directional light per drawn star (lighting::star_slots), at most
    // MAX_STARS; only the brightest casts shadow maps (look::apply).
    for slot in 0..defs.stars.len().min(crate::lighting::MAX_STARS) {
        commands.spawn((
            SunLight,
            crate::lighting::StarLight(slot),
            DirectionalLight { illuminance: 0.0, ..default() },
            bevy::light::SunDisk::EARTH,
            cascades.clone(),
            Transform::IDENTITY,
        ));
    }
    crate::lighting::shadows::spawn_light(&mut commands);
    commands.insert_resource(GlobalAmbientLight { brightness: 0.0, ..default() });
    commands.insert_resource(defs);
    commands.insert_resource(Assets3d {
        // A unit shallow cone, scaled per vessel by `canopy`.
        canopy_mesh: meshes.add(Cone { radius: 1.0, height: CANOPY_DEPTH }),
        canopy_material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.45, 0.2),
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
    });
}

/// Visual definitions of the bodies that have one, and which body lights
/// the scene.
#[derive(Resource, Default)]
pub struct BodyDefs {
    pub defs: Vec<(NodeId, BodyVisualDef)>,
    /// Every star (an emissive body), in the world's order.
    pub stars: Vec<crate::lighting::StarDef>,
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
    mut bodies: Query<(&BodyVisual, &mut Transform), Without<SunLight>>,
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

/// Whether vessel `i`'s engine is running now: a burn segment of its stored
/// trajectory, or live flight with an engine giving thrust at the pilot's
/// throttle (`Vessel::engine_running`: propellant or debug mode; a craft of
/// parts' running engines).
pub fn engine_on(sim: &SimState, i: usize) -> bool {
    let v = &sim.fleet[i];
    if let Some(seg) = v.trajectory().and_then(|tr| tr.segment_at(sim.clock)) {
        return matches!(seg.kind, sim::vessel::SegmentKind::Burn(_));
    }
    i == sim.active && matches!(v.phase, sim::vessel::Phase::Powered { .. }) && v.engine_running(sim.controls())
}

/// Drag coefficient of a round canopy on its nominal area (Knacke,
/// *Parachute Recovery Systems Design Manual*, NWC TP 6575, 1991, table
/// 5-1: flat circular 0.75-0.80), used only to size the mark from Cd·A.
const CANOPY_CD: f64 = 0.75;
/// Suspension lines about one nominal diameter long (Knacke: 1.0-1.15 D0).
const RISER_PER_DIAMETER: f64 = 1.0;
/// The canopy mark's depth as a fraction of its radius.
const CANOPY_DEPTH: f32 = 0.5;

/// Where a deployed canopy sits (body axes): its centre, its axis (the
/// direction the canopy trails, downwind of the mount) and its radius (m).
/// `wind_body` is the air's velocity relative to the craft in body axes;
/// without it (no air, no wind) the canopy trails beyond the mount along
/// the mount's own direction from the centre of the body.
fn canopy(cd_area: f64, mount: DVec3, wind_body: Option<DVec3>) -> (DVec3, DVec3, f64) {
    let radius = (cd_area / CANOPY_CD / std::f64::consts::PI).sqrt();
    let axis = wind_body.and_then(|w| w.try_normalize()).or_else(|| mount.try_normalize()).unwrap_or(DVec3::Z);
    (mount + axis * (2.0 * radius * RISER_PER_DIAMETER), axis, radius)
}

/// Places the canopy marks: shown for every live vessel with its chute
/// deployed, trailing downwind of its mount; hidden otherwise.
pub fn update_canopies(
    mut commands: Commands,
    sim: Res<SimState>,
    assets: Res<Assets3d>,
    ships: Query<(&ShipVisual, &Transform), Without<CanopyVisual>>,
    mut canopies: Query<(Entity, &CanopyVisual, &mut Transform, &mut Visibility)>,
) {
    let mut seen = vec![false; sim.fleet.len()];
    let snap = sim.world.snapshot(sim.clock);
    for (entity, c, mut t, mut vis) in &mut canopies {
        let Some(vessel) = sim.fleet.get(c.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        seen[c.0] = true;
        let ship = ships.iter().find(|(s, _)| s.index == c.0).map(|(_, t)| *t);
        let live = matches!(vessel.phase, sim::vessel::Phase::Powered { .. });
        let (Some(ship), true) = (ship, live && vessel.chute.open()) else {
            *vis = Visibility::Hidden;
            continue;
        };
        let (anchor, r, v) = vessel.state_at(&sim.world, sim.clock);
        let wind = sim::vessel::air_at(&sim.world, &snap, anchor, r, v).map(|a| vessel.attitude.q.inverse() * a.wind);
        let (centre, axis, radius) = canopy(vessel.chute.cd_area(&vessel.craft.chute), vessel.craft.chute.mount, wind);
        // The cone's axis is +Y with its apex up: the apex trails downwind.
        let local = Transform {
            translation: centre.as_vec3(),
            rotation: Quat::from_rotation_arc(Vec3::Y, axis.as_vec3()),
            scale: Vec3::splat(radius as f32),
        };
        *t = ship * local;
        *vis = Visibility::Visible;
    }
    for (i, _) in seen.iter().enumerate().filter(|(_, s)| !**s) {
        commands.spawn((
            CanopyVisual(i),
            Mesh3d(assets.canopy_mesh.clone()),
            MeshMaterial3d(assets.canopy_material.clone()),
            Transform::IDENTITY,
            Visibility::Hidden,
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

    #[test]
    fn a_canopy_trails_downwind_of_its_mount_one_diameter_out() {
        let mount = DVec3::new(0.0, 0.0, 6.8);
        // Falling nose-up: the air comes from below, the canopy is above.
        let (centre, axis, radius) = canopy(600.0, mount, Some(DVec3::new(0.0, 0.0, 23.0)));
        assert!((radius - (800.0 / std::f64::consts::PI).sqrt()).abs() < 1e-9, "{radius}");
        assert!((axis - DVec3::Z).length() < 1e-12);
        assert!((centre - (mount + DVec3::Z * 2.0 * radius)).length() < 1e-9);
        // Swinging: the canopy follows the wind, not the nose.
        let (c, a, _) = canopy(600.0, mount, Some(DVec3::new(10.0, 0.0, 0.0)));
        assert!((a - DVec3::X).length() < 1e-12 && c.x > 0.0);
        // No wind: beyond the mount.
        assert!((canopy(600.0, mount, Some(DVec3::ZERO)).1 - DVec3::Z).length() < 1e-12);
        assert!((canopy(600.0, mount, None).1 - DVec3::Z).length() < 1e-12);
    }
}
