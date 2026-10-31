//! The program API (D043, D128 layer 2, D129; software-layers.md §6): a
//! program reads what the computer knows and returns the commands a player
//! issues, at fixed epochs, with its state saved with the vessel.
//!
//! - **In:** a [`Context`]: the epoch, the [`View`] (`Knowledge` through
//!   `sense`), the computers' [`Health`], the craft's definition, and the
//!   ship-state quantities `Knowledge` knows ([`Context::read`]: the truth
//!   at its precision, D106; nothing it does not know).
//! - **Out:** [`Commands`]: the controls a flying program lays over the
//!   pilot's (the maneuver program's), ignitions, ship commands through the
//!   native boundary (`systems::command::apply`), and the next epoch it
//!   asks to wake at.
//! - **The schedule** (D129, [`Schedule`]): its period from its own epoch,
//!   or the wake epochs it asks for. Each due epoch stops the vessel's
//!   advance as an order does, so it acts at the same epoch at every warp.
//! - **The switch** (D126, [`Slot`]): by itself, when the player presses
//!   for it (one run per press), or not at all.
//! - **No computer, no program:** a due epoch without a working flight
//!   computer is missed (`Health::computing`).
//!
//! The programs are native for now ([`Builtin`]); WASM modules with the
//! same signature later (D043). The maneuver program (`systems::maneuver`)
//! keeps its own inputs and flies live ticks; its switch is a [`Slot`] and
//! its wake epoch stops the advance the same way.

use super::{Health, Switch, View};
use crate::craft::command::ShipCommand;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use crate::systems::data::{self, Quantity, Reading};
use crate::time::Epoch;
use crate::vessel::{Controls, HoldMode};
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// D126's switch for one program, and the player's press.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    #[serde(default)]
    pub switch: Switch,
    /// Pressed for and not yet run (`Switch::Press`).
    #[serde(default)]
    pub pressed: bool,
}

impl Slot {
    /// Running by itself.
    pub fn auto() -> Slot {
        Slot { switch: Switch::Auto, pressed: false }
    }

    /// Whether the program may run at its next due epoch.
    pub fn may_run(&self) -> bool {
        match self.switch {
            Switch::Auto => true,
            Switch::Press => self.pressed,
            Switch::Off => false,
        }
    }
}

/// When a program runs (D129): epochs fixed in absolute time.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Schedule {
    /// Every `period` seconds from `from`.
    Every { from: Epoch, period: f64 },
    /// Once, at a wake epoch (the program asks for the next one).
    At(Epoch),
}

impl Schedule {
    /// The first due epoch after the last run `last` (`None`: never run):
    /// the period's first epoch strictly after it, or the wake epoch if it
    /// has not run since. `None`: nothing due.
    pub fn next(&self, last: Option<Epoch>) -> Option<Epoch> {
        match *self {
            Schedule::At(t) => last.is_none_or(|l| l < t).then_some(t),
            Schedule::Every { from, period } => {
                if period.is_nan() || period <= 0.0 {
                    return None;
                }
                let Some(l) = last.filter(|&l| l >= from) else { return Some(from) };
                let k = (l.seconds_since(from) / period).floor();
                // The rounding of each epoch's seconds is the epoch's own:
                // the first of the neighbours strictly after `l`.
                let mut k = (k - 1.0).max(0.0);
                loop {
                    let e = from.add_seconds(k * period);
                    if e > l {
                        return Some(e);
                    }
                    k += 1.0;
                }
            }
        }
    }
}

/// What a program reads at one of its epochs.
#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    pub t: Epoch,
    pub view: &'a View,
    pub health: &'a Health,
    pub def: &'a ShipDef,
    /// Read only through [`Context::read`] (what `Knowledge` knows).
    state: &'a ShipState,
}

impl<'a> Context<'a> {
    pub fn new(t: Epoch, view: &'a View, health: &'a Health, def: &'a ShipDef, state: &'a ShipState) -> Self {
        Context { t, view, health, def, state }
    }

    /// The reading of a ship-state quantity (`systems::data::truth`), if
    /// the computer knows it.
    pub fn read(&self, q: Quantity) -> Option<Reading> {
        self.view.knowledge.read(q, data::truth(self.state, q)?)
    }
}

/// What a program commands: what a player would set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commands {
    /// SAS on with the maneuver hold along this (inertial, unit).
    pub hold: Option<DVec3>,
    pub throttle: Option<f64>,
    /// Translation in body axes.
    pub translate: Option<DVec3>,
    /// Engines to ignite now.
    pub ignite: Vec<u32>,
    /// Ship commands now, through the native boundary.
    pub ship: Vec<ShipCommand>,
    /// The next epoch it asks to run at ([`Schedule::At`]).
    pub wake: Option<Epoch>,
}

impl Commands {
    /// The pilot's controls with these laid over them.
    pub fn over(&self, pilot: &Controls) -> Controls {
        let mut c = *pilot;
        if self.hold.is_some() {
            c.sas = true;
            c.hold = HoldMode::Maneuver;
        }
        if let Some(t) = self.throttle {
            c.throttle = t;
        }
        if let Some(t) = self.translate {
            c.translate = t;
        }
        c
    }
}

/// The built-in programs (native until D043's WASM).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Builtin {
    PowerGuard(super::power_guard::PowerGuard),
}

/// A program on a vessel's computer: its switch, schedule and state
/// (saved with the vessel).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Installed {
    pub name: String,
    pub slot: Slot,
    pub schedule: Schedule,
    /// When it last ran, missed its epoch, or was resumed ([`Self::resume`]).
    #[serde(default)]
    pub last: Option<Epoch>,
    pub program: Builtin,
}

impl Installed {
    /// Its next due epoch, while its switch lets it run.
    pub fn due(&self) -> Option<Epoch> {
        self.slot.may_run().then(|| self.schedule.next(self.last)).flatten()
    }

    /// Switched on or pressed for at `now` (the player's input, D129): a
    /// period whose epochs passed while it could not run starts again at
    /// its first epoch after `now`, not at once.
    pub fn resume(&mut self, now: Epoch) {
        let behind =
            matches!(self.schedule, Schedule::Every { .. }) && self.schedule.next(self.last).is_some_and(|e| e < now);
        if behind {
            self.last = Some(now);
        }
    }

    /// Runs it at `cx.t` (one of its due epochs): its press is spent, and
    /// a wake it asks for becomes its schedule. Without a working computer
    /// it misses the epoch.
    pub fn run(&mut self, cx: &Context) -> Commands {
        self.last = Some(cx.t);
        self.slot.pressed = false;
        if !cx.health.computing() {
            return Commands::default();
        }
        let c = match &mut self.program {
            Builtin::PowerGuard(g) => g.step(cx),
        };
        if let Some(w) = c.wake {
            self.schedule = Schedule::At(w);
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The period's epochs from its own, strictly after the last run; a
    /// wake epoch once; a press runs it once; off never.
    #[test]
    fn a_schedule_gives_its_epochs_and_the_switch_gates_them() {
        let t0 = Epoch::new(100_000_000, 0.0);
        let at = |s: f64| t0.add_seconds(s);
        let every = Schedule::Every { from: at(10.0), period: 60.0 };
        // (last run, next due)
        let cases = [
            (None, Some(at(10.0))),
            (Some(at(5.0)), Some(at(10.0))),
            (Some(at(10.0)), Some(at(70.0))),
            (Some(at(69.999)), Some(at(70.0))),
            (Some(at(70.0)), Some(at(130.0))),
            (Some(at(1.0e6 * 60.0 + 10.0)), Some(at(1.0e6 * 60.0 + 70.0))),
        ];
        for (last, next) in cases {
            assert_eq!(every.next(last), next, "{last:?}");
        }
        // An epoch the schedule gave is strictly after itself next time.
        let mut last = None;
        for _ in 0..1000 {
            let e = every.next(last).unwrap();
            assert!(last.is_none_or(|l| e > l));
            last = Some(e);
        }
        assert_eq!(Schedule::At(at(5.0)).next(None), Some(at(5.0)));
        assert_eq!(Schedule::At(at(5.0)).next(Some(at(5.0))), None);
        assert_eq!(Schedule::Every { from: t0, period: 0.0 }.next(None), None);

        let slot = |switch, pressed| Slot { switch, pressed };
        assert!(slot(Switch::Auto, false).may_run());
        assert!(!slot(Switch::Press, false).may_run() && slot(Switch::Press, true).may_run());
        assert!(!slot(Switch::Off, true).may_run());
    }
}
