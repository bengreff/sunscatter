//! Trajectory display: stored segments sampled into the line's frame
//! ([`LineFrame`], [`lines`]), apsides found along them ([`apsides`]: the extrema of
//! distance on the N-body trajectory, D076), and how long vessel and body
//! lines are ([`line`], [`settings`]; D056).

pub mod apsides;
pub mod clip;
pub mod line;
pub mod lines;
pub mod settings;

pub use lines::draw;

use crate::camera::CameraRig;
use crate::relations::{Dominance, Orbit};
use crate::state::SimState;
use glam::DVec3;
use line::{Limits, LineRule};
use settings::OrbitSettings;
use sim::ephem::Ephemeris;
use sim::frame::{NodeId, Vec3};
use sim::kepler::Elements;
use sim::time::Epoch;
use sim::vessel::{Segment, Vessel};
use sim::world::World;

/// The frame a vessel's line is drawn in (D089): the navball's switch
/// for the active vessel, the dominant body's non-rotating frame for the
/// others. The path is one physical path; the frame only changes how it is
/// drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineFrame {
    /// About a body, not turning: its position at each point's time is
    /// taken out and the path drawn around where it is now (ORBIT).
    About(NodeId),
    /// Over a body's ground: each point in the body-fixed frame at its own
    /// time, drawn where that ground is now (SURFACE: the path over the
    /// ground, the body's rotation and the air's with it).
    Ground(NodeId),
}

impl LineFrame {
    /// The body the frame is centred on.
    pub fn body(self) -> NodeId {
        match self {
            LineFrame::About(b) | LineFrame::Ground(b) => b,
        }
    }
}

/// The frame of the line of fleet vessel `i`: the navball's frame for the
/// active one (`active`), else about its dominant body.
pub fn frame_of(sim: &SimState, i: usize, active: Option<LineFrame>) -> LineFrame {
    match active.filter(|_| i == sim.active) {
        Some(f) => f,
        None => LineFrame::About(vessel_dominant(&sim.world, &sim.dominance, sim.clock, &sim.fleet[i])),
    }
}

/// Maps a trajectory point at `epoch` to camera-relative render space in a
/// [`LineFrame`], centred on its body's current position.
pub struct Plotter<'a> {
    world: &'a World,
    frame: LineFrame,
    now: Epoch,
    /// The frame's body, now, camera-relative.
    origin: DVec3,
}

impl<'a> Plotter<'a> {
    pub fn new(sim: &'a SimState, rig: &CameraRig, frame: LineFrame) -> Self {
        Self::at(&sim.world, sim.clock, rig.anchor, rig.cam_pos, frame)
    }

    /// A plotter at clock `now` for a camera at `cam_pos` relative to
    /// `anchor`.
    pub fn at(world: &'a World, now: Epoch, anchor: NodeId, cam_pos: DVec3, frame: LineFrame) -> Self {
        let origin = world.snapshot(now).relative_r(frame.body(), anchor) - cam_pos;
        // A body without a shape has no ground to draw over.
        let frame = match frame {
            LineFrame::Ground(b) if Self::spin(world, b).is_none() => LineFrame::About(b),
            f => f,
        };
        Plotter { world, frame, now, origin }
    }

    fn spin(world: &World, body: NodeId) -> Option<&sim::body::BodyPhysical> {
        world.source(body).and_then(|s| s.physical.as_ref())
    }

    /// Position `r` relative to `anchor` at `epoch`, plotted.
    pub fn plot(&self, anchor: NodeId, r: DVec3, epoch: Epoch) -> DVec3 {
        let body = self.frame.body();
        let rel = r + self.world.eph.relative(anchor, body, epoch).r;
        let placed = match self.frame {
            LineFrame::About(_) => rel,
            LineFrame::Ground(b) => {
                let rot = Self::spin(self.world, b).expect("checked in `at`").rotation;
                rot.to_inertial(rot.to_fixed(Vec3::from_raw(rel), epoch), self.now).raw()
            }
        };
        placed + self.origin
    }

    /// How fast the frame turns (rad/s): a straight leg drawn in it bends by
    /// this much more, which the line's sampling follows.
    pub fn turn_rate(&self) -> f64 {
        match self.frame {
            LineFrame::About(_) => 0.0,
            LineFrame::Ground(b) => Self::spin(self.world, b).map_or(0.0, |p| p.rotation.w_rate.abs()),
        }
    }
}

/// The future time span of a segment that can be drawn, in local seconds.
pub fn future_span(seg: &Segment, now: Epoch) -> Option<(f64, f64)> {
    let t_start = now.seconds_since(seg.t0).max(seg.samples.first().map_or(0.0, |s| s.s.t));
    let t_end = seg.computed_until();
    (t_end > t_start).then_some((t_start, t_end))
}

/// Most stored samples scanned for one vessel line (bounds per-frame work
/// and, through the look-ahead, how far a coast is computed).
pub const MAX_LINE_SAMPLES: usize = 20_000;

/// Where a vessel's line along `seg` from local time `t_start` ends (local
/// time), and whether that is final: the rule is met, a cap or the sample
/// budget is reached, or the segment ended (impact). If not final, the
/// computed part ran out first and the look-ahead should extend it.
pub fn vessel_line_end(world: &World, seg: &Segment, t_start: f64, settings: &OrbitSettings) -> (f64, bool) {
    let dom = Dominance::new(&world.eph, seg.t0.add_seconds(t_start));
    let limits = settings.vessel_limits(t_start, dom.region());
    let mut rule = LineRule::new(limits, dom.root());
    let mut last = t_start;
    let mut n = 0;
    for p in line::segment_points(&world.eph, &dom, seg, t_start, MAX_LINE_SAMPLES) {
        if let Some(end) = rule.push(p) {
            return (end.t, true);
        }
        last = p.t;
        n += 1;
    }
    (last, seg.finished() || n >= MAX_LINE_SAMPLES)
}

/// Where the look-ahead should compute a coast to: the line's end once it is
/// known, else as far as the time cap allows (the per-frame step budget
/// bounds each extension).
pub fn lookahead_until(world: &World, seg: &Segment, now: Epoch, settings: &OrbitSettings) -> Epoch {
    let t_start = now.seconds_since(seg.t0).max(0.0);
    match vessel_line_end(world, seg, t_start, settings) {
        (t, true) => seg.t0.add_seconds(t),
        (_, false) => seg.t0.add_seconds(settings.vessel_limits(t_start, f64::INFINITY).t_cap),
    }
}

/// Integrates `seg` (owned, e.g. a background prediction) until its line
/// from local time 0 ends, or `max_steps` steps.
pub fn extend_to_line_end(world: &World, seg: &mut Segment, settings: &OrbitSettings, max_steps: usize) {
    let dom = Dominance::new(&world.eph, seg.t0);
    let mut rule = LineRule::new(settings.vessel_limits(0.0, dom.region()), dom.root());
    let (mut fed, mut budget) = (0.0, max_steps);
    loop {
        // Points from the last one fed (repeated at the same time: ignored).
        for p in line::segment_points(&world.eph, &dom, seg, fed, MAX_LINE_SAMPLES) {
            if rule.push(p).is_some() {
                return;
            }
            fed = p.t;
        }
        if seg.finished() || budget == 0 || seg.samples.len() >= MAX_LINE_SAMPLES {
            return;
        }
        // Small chunks: with DOP853 a low orbit takes ~16 steps, and the
        // rule only rescans the new points.
        let chunk = budget.min(4);
        seg.extend(world, chunk);
        budget -= chunk;
    }
}

/// The dominant body of a vessel now (its "current system").
pub fn vessel_dominant(world: &World, dom: &Dominance, t: Epoch, vessel: &Vessel) -> NodeId {
    let (anchor, r, _) = vessel.state_at(world, t);
    dom.of(&world.eph, t, anchor, r, None, None)
}

/// Body `node`'s line about `about` from `t`: its osculating orbit (for
/// sizes) and points relative to `about`, the ephemeris sampled `per_rev`
/// times per osculating period until the line rule ends it. Past the
/// ephemeris span: one period of the osculating conic. `None` if unbound.
pub fn body_line(
    eph: &Ephemeris,
    dom: &Dominance,
    node: NodeId,
    about: NodeId,
    t: Epoch,
    limits: Limits,
    per_rev: usize,
) -> Option<(Orbit, Vec<DVec3>)> {
    let orbit = crate::relations::body_orbit_about(eph, t, node, about);
    let period = orbit.period()?;
    let dt = period / per_rev as f64;
    // Room for the true revolution to be longer than the osculating period.
    let n = per_rev + per_rev / 2;
    let mut out = Vec::with_capacity(n + 2);
    if t.add_seconds(n as f64 * dt) <= eph.end {
        let mut rule = LineRule::new(limits, dom.root());
        for p in line::body_points(eph, node, about, t, dt, n) {
            match rule.push(p) {
                None => out.push(p.r),
                Some(end) => {
                    out.push(eph.relative(node, about, t.add_seconds(end.t)).r);
                    break;
                }
            }
        }
    } else {
        let (el, mu) = (orbit.elements, orbit.mu);
        out.extend((0..=per_rev).map(|i| {
            let f = i as f64 / per_rev as f64;
            Elements { mean_anomaly: el.mean_anomaly + f * std::f64::consts::TAU, ..el }.to_state(mu).0
        }));
    }
    Some((orbit, out))
}

/// State at `t` along `segs` (in time order): the last segment starting at
/// or before `t`.
pub fn eval_at(segs: &[&Segment], t: Epoch) -> Option<(NodeId, DVec3, DVec3)> {
    let seg = segs.iter().rev().find(|s| t.seconds_since(s.t0) >= 0.0)?;
    seg.eval(t.seconds_since(seg.t0))
}
