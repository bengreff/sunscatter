//! Apsis markers (D076): the one owner of apoapsides and periapsides. An
//! apsis is a local extremum of distance to the dominant body along the
//! predicted trajectory (every future segment, through planned burns), not
//! an osculating element: an escape has a periapsis and no apoapsis, and a
//! trajectory ending on a surface shows its impact instead of a periapsis
//! below the ground.
//!
//! Extrema are sign changes of r·v on the stored samples (relative to each
//! point's dominant body), refined by a root search on the segment's own
//! interpolant. A change of dominant body starts a new run. An extremum is
//! kept only when it differs from its neighbour by more than
//! [`SIGNIFICANCE`] of the altitude (J2 and lunar wobbles of a near-circular
//! orbit are not apsides). Results are cached per vessel and extended as
//! the stored trajectory grows; the navball, flight panel, tracking station,
//! MCP tools, planner and map markers all read them here.

use crate::state::{Prediction, SimState};
use bevy::prelude::*;
use glam::DVec3;
use sim::frame::NodeId;
use sim::time::Epoch;
use sim::vessel::{EndKind, Segment, SegmentEnd, VesselId};
use std::collections::HashMap;

/// An extremum is kept when it differs from its neighbouring extremum by
/// more than this fraction of the altitude above the body's surface.
///
/// Why 0.5 %: a real low Earth orbit started with osculating e = 0.001 is
/// nearly frozen under J2: its distance swings only ≈3 km (0.8 % of the
/// altitude) once per revolution, and those are its true Ap and Pe. J2's
/// short-period term (twice per revolution, ¼·J2·R²/a·sin²i ≤ 1.6 km) and
/// the Moon's pull on high orbits add extrema pairs that differ by less
/// than that at moderate inclinations; 0.5 % (2 km at 400 km) drops those
/// and keeps the once-per-revolution apsides. Only a very nearly circular
/// polar orbit may still show extra wobble apsides.
pub const SIGNIFICANCE: f64 = 0.005;

/// One apsis on the predicted trajectory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Apsis {
    pub is_apo: bool,
    pub t: Epoch,
    /// The dominant body there.
    pub body: NodeId,
    /// Distance from the body's centre (m).
    pub distance: f64,
    /// Above the body's equatorial radius (m).
    pub altitude: f64,
}

/// Where the trajectory reaches a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub t: Epoch,
    pub body: NodeId,
}

/// A vessel's apsides in time order, and its impact if it has one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VesselApsides {
    pub list: Vec<Apsis>,
    pub impact: Option<Impact>,
}

impl VesselApsides {
    /// The next apoapsis and periapsis strictly after `t`.
    pub fn next_after(&self, t: Epoch) -> (Option<&Apsis>, Option<&Apsis>) {
        let ahead = || self.list.iter().filter(move |a| a.t.seconds_since(t) > 0.0);
        (ahead().find(|a| a.is_apo), ahead().find(|a| !a.is_apo))
    }

    /// Seconds from `now` to the next apoapsis and periapsis.
    pub fn times_to_next(&self, now: Epoch) -> (Option<f64>, Option<f64>) {
        let (ap, pe) = self.next_after(now);
        (ap.map(|a| a.t.seconds_since(now)), pe.map(|a| a.t.seconds_since(now)))
    }
}

/// A raw (unfiltered) extremum.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Raw {
    t: Epoch,
    distance: f64,
    is_apo: bool,
}

/// Points about one dominant body.
#[derive(Clone, Debug, PartialEq)]
struct Run {
    body: NodeId,
    /// Distances at the run's first and last point.
    start: f64,
    end: f64,
    extrema: Vec<Raw>,
}

/// The previous point fed to a [`Scanner`].
#[derive(Clone, Copy, Debug, PartialEq)]
struct Prev {
    t: Epoch,
    body: NodeId,
    rv: f64,
}

/// Finds the raw extrema of distance along points fed in time order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scanner {
    runs: Vec<Run>,
    prev: Option<Prev>,
}

impl Scanner {
    /// The dominant body of the last point (a hint for the next one).
    pub fn last_body(&self) -> Option<NodeId> {
        self.prev.map(|p| p.body)
    }

    /// Feeds the next point: position and velocity relative to its dominant
    /// `body`. On a sign change of r·v since the previous point,
    /// `refine(t_prev, t)` gives the extremum's time and distance (`None`:
    /// the midpoint and the nearer sample are used).
    pub fn push(
        &mut self,
        t: Epoch,
        body: NodeId,
        r: DVec3,
        v: DVec3,
        refine: impl FnOnce(Epoch, Epoch) -> Option<(Epoch, f64)>,
    ) {
        let (rv, d) = (r.dot(v), r.length());
        let prev = match self.prev {
            Some(p) if p.body == body => p,
            _ => {
                self.runs.push(Run { body, start: d, end: d, extrema: Vec::new() });
                self.prev = Some(Prev { t, body, rv });
                return;
            }
        };
        if t.seconds_since(prev.t) <= 0.0 {
            return;
        }
        let run = self.runs.last_mut().expect("a run is open");
        let is_apo = prev.rv > 0.0 && rv <= 0.0;
        let is_peri = prev.rv < 0.0 && rv >= 0.0;
        if is_apo || is_peri {
            let (te, de) = refine(prev.t, t).unwrap_or_else(|| {
                let pick = if is_apo { run.end.max(d) } else { run.end.min(d) };
                (prev.t.add_seconds(0.5 * t.seconds_since(prev.t)), pick)
            });
            run.extrema.push(Raw { t: te, distance: de, is_apo });
        }
        run.end = d;
        self.prev = Some(Prev { t, body, rv });
    }

    /// The significant extrema of every run, in time order; `radius` gives
    /// a body's equatorial radius.
    pub fn apsides(&self, radius: impl Fn(NodeId) -> f64) -> Vec<Apsis> {
        let mut out = Vec::new();
        for run in &self.runs {
            let r = radius(run.body);
            for e in significant(run.start, run.end, &run.extrema, r, SIGNIFICANCE) {
                out.push(Apsis {
                    is_apo: e.is_apo,
                    t: e.t,
                    body: run.body,
                    distance: e.distance,
                    altitude: e.distance - r,
                });
            }
        }
        out
    }
}

/// The extrema of a run that are significant: repeatedly drops the adjacent
/// pair (a maximum and a minimum) that differ least, while they differ by
/// less than `fraction` of the lower one's altitude above `radius`. A single
/// remaining extremum is dropped when the run's ends are as close to it.
fn significant(start: f64, end: f64, extrema: &[Raw], radius: f64, fraction: f64) -> Vec<Raw> {
    let threshold = |a: f64, b: f64| fraction * (a.min(b) - radius).max(0.0);
    let mut e = extrema.to_vec();
    loop {
        let weakest = (0..e.len().saturating_sub(1))
            .map(|i| (i, (e[i].distance - e[i + 1].distance).abs()))
            .filter(|&(i, diff)| diff < threshold(e[i].distance, e[i + 1].distance))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match weakest {
            Some((i, _)) => {
                e.drain(i..i + 2);
            }
            None => break,
        }
    }
    if let [x] = e[..] {
        let near = |d: f64| (x.distance - d).abs() < threshold(x.distance, d);
        if near(start) && near(end) {
            e.clear();
        }
    }
    e
}

/// A root of `g` in `[a, b]` where `g(a)` and `g(b)` (given) differ in
/// sign: the Illinois variant of regula falsi, to `tol` seconds.
pub fn refine_root(g: impl Fn(f64) -> Option<f64>, mut a: f64, mut b: f64, mut ga: f64, mut gb: f64, tol: f64) -> f64 {
    let mut side = 0;
    for _ in 0..60 {
        if (b - a).abs() <= tol || ga == gb {
            break;
        }
        let c = (a * gb - b * ga) / (gb - ga);
        let Some(gc) = g(c) else { break };
        if gc == 0.0 {
            return c;
        }
        if (gc > 0.0) == (gb > 0.0) {
            (b, gb) = (c, gc);
            if side == 1 {
                ga *= 0.5;
            }
            side = 1;
        } else {
            (a, ga) = (c, gc);
            if side == -1 {
                gb *= 0.5;
            }
            side = -1;
        }
    }
    if ga.abs() < gb.abs() {
        a
    } else {
        b
    }
}

/// Position and velocity of `seg` at local time `t` relative to `body`.
fn relative_to(sim: &SimState, seg: &Segment, t: f64, body: NodeId) -> Option<(DVec3, DVec3)> {
    let (anchor, r, v) = seg.eval(t)?;
    let k = sim.world.eph.relative(body, anchor, seg.t0.add_seconds(t));
    Some((r - k.r, v - k.v))
}

/// The segments whose apsides and lines are shown for vessel `i`: its
/// stored trajectory, or (the active vessel flying live) the background
/// prediction.
pub fn vessel_segments<'a>(sim: &'a SimState, pred: &'a Prediction, i: usize) -> Vec<&'a Segment> {
    match sim.fleet[i].trajectory() {
        Some(tr) => tr.segments.iter().collect(),
        None if i == sim.active => pred.segment.iter().collect(),
        None => Vec::new(),
    }
}

/// What identifies a segment's stored content (samples only grow at the end).
#[derive(Clone, Copy, Debug, PartialEq)]
struct SegKey {
    t0: Epoch,
    end: Option<SegmentEnd>,
    /// The last sample scanned: local time and position.
    last: Option<(f64, DVec3)>,
}

fn key(seg: &Segment) -> SegKey {
    SegKey { t0: seg.t0, end: seg.end, last: seg.samples.last().map(|s| (s.s.t, s.s.r)) }
}

/// One vessel's cached scan.
#[derive(Clone, Debug, Default)]
struct Entry {
    keys: Vec<SegKey>,
    scanner: Scanner,
    result: VesselApsides,
}

/// Where a scan resumes: the segment and the first sample not yet fed.
fn resume_point(keys: &[SegKey], segs: &[&Segment]) -> Option<(usize, usize)> {
    let (last, before) = keys.split_last()?;
    if segs.len() < keys.len() || before.iter().zip(segs).any(|(k, s)| *k != key(s)) {
        return None;
    }
    let k = keys.len() - 1;
    let seg = segs[k];
    if seg.t0 != last.t0 {
        return None;
    }
    let Some((t, r)) = last.last else { return Some((k, 0)) };
    let i = seg.samples.partition_point(|s| s.s.t < t);
    // The same sample (the same segment, grown) must still be there.
    let same = seg.samples[i..].iter().take_while(|s| s.s.t == t).position(|s| s.s.r == r)?;
    Some((k, i + same + 1))
}

impl Entry {
    /// Scans what is new in `segs`; returns whether anything changed.
    fn update(&mut self, sim: &SimState, segs: &[&Segment]) -> bool {
        let (from_seg, from_sample) = match resume_point(&self.keys, segs) {
            Some(p) => p,
            None => {
                self.scanner = Scanner::default();
                (0, 0)
            }
        };
        let keys: Vec<SegKey> = segs.iter().map(|s| key(s)).collect();
        if keys == self.keys {
            return false;
        }
        let eph = &sim.world.eph;
        for (k, seg) in segs.iter().enumerate().skip(from_seg) {
            let start = if k == from_seg { from_sample } else { 0 };
            for s in &seg.samples[start.min(seg.samples.len())..] {
                let epoch = seg.t0.add_seconds(s.s.t);
                let body = sim.dominance.of(eph, epoch, s.anchor, s.s.r, None, self.scanner.last_body());
                let rel = eph.relative(body, s.anchor, epoch);
                let refine = |ta: Epoch, tb: Epoch| {
                    let (a, b) = (ta.seconds_since(seg.t0), tb.seconds_since(seg.t0));
                    let g = |t: f64| relative_to(sim, seg, t, body).map(|(r, v)| r.dot(v));
                    let t = refine_root(g, a, b, g(a)?, g(b)?, 1e-3);
                    let (r, _) = relative_to(sim, seg, t, body)?;
                    Some((seg.t0.add_seconds(t), r.length()))
                };
                self.scanner.push(epoch, body, s.s.r - rel.r, s.s.v - rel.v, refine);
            }
        }
        self.keys = keys;
        let radius = |b: NodeId| sim.world.source(b).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
        let impact = segs.last().and_then(|s| match s.end {
            Some(SegmentEnd { t, kind: EndKind::Surface { body } }) => Some(Impact { t: s.t0.add_seconds(t), body }),
            _ => None,
        });
        self.result = VesselApsides { list: self.scanner.apsides(radius), impact };
        true
    }
}

/// The apsides along `segs` (in time order), computed now, uncached.
pub fn of_segments(sim: &SimState, segs: &[&Segment]) -> VesselApsides {
    let mut e = Entry::default();
    e.update(sim, segs);
    e.result
}

/// Every vessel's apsides, kept current by [`update`].
#[derive(Resource, Default)]
pub struct Apsides {
    entries: HashMap<VesselId, Entry>,
}

impl Apsides {
    pub fn get(&self, id: VesselId) -> Option<&VesselApsides> {
        self.entries.get(&id).map(|e| &e.result)
    }

    /// Brings every vessel's list up to date with its stored trajectory.
    pub fn refresh(&mut self, sim: &SimState, pred: &Prediction) {
        self.entries.retain(|id, _| sim.index_of(*id).is_some());
        for i in 0..sim.fleet.len() {
            let segs = vessel_segments(sim, pred, i);
            self.entries.entry(sim.fleet[i].id()).or_default().update(sim, &segs);
        }
    }

    /// A list computed now, uncached (tests).
    #[cfg(test)]
    pub fn compute(sim: &SimState, pred: &Prediction, i: usize) -> VesselApsides {
        of_segments(sim, &vessel_segments(sim, pred, i))
    }
}

/// Keeps [`Apsides`] current (after the simulation step).
pub fn update(sim: Res<SimState>, pred: Res<Prediction>, mut aps: ResMut<Apsides>) {
    aps.refresh(&sim, &pred);
}

#[cfg(test)]
#[path = "apsides_tests.rs"]
mod tests;
