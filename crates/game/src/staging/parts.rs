//! The stage stack of a craft of catalog parts (D107): the vessel's own
//! stage list (`ShipState::stages`, the craft file's at load, edited in
//! the systems screen) in its view (`sim::craft::program`: after a split,
//! the stages and commands acting on this piece), numbered as KSP does
//! (the first stage the highest number), each stage's commands with the
//! state of what they act on now.
//! Space runs the next stage (`Vessel::next_stage`); nothing is locked on
//! the ground, since a craft of parts launches by its sequence.

use super::{Part, PartTone, Stage, StageState};
use sim::craft::command::{matches, ShipCommand};
use sim::craft::program;
use sim::craft::ship::ShipDef;
use sim::craft::state::{EngineStatus, ShipState};

/// The stack, stage 0 (the last of the sequence) first. `chute_armed`: the
/// parachute command was given.
pub fn stack(def: &ShipDef, state: &ShipState, chute_armed: bool) -> Vec<Stage> {
    let shown = program::view(def, &state.stages);
    let next = program::next(def, &state.stages, state.sequence);
    let n = shown.len();
    let mut out: Vec<Stage> = (shown.iter().enumerate())
        .map(|(k, v)| Stage {
            number: (n - 1 - k) as u32,
            state: match v.index {
                j if j < state.sequence as usize => StageState::Active,
                j if Some(j) == next => StageState::Next,
                _ => StageState::Later,
            },
            name: Some(state.stages[v.index].name.clone()),
            parts: (v.actions.iter())
                .map(|&a| {
                    let c = &state.stages[v.index].actions[a];
                    let (state, tone) = status(def, state, c, chute_armed);
                    Part { name: label(c), state, tone }
                })
                .collect(),
        })
        .collect();
    out.reverse();
    out
}

/// A command as the menus and the systems screen write it.
pub fn label(c: &ShipCommand) -> String {
    let on = |b: bool| if b { "on" } else { "off" };
    match c {
        ShipCommand::Ignite(s) => format!("IGNITE {s}"),
        ShipCommand::Shutdown(s) => format!("SHUT DOWN {s}"),
        ShipCommand::Throttle(s, x) => format!("THROTTLE {s} {:.0} %", x * 100.0),
        ShipCommand::Select(s) => format!("SELECT {s}"),
        ShipCommand::Open(s) => format!("OPEN {s}"),
        ShipCommand::Close(s) => format!("CLOSE {s}"),
        ShipCommand::Deploy(s) => format!("DEPLOY {s}"),
        ShipCommand::Retract(s) => format!("RETRACT {s}"),
        ShipCommand::Angle(s, a) => format!("ANGLE {s} {a:.0}°"),
        ShipCommand::Separate(s) => format!("SEPARATE {s}"),
        ShipCommand::ReleaseClamps => "RELEASE CLAMPS".into(),
        ShipCommand::ArmChutes => "ARM PARACHUTES".into(),
        ShipCommand::Rcs(s, b) => format!("RCS {s} {}", on(*b)),
        ShipCommand::Power(s, b) => format!("POWER {s} {}", on(*b)),
        ShipCommand::Sequence(_) => "EDIT SEQUENCE".into(),
    }
}

/// The state now of what command `c` acts on, and how it reads.
pub fn status(def: &ShipDef, state: &ShipState, c: &ShipCommand, chute_armed: bool) -> (String, PartTone) {
    let picked = |ids: &mut dyn Iterator<Item = &str>, s: &str| -> Vec<usize> {
        ids.enumerate().filter(|(_, id)| matches(s, id)).map(|(i, _)| i).collect()
    };
    let none = || ("not on this craft".to_string(), PartTone::Idle);
    match c {
        ShipCommand::Ignite(s) | ShipCommand::Shutdown(s) | ShipCommand::Throttle(s, _) | ShipCommand::Select(s) => {
            let engines = def.engines_of(s);
            if engines.is_empty() {
                return none();
            }
            let status = |i: &usize| state.engines.get(*i).map(|e| e.status);
            let count = |s: EngineStatus| engines.iter().filter(|i| status(i) == Some(s)).count();
            let (running, starting) = (count(EngineStatus::Running), count(EngineStatus::Starting));
            let failed = count(EngineStatus::Failed);
            let n = engines.len();
            let starting_text = if starting > 0 { format!(" · {starting} starting") } else { String::new() };
            match (running, failed) {
                (_, f) if f > 0 => (format!("{running}/{n} running{starting_text} · {f} failed"), PartTone::Failed),
                (0, _) if starting > 0 => (format!("{starting}/{n} starting"), PartTone::Working),
                (0, _) => (if n == 1 { "off".into() } else { format!("{n} off") }, PartTone::Idle),
                (_, _) if n == 1 => ("running".into(), PartTone::Working),
                (r, _) => (format!("{r}/{n} running{starting_text}"), PartTone::Working),
            }
        }
        ShipCommand::Open(s) | ShipCommand::Close(s) => {
            let valves = picked(&mut def.valves.iter().map(|v| v.id.as_str()), s);
            if valves.is_empty() {
                return none();
            }
            let open = valves.iter().filter(|&&i| state.valves.get(i).is_some_and(|v| v.open)).count();
            let stuck = valves.iter().any(|&i| state.valves.get(i).is_some_and(|v| v.stuck));
            let text = match open {
                0 => "closed".to_string(),
                k if k == valves.len() => "open".to_string(),
                k => format!("{k}/{} open", valves.len()),
            };
            (
                if stuck { format!("{text} · STUCK") } else { text },
                if stuck { PartTone::Failed } else { PartTone::Idle },
            )
        }
        ShipCommand::Deploy(s) | ShipCommand::Retract(s) | ShipCommand::Angle(s, _) => {
            let found = picked(&mut def.mechanisms.iter().map(|m| m.id.as_str()), s);
            let Some(&i) = found.first() else { return none() };
            let (m, angle) = (&def.mechanisms[i].item, state.mechanisms.get(i).map_or(0.0, |m| m.angle));
            if (angle - m.deployed_deg).abs() < 0.5 {
                ("deployed".into(), PartTone::Working)
            } else if (angle - m.stowed_deg).abs() < 0.5 {
                ("stowed".into(), PartTone::Idle)
            } else {
                (format!("{angle:.0}°"), PartTone::Working)
            }
        }
        ShipCommand::Separate(name) => match def.joints.iter().position(|j| j.name == *name) {
            Some(j) if state.joints.get(j).is_some_and(|j| j.intact) => ("attached".into(), PartTone::Idle),
            Some(_) => ("separated".into(), PartTone::Working),
            None => ("separated".into(), PartTone::Working),
        },
        ShipCommand::ReleaseClamps => {
            // The pad's hold-down joints (`Vessel::clamped` holds while one does).
            let hold = |j: &sim::craft::ship::JointDef| j.kind == sim::craft::assembly::JointKind::HoldDown;
            let holding = def.joints.iter().zip(&state.joints).any(|(d, s)| hold(d) && s.intact);
            if state.clamped && holding && state.runtime.release_armed {
                // The pad's rule: they let go when every lit engine is at its rated thrust.
                ("armed: lets go at full thrust".into(), PartTone::Working)
            } else if state.clamped && holding {
                ("holding".into(), PartTone::Idle)
            } else {
                ("released".into(), PartTone::Working)
            }
        }
        ShipCommand::ArmChutes => {
            if chute_armed {
                ("armed".into(), PartTone::Working)
            } else {
                ("stowed".into(), PartTone::Idle)
            }
        }
        ShipCommand::Rcs(s, _) => {
            let blocks = picked(&mut def.thrusters.iter().map(|t| t.id.as_str()), s);
            if blocks.is_empty() {
                return none();
            }
            let on = blocks.iter().filter(|&&i| state.thrusters.get(i).is_some_and(|t| t.enabled)).count();
            (format!("{on}/{} enabled", blocks.len()), if on > 0 { PartTone::Working } else { PartTone::Idle })
        }
        ShipCommand::Power(s, _) => {
            let comps = picked(&mut def.components.iter().map(|c| c.id.as_str()), s);
            if comps.is_empty() {
                return none();
            }
            let on = comps.iter().filter(|&&i| state.power.get(i).is_some_and(|p| p.on)).count();
            (format!("{on}/{} on", comps.len()), if on > 0 { PartTone::Working } else { PartTone::Idle })
        }
        ShipCommand::Sequence(_) => (String::new(), PartTone::Idle),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_rocket::rocket;

    #[test]
    fn the_sequence_stacks_last_stage_first_with_the_next_one_marked() {
        let craft = rocket();
        let def = craft.ship.as_deref().unwrap();
        let mut state = ShipState::initial(def);
        let s = stack(def, &state, false);
        let summary: Vec<_> = s.iter().map(|s| (s.number, s.state, s.name.clone().unwrap())).collect();
        assert_eq!(
            summary,
            [(0, StageState::Later, "Separation".to_string()), (1, StageState::Next, "Ignition".to_string())]
        );
        assert_eq!(super::super::next(&s), Some(1));
        assert_eq!(s[1].parts[0].name, "IGNITE main");
        assert_eq!((s[1].parts[0].state.as_str(), s[1].parts[0].tone), ("4 off", PartTone::Idle));
        // After the ignition: stage 1 done, its engines running; Separation next.
        state.next_stage(def).unwrap();
        let s = stack(def, &state, false);
        assert_eq!((s[1].state, s[0].state), (StageState::Active, StageState::Next));
        assert_eq!(s[1].parts[0].state, "4/4 running");
        assert_eq!(
            s[0].parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
            ["SHUT DOWN main", "SEPARATE separation"]
        );
        assert_eq!(s[0].parts[1].state, "attached");
    }

    #[test]
    fn orders_that_arm_the_parachutes() {
        use sim::systems::OrderKind;
        let craft = rocket();
        let def = craft.ship.as_deref().unwrap();
        let mut state = ShipState::initial(def);
        state.functions.push(sim::craft::command::Stage {
            name: "Chutes".into(),
            key: None,
            actions: vec![ShipCommand::ArmChutes],
            pinned: false,
        });
        state.stages[1].actions.push(ShipCommand::ArmChutes);
        // (orders, arms)
        let table = [
            (vec![OrderKind::Command(ShipCommand::ArmChutes)], true),
            (vec![OrderKind::Command(ShipCommand::ReleaseClamps)], false),
            (vec![OrderKind::Function("Chutes".into())], true),
            (vec![OrderKind::Function("Legs".into())], false),
            (vec![OrderKind::Stage], false),
        ];
        for (kinds, want) in table {
            assert_eq!(super::super::arms_chutes(def, &state, &kinds), want, "{kinds:?}");
        }
        // The next stage arms them once the sequence reaches it.
        state.sequence = 1;
        assert!(super::super::arms_chutes(def, &state, &[OrderKind::Stage]));
    }

    #[test]
    fn statuses_follow_the_ship_state() {
        let craft = rocket();
        let def = craft.ship.as_deref().unwrap();
        let mut state = ShipState::initial(def);
        let st = |state: &ShipState, c: ShipCommand| status(def, state, &c, false).0;
        assert_eq!(st(&state, ShipCommand::Deploy("leg#*".into())), "stowed");
        state.apply(def, &ShipCommand::Deploy("leg#*".into())).unwrap();
        assert_eq!(st(&state, ShipCommand::Deploy("leg#*".into())), "deployed");
        state.apply(def, &ShipCommand::Ignite("pair".into())).unwrap();
        assert_eq!(st(&state, ShipCommand::Ignite("main".into())), "2/4 running");
        assert_eq!(st(&state, ShipCommand::Ignite("engine#1".into())), "running");
        // Engines on their start transient (RUNTIME's): starting.
        state.engines[1].status = EngineStatus::Starting;
        assert_eq!(st(&state, ShipCommand::Ignite("main".into())), "2/4 running · 1 starting");
        state.engines[0].status = EngineStatus::Off;
        state.engines[2].status = EngineStatus::Off;
        assert_eq!(st(&state, ShipCommand::Ignite("main".into())), "1/4 starting");
        state.engines[0].status = EngineStatus::Running;
        state.engines[2].status = EngineStatus::Running;
        state.engines[1].status = EngineStatus::Off;
        assert_eq!(st(&state, ShipCommand::Rcs("rcs#*".into(), true)), "2/2 enabled");
        assert_eq!(st(&state, ShipCommand::Ignite("nothing".into())), "not on this craft");
        assert_eq!(status(def, &state, &ShipCommand::ArmChutes, true).0, "armed");
        assert_eq!(st(&state, ShipCommand::ReleaseClamps), "released", "no pad mount");
        assert_eq!(label(&ShipCommand::Throttle("main".into(), 0.6)), "THROTTLE main 60 %");
    }
}
