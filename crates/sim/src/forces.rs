//! Forces on a vessel (translational), expressed in an anchor's frame.
//!
//! In an anchor frame the equation of motion is
//! `a = Σ_s g_s(r − r_s) + non-gravitational − a_anchor`, where `a_anchor` is the
//! anchor's *kinematic* acceleration from its ephemeris. That subtraction is
//! exact for any anchor, so the choice of anchor cannot change the physics
//! (checked by the anchor-invariance tests).
//!
//! Sources whose tidal acceleration is below the world cutoff are *cut*
//! (D023): the vessel feels their pull at the anchor instead of at itself, so
//! their share of `a_anchor` cancels and only the tidal term, bounded by the
//! cutoff, is neglected. (Dropping a cut source's pull altogether would leave
//! its full pull on the anchor as a fictitious force on the vessel.)

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

/// The gravity sources simulated in full (indices into `World::sources`,
/// sorted). Every other source is *cut*: it pulls the vessel exactly as it
/// pulls the anchor, so only its tidal term (the difference) is neglected.
/// A segment only ever adds sources ([`ActiveSources::add`]), at step
/// boundaries, so the set is part of the stored integrator state.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ActiveSources(pub Vec<usize>);

impl ActiveSources {
    /// The anchor (if it is a source) and every source whose tidal
    /// acceleration at the vessel relative to the anchor,
    /// `|g_s(vessel) − g_s(anchor)|`, is at least the world cutoff.
    pub fn select(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> Self {
        let keep = world
            .sources
            .iter()
            .enumerate()
            .filter(|(_, s)| s.node == anchor || tidal(s.gm, snap.relative_r(s.node, anchor), r) >= world.cutoff)
            .map(|(i, _)| i)
            .collect();
        ActiveSources(keep)
    }

    /// Adds the sources of `other`. Returns whether the set grew.
    pub fn add(&mut self, other: &ActiveSources) -> bool {
        let before = self.0.len();
        for &i in &other.0 {
            if let Err(pos) = self.0.binary_search(&i) {
                self.0.insert(pos, i);
            }
        }
        self.0.len() != before
    }
}

/// Point-mass gravity at `d` from a source.
fn point_gravity(gm: f64, d: DVec3) -> DVec3 {
    let d2 = d.length_squared();
    -d * (gm / (d2 * d2.sqrt()))
}

/// Magnitude of a source's tidal acceleration at `r` relative to the origin
/// (the anchor), for a source at `r_s`.
fn tidal(gm: f64, r_s: DVec3, r: DVec3) -> f64 {
    (point_gravity(gm, r - r_s) - point_gravity(gm, -r_s)).length()
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
        let mut active = self.active.0.iter().peekable();
        for (i, s) in self.world.sources.iter().enumerate() {
            // Positions only, except where a velocity is needed (drag, 1PN).
            let r_s = snap.relative_r(s.node, self.anchor);
            if active.next_if_eq(&&i).is_none() {
                // Cut: it pulls the vessel as it pulls the anchor, which
                // cancels its share of the anchor's acceleration (rule 1).
                if s.node != self.anchor {
                    a += point_gravity(s.gm, -r_s);
                }
                continue;
            }
            let d = r - r_s;
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

/// Ambient pressure (Pa) at an anchor-relative position: the atmosphere of
/// whichever body the vessel is in (zero in vacuum). Uses the same altitude
/// above the ellipsoid as drag.
pub fn ambient_pressure(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> f64 {
    let mut p_total = 0.0;
    for src in &world.sources {
        let Some(p) = src.physical.as_ref() else { continue };
        let Some(atm) = p.atmosphere else { continue };
        let d = r - snap.relative_r(src.node, anchor);
        if d.length() - p.radius_eq >= atm.top {
            continue;
        }
        let fixed = p.rotation.to_fixed(Vec3::from_raw(d), snap.t);
        p_total += atm.pressure(p.altitude(fixed));
    }
    p_total
}
