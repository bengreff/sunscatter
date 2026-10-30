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
//! Read by `systems::command` (Space, AGAIN, functions, the sequence's
//! edits), `systems::estimate`, `Vessel::set_stages` and the game's staging
//! bar, action bar and systems screen.

use super::command::{ShipCommand, Stage};
use super::ship::ShipDef;

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
        let empty = Stage { name: "New".into(), key: None, actions: vec![], pinned: false };
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
        stages.push(Stage { name: "Grid fins".into(), key: None, actions: vec![flip], pinned: false });
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
        // Functions: the booster's own and the abort (its Merlins' shutdown).
        let named = |n: &str| function(booster, &def.functions, n).is_some();
        assert!(named("Landing legs") && named("Launch abort") && named("Parachutes"));
        assert!(!named("Nosecone") && !named("No such"));
    }
}
