//! Slender-body and fin lift (D070).

use super::tests::{craft_bake, flow, shape};
use super::*;
use crate::craft::Shape;

/// A 15° cone on a cylinder, radius 0.5 m, 10 m long (fineness 10), nose +Z.
fn cone_cylinder() -> AeroBake {
    let half = 15f64.to_radians();
    let cone = 0.5 / math::tan(half);
    let body = Shape::Cylinder { base: DVec3::ZERO, axis: DVec3::Z * (10.0 - cone), radius: 0.5 };
    let nose = Shape::Frustum { base: DVec3::Z * (10.0 - cone), axis: DVec3::Z * cone, r_base: 0.5, r_top: 0.001 };
    let (s, c) = shape(&[body, nose]);
    bake(&s, &c, &BakeOptions::default())
}

/// Normal-force coefficient on area `a` at incidence `alpha_deg` in the X–Z
/// plane (nose +Z; the air comes from below the nose, toward +X).
fn normal_coefficient(b: &AeroBake, alpha_deg: f64, mach: f64, cd0: f64, a: f64) -> f64 {
    let al = alpha_deg.to_radians();
    let f = flow(DVec3::new(math::sin(al), 0.0, -math::cos(al)), mach, 1e-5);
    let (force, _) = aero_forces(b, &f, cd0);
    force.x / (f.q * a)
}

#[test]
fn a_slender_cone_cylinder_has_the_slender_body_normal_force_slope() {
    // Slender-body theory: C_Nα = 2 per radian on the base area (Munk;
    // Allen & Perkins, NACA TR 1048), measured on cone- and
    // ogive-cylinders at M 0.5–2 within about 10 % of it.
    let b = cone_cylinder();
    assert!(b.sections.axis.z.abs() > 0.999, "{}", b.sections.axis);
    let base = math::PI * 0.25;
    for mach in [0.5, 2.0] {
        let slope = normal_coefficient(&b, 1.0, mach, 0.3, base) / 1f64.to_radians();
        println!("M {mach}: C_Nα {slope:.3} /rad");
        assert!((slope / 2.0 - 1.0).abs() < 0.1, "M {mach}: {slope}");
    }
    // At 10°: Allen–Perkins by hand, C_N = sin 20°·cos 5° + η·C_dc·(A_p/A_b)·
    // sin²10° with A_p = 9.067 m² (plan area), η(L/D 10) = 0.675, C_dc 1.2:
    // 0.341 + 0.282 = 0.623.
    let cn = normal_coefficient(&b, 10.0, 0.5, 0.3, base);
    println!("M 0.5, α 10°: C_N {cn:.3}");
    assert!((cn / 0.623 - 1.0).abs() < 0.08, "{cn}");
}

#[test]
fn slender_body_lift_fades_into_newtonian_by_mach_5() {
    let b = cone_cylinder();
    let base = math::PI * 0.25;
    let hyper = normal_coefficient(&b, 10.0, 5.0, 0.3, base);
    let just = normal_coefficient(&b, 10.0, 5.0 - 1e-9, 0.3, base);
    assert!((hyper - just).abs() < 1e-6 * hyper.abs());
}

/// A thin flat plate: chord 1 m along X, span 4 m along Y, 2 cm thick.
fn wing() -> AeroBake {
    let (s, c) = shape(&[Shape::Box { center: DVec3::ZERO, half: DVec3::new(0.5, 2.0, 0.01) }]);
    bake(&s, &c, &BakeOptions { level: 2, raster: 32 })
}

#[test]
fn a_flat_plate_wing_has_the_finite_wing_lift_slope() {
    let b = wing();
    assert_eq!(b.fins.len(), 1);
    let fin = &b.fins[0];
    assert!(fin.normal.z.abs() > 0.999 && (fin.area - 4.0).abs() < 0.01, "{fin:?}");
    // Air along +X tilted α toward +Z (it hits the −Z face): normal force +Z.
    let cn = |alpha_deg: f64, mach: f64, along: DVec3| {
        let al = alpha_deg.to_radians();
        let f = flow(along * math::cos(al) + DVec3::Z * math::sin(al), mach, 1e-5);
        aero_forces(&b, &f, 0.0).0.z / (f.q * 4.0)
    };
    // Aspect ratio 4. Helmbold (DATCOM 1.2.2.1): 2π·AR/(2 + √(4 + AR²β²)),
    // M 0.3: 4.00 /rad (NACA rectangular AR-4 wings measure ≈ 3.9–4.1);
    // supersonic rectangular wing (4/β)(1 − 1/(2β·AR)), M 2: 2.14 /rad.
    for (mach, want) in [(0.3, 4.00), (2.0, 2.143)] {
        let slope = cn(2.0, mach, DVec3::X) / (math::sin(2f64.to_radians()) * math::cos(2f64.to_radians()));
        println!("wing AR 4, M {mach}: C_Nα {slope:.3} /rad");
        assert!((slope / want - 1.0).abs() < 0.05, "M {mach}: {slope} vs {want}");
        assert!((cn(-2.0, mach, DVec3::X) + cn(2.0, mach, DVec3::X)).abs() < 1e-3 * cn(2.0, mach, DVec3::X));
    }
    // Along the span (aspect ratio 1/4) it lifts far less.
    assert!(cn(2.0, 0.3, DVec3::Y) < 0.3 * cn(2.0, 0.3, DVec3::X));
    // Stalled beyond 30°: only the plate's pressure (none with Cd₀ = 0).
    assert_eq!(cn(35.0, 0.3, DVec3::X), 0.0);
}

#[test]
fn round_and_stubby_parts_are_not_fins() {
    // The test craft's legs (thin struts) and feet (discs 0.6 m across,
    // 0.12 m thick) are not wings.
    assert!(craft_bake().fins.is_empty(), "{:?}", craft_bake().fins.len());
}

/// A Sears–Haack body along Z, `l` long, radius `r` at the middle, as 32
/// frustums: r(ξ) = r·(4ξ(1−ξ))^¾.
fn sears_haack(l: f64, r: f64) -> AeroBake {
    let n = 32;
    let radius = |k: usize| {
        let xi = k as f64 / n as f64;
        (r * math::exp(0.75 * math::ln((4.0 * xi * (1.0 - xi)).max(1e-12)))).max(0.002)
    };
    let shapes: Vec<Shape> = (0..n)
        .map(|k| Shape::Frustum {
            base: DVec3::Z * (l * k as f64 / n as f64 - 0.5 * l),
            axis: DVec3::Z * (l / n as f64),
            r_base: radius(k),
            r_top: radius(k + 1),
        })
        .collect();
    let (s, c) = shape(&shapes);
    bake(&s, &c, &BakeOptions::default())
}

#[test]
fn a_sears_haack_body_has_its_analytic_wave_drag() {
    // D/q = 9π·A_max²/(2L²) (Sears 1947, Haack 1941): L 10 m, r 0.5 m:
    // 0.0872 m², C_D 0.111 on A_max.
    let (l, r) = (10.0, 0.5);
    let a_max = math::PI * r * r;
    let want = 9.0 * math::PI * a_max * a_max / (2.0 * l * l);
    // The integral alone, on the exact area distribution.
    let exact: Vec<f64> = (0..=wave::STATIONS)
        .map(|k| {
            let xi = k as f64 / wave::STATIONS as f64;
            a_max * math::exp(1.5 * math::ln((4.0 * xi * (1.0 - xi)).max(1e-300)))
        })
        .collect();
    let direct = wave::wave_drag_area(&exact, l / wave::STATIONS as f64);
    println!("Sears–Haack: integral {direct:.5} m², analytic {want:.5}");
    assert!((direct / want - 1.0).abs() < 0.03, "{direct}");
    // The baked body (frustums, triangles), nose first and tail first.
    let b = sears_haack(l, r);
    for d in [DVec3::Z, -DVec3::Z] {
        let got = b.wave_at(d);
        println!("baked, along {d}: {got:.5} m²");
        assert!((got / want - 1.0).abs() < 0.1, "{d}: {got}");
    }
}

#[test]
fn a_blunt_body_keeps_a_large_drag_rise_and_a_slender_one_a_small() {
    // Drag coefficient on the projected area, M 0.5 → M 1.2 (Cd₀ 0.8 for
    // the capsule, Apollo-like; 0.1 for the Sears–Haack body).
    let rise = |b: &AeroBake, d: DVec3, cd0: f64| {
        let cd = |m: f64| {
            let f = flow(d, m, 1e-5);
            aero_forces(b, &f, cd0).0.dot(d) / (f.q * b.sums_at(d).area)
        };
        (cd(0.5), cd(1.2))
    };
    let capsule = super::tests::capsule();
    let (sub, sup) = rise(capsule, DVec3::Z, 0.8);
    println!("capsule: C_D {sub:.3} → {sup:.3}");
    // Apollo: ≈ 0.8 → ≈ 1.3 (heat shield first).
    assert!(sup - sub > 0.4 && sup < 1.4, "{sub} {sup}");
    let sh = sears_haack(10.0, 0.5);
    let (sub, sup) = rise(&sh, -DVec3::Z, 0.1);
    println!("Sears–Haack: C_D {sub:.3} → {sup:.3}");
    assert!((sup - sub - 0.111).abs() < 0.02, "{sub} {sup}");
}

#[test]
fn a_sphere_at_lunar_return_speed_has_the_real_gas_newtonian_drag() {
    // Cd = Cp,max/2: 0.92 for a perfect gas, ≈ 0.96 in equilibrium air at
    // 11 km/s (Cp,max ≈ 2 − 1/14.2).
    let b = super::tests::sphere();
    let mut f = flow(DVec3::Z, 36.0, 1e-5);
    f.speed = 11_000.0;
    let cd = aero_forces(b, &f, 0.8).0.z / (f.q * math::PI);
    let want = air::cp_max(air::real_gas_gamma(1.4, 11_000.0), 36.0) / 2.0;
    assert!(want > 0.95 && (cd / want - 1.0).abs() < 0.03, "{cd} vs {want}");
}

#[test]
fn an_apollo_like_capsule_at_its_trim_incidence_has_apollo_lift_to_drag() {
    // Apollo command module: L/D ≈ 0.3 at its hypersonic trim, α ≈ 20°
    // (flight: Hillje, "Entry Aerodynamics at Lunar Return Conditions
    // Obtained from the Flight of Apollo 4", NASA TN D-5399, 1969).
    let b = super::tests::capsule();
    for mach in [10.0, 30.0] {
        let a = 20f64.to_radians();
        let mut f = flow(DVec3::new(math::sin(a), 0.0, math::cos(a)), mach, 1e-5);
        f.speed = mach * 300.0;
        let force = aero_forces(b, &f, 0.8).0;
        let drag = force.dot(f.dir);
        let lift = (force - f.dir * drag).length();
        println!("capsule α 20°, M {mach}: L/D {:.3}", lift / drag);
        assert!((lift / drag - 0.3).abs() < 0.05, "{}", lift / drag);
    }
}
