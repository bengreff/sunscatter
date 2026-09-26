//! Trajectory display: stored segments resampled into the plotting frame,
//! and apsides found along them (the true extrema of distance to the primary
//! along the N-body trajectory, not conic elements).

use crate::camera::CameraRig;
use crate::hud::{PlotFrame, UiState};
use crate::map::MapMode;
use crate::state::{Prediction, SimState};
use crate::tracking::Tracked;
use bevy::prelude::*;
use glam::{DMat3, DVec3};
use sim::frame::NodeId;
use sim::time::Epoch;
use sim::vessel::Segment;
use sim::world::World;

/// Number of points used to draw a trajectory.
pub const TRAJECTORY_POINTS: usize = 600;

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

/// The active vessel's displayed segment: the stored one while coasting,
/// otherwise the background prediction.
pub fn active_segment<'a>(sim: &'a SimState, pred: &'a Prediction) -> Option<&'a Segment> {
    sim.ship().segment().or(pred.segment.as_ref())
}

/// An apsis on the displayed trajectory.
#[derive(Clone, Copy, Debug)]
pub struct Apsis {
    pub is_apo: bool,
    /// Local segment time.
    pub t: f64,
    /// Distance from the primary's centre (m).
    pub distance: f64,
}

/// Finds apsides of `seg` relative to `primary` over `[t0, t1]`: local
/// extrema of the sampled distance, refined by a parabola through the three
/// samples around each extremum.
pub fn apsides(world: &World, seg: &Segment, primary: NodeId, t0: f64, t1: f64, n: usize) -> Vec<Apsis> {
    let dist = |t: f64| {
        let (anchor, r, _) = seg.eval(t)?;
        Some((r + world.eph.relative(anchor, primary, seg.t0.add_seconds(t)).r).length())
    };
    let ts: Vec<f64> = (0..n).map(|k| t0 + (t1 - t0) * k as f64 / (n - 1) as f64).collect();
    let ds: Vec<Option<f64>> = ts.iter().map(|&t| dist(t)).collect();
    let mut out = Vec::new();
    for k in 1..n - 1 {
        let (Some(a), Some(b), Some(c)) = (ds[k - 1], ds[k], ds[k + 1]) else { continue };
        let is_apo = b > a && b >= c;
        let is_peri = b < a && b <= c;
        if !(is_apo || is_peri) {
            continue;
        }
        let h = ts[k + 1] - ts[k];
        let denom = a - 2.0 * b + c;
        let offset = if denom.abs() > 0.0 { 0.5 * (a - c) / denom } else { 0.0 };
        let t = ts[k] + offset.clamp(-1.0, 1.0) * h;
        let distance = dist(t).unwrap_or(b);
        out.push(Apsis { is_apo, t, distance });
    }
    out
}

/// Draws the future trajectories of the active vessel and of tracked vessels
/// (map mode only), resampled at
/// uniform times with the segment's own interpolation, so the line starts
/// exactly at the ship and stays smooth.
pub fn draw(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    pred: Res<Prediction>,
    ui: Res<UiState>,
    map: Res<MapMode>,
    tracked: Res<Tracked>,
    mut gizmos: Gizmos,
) {
    if !map.active {
        return;
    }
    let Some(plotter) = Plotter::new(&sim, &rig, ui.plot_frame) else { return };
    for (i, vessel) in sim.fleet.iter().enumerate() {
        let active = i == sim.active;
        if !active && !tracked.is_tracked(i) {
            continue;
        }
        let seg = if active { active_segment(&sim, &pred) } else { vessel.segment() };
        let Some(seg) = seg else { continue };
        let Some((t_start, t_end)) = future_span(seg, sim.clock) else { continue };
        // One revolution is enough: later passes nearly overlap it.
        let t_end = t_end.min(
            t_start
                + crate::relations::vessel_orbit(&sim.world, sim.clock, vessel)
                    .and_then(|o| o.period())
                    .unwrap_or(f64::INFINITY),
        );
        let points = if active { TRAJECTORY_POINTS } else { TRAJECTORY_POINTS / 3 };
        let resampled = (0..points).filter_map(|k| {
            let t = t_start + (t_end - t_start) * k as f64 / (points - 1) as f64;
            let (anchor, r, _) = seg.eval(t)?;
            Some(plotter.plot(anchor, r, seg.t0.add_seconds(t)).as_vec3())
        });
        if active {
            // While powered the prediction was computed a moment ago; join it
            // to the ship's current position so the line starts at the ship.
            let (ship_anchor, ship_r, _) = sim.ship().state(&sim.world);
            let ship_now = sim.world.snapshot(sim.clock).relative_r(ship_anchor, rig.anchor) + ship_r - rig.cam_pos;
            gizmos.linestrip(std::iter::once(ship_now.as_vec3()).chain(resampled), Color::srgb(1.0, 0.85, 0.2));
        } else {
            gizmos.linestrip(resampled, Color::srgba(0.7, 0.75, 0.8, 0.6));
        }
    }
}
