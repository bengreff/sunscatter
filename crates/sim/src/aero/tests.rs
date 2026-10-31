use super::air::{cp_max, EARTH_AIR_GAMMA};
use super::*;
use crate::craft::cells::build_cells;
use crate::craft::mesh::union_surface;
use crate::craft::{test_craft, CellOptions, Cells, Primitive, Resolution, Shape, Skin, Surface};
use std::sync::OnceLock;

const SKIN: Skin = Skin {
    areal_mass: 8.1,
    specific_heat: 900.0,
    emissivity: 0.8,
    conductivity: 150.0,
    thickness: 0.003,
    max_k: None,
    coupling: None,
};

pub(crate) fn shape(shapes: &[Shape]) -> (Surface, Cells) {
    let prims: Vec<Primitive> = shapes
        .iter()
        .enumerate()
        .map(|(i, &shape)| Primitive { name: format!("p{i}"), shape, foot: false, skin: None, nozzle: false })
        .collect();
    let s = union_surface(&prims, &Resolution::default());
    let cells = build_cells(&s, &prims, &SKIN, &CellOptions::default());
    (s, cells)
}

/// A unit sphere at the origin (two hemispheres).
pub(super) fn sphere() -> &'static AeroBake {
    static B: OnceLock<AeroBake> = OnceLock::new();
    B.get_or_init(|| {
        let cap = |axis: DVec3| Shape::SphereCap { center: DVec3::ZERO, axis, radius: 1.0, height: 1.0 };
        let (s, c) = shape(&[cap(DVec3::Z), cap(-DVec3::Z)]);
        bake(&s, &c, &BakeOptions::default())
    })
}

/// Cone half-angle (rad) and base radius of the test cone.
const CONE_HALF_ANGLE: f64 = 20.0 * math::PI / 180.0;

/// A sharp cone (base radius 1 at z = 0, apex at +Z).
fn cone() -> &'static AeroBake {
    static B: OnceLock<AeroBake> = OnceLock::new();
    B.get_or_init(|| {
        let h = 1.0 / math::tan(CONE_HALF_ANGLE);
        let (s, c) = shape(&[Shape::Frustum { base: DVec3::ZERO, axis: DVec3::Z * h, r_base: 1.0, r_top: 0.002 }]);
        bake(&s, &c, &BakeOptions::default())
    })
}

/// The small rocket's bake (`craft::small`).
pub(crate) fn craft_bake() -> &'static AeroBake {
    static B: OnceLock<AeroBake> = OnceLock::new();
    let c = crate::craft::small::rocket_ref();
    B.get_or_init(|| bake(&c.surface, &c.cells, &BakeOptions::default()))
}

/// The finned test craft's bake: its passive stability and fins (D074),
/// which go with it.
pub(crate) fn finned_bake() -> &'static AeroBake {
    static B: OnceLock<AeroBake> = OnceLock::new();
    B.get_or_init(|| bake(&test_craft().surface, &test_craft().cells, &BakeOptions::default()))
}

pub(super) fn flow(dir: DVec3, mach: f64, knudsen: f64) -> Flow {
    Flow { dir: dir.normalize(), q: 1000.0, mach, knudsen, gamma: EARTH_AIR_GAMMA, ..Flow::default() }
}

/// Drag and side coefficients on area `a`.
fn coefficients(b: &AeroBake, f: &Flow, cd0: f64, a: f64) -> (f64, f64) {
    let (force, _) = aero_forces(b, f, cd0);
    let drag = force.dot(f.dir);
    (drag / (f.q * a), (force - f.dir * drag).length() / (f.q * a))
}

fn some_dirs() -> Vec<DVec3> {
    vec![DVec3::Z, -DVec3::X, DVec3::new(0.3, -0.5, 0.8), DVec3::new(-0.7, 0.1, -0.2)]
}

#[test]
fn sphere_newtonian_cd_is_half_cp_max() {
    let want = cp_max(1.4, 20.0) / 2.0;
    assert!((want - 0.918).abs() < 0.002);
    for d in some_dirs() {
        let (cd, side) = coefficients(sphere(), &flow(d, 20.0, 1e-5), 0.8, math::PI);
        assert!((cd / want - 1.0).abs() < 0.03, "{d}: Cd {cd} vs {want}");
        assert!(side < 0.01 * cd, "{d}: side {side}");
    }
}

#[test]
fn sphere_free_molecular_cd_is_two() {
    for d in some_dirs() {
        let (cd, side) = coefficients(sphere(), &flow(d, 20.0, 100.0), 0.8, math::PI);
        assert!((cd / 2.0 - 1.0).abs() < 0.05, "{d}: Cd {cd}");
        assert!(side < 1e-9, "{d}: side {side}");
    }
}

#[test]
fn subsonic_drag_is_cd0_on_the_projected_area() {
    for d in some_dirs() {
        let (cd, _) = coefficients(sphere(), &flow(d, 0.3, 1e-5), 0.47, math::PI);
        assert!((cd / 0.47 - 1.0).abs() < 0.03, "{d}: Cd {cd}");
    }
}

#[test]
fn sphere_nose_radius_is_its_radius() {
    for d in some_dirs() {
        let rn = sphere().sums_at(d).nose_radius;
        assert!((rn - 1.0).abs() < 0.05, "{d}: Rn {rn}");
    }
}

#[test]
fn cone_newtonian_forces_match_the_sin2_integral() {
    // Newtonian cone at incidence α ≤ δ, per Cp,max on the base area
    // (Anderson, Hypersonic and High-Temperature Gas Dynamics §3.3):
    // C_A = sin²δ + ½ sin²α (1 − 3 sin²δ), C_N = ½ cos²δ sin 2α.
    let (sd, cd) = (math::sin(CONE_HALF_ANGLE), math::cos(CONE_HALF_ANGLE));
    for alpha_deg in [0.0f64, 5.0, 10.0] {
        let a = alpha_deg.to_radians();
        let (sa, ca) = (math::sin(a), math::cos(a));
        // Nose-first: the air moves along −Z, tilted toward −X.
        let f = flow(DVec3::new(-sa, 0.0, -ca), 20.0, 1e-5);
        let (force, _) = aero_forces(cone(), &f, 0.8);
        let norm = f.q * cp_max(1.4, 20.0) * math::PI;
        let (axial, normal) = (-force.z / norm, -force.x / norm);
        let want_a = sd * sd + 0.5 * sa * sa * (1.0 - 3.0 * sd * sd);
        let want_n = 0.5 * cd * cd * math::sin(2.0 * a);
        assert!((axial / want_a - 1.0).abs() < 0.03, "α {alpha_deg}: C_A {axial} vs {want_a}");
        assert!((normal - want_n).abs() < 0.03 * want_a.max(want_n), "α {alpha_deg}: C_N {normal} vs {want_n}");
        assert!(force.y.abs() < 1e-3 * force.length(), "α {alpha_deg}: {force}");
    }
}

#[test]
fn symmetric_shapes_have_no_side_force_at_zero_incidence() {
    // Zero up to the cells' asymmetry (they are split at equal area, not
    // symmetrically): a larger share of the slender craft's small end-on
    // force.
    for (b, name, tol) in [(cone(), "cone", 5e-3), (craft_bake(), "craft", 2e-2)] {
        for d in [DVec3::Z, -DVec3::Z] {
            for (mach, kn) in [(0.5, 1e-5), (1.0, 1e-5), (3.0, 1e-5), (10.0, 1e-5), (10.0, 0.1), (10.0, 100.0)] {
                let (force, moment) = aero_forces(b, &flow(d, mach, kn), 0.8);
                let side = (force - d * force.dot(d)).length();
                assert!(side < tol * force.length(), "{name} {d} M {mach} Kn {kn}: {force}");
                let size = b.sections.length();
                assert!(moment.length() < tol * force.length() * size, "{name} {d} M {mach}: {moment}");
            }
        }
    }
}

#[test]
fn regimes_join_continuously() {
    let d = DVec3::new(0.2, -0.1, -1.0);
    for b in [cone(), craft_bake()] {
        for mach in [0.8, 1.2, HYPERSONIC_MACH] {
            let (lo, _) = aero_forces(b, &flow(d, mach - 1e-9, 1e-5), 0.8);
            let (hi, _) = aero_forces(b, &flow(d, mach + 1e-9, 1e-5), 0.8);
            assert!((lo - hi).length() < 1e-6 * lo.length(), "M {mach}: {lo} vs {hi}");
        }
        // Wilmoth: end values and midpoint.
        let (cont, _) = aero_forces(b, &flow(d, 20.0, 1e-4), 0.8);
        let (fm, _) = aero_forces(b, &flow(d, 20.0, 50.0), 0.8);
        let (mid, _) = aero_forces(b, &flow(d, 20.0, 0.1), 0.8);
        assert!((mid - (cont + fm) * 0.5).length() < 1e-9 * fm.length());
    }
}

#[test]
fn wilmoth_bridge_table() {
    // (Kn, weight): sin²[π(3/8 + ⅛ log₁₀ Kn)].
    let s2 = |x: f64| math::sin(x) * math::sin(x);
    let cases = [
        (1e-5, 0.0),
        (1e-3, 0.0),
        (1e-2, s2(math::PI / 8.0)),
        (0.1, 0.5),
        (1.0, s2(3.0 * math::PI / 8.0)),
        (10.0, 1.0),
        (1e4, 1.0),
    ];
    for (kn, want) in cases {
        assert!((bridge(kn) - want).abs() < 1e-12, "Kn {kn}: {}", bridge(kn));
    }
    assert_eq!(bridge(f64::INFINITY), 1.0);
}

/// Restoring or not: tilted 5° from the trim, the torque about the CoM
/// turns the upwind axis back toward the oncoming air.
fn restoring(b: &AeroBake, com: DVec3, trim_flow: DVec3, mach: f64, knudsen: f64) -> bool {
    restoring_from(b, com, trim_flow, (mach, knudsen), 5.0, 0.8)
}

/// [`restoring`] tilted `deg` degrees from the trim, with subsonic drag
/// coefficient `cd0`.
fn restoring_from(b: &AeroBake, com: DVec3, trim_flow: DVec3, (mach, knudsen): (f64, f64), deg: f64, cd0: f64) -> bool {
    let d = math::rotate_axis(trim_flow, DVec3::Y, deg.to_radians());
    let (force, moment) = aero_forces(b, &flow(d, mach, knudsen), cd0);
    let torque = moment - com.cross(force);
    // The upwind body axis â = −trim_flow turns as τ × â; the air comes from −d.
    torque.cross(-trim_flow).dot(-d) > 0.0
}

/// A blunt capsule: a spherical heat shield (radius 4.7 m, centre at
/// z = 4) facing −Z, and a conical afterbody to z = 2.8.
pub(super) fn capsule() -> &'static AeroBake {
    static B: OnceLock<AeroBake> = OnceLock::new();
    B.get_or_init(|| {
        let shield = Shape::SphereCap { center: DVec3::Z * 4.0, axis: -DVec3::Z, radius: 4.7, height: 0.5 };
        let after = Shape::Frustum { base: DVec3::Z * -0.2, axis: DVec3::Z * 3.0, r_base: 2.0, r_top: 0.6 };
        let (s, c) = shape(&[shield, after]);
        bake(&s, &c, &BakeOptions::default())
    })
}

#[test]
fn a_blunt_capsule_trims_heat_shield_first() {
    // Heat shield first (the air moves +Z): the shield's pressure passes
    // through its sphere centre (z = 4), behind any CoM inside the capsule:
    // stable. Apex first: the cone's normal force grows linearly with
    // incidence ahead of a forward CoM: unstable; with the CoM moved toward
    // the apex (z = 2) the apex-first trim becomes stable too (Apollo's
    // known second trim).
    let (forward, aft) = (DVec3::Z * 0.3, DVec3::Z * 2.0);
    for mach in [0.5, 2.0, 10.0] {
        assert!(restoring(capsule(), forward, DVec3::Z, mach, 1e-5), "shield-first, M {mach}, CoM forward");
    }
    // Apex first with the CoM forward: unstable in hypersonic flow. Below
    // M 5 the Cd₀ drag, scaled onto the cone's small Newtonian drag, acts
    // on its windward side and stabilises this trim too (not asserted: the
    // sub/supersonic model is crude until the area-distribution step).
    assert!(!restoring(capsule(), forward, -DVec3::Z, 10.0, 1e-5), "apex-first, CoM forward");
    // Hypersonic (pure Newtonian: the shield's force passes through its
    // centre exactly); below M 5 drag and lift scale differently and the
    // margin of an aft CoM shrinks.
    assert!(restoring(capsule(), aft, DVec3::Z, 10.0, 1e-5), "shield-first, CoM aft");
    assert!(restoring(capsule(), aft, -DVec3::Z, 10.0, 1e-5), "apex-first, CoM aft");
}

#[test]
fn the_test_craft_is_stable_nose_first_at_every_mach_number() {
    // D074. Below M 5 the fins' lift and the skirt's area growth put the
    // centre of pressure behind the CoM; hypersonic (Newtonian) the skirt's
    // normal force, linear in incidence at the base, outweighs the nose
    // cone's; free molecular the side area behind the CoM (fins, skirt)
    // does. Full and empty, at small and large incidence (up to 60°, where
    // the side of the body is broadside and the fins have stalled), the
    // torque turns the nose back into the air.
    let c = test_craft();
    for propellant in [0.0, c.spec.propellant.capacity] {
        let com = c.mass.at(propellant).com;
        let regimes = [0.3, 0.9, 1.2, 2.0, 5.0, 10.0, 25.0].map(|m| (m, 1e-5)).into_iter().chain([(25.0, 100.0)]);
        for (mach, kn) in regimes {
            for deg in [2.0, 5.0, 20.0, 60.0] {
                assert!(
                    restoring_from(finned_bake(), com, -DVec3::Z, (mach, kn), deg, c.spec.aero.cd0),
                    "M {mach} Kn {kn} at {deg}°, CoM {com}"
                );
            }
        }
        // Tail first is the unstable trim (it turns around).
        for mach in [0.9, 2.0, 10.0] {
            let tail_first = restoring_from(finned_bake(), com, DVec3::Z, (mach, 1e-5), 5.0, c.spec.aero.cd0);
            assert!(!tail_first, "tail first, M {mach}, CoM {com}");
        }
    }
}

#[test]
fn the_nozzles_shadow_the_base_when_flying_base_first() {
    // The small rocket's base: the thrust structure's bottom disc at z = −5
    // (r 1 m, less the four powerheads' 0.15 m footprints), under four
    // nozzle exits (r 0.2 m, 0.8 m below it, 0.6 m off the axis). Base
    // first, each exit shadows the ring r 0.15–0.2 m round its powerhead.
    let b = craft_bake();
    let mut f = vec![0.0; b.cells];
    b.exposure_at(DVec3::Z, &mut f);
    let c = crate::craft::small::rocket_ref();
    let named = |i: usize, name: &str| c.geometry.primitives[c.cells.cells[i].primitive as usize].name == name;
    let (mut area, mut exposed, mut rings) = (0.0, 0.0, 0);
    for (i, cell) in c.cells.cells.iter().enumerate() {
        let down = cell.normal.dot(-DVec3::Z) > 0.99;
        if named(i, "thrust structure") && down && (cell.centroid.z + 5.0).abs() < 1e-6 {
            area += cell.area;
            exposed += cell.area * f[i];
        }
        // The powerheads' bottom rings, round the nozzles' throats: in
        // their exits' shadow.
        if named(i, "powerhead") && down {
            assert!(f[i] < 0.05, "cell {i}: {}", f[i]);
            rings += 1;
        }
        // Back faces are never exposed.
        if cell.normal.dot(DVec3::Z) > 0.0 {
            assert_eq!(f[i], 0.0);
        }
    }
    assert_eq!(rings, 4);
    let pi = std::f64::consts::PI;
    let base = pi * (1.0 - 4.0 * 0.15f64.powi(2));
    assert!((area / base - 1.0).abs() < 0.02, "{area} vs {base}");
    let want = 1.0 - 4.0 * pi * (0.2f64.powi(2) - 0.15f64.powi(2)) / base;
    assert!((exposed / area / want - 1.0).abs() < 0.02, "{} vs {want}", exposed / area);
    // Nose first, the stagnation region is the capsule's flat top (r 0.4
    // m: a face without a smooth stagnation region takes its cell's
    // size), not the tank's shoulder 1.5 m downstream that faces the flow
    // too.
    let rn = b.sums_at(-DVec3::Z).nose_radius;
    assert!((rn / 0.4 - 1.0).abs() < 0.1, "Rn {rn}");
}

#[test]
fn flat_faces_take_zoby_and_sullivan_s_effective_nose_radius() {
    // NASA TM X-1067 fig. 4: a flat face with a sharp corner has the
    // stagnation velocity gradient of a hemisphere of 3.45 times its
    // radius; a spherical segment with r_B/r_N 0.4 that of 2 r_B; from
    // about 0.6 the sphere's own radius.
    assert!((bake::effective_nose_radius(1.0, f64::INFINITY) / 3.448 - 1.0).abs() < 1e-3);
    assert!((bake::effective_nose_radius(1.0, 2.5) - 2.0).abs() < 1e-12);
    assert!((bake::effective_nose_radius(0.6, 1.0) - 1.0).abs() < 1e-12);
    assert_eq!(bake::effective_nose_radius(0.5, 0.5), 0.5);
    // A flat-faced cylinder (radius 1 m) end on: ≈ 3.45 m; the capsule's
    // shield (r_N 4.7 m over a 2.1 m disc): ≈ 3.9 m.
    let (s, c) =
        shape(&[Shape::Cylinder { base: DVec3::new(0.0, 0.0, -1.0), axis: DVec3::new(0.0, 0.0, 2.0), radius: 1.0 }]);
    let b = bake(&s, &c, &BakeOptions::default());
    let rn = b.nose_radius_at(DVec3::Z);
    assert!((rn / 3.45 - 1.0).abs() < 0.05, "{rn}");
    let rn = capsule().nose_radius_at(DVec3::Z);
    assert!(rn > 3.6 && rn < 4.2, "{rn}");
    // A sphere: its own radius.
    assert!((sphere().nose_radius_at(DVec3::Z) - 1.0).abs() < 0.05);
}

#[test]
fn the_knudsen_length_is_the_maximum_body_diameter() {
    // Wilmoth's Kn = λ/D_b (AIAA 97-2510): a sphere's diameter, a capsule's
    // heat-shield diameter (2·√(4.7² − 4.2²) = 4.22 m), the slender small
    // rocket's broadside silhouette (7.3 m long, 2.4 m across its stowed
    // legs).
    assert!((sphere().length / 2.0 - 1.0).abs() < 0.01, "{}", sphere().length);
    let shield = 2.0 * (4.7f64 * 4.7 - 4.2 * 4.2).sqrt();
    assert!((capsule().length / shield - 1.0).abs() < 0.02, "{}", capsule().length);
    let craft = craft_bake().length;
    println!("small rocket: D_b {craft:.2} m");
    assert!(craft > 2.4 && craft < 7.3, "{craft}");
}

#[test]
fn the_bake_is_deterministic() {
    let cap = |axis: DVec3| Shape::SphereCap { center: DVec3::ZERO, axis, radius: 1.0, height: 1.0 };
    let (s, c) = shape(&[cap(DVec3::Z), cap(-DVec3::Z)]);
    let again = bake(&s, &c, &BakeOptions::default());
    assert_eq!(again.hash(), sphere().hash());
    assert_eq!(sphere().hash(), SPHERE_BAKE_HASH, "golden: {:#x}", sphere().hash());
}

/// Golden hash of the unit-sphere bake (identical on every platform).
/// 2026-10-26: the Knudsen length became the maximum body diameter (was
/// the bounding-sphere diameter); then the layer's 3D relief was baked;
/// then the area and perimeter distributions came from the cells.
const SPHERE_BAKE_HASH: u64 = 0xa09c_5df9_19c5_7322;

/// A thin flat plate, `l` long along X, 2 m wide, 4 mm thick.
fn plate(l: f64) -> AeroBake {
    let (s, c) = shape(&[Shape::Box { center: DVec3::ZERO, half: DVec3::new(0.5 * l, 1.0, 0.002) }]);
    bake(&s, &c, &BakeOptions { level: 1, raster: 32 })
}

/// The free stream of a plate test: Re per metre, Mach, 216.65 K, no
/// impact pressure (the plate is along the flow).
fn stream(re_per_m: f64, mach: f64) -> boundary::Stream {
    boundary::Stream::new(re_per_m, mach, 1.4, 216.65, 0.0)
}

/// Mean friction coefficient of the plate (both faces) at `re_l` along X.
fn plate_cf(b: &AeroBake, l: f64, re_l: f64, mach: f64) -> f64 {
    let f = friction::friction(&b.geometry, DVec3::X, &stream(re_l / l, mach), |_| 1.0, |_| (1.0, 1.0), None);
    f.0.x / (2.0 * l * 2.0)
}

#[test]
fn flat_plate_friction_matches_blasius_and_the_turbulent_plate() {
    let l = 4.0;
    let b = plate(l);
    // Laminar (Blasius): C_f = 1.328/√Re_L; within 8 % (the cells near
    // the leading edge, where c_f ∝ x^−½, are not rectangles).
    for re in [1e4, 1e5, 4e5] {
        let (got, want) = (plate_cf(&b, l, re, 0.0), 1.328 / re.sqrt());
        println!("Re {re:e}: C_f {got:.5} (Blasius {want:.5})");
        assert!((got / want - 1.0).abs() < 0.08, "Re {re}: {got} vs {want}");
    }
    // Transition from Re_x 5·10⁵ (Re_θ 469.5) to 10⁶, turbulent after:
    // against the plate's mean from the local laws (Blasius, then White's
    // c_f = 0.455/ln²(0.06·Re_x), Viscous Fluid Flow, 3rd ed., eq. 6-78,
    // blended by the Dhawan–Narasimha intermittency), integrated here; and
    // against the published closed forms: Prandtl–Schlichting's mixed
    // plate 0.455/(log₁₀Re)^2.58 − 1700/Re for transition at 5·10⁵
    // (Schlichting, Boundary-Layer Theory, ch. 21, from memory), and
    // White's fully turbulent 0.523/ln²(0.06·Re) (eq. 6-79), where the
    // laminar start is small.
    let mean_of_laws = |re: f64| {
        let n = 40_000;
        let h = math::ln(re) / n as f64;
        // ∫ c_f dRe over [1, re] in ln Re (midpoint rule).
        let sum: f64 = (0..n)
            .map(|k| {
                let r = math::exp((k as f64 + 0.5) * h);
                let t = 5.0e5;
                let g = if r <= t {
                    0.0
                } else {
                    let xi = 3.343 * (r - t) / t;
                    1.0 - math::exp(-0.412 * xi * xi)
                };
                ((1.0 - g) * 0.664 / r.sqrt() + g * boundary::cf_turbulent(r)) * r * h
            })
            .sum();
        sum / re
    };
    for re in [3e6, 1e7, 3e7, 1e8] {
        let got = plate_cf(&b, l, re, 0.0);
        let laws = mean_of_laws(re);
        let lg = math::ln(re) / math::ln(10.0);
        let schlichting = 0.455 / math::exp(2.58 * math::ln(lg)) - 1700.0 / re;
        let white = 0.523 / (math::ln(0.06 * re) * math::ln(0.06 * re));
        println!("Re {re:e}: C_f {got:.5} (laws {laws:.5}, Prandtl–Schlichting {schlichting:.5}, White {white:.5})");
        assert!((got / laws - 1.0).abs() < 0.03, "Re {re}: {got} vs {laws}");
        assert!((got / schlichting - 1.0).abs() < 0.05, "Re {re}: {got} vs {schlichting}");
        if re >= 3e7 {
            assert!((got / white - 1.0).abs() < 0.02, "Re {re}: {got} vs {white}");
        }
    }
}

#[test]
fn compressible_turbulent_friction_falls_like_van_driest() {
    // Adiabatic wall, Re 10⁷, T 216.65 K: Van Driest II by hand (F =
    // 1 + 0.89·0.2·M²; A = √((F−1)/F); F_c = (F−1)/asin²A; F_Rx =
    // (μ/μ_w)/F_c): M 2: F_c 1.451, F_Rx 0.453, C_f/C_f,inc 0.779; M 5:
    // F_c 3.49, F_Rx 0.089, 0.428. Hopkins & Inouye (AIAA J. 9(6), 1971)
    // found Van Driest II within about ±10 % of flat-plate and cone data
    // to M 7.4.
    let p = |m: f64| {
        let e = stream(1e7, m).edge(0.0, 0.0);
        let (inv_fc, frx) = e.van_driest(e.recovery(boundary::TURBULENT_RECOVERY, 216.65), 216.65);
        boundary::cf_turbulent(frx * 1e7) * inv_fc
    };
    for (m, want) in [(2.0, 0.779), (5.0, 0.427)] {
        let ratio = p(m) / p(0.0);
        println!("M {m}: C_f/C_f,inc {ratio:.3}");
        assert!((ratio / want - 1.0).abs() < 0.01, "M {m}: {ratio}");
    }
    assert!(p(5.0) < p(3.0) && p(3.0) < p(2.0));
    assert!((p(0.0) / boundary::cf_turbulent(1e7) - 1.0).abs() < 1e-12);
    // The layer at M 2, Re_x 10⁷ is fully turbulent (Re_θ ≫ 350·M_e) and
    // takes that coefficient; at M 5 it is too, past a sharp cone's onset in
    // flight (Re_x 3·10⁶, Softley); at M 5, Re_x 2·10⁶ (Re_θ ≈ 730, under
    // 350·5) it is laminar.
    let layer = |m: f64, re: f64| stream(re, m).edge(0.0, 0.0).layer(1.0, 1.0, None, 216.65, false);
    assert!((layer(2.0, 1e7).shear / p(2.0) - 1.0).abs() < 1e-12 && layer(2.0, 1e7).intermittency == 1.0);
    assert!((layer(5.0, 1e7).shear / p(5.0) - 1.0).abs() < 1e-9);
    assert_eq!(layer(5.0, 2e6).intermittency, 0.0);
}

#[test]
fn a_blunt_base_carries_the_published_base_pressure() {
    // A cylinder (radius 0.5 m, 6 m) nose first: its base disc (0.785 m²)
    // takes −C_pb(M)·q·A over the pressure model, which holds the subsonic
    // 0.12 in Cd₀ and fades it into Newtonian (no base suction) by M 5.
    let (s, c) =
        shape(&[Shape::Cylinder { base: DVec3::new(0.0, 0.0, -3.0), axis: DVec3::new(0.0, 0.0, 6.0), radius: 0.5 }]);
    let b = bake(&s, &c, &BakeOptions { level: 2, raster: 32 });
    let d = -DVec3::Z;
    let sums = b.sums_at(d);
    let disc = math::PI * 0.25;
    // The disc's cells: a polygon, 0.5 % short of the circle.
    assert!((sums.base.dot(d) / disc - 1.0).abs() < 0.01, "{}", sums.base);
    assert!(sums.base.truncate().length() < 1e-9 && sums.base_moment.length() < 1e-9);
    let disc = sums.base.dot(d);
    for mach in [0.3, 0.9, 1.0, 2.0, 3.0, 5.0, 8.0] {
        let f = flow(d, mach, 1e-5);
        let added = (continuum(&b, &sums, d, &f, 0.8).0 - pressure(&b, &sums, d, &f, 0.8).0).dot(d) / disc;
        // Subsonic Cd₀ (0.8, form and base) holds the 0.12.
        let total = added + base::SUBSONIC * base::subsonic_share(mach);
        println!("M {mach}: base C_D {total:.3} (−C_pb {:.3})", base::pressure(mach));
        assert!((total - base::pressure(mach)).abs() < 1e-9, "M {mach}: {total}");
    }
    // Along the base's normal, not the flow: at incidence it adds no
    // normal force.
    let tilted = DVec3::new(0.1, 0.0, -1.0).normalize();
    let (st, f) = (b.sums_at(tilted), flow(tilted, 2.0, 1e-5));
    let added = continuum(&b, &st, tilted, &f, 0.8).0 - pressure(&b, &st, tilted, &f, 0.8).0;
    assert!(added.truncate().length() < 1e-9 * added.length(), "{added}");
    // The transonic peak above both ends.
    assert!(base::pressure(1.05) > 1.5 * base::pressure(0.6) && base::pressure(1.05) > 4.0 * base::pressure(5.0));
}

#[test]
fn a_tick_s_boundary_layer_gives_the_same_forces() {
    // The small rocket at M 1.2, 12 km (turbulent aft), skin at 300–600 K:
    // the layer computed once gives aero_forces_wall's bits at its own
    // flow, and within 0.1 % at a flow 2 mrad away (an integration stage).
    let c = crate::craft::small::rocket_ref();
    let wall: Vec<f64> = (0..c.cells.cells.len()).map(|i| 300.0 + (i % 7) as f64 * 50.0).collect();
    let mut f = flow(DVec3::new(0.05, 0.02, -1.0), 1.2, 1e-5);
    (f.reynolds_per_m, f.temperature, f.speed) = (6.4e6, 216.65, 354.0);
    let b = craft_bake();
    let layer = friction::FlowLayer::new(b, &f, Some(&wall), false);
    assert_eq!(aero_forces_layer(b, &f, 0.8, &layer), aero_forces_wall(b, &f, 0.8, Some(&wall)));
    let mut g = f;
    g.dir = math::rotate_axis(f.dir.normalize(), DVec3::X, 2e-3);
    let (exact, reused) = (aero_forces_wall(b, &g, 0.8, Some(&wall)), aero_forces_layer(b, &g, 0.8, &layer));
    assert!((exact.0 - reused.0).length() < 1e-3 * exact.0.length(), "{exact:?} {reused:?}");
    assert!((exact.1 - reused.1).length() < 1e-3 * exact.1.length(), "{exact:?} {reused:?}");
}

#[test]
fn friction_acts_along_the_flow_and_not_on_the_base() {
    // A cylinder along the flow: friction on its side only (its upstream
    // face has no flow along it, the downstream face is separated).
    let (s, c) =
        shape(&[Shape::Cylinder { base: DVec3::new(0.0, 0.0, -3.0), axis: DVec3::new(0.0, 0.0, 6.0), radius: 0.5 }]);
    let b = bake(&s, &c, &BakeOptions { level: 1, raster: 32 });
    let p = boundary::Stream::new(2e6, 0.5, 1.4, 288.15, 0.0);
    let (f, m) = friction::friction(&b.geometry, DVec3::Z, &p, |_| 1.0, |_| (1.0, 1.0), None);
    assert!(f.z > 0.0 && f.truncate().length() < 1e-3 * f.z, "{f}");
    assert!(m.length() < 1e-3 * f.z * 6.0, "{m}");
    // About the flat plate's mean over the side (6 m run, πD·L wetted).
    let side = math::PI * 1.0 * 6.0;
    let flat = plate_cf(&plate(6.0), 6.0, 2e6 * 6.0, 0.5);
    assert!((f.z / (side * flat) - 1.0).abs() < 0.05, "{} vs {flat}", f.z / side);
}
