//! Information (D106; ship-model.md §7): what a vessel knows, from its
//! working sensors. The interface never shows something wrong: a quantity
//! no working sensor measures is unknown (a dash), and a known one is shown
//! to the precision its best sensor gives ("you can't get your velocity to
//! 50 decimal places"). Errors are simplified: values are the truth shown
//! at their precision, never noise. Debug mode (D064) knows everything
//! exactly.
//!
//! **The stable API** (read by the flight view, the systems screen, the MCP
//! server and later scripts): [`Quantity`] names what can be known (vehicle
//! quantities, and per-item ones by their index in the `ShipDef` lists),
//! [`Knowledge`] says for each whether it is known and how well
//! ([`Knowledge::precision`]), and [`Knowledge::read`] turns a true value
//! into the [`Reading`] the interface shows ([`Reading::decimals`],
//! [`Reading::shown`]). Uncrewed craft's knowledge reaches mission control
//! with the comm network's delay (the game shows telemetry retarded).
//!
//! **What measures what** ([`provides`], by a sensor's `quantity`):
//! `inertial` (an IMU: attitude, angular rates, specific force), `attitude`
//! (a star tracker), `position` (a GNSS receiver: position, velocity and
//! altitude), `rates`, `acceleration`, `altitude` (a radar altimeter),
//! `tank-level`, `pressure` (the tanks), `bottle-pressure` (the bottles),
//! `temperature` (the tanks' liquids and the part), each of its own part;
//! `strain` (the joints at its part). A sensor
//! counts while it is switched on, its part intact, and linked to a
//! working computer (D106: "the actual mechanisms behind how your ship gets
//! the data"; `systems::flight::works`, `read_by_computer`). Each engine's
//! own controller reads its thrust (falcon9.md §5.2: chamber pressure and
//! the rest), to 1 %, while it works and a computer reads it; each
//! battery's management electronics its charge, to 1 %.

use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Something the vessel can know. Per-item quantities carry the item's
/// index in its `ShipDef` list (`tanks`, `bottles`, `engines`, `batteries`,
/// `joints`, `parts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Quantity {
    /// m: where the vessel is (and everything derived from it: orbits,
    /// apsides, ground track).
    Position,
    /// m/s: its velocity (orbital or surface speed).
    Velocity,
    /// m above the surface below.
    Altitude,
    /// rad: its attitude.
    Attitude,
    /// rad/s: its angular velocity.
    AngularRate,
    /// m/s²: the proper acceleration (g-load).
    Acceleration,
    /// kg of liquid in a tank.
    TankLevel(u32),
    /// Pa in a tank's ullage.
    TankPressure(u32),
    /// K of a tank's liquid.
    TankTemperature(u32),
    /// Pa in a bottle.
    BottlePressure(u32),
    /// N of an engine's thrust (its controller's readout).
    EngineThrust(u32),
    /// J in a battery.
    BatteryCharge(u32),
    /// A joint's load over its allowable.
    JointLoad(u32),
    /// K of a part.
    PartTemperature(u32),
    /// Pa: the cabin's total pressure, O₂ and CO₂ partial pressures.
    CabinPressure,
    CabinO2,
    CabinCo2,
}

/// A value as the interface shows it: the true value and the precision it
/// is known to (one sigma, in the quantity's unit; 0 is exact).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub value: f64,
    pub precision: f64,
}

/// Most decimals shown for any quantity.
const MAX_DECIMALS: usize = 6;

impl Reading {
    /// Decimals to show: down to the precision's order of magnitude (0 for
    /// 3 m, 1 for 0.1 m/s); exact values at most [`MAX_DECIMALS`].
    pub fn decimals(&self) -> usize {
        if self.precision.is_nan() || self.precision <= 0.0 {
            return MAX_DECIMALS;
        }
        let order = crate::math::log10(self.precision).floor();
        (-order).clamp(0.0, MAX_DECIMALS as f64) as usize
    }

    /// The value rounded to the precision's decade (what is shown).
    pub fn shown(&self) -> f64 {
        if self.precision.is_nan() || self.precision <= 0.0 {
            return self.value;
        }
        let step = crate::math::pow(10.0, crate::math::log10(self.precision).floor());
        (self.value / step).round() * step
    }

    /// The shown value as text with `unit` (`"m/s"`).
    pub fn text(&self, unit: &str) -> String {
        format!("{:.*} {unit}", self.decimals(), self.shown())
    }
}

/// What a vessel knows now: each known quantity's precision.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Knowledge {
    /// Everything, exactly (debug mode, or a craft without the part system).
    pub perfect: bool,
    known: BTreeMap<Quantity, f64>,
}

impl Knowledge {
    /// Everything known exactly (debug mode, D064 and D106).
    pub fn perfect() -> Knowledge {
        Knowledge { perfect: true, known: BTreeMap::new() }
    }

    /// The precision `q` is known to (0: exactly), or `None`: unknown,
    /// shown as a dash.
    pub fn precision(&self, q: Quantity) -> Option<f64> {
        if self.perfect {
            return Some(0.0);
        }
        self.known.get(&q).copied()
    }

    /// Whether `q` is known at all.
    pub fn knows(&self, q: Quantity) -> bool {
        self.precision(q).is_some()
    }

    /// The reading of `q` whose true value is `truth`, if it is known.
    pub fn read(&self, q: Quantity, truth: f64) -> Option<Reading> {
        self.precision(q).map(|precision| Reading { value: truth, precision })
    }

    /// Records that `q` is known to `precision` (the best sensor wins).
    pub fn learn(&mut self, q: Quantity, precision: f64) {
        let p = precision.max(0.0);
        self.known.entry(q).and_modify(|x| *x = x.min(p)).or_insert(p);
    }

    /// Every known quantity and its precision, in order.
    pub fn iter(&self) -> impl Iterator<Item = (Quantity, f64)> + '_ {
        self.known.iter().map(|(q, p)| (*q, *p))
    }
}

/// What a vehicle-wide sensor class measures, with the precisions its
/// accuracy gives (class values of falcon9.md §5.2 where one number does
/// not say: an IMU's gyros to 1e-5 rad/s and accelerometers to 50 µg, a
/// GNSS receiver's velocity to 0.1 m/s per 3 m of position and its altitude
/// to 1.5× its position).
pub fn provides(quantity: &str, accuracy: f64) -> Vec<(Quantity, f64)> {
    match quantity {
        "inertial" => {
            vec![(Quantity::Attitude, accuracy), (Quantity::AngularRate, 1e-5), (Quantity::Acceleration, 5e-4)]
        }
        "attitude" => vec![(Quantity::Attitude, accuracy)],
        "rates" => vec![(Quantity::AngularRate, accuracy)],
        "acceleration" => vec![(Quantity::Acceleration, accuracy)],
        "position" => vec![
            (Quantity::Position, accuracy),
            (Quantity::Velocity, accuracy / 30.0),
            (Quantity::Altitude, 1.5 * accuracy),
        ],
        "velocity" => vec![(Quantity::Velocity, accuracy)],
        "altitude" => vec![(Quantity::Altitude, accuracy)],
        _ => Vec::new(),
    }
}

/// What the vessel of `def` in `state` knows now (see the module).
pub fn knowledge(def: &ShipDef, state: &ShipState, debug: bool) -> Knowledge {
    if debug {
        return Knowledge::perfect();
    }
    use super::flight::{read_by_computer, works};
    let mut k = Knowledge::default();
    let intact = |part: u32| state.parts.iter().find(|p| p.part == part).is_none_or(|p| p.integrity > 0.0);
    for s in &def.sensors {
        if !(works(def, state, &s.id, s.part) && read_by_computer(def, state, &s.id)) {
            continue;
        }
        let (q, acc) = (s.item.quantity.as_str(), s.item.accuracy);
        for (quantity, precision) in provides(q, acc) {
            k.learn(quantity, precision);
        }
        let mine = |part: u32| part == s.part;
        match q {
            "bottle-pressure" => {
                for (i, _) in def.bottles.iter().enumerate().filter(|(_, b)| mine(b.part)) {
                    k.learn(Quantity::BottlePressure(i as u32), acc);
                }
            }
            "tank-level" | "pressure" | "temperature" => {
                for (i, t) in def.tanks.iter().enumerate().filter(|(_, t)| mine(t.part)) {
                    let quantity = match q {
                        "tank-level" => Quantity::TankLevel(i as u32),
                        "pressure" => Quantity::TankPressure(i as u32),
                        _ => Quantity::TankTemperature(i as u32),
                    };
                    k.learn(quantity, if q == "tank-level" { acc * t.capacity } else { acc });
                }
                if q == "temperature" {
                    k.learn(Quantity::PartTemperature(s.part), acc);
                }
            }
            "strain" => {
                for (i, _) in def.joints.iter().enumerate().filter(|(_, j)| mine(j.parent) || mine(j.child)) {
                    k.learn(Quantity::JointLoad(i as u32), acc);
                }
            }
            _ => {}
        }
    }
    // Each engine's own controller (falcon9.md §5.2), while it works and a
    // computer reads it (F3: a lost controller or a dead stage computer
    // leaves its engine's thrust unknown).
    let reported = |e: &crate::craft::ship::EngineDef| {
        works(def, state, &e.owner, e.part) && read_by_computer(def, state, &e.owner)
    };
    for (i, e) in def.engines.iter().enumerate().filter(|(_, e)| reported(e)) {
        k.learn(Quantity::EngineThrust(i as u32), 0.01 * e.data.thrust_vac);
    }
    // Each battery's management electronics report its charge, to 1 %.
    for (i, b) in def.batteries.iter().enumerate().filter(|(_, b)| intact(b.part)) {
        k.learn(Quantity::BatteryCharge(i as u32), 0.01 * b.item.capacity);
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_show_the_digits_their_precision_gives() {
        // (value, precision) → (decimals, shown, text in m)
        let cases = [
            (7_812.345_6, 0.1, 1, 7_812.3, "7812.3 m"),
            (6_771_234.7, 3.0, 0, 6_771_235.0, "6771235 m"),
            (6_771_234.7, 30.0, 0, 6_771_230.0, "6771230 m"),
            (0.123_456_789, 0.0, 6, 0.123_456_789, "0.123457 m"),
            (12.5, 0.01, 2, 12.5, "12.50 m"),
        ];
        for (value, precision, decimals, shown, text) in cases {
            let r = Reading { value, precision };
            assert_eq!(r.decimals(), decimals, "{value} ± {precision}");
            assert!((r.shown() - shown).abs() < 1e-9 * shown.abs().max(1.0), "{value} ± {precision}: {}", r.shown());
            assert_eq!(r.text("m"), text);
        }
    }

    #[test]
    fn the_best_sensor_wins_and_debug_knows_everything() {
        let mut k = Knowledge::default();
        assert_eq!(k.precision(Quantity::Velocity), None, "no sensor: a dash");
        assert_eq!(k.read(Quantity::Velocity, 7_800.0), None);
        for (q, p) in provides("position", 3.0) {
            k.learn(q, p);
        }
        k.learn(Quantity::Position, 10.0);
        assert_eq!(k.precision(Quantity::Position), Some(3.0));
        assert_eq!(k.precision(Quantity::Velocity), Some(0.1));
        assert_eq!(k.precision(Quantity::Attitude), None);
        let perfect = Knowledge::perfect();
        assert_eq!(perfect.precision(Quantity::TankLevel(7)), Some(0.0));
        assert_eq!(perfect.read(Quantity::Velocity, 1.5), Some(Reading { value: 1.5, precision: 0.0 }));
    }

    /// The Falcon 9 Crew: its IMUs, GNSS receivers and star trackers through
    /// their computers; without power to a stage's computer, nothing of
    /// that stage's sensors; debug knows all.
    #[test]
    fn the_falcon_9_knows_what_its_sensors_measure() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
        let mut state = ShipState::initial(&def);
        let k = knowledge(&def, &state, false);
        assert_eq!(k.precision(Quantity::Position), Some(3.0));
        assert!(k.knows(Quantity::Attitude) && k.knows(Quantity::Velocity));
        assert!(k.knows(Quantity::EngineThrust(0)));
        // falcon9.md §5.2's tank sensors: levels, pressures, temperatures.
        let tank = |id: &str| def.tanks.iter().position(|t| t.id == id).unwrap() as u32;
        let lox = tank("s1.tank:lox");
        assert_eq!(k.precision(Quantity::TankPressure(lox)), Some(700.0));
        assert_eq!(k.precision(Quantity::TankTemperature(lox)), Some(1.0));
        assert!(
            (k.precision(Quantity::TankLevel(lox)).unwrap() / def.tanks[lox as usize].capacity - 0.005).abs() < 1e-12
        );
        assert!(k.knows(Quantity::TankPressure(tank("dragon.nto#1:main"))));
        assert!(!k.knows(Quantity::TankLevel(tank("dragon.nto#1:main"))), "Dragon's gauging: unknown");
        assert!((0..def.bottles.len() as u32).all(|b| k.knows(Quantity::BottlePressure(b))));
        assert!((0..def.batteries.len() as u32).all(|b| k.knows(Quantity::BatteryCharge(b))));
        assert!(!k.knows(Quantity::JointLoad(0)), "no strain gauges in flight");
        // Every computer off: no sensor reaches anything.
        for (i, c) in def.components.iter().enumerate() {
            if c.id.ends_with(".computer") {
                state.power[i].on = false;
            }
        }
        let k = knowledge(&def, &state, false);
        assert!(!k.knows(Quantity::Position) && !k.knows(Quantity::Attitude));
        assert!(knowledge(&def, &state, true).knows(Quantity::Position));
        // Lost sensors (failures) measure nothing.
        let mut state = ShipState::initial(&def);
        for id in ["s1.gps#1", "s1.gps#2", "s2.gps#1", "s2.gps#2", "dragon.gps#1", "dragon.gps#2"] {
            let part = def.sensors.iter().find(|s| s.id == id).unwrap().part;
            state.runtime.lost.push(crate::systems::runtime::Lost { part, owner: id.into(), why: "test".into() });
        }
        let k = knowledge(&def, &state, false);
        assert!(!k.knows(Quantity::Position) && k.knows(Quantity::Attitude));
    }
}
