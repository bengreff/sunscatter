//! The settling push before a start (layer 2, D128; D126; falcon9.md §3.5):
//! the ship computer's ullage function. An ignition it carries on floating
//! propellant waits (up to [`SETTLE_LIMIT`]) while the thrusters that push
//! along the engine's thrust fire (a second stage's nitrogen pods), or drag
//! settles it, and is passed to the engine once its tanks' outlets are
//! covered; commanded again while it waits, it is passed at once (the
//! engine then ingests gas if its propellant still floats).
//!
//! **Why layer 2** (software-layers.md §8, open point 1, decided by D128's
//! rule): the wait reads the feed tanks and fires other parts' thrusters,
//! so it coordinates parts, and the real sequence's ullage burn is the
//! stage computer's. The engine controller keeps only its own start on
//! command (`propulsion::light`, layer 1). D126: an automatic burn settles
//! its propellant with the craft's own thrusters; by hand the player does
//! the same with the translation keys and ignites.
//!
//! **What it reads:** the true liquid position (`propulsion::feeds_settled`),
//! as before the move (no bits changed); reading the level sensors through
//! `Knowledge`, and a timed ullage burn where they do not read the tanks,
//! is FIXES3's (flight-software.md, Not done). Its state is each engine's
//! `settling` (`EngineState`, saved): how long the wait has run.

use crate::craft::ship::ShipDef;
use crate::craft::state::{EngineStatus, ShipState};
use crate::systems::{propulsion, rcs};
use crate::time::Epoch;
use glam::DVec3;

/// The longest the ullage function waits for an engine's propellant to
/// settle (s): then it gives up (the engine's ignition kept).
pub const SETTLE_LIMIT: f64 = 120.0;

/// Whether the ship computer holds engine `i`'s ignition (commanded at
/// `at`, from the stage sequence or not) to settle its propellant first:
/// its feeds floating and not already waiting. Holding, its wait starts and
/// the event says how; waiting already, the ignition is passed on.
pub fn hold(def: &ShipDef, state: &mut ShipState, i: usize, at: Epoch, sequence: bool) -> bool {
    if state.engines[i].settling.is_some() || propulsion::feeds_settled(def, state, i) {
        return false;
    }
    state.engines[i].settling = Some(0.0);
    let d = &def.engines[i];
    let how = if rcs::can_push(def, state, d.data.dir) { "settling" } else { "waiting for" };
    // From the sequence, the next stage order starts it
    // (`command::next_stage`).
    let again = if sequence { "stage" } else { "ignite" };
    state.event(at, format!("{}: {how} its propellant before the start ({again} again to start now)", d.id), false);
    true
}

/// The direction (body axes) the thrusters push for the engines waiting on
/// their propellant (their thrust's), if any is waiting.
pub fn push(def: &ShipDef, state: &ShipState) -> DVec3 {
    let want: DVec3 = (def.engines.iter().zip(&state.engines))
        .filter(|(_, r)| r.settling.is_some())
        .map(|(d, _)| d.data.dir.normalize())
        .sum();
    if want.length() > 0.0 {
        want.normalize()
    } else {
        DVec3::ZERO
    }
}

/// The engines waiting over a tick at `at` of `dt` (`debug`: D064, no
/// waiting): each starts once its propellant is settled, or is given up
/// after [`SETTLE_LIMIT`]; one lit or stopped otherwise waits no more.
pub fn step(def: &ShipDef, state: &mut ShipState, at: Epoch, dt: f64, debug: bool) {
    for i in 0..def.engines.len() {
        let Some(waited) = state.engines.get(i).and_then(|r| r.settling) else { continue };
        if state.engines[i].status != EngineStatus::Off {
            state.engines[i].settling = None;
        } else if propulsion::feeds_settled(def, state, i) || debug {
            propulsion::light(def, state, i);
            state.event(at, format!("{}: settled after {waited:.1} s, starting", def.engines[i].id), false);
        } else if waited + dt > SETTLE_LIMIT {
            state.engines[i].settling = None;
            let text = format!("{}: start abandoned, its propellant would not settle", def.engines[i].id);
            state.event(at, text, true);
        } else {
            state.engines[i].settling = Some(waited + dt);
        }
    }
}
