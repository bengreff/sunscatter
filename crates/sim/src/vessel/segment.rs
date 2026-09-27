//! Trajectory segments: "the prediction is the truth" (D024).
//!
//! A segment is a coast or a burn ([`SegmentKind`]). It is integrated once,
//! in chunks of accepted steps, and stores the step endpoints. The vessel's state at any time inside it is a quintic
//! Hermite evaluation of those samples, so time warp only changes how fast the
//! segment is sampled. Extending in chunks is bit-identical to one long run
//! because the integrator state (including compensation terms and the next
//! step size) is stored and the step limit (`horizon`) never changes.
//!
//! Mass is part of the state: constant in a coast, `m0 − ṁ·t` in a burn
//! (constant mass flow, so the exact expression replaces integration).
//! Proper time (δ = τ − t, realism-1 §4a) is the integrator's extra scalar:
//! integrated with the same stages, outside error control (so the steps are
//! those of the translation alone), stored with each sample with its rate
//! and evaluated between samples by cubic Hermite interpolation.
//! Segments are chained into a [`super::Trajectory`].

use super::anchor::preferred_anchor;
use super::burn::BurnLaw;
use crate::ephem::{Snapshot, SnapshotCache};
use crate::forces::{altitude_above, ActiveSources, DragModel, ForceContext};
use crate::frame::{NodeId, Vec3};
use crate::integrate::{hermite5, Dopri5, Dynamics, StepSample, StepState, Tolerance};
use crate::math::{Compensated, CompensatedScalar};
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
    /// Proper time offset δ = τ − t (s) and its rate at `s.t`.
    #[serde(default)]
    pub delta: f64,
    #[serde(default)]
    pub delta_rate: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EndKind {
    /// Reached a body's surface (minus the vessel's contact height).
    Surface { body: NodeId },
    /// Reached the maximum horizon (the vessel starts a new segment there).
    Horizon,
    /// A coast reached the ignition of the next planned burn.
    BurnStart,
    /// A burn completed.
    BurnEnd,
    /// Reached the end of the ephemeris window: nothing is simulated past it.
    EphemerisEnd,
    /// The integration could not continue (non-finite state or force). The
    /// segment ends at its last good sample; the vessel stays there.
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SegmentEnd {
    pub t: f64,
    pub kind: EndKind,
}

/// What moves the vessel during a segment (besides gravity and drag).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SegmentKind {
    Coast,
    Burn(BurnLaw),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Segment {
    /// Epoch of local time 0.
    pub t0: Epoch,
    pub kind: SegmentKind,
    pub samples: Vec<Sample>,
    pub end: Option<SegmentEnd>,
    state: StepState,
    anchor: NodeId,
    active: ActiveSources,
    drag: Option<DragModel>,
    /// Height above the surface at which contact happens (m).
    contact_height: f64,
    /// Local time limit of the integration (fixed for the segment's life):
    /// the requested horizon, or the ephemeris end if that is sooner.
    horizon: f64,
    fixed_anchor: bool,
    /// Mass at local time 0 (kg).
    mass0: f64,
    /// How the segment ends when it reaches `horizon` before the ephemeris end.
    horizon_end: EndKind,
    /// The initial conditions (to rebuild the segment when a plan changes).
    start: CoastStart,
    /// Planned burns started before this segment (its own index if a burn).
    pub(super) plan_index: usize,
    /// Where the segment first descends into an atmosphere from above
    /// (local time, body): a vessel is flown live from there
    /// ([`super::aerothermal`]); the rest of the segment is a prediction
    /// (with the attitude-independent drag), kept for display.
    #[serde(default)]
    pub entry: Option<(f64, NodeId)>,
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
    /// Local time limit of the integration (s). Capped at the ephemeris end.
    pub horizon: f64,
    /// Keep the starting anchor for the whole segment (tests of anchor
    /// invariance); normally the precision policy may re-anchor.
    pub fixed_anchor: bool,
    /// Proper time offset δ = τ − t at the start (s).
    #[serde(default)]
    pub proper_time: f64,
}

impl Segment {
    /// Starts a coast at `t0` (mass from the drag model, if any).
    pub fn new(world: &World, t0: Epoch, start: CoastStart) -> Self {
        let mass = start.drag.map_or(0.0, |d| d.mass);
        Self::start(world, t0, start, mass, SegmentKind::Coast, EndKind::Horizon, 0)
    }

    /// Starts a segment of `kind` at `t0` with mass `mass`; reaching
    /// `start.horizon` ends it with `horizon_end`. A burn whose law or
    /// duration cannot be flown ends at once as [`EndKind::Failed`].
    pub(super) fn start(
        world: &World,
        t0: Epoch,
        start: CoastStart,
        mass: f64,
        kind: SegmentKind,
        horizon_end: EndKind,
        plan_index: usize,
    ) -> Self {
        let CoastStart { anchor, r, v, drag, contact_height, horizon, fixed_anchor, proper_time } = start;
        let valid = match kind {
            SegmentKind::Coast => true,
            SegmentKind::Burn(law) => {
                law.is_valid() && horizon.is_finite() && horizon >= 0.0 && mass - law.mass_flow * horizon > 0.0
            }
        };
        let horizon = if valid { horizon.min(world.end().seconds_since(t0)).max(0.0) } else { 0.0 };
        let snaps = SnapshotCache::new(&world.eph);
        let active = snaps.with(t0, |snap| ActiveSources::select(world, snap, anchor, r));
        let mut seg = Segment {
            t0,
            kind,
            samples: Vec::new(),
            end: None,
            state: StepState {
                t: 0.0,
                r: Compensated::new(r),
                v: Compensated::new(v),
                a: DVec3::ZERO,
                h: 1.0,
                x: CompensatedScalar::new(proper_time),
                x_rate: 0.0,
            },
            anchor,
            active,
            drag: drag.map(|d| DragModel { mass, ..d }),
            contact_height,
            horizon,
            fixed_anchor,
            mass0: mass,
            horizon_end,
            start,
            plan_index,
            entry: None,
        };
        let integ = Dopri5::new(coast_tolerance());
        let f = seg.forces(world, &snaps);
        seg.state = integ.start(&f, 0.0, r, v, 1.0);
        seg.state.x = CompensatedScalar::new(proper_time);
        let s = StepSample { t: 0.0, r, v, a: seg.state.a };
        seg.samples.push(Sample { anchor, s, delta: proper_time, delta_rate: seg.state.x_rate });
        if !valid {
            seg.end = Some(SegmentEnd { t: 0.0, kind: EndKind::Failed });
        }
        seg
    }

    fn forces<'a, 'w>(&'a self, world: &'a World, snaps: &'a SnapshotCache<'w>) -> Forces<'a, 'w> {
        Forces {
            world,
            snaps,
            t0: self.t0,
            anchor: self.anchor,
            active: &self.active,
            drag: self.drag,
            mass0: self.mass0,
            kind: &self.kind,
        }
    }

    /// Mass at local time `t` (kg).
    pub fn mass_at(&self, t: f64) -> f64 {
        mass_at(&self.kind, self.mass0, t)
    }

    /// The initial conditions this segment was started from.
    pub fn initial(&self) -> CoastStart {
        self.start
    }

    /// Mass at local time 0 (kg).
    pub fn initial_mass(&self) -> f64 {
        self.mass0
    }

    /// Local time at which the segment ends if nothing happens first.
    pub fn horizon(&self) -> f64 {
        self.horizon
    }

    /// Local time of the last computed sample.
    pub fn computed_until(&self) -> f64 {
        self.samples.last().map_or(0.0, |s| s.s.t)
    }

    /// The gravity sources simulated in full so far (only ever grows).
    pub fn active_sources(&self) -> &ActiveSources {
        &self.active
    }

    pub fn finished(&self) -> bool {
        self.end.is_some()
    }

    /// Integrates up to `max_steps` more accepted steps (stops at the end).
    pub fn extend(&mut self, world: &World, max_steps: usize) {
        self.extend_until(world, f64::INFINITY, max_steps);
    }

    /// Integrates until local time `until` is computed (the first step at or
    /// past it), the segment ends, or `max_steps` steps were taken. Returns
    /// the number of steps taken. Where it stops never changes the steps.
    pub fn extend_until(&mut self, world: &World, until: f64, max_steps: usize) -> usize {
        let integ = Dopri5::new(coast_tolerance());
        let snaps = SnapshotCache::new(&world.eph);
        for taken in 0..max_steps {
            if self.end.is_some() || self.computed_until() >= until {
                return taken;
            }
            if self.state.t >= self.horizon {
                // Only when started at the limit (otherwise the end is set
                // after the step that reaches it).
                self.end = Some(self.limit_end(world));
                return taken;
            }
            let prev = *self.samples.last().expect("segment has a first sample");
            let step = {
                // Field-level borrows: the forces read `active`/`drag` while
                // the integrator mutates `state`.
                let f = Forces {
                    world,
                    snaps: &snaps,
                    t0: self.t0,
                    anchor: self.anchor,
                    active: &self.active,
                    drag: self.drag,
                    mass0: self.mass0,
                    kind: &self.kind,
                };
                integ.step(&f, &mut self.state, self.horizon)
            };
            let Ok(sample) = step else {
                self.end = Some(SegmentEnd { t: prev.s.t, kind: EndKind::Failed });
                return taken + 1;
            };
            let new =
                Sample { anchor: self.anchor, s: sample, delta: self.state.x.value(), delta_rate: self.state.x_rate };
            if self.entry.is_none() {
                self.entry = self.find_entry(world, &snaps, &prev, &new);
            }
            if let Some((t_hit, body)) = self.find_contact(world, &snaps, &prev, &new) {
                let (r, v) = hermite5(&prev.s, &new.s, t_hit);
                let (a, delta_rate) = self.forces(world, &snaps).accel_rate(t_hit, r, v);
                let delta = hermite3(&prev, &new, t_hit);
                let s = StepSample { t: t_hit, r, v, a };
                self.samples.push(Sample { anchor: self.anchor, s, delta, delta_rate });
                self.end = Some(SegmentEnd { t: t_hit, kind: EndKind::Surface { body } });
                return taken + 1;
            }
            self.samples.push(new);
            if self.state.t >= self.horizon {
                self.end = Some(self.limit_end(world));
                return taken + 1;
            }
            self.after_step(world, &snaps);
        }
        max_steps
    }

    /// The end at `horizon`: the ephemeris end if that is what limited it.
    fn limit_end(&self, world: &World) -> SegmentEnd {
        let at_ephemeris_end = world.end().seconds_since(self.t0) <= self.horizon;
        SegmentEnd { t: self.horizon, kind: if at_ephemeris_end { EndKind::EphemerisEnd } else { self.horizon_end } }
    }

    /// At a step boundary: re-anchors if the precision policy prefers another
    /// anchor (an exact change of coordinates), and adds the gravity sources
    /// that now pass the cutoff (a flyby). Sources are never removed within
    /// a segment. Both are decided from the stored state only, so chunked
    /// integration still equals a single pass.
    fn after_step(&mut self, world: &World, snaps: &SnapshotCache) {
        let reanchored = snaps.with(self.t0.add_seconds(self.state.t), |snap| {
            let reanchored = self.maybe_reanchor(world, snap);
            let grew = self.active.grow(world, snap, self.anchor, self.state.r.value());
            if reanchored || grew {
                let (r, v) = (self.state.r.value(), self.state.v.value());
                let f = Forces {
                    world,
                    snaps,
                    t0: self.t0,
                    anchor: self.anchor,
                    active: &self.active,
                    drag: self.drag,
                    mass0: self.mass0,
                    kind: &self.kind,
                };
                (self.state.a, self.state.x_rate) = f.accel_rate_with(snap, self.state.t, r, v);
            }
            reanchored
        });
        if reanchored {
            let s = StepSample { t: self.state.t, r: self.state.r.value(), v: self.state.v.value(), a: self.state.a };
            let (delta, delta_rate) = (self.state.x.value(), self.state.x_rate);
            self.samples.push(Sample { anchor: self.anchor, s, delta, delta_rate });
        }
    }

    /// Switches anchor if the policy prefers another one (the caller
    /// recomputes the acceleration). Returns whether it switched.
    fn maybe_reanchor(&mut self, world: &World, snap: &Snapshot) -> bool {
        if self.fixed_anchor {
            return false;
        }
        let r = self.state.r.value();
        let next = preferred_anchor(world, snap, self.anchor, r);
        if next == self.anchor {
            return false;
        }
        let shift = snap.relative(self.anchor, next);
        let (r_new, v_new) = (r + shift.r, self.state.v.value() + shift.v);
        self.anchor = next;
        self.state.r = Compensated::new(r_new);
        self.state.v = Compensated::new(v_new);
        true
    }

    /// First contact with a surface between two samples, by bisection on the
    /// interpolated altitude.
    fn find_contact(&self, world: &World, snaps: &SnapshotCache, a: &Sample, b: &Sample) -> Option<(f64, NodeId)> {
        let near = |src: &crate::world::Source, reach: f64| {
            snaps.with(self.t0.add_seconds(b.s.t), |snap| (b.s.r - snap.relative_r(src.node, self.anchor)).length())
                <= reach
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
    fn find_entry(&self, world: &World, snaps: &SnapshotCache, a: &Sample, b: &Sample) -> Option<(f64, NodeId)> {
        for src in &world.sources {
            let Some(p) = src.physical.as_ref() else { continue };
            let Some(atm) = &p.atmosphere else { continue };
            let d = snaps
                .with(self.t0.add_seconds(b.s.t), |snap| (b.s.r - snap.relative_r(src.node, self.anchor)).length());
            if d > p.radius_eq + atm.top + 200_000.0 {
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

    /// Proper time offset δ = τ − t (s) at local time `t`. `None` if not
    /// computed yet.
    pub fn proper_time_at(&self, t: f64) -> Option<f64> {
        let last = self.samples.last()?;
        if t > last.s.t {
            return None;
        }
        let i = self.samples.partition_point(|s| s.s.t < t);
        if i == 0 {
            return Some(self.samples[0].delta);
        }
        let (a, b) = (self.samples[i - 1], self.samples[i]);
        if a.s.t == b.s.t {
            return Some(b.delta);
        }
        Some(hermite3(&a, &b, t))
    }
}

/// Cubic Hermite interpolation of δ between two samples (values and rates).
fn hermite3(a: &Sample, b: &Sample, t: f64) -> f64 {
    let h = b.s.t - a.s.t;
    let s = (t - a.s.t) / h;
    let (s2, s3) = (s * s, s * s * s);
    let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
    let h10 = s3 - 2.0 * s2 + s;
    let h01 = -2.0 * s3 + 3.0 * s2;
    let h11 = s3 - s2;
    h00 * a.delta + h10 * h * a.delta_rate + h01 * b.delta + h11 * h * b.delta_rate
}

/// Mass at local time `t` of a segment of `kind` starting with `mass0`.
fn mass_at(kind: &SegmentKind, mass0: f64, t: f64) -> f64 {
    match kind {
        SegmentKind::Coast => mass0,
        SegmentKind::Burn(law) => mass0 - law.mass_flow * t,
    }
}

/// The dynamics of a segment: gravity, drag and (in a burn) thrust.
struct Forces<'a, 'w> {
    world: &'a World,
    /// The snapshots of the ephemeris, reused across evaluations.
    snaps: &'a SnapshotCache<'w>,
    t0: Epoch,
    anchor: NodeId,
    active: &'a ActiveSources,
    drag: Option<DragModel>,
    mass0: f64,
    kind: &'a SegmentKind,
}

impl Dynamics for Forces<'_, '_> {
    /// Acceleration at local time `t`.
    fn accel(&self, t: f64, r: DVec3, v: DVec3) -> DVec3 {
        self.snaps.with(self.t0.add_seconds(t), |snap| self.context(snap, t, r, v).accel_with(snap, r, v))
    }

    /// Acceleration and proper-time rate at local time `t`.
    fn accel_rate(&self, t: f64, r: DVec3, v: DVec3) -> (DVec3, f64) {
        self.snaps.with(self.t0.add_seconds(t), |snap| self.accel_rate_with(snap, t, r, v))
    }
}

impl Forces<'_, '_> {
    fn accel_rate_with(&self, snap: &Snapshot, t: f64, r: DVec3, v: DVec3) -> (DVec3, f64) {
        self.context(snap, t, r, v).accel_rate(snap, r, v)
    }

    /// The forces at local time `t` (thrust and drag mass of a burn).
    fn context(&self, snap: &Snapshot, t: f64, r: DVec3, v: DVec3) -> ForceContext<'_> {
        let (drag, thrust) = match self.kind {
            SegmentKind::Coast => (self.drag, DVec3::ZERO),
            SegmentKind::Burn(law) => {
                let m = mass_at(self.kind, self.mass0, t);
                let dir = law.direction.direction(snap, self.anchor, r, v);
                (self.drag.map(|d| DragModel { mass: m, ..d }), dir * (law.thrust / m))
            }
        };
        ForceContext { world: self.world, anchor: self.anchor, active: self.active, drag, thrust }
    }
}
