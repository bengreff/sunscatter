//! Placing generated parts in the craft's axes (D102): rigid poses, a child
//! meeting its parent at a mounting point (normals opposed, roll references
//! aligned, turned by the roll), coordinates, and primitives moved by a
//! pose. Rotation entries within 1e-12 of 0 or ±1 snap there, so parts at
//! quarter turns stay exactly axis-aligned (boxes must: [`Shape::Box`] is
//! axis-aligned in body axes). Trigonometry through [`crate::math`].

use super::file::Shape;
use crate::math;
use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

/// A rigid placement: a point x of the part is `rot · x + pos` in the craft.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    pub rot: DMat3,
    pub pos: DVec3,
}

impl Pose {
    pub const IDENTITY: Pose = Pose { rot: DMat3::IDENTITY, pos: DVec3::ZERO };

    pub fn point(&self, p: DVec3) -> DVec3 {
        self.rot * p + self.pos
    }

    pub fn dir(&self, d: DVec3) -> DVec3 {
        self.rot * d
    }

    /// This pose after `inner` (a point of the inner frame into this one's
    /// parent).
    pub fn then(&self, inner: &Pose) -> Pose {
        Pose { rot: snap(self.rot * inner.rot), pos: self.point(inner.pos) }
    }

    /// `shape` moved by this pose; a box only by quarter turns.
    pub fn shape(&self, s: &Shape) -> Result<Shape, String> {
        Ok(match *s {
            Shape::Frustum { base, axis, r_base, r_top } => {
                Shape::Frustum { base: self.point(base), axis: self.dir(axis), r_base, r_top }
            }
            Shape::Cylinder { base, axis, radius } => {
                Shape::Cylinder { base: self.point(base), axis: self.dir(axis), radius }
            }
            Shape::SphereCap { center, axis, radius, height } => {
                Shape::SphereCap { center: self.point(center), axis: self.dir(axis), radius, height }
            }
            Shape::Strut { from, to, radius } => Shape::Strut { from: self.point(from), to: self.point(to), radius },
            Shape::Box { center, half } => {
                let m = self.rot;
                let aligned = [m.x_axis, m.y_axis, m.z_axis].iter().all(|c| {
                    let a = c.abs();
                    a.max_element() == 1.0 && a.min_element() == 0.0 && (a.x + a.y + a.z) == 1.0
                });
                if !aligned {
                    return Err("a box can only be placed at quarter turns (boxes are axis-aligned)".into());
                }
                let half = DVec3::new(m.row(0).abs().dot(half), m.row(1).abs().dot(half), m.row(2).abs().dot(half));
                Shape::Box { center: self.point(center), half }
            }
        })
    }
}

/// Entries within 1e-12 of 0 or ±1 set there.
fn snap(m: DMat3) -> DMat3 {
    let s = |x: f64| {
        if x.abs() < 1e-12 {
            0.0
        } else if (x.abs() - 1.0).abs() < 1e-12 {
            x.signum()
        } else {
            x
        }
    };
    let col = |c: DVec3| DVec3::new(s(c.x), s(c.y), s(c.z));
    DMat3::from_cols(col(m.x_axis), col(m.y_axis), col(m.z_axis))
}

/// The orthonormal frame [roll, normal × roll, normal] of a mount.
fn mount_frame(normal: DVec3, roll: DVec3) -> DMat3 {
    let n = normal.normalize();
    let r = (roll - n * roll.dot(n)).normalize();
    DMat3::from_cols(r, n.cross(r), n)
}

/// A mounting point's position, normal and roll reference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub pos: DVec3,
    pub normal: DVec3,
    pub roll: DVec3,
}

/// The pose of a child whose mount `child` (its own frame) meets the
/// parent's mount `parent` (craft axes): the child's normal against the
/// parent's, its roll reference along the parent's turned `roll_deg` about
/// the child's normal (right-handed: a stacked child turned 90° takes its
/// +X to the parent's +Y).
pub fn attach(parent: Frame, child: Frame, roll_deg: f64) -> Pose {
    let n = parent.normal.normalize();
    let turned = math::rotate_axis(parent.roll, -n, roll_deg * math::PI / 180.0);
    // The child's mount frame must land on [turned, −n × turned, −n].
    let target = mount_frame(-n, turned);
    let rot = snap(target * mount_frame(child.normal, child.roll).transpose());
    Pose { rot, pos: parent.pos - rot * child.pos }
}

/// The pose of a part with its +Z along `axis` and turned `roll_deg` about
/// it, its origin at `pos`.
pub fn placed(pos: DVec3, axis: DVec3, roll_deg: f64) -> Pose {
    let q = crate::vessel::quat_z_to(axis.normalize());
    let tilt = DMat3::from_quat(q);
    let roll = about_z(roll_deg);
    Pose { rot: snap(tilt * roll), pos }
}

/// A turn of `deg` about +Z.
pub fn about_z(deg: f64) -> DMat3 {
    let a = deg * math::PI / 180.0;
    snap(DMat3::from_cols(math::rotate_z(DVec3::X, a), math::rotate_z(DVec3::Y, a), DVec3::Z))
}

/// A turn of `deg` about the unit axis `k` through `at`.
pub fn hinge(at: DVec3, k: DVec3, deg: f64) -> Pose {
    let a = deg * math::PI / 180.0;
    let r = |v: DVec3| math::rotate_axis(v, k, a);
    let rot = snap(DMat3::from_cols(r(DVec3::X), r(DVec3::Y), r(DVec3::Z)));
    Pose { rot, pos: at - rot * at }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: DVec3, b: DVec3) -> bool {
        (a - b).length() < 1e-12
    }

    /// A child stacked under its parent: its top mount meets the parent's
    /// bottom one, normals opposed, origins one child length apart.
    #[test]
    fn stacking_and_radial_attachment() {
        let parent = Frame { pos: DVec3::new(0.0, 0.0, -2.0), normal: -DVec3::Z, roll: DVec3::X };
        let child = Frame { pos: DVec3::Z * 5.0, normal: DVec3::Z, roll: DVec3::X };
        let p = attach(parent, child, 0.0);
        assert!(close(p.point(DVec3::Z * 5.0), parent.pos));
        assert!(close(p.dir(DVec3::Z), DVec3::Z), "a stack keeps the axis");
        assert!(close(p.point(DVec3::ZERO), DVec3::new(0.0, 0.0, -7.0)));
        assert_eq!(p.rot.determinant(), 1.0);
        // A fin's root (normal −X in its frame) on a radial mount at +Y.
        let side = Frame { pos: DVec3::Y * 1.83, normal: DVec3::Y, roll: DVec3::Z };
        let root = Frame { pos: DVec3::ZERO, normal: -DVec3::X, roll: DVec3::Z };
        let f = attach(side, root, 0.0);
        assert!(close(f.dir(DVec3::X), DVec3::Y), "the fin sticks out along the mount's normal");
        assert!(close(f.dir(DVec3::Z), DVec3::Z));
        let b = f.shape(&Shape::Box { center: DVec3::X, half: DVec3::new(1.0, 0.1, 0.5) }).unwrap();
        assert_eq!(b, Shape::Box { center: DVec3::new(0.0, 2.83, 0.0), half: DVec3::new(0.1, 1.0, 0.5) });
        // A roll of a quarter turn about the mount.
        let r = attach(parent, child, 90.0);
        assert!(close(r.dir(DVec3::X), DVec3::Y));
        // At 45° a box cannot follow.
        let diag = Frame { pos: DVec3::ZERO, normal: DVec3::new(1.0, 1.0, 0.0).normalize(), roll: DVec3::Z };
        assert!(attach(diag, root, 0.0).shape(&Shape::Box { center: DVec3::ZERO, half: DVec3::ONE }).is_err());
    }

    #[test]
    fn coordinates_and_hinges() {
        let p = placed(DVec3::new(1.0, 2.0, 3.0), DVec3::Z, 90.0);
        assert!(close(p.dir(DVec3::X), DVec3::Y) && close(p.point(DVec3::ZERO), DVec3::new(1.0, 2.0, 3.0)));
        let down = placed(DVec3::ZERO, -DVec3::Z, 0.0);
        assert!(close(down.dir(DVec3::Z), -DVec3::Z));
        // A leg hinged at (1, 0, 0) about +Y turned 90°: +Z goes to +X.
        let h = hinge(DVec3::X, DVec3::Y, 90.0);
        assert!(close(h.point(DVec3::new(1.0, 0.0, 2.0)), DVec3::new(3.0, 0.0, 0.0)));
        assert!(close(h.point(DVec3::X), DVec3::X), "the hinge stays");
    }
}
