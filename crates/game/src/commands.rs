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
//! read by every key system; [`keys`] which keys the game takes and the
//! keys a craft's pinned functions get.

pub mod keys;

use crate::camera::{CameraRig, Focus};
use crate::state::{Prediction, SimState, WARP_LEVELS};
use crate::tracking::{self, Tracked, TrackingStation};
use bevy::prelude::*;
use sim::craft::command::{ShipCommand, Stage};
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
    /// Put the active vessel back on the pad (a new craft of its kind).
    Reset,
    /// A new flight: the active vessel replaced by a new craft `.0` (a
    /// directory under `data/craft`) on the pad.
    NewFlight(String),
    /// Debug: spawn test ships in low Earth orbit.
    SpawnTestShips(usize),
    /// Debug mode (D064) on every vessel: infinite propellant, no
    /// overheating, infinite impact tolerance.
    SetDebug(bool),
    /// Replace a vessel's flight plan (a vessel-directed command: from
    /// anywhere but aboard it, it travels with the signal's light delay).
    SetPlan {
        vessel: VesselId,
        plan: FlightPlan,
    },
    /// The active vessel's attitude hold (D075); turns SAS on.
    SetHold(HoldMode),
    /// The active vessel's throttle, 0–1 (the navball's throttle bar).
    SetThrottle(f64),
    /// SAS on or off (the navball's SAS button; T toggles it directly).
    SetSas(bool),
    /// Activate the active vessel's next stage (Space, the Staging menu);
    /// for a craft of parts it becomes [`GameCommand::Stage`] of that
    /// vessel, sent to it.
    NextStage,
    /// Run a craft of parts' next stage (its sequence), sent to it.
    Stage(VesselId),
    /// Commands to a craft of parts (D107: the systems screen, the mini
    /// systems panel's functions), applied in order.
    Ship {
        vessel: VesselId,
        commands: Vec<ShipCommand>,
    },
    /// A craft of parts' stage sequence or saved functions, edited in the
    /// systems screen (saved with the vessel; checked, refused whole).
    SetStages {
        vessel: VesselId,
        stages: Vec<Stage>,
    },
    SetFunctions {
        vessel: VesselId,
        functions: Vec<Stage>,
    },
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
    comms: Res<crate::comms::Comms>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs_f64();
    let fresh: Vec<(GameCommand, bool)> = commands.read().map(|c| (c.clone(), true)).collect();
    let arrived = in_flight.arrived(sim.clock).into_iter().map(|c| (c, false));
    for (c, fresh) in fresh.into_iter().chain(arrived) {
        // Space on a craft of parts: its own sequence's next stage.
        let c = match c {
            GameCommand::NextStage if sim.ship().ship().is_some() => GameCommand::Stage(sim.ship().id()),
            c => c,
        };
        // A command for a vessel reaches it at once from aboard, else after
        // the signal's light delay, and not at all without one (D067).
        if let Some(id) = addressee(&c).filter(|_| fresh) {
            let aboard = comms.location == crate::comms::Location::Vessel(id);
            match route(aboard, comms.signal(id).map(|s| s.delay)) {
                Route::Now => {}
                Route::After(delay) => {
                    if delay >= 1.0 {
                        toasts.push(now, format!("Sent to Vessel {} ({})", id.0, crate::format::delay(delay)), false);
                    }
                    in_flight.send(sim.clock, delay, c);
                    continue;
                }
                Route::NoSignal => {
                    toasts.push(now, format!("Vessel {}: no signal, the command cannot reach it", id.0), true);
                    continue;
                }
            }
        }
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
            GameCommand::NewFlight(craft) => {
                sim.new_flight(&crate::state::craft_or_test(craft));
                rig.focus = Focus::Ship;
            }
            GameCommand::Ship { vessel, commands: list } => {
                let now = time.elapsed_secs_f64();
                for message in ship_commands(&mut sim, *vessel, list) {
                    toasts.push(now, message, true);
                }
            }
            GameCommand::SetStages { vessel, stages } => {
                let done = sim.index_of(*vessel).map(|i| sim.fleet[i].set_stages(stages.clone()));
                edited(&mut toasts, time.elapsed_secs_f64(), *vessel, "stages", done);
            }
            GameCommand::SetFunctions { vessel, functions } => {
                let done = sim.index_of(*vessel).map(|i| sim.fleet[i].set_functions(functions.clone()));
                edited(&mut toasts, time.elapsed_secs_f64(), *vessel, "functions", done);
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
            GameCommand::SetSas(on) => sim.controls.sas = *on,
            GameCommand::Stage(id) => {
                let sim = &mut *sim;
                let Some(i) = sim.index_of(*id) else { continue };
                let text = match sim.fleet[i].next_stage(&sim.world) {
                    Some((name, effects)) => {
                        if i == sim.active {
                            crate::staging::controls_after(&effects, &mut sim.controls);
                        }
                        format!("Stage: {name}")
                    }
                    None => crate::staging::Refusal::NoStageLeft.message().to_string(),
                };
                toasts.push(now, text, false);
            }
            GameCommand::NextStage => {
                let mut controls = sim.controls;
                let ship = sim.ship();
                let inputs = crate::staging::StageInputs::of(ship, &sim.controls);
                let done = crate::staging::activate_next(&inputs, crate::staging::grounded(ship), &mut controls);
                sim.controls = controls;
                if let Err(why) = done {
                    toasts.push(time.elapsed_secs_f64(), why.message(), false);
                }
            }
            GameCommand::SetDebug(on) => {
                let sim = &mut *sim;
                for v in &mut sim.fleet {
                    v.set_debug(&sim.world, *on);
                }
            }
        }
    }
}

/// How a command reaches a vessel (D067).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Route {
    Now,
    /// After this light delay (s).
    After(f64),
    NoSignal,
}

/// How a command reaches a vessel: at once from aboard it, after the
/// signal's one-way delay (`delay`, `None`: no path) from anywhere else.
pub fn route(aboard: bool, delay: Option<f64>) -> Route {
    match (aboard, delay) {
        (true, _) => Route::Now,
        (false, Some(d)) if d > 0.0 => Route::After(d),
        (false, Some(_)) => Route::Now,
        (false, None) => Route::NoSignal,
    }
}

/// The vessel a command travels to: a craft of parts' commands, its next
/// stage, its edited stages and functions (flight plans are routed by the
/// planner; controls by `state::RemoteControls`).
pub fn addressee(c: &GameCommand) -> Option<VesselId> {
    match c {
        GameCommand::Ship { vessel, .. }
        | GameCommand::SetStages { vessel, .. }
        | GameCommand::SetFunctions { vessel, .. } => Some(*vessel),
        GameCommand::Stage(id) => Some(*id),
        _ => None,
    }
}

/// Applies `list` to vessel `id` (a craft of parts) now, the active
/// vessel's controls following their effects (the parachutes armed); the
/// messages for what failed.
pub fn ship_commands(sim: &mut SimState, id: VesselId, list: &[ShipCommand]) -> Vec<String> {
    let Some(i) = sim.index_of(id) else { return vec![format!("Vessel {}: no such vessel", id.0)] };
    let mut errors = Vec::new();
    for c in list {
        match sim.fleet[i].command(&sim.world, c) {
            Ok(effects) if i == sim.active => crate::staging::controls_after(&effects, &mut sim.controls),
            Ok(_) => {}
            Err(e) => errors.push(format!("Vessel {}: {e}", id.0)),
        }
    }
    errors
}

/// The toast after an edit of `what` (stages or functions) of `vessel`:
/// saved, or refused with the reasons (`None`: no such vessel).
fn edited(
    toasts: &mut crate::interface::toasts::Toasts,
    now: f64,
    vessel: VesselId,
    what: &str,
    done: Option<Result<(), Vec<String>>>,
) {
    match done {
        Some(Ok(())) => toasts.push(now, format!("Vessel {}: {what} saved", vessel.0), false),
        Some(Err(e)) => toasts.push(now, format!("Vessel {}: {what} refused: {}", vessel.0, e.join("; ")), true),
        None => toasts.push(now, format!("Vessel {}: no such vessel", vessel.0), true),
    }
}

/// Where keyboard input goes this frame.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputContext {
    #[default]
    Flight,
    /// The tracking station is open.
    Station,
    /// The systems screen is open (D107).
    Systems,
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
    /// The flight view's task menus (O, L, U, G): also while paused.
    Hud,
}

impl InputContext {
    /// The context from what is open; text entry wins, then the pause
    /// menu, the tracking station, the systems screen.
    pub fn from_state(text_entry: bool, paused: bool, station: bool, systems: bool) -> Self {
        if text_entry {
            InputContext::TextEntry
        } else if paused {
            InputContext::Paused
        } else if station {
            InputContext::Station
        } else if systems {
            InputContext::Systems
        } else {
            InputContext::Flight
        }
    }

    /// Whether `keys` act in this context.
    pub fn allows(self, keys: Keys) -> bool {
        use InputContext as C;
        match keys {
            Keys::Flight | Keys::Debug => self == C::Flight,
            Keys::Warp => matches!(self, C::Flight | C::Station | C::Systems),
            Keys::Menus => self != C::TextEntry,
            Keys::Hud => matches!(self, C::Flight | C::Paused),
        }
    }
}

/// Sets the input context from what is open (runs first in the frame).
pub fn update_context(
    egui: Res<bevy_egui::input::EguiWantsInput>,
    menu: Res<crate::interface::pause::PauseMenu>,
    station: Res<TrackingStation>,
    systems: Res<crate::systems_screen::SystemsScreen>,
    mut ctx: ResMut<InputContext>,
) {
    let context = InputContext::from_state(egui.wants_any_keyboard_input(), menu.open, station.open, systems.open);
    ctx.set_if_neq(context);
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
    fn commands_reach_a_vessel_from_aboard_now_else_after_the_delay() {
        // (aboard, delay, route)
        let table = [
            (true, None, Route::Now),
            (true, Some(3.0), Route::Now),
            (false, Some(0.0), Route::Now),
            (false, Some(1.3), Route::After(1.3)),
            (false, None, Route::NoSignal),
        ];
        for (aboard, delay, want) in table {
            assert_eq!(route(aboard, delay), want, "{aboard} {delay:?}");
        }
        let v = VesselId(7);
        assert_eq!(addressee(&GameCommand::Ship { vessel: v, commands: vec![] }), Some(v));
        assert_eq!(addressee(&GameCommand::Stage(v)), Some(v));
        assert_eq!(addressee(&GameCommand::SetFunctions { vessel: v, functions: vec![] }), Some(v));
        assert_eq!(addressee(&GameCommand::NextStage), None, "the test craft's, at once");
        assert_eq!(addressee(&GameCommand::SetWarp(2)), None);
    }

    #[test]
    fn text_entry_wins_then_pause_then_station_then_systems() {
        use InputContext as C;
        // (text entry, paused, station, systems, context)
        let table = [
            (true, true, true, true, C::TextEntry),
            (false, true, true, true, C::Paused),
            (false, false, true, true, C::Station),
            (false, false, false, true, C::Systems),
            (false, false, false, false, C::Flight),
        ];
        for (text, paused, station, systems, want) in table {
            assert_eq!(InputContext::from_state(text, paused, station, systems), want);
        }
    }

    #[test]
    fn keys_by_context() {
        use InputContext as C;
        // (context, flight, warp, menus, debug, hud)
        let table = [
            (C::Flight, true, true, true, true, true),
            (C::Station, false, true, true, false, false),
            (C::Systems, false, true, true, false, false),
            (C::Paused, false, false, true, false, true),
            (C::TextEntry, false, false, false, false, false),
        ];
        for (c, flight, warp, menus, debug, hud) in table {
            assert_eq!(c.allows(Keys::Flight), flight, "{c:?} flight");
            assert_eq!(c.allows(Keys::Warp), warp, "{c:?} warp");
            assert_eq!(c.allows(Keys::Menus), menus, "{c:?} menus");
            assert_eq!(c.allows(Keys::Debug), debug, "{c:?} debug");
            assert_eq!(c.allows(Keys::Hud), hud, "{c:?} hud");
        }
    }
}
