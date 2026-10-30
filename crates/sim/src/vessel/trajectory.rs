//! A vessel's stored trajectory: a chain of coast and burn segments (review
//! sim 11, realism-1 §6a).
//!
//! Each segment ends at an event (surface contact, horizon, ephemeris end,
//! failure, burn start or burn end); the next one starts from its last sample
//! and mass, so the chain is a pure function of the first segment's start and
//! the flight plan. Segments are only created from stored state, so extending
//! in chunks gives the same chain, bit for bit, as one long run (rule 4).
//!
//! Burns are flown by the craft's attitude (D075, [`super::flown`]). A burn
//! predicted ahead of time starts from an estimate of the attitude at
//! ignition (the engine settled on the burn's direction, the state the
//! maneuver hold's turn ends in); the vessel checks it when it reaches the
//! ignition and, if its actual attitude differs, flies the burn again from
//! it ([`Trajectory::refly`]): the stored trajectory is then what is flown.

use super::attitude::{Attitude, AttitudeControl};
use super::burn::{BurnLimits, FlightPlan};
use super::flown::{direction_and_rate, BurnCraft, Flight};
use super::hold::AimKey;
use super::limbo::{Limbo, LowThrust, LOW_THRUST};
use super::pointing::{self, Pointing};
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
    /// Propellant limits of the burns.
    #[serde(default)]
    limits: BurnLimits,
    /// The craft that flies the burns (`None`: thrust along the law).
    #[serde(default)]
    craft: Option<BurnCraft>,
    /// The attitude and control when the trajectory was started: a burn
    /// igniting right then starts from them.
    #[serde(default)]
    seed: Option<(Attitude, AttitudeControl)>,
    /// The perturbation model of the vessel's segments ([`super::limbo`];
    /// each segment has its own pointing). `None`: bare segments.
    #[serde(default)]
    limbo: Option<Limbo>,
}

/// What the burns of a trajectory are flown with.
#[derive(Clone, Copy)]
struct Flying {
    coast_horizon: f64,
    limits: BurnLimits,
    craft: Option<BurnCraft>,
    /// The attitude the trajectory started with: predicted burns turn
    /// from it.
    seed: Option<Attitude>,
}

impl Trajectory {
    /// A trajectory from `start` at `t` with mass `mass`, following `plan`
    /// (burns igniting before `t` are skipped) within `limits`.
    /// `start.horizon` is the horizon of coasts with no burn ahead.
    /// Burns point their thrust along their law exactly (no craft).
    pub fn new(world: &World, t: Epoch, start: CoastStart, mass: f64, plan: &FlightPlan, limits: BurnLimits) -> Self {
        let fly = Flying { coast_horizon: start.horizon, limits, craft: None, seed: None };
        let first = make_segment(world, plan, t, start, mass, 0, fly, BurnFrom::Coast(DVec3::ZERO), None);
        Trajectory { segments: vec![first], coast_horizon: start.horizon, limits, craft: None, seed: None, limbo: None }
    }

    /// As [`Trajectory::new`], with the burns flown by `craft`'s attitude
    /// (D075) from `seed` (the attitude and control now).
    #[allow(clippy::too_many_arguments)]
    pub fn flown(
        world: &World,
        t: Epoch,
        start: CoastStart,
        mass: f64,
        plan: &FlightPlan,
        limits: BurnLimits,
        craft: BurnCraft,
        seed: (Attitude, AttitudeControl),
    ) -> Self {
        Self::perturbed(world, t, start, mass, plan, limits, craft, seed, None)
    }

    /// As [`Trajectory::flown`], with perturbed segments ([`super::limbo`]):
    /// the first coast holds `limbo`'s pointing, coasts after burns start
    /// with an unknown attitude ([`Pointing::Mean`]; the vessel restarts
    /// them once its attitude is known), and burns below [`LOW_THRUST`] are
    /// flown as perturbed segments with continuous thrust.
    #[allow(clippy::too_many_arguments)]
    pub fn perturbed(
        world: &World,
        t: Epoch,
        start: CoastStart,
        mass: f64,
        plan: &FlightPlan,
        limits: BurnLimits,
        craft: BurnCraft,
        seed: (Attitude, AttitudeControl),
        limbo: Option<Limbo>,
    ) -> Self {
        let fly = Flying { coast_horizon: start.horizon, limits, craft: Some(craft), seed: Some(seed.0) };
        let first = make_segment(world, plan, t, start, mass, 0, fly, BurnFrom::Exact(seed), limbo.as_ref());
        let seed = Some(seed);
        Trajectory { segments: vec![first], coast_horizon: start.horizon, limits, craft: Some(craft), seed, limbo }
    }

    /// A trajectory made of one given segment (tests, and callers that build
    /// a segment themselves). Burns of a plan are followed after it ends.
    pub fn from_segment(segment: Segment) -> Self {
        let coast_horizon = segment.initial().horizon;
        let limits = BurnLimits::default();
        // A perturbed segment's continuations are perturbed alike.
        let limbo = segment.limbo().cloned();
        Trajectory { segments: vec![segment], coast_horizon, limits, craft: None, seed: None, limbo }
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
    /// `max_steps` new steps, starting the next segments from `plan`.
    /// Returns the steps taken.
    pub fn extend(&mut self, world: &World, plan: &FlightPlan, until: Epoch, max_steps: usize) -> usize {
        let mut budget = max_steps;
        loop {
            let last = self.segments.last_mut().expect("a trajectory has a segment");
            if last.finished() {
                match self.next_segment(world, plan) {
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

    /// The segment after the (finished) last one, if the chain continues.
    fn next_segment(&self, world: &World, plan: &FlightPlan) -> Option<Segment> {
        let prev = self.last();
        let end = prev.end?;
        let (t, index) = match end.kind {
            EndKind::BurnStart => (plan.burns.get(prev.plan_index)?.t_start, prev.plan_index),
            EndKind::BurnEnd => (prev.t0.add_seconds(end.t), prev.plan_index + 1),
            EndKind::Horizon | EndKind::Regime => (prev.t0.add_seconds(end.t), prev.plan_index),
            EndKind::Surface { .. } | EndKind::EphemerisEnd | EndKind::Failed => return None,
        };
        let last = prev.samples.last().expect("segment has a sample");
        let start =
            CoastStart { anchor: last.anchor, r: last.s.r, v: last.s.v, proper_time: last.delta, ..prev.initial() };
        // A burn right after a burn starts where that one ended; after a
        // coast, the acceleration there gives the burn direction's rate.
        let from = prev.flight_end().map_or(BurnFrom::Coast(last.s.a), BurnFrom::Exact);
        // A coast continuing a coast keeps its pointing (a pure function of
        // time); after a burn the attitude is not known ahead.
        let limbo = self.limbo.as_ref().map(|l| match (end.kind, prev.limbo()) {
            (EndKind::Horizon, Some(p)) if matches!(prev.kind, SegmentKind::Coast) => l.with(p.pointing.clone(), None),
            _ => l.with(Pointing::Mean, None),
        });
        // A craft of parts' law of its tanks goes on from where a coast left
        // it (D119); after a predicted burn there is none (the burn is
        // flown live, and the coast after it restarts from the ship).
        let limbo = limbo.map(|mut l| {
            let state = (last.anchor, last.s.r, last.s.v);
            let prev_law =
                prev.limbo().and_then(|p| p.law.as_deref()).filter(|_| matches!(prev.kind, SegmentKind::Coast));
            l.law = prev_law.and_then(|law| {
                let heat = l.law_heat(world, t, state)?;
                Some(Box::new(law.continued(l.ship_def()?, end.t, &heat)))
            });
            l
        });
        Some(make_segment(world, plan, t, start, prev.mass_at(end.t), index, self.flying(), from, limbo.as_ref()))
    }

    fn flying(&self) -> Flying {
        Flying {
            coast_horizon: self.coast_horizon,
            limits: self.limits,
            craft: self.craft,
            seed: self.seed.map(|s| s.0),
        }
    }

    /// The craft flying the burns, if any.
    pub fn craft(&self) -> Option<&BurnCraft> {
        self.craft.as_ref()
    }

    /// The first burn segment starting after `t` and at or before `until`:
    /// its ignition.
    pub fn next_ignition(&self, t: Epoch, until: Epoch) -> Option<Epoch> {
        (self.segments.iter())
            .filter(|s| matches!(s.kind, SegmentKind::Burn(_)))
            .map(|s| s.t0)
            .find(|&e| e.seconds_since(t) > 0.0 && e.seconds_since(until) <= 0.0)
    }

    /// The first coast segment that follows a burn, or a coast that ended
    /// where its automatic reference changed ([`EndKind::Regime`]), and
    /// starts after `t` and at or before `until`: its start (the attitude
    /// regime is checked there).
    pub fn next_coast_after_burn(&self, t: Epoch, until: Epoch) -> Option<Epoch> {
        let regime = |s: &Segment| s.end.is_some_and(|e| e.kind == EndKind::Regime);
        (self.segments.windows(2))
            .filter(|w| {
                (matches!(w[0].kind, SegmentKind::Burn(_)) || regime(&w[0])) && matches!(w[1].kind, SegmentKind::Coast)
            })
            .map(|w| w[1].t0)
            .find(|&e| e.seconds_since(t) > 0.0 && e.seconds_since(until) <= 0.0)
    }

    /// At a burn's ignition (the current segment, not yet flown past its
    /// start): flies it again from the vessel's actual attitude and control
    /// if they differ from the ones it was predicted with, dropping what
    /// followed it. A craft whose maneuver hold has settled turns at the
    /// burn direction's rate (as predicted; followed holds carry the rate
    /// of their last tick). Returns the attitude the burn starts from.
    /// Idempotent: flying it again from that attitude (and the same
    /// control) changes nothing, so it may run in every call that starts
    /// at the ignition.
    pub(super) fn refly(&mut self, world: &World, mut att: Attitude, control: AttitudeControl) -> Attitude {
        let cur = &self.segments[0];
        if let (SegmentKind::Burn(law), Some(l)) = (cur.kind, cur.limbo().filter(|l| l.thrust.is_some())) {
            // A low-thrust burn: held on its law from the actual attitude.
            let s = cur.initial();
            let snap = world.snapshot(cur.t0);
            // Already held on its law from this attitude (re-flown by an
            // earlier call at the same ignition, which returned it):
            // snapping it onto the law again would move the roll by a
            // rounding, so a call that ends here would change the burn.
            if l.pointing.attitude(world, &snap, s.anchor, s.r, s.v) == Some(att) {
                return att;
            }
            let axis = l.thrust.map_or(DVec3::Z, |t| t.axis);
            let p = pointing::on_law(&law.direction, &att, axis, world, &snap, (s.anchor, s.r, s.v));
            let held = p.attitude(world, &snap, s.anchor, s.r, s.v).unwrap_or(att);
            if p != l.pointing {
                let limbo = Some(Box::new(l.with(p, l.thrust)));
                let (t0, mass, kind, index) = (cur.t0, cur.initial_mass(), cur.kind, cur.plan_index);
                let rebuilt = Segment::start(world, t0, s, mass, kind, EndKind::BurnEnd, index, None, limbo);
                self.segments = vec![rebuilt];
            }
            return held;
        }
        let (SegmentKind::Burn(law), Some(flight)) = (cur.kind, cur.flight()) else { return att };
        if flight.craft.is_none() {
            return att;
        }
        if control.settled == Some(AimKey::BURN) {
            let s = cur.initial();
            att.omega = direction_and_rate(world, &law, cur.t0, (s.anchor, s.r, s.v), flight.accel0).1;
        }
        let fresh = Flight::new(flight.craft, att, control, flight.accel0);
        if fresh.start == flight.start {
            return att;
        }
        let rebuilt = Segment::start(
            world,
            cur.t0,
            cur.initial(),
            cur.initial_mass(),
            cur.kind,
            EndKind::BurnEnd,
            cur.plan_index,
            Some(fresh),
            cur.limbo().map(|l| Box::new(l.clone())),
        );
        self.segments = vec![rebuilt];
        att
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
        let seed = cur.flight().map(|f| f.start).or(self.seed).map_or(BurnFrom::Coast(DVec3::ZERO), BurnFrom::Exact);
        let pointing = cur.limbo().filter(|_| matches!(cur.kind, SegmentKind::Coast)).map(|l| l.pointing.clone());
        let limbo = self.limbo.as_ref().map(|l| l.with(pointing.unwrap_or(Pointing::Mean), None));
        let (t0, start, mass, index) = (cur.t0, cur.initial(), cur.initial_mass(), cur.plan_index);
        let rebuilt = make_segment(world, plan, t0, start, mass, index, self.flying(), seed, limbo.as_ref());
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
    matches!(seg.end.map(|e| e.kind), Some(EndKind::BurnStart | EndKind::BurnEnd | EndKind::Horizon | EndKind::Regime))
}

/// What a burn starting a segment is flown from (passed once per segment).
#[derive(Clone, Copy)]
#[allow(clippy::large_enum_variant)]
enum BurnFrom {
    /// This attitude and control: a burn at the trajectory's start or
    /// right after another burn.
    Exact((Attitude, AttitudeControl)),
    /// A prediction after a coast that ended with acceleration `a` (for
    /// the burn direction's rate): the craft settled on the burn
    /// ([`BurnCraft::settled_on`]).
    Coast(DVec3),
}

/// The segment starting at `t`: the burn `plan.burns[i]` if it ignites at `t`
/// (with `i` the first burn from `index` not missed), else a coast up to
/// that burn's ignition (or `coast_horizon`). Burns are flown within
/// `limits` (burnout, or no mass flow with infinite propellant), by the
/// craft from `from`. With a perturbation model `limbo`, a coast takes its
/// pointing, a burn below [`LOW_THRUST`] is flown as a perturbed segment
/// held on its law, and a heavier burn gets the model with an unknown
/// attitude (its thrust is flown by ticks).
#[allow(clippy::too_many_arguments)]
fn make_segment(
    world: &World,
    plan: &FlightPlan,
    t: Epoch,
    start: CoastStart,
    mass: f64,
    index: usize,
    fly: Flying,
    from: BurnFrom,
    limbo: Option<&Limbo>,
) -> Segment {
    let Flying { coast_horizon, limits, craft, seed } = fly;
    let i = plan.next_from(index, t);
    match plan.burns.get(i) {
        Some(b) if b.t_start == t => {
            let law = limits.effective(b.law);
            let horizon = b.end.duration_limited(&law, mass, limits.dry_mass).unwrap_or(f64::NAN);
            let (att, control) = match (from, craft) {
                (BurnFrom::Exact(x), _) => x,
                (BurnFrom::Coast(a), Some(c)) => {
                    let (dir, rate) = direction_and_rate(world, &law, t, (start.anchor, start.r, start.v), a);
                    let from = seed.map_or(glam::DQuat::IDENTITY, |s| s.q);
                    c.settled_on(from, dir, rate)
                }
                (BurnFrom::Coast(_), None) => {
                    (Attitude { q: glam::DQuat::IDENTITY, omega: DVec3::ZERO }, Default::default())
                }
            };
            let accel0 = match from {
                BurnFrom::Coast(a) => a,
                BurnFrom::Exact(_) => DVec3::ZERO,
            };
            let kind = SegmentKind::Burn(law);
            if let (Some(l), Some(c)) = (limbo, craft.filter(|_| law.thrust < LOW_THRUST * mass)) {
                // Boil-off drains the tank with the engine: the burn ends
                // when it is empty, its Δv by the total flow (never thrust
                // from the boiled propellant).
                let dry = limits.dry_mass;
                let horizon = b.end.duration_with_loss(&law, mass, dry, l.boil_off).unwrap_or(f64::NAN);
                let start = CoastStart { horizon, ..start };
                let axis = c.engine.mount_dir;
                let snap = world.snapshot(t);
                let p = pointing::on_law(&law.direction, &att, axis, world, &snap, (start.anchor, start.r, start.v));
                let limbo = Some(Box::new(l.with(p, Some(LowThrust { thrust: law.thrust, axis })).without_law()));
                return Segment::start(world, t, start, mass, kind, EndKind::BurnEnd, i, None, limbo);
            }
            let start = CoastStart { horizon, ..start };
            let flight = Flight::new(craft, att, control, accel0);
            // A flown (chemical) burn of minutes takes no boil-off: its thrust
            // is the engine's mass flow alone (`flown::Flight::tick`).
            let limbo = limbo.map(|l| {
                let mut flown = l.with(Pointing::Mean, None).without_law();
                flown.boil_off = 0.0;
                Box::new(flown)
            });
            Segment::start(world, t, start, mass, kind, EndKind::BurnEnd, i, Some(flight), limbo)
        }
        Some(b) if b.t_start.seconds_since(t) <= coast_horizon => {
            let start = CoastStart { horizon: b.t_start.seconds_since(t), ..start };
            let limbo = limbo.map(|l| Box::new(l.clone()));
            Segment::start(world, t, start, mass, SegmentKind::Coast, EndKind::BurnStart, i, None, limbo)
        }
        _ => {
            let start = CoastStart { horizon: coast_horizon, ..start };
            let limbo = limbo.map(|l| Box::new(l.clone()));
            Segment::start(world, t, start, mass, SegmentKind::Coast, EndKind::Horizon, i, None, limbo)
        }
    }
}
