//! The warp stop on a vessel (D128; `systems::ship_os::warp_stop`): the
//! risks re-read at each lattice step of its systems, a stop raised when a
//! new one stops warp by the craft's line, and its notifications.
//!
//! In a game frame ([`Vessel::advance_frame`]) the vessel stops at the
//! stop's epoch and holds there until the game takes the stop (it drops
//! the warp there, D128). Elsewhere ([`Vessel::advance`]) the stop is only
//! recorded. Stopping is the budget's stop (the coast's lattices end at an
//! epoch with all of its events done), so the flight is the same bits
//! either way.

use super::{Helm, Vessel};
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use crate::systems::ship_os::warp_stop::{self, Facts, Stop};
use crate::systems::ship_os::Software;
use crate::time::Epoch;

/// The check at lattice epoch `at`: the risks now, and a stop if a new one
/// stops warp (notifications on, no stop pending). Returns whether it
/// raised one. Over the vessel's fields, so a coast can call it while it
/// holds its trajectory.
pub(super) fn watch(
    software: &mut Software,
    helm: &Helm,
    ship: (Option<&ShipDef>, Option<&ShipState>),
    debug: bool,
    at: Epoch,
) -> bool {
    let (Some(def), Some(state)) = ship else { return false };
    let k = crate::systems::data::knowledge(def, state, debug);
    let facts = Facts::of(def, state, &k, crate::systems::crew::supply_left(def, state));
    let now = warp_stop::risks(&facts);
    let new = warp_stop::new_risks(&software.watch.seen, &now);
    software.watch.seen = now;
    let first = !std::mem::replace(&mut software.watch.primed, true);
    if first || new.is_empty() || !helm.notify() || software.watch.pending.is_some() {
        return false;
    }
    if !warp_stop::decide(software.warp_stop.as_ref(), facts.computing, &new) {
        return false;
    }
    let why = new.iter().map(|r| r.text(def)).collect::<Vec<_>>().join("; ");
    software.watch.pending = Some(Stop { at, why });
    true
}

impl Vessel {
    /// Whether a risk to this craft stops warp (its notifications, D128).
    pub fn notify(&self) -> bool {
        self.helm.notify()
    }

    pub fn set_notify(&mut self, on: bool) {
        self.helm.set_notify(on);
    }

    /// The warp stop raised and not yet taken.
    pub fn warp_stop(&self) -> Option<&Stop> {
        self.software.watch.pending.as_ref()
    }

    /// Takes the stop (the game, as it drops the warp): the vessel then
    /// advances on.
    pub fn take_warp_stop(&mut self) -> Option<Stop> {
        self.software.watch.pending.take()
    }

    /// Installs (or removes) the warp-stop program that replaces the
    /// built-in line while it runs (D126's switch in its slot).
    pub fn set_warp_line(&mut self, program: Option<warp_stop::Program>) {
        self.software.warp_stop = program;
    }

    /// The check at the vessel's time (after a live tick that crossed a
    /// lattice epoch, or on the ground).
    pub(super) fn watch_now(&mut self) -> bool {
        let def = self.ship_def();
        let at = self.time;
        watch(&mut self.software, &self.helm, (def.as_deref(), self.ship.as_ref()), self.debug, at)
    }
}
