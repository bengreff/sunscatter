//! Ship commands as the systems carry them out (D104, D107, D108;
//! ship-model.md §5.0, §5.3, §8): an ignition starts an engine's start
//! sequence (one of its igniter charges, or the ground's supply while the
//! pad's clamps hold it; on floating propellant the ship computer first
//! waits for it to settle, `ship_os::ullage`), a shutdown its tail-off, a deployment moves a
//! mechanism at its actuator's rate, and the sequence's clamp release is
//! armed until every ignited engine delivers its rated thrust (the real
//! pad's rule); a release commanded by hand lets go at once. This is the
//! one place an engine or a mechanism is commanded (F8 of the fidelity
//! review: its throttle and selection too). Everything else is the ship
//! state's plain effect (`ShipState::apply`, which refuses an engine's or
//! a mechanism's command).
//!
//! Commands are applied at a tick: orders wait in the ship state for their
//! epoch ([`crate::craft::state::ShipState::order`]), so the same orders give the
//! same flight whatever the frames.

use super::ship_os::sequence;
use crate::catalog::Igniter;
use crate::craft::command::{matches, ShipCommand};
use crate::craft::ship::{Owner, ShipDef};
use crate::craft::state::OrderKind;
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
/// one of them working (`systems::links::commanded`).
pub fn reachable(def: &ShipDef, state: &ShipState, owner: Owner) -> bool {
    !state.out(owner) && super::links::commanded(def, state, owner)
}

/// Applies `command` (see the module); the effects the vessel acts on are
/// returned. Naming nothing the craft has is an error (nothing changes).
pub fn apply(def: &ShipDef, state: &mut ShipState, command: &ShipCommand, cx: Context) -> Result<Vec<Effect>, String> {
    let none = || Err(format!("{command:?}: names nothing on this craft"));
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
                if !reachable(def, state, d.owner) {
                    state.event(cx.at, format!("{}: no working controller or computer: no start", d.id), true);
                    continue;
                }
                let e = &mut state.engines[i];
                if !can_ignite(d.data.igniter, e.ignitions, ground) {
                    state.event(cx.at, format!("{}: no ignition left", d.id), true);
                    continue;
                }
                e.selected = true;
                // Floating propellant: the ship computer's ullage function
                // holds the ignition while the thrusters settle it;
                // commanded again while it waits, it starts at once.
                if super::ship_os::ullage::hold(def, state, i, cx.at, cx.sequence) {
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
        // An engine's own setting (the pilot's throttle scales it).
        ShipCommand::Throttle(s, x) => {
            let picked = def.engines_of(s);
            if picked.is_empty() {
                return none();
            }
            for i in picked {
                state.engines[i].throttle = x.clamp(0.0, 1.0);
            }
            Ok(Vec::new())
        }
        // The engines the pilot's throttle and the readouts refer to.
        ShipCommand::Select(s) => {
            let picked = def.engines_of(s);
            if picked.is_empty() {
                return none();
            }
            for (i, e) in state.engines.iter_mut().enumerate() {
                e.selected = picked.contains(&i);
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
                if let Some(text) = sequence::interlock(def, state, i, command) {
                    state.event(cx.at, text, true);
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
                    if let Some(p) = def.part_index(part).map(|i| &mut state.parts[i]) {
                        p.history.cycles += 1;
                    }
                }
            }
            Ok(Vec::new())
        }
        // Released already (a stage run again, D114): nothing to do.
        ShipCommand::ReleaseClamps if !state.clamped => Ok(Vec::new()),
        ShipCommand::ReleaseClamps if cx.sequence => {
            state.release_armed = true;
            super::ship_os::commit::await_lit(state);
            state.event(cx.at, "Clamps armed: release at full thrust", false);
            Ok(Vec::new())
        }
        _ => state.apply(def, command),
    }
}

/// Carries out an order: its effects, and a line for the event log.
pub fn carry_out(def: &ShipDef, state: &mut ShipState, kind: &OrderKind, at: Epoch) -> Vec<Effect> {
    match kind {
        OrderKind::Command(c) => match apply(def, state, c, Context { at, sequence: false }) {
            Ok(e) => e,
            Err(e) => {
                state.event(at, e, true);
                Vec::new()
            }
        },
        OrderKind::Stage => match sequence::next_stage(def, state, at) {
            Some((name, e)) => {
                state.event(at, format!("Stage: {name}"), false);
                e
            }
            None => Vec::new(),
        },
        OrderKind::Again(k) => match sequence::again(def, state, *k, at) {
            Some((name, e)) => {
                state.event(at, format!("Stage again: {name}"), false);
                e
            }
            None => {
                state.event(at, format!("Stage {} has not run yet on this vessel", k + 1), true);
                Vec::new()
            }
        },
        OrderKind::Function(name) => match sequence::function(def, state, name, at) {
            Some(e) => {
                state.event(at, format!("Function: {name}"), false);
                e
            }
            None => {
                state.event(at, format!("No function {name:?}"), true);
                Vec::new()
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::state::Lost;

    /// The engine rule, in one place (F8 of the fidelity review): one
    /// engine of the small rocket (TEA-TEB, three charges) commanded in
    /// each state that decides what the command does.
    #[test]
    fn the_engine_rule() {
        use crate::catalog::Igniter;
        use EngineStatus::{Failed, Off, Running, ShuttingDown, Starting};
        let craft = crate::craft::small::rocket();
        let at = Epoch::new(1_000_000_000, 0.0);
        let cx = Context { at, sequence: false };
        let ignite = || ShipCommand::Ignite("engine#1".into());
        let shutdown = || ShipCommand::Shutdown("engine#1".into());
        let throttle = |x: f64| ShipCommand::Throttle("engine#1".into(), x);
        /// The engine before: its status, the pad's clamps, its charges, a
        /// hypergolic igniter, its controller lost, its propellant floating,
        /// waiting for it to settle (s).
        #[derive(Clone, Copy)]
        struct Before {
            status: EngineStatus,
            clamped: bool,
            ignitions: u32,
            hypergolic: bool,
            lost: bool,
            floating: bool,
            settling: Option<f64>,
        }
        let off = Before {
            status: Off,
            clamped: false,
            ignitions: 3,
            hypergolic: false,
            lost: false,
            floating: false,
            settling: None,
        };
        /// The engine after: its status, charges, own throttle, waiting to
        /// settle (s), and an event saying.
        type After<'a> = (EngineStatus, u32, f64, Option<f64>, Option<&'a str>);
        // (case, command, before, after)
        #[rustfmt::skip]
        let cases: [(&str, ShipCommand, Before, After); 17] = [
            ("in flight a start spends a charge", ignite(), off, (Starting, 2, 1.0, None, None)),
            ("on the pad the ground supply lights it", ignite(), Before { clamped: true, ignitions: 0, ..off }, (Starting, 0, 1.0, None, None)),
            ("no charge left", ignite(), Before { ignitions: 0, ..off }, (Off, 0, 1.0, None, Some("no ignition left"))),
            ("hypergolic: no charge counted", ignite(), Before { hypergolic: true, ignitions: 0, ..off }, (Starting, 0, 1.0, None, None)),
            ("hypergolic, unlimited", ignite(), Before { hypergolic: true, ignitions: u32::MAX, ..off }, (Starting, u32::MAX, 1.0, None, None)),
            ("lit already: nothing", ignite(), Before { status: Running, ..off }, (Running, 3, 1.0, None, None)),
            ("in its tail-off: nothing", ignite(), Before { status: ShuttingDown, ..off }, (ShuttingDown, 3, 1.0, None, None)),
            ("failed: nothing", ignite(), Before { status: Failed, ..off }, (Failed, 3, 1.0, None, None)),
            ("its controller lost: no start", ignite(), Before { lost: true, ..off }, (Off, 3, 1.0, None, Some("no working controller"))),
            ("floating propellant: it waits", ignite(), Before { floating: true, ..off }, (Off, 3, 1.0, Some(0.0), Some("waiting for its propellant"))),
            ("waiting, commanded again: it starts now", ignite(), Before { floating: true, settling: Some(5.0), ..off }, (Starting, 2, 1.0, None, None)),
            ("running: its tail-off", shutdown(), Before { status: Running, ..off }, (ShuttingDown, 3, 1.0, None, None)),
            ("starting: its tail-off", shutdown(), Before { status: Starting, ..off }, (ShuttingDown, 3, 1.0, None, None)),
            ("off: nothing", shutdown(), off, (Off, 3, 1.0, None, None)),
            ("waiting: the wait ends", shutdown(), Before { settling: Some(5.0), ..off }, (Off, 3, 1.0, None, None)),
            ("its own setting, within 0..1", throttle(1.7), Before { status: Running, ..off }, (Running, 3, 1.0, None, None)),
            ("closed", throttle(-0.2), Before { status: Running, ..off }, (Running, 3, 0.0, None, None)),
        ];
        for (case, command, b, (status, ignitions, own, settling, says)) in cases {
            let mut def = (**craft.ship.as_ref().unwrap()).clone();
            let i = def.engines_of("engine#1")[0];
            if b.hypergolic {
                def.engines[i].data.igniter = Igniter::Hypergolic;
            }
            let mut state = ShipState::initial(&def);
            state.clamped = b.clamped;
            let e = &mut state.engines[i];
            (e.status, e.ignitions, e.settling) = (b.status, b.ignitions, b.settling);
            if b.lost {
                let (part, owner) = (def.engines[i].part, def.engines[i].owner);
                state.lost.push(Lost { part, owner, why: "test".into() });
            }
            if b.floating {
                for t in &mut state.tanks {
                    t.drift = 100.0;
                }
            }
            let events = state.events.len();
            apply(&def, &mut state, &command, cx).unwrap();
            let e = &state.engines[i];
            assert_eq!((e.status, e.ignitions, e.throttle, e.settling), (status, ignitions, own, settling), "{case}");
            let said = state.events[events..].iter().map(|e| e.text.as_str()).collect::<Vec<_>>();
            match says {
                Some(text) => assert!(said.iter().any(|s| s.contains(text)), "{case}: {said:?}"),
                None => assert!(said.is_empty(), "{case}: {said:?}"),
            }
        }
        // The selection is exactly the engines named.
        let def = craft.ship.as_deref().unwrap();
        let mut state = ShipState::initial(def);
        apply(def, &mut state, &ShipCommand::Select("pair".into()), cx).unwrap();
        let pair = def.engines_of("pair");
        assert!(pair.len() == 2 && (0..def.engines.len()).all(|i| state.engines[i].selected == pair.contains(&i)));
        // A deployment moves the leg at its actuator's rate
        // (`systems::mechanisms`), not at once; on the pad's clamps a leg
        // with a foot stays stowed.
        let leg = def.mechanisms.iter().position(|m| m.id == "leg#1").unwrap();
        apply(def, &mut state, &ShipCommand::Deploy("leg#1".into()), cx).unwrap();
        let m = &state.mechanisms[leg];
        assert_eq!((m.target, m.angle), (def.mechanisms[leg].item.deployed_deg, def.mechanisms[leg].item.stowed_deg));
        let mut held = ShipState::initial(def);
        held.clamped = true;
        apply(def, &mut held, &ShipCommand::Deploy("leg#1".into()), cx).unwrap();
        assert_eq!(held.mechanisms[leg].target, def.mechanisms[leg].item.stowed_deg);
        // The ship state's plain effect refuses every engine and mechanism
        // command, and nothing changes.
        let refused = [
            ignite(),
            shutdown(),
            throttle(0.5),
            ShipCommand::Select("pair".into()),
            ShipCommand::Deploy("leg#1".into()),
            ShipCommand::Retract("leg#1".into()),
            ShipCommand::Angle("leg#1".into(), 30.0),
        ];
        for c in refused {
            let mut s = ShipState::initial(def);
            let before = s.clone();
            assert!(s.apply(def, &c).is_err(), "{c:?}");
            assert_eq!(s, before, "{c:?}");
        }
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
        let owner = def.owner_named("s2.computer").unwrap();
        state.unpowered.push(Lost { part, owner, why: "bus dead".into() });
        apply(&def, &mut state, &ShipCommand::Ignite("s2".into()), cx).unwrap();
        assert_eq!(state.engines[mvac].status, EngineStatus::Off);
        assert!(state.events.last().unwrap().text.contains("no working controller"));
        state.unpowered.clear();
        apply(&def, &mut state, &ShipCommand::Ignite("s2".into()), cx).unwrap();
        assert!(matches!(state.engines[mvac].status, EngineStatus::Starting) || state.engines[mvac].settling.is_some());
    }
}
