//! Vessel lines, built once per frame: the future of the active vessel and
//! of tracked vessels in map view, sampled adaptively from the stored
//! trajectory (the renderer never integrates, rule 4) and cut to the view.
//!
//! Sampling follows the path, not the clock: the stored samples are the
//! integrator's own steps (dense where the path bends), thinned where the
//! velocity turns less than [`MAX_TURN`] and subdivided on the interpolant
//! where it turns more, then subdivided again where the chord would stray
//! more than a pixel from the curve on screen (near the camera). Pieces
//! wholly outside the view frustum are skipped before that refinement.
//! The drawn points (with their times) are kept in [`Lines`] for the
//! markers and for picking a point on the line.

use super::{clip, future_span, vessel_line_end, Plotter};
use crate::camera::{CameraRig, MainCamera};
use crate::hud::UiState;
use crate::map::MapView;
use crate::map_view::ObjectId;
use crate::state::{Prediction, SimState};
use crate::tracking::Tracked;
use crate::trajectory::apsides::vessel_segments;
use crate::trajectory::settings::OrbitSettings;
use bevy::prelude::*;
use glam::DVec3;
use sim::time::Epoch;
use sim::vessel::{Segment, SegmentKind, VesselId};

/// Largest turn of the velocity between two drawn points (rad).
pub const MAX_TURN: f64 = 0.035;
/// Longest time between drawn points (s): the Earth–Moon plotting frame
/// turns about 2° an hour, so straight inertial legs still bend there.
pub const MAX_DT: f64 = 3_600.0;
/// Most subdivisions of one interval.
const MAX_SPLIT: usize = 32;

/// A drawn point: its time and camera-relative position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinePoint {
    pub t: Epoch,
    pub p: Vec3,
}

/// A visible run of a line in one colour.
#[derive(Clone, Debug)]
pub struct Piece {
    pub colour: Color,
    pub points: Vec<LinePoint>,
}

/// One vessel's line this frame.
#[derive(Clone, Debug)]
pub struct VesselLine {
    pub id: VesselId,
    pub active: bool,
    /// Where the drawn line ends (the line rule, D056).
    pub end: Epoch,
    pub pieces: Vec<Piece>,
}

/// This frame's vessel lines.
#[derive(Resource, Default)]
pub struct Lines {
    pub vessels: Vec<VesselLine>,
    /// Points evaluated this frame (a performance readout).
    pub evaluated: usize,
}

impl Lines {
    pub fn get(&self, id: VesselId) -> Option<&VesselLine> {
        self.vessels.iter().find(|l| l.id == id)
    }
}

/// Angle between two directions (rad).
pub fn turn(a: DVec3, b: DVec3) -> f64 {
    a.cross(b).length().atan2(a.dot(b))
}

/// Local times at which to draw `seg` over `[t0, t1]`: the stored samples
/// between, thinned while the velocity turns less than [`MAX_TURN`] from
/// the last kept point (within [`MAX_DT`]), and one interval subdivided
/// where it alone turns more. Each time comes with its velocity.
pub fn sample_times(seg: &Segment, t0: f64, t1: f64) -> Vec<(f64, DVec3)> {
    let Some((a0, _, v0)) = seg.eval(t0) else { return Vec::new() };
    let Some((a1, _, v1)) = seg.eval(t1) else { return vec![(t0, v0)] };
    let lo = seg.samples.partition_point(|s| s.s.t <= t0);
    let hi = seg.samples.partition_point(|s| s.s.t < t1);
    let inner = seg.samples[lo..hi.max(lo)].iter().map(|s| (s.s.t, s.s.v, s.anchor));
    let candidates = std::iter::once((t0, v0, a0)).chain(inner).chain(std::iter::once((t1, v1, a1)));
    let mut out = vec![(t0, v0)];
    // Velocities are relative to each sample's anchor: turns are measured
    // only between samples on the same anchor (`last_v` is the last kept
    // point's velocity on the current one).
    let mut prev = (t0, v0, a0);
    let mut last_v = v0;
    for c in candidates.skip(1) {
        if c.2 != prev.2 {
            // An anchor switch: keep the point, restart the comparisons.
            if c.0 > out.last().expect("started").0 {
                out.push((c.0, c.1));
            }
            (prev, last_v) = (c, c.1);
            continue;
        }
        if c.0 <= prev.0 {
            continue;
        }
        let (step_turn, step_dt) = (turn(prev.1, c.1), c.0 - prev.0);
        let last_t = out.last().expect("started").0;
        if step_turn > MAX_TURN || step_dt > MAX_DT {
            // One coarse interval: keep its start, split it on the interpolant.
            if prev.0 > last_t {
                out.push((prev.0, prev.1));
            }
            let n = ((step_turn / MAX_TURN).max(step_dt / MAX_DT).ceil() as usize).clamp(1, MAX_SPLIT);
            for k in 1..n {
                let t = prev.0 + step_dt * k as f64 / n as f64;
                if let Some((_, _, v)) = seg.eval(t) {
                    out.push((t, v));
                }
            }
            out.push((c.0, c.1));
            last_v = c.1;
        } else if (turn(last_v, c.1) > MAX_TURN || c.0 - last_t > MAX_DT) && prev.0 > last_t {
            out.push((prev.0, prev.1));
            last_v = prev.1;
        }
        prev = c;
    }
    if out.last().is_some_and(|l| l.0 < t1) {
        out.push((t1, v1));
    }
    out
}

/// The view pyramid's four side planes, as bits of a point that is outside
/// them (camera at the origin looking along `forward`, `tan_x`/`tan_y` the
/// tangents of the half fields of view).
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub tan_x: f32,
    pub tan_y: f32,
}

impl Frustum {
    pub fn outside(&self, p: Vec3) -> u8 {
        let (x, y, z) = (p.dot(self.right), p.dot(self.up), p.dot(self.forward));
        let mut bits = 0;
        bits |= u8::from(x > self.tan_x * z);
        bits |= u8::from(-x > self.tan_x * z) << 1;
        bits |= u8::from(y > self.tan_y * z) << 2;
        bits |= u8::from(-y > self.tan_y * z) << 3;
        bits
    }

    /// Whether the straight piece `a`–`b` is certainly out of view.
    pub fn culls(&self, a: Vec3, b: Vec3) -> bool {
        self.outside(a) & self.outside(b) != 0
    }
}

/// Screen pixels a chord strays from the arc it replaces: the sagitta
/// (chord × turn / 8) over the nearer end's distance, times the focal
/// length in pixels.
pub fn chord_error_px(a: DVec3, b: DVec3, turn: f64, focal: f64) -> f64 {
    let depth = a.length().min(b.length()).max(1e-3);
    (a - b).length() * turn / 8.0 / depth * focal
}

/// What drawing a segment needs from the frame.
pub struct Viewer<'a> {
    pub plotter: &'a Plotter<'a>,
    pub frustum: Frustum,
    pub focal: f64,
}

/// The visible runs of `seg` over `[t0, t1]` (camera-relative), with the
/// number of points evaluated. `start` replaces the first point (the ship
/// as drawn, for a prediction computed a moment ago).
pub fn segment_runs(
    seg: &Segment,
    t0: f64,
    t1: f64,
    view: &Viewer,
    start: Option<DVec3>,
) -> (Vec<Vec<LinePoint>>, usize) {
    let times = sample_times(seg, t0, t1);
    let at = |t: f64| {
        let (anchor, r, _) = seg.eval(t)?;
        Some(view.plotter.plot(anchor, r, seg.t0.add_seconds(t)))
    };
    let mut pts: Vec<(f64, DVec3, DVec3)> = times.iter().filter_map(|&(t, v)| Some((t, at(t)?, v))).collect();
    if let (Some(s), Some(first)) = (start, pts.first_mut()) {
        first.1 = s;
    }
    let mut evaluated = pts.len();
    let mut runs = Vec::new();
    let mut run: Vec<LinePoint> = Vec::new();
    let point = |t: f64, p: DVec3| LinePoint { t: seg.t0.add_seconds(t), p: p.as_vec3() };
    for w in pts.windows(2) {
        let ((ta, pa, va), (tb, pb, vb)) = (w[0], w[1]);
        if view.frustum.culls(pa.as_vec3(), pb.as_vec3()) {
            if run.len() > 1 {
                runs.push(std::mem::take(&mut run));
            }
            run.clear();
            continue;
        }
        if run.is_empty() {
            run.push(point(ta, pa));
        }
        // (Capped: across an anchor switch the velocities are not comparable.)
        let err = chord_error_px(pa, pb, turn(va, vb).min(4.0 * MAX_TURN), view.focal);
        if err > 1.0 {
            let n = (err.sqrt().ceil() as usize).clamp(1, MAX_SPLIT);
            for k in 1..n {
                let t = ta + (tb - ta) * k as f64 / n as f64;
                if let Some(p) = at(t) {
                    run.push(point(t, p));
                    evaluated += 1;
                }
            }
        }
        run.push(point(tb, pb));
    }
    if run.len() > 1 {
        runs.push(run);
    }
    (runs, evaluated)
}

/// Builds and draws the future trajectories of the active vessel and of
/// tracked vessels in map view (the stored trajectory from now through
/// every planned burn, or the powered prediction).
#[allow(clippy::too_many_arguments)]
pub fn draw(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    pred: Res<Prediction>,
    ui: Res<UiState>,
    map: Res<MapView>,
    tracked: Res<Tracked>,
    orbits: Res<OrbitSettings>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    mut lines: ResMut<Lines>,
    mut gizmos: Gizmos,
) {
    lines.vessels.clear();
    lines.evaluated = 0;
    let Some(v) = crate::map::view(&cam) else { return };
    let Ok((_, tf, _)) = cam.single() else { return };
    let Some(plotter) = Plotter::new(&sim, &rig, ui.plot_frame) else { return };
    let tan_y = (v.fov() * 0.5).tan() as f32;
    let frustum = Frustum {
        right: tf.right().as_vec3(),
        up: tf.up().as_vec3(),
        forward: tf.forward().as_vec3(),
        tan_x: tan_y * v.aspect() as f32 * 1.05,
        tan_y: tan_y * 1.05,
    };
    let viewer = Viewer { plotter: &plotter, frustum, focal: v.focal() };
    for (i, vessel) in sim.fleet.iter().enumerate() {
        let active = i == sim.active;
        if (!active && !tracked.is_tracked(vessel.id())) || !map.in_map(ObjectId::Vessel(vessel.id())) {
            continue;
        }
        let segs: Vec<&Segment> = vessel_segments(&sim, &pred, i)
            .into_iter()
            .filter(|s| s.t0.add_seconds(s.computed_until()).seconds_since(sim.clock) > 0.0)
            .collect();
        let Some(&last) = segs.last() else { continue };
        let mut line = VesselLine { id: vessel.id(), active, end: sim.clock, pieces: Vec::new() };
        let mut after_burn = false;
        for (n, &seg) in segs.iter().enumerate() {
            let Some((t_start, t_end)) = future_span(seg, sim.clock) else { continue };
            // The line rule (D056) on the last segment: one revolution by default.
            let t_end = if std::ptr::eq(seg, last) {
                t_end.min(vessel_line_end(&sim.world, seg, t_start, &orbits).0)
            } else {
                t_end
            };
            line.end = seg.t0.add_seconds(t_end);
            let burn = matches!(seg.kind, SegmentKind::Burn(_));
            after_burn |= burn;
            let colour = match (burn, after_burn, active) {
                (true, _, _) => Color::srgb(1.0, 0.35, 0.2),
                (false, true, _) => Color::srgb(0.3, 0.85, 1.0),
                (false, false, true) => Color::srgb(1.0, 0.85, 0.2),
                (false, false, false) => Color::srgba(0.7, 0.75, 0.8, 0.6),
            };
            // The first line starts at the ship as drawn.
            let start = (active && n == 0).then(|| {
                let (a, r, _) = vessel.state_at(&sim.world, sim.clock);
                sim.world.snapshot(sim.clock).relative_r(a, rig.anchor) + r - rig.cam_pos
            });
            let (runs, evaluated) = segment_runs(seg, t_start, t_end, &viewer, start);
            lines.evaluated += evaluated;
            line.pieces.extend(runs.into_iter().map(|points| Piece { colour, points }));
        }
        for piece in &line.pieces {
            for run in clip::front_runs(piece.points.iter().map(|p| p.p), frustum.forward, 1.0) {
                gizmos.linestrip(run, piece.colour);
            }
        }
        lines.vessels.push(line);
    }
}

#[cfg(test)]
#[path = "lines_tests.rs"]
mod tests;
