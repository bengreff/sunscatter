//! Physical lighting per object (D055): the illuminance each lit object
//! receives from a star at *its own* position (not the camera's), the part
//! of the star's disc that other bodies hide (eclipses), and planetshine
//! (sunlight reflected by a nearby body, Lambert sphere with phase angle).
//! Pure functions; the terrain shader and scene systems apply them.
//!
//! Units: luminous power in lumens, illuminance in lux, distances in metres.

use crate::camera::CameraRig;
use crate::scene::BodyDefs;
use crate::settings::GraphicsSettings;
use crate::state::SimState;
use crate::terrain::{Terrain, TerrainMaterial};
use bevy::prelude::*;
use glam::DVec3;
use sim::frame::NodeId;
use std::f64::consts::PI;

/// A star's luminous power (lm) from its luminosity (W) and luminous
/// efficacy (lm/W).
pub fn luminous_power(luminosity_w: f64, efficacy_lm_per_w: f64) -> f64 {
    luminosity_w * efficacy_lm_per_w
}

/// Illuminance (lux) at distance `d` from a source of luminous power `lm`.
pub fn flux(lm: f64, d: f64) -> f64 {
    lm / (4.0 * PI * d * d)
}

/// Area of overlap of two discs of radii `r1`, `r2` whose centres are `d`
/// apart (any consistent unit, here angles in radians).
fn disc_overlap(r1: f64, r2: f64, d: f64) -> f64 {
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= (r1 - r2).abs() {
        let r = r1.min(r2);
        return PI * r * r;
    }
    let a1 = ((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1)).clamp(-1.0, 1.0).acos();
    let a2 = ((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2)).clamp(-1.0, 1.0).acos();
    r1 * r1 * (a1 - a1.sin() * a1.cos()) + r2 * r2 * (a2 - a2.sin() * a2.cos())
}

/// Fraction (0..1) of a star's disc visible from `at`, with the star at
/// `star` (radius `star_r`) and sphere occluders `(centre, radius)`. Discs
/// are compared as angles on the sky; overlapping occluders are not
/// double-counted beyond hiding the whole disc.
pub fn eclipse_factor(at: DVec3, star: DVec3, star_r: f64, occluders: &[(DVec3, f64)]) -> f64 {
    let to_star = star - at;
    let ds = to_star.length();
    let s_ang = (star_r / ds).clamp(0.0, 1.0).asin();
    let s_area = PI * s_ang * s_ang;
    let mut hidden = 0.0;
    for &(c, r) in occluders {
        let to_c = c - at;
        let dc = to_c.length();
        // Behind us, farther than the star, or containing the point: skip
        // (a point inside a body is its surface; the body itself is lit by
        // the shader's Lambert term, not eclipsed).
        if dc <= r || dc >= ds || to_c.dot(to_star) <= 0.0 {
            continue;
        }
        let o_ang = (r / dc).clamp(0.0, 1.0).asin();
        let sep = to_c.normalize().dot(to_star / ds).clamp(-1.0, 1.0).acos();
        hidden += disc_overlap(s_ang, o_ang, sep);
    }
    (1.0 - hidden / s_area).clamp(0.0, 1.0)
}

/// Lambert-sphere phase function: reflected light at phase angle `alpha`
/// relative to full phase (1 at alpha = 0, 0 at alpha = π).
pub fn lambert_phase(alpha: f64) -> f64 {
    (alpha.sin() + (PI - alpha) * alpha.cos()) / PI
}

/// Planetshine: illuminance (lux) at `at` from a Lambert sphere at `body`
/// (radius `r`, geometric albedo `albedo`) lit by `e_star` lux from the
/// direction `to_star` (unit, from the body). Zero inside the body.
pub fn planetshine(at: DVec3, body: DVec3, r: f64, albedo: f64, e_star: f64, to_star: DVec3) -> f64 {
    let to_obs = at - body;
    let d = to_obs.length();
    if d <= r {
        return 0.0;
    }
    let alpha = (to_obs / d).dot(to_star).clamp(-1.0, 1.0).acos();
    // A Lambert sphere reflects 2/3 of albedo × incident flux towards full
    // phase, spread with 1/d² from the sphere's cross-section.
    e_star * albedo * 2.0 / 3.0 * (r / d).powi(2) * lambert_phase(alpha)
}

/// Eye adaptation (D055). At the fixed exposure (EV100 14.5) a sunlit
/// scene's mean exposed luminance, as Bevy's auto exposure meters it, is
/// about 2^DAYLIGHT_LOG_LUM (provisional: calibrated by eye on the pad view;
/// -2.2 from a hand estimate overexposed everything by ~4 stops). Auto exposure
/// keeps daylight scenes there (no change) and brightens darker ones by at
/// most MAX_BRIGHTEN_STOPS, slowly: enough to show Earthshine on the Moon's
/// night side (~7 lux, ~14 stops below sunlight) in a close-up, while a
/// sunlit scene keeps night sides near-black.
pub const DAYLIGHT_LOG_LUM: f32 = -6.0;
pub const MAX_BRIGHTEN_STOPS: f32 = 15.0;
/// Adaptation speeds (stops per second): slow to brighten, quicker back.
pub const BRIGHTEN_SPEED: f32 = 1.0;
pub const DARKEN_SPEED: f32 = 3.0;

/// The metering range (log2 exposed luminance) given to auto exposure.
pub fn metering_range() -> std::ops::RangeInclusive<f32> {
    (DAYLIGHT_LOG_LUM - MAX_BRIGHTEN_STOPS)..=DAYLIGHT_LOG_LUM
}

/// What one body receives: its sunlight relative to the flux at `camera`,
/// the body most likely to eclipse the star there, and the brightest
/// planetshine (reflector index, lux at the body's centre).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyLight {
    pub sun_scale: f64,
    pub occluder: Option<usize>,
    pub shine: Option<(usize, f64)>,
}

/// A lit or occluding body for [`body_light`]: centre, radius, albedo.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub centre: DVec3,
    pub radius: f64,
    pub albedo: f64,
}

/// Lighting for body `i` of `bodies`, with a star of luminous power `lm` at
/// `star` and the camera at the origin.
pub fn body_light(i: usize, bodies: &[Sphere], star: DVec3, lm: f64) -> BodyLight {
    let me = bodies[i];
    let to_star = star - me.centre;
    let e_here = flux(lm, to_star.length());
    let sun_scale = e_here / flux(lm, star.length().max(1.0));
    // The occluder: the other body closest to the star's direction, between
    // this body and the star.
    let occluder = (0..bodies.len())
        .filter(|&j| j != i)
        .filter(|&j| (bodies[j].centre - me.centre).dot(to_star) > 0.0)
        .map(|j| {
            let to_j = bodies[j].centre - me.centre;
            let sep = to_j.normalize().dot(to_star.normalize()).clamp(-1.0, 1.0).acos();
            (j, sep - (bodies[j].radius / to_j.length()).clamp(0.0, 1.0).asin())
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(j, _)| j);
    let shine = (0..bodies.len())
        .filter(|&j| j != i)
        .map(|j| {
            let p = bodies[j];
            let e_p = flux(lm, (star - p.centre).length());
            let dir = (star - p.centre).normalize();
            (j, planetshine(me.centre, p.centre, p.radius, p.albedo, e_p, dir))
        })
        // Below starlight (~0.001 lux) it would not show.
        .filter(|(_, e)| *e > 1e-4)
        .max_by(|a, b| a.1.total_cmp(&b.1));
    BodyLight { sun_scale, occluder, shine }
}

/// Fills each terrain body's lighting uniforms from [`body_light`].
pub fn update_terrain(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    settings: Res<GraphicsSettings>,
    terrain: Res<Terrain>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let Some((star_node, lm)) = defs.light_source else { return };
    let snap = sim.world.snapshot(sim.clock);
    let rel = |n: NodeId| snap.relative_r(n, rig.anchor) - rig.cam_pos;
    let radius = |n: NodeId| sim.world.source(n).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
    let star = rel(star_node);
    let star_r = radius(star_node);
    let nodes: Vec<NodeId> = terrain.materials().map(|(n, _)| n).collect();
    let spheres: Vec<Sphere> = nodes
        .iter()
        .map(|&n| Sphere {
            centre: rel(n),
            radius: radius(n),
            albedo: defs.get(n).map_or(0.3, |d| f64::from(d.albedo)),
        })
        .collect();
    for (i, (_, handle)) in terrain.materials().enumerate() {
        let l = body_light(i, &spheres, star, lm);
        let Some(mut mat) = materials.get_mut(handle) else { continue };
        let p = &mut mat.extension.params;
        p.star = star.as_vec3().extend(star_r as f32);
        p.occluder = l.occluder.map_or(Vec4::ZERO, |j| spheres[j].centre.as_vec3().extend(spheres[j].radius as f32));
        p.shine = match l.shine.filter(|_| settings.earthshine) {
            Some((j, e)) => spheres[j].centre.as_vec3().extend(e as f32),
            None => Vec4::ZERO,
        };
        let tint = l.shine.and_then(|(j, _)| defs.get(nodes[j])).map_or([1.0; 3], |d| d.base_color);
        p.light = Vec4::new(tint[0], tint[1], tint[2], l.sun_scale as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AU: f64 = 1.495_978_707e11;
    const SUN_R: f64 = 6.957e8;
    const EARTH_R: f64 = 6.371e6;

    #[test]
    fn sunlight_at_one_au_and_inverse_square() {
        let sun = luminous_power(3.828e26, 93.0);
        let e1 = flux(sun, AU);
        assert!((e1 - 126_600.0).abs() < 500.0, "{e1}");
        assert!((flux(sun, 2.0 * AU) - e1 / 4.0).abs() < 1e-6 * e1);
        assert!((flux(sun, 5.2 * AU) * 5.2 * 5.2 - e1).abs() < 1e-6 * e1);
    }

    #[test]
    fn eclipse_table() {
        let sun = DVec3::new(-AU, 0.0, 0.0);
        let earth = DVec3::ZERO;
        let occ = [(earth, EARTH_R)];
        // (case, point, expected visible fraction, tolerance)
        let cases = [
            ("behind Earth's centre, in the umbra", DVec3::new(2.0 * EARTH_R, 0.0, 0.0), 0.0, 1e-9),
            ("Moon distance on the axis: umbra", DVec3::new(3.84e8, 0.0, 0.0), 0.0, 1e-9),
            ("well outside the shadow", DVec3::new(3.84e8, 2.0e7, 0.0), 1.0, 1e-9),
            ("on the day side", DVec3::new(-2.0 * EARTH_R, 0.0, 0.0), 1.0, 1e-9),
            ("far off to the side", DVec3::new(0.0, 1.0e9, 0.0), 1.0, 1e-9),
        ];
        for (case, p, expected, tol) in cases {
            let f = eclipse_factor(p, sun, SUN_R, &occ);
            assert!((f - expected).abs() <= tol, "{case}: {f}");
        }
        // In the penumbra the factor is strictly between 0 and 1, and grows
        // outwards.
        let edge = |y: f64| eclipse_factor(DVec3::new(3.84e8, y, 0.0), sun, SUN_R, &occ);
        let (a, b) = (edge(6.0e6), edge(7.5e6));
        assert!(a > 0.0 && a < b && b < 1.0, "{a} {b}");
    }

    #[test]
    fn each_body_gets_its_own_flux_occluder_and_shine() {
        let sun = DVec3::new(-AU, 0.0, 0.0);
        let lm = luminous_power(3.828e26, 93.0);
        // Camera near the Moon, Moon behind Earth (full Moon, eclipse side).
        let earth = Sphere { centre: DVec3::new(-3.84e8, 0.0, 0.0), radius: EARTH_R, albedo: 0.3 };
        let moon = Sphere { centre: DVec3::new(0.0, 0.0, 0.0) + DVec3::X * 2.0e6, radius: 1.737e6, albedo: 0.12 };
        let bodies = [earth, moon];
        let lm_moon = body_light(1, &bodies, sun, lm);
        let lm_earth = body_light(0, &bodies, sun, lm);
        // Flux at each body's own distance, relative to the camera's.
        assert!((lm_moon.sun_scale - (AU / (AU + 2.0e6)).powi(2)).abs() < 1e-9);
        assert!(lm_earth.sun_scale > lm_moon.sun_scale);
        // Earth sits between the Moon and the Sun: the Moon's occluder.
        assert_eq!(lm_moon.occluder, Some(0));
        // Full Moon: from the Moon, Earth is new (no Earthshine), and the
        // full Moon lights Earth's night side.
        assert_eq!(lm_moon.shine, None);
        let (from, moonshine) = lm_earth.shine.expect("moonshine");
        assert_eq!(from, 1);
        assert!(moonshine > 0.05 && moonshine < 0.5, "full moonlight ~0.2 lux: {moonshine}");
        // It is computed at the body, not the camera: moving the camera
        // (everything shifts) leaves it unchanged.
        let shift = DVec3::new(1e7, 3e7, 0.0);
        let moved: Vec<Sphere> = bodies.iter().map(|b| Sphere { centre: b.centre + shift, ..*b }).collect();
        let (_, moved_shine) = body_light(0, &moved, sun + shift, lm).shine.expect("moonshine");
        assert!((moved_shine - moonshine).abs() < 1e-9 * moonshine);
    }

    /// The correction (stops) auto exposure settles at for a scene whose mean
    /// exposed log luminance is `mean` (Bevy's rule: compensation − clamped
    /// mean, with our flat compensation at DAYLIGHT_LOG_LUM).
    fn adapted_stops(mean: f32) -> f32 {
        let r = metering_range();
        DAYLIGHT_LOG_LUM - mean.clamp(*r.start(), *r.end())
    }

    #[test]
    fn eye_adaptation_leaves_daylight_alone_and_brightens_dark_scenes_within_limits() {
        // Mean exposed luminance of the scene, in stops relative to daylight.
        let stops = |rel: f32| adapted_stops(DAYLIGHT_LOG_LUM + rel);
        // (scene relative to daylight, expected correction)
        let cases = [(0.0, 0.0), (3.0, 0.0), (-4.0, 4.0), (-14.0, 14.0), (-30.0, 15.0)];
        for (rel, expected) in cases {
            assert!((stops(rel) - expected).abs() < 1e-5, "{rel}: {}", stops(rel));
        }
        // Earthshine on the Moon (7 lux vs ~128,000 lux sunlight) is within reach.
        let earthshine = (7.0f32 / 128_000.0).log2();
        assert!(earthshine > -MAX_BRIGHTEN_STOPS, "{earthshine}");
    }

    #[test]
    fn phase_function_peaks_at_full_phase() {
        assert!((lambert_phase(0.0) - 1.0).abs() < 1e-12);
        assert!(lambert_phase(PI).abs() < 1e-12);
        assert!(lambert_phase(0.5) > lambert_phase(1.0) && lambert_phase(1.0) > lambert_phase(2.0));
    }

    #[test]
    fn earthshine_on_the_moon_is_full_earth_at_new_moon() {
        let sun_dir = DVec3::new(-1.0, 0.0, 0.0);
        let e_sun = 128_000.0;
        // New Moon: the Moon between Earth and Sun sees a full Earth.
        let new_moon = planetshine(DVec3::new(-3.84e8, 0.0, 0.0), DVec3::ZERO, EARTH_R, 0.3, e_sun, sun_dir);
        let full_moon = planetshine(DVec3::new(3.84e8, 0.0, 0.0), DVec3::ZERO, EARTH_R, 0.3, e_sun, sun_dir);
        let quarter = planetshine(DVec3::new(0.0, 3.84e8, 0.0), DVec3::ZERO, EARTH_R, 0.3, e_sun, sun_dir);
        assert!(new_moon > quarter && quarter > full_moon && full_moon.abs() < 1e-9);
        // About 7 lux at new Moon: ~10,000x weaker than sunlight.
        assert!(new_moon > 3.0 && new_moon < 15.0, "{new_moon}");
        assert_eq!(planetshine(DVec3::new(1.0e6, 0.0, 0.0), DVec3::ZERO, EARTH_R, 0.3, e_sun, sun_dir), 0.0);
    }
}
