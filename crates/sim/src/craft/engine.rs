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
use crate::vessel::G0;
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
    /// Share of the jet power absorbed by the nozzle's wall (D077).
    pub nozzle_heat_fraction: f64,
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
            nozzle_heat_fraction: f.nozzle_heat_fraction,
        }
    }

    /// Heat absorbed by the nozzle's wall (W) while giving `thrust` (N) at
    /// mass flow `mdot` (kg/s): the share of the jet power ½·F·vₑ, vₑ = F/ṁ.
    pub fn heat(&self, thrust: f64, mdot: f64) -> f64 {
        if mdot > 0.0 {
            self.nozzle_heat_fraction * 0.5 * thrust * thrust / mdot
        } else {
            0.0
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
}

/// The engines of a craft of parts running together over a tick
/// (`systems::propulsion::output`): each at its own mount with its own Isp
/// pair and level, drawing its oxidizer and fuel by its mixture ratio from
/// the first of its feed tanks that holds any; an engine whose tanks cannot
/// supply the whole tick gives that share of it (the last partial tick, as
/// [`Engine::tick_output`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShipThrust {
    pub thrust: f64,
    pub mdot: f64,
    /// Thrust-weighted mount and direction (body axes; unit), and the
    /// smallest gimbal range of the engines giving thrust (rad): all
    /// deflecting together, their torque is the summed thrust's about the
    /// weighted mount, exactly.
    pub pos: DVec3,
    pub dir: DVec3,
    pub gimbal: f64,
    /// Heat into the nozzles' walls (W, D077).
    pub heat: f64,
    /// Each tank's draw (kg/s).
    pub draws: Vec<f64>,
    /// Each engine's thrust (N; aligned with `ShipDef::engines`): what a
    /// flame is drawn from.
    pub per_engine: Vec<f64>,
}

/// The engine file of one engine of a craft of parts (its gimbal point and
/// direction as given).
pub fn file_of(d: &crate::catalog::EngineData) -> EngineFile {
    EngineFile {
        thrust_vac: d.thrust_vac,
        isp_vac: d.isp_vac,
        isp_sl: d.isp_sl,
        min_throttle: d.throttle_min,
        gimbal_deg: d.gimbal_deg,
        mount: super::file::Mount { pos: d.pos, dir: d.dir },
        nozzle_heat_fraction: d.nozzle_heat_fraction,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A 300 kN engine (Isp 320 s in vacuum, 280 s at sea level, down to
    /// 10 %) at the base of a craft, thrusting along +Z.
    pub(crate) fn test_file() -> EngineFile {
        EngineFile {
            thrust_vac: 300e3,
            isp_vac: 320.0,
            isp_sl: 280.0,
            min_throttle: 0.1,
            gimbal_deg: 5.0,
            mount: super::super::file::Mount { pos: DVec3::new(0.0, 0.0, -5.9), dir: DVec3::Z },
            nozzle_heat_fraction: 2.2e-3,
        }
    }

    #[test]
    fn engine_heat_is_a_share_of_the_jet_power() {
        let e = engine();
        let out = e.output(1.0, 0.0);
        let jet = 0.5 * out.thrust * e.isp_vac * G0;
        assert!((e.heat(out.thrust, out.mdot) / (e.nozzle_heat_fraction * jet) - 1.0).abs() < 1e-12);
        assert_eq!(e.heat(0.0, 0.0), 0.0);
    }

    fn engine() -> Engine {
        Engine::from_file(&test_file())
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
    fn earth_back_pressure_table() {
        // US 1976: 54,048 Pa at 5 km; zero from the atmosphere's top.
        let atm = crate::body::earth().atmosphere.unwrap();
        for (h, p) in [(0.0, P_SEA_LEVEL), (5000.0, 54_048.0), (-50.0, P_SEA_LEVEL), (150e3, 0.0)] {
            assert!((atm.pressure(h) - p).abs() < 1e-3 * p.max(1.0), "{h} m: {}", atm.pressure(h));
        }
    }
}
