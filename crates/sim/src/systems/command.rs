//! Ship commands as the systems carry them out (D104, D107, D108;
//! ship-model.md §5.0, §5.3, §8): an ignition starts an engine's start
//! sequence (one of its igniter charges, or the ground's supply while the
//! pad's clamps hold it; on floating propellant it first waits for it to
//! settle, `systems::propulsion`), a shutdown its tail-off, a deployment moves a
//! mechanism at its actuator's rate, and the sequence's clamp release is
//! armed until every ignited engine delivers its rated thrust (the real
//! pad's rule); a release commanded by hand lets go at once. Everything
//! else is the ship state's plain effect (`ShipState::apply`).
//!
//! Commands are applied at a tick: orders wait in the runtime for their
//! epoch ([`super::runtime::Runtime::order`]), so the same orders give the
//! same flight whatever the frames.

use super::runtime::OrderKind;
use crate::catalog::Igniter;
use crate::craft::command::{matches, ShipCommand, Stage};
use crate::craft::ship::ShipDef;
use crate::craft::state::{Effect, EngineStatus, ShipState};
use crate::time::Epoch;

/// Where a command comes from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Context {
    /// When it is applied (the tick's start).
    pub at: Epoch,
    /// From the stage sequence (its clamp release waits for the engines).
    pub sequence: bool,
}

/// Whether an engine can light now: the ground's supply on the pad, a
/// hypergolic engine always, else a charge left.
pub fn can_ignite(igniter: Igniter, ignitions: u32, ground: bool) -> bool {
    ground || igniter == Igniter::Hypergolic || ignitions > 0
}

/// Applies `command` (see the module); the effects the vessel acts on are
/// returned. Naming nothing the craft has is an error (nothing changes).
pub fn apply(def: &ShipDef, state: &mut ShipState, command: &ShipCommand, cx: Context) -> Result<Vec<Effect>, String> {
    let none = || Err(format!("{command:?}: names nothing on this craft"));
    state.runtime.refit(def);
    match command {
        ShipCommand::Ignite(s) => {
            let picked = def.engines_of(s);
            if picked.is_empty() {
                return none();
            }
            let ground = state.clamped;
            for i in picked {
                let (d, e) = (&def.engines[i], &mut state.engines[i]);
                if e.status != EngineStatus::Off {
                    continue;
                }
                if !can_ignite(d.data.igniter, e.ignitions, ground) {
                    state.runtime.event(cx.at, format!("{}: no ignition left", d.id), true);
                    continue;
                }
                e.selected = true;
                // Floating propellant: the start sequence waits for it to
                // settle (its acceleration history says when), firing the
                // thrusters that push along the engine's thrust if it has
                // any; commanded again while it waits, it starts at once.
                let waiting = state.runtime.engines[i].settling.is_some();
                if !waiting && !super::propulsion::feeds_settled(def, state, i) {
                    state.runtime.engines[i].settling = Some(0.0);
                    let how = if super::rcs::can_push(def, state, d.data.dir) { "settling" } else { "waiting for" };
                    let text = format!("{}: {how} its propellant before the start (ignite again to start now)", d.id);
                    state.runtime.event(cx.at, text, false);
                    continue;
                }
                super::propulsion::light(def, state, i);
            }
            Ok(Vec::new())
        }
        ShipCommand::Shutdown(s) => {
            let picked = def.engines_of(s);
            if picked.is_empty() {
                return none();
            }
            for i in picked {
                state.runtime.engines[i].settling = None;
                let e = &mut state.engines[i];
                if matches!(e.status, EngineStatus::Starting | EngineStatus::Running) {
                    e.status = EngineStatus::ShuttingDown;
                    e.timer = 0.0;
                    let run = &mut state.runtime.engines[i];
                    run.cutoff = run.level;
                    run.reason = Some("commanded".into());
                }
            }
            Ok(Vec::new())
        }
        ShipCommand::Deploy(s) | ShipCommand::Retract(s) | ShipCommand::Angle(s, _) => {
            let picked: Vec<usize> = (0..def.mechanisms.len()).filter(|&i| matches(s, &def.mechanisms[i].id)).collect();
            if picked.is_empty() {
                return none();
            }
            for i in picked {
                let m = &def.mechanisms[i].item;
                let before = state.mechanisms[i].target;
                let now = &mut state.mechanisms[i];
                now.target = match command {
                    ShipCommand::Deploy(_) => m.deployed_deg,
                    ShipCommand::Retract(_) if m.retractable => m.stowed_deg,
                    ShipCommand::Retract(_) => now.target,
                    ShipCommand::Angle(_, a) => {
                        a.clamp(m.stowed_deg.min(m.deployed_deg), m.stowed_deg.max(m.deployed_deg))
                    }
                    _ => unreachable!(),
                };
                // A movement begun is a cycle of its actuator (its failure
                // modes count them, `systems::failure`).
                if now.target != before {
                    let part = def.mechanisms[i].part;
                    if let Some(p) = state.parts.iter_mut().find(|p| p.part == part) {
                        p.history.cycles += 1;
                    }
                }
            }
            Ok(Vec::new())
        }
        ShipCommand::ReleaseClamps if cx.sequence && state.clamped => {
            state.runtime.release_armed = true;
            state.runtime.release_engines = super::propulsion::lit(state);
            state.runtime.event(cx.at, "Clamps armed: release at full thrust", false);
            Ok(Vec::new())
        }
        _ => state.apply(def, command),
    }
}

/// Runs the next stage of the ship's own sequence: its commands (one naming
/// what the craft lacks is skipped); `None` when the sequence is done.
pub fn next_stage(def: &ShipDef, state: &mut ShipState, at: Epoch) -> Option<(String, Vec<Effect>)> {
    let stage = state.stages.get(state.sequence as usize)?.clone();
    state.sequence += 1;
    let cx = Context { at, sequence: true };
    Some((stage.name.clone(), run(def, state, &stage, cx)))
}

/// Runs a saved function by name; `None` if there is none.
pub fn function(def: &ShipDef, state: &mut ShipState, name: &str, at: Epoch) -> Option<Vec<Effect>> {
    let stage = state.functions.iter().find(|f| f.name == name)?.clone();
    Some(run(def, state, &stage, Context { at, sequence: false }))
}

fn run(def: &ShipDef, state: &mut ShipState, stage: &Stage, cx: Context) -> Vec<Effect> {
    let mut effects = Vec::new();
    for a in &stage.actions {
        effects.extend(apply(def, state, a, cx).unwrap_or_default());
    }
    effects
}

/// Carries out an order: its effects, and a line for the event log.
pub fn carry_out(def: &ShipDef, state: &mut ShipState, kind: &OrderKind, at: Epoch) -> Vec<Effect> {
    match kind {
        OrderKind::Command(c) => match apply(def, state, c, Context { at, sequence: false }) {
            Ok(e) => e,
            Err(e) => {
                state.runtime.event(at, e, true);
                Vec::new()
            }
        },
        OrderKind::Stage => match next_stage(def, state, at) {
            Some((name, e)) => {
                state.runtime.event(at, format!("Stage: {name}"), false);
                e
            }
            None => Vec::new(),
        },
        OrderKind::Function(name) => match function(def, state, name, at) {
            Some(e) => {
                state.runtime.event(at, format!("Function: {name}"), false);
                e
            }
            None => {
                state.runtime.event(at, format!("No function {name:?}"), true);
                Vec::new()
            }
        },
    }
}
