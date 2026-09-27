//! The engine (realism-1 §3c, D064): realistic but abstracted — thrust,
//! specific impulse and mass flow as numbers, no engine simulation.
//!
//! The mass flow at a throttle setting does not depend on the ambient
//! pressure; the thrust does, through the nozzle exit area `A_e`:
//! `F = throttle · (F_vac − A_e·p)`, so `Isp(p) = Isp_vac · (F_vac − A_e·p) / F_vac`.
//! `A_e` follows from the vacuum/sea-level Isp pair:
//! `A_e = F_vac · (1 − Isp_sl/Isp_vac) / p_sl`.

use super::file::EngineFile;
use crate::math;
use crate::vessel::{BurnLaw, DirectionLaw, G0};
use glam::DVec3;

/// Sea-level pressure the Isp pair refers to (Pa).
pub const P_SEA_LEVEL: f64 = 101_325.0;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Engine {
    /// Vacuum thrust at full throttle (N).
    pub thrust_vac: f64,
    /// Specific impulse in vacuum and at sea level (s).
    pub isp_vac: f64,
    pub isp_sl: f64,
    /// Lowest throttle while running.
    pub min_throttle: f64,
    /// Gimbal range (rad).
    pub gimbal: f64,
    /// Where the thrust acts and its direction (unit), body axes.
    pub mount_pos: DVec3,
    pub mount_dir: DVec3,
    /// Nozzle exit area (m²), from the Isp pair.
    pub exit_area: f64,
}

/// Thrust (N) and propellant mass flow (kg/s).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EngineOutput {
    pub thrust: f64,
    pub mdot: f64,
}

impl Engine {
    pub fn from_file(f: &EngineFile) -> Self {
        Engine {
            thrust_vac: f.thrust_vac,
            isp_vac: f.isp_vac,
            isp_sl: f.isp_sl,
            min_throttle: f.min_throttle,
            gimbal: f.gimbal_deg * math::PI / 180.0,
            mount_pos: f.mount.pos,
            mount_dir: f.mount.dir.normalize(),
            exit_area: f.thrust_vac * (1.0 - f.isp_sl / f.isp_vac) / P_SEA_LEVEL,
        }
    }

    /// Mass flow at full throttle (kg/s).
    pub fn mdot_max(&self) -> f64 {
        self.thrust_vac / (self.isp_vac * G0)
    }

    /// The throttle the engine actually runs at: off at or below zero,
    /// otherwise within [min_throttle, 1].
    pub fn setting(&self, throttle: f64) -> f64 {
        if throttle > 0.0 {
            throttle.clamp(self.min_throttle, 1.0)
        } else {
            0.0
        }
    }

    /// Specific impulse (s) at ambient pressure `p` (Pa); zero once the back
    /// pressure cancels the thrust.
    pub fn isp(&self, p: f64) -> f64 {
        (self.isp_vac * (self.thrust_vac - self.exit_area * p) / self.thrust_vac).max(0.0)
    }

    /// Thrust and mass flow at `throttle` and ambient pressure `p` (Pa).
    pub fn output(&self, throttle: f64, p: f64) -> EngineOutput {
        let s = self.setting(throttle);
        EngineOutput { thrust: s * (self.thrust_vac - self.exit_area * p).max(0.0), mdot: s * self.mdot_max() }
    }

    /// Output over a tick of `dt` seconds with `propellant` kg left: none
    /// without propellant; the last partial tick burns what is left (thrust
    /// and flow scaled together). `infinite` (debug mode, D064) ignores the
    /// propellant.
    pub fn tick_output(&self, throttle: f64, p: f64, propellant: f64, dt: f64, infinite: bool) -> EngineOutput {
        let out = self.output(throttle, p);
        if infinite || out.mdot * dt <= propellant {
            return out;
        }
        let f = (propellant / (out.mdot * dt)).max(0.0);
        EngineOutput { thrust: out.thrust * f, mdot: out.mdot * f }
    }

    /// A planned burn at full throttle in vacuum (burns on rails run above
    /// the atmosphere).
    pub fn burn_law(&self, direction: DirectionLaw) -> BurnLaw {
        BurnLaw::from_isp(self.thrust_vac, self.isp_vac, direction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::test_craft;

    fn engine() -> Engine {
        Engine::from_file(&test_craft().spec.engine)
    }

    #[test]
    fn output_table() {
        let e = engine();
        let mdot = 300e3 / (320.0 * G0);
        let f_sl = 300e3 * 280.0 / 320.0;
        // (throttle, pressure) → (thrust, mdot)
        let cases = [
            (1.0, 0.0, 300e3, mdot),
            (1.0, P_SEA_LEVEL, f_sl, mdot),
            (0.5, 0.0, 150e3, 0.5 * mdot),
            (0.5, P_SEA_LEVEL, 0.5 * f_sl, 0.5 * mdot),
            (0.05, 0.0, 30e3, 0.1 * mdot), // below the minimum: runs at 10 %
            (0.0, 0.0, 0.0, 0.0),
            (-1.0, 0.0, 0.0, 0.0),
            (2.0, 0.0, 300e3, mdot),
            (1.0, 1e7, 0.0, mdot), // back pressure beyond the nozzle's design: no thrust
        ];
        for (throttle, p, thrust, flow) in cases {
            let o = e.output(throttle, p);
            assert!((o.thrust - thrust).abs() < 1e-6 && (o.mdot - flow).abs() < 1e-12, "{throttle} {p}: {o:?}");
        }
        assert!((e.isp(0.0) - 320.0).abs() < 1e-12 && (e.isp(P_SEA_LEVEL) - 280.0).abs() < 1e-9);
        // Isp = F / (ṁ g0) at any pressure.
        for p in [0.0, 2e4, P_SEA_LEVEL] {
            let o = e.output(0.7, p);
            assert!((o.thrust / (o.mdot * G0) - e.isp(p)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_vacuum_burn_follows_the_rocket_equation() {
        // Integrate a = F/m with ṁ from the engine; compare with Isp·g0·ln(m0/m1).
        let e = engine();
        for (throttle, m0, seconds) in [(1.0, 20_000.0, 100.0), (0.3, 20_000.0, 400.0), (1.0, 6_000.0, 30.0)] {
            let o = e.output(throttle, 0.0);
            let (mut m, mut v, dt) = (m0, 0.0, 1e-3);
            let steps = (seconds / dt) as usize;
            for _ in 0..steps {
                // Midpoint in mass: exact for this ODE to O(dt²).
                v += o.thrust / (m - 0.5 * o.mdot * dt) * dt;
                m -= o.mdot * dt;
            }
            let exact = e.isp_vac * G0 * math::ln(m0 / m);
            assert!((v / exact - 1.0).abs() < 1e-9, "{throttle}: {v} vs {exact}");
        }
    }

    #[test]
    fn no_propellant_no_thrust_unless_debug() {
        let e = engine();
        let full = e.output(1.0, 0.0);
        assert_eq!(e.tick_output(1.0, 0.0, 0.0, 0.02, false), EngineOutput::default());
        assert_eq!(e.tick_output(1.0, 0.0, 0.0, 0.02, true), full);
        assert_eq!(e.tick_output(1.0, 0.0, 1000.0, 0.02, false), full);
        // The last partial tick burns exactly what is left.
        let left = 0.25 * full.mdot * 0.02;
        let o = e.tick_output(1.0, 0.0, left, 0.02, false);
        assert!((o.mdot * 0.02 - left).abs() < 1e-15 && (o.thrust - 0.25 * full.thrust).abs() < 1e-9);
    }

    #[test]
    fn planned_burns_use_the_vacuum_engine() {
        let e = engine();
        let law = e.burn_law(DirectionLaw::Inertial(DVec3::X));
        assert_eq!((law.thrust, law.mass_flow), (300e3, e.mdot_max()));
        // Δv of a full tank: ≈ 5.05 km/s (20 t → 4 t at 320 s).
        let dv = e.isp_vac * G0 * math::ln(20_000.0 / 4_000.0);
        assert!((dv - 5050.6).abs() < 1.0, "{dv}");
    }

    #[test]
    fn earth_back_pressure_table() {
        let atm = crate::body::earth().atmosphere.unwrap();
        for (h, p) in
            [(0.0, P_SEA_LEVEL), (7200.0, P_SEA_LEVEL / std::f64::consts::E), (-50.0, P_SEA_LEVEL), (150e3, 0.0)]
        {
            assert!((atm.pressure(h) - p).abs() < 1e-9, "{h} m: {}", atm.pressure(h));
        }
    }
}
