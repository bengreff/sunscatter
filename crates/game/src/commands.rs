//! The command API (review game 7): every discrete change to the
//! simulation (warp, switching or deleting vessels, loading, reverting,
//! resetting) is a `GameCommand` message. UI and key systems only write
//! commands; `apply` runs them at the start of the next frame's simulation,
//! so a frame is never drawn with a new fleet and an old camera. The same
//! messages are the hook for the MCP server and scripting (D063), which will
//! add an origin (a control location, D067).
//!
//! Continuous flight input (throttle, rotation) goes straight to the active
//! vessel's controls (`SimState::controls_mut`), written by
//! `keys::read_controls` in the input stage. A script sets any vessel's
//! with [`GameCommand::SetControls`].
//!
//! `InputContext` says which keys act this frame (review game 2): one rule,
//! read by every key system; [`keys`] which keys the game takes.

pub mod keys;

use crate::camera::{CameraRig, Focus};
use crate::state::{Prediction, SimState, WARP_LEVELS};
use crate::tracking::{self, Tracked, TrackingStation};
use bevy::prelude::*;
use sim::craft::command::{ShipCommand, Stage};
use sim::save::SaveGame;
use sim::systems::OrderKind;
use sim::time::Epoch;
use sim::vessel::{Controls, HoldMode, VesselId};

#[derive(Message, Clone, Debug, PartialEq)]
pub enum GameCommand {
    /// Requested warp level (index into `WARP_LEVELS`); resumes stopped
    /// time.
    SetWarp(usize),
    /// Stops time (D111: `,` at 1×); a warp command resumes it.
    PauseTime,
    /// Make a vessel active.
    Switch(VesselId),
    /// Delete a non-active vessel.
    Delete(VesselId),
    /// Replace the game with a (checked, non-empty) save: load or revert.
    Restore(Box<SaveGame>),
    /// Put the active vessel back on the pad (a new craft of its kind):
    /// the revert of a vessel without a launch save, and the fresh vessel
    /// after a pad abort (D114, `pad`).
    Reset,
    /// A new flight: the active vessel replaced by a new craft `.0` (a
    /// directory under `data/craft`) on the pad.
    NewFlight(String),
    /// Debug mode (D064) on every vessel: infinite propellant, no
    /// overheating, infinite impact tolerance.
    SetDebug(bool),
    /// Replace a craft of parts' planned burns (D126: nodes; a
    /// vessel-directed command: from anywhere but aboard it, it travels
    /// with the signal's light delay).
    SetNodes {
        vessel: VesselId,
        nodes: Vec<sim::systems::maneuver::Node>,
    },
    /// The active vessel's attitude hold (D075); turns SAS on.
    SetHold(HoldMode),
    /// The active vessel's throttle, 0–1 (the navball's throttle bar).
    SetThrottle(f64),
    /// SAS on or off (the navball's SAS button; T toggles it directly).
    SetSas(bool),
    /// The pilot's RCS mode (D111: on, translation or rotation only, fine).
    SetRcs(sim::systems::rcs::RcsMode),
    /// Any vessel's latched controls (D112: a script flies an inactive
    /// vessel; `None` leaves a setting as it is). Sent to the vessel
    /// ([`controls_route`]).
    SetControls {
        vessel: VesselId,
        throttle: Option<f64>,
        sas: Option<bool>,
        hold: Option<HoldMode>,
        /// What prograde and retrograde follow (the navball's speed display).
        speed: Option<sim::vessel::SpeedReference>,
    },
    /// Activate the active vessel's next stage (Space, the Staging menu);
    /// for a craft of parts it becomes [`GameCommand::Stage`] of that
    /// vessel, sent to it.
    NextStage,
    /// Run a craft of parts' next stage (its sequence), sent to it.
    Stage(VesselId),
    /// Run stage `stage` of a craft of parts' sequence again, one already
    /// run (D114: the staging bar), sent to it.
    StageAgain {
        vessel: VesselId,
        stage: u32,
    },
    /// Run a craft of parts' saved function by name (the action-group bar), sent
    /// to it.
    Function {
        vessel: VesselId,
        name: String,
    },
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
    mut systems: ResMut<crate::systems_screen::SystemsScreen>,
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
            let delay = match route_to(&c, id, &sim, &comms) {
                Route::Now => 0.0,
                Route::After(delay) => delay,
                Route::NoSignal => {
                    toasts.push_linked(now, "No signal: the command cannot reach it", true, to(id));
                    continue;
                }
            };
            if delay >= 1.0 {
                let text = format!("Command sent ({})", crate::format::delay(delay));
                toasts.push_linked(now, text, false, to(id));
            }
            // Orders carry their epoch: the vessel applies them at its tick
            // (`Vessel::order_at`), whatever the frames.
            if let Some(kinds) = orders(&c) {
                place(&mut sim, &mut toasts, now, id, kinds, delay);
                continue;
            }
            if delay > 0.0 {
                in_flight.send(sim.clock, delay, c);
                continue;
            }
        }
        match &c {
            GameCommand::SetWarp(level) => {
                sim.warp = (*level).min(WARP_LEVELS.len() - 1);
                sim.paused = false;
            }
            GameCommand::PauseTime => sim.paused = true,
            GameCommand::Switch(i) => {
                let left = sim.active;
                tracking::switch_to(&mut sim, &mut rig, &mut pred, *i);
                if let Some(text) = (sim.active != left).then(|| tracking::left_lit_notice(&sim.fleet[left])).flatten()
                {
                    toasts.push(now, text, true);
                }
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
                // Back on the screen it was saved on (D114).
                systems.open = save.screen == sim::save::Screen::Systems;
                ts.open = save.screen == sim::save::Screen::TrackingStation;
            }
            GameCommand::Reset => {
                sim.reset();
                rig.focus = Focus::Ship;
            }
            GameCommand::NewFlight(craft) => {
                sim.new_flight(&crate::state::craft_or_test(craft));
                rig.focus = Focus::Ship;
            }
            GameCommand::Ship { vessel, .. }
            | GameCommand::Stage(vessel)
            | GameCommand::StageAgain { vessel, .. }
            | GameCommand::Function { vessel, .. } => {
                let kinds = orders(&c).unwrap_or_default();
                place(&mut sim, &mut toasts, now, *vessel, kinds, 0.0);
            }
            GameCommand::SetStages { vessel, stages } => {
                let done = sim.index_of(*vessel).map(|i| sim.fleet[i].set_stages(stages.clone()));
                edited(&mut toasts, time.elapsed_secs_f64(), *vessel, "Stages", done);
            }
            GameCommand::SetFunctions { vessel, functions } => {
                let done = sim.index_of(*vessel).map(|i| sim.fleet[i].set_functions(functions.clone()));
                edited(&mut toasts, time.elapsed_secs_f64(), *vessel, "Functions", done);
            }
            GameCommand::SetNodes { vessel, nodes } => {
                let done = sim.index_of(*vessel).map(|i| sim.fleet[i].set_nodes(nodes.clone()));
                planned(&mut toasts, time.elapsed_secs_f64(), *vessel, done);
            }
            GameCommand::SetHold(mode) => {
                sim.controls_mut().hold = *mode;
                sim.controls_mut().sas = true;
            }
            GameCommand::SetThrottle(t) => sim.controls_mut().throttle = t.clamp(0.0, 1.0),
            GameCommand::SetSas(on) => sim.controls_mut().sas = *on,
            GameCommand::SetRcs(mode) => sim.controls_mut().rcs = *mode,
            GameCommand::SetControls { vessel, throttle, sas, hold, speed } => match sim.index_of(*vessel) {
                Some(i) => apply_controls_with(sim.fleet[i].controls_mut(), (*throttle, *sas, *hold), *speed),
                None => toasts.push(now, format!("Controls for Vessel {}: no such vessel", vessel.0), true),
            },
            GameCommand::NextStage => {
                let mut controls = *sim.controls();
                let ship = sim.ship();
                let inputs = crate::staging::StageInputs::of(ship, sim.controls());
                let done = crate::staging::activate_next(&inputs, crate::staging::grounded(ship), &mut controls);
                *sim.controls_mut() = controls;
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

/// How command `c` reaches vessel `id` from the control location: a change
/// of controls by [`controls_route`], anything else by [`route`].
fn route_to(c: &GameCommand, id: VesselId, sim: &SimState, comms: &crate::comms::Comms) -> Route {
    let aboard = comms.location == crate::comms::Location::Vessel(id);
    let signal = comms.signal(id).map(|s| s.delay);
    match c {
        GameCommand::SetControls { .. } => {
            controls_route(aboard, sim.index_of(id).is_some_and(|i| sim.fleet[i].crew() > 0), signal)
        }
        _ => route(aboard, signal),
    }
}

/// How a change of a vessel's controls reaches it ([`GameCommand::SetControls`]).
/// An uncrewed vessel's helm carries the light delay itself (D034,
/// `sim::vessel::helm`): the change is set at once with a signal (never
/// delayed twice) and not at all without. A crewed vessel's crew acts on
/// its controls at once, so the change travels to them like any command
/// (D067, [`route`]).
pub fn controls_route(aboard: bool, crewed: bool, delay: Option<f64>) -> Route {
    match (crewed, delay) {
        (true, _) => route(aboard, delay),
        (false, Some(_)) => Route::Now,
        (false, None) => Route::NoSignal,
    }
}

/// A change of controls (`None`: left as it is): the throttle clamped to
/// 0–1, then SAS, then the hold (choosing a hold turns SAS on, as the
/// navball's buttons do).
pub fn apply_controls(c: &mut Controls, throttle: Option<f64>, sas: Option<bool>, hold: Option<HoldMode>) {
    if let Some(t) = throttle {
        c.throttle = t.clamp(0.0, 1.0);
    }
    if let Some(s) = sas {
        c.sas = s;
    }
    if let Some(h) = hold {
        (c.hold, c.sas) = (h, true);
    }
}

/// [`apply_controls`], and the speed reference prograde and retrograde follow.
fn apply_controls_with(
    c: &mut Controls,
    (throttle, sas, hold): (Option<f64>, Option<bool>, Option<HoldMode>),
    speed: Option<sim::vessel::SpeedReference>,
) {
    apply_controls(c, throttle, sas, hold);
    c.speed = speed.unwrap_or(c.speed);
}

/// The vessel a command travels to: a craft of parts' commands, its next
/// stage, its edited stages and functions, a script's controls (flight
/// plans are routed by the planner).
pub fn addressee(c: &GameCommand) -> Option<VesselId> {
    match c {
        GameCommand::Ship { vessel, .. }
        | GameCommand::Function { vessel, .. }
        | GameCommand::StageAgain { vessel, .. }
        | GameCommand::SetStages { vessel, .. }
        | GameCommand::SetFunctions { vessel, .. } => Some(*vessel),
        GameCommand::Stage(id) | GameCommand::SetControls { vessel: id, .. } => Some(*id),
        _ => None,
    }
}

/// The orders a command gives a craft of parts (`None`: not an order).
pub fn orders(c: &GameCommand) -> Option<Vec<OrderKind>> {
    match c {
        GameCommand::Ship { commands, .. } => Some(commands.iter().cloned().map(OrderKind::Command).collect()),
        GameCommand::Stage(_) => Some(vec![OrderKind::Stage]),
        GameCommand::StageAgain { stage, .. } => Some(vec![OrderKind::Again(*stage)]),
        GameCommand::Function { name, .. } => Some(vec![OrderKind::Function(name.clone())]),
        _ => None,
    }
}

/// Gives vessel `id` the orders `kinds` for `delay` seconds from now (its
/// systems apply them at the tick; the events they log are toasted by
/// `interface::events`). From aboard the active vessel, an order that arms
/// the parachutes also gives the parachute command (D079: the pilot's).
fn place(
    sim: &mut SimState,
    toasts: &mut crate::interface::toasts::Toasts,
    now: f64,
    id: VesselId,
    kinds: Vec<OrderKind>,
    delay: f64,
) {
    let Some(i) = sim.index_of(id) else {
        toasts.push(now, format!("Vessel {}: no such vessel", id.0), true);
        return;
    };
    let at = sim.clock.add_seconds(delay);
    let def = sim.fleet[i].ship_def();
    let arms = (def.as_deref().zip(sim.fleet[i].ship()))
        .is_some_and(|(def, state)| crate::staging::arms_chutes(def, state, &kinds));
    for kind in kinds {
        let none_left = (def.as_deref().zip(sim.fleet[i].ship()))
            .is_some_and(|(def, state)| crate::staging::no_stage_left(def, state));
        if matches!(kind, OrderKind::Stage) && none_left {
            let message = crate::staging::Refusal::NoStageLeft.message();
            toasts.push_linked(now, message, false, to(id));
            continue;
        }
        if let Err(e) = sim.fleet[i].order_at(at, kind) {
            toasts.push_linked(now, e.to_string(), true, to(id));
        }
    }
    if arms && i == sim.active && delay == 0.0 {
        sim.controls_mut().chute = true;
    }
}

/// The toast after an edit of `what` (stages or functions) of `vessel`:
/// saved, or refused with the reasons (`None`: no such vessel).
/// The toast of a plan or nodes set on `vessel` (`None`: no such vessel).
fn planned(
    toasts: &mut crate::interface::toasts::Toasts,
    now: f64,
    vessel: VesselId,
    done: Option<Result<(), sim::vessel::PlanError>>,
) {
    match done {
        Some(Ok(())) => toasts.push_linked(now, "Flight plan accepted", false, to(vessel)),
        Some(Err(e)) => toasts.push_linked(now, format!("Flight plan rejected: {e:?}"), true, to(vessel)),
        None => toasts.push(now, format!("Plan for Vessel {}: no such vessel", vessel.0), true),
    }
}

fn edited(
    toasts: &mut crate::interface::toasts::Toasts,
    now: f64,
    vessel: VesselId,
    what: &str,
    done: Option<Result<(), Vec<String>>>,
) {
    match done {
        Some(Ok(())) => toasts.push_linked(now, format!("{what} saved"), false, to(vessel)),
        Some(Err(e)) => toasts.push_linked(now, format!("{what} refused: {}", e.join("; ")), true, to(vessel)),
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
    /// Throttle, rotation, translation, SAS, RCS, chute, staging, vessel
    /// switching, the camera's focus.
    Flight,
    /// Time warp.
    Warp,
    /// Quicksave, quickload, the save list, the tracking station, settings.
    Menus,
    /// The flight view's task menus (1–6): also while paused.
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
            Keys::Flight => self == C::Flight,
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

/// A notification's link to vessel `id`.
fn to(id: VesselId) -> Option<crate::interface::toasts::Link> {
    Some(crate::interface::toasts::Link::Vessel(id))
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
    fn a_change_of_controls_sets_only_what_it_names() {
        let start = Controls { throttle: 0.4, sas: false, ..Controls::idle() };
        // (throttle, sas, hold) → (throttle, sas, hold)
        let table = [
            ((None, None, None), (0.4, false, HoldMode::Stability)),
            ((Some(1.5), None, None), (1.0, false, HoldMode::Stability)),
            ((Some(-1.0), Some(true), None), (0.0, true, HoldMode::Stability)),
            ((None, Some(false), Some(HoldMode::Prograde)), (0.4, true, HoldMode::Prograde)),
        ];
        for (i, ((t, s, h), want)) in table.into_iter().enumerate() {
            let mut c = start;
            apply_controls(&mut c, t, s, h);
            assert_eq!((c.throttle, c.sas, c.hold), want, "case {i}");
        }
        let v = VesselId(3);
        let c = GameCommand::SetControls { vessel: v, throttle: Some(1.0), sas: None, hold: None, speed: None };
        assert_eq!(addressee(&c), Some(v), "sent to its vessel");
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
        // Controls (aboard, crewed, delay, route): an uncrewed vessel's helm
        // carries the delay, so its changes are set now (D034).
        let table = [
            (true, true, Some(0.0), Route::Now),
            (false, true, Some(1.3), Route::After(1.3)),
            (false, true, None, Route::NoSignal),
            (false, false, Some(1.3), Route::Now),
            (false, false, Some(0.0), Route::Now),
            (false, false, None, Route::NoSignal),
        ];
        for (aboard, crewed, delay, want) in table {
            assert_eq!(controls_route(aboard, crewed, delay), want, "{aboard} {crewed} {delay:?}");
        }
        let v = VesselId(7);
        assert_eq!(addressee(&GameCommand::Ship { vessel: v, commands: vec![] }), Some(v));
        assert_eq!(addressee(&GameCommand::Stage(v)), Some(v));
        assert_eq!(addressee(&GameCommand::Function { vessel: v, name: "Legs".into() }), Some(v));
        // Ship commands, a stage and a function are orders the vessel applies at its tick.
        let ship = GameCommand::Ship { vessel: v, commands: vec![ShipCommand::ArmChutes, ShipCommand::ReleaseClamps] };
        assert_eq!(
            orders(&ship),
            Some(vec![OrderKind::Command(ShipCommand::ArmChutes), OrderKind::Command(ShipCommand::ReleaseClamps)])
        );
        assert_eq!(orders(&GameCommand::Stage(v)), Some(vec![OrderKind::Stage]));
        assert_eq!(orders(&GameCommand::SetStages { vessel: v, stages: vec![] }), None, "applied as sent");
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
        // (context, flight, warp, menus, hud)
        let table = [
            (C::Flight, true, true, true, true),
            (C::Station, false, true, true, false),
            (C::Systems, false, true, true, false),
            (C::Paused, false, false, true, true),
            (C::TextEntry, false, false, false, false),
        ];
        for (c, flight, warp, menus, hud) in table {
            assert_eq!(c.allows(Keys::Flight), flight, "{c:?} flight");
            assert_eq!(c.allows(Keys::Warp), warp, "{c:?} warp");
            assert_eq!(c.allows(Keys::Menus), menus, "{c:?} menus");
            assert_eq!(c.allows(Keys::Hud), hud, "{c:?} hud");
        }
    }
}
