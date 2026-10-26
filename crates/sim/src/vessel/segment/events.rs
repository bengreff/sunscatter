//! Events found on a segment's dense output between two samples: surface
//! contact, atmosphere entry (bare segments) and the live boundary
//! (perturbed segments, `vessel::limbo`), each by bisection.

use super::{Sample, Segment};
use crate::ephem::SnapshotCache;
use crate::forces::altitude_above;
use crate::frame::{NodeId, Vec3};
use crate::integrate::hermite5;
use crate::vessel::limbo::{each_air, Limbo};
use crate::world::World;

impl Segment {
    /// First contact with a surface between two samples, by bisection on the
    /// interpolated altitude.
    pub(super) fn find_contact(
        &self,
        world: &World,
        snaps: &SnapshotCache,
        a: &Sample,
        b: &Sample,
    ) -> Option<(f64, NodeId)> {
        // Far moon systems and dwarf planets are skipped without evaluating them.
        let d_star = snaps.with(self.t0.add_seconds(b.s.t), |snap| world.star_distance(snap, self.anchor, b.s.r));
        let near = |src: &crate::world::Source, reach: f64| {
            snaps.with(self.t0.add_seconds(b.s.t), |snap| {
                world.may_reach(snap, self.anchor, b.s.r, d_star, src, reach)
                    && (b.s.r - snap.relative_r(src.node, self.anchor)).length() <= reach
            })
        };
        for src in world.surfaces() {
            let alt = |t: f64| {
                let (r, _) = hermite5(&a.s, &b.s, t);
                let snap = world.snapshot(self.t0.add_seconds(t));
                altitude_above(world, &snap, self.anchor, src.node, r).0 - self.contact_height
            };
            let radius = src.physical.as_ref().map_or(0.0, |p| p.radius_eq);
            if !near(src, radius + 200_000.0) || alt(b.s.t) >= 0.0 {
                continue;
            }
            let (mut lo, mut hi) = (a.s.t, b.s.t);
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if alt(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some((hi, src.node));
        }
        None
    }

    /// First descent below an atmosphere's top between two samples (from
    /// at or above it at `a` to below at `b`), by bisection on the
    /// interpolated altitude above the ellipsoid.
    pub(super) fn find_entry(
        &self,
        world: &World,
        snaps: &SnapshotCache,
        a: &Sample,
        b: &Sample,
    ) -> Option<(f64, NodeId)> {
        if let Some(limbo) = self.limbo.as_deref() {
            return self.find_live_boundary(world, limbo, a, b);
        }
        let d_star = snaps.with(self.t0.add_seconds(b.s.t), |snap| world.star_distance(snap, self.anchor, b.s.r));
        for src in &world.sources {
            let Some(p) = src.physical.as_ref() else { continue };
            let Some(atm) = &p.atmosphere else { continue };
            let reach = p.radius_eq + atm.top + 200_000.0;
            let near = snaps.with(self.t0.add_seconds(b.s.t), |snap| {
                world.may_reach(snap, self.anchor, b.s.r, d_star, src, reach)
                    && (b.s.r - snap.relative_r(src.node, self.anchor)).length() <= reach
            });
            if !near {
                continue;
            }
            let alt = |t: f64| {
                let (r, _) = hermite5(&a.s, &b.s, t);
                let snap = world.snapshot(self.t0.add_seconds(t));
                let d = r - snap.relative_r(src.node, self.anchor);
                p.altitude(p.rotation.to_fixed(Vec3::from_raw(d), snap.t)) - atm.top
            };
            if alt(b.s.t) >= 0.0 || alt(a.s.t) < 0.0 {
                continue;
            }
            let (mut lo, mut hi) = (a.s.t, b.s.t);
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if alt(mid) >= 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some((hi, src.node));
        }
        None
    }

    /// First crossing into air that needs live flight (the limbo model's
    /// boundary) between two samples, by bisection on the interpolated
    /// state; the body is the one whose air it is.
    fn find_live_boundary(&self, world: &World, limbo: &Limbo, a: &Sample, b: &Sample) -> Option<(f64, NodeId)> {
        let live = |t: f64| {
            let (r, v) = hermite5(&a.s, &b.s, t);
            let snap = world.snapshot(self.t0.add_seconds(t));
            let mut body = None;
            each_air(world, &snap, self.anchor, r, v, |air| {
                if body.is_none() && air.altitude < air.top && limbo.boundary.exceeded(air.rho, air.v_rel.length()) {
                    body = Some(air.body);
                }
            });
            body
        };
        if live(a.s.t).is_some() {
            return None;
        }
        let body = live(b.s.t)?;
        let (mut lo, mut hi) = (a.s.t, b.s.t);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if live(mid).is_none() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some((hi, live(hi).unwrap_or(body)))
    }
}
