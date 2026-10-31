//! The ship computer: layer 2 of D128's three (D043's ship OS; F3; D106,
//! D115, D117; ship-model.md §7, software-layers.md). **One owner**:
//! everything the vehicle's own software decides is a function here whose
//! only inputs are what its sensors know ([`View`], read through
//! `Knowledge`), the computers' [`Health`] and the craft's static
//! definition (what the software was written for), and whose outputs are
//! the commands a player issues ([`commit`]) and what the attitude
//! controller is allowed to fly ([`Sas::acting`]). The physics and the
//! parts' own logic (layer 1: `propulsion`'s engine controllers, `plumbing`,
//! `power`, `crew`, `mechanisms`) never call into it; both read the data
//! links through [`super::links`].
//!
//! **The sensor boundary is [`sense`]**: the only place the true state
//! enters, filtered by `Knowledge`. A reading's value is the truth, known to
//! its sensor's precision (D106: no noise; D115 adds noise, drift and
//! outages only where they matter), so with every sensor working the
//! software flies exactly what it flew on the truth. Debug mode (D064) keeps
//! its truth view (`Knowledge::perfect`) and changes nothing else: the
//! computers' health still counts.
//!
//! **Its functions:**
//! - SAS and the holds ([`sas`]): attitude hold needs an attitude reference
//!   (attitude and angular rate: an IMU), a direction hold a navigation
//!   solution too (position and velocity: GNSS), and both a working flight
//!   computer; the attitude controller (`vessel::attitude`) flies the
//!   controls [`Sas::acting`] gives it, from the attitude [`sense`] gives.
//! - The pad's launch commit ([`commit`]): the armed release lets the
//!   clamps go when every engine reports its rated thrust through its
//!   controller.
//! - The stage sequence and the functions ([`sequence`]): Space, AGAIN and
//!   a saved function, run through the native boundary.
//! - The settling push before a start ([`ullage`]): an ignition on floating
//!   propellant waits while the thrusters along its thrust settle it.
//! - The parachutes' deploy ([`chutes`]): arming, on the press or by
//!   itself below its height, and the main's opening by the altitude
//!   reading (D126's switch, [`Switch`]).
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

pub mod chutes;
pub mod commit;
pub mod sas;
pub mod sequence;
pub mod ullage;

pub use commit::{commit, Commit};
pub use sas::{sas, Sas};

use super::data::{Knowledge, Quantity};
use super::links::{self, Controller};
use crate::craft::ship::ShipDef;
use crate::craft::state::{EngineStatus, ShipState};
use crate::frame::NodeId;
use crate::vessel::Attitude;
use glam::DVec3;

/// D126's switch for an automatic sequence: it runs by itself, when the
/// player presses for it, or not at all (manual control stays possible in
/// every position).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Switch {
    Auto,
    #[default]
    Press,
    Off,
}

/// The ship computer's software on a vessel and its saved state (D128
/// layer 2; saved with the vessel, copied to each piece of a split).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Software {
    /// The parachutes' deploy.
    #[serde(default)]
    pub chutes: chutes::Chutes,
}

/// The computers' health: the flight software's second input.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Health {
    /// Each of `ShipDef::computers`: it works ([`links::works`]).
    pub computers: Vec<bool>,
    /// A craft whose computers and data links are not modelled (none in its
    /// definition): its control works as if they were.
    pub ideal: bool,
    /// Each of `ShipDef::engines`: its own controller (its links).
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
    let computers = def.computers.iter().map(|c| links::works(def, state, c.owner)).collect();
    let controllers = (0..def.engines.len()).map(|i| links::controller(def, state, i)).collect();
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

/// Whether every feed tank of `engines` holds its liquid over its outlet,
/// as the level sensors read it (`propulsion::feeds_settled` where every
/// one of those tanks' levels is known); `None` when one is not read.
pub fn feeds_read(def: &ShipDef, state: &ShipState, k: &Knowledge, engines: &[u32]) -> Option<bool> {
    let mut tanks = engines.iter().filter_map(|&i| def.engines.get(i as usize)).flat_map(|e| e.feeds.concat());
    if !tanks.all(|t| k.knows(Quantity::TankLevel(t))) {
        return None;
    }
    Some(engines.iter().all(|&i| crate::systems::propulsion::feeds_settled(def, state, i as usize)))
}

/// Whether the landing prediction can run: a navigation solution, and the
/// attitude too when it assumes the current attitude held (`inertial`).
pub fn can_predict(k: &Knowledge, inertial: bool) -> bool {
    navigation(k) && (!inertial || k.knows(Quantity::Attitude))
}

#[cfg(test)]
mod tests;
