//! Sky (A6): the starfield from the Yale Bright Star Catalogue, the Sun's
//! lens flare, and Earthshine (light reflected by the nearest bright body).

mod flare;
mod stars;

pub use flare::draw as draw_flare;
pub use stars::{setup as setup_stars, update as update_stars};

use crate::camera::CameraRig;
use crate::scene::BodyDefs;
use crate::settings::GraphicsSettings;
use crate::state::SimState;
use bevy::asset::embedded_asset;
use bevy::light::{GlobalAmbientLight, SunDisk};
use bevy::prelude::*;
use glam::DVec3;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "stars.wgsl");
        app.add_plugins(MaterialPlugin::<stars::StarMaterial>::default());
    }
}

/// Light reflected from a planet onto nearby objects.
#[derive(Component)]
pub struct Earthshine;

pub fn setup_earthshine(mut commands: Commands) {
    commands.spawn((
        Earthshine,
        DirectionalLight { illuminance: 0.0, shadow_maps_enabled: false, ..default() },
        SunDisk::OFF,
        Transform::IDENTITY,
    ));
}

/// Lambert-sphere phase function: reflected light at phase angle `alpha`
/// relative to full phase.
fn lambert_phase(alpha: f64) -> f64 {
    (alpha.sin() + (std::f64::consts::PI - alpha) * alpha.cos()) / std::f64::consts::PI
}

/// Picks the body that reflects the most sunlight onto the camera and
/// points the Earthshine light from it.
pub fn update_earthshine(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    settings: Res<GraphicsSettings>,
    mut light: Query<(&mut DirectionalLight, &mut Transform), With<Earthshine>>,
) {
    let Ok((mut l, mut t)) = light.single_mut() else { return };
    let Some((sun, sun_lux_1au)) = defs.light_source else { return };
    if !settings.earthshine {
        l.illuminance = 0.0;
        return;
    }
    let snap = sim.world.snapshot(sim.clock);
    let sun_pos = snap.relative_r(sun, rig.anchor) - rig.cam_pos;
    let best = defs
        .defs
        .iter()
        .filter(|(_, d)| d.emissive.is_none())
        .filter_map(|(node, d)| {
            let radius = sim.world.source(*node)?.physical.as_ref()?.radius_eq;
            let body = snap.relative_r(*node, rig.anchor) - rig.cam_pos;
            let to_cam = -body;
            let to_sun = sun_pos - body;
            let alpha = to_cam.normalize().dot(to_sun.normalize()).clamp(-1.0, 1.0).acos();
            let e_sun = f64::from(sun_lux_1au) * (1.495_978_707e11 / to_sun.length()).powi(2);
            let e = e_sun * f64::from(d.albedo) * 2.0 / 3.0 * (radius / to_cam.length()).powi(2) * lambert_phase(alpha);
            let [r, g, b] = d.base_color;
            Some((e, to_cam, Color::srgb(r, g, b)))
        })
        .max_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((e, dir, color)) = best {
        l.illuminance = e as f32;
        l.color = color;
        *t = Transform::IDENTITY.looking_to(dir.normalize().as_vec3(), Vec3::Z);
    }
}

/// Ambient light: a small floor everywhere (so night sides are not pure
/// black on screen), plus a stand-in for sky light when the atmosphere is
/// not rendered (with it, the atmosphere's environment map provides it).
pub fn update_ambient(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    settings: Res<GraphicsSettings>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    let sky = if settings.atmosphere == crate::settings::AtmosphereQuality::Off {
        stars::daylight(&sim, &rig, &defs) * SKY_AMBIENT
    } else {
        0.0
    };
    let target = NIGHT_AMBIENT + sky;
    if (ambient.brightness - target).abs() > 1.0 {
        ambient.brightness = target;
    }
}

/// Ambient brightness floor, and the daylight sky stand-in (Bevy units).
const NIGHT_AMBIENT: f32 = 30.0;
const SKY_AMBIENT: f32 = 4000.0;

/// Fraction of the Sun's disc not hidden by any body, from a camera at the
/// render origin (samples on the disc).
pub fn sun_visibility(sim: &SimState, rig: &CameraRig, sun: sim::frame::NodeId, sun_radius: f64) -> f64 {
    let snap = sim.world.snapshot(sim.clock);
    let sun_pos = snap.relative_r(sun, rig.anchor) - rig.cam_pos;
    let dir = sun_pos.normalize();
    let (e1, e2) = crate::camera::basis(dir);
    let bodies: Vec<(DVec3, f64)> = sim
        .world
        .surfaces()
        .filter_map(|s| {
            let p = s.physical.as_ref()?;
            Some((snap.relative_r(s.node, rig.anchor) - rig.cam_pos, (p.radius_eq + p.radius_polar) * 0.5))
        })
        .collect();
    let samples: Vec<DVec3> = std::iter::once(DVec3::ZERO)
        .chain((0..8).map(|k| {
            let a = f64::from(k) * std::f64::consts::TAU / 8.0;
            (e1 * a.cos() + e2 * a.sin()) * sun_radius * 0.7
        }))
        .map(|o| sun_pos + o)
        .collect();
    let visible = samples
        .iter()
        .filter(|p| {
            let d = p.normalize();
            let len = p.length();
            !bodies.iter().any(|(c, r)| {
                let t = c.dot(d);
                t > 0.0 && t < len && (*c - d * t).length() < *r
            })
        })
        .count();
    visible as f64 / samples.len() as f64
}
