//! Atmospheric scattering (A5) for every body with an atmosphere definition,
//! using Bevy's Hillaire 2020 implementation. Each atmosphere entity sits at
//! its body's centre, camera-relative, updated every frame; the camera uses
//! the nearest one. The medium is built from absolute scale heights in the
//! body's visual data.
//!
//! Note: Bevy's `ScatteringMedium::earth` swaps the Mie absorption and
//! scattering coefficients relative to Hillaire's paper (scattering 3.996e-6,
//! extinction 4.44e-6 per m); our data uses the paper's values.

use crate::body_visual::{AtmosphereDef, ProfileDef};
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
pub struct BodyAtmosphere(pub NodeId, pub Atmosphere, pub RaymarchInside);

/// The two media of an atmosphere with an opaque cloud deck
/// (`AtmosphereDef::cloud_top`): above the deck's top only the gas (the
/// deck is the `clouds` shell there), below it everything.
#[derive(Component)]
pub struct DeckMedia {
    pub above: Handle<ScatteringMedium>,
    pub full: Handle<ScatteringMedium>,
    pub top_m: f32,
}

/// Whether an atmosphere is raymarched from inside too (`AtmosphereDef::raymarch_inside`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RaymarchInside(pub bool);

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

/// Mie scattering terms for an aerosol with asymmetry `g` per channel:
/// one term when the channels share it, otherwise one term per channel
/// (Bevy's phase function has a single asymmetry), which together hold
/// the same coefficients.
pub fn mie_terms(scattering: [f32; 3], absorption: [f32; 3], falloff: Falloff, g: [f32; 3]) -> Vec<ScatteringTerm> {
    if g[1] == g[0] && g[2] == g[0] {
        return vec![ScatteringTerm {
            absorption: v3(absorption),
            scattering: v3(scattering),
            falloff,
            phase: PhaseFunction::Mie { asymmetry: g[0] },
        }];
    }
    (0..3)
        .map(|c| {
            let only = |a: [f32; 3]| Vec3::from_array(std::array::from_fn(|i| if i == c { a[i] } else { 0.0 }));
            ScatteringTerm {
                absorption: only(absorption),
                scattering: only(scattering),
                falloff: falloff.clone(),
                phase: PhaseFunction::Mie { asymmetry: g[c] },
            }
        })
        .collect()
}

/// A layer's falloff over an atmosphere `h` thick (Bevy's falloff
/// parameter is 1 at the ground and 0 at the top).
fn falloff(profile: ProfileDef, h: f32) -> Falloff {
    match profile {
        ProfileDef::Exponential { scale_height_m } => Falloff::Exponential { scale: scale_height_m / h },
        ProfileDef::Tent { center_m, width_m } => Falloff::Tent { center: 1.0 - center_m / h, width: width_m / h },
    }
}

/// Builds the scattering medium; falloff parameters are normalised by the
/// atmosphere thickness as Bevy expects.
pub fn medium(def: &AtmosphereDef) -> ScatteringMedium {
    let h = def.thickness_m;
    let mut terms = vec![ScatteringTerm {
        absorption: Vec3::ZERO,
        scattering: v3(def.rayleigh_scattering),
        falloff: Falloff::Exponential { scale: def.rayleigh_scale_height_m / h },
        phase: PhaseFunction::Rayleigh,
    }];
    let mie_profile = ProfileDef::Exponential { scale_height_m: def.mie_scale_height_m };
    let g = def.mie_asymmetry_rgb.unwrap_or([def.mie_asymmetry; 3]);
    terms.extend(mie_terms(def.mie_scattering, def.mie_absorption, falloff(mie_profile, h), g));
    if let Some(o) = &def.ozone {
        terms.push(ScatteringTerm {
            absorption: v3(o.absorption),
            scattering: Vec3::ZERO,
            falloff: falloff(ProfileDef::Tent { center_m: o.center_m, width_m: o.width_m }, h),
            phase: PhaseFunction::Isotropic,
        });
    }
    for l in &def.layers {
        terms.extend(mie_terms(l.scattering, l.absorption, falloff(l.profile, h), l.asymmetry));
    }
    ScatteringMedium::new(256, 256, terms)
}

/// The medium above an opaque cloud deck: [`medium`] without the layers
/// (they are the deck, drawn as a shell from above).
pub fn medium_above_deck(def: &AtmosphereDef) -> ScatteringMedium {
    medium(&AtmosphereDef { layers: Vec::new(), ..def.clone() })
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
        medium: handle.clone(),
    };
    let inside = RaymarchInside(def.raymarch_inside);
    let mut e = commands.spawn((BodyAtmosphere(node, atmosphere.clone(), inside), atmosphere, Transform::IDENTITY));
    if let Some(top) = def.cloud_top {
        let above = media.add(medium_above_deck(def));
        e.insert(DeckMedia { above, full: handle, top_m: top.altitude_m });
    }
}

/// Atmospheres are drawn only within this many outer radii of their body:
/// Bevy draws one per camera (the nearest), and a far one (Mars's seen from
/// Jupiter) tinted everything through f32 round-off. Earth's still covers
/// the Moon (65 radii).
pub const RANGE_RADII: f64 = 100.0;

/// The atmosphere to draw: of those within [`RANGE_RADII`] of the camera
/// (`(distance from the body's centre, outer radius)` each), the one the
/// camera is deepest in, relative to its size.
pub fn active(candidates: &[(f64, f64)]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, (d, r))| *d < RANGE_RADII * r)
        .min_by(|a, b| (a.1 .0 / a.1 .1).total_cmp(&(b.1 .0 / b.1 .1)))
        .map(|(i, _)| i)
}

type AtmosphereItem = (
    Entity,
    &'static mut BodyAtmosphere,
    &'static mut Transform,
    Option<&'static mut Atmosphere>,
    Option<&'static DeckMedia>,
);

/// Keeps atmospheres at their bodies' camera-relative centres, with their
/// ground on the reference ellipsoid below the camera, and draws only the
/// [`active`] one (none with the atmosphere setting off).
///
/// Bevy's atmosphere is a sphere. At the mean radius, ground on the
/// ellipsoid sat kilometres above or below its "ground" (2 km above at the
/// pad, 14 km below at the poles): air near the surface was too thin, and
/// ground below the sphere got no haze and no sun (dark far ground).
pub fn update(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    settings: Res<GraphicsSettings>,
    mut commands: Commands,
    mut atmos: Query<AtmosphereItem>,
    mut drawn: Local<Option<Option<Entity>>>,
) {
    let snap = sim.world.snapshot(sim.clock);
    let mut candidates = Vec::new();
    for (e, mut a, mut t, mut live, deck) in &mut atmos {
        let centre = snap.relative_r(a.0, rig.anchor);
        t.translation = (centre - rig.cam_pos).as_vec3();
        candidates.push((e, ((rig.cam_pos - centre).length(), f64::from(a.1.outer_radius))));
        let Some(p) = sim.world.source(a.0).and_then(|s| s.physical.as_ref()) else { continue };
        let up = (rig.cam_pos - centre).normalize_or_zero();
        let ground = p.ellipsoid_radius(up.dot(p.rotation.pole(sim.clock))) as f32;
        if (ground - a.1.inner_radius).abs() > 1.0 {
            let thickness = a.1.outer_radius - a.1.inner_radius;
            (a.1.inner_radius, a.1.outer_radius) = (ground, ground + thickness);
            if let Some(live) = live.as_mut() {
                (live.inner_radius, live.outer_radius) = (a.1.inner_radius, a.1.outer_radius);
            }
        }
        if let Some(d) = deck {
            // Above the deck's top, only the gas above it (the shell is the
            // deck); below it, none: the shell is the sky, and the gas fogs
            // the ground (`clouds::update`).
            let above = (rig.cam_pos - centre).length() - p.radius_eq > f64::from(d.top_m);
            if !above {
                candidates.pop();
            }
            let want = if above { &d.above } else { &d.full };
            if a.1.medium != *want {
                a.1.medium = want.clone();
                if let Some(live) = live.as_mut() {
                    live.medium = want.clone();
                }
            }
        }
    }
    let on = settings.atmosphere != AtmosphereQuality::Off;
    let sizes: Vec<(f64, f64)> = candidates.iter().map(|c| c.1).collect();
    let want = active(&sizes).filter(|_| on).map(|i| candidates[i].0);
    if *drawn != Some(want) {
        *drawn = Some(want);
        for (e, a, _, _, _) in &atmos {
            if Some(e) == want {
                commands.entity(e).insert(a.1.clone());
            } else {
                commands.entity(e).remove::<Atmosphere>();
            }
        }
    }
}

/// Tunes the camera's atmosphere settings, or turns atmospheres off.
///
/// Off is done by removing the bodies' `Atmosphere` components, not the
/// camera's `AtmosphereSettings`: Bevy 0.19's extraction only visits cameras
/// that still have settings, so removing them leaves stale render state (a
/// grey sky).
pub fn apply_settings(settings: Res<GraphicsSettings>, mut commands: Commands, cams: Query<Entity, With<MainCamera>>) {
    if !settings.is_changed() {
        return;
    }
    // Which atmosphere is drawn (if any): `update`.
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

/// Whether the camera's atmosphere is raymarched, at a camera `depth` in
/// the nearest atmosphere (height above its ground as a fraction of its
/// thickness; 1 or more: outside). "Raymarched" is adaptive: inside an
/// atmosphere the lookup tables are accurate and much cheaper (~3 ms at the
/// pad, 2026-09-26); from outside, their aerial perspective washes the
/// ground out, so it raymarches there. Hazy atmospheres (Titan's, Venus's:
/// `inside`) are raymarched inside too: their aerial-perspective table
/// broke into coloured rings on the ground (2026-10-26).
pub fn raymarch(quality: AtmosphereQuality, depth: f64, inside: RaymarchInside) -> bool {
    quality == AtmosphereQuality::Raymarched && (depth >= 1.0 || inside.0)
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
    atmos: Query<(&BodyAtmosphere, Has<Atmosphere>)>,
    mut stash: Local<Option<GeneratedEnvironmentMapLight>>,
) {
    let snap = sim.world.snapshot(sim.clock);
    // How deep the camera is in the nearest drawn atmosphere (fraction of
    // its thickness above the ground; above 1: outside). An atmosphere not
    // drawn (below an opaque deck: `update`) gives no sky light: Bevy would
    // light the ground with the last drawn atmosphere's sky.
    let (depth, hazy) = atmos
        .iter()
        .filter(|(_, drawn)| *drawn)
        .map(|(a, _)| {
            let r = (rig.cam_pos - snap.relative_r(a.0, rig.anchor)).length();
            let (lo, hi) = (f64::from(a.1.inner_radius), f64::from(a.1.outer_radius));
            ((r - lo) / (hi - lo), a.2)
        })
        .fold((f64::INFINITY, RaymarchInside(false)), |a, b| if b.0 < a.0 { b } else { a });
    let inside = settings.atmosphere != AtmosphereQuality::Off && settings.sky_light && depth < 1.0;
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
            // Bevy also derives an `EnvironmentMapLight` (the filtered sky)
            // and leaves it behind: it kept lighting everything, night sides
            // included, after leaving the atmosphere.
            commands
                .entity(cam)
                .remove::<(AtmosphereEnvironmentMapLight, GeneratedEnvironmentMapLight, EnvironmentMapLight)>();
            if let Some(g) = generated {
                *stash = Some(g.clone());
            }
        }
        // "Raymarched" is adaptive (`raymarch`), whether or not the sky light
        // is on (it was tied to it from 2026-09-27, so the tiers without sky
        // light raymarched at the pad too).
        if let Some(mut a) = atmo.filter(|_| settings.atmosphere == AtmosphereQuality::Raymarched) {
            let want = if raymarch(settings.atmosphere, depth, hazy) {
                AtmosphereMode::Raymarched
            } else {
                AtmosphereMode::LookupTexture
            };
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
    fn raymarched_only_from_outside_the_atmosphere() {
        // (quality, camera depth in the atmosphere, raymarched)
        let cases = [
            (AtmosphereQuality::Raymarched, 0.0, false), // at the pad: lookup tables
            (AtmosphereQuality::Raymarched, 0.99, false),
            (AtmosphereQuality::Raymarched, 1.0, true), // the top of the atmosphere
            (AtmosphereQuality::Raymarched, 3.0, true), // 300 km up: raymarched
            (AtmosphereQuality::Raymarched, f64::INFINITY, true), // no atmosphere near
            (AtmosphereQuality::Lut, 3.0, false),
            (AtmosphereQuality::Off, 3.0, false),
        ];
        for (q, depth, want) in cases {
            assert_eq!(raymarch(q, depth, RaymarchInside(false)), want, "{q:?} at {depth}");
        }
        // A hazy atmosphere (Titan's) is raymarched at the ground too, if raymarching at all.
        assert!(raymarch(AtmosphereQuality::Raymarched, 0.0, RaymarchInside(true)));
        assert!(!raymarch(AtmosphereQuality::Lut, 0.0, RaymarchInside(true)));
    }

    #[test]
    fn mie_terms_split_by_channel_only_when_the_asymmetry_differs() {
        let (s, a) = ([4.1e-5, 3.9e-5, 3.6e-5], [2.2e-6, 3.4e-6, 5.3e-6]);
        let one = mie_terms(s, a, Falloff::Linear, [0.7; 3]);
        assert_eq!(one.len(), 1);
        assert_eq!((one[0].scattering, one[0].absorption), (v3(s), v3(a)));
        // (asymmetry per channel): three terms, each one channel, summing
        // to the aerosol's coefficients.
        let g = [0.73, 0.75, 0.78];
        let split = mie_terms(s, a, Falloff::Linear, g);
        assert_eq!(split.len(), 3);
        let sum = |f: fn(&ScatteringTerm) -> Vec3| split.iter().map(f).fold(Vec3::ZERO, |x, y| x + y);
        assert_eq!(sum(|t| t.scattering), v3(s));
        assert_eq!(sum(|t| t.absorption), v3(a));
        for (c, t) in split.iter().enumerate() {
            assert!(matches!(t.phase, PhaseFunction::Mie { asymmetry } if asymmetry == g[c]), "{c}");
            assert_eq!(t.scattering.to_array().iter().filter(|&&v| v != 0.0).count(), 1, "{c}");
        }
    }

    #[test]
    fn layer_profiles_map_to_the_falloff_parameter() {
        // Bevy's parameter is 1 at the ground and 0 at the top.
        let h = 100_000.0;
        let tent = falloff(ProfileDef::Tent { center_m: 25_000.0, width_m: 30_000.0 }, h);
        assert!(
            matches!(tent, Falloff::Tent { center, width } if (center - 0.75).abs() < 1e-6 && (width - 0.3).abs() < 1e-6)
        );
        let exp = falloff(ProfileDef::Exponential { scale_height_m: 11_100.0 }, h);
        assert!(matches!(exp, Falloff::Exponential { scale } if (scale - 0.111).abs() < 1e-6));
        // The density at the ground is the coefficient's, and at the top nothing.
        assert!((exp.sample(1.0) - 1.0).abs() < 1e-6 && exp.sample(0.0).abs() < 1e-6);
        assert!((tent.sample(0.75) - 1.0).abs() < 1e-6 && tent.sample(0.5) == 0.0);
    }

    #[test]
    fn only_the_nearest_atmosphere_in_range_is_drawn() {
        const EARTH: f64 = 6.471e6;
        const MARS: f64 = 3.49e6;
        // (case, (distance, outer radius) per atmosphere, drawn)
        type Case<'a> = (&'a str, &'a [(f64, f64)], Option<usize>);
        let cases: [Case; 5] = [
            ("at Earth's pad", &[(6.37e6, EARTH), (7.8e10, MARS)], Some(0)),
            ("at the Moon: Earth's still", &[(3.84e8, EARTH), (7.8e10, MARS)], Some(0)),
            ("at Jupiter: none", &[(6.3e11, EARTH), (5.5e11, MARS)], None),
            ("in Mars orbit", &[(7.8e10, EARTH), (3.8e6, MARS)], Some(1)),
            ("no atmospheres", &[], None),
        ];
        for (case, c, want) in cases {
            assert_eq!(active(c), want, "{case}");
        }
        // Between two bodies, the one the camera is deeper in, by size.
        assert_eq!(active(&[(2.0 * EARTH, EARTH), (1.5 * MARS, MARS)]), Some(1));
    }

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
