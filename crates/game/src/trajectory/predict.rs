//! The active vessel's predicted path where its stored trajectory does not
//! give it (D089): while it flies live, past the entry of a coast into air
//! that needs live flight, and past a craft of parts' next node (D126: the
//! stored coast does not hold its planned burns; from each node, the coast
//! from the state its Δv gives, `Vessel::predict_node`). In air the path is the vessel's own force
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

/// Whether a vessel's line needs a prediction: it flies live, its coast
/// enters air that needs live flight, or a node waits.
pub fn needed(vessel: &Vessel) -> bool {
    match &vessel.phase {
        Phase::Powered { .. } => true,
        Phase::Coasting { trajectory } => trajectory.last().entry.is_some() || !vessel.nodes_unburnt().is_empty(),
        Phase::Landed { .. } | Phase::Crashed { .. } => false,
    }
}

/// The path `vessel` would fly with `controls` and the engine off, past its
/// nodes not yet burnt.
pub fn path(world: &World, vessel: &Vessel, controls: &Controls, orbits: &OrbitSettings) -> Path {
    past_nodes(world, vessel, unplanned(world, vessel, controls, orbits), orbits)
}

/// Whether `seg` is computed at `t`.
fn covers(seg: &Segment, t: Epoch) -> bool {
    let s = t.seconds_since(seg.t0);
    s >= 0.0 && s <= seg.computed_until()
}

/// `base` (the path with no planned burns) past the nodes not yet burnt:
/// cut at each node, then the coast from the state the node's Δv gives
/// there, with the mass after it (a prediction: the program flies to that
/// orbit closed-loop, D126). While coasting, the path takes over from the
/// stored segment the first node falls in, unless the coast enters the air
/// first.
fn past_nodes(world: &World, vessel: &Vessel, base: Path, orbits: &OrbitSettings) -> Path {
    let nodes = vessel.nodes_unburnt();
    let Some(first) = nodes.first() else { return base };
    let (mut segs, after) = match vessel.trajectory() {
        Some(tr) => {
            let entry = base.segments.first().map(|s| s.t0.add_seconds(s.computed_until()));
            if base.after.is_some() && entry.is_some_and(|e| e < first.t) {
                return base;
            }
            let Some(seg) = tr.segments.iter().find(|s| covers(s, first.t)) else { return base };
            (vec![seg.clone()], Some((vessel.id(), seg.t0)))
        }
        None => (base.segments, None),
    };
    for (k, node) in nodes.iter().enumerate() {
        if let Some(last) = segs.last_mut().filter(|s| !covers(s, node.t)) {
            last.extend_until(world, node.t.seconds_since(last.t0), COAST_STEPS);
        }
        let Some(j) = segs.iter().position(|s| covers(s, node.t)) else { break };
        let local = node.t.seconds_since(segs[j].t0);
        let Some(cut) = segs[j].cut_at(local, EndKind::BurnStart) else { break };
        let Some(state) = cut.eval(local) else { break };
        segs.truncate(j);
        segs.push(cut);
        let Some(p) = vessel.predict_node(world, node, k == 0, Some(state)) else { break };
        let mut seg = vessel.coast_from(world, node.t, p.after, p.mass);
        match nodes.get(k + 1) {
            Some(next) => {
                seg.extend_until(world, next.t.seconds_since(node.t), COAST_STEPS);
            }
            None => extend_to_line_end(world, &mut seg, orbits, COAST_STEPS),
        }
        segs.push(seg);
    }
    Path { segments: segs, after }
}

/// The path `vessel` would fly with `controls` and the engine off, with no
/// planned burns.
fn unplanned(world: &World, vessel: &Vessel, controls: &Controls, orbits: &OrbitSettings) -> Path {
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
    let (propellant, thrust, props) = landing::propulsion_of(vessel, mass);
    Some(LandingStart {
        t,
        anchor,
        r,
        v,
        propellant,
        thrust,
        mass: props,
        infinite: vessel.debug(),
        throttle: 0.0,
        chute,
        deploy: vessel.chute_deploy(),
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
