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
use bevy::light::Atmosphere;
use bevy::pbr::{AtmosphereMode, AtmosphereSettings};
use bevy::prelude::*;
use sim::frame::NodeId;

/// An atmosphere entity belonging to a body.
#[derive(Component)]
pub struct BodyAtmosphere(pub NodeId);

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
    commands.spawn((
        BodyAtmosphere(node),
        Atmosphere {
            inner_radius: mean_radius as f32,
            outer_radius: mean_radius as f32 + def.thickness_m,
            ground_albedo: v3(def.ground_albedo),
            medium: handle,
        },
        Transform::IDENTITY,
    ));
}

/// Keeps atmospheres at their bodies' camera-relative centres.
pub fn update(sim: Res<SimState>, rig: Res<CameraRig>, mut atmos: Query<(&BodyAtmosphere, &mut Transform)>) {
    let snap = sim.world.snapshot(sim.clock);
    for (a, mut t) in &mut atmos {
        t.translation = (snap.relative_r(a.0, rig.anchor) - rig.cam_pos).as_vec3();
    }
}

/// Adds, tunes or removes the camera's atmosphere settings.
pub fn apply_settings(settings: Res<GraphicsSettings>, mut commands: Commands, cams: Query<Entity, With<MainCamera>>) {
    if !settings.is_changed() {
        return;
    }
    for cam in &cams {
        match settings.atmosphere {
            AtmosphereQuality::Off => {
                commands.entity(cam).remove::<AtmosphereSettings>();
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
