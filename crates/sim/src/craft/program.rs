//! The vessel's program (D107, D111, D114): its stage sequence, its saved
//! functions and the sequence's position. The program is the vessel's as a
//! whole: every piece of a split keeps all of it, unfiltered
//! (`craft::partition`), so merging the pieces gives it back.
//!
//! What the program does in one piece is the piece's **view** of it: the
//! commands that name something the piece has ([`ShipDef::check`]; the
//! clamps and the parachutes name nothing, so every piece takes them). A
//! stage whose every command names what the piece lacks (another piece's
//! stage) is not in its view: Space skips it, AGAIN and the sequence's edits
//! refuse it, the estimates and the staging bar leave it out. A stage with
//! no command at all (one just added) is in every view. D114 holds in each
//! piece: its commands that can act do, the rest do nothing.
//!
//! **An abort** (a function marked `abort`, D120: "Launch abort should only
//! show when there is a valid abort procedure that would actually save the
//! rocket or the crew") is offered only while what the piece holds makes
//! its procedure an escape, whatever its name ([`offered`]):
//!
//! * a crew cabin (seats) in the piece;
//! * a joint it separates, intact, holding the cabin to a stack still in
//!   the piece;
//! * an engine it lights on the cabin's side of that joint that can start
//!   ([`can_start`]: not failed, an ignition left, its controller working
//!   with power, propellant in its feeds).
//!
//! So the Falcon 9 offers its launch abort on the pad and in ascent; the
//! booster after the stage separation (no cabin), Dragon after its own
//! (nothing to leave) and a capsule without escape engines (Cargo Dragon
//! has no SuperDracos) do not.
//!
//! Read by `systems::command` (Space, AGAIN, functions, the sequence's
//! edits), `systems::estimate`, `Vessel::set_stages` and the game's staging
//! bar, action bar and systems screen.

use super::command::{ShipCommand, Stage};
use super::ship::ShipDef;
use super::state::{EngineStatus, ShipState};
use crate::systems::{command, propulsion};

/// Whether the piece `def` takes command `c`: it names something the piece
/// has, or nothing at all (the clamps, the parachutes).
pub fn takes(def: &ShipDef, c: &ShipCommand) -> bool {
    def.check(c).is_ok()
}

/// Whether `stage` (or a function) is in the piece's view: it has a command
/// the piece takes, or none at all.
pub fn shows(def: &ShipDef, stage: &Stage) -> bool {
    stage.actions.is_empty() || stage.actions.iter().any(|c| takes(def, c))
}

/// An entry of a piece's view: its index in the program's list, and the
/// indices of its commands the piece takes, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shown {
    pub index: usize,
    pub actions: Vec<usize>,
}

/// The piece's view of a list (the sequence or the functions), in order.
pub fn view(def: &ShipDef, list: &[Stage]) -> Vec<Shown> {
    (list.iter().enumerate())
        .filter(|(_, s)| shows(def, s))
        .map(|(index, s)| Shown {
            index,
            actions: (0..s.actions.len()).filter(|&j| takes(def, &s.actions[j])).collect(),
        })
        .collect()
}

/// `stage` as the piece runs it: its commands the piece takes.
pub fn mine(def: &ShipDef, stage: &Stage) -> Stage {
    Stage { actions: stage.actions.iter().filter(|c| takes(def, c)).cloned().collect(), ..stage.clone() }
}

/// The stage Space runs next: the first in the view at or after
/// `sequence`; `None` when the piece has none left.
pub fn next(def: &ShipDef, stages: &[Stage], sequence: u32) -> Option<usize> {
    (sequence as usize..stages.len()).find(|&k| shows(def, &stages[k]))
}

/// The piece's active stage: the last in the view before `sequence`.
pub fn active(def: &ShipDef, stages: &[Stage], sequence: u32) -> Option<usize> {
    (0..(sequence as usize).min(stages.len())).rev().find(|&k| shows(def, &stages[k]))
}

/// The stages still to run in the piece's view, in order.
pub fn pending(def: &ShipDef, stages: &[Stage], sequence: u32) -> Vec<usize> {
    (sequence as usize..stages.len()).filter(|&k| shows(def, &stages[k])).collect()
}

/// Whether stage `k` is one the piece has run (in its view, before
/// `sequence`): AGAIN runs it.
pub fn has_run(def: &ShipDef, stages: &[Stage], sequence: u32, k: u32) -> bool {
    k < sequence && stages.get(k as usize).is_some_and(|s| shows(def, s))
}

/// The function `name` in the piece's view.
pub fn function<'a>(def: &ShipDef, functions: &'a [Stage], name: &str) -> Option<&'a Stage> {
    functions.iter().find(|f| f.name == name && shows(def, f))
}

/// Why an abort would save no one in this piece (the furthest its
/// procedure gets, in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NoEscape {
    /// No crew cabin aboard.
    NoCabin,
    /// Nothing it separates holds the cabin to a stack still attached.
    NothingToLeave,
    /// No engine it lights rides with the cabin.
    NoEngines,
    /// Its engines there cannot start (failed, no ignition, no controller
    /// or power, no propellant).
    CannotStart,
}

impl NoEscape {
    /// A few words for the player.
    pub fn describe(self) -> &'static str {
        match self {
            NoEscape::NoCabin => "no crew cabin aboard",
            NoEscape::NothingToLeave => "nothing to escape from: the cabin is not attached to a stack",
            NoEscape::NoEngines => "no escape engines ride with the cabin",
            NoEscape::CannotStart => "the escape engines cannot start (ignitions, propellant, power)",
        }
    }
}

/// Whether engine `i` can start now: not failed, an ignition left (or the
/// ground's), its controller reachable with power, propellant in its feeds.
pub fn can_start(def: &ShipDef, state: &ShipState, i: usize) -> bool {
    let (d, e) = (&def.engines[i], &state.engines[i]);
    e.status != EngineStatus::Failed
        && command::can_ignite(d.data.igniter, e.ignitions, state.clamped)
        && command::reachable(def, state, d.owner)
        && propulsion::fed(def, state, i)
}

/// Whether the piece (`def`, `state`) offers function `f`: any function
/// not marked as an abort; an abort only when its procedure is an escape
/// here (see the module), else why not.
pub fn offered(def: &ShipDef, state: &ShipState, f: &Stage) -> Result<(), NoEscape> {
    if !f.abort {
        return Ok(());
    }
    let cabin: Vec<u32> = def.seats.iter().filter(|s| s.item > 0).map(|s| s.part).collect();
    if cabin.is_empty() {
        return Err(NoEscape::NoCabin);
    }
    let lit: Vec<usize> = (f.actions.iter())
        .filter_map(|c| match c {
            ShipCommand::Ignite(s) => Some(def.engines_of(s)),
            _ => None,
        })
        .flatten()
        .collect();
    let mut why = NoEscape::NothingToLeave;
    for c in &f.actions {
        let ShipCommand::Separate(name) = c else { continue };
        let Some(j) = def.joints.iter().position(|j| j.name == *name) else { continue };
        if !state.joints[j].intact {
            continue;
        }
        let (parent, child) = (def.joints[j].parent, def.joints[j].child);
        let ours = [side(def, state, j, parent), side(def, state, j, child)]
            .into_iter()
            .find(|side| cabin.iter().any(|p| side.contains(p)));
        let Some(ours) = ours else { continue };
        let engines: Vec<usize> = lit.iter().copied().filter(|&i| ours.contains(&def.engines[i].part)).collect();
        if engines.iter().any(|&i| can_start(def, state, i)) {
            return Ok(());
        }
        why = why.max(if engines.is_empty() { NoEscape::NoEngines } else { NoEscape::CannotStart });
    }
    Err(why)
}

/// The parts joined to part `from` without crossing joint `cut` (by intact
/// joints of the piece).
fn side(def: &ShipDef, state: &ShipState, cut: usize, from: u32) -> Vec<u32> {
    let mut seen = vec![from];
    let mut k = 0;
    while k < seen.len() {
        let p = seen[k];
        for (i, j) in def.joints.iter().enumerate() {
            if i == cut || !state.joints[i].intact {
                continue;
            }
            let other = match (j.parent == p, j.child == p) {
                (true, _) => j.child,
                (_, true) => j.parent,
                _ => continue,
            };
            if !seen.contains(&other) {
                seen.push(other);
            }
        }
        k += 1;
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn falcon9() -> ShipDef {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        (*super::super::load_definition(&path).unwrap().ship.unwrap()).clone()
    }

    /// The Falcon 9's pieces after its stage separation and Dragon's: the
    /// booster, the second stage with Dragon, and Dragon alone.
    fn pieces(def: &ShipDef) -> [(&'static str, ShipDef); 4] {
        let of = |f: &dyn Fn(&str) -> bool| {
            def.subset(&def.parts.iter().filter(|p| f(&p.id)).map(|p| p.part).collect::<Vec<_>>())
        };
        [
            ("whole", def.clone()),
            ("booster", of(&|id| !id.starts_with("s2.") && !id.starts_with("dragon."))),
            ("upper", of(&|id| id.starts_with("s2.") || id.starts_with("dragon."))),
            ("dragon", of(&|id| id.starts_with("dragon."))),
        ]
    }

    /// D120: the launch abort is offered where it would save the crew, from
    /// what each piece holds: on the pad and in ascent, never on the
    /// booster after the stage separation, on Dragon after its own, on a
    /// capsule without escape engines, or with the escape engines unable
    /// to start.
    #[test]
    fn an_abort_is_offered_only_where_it_would_save_the_crew() {
        use NoEscape::*;
        let def = falcon9();
        let abort = def.functions.iter().find(|f| f.abort).expect("the Falcon 9's abort").clone();
        let [whole, booster, upper, dragon] = pieces(&def).map(|(_, d)| d);
        let flying = def.subset(&def.parts.iter().filter(|p| !p.ground).map(|p| p.part).collect::<Vec<_>>());
        // Cargo Dragon: no SuperDracos.
        let mut cargo = def.clone();
        cargo.engines.retain(|e| !def.owner_id(e.owner).starts_with("dragon.superdraco"));
        let superdracos = |d: &ShipDef| -> Vec<usize> {
            (0..d.engines.len()).filter(|&i| d.owner_id(d.engines[i].owner).starts_with("dragon.superdraco")).collect()
        };
        let failed = |d: &ShipDef, s: &mut ShipState| {
            superdracos(d).into_iter().for_each(|i| s.engines[i].status = EngineStatus::Failed)
        };
        let dry = |d: &ShipDef, s: &mut ShipState| {
            let capsule = d.part_named("dragon.capsule").unwrap();
            (s.tanks.iter_mut()).filter(|t| t.part == capsule).for_each(|t| t.liquid = 0.0);
        };
        let unpowered = |d: &ShipDef, s: &mut ShipState| {
            for i in superdracos(d) {
                let (part, owner) = (d.engines[i].part, d.engines[i].owner);
                s.unpowered.push(crate::craft::state::Lost { part, owner, why: "bus dead".into() });
            }
        };
        let separated = |d: &ShipDef, s: &mut ShipState| {
            let j = d.joints.iter().position(|j| j.name == "dragon separation").unwrap();
            s.joints[j].intact = false;
        };
        let asis = |_: &ShipDef, _: &mut ShipState| {};
        let no_separation = Stage {
            actions: abort.actions.iter().filter(|c| !matches!(c, ShipCommand::Separate(_))).cloned().collect(),
            ..abort.clone()
        };
        let lights_the_booster = Stage {
            actions: [ShipCommand::Separate("dragon separation".into()), ShipCommand::Ignite("s1".into())].into(),
            ..abort.clone()
        };
        let plain = Stage { abort: false, ..abort.clone() };
        type Change<'a> = &'a dyn Fn(&ShipDef, &mut ShipState);
        // (case, piece, its state, the function, offered)
        type Row<'a> = (&'a str, &'a ShipDef, Change<'a>, &'a Stage, Result<(), NoEscape>);
        let table: [Row; 13] = [
            ("the stack on the pad", &whole, &asis, &abort, Ok(())),
            ("the stack in ascent", &flying, &asis, &abort, Ok(())),
            ("the upper stack after the stage separation", &upper, &asis, &abort, Ok(())),
            ("the booster after the stage separation", &booster, &asis, &abort, Err(NoCabin)),
            ("Dragon after its separation", &dragon, &asis, &abort, Err(NothingToLeave)),
            ("Cargo Dragon (no SuperDracos)", &cargo, &asis, &abort, Err(NoEngines)),
            ("the SuperDracos failed", &flying, &failed, &abort, Err(CannotStart)),
            ("Dragon's tanks dry", &upper, &dry, &abort, Err(CannotStart)),
            ("the SuperDracos without power", &whole, &unpowered, &abort, Err(CannotStart)),
            ("Dragon's separation fired, the split not yet made", &upper, &separated, &abort, Err(NothingToLeave)),
            ("a procedure that separates nothing", &whole, &asis, &no_separation, Err(NothingToLeave)),
            ("a procedure lighting what stays behind", &whole, &asis, &lights_the_booster, Err(NoEngines)),
            ("a function not marked as an abort", &booster, &asis, &plain, Ok(())),
        ];
        for (case, piece, change, f, want) in table {
            let mut state = ShipState::initial(piece);
            change(piece, &mut state);
            assert_eq!(offered(piece, &state, f), want, "{case}");
        }
    }

    /// Each piece's view of the Falcon 9's sequence: the stages with a
    /// command it takes and those commands' indices.
    #[test]
    fn each_piece_sees_the_stages_that_act_on_it() {
        let def = falcon9();
        let stages = def.sequence.clone();
        // (piece, [(stage index, its commands shown)])
        type Row<'a> = (&'a str, &'a [(usize, &'a [usize])]);
        let table: [Row; 4] = [
            ("whole", &[(0, &[0]), (1, &[0]), (2, &[0]), (3, &[0, 1]), (4, &[0, 1]), (5, &[0]), (6, &[0, 1, 2])]),
            // The cut joints' separations name nothing left in either piece.
            ("booster", &[(0, &[0]), (1, &[0]), (2, &[0]), (3, &[1])]),
            ("upper", &[(1, &[0]), (4, &[0, 1]), (5, &[0]), (6, &[0, 1, 2])]),
            ("dragon", &[(1, &[0]), (6, &[1, 2])]),
        ];
        for ((name, piece), (want_name, want)) in pieces(&def).iter().zip(table) {
            assert_eq!(*name, want_name);
            let got: Vec<(usize, Vec<usize>)> =
                view(piece, &stages).into_iter().map(|s| (s.index, s.actions)).collect();
            let want: Vec<(usize, Vec<usize>)> = want.iter().map(|(k, a)| (*k, a.to_vec())).collect();
            assert_eq!(got, want, "{name}");
        }
        // The booster runs its own share of the stage separation.
        let booster = &pieces(&def)[1].1;
        assert_eq!(mine(booster, &stages[3]).actions, [ShipCommand::Rcs("s1.coldgas#*".into(), true)]);
        // A stage without commands is everyone's.
        let empty = Stage { name: "New".into(), key: None, actions: vec![], pinned: false, abort: false };
        assert!(pieces(&def).iter().all(|(_, d)| shows(d, &empty)));
    }

    /// Space's next stage, the active one, those still to run and those
    /// AGAIN runs, in the booster's view (stages 4 to 6 are the upper
    /// stack's; a stage of the booster's own is added after them).
    #[test]
    fn the_next_and_active_stages_skip_the_other_pieces() {
        let def = falcon9();
        let booster = &pieces(&def)[1].1;
        let mut stages = def.sequence.clone();
        let flip = ShipCommand::Deploy("s1.gridfin#*".into());
        stages.push(Stage { name: "Grid fins".into(), key: None, actions: vec![flip], pinned: false, abort: false });
        // (sequence, next, active, pending)
        type Row<'a> = (u32, Option<usize>, Option<usize>, &'a [usize]);
        let table: [Row; 6] = [
            (0, Some(0), None, &[0, 1, 2, 3, 7]),
            (1, Some(1), Some(0), &[1, 2, 3, 7]),
            (4, Some(7), Some(3), &[7]),
            (6, Some(7), Some(3), &[7]),
            (8, None, Some(7), &[]),
            (99, None, Some(7), &[]),
        ];
        for (sequence, n, a, p) in table {
            assert_eq!(next(booster, &stages, sequence), n, "{sequence}");
            assert_eq!(active(booster, &stages, sequence), a, "{sequence}");
            assert_eq!(pending(booster, &stages, sequence), p, "{sequence}");
        }
        // (sequence, stage, run in the booster's view)
        let runs =
            [(4, 3, true), (4, 4, false), (6, 4, false), (6, 5, false), (8, 7, true), (4, 7, false), (9, 9, false)];
        for (sequence, k, want) in runs {
            assert_eq!(has_run(booster, &stages, sequence, k), want, "{sequence} {k}");
        }
        // Functions: the booster's own and the abort (its Merlins' shutdown),
        // which it does not offer ([`offered`]: no cabin).
        let named = |n: &str| function(booster, &def.functions, n).is_some();
        assert!(named("Landing legs") && named("Launch abort") && named("Parachutes"));
        assert!(!named("Nosecone") && !named("No such"));
    }
}
