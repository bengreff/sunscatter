//! Sky (A6): the starfield from the Yale Bright Star Catalogue, the Sun's
//! lens flare, and the ambient (starlight) level. Planetshine is per body
//! in `lighting`.

mod flare;
mod haze;
mod stars;

pub use flare::draw as draw_flare;
pub use haze::install as install_haze;
pub use stars::{setup as setup_stars, update as update_stars};

use crate::camera::CameraRig;
use crate::scene::BodyDefs;
use crate::settings::GraphicsSettings;
use crate::state::SimState;
use bevy::asset::embedded_asset;
use bevy::light::GlobalAmbientLight;
use bevy::prelude::*;
use glam::DVec3;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "stars.wgsl");
        app.add_plugins(MaterialPlugin::<stars::StarMaterial>::default());
    }
}

/// Ambient light: starlight everywhere (night sides get only that and
/// planetshine, D055), plus a stand-in for sky light when the atmosphere is
/// not rendered (with it, the atmosphere's environment map provides it).
pub fn update_ambient(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    settings: Res<GraphicsSettings>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    // With the sky light on, the atmosphere's environment map lights things
    // inside it; otherwise a daylight stand-in.
    let sky = if settings.atmosphere == crate::settings::AtmosphereQuality::Off || !settings.sky_light {
        stars::daylight(&sim, &rig, &defs) * SKY_AMBIENT
    } else {
        0.0
    };
    let target = NIGHT_AMBIENT + sky;
    if (ambient.brightness - target).abs() > 1e-4 * target.max(1e-3) {
        ambient.brightness = target;
    }
}

/// Starlight (~0.001 lux on a white surface; Bevy's ambient is in cd/m²,
/// so / π), and the daylight sky stand-in (Bevy units).
const NIGHT_AMBIENT: f32 = 0.001 / std::f32::consts::PI;
const SKY_AMBIENT: f32 = 4000.0;

/// Fraction of the Sun's disc not hidden by any body, from the camera (at
/// the render origin); `lighting::eclipse_factor` is the one rule.
pub fn sun_visibility(sim: &SimState, rig: &CameraRig, sun: sim::frame::NodeId, sun_radius: f64) -> f64 {
    let snap = sim.world.snapshot(sim.clock);
    let sun_pos = snap.relative_r(sun, rig.anchor) - rig.cam_pos;
    let bodies: Vec<(DVec3, f64)> = sim
        .world
        .surfaces()
        .filter_map(|s| {
            let p = s.physical.as_ref()?;
            Some((snap.relative_r(s.node, rig.anchor) - rig.cam_pos, (p.radius_eq + p.radius_polar) * 0.5))
        })
        .collect();
    crate::lighting::eclipse_factor(DVec3::ZERO, sun_pos, sun_radius, &bodies)
}
