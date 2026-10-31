//! A vessel's stored trajectory: a chain of coast segments (review sim 11,
//! realism-1 §6a).
//!
//! Each segment ends at an event (surface contact, horizon, ephemeris end,
//! failure, a change of attitude regime); the next one starts from its last
//! sample and mass, so the chain is a pure function of the first segment's
//! start. Segments are only created from stored state, so extending in
//! chunks gives the same chain, bit for bit, as one long run (rule 4).
//! Planned burns are nodes, flown live by the maneuver program (D126).

use super::limbo::Limbo;
use super::pointing::Pointing;
use super::segment::{CoastStart, EndKind, Segment};
use crate::frame::NodeId;
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Trajectory {
    /// The segments, in time order. The first contains the vessel's time
    /// (earlier ones are dropped as the vessel passes them).
    pub segments: Vec<Segment>,
    /// Horizon of a coast (s).
    coast_horizon: f64,
    /// The perturbation model of the vessel's segments ([`super::limbo`];
    /// each segment has its own pointing). `None`: bare segments.
    #[serde(default)]
    limbo: Option<Limbo>,
}

impl Trajectory {
    /// A trajectory of bare coasts from `start` at `t` with mass `mass`.
    /// `start.horizon` is the horizon of each coast.
    pub fn new(world: &World, t: Epoch, start: CoastStart, mass: f64) -> Self {
        let first = coast(world, t, start, mass, start.horizon, None);
        Trajectory { segments: vec![first], coast_horizon: start.horizon, limbo: None }
    }

    /// As [`Trajectory::new`], with perturbed segments ([`super::limbo`]):
    /// the first coast holds `limbo`'s pointing.
    pub fn perturbed(world: &World, t: Epoch, start: CoastStart, mass: f64, limbo: Limbo) -> Self {
        let first = coast(world, t, start, mass, start.horizon, Some(&limbo));
        Trajectory { segments: vec![first], coast_horizon: start.horizon, limbo: Some(limbo) }
    }

    /// A trajectory made of one given segment (tests, and callers that build
    /// a segment themselves).
    pub fn from_segment(segment: Segment) -> Self {
        let coast_horizon = segment.initial().horizon;
        // A perturbed segment's continuations are perturbed alike.
        let limbo = segment.limbo().cloned();
        Trajectory { segments: vec![segment], coast_horizon, limbo }
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

    /// Proper time offset δ = τ − t at `t` (s). `None` if not stored.
    pub fn proper_time_at(&self, t: Epoch) -> Option<f64> {
        let seg = self.segment_at(t)?;
        seg.proper_time_at(t.seconds_since(seg.t0))
    }

    /// The first atmosphere entry ([`Segment::entry`]) at or after `t`.
    pub fn entry_from(&self, t: Epoch) -> Option<(Epoch, NodeId)> {
        (self.segments.iter())
            .filter_map(|s| s.entry.map(|(te, body)| (s.t0.add_seconds(te), body)))
            .find(|(e, _)| e.seconds_since(t) >= 0.0)
    }

    /// Mass at `t` (kg); `None` before the first segment.
    pub fn mass_at(&self, t: Epoch) -> Option<f64> {
        let seg = self.segment_at(t)?;
        Some(seg.mass_at(t.seconds_since(seg.t0).min(seg.computed_until())))
    }

    /// Integrates until `until` is covered (or the chain ends), with at most
    /// `max_steps` new steps. Returns the steps taken.
    pub fn extend(&mut self, world: &World, until: Epoch, max_steps: usize) -> usize {
        let mut budget = max_steps;
        loop {
            let last = self.segments.last_mut().expect("a trajectory has a segment");
            if last.finished() {
                match self.next_segment(world) {
                    Some(next) => {
                        self.segments.push(next);
                        continue;
                    }
                    None => return max_steps - budget,
                }
            }
            let want = until.seconds_since(last.t0);
            if last.computed_until() >= want || budget == 0 {
                return max_steps - budget;
            }
            budget -= last.extend_until(world, want, budget);
        }
    }

    /// The coast after the (finished) last one, if the chain continues.
    fn next_segment(&self, world: &World) -> Option<Segment> {
        let prev = self.last();
        let end = prev.end?;
        if !continues(prev) {
            return None;
        }
        let t = prev.t0.add_seconds(end.t);
        let last = prev.samples.last().expect("segment has a sample");
        let start =
            CoastStart { anchor: last.anchor, r: last.s.r, v: last.s.v, proper_time: last.delta, ..prev.initial() };
        // A coast continuing a coast at its horizon keeps its pointing (a
        // pure function of time); after a change of automatic reference
        // the attitude is not known ahead.
        let limbo = self.limbo.as_ref().map(|l| match (end.kind, prev.limbo()) {
            (EndKind::Horizon, Some(p)) => l.with(p.pointing.clone()),
            _ => l.with(Pointing::Mean),
        });
        // A craft of parts' law of its tanks goes on from where the coast
        // left it (D119).
        let limbo = limbo.map(|mut l| {
            let state = (last.anchor, last.s.r, last.s.v);
            let prev_law = prev.limbo().and_then(|p| p.law.as_deref());
            l.law = prev_law.and_then(|law| {
                let heat = l.law_heat(world, t, state)?;
                Some(Box::new(law.continued(l.ship_def()?, end.t, &heat)))
            });
            l
        });
        Some(coast(world, t, start, prev.mass_at(end.t), self.coast_horizon, limbo.as_ref()))
    }

    /// The first coast that follows one that ended where its automatic
    /// reference changed ([`EndKind::Regime`]), starting after `t` and at
    /// or before `until`: its start (the attitude regime is checked there).
    pub fn next_regime_start(&self, t: Epoch, until: Epoch) -> Option<Epoch> {
        let regime = |s: &Segment| s.end.is_some_and(|e| e.kind == EndKind::Regime);
        (self.segments.windows(2))
            .filter(|w| regime(&w[0]))
            .map(|w| w[1].t0)
            .find(|&e| e.seconds_since(t) > 0.0 && e.seconds_since(until) <= 0.0)
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
}

/// Whether a finished segment is followed by another.
fn continues(seg: &Segment) -> bool {
    matches!(seg.end.map(|e| e.kind), Some(EndKind::Horizon | EndKind::Regime))
}

/// A coast from `start` at `t` with mass `mass` up to `horizon`, with the
/// perturbation model `limbo` (its pointing) if any.
fn coast(world: &World, t: Epoch, start: CoastStart, mass: f64, horizon: f64, limbo: Option<&Limbo>) -> Segment {
    let start = CoastStart { horizon, ..start };
    Segment::start(world, t, start, mass, EndKind::Horizon, limbo.map(|l| Box::new(l.clone())))
}
