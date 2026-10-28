//! The engine generator (D104: published engines before the engine
//! designer). The geometry (powerhead, nozzle from throat to exit, a
//! radiatively cooled extension) comes from the parameters; the
//! performance (thrust, Isp) from the entry's published numbers, or from
//! the parameters' unverified `performance` (sandbox engines), which a
//! published number then overrides with the gap reported. The dry mass is
//! derived from a thrust-to-weight ratio typical of the cycle (an estimate
//! the published mass replaces).
//!
//! Part frame: the gimbal point at the origin, thrust (the force on the
//! craft) along +Z, the nozzle below along −Z.

use super::output::{Cycle, EngineData, Function, Generated, Igniter, Kind, MountKind, MountPoint, Port, PortKind};
use super::wall::{self, Wall};
use super::{settable, Ctx, Published};
use crate::math;
use crate::vessel::G0;
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// What a published number may override on an engine.
pub const OVERRIDABLE: &[&str] = &["thrust_vac", "thrust_sl", "isp_vac", "isp_sl"];

/// Unverified performance (sandbox engines, D101): published numbers win.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Performance {
    pub thrust_vac: f64,
    pub isp_vac: f64,
    pub isp_sl: f64,
}

/// A radiatively cooled nozzle extension from `start` (a share of the
/// nozzle's length below the throat) to the exit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extension {
    pub material: String,
    pub thickness: f64,
    #[serde(default)]
    pub finish: Option<String>,
    pub start: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineParams {
    pub cycle: Cycle,
    pub oxidizer: String,
    pub fuel: String,
    pub mixture_ratio: f64,
    #[serde(default)]
    pub performance: Option<Performance>,
    pub throttle_min: f64,
    #[serde(default = "one")]
    pub throttle_max: f64,
    pub gimbal_deg: f64,
    #[serde(default)]
    pub gimbal_rate_dps: f64,
    pub ignitions: u32,
    pub igniter: Igniter,
    pub start_time: f64,
    #[serde(default)]
    pub shutdown_impulse: f64,
    #[serde(default)]
    pub npsp_ox: f64,
    #[serde(default)]
    pub npsp_fuel: f64,
    /// D077's share of the jet power the nozzle's cooled-by-radiation wall takes.
    #[serde(default)]
    pub nozzle_heat_fraction: f64,
    pub throat_diameter: f64,
    pub exit_diameter: f64,
    pub nozzle_length: f64,
    pub powerhead_length: f64,
    pub powerhead_radius: f64,
    /// The nozzle's wall (regeneratively cooled: the liner and jacket).
    pub nozzle_material: String,
    pub nozzle_thickness: f64,
    #[serde(default)]
    pub extension: Option<Extension>,
    /// Vacuum thrust over sea-level weight, typical of the cycle: the
    /// derived dry mass.
    pub thrust_to_weight: f64,
}

fn one() -> f64 {
    1.0
}

settable!(EngineParams {
    mixture_ratio,
    throttle_min,
    throttle_max,
    gimbal_deg,
    gimbal_rate_dps,
    ignitions,
    start_time,
    shutdown_impulse,
    npsp_ox,
    npsp_fuel,
    nozzle_heat_fraction,
    throat_diameter,
    exit_diameter,
    nozzle_length,
    powerhead_length,
    powerhead_radius,
    nozzle_thickness,
    thrust_to_weight
});

impl EngineParams {
    /// The performance: the published numbers over the parameters'. A
    /// published sea-level thrust gives the sea-level Isp (the mass flow
    /// does not depend on the back pressure: Isp_sl = Isp_vac·F_sl/F_vac).
    fn performance(&self, published: &Published) -> Result<Performance, String> {
        let get = |k: &str| published.get(k).map(|s| s.0);
        let own = self.performance;
        let thrust_vac = get("thrust_vac").or(own.map(|p| p.thrust_vac));
        let isp_vac = get("isp_vac").or(own.map(|p| p.isp_vac));
        let (Some(thrust_vac), Some(isp_vac)) = (thrust_vac, isp_vac) else {
            return Err("engine: needs published thrust_vac and isp_vac (or unverified performance)".into());
        };
        let from_sl_thrust = get("thrust_sl").map(|f| isp_vac * f / thrust_vac);
        let isp_sl = get("isp_sl").or(from_sl_thrust).or(own.map(|p| p.isp_sl));
        let Some(isp_sl) = isp_sl else { return Err("engine: needs isp_sl or thrust_sl".into()) };
        if !(thrust_vac > 0.0 && isp_vac > 0.0 && isp_sl > 0.0 && isp_sl <= isp_vac) {
            return Err(format!(
                "engine: needs thrust > 0 and 0 < Isp_sl ≤ Isp_vac, got {thrust_vac}, {isp_vac}, {isp_sl}"
            ));
        }
        Ok(Performance { thrust_vac, isp_vac, isp_sl })
    }

    /// The engine's data at its gimbal point (part frame).
    pub fn data(&self, ctx: &Ctx, published: &Published) -> Result<EngineData, String> {
        let p = self.performance(published)?;
        ctx.fluids.liquid(&self.oxidizer)?;
        ctx.fluids.liquid(&self.fuel)?;
        let ok = self.mixture_ratio > 0.0
            && self.throttle_min > 0.0
            && self.throttle_min <= self.throttle_max
            && self.throttle_max <= 1.0
            && (0.0..=30.0).contains(&self.gimbal_deg)
            && (0.0..1.0).contains(&self.nozzle_heat_fraction)
            && self.exit_diameter > self.throat_diameter
            && self.throat_diameter > 0.0;
        if !ok {
            return Err("engine: needs mixture ratio > 0, 0 < throttle_min ≤ throttle_max ≤ 1, gimbal in [0, 30]°, \
                        nozzle heat in [0, 1), exit wider than the throat"
                .into());
        }
        Ok(EngineData {
            cycle: self.cycle,
            oxidizer: self.oxidizer.clone(),
            fuel: self.fuel.clone(),
            mixture_ratio: self.mixture_ratio,
            thrust_vac: p.thrust_vac,
            isp_vac: p.isp_vac,
            isp_sl: p.isp_sl,
            throttle_min: self.throttle_min,
            throttle_max: self.throttle_max,
            gimbal_deg: self.gimbal_deg,
            gimbal_rate_dps: self.gimbal_rate_dps,
            ignitions: self.ignitions,
            igniter: self.igniter,
            start_time: self.start_time,
            shutdown_impulse: self.shutdown_impulse,
            npsp_ox: self.npsp_ox,
            npsp_fuel: self.npsp_fuel,
            nozzle_heat_fraction: self.nozzle_heat_fraction,
            exit_area: math::PI * self.exit_diameter * self.exit_diameter / 4.0,
            throat_area: math::PI * self.throat_diameter * self.throat_diameter / 4.0,
            pos: DVec3::ZERO,
            dir: DVec3::Z,
        })
    }

    /// The derived dry mass (kg): vacuum thrust over the cycle's
    /// thrust-to-weight.
    pub fn derived_mass(&self, thrust_vac: f64) -> f64 {
        thrust_vac / (G0 * self.thrust_to_weight)
    }

    /// The parameters' own performance, as derived outputs.
    fn outputs(&self) -> Vec<(String, f64)> {
        let Some(p) = self.performance else { return Vec::new() };
        vec![
            ("thrust_vac".into(), p.thrust_vac),
            ("isp_vac".into(), p.isp_vac),
            ("isp_sl".into(), p.isp_sl),
            ("thrust_sl".into(), p.thrust_vac * p.isp_sl / p.isp_vac),
        ]
    }

    pub fn generate(&self, ctx: &Ctx, published: &Published) -> Result<Generated, String> {
        let data = self.data(ctx, published)?;
        if !(self.thrust_to_weight > 0.0 && self.nozzle_length > 0.0 && self.powerhead_length > 0.0) {
            return Err("engine: thrust_to_weight, nozzle_length and powerhead_length must be positive".into());
        }
        let m = ctx.materials;
        let nozzle_wall = Wall::Solid { material: self.nozzle_material.clone(), thickness: self.nozzle_thickness };
        let skin = nozzle_wall.skin(m, None)?;
        let (rt, re, ph, nl) =
            (self.throat_diameter / 2.0, self.exit_diameter / 2.0, self.powerhead_length, self.nozzle_length);
        let mut primitives = vec![wall::primitive(
            "powerhead",
            wall::along_z(-ph, 0.0, self.powerhead_radius, self.powerhead_radius),
            skin,
        )];
        let exit_z = -ph - nl;
        match &self.extension {
            Some(x) => {
                if !(x.start > 0.0 && x.start < 1.0) {
                    return Err("engine: extension.start must be in (0, 1)".into());
                }
                let (zs, rs) = (-ph - x.start * nl, rt + (re - rt) * x.start);
                primitives.push(wall::primitive("nozzle", wall::along_z(zs, -ph, rs, rt), skin));
                let ext_skin = Wall::Solid { material: x.material.clone(), thickness: x.thickness }
                    .skin(m, wall::finish(m, &x.finish)?)?;
                let mut ext = wall::primitive("nozzle extension", wall::along_z(exit_z, zs, re, rs), ext_skin);
                ext.nozzle = true;
                primitives.push(ext);
            }
            None => {
                let mut bell = wall::primitive("nozzle", wall::along_z(exit_z, -ph, re, rt), skin);
                bell.nozzle = true;
                primitives.push(bell);
            }
        }
        let mass = self.derived_mass(data.thrust_vac);
        let derived = vec![("engine (thrust-to-weight of its cycle)".to_string(), mass)];
        let port = |name: &str, fluid: &str| Port {
            name: name.into(),
            kind: PortKind::Inlet,
            fluid: Some(fluid.into()),
            pos: DVec3::new(0.0, 0.0, -0.1),
        };
        let nozzle_material = m.get(&self.nozzle_material)?;
        Ok(Generated {
            kind: Kind::Part,
            primitives,
            envelope: None,
            mounts: vec![MountPoint {
                name: "mount".into(),
                pos: DVec3::ZERO,
                normal: DVec3::Z,
                roll: DVec3::X,
                size: 2.0 * self.powerhead_radius,
                kind: MountKind::Engine,
                allowable: 0.0,
                buckling: 0.0,
            }],
            mass,
            derived,
            outputs: self.outputs(),
            material: self.nozzle_material.clone(),
            finish: None,
            tanks: Vec::new(),
            ports: vec![port("ox_in", &self.oxidizer), port("fuel_in", &self.fuel)],
            functions: vec![Function::Engine(data)],
            power: 0.0,
            specific_heat: nozzle_material.specific_heat,
            max_k: nozzle_material.service_k,
        })
    }
}
