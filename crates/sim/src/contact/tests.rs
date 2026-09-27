use super::*;
use crate::craft::Spring;

fn params() -> ContactFile {
    ContactFile {
        foot: Spring { stiffness: 4e5, damping: 6e4, stroke: 0.35, friction: 0.8 },
        hull: Spring { stiffness: 2e6, damping: 1e5, stroke: 0.0, friction: 0.5 },
        stick_speed: 0.05,
    }
}

#[test]
fn normal_force_table() {
    use ContactKind::{Foot, Hull};
    let c = params();
    // (kind, depth, closing) → force
    let cases = [
        (Foot, -0.1, 5.0, 0.0),                            // above the ground
        (Foot, 0.0, 5.0, 0.0),                             // just touching
        (Foot, 0.1, 0.0, 4e4),                             // spring
        (Foot, 0.1, 1.0, 4e4 + 6e4),                       // spring + damper
        (Foot, 0.1, -2.0, 0.0),                            // pulling away: never pulls
        (Foot, 0.45, 0.0, 4e5 * 0.35 + 2e6 * 0.1),         // past the stroke: hull spring
        (Foot, 0.45, 1.0, 4e5 * 0.35 + 2e6 * 0.1 + 1.6e5), // and both dampers
        (Hull, 0.01, 0.0, 2e4),
        (Hull, 0.01, 0.5, 2e4 + 5e4),
    ];
    for (kind, depth, closing, expected) in cases {
        let f = normal_force(kind, &c, depth, closing);
        assert!((f - expected).abs() < 1e-6 * expected.max(1.0), "{kind:?} {depth} {closing}: {f} vs {expected}");
    }
}

#[test]
fn friction_table() {
    // (μ, F_n, v_t, stick, cap) → force
    let x = DVec3::X;
    let cases = [
        (0.8, 1e4, x * 2.0, 0.05, f64::INFINITY, -x * 8e3), // sliding: Coulomb
        (0.8, 1e4, x * 0.01, 0.05, f64::INFINITY, -x * 1.6e3), // below stick: viscous
        (0.8, 1e4, x * 2.0, 0.05, 100.0, -x * 200.0),       // capped by the substep
        (0.8, 0.0, x * 2.0, 0.05, f64::INFINITY, DVec3::ZERO), // no load, no friction
        (0.8, 1e4, DVec3::ZERO, 0.05, f64::INFINITY, DVec3::ZERO), // not sliding
        (0.0, 1e4, x, 0.05, f64::INFINITY, DVec3::ZERO),    // frictionless
    ];
    for (mu, f_n, v_t, stick, cap, expected) in cases {
        let f = friction_force(mu, f_n, v_t, stick, cap);
        assert!((f - expected).length() < 1e-9, "{mu} {f_n} {v_t}: {f} vs {expected}");
    }
}

#[test]
fn impact_table() {
    use ContactKind::{Foot, Hull};
    // (kind, depth, closing) → destroyed, with stroke 0.35 and limit 8 m/s
    let cases = [
        (Foot, 0.1, 20.0, false), // within the stroke the gear takes it
        (Foot, 0.4, 7.0, false),  // past the stroke but slow enough
        (Foot, 0.4, 9.0, true),   // past the stroke and too fast
        (Hull, 0.001, 9.0, true), // any hull contact too fast
        (Hull, 0.001, 7.9, false),
        (Hull, 0.0, 30.0, false), // not touching
    ];
    for (kind, depth, closing, expected) in cases {
        assert_eq!(impact(kind, 0.35, depth, closing, 8.0), expected, "{kind:?} {depth} {closing}");
    }
}

#[test]
fn ground_normal_of_planes_and_spheres() {
    // A plane rising to the north by tan θ at a point on the x axis (east
    // is +y, north is +z there): clearance = x − R − z·tan θ.
    let r = 1_737_400.0;
    for deg in [0.0, 2.0, 30.0] {
        let t = crate::math::tan(deg * crate::math::PI / 180.0);
        let p = DVec3::new(r, 0.0, 0.0);
        let n = ground_normal(|q| q.x - r - q.z * t, p, NORMAL_STEP);
        let expected = DVec3::new(1.0, 0.0, -t).normalize();
        assert!((n - expected).length() < 1e-9, "{deg}°: {n} vs {expected}");
    }
    // A sphere: the normal is radial.
    let p = DVec3::new(3e6, -4e6, 2e6);
    let n = ground_normal(|q| q.length() - 6.4e6, p, NORMAL_STEP);
    assert!((n - p.normalize()).length() < 1e-9, "{n}");
}

#[test]
fn rest_counter_table() {
    let slow = DVec3::X * 0.01;
    let fast = DVec3::X * 0.2;
    let spin = DVec3::Z * 0.05;
    // (count, touching, input, v, ω) → new count
    let cases = [
        (0, true, false, slow, DVec3::ZERO, 1),
        (49, true, false, slow, DVec3::X * 0.005, 50),
        (30, false, false, slow, DVec3::ZERO, 0), // in the air
        (30, true, true, slow, DVec3::ZERO, 0),   // thrust or rotation input
        (30, true, false, fast, DVec3::ZERO, 0),
        (30, true, false, slow, spin, 0),
    ];
    for (count, touching, input, v, w, expected) in cases {
        assert_eq!(rest_ticks(count, touching, input, v, w), expected, "{count} {touching} {input} {v} {w}");
    }
}

#[test]
fn holds_table() {
    // Four feet at radius 3 m, 3.6 m below the centre of mass, on the
    // ground (z = 0); gravity along −z, or tilted by θ towards the diagonal
    // between two feet (a slope with two feet downhill).
    let feet =
        [DVec3::new(3.0, 0.0, 0.0), DVec3::new(0.0, 3.0, 0.0), DVec3::new(-3.0, 0.0, 0.0), DVec3::new(0.0, -3.0, 0.0)];
    let com = DVec3::new(0.0, 0.0, 3.6);
    let tilted = |deg: f64| {
        let a = deg * crate::math::PI / 180.0;
        let s = crate::math::sin(a) * std::f64::consts::FRAC_1_SQRT_2;
        DVec3::new(s, s, -crate::math::cos(a))
    };
    let on = |mu: f64| feet.iter().map(|&p| (p, DVec3::Z, mu)).collect::<Vec<_>>();
    // Tipping about the edge between two feet (half-width 3/√2 m): at
    // atan(2.12/3.6) = 30.5°. Sliding: at atan(μ).
    let cases = [
        (0.0, 0.8, true),
        (2.0, 0.8, true),
        (30.0, 0.8, true),
        (31.0, 0.8, false), // tips
        (20.0, 0.3, false), // slides: tan 20° > 0.3
        (16.0, 0.3, true),
    ];
    for (deg, mu, expected) in cases {
        let support = on(mu);
        assert_eq!(holds(com, tilted(deg), &support), expected, "{deg}° μ {mu}");
    }
    // Two points cannot hold, nor can a centre of mass beside them.
    assert!(!holds(com, -DVec3::Z, &on(0.8)[..2]));
    assert!(!holds(DVec3::new(5.0, 0.0, 3.6), -DVec3::Z, &on(0.8)));
}
