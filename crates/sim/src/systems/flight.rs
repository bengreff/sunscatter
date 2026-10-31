//! The flight software (F3; D106, D115, D117; ship-model.md §7): the
//! functions a craft of parts' computers run. **One owner**: everything the
//! vehicle's own software decides is a function here whose only inputs are
//! what its sensors know ([`View`], read through `Knowledge`), the
//! computers' [`Health`] and the craft's static definition (what the
//! software was written for), and whose outputs are commands: ship commands
//! ([`Commit`], [`safing`]) and what the attitude controller is allowed to
//! fly ([`Sas::acting`]). The physics never calls back into it.
//!
//! **The sensor boundary is [`sense`]**: the only place the true state
//! enters, filtered by `Knowledge`. A reading's value is the truth, known to
//! its sensor's precision (D106: no noise; D115 adds noise, drift and
//! outages only where they matter), so with every sensor working the
//! software flies exactly what it flew on the truth. Debug mode (D064) keeps
//! its truth view (`Knowledge::perfect`) and changes nothing else: the
//! computers' health still counts.
//!
//! **Functions now:**
//! - SAS and the holds ([`sas`]): attitude hold needs an attitude reference
//!   (attitude and angular rate: an IMU), a direction hold a navigation
//!   solution too (position and velocity: GNSS), and both a working flight
//!   computer; the attitude controller (`vessel::attitude`) flies the
//!   controls [`Sas::acting`] gives it, from the attitude [`sense`] gives.
//! - The pad's launch commit ([`commit`]): the clamps let go when every
//!   engine reports its rated thrust through its controller.
//! - The engine controllers' safing ([`safing`]).
//! - What the landing prediction needs ([`can_predict`]); the stage
//!   estimates read the same `Knowledge` (`systems::estimate`).
//!
//! **Plugging in** (not built): D115's adaptive functions (engine-out
//! re-trim, propellant utilisation, the navigation filter that carries the
//! state on the IMU when GNSS is lost), D117's guidance (the booster's
//! boostback, entry and landing burns; Dragon's autonomous docking) and
//! pillar 4's ship-OS programs are each a function of `(&View, &Health,
//! &ShipDef)` and their own saved state, returning ship commands and the
//! controls the attitude controller flies, run on the computer that
//! carries them (its health in [`Health::computers`]).

use super::data::{Knowledge, Quantity};
use crate::craft::ship::ShipDef;
use crate::craft::state::{EngineStatus, ShipState};
use crate::frame::NodeId;
use crate::vessel::{Attitude, Controls, HoldMode};
use glam::DVec3;

/// Whether component (or part function) `id` on `part` works: its part
/// intact, its function neither lost (a failure) nor without power, and
/// switched on.
pub fn works(def: &ShipDef, state: &ShipState, id: &str, part: u32) -> bool {
    let intact = state.parts.iter().find(|p| p.part == part).is_none_or(|p| p.integrity > 0.0);
    let switched = || {
        let i = def.components.iter().position(|c| c.id == id);
        i.is_none_or(|i| state.power.get(i).is_none_or(|s| s.on))
    };
    intact && !state.runtime.out(id) && switched()
}

/// Whether a working computer is linked to `id`: `None` when no data link
/// names it.
fn computer_for(def: &ShipDef, state: &ShipState, id: &str) -> Option<bool> {
    let mut links = def.links.iter().filter(|l| l.to.iter().any(|(to, _)| to == id)).peekable();
    links.peek()?;
    Some(links.any(|l| works(def, state, &l.from.0, l.from.1)))
}

/// Whether sensor `id`'s data reach a working computer (D106): a link from
/// one, or a craft without data links (its data paths are not modelled).
pub fn read_by_computer(def: &ShipDef, state: &ShipState, id: &str) -> bool {
    def.links.is_empty() || computer_for(def, state, id) == Some(true)
}

/// Whether actuator `id` can be commanded through the data links: a working
/// computer linked to it, or none linked at all (commanded directly).
pub fn commanded(def: &ShipDef, state: &ShipState, id: &str) -> bool {
    computer_for(def, state, id) != Some(false)
}

/// An engine's own controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controller {
    /// Its own function in: the engine's part intact, not lost or unpowered.
    pub works: bool,
    /// A computer linked to it works, or none is linked.
    pub commanded: bool,
}

/// The computers' health: the flight software's second input.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Health {
    /// Each of `ShipDef::computers`: it works ([`works`]).
    pub computers: Vec<bool>,
    /// A craft whose computers and data links are not modelled (none in its
    /// definition): its control works as if they were.
    pub ideal: bool,
    /// Each of `ShipDef::engines`: its own controller.
    pub controllers: Vec<Controller>,
}

impl Health {
    /// All working (a craft without the part system, the test craft).
    pub fn ideal() -> Health {
        Health { ideal: true, ..Health::default() }
    }

    /// Whether a flight computer works to run the software.
    pub fn computing(&self) -> bool {
        self.ideal || self.computers.iter().any(|&c| c)
    }
}

/// The computers' health for the vessel of `def` in `state`.
pub fn health(def: &ShipDef, state: &ShipState) -> Health {
    let computers = def.computers.iter().map(|c| works(def, state, &c.id, c.part)).collect();
    let controllers = (def.engines.iter())
        .map(|e| Controller { works: works(def, state, &e.owner, e.part), commanded: commanded(def, state, &e.owner) })
        .collect();
    Health { computers, ideal: def.computers.is_empty() && def.links.is_empty(), controllers }
}

/// An engine controller's readout (falcon9.md §5.2: chamber pressure and
/// the rest), as the computers receive it.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineReading {
    pub status: EngineStatus,
    /// Its flow level: a fraction of full flow (its chamber pressure over
    /// the rated one); its thrust follows at the ambient pressure.
    pub level: f64,
    /// Why it last stopped, as the controller reports it.
    pub reason: Option<String>,
}

/// What the flight software sees now (built by [`sense`]).
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub knowledge: Knowledge,
    /// The attitude and angular rate, with an attitude reference
    /// ([`attitude_reference`]).
    pub attitude: Option<Attitude>,
    /// Where the vessel is and how it moves, relative to an anchor, with a
    /// navigation solution ([`navigation`]).
    pub nav: Option<(NodeId, DVec3, DVec3)>,
    /// Each engine's controller readout, where it reaches a computer
    /// (`Quantity::EngineThrust` known).
    pub engines: Vec<Option<EngineReading>>,
}

/// Whether the vessel has an attitude reference: its attitude and angular
/// rate known (an IMU; a star tracker alone gives no rates).
pub fn attitude_reference(k: &Knowledge) -> bool {
    k.knows(Quantity::Attitude) && k.knows(Quantity::AngularRate)
}

/// Whether the vessel has a navigation solution: its position and velocity
/// known (GNSS; dead reckoning on the IMU is D115's navigation filter).
pub fn navigation(k: &Knowledge) -> bool {
    k.knows(Quantity::Position) && k.knows(Quantity::Velocity)
}

/// Each engine controller's readout reaching the computers (`None`: its
/// controller lost, or no working computer reads it).
pub fn engine_readings(def: &ShipDef, state: &ShipState, k: &Knowledge) -> Vec<Option<EngineReading>> {
    (0..def.engines.len())
        .map(|i| {
            k.knows(Quantity::EngineThrust(i as u32)).then(|| {
                let run = state.engines.get(i);
                EngineReading {
                    status: state.engines[i].status,
                    level: run.map_or(0.0, |r| r.level),
                    reason: run.and_then(|r| r.reason.clone()),
                }
            })
        })
        .collect()
}

/// The sensor boundary: what the software sees of the vessel at `attitude`
/// and `nav` (its true motion) with `ship` (a craft of parts' definition and
/// state; `None`, the test craft: everything), through `k`.
pub fn sense(
    k: Knowledge,
    (attitude, nav): (Attitude, (NodeId, DVec3, DVec3)),
    ship: Option<(&ShipDef, &ShipState)>,
) -> View {
    let engines = ship.map_or(Vec::new(), |(def, state)| engine_readings(def, state, &k));
    let attitude = attitude_reference(&k).then_some(attitude);
    let nav = navigation(&k).then_some(nav);
    View { knowledge: k, attitude, nav, engines }
}

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

/// The pad's launch commit for the engines the armed release waits for.
#[derive(Clone, Debug, PartialEq)]
pub enum Commit {
    /// Not all at their rated thrust yet.
    Wait,
    /// Every one at its rated thrust: let the clamps go.
    Release,
    /// An engine reports it stopped, or reports nothing: shut every engine
    /// down and keep the clamps.
    Abort { engine: usize, why: String },
}

/// The pad's launch commit (falcon9.md §1.2, §4.2): the clamps let go at
/// the first tick every engine in `waits` reports, through its controller,
/// that it runs at its rated level; one whose controller reports it stopped,
/// or whose readout reaches no computer (a lost controller, a dead stage
/// computer), aborts the launch.
pub fn commit(def: &ShipDef, engines: &[Option<EngineReading>], waits: &[usize]) -> Commit {
    for &i in waits {
        match engines.get(i).and_then(Option::as_ref) {
            None => return Commit::Abort { engine: i, why: "no thrust reading".into() },
            Some(r) if !matches!(r.status, EngineStatus::Starting | EngineStatus::Running) => {
                let why = r.reason.clone().unwrap_or_else(|| "stopped".into());
                return Commit::Abort { engine: i, why };
            }
            Some(_) => {}
        }
    }
    let rated = |i: usize| {
        engines[i].as_ref().is_some_and(|r| {
            r.status == EngineStatus::Running && r.level >= def.engines[i].data.throttle_max * (1.0 - 1e-9)
        })
    };
    if waits.iter().all(|&i| rated(i)) {
        Commit::Release
    } else {
        Commit::Wait
    }
}

/// The engine controllers' safing (E: the class default; falcon9.md has no
/// source for the Merlin's, and the Shuttle's SSME held its last command
/// instead): an engine lit (`lit[i]`: starting, running or waiting for its
/// propellant to settle) whose controller works but whose every linked
/// computer is lost or unpowered is shut down by its controller (its normal
/// tail-off). Returns those engines.
pub fn safing(health: &Health, lit: &[bool]) -> Vec<usize> {
    (health.controllers.iter().zip(lit).enumerate())
        .filter(|(_, (c, &lit))| lit && c.works && !c.commanded)
        .map(|(i, _)| i)
        .collect()
}

/// Whether every feed tank of `engines` holds its liquid over its outlet,
/// as the level sensors read it (`propulsion::feeds_settled` where every
/// one of those tanks' levels is known); `None` when one is not read.
pub fn feeds_read(def: &ShipDef, state: &ShipState, k: &Knowledge, engines: &[u32]) -> Option<bool> {
    let mut tanks = engines.iter().filter_map(|&i| def.engines.get(i as usize)).flat_map(|e| e.feeds.concat());
    if !tanks.all(|t| k.knows(Quantity::TankLevel(t))) {
        return None;
    }
    Some(engines.iter().all(|&i| super::propulsion::feeds_settled(def, state, i as usize)))
}

/// Whether the landing prediction can run: a navigation solution, and the
/// attitude too when it assumes the current attitude held (`inertial`).
pub fn can_predict(k: &Knowledge, inertial: bool) -> bool {
    navigation(k) && (!inertial || k.knows(Quantity::Attitude))
}

#[cfg(test)]
#[path = "flight_tests.rs"]
mod tests;
