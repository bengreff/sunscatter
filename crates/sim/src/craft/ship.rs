//! What a craft of catalog parts is made of beyond its shape (D100, D104;
//! `docs/design/ship-model.md` §2, §4): the static definition the build
//! gives ([`super::build`]) and the systems read — every part's and
//! component's identity and pose, tanks, bottles, engines, thruster blocks,
//! valves, batteries, mechanisms, aero surfaces, sensors, computers, seats,
//! joints, plumbing lines, buses, data links, engine groups, the stage
//! sequence and saved functions. Positions and directions are in the
//! craft's axes.
//!
//! Each item names the part it belongs to by its part id (`Part::id`, its
//! index in the craft built from the file), so a piece of the craft keeps
//! exactly the items of its parts ([`ShipDef::subset`]) and a ship state
//! partitions the same way (`super::state`).

use super::assembly::{JointKind, Loading};
use super::command::Stage;
use super::file::{Shape, Tank};
use super::place::Pose;
use crate::catalog::{
    AeroSurface, Battery, Computer, EngineData, FailureMode, Mechanism, Nozzle, Propellant, Ratings, Sensor, Valve,
};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An item owned by a part: the craft file's id of what provides it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Owned<T> {
    pub id: String,
    pub part: u32,
    pub item: T,
}

/// A part's identity: its craft-file id, catalog entry, pose, mass and
/// ratings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartInfo {
    pub id: String,
    pub entry: String,
    pub part: u32,
    pub pose: Pose,
    /// The part's own dry mass (kg; its components are listed apart) and
    /// what its generator derived.
    pub mass: f64,
    pub derived: f64,
    /// Published minus derived dry mass (kg) and the reason, if published.
    pub unmodelled: Option<(f64, String)>,
    /// Its outer surface's material and finish (`data/materials.ron` ids).
    pub material: String,
    pub finish: Option<String>,
    /// Ground, not vehicle: the pad's mount (`Function::Pad`).
    pub ground: bool,
    pub ratings: Ratings,
    pub failures: Vec<FailureMode>,
}

/// A component: identity, host part, pose, mass and the solid it fills.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentDef {
    pub id: String,
    pub entry: String,
    pub part: u32,
    pub pose: Pose,
    pub mass: f64,
    /// Its envelope in its own frame (centred).
    pub envelope: Shape,
    /// Electrical draw when on (W).
    pub power: f64,
    pub ratings: Ratings,
    pub failures: Vec<FailureMode>,
}

/// A tank compartment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TankDef {
    /// `owner:compartment`.
    pub id: String,
    pub part: u32,
    pub fluid: String,
    /// m³, less what components inside displace.
    pub volume: f64,
    /// Where the liquid sits: a solid cylinder from `base` along `axis`,
    /// whose length is the liquid's height at `capacity`.
    pub region: Tank,
    /// Liquid mass when loaded full (kg): the volume less the ullage at the
    /// loading temperature's density.
    pub capacity: f64,
    /// Maximum expected operating pressure and burst (Pa).
    pub pressure: f64,
    pub burst: f64,
    pub load: Loading,
}

/// A pressurant bottle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BottleDef {
    pub id: String,
    pub part: u32,
    pub gas: String,
    /// m³, Pa, K at loading, position.
    pub volume: f64,
    pub pressure: f64,
    pub burst: f64,
    pub temperature_k: f64,
    pub pos: DVec3,
}

/// An engine: on its own part (a main engine, the craft's propulsion) or
/// one of a component's (a pod's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineDef {
    /// The engine part's id, or `<pod>/<n>`.
    pub id: String,
    /// The part or component it belongs to (what selectors name).
    pub owner: String,
    pub part: u32,
    /// Its gimbal point and thrust direction in the craft's axes.
    pub data: EngineData,
    /// The tanks feeding its oxidizer and fuel inlets (indices into `tanks`).
    pub feeds: [Vec<u32>; 2],
    pub main: bool,
}

/// What a thruster block draws on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Feed {
    /// Bottles (indices into `bottles`).
    Bottles(Vec<u32>),
    /// Oxidizer and fuel tanks (indices into `tanks`).
    Tanks([Vec<u32>; 2]),
}

/// A block of reaction-control thrusters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThrusterDef {
    pub id: String,
    pub part: u32,
    pub thrust: f64,
    pub isp: f64,
    pub propellant: Propellant,
    /// Craft axes.
    pub nozzles: Vec<Nozzle>,
    pub enabled: bool,
    pub feed: Feed,
}

/// A joint between a part and the one it hangs from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JointDef {
    pub name: String,
    pub parent: u32,
    pub child: u32,
    pub kind: JointKind,
    /// Radius of the joint's ring (m; zero for a point attachment).
    pub radius: f64,
    /// Line load it carries at room temperature (N/m): the weaker wall's,
    /// a separation system's share of it.
    pub allowable: f64,
    /// From the parent towards the child (craft axes, unit): the way a
    /// separation system pushes the child.
    pub normal: DVec3,
}

/// A plumbing line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineDef {
    pub from: String,
    pub to: String,
    pub diameter: f64,
    pub length: f64,
    pub parts: (u32, u32),
}

/// An electrical bus and its members (id, part).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BusDef {
    pub name: String,
    pub voltage: f64,
    pub members: Vec<(String, u32)>,
}

/// A data link from a computer to sensors and actuators (id, part).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinkDef {
    pub from: (String, u32),
    pub to: Vec<(String, u32)>,
}

/// The static definition of a craft of parts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShipDef {
    pub name: String,
    pub parts: Vec<PartInfo>,
    pub components: Vec<ComponentDef>,
    pub tanks: Vec<TankDef>,
    pub bottles: Vec<BottleDef>,
    pub engines: Vec<EngineDef>,
    pub thrusters: Vec<ThrusterDef>,
    pub valves: Vec<Owned<Valve>>,
    pub batteries: Vec<Owned<Battery>>,
    /// Craft axes (as built: stowed).
    pub mechanisms: Vec<Owned<Mechanism>>,
    pub surfaces: Vec<Owned<AeroSurface>>,
    pub sensors: Vec<Owned<Sensor>>,
    pub computers: Vec<Owned<Computer>>,
    pub seats: Vec<Owned<u32>>,
    pub joints: Vec<JointDef>,
    pub lines: Vec<LineDef>,
    pub buses: Vec<BusDef>,
    pub links: Vec<LinkDef>,
    /// Engine groups: name → selectors of engine owners.
    pub groups: BTreeMap<String, Vec<String>>,
    pub sequence: Vec<Stage>,
    pub functions: Vec<Stage>,
}

fn keep<T: Clone>(items: &[T], part: impl Fn(&T) -> u32, kept: &[u32]) -> (Vec<T>, Vec<u32>) {
    let mut map = vec![u32::MAX; items.len()];
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if kept.contains(&part(item)) {
            map[i] = out.len() as u32;
            out.push(item.clone());
        }
    }
    (out, map)
}

fn remap(list: &[u32], map: &[u32]) -> Vec<u32> {
    list.iter().map(|&i| map[i as usize]).filter(|&i| i != u32::MAX).collect()
}

impl ShipDef {
    /// The definition of the piece holding the parts with ids `kept`: the
    /// items of those parts (in order), feeds renumbered, joints, lines and
    /// links within the piece; groups, sequence and functions as they were
    /// (a command naming what the piece lacks does nothing).
    pub fn subset(&self, kept: &[u32]) -> ShipDef {
        let (tanks, tank_map) = keep(&self.tanks, |t| t.part, kept);
        let (bottles, bottle_map) = keep(&self.bottles, |b| b.part, kept);
        let (mut engines, _) = keep(&self.engines, |e| e.part, kept);
        for e in &mut engines {
            e.feeds = [remap(&e.feeds[0], &tank_map), remap(&e.feeds[1], &tank_map)];
        }
        let (mut thrusters, _) = keep(&self.thrusters, |t| t.part, kept);
        for t in &mut thrusters {
            t.feed = match &t.feed {
                Feed::Bottles(b) => Feed::Bottles(remap(b, &bottle_map)),
                Feed::Tanks([o, f]) => Feed::Tanks([remap(o, &tank_map), remap(f, &tank_map)]),
            };
        }
        let within = |p: u32| kept.contains(&p);
        ShipDef {
            name: self.name.clone(),
            parts: keep(&self.parts, |p| p.part, kept).0,
            components: keep(&self.components, |c| c.part, kept).0,
            tanks,
            bottles,
            engines,
            thrusters,
            valves: keep(&self.valves, |v| v.part, kept).0,
            batteries: keep(&self.batteries, |v| v.part, kept).0,
            mechanisms: keep(&self.mechanisms, |v| v.part, kept).0,
            surfaces: keep(&self.surfaces, |v| v.part, kept).0,
            sensors: keep(&self.sensors, |v| v.part, kept).0,
            computers: keep(&self.computers, |v| v.part, kept).0,
            seats: keep(&self.seats, |v| v.part, kept).0,
            joints: self.joints.iter().filter(|j| within(j.parent) && within(j.child)).cloned().collect(),
            lines: self.lines.iter().filter(|l| within(l.parts.0) && within(l.parts.1)).cloned().collect(),
            buses: (self.buses.iter())
                .map(|b| BusDef { members: b.members.iter().filter(|m| within(m.1)).cloned().collect(), ..b.clone() })
                .filter(|b| !b.members.is_empty())
                .collect(),
            links: (self.links.iter())
                .filter(|l| within(l.from.1))
                .map(|l| LinkDef { to: l.to.iter().filter(|m| within(m.1)).cloned().collect(), ..l.clone() })
                .collect(),
            groups: self.groups.clone(),
            sequence: self.sequence.clone(),
            functions: self.functions.clone(),
        }
    }

    /// Gas mass in a bottle at its loading (ideal gas, kg).
    pub fn bottle_gas(&self, b: &BottleDef) -> f64 {
        let gas = crate::fluids::fluids().gas(&b.gas).expect("a bottle's gas was checked by the build");
        gas.density(b.pressure, b.temperature_k) * b.volume
    }

    /// Every tank's region and capacity, then every bottle as a small
    /// cylinder holding its loaded gas: the regions the craft-level mass
    /// model fills together (`MassModel::of_parts`, the approximation of
    /// predictions; a vessel uses each tank's own fill).
    pub fn regions(&self) -> Vec<(Tank, f64)> {
        let tanks = self.tanks.iter().map(|t| (t.region, t.capacity));
        let bottles = self.bottles.iter().map(|b| {
            let r = crate::math::cbrt(b.volume / crate::math::PI) / 2.0;
            let h = b.volume / (crate::math::PI * r * r);
            (Tank { base: b.pos - DVec3::Z * (h / 2.0), axis: DVec3::Z * h, radius: r }, self.bottle_gas(b))
        });
        tanks.chain(bottles).collect()
    }

    /// Propellant and gas loaded at the start (kg).
    pub fn loaded(&self) -> f64 {
        let liquid: f64 = self.tanks.iter().map(|t| t.capacity * t.load.fill).sum();
        liquid + self.bottles.iter().map(|b| self.bottle_gas(b)).sum::<f64>()
    }

    /// The engines a selector names: a group's members, or the engines
    /// whose owner it matches (indices into `engines`).
    pub fn engines_of(&self, selector: &str) -> Vec<usize> {
        let selectors: Vec<&str> = match self.groups.get(selector) {
            Some(members) => members.iter().map(String::as_str).collect(),
            None => vec![selector],
        };
        (0..self.engines.len())
            .filter(|&i| selectors.iter().any(|s| super::command::matches(s, &self.engines[i].owner)))
            .collect()
    }

    /// The attitude authority of the `enabled` thruster blocks about a
    /// centre of mass `com` (D104: from the geometry): per body axis the
    /// smaller of the torques the nozzles give either way (N·m), and the
    /// flow (kg/s) of the nozzles turning the craft one way at full command.
    pub fn rcs_authority(&self, com: DVec3, enabled: &[bool]) -> (DVec3, DVec3) {
        let (mut plus, mut minus, mut flow) = (DVec3::ZERO, DVec3::ZERO, DVec3::ZERO);
        for (t, _) in self.thrusters.iter().zip(enabled).filter(|(_, &on)| on) {
            let one = t.thrust / (t.isp * crate::vessel::G0);
            for n in &t.nozzles {
                let tau = (n.pos - com).cross(n.dir * t.thrust);
                plus += tau.max(DVec3::ZERO);
                minus += (-tau).max(DVec3::ZERO);
                flow += DVec3::select(tau.cmpgt(DVec3::splat(1e-9)), DVec3::splat(one), DVec3::ZERO);
            }
        }
        (plus.min(minus), flow)
    }

    /// Whether a command names something this craft has (each engine
    /// selector, valve, mechanism, joint, thruster block or component).
    pub fn check(&self, command: &super::command::ShipCommand) -> Result<(), String> {
        super::layers::check(self, command)
    }

    /// The index of the part with craft-file id `id`.
    pub fn part_named(&self, id: &str) -> Option<u32> {
        self.parts.iter().find(|p| p.id == id).map(|p| p.part)
    }
}
