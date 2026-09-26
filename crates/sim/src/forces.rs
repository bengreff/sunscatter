//! Forces on a vessel (translational), expressed in an anchor's frame.
//!
//! In an anchor frame the equation of motion is
//! `a = Σ_s g_s(r − r_s) + non-gravitational − a_anchor`, where `a_anchor` is the
//! anchor's *kinematic* acceleration from its ephemeris. That subtraction is
//! exact for any anchor, so the choice of anchor cannot change the physics
//! (checked by the anchor-invariance tests).

use crate::ephem::Snapshot;
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::gen::{j2_accel, schwarzschild_accel};
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// Aerodynamic properties used for drag (isotropic in the prototype).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DragModel {
    /// Drag coefficient times reference area (m²).
    pub cd_area: f64,
    pub mass: f64,
}

/// The set of gravity sources simulated for a segment (after the cutoff).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActiveSources(pub Vec<usize>);

impl ActiveSources {
    /// Sources whose acceleration at the vessel exceeds the world cutoff.
    /// Decided once per segment from the start state, so it is deterministic.
    pub fn select(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> Self {
        let keep = world
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                let d = (r - snap.relative_r(s.node, anchor)).length();
                s.gm / (d * d) >= world.cutoff
            })
            .map(|(i, _)| i)
            .collect();
        ActiveSources(keep)
    }
}

/// Everything the dynamics need for one evaluation.
pub struct ForceContext<'a> {
    pub world: &'a World,
    pub anchor: NodeId,
    pub active: &'a ActiveSources,
    pub drag: Option<DragModel>,
    /// Thrust acceleration (inertial axes, m/s²), constant over the step.
    pub thrust: DVec3,
}

impl ForceContext<'_> {
    /// Acceleration of a vessel at `(r, v)` relative to the anchor at time `t`.
    pub fn accel(&self, t: Epoch, r: DVec3, v: DVec3) -> DVec3 {
        let snap = self.world.snapshot(t);
        self.accel_with(&snap, r, v)
    }

    pub fn accel_with(&self, snap: &Snapshot, r: DVec3, v: DVec3) -> DVec3 {
        let root = self.world.eph.root();
        let anchor_kin = snap.relative(self.anchor, root);
        let mut a = -anchor_kin.a + self.thrust;
        for &i in &self.active.0 {
            let s = &self.world.sources[i];
            // Positions only, except where a velocity is needed (drag, 1PN).
            let d = r - snap.relative_r(s.node, self.anchor);
            let d2 = d.length_squared();
            a -= d * (s.gm / (d2 * d2.sqrt()));
            if let Some(p) = &s.physical {
                if p.j2 != 0.0 {
                    a += j2_accel(d, s.gm, p.j2, p.radius_eq, p.rotation.pole(snap.t));
                }
                if let (Some(atm), Some(drag)) = (p.atmosphere, self.drag) {
                    // Cheap spherical bound before the exact ellipsoid altitude.
                    if d2.sqrt() - p.radius_eq < atm.top {
                        let fixed = p.rotation.to_fixed(Vec3::from_raw(d), snap.t);
                        let rho = atm.density(p.altitude(fixed));
                        if rho > 0.0 {
                            let body_v = snap.relative(s.node, self.anchor).v;
                            let v_air = body_v + p.rotation.omega(snap.t).raw().cross(d);
                            let v_rel = v - v_air;
                            a -= v_rel * (0.5 * rho * v_rel.length() * drag.cd_area / drag.mass);
                        }
                    }
                }
            }
            if s.relativistic {
                a += schwarzschild_accel(d, v - snap.relative(s.node, self.anchor).v, s.gm);
            }
        }
        a
    }
}

/// Altitude of a vessel above a body's solid surface (terrain, or sea level
/// where the body has an ocean), and its body-fixed
/// position, from an anchor-relative position.
pub fn altitude_above(
    world: &World,
    snap: &Snapshot,
    anchor: NodeId,
    body: NodeId,
    r: DVec3,
) -> (f64, Vec3<BodyFixed>) {
    let src = world.source(body).expect("body is a source");
    let p = src.physical.as_ref().expect("body has physical data");
    let d = r - snap.relative_r(body, anchor);
    let fixed = p.rotation.to_fixed(Vec3::from_raw(d), snap.t);
    (p.altitude_above_surface(fixed), fixed)
}
