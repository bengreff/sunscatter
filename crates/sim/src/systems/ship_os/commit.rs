//! The pad's launch commit (layer 2, D128; falcon9.md §1.2, §4.2): the
//! sequence's clamp release is armed (`systems::command`), and the clamps
//! let go at the first tick every engine it waits for reports its rated
//! thrust through its controller; an engine that fails or stops first, or
//! whose readout no computer receives, aborts the launch (every engine shut
//! down, the clamps kept); armed with none lit, it is disarmed. A release
//! disarmed without letting go did not happen: its stage is next again
//! ([`disarm`]). The engines themselves only report (`propulsion`'s
//! controllers, layer 1); this decides on their readouts, as the real
//! launch commit does.

use super::{engine_readings, EngineReading};
use crate::craft::command::ShipCommand;
use crate::craft::ship::ShipDef;
use crate::craft::state::{Effect, EngineStatus, ShipState};
use crate::systems::propulsion;
use crate::time::Epoch;

/// The pad's launch commit for the engines the armed release waits for.
#[derive(Clone, Debug, PartialEq)]
pub enum Commit {
    /// Not all at their rated thrust yet.
    Wait,
    /// Every one at its rated thrust: let the clamps go.
    Release,
    /// An engine reports it stopped, or reports nothing: shut every engine
    /// down and keep the clamps.
    Abort { engine: usize, why: String },
}

/// The pad's launch commit (falcon9.md §1.2, §4.2): the clamps let go at
/// the first tick every engine in `waits` reports, through its controller,
/// that it runs at its rated level; one whose controller reports it stopped,
/// or whose readout reaches no computer (a lost controller, a dead stage
/// computer), aborts the launch.
pub fn commit(def: &ShipDef, engines: &[Option<EngineReading>], waits: &[usize]) -> Commit {
    for &i in waits {
        match engines.get(i).and_then(Option::as_ref) {
            None => return Commit::Abort { engine: i, why: "no thrust reading".into() },
            Some(r) if !matches!(r.status, EngineStatus::Starting | EngineStatus::Running) => {
                let why = r.reason.clone().unwrap_or_else(|| "stopped".into());
                return Commit::Abort { engine: i, why };
            }
            Some(_) => {}
        }
    }
    let rated = |i: usize| {
        engines[i].as_ref().is_some_and(|r| {
            r.status == EngineStatus::Running && r.level >= def.engines[i].data.throttle_max * (1.0 - 1e-9)
        })
    };
    if waits.iter().all(|&i| rated(i)) {
        Commit::Release
    } else {
        Commit::Wait
    }
}

/// The armed release waits for the engines lit now (and for no other).
pub(crate) fn await_lit(state: &mut ShipState) {
    for e in &mut state.engines {
        e.awaited = matches!(e.status, EngineStatus::Starting | EngineStatus::Running);
    }
}

/// The release waits for no engine (it is disarmed or let go).
fn await_none(state: &mut ShipState) {
    for e in &mut state.engines {
        e.awaited = false;
    }
}

/// Disarms the pad's armed release without letting go, with the event
/// `text`: the release's stage is next again
/// (`command::release_undone`). Nothing if it is not armed.
pub fn disarm(state: &mut ShipState, at: Epoch, text: String, alarm: bool) {
    if !state.release_armed {
        return;
    }
    state.release_armed = false;
    await_none(state);
    state.sequence = super::sequence::release_undone(&state.stages, state.sequence);
    state.event(at, text, alarm);
}

/// The pad's armed release over a tick, after the engines have stepped
/// (see the module): it waits on the launch commit ([`commit`]), which
/// reads the engine controllers' readouts. Returns the effects the vessel
/// acts on (the clamps' release).
pub fn step(def: &ShipDef, state: &mut ShipState, at: Epoch, debug: bool) -> Vec<Effect> {
    if !state.release_armed || !state.clamped {
        state.release_armed = false;
        await_none(state);
        return Vec::new();
    }
    if !state.engines.iter().any(|e| e.awaited) {
        await_lit(state);
        if !state.engines.iter().any(|e| e.awaited) {
            // None lit (one waiting for its propellant to settle is about
            // to): armed, it would hold the vessel in live ticks for ever.
            if !state.engines.iter().any(|r| r.settling.is_some()) {
                disarm(state, at, "Clamps: release disarmed: no engine lit; clamps held".into(), true);
            }
            return Vec::new();
        }
    }
    let waits: Vec<usize> = (state.engines.iter().enumerate()).filter(|(_, e)| e.awaited).map(|(i, _)| i).collect();
    let k = crate::systems::data::knowledge(def, state, debug);
    match commit(def, &engine_readings(def, state, &k), &waits) {
        Commit::Wait => return Vec::new(),
        Commit::Abort { engine: i, why } => {
            for j in propulsion::lit(state) {
                propulsion::shut_down(state, j as usize, "pad abort");
            }
            disarm(state, at, format!("Pad abort: {} {why}; clamps held", def.engines[i].id), true);
            return Vec::new();
        }
        Commit::Release => {}
    }
    state.release_armed = false;
    await_none(state);
    state.event(at, "Liftoff: the clamps released at full thrust", false);
    state.apply(def, &ShipCommand::ReleaseClamps).unwrap_or_default()
}
