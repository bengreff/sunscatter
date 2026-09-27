use super::*;
use crate::math;
use crate::vessel::quat_from_rotvec;

fn diag(a: f64, b: f64, c: f64) -> DMat3 {
    DMat3::from_diagonal(DVec3::new(a, b, c))
}

/// A tensor with principal moments `m` along axes rotated by `r`.
fn rotated(m: DVec3, r: DQuat) -> DMat3 {
    let axes = DMat3::from_quat(r);
    axes * DMat3::from_diagonal(m) * axes.transpose()
}

fn angle_between(a: DQuat, b: DQuat) -> f64 {
    2.0 * math::asin((a.inverse() * b).xyz().length().min(1.0))
}

/// Fine RK4 reference: `seconds` of torque-free motion in steps of `h`.
fn reference(att: &Attitude, inertia: &DMat3, seconds: f64, h: f64) -> Attitude {
    let n = (seconds / h).round() as usize;
    let mut a = *att;
    for _ in 0..n {
        a = tick(&a, inertia, DVec3::ZERO, h);
    }
    a
}

#[test]
fn principal_axes_recover_a_rotated_tensor() {
    let r = quat_from_rotvec(DVec3::new(0.3, -0.7, 0.4));
    for m in [DVec3::new(1.0, 2.0, 3.0), DVec3::new(5.0, 5.0, 9.0), DVec3::new(2.0, 7.0, 7.0), DVec3::splat(4.0)] {
        let t = rotated(m, r);
        let (moments, axes) = principal_axes(t);
        assert!((moments - m).length() < 1e-12 * m.length(), "{moments} vs {m}");
        assert!((axes.determinant() - 1.0).abs() < 1e-12);
        let back = axes * DMat3::from_diagonal(moments) * axes.transpose();
        assert!(
            (back - t).x_axis.length() + (back - t).y_axis.length() + (back - t).z_axis.length() < 1e-12 * m.length()
        );
    }
    // Already diagonal, any order: sorted ascending.
    let (moments, _) = principal_axes(diag(3.0, 1.0, 2.0));
    assert_eq!(moments, DVec3::new(1.0, 2.0, 3.0));
}

/// Inertia cases: symmetric (prolate like the test craft, oblate),
/// asymmetric, near-symmetric, rotated.
fn cases() -> Vec<(&'static str, DMat3, DVec3)> {
    let r = quat_from_rotvec(DVec3::new(0.2, 0.5, -0.3));
    vec![
        ("prolate top", diag(1.3e5, 1.3e5, 4.0e4), DVec3::new(0.2, -0.1, 0.5)),
        ("oblate top", diag(2.0, 2.0, 3.5), DVec3::new(0.3, 0.1, 0.8)),
        ("asymmetric, about the largest axis", diag(1.0, 2.0, 3.0), DVec3::new(0.2, 0.1, 0.9)),
        ("asymmetric, about the smallest axis", diag(1.0, 2.0, 3.0), DVec3::new(0.9, 0.1, 0.2)),
        ("asymmetric, near the separatrix", diag(1.0, 2.0, 3.0), DVec3::new(0.3, 0.9, 0.29)),
        ("asymmetric, rotated axes", rotated(DVec3::new(4.0, 6.0, 9.0), r), DVec3::new(-0.4, 0.3, 0.6)),
        ("near-symmetric", diag(1.0, 1.0 + 1e-12, 3.0), DVec3::new(0.3, 0.2, 0.5)),
        ("spin about an axis", diag(1.0, 2.0, 3.0), DVec3::new(0.0, 0.7, 0.0)),
    ]
}

#[test]
fn free_rotation_matches_a_fine_integration() {
    let q0 = quat_from_rotvec(DVec3::new(0.4, 0.1, -0.2));
    for (name, inertia, w_body) in cases() {
        let att = Attitude { q: q0, omega: q0 * w_body };
        let free = FreeRotation::new(&att, &inertia);
        for t in [0.0, 1.7, 13.0, 40.0] {
            let exact = free.at(t);
            let fine = reference(&att, &inertia, t, 1e-3);
            let da = angle_between(exact.q, fine.q);
            let dw = (exact.omega - fine.omega).length();
            assert!(da < 1e-8 && dw < 1e-9, "{name} at {t} s: attitude {da:e} rad, ω {dw:e} rad/s");
        }
    }
}

#[test]
fn free_rotation_conserves_energy_and_momentum_for_an_hour_and_a_year() {
    let q0 = quat_from_rotvec(DVec3::new(-0.3, 0.8, 0.1));
    for (name, inertia, w_body) in cases() {
        let att = Attitude { q: q0, omega: q0 * w_body };
        let (l0, e0) = (angular_momentum(&att, &inertia), kinetic_energy(&att, &inertia));
        let free = FreeRotation::new(&att, &inertia);
        for t in [3600.0, 3.2e7] {
            let a = free.at(t);
            let (l, e) = (angular_momentum(&a, &inertia), kinetic_energy(&a, &inertia));
            assert!((l - l0).length() < 1e-9 * l0.length(), "{name} at {t} s: L {l} vs {l0}");
            assert!((e / e0 - 1.0).abs() < 1e-9, "{name} at {t} s: E {e} vs {e0}");
            assert!((a.q.length() - 1.0).abs() < 1e-15);
        }
        // Restarting from an intermediate state continues the same motion.
        let mid = free.at(1234.5);
        let (a, b) = (free.at(4000.0), FreeRotation::new(&mid, &inertia).at(4000.0 - 1234.5));
        assert!(angle_between(a.q, b.q) < 1e-7, "{name}: {}", angle_between(a.q, b.q));
    }
}

#[test]
fn rk4_ticks_conserve_energy_and_momentum_for_an_hour() {
    // 180 000 control ticks with no torque (the attitude's own lattice).
    let inertia = diag(1.0e5, 1.3e5, 4.0e4);
    let q0 = quat_from_rotvec(DVec3::new(0.1, 0.2, 0.3));
    let att = Attitude { q: q0, omega: q0 * DVec3::new(0.05, 0.3, 0.2) };
    let (l0, e0) = (angular_momentum(&att, &inertia), kinetic_energy(&att, &inertia));
    let a = reference(&att, &inertia, 3600.0, 0.02);
    let (l, e) = (angular_momentum(&a, &inertia), kinetic_energy(&a, &inertia));
    assert!((l.length() / l0.length() - 1.0).abs() < 1e-8, "|L| drift {:e}", l.length() / l0.length() - 1.0);
    assert!((e / e0 - 1.0).abs() < 1e-8, "E drift {:e}", e / e0 - 1.0);
    // And it agrees with the exact motion.
    let exact = FreeRotation::new(&att, &inertia).at(3600.0);
    assert!(angle_between(exact.q, a.q) < 1e-5, "{}", angle_between(exact.q, a.q));
}

#[test]
fn symmetric_top_precesses_at_the_analytic_rates() {
    // I₁ = I₂ = 3, I₃ = 1 (prolate); body spin ω₃, transverse ω⊥.
    let (i1, i3, w3, wp) = (3.0, 1.0, 0.8, 0.25);
    let inertia = diag(i1, i1, i3);
    let att = Attitude { q: DQuat::IDENTITY, omega: DVec3::new(wp, 0.0, w3) };
    let l = angular_momentum(&att, &inertia);
    let free = FreeRotation::new(&att, &inertia);
    let t = 7.3;
    for a in [free.at(t), reference(&att, &inertia, t, 1e-3)] {
        // Body frame: ω⊥ turns about the symmetry axis at Ω = (I₃ − I₁)/I₁ · ω₃.
        let w_body = a.q.inverse() * a.omega;
        let body_angle = math::atan2(w_body.y, w_body.x);
        let expected_body = (i3 - i1) / i1 * w3 * t;
        let wrap = |x: f64| x - math::TAU * libm::round(x / math::TAU);
        assert!(wrap(body_angle - expected_body).abs() < 1e-8, "body {body_angle} vs {expected_body}");
        // Inertial frame: the symmetry axis cones about L at |L|/I₁.
        let (axis0, axis) = (DVec3::Z, a.q * DVec3::Z);
        let lh = l.normalize();
        let perp = |v: DVec3| (v - lh * v.dot(lh)).normalize();
        let (p0, p1) = (perp(axis0), perp(axis));
        let turned = math::atan2(p0.cross(p1).dot(lh), p0.dot(p1));
        assert!(wrap(turned - l.length() / i1 * t).abs() < 1e-8, "cone {turned} vs {}", l.length() / i1 * t);
    }
}

#[test]
fn a_constant_torque_spins_up_at_torque_over_inertia() {
    let inertia = diag(2.0e4, 2.0e4, 1.0e4);
    let mut a = Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO };
    for _ in 0..50 {
        a = tick(&a, &inertia, DVec3::new(0.0, 0.0, 1.0e3), 0.02);
    }
    assert!((a.omega.z - 0.1).abs() < 1e-12, "{}", a.omega);
    assert!((angle_between(a.q, DQuat::IDENTITY) - 0.05).abs() < 1e-12);
}

#[test]
#[ignore = "timing; run with --ignored --nocapture"]
fn timing() {
    let q0 = quat_from_rotvec(DVec3::new(0.4, 0.1, -0.2));
    for (name, inertia, w) in [cases()[0], cases()[2]] {
        let att = Attitude { q: q0, omega: q0 * w };
        let n = 2000;
        let t0 = std::time::Instant::now();
        let mut acc = 0.0;
        for k in 0..n {
            acc += FreeRotation::new(&att, &inertia).at(1000.0 + k as f64).q.w;
        }
        let per = t0.elapsed() / n;
        let t1 = std::time::Instant::now();
        let mut a = att;
        for _ in 0..n {
            a = tick(&a, &inertia, DVec3::new(1.0, 2.0, 3.0), 0.02);
        }
        println!("{name}: new + at {per:?}, RK4 tick {:?} ({acc:.1} {})", t1.elapsed() / n, a.q.w);
    }
}
