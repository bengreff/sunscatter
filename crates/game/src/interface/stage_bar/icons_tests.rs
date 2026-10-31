//! The icons' rules on the game's test rocket (four engines, four legs, a
//! separation, two thruster blocks): which icons a command has and how
//! they look as the ship state changes.

use super::*;
use crate::test_rocket::rocket;
use sim::chute::{CanopyState, ChuteState};
use sim::craft::command::SequenceEdit;

fn kinds(v: &[Icon]) -> Vec<(Kind, Look)> {
    v.iter().map(|i| (i.kind, i.look)).collect()
}

#[test]
fn each_command_shows_what_it_acts_on() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let state = ShipState::initial(def);
    let none: (&[u32], Look) = (&[], Look::Idle);
    let chute: (&[u32], Look) = (&[0], Look::Armed);
    // (command, icons)
    let table: [(ShipCommand, Vec<(Kind, Look)>); 8] = [
        (ShipCommand::Ignite("main".into()), vec![(Kind::Engine, Look::Idle); 4]),
        (ShipCommand::Select("pair".into()), vec![(Kind::Throttle, Look::Running); 2]),
        (ShipCommand::Deploy("leg#*".into()), vec![(Kind::Leg, Look::Idle); 4]),
        (ShipCommand::Separate("separation".into()), vec![(Kind::Decoupler, Look::Idle)]),
        (ShipCommand::Rcs("rcs#*".into(), false), vec![(Kind::Rcs, Look::Running); 2]),
        (ShipCommand::Power("computer".into(), false), vec![(Kind::Power, Look::Running)]),
        (ShipCommand::ReleaseClamps, vec![]),
        (ShipCommand::Sequence(SequenceEdit::Delete(0)), vec![]),
    ];
    for (c, want) in table {
        assert_eq!(kinds(&icons(def, &state, &c, none)), want, "{c:?}");
    }
    assert_eq!(kinds(&icons(def, &state, &ShipCommand::ArmChutes, chute)), [(Kind::Parachute, Look::Armed)]);
    // Each icon opens its own part's card.
    let engines = icons(def, &state, &ShipCommand::Ignite("main".into()), none);
    let parts: Vec<u32> = def.engines.iter().map(|e| e.part).collect();
    assert_eq!(engines.iter().map(|i| i.part).collect::<Vec<_>>(), parts);
    let sep = &icons(def, &state, &ShipCommand::Separate("separation".into()), none)[0];
    let j = def.joints.iter().find(|j| j.name == "separation").unwrap();
    assert_eq!(sep.part, j.child);
}

#[test]
fn icons_follow_the_ship_state() {
    let craft = rocket();
    let def = craft.ship.as_deref().unwrap();
    let mut state = ShipState::initial(def);
    let none: (&[u32], Look) = (&[], Look::Idle);
    let looks = |state: &ShipState, c: &str| -> Vec<Look> {
        let c = match c {
            "engines" => ShipCommand::Ignite("main".into()),
            "legs" => ShipCommand::Deploy("leg#*".into()),
            _ => ShipCommand::Separate("separation".into()),
        };
        icons(def, state, &c, none).iter().map(|i| i.look).collect()
    };
    state.apply(def, &ShipCommand::Ignite("pair".into())).unwrap();
    use Look::*;
    assert_eq!(looks(&state, "engines"), [Running, Idle, Running, Idle]);
    state.engines[1].status = EngineStatus::Failed;
    state.engines[3].ignitions = 0;
    state.clamped = false;
    assert_eq!(looks(&state, "engines"), [Running, Failed, Running, Spent]);
    state.runtime.refit(def);
    state.engines[3].ignitions = 1;
    state.engines[3].settling = Some(0.5);
    assert_eq!(looks(&state, "engines")[3], Armed, "waiting for its propellant");
    // Legs: one on its way, one deployed.
    state.mechanisms[0].target = 120.0;
    state.apply(def, &ShipCommand::Deploy("leg#2".into())).unwrap();
    assert_eq!(looks(&state, "legs"), [Armed, Running, Idle, Idle]);
    state.apply(def, &ShipCommand::Separate("separation".into())).unwrap();
    assert_eq!(looks(&state, "sep"), [Spent]);
}

#[test]
fn the_parachutes_look_as_their_canopies() {
    let open = CanopyState::Open { age: 1.0, diameter: 3.0 };
    // (armed, drogue, main, look)
    let table = [
        (false, CanopyState::Stowed, CanopyState::Stowed, Look::Idle),
        (true, CanopyState::Stowed, CanopyState::Stowed, Look::Armed),
        (true, open, CanopyState::Stowed, Look::Running),
        (true, CanopyState::Released, open, Look::Running),
        (true, CanopyState::Released, CanopyState::Failed, Look::Failed),
        (true, CanopyState::Released, CanopyState::Released, Look::Spent),
    ];
    for (armed, drogue, main, want) in table {
        assert_eq!(chute_look(&ChuteState { armed, drogue, main }), want, "{drogue:?} {main:?}");
    }
}

#[test]
fn icons_wrap_in_rows() {
    let o = pos2(100.0, 50.0);
    assert_eq!(cell(o, 0, 8).min, o);
    assert_eq!(cell(o, 9, 8).min, pos2(100.0 + SIZE + 2.0, 50.0 + SIZE + 2.0));
    assert_eq!(cell(o, 3, 0).min.y, o.y + 3.0 * (SIZE + 2.0), "never divides by zero");
}
