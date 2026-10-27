//! Table tests of the lighting rules, with one star (Sol) and with
//! synthetic multiple-star systems (data only).

use super::rules::*;
use super::*;
use std::f64::consts::PI;

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
    let sun = [sun_at(DVec3::new(-AU, 0.0, 0.0), [1.0; 3])];
    // Camera near the Moon, Moon behind Earth (full Moon, eclipse side).
    let earth = Sphere { centre: DVec3::new(-3.84e8, 0.0, 0.0), radius: EARTH_R, albedo: 0.3, tint: [1.0; 3] };
    let moon = Sphere { centre: DVec3::X * 2.0e6, radius: 1.737e6, albedo: 0.12, tint: [1.0; 3] };
    let bodies = [earth, moon];
    let lm_moon = body_light(1, &bodies, &sun, &[0]);
    let lm_earth = body_light(0, &bodies, &sun, &[0]);
    // Flux at each body's own distance, relative to the camera's.
    assert!((lm_moon.scale[0] - (AU / (AU + 2.0e6)).powi(2)).abs() < 1e-9);
    assert!(lm_earth.scale[0] > lm_moon.scale[0]);
    // Earth sits between the Moon and the Sun: the Moon's occluder.
    assert_eq!(lm_moon.occluders, vec![0]);
    // Full Moon: from the Moon, Earth is new (no Earthshine), and the
    // full Moon lights Earth's night side.
    assert!(lm_moon.shines.is_empty());
    let (from, moonshine) = lm_earth.shines[0];
    assert_eq!(from, 1);
    assert!(moonshine[1] > 0.05 && moonshine[1] < 0.5, "full moonlight ~0.2 lux: {moonshine:?}");
    // It is computed at the body, not the camera: moving the camera
    // (everything shifts) leaves it unchanged.
    let shift = DVec3::new(1e7, 3e7, 0.0);
    let moved: Vec<Sphere> = bodies.iter().map(|b| Sphere { centre: b.centre + shift, ..*b }).collect();
    let moved_sun = [sun_at(sun[0].centre + shift, [1.0; 3])];
    let (_, moved_shine) = body_light(0, &moved, &moved_sun, &[0]).shines[0];
    assert!((moved_shine[1] - moonshine[1]).abs() < 1e-9 * moonshine[1]);
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

const JUP_R: f64 = 7.1492e7;
const IO_R: f64 = 1.8216e6;
const IO_A: f64 = 4.217e8;

fn sphere(x: f64, y: f64, radius: f64) -> Sphere {
    Sphere { centre: DVec3::new(x, y, 0.0), radius, albedo: 0.5, tint: [1.0; 3] }
}

/// A star of the Sun's power and radius at `centre`, tinted `tint`.
fn sun_at(centre: DVec3, tint: [f64; 3]) -> Star {
    Star { centre, radius: SUN_R, lm: luminous_power(3.828e26, 93.0), tint }
}

/// [`occluders`] with one Sun at `sun`.
fn occ1(i: usize, bodies: &[Sphere], sun: DVec3) -> Vec<usize> {
    occluders(i, bodies, &[sun_at(sun, [1.0; 3])], &[0])
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
        assert_eq!(occ1(target, &bodies, sun), want, "{case}");
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
    assert_eq!(occ1(0, &bodies, sun), vec![2, 4, 6, 3]);
    // Moving the camera (every centre shifts) changes nothing.
    let shift = DVec3::new(3e9, -2e8, 7e8);
    let moved: Vec<Sphere> = bodies.iter().map(|b| Sphere { centre: b.centre + shift, ..*b }).collect();
    assert_eq!(occ1(0, &moved, sun + shift), vec![2, 4, 6, 3]);
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
        let chosen = occ1(0, &bodies, sun) == vec![1];
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

/// A synthetic binary (data only): a Sun-like star A at 1 AU on −x and a
/// red dwarf B (3200 K, 5 % of the Sun's power, 0.3 solar radii) at
/// 0.5 AU on +x from the camera at the origin; a planet P 1e9 m off on +y
/// with a moon m 4e8 m from it on +x (towards B).
fn binary() -> (Vec<Star>, Vec<Sphere>) {
    let sun = luminous_power(3.828e26, 93.0);
    let a = Star { centre: DVec3::new(-AU, 0.0, 0.0), radius: SUN_R, lm: sun, tint: [1.0; 3] };
    let b = Star {
        centre: DVec3::new(0.5 * AU, 0.0, 0.0),
        radius: 0.3 * SUN_R,
        lm: 0.05 * sun,
        tint: color::star_tint(3200.0),
    };
    let p = Sphere { centre: DVec3::new(0.0, 1e9, 0.0), radius: 6.4e6, albedo: 0.3, tint: [1.0; 3] };
    let m = Sphere { centre: DVec3::new(4e8, 1e9, 0.0), radius: 1.7e6, albedo: 0.12, tint: [1.0; 3] };
    (vec![a, b], vec![p, m])
}

#[test]
fn stars_are_drawn_brightest_first_and_the_rest_is_ambient() {
    let (stars, _) = binary();
    // A gives ~126,600 lux at the camera, B ~25,000: A first.
    assert_eq!(star_slots(&stars, DVec3::ZERO), vec![0, 1]);
    // Seen from near B, B is first.
    assert_eq!(star_slots(&stars, DVec3::new(0.45 * AU, 0.0, 0.0)), vec![1, 0]);
    // Equal stars: by index. A dark star (no power) or one around the
    // camera sends no light and takes no slot.
    let twin = |x: f64| Star { centre: DVec3::new(x, 0.0, 0.0), ..stars[0] };
    assert_eq!(star_slots(&[twin(AU), twin(-AU)], DVec3::ZERO), vec![0, 1]);
    let dark = Star { lm: 0.0, ..stars[1] };
    let around = Star { centre: DVec3::ZERO, ..stars[1] };
    assert_eq!(star_slots(&[dark, stars[0], around], DVec3::ZERO), vec![1]);
    // Six stars: four slots; the other two light every body as ambient,
    // sphere-averaged (E/4), so no starlight is dropped.
    let six: Vec<Star> = (0..6).map(|k| twin(AU * (1.0 + f64::from(k)))).collect();
    let slots = star_slots(&six, DVec3::ZERO);
    assert_eq!(slots, vec![0, 1, 2, 3]);
    let body = [Sphere { centre: DVec3::new(0.0, 1e9, 0.0), radius: 1e6, albedo: 0.3, tint: [1.0; 3] }];
    let l = body_light(0, &body, &six, &slots);
    let rest = (six[4].lux_at(body[0].centre) + six[5].lux_at(body[0].centre)) / 4.0;
    assert!((l.ambient - rest).abs() < 1e-9 * rest, "{} {rest}", l.ambient);
    assert_eq!(l.scale.len(), 4);
}

#[test]
fn a_moon_eclipsed_from_one_star_but_not_the_other() {
    let (stars, bodies) = binary();
    let slots = star_slots(&stars, DVec3::ZERO);
    let (p, m) = (bodies[0], bodies[1]);
    // The planet hides A from the moon (A lies beyond it), not B.
    assert_eq!(occluders(1, &bodies, &stars, &slots), vec![0]);
    let occ = [(p.centre, p.radius)];
    assert_eq!(eclipse_factor(m.centre, stars[0].centre, stars[0].radius, &occ), 0.0);
    assert_eq!(eclipse_factor(m.centre, stars[1].centre, stars[1].radius, &occ), 1.0);
    // A small object at the moon keeps B's share of the drawn light.
    let o = object_light(m.centre + DVec3::Z * 2e6, &stars, &slots, &bodies);
    let (ea, eb) = (stars[0].lux_at(m.centre), stars[1].lux_at(m.centre));
    assert!((o.visible - eb / (ea + eb)).abs() < 1e-3, "{}", o.visible);
    // With one star the share is that star's eclipse factor.
    let one = object_light(m.centre + DVec3::Z * 2e6, &stars[..1], &[0], &bodies);
    assert!(one.visible < 1e-9, "{}", one.visible);
}

#[test]
fn two_stars_of_two_colours_light_one_moon() {
    let (stars, bodies) = binary();
    let slots = star_slots(&stars, DVec3::ZERO);
    let l = body_light(1, &bodies, &stars, &slots);
    // Each star's flux at the moon, relative to its light at the camera.
    for (k, &s) in slots.iter().enumerate() {
        let want = stars[s].lux_at(bodies[1].centre) / stars[s].lux_at(DVec3::ZERO);
        assert!((l.scale[k] - want).abs() < 1e-12, "slot {k}");
    }
    // The uniforms: A's light white, B's red, each the flux at the moon.
    let ids = [sim::frame::NodeId(0), sim::frame::NodeId(1)];
    let scene = LightScene {
        stars: stars.clone(),
        slots: slots.clone(),
        bodies: ids.into_iter().zip(bodies.iter().copied()).collect(),
    };
    let u = LightUniform::new(&scene, &l, &[], true);
    let a = u.star_light[0];
    let b = u.star_light[1];
    assert!((a.x - a.z).abs() < 1e-3 * a.x, "A white: {a}");
    assert!(b.x > 1.5 * b.z, "B red: {b}");
    let lux_b = stars[1].lux_at(bodies[1].centre) as f32;
    let y = 0.2126 * b.x + 0.7152 * b.y + 0.0722 * b.z;
    assert!((y - lux_b).abs() < 1e-3 * lux_b, "B's luminance is its flux: {y} {lux_b}");
    assert_eq!(u.stars[2].w, 0.0, "unused slots are empty");
    assert_eq!(u.occluders[0].w, bodies[0].radius as f32);
    // A ring's planet comes first, then the body's own occluders, once.
    let ring = LightUniform::new(&scene, &l, &[0], true);
    assert_eq!(ring.occluders[0], u.occluders[0]);
    assert_eq!(ring.occluders[1].w, 0.0);
}

#[test]
fn planetshine_from_two_bodies_in_two_colours() {
    // A moon between A (on −x) and a large planet Q on +x (seen nearly
    // full), a planet R off on +z (seen at quarter), and a far small one.
    let (stars, _) = binary();
    let m = Sphere { centre: DVec3::new(0.0, 1e9, 0.0), radius: 1.7e6, albedo: 0.12, tint: [1.0; 3] };
    let q = Sphere { centre: DVec3::new(3e8, 1e9, 0.0), radius: 6e7, albedo: 0.5, tint: [1.2, 1.0, 0.7] };
    let r = Sphere { centre: DVec3::new(0.0, 1e9, 4e8), radius: 2.5e7, albedo: 0.6, tint: [0.7, 1.0, 1.3] };
    let far = Sphere { centre: DVec3::new(0.0, 1e9, -3e10), radius: 1e6, albedo: 0.3, tint: [1.0; 3] };
    let bodies = [m, far, r, q];
    let l = body_light(0, &bodies, &stars, &[0, 1]);
    // The two brightest, brightest first: Q (large, near full) then R.
    let order: Vec<usize> = l.shines.iter().map(|s| s.0).collect();
    assert_eq!(order, vec![3, 2]);
    // Each reflector's colour is its tint times the stars' light.
    let (_, qs) = l.shines[0];
    let (_, rs) = l.shines[1];
    assert!(qs[0] > qs[2] && rs[2] > rs[0], "{qs:?} {rs:?}");
    // The sum equals the two stars' shine computed separately.
    let one = |k: usize| rules::shine_from(m.centre, q, &stars[k..=k]);
    for c in 0..3 {
        assert!((qs[c] - one(0)[c] - one(1)[c]).abs() < 1e-9 * qs[c]);
    }
}

#[test]
fn no_nan_at_zero_distance_or_zero_light() {
    let (stars, bodies) = binary();
    // No stars: nothing is lit, nothing is NaN.
    let dark = body_light(0, &bodies, &[], &[]);
    assert!(dark.scale.is_empty() && dark.shines.is_empty() && dark.occluders.is_empty() && dark.ambient == 0.0);
    let o = object_light(DVec3::ZERO, &[], &[], &bodies);
    assert_eq!(o.visible, 1.0);
    // A body at the camera, a body inside a star, a point-sized body, a
    // dark star, the camera inside a star.
    let at_cam = Sphere { centre: DVec3::ZERO, ..bodies[1] };
    let in_star = Sphere { centre: stars[1].centre, ..bodies[1] };
    let point = Sphere { radius: 0.0, ..bodies[1] };
    let mut odd = stars.clone();
    odd.push(Star { lm: 0.0, ..stars[0] });
    odd.push(Star { centre: DVec3::ZERO, ..stars[1] });
    let set = [at_cam, in_star, point, bodies[0]];
    let slots = star_slots(&odd, DVec3::ZERO);
    for i in 0..set.len() {
        let l = body_light(i, &set, &odd, &slots);
        assert!(l.scale.iter().all(|v| v.is_finite()), "{i}: {:?}", l.scale);
        assert!(l.ambient.is_finite());
        assert!(l.shines.iter().all(|s| s.1.iter().all(|c| c.is_finite())), "{i}");
        let o = object_light(set[i].centre, &odd, &slots, &set);
        assert!(o.visible.is_finite() && o.shine.iter().all(|c| c.is_finite()), "{i}: {o:?}");
    }
    for j in 0..set.len() {
        for s in &odd {
            assert!(shadow_depth(set[0], set[j], s.centre, s.radius).is_finite());
        }
    }
}

#[test]
fn a_ring_shadows_differently_under_two_stars() {
    // A ring plane z = 0 of optical depth 1 between 1e8 and 2e8 m; a point
    // above the plane: the ray to a star low in the south crosses the
    // ring, the ray to a star overhead does not.
    let n = DVec3::Z;
    let tau = |r: f64| if (1e8..2e8).contains(&r) { 1.0 } else { 0.0 };
    let p = DVec3::new(0.0, 0.0, 5e7);
    let low = (DVec3::new(2.5e8, 0.0, -5e7) - p).normalize();
    let overhead = DVec3::Z;
    let t_low = crate::rings::ring_transmission(p, low, n, tau);
    let t_up = crate::rings::ring_transmission(p, overhead, n, tau);
    assert!(t_low < 0.5 && t_low > 0.0, "{t_low}");
    assert_eq!(t_up, 1.0);
    // In the ring plane itself (μ = 0) the rule stays finite.
    assert!(crate::rings::ring_transmission(DVec3::new(1.5e8, 0.0, 0.0), DVec3::X, n, tau).is_finite());
}

#[test]
fn the_shaders_share_the_lighting_limits() {
    let wgsl = include_str!("light.wgsl");
    for decl in [
        format!("const MAX_STARS: i32 = {MAX_STARS};"),
        format!("const MAX_OCCLUDERS: i32 = {MAX_OCCLUDERS};"),
        format!("const MAX_SHINES: i32 = {MAX_SHINES};"),
        format!("stars: array<vec4<f32>, {MAX_STARS}>,"),
        format!("star_light: array<vec4<f32>, {MAX_STARS}>,"),
        format!("occluders: array<vec4<f32>, {MAX_OCCLUDERS}>,"),
        format!("shines: array<vec4<f32>, {MAX_SHINES}>,"),
        format!("shine_light: array<vec4<f32>, {MAX_SHINES}>,"),
    ] {
        assert!(wgsl.contains(&decl), "light.wgsl lacks `{decl}`");
    }
}

#[test]
fn planetshine_from_the_data_is_of_the_measured_size() {
    // Full-phase planetshine is E p (R/d)^2 with the body's measured
    // geometric albedo (visual.ron), not 2/3 of its Bond albedo.
    let reflector = |name: &str| {
        let d = crate::body_visual::load(name).expect(name);
        super::reflector_albedo(d.albedo, d.geometric_albedo)
    };
    let sun = luminous_power(3.828e26, 93.0);
    let full = |name: &str, r: f64, d: f64, au: f64| {
        let e = flux(sun, au * AU);
        planetshine(DVec3::new(-d, 0.0, 0.0), DVec3::ZERO, r, reflector(name), e, DVec3::new(-1.0, 0.0, 0.0))
    };
    // (case, lux at full phase, expected range)
    let cases = [
        // Earth (p 0.434) from the Moon at new Moon: ~15 lux, some 50 times
        // full moonlight (~0.3 lux).
        ("Earthshine on the Moon", full("Earth", EARTH_R, 3.844e8, 1.0), (12.0, 18.0)),
        // Jupiter (p 0.538) from Io at full Jupiter: ~73 lux, 1.5 % of
        // Io's sunlight (4,680 lux at 5.2 AU).
        ("Jupitershine on Io", full("Jupiter", 7.1492e7, 4.217e8, 5.203), (60.0, 85.0)),
        // Saturn (p 0.499) from Titan: ~1.7 lux, 0.12 % of its sunlight.
        ("Saturnshine on Titan", full("Saturn", 6.0268e7, 1.2219e9, 9.537), (1.4, 2.0)),
    ];
    for (case, lux, (lo, hi)) in cases {
        assert!(lux >= lo && lux <= hi, "{case}: {lux}");
    }
    // A body without a measured geometric albedo reflects as a Lambert
    // sphere of its Bond albedo.
    assert_eq!(super::reflector_albedo(0.3, None), 0.3f32 as f64);
    assert!((super::reflector_albedo(0.3, Some(0.4)) - 0.6).abs() < 1e-6);
}
