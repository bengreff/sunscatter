//! Components (D100): equipment inside a part, with mass, volume, power and
//! heat but no outer shape: pressurant bottles, small tanks, batteries,
//! computers, sensors, valves, seats, thruster blocks, engine pods, life
//! support, parachute packs, docking ports.
//!
//! Component frame: the envelope's centre at the origin, its axis +Z. What
//! physics gives is derived (a bottle's mass from its burst pressure ×
//! volume over its performance factor, a tank's wall from its pressure, a
//! battery's from its capacity and specific energy); the rest carries an
//! estimated mass that a published one replaces.

use super::engine::{self, EngineParams};
use super::output::{
    Battery, Bottle, Computer, Function, Generated, Kind, LifeSupport, Nozzle, Port, PortKind, Propellant, Sensor,
    Solar, TankSpec, Thrusters, Valve, ValveKind,
};
use super::{assign, Ctx, Published};
use crate::craft::Shape;
use crate::math;
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// A component's outer volume, centred on its origin.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Envelope {
    /// Full sizes along X, Y, Z.
    Box(DVec3),
    /// Radius and length along Z.
    Cylinder(f64, f64),
    Sphere(f64),
}

impl Envelope {
    pub fn shape(&self) -> Shape {
        match *self {
            Envelope::Box(size) => Shape::Box { center: DVec3::ZERO, half: size / 2.0 },
            Envelope::Cylinder(r, l) => Shape::Cylinder { base: DVec3::Z * (-l / 2.0), axis: DVec3::Z * l, radius: r },
            Envelope::Sphere(r) => Shape::SphereCap { center: DVec3::ZERO, axis: DVec3::Z, radius: r, height: 2.0 * r },
        }
    }

    /// m³.
    pub fn volume(&self) -> f64 {
        crate::craft::mesh::analytic_volume(&self.shape())
    }

    /// The radius of a cylinder or sphere, the half width of a box.
    fn radius(&self) -> f64 {
        match *self {
            Envelope::Box(s) => 0.5 * s.x.min(s.y),
            Envelope::Cylinder(r, _) | Envelope::Sphere(r) => r,
        }
    }
}

/// What the component is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Equipment {
    /// A pressurant bottle (COPV): its envelope's volume of `gas` at
    /// `pressure` (MEOP). Mass = burst pressure × volume / `performance`
    /// (J/kg; composite-overwrapped bottles reach ~2.5e5).
    Bottle {
        gas: String,
        pressure: f64,
        temperature_k: f64,
        safety_factor: f64,
        performance: f64,
    },
    /// A small propellant tank (a sphere or capped cylinder) of `material`
    /// sized for `pressure`.
    Tank {
        fluid: String,
        pressure: f64,
        safety_factor: f64,
        material: String,
        min_gauge: f64,
        ullage: f64,
    },
    /// Mass = capacity / specific energy.
    Battery {
        capacity_wh: f64,
        voltage: f64,
        specific_energy: f64,
        internal_resistance: f64,
        max_power: f64,
    },
    Computer {
        channels: u32,
        mass: f64,
    },
    Sensor {
        quantity: String,
        accuracy: f64,
        rate_hz: f64,
        mass: f64,
    },
    Valve {
        kind: ValveKind,
        diameter: f64,
        open: bool,
        cycles: f64,
        mass: f64,
    },
    Seat {
        crew: u32,
        mass: f64,
    },
    /// Reaction-control thrusters at `nozzles` (component frame).
    Thrusters {
        thrust: f64,
        isp: f64,
        propellant: Propellant,
        nozzles: Vec<Nozzle>,
        enabled: bool,
        mass: f64,
    },
    /// Engines of one design at `nozzles` (a SuperDraco pod: two), fixed.
    Engines {
        engine: EngineParams,
        nozzles: Vec<Nozzle>,
        housing: f64,
    },
    LifeSupport {
        co2_removal: f64,
        o2_supply: f64,
        cabin_volume: f64,
        mass: f64,
    },
    /// A parachute system's packs and mortars.
    Chute {
        spec: crate::chute::ChuteSpec,
        mass: f64,
    },
    Docking {
        standard: String,
        diameter: f64,
        mass: f64,
    },
    Solar {
        area: f64,
        efficiency: f64,
        mass: f64,
    },
    /// Anything else: an avionics box, harnesses, ballast.
    Other {
        what: String,
        mass: f64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentParams {
    pub envelope: Envelope,
    pub equipment: Equipment,
    /// Electrical draw when on (W).
    #[serde(default)]
    pub power: f64,
    /// Makes the craft commandable (a flight computer, D083).
    #[serde(default)]
    pub control: bool,
    #[serde(default = "specific_heat")]
    pub specific_heat: f64,
    #[serde(default = "max_k")]
    pub max_k: f64,
}

fn specific_heat() -> f64 {
    900.0
}

fn max_k() -> f64 {
    400.0
}

impl super::Settable for ComponentParams {
    fn set(&mut self, key: &str, value: f64) -> Result<(), String> {
        match (key, &mut self.equipment) {
            ("power", _) => assign(&mut self.power, value, key),
            ("pressure", Equipment::Bottle { pressure, .. } | Equipment::Tank { pressure, .. }) => {
                assign(pressure, value, key)
            }
            ("capacity_wh", Equipment::Battery { capacity_wh, .. }) => assign(capacity_wh, value, key),
            ("thrust", Equipment::Thrusters { thrust, .. }) => assign(thrust, value, key),
            ("crew", Equipment::Seat { crew, .. }) => assign(crew, value, key),
            ("radius", _) => match &mut self.envelope {
                Envelope::Cylinder(r, _) | Envelope::Sphere(r) => assign(r, value, key),
                Envelope::Box(_) => Err("a box has no radius".into()),
            },
            (
                "mass",
                Equipment::Computer { mass, .. }
                | Equipment::Sensor { mass, .. }
                | Equipment::Valve { mass, .. }
                | Equipment::Seat { mass, .. }
                | Equipment::Thrusters { mass, .. }
                | Equipment::LifeSupport { mass, .. }
                | Equipment::Chute { mass, .. }
                | Equipment::Docking { mass, .. }
                | Equipment::Solar { mass, .. }
                | Equipment::Other { mass, .. },
            ) => assign(mass, value, key),
            _ => Err(format!("this component has no settable parameter {key:?}")),
        }
    }
}

impl ComponentParams {
    /// Published outputs this component may take.
    pub fn overridable(&self) -> &'static [&'static str] {
        match self.equipment {
            Equipment::Battery { .. } => &["capacity_wh"],
            Equipment::Thrusters { .. } => &["thrust", "isp"],
            Equipment::Engines { .. } => engine::OVERRIDABLE,
            _ => &[],
        }
    }

    pub fn generate(&self, ctx: &Ctx, published: &Published) -> Result<Generated, String> {
        let shape = self.envelope.shape();
        let volume = self.envelope.volume();
        let mut g = Generated {
            kind: Kind::Component,
            primitives: Vec::new(),
            envelope: Some(shape),
            mounts: Vec::new(),
            mass: 0.0,
            derived: Vec::new(),
            outputs: Vec::new(),
            material: String::new(),
            finish: None,
            tanks: Vec::new(),
            ports: Vec::new(),
            functions: Vec::new(),
            power: self.power,
            specific_heat: self.specific_heat,
            max_k: self.max_k,
        };
        let estimate = |what: &str, mass: f64| (format!("{what} (estimate)"), mass);
        let (derived, functions) = (&mut g.derived, &mut g.functions);
        match &self.equipment {
            Equipment::Bottle { .. } | Equipment::Tank { .. } => self.vessel(ctx, volume, &mut g)?,
            Equipment::Battery { .. } | Equipment::Thrusters { .. } | Equipment::Engines { .. } => {
                self.active(ctx, published, &mut g)?
            }
            Equipment::Computer { channels, mass } => {
                derived.push(estimate("flight computer", *mass));
                functions.push(Function::Computer(Computer { channels: *channels }));
            }
            Equipment::Sensor { quantity, accuracy, rate_hz, mass } => {
                derived.push(estimate("sensor", *mass));
                let sensor = Sensor { quantity: quantity.clone(), accuracy: *accuracy, rate_hz: *rate_hz };
                functions.push(Function::Sensor(sensor));
            }
            Equipment::Valve { kind, diameter, open, cycles, mass } => {
                derived.push(estimate("valve", *mass));
                let valve = Valve { kind: *kind, diameter: *diameter, open: *open, cycles: *cycles };
                functions.push(Function::Valve(valve));
                g.ports.push(port("in", PortKind::Through, None));
                g.ports.push(port("out", PortKind::Through, None));
            }
            Equipment::Seat { crew, mass } => {
                derived.push(estimate("seat", *mass));
                functions.push(Function::Seat(*crew));
            }
            Equipment::LifeSupport { co2_removal, o2_supply, cabin_volume, mass } => {
                derived.push(estimate("life support", *mass));
                let ls = LifeSupport { co2_removal: *co2_removal, o2_supply: *o2_supply, cabin_volume: *cabin_volume };
                functions.push(Function::LifeSupport(ls));
            }
            Equipment::Chute { spec, mass } => {
                derived.push(estimate("parachutes and mortars", *mass));
                functions.push(Function::Chute(spec.clone()));
            }
            Equipment::Docking { standard, diameter, mass } => {
                derived.push(estimate("docking system", *mass));
                functions.push(Function::Docking(standard.clone(), *diameter));
            }
            Equipment::Solar { area, efficiency, mass } => {
                derived.push(estimate("solar array", *mass));
                functions.push(Function::Solar(Solar { area: *area, efficiency: *efficiency }));
            }
            Equipment::Other { what, mass } => derived.push(estimate(what, *mass)),
        }
        if self.control {
            g.functions.push(Function::Control);
        }
        if g.derived.iter().any(|d| !(d.1.is_finite() && d.1 >= 0.0)) || !(volume.is_finite() && volume > 0.0) {
            return Err("component: masses must be non-negative and the envelope's volume positive".into());
        }
        g.mass = g.derived_mass();
        Ok(g)
    }

    /// Bottles and tanks: walls from their pressure.
    fn vessel(&self, ctx: &Ctx, volume: f64, g: &mut Generated) -> Result<(), String> {
        match &self.equipment {
            Equipment::Bottle { gas, pressure, temperature_k, safety_factor, performance } => {
                ctx.fluids.gas(gas)?;
                if !(*pressure > 0.0 && *safety_factor >= 1.0 && *performance > 0.0 && *temperature_k > 0.0) {
                    return Err("bottle: pressure, temperature, performance positive; safety factor ≥ 1".into());
                }
                let burst = pressure * safety_factor;
                g.derived.push(("bottle (burst × volume / performance)".into(), burst * volume / performance));
                let bottle =
                    Bottle { gas: gas.clone(), volume, pressure: *pressure, burst, temperature_k: *temperature_k };
                g.functions.push(Function::Bottle(bottle));
                g.ports.push(port("out", PortKind::Gas, Some(gas.clone())));
            }
            Equipment::Tank { fluid, pressure, safety_factor, material, min_gauge, ullage } => {
                ctx.fluids.liquid(fluid)?;
                let m = ctx.materials.get(material)?;
                let r = self.envelope.radius();
                let sigma = m.strength_at(super::wall::ROOM_K).1;
                // A sphere's membrane stress p·r/(2t); a cylinder's hoop p·r/t.
                let k = if matches!(self.envelope, Envelope::Sphere(_)) { 0.5 } else { 1.0 };
                let t = (safety_factor * pressure * r * k / sigma).max(*min_gauge);
                let area = crate::craft::mesh::analytic_area(&self.envelope.shape());
                g.derived.push(("tank wall".into(), area * t * m.density));
                let height = volume / (math::PI * r * r);
                g.tanks.push(TankSpec {
                    name: "main".into(),
                    fluid: fluid.clone(),
                    volume,
                    base: DVec3::Z * (-height / 2.0),
                    axis: DVec3::Z,
                    radius: r,
                    pressure: *pressure,
                    burst: pressure * safety_factor,
                    ullage: *ullage,
                });
                g.ports.push(port("main.outlet", PortKind::Outlet, Some(fluid.clone())));
                g.ports.push(port("main.press", PortKind::Pressurant, None));
                g.ports.push(port("main.vent", PortKind::Vent, None));
            }
            _ => unreachable!("vessel() is for bottles and tanks"),
        }
        Ok(())
    }

    /// Batteries, thrusters and engines: their published outputs apply.
    fn active(&self, ctx: &Ctx, published: &Published, g: &mut Generated) -> Result<(), String> {
        let estimate = |what: &str, mass: f64| (format!("{what} (estimate)"), mass);
        match &self.equipment {
            Equipment::Battery { capacity_wh, voltage, specific_energy, internal_resistance, max_power } => {
                let capacity = published.get("capacity_wh").map_or(*capacity_wh, |p| p.0);
                g.outputs.push(("capacity_wh".to_string(), *capacity_wh));
                if !(capacity > 0.0 && *specific_energy > 0.0 && *voltage > 0.0) {
                    return Err("battery: capacity, specific energy and voltage must be positive".into());
                }
                g.derived.push(("cells (capacity / specific energy)".into(), capacity_wh / specific_energy));
                g.functions.push(Function::Battery(Battery {
                    capacity: capacity * 3600.0,
                    voltage: *voltage,
                    internal_resistance: *internal_resistance,
                    max_power: *max_power,
                }));
            }
            Equipment::Thrusters { thrust, isp, propellant, nozzles, enabled, mass } => {
                g.outputs.push(("thrust".to_string(), *thrust));
                g.outputs.push(("isp".to_string(), *isp));
                let thrust = published.get("thrust").map_or(*thrust, |p| p.0);
                let isp = published.get("isp").map_or(*isp, |p| p.0);
                if !(thrust > 0.0 && isp > 0.0 && !nozzles.is_empty()) {
                    return Err("thrusters: thrust and Isp positive, at least one nozzle".into());
                }
                g.derived.push(estimate("thruster block", *mass));
                match propellant {
                    Propellant::ColdGas(gas) => {
                        ctx.fluids.gas(gas)?;
                        g.ports.push(port("in", PortKind::Gas, Some(gas.clone())));
                    }
                    Propellant::Biprop { oxidizer, fuel, .. } => {
                        g.ports.push(port("ox_in", PortKind::Inlet, Some(oxidizer.clone())));
                        g.ports.push(port("fuel_in", PortKind::Inlet, Some(fuel.clone())));
                    }
                }
                let nozzles = nozzles.iter().map(|n| Nozzle { pos: n.pos, dir: n.dir.normalize() }).collect();
                let t = Thrusters { thrust, isp, propellant: propellant.clone(), nozzles, enabled: *enabled };
                g.functions.push(Function::Thrusters(t));
            }
            Equipment::Engines { engine, nozzles, housing } => {
                let data = engine.data(ctx, published)?;
                if nozzles.is_empty() {
                    return Err("engines: at least one nozzle".into());
                }
                let each = engine.derived_mass(data.thrust_vac);
                g.derived.push((format!("{} engines (thrust-to-weight)", nozzles.len()), each * nozzles.len() as f64));
                g.derived.push(estimate("housing", *housing));
                for n in nozzles {
                    let one = super::output::EngineData { pos: n.pos, dir: n.dir.normalize(), ..data.clone() };
                    g.functions.push(Function::Engine(one));
                }
                g.ports.push(port("ox_in", PortKind::Inlet, Some(engine.oxidizer.clone())));
                g.ports.push(port("fuel_in", PortKind::Inlet, Some(engine.fuel.clone())));
            }
            _ => unreachable!("active() is for batteries, thrusters and engines"),
        }
        Ok(())
    }
}

/// A port at the component's centre.
fn port(name: &str, kind: PortKind, fluid: Option<String>) -> Port {
    Port { name: name.into(), kind, fluid, pos: DVec3::ZERO }
}
