//! Electrical power. A craft of parts' buses ([`step_ship`], D104;
//! ship-model.md §5.5): power flow in watts from the pad's umbilical, the
//! solar cells and the batteries to every component's load, shedding the
//! least essential when short; live each tick and on the lattice while
//! coasting. The test craft's placeholder (D033): a solar panel, a battery
//! and a constant load, stepped on the systems lattice ([`super::STEP`]).
//!
//! Production is η·A·S·cos θ with S the sunlight at the vessel (eclipses
//! included) and θ the panel's incidence: a sun-tracking panel has cos θ = 1
//! whenever lit, a body-fixed one takes the attitude. The battery takes the
//! surplus up to its capacity (the rest is spilled) and covers the deficit
//! down to empty; a load the battery cannot cover is recorded as a
//! shortfall. Numbers are placeholders until parts carry them.

use super::Env;
use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;
use crate::time::Epoch;
use glam::DVec3;

/// What the power system is made of.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PowerSpec {
    /// Panel area (m²) and conversion efficiency.
    pub panel_area: f64,
    pub efficiency: f64,
    /// The panel's normal (body axes; its lit side).
    pub panel_normal: DVec3,
    /// Turned to the Sun whenever lit (two-axis tracking).
    pub tracking: bool,
    /// Battery capacity (J).
    pub capacity: f64,
    /// Constant electrical load (W).
    pub load: f64,
}

impl Default for PowerSpec {
    /// A small craft's placeholders: 10 m² of 30 % cells on trackers
    /// (≈ 4 kW at 1 AU), a 10 kWh battery, 1 kW of loads.
    fn default() -> Self {
        PowerSpec {
            panel_area: 10.0,
            efficiency: 0.3,
            panel_normal: DVec3::X,
            tracking: true,
            capacity: 3.6e7,
            load: 1_000.0,
        }
    }
}

/// The power system's state at the last lattice point.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Power {
    pub spec: PowerSpec,
    /// Battery charge (J).
    pub charge: f64,
    /// Panel output over the last step (W).
    pub produced: f64,
    /// Energy the loads asked for and did not get (J, since the start).
    pub shortfall: f64,
}

impl Default for Power {
    /// The default spec, battery full.
    fn default() -> Self {
        let spec = PowerSpec::default();
        Power { spec, charge: spec.capacity, produced: 0.0, shortfall: 0.0 }
    }
}

impl Power {
    /// The panel's output (W) in sunlight `sun` (W/m², along the light,
    /// inertial) at the attitude of `env`.
    pub fn output(&self, env: &Env) -> f64 {
        let flux = env.sun.length();
        if flux <= 0.0 {
            return 0.0;
        }
        let s = &self.spec;
        let cos = if s.tracking { 1.0 } else { (-(env.attitude * s.panel_normal)).dot(env.sun / flux).max(0.0) };
        s.efficiency * s.panel_area * flux * cos
    }

    /// One lattice step of `dt` seconds with the environment at its end.
    pub fn step(&mut self, env: &Env, dt: f64) {
        self.produced = self.output(env);
        let charge = self.charge + (self.produced - self.spec.load) * dt;
        if charge > self.spec.capacity {
            self.charge = self.spec.capacity;
        } else if charge < 0.0 {
            self.shortfall -= charge;
            self.charge = 0.0;
        } else {
            self.charge = charge;
        }
    }
}

/// A bus of a craft of parts as it stood at its last step (D104;
/// ship-model.md §5.5): what the systems screen's electrical layer shows.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BusRun {
    pub name: String,
    /// Volts (derived: the batteries' open-circuit voltage by their charge
    /// less their resistance's drop; the nominal on ground power).
    pub volts: f64,
    /// The loads it powers and what the cells give (W).
    pub load: f64,
    pub solar: f64,
    /// Fed through the pad's umbilical.
    pub ground: bool,
}

/// What a load is to a bus's shedding: the vehicle's functions first
/// (computers, sensors, valves, thrusters, life support), the rest shed
/// before them.
fn essential(def: &ShipDef, id: &str) -> bool {
    def.computers.iter().any(|x| x.id == id)
        || def.sensors.iter().any(|x| x.id == id)
        || def.valves.iter().any(|x| x.id == id)
        || def.thrusters.iter().any(|x| x.id == id)
        || def.life_support.iter().any(|x| x.id == id)
}

/// A battery's open-circuit voltage at its state of charge (V; E: a
/// lithium cell's 3.0–4.2 V over its charge, as a share of the nominal).
pub fn open_circuit(nominal: f64, soc: f64) -> f64 {
    nominal * (0.85 + 0.2 * soc.clamp(0.0, 1.0))
}

/// The cells' output (W) in sunlight `sun` (W/m², along the light, body
/// axes): each panel facing out from the craft's axis where it sits (body-
/// mounted cells, as Dragon's trunk), at its area and efficiency.
pub fn solar_output(def: &ShipDef, sun: DVec3) -> Vec<f64> {
    let flux = sun.length();
    (def.solars.iter())
        .map(|s| {
            let Some(c) = def.components.iter().find(|c| c.id == s.id) else { return 0.0 };
            let out = DVec3::new(c.pose.pos.x, c.pose.pos.y, 0.0).normalize_or(DVec3::Z);
            let cos = if flux > 0.0 { (-out).dot(sun / flux).max(0.0) } else { 0.0 };
            s.item.area * s.item.efficiency * flux * cos
        })
        .collect()
}

/// Thermal mass of a battery (J/K per kg; E, lithium cells and their case).
const BATTERY_C: f64 = 1_000.0;
/// How fast a battery's temperature follows its surroundings (s; E).
const BATTERY_TAU: f64 = 3_600.0;
/// The surroundings' temperature batteries relax to (K).
const CABIN_K: f64 = 293.15;

/// One step of `dt` seconds of a craft of parts' power (see the module):
/// every bus's loads (each component switched on, not lost, drawing its
/// power) against its sources — the pad's umbilical while the clamps hold
/// it, its solar cells in `sun` (W/m², body axes), then its batteries
/// (each to its maximum power, losing I²R in its resistance and warming).
/// A bus short of power sheds its loads, the non-essential first, then the
/// largest; a shed or lost-power load's function stops
/// (`ShipState::unpowered`) until power returns. Surplus charges the
/// batteries. Events tell the player when a load loses or regains power.
pub fn step_ship(def: &ShipDef, state: &mut ShipState, sun: DVec3, dt: f64, at: Epoch) {
    let solar = solar_output(def, sun);
    let ground = state.clamped;
    let mut buses = Vec::new();
    let mut unpowered: Vec<(u32, String)> = Vec::new();
    for bus in &def.buses {
        let on = |id: &str| bus.members.iter().any(|m| m.0 == id);
        let lost = |id: &str| state.lost.iter().any(|l| l.owner == id);
        let mut loads: Vec<(usize, f64, bool)> = (def.components.iter().enumerate())
            .filter(|(c, d)| on(&d.id) && d.power > 0.0 && state.power[*c].on && !lost(&d.id))
            .map(|(c, d)| (c, d.power, essential(def, &d.id)))
            .collect();
        let sun_w: f64 = (def.solars.iter().zip(&solar)).filter(|(s, _)| on(&s.id)).map(|(_, w)| w).sum();
        let cells: Vec<usize> = (0..def.batteries.len()).filter(|&b| on(&def.batteries[b].id)).collect();
        let nominal = bus.voltage.max(1.0);
        let demand: f64 = loads.iter().map(|l| l.1).sum();
        if ground {
            // The umbilical feeds the loads and tops the batteries up.
            for &b in &cells {
                let cap = def.batteries[b].item;
                let s = &mut state.batteries[b];
                s.charge = (s.charge + cap.max_power * dt).min(cap.capacity);
            }
            buses.push(BusRun { name: bus.name.clone(), volts: nominal, load: demand, solar: sun_w, ground: true });
            continue;
        }
        // What the batteries can give over the step.
        let can = |state: &ShipState, b: usize| -> f64 {
            let cap = def.batteries[b].item;
            cap.max_power.min(state.batteries[b].charge / dt.max(1e-9)).max(0.0)
        };
        let available: f64 = cells.iter().map(|&b| can(state, b)).sum();
        // Shed until the loads fit: the non-essential first, then the largest.
        loads.sort_by(|a, b| a.2.cmp(&b.2).then(b.1.total_cmp(&a.1)).then(a.0.cmp(&b.0)));
        let mut need = demand - sun_w;
        while need > available + 1e-9 && !loads.is_empty() {
            let (c, w, _) = loads.remove(0);
            unpowered.push((def.components[c].part, def.components[c].id.clone()));
            need -= w;
        }
        let load: f64 = loads.iter().map(|l| l.1).sum();
        let mut volts = if cells.is_empty() {
            if need <= 0.0 && load > 0.0 {
                nominal
            } else {
                0.0
            }
        } else {
            0.0
        };
        if need > 0.0 && available > 0.0 {
            // Each battery its share of the deficit by what it can give.
            for &b in &cells {
                let share = need * can(state, b) / available;
                let cap = def.batteries[b].item;
                let soc = state.batteries[b].charge / cap.capacity.max(1.0);
                let v = open_circuit(cap.voltage, soc);
                let i = share / v.max(1.0);
                let heat = i * i * cap.internal_resistance;
                let s = &mut state.batteries[b];
                s.charge = (s.charge - (share + heat) * dt).max(0.0);
                let mass = def.components.iter().find(|c| c.id == def.batteries[b].id).map_or(10.0, |c| c.mass);
                s.k += (heat - (s.k - CABIN_K) * mass * BATTERY_C / BATTERY_TAU) * dt / (mass * BATTERY_C);
                volts = volts.max(v - i * cap.internal_resistance);
            }
        } else if need < 0.0 && !cells.is_empty() {
            // Surplus from the cells charges the batteries.
            let mut spare = -need;
            for &b in &cells {
                let cap = def.batteries[b].item;
                let s = &mut state.batteries[b];
                let take = spare.min(cap.max_power).min((cap.capacity - s.charge).max(0.0) / dt.max(1e-9));
                s.charge += take * dt;
                spare -= take;
                volts = volts.max(open_circuit(cap.voltage, s.charge / cap.capacity.max(1.0)));
            }
        } else if let Some(&b) = cells.first() {
            let cap = def.batteries[b].item;
            volts = open_circuit(cap.voltage, state.batteries[b].charge / cap.capacity.max(1.0));
        }
        buses.push(BusRun { name: bus.name.clone(), volts, load, solar: sun_w, ground: false });
    }
    // Tell the player what lost or regained power.
    let before: Vec<String> = state.unpowered.iter().map(|u| u.owner.clone()).collect();
    for (part, id) in &unpowered {
        if !before.contains(id) {
            state.event(at, format!("{id}: no power (its bus is short; shed)"), true);
        }
        let _ = part;
    }
    for id in before.iter().filter(|id| !unpowered.iter().any(|u| &u.1 == *id)) {
        state.event(at, format!("{id}: powered again"), false);
    }
    state.unpowered = (unpowered.into_iter())
        .map(|(part, owner)| crate::craft::state::Lost { part, owner, why: "no power".into() })
        .collect();
    state.buses = buses;
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DQuat;

    #[test]
    fn production_and_the_battery_table() {
        let lit = |q: DQuat| Env { sun: DVec3::new(-1_361.0, 0.0, 0.0), attitude: q };
        let dark = Env { sun: DVec3::ZERO, attitude: DQuat::IDENTITY };
        let fixed = Power { spec: PowerSpec { tracking: false, ..PowerSpec::default() }, ..Power::default() };
        // (power, environment, output W): a fixed panel facing the Sun
        // (+X lit side towards the light, which travels −X), turned 60°
        // away, turned away, tracking, in shadow.
        let turn =
            |deg: f64| DQuat::from_xyzw(0.0, 0.0, (deg.to_radians() / 2.0).sin(), (deg.to_radians() / 2.0).cos());
        let cases = [
            (fixed, lit(DQuat::IDENTITY), 0.3 * 10.0 * 1_361.0),
            (fixed, lit(turn(60.0)), 0.3 * 10.0 * 1_361.0 * 0.5),
            (fixed, lit(turn(180.0)), 0.0),
            (Power::default(), lit(turn(123.0)), 0.3 * 10.0 * 1_361.0),
            (Power::default(), dark, 0.0),
        ];
        for (p, env, expected) in cases {
            assert!((p.output(&env) - expected).abs() < 1e-9 * expected.max(1.0), "{expected}: {}", p.output(&env));
        }
        // An hour of eclipse drains 1 kWh; a full battery spills the surplus.
        let mut p = Power::default();
        for _ in 0..60 {
            p.step(&dark, 60.0);
        }
        assert!((p.charge - (3.6e7 - 3.6e6)).abs() < 1e-6, "{}", p.charge);
        for _ in 0..600 {
            p.step(&lit(DQuat::IDENTITY), 60.0);
        }
        assert_eq!(p.charge, p.spec.capacity);
        // Empty: the rest of the load is a shortfall.
        let mut p = Power { charge: 30_000.0, ..Power::default() };
        p.step(&dark, 60.0);
        assert_eq!((p.charge, p.shortfall), (0.0, 30_000.0));
    }

    mod ship {
        use super::super::*;
        use crate::craft::state::ShipState;
        use crate::craft::{default_craft_dir, load_assembly};

        fn falcon9() -> std::sync::Arc<ShipDef> {
            load_assembly(&default_craft_dir().join("falcon9-crew")).unwrap().def.ship.unwrap()
        }

        fn t0() -> Epoch {
            Epoch::new(845_000_000, 0.0)
        }

        fn bus<'a>(s: &'a ShipState, name: &str) -> &'a BusRun {
            s.buses.iter().find(|b| b.name == name).unwrap()
        }

        fn load_of(def: &ShipDef, name: &str) -> f64 {
            let b = def.buses.iter().find(|b| b.name == name).unwrap();
            (def.components.iter()).filter(|c| b.members.iter().any(|m| m.0 == c.id)).map(|c| c.power).sum()
        }

        /// On the pad the umbilical feeds every bus at its nominal voltage
        /// and tops the batteries up; off it the batteries carry the loads:
        /// each stage's charge falls by its loads and their I²R.
        #[test]
        fn the_umbilical_then_the_batteries() {
            let def = falcon9();
            let mut s = ShipState::initial(&def);
            assert!(s.clamped);
            let b = def.batteries.iter().position(|b| b.id.starts_with("s1.")).unwrap();
            s.batteries[b].charge *= 0.5;
            step_ship(&def, &mut s, DVec3::ZERO, 60.0, t0());
            let s1 = bus(&s, "s1.bus");
            assert!(s1.ground && s1.volts == 28.0 && (s1.load - load_of(&def, "s1.bus")).abs() < 1e-9);
            assert!(s.batteries[b].charge > 0.5 * def.batteries[b].item.capacity);
            assert!(s.unpowered.is_empty());
            // Released: an hour on the first stage's two batteries.
            s.clamped = false;
            let cells: Vec<usize> =
                (0..def.batteries.len()).filter(|&i| def.batteries[i].id.starts_with("s1.")).collect();
            let full: f64 = cells.iter().map(|&i| def.batteries[i].item.capacity).sum();
            for &i in &cells {
                s.batteries[i].charge = def.batteries[i].item.capacity;
            }
            for k in 0..60 {
                step_ship(&def, &mut s, DVec3::ZERO, 60.0, t0().add_seconds(60.0 * f64::from(k)));
            }
            let used: f64 = full - cells.iter().map(|&i| s.batteries[i].charge).sum::<f64>();
            let load = load_of(&def, "s1.bus");
            println!("s1.bus: {load:.0} W, {:.0} Wh in an hour, {:.1} V", used / 3600.0, bus(&s, "s1.bus").volts);
            assert!(used > load * 3600.0 && used < load * 3600.0 * 1.01, "{used} for {load} W");
            let v = bus(&s, "s1.bus").volts;
            assert!(v > 0.8 * 28.0 && v < 1.1 * 28.0, "{v}");
            assert!(s.unpowered.is_empty());
        }

        /// Dragon's trunk cells with the Sun square on them give their 3.6 kW
        /// (8.9 m² at 30 %), more than its ~1.2 kW of loads: the surplus
        /// charges its batteries; in the dark they carry the loads.
        #[test]
        fn the_sun_charges_dragon() {
            let def = falcon9();
            let mut s = ShipState::initial(&def);
            s.clamped = false;
            let sun = DVec3::new(-1_361.0, 0.0, 0.0);
            let out: f64 = solar_output(&def, sun).iter().sum();
            assert!((out - 8.9 * 0.3 * 1_361.0).abs() < 1.0, "{out}");
            let cells: Vec<usize> =
                (0..def.batteries.len()).filter(|&i| def.batteries[i].id.starts_with("dragon.")).collect();
            for &i in &cells {
                s.batteries[i].charge = 0.5 * def.batteries[i].item.capacity;
            }
            let before: f64 = cells.iter().map(|&i| s.batteries[i].charge).sum();
            step_ship(&def, &mut s, sun, 60.0, t0());
            let after: f64 = cells.iter().map(|&i| s.batteries[i].charge).sum();
            let load = load_of(&def, "dragon.bus");
            assert!(
                (after - before - (out - load) * 60.0).abs() < 1.0,
                "{} vs {}",
                after - before,
                (out - load) * 60.0
            );
            step_ship(&def, &mut s, DVec3::ZERO, 60.0, t0());
            let dark: f64 = cells.iter().map(|&i| s.batteries[i].charge).sum();
            assert!(dark < after - load * 60.0 && dark > after - 1.01 * load * 60.0);
        }

        /// Flat batteries in the dark: the bus sheds everything; with the
        /// Sun on part of the cells it keeps what fits, the essential loads
        /// first (the docking system goes first); power back, they return.
        #[test]
        fn a_short_bus_sheds_the_least_essential_first() {
            let def = falcon9();
            let mut s = ShipState::initial(&def);
            s.clamped = false;
            for (b, d) in s.batteries.iter_mut().zip(&def.batteries) {
                if d.id.starts_with("dragon.") {
                    b.charge = 0.0;
                }
            }
            step_ship(&def, &mut s, DVec3::ZERO, 60.0, t0());
            let shed = |s: &ShipState, id: &str| s.unpowered.iter().any(|u| u.owner == id);
            assert!(shed(&s, "dragon.eclss") && shed(&s, "dragon.computer") && shed(&s, "dragon.docking"));
            assert!(!shed(&s, "s1.computer"), "the first stage's own batteries");
            assert_eq!(bus(&s, "dragon.bus").load, 0.0, "nothing powered");
            // Cells giving all but 30 W of the loads: only the docking system (50 W, not essential) goes.
            let load = load_of(&def, "dragon.bus");
            let flux = (load - 30.0) / (8.9 * 0.3);
            step_ship(&def, &mut s, DVec3::new(-flux, 0.0, 0.0), 60.0, t0().add_seconds(60.0));
            assert!(shed(&s, "dragon.docking"));
            assert!(!shed(&s, "dragon.eclss") && !shed(&s, "dragon.computer"));
            assert!(s.events.iter().any(|e| e.text.starts_with("dragon.eclss: powered again")));
        }
    }
}
