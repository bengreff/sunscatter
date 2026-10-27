//! The structure's rules against hand calculations: a stack under thrust,
//! a side load, spin, subtrees in any order, the capacity table.

use super::*;

fn point(mass: f64, at: DVec3) -> MassProps {
    MassProps { mass, com: at, inertia: DMat3::ZERO }
}

/// A stack of three point masses along Z under thrust at the bottom:
/// each joint carries what accelerates the masses above it (compression).
#[test]
fn a_stack_under_thrust() {
    // Root at the top (as the Falcon 9's capsule): 0 at z 20 (2 t), 1 at
    // z 10 (5 t) under it, 2 at z 0 (10 t) under that with the engine.
    let parent = [None, Some(0), Some(1)];
    let mass = [point(2_000.0, DVec3::Z * 20.0), point(5_000.0, DVec3::Z * 10.0), point(10_000.0, DVec3::ZERO)];
    let thrust = 340_000.0;
    let a = thrust / 17_000.0;
    let com = (DVec3::Z * 20.0 * 2_000.0 + DVec3::Z * 10.0 * 5_000.0) / 17_000.0;
    let m = Motion { com, accel: DVec3::Z * a, ..Motion::default() };
    let push = Push { part: 2, force: DVec3::Z * thrust, at: DVec3::ZERO, torque: DVec3::ZERO };
    let loads = subtree_loads(&parent, &mass, &[push], &m);
    // The root lacks nothing: the motion is the thrust's.
    assert!(loads[0].0.length() < 1e-9 * thrust && loads[0].1.length() < 1e-6 * thrust);
    // Joint 0|1: the parent pushes the child's side down by the top's
    // inertia, 2 t × a; joint 1|2: 7 t × a.
    assert!((loads[1].0 - DVec3::Z * (-2_000.0 * a)).length() < 1e-6, "{:?}", loads[1].0);
    assert!((loads[2].0 - DVec3::Z * (-7_000.0 * a)).length() < 1e-6, "{:?}", loads[2].0);
    // On the axis: no moment about the joints.
    for (i, at) in [(1, DVec3::Z * 15.0), (2, DVec3::Z * 5.0)] {
        assert!(moment_about(loads[i].0, loads[i].1, at).length() < 1e-6);
    }
    // Compression in a 1 m ring at joint 1|2 (normal −Z, parent above).
    let s = section(loads[2].0, DVec3::ZERO, -DVec3::Z, 1.0, 0.0);
    let n = 7_000.0 * a / math::TAU;
    assert!((s.compression - n).abs() < 1e-6 && (s.tension + n).abs() < 1e-6 && s.shear < 1e-9, "{s:?}");
    // 1 bar inside the ring relieves π·10⁵ N of it.
    let p = section(loads[2].0, DVec3::ZERO, -DVec3::Z, 1.0, 1e5);
    assert!((p.compression - (7_000.0 * a - 1e5 * math::PI) / math::TAU).abs() < 1e-6, "{p:?}");
}

/// A side load on the top of a cantilevered stack: shear and a bending
/// moment growing with the lever, as by hand.
#[test]
fn a_side_load_bends_the_joint() {
    // Root at the bottom (held, as a vehicle on its clamps): 0 at z 0,
    // 1 (1 t) at z 10. A 10 kN side force at z 15 on part 1, the body at
    // rest: the joint at z 5 carries −10 kN of shear and a moment of
    // 10 kN × 10 m.
    let parent = [None, Some(0)];
    let mass = [point(5_000.0, DVec3::ZERO), point(1_000.0, DVec3::Z * 10.0)];
    let push = Push { part: 1, force: DVec3::X * 1e4, at: DVec3::Z * 15.0, torque: DVec3::ZERO };
    let loads = subtree_loads(&parent, &mass, &[push], &Motion::default());
    let at = DVec3::Z * 5.0;
    let moment = moment_about(loads[1].0, loads[1].1, at);
    assert!((loads[1].0 - DVec3::X * -1e4).length() < 1e-9, "{:?}", loads[1].0);
    // The joint holds the top against the force's moment about it: −(10 m × 10 kN) about Y.
    assert!((moment - DVec3::Y * -1e5).length() < 1e-6, "{moment:?}");
    // A 2 m ring: shear flow V/(πR), bending M/(πR²) both ways.
    let s = section(loads[1].0, moment, DVec3::Z, 2.0, 0.0);
    let bend = 1e5 / (math::PI * 4.0);
    assert!((s.tension - bend).abs() < 1e-6 && (s.compression - bend).abs() < 1e-6, "{s:?}");
    assert!((s.shear - 1e4 / (math::PI * 2.0)).abs() < 1e-9, "{s:?}");
}

/// Rotation: a part spun about the root needs the centripetal force
/// m·ω²·r towards the axis, and angular acceleration its tangential force
/// and its own I·α.
#[test]
fn spin_and_angular_acceleration() {
    let parent = [None, Some(0)];
    let inertia = DMat3::from_diagonal(DVec3::new(10.0, 20.0, 30.0));
    let mass = [point(1e6, DVec3::ZERO), MassProps { mass: 100.0, com: DVec3::X * 4.0, inertia }];
    let m = Motion { omega: DVec3::Z * 0.5, alpha: DVec3::Z * 2.0, ..Motion::default() };
    let loads = subtree_loads(&parent, &mass, &[], &m);
    // a = α×r + ω×(ω×r) = (0, 8, 0) + (−1, 0, 0) at r = 4 X.
    assert!((loads[1].0 - DVec3::new(-100.0, 800.0, 0.0)).length() < 1e-9, "{:?}", loads[1].0);
    // About the origin: r × F + I·α (ω ∥ a principal axis: no gyroscopic term).
    let expect = DVec3::X * 4.0 * 1.0 * 0.0 + DVec3::new(0.0, 0.0, 4.0 * 800.0) + DVec3::Z * 60.0;
    assert!((loads[1].1 - expect).length() < 1e-9, "{:?}", loads[1].1);
}

/// The order of the parts does not matter, and a deep tree sums its
/// subtrees exactly.
#[test]
fn subtrees_in_any_order() {
    // 0 ← 1 ← 2, 0 ← 3 ← 4, listed children first.
    let parent = [Some(1), Some(4), None, Some(4), Some(2)];
    let mass: Vec<MassProps> = (0..5).map(|i| point(10.0 * (i + 1) as f64, DVec3::X * i as f64)).collect();
    let m = Motion { accel: DVec3::Y * 3.0, ..Motion::default() };
    let loads = subtree_loads(&parent, &mass, &[], &m);
    let f = |list: &[usize]| list.iter().map(|&i| mass[i].mass * 3.0).sum::<f64>();
    assert_eq!(loads[0].0, DVec3::Y * f(&[0]));
    assert_eq!(loads[1].0, DVec3::Y * f(&[0, 1]));
    assert_eq!(loads[3].0, DVec3::Y * f(&[3]));
    assert_eq!(loads[4].0, DVec3::Y * f(&[0, 1, 3, 4]));
    assert_eq!(loads[2].0, DVec3::Y * f(&[0, 1, 2, 3, 4]));
}

#[test]
fn capacity_and_ratio_table() {
    // (section, capacity, ratio): tension, compression against buckling,
    // shear against the ultimate over √3, unknown capacity.
    let s = |t: f64, c: f64, v: f64| Section { tension: t, compression: c, shear: v };
    let cases = [
        (s(5e5, -5e5, 0.0), Capacity::of(1e6, 2e5, 1.0), 0.5),
        (s(-1e5, 1e5, 0.0), Capacity::of(1e6, 2e5, 1.0), 0.5),
        (s(-1e5, 1e5, 0.0), Capacity::of(1e6, 0.0, 1.0), 0.1),
        (s(0.0, 0.0, 1e6 / 3f64.sqrt()), Capacity::of(1e6, 0.0, 1.0), 1.0),
        (s(5e5, -5e5, 0.0), Capacity::of(1e6, 0.0, 0.5), 1.0),
        (s(5e5, 1e5, 1.0), Capacity::of(0.0, 0.0, 1.0), 0.0),
    ];
    for (sec, cap, expect) in cases {
        assert!((ratio(&sec, &cap) - expect).abs() < 1e-12, "{sec:?} {cap:?}: {}", ratio(&sec, &cap));
    }
}
