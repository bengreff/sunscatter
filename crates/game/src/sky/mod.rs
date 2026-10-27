//! Sky (A6): the starfield from the Yale Bright Star Catalogue, the stars'
//! flare, and the ambient (starlight) level. Planetshine is per body
//! in `lighting`.

mod flare;
mod haze;
mod stars;

pub use flare::draw as draw_flare;
pub use haze::install as install_haze;
pub use stars::{setup as setup_stars, update as update_stars};

use crate::camera::CameraRig;
use crate::scene::BodyDefs;
use crate::state::SimState;
use bevy::asset::embedded_asset;
use bevy::light::GlobalAmbientLight;
use bevy::prelude::*;
use sim::frame::NodeId;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "stars.wgsl");
        app.add_plugins(MaterialPlugin::<stars::StarMaterial>::default());
    }
}

/// Ambient light: starlight everywhere (night sides get only that and
/// planetshine, D055), plus a stand-in for the atmosphere's sky light
/// ([`sky_ambient`]). Bevy's ambient light is global; the terrain takes
/// only its share ([`ambient_share`], `lighting`).
pub fn update_ambient(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    let target = NIGHT_AMBIENT + sky_ambient(&sim, &rig, &defs).0;
    if (ambient.brightness - target).abs() > 1e-4 * target.max(1e-3) {
        ambient.brightness = target;
    }
}

/// The daylight sky stand-in (Bevy units) around the camera, and the body
/// whose air it is. Bevy's environment map from the atmosphere would be the
/// sky light itself, but it is regenerated and filtered every frame (~3 ms
/// at 1600×900, ~12 ms on a Retina window), so the look uses this.
pub fn sky_ambient(sim: &SimState, rig: &CameraRig, defs: &BodyDefs) -> (f32, Option<NodeId>) {
    let (day, body) = stars::daylight_of(sim, rig, defs);
    (day * SKY_AMBIENT, body)
}

/// The sky's light around a craft at `at` (camera-relative; Bevy's
/// ambient units, a radiance): starlight, plus the daylight sky stand-in of
/// the air the craft is in (D110: its own air, never the camera's).
pub fn sky_radiance_at(at: glam::DVec3, scene: &crate::lighting::LightScene, defs: &BodyDefs) -> f32 {
    let Some(sun) = scene.slots.first().map(|&k| scene.stars[k].centre) else { return NIGHT_AMBIENT };
    let day = (scene.airs(defs).iter())
        .map(|a| {
            let up = at - a.centre;
            stars::daylight_from(up, up.length() - a.radius, f64::from(a.def.rayleigh_scale_height_m), sun - at)
        })
        .fold(0.0, f32::max);
    NIGHT_AMBIENT + day * SKY_AMBIENT
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
