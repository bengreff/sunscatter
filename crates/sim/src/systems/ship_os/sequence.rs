//! The stage sequence and the saved functions (layer 2, D128; D107, D111,
//! D114): what Space, AGAIN and a function run, as the ship computer runs
//! them, through the native boundary (`systems::command::apply`) one
//! command at a time; the sequence's edits (the staging bar, the systems
//! screen); the position a release undone puts it back to. The program's
//! view of a piece (which stages it shows and which commands it takes) is
//! `craft::program`.

use crate::craft::command::{SequenceEdit, ShipCommand, Stage};
use crate::craft::program;
use crate::craft::ship::ShipDef;
use crate::craft::state::{Effect, EngineStatus, ShipState};
use crate::systems::command::{apply, Context};
use crate::time::Epoch;

/// Edits the stage sequence (D111: the staging bar in flight), by indices
/// into the program: moves a stage, moves one of a stage's commands to the
/// end of another, or deletes a stage. Only stages still to run (from
/// `sequence` on) in the piece's view (`def`: `craft::program`) change, and
/// only commands it takes move; anything else is refused and nothing
/// changes. A stage left without commands stays (it can be deleted).
pub fn edit_sequence(def: &ShipDef, stages: &mut Vec<Stage>, sequence: u32, edit: &SequenceEdit) -> Result<(), String> {
    let pending = |k: u32| match stages.get(k as usize) {
        Some(s) if k >= sequence && program::shows(def, s) => Ok(k as usize),
        _ => Err(format!("{edit:?}: stage {k} is not one still to run here ({sequence}..{})", stages.len())),
    };
    match *edit {
        SequenceEdit::Move { from, to } => {
            let (from, to) = (pending(from)?, pending(to)?);
            let stage = stages.remove(from);
            stages.insert(to, stage);
        }
        SequenceEdit::MoveAction { from, action, to } => {
            let (from, to) = (pending(from)?, pending(to)?);
            let taken = stages[from].actions.get(action as usize).is_some_and(|c| program::takes(def, c));
            if !taken || from == to {
                return Err(format!("{edit:?}: no such command to move"));
            }
            let command = stages[from].actions.remove(action as usize);
            stages[to].actions.push(command);
        }
        SequenceEdit::Delete(k) => {
            stages.remove(pending(k)?);
        }
    }
    Ok(())
}

/// The sequence's position once a clamp release that did not let go is
/// undone (a pad abort, a split while it was armed; the clamps still hold
/// the vessel): the last stage run that releases the clamps is next again,
/// and the stages run after it are to run again too (review 2026-10-27:
/// after a pad abort, Space ran MECO and the stage separation on the pad).
/// AGAIN on the ignition relights the engines (D114). `sequence` itself if
/// no stage run releases them. A function never moves the sequence.
pub fn release_undone(stages: &[Stage], sequence: u32) -> u32 {
    let run = &stages[..(sequence as usize).min(stages.len())];
    run.iter().rposition(|s| s.actions.contains(&ShipCommand::ReleaseClamps)).map_or(sequence, |k| k as u32)
}

/// Whether an engine named by `s` waits for its propellant to settle.
fn settling(def: &ShipDef, state: &ShipState, s: &str) -> bool {
    let waits = |i: usize| state.engines.get(i).is_some_and(|r| r.settling.is_some());
    def.engines_of(s).into_iter().any(|i| waits(i) && state.engines[i].status == EngineStatus::Off)
}

/// Runs the next stage of the ship's own sequence in its view
/// (`craft::program`: a stage of other pieces only is skipped): its
/// commands the craft takes; `None` when it has none left. While an
/// ignition of its active stage waits for its propellant to settle, the
/// order starts it now instead (its hint says so) and the sequence stays:
/// running on, the next stage's shutdown would cancel the start (review
/// H4: Space while the second stage settled ran SECO).
pub fn next_stage(def: &ShipDef, state: &mut ShipState, at: Epoch) -> Option<(String, Vec<Effect>)> {
    let last = program::active(def, &state.stages, state.sequence).map(|k| &state.stages[k]);
    let waiting: Vec<ShipCommand> = last
        .map(|st| st.actions.iter().filter(|a| matches!(a, ShipCommand::Ignite(s) if settling(def, state, s))))
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    if !waiting.is_empty() {
        let name = format!("{}: start now", last.map_or("", |st| st.name.as_str()));
        let cx = Context { at, sequence: true };
        let effects = waiting.iter().flat_map(|a| apply(def, state, a, cx).unwrap_or_default()).collect();
        return Some((name, effects));
    }
    let k = program::next(def, &state.stages, state.sequence)?;
    let stage = program::mine(def, &state.stages[k]);
    state.sequence = k as u32 + 1;
    let cx = Context { at, sequence: true };
    Some((stage.name.clone(), run(def, state, &stage, cx)))
}

/// Runs stage `k` of the program again (D114), one already run in the
/// craft's view: each of its commands that can act does (an engine with an
/// ignition left starts), the rest do nothing (a joint already parted,
/// clamps let go, a mechanism where it is); the sequence stays. `None` for
/// a stage not yet run or not in the view.
pub fn again(def: &ShipDef, state: &mut ShipState, k: u32, at: Epoch) -> Option<(String, Vec<Effect>)> {
    if !program::has_run(def, &state.stages, state.sequence, k) {
        return None;
    }
    let stage = program::mine(def, &state.stages[k as usize]);
    let cx = Context { at, sequence: true };
    Some((stage.name.clone(), run(def, state, &stage, cx)))
}

/// Runs a saved function by name, its commands the craft takes; `None` if
/// its view has none of that name.
pub fn function(def: &ShipDef, state: &mut ShipState, name: &str, at: Epoch) -> Option<Vec<Effect>> {
    let stage = program::mine(def, program::function(def, &state.functions, name)?);
    Some(run(def, state, &stage, Context { at, sequence: false }))
}

fn run(def: &ShipDef, state: &mut ShipState, stage: &Stage, cx: Context) -> Vec<Effect> {
    let mut effects = Vec::new();
    for a in &stage.actions {
        effects.extend(apply(def, state, a, cx).unwrap_or_default());
    }
    effects
}

/// The interlocks (the real pad's): why `command` is refused for mechanism
/// `i` now, if it is. Legs stay stowed while the pad's clamps hold the
/// vessel: deployed, their feet would reach below the launch mount and the
/// ground (a reshaping there blew the stack apart, found by review
/// 2026-10-27).
pub fn interlock(def: &ShipDef, state: &ShipState, i: usize, command: &ShipCommand) -> Option<String> {
    let deploys = matches!(command, ShipCommand::Deploy(_) | ShipCommand::Angle(..));
    (state.clamped && def.mechanisms[i].item.foot.is_some() && deploys)
        .then(|| format!("{}: cannot deploy while the clamps hold the vessel", def.mechanisms[i].id))
}

#[cfg(test)]
#[path = "sequence_tests.rs"]
mod tests;
