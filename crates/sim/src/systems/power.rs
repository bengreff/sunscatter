//! Placeholder electrical power (D033): a solar panel, a battery and a
//! constant load, stepped on the systems lattice ([`super::STEP`]).
//!
//! Production is η·A·S·cos θ with S the sunlight at the vessel (eclipses
//! included) and θ the panel's incidence: a sun-tracking panel has cos θ = 1
//! whenever lit, a body-fixed one takes the attitude. The battery takes the
//! surplus up to its capacity (the rest is spilled) and covers the deficit
//! down to empty; a load the battery cannot cover is recorded as a
//! shortfall. Numbers are placeholders until parts carry them.

use super::Env;
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

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DQuat;

    #[test]
    fn production_and_the_battery_table() {
        let lit = |q: DQuat| Env { sun: DVec3::new(-1_361.0, 0.0, 0.0), attitude: q, propellant: 0.0 };
        let dark = Env { sun: DVec3::ZERO, attitude: DQuat::IDENTITY, propellant: 0.0 };
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
}
