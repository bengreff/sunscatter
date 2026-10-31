//! Stage estimates (D111; the staging bar): for the burn going on now and
//! for each stage of the vessel's sequence still to run in its view
//! (`craft::program`: the stages of other pieces are left out), which tanks its
//! engines draw on, what each propellant holds of those tanks' capacity,
//! how long until the first propellant runs out (burnout), the Δv, and the
//! thrust, thrust-to-weight ratio, mass at the start and at burnout and
//! mean Isp.
//!
//! **How:** the real feed and engine code run forward on a light copy of
//! the ship state ([`propulsion::output`]: each engine draws on the first
//! tank of each feed list holding liquid, at the steady map's flow for the
//! held ambient pressure), from one tank running dry to the next: between
//! two such events every flow and thrust is constant, so each span's Δv is
//! exactly `F/ṁ · ln(m₀/m₁)` and nothing is stepped. Nothing more is
//! commanded ("all estimations within a stage assume no input"): the
//! engines hold their setting at the pilot's throttle now. A stage's own
//! commands act as the sequence would apply them (ignitions, shutdowns,
//! throttles, separations dropping what the vessel would not keep, by
//! `vessel::split`'s rule: the piece with the lowest control part, else the
//! heaviest); a stage has a burn when engines burn after its commands, and
//! the burn runs until the first of its propellants is out. Later stages
//! start from what is left; the pad's mount (ground) is never counted.
//!
//! **What is known (D106):** a burn drawing on a tank no working level
//! sensor measures has no bar for that propellant, no burnout and no Δv
//! (dashes). Everything else is shown to the precision the sensors give:
//! the walk runs again with every tank at the low and at the high end of
//! what is known of it (an unmeasured one empty and full), and each
//! number's precision is its largest change. Debug mode knows all exactly.
//!
//! Cheap (a few spans per burn), but the game asks a few times a second,
//! not every frame.

use super::command::{can_ignite, reachable};
use super::data::{Knowledge, Quantity, Reading};
use super::plumbing;
use super::propulsion;
use crate::craft::assembly::JointKind;
use crate::craft::command::ShipCommand;
use crate::craft::ship::ShipDef;
use crate::craft::state::{EngineStatus, ShipState};
use crate::fluids::fluids;
use crate::math;
use crate::vessel::G0;
use std::collections::{BTreeMap, BTreeSet};

/// A part's dry mass (kg, its components' included) and whether it can
/// command the craft (what the vessel keeps at a separation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hull {
    pub part: u32,
    pub dry: f64,
    pub control: bool,
}

/// The hull of a craft's parts (`CraftDesign::parts`).
pub fn hull(parts: &[crate::craft::parts::Part]) -> Vec<Hull> {
    parts.iter().map(|p| Hull { part: p.id, dry: p.dry_mass, control: p.control }).collect()
}

/// What an estimate reads besides the ship.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    pub hull: &'a [Hull],
    pub knowledge: &'a Knowledge,
    /// The pilot's throttle, held.
    pub throttle: f64,
    /// The ambient pressure held over every burn (Pa): the air's now, or
    /// zero for the vacuum figures.
    pub p: f64,
    /// Local gravity (m/s²) for the thrust-to-weight ratio; `None` when the
    /// vessel does not know where it is.
    pub g: Option<f64>,
}

/// One propellant of a burn: its tanks and what they hold.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub fluid: String,
    /// What the tanks hold at the burn's start (kg), if measured.
    pub kg: Option<Reading>,
    /// Their capacity (kg).
    pub capacity: f64,
    /// Indices into `ShipDef::tanks`.
    pub tanks: Vec<u32>,
}

/// A burn: engines burning from a stage (or now) until the first of their
/// propellants is out.
#[derive(Clone, Debug, PartialEq)]
pub struct Burn {
    /// Indices into `ShipDef::engines`.
    pub engines: Vec<u32>,
    pub bars: Vec<Bar>,
    /// From the start to burnout (s; infinite when the setting gives no
    /// flow), and the Δv (m/s).
    pub time: Option<Reading>,
    pub dv: Option<Reading>,
    /// At the start: thrust (N), mass (kg), thrust-to-weight ratio.
    pub thrust: Option<Reading>,
    pub mass: Reading,
    pub twr: Option<Reading>,
    /// Mass at burnout (kg).
    pub burnout_mass: Option<Reading>,
    /// Mean specific impulse (s): thrust over flow.
    pub isp: f64,
}

/// The estimates of a vessel's burns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Estimates {
    /// The burn going on now (engines starting, running or waiting for
    /// their propellant to settle), if any.
    pub now: Option<Burn>,
    /// Each stage still to run in the craft's view (its index into
    /// `ShipState::stages`, the program) and its burn, if engines burn
    /// after its commands.
    pub stages: Vec<(usize, Option<Burn>)>,
}

/// The estimates for the vessel of `def` in `state` (see the module).
pub fn estimate(def: &ShipDef, state: &ShipState, inp: &Inputs) -> Estimates {
    let k = inp.knowledge;
    let truth: Vec<f64> = state.tanks.iter().map(|t| t.liquid).collect();
    let main = walk(def, state, inp, &truth);
    let others = if k.perfect {
        Vec::new()
    } else {
        vec![walk(def, state, inp, &bound(def, state, k, false)), walk(def, state, inp, &bound(def, state, k, true))]
    };
    let now = main.now.as_ref().map(|r| finish(def, inp, r, others.iter().map(|o| o.now.as_ref())));
    let stages = (main.stages.iter().enumerate())
        .map(|(j, (index, raw))| {
            let alts = others.iter().map(|o| o.stages.get(j).and_then(|s| s.1.as_ref()));
            (*index, raw.as_ref().map(|r| finish(def, inp, r, alts)))
        })
        .collect();
    Estimates { now, stages }
}

/// The burn engines `engines` (indices into `ShipDef::engines`) would make
/// lit now at the held throttle, every other engine off, until the first of
/// their propellants is out: the same walk and readings as the staging
/// bar's, for a burn not yet started (a planned burn's, `systems::maneuver`).
/// `None` when none of them is aboard.
pub fn lit(def: &ShipDef, state: &ShipState, inp: &Inputs, engines: &[u32]) -> Option<Burn> {
    let run = |levels: &[f64]| {
        let mut w = Walk::new(def, state, inp, levels);
        for i in 0..def.engines.len() {
            if engines.contains(&(i as u32)) && !w.gone.contains(&def.engines[i].part) {
                w.light(i);
            } else {
                w.douse(i);
            }
        }
        w.burn()
    };
    let truth: Vec<f64> = state.tanks.iter().map(|t| t.liquid).collect();
    let main = run(&truth)?;
    let k = inp.knowledge;
    let others: Vec<Option<Raw>> =
        if k.perfect { Vec::new() } else { vec![run(&bound(def, state, k, false)), run(&bound(def, state, k, true))] };
    Some(finish(def, inp, &main, others.iter().map(Option::as_ref)))
}

/// What is aboard a vessel now: its mass, the pad's mount not counted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aboard {
    /// Its mass (kg) to the precision the sensors give.
    pub mass: Reading,
    /// Its dry mass (kg): the parts aboard and their components.
    pub dry: f64,
}

/// What is aboard now (see [`Aboard`]): the estimate's own sum, the mass a
/// burn going now starts from (`Burn::mass`), with its precision the
/// largest change with every tank at the low and at the high end of what
/// `knowledge` knows of it (an unmeasured one empty and full). One rule for
/// every readout of the mass now: the staging bar's burn, the systems
/// screen's (interface-map finding 41: the screen dashed the whole when a
/// tank was unmeasured, the bar gave it to its precision).
pub fn aboard(def: &ShipDef, state: &ShipState, hull: &[Hull], knowledge: &Knowledge) -> Aboard {
    let inp = Inputs { hull, knowledge, throttle: 0.0, p: 0.0, g: None };
    let at = |levels: &[f64]| Walk::new(def, state, &inp, levels).mass();
    let truth: Vec<f64> = state.tanks.iter().map(|t| t.liquid).collect();
    let value = at(&truth);
    let spread = |high: bool| (at(&bound(def, state, knowledge, high)) - value).abs();
    let precision = if knowledge.perfect { 0.0 } else { spread(false).max(spread(true)) };
    let w = Walk::new(def, state, &inp, &truth);
    let dry = hull.iter().filter(|h| !w.gone.contains(&h.part)).map(|h| h.dry).sum();
    Aboard { mass: Reading { value, precision }, dry }
}

/// Every tank at the low (or high) end of what is known of it: an
/// unmeasured one empty (or full).
fn bound(def: &ShipDef, state: &ShipState, k: &Knowledge, high: bool) -> Vec<f64> {
    (0..def.tanks.len()).map(|i| if high { range(def, state, k, i).1 } else { range(def, state, k, i).0 }).collect()
}

/// What tank `i` may hold (kg, low and high) as `k` knows it: its reading
/// within its precision, an unmeasured one empty to full.
fn range(def: &ShipDef, state: &ShipState, k: &Knowledge, i: usize) -> (f64, f64) {
    let (s, d) = (&state.tanks[i], &def.tanks[i]);
    let full = d.capacity.max(s.liquid);
    match k.precision(Quantity::TankLevel(i as u32)) {
        Some(p) => ((s.liquid - p).max(0.0), (s.liquid + p).min(full)),
        None => (0.0, full),
    }
}

/// Levels across a tank's range that [`centre`] tries for its extremes.
const LEVELS: usize = 9;

/// Where the centre of mass of what is aboard sits now: its height above
/// the lowest point aboard (m, along the craft's axis), to the precision
/// the tank sensors give: the farthest it moves with each tank anywhere in
/// what `knowledge` knows of it (an unmeasured one empty to full), found
/// by a search over each tank's levels. What is aboard is [`aboard`]'s: the
/// parts less the pad's mount (its mass and its foot left out), their
/// tanks' liquid where it sits and their gas (`plumbing::contents`' model).
/// `design` and `contacts` are the vessel's craft's. One rule for the
/// systems screen's centre of mass (interface-map finding 41: a dash while
/// any tank was unmeasured, and on the pad the mount's 50 t and its foot
/// counted).
pub fn centre(
    def: &ShipDef,
    state: &ShipState,
    design: &crate::craft::design::CraftDesign,
    contacts: &[crate::craft::cells::ContactPoint],
    knowledge: &Knowledge,
) -> Reading {
    let ground: BTreeSet<u32> = def.parts.iter().filter(|p| p.ground).map(|p| p.part).collect();
    let on = |part: u32| !ground.contains(&part);
    let molar = |g: &str| fluids().gas(g).map_or(0.0, |g| g.molar_mass);
    // The dry parts and the bottles' gas, which no level changes: (mass,
    // its moment along z).
    let mut fixed = (0.0, 0.0);
    for (_, m) in design.parts.iter().zip(&design.part_mass).filter(|(p, _)| on(p.id)) {
        fixed = (fixed.0 + m.mass, fixed.1 + m.mass * m.com.z);
    }
    for (b, d) in state.bottles.iter().zip(&def.bottles).filter(|(_, d)| on(d.part)) {
        let kg = b.moles * molar(&d.gas);
        fixed = (fixed.0 + kg, fixed.1 + kg * d.pos.z);
    }
    // A tank holding `kg`: its liquid where it sits and its ullage's gas.
    let tank = |i: usize, kg: f64| {
        let (d, s) = (&def.tanks[i], &state.tanks[i]);
        let s = crate::craft::state::TankState { liquid: kg, ..s.clone() };
        let slug = plumbing::slug(d, &s);
        let gas = s.ullage_moles * molar(&d.load.pressurant);
        let z = slug.base.z + slug.axis.z * 0.5 * slug.height;
        (kg + gas, kg * z + gas * plumbing::ullage_centre(d, &slug).z)
    };
    let tanks: Vec<usize> = (0..def.tanks.len()).filter(|&i| on(def.tanks[i].part)).collect();
    let now: Vec<(f64, f64)> = tanks.iter().map(|&i| tank(i, state.tanks[i].liquid)).collect();
    let sum = |items: &[(f64, f64)]| items.iter().fold(fixed, |(m, mz), (a, b)| (m + a, mz + b));
    let height = |(m, mz): (f64, f64)| if m > 0.0 { mz / m } else { 0.0 };
    let z = height(sum(&now));
    // Each tank across what is known of it (none when known exactly).
    let tries: Vec<Vec<(f64, f64)>> = (tanks.iter())
        .map(|&i| match range(def, state, knowledge, i) {
            (lo, hi) if hi > lo => {
                (0..LEVELS).map(|n| tank(i, lo + (hi - lo) * n as f64 / (LEVELS - 1) as f64)).collect()
            }
            _ => Vec::new(),
        })
        .collect();
    // The highest (`sign` 1) or lowest (−1): each tank in turn to its best
    // level with the others held, until no tank moves it.
    let extreme = |sign: f64| {
        let mut items = now.clone();
        let mut best = sign * z;
        for _ in 0..tanks.len().max(1) {
            let mut moved = false;
            for (n, levels) in tries.iter().enumerate() {
                for &t in levels {
                    let held = std::mem::replace(&mut items[n], t);
                    let v = sign * height(sum(&items));
                    if v > best + 1e-12 {
                        (best, moved) = (v, true);
                    } else {
                        items[n] = held;
                    }
                }
            }
            if !moved {
                break;
            }
        }
        sign * best
    };
    let precision = if knowledge.perfect { 0.0 } else { (extreme(1.0) - z).max(z - extreme(-1.0)) };
    // The lowest point aboard: its lowest contact point (the craft's own
    // bottom, `CraftParams::bottom_z`'s rule, less the mount's).
    let part_of = |c: &crate::craft::cells::ContactPoint| design.cell_part.get(c.cell as usize).copied();
    let base = (contacts.iter()).filter(|c| part_of(c).is_some_and(on)).map(|c| c.pos.z).fold(f64::INFINITY, f64::min);
    let base = if base.is_finite() { base } else { contacts.iter().map(|c| c.pos.z).fold(z, f64::min) };
    Reading { value: z - base, precision }
}

/// A burn's raw numbers in one walk.
#[derive(Clone, Debug, Default, PartialEq)]
struct Raw {
    engines: Vec<u32>,
    tanks: Vec<u32>,
    /// What each of `tanks` holds at the start (kg).
    held: Vec<f64>,
    time: f64,
    dv: f64,
    thrust: f64,
    mdot: f64,
    m0: f64,
    m1: f64,
}

/// One walk's burns.
struct Walked {
    now: Option<Raw>,
    stages: Vec<(usize, Option<Raw>)>,
}

/// Turns a burn's numbers into readings: what is measured, to the largest
/// change across the other walks (`alts`).
fn finish<'a>(def: &ShipDef, inp: &Inputs, raw: &Raw, alts: impl Iterator<Item = Option<&'a Raw>>) -> Burn {
    let k = inp.knowledge;
    let alts: Vec<&Raw> = alts.flatten().collect();
    let spread = |f: &dyn Fn(&Raw) -> f64| alts.iter().map(|a| (f(a) - f(raw)).abs()).fold(0.0, f64::max);
    let level = |t: u32| k.precision(Quantity::TankLevel(t));
    let measured = raw.tanks.iter().all(|&t| level(t).is_some());
    let mut bars: Vec<Bar> = Vec::new();
    for (&t, &held) in raw.tanks.iter().zip(&raw.held) {
        let d = &def.tanks[t as usize];
        let at = match bars.iter().position(|b| b.fluid == d.fluid) {
            Some(b) => b,
            None => {
                let kg = Some(Reading { value: 0.0, precision: 0.0 });
                bars.push(Bar { fluid: d.fluid.clone(), kg, capacity: 0.0, tanks: Vec::new() });
                bars.len() - 1
            }
        };
        let b = &mut bars[at];
        b.capacity += d.capacity;
        b.tanks.push(t);
        b.kg = b.kg.zip(level(t)).map(|(r, p)| Reading { value: r.value + held, precision: r.precision + p });
    }
    let reading = |f: &dyn Fn(&Raw) -> f64| Reading { value: f(raw), precision: spread(f) };
    let thrust_known = raw.engines.iter().map(|&i| k.precision(Quantity::EngineThrust(i)));
    let thrust =
        thrust_known.sum::<Option<f64>>().map(|p| Reading { value: raw.thrust, precision: p + spread(&|r| r.thrust) });
    let mass = reading(&|r| r.m0);
    let twr = thrust.zip(inp.g.filter(|g| *g > 0.0)).filter(|_| mass.value > 0.0).map(|(f, g)| {
        let value = f.value / (mass.value * g);
        let rel = if f.value > 0.0 { f.precision / f.value } else { 0.0 } + mass.precision / mass.value;
        Reading { value, precision: value * rel }
    });
    let isp = if raw.mdot > 0.0 { raw.thrust / (raw.mdot * G0) } else { 0.0 };
    Burn {
        engines: raw.engines.clone(),
        bars,
        time: measured.then(|| reading(&|r| r.time)),
        dv: measured.then(|| reading(&|r| r.dv)),
        thrust,
        mass,
        twr,
        burnout_mass: measured.then(|| reading(&|r| r.m1)),
        isp,
    }
}

/// The walk's state: a light copy of the ship state, the parts no longer
/// aboard, and each part's gas (bottles and ullages, which no burn spends).
struct Walk<'a> {
    def: &'a ShipDef,
    hull: &'a [Hull],
    s: ShipState,
    gone: BTreeSet<u32>,
    gas: BTreeMap<u32, f64>,
    throttle: f64,
    p: f64,
}

/// Runs the walk (see the module) with the tanks holding `levels`.
fn walk(def: &ShipDef, state: &ShipState, inp: &Inputs, levels: &[f64]) -> Walked {
    let mut w = Walk::new(def, state, inp, levels);
    // The burn now: what is starting, running or waiting to start.
    for i in 0..def.engines.len() {
        let e = &w.s.engines[i];
        let settling = state.engines.get(i).is_some_and(|r| r.settling.is_some());
        let burning = matches!(e.status, EngineStatus::Starting | EngineStatus::Running);
        if (burning || (settling && e.status == EngineStatus::Off)) && !w.gone.contains(&def.engines[i].part) {
            w.light(i);
        } else {
            w.douse(i);
        }
    }
    let now = w.burn();
    // The stages still to run in the craft's view (`craft::program`).
    let stages = (crate::craft::program::pending(def, &state.stages, state.sequence).into_iter())
        .map(|j| {
            for c in &crate::craft::program::mine(def, &state.stages[j]).actions {
                w.command(c);
            }
            (j, w.burn())
        })
        .collect();
    Walked { now, stages }
}

/// Spans shorter than this end a burn (s): nothing more flows.
const TINY: f64 = 1e-9;

impl<'a> Walk<'a> {
    fn new(def: &'a ShipDef, state: &ShipState, inp: &Inputs<'a>, levels: &[f64]) -> Self {
        let mut s = ShipState {
            tanks: state.tanks.clone(),
            engines: state.engines.clone(),
            joints: state.joints.clone(),
            clamped: state.clamped,
            ..ShipState::default()
        };
        s.lost = state.lost.clone();
        s.unpowered = state.unpowered.clone();
        // Each burn of the walk settles its liquids on their outlets.
        for (t, &kg) in s.tanks.iter_mut().zip(levels) {
            (t.liquid, t.drift, t.drift_speed) = (kg, 0.0, 0.0);
        }
        let molar = |g: &str| fluids().gas(g).map_or(0.0, |g| g.molar_mass);
        let mut gas: BTreeMap<u32, f64> = BTreeMap::new();
        for (b, d) in state.bottles.iter().zip(&def.bottles) {
            *gas.entry(d.part).or_default() += b.moles * molar(&d.gas);
        }
        for (t, d) in state.tanks.iter().zip(&def.tanks) {
            *gas.entry(d.part).or_default() += t.ullage_moles * molar(&d.load.pressurant);
        }
        let gone = def.parts.iter().filter(|p| p.ground).map(|p| p.part).collect();
        Walk { def, hull: inp.hull, s, gone, gas, throttle: inp.throttle, p: inp.p }
    }

    /// What is aboard (kg): the parts' dry mass, the tanks' liquid, the gas.
    fn mass(&self) -> f64 {
        let aboard = |part: &u32| !self.gone.contains(part);
        let dry: f64 = self.hull.iter().filter(|h| aboard(&h.part)).map(|h| h.dry).sum();
        let liquid: f64 =
            (self.s.tanks.iter().zip(&self.def.tanks)).filter(|(_, d)| aboard(&d.part)).map(|(t, _)| t.liquid).sum();
        let gas: f64 = self.gas.iter().filter(|(p, _)| aboard(p)).map(|(_, kg)| kg).sum();
        dry + liquid + gas
    }

    /// Engine `i` burns at its setting for the held throttle.
    fn light(&mut self, i: usize) {
        let d = &self.def.engines[i].data;
        let e = &mut self.s.engines[i];
        e.status = EngineStatus::Running;
        self.s.engines[i].level = propulsion::setting(d, self.throttle, e.throttle);
    }

    fn douse(&mut self, i: usize) {
        self.s.engines[i].status = EngineStatus::Off;
        self.s.engines[i].level = 0.0;
    }

    fn lit(&self) -> Vec<usize> {
        (0..self.def.engines.len()).filter(|&i| self.s.engines[i].status == EngineStatus::Running).collect()
    }

    /// Whether both of engine `i`'s feed lists hold liquid.
    fn fed(&self, i: usize) -> bool {
        let holds = |l: &[u32]| l.iter().any(|&t| self.s.tanks[t as usize].liquid > 0.0);
        self.def.engines[i].feeds.iter().all(|l| holds(l))
    }

    /// A stage's command as the sequence would apply it (what changes the
    /// burns; anything else does not).
    fn command(&mut self, c: &ShipCommand) {
        let def = self.def;
        match c {
            ShipCommand::Ignite(sel) => {
                for i in def.engines_of(sel) {
                    let (d, e) = (&def.engines[i], &self.s.engines[i]);
                    let ready = e.status == EngineStatus::Off
                        && !self.gone.contains(&d.part)
                        && can_ignite(d.data.igniter, e.ignitions, self.s.clamped)
                        && reachable(def, &self.s, d.owner);
                    if ready {
                        let e = &mut self.s.engines[i];
                        if !self.s.clamped && e.ignitions != u32::MAX && e.ignitions > 0 {
                            e.ignitions -= 1;
                        }
                        self.light(i);
                    }
                }
            }
            ShipCommand::Shutdown(sel) => def.engines_of(sel).into_iter().for_each(|i| self.douse(i)),
            ShipCommand::Throttle(sel, x) => {
                for i in def.engines_of(sel) {
                    self.s.engines[i].throttle = x.clamp(0.0, 1.0);
                    if self.s.engines[i].status == EngineStatus::Running {
                        self.light(i);
                    }
                }
            }
            ShipCommand::Separate(name) => {
                if let Some(j) = def.joint_named(name) {
                    self.s.joints[j].intact = false;
                    self.regroup();
                }
            }
            ShipCommand::ReleaseClamps => self.s.clamped = false,
            _ => {}
        }
    }

    /// After a separation: the parts still joined to the piece the vessel
    /// keeps stay aboard; the rest leave with their tanks and engines.
    fn regroup(&mut self) {
        let def = self.def;
        let aboard: Vec<u32> = def.parts.iter().map(|p| p.part).filter(|p| !self.gone.contains(p)).collect();
        let joined: Vec<(u32, u32)> = (def.joints.iter().zip(&self.s.joints))
            .filter(|(d, s)| s.intact && d.kind != JointKind::HoldDown)
            .map(|(d, _)| (d.parent, d.child))
            .collect();
        let pieces = pieces(&aboard, &joined);
        let hull = |p: u32| self.hull.iter().find(|h| h.part == p);
        let control = |g: &Vec<u32>| g.iter().copied().filter(|&p| hull(p).is_some_and(|h| h.control)).min();
        let dry = |g: &Vec<u32>| g.iter().filter_map(|&p| hull(p)).map(|h| h.dry).sum::<f64>();
        let keep = (0..pieces.len()).min_by(|&a, &b| match (control(&pieces[a]), control(&pieces[b])) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => dry(&pieces[b]).total_cmp(&dry(&pieces[a])),
        });
        for (k, g) in pieces.iter().enumerate() {
            if Some(k) != keep {
                self.gone.extend(g);
            }
        }
        for i in 0..def.engines.len() {
            if self.gone.contains(&def.engines[i].part) {
                self.douse(i);
            }
        }
        for (t, d) in self.s.tanks.iter_mut().zip(&def.tanks) {
            if self.gone.contains(&d.part) {
                t.liquid = 0.0;
            }
        }
    }

    /// The burn of the engines burning now, to the first propellant out
    /// (engines without feed flame out first); `None` if none burns.
    fn burn(&mut self) -> Option<Raw> {
        for i in self.lit() {
            if !self.fed(i) {
                self.douse(i);
            }
        }
        let engines = self.lit();
        if engines.is_empty() {
            return None;
        }
        let mut tanks: Vec<u32> = engines.iter().flat_map(|&i| self.def.engines[i].feeds.concat()).collect();
        tanks.sort_unstable();
        tanks.dedup();
        let held = tanks.iter().map(|&t| self.s.tanks[t as usize].liquid).collect();
        let start = propulsion::output(self.def, &self.s, self.p, TINY, false);
        let m0 = self.mass();
        let (mut time, mut dv, mut m) = (0.0, 0.0, m0);
        // A setting that gives no flow never burns out; its Δv is the full
        // throttle's.
        let (idle, held_throttle) = (start.mdot <= 0.0, self.throttle);
        if idle {
            self.throttle = 1.0;
            engines.iter().for_each(|&i| self.light(i));
        }
        // Each span empties at least one tank.
        for _ in 0..=self.def.tanks.len() {
            if engines.iter().any(|&i| !self.fed(i)) {
                break;
            }
            let out = propulsion::output(self.def, &self.s, self.p, TINY, false);
            let dt = (out.draws.iter().enumerate())
                .filter(|(_, d)| **d > 0.0)
                .map(|(t, d)| self.s.tanks[t].liquid / d)
                .fold(f64::INFINITY, f64::min);
            if !dt.is_finite() || dt < TINY || out.mdot <= 0.0 {
                break;
            }
            for (t, d) in out.draws.iter().enumerate().filter(|(_, d)| **d > 0.0) {
                let s = &mut self.s.tanks[t];
                let kg = if s.liquid / d <= dt { s.liquid } else { d * dt };
                plumbing::drain(s, kg);
            }
            let m1 = self.mass();
            if m1 > 0.0 && m1 < m {
                dv += out.thrust / out.mdot * math::ln(m / m1);
            }
            (m, time) = (m1, time + dt);
        }
        self.throttle = held_throttle;
        for &i in &engines {
            if self.fed(i) {
                self.light(i);
            } else {
                self.douse(i);
            }
        }
        Some(Raw {
            engines: engines.iter().map(|&i| i as u32).collect(),
            tanks,
            held,
            time: if idle { f64::INFINITY } else { time },
            dv,
            thrust: start.thrust,
            mdot: start.mdot,
            m0,
            m1: m,
        })
    }
}

/// The connected groups of `parts` joined by `joined` (parent, child),
/// each sorted, in order of their first part.
fn pieces(parts: &[u32], joined: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let mut piece: BTreeMap<u32, u32> = parts.iter().map(|&p| (p, p)).collect();
    // Merge until nothing changes (a craft's few dozen parts).
    loop {
        let mut changed = false;
        for &(a, b) in joined {
            let (Some(&x), Some(&y)) = (piece.get(&a), piece.get(&b)) else { continue };
            if x != y {
                let (lo, hi) = (x.min(y), x.max(y));
                piece.values_mut().filter(|v| **v == hi).for_each(|v| *v = lo);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (p, g) in piece {
        groups.entry(g).or_default().push(p);
    }
    groups.into_values().collect()
}

#[cfg(test)]
#[path = "estimate_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "estimate_centre_tests.rs"]
mod centre_tests;
