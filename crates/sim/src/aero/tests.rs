use super::air::{cp_max, EARTH_AIR_GAMMA};
use super::*;
use crate::craft::cells::build_cells;
use crate::craft::mesh::union_surface;
use crate::craft::{test_craft, CellOptions, Cells, Primitive, Resolution, Shape, Skin, Surface};
use std::sync::OnceLock;

const SKIN: Skin =
    Skin { areal_mass: 8.1, specific_heat: 900.0, emissivity: 0.8, conductivity: 150.0, thickness: 0.003 };

fn shape(shapes: &[Shape]) -> (Surface, Cells) {
    let prims: Vec<Primitive> = shapes
        .iter()
        .enumerate()
        .map(|(i, &shape)| Primitive { name: format!("p{i}"), shape, foot: false, skin: None })
        .collect();
    let s = union_surface(&prims, &Resolution::default());
    let cells = build_cells(&s, &prims, &SKIN, &CellOptions::default());
    (s, cells)
}

/// A unit sphere at the origin (two hemispheres).
fn sphere() -> &'static AeroBake {
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

fn craft_bake() -> &'static AeroBake {
    static B: OnceLock<AeroBake> = OnceLock::new();
    B.get_or_init(|| bake(&test_craft().surface, &test_craft().cells, &BakeOptions::default()))
}

fn flow(dir: DVec3, mach: f64, knudsen: f64) -> Flow {
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
    for (b, name) in [(cone(), "cone"), (craft_bake(), "craft")] {
        for d in [DVec3::Z, -DVec3::Z] {
            for (mach, kn) in [(0.5, 1e-5), (1.0, 1e-5), (3.0, 1e-5), (10.0, 1e-5), (10.0, 0.1), (10.0, 100.0)] {
                let (force, moment) = aero_forces(b, &flow(d, mach, kn), 0.8);
                let side = (force - d * force.dot(d)).length();
                // Zero up to the cells' asymmetry (they are split at equal
                // area, not symmetrically).
                assert!(side < 5e-3 * force.length(), "{name} {d} M {mach} Kn {kn}: {force}");
                assert!(moment.length() < 5e-3 * force.length() * b.length, "{name} {d} M {mach}: {moment}");
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
    let d = math::rotate_axis(trim_flow, DVec3::Y, 5.0f64.to_radians());
    let (force, moment) = aero_forces(b, &flow(d, mach, knudsen), 0.8);
    let torque = moment - com.cross(force);
    // The upwind body axis â = −trim_flow turns as τ × â; the air comes from −d.
    torque.cross(-trim_flow).dot(-d) > 0.0
}

/// A blunt capsule: a spherical heat shield (radius 4.7 m, centre at
/// z = 4) facing −Z, and a conical afterbody to z = 2.8.
fn capsule() -> &'static AeroBake {
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
fn the_test_craft_is_unstable_nose_first_in_continuum_flow() {
    // Nose first, the capsule cone ahead of the CoM carries a normal force
    // linear in incidence (the cylinder's only quadratic): the centre of
    // pressure is ahead of the CoM, full or empty. In free-molecular flow
    // the force is drag along the flow at the projected area's centroid,
    // which moves aft as the body's side comes into view: stable.
    let c = test_craft();
    for propellant in [0.0, c.spec.propellant.capacity] {
        let com = c.mass.at(propellant).com;
        for mach in [0.5, 2.0, 10.0] {
            assert!(!restoring(craft_bake(), com, -DVec3::Z, mach, 1e-5), "M {mach}, CoM {com}");
        }
        assert!(restoring(craft_bake(), com, -DVec3::Z, 10.0, 100.0), "free molecular, CoM {com}");
    }
}

#[test]
fn the_bell_shadows_the_base_when_flying_base_first() {
    let b = craft_bake();
    let mut f = vec![0.0; b.cells];
    b.exposure_at(DVec3::Z, &mut f);
    let cells = &test_craft().cells.cells;
    let (mut shadowed, mut open) = (0, 0);
    for (i, c) in cells.iter().enumerate() {
        let base = c.normal.dot(-DVec3::Z) > 0.99 && (c.centroid.z + 3.2).abs() < 1e-6;
        let r = c.centroid.truncate().length();
        let off_legs = c.centroid.x.abs().min(c.centroid.y.abs()) > 0.4;
        if base && r > 0.35 && r < 0.65 {
            assert!(f[i] < 0.25, "cell {i} at r {r}: {}", f[i]);
            shadowed += 1;
        }
        if base && r > 1.1 && off_legs {
            assert!(f[i] > 0.75, "cell {i} at r {r}: {}", f[i]);
            open += 1;
        }
        // Back faces are never exposed.
        if c.normal.dot(DVec3::Z) > 0.0 {
            assert_eq!(f[i], 0.0);
        }
    }
    assert!(shadowed > 0 && open > 0, "{shadowed} {open}");
    // Nose first, the stagnation region is the nose cap (radius 0.875 m).
    let rn = b.sums_at(-DVec3::Z).nose_radius;
    assert!((rn / 0.875 - 1.0).abs() < 0.1, "Rn {rn}");
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
const SPHERE_BAKE_HASH: u64 = 0x2ee8_960b_0b14_a000;

/// A thin flat plate, `l` long along X, 2 m wide, 4 mm thick.
fn plate(l: f64) -> AeroBake {
    let (s, c) = shape(&[Shape::Box { center: DVec3::ZERO, half: DVec3::new(0.5 * l, 1.0, 0.002) }]);
    bake(&s, &c, &BakeOptions { level: 1, raster: 32 })
}

/// Mean friction coefficient of the plate (both faces) at `re_l` along X.
fn plate_cf(b: &AeroBake, l: f64, re_l: f64, mach: f64) -> f64 {
    let f = friction::friction(&b.geometry, DVec3::X, &friction::Plate::new(re_l / l, mach, 1.4, 216.65));
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
    // Laminar to 5·10⁵, turbulent after: against the plate's mean from
    // the local laws (Blasius, then White's c_f = 0.455/ln²(0.06·Re_x),
    // Viscous Fluid Flow, 3rd ed., eq. 6-78), integrated here; and against
    // the published closed forms: Prandtl's mixed plate 0.074·Re^−⅕ −
    // 1742/Re (Schlichting, Boundary-Layer Theory, ch. 21; its ⅕-power law
    // holds to ~10⁷) and White's fully turbulent 0.523/ln²(0.06·Re) (eq.
    // 6-79), where the laminar start is small.
    let mean_of_laws = |re: f64| {
        let tr = friction::TRANSITION_RE;
        let n = 20_000;
        let h = (math::ln(re) - math::ln(tr)) / n as f64;
        // ∫ c_f dRe over [tr, re] in ln Re (midpoint rule).
        let turb: f64 = (0..n)
            .map(|k| {
                let r = math::exp(math::ln(tr) + (k as f64 + 0.5) * h);
                friction::cf_incompressible(r) * r * h
            })
            .sum();
        (1.328 * tr.sqrt() + turb) / re
    };
    for re in [3e6, 1e7, 3e7, 1e8] {
        let got = plate_cf(&b, l, re, 0.0);
        let laws = mean_of_laws(re);
        let prandtl = 0.074 / math::exp(0.2 * math::ln(re)) - 1742.0 / re;
        let white = 0.523 / (math::ln(0.06 * re) * math::ln(0.06 * re));
        println!("Re {re:e}: C_f {got:.5} (laws {laws:.5}, Prandtl {prandtl:.5}, White {white:.5})");
        assert!((got / laws - 1.0).abs() < 0.02, "Re {re}: {got} vs {laws}");
        if re <= 1e7 {
            assert!((got / prandtl - 1.0).abs() < 0.04, "Re {re}: {got} vs {prandtl}");
        }
        if re >= 3e7 {
            assert!((got / white - 1.0).abs() < 0.03, "Re {re}: {got} vs {white}");
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
    let p = |m: f64| friction::Plate::new(1e7, m, 1.4, 216.65).local(1.0);
    for (m, want) in [(2.0, 0.779), (5.0, 0.427)] {
        let ratio = p(m) / p(0.0);
        println!("M {m}: C_f/C_f,inc {ratio:.3}");
        assert!((ratio / want - 1.0).abs() < 0.01, "M {m}: {ratio}");
    }
    assert!(p(5.0) < p(3.0) && p(3.0) < p(2.0));
    // The incompressible local law.
    assert!((p(0.0) - friction::cf_incompressible(1e7)).abs() < 1e-15);
}

#[test]
fn friction_acts_along_the_flow_and_not_on_the_base() {
    // A cylinder along the flow: friction on its side only (its upstream
    // face has no flow along it, the downstream face is separated).
    let (s, c) =
        shape(&[Shape::Cylinder { base: DVec3::new(0.0, 0.0, -3.0), axis: DVec3::new(0.0, 0.0, 6.0), radius: 0.5 }]);
    let b = bake(&s, &c, &BakeOptions { level: 1, raster: 32 });
    let p = friction::Plate::new(2e6, 0.5, 1.4, 288.15);
    let (f, m) = friction::friction(&b.geometry, DVec3::Z, &p);
    assert!(f.z > 0.0 && f.truncate().length() < 1e-3 * f.z, "{f}");
    assert!(m.length() < 1e-3 * f.z * 6.0, "{m}");
    // About the flat plate's mean over the side (6 m run, πD·L wetted).
    let side = math::PI * 1.0 * 6.0;
    let flat = plate_cf(&plate(6.0), 6.0, 2e6 * 6.0, 0.5);
    assert!((f.z / (side * flat) - 1.0).abs() < 0.05, "{} vs {flat}", f.z / side);
}
