//! SAS and the holds (layer 2, D128): whether SAS can hold, from what the
//! sensors know and the computers' health, and the controls the attitude
//! controller (`vessel::attitude`) acts on.

use super::{attitude_reference, navigation, Health};
use crate::systems::data::Knowledge;
use crate::vessel::{Controls, HoldMode};
use glam::DVec3;

/// What SAS can do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sas {
    /// Switched off by the pilot.
    Off,
    /// Holding as asked.
    Holding,
    /// Holding the attitude: the asked direction needs a navigation
    /// solution.
    NoNavigation,
    /// No attitude reference: SAS cannot hold (the pilot's rotation still
    /// reaches the actuators through the computer).
    NoAttitude,
    /// No working flight computer: nothing commands the attitude actuators.
    NoComputer,
}

impl Sas {
    /// How the interface says it.
    pub fn text(self) -> &'static str {
        match self {
            Sas::Off => "off",
            Sas::Holding => "holding",
            Sas::NoNavigation => "no navigation solution: holding attitude",
            Sas::NoAttitude => "no attitude reference",
            Sas::NoComputer => "no flight computer",
        }
    }

    /// Whether it cannot do what the pilot asked (SAS on, or no computer
    /// for the pilot's rotation either).
    pub fn degraded(self) -> bool {
        matches!(self, Sas::NoNavigation | Sas::NoAttitude | Sas::NoComputer)
    }

    /// The controls the attitude controller acts on, from the pilot's.
    pub fn acting(self, c: &Controls) -> Controls {
        match self {
            Sas::Off | Sas::Holding => *c,
            Sas::NoNavigation => Controls { hold: HoldMode::Stability, ..*c },
            Sas::NoAttitude => Controls { sas: false, ..*c },
            Sas::NoComputer => Controls { sas: false, rotate: DVec3::ZERO, ..*c },
        }
    }
}

/// SAS and the holds with the pilot's `controls`, from what the sensors
/// know and the computers' health (see the module).
pub fn sas(k: &Knowledge, health: &Health, controls: &Controls) -> Sas {
    if !health.computing() {
        return Sas::NoComputer;
    }
    if !controls.sas {
        return Sas::Off;
    }
    if !attitude_reference(k) {
        return Sas::NoAttitude;
    }
    if controls.hold != HoldMode::Stability && !navigation(k) {
        return Sas::NoNavigation;
    }
    Sas::Holding
}
