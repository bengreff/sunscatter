//! What stops time warp (D128; software-layers.md §4, §5).
//!
//! - **The built-in line** ([`bare_minimum`]): a risk to the crew or the
//!   vessel that was not there at the last check stops warp at its own
//!   epoch, with the craft's state at that epoch. It needs no software:
//!   the parts report their own state (a life support out, a part lost, a
//!   computer down, a load shed) and the cabin's air comes through its
//!   sensors (`Knowledge`, D106: a cabin without a working air monitor
//!   stops nothing on its air).
//! - **A program's line** ([`Line`], D126's switch in
//!   `Software::warp_stop`): while it runs on a working computer it
//!   decides instead; switched off, or its computer lost, the built-in
//!   line returns, so the fail-safe stays the parts'.
//! - **Notifications off** (`Helm::notify`, per craft, saved): nothing on
//!   that craft stops warp; its events are still logged.
//!
//! The risks are re-read at each lattice step of the systems (D129: the
//! same epochs at every warp), so a stop is on a lattice epoch until the
//! laws give each event's own (warp-invariance.md §7, point 3). The
//! thresholds are E until D116's crew thresholds (NASA-STD-3001) are
//! sourced (software-layers.md §8, point 2).

use super::program::Slot;
use crate::craft::handle::Owner;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use crate::systems::data::{Knowledge, Quantity};
use crate::time::Epoch;
use serde::{Deserialize, Serialize};

/// The cabin's CO₂ limit (Pa, E: 7.5 mmHg, the cabin air's alarm in
/// `systems::crew`; NASA-STD-3001's 1-hour limit is lower).
pub const CO2_LIMIT: f64 = 1_000.0;
/// The cabin's O₂ floor (Pa, E: below it hypoxia starts to impair).
pub const O2_FLOOR: f64 = 18_000.0;
/// The cabin's total pressure floor (Pa, E).
pub const PRESSURE_FLOOR: f64 = 70_000.0;
/// A consumable's time to empty below which the crew is at risk (s, E: a
/// day, time to come home from low orbit).
pub const SUPPLY_MARGIN: f64 = 86_400.0;

/// A risk to the crew or the vessel (software-layers.md §4's table).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Risk {
    /// The cabin's air past a limit, as its sensors read it.
    Co2High,
    O2Low,
    PressureLow,
    /// A consumable of the life support will run out within the margin.
    SupplyLow,
    /// The life support out (failed or unpowered) with crew aboard.
    SupportLost,
    /// A part destroyed (its `Part::id`).
    PartLost(u32),
    /// No flight computer working (D121's controllability).
    Uncontrollable,
    /// An essential load shed (a computer, the life support).
    Shed(Owner),
}

impl Risk {
    /// Its kind, for a program's line.
    pub fn kind(&self) -> Kind {
        match self {
            Risk::Co2High | Risk::O2Low | Risk::PressureLow | Risk::SupplyLow | Risk::SupportLost => Kind::Crew,
            Risk::PartLost(_) => Kind::Vessel,
            Risk::Uncontrollable => Kind::Control,
            Risk::Shed(_) => Kind::Power,
        }
    }

    /// For the player.
    pub fn text(&self, def: &ShipDef) -> String {
        match self {
            Risk::Co2High => "crew at risk: cabin CO₂ over its limit".into(),
            Risk::O2Low => "crew at risk: cabin O₂ under its floor".into(),
            Risk::PressureLow => "crew at risk: cabin pressure under its floor".into(),
            Risk::SupplyLow => "crew at risk: the life support runs out within a day".into(),
            Risk::SupportLost => "crew at risk: the life support is out".into(),
            Risk::PartLost(p) => format!("vessel at risk: {} destroyed", def.owner_id(Owner::Part(*p))),
            Risk::Uncontrollable => "vessel at risk: no flight computer working".into(),
            Risk::Shed(o) => format!("vessel at risk: {} shed", def.owner_id(*o)),
        }
    }
}

/// What the risks are read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Kind {
    Crew,
    Vessel,
    Control,
    Power,
}

/// What the parts report and the sensors read at an epoch.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    pub crew: u32,
    /// The cabin's CO₂, O₂ and total pressure (Pa) where a sensor reads it.
    pub co2: Option<f64>,
    pub o2: Option<f64>,
    pub pressure: Option<f64>,
    /// The life support's shortest time to empty (s), if it has stocks.
    pub supply_left: Option<f64>,
    /// The life support out: each failed or unpowered (a craft without one
    /// has none to lose).
    pub support_out: bool,
    /// The parts destroyed.
    pub destroyed: Vec<u32>,
    /// A flight computer works (or the craft has none modelled).
    pub computing: bool,
    /// The essential loads shed.
    pub shed: Vec<Owner>,
}

impl Facts {
    /// The facts of `state` on `def`, the cabin's air as `k` knows it.
    pub fn of(def: &ShipDef, state: &ShipState, k: &Knowledge, supply_left: Option<f64>) -> Facts {
        let read = |q| k.read(q, crate::systems::data::truth(state, q)?).map(|r| r.value);
        let essential =
            |o: Owner| def.computers.iter().any(|c| c.owner == o) || def.life_support.iter().any(|l| l.owner == o);
        Facts {
            crew: crate::systems::crew::crew_of(def),
            co2: read(Quantity::CabinCo2),
            o2: read(Quantity::CabinO2),
            pressure: read(Quantity::CabinPressure),
            supply_left,
            support_out: !def.life_support.is_empty() && def.life_support.iter().all(|l| state.out(l.owner)),
            destroyed: state.parts.iter().filter(|p| p.integrity <= 0.0).map(|p| p.part).collect(),
            computing: super::health(def, state).computing(),
            shed: state.unpowered.iter().map(|l| l.owner).filter(|&o| essential(o)).collect(),
        }
    }
}

/// The risks the facts show (a pure rule; the table test is its spec).
pub fn risks(f: &Facts) -> Vec<Risk> {
    let mut out = Vec::new();
    if f.crew > 0 {
        let over = |v: Option<f64>, limit: f64| v.is_some_and(|v| v > limit);
        let under = |v: Option<f64>, floor: f64| v.is_some_and(|v| v < floor);
        let air = [
            (over(f.co2, CO2_LIMIT), Risk::Co2High),
            (under(f.o2, O2_FLOOR), Risk::O2Low),
            (under(f.pressure, PRESSURE_FLOOR), Risk::PressureLow),
            (f.supply_left.is_some_and(|s| s < SUPPLY_MARGIN), Risk::SupplyLow),
            (f.support_out, Risk::SupportLost),
        ];
        out.extend(air.into_iter().filter(|(at, _)| *at).map(|(_, r)| r));
    }
    out.extend(f.destroyed.iter().map(|&p| Risk::PartLost(p)));
    if !f.computing {
        out.push(Risk::Uncontrollable);
    }
    out.extend(f.shed.iter().map(|&o| Risk::Shed(o)));
    out.sort();
    out.dedup();
    out
}

/// The risks in `now` not in `seen` (both sorted).
pub fn new_risks(seen: &[Risk], now: &[Risk]) -> Vec<Risk> {
    now.iter().filter(|r| seen.binary_search(r).is_err()).cloned().collect()
}

/// The built-in line (layer 1's): every new risk stops warp.
pub fn bare_minimum(new: &[Risk]) -> bool {
    !new.is_empty()
}

/// A program's line (software-layers.md §4): which kinds of risk stop warp.
/// A native program for now (D043's WASM later), configured per craft.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Line {
    pub crew: bool,
    pub vessel: bool,
    pub control: bool,
    pub power: bool,
}

impl Default for Line {
    /// The built-in line's kinds.
    fn default() -> Self {
        Line { crew: true, vessel: true, control: true, power: true }
    }
}

impl Line {
    /// Whether these new risks stop warp.
    pub fn stops(&self, new: &[Risk]) -> bool {
        new.iter().any(|r| match r.kind() {
            Kind::Crew => self.crew,
            Kind::Vessel => self.vessel,
            Kind::Control => self.control,
            Kind::Power => self.power,
        })
    }
}

/// A craft's warp-stop program: its switch (D126) and its line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub slot: Slot,
    pub line: Line,
}

/// A warp stop: its epoch and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub at: Epoch,
    pub why: String,
}

/// What the warp stop keeps between checks (saved with the vessel's
/// software): the risks at the last check, and a stop the game has not
/// taken yet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Watch {
    /// Checked once: the risks a vessel starts with are not failures at an
    /// epoch, so its first check only records them.
    #[serde(default)]
    pub primed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen: Vec<Risk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<Stop>,
}

/// Who decides at a check: the program while it runs on a working
/// computer, else the built-in line.
pub fn decide(program: Option<&Program>, computing: bool, new: &[Risk]) -> bool {
    match program.filter(|p| p.slot.may_run() && computing) {
        Some(p) => p.line.stops(new),
        None => bare_minimum(new),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::ship_os::Switch;

    fn calm() -> Facts {
        Facts {
            crew: 4,
            co2: Some(200.0),
            o2: Some(21_300.0),
            pressure: Some(101_325.0),
            supply_left: Some(5.0 * 86_400.0),
            computing: true,
            ..Facts::default()
        }
    }

    /// software-layers.md §4's table, row by row, and what is not a stop.
    #[test]
    fn the_risks_table() {
        let comp = Owner::Component(3);
        let cases: Vec<(&str, Facts, Vec<Risk>)> = vec![
            ("calm", calm(), vec![]),
            ("CO₂ over", Facts { co2: Some(1_200.0), ..calm() }, vec![Risk::Co2High]),
            ("O₂ under", Facts { o2: Some(15_000.0), ..calm() }, vec![Risk::O2Low]),
            ("pressure under", Facts { pressure: Some(50_000.0), ..calm() }, vec![Risk::PressureLow]),
            ("a day of stock left", Facts { supply_left: Some(80_000.0), ..calm() }, vec![Risk::SupplyLow]),
            ("life support out", Facts { support_out: true, ..calm() }, vec![Risk::SupportLost]),
            ("no air sensor: nothing read", Facts { co2: None, o2: None, pressure: None, ..calm() }, vec![]),
            (
                "uncrewed: the air is not a risk",
                Facts { crew: 0, co2: Some(5_000.0), support_out: true, ..calm() },
                vec![],
            ),
            ("a part destroyed", Facts { destroyed: vec![7], ..calm() }, vec![Risk::PartLost(7)]),
            ("no computer", Facts { computing: false, ..calm() }, vec![Risk::Uncontrollable]),
            ("an essential load shed", Facts { shed: vec![comp], ..calm() }, vec![Risk::Shed(comp)]),
            (
                "several, sorted",
                Facts { co2: Some(2_000.0), computing: false, destroyed: vec![2], ..calm() },
                vec![Risk::Co2High, Risk::PartLost(2), Risk::Uncontrollable],
            ),
        ];
        for (name, f, want) in cases {
            assert_eq!(risks(&f), want, "{name}");
        }
    }

    /// Only a risk not seen at the last check stops warp; the program
    /// decides while it runs on a working computer, else the built-in line.
    #[test]
    fn new_risks_and_who_decides() {
        let seen = vec![Risk::Co2High];
        assert!(new_risks(&seen, &[Risk::Co2High]).is_empty());
        assert_eq!(new_risks(&seen, &[Risk::Co2High, Risk::Uncontrollable]), vec![Risk::Uncontrollable]);
        let power = [Risk::Shed(Owner::Component(1))];
        let only_crew =
            Program { slot: Slot::auto(), line: Line { crew: true, vessel: false, control: false, power: false } };
        // (program, computing, new risks, stops)
        let cases = [
            (None, true, &power[..], true),
            (None, true, &[][..], false),
            (Some(only_crew), true, &power[..], false),
            (Some(only_crew), true, &[Risk::SupplyLow][..], true),
            (Some(only_crew), false, &power[..], true),
            (Some(Program { slot: Slot { switch: Switch::Off, pressed: false }, ..only_crew }), true, &power[..], true),
        ];
        for (i, (p, computing, new, stops)) in cases.into_iter().enumerate() {
            assert_eq!(decide(p.as_ref(), computing, new), stops, "case {i}");
        }
    }
}
