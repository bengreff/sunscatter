use super::zonal::{j2_accel, j4_j6_accel};
use super::*;
use crate::body::{self, Rotation};
use crate::frame::{BodyFixed, Vec3};

const GM: f64 = 4.9028e12;
const R: f64 = 1.738e6;

/// A synthetic field whose coefficients are exact in IEEE arithmetic, so
/// the independent evaluations see the same bits.
fn synthetic(n_max: usize) -> Field {
    let mut c = Vec::new();
    for n in 2..=n_max {
        let k = 1e-5 / (n * n) as f64;
        for m in 0..=n {
            let cc = ((n * 131 + m * 71) % 997) as f64;
            let ss = ((n * 37 + m * 113) % 991) as f64;
            let s = if m > 0 { (ss - 495.0) / 495.0 * k } else { 0.0 };
            c.push(Coefficient { n, m, c: (cc - 498.0) / 498.0 * k, s });
        }
    }
    Field::new("synthetic", R, Some(GM), &c, None).unwrap()
}

/// (degree, position (m), acceleration (m/s²), potential (m²/s²)) of the
/// synthetic field above (GM 4.9028e12, R 1738 km) from independent
/// evaluations: pyshtools 4.14 (`gravmag.MakeGravGridPoint`, and
/// `expand.MakeGridPoint` with the coefficients scaled by (R/r)^n for the
/// potential) at (lat°, lon°, r/R) = (10, 20, 1.05), (−63, 211, 1.3),
/// (−0.3, −170, 2.7), (45, 90, 1) and, at degree 300, (1, 33, 1.002),
/// (−71.5, 300, 1.002), (37, 181, 1.1). Within 0.1° of a pole pyshtools
/// divides by the sine of the colatitude (1e-12 off), so there the degree-12
/// point (89.9, 45, 1.01) is a 40-digit mpmath sum (exact Legendre
/// derivatives, the gradient by mpmath.diff), and the degree-300 points
/// (89.95, 10, 1.01), (−89.9, 123, 1.05) pyshtools through coefficients
/// rotated 90° about y (`SHCoeffs.rotate`, good to ~1e-14 against the
/// mpmath point). Scripts in the commit message.
#[allow(clippy::type_complexity, clippy::excessive_precision)]
const INDEPENDENT: [(usize, [f64; 3], [f64; 3], f64); 10] = [
    (
        12,
        [1.68879271391910035e+06, 6.14670279712190502e+05, 3.16890559424381179e+05],
        [-2.92909316012923712e-06, -5.67576235996062460e-06, -9.81920130131767875e-07],
        4.15845876690623839e-01,
    ),
    (
        12,
        [-8.79236045839176280e+05, -5.28298314774271799e+05, -2.01314014075119840e+06],
        [-1.90141697662717547e-06, -4.63652822683356078e-07, -4.76105421280832731e-06],
        -4.53958261222534531e+00,
    ),
    (
        12,
        [2.16637330347014949e+03, 2.16637330347014949e+03, 1.75537732640404697e+06],
        [4.1440309148086701e-6, -2.7439322218096754e-5, 1.2904262507900505e-5],
        -7.12209125171794000e+00,
    ),
    (
        12,
        [-4.62124551402664557e+06, -8.14850268600856303e+05, -2.45702838753195210e+04],
        [-4.28021811990504429e-08, 9.06126486449790469e-08, 1.32132277105995460e-07],
        -4.45830260865508268e-02,
    ),
    (
        12,
        [7.52515812868643754e-11, 1.22895158570221975e+06, 1.22895158570221951e+06],
        [-3.47106897553607968e-05, 4.23040949363944258e-05, -6.91157616450220199e-06],
        -1.31373725996337445e+01,
    ),
    (
        300,
        [1.46030222100248607e+06, 9.48331350682203658e+05, 3.03929469527747424e+04],
        [-3.23785182149758212e-05, 1.95137961869558095e-07, 1.47874416538790121e-06],
        1.10592271891417826e+00,
    ),
    (
        300,
        [2.76289221908857115e+05, -4.78546969929812592e+05, -1.65148288577387109e+06],
        [9.89345956025194578e-06, -7.63435918440254931e-07, -4.69961329739551285e-06],
        -6.83893915677221464e+00,
    ),
    (
        300,
        [1.50858547414734517e+03, 2.66004321797049386e+02, 1.75537933160088444e+06],
        [4.34766482479977463e-06, -1.93930148263308086e-05, 7.57464646594476336e-06],
        -6.39516349413967244e+00,
    ),
    (
        300,
        [-1.73470241410591098e+03, 2.67120747012626816e+03, -1.82489722051905864e+06],
        [5.46713246339976001e-06, 5.89062780430743192e-06, -1.40104342301607529e-05],
        -6.56107492789285107e+00,
    ),
    (
        300,
        [-1.52659882429567794e+06, -2.66468815974218778e+04, 1.15054996126208594e+06],
        [1.02415226907234289e-05, -3.95375398614447153e-06, -6.97735676047313752e-06],
        4.43640138893815461e+00,
    ),
];

#[test]
fn matches_an_independent_evaluation() {
    let (low, high) = (synthetic(12), synthetic(300));
    let mut worst: f64 = 0.0;
    for (n, x, a_ref, u_ref) in INDEPENDENT {
        let field = if n == 12 { &low } else { &high };
        let (a, u) = field.accel_in_field_frame(GM, DVec3::from(x), n);
        let a_ref = DVec3::from(a_ref);
        let err = (a - a_ref).length() / a_ref.length();
        worst = worst.max(err);
        assert!(err < 5e-14, "degree {n} at {x:?}: {a} vs {a_ref} ({err:e})");
        assert!((u - u_ref).abs() < 5e-14 * u_ref.abs(), "degree {n} potential {u} vs {u_ref}");
    }
    println!("worst relative difference: {worst:e}"); // 2.0e-14 (degree 300), 1.7e-15 (degree 12)
}

/// The acceleration is the gradient of the potential (central differences),
/// including at a pole and below the reference radius.
#[test]
fn acceleration_is_the_gradient_of_the_potential() {
    let field = synthetic(40);
    for x in [DVec3::new(1.2e6, -0.9e6, 0.7e6), DVec3::new(0.0, 0.0, 1.8e6), DVec3::new(-1.0e6, 1.4e6, -0.2e6)] {
        let (a, _) = field.accel_in_field_frame(GM, x, 40);
        let h = 1.0;
        let pot = |d: DVec3| field.accel_in_field_frame(GM, x + d * h, 40).1;
        let g =
            DVec3::new(pot(DVec3::X) - pot(-DVec3::X), pot(DVec3::Y) - pot(-DVec3::Y), pot(DVec3::Z) - pot(-DVec3::Z))
                / (2.0 * h);
        assert!((a - g).length() < 1e-7 * a.length(), "{a} vs {g} at {x}");
    }
}

/// The degree-2 zonal field is the closed form the generator uses, and a
/// zonal J2/J4/J6 field the closed J4/J6 terms, to rounding.
#[test]
fn closed_forms_match_the_field() {
    let (gm, rad, j2, j4, j6) = (1.266_865_3e17, 7.1492e7, 1.4697e-2, -5.866e-4, 3.42e-5);
    let zonal = |n: usize, j: f64| Coefficient { n, m: 0, c: -j / ((2 * n + 1) as f64).sqrt(), s: 0.0 };
    let f2 = Field::j2(j2, rad);
    let f246 = Field::new("zonal", rad, None, &[zonal(2, j2), zonal(4, j4), zonal(6, j6)], None).unwrap();
    let rot = Rotation { ra: 0.3, dec: 1.1, ..body::earth().rotation };
    let t = crate::time::Epoch::J2000;
    let pole = rot.pole(t);
    for d in [DVec3::new(4.2e8, 1.0e7, 3.0e7), DVec3::new(1.0e8, -5.0e7, 6.0e7), DVec3::new(0.0, 0.0, 9.0e7)] {
        let closed = j2_accel(d, gm, j2, rad, pole);
        let (a, _) = f2.accel(gm, d, &rot, t, 2, false);
        assert!((a - closed).length() < 1e-14 * closed.length(), "J2 {a} vs {closed}");
        let closed = closed + j4_j6_accel(d, gm, j4, j6, rad, pole);
        let (a, _) = f246.accel(gm, d, &rot, t, 6, false);
        assert!((a - closed).length() < 1e-14 * closed.length(), "J2-J6 {a} vs {closed}");
    }
}

/// A tesseral field rotates with the body: the inertial acceleration is the
/// body-fixed one turned by the rotation model, and the matrix is the
/// rotation model's.
#[test]
fn a_tesseral_field_turns_with_the_body() {
    let field = synthetic(8);
    let rot = body::moon().rotation;
    let t = crate::time::Epoch::from_calendar(2031, 5, 4, 3, 2, 1.0);
    let x_fixed = DVec3::new(1.1e6, 1.5e6, -0.4e6);
    let d = rot.to_inertial(Vec3::<BodyFixed>::from_raw(x_fixed), t).raw();
    let (a, u) = field.accel(GM, d, &rot, t, 8, true);
    let (a_fixed, u_fixed) = field.accel_in_field_frame(GM, x_fixed, 8);
    let turned = rot.to_inertial(Vec3::from_raw(a_fixed), t).raw();
    assert!((a - turned).length() < 1e-13 * a.length(), "{a} vs {turned}");
    assert!((u - u_fixed).abs() < 1e-13 * u.abs());
    let m = rot.matrix(t);
    for e in [DVec3::X, DVec3::Y, DVec3::Z] {
        let by_matrix = m * e;
        let by_model = rot.to_inertial(Vec3::from_raw(e), t).raw();
        assert!((by_matrix - by_model).length() < 1e-15, "{by_matrix} vs {by_model}");
    }
}

/// No degree exceeds its bound anywhere (sampled), the bound is not loose
/// by orders of magnitude, and the included degree leaves a tail under the
/// threshold.
#[test]
fn the_degree_bound_holds_and_sets_the_included_degree() {
    let field = synthetic(30);
    let single = |n: usize| {
        let mut c = Vec::new();
        for m in 0..=n {
            let (cc, ss) = field.coefficient(n, m);
            c.push(Coefficient { n, m, c: cc, s: ss });
        }
        Field::new("one degree", R, Some(GM), &c, None).unwrap()
    };
    for n in [2, 3, 7, 30] {
        let one = single(n);
        let r = 1.3 * R;
        let mut most: f64 = 0.0;
        for i in 0..400 {
            let z = -1.0 + (i as f64 + 0.5) / 200.0;
            let lon = i as f64 * 2.399_963; // golden-angle spiral
            let rho = (1.0 - z * z).sqrt();
            let x = DVec3::new(rho * crate::math::cos(lon), rho * crate::math::sin(lon), z) * r;
            most = most.max(one.accel_in_field_frame(GM, x, n).0.length());
        }
        let bound = field.bound(GM, n, r);
        assert!(most <= bound && most > bound / (10.0 * n as f64), "degree {n}: {most:e} against {bound:e}");
    }
    for r in [1.01 * R, 2.0 * R, 10.0 * R] {
        let threshold = 1e-9;
        let n = field.included_degree(GM, r, threshold, None);
        let tail: f64 = (n + 1..=30).map(|k| field.bound(GM, k, r)).sum();
        assert!(tail <= threshold * (1.0 + 1e-9), "r {r:e}: degree {n}, tail {tail:e}");
        assert!(n == 0 || field.bound(GM, n, r) + tail > threshold);
    }
    assert_eq!(field.included_degree(GM, 1e12, 0.0, None), 30, "a zero threshold keeps everything");
    assert_eq!(field.included_degree(GM, 1e12, 1e-14, None), 0, "far away nothing");
    // The tidal form: the change over a segment is under ℓ times the
    // Hessian bound (sampled along a segment).
    let one = single(4);
    let (p, q) = (DVec3::new(2.0 * R, 0.3 * R, 0.5 * R), DVec3::new(2.0 * R, 0.4 * R, 0.6 * R));
    let change = (one.accel_in_field_frame(GM, p, 4).0 - one.accel_in_field_frame(GM, q, 4).0).length();
    let hessian = field.bound(GM, 4, 2.0 * R) / (2.0 * R) * 11.0 * (6.0f64 / 9.0).sqrt();
    assert!(change <= hessian * (p - q).length(), "{change:e} > {:e}", hessian * (p - q).length());
}

#[test]
fn unnormalized_coefficients_convert() {
    // N_20 = √5, N_22 = √(5/12): Earth's J2 = −C20 = √5 · 4.8417e-4.
    assert!((normalization(2, 0) - 5f64.sqrt()).abs() < 1e-15);
    assert!((normalization(2, 2) - (5.0f64 / 12.0).sqrt()).abs() < 1e-15);
    assert!((normalization(3, 1) - (7.0f64 / 6.0).sqrt()).abs() < 1e-15);
    let text =
        "(model: \"test\", radius: 1000.0, normalized: false, coefficients: [(2, 0, -1e-3, 0.0), (2, 2, 3e-4, 1e-4)])";
    let f = file::parse(text).unwrap();
    let (c20, _) = f.coefficient(2, 0);
    let (c22, s22) = f.coefficient(2, 2);
    assert!((c20 + 1e-3 / 5f64.sqrt()).abs() < 1e-18);
    assert!((c22 - 3e-4 / (5.0f64 / 12.0).sqrt()).abs() < 1e-18 && (s22 - 1e-4 / (5.0f64 / 12.0).sqrt()).abs() < 1e-18);
    assert!(file::parse("(model: \"x\", radius: 1.0, normalized: true, coefficients: [(2, 0, 0.0, 1.0)])").is_err());
}

/// Bit-identical evaluations: the same inputs give the same bits (the
/// summation order is fixed), whatever the buffer size class.
#[test]
fn evaluation_is_deterministic() {
    let field = synthetic(70);
    let x = DVec3::new(1.3e6, 0.2e6, -1.1e6);
    let a = field.accel_in_field_frame(GM, x, 70);
    assert_eq!(a, field.accel_in_field_frame(GM, x, 70));
    // Degrees 13 and 14 straddle the first two buffer sizes.
    let (a13, a14) = (field.accel_in_field_frame(GM, x, 13).0, field.accel_in_field_frame(GM, x, 14).0);
    assert!((a13 - a14).length() < field.bound(GM, 14, x.length()));
}
