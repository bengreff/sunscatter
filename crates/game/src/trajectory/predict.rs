//! The active vessel's predicted path where its stored trajectory does not
//! give it (D089): while it flies live, and past the entry of a coast into
//! air that needs live flight. In air the path is the vessel's own force
//! model (`sim::landing::air_path`: its aerodynamic bake against the air
//! turning with the planet, at the attitude its controls will hold, the
//! engine off), until the air is clear for coasting by the vessel's own
//! hand-over rule; then the coast it would fly. Computed by the sim on a
//! background task (`state::update_prediction`); the line only samples it
//! (rule 4).
//!
//! Before, the prediction coasted through dense air with the coast's
//! thin-air drag (the orientation-averaged area in the hypersonic limit),
//! and the line, drawn without the ground's rotation, turned sharply where
//! the air caught the craft and carried it east (the owner's "rotation of
//! the earth accounted for oddly", 2026-10-26; reproduced in the tests).

use super::extend_to_line_end;
use super::settings::OrbitSettings;
use glam::DVec3;
use sim::frame::NodeId;
use sim::landing::{self, LandingStart, Limits};
use sim::time::Epoch;
use sim::vessel::{limbo, Controls, EndKind, Phase, Segment, Vessel, VesselId, LEAVE_ATMOSPHERE};
use sim::world::World;

/// Steps of a predicted coast, at most.
const COAST_STEPS: usize = 40_000;

/// A predicted path.
#[derive(Clone, Debug, Default)]
pub struct Path {
    /// The segments, in time order.
    pub segments: Vec<Segment>,
    /// While coasting: the vessel and the start of the stored segment the
    /// path takes over from (its first segment is that one, cut at its
    /// entry).
    pub after: Option<(VesselId, Epoch)>,
}

/// Whether a vessel's line needs a prediction: it flies live, or its coast
/// enters air that needs live flight.
pub fn needed(vessel: &Vessel) -> bool {
    match &vessel.phase {
        Phase::Powered { .. } => true,
        Phase::Coasting { trajectory } => trajectory.last().entry.is_some(),
        Phase::Landed { .. } | Phase::Crashed { .. } => false,
    }
}

/// The path `vessel` would fly with `controls` and the engine off.
pub fn path(world: &World, vessel: &Vessel, controls: &Controls, orbits: &OrbitSettings) -> Path {
    let (mut segments, after, t, (anchor, r, v), mass) = match &vessel.phase {
        Phase::Coasting { trajectory } => {
            let last = trajectory.last();
            let Some((entry, _)) = last.entry else { return Path::default() };
            let (Some(cut), Some(state)) = (last.cut_at(entry, EndKind::Horizon), last.eval(entry)) else {
                return Path::default();
            };
            (vec![cut], Some((vessel.id(), last.t0)), last.t0.add_seconds(entry), state, last.mass_at(entry))
        }
        Phase::Powered { .. } => {
            let state = vessel.state(world);
            let snap = world.snapshot(vessel.time);
            if limbo::may_coast(&vessel.limbo.boundary, world, &snap, state.0, (state.1, state.2), LEAVE_ATMOSPHERE) {
                // Clear air (a burn in vacuum): the coast at once.
                let coast = coast(world, vessel, vessel.time, state, vessel.mass(), orbits);
                return Path { segments: vec![coast], after: None };
            }
            (Vec::new(), None, vessel.time, state, vessel.mass())
        }
        Phase::Landed { .. } | Phase::Crashed { .. } => return Path::default(),
    };
    let Some(start) = air_start(world, vessel, controls, t, (anchor, r, v), mass) else { return Path::default() };
    let limits = Limits::two_orbits(world, &start);
    let air =
        landing::air_path(world, &vessel.craft, &start, vessel.limbo.boundary, limits, vessel.proper_time_offset());
    let end = air.segment.computed_until();
    let exit = air.cleared.then(|| air.segment.eval(end)).flatten();
    let t_exit = air.segment.t0.add_seconds(end);
    segments.push(air.segment);
    if let Some(state) = exit {
        segments.push(coast(world, vessel, t_exit, state, mass, orbits));
    }
    Path { segments, after }
}

/// Where the path through the air starts: the vessel's chutes (armed by
/// the command) and the attitude its controls will hold
/// (`landing::assumed_attitude`, the Landing menu's rule).
fn air_start(
    world: &World,
    vessel: &Vessel,
    controls: &Controls,
    t: Epoch,
    (anchor, r, v): (NodeId, DVec3, DVec3),
    mass: f64,
) -> Option<LandingStart> {
    let body = landing::nearest_surface(world, &world.snapshot(t), anchor, r)?;
    let mut chute = if vessel.destruction().is_none() { vessel.chute } else { Default::default() };
    chute.armed |= controls.chute;
    Some(LandingStart {
        t,
        anchor,
        r,
        v,
        propellant: (mass - vessel.craft.mass.dry_mass).max(0.0),
        infinite: vessel.debug(),
        throttle: 0.0,
        chute,
        attitude: crate::landing::assumed_attitude(controls, vessel.attitude.q),
        body,
    })
}

/// The coast from `state` at `t`, computed to the line's end.
fn coast(
    world: &World,
    vessel: &Vessel,
    t: Epoch,
    state: (NodeId, DVec3, DVec3),
    mass: f64,
    orbits: &OrbitSettings,
) -> Segment {
    let mut seg = vessel.coast_from(world, t, state, mass);
    extend_to_line_end(world, &mut seg, orbits, COAST_STEPS);
    seg
}

#[cfg(test)]
#[path = "predict_tests.rs"]
mod tests;
