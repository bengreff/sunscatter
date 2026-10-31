//! The bar's rows on the Falcon 9: the sequence on the pad, the active
//! stage once the first stage burns, dashes for what is not measured, the
//! amounts' text, what a drop edits.

use super::*;
use sim::craft::command::ShipCommand;
use sim::systems::estimate::{estimate, hull, Hull, Inputs};
use sim::systems::{Knowledge, Quantity};

fn falcon9() -> (ShipDef, ShipState, Vec<Hull>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let craft = sim::craft::load_definition(&path).unwrap();
    let def = (**craft.ship.as_ref().unwrap()).clone();
    let state = ShipState::initial(&def);
    (def, state, hull(&craft.parts))
}

fn bar(def: &ShipDef, state: &ShipState, hull: &[Hull], k: &Knowledge) -> Vec<Row> {
    let inputs = Inputs { hull, knowledge: k, throttle: 1.0, p: 0.0, g: Some(9.81) };
    rows(def, state, &estimate(def, state, &inputs), (&[], Look::Idle))
}

#[test]
fn on_the_pad_the_next_stage_is_at_the_bottom() {
    let (def, state, hull) = falcon9();
    let rows = bar(&def, &state, &hull, &Knowledge::perfect());
    let mut names: Vec<&str> = state.stages.iter().map(|s| s.name.as_str()).collect();
    names.reverse();
    assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), names);
    let places: Vec<Place> = rows.iter().map(|r| r.place).collect();
    assert_eq!(places.last(), Some(&Place::Next));
    assert!(places[..places.len() - 1].iter().all(|p| *p == Place::Later));
    let ignition = rows.last().unwrap();
    assert_eq!((ignition.number, ignition.stage), (Some(6), Some(0)));
    assert_eq!(ignition.items[0].icons.len(), 9, "nine Merlins");
    let f = ignition.figures.as_ref().expect("the first stage's burn");
    let fluids: Vec<(&str, Option<f32>)> = f.bars.iter().map(|b| (b.fluid.as_str(), b.fill)).collect();
    assert_eq!(fluids, [("RP-1", Some(1.0)), ("LOX", Some(1.0))], "in the tanks' order");
    assert_eq!(f.burnout, "2m 31s");
    assert!(f.dv.ends_with("m/s") && f.active.is_none(), "{f:?}");
    let with_burns: Vec<&str> = rows.iter().filter(|r| r.figures.is_some()).map(|r| r.name.as_str()).collect();
    assert_eq!(with_burns, ["Second stage ignition", "Ignition"]);
}

#[test]
fn the_active_stage_shows_the_burn_now() {
    let (def, mut state, hull) = falcon9();
    state.next_stage(&def).unwrap();
    for r in state.runtime.engines.iter_mut() {
        r.level = 1.0;
    }
    let rows = bar(&def, &state, &hull, &Knowledge::perfect());
    let active = rows.last().unwrap();
    assert_eq!((active.name.as_str(), active.place), ("Ignition", Place::Active));
    assert_eq!(rows[rows.len() - 2].place, Place::Next);
    let a = active.figures.as_ref().unwrap().active.as_ref().expect("the active stage's figures");
    assert!(a.thrust.ends_with("kN") && a.mass.contains(" t → ") && a.isp.ends_with(" s"), "{a:?}");
    let twr: f64 = a.twr.parse().unwrap();
    assert!(twr > 1.0, "{twr}");
}

/// The stages run before the active one follow it, the latest first,
/// as names only: each can be run again (D114; the owner could not
/// relight the Merlins once Liftoff was the active stage).
#[test]
fn the_stages_run_before_follow_the_active_one() {
    let (def, mut state, hull) = falcon9();
    // (stages run, the rows after the next one: name and place)
    let table: [(u32, &[(&str, Place)]); 3] = [
        (1, &[("Ignition", Place::Active)]),
        (2, &[("Liftoff", Place::Active), ("Ignition", Place::Done)]),
        (3, &[("MECO", Place::Active), ("Liftoff", Place::Done), ("Ignition", Place::Done)]),
    ];
    for (run, want) in table {
        state.sequence = run;
        let rows = bar(&def, &state, &hull, &Knowledge::perfect());
        let next = rows.iter().position(|r| r.place == Place::Next).unwrap();
        let got: Vec<(&str, Place)> = rows[next + 1..].iter().map(|r| (r.name.as_str(), r.place)).collect();
        assert_eq!(got, want, "{run} run");
        let done = rows.iter().filter(|r| r.place == Place::Done);
        assert!(done.clone().all(|r| r.items.is_empty() && r.figures.is_none() && r.stage.is_some()));
        assert_eq!(done.count() as u32, run - 1);
    }
}

/// After the stage separation each piece's bar shows its own view of the
/// program (`sim::craft::program`), numbered in it, by the stages' indices
/// in the program: the booster its stages run (the separation active, its
/// cold-gas thrusters' command only), the upper stack its own still to run.
#[test]
fn after_the_stage_separation_each_piece_shows_its_own_stages() {
    let (def, mut state, hull) = falcon9();
    state.clamped = false;
    for _ in 0..4 {
        state.next_stage(&def).unwrap();
    }
    let ids = |f: &dyn Fn(&str) -> bool| def.parts.iter().filter(|p| f(&p.id)).map(|p| p.part).collect::<Vec<_>>();
    let upper = |id: &str| id.starts_with("s2.") || id.starts_with("dragon.");
    // (piece, its parts, the rows: number, program index, name, place, commands)
    type Want<'a> = &'a [(u32, usize, &'a str, Place, usize)];
    let table: [(&str, Vec<u32>, Want); 2] = [
        (
            "booster",
            ids(&|id| !upper(id)),
            &[
                (0, 3, "Stage separation", Place::Active, 1),
                (1, 2, "MECO", Place::Done, 0),
                (2, 1, "Liftoff", Place::Done, 0),
                (3, 0, "Ignition", Place::Done, 0),
            ],
        ),
        (
            "upper",
            ids(&upper),
            &[
                (0, 6, "Dragon separation", Place::Later, 3),
                (1, 5, "SECO", Place::Later, 1),
                (2, 4, "Second stage ignition", Place::Next, 2),
                (3, 1, "Liftoff", Place::Active, 1),
            ],
        ),
    ];
    for (name, parts, want) in table {
        let piece = def.subset(&parts);
        let s = state.partition(&parts, &def, &piece, name == "upper");
        let hull: Vec<Hull> = hull.iter().filter(|h| parts.contains(&h.part)).copied().collect();
        let rows = bar(&piece, &s, &hull, &Knowledge::perfect());
        let got: Vec<_> = (rows.iter())
            .map(|r| (r.number.unwrap(), r.stage.unwrap(), r.name.as_str(), r.place, r.items.len()))
            .collect();
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn what_is_not_measured_is_a_dash() {
    let (def, state, hull) = falcon9();
    let lox = def.tanks.iter().position(|t| t.id == "s1.tank:lox").unwrap() as u32;
    let mut k = Knowledge::default();
    for t in (0..def.tanks.len() as u32).filter(|&t| t != lox) {
        k.learn(Quantity::TankLevel(t), 100.0);
    }
    let rows = bar(&def, &state, &hull, &k);
    let f = rows.last().unwrap().figures.as_ref().unwrap();
    let (lox, rp1) =
        (f.bars.iter().find(|b| b.fluid == "LOX").unwrap(), f.bars.iter().find(|b| b.fluid == "RP-1").unwrap());
    assert_eq!(lox.fill, None);
    assert!(lox.text.starts_with("—/"), "{}", lox.text);
    assert!(rp1.fill.is_some());
    assert_eq!((f.burnout.as_str(), f.dv.as_str()), ("—", "—"));
}

/// The test craft (no part system): its engine and parachutes, the
/// active engine stage at the bottom, nothing to edit or estimate.
#[test]
fn the_test_craft_shows_its_engine_and_parachutes() {
    use crate::staging::{stack, StageInputs};
    use sim::chute::ChuteState;
    use sim::vessel::Controls;
    let controls = Controls { throttle: 1.0, ..Controls::default() };
    let spec = sim::craft::small::rocket_ref().spec.chute.clone();
    let chute = ChuteState::default();
    let inputs = StageInputs { controls: &controls, chute: &chute, spec: &spec, engine_running: true, fuelled: true };
    let rows = plain(&stack(&inputs));
    let summary: Vec<(Option<u32>, Place, usize)> = rows.iter().map(|r| (r.number, r.place, r.items.len())).collect();
    assert_eq!(summary, [(Some(0), Place::Next, 2), (Some(1), Place::Active, 1)]);
    let engine = &rows[1].items[0].icons[0];
    assert_eq!((engine.kind, engine.look), (super::super::icons::Kind::Engine, Look::Running));
    assert!(rows.iter().all(|r| r.stage.is_none() && r.figures.is_none()));
}

#[test]
fn amounts_read_in_the_capacity_unit() {
    let r = |value: f64, precision: f64| Some(Reading { value, precision });
    // (held, capacity, text)
    let table = [
        (r(287_412.0, 0.0), 287_412.0, "287.4/287.4 t"),
        (r(150_000.0, 1_400.0), 287_412.0, "150/287.4 t"),
        (None, 2_000.0, "—/2000 kg"),
        (r(1_234.5, 0.0), 2_000.0, "1234/2000 kg"),
    ];
    for (held, capacity, want) in table {
        assert_eq!(amount(held, capacity), want);
    }
}

#[test]
fn drops_move_stages_and_commands() {
    // (drag, onto, edit)
    let table = [
        (Drag::Stage(2), 4, Some(SequenceEdit::Move { from: 2, to: 4 })),
        (Drag::Stage(3), 3, None),
        (Drag::Action { stage: 3, action: 1 }, 5, Some(SequenceEdit::MoveAction { from: 3, action: 1, to: 5 })),
        (Drag::Action { stage: 3, action: 1 }, 3, None),
    ];
    for (drag, onto, want) in table {
        assert_eq!(drop_edit(drag, onto), want, "{drag:?} onto {onto}");
    }
    // A drop is a ship command (D108: one command type).
    let c = ShipCommand::Sequence(drop_edit(Drag::Stage(2), 4).unwrap());
    assert!(matches!(c, ShipCommand::Sequence(SequenceEdit::Move { .. })));
}

/// The edits a player makes on the bar, as the agent sends them
/// (`edit_stages`): only the stages still to run are dragged or deleted,
/// only those run have AGAIN. The Falcon 9 after its first stage.
#[test]
fn the_bar_edits_what_a_player_can_drag_delete_or_run_again() {
    let (def, mut state, hull) = falcon9();
    state.next_stage(&def).unwrap();
    let rows = bar(&def, &state, &hull, &Knowledge::perfect());
    let edit = |e| Ok(Sent::Edit(e));
    // (op, what it sends or the refusal's words)
    let table: [(Op, Result<Sent, &str>); 12] = [
        (Op::Move { stage: 2, to: 4 }, edit(SequenceEdit::Move { from: 2, to: 4 })),
        (Op::Move { stage: 0, to: 2 }, Err("has run")),
        (Op::Move { stage: 2, to: 0 }, Err("has run")),
        (Op::Move { stage: 2, to: 2 }, Err("already in")),
        (Op::Move { stage: 2, to: 99 }, Err("no stage 99")),
        (Op::MoveCommand { stage: 3, command: 1, to: 6 }, edit(SequenceEdit::MoveAction { from: 3, action: 1, to: 6 })),
        (Op::MoveCommand { stage: 3, command: 7, to: 6 }, Err("no command 7")),
        (Op::MoveCommand { stage: 3, command: 1, to: 3 }, Err("already in")),
        (Op::Delete(6), edit(SequenceEdit::Delete(6))),
        (Op::Delete(0), Err("has run")),
        (Op::Again(0), Ok(Sent::Again(0))),
        (Op::Again(1), Err("has not run")),
    ];
    for (op, want) in table {
        match (check(op, &rows), want) {
            (Ok(got), Ok(want)) => assert_eq!(got, want, "{op:?}"),
            (Err(e), Err(words)) => assert!(e.contains(words), "{op:?}: {e}"),
            (got, want) => panic!("{op:?}: {got:?}, want {want:?}"),
        }
    }
}
