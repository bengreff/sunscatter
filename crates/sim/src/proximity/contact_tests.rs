use super::*;
use crate::craft::test_craft;
use crate::rigid;
use crate::vessel::Attitude;

/// A free rigid body for the tests (no gravity).
#[derive(Clone, Copy, Debug)]
struct Free {
    r: DVec3,
    v: DVec3,
    att: Attitude,
}

fn hull() -> Hull {
    Hull::new(&test_craft().def())
}

fn props() -> crate::craft::MassProps {
    test_craft().mass.at(test_craft().spec.propellant.mass)
}

fn body<'a>(hull: &'a Hull, f: &Free) -> Body<'a> {
    let m = props();
    Body { hull, r: f.r, v: f.v, q: f.att.q, omega: f.att.omega, com: m.com, mass: m.mass, inertia: m.inertia }
}

/// Linear momentum, angular momentum about the origin, kinetic energy.
fn totals(bodies: &[Free]) -> (DVec3, DVec3, f64) {
    let m = props();
    let (mut p, mut l, mut e) = (DVec3::ZERO, DVec3::ZERO, 0.0);
    for b in bodies {
        let w = b.att.q.inverse() * b.att.omega;
        let spin = b.att.q * (m.inertia * w);
        p += b.v * m.mass;
        l += b.r.cross(b.v * m.mass) + spin;
        e += 0.5 * m.mass * b.v.length_squared() + 0.5 * w.dot(m.inertia * w);
    }
    (p, l, e)
}

/// One contact substep of two free bodies: forces from the start of the
/// substep, velocity and spin kicked by the impulse, then drift and
/// torque-free rotation (as the bubble's contact substeps do).
fn substep(h: &Hull, a: &mut Free, b: &mut Free, dt: f64, push: DVec3) -> PairForces {
    let m = props();
    let f = pair_forces(&body(h, a), &body(h, b), 0.5, 0.05, dt);
    let kick = |s: &mut Free, force: DVec3, torque: DVec3| {
        s.v += force * (dt / m.mass);
        let w = m.inertia.inverse() * (s.att.q.inverse() * torque) * dt;
        s.att.omega += s.att.q * w;
        s.r += s.v * dt;
        s.att = rigid::tick(&s.att, &m.inertia, DVec3::ZERO, dt);
    };
    kick(a, f.force + push, f.torque_a);
    kick(b, -f.force, f.torque_b);
    f
}

/// Two test craft side by side, one turned 45° about its axis so a fin
/// meets the other's body, offset so the hit is off-centre.
fn approach(speed: f64) -> (Free, Free) {
    let a = Free { r: DVec3::ZERO, v: DVec3::ZERO, att: Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO } };
    let eighth = crate::math::PI / 8.0;
    let q = DQuat::from_xyzw(0.0, 0.0, crate::math::sin(eighth), crate::math::cos(eighth));
    let b =
        Free { r: DVec3::new(5.0, 0.0, 1.5), v: DVec3::new(-speed, 0.0, 0.0), att: Attitude { q, omega: DVec3::ZERO } };
    (a, b)
}

/// Every contact force has its equal and opposite: the pair's momentum
/// and angular momentum (about A's centre of mass) do not change, to
/// round-off, at overlapping poses with motion.
#[test]
fn contact_forces_are_equal_and_opposite() {
    let h = hull();
    let mut tested = 0;
    for k in 0..12 {
        let half = 0.15 * k as f64;
        let qz = DQuat::from_xyzw(0.0, 0.0, crate::math::sin(half), crate::math::cos(half));
        let qx = DQuat::from_xyzw(crate::math::sin(0.1 * k as f64), 0.0, 0.0, crate::math::cos(0.1 * k as f64));
        let q = qz * qx;
        let a = Free {
            r: DVec3::ZERO,
            v: DVec3::new(0.1, 0.0, 0.0),
            att: Attitude { q: DQuat::IDENTITY, omega: DVec3::new(0.0, 0.0, 0.05) },
        };
        let b = Free {
            r: DVec3::new(2.0 + 0.1 * k as f64, 0.3, 0.5),
            v: DVec3::new(-0.4, 0.05, 0.0),
            att: Attitude { q, omega: DVec3::new(0.02, 0.0, 0.0) },
        };
        let (ba, bb) = (body(&h, &a), body(&h, &b));
        let f = pair_forces(&ba, &bb, 0.5, 0.05, 0.002);
        if f.count == 0 {
            continue;
        }
        tested += 1;
        // Torques about A's centre of mass: τ_A + τ_B + d × F_B.
        let total = f.torque_a + f.torque_b + (b.r - a.r).cross(-f.force);
        let scale = f.torque_a.length() + f.torque_b.length() + f.force.length() * 10.0;
        assert!(total.length() <= 1e-12 * scale, "{k}: {total} of {scale}");
        assert!(f.depth > 0.0 && f.depth < 1.0, "{k}: {}", f.depth);
    }
    assert!(tested >= 6, "{tested} poses touch");
}

/// Two craft nose to nose (B turned over), the nose tips `gap` apart, B
/// closing at `speed` along the line of centres.
fn nose_to_nose(gap: f64, speed: f64) -> (Free, Free) {
    let tip = 6.796 - props().com.z;
    let still = Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO };
    let a = Free { r: DVec3::ZERO, v: DVec3::ZERO, att: still };
    let flipped = Attitude { q: DQuat::from_xyzw(1.0, 0.0, 0.0, 0.0), omega: DVec3::ZERO };
    (a, Free { r: DVec3::new(0.0, 0.0, 2.0 * tip + gap), v: DVec3::new(0.0, 0.0, -speed), att: flipped })
}

/// Two craft drifting together at 0.5 m/s collide and separate: momentum
/// and angular momentum conserved, kinetic energy lost (restitution about
/// e^(−ζπ/√(1−ζ²)) = 0.37), a shallow, short contact.
#[test]
fn two_craft_drifting_together_collide_and_separate() {
    let h = hull();
    let (mut a, mut b) = nose_to_nose(0.1, 0.5);
    let (p0, l0, e0) = totals(&[a, b]);
    let dt = 0.002;
    let (mut touched, mut deepest, mut fastest) = (0, 0.0f64, 0.0f64);
    for _ in 0..(5.0 / dt) as usize {
        let f = substep(&h, &mut a, &mut b, dt, DVec3::ZERO);
        if f.count > 0 {
            touched += 1;
            deepest = deepest.max(f.depth);
            fastest = fastest.max(f.impact.map_or(0.0, |i| i.speed));
        }
    }
    let (p1, l1, e1) = totals(&[a, b]);
    let m = props().mass;
    let e = (b.v - a.v).z / 0.5;
    eprintln!(
        "nose to nose: contact {:.3} s, deepest {:.1} mm, impact {fastest:.3} m/s, restitution {e:.3}; \
         energy {e0:.1} -> {e1:.1} J; |dp| {:.2e}, |dL| {:.2e}",
        touched as f64 * dt,
        deepest * 1e3,
        (p1 - p0).length(),
        (l1 - l0).length(),
    );
    assert!(touched > 10, "they met ({touched} substeps)");
    assert_eq!(pair_forces(&body(&h, &a), &body(&h, &b), 0.5, 0.05, dt).count, 0, "apart again");
    assert!((p1 - p0).length() < 1e-9 * m, "momentum {p0} -> {p1}");
    assert!((l1 - l0).length() < 1e-9 * m, "angular momentum {l0} -> {l1}");
    assert!(e1 < e0, "energy {e0} -> {e1}");
    assert!(e > 0.2 && e < 0.6, "restitution {e}");
    assert!(deepest < 0.03, "{deepest} m deep");
    assert!((fastest - 0.5).abs() < 1e-3, "{fastest} m/s");
}

/// An off-centre hit (a fin's edge on the other craft's flared skirt) turns
/// the craft: their momentum and angular momentum are conserved (the
/// impulses exactly; the torque-free rotation steps to their truncation),
/// kinetic energy does not grow, and spin is exchanged.
#[test]
fn an_off_centre_hit_conserves_momentum_and_angular_momentum() {
    let h = hull();
    let (mut a, mut b) = approach(0.5);
    let (p0, l0, e0) = totals(&[a, b]);
    let dt = 0.002;
    let mut touched = 0;
    for _ in 0..(20.0 / dt) as usize {
        if substep(&h, &mut a, &mut b, dt, DVec3::ZERO).count > 0 {
            touched += 1;
        }
    }
    let (p1, l1, e1) = totals(&[a, b]);
    let m = props().mass;
    eprintln!(
        "off centre: contact {:.3} s; energy {e0:.1} -> {e1:.1} J; |dp| {:.2e}, |dL| {:.2e} of {:.2e}; spins {} {}",
        touched as f64 * dt,
        (p1 - p0).length(),
        (l1 - l0).length(),
        l0.length(),
        a.att.omega,
        b.att.omega
    );
    assert!(touched > 10, "they met ({touched} substeps)");
    assert!((p1 - p0).length() < 1e-9 * m, "momentum {p0} -> {p1}");
    assert!((l1 - l0).length() < 1e-6 * l0.length(), "angular momentum {l0} -> {l1}");
    assert!(e1 <= e0, "energy {e0} -> {e1}");
    assert!(a.att.omega.length() > 1e-3 && b.att.omega.length() > 1e-3, "spin exchanged");
}

/// A craft pushed against another (0.02 m/s² on A, as a gentle thrust,
/// nose to nose along the line of centres) comes to rest against it: after
/// a few seconds the two move together, with no jitter in the depth or
/// the relative velocity.
#[test]
fn a_craft_pressed_against_another_rests_without_jitter() {
    let h = hull();
    let (mut a, mut b) = nose_to_nose(0.05, 0.0);
    a.v = DVec3::new(0.0, 0.0, 0.05);
    let dt = 0.002;
    let push = DVec3::new(0.0, 0.0, 0.02 * props().mass);
    let (mut depths, mut rel) = (Vec::new(), Vec::new());
    for k in 0..(30.0 / dt) as usize {
        let f = substep(&h, &mut a, &mut b, dt, push);
        if k as f64 * dt > 20.0 {
            depths.push(f.depth);
            rel.push((b.v - a.v).length() + (b.att.omega - a.att.omega).length());
        }
    }
    let (lo, hi) = depths.iter().fold((f64::MAX, f64::MIN), |(l, h), &d| (l.min(d), h.max(d)));
    let fastest = rel.iter().copied().fold(0.0, f64::max);
    eprintln!("resting: depth {:.4}..{:.4} mm, relative motion under {fastest:.2e}", lo * 1e3, hi * 1e3);
    assert!(lo > 0.0, "still touching");
    assert!(hi - lo < 1e-5, "the depth jitters: {lo}..{hi}");
    assert!(fastest < 1e-4, "relative motion {fastest}");
}
