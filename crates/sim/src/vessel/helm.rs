//! A vessel's own controls (D112): latched with the vessel and saved with
//! it, whichever vessel is active, and the controls that have reached an
//! uncrewed vessel (D034).
//!
//! A vessel's controls are set where it is flown from: aboard, by its crew
//! (the pilot), or by an operator for an uncrewed vessel (the player, the
//! player's agent, a script at the control location). A crewed vessel acts
//! on its controls at once. An uncrewed one acts on each setting after the
//! signal's one-way light delay from the operator, and on nothing new
//! without a signal (it keeps what it last received). Only changes are
//! queued: a setting equal to the last one sent is not sent again, which
//! gives the same controls at every instant with a short queue.
//!
//! The pilot's momentary inputs (rotation and translation keys) are the
//! active vessel's only: the game zeroes them on the vessel it leaves.

use super::{Controls, Vessel, VesselId};
use crate::time::Epoch;
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

impl Controls {
    /// A new vessel's controls: SAS on, holding its attitude; throttle
    /// closed.
    pub fn idle() -> Self {
        Controls { sas: true, ..Default::default() }
    }

    /// The controls a piece split off a vessel starts with (every piece
    /// but the one that keeps the vessel's id): the latched choices (SAS
    /// and its hold, the speed reference, reference body and target, the
    /// RCS mode, the chute's arming) carried over, the throttle closed, and
    /// no rotation or translation (the pilot's inputs stay with the
    /// vessel). A booster thus keeps holding the attitude it had; an engine
    /// still lit burns at its minimum until its sequence or a command cuts
    /// it.
    pub fn for_piece(&self) -> Self {
        Controls { throttle: 0.0, rotate: DVec3::ZERO, translate: DVec3::ZERO, ..*self }
    }

    /// The pilot's momentary inputs released: what a vessel keeps when the
    /// pilot leaves it for another (its latched controls unchanged).
    pub fn released(&self) -> Self {
        Controls { rotate: DVec3::ZERO, translate: DVec3::ZERO, ..*self }
    }
}

/// A vessel's controls: as set, and as received.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Helm {
    /// The controls as set (latched; the active vessel's also carry the
    /// pilot's momentary inputs).
    set: Controls,
    /// What the vessel acts on: an uncrewed vessel's last received
    /// setting, a crewed vessel's controls as set.
    received: Controls,
    /// An uncrewed vessel's settings on their way, with their arrival
    /// times, in the order sent.
    #[serde(default, skip_serializing_if = "VecDeque::is_empty")]
    queue: VecDeque<(Epoch, Controls)>,
    /// After a planned burn was handed to the pilot ([`Helm::hand_over`]):
    /// the controls the step was flying with, which read as the handed
    /// ones until the caller passes others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    taken: Option<Controls>,
}

impl Default for Helm {
    fn default() -> Self {
        Helm::new(Controls::idle())
    }
}

impl Helm {
    /// Controls `set`, already received.
    pub fn new(set: Controls) -> Self {
        Helm { set, received: set, queue: VecDeque::new(), taken: None }
    }

    /// Hands the pilot the controls `handed` (a planned burn taken over
    /// or ended by a separation, D126; `maneuver::handover`): set and
    /// received at once, as the program flew them. The caller's controls
    /// for the rest of the step (`taken`, from before) then read as these
    /// ([`Helm::current`]); its next ones carry them.
    pub(super) fn hand_over(&mut self, taken: &Controls, handed: Controls) {
        self.set = handed;
        self.received = handed;
        self.taken = Some(*taken);
    }

    /// The controls a tick flies with the caller's `controls`: after a
    /// hand-over, the handed ones (with the caller's momentary inputs)
    /// while the caller still passes those from before it.
    pub(super) fn current(&mut self, controls: &Controls) -> Controls {
        match self.taken {
            Some(t) if t.released() == controls.released() => {
                Controls { rotate: controls.rotate, translate: controls.translate, ..self.received.released() }
            }
            Some(_) => {
                self.taken = None;
                *controls
            }
            None => *controls,
        }
    }

    /// The helm a piece of a split starts with (every piece but the one
    /// that keeps the vessel's id): the controls of [`Controls::for_piece`],
    /// received at once (nothing in flight: settings sent were for the
    /// vessel).
    pub(super) fn for_piece(&self) -> Self {
        Helm::new(self.set.for_piece())
    }

    /// What the vessel acts on at `now`: `crewed`, its controls as set;
    /// otherwise those that have reached it, the setting at `now` sent with
    /// one-way `delay` (s; `None`: no signal, nothing sent).
    pub fn acting(&mut self, now: Epoch, crewed: bool, delay: Option<f64>) -> Controls {
        if crewed {
            self.queue.clear();
            self.received = self.set;
            return self.set;
        }
        if let Some(d) = delay {
            let last = self.queue.back().map_or(self.received, |q| q.1);
            if last != self.set {
                self.queue.push_back((now.add_seconds(d), self.set));
            }
        }
        while self.queue.front().is_some_and(|(t, _)| now.seconds_since(*t) >= 0.0) {
            self.received = self.queue.pop_front().expect("front").1;
        }
        self.received
    }

    /// Settings sent and not yet received.
    pub fn in_flight(&self) -> usize {
        self.queue.len()
    }
}

impl Vessel {
    /// The vessel's controls as set (D112): what its pilot, operator or a
    /// script last chose. An uncrewed vessel acts on them once they reach
    /// it ([`Vessel::acting_controls`]).
    pub fn controls(&self) -> &Controls {
        &self.helm.set
    }

    /// Sets the vessel's controls (they reach an uncrewed vessel after the
    /// light delay).
    pub fn controls_mut(&mut self) -> &mut Controls {
        &mut self.helm.set
    }

    /// The controls the vessel last acted on: for an uncrewed vessel, the
    /// last setting that reached it.
    pub fn received_controls(&self) -> &Controls {
        &self.helm.received
    }

    /// Settings on their way to the vessel (uncrewed, D034).
    pub fn controls_in_flight(&self) -> usize {
        self.helm.in_flight()
    }

    /// The controls the vessel acts on from `now` (call once per step,
    /// before advancing it): its own at once with a crew aboard; else the
    /// settings that have reached it, the current one sent now with
    /// one-way `delay` from its operator (s; `None`: no signal).
    pub fn acting_controls(&mut self, now: Epoch, delay: Option<f64>) -> Controls {
        let crewed = self.crew() > 0;
        self.helm.acting(now, crewed, delay)
    }
}

/// Every vessel's controls for a step from `now`, by fleet index (call once
/// per step, before advancing the fleet): each its own (D112), an uncrewed
/// one's as they reach it with one-way `delay(id)` from its operator (D034).
pub fn fleet_controls(fleet: &mut [Vessel], now: Epoch, delay: impl Fn(VesselId) -> Option<f64>) -> Vec<Controls> {
    fleet.iter_mut().map(|v| v.acting_controls(now, delay(v.id()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sol;

    fn full() -> Controls {
        Controls { throttle: 1.0, ..Controls::idle() }
    }

    #[test]
    fn settings_reach_an_uncrewed_vessel_after_the_delay_and_not_without_signal() {
        let t = sol::sol_epoch();
        let mut h = Helm { set: full(), ..Default::default() };
        assert_eq!(h.acting(t, false, Some(1.3)).throttle, 0.0);
        assert_eq!(h.acting(t.add_seconds(1.0), false, Some(1.3)).throttle, 0.0);
        h.set = Controls::idle();
        assert_eq!(h.acting(t.add_seconds(1.3), false, Some(1.3)).throttle, 1.0, "sent at 0");
        assert_eq!(h.acting(t.add_seconds(2.5), false, Some(1.3)).throttle, 1.0);
        assert_eq!(h.acting(t.add_seconds(2.6), false, Some(1.3)).throttle, 0.0, "closed at 1.3");
        // No signal: the vessel keeps what it last received.
        h.set = full();
        assert_eq!(h.acting(t.add_seconds(10.0), false, None).throttle, 0.0);
        assert_eq!(h.acting(t.add_seconds(100.0), false, None).throttle, 0.0, "nothing sent without signal");
        assert_eq!(h.in_flight(), 0);
    }

    #[test]
    fn only_changes_are_queued_and_the_controls_are_the_same_at_every_instant() {
        let t = sol::sol_epoch();
        // Every frame's setting queued (the old rule) against changes only.
        let settings = |k: usize| if (k / 7).is_multiple_of(2) { full() } else { Controls::idle() };
        let mut h = Helm::default();
        let mut every: VecDeque<(Epoch, Controls)> = VecDeque::new();
        let mut received = Controls::idle();
        for k in 0..100 {
            let now = t.add_seconds(0.25 * k as f64);
            h.set = settings(k);
            every.push_back((now.add_seconds(2.0), settings(k)));
            while every.front().is_some_and(|(at, _)| now.seconds_since(*at) >= 0.0) {
                received = every.pop_front().unwrap().1;
            }
            assert_eq!(h.acting(now, false, Some(2.0)), received, "frame {k}");
            assert!(h.in_flight() <= 2, "frame {k}: {} queued", h.in_flight());
        }
    }

    #[test]
    fn a_crewed_vessel_acts_at_once() {
        let t = sol::sol_epoch();
        let mut h = Helm { set: full(), ..Default::default() };
        assert_eq!(h.acting(t, true, Some(5.0)), full());
        assert_eq!(h.acting(t, true, None), full(), "no signal needed aboard");
    }

    #[test]
    fn a_piece_keeps_the_latched_choices_and_closes_the_throttle() {
        let c = Controls {
            throttle: 0.8,
            rotate: DVec3::X,
            translate: DVec3::Y,
            hold: super::super::HoldMode::Prograde,
            ..Controls::idle()
        };
        let p = c.for_piece();
        assert_eq!((p.throttle, p.rotate, p.translate), (0.0, DVec3::ZERO, DVec3::ZERO));
        assert_eq!((p.sas, p.hold, p.speed, p.rcs), (c.sas, c.hold, c.speed, c.rcs));
        let r = c.released();
        assert_eq!((r.throttle, r.rotate, r.translate, r.hold), (0.8, DVec3::ZERO, DVec3::ZERO, c.hold));
    }
}
