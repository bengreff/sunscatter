//! The stage sequence and the functions as the ship computer runs them
//! (moved from `systems::command`'s tests with the code).

use super::*;
/// A release undone puts the sequence back to the last stage run that
/// releases the clamps; without one, it stays.
#[test]
fn a_release_undone_is_next_again() {
    let stage =
        |actions: Vec<ShipCommand>| Stage { name: String::new(), key: None, actions, pinned: false, abort: false };
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
    let lit: Vec<usize> = (0..def.engines.len()).filter(|&i| state.engines[i].status != EngineStatus::Off).collect();
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

/// After the stage separation each piece keeps the whole program and
/// runs its own stages (`craft::program`): Space on the booster skips
/// the upper stack's to its own next one, on the upper stack it runs
/// the second stage's ignition; AGAIN refuses another piece's stage.
#[test]
fn each_piece_runs_its_own_stages_after_a_split() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
    let at = Epoch::new(1_000_000_000, 0.0);
    let mut state = ShipState::initial(&def);
    let fins = Stage { name: "Grid fins".into(), key: None, actions: vec![], pinned: false, abort: false };
    state.stages.push(Stage { actions: vec![ShipCommand::Deploy("s1.gridfin#*".into())], ..fins });
    state.clamped = false;
    for _ in 0..4 {
        next_stage(&def, &mut state, at).unwrap();
    }
    let ids = |f: &dyn Fn(&str) -> bool| def.parts.iter().filter(|p| f(&p.id)).map(|p| p.part).collect::<Vec<_>>();
    let upper = |id: &str| id.starts_with("s2.") || id.starts_with("dragon.");
    let (a, b) = (ids(&upper), ids(&|id| !upper(id)));
    let (da, db) = (def.subset(&a), def.subset(&b));
    let (mut pa, mut pb) = (state.partition(&a, &def, &da, true), state.partition(&b, &def, &db, false));
    assert_eq!(next_stage(&db, &mut pb, at).unwrap().0, "Grid fins");
    assert_eq!(pb.sequence, 8);
    let fin = db.mechanisms.iter().position(|m| m.id.starts_with("s1.gridfin")).unwrap();
    assert_eq!(pb.mechanisms[fin].target, db.mechanisms[fin].item.deployed_deg);
    assert!(next_stage(&db, &mut pb, at).is_none(), "none left");
    assert!(again(&db, &mut pb, 4, at).is_none(), "the upper stack's");
    assert_eq!(again(&db, &mut pb, 3, at).unwrap().0, "Stage separation");
    assert_eq!(next_stage(&da, &mut pa, at).unwrap().0, "Second stage ignition");
    assert_eq!(pa.sequence, 5);
}

/// The sequence is edited in flight (D111) through the ship's commands:
/// stages still to run move, swap commands and go; the stages run,
/// out-of-range indices and what is not in the craft's view are refused
/// whole.
#[test]
fn the_sequence_is_edited_where_it_has_not_run() {
    use SequenceEdit::{Delete, Move, MoveAction};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
    // Commands every craft takes (they name nothing).
    let stage = |name: &str, n: usize| Stage {
        name: name.into(),
        key: None,
        actions: (0..n).map(|k| if k % 2 == 0 { ShipCommand::ArmChutes } else { ShipCommand::ReleaseClamps }).collect(),
        pinned: false,
        abort: false,
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
        let got = edit_sequence(&def, &mut stages, sequence, &edit);
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
    // In the booster's view after the stage separation, the upper
    // stack's stages and commands stay as they are.
    let parts: Vec<u32> = (def.parts.iter())
        .filter(|p| !p.id.starts_with("s2.") && !p.id.starts_with("dragon."))
        .map(|p| p.part)
        .collect();
    let booster = def.subset(&parts);
    let mut stages = def.sequence.clone();
    let refused =
        [Move { from: 4, to: 3 }, Move { from: 2, to: 5 }, Delete(6), MoveAction { from: 3, action: 0, to: 2 }];
    for edit in refused {
        assert!(edit_sequence(&booster, &mut stages, 2, &edit).is_err(), "{edit:?}");
    }
    edit_sequence(&booster, &mut stages, 2, &MoveAction { from: 3, action: 1, to: 2 }).unwrap();
    assert_eq!(stages[2].actions, [ShipCommand::Shutdown("s1".into()), ShipCommand::Rcs("s1.coldgas#*".into(), true)]);
    assert_eq!(stages[3].actions, [ShipCommand::Separate("stage separation".into())], "the cut joint's stays");
    // As a ship command: the moved command keeps its place in order.
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
    for t in state.tanks.iter_mut() {
        t.drift = 100.0;
    }
    let (name, _) = next_stage(&def, &mut state, at).unwrap();
    println!("{name}: {:?}", state.events.last());
    assert_eq!(state.engines[mvac].status, EngineStatus::Off);
    assert!(state.engines[mvac].settling.is_some());
    assert!(state.events.iter().any(|e| e.text.contains("stage again to start now")));
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
        for (i, d) in def.engines.iter().enumerate() {
            if d.id.starts_with(running) {
                state.engines[i].status = EngineStatus::Running;
                state.engines[i].level = 1.0;
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
