//! A ship's light from the real Sun and Earth (the lead's blue Falcon 9 at
//! max-Q, 2026-10-27): sunlit white paint stays near white; Earthshine is a
//! fraction of the sunlight and bluish only on the sides away from the Sun.

use super::color::{reflector_tint, star_tint};
use super::rules::{luminous_power, object_light, star_slots, Sphere, Star};
use crate::state::SimState;
use glam::DVec3;

/// The light at `height` above the pad at the game's start (17:00 TDB on
/// 2030-01-01, the Sun ~38° up over Kennedy), relative to that point: the
/// stars, the reflecting bodies (planetshine tinted by `tint_of`), the
/// local up and the direction to the Sun.
fn scene(
    height: f64,
    tint_of: impl Fn(&crate::body_visual::BodyVisualDef) -> [f32; 3],
) -> (Vec<Star>, Vec<Sphere>, DVec3, DVec3) {
    let sim = SimState::new();
    let (anchor, r, _) = sim.ship().state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let earth = sim.world.find("Earth").unwrap().node;
    let up = (r - snap.relative_r(earth, anchor)).normalize();
    let at = r + up * height;
    let rel = |n| snap.relative_r(n, anchor) - at;
    let (mut stars, mut bodies) = (Vec::new(), Vec::new());
    for src in &sim.world.sources {
        let (Some(def), Some(p)) = (crate::body_visual::load(&src.name), src.physical.as_ref()) else { continue };
        match &def.emissive {
            Some(e) => stars.push(Star {
                centre: rel(src.node),
                radius: p.radius_eq,
                lm: luminous_power(e.luminosity_w, e.luminous_efficacy),
                tint: e.temperature_k.map_or([1.0; 3], star_tint),
            }),
            None => bodies.push(Sphere {
                centre: rel(src.node),
                radius: p.radius_eq,
                albedo: super::reflector_albedo(def.albedo, def.geometric_albedo),
                tint: reflector_tint(tint_of(&def)),
            }),
        }
    }
    let sun = stars[0].centre.normalize();
    (stars, bodies, up, sun)
}

/// Irradiance per channel (lux) on a face with normal `n`: (sunlight,
/// planetshine).
fn faces(stars: &[Star], bodies: &[Sphere], n: DVec3, to_sun: DVec3) -> ([f64; 3], [f64; 3]) {
    let slots = star_slots(stars, DVec3::ZERO);
    let o = object_light(DVec3::ZERO, stars, &slots, bodies, &[]);
    let sun = stars[slots[0]].lux_at(DVec3::ZERO) * o.scale[0] * n.dot(to_sun).max(0.0);
    let mut shine = [0.0; 3];
    for s in &o.shines {
        let e = s.irradiance(n);
        for k in 0..3 {
            shine[k] += e[k];
        }
    }
    ([sun; 3], shine)
}

fn total(f: ([f64; 3], [f64; 3])) -> [f64; 3] {
    [0, 1, 2].map(|k| f.0[k] + f.1[k])
}

/// The light on a ship's faces at 12 km (max-Q) and 185 km. Measured
/// before the fix (Earthshine tinted with the drawn ocean colour): on the
/// faces away from the Sun blue/red 9.7, the blue channel alone 128,000 lux
/// at 12 km against the Sun's 131,000 (a saturated blue rocket); its
/// luminance ~31 % of the sunlight, which is fine.
#[test]
fn sunlit_white_paint_stays_white_and_earthshine_is_a_bluish_fraction() {
    let ratio = |c: [f64; 3]| c[2] / c[0].max(1.0);
    let lum = super::rules::luminance;
    for height in [12_000.0, 185_000.0] {
        let (stars, bodies, up, sun) = scene(height, |d| d.shine_color.unwrap_or(d.base_color));
        let away = (sun - up * sun.dot(up)).normalize_or(DVec3::X);
        // (face, its normal, blue/red range of its light)
        let table = [
            ("sunlit", sun, (1.0, 1.05)),
            ("side", up.cross(away).normalize(), (1.1, 1.3)),
            ("away", -sun, (1.1, 1.3)),
            ("down", -up, (1.1, 1.3)),
        ];
        for (face, n, (lo, hi)) in table {
            let (s, e) = faces(&stars, &bodies, n, sun);
            let r = ratio(total((s, e)));
            assert!(r >= lo && r <= hi, "{height} m {face}: blue/red {r:.2}, earthshine {e:?}, sun {:.0}", s[0]);
            // Earthshine is a fraction of the sunlight, on every face.
            let full = stars[0].lux_at(DVec3::ZERO);
            assert!(lum(e) < 0.45 * full, "{height} m {face}: earthshine {:.0} of {full:.0} lux", lum(e));
        }
        // The sunlit face is lit mostly by the Sun.
        let (s, e) = faces(&stars, &bodies, sun, sun);
        assert!(lum(e) < 0.1 * s[0], "{height} m: {:.0} against {:.0}", lum(e), s[0]);
    }
    // The drawn colour as a tint would make Earthshine ten times bluer than red.
    let earth = crate::body_visual::load("Earth").unwrap();
    let (drawn, shine) = (reflector_tint(earth.base_color), reflector_tint(earth.shine_color.unwrap()));
    let tint_ratio = |c: [f64; 3]| c[2] / c[0];
    assert!(tint_ratio(drawn) > 5.0 && tint_ratio(shine) < 1.3, "{drawn:?} {shine:?}");
}
