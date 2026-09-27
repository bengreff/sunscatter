//! A vessel's stored trajectory: a chain of coast and burn segments (review
//! sim 11, realism-1 §6a).
//!
//! Each segment ends at an event (surface contact, horizon, ephemeris end,
//! failure, burn start or burn end); the next one starts from its last sample
//! and mass, so the chain is a pure function of the first segment's start and
//! the flight plan. Segments are only created from stored state, so extending
//! in chunks gives the same chain, bit for bit, as one long run (rule 4).

use super::burn::FlightPlan;
use super::segment::{CoastStart, EndKind, Segment, SegmentKind};
use crate::frame::NodeId;
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Trajectory {
    /// The segments, in time order. The first contains the vessel's time
    /// (earlier ones are dropped as the vessel passes them).
    pub segments: Vec<Segment>,
    /// Horizon of a coast with no burn ahead (s).
    coast_horizon: f64,
}

impl Trajectory {
    /// A trajectory from `start` at `t` with mass `mass`, following `plan`
    /// (burns igniting before `t` are skipped). `start.horizon` is the horizon
    /// of coasts with no burn ahead.
    pub fn new(world: &World, t: Epoch, start: CoastStart, mass: f64, plan: &FlightPlan) -> Self {
        let first = make_segment(world, plan, t, start, mass, 0, start.horizon);
        Trajectory { segments: vec![first], coast_horizon: start.horizon }
    }

    /// A trajectory made of one given segment (tests, and callers that build
    /// a segment themselves). Burns of a plan are followed after it ends.
    pub fn from_segment(segment: Segment) -> Self {
        let coast_horizon = segment.initial().horizon;
        Trajectory { segments: vec![segment], coast_horizon }
    }

    /// The segment containing the earliest stored time.
    pub fn current(&self) -> &Segment {
        &self.segments[0]
    }

    pub fn last(&self) -> &Segment {
        self.segments.last().expect("a trajectory has a segment")
    }

    /// Epoch of the last computed sample.
    pub fn computed_until(&self) -> Epoch {
        let last = self.last();
        last.t0.add_seconds(last.computed_until())
    }

    /// Whether the chain has ended for good (surface, ephemeris end, failure).
    pub fn finished(&self) -> bool {
        let last = self.last();
        last.finished() && !continues(last)
    }

    /// The segment covering `t` (the last one starting at or before it).
    pub fn segment_at(&self, t: Epoch) -> Option<&Segment> {
        let n = self.segments.partition_point(|s| t.seconds_since(s.t0) >= 0.0);
        n.checked_sub(1).map(|i| &self.segments[i])
    }

    /// State at `t`: (anchor, r, v). `None` if not stored.
    pub fn eval(&self, t: Epoch) -> Option<(NodeId, DVec3, DVec3)> {
        let seg = self.segment_at(t)?;
        seg.eval(t.seconds_since(seg.t0))
    }

    /// Mass at `t` (kg); `None` before the first segment.
    pub fn mass_at(&self, t: Epoch) -> Option<f64> {
        let seg = self.segment_at(t)?;
        Some(seg.mass_at(t.seconds_since(seg.t0).min(seg.computed_until())))
    }

    /// Integrates until `until` is covered (or the chain ends), with at most
    /// `max_steps` new steps, starting the next segments from `plan`.
    pub fn extend(&mut self, world: &World, plan: &FlightPlan, until: Epoch, max_steps: usize) {
        let mut budget = max_steps;
        loop {
            let last = self.segments.last_mut().expect("a trajectory has a segment");
            if last.finished() {
                match self.next_segment(world, plan) {
                    Some(next) => {
                        self.segments.push(next);
                        continue;
                    }
                    None => return,
                }
            }
            let want = until.seconds_since(last.t0);
            if last.computed_until() >= want || budget == 0 {
                return;
            }
            budget -= last.extend_until(world, want, budget);
        }
    }

    /// The segment after the (finished) last one, if the chain continues.
    fn next_segment(&self, world: &World, plan: &FlightPlan) -> Option<Segment> {
        let prev = self.last();
        let end = prev.end?;
        let (t, index) = match end.kind {
            EndKind::BurnStart => (plan.burns.get(prev.plan_index)?.t_start, prev.plan_index),
            EndKind::BurnEnd => (prev.t0.add_seconds(end.t), prev.plan_index + 1),
            EndKind::Horizon => (prev.t0.add_seconds(end.t), prev.plan_index),
            EndKind::Surface { .. } | EndKind::EphemerisEnd | EndKind::Failed => return None,
        };
        let last = prev.samples.last().expect("segment has a sample");
        let start = CoastStart { anchor: last.anchor, r: last.s.r, v: last.s.v, ..prev.initial() };
        Some(make_segment(world, plan, t, start, prev.mass_at(end.t), index, self.coast_horizon))
    }

    /// Drops what the vessel has passed: whole segments before the one
    /// containing `t`, and that segment's samples before `t`.
    pub fn prune_before(&mut self, t: Epoch) {
        let n = self.segments.partition_point(|s| t.seconds_since(s.t0) >= 0.0);
        if n > 1 {
            self.segments.drain(..n - 1);
        }
        let first = &mut self.segments[0];
        first.prune_before(t.seconds_since(first.t0));
    }

    /// Replaces `plan`'s burns from index `changed` on (the caller checked
    /// that none of them has started): keeps every segment that does not
    /// depend on them bit for bit and rebuilds the rest; the current segment,
    /// if it depends on them, is rebuilt from its own start and integrated
    /// back to `now`.
    pub(super) fn replan(&mut self, world: &World, plan: &FlightPlan, changed: usize, now: Epoch) {
        let keep = self.segments.iter().position(|s| s.plan_index >= changed).unwrap_or(self.segments.len());
        if keep > 0 {
            self.segments.truncate(keep);
            return;
        }
        let cur = &self.segments[0];
        let rebuilt =
            make_segment(world, plan, cur.t0, cur.initial(), cur.initial_mass(), cur.plan_index, self.coast_horizon);
        self.segments = vec![rebuilt];
        self.extend(world, plan, now, usize::MAX);
        self.prune_before(now);
    }

    /// Number of planned burns started (or skipped) by the current segment.
    pub(super) fn burns_started(&self) -> usize {
        let cur = self.current();
        cur.plan_index + usize::from(matches!(cur.kind, SegmentKind::Burn(_)))
    }
}

/// Whether a finished segment is followed by another.
fn continues(seg: &Segment) -> bool {
    matches!(seg.end.map(|e| e.kind), Some(EndKind::BurnStart | EndKind::BurnEnd | EndKind::Horizon))
}

/// The segment starting at `t`: the burn `plan.burns[i]` if it ignites at `t`
/// (with `i` the first burn from `index` not missed), else a coast up to
/// that burn's ignition (or `coast_horizon`).
fn make_segment(
    world: &World,
    plan: &FlightPlan,
    t: Epoch,
    start: CoastStart,
    mass: f64,
    index: usize,
    coast_horizon: f64,
) -> Segment {
    let i = plan.next_from(index, t);
    match plan.burns.get(i) {
        Some(b) if b.t_start == t => {
            let horizon = b.end.duration(&b.law, mass).unwrap_or(f64::NAN);
            let start = CoastStart { horizon, ..start };
            Segment::start(world, t, start, mass, SegmentKind::Burn(b.law), EndKind::BurnEnd, i)
        }
        Some(b) if b.t_start.seconds_since(t) <= coast_horizon => {
            let start = CoastStart { horizon: b.t_start.seconds_since(t), ..start };
            Segment::start(world, t, start, mass, SegmentKind::Coast, EndKind::BurnStart, i)
        }
        _ => {
            let start = CoastStart { horizon: coast_horizon, ..start };
            Segment::start(world, t, start, mass, SegmentKind::Coast, EndKind::Horizon, i)
        }
    }
}
