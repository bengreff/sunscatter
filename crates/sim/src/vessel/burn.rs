//! Burn directions: where a planned burn (a craft of parts' node, D126,
//! flown by its maneuver program) points, and why a plan of nodes is
//! refused.
//!
//! The `Tracking` direction law names a reference body only to say what
//! "prograde" means (a display choice): the direction is a function of the
//! vessel's state relative to that body, and the physics is unchanged by it
//! (rule 1: no reference bodies).

use crate::ephem::{Kinematics, Snapshot};
use crate::frame::NodeId;
use crate::time::Epoch;
use glam::DVec3;

/// Standard gravity (m/s²), for specific impulse.
pub const G0: f64 = 9.806_65;

/// Where the thrust points.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DirectionLaw {
    /// A fixed inertial direction (any length; normalised when used).
    Inertial(DVec3),
    /// Components along prograde, normal and radial-out (`x`, `y`, `z` of
    /// `axes`) of the vessel's motion relative to `reference`.
    Tracking { reference: NodeId, axes: DVec3 },
}

impl DirectionLaw {
    /// Unit thrust direction (inertial axes) for a vessel at `(r, v)` relative
    /// to `anchor`. The reference body's state comes from the frame tree
    /// (lowest common ancestor), never from subtracting absolute positions.
    pub fn direction(&self, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> DVec3 {
        self.direction_from(|node| snap.relative(node, anchor), anchor, r, v)
    }

    /// As [`DirectionLaw::direction`], with `relative(node)` giving a
    /// node's state relative to `anchor` (called only for a tracking law
    /// whose reference is not the anchor).
    pub fn direction_from(
        &self,
        relative: impl FnOnce(NodeId) -> Kinematics,
        anchor: NodeId,
        r: DVec3,
        v: DVec3,
    ) -> DVec3 {
        match *self {
            DirectionLaw::Inertial(d) => d.normalize(),
            DirectionLaw::Tracking { reference, axes } => {
                let (rel_r, rel_v) = if reference == anchor {
                    (r, v)
                } else {
                    let k = relative(reference);
                    (r - k.r, v - k.v)
                };
                let (prograde, normal, radial) = prograde_normal_radial(rel_r, rel_v);
                (prograde * axes.x + normal * axes.y + radial * axes.z).normalize()
            }
        }
    }
}

/// Unit prograde, orbit-normal and radial-out axes of a relative state.
pub fn prograde_normal_radial(r: DVec3, v: DVec3) -> (DVec3, DVec3, DVec3) {
    let prograde = v.normalize();
    let normal = r.cross(v).normalize();
    (prograde, normal, prograde.cross(normal))
}

/// Why a plan of nodes was not accepted ([`super::Vessel::set_nodes`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    /// Nodes must be in order of time.
    NotSorted,
    /// The edit changes a node that has already started (or was skipped).
    Started,
    /// A changed node falls before the vessel's current time.
    InThePast,
    /// Nodes are flown by a craft of parts' maneuver program.
    NodesNeedParts,
}

/// The direction law of the first of `nodes` at or after `t` (its Δv's
/// prograde, normal and radial components): what the maneuver hold points
/// along before it.
pub fn node_direction(nodes: &[crate::systems::maneuver::Node], t: Epoch) -> Option<DirectionLaw> {
    let n = nodes.iter().find(|n| n.t >= t)?;
    Some(DirectionLaw::Tracking { reference: n.reference, axes: n.dv })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_axes_table() {
        // (r, v) → (prograde, normal, radial-out).
        let x = DVec3::X * 7e6;
        let cases = [
            (x, DVec3::Y * 7.5e3, (DVec3::Y, DVec3::Z, DVec3::X)),
            (x, -DVec3::Y * 7.5e3, (-DVec3::Y, -DVec3::Z, DVec3::X)),
            (
                x,
                DVec3::new(1e3, 7.5e3, 0.0),
                (DVec3::new(1e3, 7.5e3, 0.0).normalize(), DVec3::Z, DVec3::new(7.5e3, -1e3, 0.0).normalize()),
            ),
        ];
        for (r, v, (p, n, rad)) in cases {
            let (gp, gn, grad) = prograde_normal_radial(r, v);
            assert!(gp.abs_diff_eq(p, 1e-15) && gn.abs_diff_eq(n, 1e-15) && grad.abs_diff_eq(rad, 1e-15), "{r} {v}");
        }
    }
}
