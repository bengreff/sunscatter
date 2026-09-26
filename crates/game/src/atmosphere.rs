//! Atmospheric scattering (A5) for every body with an atmosphere definition,
//! using Bevy's Hillaire 2020 implementation. Each atmosphere entity sits at
//! its body's centre, camera-relative, updated every frame; the camera uses
//! the nearest one. The medium is built from absolute scale heights in the
//! body's visual data.
//!
//! Note: Bevy's `ScatteringMedium::earth` swaps the Mie absorption and
//! scattering coefficients relative to Hillaire's paper (scattering 3.996e-6,
//! extinction 4.44e-6 per m); our data uses the paper's values.

use crate::body_visual::AtmosphereDef;
use crate::camera::{CameraRig, MainCamera};
use crate::settings::{AtmosphereQuality, GraphicsSettings};
use crate::state::SimState;
use bevy::light::atmosphere::{Falloff, PhaseFunction, ScatteringMedium, ScatteringTerm};
use bevy::light::{Atmosphere, AtmosphereEnvironmentMapLight, GeneratedEnvironmentMapLight};
use bevy::pbr::resources::AtmosphereTransformsOffset;
use bevy::pbr::{AtmosphereMode, AtmosphereSettings, ExtractedAtmosphere};
use bevy::prelude::*;
use bevy::render::{Render, RenderApp, RenderSystems};
use sim::frame::NodeId;

/// Works around stale render state in Bevy 0.19's atmosphere: when a
/// camera loses its atmosphere, its render entity keeps the per-view
/// components, so the sky pass keeps running (a grey sky, or a crash after an
/// MSAA change, since its pipeline is no longer re-specialised). Removing the
/// transform offset, which both atmosphere passes require, stops them.
pub struct AtmosphereFixPlugin;

impl Plugin for AtmosphereFixPlugin {
    fn build(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(Render, drop_stale_atmosphere.in_set(RenderSystems::PrepareBindGroups));
        }
    }
}

fn drop_stale_atmosphere(
    mut commands: Commands,
    stale: Query<Entity, (With<AtmosphereTransformsOffset>, Without<ExtractedAtmosphere>)>,
) {
    for e in &stale {
        commands.entity(e).remove::<AtmosphereTransformsOffset>();
    }
}

/// An atmosphere entity belonging to a body, with its atmosphere (the
/// `Atmosphere` component itself is removed while the atmosphere is off).
#[derive(Component)]
pub struct BodyAtmosphere(pub NodeId, pub Atmosphere);

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

/// Builds the scattering medium; falloff parameters are normalised by the
/// atmosphere thickness as Bevy expects.
pub fn medium(def: &AtmosphereDef) -> ScatteringMedium {
    let h = def.thickness_m;
    let mut terms = vec![
        ScatteringTerm {
            absorption: Vec3::ZERO,
            scattering: v3(def.rayleigh_scattering),
            falloff: Falloff::Exponential { scale: def.rayleigh_scale_height_m / h },
            phase: PhaseFunction::Rayleigh,
        },
        ScatteringTerm {
            absorption: v3(def.mie_absorption),
            scattering: v3(def.mie_scattering),
            falloff: Falloff::Exponential { scale: def.mie_scale_height_m / h },
            phase: PhaseFunction::Mie { asymmetry: def.mie_asymmetry },
        },
    ];
    if let Some(o) = &def.ozone {
        terms.push(ScatteringTerm {
            absorption: v3(o.absorption),
            scattering: Vec3::ZERO,
            // Falloff parameter p = 1 at the ground, 0 at the top.
            falloff: Falloff::Tent { center: 1.0 - o.center_m / h, width: o.width_m / h },
            phase: PhaseFunction::Isotropic,
        });
    }
    ScatteringMedium::new(256, 256, terms)
}

/// Spawns one atmosphere per body that defines one.
pub fn spawn(
    commands: &mut Commands,
    media: &mut Assets<ScatteringMedium>,
    node: NodeId,
    mean_radius: f64,
    def: &AtmosphereDef,
) {
    let handle = media.add(medium(def));
    let atmosphere = Atmosphere {
        inner_radius: mean_radius as f32,
        outer_radius: mean_radius as f32 + def.thickness_m,
        ground_albedo: v3(def.ground_albedo),
        medium: handle,
    };
    commands.spawn((BodyAtmosphere(node, atmosphere.clone()), atmosphere, Transform::IDENTITY));
}

/// Keeps atmospheres at their bodies' camera-relative centres.
pub fn update(sim: Res<SimState>, rig: Res<CameraRig>, mut atmos: Query<(&BodyAtmosphere, &mut Transform)>) {
    let snap = sim.world.snapshot(sim.clock);
    for (a, mut t) in &mut atmos {
        t.translation = (snap.relative_r(a.0, rig.anchor) - rig.cam_pos).as_vec3();
    }
}

/// Tunes the camera's atmosphere settings, or turns atmospheres off.
///
/// Off is done by removing the bodies' `Atmosphere` components, not the
/// camera's `AtmosphereSettings`: Bevy 0.19's extraction only visits cameras
/// that still have settings, so removing them leaves stale render state (a
/// grey sky).
pub fn apply_settings(
    settings: Res<GraphicsSettings>,
    mut commands: Commands,
    cams: Query<Entity, With<MainCamera>>,
    atmos: Query<(Entity, &BodyAtmosphere)>,
) {
    if !settings.is_changed() {
        return;
    }
    let on = settings.atmosphere != AtmosphereQuality::Off;
    for (e, a) in &atmos {
        if on {
            commands.entity(e).insert(a.1.clone());
        } else {
            commands.entity(e).remove::<Atmosphere>();
        }
    }
    for cam in &cams {
        match settings.atmosphere {
            AtmosphereQuality::Off => {
                commands.entity(cam).insert(AtmosphereSettings::default());
                continue;
            }
            AtmosphereQuality::Lut => {
                commands.entity(cam).insert(AtmosphereSettings {
                    // Aerial perspective reaches the horizon from low altitude.
                    aerial_view_lut_max_distance: 4.0e5,
                    rendering_method: AtmosphereMode::LookupTexture,
                    ..default()
                });
            }
            AtmosphereQuality::Raymarched => {
                commands.entity(cam).insert(AtmosphereSettings {
                    aerial_view_lut_max_distance: 4.0e5,
                    sky_max_samples: 24,
                    rendering_method: AtmosphereMode::Raymarched,
                    ..default()
                });
            }
        }
    }
}

/// Sky-light strength at a height above the ground, as a fraction of the
/// atmosphere's thickness: full in the lower 60%, fading smoothly to zero at
/// the top so it does not switch off abruptly when leaving the atmosphere.
pub fn sky_light_fade(height_fraction: f64) -> f32 {
    let t = ((height_fraction - 0.6) / 0.4).clamp(0.0, 1.0);
    (1.0 - t * t * (3.0 - 2.0 * t)) as f32
}

/// Sky-light cubemap size (px per face): only diffuse and blurry reflections
/// use it, and it is regenerated and filtered every frame.
const SKY_LIGHT_SIZE: u32 = 64;

type SkyLightCamera = (
    Entity,
    Has<AtmosphereEnvironmentMapLight>,
    Option<&'static GeneratedEnvironmentMapLight>,
    Option<Mut<'static, AtmosphereSettings>>,
);

/// Sky light (ambient and reflections from Bevy's atmosphere environment
/// map) only inside an atmosphere: from above it, the map would still show
/// a lit sky and tint everything blue.
pub fn update_sky_light(
    settings: Res<GraphicsSettings>,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    mut commands: Commands,
    mut cams: Query<SkyLightCamera, With<MainCamera>>,
    atmos: Query<&BodyAtmosphere>,
    mut stash: Local<Option<GeneratedEnvironmentMapLight>>,
) {
    let snap = sim.world.snapshot(sim.clock);
    // How deep the camera is in the nearest atmosphere (fraction of its
    // thickness above the ground; above 1: outside).
    let depth = atmos
        .iter()
        .map(|a| {
            let r = (rig.cam_pos - snap.relative_r(a.0, rig.anchor)).length();
            let (lo, hi) = (f64::from(a.1.inner_radius), f64::from(a.1.outer_radius));
            (r - lo) / (hi - lo)
        })
        .fold(f64::INFINITY, f64::min);
    let inside = settings.atmosphere != AtmosphereQuality::Off && depth < 1.0;
    let fade = sky_light_fade(depth);
    for (cam, has, generated, atmo) in &mut cams {
        // Bevy adds a private cubemap component and a filtered
        // `GeneratedEnvironmentMapLight` with the sky light, and removing the
        // sky light leaves both, so the cubemap kept being filtered every
        // frame (~3 ms). We take the generated light off with it and put it
        // back later (Bevy won't recreate it while its private marker stays).
        if let Some(g) = generated.filter(|g| inside && (g.intensity - fade).abs() > 1e-3) {
            let mut g = g.clone();
            g.intensity = fade;
            commands.entity(cam).insert(g);
        }
        if inside && !has {
            commands.entity(cam).insert(AtmosphereEnvironmentMapLight {
                size: UVec2::splat(SKY_LIGHT_SIZE),
                intensity: fade,
                ..default()
            });
            if let Some(g) = stash.take() {
                commands.entity(cam).insert(g);
            }
        } else if !inside && has {
            commands.entity(cam).remove::<(AtmosphereEnvironmentMapLight, GeneratedEnvironmentMapLight)>();
            if let Some(g) = generated {
                *stash = Some(g.clone());
            }
        }
        // "Raymarched" is adaptive: inside an atmosphere the lookup tables
        // are accurate and much cheaper; from outside, raymarching is needed.
        if let Some(mut a) = atmo.filter(|_| settings.atmosphere == AtmosphereQuality::Raymarched) {
            let want = if inside { AtmosphereMode::LookupTexture } else { AtmosphereMode::Raymarched };
            if a.rendering_method as u32 != want as u32 {
                a.rendering_method = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_light_fades_out_towards_the_top() {
        // (height as a fraction of the thickness, expected strength)
        let cases = [(0.0, 1.0), (0.5, 1.0), (0.6, 1.0), (0.8, 0.5), (1.0, 0.0), (3.0, 0.0)];
        for (h, expected) in cases {
            assert!((sky_light_fade(h) - expected).abs() < 1e-6, "{h}");
        }
        assert!(sky_light_fade(0.7) > sky_light_fade(0.9));
    }
}
