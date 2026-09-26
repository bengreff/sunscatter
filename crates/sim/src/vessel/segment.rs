//! Coast segments: "the prediction is the truth" (D024).
//!
//! A segment is integrated once, in chunks of accepted steps, and stores the
//! step endpoints. The vessel's state at any time inside it is a quintic
//! Hermite evaluation of those samples, so time warp only changes how fast the
//! segment is sampled. Extending in chunks is bit-identical to one long run
//! because the integrator state (including compensation terms and the next
//! step size) is stored and the step limit (`horizon`) never changes.

use super::anchor::preferred_anchor;
use crate::forces::{altitude_above, ActiveSources, DragModel, ForceContext};
use crate::frame::NodeId;
use crate::integrate::{hermite5, Dopri5, StepSample, StepState, Tolerance};
use crate::math::Compensated;
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// Integration tolerance for coasts (position ~1 cm absolute, 1e-11 relative).
pub fn coast_tolerance() -> Tolerance {
    Tolerance { abs_r: 1e-2, abs_v: 1e-5, rel: 1e-11, h_min: 1e-4, h_max: 6.0 * 3600.0 }
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sample {
    pub anchor: NodeId,
    pub s: StepSample,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EndKind {
    /// Reached a body's surface (minus the vessel's contact height).
    Surface { body: NodeId },
    /// Reached the maximum horizon.
    Horizon,
    /// The integration could not continue (non-finite state or force). The
    /// segment ends at its last good sample; the vessel stays there.
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SegmentEnd {
    pub t: f64,
    pub kind: EndKind,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Segment {
    /// Epoch of local time 0.
    pub t0: Epoch,
    pub samples: Vec<Sample>,
    pub end: Option<SegmentEnd>,
    state: StepState,
    anchor: NodeId,
    active: ActiveSources,
    drag: Option<DragModel>,
    /// Height above the surface at which contact happens (m).
    contact_height: f64,
    /// Local time limit of the integration (fixed for the segment's life).
    horizon: f64,
    fixed_anchor: bool,
}

/// Initial conditions and settings of a coast.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CoastStart {
    pub anchor: NodeId,
    pub r: DVec3,
    pub v: DVec3,
    pub drag: Option<DragModel>,
    /// Height above the surface at which contact happens (m).
    pub contact_height: f64,
    /// Local time limit of the integration (s).
    pub horizon: f64,
    /// Keep the starting anchor for the whole segment (tests of anchor
    /// invariance); normally the precision policy may re-anchor.
    pub fixed_anchor: bool,
}

impl Segment {
    /// Starts a coast at `t0`.
    pub fn new(world: &World, t0: Epoch, start: CoastStart) -> Self {
        let CoastStart { anchor, r, v, drag, contact_height, horizon, fixed_anchor } = start;
        let snap = world.snapshot(t0);
        let active = ActiveSources::select(world, &snap, anchor, r);
        let mut seg = Segment {
            t0,
            samples: Vec::new(),
            end: None,
            state: StepState { t: 0.0, r: Compensated::new(r), v: Compensated::new(v), a: DVec3::ZERO, h: 1.0 },
            anchor,
            active,
            drag,
            contact_height,
            horizon,
            fixed_anchor,
        };
        let integ = Dopri5::new(coast_tolerance());
        let ctx = seg.context(world);
        seg.state = integ.start(&|t: f64, r, v| ctx.accel(t0.add_seconds(t), r, v), 0.0, r, v, 1.0);
        seg.samples.push(Sample { anchor, s: StepSample { t: 0.0, r, v, a: seg.state.a } });
        seg
    }

    fn context<'a>(&'a self, world: &'a World) -> ForceContext<'a> {
        ForceContext { world, anchor: self.anchor, active: &self.active, drag: self.drag, thrust: DVec3::ZERO }
    }

    /// Local time of the last computed sample.
    pub fn computed_until(&self) -> f64 {
        self.samples.last().map_or(0.0, |s| s.s.t)
    }

    pub fn finished(&self) -> bool {
        self.end.is_some()
    }

    /// Integrates up to `max_steps` more accepted steps (stops at the end).
    pub fn extend(&mut self, world: &World, max_steps: usize) {
        let integ = Dopri5::new(coast_tolerance());
        for _ in 0..max_steps {
            if self.end.is_some() {
                return;
            }
            let prev = *self.samples.last().expect("segment has a first sample");
            let t0 = self.t0;
            let step = {
                // Field-level borrows: the context reads `active`/`drag` while
                // the integrator mutates `state`.
                let ctx = ForceContext {
                    world,
                    anchor: self.anchor,
                    active: &self.active,
                    drag: self.drag,
                    thrust: DVec3::ZERO,
                };
                integ.step(&|t: f64, r, v| ctx.accel(t0.add_seconds(t), r, v), &mut self.state, self.horizon)
            };
            let Ok(sample) = step else {
                self.end = Some(SegmentEnd { t: prev.s.t, kind: EndKind::Failed });
                return;
            };
            let new = Sample { anchor: self.anchor, s: sample };
            if let Some((t_hit, body)) = self.find_contact(world, &prev, &new) {
                let (r, v) = hermite5(&prev.s, &new.s, t_hit);
                let a = self.context(world).accel(t0.add_seconds(t_hit), r, v);
                self.samples.push(Sample { anchor: self.anchor, s: StepSample { t: t_hit, r, v, a } });
                self.end = Some(SegmentEnd { t: t_hit, kind: EndKind::Surface { body } });
                return;
            }
            self.samples.push(new);
            if self.state.t >= self.horizon {
                self.end = Some(SegmentEnd { t: self.horizon, kind: EndKind::Horizon });
                return;
            }
            self.maybe_reanchor(world);
        }
    }

    /// Switches anchor at a step boundary if the policy prefers another one.
    /// An exact change of coordinates: the physics is unaffected.
    fn maybe_reanchor(&mut self, world: &World) {
        if self.fixed_anchor {
            return;
        }
        let t = self.t0.add_seconds(self.state.t);
        let snap = world.snapshot(t);
        let r = self.state.r.value();
        let next = preferred_anchor(world, &snap, self.anchor, r);
        if next == self.anchor {
            return;
        }
        let shift = snap.relative(self.anchor, next);
        let (r_new, v_new) = (r + shift.r, self.state.v.value() + shift.v);
        self.anchor = next;
        let a_new = self.context(world).accel_with(&snap, r_new, v_new);
        self.state = StepState {
            t: self.state.t,
            r: Compensated::new(r_new),
            v: Compensated::new(v_new),
            a: a_new,
            h: self.state.h,
        };
        self.samples.push(Sample { anchor: next, s: StepSample { t: self.state.t, r: r_new, v: v_new, a: a_new } });
    }

    /// First contact with a surface between two samples, by bisection on the
    /// interpolated altitude.
    fn find_contact(&self, world: &World, a: &Sample, b: &Sample) -> Option<(f64, NodeId)> {
        let snap_b = world.snapshot(self.t0.add_seconds(b.s.t));
        for src in world.surfaces() {
            let alt = |t: f64| {
                let (r, _) = hermite5(&a.s, &b.s, t);
                let snap = world.snapshot(self.t0.add_seconds(t));
                altitude_above(world, &snap, self.anchor, src.node, r).0 - self.contact_height
            };
            let radius = src.physical.as_ref().map_or(0.0, |p| p.radius_eq);
            let d = (b.s.r - snap_b.relative_r(src.node, self.anchor)).length();
            if d > radius + 200_000.0 || alt(b.s.t) >= 0.0 {
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

    /// Drops samples the vessel has passed, keeping the one at or before `t`
    /// (still needed to interpolate at `t`). Bounds memory over long coasts;
    /// evaluation at or after `t` is unchanged.
    pub fn prune_before(&mut self, t: f64) {
        let first_after = self.samples.partition_point(|s| s.s.t <= t);
        if first_after > 1 {
            // Keep a pre-switch duplicate pair together (same time, two anchors).
            let mut keep_from = first_after - 1;
            while keep_from > 0 && self.samples[keep_from - 1].s.t == self.samples[keep_from].s.t {
                keep_from -= 1;
            }
            self.samples.drain(..keep_from);
        }
    }

    /// State at local time `t`: (anchor, r, v). `None` if not computed yet.
    pub fn eval(&self, t: f64) -> Option<(NodeId, DVec3, DVec3)> {
        let last = self.samples.last()?;
        if t > last.s.t {
            return None;
        }
        // First sample with time >= t.
        let i = self.samples.partition_point(|s| s.s.t < t);
        if i == 0 {
            let s = &self.samples[0];
            return Some((s.anchor, s.s.r, s.s.v));
        }
        let (mut a, b) = (self.samples[i - 1], self.samples[i]);
        if a.anchor != b.anchor {
            // `a` is the pre-switch duplicate at the same time as `b`.
            a = b;
        }
        let (r, v) = hermite5(&a.s, &b.s, t);
        Some((b.anchor, r, v))
    }
}
