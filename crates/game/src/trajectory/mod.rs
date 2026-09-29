//! Trajectory display: stored segments sampled into the plotting frame
//! ([`lines`]), apsides found along them ([`apsides`]: the extrema of
//! distance on the N-body trajectory, D076), and how long vessel and body
//! lines are ([`line`], [`settings`]; D056).

pub mod apsides;
pub mod clip;
pub mod line;
pub mod lines;
pub mod settings;

pub use lines::draw;

use crate::camera::CameraRig;
use crate::hud::PlotFrame;
use crate::relations::{Dominance, Orbit};
use crate::state::SimState;
use glam::{DMat3, DVec3};
use line::{Limits, LineRule};
use settings::OrbitSettings;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::time::Epoch;
use sim::vessel::{Segment, Vessel};
use sim::world::World;

/// Earth–Moon rotating basis at `t` (x → Moon, z → orbit normal).
fn rotating_basis(world: &World, t: Epoch) -> DMat3 {
    let (earth, moon) = (world.find("Earth").map(|s| s.node), world.find("Moon").map(|s| s.node));
    let (Some(e), Some(m)) = (earth, moon) else { return DMat3::IDENTITY };
    let k = world.eph.relative(m, e, t);
    let x = k.r.normalize();
    let z = k.r.cross(k.v).normalize();
    DMat3::from_cols(x, z.cross(x), z)
}

/// Maps a trajectory point at `epoch` to camera-relative render space in the
/// selected plotting frame (centred on Earth's current position).
pub struct Plotter<'a> {
    world: &'a World,
    frame: PlotFrame,
    earth: NodeId,
    basis_now: DMat3,
    /// Earth's current position, camera-relative.
    origin: DVec3,
}

impl<'a> Plotter<'a> {
    pub fn new(sim: &'a SimState, rig: &CameraRig, frame: PlotFrame) -> Option<Self> {
        let earth = sim.world.find("Earth")?.node;
        let basis_now = match frame {
            PlotFrame::EarthInertial => DMat3::IDENTITY,
            PlotFrame::EarthMoonRotating => rotating_basis(&sim.world, sim.clock),
        };
        let origin = sim.world.snapshot(sim.clock).relative_r(earth, rig.anchor) - rig.cam_pos;
        Some(Plotter { world: &sim.world, frame, earth, basis_now, origin })
    }

    /// Position `r` relative to `anchor` at `epoch`, plotted.
    pub fn plot(&self, anchor: NodeId, r: DVec3, epoch: Epoch) -> DVec3 {
        let rel_earth = r + self.world.eph.relative(anchor, self.earth, epoch).r;
        let plotted = match self.frame {
            PlotFrame::EarthInertial => rel_earth,
            PlotFrame::EarthMoonRotating => {
                self.basis_now * (rotating_basis(self.world, epoch).transpose() * rel_earth)
            }
        };
        plotted + self.origin
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
        // Small chunks: an orbit can take under 100 steps, and rescanning is cheap.
        let chunk = budget.min(32);
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
