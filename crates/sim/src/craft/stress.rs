//! Stress craft (D081, D121): **for benchmarks and tests only.** Not a part
//! catalog, a craft builder or a design the game offers: craft creation is
//! the owner's to guide (interview 2026-10-26). It measures the craft
//! pipeline at scale (`examples/bench_scale`) and gives tests many-part craft.
//!
//! **`stress-<n>`** is a craft of catalog parts on the real path ([`file`],
//! [`parts_plan`]): [`super::small`]'s test parts of the real generators
//! (capsule, tanks, thrust structures, engines, legs, nose cones, the
//! capsule's avionics, cells and thrusters) assembled by rule into a core
//! stack and rings of radial boosters of exactly n parts, built like any
//! assembly ([`super::build`]). Not a count of arbitrary cubes: a plausible
//! many-part craft (the owner, 2026-10-30). 300 parts: a core of 2 tanks
//! and 4 rings (40 boosters of 1–2 tanks, 164 engines); 1,000: 3 tanks, 7
//! rings (112 boosters, 452 engines); 5,000: 10 tanks, 12 rings (312
//! boosters of 9–10 tanks, 1,252 engines).
//!
//! The **placeholder layout** below ([`layout`], [`stress_craft`]) is the
//! made-up path's (placeholder parts, `ship: None`): its last direct users
//! are tests of it, and it goes with that path (D121).
//!
//! [`layout`] lays out exactly `n` parts (n ≥ 1; tested 1 … 1000) from the
//! existing primitives, deterministically (arithmetic, square roots and
//! `sim::math` only: the same bits on every platform):
//! * the **core stack** along +Z: an engine (its bell is the geometry's
//!   nozzle), tanks with a decoupler after every fourth, avionics rings that
//!   make up the count, a pod and its nose cap;
//! * four **fins** at the base (0°, 90°, 180°, 270°) and four **legs** from
//!   their tips;
//! * **rings of radial boosters** (ring k: 4k, off the fins' lanes), each an
//!   engine, tanks, a nose cone and two struts to the core or the ring inside;
//! * **small parts** in sets of four, 90° apart (solar panels, RCS blocks,
//!   batteries, antennas), in rows on the core above the boosters and on the
//!   outer ring's outward faces.
//!
//! Every layout is symmetric under quarter turns about Z. 1 + n/60 core tanks
//! or more, booster rings while their parts stay within [`BOOSTER_SHARE`] of
//! n, the rest small parts (core tanks are added until they fit). 300 parts:
//! 8 core tanks, 12 boosters of 6 tanks, 160 small parts, ≈ 30 m tall; 1000:
//! 18 core tanks, 24 boosters of 14 tanks, 532 small parts, ≈ 62 m.
//!
//! Each part is a [`Part`] of the part layer ([`super::parts`]): its
//! primitives, the part it hangs from (a tree rooted at part 0) and
//! **placeholder properties** (allowed by the owner "to confirm high
//! timewarp behavior equivalence"): dry mass, a tank (its cylinder, capacity
//! from its volume), an engine (thrust sized for the lift-off ratio
//! [`LIFTOFF_TWR`], every bell its engine's nozzle), the interior's specific
//! heat and temperature limit, power produced, used and stored, control
//! (the pod and avionics rings), crew (the pod), and a load limit
//! ([`LOAD_FACTOR`] times its weight; a leg [`LEG_LOAD`] times the craft's).
//! The pod (else the root) carries the thrusters and the parachutes. Each
//! primitive's skin carries [`SKIN_SHARE`] of its part's dry mass over the
//! part's surface and the part's limit. The craft's aggregates (dry mass,
//! propellant, the combined engine, crew) come from the parts. A stress
//! vessel reloads from a save: its craft id `stress-<n>` names the layout
//! ([`super::registry`]).

use super::assembly::AssemblyFile;
use super::file::{CraftFile, EngineFile, GeometryFile, Mount, Primitive, RcsFile, Shape, Skin, Tank};
use super::mesh::{analytic_area, analytic_volume};
use super::parts::{aggregate, CraftDef, Part, PartPower, PartTank};
use super::{CellOptions, Craft, CraftId, Resolution};
use crate::math;
use crate::vessel::G0;
use glam::DVec3;

/// Share of a part's dry mass carried by its skin (the test craft's ≈ 30 %).
pub const SKIN_SHARE: f64 = 0.3;
/// Lift-off thrust-to-weight ratio the engines are sized for.
pub const LIFTOFF_TWR: f64 = 1.5;
/// Booster rings are added while their parts stay within this share of n.
pub const BOOSTER_SHARE: f64 = 0.55;

const R_CORE: f64 = 1.5;
const H_CORE_TANK: f64 = 3.0;
const H_DECOUPLER: f64 = 0.3;
const H_AVIONICS: f64 = 0.4;
const H_POD: f64 = 2.2;
const R_POD_TOP: f64 = 0.7;
const R_BOOSTER: f64 = 0.75;
const H_BOOSTER_TANK: f64 = 2.5;
const H_BOOSTER_NOSE: f64 = 1.6;
/// Clearance between neighbouring rings (m).
const GAP: f64 = 0.25;
/// Spacing of the small parts' rows (m).
const ROW: f64 = 1.0;
const FIN_TOP: f64 = 2.0;
/// Height of the feet's soles (below every bell).
const FOOT_Z: f64 = -2.3;
/// Propellant (NTO/MMH-like, kg/m³), the usable share of a tank's volume and
/// a tank's dry mass per kg of capacity.
const PROPELLANT_DENSITY: f64 = 1200.0;
const TANK_FILL: f64 = 0.9;
const TANK_DRY_FRACTION: f64 = 0.08;
/// The test craft's wet mass (kg): springs and RCS scale from it.
const REFERENCE_MASS: f64 = 20_000.0;
/// A part's load limit in its own weights (a structural load factor).
pub const LOAD_FACTOR: f64 = 30.0;
/// A leg's load limit in the craft's wet weights.
pub const LEG_LOAD: f64 = 2.0;
/// Every engine's share of its jet power heating its bell (the test
/// craft's, D077).
const NOZZLE_HEAT: f64 = 2.2e-3;

/// What a stress part is. Shapes: tanks, decouplers and avionics rings are
/// cylinders; an engine a chamber (cylinder) and a bell (frustum); the pod a
/// frustum; a nose a sphere cap (core) or a cone (boosters); a fin a thin
/// box; a leg a strut and a foot; a panel a thin disc; RCS and batteries
/// boxes; antennas and booster struts struts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StressKind {
    Tank,
    Engine,
    Pod,
    Nose,
    Decoupler,
    /// Avionics ring in the core stack (makes up the count on the axis).
    Avionics,
    Fin,
    Leg,
    Strut,
    Panel,
    Rcs,
    Battery,
    Antenna,
}

/// Placeholder per kind: specific heat of the dry mass (J/(kg·K)),
/// temperature limit (K), power produced and used (W), battery (J).
fn kind_data(kind: StressKind) -> (f64, f64, f64, f64, f64) {
    match kind {
        StressKind::Tank | StressKind::Decoupler | StressKind::Leg | StressKind::Strut => (900., 1100., 0., 0., 0.),
        StressKind::Engine => (500.0, 1640.0, 0.0, 50.0, 0.0),
        StressKind::Pod => (900.0, 1100.0, 0.0, 300.0, 1.0e7),
        StressKind::Nose => (900.0, 1400.0, 0.0, 0.0, 0.0),
        StressKind::Avionics => (900.0, 1100.0, 0.0, 150.0, 3.6e6),
        StressKind::Fin => (900.0, 1300.0, 0.0, 0.0, 0.0),
        StressKind::Panel => (700.0, 900.0, 150.0, 0.0, 0.0),
        StressKind::Rcs => (900.0, 1100.0, 0.0, 20.0, 0.0),
        StressKind::Battery => (1000.0, 600.0, 0.0, 5.0, 3.6e6),
        StressKind::Antenna => (900.0, 1100.0, 0.0, 20.0, 0.0),
    }
}

/// A laid-out stress craft: the two craft files, the parts and each
/// part's kind.
#[derive(Clone, Debug, PartialEq)]
pub struct StressLayout {
    pub spec: CraftFile,
    pub geometry: GeometryFile,
    pub parts: Vec<Part>,
    pub kinds: Vec<StressKind>,
}

/// A built stress craft: the craft (surface, cells, mass model, parts; its
/// design is baked on first use) and each part's kind.
#[derive(Clone, Debug)]
pub struct StressCraft {
    pub craft: Craft,
    pub kinds: Vec<StressKind>,
}

/// The cell options of stress craft: every craft of parts' (so a design
/// rebuilt from its id after a load is the same bits).
pub fn cell_options(parts: usize) -> CellOptions {
    CellOptions::for_parts(parts)
}

/// A stress craft of `parts` parts built with the default resolution and
/// [`cell_options`].
pub fn stress_craft(parts: usize) -> StressCraft {
    layout(parts).build(&Resolution::default(), &cell_options(parts))
}

/// The craft id of the stress craft of `parts` parts.
pub fn id(parts: usize) -> CraftId {
    CraftId(format!("stress-{parts}"))
}

impl StressLayout {
    /// The craft's definition (files and parts).
    pub fn def(&self) -> CraftDef {
        CraftDef { spec: self.spec.clone(), geometry: self.geometry.clone(), parts: self.parts.clone(), ship: None }
    }

    /// Builds the craft (surface, cells, mass model) through the craft
    /// pipeline ([`Craft::from_def`]). Panics if the definition is invalid
    /// (a bug here).
    pub fn build(self, res: &Resolution, opts: &CellOptions) -> StressCraft {
        let def = self.def();
        if let Err(e) = super::file::validate_shapes(&def.geometry).and(def.validate()) {
            panic!("stress craft of {} parts: {e}", self.parts.len());
        }
        StressCraft { craft: Craft::from_def(id(self.parts.len()), def, res, opts), kinds: self.kinds }
    }
}

/// The parts' dry centre of mass with each part's mass at its primitives'
/// volume centroid (body axes): against the shells' (`Craft::mass`).
pub fn volume_com(def: &CraftDef) -> DVec3 {
    let (m, moment) = def.parts.iter().fold((0.0, DVec3::ZERO), |(m, s), p| {
        let shapes = p.primitives.iter().map(|&q| &def.geometry.primitives[q as usize].shape);
        let (v, c) =
            shapes.fold((0.0, DVec3::ZERO), |(v, c), x| (v + analytic_volume(x), c + centroid(x) * analytic_volume(x)));
        (m + p.dry_mass, s + c / v * p.dry_mass)
    });
    moment / m
}

/// How many of each element a craft of n parts has.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Plan {
    core_tanks: usize,
    /// Tank 0, engine, pod, nose: how many of them (1 … 4).
    head: usize,
    fins: bool,
    legs: bool,
    rings: usize,
    booster_tanks: usize,
    smalls: usize,
    avionics: usize,
}

fn decouplers(core_tanks: usize) -> usize {
    (core_tanks - 1) / 4
}

/// Height of the core's tanks and decouplers (m).
fn core_height(core_tanks: usize) -> f64 {
    core_tanks as f64 * H_CORE_TANK + decouplers(core_tanks) as f64 * H_DECOUPLER
}

/// Tanks per booster: about 65 % of the core's height.
fn booster_tanks(core_tanks: usize) -> usize {
    (libm::floor(0.65 * core_height(core_tanks) / H_BOOSTER_TANK) as usize).max(1)
}

/// Parts in `rings` rings of boosters of `tanks` tanks (ring k: 4k
/// boosters of tanks + engine, nose and two struts).
fn booster_parts(rings: usize, tanks: usize) -> usize {
    2 * rings * (rings + 1) * (tanks + 4)
}

/// Distance of ring k's boosters from the axis (m).
fn ring_radius(k: usize) -> f64 {
    R_CORE + GAP + R_BOOSTER + (k - 1) as f64 * (2.0 * R_BOOSTER + GAP)
}

/// Rows at lo, lo + ROW, … up to hi.
fn rows(lo: f64, hi: f64) -> usize {
    if hi >= lo {
        libm::floor((hi - lo) / ROW) as usize + 1
    } else {
        0
    }
}

impl Plan {
    /// Height and count of the small parts' rows on the core.
    fn core_rows(&self) -> (f64, usize) {
        let lo = if self.rings > 0 {
            self.booster_tanks as f64 * H_BOOSTER_TANK + H_BOOSTER_NOSE + 0.4
        } else if self.fins {
            FIN_TOP + 0.6
        } else {
            0.6
        };
        (lo, rows(lo, core_height(self.core_tanks) - 0.5))
    }

    fn booster_rows(&self) -> usize {
        rows(0.6, self.booster_tanks as f64 * H_BOOSTER_TANK - 0.6)
    }

    /// Room for small parts: eight per core row, one per booster of the
    /// outer ring per booster row.
    fn slots(&self) -> usize {
        let outer = if self.rings > 0 { 4 * self.rings * self.booster_rows() } else { 0 };
        8 * self.core_rows().1 + outer
    }
}

fn plan(n: usize) -> Plan {
    assert!(n >= 1, "a stress craft has at least one part");
    let mut p = Plan { core_tanks: 1, head: n.min(4), ..Plan::default() };
    if n <= 4 {
        return p;
    }
    let mut rest = n - 4;
    for flag in [&mut p.fins, &mut p.legs] {
        if rest >= 4 {
            *flag = true;
            rest -= 4;
        }
    }
    p.avionics = rest;
    let share = libm::floor(BOOSTER_SHARE * n as f64) as usize;
    let mut tanks = 1 + n / 60;
    loop {
        let core = tanks - 1 + decouplers(tanks);
        if core > rest {
            // No room for another tank: the rest go on the axis.
            break;
        }
        let tb = booster_tanks(tanks);
        let mut rings = 0;
        while core + booster_parts(rings + 1, tb) <= rest.min(share) {
            rings += 1;
        }
        let left = rest - core - booster_parts(rings, tb);
        let q = Plan { core_tanks: tanks, rings, booster_tanks: tb, ..p };
        let smalls = (4 * (left / 4)).min(4 * (q.slots() / 4));
        p = Plan { smalls, avionics: left - smalls, ..q };
        if smalls == 4 * (left / 4) {
            break;
        }
        tanks += 1;
    }
    p
}

/// Volume centroid of a primitive (body axes).
fn centroid(shape: &Shape) -> DVec3 {
    // Radii never both zero (validated).
    let frustum = |base: DVec3, axis: DVec3, a: f64, b: f64| {
        base + axis * ((a * a + 2.0 * a * b + 3.0 * b * b) / (4.0 * (a * a + a * b + b * b)))
    };
    match *shape {
        Shape::Frustum { base, axis, r_base, r_top } => frustum(base, axis, r_base, r_top),
        Shape::Cylinder { base, axis, .. } => base + axis * 0.5,
        Shape::Strut { from, to, .. } => (from + to) * 0.5,
        Shape::SphereCap { center, axis, radius, height } => {
            let d = 3.0 * (2.0 * radius - height) * (2.0 * radius - height) / (4.0 * (3.0 * radius - height));
            center + axis.normalize() * d
        }
        Shape::Box { center, .. } => center,
    }
}

/// One primitive of a part: name suffix, shape, foot, nozzle.
type Piece = (&'static str, Shape, bool, bool);

/// Collects primitives and parts.
#[derive(Default)]
struct Builder {
    prims: Vec<Primitive>,
    parts: Vec<Part>,
    kinds: Vec<StressKind>,
}

impl Builder {
    fn add(&mut self, kind: StressKind, name: String, parent: Option<u32>, mass: (f64, f64), pieces: &[Piece]) -> u32 {
        let (dry_mass, capacity) = mass;
        let (c, max_k, produced, consumed, battery) = kind_data(kind);
        let area: f64 = pieces.iter().map(|p| analytic_area(&p.1)).sum();
        let areal_mass = SKIN_SHARE * dry_mass / area;
        let first = self.prims.len() as u32;
        for &(suffix, shape, foot, nozzle) in pieces {
            // A bell is a thin niobium extension coupled only at its throat
            // (as the test craft's, D077); the rest aluminium.
            let skin = if nozzle {
                Skin { specific_heat: 270.0, conductivity: 50.0, thickness: 0.001, coupling: Some(0.2), ..aluminium() }
            } else {
                Skin { specific_heat: c, ..aluminium() }
            };
            let skin = Skin { areal_mass, max_k: Some(max_k), ..skin };
            let name = if suffix.is_empty() { name.clone() } else { format!("{name} {suffix}") };
            self.prims.push(Primitive { name, shape, foot, skin: Some(skin), nozzle });
        }
        // A tank is its one cylinder.
        let tank = match (capacity > 0.0, pieces) {
            (true, [(_, Shape::Cylinder { base, axis, radius }, ..)]) => {
                Some(PartTank { capacity, region: Tank { base: *base, axis: *axis, radius: *radius } })
            }
            _ => None,
        };
        let control = matches!(kind, StressKind::Pod | StressKind::Avionics);
        self.parts.push(Part {
            id: self.parts.len() as u32,
            name,
            primitives: (first..self.prims.len() as u32).collect(),
            parent,
            dry_mass,
            tank,
            engine: None,
            rcs: None,
            chute: false,
            interior_specific_heat: Some(c),
            // The pod's interior holds the crew: the craft's limit.
            node_max_k: (kind != StressKind::Pod).then_some(max_k),
            power: PartPower { produced, consumed, battery },
            control,
            crew: if kind == StressKind::Pod { 3 } else { 0 },
            max_load: LOAD_FACTOR * dry_mass * G0,
        });
        self.kinds.push(kind);
        (self.parts.len() - 1) as u32
    }

    /// A part of one primitive and no propellant.
    fn one(&mut self, kind: StressKind, name: String, parent: u32, dry_mass: f64, shape: Shape) -> u32 {
        self.add(kind, name, Some(parent), (dry_mass, 0.0), &[("", shape, false, false)])
    }

    /// A tank: a cylinder, its capacity from its volume.
    fn tank(&mut self, name: String, parent: Option<u32>, base: DVec3, h: f64, r: f64) -> u32 {
        let capacity = PROPELLANT_DENSITY * TANK_FILL * math::PI * r * r * h;
        let shape = Shape::Cylinder { base, axis: DVec3::Z * h, radius: r };
        self.add(StressKind::Tank, name, parent, (TANK_DRY_FRACTION * capacity, capacity), &[("", shape, false, false)])
    }

    /// An engine under `c`: a chamber and a bell of size `scale` (its
    /// nozzle), its dry mass, a thrust weight (scaled by [`finish`]) and
    /// Isp pair; it thrusts along +Z from the chamber's base.
    fn engine(&mut self, name: String, parent: u32, c: DVec3, (scale, dry): (f64, f64), w: f64, isp: (f64, f64)) {
        let chamber = Shape::Cylinder { base: c - DVec3::Z * 0.5, axis: DVec3::Z * 0.5, radius: 0.5 * scale };
        let (h, r0, r1) = (1.3 * scale, 1.1 * scale, 0.45 * scale);
        let bell = Shape::Frustum { base: c - DVec3::Z * (0.5 + h), axis: DVec3::Z * h, r_base: r0, r_top: r1 };
        let pieces = [("chamber", chamber, false, false), ("bell", bell, false, true)];
        let i = self.add(StressKind::Engine, name, Some(parent), (dry, 0.0), &pieces);
        self.parts[i as usize].engine = Some(EngineFile {
            thrust_vac: w,
            isp_vac: isp.0,
            isp_sl: isp.1,
            min_throttle: 0.1,
            gimbal_deg: 5.0,
            mount: Mount { pos: c - DVec3::Z * 0.5, dir: DVec3::Z },
            nozzle_heat_fraction: NOZZLE_HEAT,
        });
    }
}

fn aluminium() -> Skin {
    let (max_k, coupling) = (None, None);
    Skin {
        areal_mass: 8.1,
        specific_heat: 900.0,
        emissivity: 0.8,
        conductivity: 150.0,
        thickness: 0.003,
        max_k,
        coupling,
    }
}

/// Unit vector at angle `a` (rad) in the XY plane.
fn radial(a: f64) -> DVec3 {
    DVec3::new(math::cos(a), math::sin(a), 0.0)
}

/// A stack of parts: (bottom, top, part), bottom up.
type Stack = Vec<(f64, f64, u32)>;

/// The element of `stack` at height `z` (the nearest).
fn at_height(stack: &[(f64, f64, u32)], z: f64) -> u32 {
    stack.iter().find(|s| z < s.1).or(stack.last()).expect("a stack").2
}

/// A booster: its centre (at z = 0), angle about Z and tanks.
struct Booster {
    centre: DVec3,
    angle: f64,
    tanks: Stack,
}

/// Lays out a stress craft of exactly `n` parts (n ≥ 1). See the module.
pub fn layout(n: usize) -> StressLayout {
    let p = plan(n);
    let mut b = Builder::default();
    let (stack, tank_top) = core_stack(&mut b, &p);
    fins_and_legs(&mut b, &p);
    let outer = boosters(&mut b, &p, &stack);
    small_parts(&mut b, &p, &stack, &outer);
    finish(n, b, &p, tank_top)
}

/// The core stack, bottom up: tank 0 (the root), the engine, the other
/// tanks (a decoupler after every fourth), avionics, pod and nose. Returns
/// the stack and the top of its tanks.
fn core_stack(b: &mut Builder, p: &Plan) -> (Stack, f64) {
    let root = b.tank("core tank 0".into(), None, DVec3::ZERO, H_CORE_TANK, R_CORE);
    if p.head >= 2 {
        b.engine("core engine".into(), root, DVec3::ZERO, (1.0, 3000.0), 2.0, (330.0, 290.0));
    }
    let mut stack = vec![(0.0, H_CORE_TANK, root)];
    let disc = |z: f64, h: f64, radius: f64| Shape::Cylinder { base: DVec3::Z * z, axis: DVec3::Z * h, radius };
    for i in 1..p.core_tanks + p.avionics {
        let (_, z, below) = *stack.last().expect("a stack");
        if i % 4 == 0 && i < p.core_tanks {
            let d = b.one(
                StressKind::Decoupler,
                format!("decoupler {}", i / 4),
                below,
                300.0,
                disc(z, H_DECOUPLER, R_CORE + 0.1),
            );
            stack.push((z, z + H_DECOUPLER, d));
        }
        let (_, z, below) = *stack.last().expect("a stack");
        let (h, part) = if i < p.core_tanks {
            (H_CORE_TANK, b.tank(format!("core tank {i}"), Some(below), DVec3::Z * z, H_CORE_TANK, R_CORE))
        } else {
            let name = format!("avionics {}", i - p.core_tanks);
            (H_AVIONICS, b.one(StressKind::Avionics, name, below, 250.0, disc(z, H_AVIONICS, R_CORE)))
        };
        stack.push((z, z + h, part));
    }
    let (_, z, top) = *stack.last().expect("a stack");
    if p.head >= 3 {
        let shape = Shape::Frustum { base: DVec3::Z * z, axis: DVec3::Z * H_POD, r_base: R_CORE, r_top: R_POD_TOP };
        let pod = b.one(StressKind::Pod, "pod".into(), top, 3000.0, shape);
        if p.head >= 4 {
            // A cap of height 0.5 whose base is the pod's top.
            let (h, top) = (0.5, z + H_POD);
            let radius = (R_POD_TOP * R_POD_TOP + h * h) / (2.0 * h);
            let shape = Shape::SphereCap { center: DVec3::Z * (top - radius + h), axis: DVec3::Z, radius, height: h };
            b.one(StressKind::Nose, "nose".into(), pod, 200.0, shape);
        }
    }
    (stack, core_height(p.core_tanks))
}

/// Four fins at the base and four legs from their tips, in the lanes at 0°,
/// 90°, 180°, 270°; the feet outside the outer booster ring.
fn fins_and_legs(b: &mut Builder, p: &Plan) {
    let lanes = [DVec3::X, DVec3::Y, -DVec3::X, -DVec3::Y];
    let mut fins = Vec::new();
    if p.fins {
        for (k, u) in lanes.iter().enumerate() {
            let half = if u.x != 0.0 { DVec3::new(0.65, 0.04, 1.0) } else { DVec3::new(0.04, 0.65, 1.0) };
            let shape = Shape::Box { center: *u * (R_CORE + 0.6) + DVec3::Z * (FIN_TOP / 2.0), half };
            fins.push(b.one(StressKind::Fin, format!("fin {k}"), 0, 150.0, shape));
        }
    }
    if p.legs {
        let reach = if p.rings > 0 { ring_radius(p.rings) + R_BOOSTER + 1.0 } else { R_CORE + 1.9 };
        for (k, u) in lanes.iter().enumerate() {
            let foot = *u * reach + DVec3::Z * FOOT_Z;
            let strut =
                Shape::Strut { from: *u * (R_CORE + 1.2) + DVec3::Z * 0.2, to: foot + DVec3::Z * 0.12, radius: 0.08 };
            let pad = Shape::Cylinder { base: foot, axis: DVec3::Z * 0.12, radius: 0.3 };
            let pieces = [("strut", strut, false, false), ("foot", pad, true, false)];
            b.add(StressKind::Leg, format!("leg {k}"), Some(fins[k]), (250.0, 0.0), &pieces);
        }
    }
}

/// A strut at height `z` from a booster at `centre` (angle `angle`) to the
/// core, or to the nearest booster of the ring `inside`: (from, to, the
/// part it attaches to).
fn strut(
    centre: DVec3,
    angle: f64,
    z: f64,
    stack: &[(f64, f64, u32)],
    inside: Option<&[Booster]>,
) -> (DVec3, DVec3, u32) {
    let (inner, reach, part) = match inside {
        None => (DVec3::Z * z, R_CORE, at_height(stack, z)),
        Some(ring) => {
            let d = |x: &&Booster| (x.angle - angle).abs();
            let nearest = ring.iter().min_by(|a, b| d(a).total_cmp(&d(b))).expect("an inner ring");
            (nearest.centre + DVec3::Z * z, R_BOOSTER, at_height(&nearest.tanks, z))
        }
    };
    let from = centre + DVec3::Z * z;
    let u = (inner - from).normalize();
    (from + u * (R_BOOSTER - 0.05), inner - u * (reach - 0.05), part)
}

/// The booster rings (ring k: 4k boosters, half a step off the fins'
/// lanes): each a strut, tanks, a nose cone, an engine and a second strut.
/// Returns the outer ring.
fn boosters(b: &mut Builder, p: &Plan, stack: &[(f64, f64, u32)]) -> Vec<Booster> {
    let mut inside: Vec<Booster> = Vec::new();
    let top = p.booster_tanks as f64 * H_BOOSTER_TANK;
    for k in 1..=p.rings {
        let mut ring = Vec::new();
        for j in 0..4 * k {
            let angle = (j as f64 + 0.5) * math::TAU / (4 * k) as f64;
            let centre = radial(angle) * ring_radius(k);
            let name = format!("booster {k}.{j}");
            let inner = (k > 1).then_some(&inside[..]);
            let (from, to, parent) = strut(centre, angle, 1.0, stack, inner);
            let mut below = b.one(
                StressKind::Strut,
                format!("{name} strut 0"),
                parent,
                60.0,
                Shape::Strut { from, to, radius: 0.08 },
            );
            let mut tanks = Vec::new();
            for t in 0..p.booster_tanks {
                let z0 = t as f64 * H_BOOSTER_TANK;
                below =
                    b.tank(format!("{name} tank {t}"), Some(below), centre + DVec3::Z * z0, H_BOOSTER_TANK, R_BOOSTER);
                tanks.push((z0, z0 + H_BOOSTER_TANK, below));
            }
            let base = centre + DVec3::Z * top;
            let cone = Shape::Frustum { base, axis: DVec3::Z * H_BOOSTER_NOSE, r_base: R_BOOSTER, r_top: 0.0 };
            b.one(StressKind::Nose, format!("{name} nose"), below, 150.0, cone);
            b.engine(format!("{name} engine"), tanks[0].2, centre, (0.55, 1000.0), 1.0, (300.0, 265.0));
            let (from, to, _) = strut(centre, angle, top - 1.0, stack, inner);
            b.one(StressKind::Strut, format!("{name} strut 1"), below, 60.0, Shape::Strut { from, to, radius: 0.08 });
            ring.push(Booster { centre, angle, tanks });
        }
        inside = ring;
    }
    inside
}

/// The small parts: sets of four 90° apart, on the core's rows, then on the
/// outer ring's outward faces.
fn small_parts(b: &mut Builder, p: &Plan, stack: &[(f64, f64, u32)], outer: &[Booster]) {
    // (mount point, outward unit, parent)
    let mut slots: Vec<(DVec3, DVec3, u32)> = Vec::new();
    let (lo, count) = p.core_rows();
    for r in 0..count {
        let z = lo + r as f64 * ROW;
        for s in 0..2 {
            for m in 0..4 {
                let u = radial((r % 2) as f64 * math::PI / 8.0 + s as f64 * math::PI / 4.0 + m as f64 * math::PI / 2.0);
                slots.push((u * R_CORE + DVec3::Z * z, u, at_height(stack, z)));
            }
        }
    }
    let k = p.rings;
    for r in 0..if k > 0 { p.booster_rows() } else { 0 } {
        let z = 0.6 + r as f64 * ROW;
        for s in 0..k {
            for m in 0..4 {
                let x = &outer[s + m * k];
                let u = radial(x.angle);
                slots.push((x.centre + u * R_BOOSTER + DVec3::Z * z, u, at_height(&x.tanks, z)));
            }
        }
    }
    for (i, &(at, u, parent)) in slots.iter().take(p.smalls).enumerate() {
        let t = DVec3::Z.cross(u);
        let (kind, mass, shape) = match (i / 4) % 4 {
            0 => (
                StressKind::Panel,
                20.0,
                Shape::Cylinder { base: at + u * 0.35 - t * 0.015, axis: t * 0.03, radius: 0.4 },
            ),
            1 => (StressKind::Rcs, 30.0, Shape::Box { center: at + u * 0.1, half: DVec3::splat(0.15) }),
            2 => (StressKind::Battery, 60.0, Shape::Box { center: at + u * 0.08, half: DVec3::new(0.18, 0.18, 0.25) }),
            _ => (StressKind::Antenna, 5.0, Shape::Strut { from: at - u * 0.05, to: at + u * 0.9, radius: 0.025 }),
        };
        b.one(kind, format!("{} {i}", format!("{kind:?}").to_lowercase()), parent, mass, shape);
    }
}

/// Sizes the engines and writes the craft files. The craft file's fixed
/// numbers are the test craft's (chute, thermal grid, limits); springs and
/// RCS scale with the mass.
fn finish(n: usize, mut b: Builder, p: &Plan, tank_top: f64) -> StressLayout {
    let dry: f64 = b.parts.iter().map(|q| q.dry_mass).sum();
    let capacity: f64 = b.parts.iter().filter_map(|q| q.tank).map(|t| t.capacity).sum();
    let weights: f64 = b.parts.iter().filter_map(|q| q.engine).map(|e| e.thrust_vac).sum();
    for q in b.parts.iter_mut() {
        if let Some(e) = q.engine.as_mut() {
            e.thrust_vac *= LIFTOFF_TWR * (dry + capacity) * G0 / weights;
        }
    }
    let s = ((dry + capacity) / REFERENCE_MASS).max(1.0);
    let top = core_height(p.core_tanks) + p.avionics as f64 * H_AVIONICS + if p.head >= 3 { H_POD } else { 0.0 };
    // The pod (else the root) carries the thrusters and the parachutes; the
    // legs hold the craft.
    let command = if p.head >= 3 { b.kinds.iter().position(|&k| k == StressKind::Pod).unwrap_or(0) } else { 0 };
    let rcs = RcsFile { thrust: 445.0 * s, isp: 290.0, fore_z: top.max(1.5), aft_z: 0.5, radius: R_CORE };
    b.parts[command].rcs = Some(rcs);
    b.parts[command].chute = true;
    for (q, k) in b.parts.iter_mut().zip(&b.kinds) {
        if *k == StressKind::Leg {
            q.max_load = LEG_LOAD * (dry + capacity) * G0;
        }
    }
    let chute = top + 0.5;
    let (foot_k, foot_c, hull_k, hull_c) = (4.0e5 * s, 2.5e4 * s, 2.0e6 * s, 1.0e5 * s);
    // The aggregates (dry mass, propellant, engine, crew) come from the
    // parts ([`aggregate`]); the file's are stand-ins.
    let text = format!(
        "(name: \"Stress craft ({n} parts)\", crew: 0, dry_mass: 1.0,
        propellant: (mass: 1.0, capacity: 1.0),
        engine: (thrust_vac: 1000.0, isp_vac: 300.0, isp_sl: 265.0, min_throttle: 0.1, gimbal_deg: 5.0,
            mount: (pos: (0.0, 0.0, -0.5), dir: (0.0, 0.0, 1.0)), nozzle_heat_fraction: 0.0),
        attitude_control: (rcs: (thrust: {:?}, isp: 290.0, fore_z: {:?}, aft_z: 0.5, radius: {R_CORE:?})),
        chute: (mount: (0.0, 0.0, {chute:?}), main_height: 3350.0,
            drogue: Some((cd_area: 21.18, diameter: 5.03, fill_constant: 10.0,
                reefing: [(cd_fraction: 0.57, until: 10.0)], deploy_max_q: 5.5e3, max_load: 212.7e3)),
            main: (cd_area: 1560.0, diameter: 25.45, fill_constant: 7.0,
                reefing: [(cd_fraction: 0.0679, until: 6.0), (cd_fraction: 0.2571, until: 10.0)],
                deploy_max_q: 3.27e3, max_load: 588.6e3)),
        aero: (cd0: 0.45),
        thermal: (skin_max_k: 1100.0, internal_max_k: 400.0, node_size: 1.0, interior_specific_heat: 900.0,
            propellant_specific_heat: 2000.0, interior_conductivity: 5.0, internal_coupling: 2.0),
        impact: (max_speed: 8.0),
        contact: (foot: (stiffness: {foot_k:?}, damping: {foot_c:?}, stroke: 0.35, friction: 0.8),
            hull: (stiffness: {hull_k:?}, damping: {hull_c:?}, friction: 0.5), stick_speed: 0.05),
        antenna: (gain_dbi: 20.0, power_w: 20.0))",
        rcs.thrust, rcs.fore_z
    );
    let mut spec: CraftFile = ron::from_str(&text).unwrap_or_else(|e| panic!("stress craft file: {e}"));
    if let Err(e) = super::file::validate_craft(&spec) {
        panic!("stress craft file: {e}");
    }
    aggregate(&mut spec, &b.parts);
    // The core's tank section (where a one-tank model would put it).
    let tank = Tank { base: DVec3::ZERO, axis: DVec3::Z * tank_top, radius: R_CORE - 0.1 };
    let geometry = GeometryFile { skin: aluminium(), tank, primitives: b.prims };
    StressLayout { spec, geometry, parts: b.parts, kinds: b.kinds }
}

#[cfg(test)]
#[path = "stress_tests.rs"]
mod tests;

// The stress craft of catalog parts (D121): what `stress-<n>` names.

/// The core stack besides its tanks: the capsule, a thrust structure, four
/// engines and four legs.
const CORE_PARTS: usize = 10;
/// A booster besides its tanks: a nose cone, a thrust structure and four
/// engines.
const BOOSTER_PARTS: usize = 6;
/// The radius of booster ring k is k times this (m): a booster's tank is
/// 2 m across.
const RING_SPACING: f64 = 2.3;
/// The height of the boosters' nose cones' bases (m; the capsule's base is
/// at 0, the core's tanks below it).
const BOOSTER_TOP: f64 = -1.0;

/// How [`file`] spends `n` parts.
#[derive(Clone, Debug, PartialEq)]
pub struct PartsPlan {
    /// Tanks in the core stack.
    pub core_tanks: usize,
    /// The core's thrust structure and four engines.
    pub engines: bool,
    /// The core's four legs.
    pub legs: bool,
    /// Rings of boosters round the core (ring k: 4k boosters).
    pub rings: usize,
    /// Each booster's tanks, ring by ring.
    pub boosters: Vec<usize>,
}

/// The plan of `n` parts (n ≥ 1): the capsule alone, then tanks under it,
/// then a thrust structure and engines, then legs, then rings of boosters
/// of `1 + n/600` tanks while they fit; the rest go to the core's tanks (at
/// most `2 + n/600`) and then to the boosters, one more tank each in turn.
pub fn parts_plan(n: usize) -> PartsPlan {
    let n = n.max(1);
    let small = |core_tanks, engines| PartsPlan { core_tanks, engines, legs: false, rings: 0, boosters: Vec::new() };
    if n < 7 {
        return small(n - 1, false);
    }
    if n <= CORE_PARTS {
        return small(n - 6, true);
    }
    let tanks = 1 + n / 600;
    let per = BOOSTER_PARTS + tanks;
    let boosters_in = |rings: usize| 2 * rings * (rings + 1);
    let mut rings = 0;
    while CORE_PARTS + 1 + boosters_in(rings + 1) * per <= n {
        rings += 1;
    }
    let count = boosters_in(rings);
    let left = n - CORE_PARTS - count * per;
    let core_tanks = if count == 0 { left } else { left.min(2 + n / 600) };
    let extra = left - core_tanks;
    let boosters = (0..count).map(|j| tanks + extra / count + usize::from(j < extra % count)).collect();
    PartsPlan { core_tanks, engines: true, legs: true, rings, boosters }
}

/// The lists of a stress craft's assembly file, as RON text.
#[derive(Default)]
struct Text {
    parts: String,
    extra: String,
    joints: String,
    plumbing: String,
    loading: String,
}

impl Text {
    /// A tank mounted under `on`'s mount `at`, loaded full of LOX and RP-1.
    fn tank(&mut self, id: &str, on: &str, at: &str) {
        use std::fmt::Write;
        let _ = writeln!(self.parts, r#"(id: "{id}", entry: "t-tank", mount: (on: "{on}", at: "{at}", via: "top")),"#);
        let ox = r#"temperature_k: 90.0, pressure: 3.0e5, pressurant: "helium""#;
        let fuel = r#"temperature_k: 293.0, pressure: 3.0e5, pressurant: "helium""#;
        let _ = writeln!(self.loading, r#"(tank: "{id}:ox", {ox}), (tank: "{id}:fuel", {fuel}),"#);
    }

    /// Lines from `tank`'s outlets to the engines of `stack`.
    fn feed(&mut self, tank: &str, stack: &str) {
        use std::fmt::Write;
        for (c, length) in [("ox", 2.0), ("fuel", 1.0)] {
            let _ = writeln!(
                self.plumbing,
                r#"(from: "{tank}:{c}.outlet", to: "{stack}.engine#*:{c}_in", diameter: 0.05, length: {length:?}),"#
            );
        }
    }

    /// Four engines on `stack`'s thrust structure.
    fn engines_on(&mut self, stack: &str) {
        use std::fmt::Write;
        let _ = writeln!(
            self.parts,
            r#"(id: "{stack}.engine", entry: "t-engine", pattern: List(4), mount: (on: "{stack}.thrust", at: "engine_#", via: "mount")),"#
        );
    }

    /// The rings of boosters of plan `p`; returns the boosters' names.
    fn rings(&mut self, p: &PartsPlan) -> Vec<String> {
        use std::fmt::Write;
        let mut boosters = vec![];
        let mut b = 0;
        for k in 1..=p.rings {
            let count = 4 * k;
            for j in 0..count {
                let a = 2.0 * crate::math::PI * j as f64 / count as f64;
                let r = RING_SPACING * k as f64;
                let (x, y) = (r * crate::math::cos(a), r * crate::math::sin(a));
                let parent = if k == 1 {
                    "core.tank1".to_string()
                } else {
                    let inner = 4 * (k - 1);
                    let first = 2 * (k - 1) * (k - 2);
                    format!("b{}.nose", first + (j * (k - 1) + k / 2) / k % inner + 1)
                };
                let tanks = p.boosters[b];
                b += 1;
                let stack = format!("b{b}");
                let _ = writeln!(
                    self.parts,
                    r#"(id: "{stack}.nose", entry: "t-nose", at: (on: Some("{parent}"), pos: ({x:?}, {y:?}, {BOOSTER_TOP:?}))),"#
                );
                let mut above = format!("{stack}.nose");
                for i in 1..=tanks {
                    let id = format!("{stack}.tank{i}");
                    self.tank(&id, &above, "bottom");
                    above = id;
                }
                let _ = writeln!(
                    self.parts,
                    r#"(id: "{stack}.thrust", entry: "t-thrust", mount: (on: "{above}", at: "bottom", via: "top")),"#
                );
                self.engines_on(&stack);
                self.feed(&above, &stack);
                let _ = writeln!(
                    self.joints,
                    r#"(name: "sep.{stack}", between: ("{parent}", "{stack}.nose"), kind: Separation(system: Clamp, impulse: 500.0, strength: 0.8)),"#
                );
                boosters.push(stack);
            }
        }
        boosters
    }
}

/// The assembly file of the stress craft of `n` parts (see [`parts_plan`]): a
/// craft of [`super::small`]'s catalog parts. The capsule carries the
/// small rocket's computer, battery, cells, IMU, GPS receiver, seat and
/// four thruster blocks (their bottle in the core's top tank). Each stack's
/// lowest tank feeds its engines; each booster hangs on a separation joint
/// from the core's top tank (ring 1) or the nearest booster of the ring
/// inside. The sequence: Ignition (every engine), Boosters (shut down and
/// separated), Separation (the capsule). Deterministic: positions from
/// `sim::math`, written exactly.
pub fn file(n: usize) -> AssemblyFile {
    use std::fmt::Write;
    let p = parts_plan(n);
    let mut t = Text::default();
    let mut engines = vec![];
    // The core.
    let mut above = ("capsule".to_string(), "bottom");
    for i in 1..=p.core_tanks {
        let id = format!("core.tank{i}");
        t.tank(&id, &above.0, above.1);
        above = (id, "bottom");
    }
    if p.engines {
        let _ = writeln!(
            t.parts,
            r#"(id: "core.thrust", entry: "t-thrust", mount: (on: "{}", at: "bottom", via: "top")),"#,
            above.0
        );
        t.engines_on("core");
        t.feed(&above.0, "core");
        engines.push("core.engine#*".to_string());
    }
    if p.legs {
        t.parts.push_str(r#"(id: "core.leg", entry: "t-leg", pattern: List(4), mount: (on: "core.thrust", at: "leg_#", via: "root")),"#);
    }
    let boosters = t.rings(&p);
    // The capsule's equipment (its thrusters' bottle in the core's top tank).
    let mut members = vec!["battery", "computer", "imu", "gps", "solar"];
    let mut data: Vec<String> = engines.clone();
    data.extend(boosters.iter().map(|s| format!("{s}.engine#*")));
    data.extend(["imu", "gps"].map(String::from));
    if p.core_tanks > 0 {
        t.extra.push_str(r#"(id: "bottle", entry: "t-bottle", in: "core.tank1", pos: (0.0, 0.0, 3.0)),
            (id: "rcs", entry: "t-rcs", pattern: Radial(count: 4, start_deg: 0.0), in: "capsule", pos: (0.7, 0.0, 0.3)),"#);
        t.plumbing.push_str(r#"(from: "bottle:out", to: "rcs#*:in", diameter: 0.01, length: 3.0),"#);
        members.push("rcs#*");
        data.push("rcs#*".into());
        t.joints.push_str(r#"(name: "separation", between: ("capsule", "core.tank1"), kind: Separation(system: Clamp, impulse: 500.0, strength: 0.8)),"#);
    }
    let list = |v: &[String]| v.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(", ");
    let members: Vec<String> = members.iter().map(|s| s.to_string()).collect();
    let mut groups = format!(r#""main": [{}]"#, list(&engines));
    let booster_engines: Vec<String> = boosters.iter().map(|s| format!("{s}.engine#*")).collect();
    let mut sequence = String::new();
    if p.engines {
        let ignite = if boosters.is_empty() { r#"Ignite("main")"# } else { r#"Ignite("main"), Ignite("boosters")"# };
        let _ = writeln!(sequence, r#"(name: "Ignition", actions: [{ignite}]),"#);
    }
    if !boosters.is_empty() {
        let _ = write!(groups, r#", "boosters": [{}]"#, list(&booster_engines));
        let separate: Vec<String> = boosters.iter().map(|s| format!(r#"Separate("sep.{s}")"#)).collect();
        let _ = writeln!(sequence, r#"(name: "Boosters", actions: [Shutdown("boosters"), {}]),"#, separate.join(", "));
    }
    if p.engines {
        let _ = writeln!(sequence, r#"(name: "Separation", actions: [Shutdown("main"), Separate("separation")]),"#);
    }
    let functions = if p.legs { r#"(name: "Legs", key: "L", actions: [Deploy("core.leg#*")])"# } else { "" };
    let Text { parts, extra, joints, plumbing, loading } = t;
    let text = format!(
        r#"(
    name: "Stress craft of {n} parts", crew: 1,
    parts: [(id: "capsule", entry: "t-capsule"),
{parts}],
    components: [
        (id: "computer", entry: "t-computer", in: "capsule", pos: (0.0, 0.0, 0.6)),
        (id: "battery", entry: "t-battery", in: "capsule", pos: (0.0, 0.4, 0.6)),
        (id: "seat", entry: "t-seat", in: "capsule", pos: (0.0, 0.0, 0.3)),
        (id: "imu", entry: "t-imu", in: "capsule", pos: (0.3, 0.0, 0.6)),
        (id: "gps", entry: "t-gps", in: "capsule", pos: (-0.3, 0.0, 0.6)),
        (id: "solar", entry: "t-solar", in: "capsule", pos: (0.45, 0.45, 0.3)),
{extra}],
    joints: [{joints}],
    plumbing: [{plumbing}],
    electrical: [(name: "bus", voltage: 28.0, members: [{}])],
    data: [(from: "computer", to: [{}])],
    loading: [{loading}],
    groups: {{ {groups} }},
    sequence: [{sequence}],
    functions: [{functions}],
    aero: (cd0: 0.4),
    thermal: (skin_max_k: 1100.0, internal_max_k: 400.0, node_size: 0.5, interior_specific_heat: 900.0,
              propellant_specific_heat: 2000.0, interior_conductivity: 5.0, internal_coupling: 2.0),
    impact: (max_speed: 8.0),
    contact: (foot: (stiffness: 1.0e5, damping: 1.0e4, stroke: 0.3, friction: 0.8),
              hull: (stiffness: 1.0e6, damping: 1.0e5, friction: 0.5), stick_speed: 0.05),
    antenna: (gain_dbi: 3.0, power_w: 5.0),
)"#,
        list(&members),
        list(&data),
    );
    crate::catalog::ron_options().from_str(&text).unwrap_or_else(|e| panic!("stress craft of {n} parts: {e}"))
}

/// The definition a stress craft's base id (`stress-<n>`) names (`None`:
/// not a stress craft), built from [`super::small::catalog`].
pub fn definition(base: &str) -> Option<Result<CraftDef, String>> {
    let n = base.strip_prefix("stress-")?.parse::<usize>().ok().filter(|&n| n > 0)?;
    let ctx = crate::catalog::Ctx::data();
    Some(super::build::build(&file(n), super::small::catalog(), &ctx).map(|a| a.def).map_err(|e| e.join("\n")))
}

/// The definition of the stress craft of `n` parts (its primitives and
/// parts, without the cells and mass a [`craft`] builds). Panics if it
/// does not build.
pub fn def(n: usize) -> CraftDef {
    definition(&id(n).0).expect("a stress craft's id").unwrap_or_else(|e| panic!("{e}"))
}

/// The stress craft of `n` parts (`stress-<n>`), built once per process.
pub fn craft(n: usize) -> std::sync::Arc<Craft> {
    super::craft_of(&id(n))
}
