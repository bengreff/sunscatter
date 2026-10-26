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

/// Fraction (0..1) of a star's disc visible from a point: the one eclipse
/// rule, owned by `sim::light` (vessel heating uses it too).
pub use sim::light::eclipse_factor;

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

/// What one body receives: its sunlight relative to the flux at `camera`,
/// the bodies that can eclipse the star somewhere on it (deepest shadow
/// first, at most [`MAX_OCCLUDERS`]), and the brightest planetshine
/// (reflector index, lux at the body's centre).
#[derive(Clone, Debug, PartialEq)]
pub struct BodyLight {
    pub sun_scale: f64,
    pub occluders: Vec<usize>,
    pub shine: Option<(usize, f64)>,
}

/// The most occluders a body's surface takes shadows from (the terrain
/// shader's array): a moon's shadow on its planet while the planet's
/// other moons, or its rings' parent, also stand near the Sun.
pub const MAX_OCCLUDERS: usize = 4;

/// Shadows hiding less of the star than this (0.1 %) are not drawn.
const MIN_SHADOW: f64 = 1e-3;

/// How deep a shadow `occ` can cast anywhere on `target`, lit by a star of
/// radius `star_r` at `star`: 0 when `target` lies wholly outside `occ`'s
/// penumbra, else the largest share of the star's disc `occ` can hide
/// there, (its angular radius / the star's)² at most 1, seen from the
/// target's point nearest it. Distant planets near the Sun's direction
/// (Mercury seen from Jupiter) hide ~10⁻⁷: they never compete with a moon.
pub fn shadow_depth(target: Sphere, occ: Sphere, star: DVec3, star_r: f64) -> f64 {
    let axis = occ.centre - star;
    let d = axis.length();
    if occ.radius <= 0.0 || d <= star_r + occ.radius {
        return 0.0;
    }
    let u = axis / d;
    let w = target.centre - occ.centre;
    let x = w.dot(u);
    if x <= -target.radius || w.length() <= target.radius + occ.radius {
        return 0.0;
    }
    // The penumbra is the cone tangent to both discs, crossing between
    // them: apex a = d r / (R + r) sunward of the occluder, half-angle β
    // with sin β = (R + r) / d. The target meets it when its centre lies
    // within its radius of the cone's surface.
    let sin_b = (star_r + occ.radius) / d;
    let cos_b = (1.0 - sin_b * sin_b).sqrt();
    let t = x + d * occ.radius / (star_r + occ.radius);
    let rho = (w - u * x).length();
    let outside = if t > 0.0 { rho * cos_b - t * sin_b } else { (w + u * (t - x)).length() };
    if outside >= target.radius {
        return 0.0;
    }
    let near = w.length() - target.radius;
    let s_ang = (star_r / (star - target.centre).length()).asin();
    let o_ang = (occ.radius / near).clamp(0.0, 1.0).asin();
    (o_ang / s_ang).powi(2).min(1.0)
}

/// The bodies that can eclipse the star on `bodies[i]`: deepest shadow
/// first (ties by index, so the choice is deterministic), at most
/// [`MAX_OCCLUDERS`], none shallower than [`MIN_SHADOW`].
pub fn occluders(i: usize, bodies: &[Sphere], star: DVec3, star_r: f64) -> Vec<usize> {
    let mut found: Vec<(usize, f64)> = (0..bodies.len())
        .filter(|&j| j != i)
        .map(|j| (j, shadow_depth(bodies[i], bodies[j], star, star_r)))
        .filter(|&(_, depth)| depth >= MIN_SHADOW)
        .collect();
    found.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    found.into_iter().take(MAX_OCCLUDERS).map(|(j, _)| j).collect()
}

/// A lit or occluding body for [`body_light`]: centre, radius, albedo.
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub centre: DVec3,
    pub radius: f64,
    pub albedo: f64,
}

/// Lighting for body `i` of `bodies`, with a star of luminous power `lm`
/// and radius `star_r` at `star` and the camera at the origin.
pub fn body_light(i: usize, bodies: &[Sphere], star: DVec3, star_r: f64, lm: f64) -> BodyLight {
    let me = bodies[i];
    let e_here = flux(lm, (star - me.centre).length());
    let sun_scale = e_here / flux(lm, star.length().max(1.0));
    let occluders = occluders(i, bodies, star, star_r);
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
    BodyLight { sun_scale, occluders, shine }
}

/// Light at a small object (a ship) at `at`: the fraction of the star's
/// disc visible past the bodies, and the brightest planetshine (lux).
pub fn object_light(at: DVec3, star: DVec3, star_r: f64, lm: f64, bodies: &[Sphere]) -> (f64, f64) {
    let occluders: Vec<(DVec3, f64)> = bodies.iter().map(|b| (b.centre, b.radius)).collect();
    let visible = eclipse_factor(at, star, star_r, &occluders);
    let shine = bodies
        .iter()
        .map(|b| {
            let e = flux(lm, (star - b.centre).length());
            planetshine(at, b.centre, b.radius, b.albedo, e, (star - b.centre).normalize())
        })
        .fold(0.0, f64::max);
    (visible, shine)
}

/// The astronomical unit (m, IAU 2012): where the base exposure applies.
pub const AU: f64 = 1.495_978_707e11;

/// How far the exposure may follow the starlight (stops from the base):
/// down to ~128 AU (past Sedna's perihelion), up to 0.25 AU.
const EXPOSURE_STOPS: (f64, f64) = (-14.0, 4.0);

/// The camera's exposure (EV100) under D087: the base exposure set for
/// the starlight at 1 AU, shifted by the stops between the stellar flux at
/// the camera (`e_cam`, inverse square, no eclipses: it is the light where
/// the photographer is, never the scene's luminance) and the flux at 1 AU
/// (`e_ref`). `follow` false: the fixed base exposure (D055).
pub fn exposure_ev100(base: f32, e_cam: f64, e_ref: f64, follow: bool) -> f32 {
    let stops = (e_cam / e_ref).log2();
    if !follow || stops.is_nan() {
        return base;
    }
    base + stops.clamp(EXPOSURE_STOPS.0, EXPOSURE_STOPS.1) as f32
}

/// Sets the camera's exposure from the starlight at the camera (D087).
pub fn update_exposure(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    settings: Res<GraphicsSettings>,
    mut cams: Query<&mut bevy::camera::Exposure, With<crate::camera::MainCamera>>,
) {
    let Some((star, lm)) = defs.light_source else { return };
    let snap = sim.world.snapshot(sim.clock);
    let d = (snap.relative_r(star, rig.anchor) - rig.cam_pos).length();
    // Under a haze, the light that reaches the camera through it.
    let haze = defs.defs.iter().find_map(|(node, def)| {
        let a = def.atmosphere.as_ref().filter(|a| a.raymarch_inside)?;
        let p = sim.world.source(*node)?.physical.as_ref()?;
        let h = (rig.cam_pos - snap.relative_r(*node, rig.anchor)).length() - p.radius_eq;
        (h < f64::from(a.thickness_m)).then(|| haze_exposure_share(&haze_above(a, p.radius_eq, h)))
    });
    let e_cam = flux(lm, d) * haze.unwrap_or(1.0);
    let ev100 = exposure_ev100(crate::camera::EV100, e_cam, flux(lm, AU), settings.starlight_exposure);
    for mut e in &mut cams {
        if (e.ev100 - ev100).abs() > 1e-4 {
            e.ev100 = ev100;
        }
    }
}

/// Sunlight under a hazy atmosphere (Titan's haze, Venus's clouds): the
/// vertical extinction optical depth per channel (the direct beam is
/// exp(−τ m(μ₀)) with the spherical-shell air mass m), the diffuse part
/// reaching the ground for an overhead Sun (a fraction of the light at the
/// top), and the radius over the extinction-weighted scale height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HazeLight {
    pub tau: [f32; 3],
    pub diffuse: [f32; 3],
    pub r_over_h: f32,
}

/// [`HazeLight`] from an atmosphere's data (one owner of the numbers: the
/// same coefficients the sky is drawn with). The diffuse part is a
/// diffusion estimate, not a radiative-transfer solution: the total
/// transmission of a slab, 1 / (1 + ¾ (1 − g) τ_s) for conservative
/// scattering (Eddington), damped by exp(−k τ) with the diffusion exponent
/// k = √(3 (1 − ω)(1 − ω g)) for absorption; minus the direct beam.
pub fn haze_light(def: &crate::body_visual::AtmosphereDef, radius: f64) -> HazeLight {
    haze_above(def, radius, 0.0)
}

/// The fraction of a profile's vertical optical depth above `h` (m).
fn share_above(profile: crate::body_visual::ProfileDef, h: f64) -> f64 {
    use crate::body_visual::ProfileDef;
    match profile {
        ProfileDef::Exponential { scale_height_m } => (-h.max(0.0) / f64::from(scale_height_m)).exp(),
        ProfileDef::Tent { center_m, width_m } => {
            let (c, w) = (f64::from(center_m), f64::from(width_m) / 2.0);
            let x = ((h - (c - w)) / w).clamp(0.0, 2.0);
            if x < 1.0 {
                1.0 - 0.5 * x * x
            } else {
                0.5 * (2.0 - x) * (2.0 - x)
            }
        }
    }
}

/// [`haze_light`] for the column above height `above_m`.
pub fn haze_above(def: &crate::body_visual::AtmosphereDef, radius: f64, above_m: f64) -> HazeLight {
    use crate::body_visual::ProfileDef;
    // Per channel: scattering and absorption optical depths, the
    // scattering-weighted asymmetry sum, and the extinction-weighted height.
    let (mut ts, mut ta, mut gs, mut th) = ([0.0f64; 3], [0.0f64; 3], [0.0f64; 3], [0.0f64; 3]);
    let mut add = |s: [f32; 3], a: [f32; 3], g: [f32; 3], profile: ProfileDef| {
        let (depth, height) = match profile {
            ProfileDef::Exponential { scale_height_m } => (f64::from(scale_height_m), f64::from(scale_height_m)),
            ProfileDef::Tent { center_m, width_m } => (f64::from(width_m) / 2.0, f64::from(center_m)),
        };
        let depth = depth * share_above(profile, above_m);
        for c in 0..3 {
            let (sc, ac) = (f64::from(s[c]) * depth, f64::from(a[c]) * depth);
            ts[c] += sc;
            ta[c] += ac;
            gs[c] += f64::from(g[c]) * sc;
            th[c] += (sc + ac) * height;
        }
    };
    let rayleigh = ProfileDef::Exponential { scale_height_m: def.rayleigh_scale_height_m };
    add(def.rayleigh_scattering, [0.0; 3], [0.0; 3], rayleigh);
    let mie = ProfileDef::Exponential { scale_height_m: def.mie_scale_height_m };
    add(def.mie_scattering, def.mie_absorption, def.mie_asymmetry_rgb.unwrap_or([def.mie_asymmetry; 3]), mie);
    if let Some(o) = &def.ozone {
        add([0.0; 3], o.absorption, [0.0; 3], ProfileDef::Tent { center_m: o.center_m, width_m: o.width_m });
    }
    for l in &def.layers {
        add(l.scattering, l.absorption, l.asymmetry, l.profile);
    }
    let mut out = HazeLight { tau: [0.0; 3], diffuse: [0.0; 3], r_over_h: 0.0 };
    let (mut tau_sum, mut h_sum) = (0.0, 0.0);
    for c in 0..3 {
        let tau = ts[c] + ta[c];
        let (omega, g) = if tau > 0.0 { (ts[c] / tau, gs[c] / ts[c].max(1e-30)) } else { (1.0, 0.0) };
        let k = (3.0 * (1.0 - omega) * (1.0 - omega * g)).max(0.0).sqrt();
        let total = (-k * tau).exp() / (1.0 + 0.75 * (1.0 - g) * ts[c]);
        out.tau[c] = tau as f32;
        out.diffuse[c] = (total - (-tau).exp()).clamp(0.0, 1.0) as f32;
        tau_sum += tau;
        h_sum += th[c];
    }
    out.r_over_h = if h_sum > 0.0 { (radius * tau_sum / h_sum) as f32 } else { 0.0 };
    out
}

/// The share of the starlight that reaches a camera under a haze, for the
/// exposure (D087: the flux at the camera): the luminance-weighted (Rec.
/// 709) direct and diffuse transmission of the column above it, for an
/// overhead Sun (so a night side stays dark: only the haze, not the Sun's
/// height, sets it).
pub fn haze_exposure_share(h: &HazeLight) -> f64 {
    let w = [0.2126, 0.7152, 0.0722];
    (0..3).map(|c| w[c] * ((-f64::from(h.tau[c])).exp() + f64::from(h.diffuse[c]))).sum()
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
    let sky = crate::sky::sky_ambient(&sim, &rig, &defs, &settings);
    for (i, (node, handle)) in terrain.materials().enumerate() {
        let l = body_light(i, &spheres, star, star_r, lm);
        let Some(mut mat) = materials.get_mut(handle) else { continue };
        let p = &mut mat.extension.params;
        p.star = star.as_vec3().extend(star_r as f32);
        p.occluders = [Vec4::ZERO; MAX_OCCLUDERS];
        for (slot, &j) in p.occluders.iter_mut().zip(&l.occluders) {
            *slot = spheres[j].centre.as_vec3().extend(spheres[j].radius as f32);
        }
        p.shine = match l.shine.filter(|_| settings.earthshine) {
            Some((j, e)) => spheres[j].centre.as_vec3().extend(e as f32),
            None => Vec4::ZERO,
        };
        let tint = l.shine.and_then(|(j, _)| defs.get(nodes[j])).map_or([1.0; 3], |d| d.base_color);
        p.light = Vec4::new(tint[0], tint[1], tint[2], l.sun_scale as f32);
        p.ambient.x = crate::sky::ambient_share(node, sky);
        // Under a haze, the diffuse light scales with the Sun's flux (lux at the camera; `light.w` scales it to the body).
        if p.haze.w > 0.0 {
            p.haze_diffuse.w = flux(lm, star.length().max(1.0)) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let lm_moon = body_light(1, &bodies, sun, SUN_R, lm);
        let lm_earth = body_light(0, &bodies, sun, SUN_R, lm);
        // Flux at each body's own distance, relative to the camera's.
        assert!((lm_moon.sun_scale - (AU / (AU + 2.0e6)).powi(2)).abs() < 1e-9);
        assert!(lm_earth.sun_scale > lm_moon.sun_scale);
        // Earth sits between the Moon and the Sun: the Moon's occluder.
        assert_eq!(lm_moon.occluders, vec![0]);
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
        let (_, moved_shine) = body_light(0, &moved, sun + shift, SUN_R, lm).shine.expect("moonshine");
        assert!((moved_shine - moonshine).abs() < 1e-9 * moonshine);
    }

    #[test]
    fn exposure_follows_the_starlight_at_the_camera() {
        let sun = luminous_power(3.828e26, 93.0);
        let e_ref = flux(sun, AU);
        let base = 14.5;
        // (case, camera distance from the Sun in AU, follow, expected EV100)
        let cases = [
            ("Earth: unchanged", 1.0, true, 14.5),
            ("near Earth's orbit: within a hundredth of a stop", 1.0067, true, 14.48),
            ("Jupiter at 5.2 AU: 1/27 of the light, 4.76 stops more", 5.2, true, 14.5 - 4.757),
            ("Saturn at 9.5 AU: ~1 % of the light", 9.5, true, 14.5 - 6.496),
            ("Neptune at 30 AU", 30.0, true, 14.5 - 9.814),
            ("Mercury at 0.387 AU: 2.7 stops less", 0.387, true, 14.5 + 2.740),
            ("past 128 AU: the darkest exposure", 500.0, true, 14.5 - 14.0),
            ("grazing the Sun: the brightest", 0.01, true, 14.5 + 4.0),
            ("off: fixed at Saturn", 9.5, false, 14.5),
        ];
        for (case, au, follow, want) in cases {
            let ev = exposure_ev100(base, flux(sun, au * AU), e_ref, follow);
            assert!((f64::from(ev) - want).abs() < 0.01, "{case}: {ev}");
        }
        // A day side keeps its brightness on screen (lux x 2^-EV), whatever
        // the distance; a night side stays as many stops below it.
        let screen = |au: f64| {
            let e = flux(sun, au * AU);
            e * 2f64.powf(-f64::from(exposure_ev100(base, e, e_ref, true)))
        };
        assert!((screen(9.5) / screen(1.0) - 1.0).abs() < 1e-5);
        // The camera's flux is a distance, never the scene: no light, no change.
        assert_eq!(exposure_ev100(base, 0.0, e_ref, true), base - 14.0);
        assert_eq!(exposure_ev100(base, f64::NAN, e_ref, true), base);
    }

    #[test]
    fn haze_light_from_the_atmosphere_data() {
        use crate::body_visual::AtmosphereDef;
        let load = |name: &str| crate::body_visual::load(name).and_then(|d| d.atmosphere).expect(name);
        let clear = AtmosphereDef { mie_scattering: [0.0; 3], mie_absorption: [0.0; 3], ..load("Titan") };
        let none = AtmosphereDef { rayleigh_scattering: [0.0; 3], layers: vec![], ..clear.clone() };
        // (case, atmosphere, vertical optical depth range, diffuse range at 550 nm)
        let cases = [
            ("no air: nothing scattered", none, (0.0, 1e-9), (0.0, 1e-9)),
            ("Titan's N2 alone (Rayleigh, conservative)", clear, (1.0, 1.3), (0.15, 0.35)),
            ("Titan with its haze: a few per cent arrive diffusely", load("Titan"), (4.0, 5.5), (0.02, 0.12)),
            ("Venus under 30 optical depths of cloud: under 1 %", load("Venus"), (35.0, 60.0), (1e-4, 0.01)),
        ];
        for (case, def, tau, diffuse) in cases {
            let h = haze_light(&def, 2.5e6);
            let (t, d) = (f64::from(h.tau[1]), f64::from(h.diffuse[1]));
            assert!(t >= tau.0 && t <= tau.1, "{case}: tau {t}");
            assert!(d >= diffuse.0 && d <= diffuse.1, "{case}: diffuse {d}");
        }
        // Blue is the most extinguished under Titan's haze, and the least transmitted.
        let titan = haze_light(&load("Titan"), 2.575e6);
        assert!(titan.tau[2] > titan.tau[1] && titan.tau[1] > titan.tau[0]);
        assert!(titan.diffuse[2] < titan.diffuse[0]);
        assert!(titan.r_over_h > 10.0 && titan.r_over_h < 200.0, "{}", titan.r_over_h);
    }

    #[test]
    fn the_exposure_follows_the_light_under_a_haze() {
        let load = |name: &str| crate::body_visual::load(name).and_then(|d| d.atmosphere).expect(name);
        let venus = load("Venus");
        let share = |h: f64| haze_exposure_share(&haze_above(&venus, 6.05e6, h));
        // (case, height above the mean radius, expected share of the light at the top)
        let cases = [
            ("on Venus's ground: about 1 %, 6-7 stops down", 0.0, (0.005, 0.03)),
            ("at the deck's top (tau ~ 1 of cloud): most of it", 74_000.0, (0.4, 0.9)),
            ("above the atmosphere: all of it", 2.0e5, (0.999, 1.0)),
        ];
        for (case, h, (lo, hi)) in cases {
            let s = share(h);
            assert!(s >= lo && s <= hi, "{case}: {s}");
        }
        // The light grows monotonically upwards.
        let mut last = 0.0;
        for i in 0..=40 {
            let s = share(f64::from(i) * 3000.0);
            assert!(s >= last, "at {} km", i * 3);
            last = s;
        }
        // A tent profile's share above a height: all below it, half at its centre, none above.
        use crate::body_visual::ProfileDef::Tent;
        let tent = Tent { center_m: 57_000.0, width_m: 46_000.0 };
        for (h, want) in
            [(0.0, 1.0), (34_000.0, 1.0), (57_000.0, 0.5), (74_000.0, 0.5 * (6.0f64 / 23.0).powi(2)), (90_000.0, 0.0)]
        {
            assert!((share_above(tent, h) - want).abs() < 1e-9, "{h}");
        }
    }

    const JUP_R: f64 = 7.1492e7;
    const IO_R: f64 = 1.8216e6;
    const IO_A: f64 = 4.217e8;

    fn sphere(x: f64, y: f64, radius: f64) -> Sphere {
        Sphere { centre: DVec3::new(x, y, 0.0), radius, albedo: 0.5 }
    }

    #[test]
    fn a_moon_shadows_its_planet_and_far_planets_do_not() {
        // Jupiter at the origin, the Sun 5.2 AU along −x; moons sunward of
        // Jupiter at Io's distance, their shadow axis `off` Jupiter radii
        // from its centre. (case, bodies, target, expected occluders)
        let sun = DVec3::new(-5.2 * AU, 0.0, 0.0);
        let jup = sphere(0.0, 0.0, JUP_R);
        let io = |off: f64| sphere(-IO_A, off * JUP_R, IO_R);
        let mercury = sphere(-4.81 * AU, 0.0, 2.44e6);
        let cases: Vec<(&str, Vec<Sphere>, usize, Vec<usize>)> = vec![
            ("Io's transit (0.33 radii off the centre)", vec![jup, io(0.33)], 0, vec![1]),
            // The old rule took the body nearest the Sun's direction: Mercury.
            ("Mercury in line hides ~1e-5: not drawn", vec![jup, mercury, io(0.33)], 0, vec![2]),
            ("the penumbra grazes the limb", vec![jup, io(1.02)], 0, vec![1]),
            ("the shadow misses the limb", vec![jup, io(1.05)], 0, vec![]),
            ("Io beside Jupiter, not in line", vec![jup, sphere(0.0, IO_A, IO_R)], 0, vec![]),
            ("Io beyond Jupiter: no shadow on Jupiter", vec![jup, sphere(IO_A, 0.0, IO_R)], 0, vec![]),
            ("Io beyond Jupiter: in its shadow", vec![jup, sphere(IO_A, 0.0, IO_R)], 1, vec![0]),
            ("Io sunward: Jupiter does not shadow it", vec![jup, io(0.0)], 1, vec![]),
        ];
        for (case, bodies, target, want) in cases {
            assert_eq!(occluders(target, &bodies, sun, SUN_R), want, "{case}");
        }
    }

    #[test]
    fn the_deepest_shadows_are_kept_in_a_fixed_order() {
        // Six small moons in transit at once, radii 20 to 120 km (each
        // hides less than the Sun's disc): the four largest, largest first;
        // two equal ones by index.
        let sun = DVec3::new(-5.2 * AU, 0.0, 0.0);
        let mut bodies = vec![sphere(0.0, 0.0, JUP_R)];
        for (k, r) in [20e3, 120e3, 60e3, 100e3, 40e3, 100e3].into_iter().enumerate() {
            bodies.push(sphere(-IO_A, (k as f64 * 0.1 - 0.25) * JUP_R, r));
        }
        assert_eq!(occluders(0, &bodies, sun, SUN_R), vec![2, 4, 6, 3]);
        // Moving the camera (every centre shifts) changes nothing.
        let shift = DVec3::new(3e9, -2e8, 7e8);
        let moved: Vec<Sphere> = bodies.iter().map(|b| Sphere { centre: b.centre + shift, ..*b }).collect();
        assert_eq!(occluders(0, &moved, sun + shift, SUN_R), vec![2, 4, 6, 3]);
    }

    #[test]
    fn every_visible_eclipse_on_the_surface_is_selected() {
        // Wherever `eclipse_factor` dims a point of Jupiter's sunlit
        // hemisphere by more than the threshold, the moon is an occluder:
        // the selection never drops a shadow the eclipse rule would draw.
        let sun = DVec3::new(-5.2 * AU, 0.0, 0.0);
        let jup = sphere(0.0, 0.0, JUP_R);
        for off in [0.0, 0.5, 0.9, 1.0, 1.01, 1.02, 1.03, 1.04, 1.1] {
            let bodies = [jup, sphere(-IO_A, off * JUP_R, IO_R)];
            let chosen = occluders(0, &bodies, sun, SUN_R) == vec![1];
            let occ = [(bodies[1].centre, IO_R)];
            let dimmed = (0..=400).any(|k| {
                let y = (off - 0.1 + 0.2 * f64::from(k) / 400.0) * JUP_R;
                let x = -(JUP_R * JUP_R - y * y).max(0.0).sqrt();
                y.abs() <= JUP_R && eclipse_factor(DVec3::new(x, y, 0.0), sun, SUN_R, &occ) < 1.0 - MIN_SHADOW
            });
            assert!(chosen || !dimmed, "offset {off}: a visible shadow was dropped");
            assert!(dimmed || !chosen || off > 1.0, "offset {off}: chosen without a shadow");
        }
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
