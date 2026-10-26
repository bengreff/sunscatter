//! The lighting rules (D055), pure: any number of stars, each lighting
//! every object at the object's own position, with its own eclipses and
//! colour, and planetshine from every reflecting body. The renderer draws a
//! bounded set of lights; [`star_slots`] picks it and the rest of the
//! starlight becomes an ambient term, so nothing is dropped silently.
//!
//! Units: luminous power in lumens, illuminance in lux, distances in metres.
//! Positions are camera-relative (the camera at the origin).

use glam::DVec3;
use std::f64::consts::PI;

/// Stars drawn as lights (Bevy directional lights, the shaders' arrays):
/// the brightest at the camera. A fifth star's light is ambient.
pub const MAX_STARS: usize = 4;

/// The most bodies whose shadows a body's surface takes (the shaders'
/// array): a moon's shadow on its planet while other moons stand near the
/// line to a star, or a planet shadowing its moon from two stars.
pub const MAX_OCCLUDERS: usize = 4;

/// The most bodies whose planetshine lights a body (the brightest).
pub const MAX_SHINES: usize = 2;

/// Shadows hiding less than this share of a body's starlight are not drawn.
pub(super) const MIN_SHADOW: f64 = 1e-3;

/// Planetshine below this (lux) would not show (starlight is ~0.001 lux).
const MIN_SHINE: f64 = 1e-4;

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
/// (radius `r`, Lambert albedo `albedo`: its geometric albedo is 2/3 of it)
/// lit by `e_star` lux from the direction `to_star` (unit, from the body).
/// Zero inside the body.
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

/// A light source: centre, radius, luminous power, and the colour of its
/// light relative to the Sun's (`color::star_tint`; Rec. 709 luminance 1).
#[derive(Clone, Copy, Debug)]
pub struct Star {
    pub centre: DVec3,
    pub radius: f64,
    pub lm: f64,
    pub tint: [f64; 3],
}

impl Star {
    /// Its illuminance (lux) at `at`; 0 inside the star.
    pub fn lux_at(&self, at: DVec3) -> f64 {
        let d = (self.centre - at).length();
        if d <= self.radius.max(1.0) {
            0.0
        } else {
            flux(self.lm, d)
        }
    }
}

/// A lit, occluding and reflecting body: centre, radius, Lambert albedo,
/// and the colour of the light it reflects (Rec. 709 luminance 1).
#[derive(Clone, Copy, Debug)]
pub struct Sphere {
    pub centre: DVec3,
    pub radius: f64,
    pub albedo: f64,
    pub tint: [f64; 3],
}

/// Rec. 709 luminance of a linear colour.
pub fn luminance(c: [f64; 3]) -> f64 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// The stars drawn as lights, seen from `at` (the camera): the brightest
/// first, ties by index (deterministic), at most [`MAX_STARS`], none that
/// sends no light there.
pub fn star_slots(stars: &[Star], at: DVec3) -> Vec<usize> {
    let mut lit: Vec<(usize, f64)> =
        stars.iter().enumerate().map(|(k, s)| (k, s.lux_at(at))).filter(|&(_, e)| e > 0.0).collect();
    lit.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    lit.into_iter().take(MAX_STARS).map(|(k, _)| k).collect()
}

/// How deep a shadow `occ` can cast anywhere on `target` from a star of
/// radius `star_r` at `star`: 0 when `target` lies wholly outside `occ`'s
/// penumbra, else the largest share of the star's disc `occ` can hide
/// there, (its angular radius / the star's)² at most 1, seen from the
/// target's point nearest it. Distant planets near a star's direction
/// (Mercury seen from Jupiter) hide ~10⁻⁵: they never compete with a moon.
pub fn shadow_depth(target: Sphere, occ: Sphere, star: DVec3, star_r: f64) -> f64 {
    let axis = occ.centre - star;
    let d = axis.length();
    if occ.radius <= 0.0 || star_r <= 0.0 || d <= star_r + occ.radius {
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
    // within its radius of the cone's surface (or of the apex, behind it).
    let sin_b = (star_r + occ.radius) / d;
    let cos_b = (1.0 - sin_b * sin_b).sqrt();
    let t = x + d * occ.radius / (star_r + occ.radius);
    let rho = (w - u * x).length();
    let outside = if t > 0.0 { rho * cos_b - t * sin_b } else { (w + u * (t - x)).length() };
    if outside >= target.radius {
        return 0.0;
    }
    let near = w.length() - target.radius;
    let s_ang = (star_r / (star - target.centre).length()).clamp(0.0, 1.0).asin();
    let o_ang = (occ.radius / near).clamp(0.0, 1.0).asin();
    (o_ang / s_ang).powi(2).min(1.0)
}

/// The bodies that can eclipse a star on `bodies[i]`, from any of
/// `stars[k]` for k in `lit`: ranked by the share of the body's light from
/// those stars they can hide (each star's [`shadow_depth`] weighted by its
/// share of the flux at the body), deepest first, ties by index, at most
/// [`MAX_OCCLUDERS`], none under [`MIN_SHADOW`].
pub fn occluders(i: usize, bodies: &[Sphere], stars: &[Star], lit: &[usize]) -> Vec<usize> {
    let me = bodies[i];
    let lux: Vec<f64> = lit.iter().map(|&k| stars[k].lux_at(me.centre)).collect();
    let total: f64 = lux.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut found: Vec<(usize, f64)> = (0..bodies.len())
        .filter(|&j| j != i)
        .map(|j| {
            let depth = lit
                .iter()
                .zip(&lux)
                .map(|(&k, e)| e / total * shadow_depth(me, bodies[j], stars[k].centre, stars[k].radius))
                .fold(0.0, f64::max);
            (j, depth)
        })
        .filter(|&(_, depth)| depth >= MIN_SHADOW)
        .collect();
    found.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    found.into_iter().take(MAX_OCCLUDERS).map(|(j, _)| j).collect()
}

/// Planetshine (rgb lux) at `at` from `body` lit by every star: each
/// star's light reflected with the body's tint and the star's.
pub fn shine_from(at: DVec3, body: Sphere, stars: &[Star]) -> [f64; 3] {
    let mut rgb = [0.0; 3];
    for s in stars {
        let to_star = s.centre - body.centre;
        let d = to_star.length();
        if d <= 0.0 {
            continue;
        }
        let e = planetshine(at, body.centre, body.radius, body.albedo, s.lux_at(body.centre), to_star / d);
        for (c, out) in rgb.iter_mut().enumerate() {
            *out += e * s.tint[c] * body.tint[c];
        }
    }
    rgb
}

/// What one body receives (D055: at its own position, never the camera's).
#[derive(Clone, Debug, PartialEq)]
pub struct BodyLight {
    /// Per light slot: the star's flux at the body over its flux at the
    /// camera (the slot's light carries the latter).
    pub scale: Vec<f64>,
    /// Bodies that can eclipse the slotted stars here ([`occluders`]).
    pub occluders: Vec<usize>,
    /// The brightest planetshine: reflector index, rgb lux at the body's
    /// centre, at most [`MAX_SHINES`].
    pub shines: Vec<(usize, [f64; 3])>,
    /// Starlight without a slot (a fifth star): lux, sphere-averaged (E/4:
    /// a directional flux spread over the whole sphere), drawn as ambient.
    pub ambient: f64,
}

/// Lighting for `bodies[i]` from `stars`, of which `slots` are drawn as
/// lights ([`star_slots`] at the camera).
pub fn body_light(i: usize, bodies: &[Sphere], stars: &[Star], slots: &[usize]) -> BodyLight {
    let me = bodies[i];
    let scale = slots
        .iter()
        .map(|&k| {
            let at_cam = stars[k].lux_at(DVec3::ZERO);
            if at_cam > 0.0 {
                stars[k].lux_at(me.centre) / at_cam
            } else {
                0.0
            }
        })
        .collect();
    let ambient = (0..stars.len()).filter(|k| !slots.contains(k)).map(|k| stars[k].lux_at(me.centre) / 4.0).sum();
    let mut shines: Vec<(usize, [f64; 3])> = (0..bodies.len())
        .filter(|&j| j != i)
        .map(|j| (j, shine_from(me.centre, bodies[j], stars)))
        .filter(|(_, rgb)| luminance(*rgb) > MIN_SHINE)
        .collect();
    shines.sort_by(|a, b| luminance(b.1).total_cmp(&luminance(a.1)).then(a.0.cmp(&b.0)));
    shines.truncate(MAX_SHINES);
    BodyLight { scale, occluders: occluders(i, bodies, stars, slots), shines, ambient }
}

/// Light at a small object (a ship) at `at`, lit by the slotted stars'
/// lights: the share of their flux there that is not eclipsed (each
/// star's visible fraction weighted by its flux: exact for one star; a
/// standard material takes one factor for all its lights), and the sum of
/// every body's planetshine (rgb lux).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObjectLight {
    pub visible: f64,
    pub shine: [f64; 3],
}

/// [`ObjectLight`] at `at` among `bodies`, with `slots` of `stars` drawn
/// as lights.
pub fn object_light(at: DVec3, stars: &[Star], slots: &[usize], bodies: &[Sphere]) -> ObjectLight {
    let occluders: Vec<(DVec3, f64)> = bodies.iter().map(|b| (b.centre, b.radius)).collect();
    let (mut lit, mut total) = (0.0, 0.0);
    for &k in slots {
        let e = stars[k].lux_at(at);
        total += e;
        lit += e * eclipse_factor(at, stars[k].centre, stars[k].radius, &occluders);
    }
    let visible = if total > 0.0 { lit / total } else { 1.0 };
    let mut shine = [0.0; 3];
    for b in bodies {
        let rgb = shine_from(at, *b, stars);
        for c in 0..3 {
            shine[c] += rgb[c];
        }
    }
    ObjectLight { visible, shine }
}

/// The astronomical unit (m, IAU 2012).
pub const AU: f64 = 1.495_978_707e11;

/// The illuminance the base exposure is set for (D087): the Sun's light at
/// 1 AU (IAU nominal luminosity 3.828e26 W, 93 lm/W), ~126,600 lux. A
/// camera calibration, the same in every system: a dim star's planets are
/// exposed for their own light, not for the Sun's at 1 AU from that star.
pub fn reference_lux() -> f64 {
    flux(luminous_power(3.828e26, 93.0), AU)
}

/// How far the exposure may follow the starlight (stops from the base):
/// down to ~128 AU from the Sun (past Sedna's perihelion), up to 0.25 AU.
const EXPOSURE_STOPS: (f64, f64) = (-14.0, 4.0);

/// The camera's exposure (EV100) under D087: the base exposure set for
/// [`reference_lux`], shifted by the stops between the stellar flux at the
/// camera (`e_cam`: every star's, inverse square, no eclipses: it is the
/// light where the photographer is, never the scene's luminance) and
/// `e_ref`. `follow` false: the fixed base exposure (D055).
pub fn exposure_ev100(base: f32, e_cam: f64, e_ref: f64, follow: bool) -> f32 {
    let stops = (e_cam / e_ref).log2();
    if !follow || stops.is_nan() {
        return base;
    }
    base + stops.clamp(EXPOSURE_STOPS.0, EXPOSURE_STOPS.1) as f32
}
