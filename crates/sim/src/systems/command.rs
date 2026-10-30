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
use crate::craft::command::{matches, SequenceEdit, ShipCommand, Stage};
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

/// Whether an engine (or other actuator) `owner` can be commanded: its own
/// function in (not lost, with power), and if computers are linked to it,
/// one of them working.
pub fn reachable(def: &ShipDef, state: &ShipState, owner: &str) -> bool {
    let mut links = def.links.iter().filter(|l| l.to.iter().any(|(id, _)| id == owner)).peekable();
    let linked = links.peek().is_some();
    !state.runtime.out(owner) && (!linked || links.any(|l| !state.runtime.out(&l.from.0)))
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
                let d = &def.engines[i];
                if state.engines[i].status != EngineStatus::Off {
                    continue;
                }
                if !reachable(def, state, &d.owner) {
                    state.runtime.event(cx.at, format!("{}: no working controller or computer: no start", d.id), true);
                    continue;
                }
                let e = &mut state.engines[i];
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
                    // From the sequence, the next stage order starts it
                    // ([`next_stage`]).
                    let again = if cx.sequence { "stage" } else { "ignite" };
                    let text = format!("{}: {how} its propellant before the start ({again} again to start now)", d.id);
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
                super::propulsion::shut_down(state, i, "commanded");
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
                // Legs stay stowed while the pad's clamps hold the vessel:
                // deployed, their feet would reach below the launch mount
                // and the ground (the real pad's interlock; a reshaping
                // there blew the stack apart, found by review 2026-10-27).
                if state.clamped
                    && m.foot.is_some()
                    && matches!(command, ShipCommand::Deploy(_) | ShipCommand::Angle(..))
                {
                    let text = format!("{}: cannot deploy while the clamps hold the vessel", def.mechanisms[i].id);
                    state.runtime.event(cx.at, text, true);
                    continue;
                }
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
        // Released already (a stage run again, D114): nothing to do.
        ShipCommand::ReleaseClamps if !state.clamped => Ok(Vec::new()),
        ShipCommand::ReleaseClamps if cx.sequence => {
            state.runtime.release_armed = true;
            state.runtime.release_engines = super::propulsion::lit(state);
            state.runtime.event(cx.at, "Clamps armed: release at full thrust", false);
            Ok(Vec::new())
        }
        _ => state.apply(def, command),
    }
}

/// Edits the stage sequence (D111: the staging bar in flight): moves a
/// stage, moves one of a stage's commands to the end of another, or deletes
/// a stage. Only stages still to run (from `sequence` on) change; anything
/// else is refused and nothing changes. A stage left without commands
/// stays (it can be deleted).
pub fn edit_sequence(stages: &mut Vec<Stage>, sequence: u32, edit: &SequenceEdit) -> Result<(), String> {
    let pending = |k: u32| {
        if (k as usize) < stages.len() && k >= sequence {
            Ok(k as usize)
        } else {
            Err(format!("{edit:?}: stage {k} is not one still to run ({sequence}..{})", stages.len()))
        }
    };
    match *edit {
        SequenceEdit::Move { from, to } => {
            let (from, to) = (pending(from)?, pending(to)?);
            let stage = stages.remove(from);
            stages.insert(to, stage);
        }
        SequenceEdit::MoveAction { from, action, to } => {
            let (from, to) = (pending(from)?, pending(to)?);
            if action as usize >= stages[from].actions.len() || from == to {
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
    let waits = |i: usize| state.runtime.engines.get(i).is_some_and(|r| r.settling.is_some());
    def.engines_of(s).into_iter().any(|i| waits(i) && state.engines[i].status == EngineStatus::Off)
}

/// Runs the next stage of the ship's own sequence: its commands (one naming
/// what the craft lacks is skipped); `None` when the sequence is done.
/// While an ignition of the last stage waits for its propellant to settle,
/// the order starts it now instead (its hint says so) and the sequence
/// stays: running on, the next stage's shutdown would cancel the start
/// (review H4: Space while the second stage settled ran SECO).
pub fn next_stage(def: &ShipDef, state: &mut ShipState, at: Epoch) -> Option<(String, Vec<Effect>)> {
    let last = state.sequence.checked_sub(1).and_then(|k| state.stages.get(k as usize));
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
    let stage = state.stages.get(state.sequence as usize)?.clone();
    state.sequence += 1;
    let cx = Context { at, sequence: true };
    Some((stage.name.clone(), run(def, state, &stage, cx)))
}

/// Runs stage `k` of the sequence again (D114), one already run: each of
/// its commands that can act does (an engine with an ignition left starts),
/// the rest do nothing (a joint already parted, clamps let go, a mechanism
/// where it is); the sequence stays. `None` for a stage not yet run.
pub fn again(def: &ShipDef, state: &mut ShipState, k: u32, at: Epoch) -> Option<(String, Vec<Effect>)> {
    if k >= state.sequence {
        return None;
    }
    let stage = state.stages.get(k as usize)?.clone();
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
        OrderKind::Again(k) => match again(def, state, *k, at) {
            Some((name, e)) => {
                state.runtime.event(at, format!("Stage again: {name}"), false);
                e
            }
            None => {
                state.runtime.event(at, format!("Stage {} has not run yet", k + 1), true);
                Vec::new()
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::runtime::Lost;

    /// A release undone puts the sequence back to the last stage run that
    /// releases the clamps; without one, it stays.
    #[test]
    fn a_release_undone_is_next_again() {
        let stage = |actions: Vec<ShipCommand>| Stage { name: String::new(), key: None, actions, pinned: false };
        let ignite = || ShipCommand::Ignite("s1".into());
        let stages = vec![
            stage(vec![ignite()]),
            stage(vec![ShipCommand::ReleaseClamps]),
            stage(vec![ShipCommand::Shutdown("s1".into())]),
            stage(vec![ignite(), ShipCommand::ReleaseClamps]),
            stage(vec![]),
        ];
        // (sequence, after)
        let cases = [(0, 0), (1, 1), (2, 1), (3, 1), (4, 3), (5, 3), (9, 3)];
        for (sequence, want) in cases {
            assert_eq!(release_undone(&stages, sequence), want, "{sequence}");
        }
        assert_eq!(release_undone(&stages[..1], 1), 1, "no release run");
    }

    /// Without power to the second stage's flight computer the MVac cannot
    /// be started (its start goes through it); with it, it starts.
    #[test]
    fn an_engine_needs_a_working_computer() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mvac = def.engines.iter().position(|e| e.id == "s2.mvac").unwrap();
        let at = Epoch::new(1_000_000_000, 0.0);
        let cx = Context { at, sequence: false };
        let mut state = ShipState::initial(&def);
        state.clamped = false;
        let part = def.computers.iter().find(|c| c.id == "s2.computer").unwrap().part;
        state.runtime.unpowered.push(Lost { part, owner: "s2.computer".into(), why: "bus dead".into() });
        apply(&def, &mut state, &ShipCommand::Ignite("s2".into()), cx).unwrap();
        assert_eq!(state.engines[mvac].status, EngineStatus::Off);
        assert!(state.runtime.events.last().unwrap().text.contains("no working controller"));
        state.runtime.unpowered.clear();
        apply(&def, &mut state, &ShipCommand::Ignite("s2".into()), cx).unwrap();
        assert!(
            matches!(state.engines[mvac].status, EngineStatus::Starting)
                || state.runtime.engines[mvac].settling.is_some()
        );
    }

    /// A stage run again (D114) starts the engines that are off and can
    /// start, does nothing to those lit, and leaves the sequence; a stage
    /// not yet run is refused.
    #[test]
    fn a_stage_runs_again() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let at = Epoch::new(1_000_000_000, 0.0);
        let mut state = ShipState::initial(&def);
        next_stage(&def, &mut state, at).unwrap();
        let lit: Vec<usize> =
            (0..def.engines.len()).filter(|&i| state.engines[i].status != EngineStatus::Off).collect();
        assert!(!lit.is_empty(), "the first stage lights engines");
        let sequence = state.sequence;
        assert!(again(&def, &mut state, sequence, at).is_none(), "not run yet");
        let before = state.engines.clone();
        again(&def, &mut state, 0, at).unwrap();
        assert_eq!(state.engines, before, "lit engines: nothing");
        for &i in &lit {
            state.engines[i].status = EngineStatus::Off;
        }
        let (name, _) = again(&def, &mut state, 0, at).unwrap();
        assert_eq!(name, state.stages[0].name);
        assert!(lit.iter().all(|&i| state.engines[i].status == EngineStatus::Starting));
        assert_eq!(state.sequence, sequence);
        // Off the pad, only the engines with a charge left relight (the
        // Falcon 9's E1, E5 and E9); the rest do nothing.
        state.clamped = false;
        for &i in &lit {
            state.engines[i].status = EngineStatus::Off;
        }
        again(&def, &mut state, 0, at).unwrap();
        for &i in &lit {
            let want = if state.engines[i].ignitions > 0 { EngineStatus::Starting } else { EngineStatus::Off };
            assert_eq!(state.engines[i].status, want, "{}", def.engines[i].id);
        }
        assert_eq!(lit.iter().filter(|&&i| state.engines[i].status == EngineStatus::Starting).count(), 3);
    }

    /// The sequence is edited in flight (D111) through the ship's commands:
    /// stages still to run move, swap commands and go; the stages run and
    /// out-of-range indices are refused whole.
    #[test]
    fn the_sequence_is_edited_where_it_has_not_run() {
        use SequenceEdit::{Delete, Move, MoveAction};
        let stage = |name: &str, n: usize| Stage {
            name: name.into(),
            key: None,
            actions: (0..n).map(|k| ShipCommand::Ignite(format!("{name}{k}"))).collect(),
            pinned: false,
        };
        let names = |s: &[Stage]| s.iter().map(|s| format!("{}{}", s.name, s.actions.len())).collect::<Vec<_>>();
        // (edit, sequence, stages after, or refused)
        let cases: [(SequenceEdit, u32, Option<[&str; 4]>); 9] = [
            (Move { from: 3, to: 1 }, 1, Some(["a1", "d1", "b2", "c0"])),
            (Move { from: 1, to: 3 }, 1, Some(["a1", "c0", "d1", "b2"])),
            (Move { from: 0, to: 2 }, 1, None),
            (Move { from: 1, to: 4 }, 0, None),
            (MoveAction { from: 1, action: 1, to: 3 }, 1, Some(["a1", "b1", "c0", "d2"])),
            (MoveAction { from: 1, action: 2, to: 3 }, 1, None),
            (MoveAction { from: 1, action: 0, to: 1 }, 0, None),
            (MoveAction { from: 0, action: 0, to: 2 }, 1, None),
            (Delete(2), 2, Some(["a1", "b2", "d1", ""])),
        ];
        for (edit, sequence, want) in cases {
            let mut stages = vec![stage("a", 1), stage("b", 2), stage("c", 0), stage("d", 1)];
            let before = stages.clone();
            let got = edit_sequence(&mut stages, sequence, &edit);
            match want {
                Some(w) => {
                    got.unwrap();
                    let w: Vec<String> = w.iter().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect();
                    assert_eq!(names(&stages), w, "{edit:?}");
                }
                None => {
                    assert!(got.is_err(), "{edit:?}");
                    assert_eq!(stages, before, "refused whole");
                }
            }
        }
        // As a ship command: the moved command keeps its place in order.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mut state = ShipState::initial(&def);
        let cx = Context { at: Epoch::new(1_000_000_000, 0.0), sequence: false };
        let n = state.stages.len();
        let edit = ShipCommand::Sequence(MoveAction { from: 3, action: 1, to: 6 });
        apply(&def, &mut state, &edit, cx).unwrap();
        assert_eq!(state.stages[6].actions.last(), Some(&ShipCommand::Rcs("s1.coldgas#*".into(), true)));
        apply(&def, &mut state, &ShipCommand::Sequence(Delete(5)), cx).unwrap();
        assert_eq!(state.stages.len(), n - 1);
        assert!(def.check(&edit).is_err(), "never a stage's own command");
    }

    /// The sequence's second-stage ignition on floating propellant waits
    /// for it to settle; the next stage order starts it now (as its hint
    /// says) instead of running SECO, which would cancel the start; then
    /// the sequence runs on (review H4).
    #[test]
    fn staging_again_while_settling_starts_the_engine() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mvac = def.engines.iter().position(|e| e.id == "s2.mvac").unwrap();
        let at = Epoch::new(1_000_000_000, 0.0);
        let mut state = ShipState::initial(&def);
        state.clamped = false;
        let ignition = (state.stages.iter())
            .position(|st| st.actions.iter().any(|a| matches!(a, ShipCommand::Ignite(s) if s == "s2")))
            .expect("the second stage's ignition");
        state.sequence = ignition as u32;
        // Floating: the liquids drifted off their outlets in free fall.
        state.runtime.refit(&def);
        for r in state.runtime.tanks.iter_mut() {
            r.drift = 100.0;
        }
        let (name, _) = next_stage(&def, &mut state, at).unwrap();
        println!("{name}: {:?}", state.runtime.events.last());
        assert_eq!(state.engines[mvac].status, EngineStatus::Off);
        assert!(state.runtime.engines[mvac].settling.is_some());
        assert!(state.runtime.events.iter().any(|e| e.text.contains("stage again to start now")));
        let (name, _) = next_stage(&def, &mut state, at).unwrap();
        assert!(name.ends_with("start now"), "{name}");
        assert_eq!(state.engines[mvac].status, EngineStatus::Starting);
        assert_eq!(state.sequence as usize, ignition + 1, "the sequence waits");
        let (name, _) = next_stage(&def, &mut state, at).unwrap();
        assert!(!name.ends_with("start now"), "{name}");
        assert_eq!(state.sequence as usize, ignition + 2);
    }

    /// The Falcon 9's "Launch abort" terminates the stack's thrust (the
    /// AFSS and stage 2's pyrovalve, falcon9.md §5.1) as Dragon separates
    /// and lights its SuperDracos: on the pad with the Merlins at full
    /// thrust, in flight on the booster, and in flight on the second stage.
    #[test]
    fn a_launch_abort_shuts_the_stack_down() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let at = Epoch::new(1_000_000_000, 0.0);
        let stack = |i: usize| def.engines[i].id.starts_with("s1.") || def.engines[i].id.starts_with("s2.");
        let lit = |s: EngineStatus| matches!(s, EngineStatus::Starting | EngineStatus::Running);
        // (case, clamped, the engines running before)
        let cases: [(&str, bool, &str); 3] = [
            ("on the pad", true, "s1.merlin"),
            ("booster burning", false, "s1.merlin"),
            ("second stage", false, "s2.mvac"),
        ];
        for (case, clamped, running) in cases {
            let mut state = ShipState::initial(&def);
            state.clamped = clamped;
            state.runtime.refit(&def);
            for (i, d) in def.engines.iter().enumerate() {
                if d.id.starts_with(running) {
                    state.engines[i].status = EngineStatus::Running;
                    state.runtime.engines[i].level = 1.0;
                }
            }
            let effects = function(&def, &mut state, "Launch abort", at).expect("the function");
            assert!(!effects.is_empty(), "{case}: Dragon separates");
            for (i, d) in def.engines.iter().enumerate() {
                let s = state.engines[i].status;
                if stack(i) {
                    assert!(!lit(s), "{case}: {} is {s:?}", d.id);
                } else if d.id.starts_with("dragon.superdraco") {
                    assert!(lit(s), "{case}: {} is {s:?}", d.id);
                }
            }
        }
    }
}
