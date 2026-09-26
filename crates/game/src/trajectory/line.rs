//! Orbit-line length (D056): where an object's drawn line ends. Pure rules;
//! the Bevy systems in [`super`] and the look-ahead in `state` wire them up.
//!
//! A line runs until it has swept a number of revolutions (usually one)
//! about its dominant body ([`crate::relations::Dominance`]) with that body
//! dominant the whole way; a change of dominant body restarts the count.
//! Hard caps: a time cap, escape (leaving the region of interest) and, by
//! the caller, the computed part of the segment. Display only (rule 1).

use crate::relations::Dominance;
use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::time::Epoch;
use sim::vessel::Segment;
use std::f64::consts::TAU;

/// One point of an object's future path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPoint {
    /// Local time (s).
    pub t: f64,
    pub dominant: NodeId,
    /// Position and velocity relative to `dominant`.
    pub r: DVec3,
    pub v: DVec3,
}

/// What ends a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The requested revolutions about one dominant body are complete.
    Revolutions,
    /// The time cap was reached.
    TimeCap,
    /// The object left the region of interest.
    Escape,
}

/// Where a line ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineEnd {
    /// Local time (s).
    pub t: f64,
    pub reason: EndReason,
}

/// The limits a line is drawn to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Revolutions about one dominant body; `None`: no revolution limit.
    pub revolutions: Option<f64>,
    /// Local time at which the line ends regardless (s).
    pub t_cap: f64,
    /// Distance from the root body beyond which the object has escaped (m).
    pub escape_radius: f64,
}

/// Feeds path points in time order and reports where the line ends. The
/// state is kept so a path can be scanned as it is computed.
#[derive(Clone, Debug)]
pub struct LineRule {
    limits: Limits,
    root: NodeId,
    prev: Option<PathPoint>,
    /// Plane normal of the current stretch (unit), from r × v at its start.
    normal: DVec3,
    /// Angle swept about the current dominant body (rad).
    swept: f64,
    end: Option<LineEnd>,
}

impl LineRule {
    /// `root` is the body with no primary (escape is measured from it).
    pub fn new(limits: Limits, root: NodeId) -> Self {
        LineRule { limits, root, prev: None, normal: DVec3::ZERO, swept: 0.0, end: None }
    }

    /// Adds the next point; returns the end once it is reached (then stays).
    pub fn push(&mut self, p: PathPoint) -> Option<LineEnd> {
        if self.end.is_some() {
            return self.end;
        }
        let prev = match self.prev {
            Some(prev) if prev.dominant == p.dominant => prev,
            _ => {
                // First point, or a new dominant body: restart the count.
                self.restart(p);
                return self.check_caps(p, None);
            }
        };
        if p.t <= prev.t {
            return None;
        }
        if self.normal == DVec3::ZERO {
            self.normal = plane_normal(prev.r, prev.v, p.r);
        }
        let step = self.normal.dot(prev.r.cross(p.r)).atan2(prev.r.dot(p.r));
        self.prev = Some(p);
        if let Some(revs) = self.limits.revolutions {
            let target = revs * TAU;
            let before = self.swept;
            self.swept += step;
            if self.swept >= target && step > 0.0 {
                let f = ((target - before) / step).clamp(0.0, 1.0);
                let t = prev.t + f * (p.t - prev.t);
                if t <= self.limits.t_cap {
                    self.end = Some(LineEnd { t, reason: EndReason::Revolutions });
                    return self.end;
                }
            }
        }
        self.check_caps(p, Some(prev))
    }

    fn restart(&mut self, p: PathPoint) {
        self.prev = Some(p);
        self.swept = 0.0;
        let h = p.r.cross(p.v);
        self.normal = if h.length_squared() > 0.0 { h.normalize() } else { DVec3::ZERO };
    }

    fn check_caps(&mut self, p: PathPoint, prev: Option<PathPoint>) -> Option<LineEnd> {
        if p.t >= self.limits.t_cap {
            let t = prev.map_or(p.t, |q| self.limits.t_cap.max(q.t));
            self.end = Some(LineEnd { t, reason: EndReason::TimeCap });
        } else if p.dominant == self.root && p.r.length() > self.limits.escape_radius {
            self.end = Some(LineEnd { t: p.t, reason: EndReason::Escape });
        }
        self.end
    }
}

/// Unit normal of the plane of motion: r × v, or r × r_next for a radial path.
fn plane_normal(r: DVec3, v: DVec3, r_next: DVec3) -> DVec3 {
    let h = r.cross(v);
    let h = if h.length_squared() > 0.0 { h } else { r.cross(r_next) };
    if h.length_squared() > 0.0 {
        h.normalize()
    } else {
        DVec3::Z
    }
}

/// The points of `seg` from local time `t_start` (evaluated there) on
/// through its stored samples, at most `max` of them, each with its dominant
/// body. Stored samples are the integrator's steps, so each spans a small
/// angle and the swept angle is exact to interpolation.
pub fn segment_points<'a>(
    eph: &'a Ephemeris,
    dom: &'a Dominance,
    seg: &'a Segment,
    t_start: f64,
    max: usize,
) -> impl Iterator<Item = PathPoint> + 'a {
    let first = seg.samples.partition_point(|s| s.s.t <= t_start);
    let states = seg
        .eval(t_start)
        .map(|(a, r, v)| (t_start, a, r, v))
        .into_iter()
        .chain(seg.samples[first..].iter().map(|s| (s.s.t, s.anchor, s.s.r, s.s.v)));
    let mut hint = None;
    states.take(max).map(move |(t, anchor, r, v)| {
        let epoch = seg.t0.add_seconds(t);
        let dominant = dom.of(eph, epoch, anchor, r, None, hint);
        hint = Some(dominant);
        let k = eph.relative(dominant, anchor, epoch);
        PathPoint { t, dominant, r: r - k.r, v: v - k.v }
    })
}

/// Points of a body's path about a fixed `about` body from the ephemeris,
/// every `dt` seconds from `t0` for `n` steps (stops at the ephemeris end).
pub fn body_points(
    eph: &Ephemeris,
    node: NodeId,
    about: NodeId,
    t0: Epoch,
    dt: f64,
    n: usize,
) -> impl Iterator<Item = PathPoint> + '_ {
    (0..=n).map_while(move |k| {
        let t = k as f64 * dt;
        let epoch = t0.add_seconds(t);
        (epoch <= eph.end).then(|| {
            let s = eph.relative(node, about, epoch);
            PathPoint { t, dominant: about, r: s.r, v: s.v }
        })
    })
}

#[cfg(test)]
#[path = "line_tests.rs"]
mod tests;
