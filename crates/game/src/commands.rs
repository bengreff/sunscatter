//! The command API (review game 7): every discrete change to the
//! simulation (warp, switching or deleting vessels, loading, reverting,
//! resetting) is a `GameCommand` message. UI and key systems only write
//! commands; `apply` runs them at the start of the next frame's simulation,
//! so a frame is never drawn with a new fleet and an old camera. The same
//! messages are the hook for the MCP server and scripting (D063), which will
//! add an origin (a control location, D067).
//!
//! Continuous flight input (throttle, rotation) stays in `SimState::controls`,
//! written by `state::read_controls` in the input stage.
//!
//! `InputContext` says which keys act this frame (review game 2): one rule,
//! read by every key system.

use crate::camera::{CameraRig, Focus};
use crate::state::{Prediction, SimState, WARP_LEVELS};
use crate::tracking::{self, Tracked, TrackingStation};
use bevy::prelude::*;
use sim::save::SaveGame;
use sim::time::Epoch;
use sim::vessel::{FlightPlan, HoldMode, VesselId};

#[derive(Message, Clone, Debug, PartialEq)]
pub enum GameCommand {
    /// Requested warp level (index into `WARP_LEVELS`).
    SetWarp(usize),
    /// Make a vessel active.
    Switch(VesselId),
    /// Delete a non-active vessel.
    Delete(VesselId),
    /// Replace the game with a (checked, non-empty) save: load or revert.
    Restore(Box<SaveGame>),
    /// Put the active vessel back on the pad.
    Reset,
    /// Debug: spawn test ships in low Earth orbit.
    SpawnTestShips(usize),
    /// Debug mode (D064) on every vessel: infinite propellant, no
    /// overheating, infinite impact tolerance.
    SetDebug(bool),
    /// Replace a vessel's flight plan (a vessel-directed command: from
    /// anywhere but aboard it, it travels with the signal's light delay).
    SetPlan { vessel: VesselId, plan: FlightPlan },
    /// The active vessel's attitude hold (D075); turns SAS on.
    SetHold(HoldMode),
    /// The active vessel's throttle, 0–1 (the flight panel's slider).
    SetThrottle(f64),
}

/// Commands on their way to a vessel at light speed (D063, D067): applied
/// when the game clock reaches their arrival time.
#[derive(Resource, Default)]
pub struct InFlight {
    pub queue: Vec<(Epoch, GameCommand)>,
}

impl InFlight {
    /// Sends `command` to arrive `delay` seconds after `now`.
    pub fn send(&mut self, now: Epoch, delay: f64, command: GameCommand) {
        self.queue.push((now.add_seconds(delay), command));
    }

    /// Removes and returns the commands that have arrived by `now`, in order
    /// of arrival (ties: order of sending).
    pub fn arrived(&mut self, now: Epoch) -> Vec<GameCommand> {
        let mut due: Vec<(Epoch, GameCommand)> = Vec::new();
        self.queue.retain(|(t, c)| {
            let arrived = now.seconds_since(*t) >= 0.0;
            if arrived {
                due.push((*t, c.clone()));
            }
            !arrived
        });
        due.sort_by(|a, b| a.0.seconds_since(b.0).total_cmp(&0.0));
        due.into_iter().map(|(_, c)| c).collect()
    }
}

/// Applies the frame's commands, in the order they were written, then
/// those arriving from far away.
#[allow(clippy::too_many_arguments)]
pub fn apply(
    mut commands: MessageReader<GameCommand>,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut pred: ResMut<Prediction>,
    mut tracked: ResMut<Tracked>,
    mut ts: ResMut<TrackingStation>,
    mut in_flight: ResMut<InFlight>,
    mut toasts: ResMut<crate::interface::toasts::Toasts>,
    time: Res<Time>,
) {
    let arrived = in_flight.arrived(sim.clock);
    for c in commands.read().cloned().chain(arrived) {
        match &c {
            GameCommand::SetWarp(level) => sim.warp = (*level).min(WARP_LEVELS.len() - 1),
            GameCommand::Switch(i) => {
                tracking::switch_to(&mut sim, &mut rig, &mut pred, *i);
                // Leaving the station restores the flight camera: follow the
                // new vessel.
                ts.follow_active_on_leave();
            }
            GameCommand::Delete(i) => {
                tracking::delete_vessel(&mut sim, &mut tracked, &mut rig, *i);
                ts.clear_selection();
            }
            GameCommand::Restore(save) => {
                crate::saves::restore(&mut sim, (**save).clone());
                crate::saves::after_load(&mut pred, &mut tracked, &mut rig);
                // Indices in the station may now name other vessels.
                ts.clear_selection();
            }
            GameCommand::Reset => {
                sim.reset();
                rig.focus = Focus::Ship;
            }
            GameCommand::SpawnTestShips(n) => sim.spawn_test_ships(*n),
            GameCommand::SetPlan { vessel, plan } => {
                let sim = &mut *sim;
                let now = time.elapsed_secs_f64();
                match sim.index_of(*vessel) {
                    None => toasts.push(now, format!("Plan for Vessel {}: no such vessel", vessel.0), true),
                    Some(i) => match sim.fleet[i].set_plan(&sim.world, plan.clone()) {
                        Ok(()) => toasts.push(now, format!("Vessel {} accepted its flight plan", vessel.0), false),
                        Err(e) => {
                            toasts.push(now, format!("Vessel {} rejected its flight plan: {e:?}", vessel.0), true)
                        }
                    },
                }
            }
            GameCommand::SetHold(mode) => {
                sim.controls.hold = *mode;
                sim.controls.sas = true;
            }
            GameCommand::SetThrottle(t) => sim.controls.throttle = t.clamp(0.0, 1.0),
            GameCommand::SetDebug(on) => {
                let sim = &mut *sim;
                for v in &mut sim.fleet {
                    v.set_debug(&sim.world, *on);
                }
            }
        }
    }
}

/// Where keyboard input goes this frame.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputContext {
    #[default]
    Flight,
    /// The tracking station is open.
    Station,
    /// The pause menu is open.
    Paused,
    /// A text field has keyboard focus.
    TextEntry,
}

/// Groups of keys, by what they act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keys {
    /// Throttle, rotation, SAS, chute, reset to the pad, vessel switching.
    Flight,
    /// Time warp.
    Warp,
    /// Quicksave, quickload, the save list, the tracking station, settings.
    Menus,
    /// Debug keys (F2).
    Debug,
}

impl InputContext {
    /// The context from what is open; text entry wins, then the pause menu.
    pub fn from_state(text_entry: bool, paused: bool, station: bool) -> Self {
        if text_entry {
            InputContext::TextEntry
        } else if paused {
            InputContext::Paused
        } else if station {
            InputContext::Station
        } else {
            InputContext::Flight
        }
    }

    /// Whether `keys` act in this context.
    pub fn allows(self, keys: Keys) -> bool {
        use InputContext as C;
        match keys {
            Keys::Flight | Keys::Debug => self == C::Flight,
            Keys::Warp => matches!(self, C::Flight | C::Station),
            Keys::Menus => self != C::TextEntry,
        }
    }
}

/// Sets the input context from what is open (runs first in the frame).
pub fn update_context(
    egui: Res<bevy_egui::input::EguiWantsInput>,
    menu: Res<crate::interface::pause::PauseMenu>,
    station: Res<TrackingStation>,
    mut ctx: ResMut<InputContext>,
) {
    ctx.set_if_neq(InputContext::from_state(egui.wants_any_keyboard_input(), menu.open, station.open));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_in_flight_arrive_in_order_after_their_delay() {
        let t0 = sim::sol::sol_epoch();
        let mut q = InFlight::default();
        q.send(t0, 1.3, GameCommand::SetWarp(2));
        q.send(t0, 0.5, GameCommand::SetWarp(1));
        assert!(q.arrived(t0.add_seconds(0.4)).is_empty());
        assert_eq!(q.arrived(t0.add_seconds(0.5)), vec![GameCommand::SetWarp(1)]);
        assert_eq!(q.arrived(t0.add_seconds(10.0)), vec![GameCommand::SetWarp(2)]);
        assert!(q.queue.is_empty());
    }

    #[test]
    fn text_entry_wins_then_pause_then_station() {
        assert_eq!(InputContext::from_state(true, true, true), InputContext::TextEntry);
        assert_eq!(InputContext::from_state(false, true, true), InputContext::Paused);
        assert_eq!(InputContext::from_state(false, false, true), InputContext::Station);
        assert_eq!(InputContext::from_state(false, false, false), InputContext::Flight);
    }

    #[test]
    fn keys_by_context() {
        use InputContext as C;
        // (context, flight, warp, menus, debug)
        let table = [
            (C::Flight, true, true, true, true),
            (C::Station, false, true, true, false),
            (C::Paused, false, false, true, false),
            (C::TextEntry, false, false, false, false),
        ];
        for (c, flight, warp, menus, debug) in table {
            assert_eq!(c.allows(Keys::Flight), flight, "{c:?} flight");
            assert_eq!(c.allows(Keys::Warp), warp, "{c:?} warp");
            assert_eq!(c.allows(Keys::Menus), menus, "{c:?} menus");
            assert_eq!(c.allows(Keys::Debug), debug, "{c:?} debug");
        }
    }
}
