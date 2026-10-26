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
use sim::frame::NodeId;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "stars.wgsl");
        app.add_plugins(MaterialPlugin::<stars::StarMaterial>::default());
    }
}

/// Ambient light: starlight everywhere (night sides get only that and
/// planetshine, D055), plus a stand-in for sky light when the atmosphere's
/// own is not drawn ([`sky_ambient`]). Bevy's ambient light is global; the
/// terrain takes only its share ([`ambient_share`], `lighting`).
pub fn update_ambient(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    settings: Res<GraphicsSettings>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    let target = NIGHT_AMBIENT + sky_ambient(&sim, &rig, &defs, &settings).0;
    if (ambient.brightness - target).abs() > 1e-4 * target.max(1e-3) {
        ambient.brightness = target;
    }
}

/// The daylight sky stand-in (Bevy units) around the camera, and the body
/// whose air it is: zero with the atmosphere off-screen or with its sky
/// light on (the atmosphere's environment map lights things then).
pub fn sky_ambient(
    sim: &SimState,
    rig: &CameraRig,
    defs: &BodyDefs,
    settings: &GraphicsSettings,
) -> (f32, Option<NodeId>) {
    if settings.atmosphere == crate::settings::AtmosphereQuality::Off || !settings.sky_light {
        let (day, body) = stars::daylight_of(sim, rig, defs);
        (day * SKY_AMBIENT, body)
    } else {
        (0.0, None)
    }
}

/// The share of the global ambient light a body's surface receives. The
/// sky stand-in is the sky around the camera, so only the body whose air
/// that is gets it; every other body (the Moon seen from the pad) gets
/// starlight only. `sky`: [`sky_ambient`].
pub fn ambient_share(body: NodeId, sky: (f32, Option<NodeId>)) -> f32 {
    match sky {
        (s, Some(owner)) if s > 0.0 && owner != body => NIGHT_AMBIENT / (NIGHT_AMBIENT + s),
        _ => 1.0,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sky_stand_in_lights_only_its_own_body() {
        let (earth, moon) = (NodeId(3), NodeId(4));
        let night = NIGHT_AMBIENT / (NIGHT_AMBIENT + SKY_AMBIENT);
        // (body, sky ambient and whose air, share of the global ambient)
        let cases = [
            (earth, (SKY_AMBIENT, Some(earth)), 1.0), // at the pad in daylight: Earth's ground
            (moon, (SKY_AMBIENT, Some(earth)), night), // the Moon seen from the pad: starlight
            (moon, (0.0, Some(earth)), 1.0),          // night at the pad: starlight is all there is
            (moon, (0.0, None), 1.0),                 // in space
            (earth, (0.0, None), 1.0),
        ];
        for (body, sky, want) in cases {
            assert!((ambient_share(body, sky) - want).abs() < 1e-9, "{body:?} {sky:?}");
        }
        // Starlight is kept: share × (starlight + sky) = starlight.
        assert!((night * (NIGHT_AMBIENT + SKY_AMBIENT) - NIGHT_AMBIENT).abs() < 1e-9);
    }
}
