//! Failures (D105; ship-model.md §6): what makes a part of a craft of parts
//! fail, and what then happens to it. Debug mode (D064) turns all of it off.
//!
//! * **Physical, deterministic:** a part at its destruction temperature is
//!   destroyed; between its rated and destruction temperatures it takes
//!   thermal damage over time and is destroyed at 1 ([`damage`]); a tank or
//!   a bottle over its burst pressure bursts; a joint over its ultimate
//!   snaps (`super::structure`).
//! * **Accumulated damage:** vibration against the qualification level
//!   (Miner's rule, [`damage`]); an engine or a mechanism whose vibration
//!   damage reaches 1 fails, other parts carry it as a doubled hazard (what a
//!   fatigued shell then does is not known: no outcome).
//! * **Statistical** ([`fates`]): each catalog failure mode of each part and
//!   component draws its threshold once from the save's seed, the launch
//!   epoch, the owner and the mode, and accumulates its hazard per hour of
//!   operation, per start or per cycle; it fails when the hazard reaches it.
//! * **Outcomes**, by mode: an engine's benign shutdown (the controller
//!   stops it, `propulsion::stop`) or an ignition failure (the start
//!   aborted); an uncontained engine failure (the engine destroyed); a
//!   valve stuck open or closed; a battery cell lost; a function lost (a
//!   computer, a sensor, a pump: recorded in `Runtime::lost`; a mechanism
//!   lost freezes where it is); a part destroyed (its mass a wreck: a
//!   split). A burst tank or bottle explodes ([`blast`]): its gas's
//!   expansion energy and its propellants' yield, whose overpressure
//!   reaches the other parts by distance; a tank or a bottle whose burst
//!   rating it exceeds bursts in turn, so failures cascade.

pub mod blast;
pub mod damage;
pub mod fates;
pub mod physical;

use crate::catalog::{FailureMode, Hazard, Outcome, Ratings};
use crate::craft::ship::ShipDef;
use crate::craft::state::{Effect, EngineStatus, Fate, PartFailure, ShipState};
use crate::systems::ship::TickEnv;
use crate::time::Epoch;

/// One live (or held) tick of failures (see the module): the fates'
/// hazards and outcomes; with the parts' environment (`env.bodies`), time at
/// temperature and vibration; bursts and their blasts. The parts destroyed
/// are returned as effects (each once). Nothing in debug mode.
pub fn step(def: &ShipDef, state: &mut ShipState, env: &TickEnv) -> Vec<Effect> {
    if env.debug {
        return Vec::new();
    }
    if let Some(launch) = state.runtime.launch {
        draw_fates(def, state, state.runtime.seed, launch);
    }
    let mut destroyed: Vec<(u32, PartFailure)> = Vec::new();
    for k in accumulate(def, state, env.dt) {
        if let Consequence::Destroyed(part, why) = outcome(def, state, k, env.at) {
            destroyed.push((part, why));
        }
    }
    let ambient = env.bodies.as_ref().map_or(env.p, |b| b.ambient);
    if let Some(b) = &env.bodies {
        destroyed.extend(physical::thermal(def, state, b, env.dt));
        for part in physical::vibration(def, state, b, env.dt) {
            fatigue(def, state, part, env.at);
        }
    }
    destroyed.extend(physical::bursts(def, state, ambient, env.at));
    freeze_lost(def, state);
    let mut effects = Vec::new();
    for (part, why) in destroyed {
        let Some(s) = state.parts.iter_mut().find(|p| p.part == part && p.integrity > 0.0) else { continue };
        s.integrity = 0.0;
        let name = def.parts.iter().find(|p| p.part == part).map_or("a part", |p| p.id.as_str());
        state.runtime.event(env.at, format!("{name}: destroyed ({})", why.describe()), true);
        effects.push(Effect::Destroy(part, why));
    }
    effects
}

/// A part whose vibration damage reached 1: an engine fails (its
/// controller stops it), a mechanism's part loses its function; another
/// part carries the damage (its hazards doubled; no outcome is known).
fn fatigue(def: &ShipDef, state: &mut ShipState, part: u32, at: Epoch) {
    let Some(p) = def.parts.iter().find(|p| p.part == part) else { return };
    let engines: Vec<usize> = (0..def.engines.len()).filter(|&i| def.engines[i].part == part).collect();
    if !engines.is_empty() {
        for i in engines {
            if !matches!(state.engines[i].status, EngineStatus::Off | EngineStatus::Failed) {
                let why = super::propulsion::Stop::Fail("vibration fatigue".into());
                super::propulsion::stop(def, state, i, why, at);
            } else {
                state.engines[i].status = EngineStatus::Failed;
            }
        }
    } else if def.mechanisms.iter().any(|m| m.part == part) {
        lose(state, part, &p.id, "vibration fatigue", at);
    } else {
        state.runtime.event(at, format!("{}: vibration damage at its qualification", p.id), true);
    }
}

/// A lost mechanism stays where it is (its actuator is gone).
fn freeze_lost(def: &ShipDef, state: &mut ShipState) {
    for (m, s) in def.mechanisms.iter().zip(state.mechanisms.iter_mut()) {
        if state.runtime.lost.iter().any(|l| l.part == m.part || l.owner == m.id) {
            s.target = s.angle;
        }
    }
}

/// The statistical modes of a ship (owner id, its part, the mode), parts
/// first then components, in the definition's order.
pub fn modes(def: &ShipDef) -> Vec<(&str, u32, &FailureMode)> {
    let parts = def.parts.iter().flat_map(|p| p.failures.iter().map(move |m| (p.id.as_str(), p.part, m)));
    let components = def.components.iter().flat_map(|c| c.failures.iter().map(move |m| (c.id.as_str(), c.part, m)));
    parts.chain(components).collect()
}

/// Draws the fates the ship lacks (each mode once, from the save's `seed`
/// and the `launch` epoch: the same every time; a split keeps them with the
/// parts, so nothing is drawn twice), each counting its owner's starts and
/// cycles from now (a reused part's earlier ones are its history's).
pub fn draw_fates(def: &ShipDef, state: &mut ShipState, seed: u64, launch: Epoch) {
    for (owner, part, m) in modes(def) {
        if !state.fates.iter().any(|f| f.owner == owner && f.mode == m.mode) {
            let threshold = fates::threshold(seed, launch, owner, &m.mode);
            let seen = counter(def, state, owner, part, m.hazard);
            let (owner, mode) = (owner.to_string(), m.mode.clone());
            state.fates.push(Fate { part, owner, mode, threshold, hazard: 0.0, seen });
        }
    }
}

/// The fates drawn again with the save's `seed` (a new game's seed set on a
/// vessel just created, D105 §6.3): each threshold from the seed, the
/// launch epoch, its owner and mode; the hazards and counters stay. The
/// seed is kept in the runtime, so a split's pieces carry it.
pub fn reseed(state: &mut ShipState, seed: u64) {
    state.runtime.seed = seed;
    let Some(launch) = state.runtime.launch else { return };
    for f in &mut state.fates {
        f.threshold = fates::threshold(seed, launch, &f.owner, &f.mode);
    }
}

/// An owner's event counter for a hazard: its part's engine starts, a
/// valve's cycles, else its part's cycles.
fn counter(def: &ShipDef, state: &ShipState, owner: &str, part: u32, hazard: Hazard) -> u32 {
    let history = state.parts.iter().find(|p| p.part == part).map(|p| &p.history);
    match hazard {
        Hazard::PerStart(_) => history.map_or(0, |h| h.starts),
        Hazard::PerCycle(_) => match def.valves.iter().position(|v| v.id == owner) {
            Some(v) => state.valves[v].cycles,
            None => history.map_or(0, |h| h.cycles),
        },
        Hazard::PerHour(_) => 0,
    }
}

/// The ratings, damage and history of an owner (a part, or a component's
/// host part's history).
fn owner_ratings<'a>(def: &'a ShipDef, owner: &str) -> Option<&'a Ratings> {
    (def.parts.iter().find(|p| p.id == owner).map(|p| &p.ratings))
        .or_else(|| def.components.iter().find(|c| c.id == owner).map(|c| &c.ratings))
}

/// What an owner did over a tick: seconds operating, and its event counter
/// (starts of a part's engines, cycles of a valve or of a part).
fn exposure(def: &ShipDef, state: &ShipState, owner: &str, part: u32, hazard: Hazard, dt: f64) -> (f64, u32) {
    let engines: Vec<usize> = (0..def.engines.len()).filter(|&i| def.engines[i].owner == owner).collect();
    let running =
        engines.iter().any(|&i| matches!(state.engines[i].status, EngineStatus::Starting | EngineStatus::Running));
    let powered = (def.components.iter().zip(&state.power)).any(|(c, s)| c.id == owner && s.on);
    let on = if engines.is_empty() { powered } else { running };
    (if on { dt } else { 0.0 }, counter(def, state, owner, part, hazard))
}

/// Accumulates every fate's hazard over a tick of `dt` (its operation, the
/// starts and cycles since it last looked, times its wear and damage);
/// returns the modes that failed now (indices into `state.fates`).
pub fn accumulate(def: &ShipDef, state: &mut ShipState, dt: f64) -> Vec<usize> {
    let mut failed = Vec::new();
    let all = modes(def);
    for k in 0..state.fates.len() {
        let f = &state.fates[k];
        if f.hazard >= f.threshold {
            continue;
        }
        let Some(&(owner, part, mode)) = all.iter().find(|(o, _, m)| *o == f.owner && m.mode == f.mode) else {
            continue;
        };
        let (on, count) = exposure(def, state, owner, part, mode.hazard, dt);
        let events = count.saturating_sub(f.seen);
        let damage = state.parts.iter().find(|p| p.part == part).map_or(0.0, |p| p.thermal_damage + p.vibration_damage);
        let history = state.parts.iter().find(|p| p.part == part).map(|p| p.history.clone()).unwrap_or_default();
        let mult = owner_ratings(def, owner).map_or(1.0, |r| fates::multiplier(r, &history, damage));
        let (starts, cycles) = match mode.hazard {
            Hazard::PerStart(_) => (events, 0),
            Hazard::PerCycle(_) => (0, events),
            Hazard::PerHour(_) => (0, 0),
        };
        let f = &mut state.fates[k];
        f.seen = f.seen.max(count);
        f.hazard += fates::hazard(mode.hazard, on, starts, cycles) * mult;
        if f.hazard >= f.threshold {
            failed.push(k);
        }
    }
    failed
}

/// What a failed mode does, for the vessel to act on beyond the ship state.
#[derive(Clone, Debug, PartialEq)]
pub enum Consequence {
    /// The part (`Part::id`) is destroyed.
    Destroyed(u32, PartFailure),
    /// Nothing beyond the ship state.
    None,
}

/// The outcome of the mode of fate `k` (see the module).
pub fn outcome(def: &ShipDef, state: &mut ShipState, k: usize, at: Epoch) -> Consequence {
    let f = state.fates[k].clone();
    let Some(&(owner, part, mode)) = modes(def).iter().find(|(o, _, m)| *o == f.owner && m.mode == f.mode) else {
        return Consequence::None;
    };
    let engines: Vec<usize> = (0..def.engines.len()).filter(|&i| def.engines[i].owner == owner).collect();
    let why = format!("{} ({})", mode.mode, describe(mode.outcome));
    match mode.outcome {
        Outcome::BenignShutdown => {
            for &i in &engines {
                if !matches!(state.engines[i].status, EngineStatus::Off | EngineStatus::Failed) {
                    super::propulsion::stop(def, state, i, super::propulsion::Stop::Fail(why.clone()), at);
                }
            }
        }
        Outcome::IgnitionFailure => {
            for &i in &engines {
                if state.engines[i].status == EngineStatus::Starting {
                    super::propulsion::stop(def, state, i, super::propulsion::Stop::Abort(why.clone()), at);
                }
            }
        }
        Outcome::Uncontained => {
            for &i in &engines {
                super::propulsion::stop(def, state, i, super::propulsion::Stop::Fail(why.clone()), at);
            }
            state.runtime.event(at, format!("{owner}: uncontained failure, the engine is destroyed"), true);
            return Consequence::Destroyed(part, PartFailure::Uncontained);
        }
        Outcome::StuckOpen | Outcome::StuckClosed => {
            if let Some(v) = def.valves.iter().position(|v| v.id == owner) {
                let s = &mut state.valves[v];
                (s.stuck, s.open) = (true, mode.outcome == Outcome::StuckOpen);
            }
            state.runtime.event(at, format!("{owner}: {why}"), true);
        }
        Outcome::CellLoss => {
            if let Some(b) = def.batteries.iter().position(|b| b.id == owner) {
                // A module of four lost (E: the batteries' modules are not data).
                state.batteries[b].charge *= 0.75;
            }
            state.runtime.event(at, format!("{owner}: {why}"), true);
        }
        Outcome::Burst | Outcome::Melt => {
            state.runtime.event(at, format!("{owner}: {why}"), true);
            return Consequence::Destroyed(part, PartFailure::Burst { pa: 0.0 });
        }
        Outcome::GimbalStuck | Outcome::Leak | Outcome::Lost | Outcome::Snap => {
            lose(state, part, owner, &why, at);
        }
    }
    Consequence::None
}

/// Records `owner`'s function as lost (once) and tells the player.
pub fn lose(state: &mut ShipState, part: u32, owner: &str, why: &str, at: Epoch) {
    if !state.runtime.lost.iter().any(|l| l.owner == owner) {
        state.runtime.lost.push(crate::systems::runtime::Lost { part, owner: owner.into(), why: why.into() });
        state.runtime.event(at, format!("{owner}: lost: {why}"), true);
    }
}

/// The player's words for an outcome.
pub fn describe(o: Outcome) -> &'static str {
    match o {
        Outcome::BenignShutdown => "shut down by its controller",
        Outcome::Uncontained => "uncontained",
        Outcome::IgnitionFailure => "ignition failed",
        Outcome::GimbalStuck => "gimbal stuck",
        Outcome::StuckOpen => "stuck open",
        Outcome::StuckClosed => "stuck closed",
        Outcome::Leak => "leaking",
        Outcome::CellLoss => "a cell lost",
        Outcome::Lost => "function lost",
        Outcome::Burst => "burst",
        Outcome::Snap => "snapped",
        Outcome::Melt => "melted",
    }
}

#[cfg(test)]
mod tests;
