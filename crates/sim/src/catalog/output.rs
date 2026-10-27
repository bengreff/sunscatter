//! What a generator gives (D101): a part's primitives in its own frame, its
//! mounting points, its derived mass, its tanks and ports, the functions it
//! provides and its ratings. The craft build places these
//! (`sim::craft::build`).
//!
//! A part's frame: +Z is its axis (up the stack, the nose), its origin the
//! bottom centre of a stack part (an engine's: the gimbal point), +X its roll
//! reference. Positions are metres, directions unit vectors.

use crate::craft::{Primitive, Shape};
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// A part (with a shape, cells and a place in the attachment tree) or a
/// component (equipment inside a part: mass, volume, power, heat, no outer
/// shape).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Part,
    Component,
}

/// What a mounting point joins (D102): a part attaches only through a
/// mounting point to a compatible one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MountKind {
    /// Stage to stage along the axis (sizes must match).
    Stack,
    /// An engine's gimbal mount to a thrust structure.
    Engine,
    /// A part on another's side (legs, fins, pods).
    Radial,
}

/// A mounting point, in the part's frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MountPoint {
    pub name: String,
    pub pos: DVec3,
    /// Outward normal (unit).
    pub normal: DVec3,
    /// Roll reference (unit, perpendicular to the normal).
    pub roll: DVec3,
    /// Diameter (m): stack mounts must match.
    pub size: f64,
    pub kind: MountKind,
    /// Line load the part's wall carries there at room temperature (N/m;
    /// zero where the part takes no stack load): a joint's capacity is the
    /// weaker side's.
    pub allowable: f64,
}

/// A tank compartment: its volume, the fluid it was designed for, where its
/// liquid sits (a solid cylinder of the compartment's volume filling from
/// its lowest point along `axis`) and its pressures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TankSpec {
    /// Compartment name (`lox`, `rp1`, …): ports are `<name>.outlet` etc.
    pub name: String,
    pub fluid: String,
    /// m³ (inside the walls, before components displace any).
    pub volume: f64,
    /// The liquid's region: base (lowest point), axis (unit, the way it
    /// fills) and radius, part frame.
    pub base: DVec3,
    pub axis: DVec3,
    pub radius: f64,
    /// Maximum expected operating pressure (Pa) and burst pressure.
    pub pressure: f64,
    pub burst: f64,
    /// Share of the volume left as ullage when loaded full.
    pub ullage: f64,
}

/// What a port carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortKind {
    /// Liquid out of a tank.
    Outlet,
    /// Liquid into an engine or a thruster block.
    Inlet,
    /// Pressurant gas into a tank's ullage.
    Pressurant,
    /// Gas out of a bottle or a regulator, into a thruster block.
    Gas,
    Vent,
    /// Through a valve or regulator (either way).
    Through,
}

/// A plumbing port, in the part's (or component's) frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Port {
    pub name: String,
    pub kind: PortKind,
    /// The fluid it is for (`None`: any).
    pub fluid: Option<String>,
    pub pos: DVec3,
}

/// How an engine is fed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cycle {
    GasGenerator,
    StagedCombustion,
    Expander,
    PressureFed,
}

/// How an engine lights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Igniter {
    /// Pyrophoric TEA-TEB slugs: one per start.
    TeaTeb,
    Spark,
    Hypergolic,
    Pyrotechnic,
}

/// An engine's data (published first, D104), in its part's frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineData {
    pub cycle: Cycle,
    pub oxidizer: String,
    pub fuel: String,
    /// Oxidizer to fuel mass ratio.
    pub mixture_ratio: f64,
    /// N, s.
    pub thrust_vac: f64,
    pub isp_vac: f64,
    pub isp_sl: f64,
    /// Throttle range as fractions of full thrust.
    pub throttle_min: f64,
    pub throttle_max: f64,
    pub gimbal_deg: f64,
    pub gimbal_rate_dps: f64,
    /// Starts it can make (igniter charges; hypergolic: unlimited as u32::MAX).
    pub ignitions: u32,
    pub igniter: Igniter,
    /// Start transient to full thrust (s) and tail-off impulse at shutdown (N·s).
    pub start_time: f64,
    pub shutdown_impulse: f64,
    /// Net positive suction pressure each pump inlet needs (Pa).
    pub npsp_ox: f64,
    pub npsp_fuel: f64,
    /// Share of the jet power the nozzle's radiatively cooled wall takes (D077).
    pub nozzle_heat_fraction: f64,
    /// Nozzle exit area (m², from the geometry).
    pub exit_area: f64,
    /// Gimbal point and thrust direction (the force on the craft), part frame.
    pub pos: DVec3,
    pub dir: DVec3,
}

/// What a set of reaction-control thrusters burns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Propellant {
    /// Cold gas from a bottle (`gas`).
    ColdGas(String),
    /// Bipropellant from the plumbing.
    Biprop { oxidizer: String, fuel: String, mixture_ratio: f64 },
}

/// One thruster nozzle: where it is and which way it pushes the craft.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Nozzle {
    pub pos: DVec3,
    pub dir: DVec3,
}

/// A block of reaction-control thrusters (D104: torques from geometry).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Thrusters {
    /// N per nozzle, vacuum.
    pub thrust: f64,
    pub isp: f64,
    pub propellant: Propellant,
    pub nozzles: Vec<Nozzle>,
    /// Enabled when the craft is loaded (a later stage's thrusters wait
    /// for a command).
    pub enabled: bool,
}

/// An aerodynamic control surface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AeroSurface {
    /// A lattice (grid) fin: its cells are not a Newtonian panel; a
    /// lattice-fin model replaces them (`sim::aero`, later).
    pub lattice: bool,
    /// Planform area (m²), span and chord (m).
    pub area: f64,
    pub span: f64,
    pub chord: f64,
    /// Deflection range (deg) and actuator rate (deg/s).
    pub deflection_deg: f64,
    pub rate_dps: f64,
    /// Hinge (the deflection's axis through `pos`), part frame.
    pub pos: DVec3,
    pub axis: DVec3,
}

/// What drives a mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Actuator {
    Pneumatic,
    Hydraulic,
    Electric,
    Spring,
}

/// A mechanism (D103): moves its part kinematically about a hinge.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mechanism {
    /// Hinge point and axis, part frame.
    pub pos: DVec3,
    pub axis: DVec3,
    /// Angles (deg) about the axis: as built (stowed) and deployed.
    pub stowed_deg: f64,
    pub deployed_deg: f64,
    pub rate_dps: f64,
    pub actuator: Actuator,
    /// Retractable after deploying (Falcon 9's legs are not).
    pub retractable: bool,
    /// A foot's contact point as built (part frame) and its crush stroke (m).
    pub foot: Option<DVec3>,
    pub stroke: f64,
}

/// A sensor: what it measures, how well and how often.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sensor {
    /// `attitude`, `rates`, `acceleration`, `position`, `velocity`,
    /// `altitude`, `tank-level`, `pressure`, `temperature`, `strain`, …
    pub quantity: String,
    /// One-sigma accuracy in the quantity's SI unit.
    pub accuracy: f64,
    pub rate_hz: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Battery {
    /// J.
    pub capacity: f64,
    pub voltage: f64,
    pub internal_resistance: f64,
    /// W.
    pub max_power: f64,
}

/// A pressurant bottle (COPV).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bottle {
    pub gas: String,
    /// m³.
    pub volume: f64,
    /// Maximum expected operating pressure and burst (Pa).
    pub pressure: f64,
    pub burst: f64,
    /// Loaded at (K).
    pub temperature_k: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ValveKind {
    Isolation,
    Check,
    /// Opens above this pressure (Pa).
    Relief(f64),
    /// Holds its outlet at this pressure (Pa).
    Regulator(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Valve {
    pub kind: ValveKind,
    /// m.
    pub diameter: f64,
    /// Open at load.
    pub open: bool,
    /// Rated cycles.
    pub cycles: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Computer {
    /// Redundant channels (Falcon 9's flight computers: three).
    pub channels: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Solar {
    /// m² of cells and their efficiency at 1 AU, normal incidence.
    pub area: f64,
    pub efficiency: f64,
}

/// A cabin's life support (placeholder rates per day).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifeSupport {
    /// kg/day.
    pub co2_removal: f64,
    pub o2_supply: f64,
    /// Pressurised volume it serves (m³).
    pub cabin_volume: f64,
}

/// What a part or component does, beyond being mass and shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Function {
    Engine(EngineData),
    Thrusters(Thrusters),
    AeroSurface(AeroSurface),
    Mechanism(Mechanism),
    Chute(crate::chute::ChuteSpec),
    Sensor(Sensor),
    Battery(Battery),
    Bottle(Bottle),
    Valve(Valve),
    Computer(Computer),
    /// Seats for this many crew.
    Seat(u32),
    Solar(Solar),
    LifeSupport(LifeSupport),
    /// A docking port of this standard and diameter (m).
    Docking(String, f64),
    /// Makes the craft commandable (a crew cabin or a flight computer, D083).
    Control,
}

/// A generated part or component, in its own frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Generated {
    pub kind: Kind,
    /// A part's solids (empty for a component).
    pub primitives: Vec<Primitive>,
    /// A component's envelope (its volume and the solid its mass fills);
    /// a part's is unused.
    pub envelope: Option<Shape>,
    pub mounts: Vec<MountPoint>,
    /// Dry mass (kg): the published value if the entry has one, else the
    /// derived.
    pub mass: f64,
    /// What the generator derives, item by item (kg).
    pub derived: Vec<(String, f64)>,
    /// Derived values of the outputs a published number may override
    /// (thrust, Isp, capacity …), before any is applied.
    pub outputs: Vec<(String, f64)>,
    pub tanks: Vec<TankSpec>,
    pub ports: Vec<Port>,
    pub functions: Vec<Function>,
    /// Electrical draw when on (W).
    pub power: f64,
    /// Specific heat of its interior (J/(kg·K)) and the limit of its
    /// interior nodes (K).
    pub specific_heat: f64,
    pub max_k: f64,
}

impl Generated {
    /// The derived dry mass (kg).
    pub fn derived_mass(&self) -> f64 {
        self.derived.iter().map(|d| d.1).sum()
    }

    /// The solid volume of a part's primitives (overlaps counted twice) or
    /// of a component's envelope (m³).
    pub fn volume(&self) -> f64 {
        match &self.envelope {
            Some(e) => crate::craft::mesh::analytic_volume(e),
            None => self.primitives.iter().map(|p| crate::craft::mesh::analytic_volume(&p.shape)).sum(),
        }
    }
}
