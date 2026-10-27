//! Structure (D103; ship-model.md §5.6): every joint's load each live tick,
//! exact for the rigid body, against its capacity at temperature.
//!
//! * **Loads.** The craft is rigid until it breaks, so the subtree of parts
//!   hanging from a joint (its child's side) is accelerated by the joint
//!   alone, beyond its own external forces: the force the parent's side
//!   exerts on it is F = Σᵢ mᵢ·aᵢ − Σ F_ext, with aᵢ the proper
//!   acceleration of part i's centre of mass, a + α×rᵢ + ω×(ω×rᵢ) (rᵢ from
//!   the craft's centre of mass; gravity acts on every mass alike, so it
//!   cancels), and the moment about the joint's centre likewise with each
//!   part's own Iᵢ·α + ω×(Iᵢ·ω). External forces are those other than
//!   gravity: each engine's thrust at its mount, each thruster, the air on
//!   each part's cells, the ground at each contact point, the parachutes,
//!   the pad's clamps. One pass over the tree, deepest parts first
//!   ([`subtree_loads`]).
//! * **The check** ([`section`], [`ratio`]): a ring joint of radius R
//!   carries its axial force P and bending moment M as the equivalent axial
//!   line load N = P/(2πR) ± M/(πR²) (the peak around the ring), tension
//!   against the wall's ultimate line load, compression against its
//!   buckling load (NASA SP-8007's knockdown, the generator's: a joint
//!   whose data has none takes the ultimate), shear and torsion as shear
//!   flow V/(πR) + T/(2πR²) against the ultimate over √3 (von Mises).
//!   Internal pressure in the section stabilises it: the gas carries p·πR²
//!   across the cut, so the wall's tension is P + p·πR² (a pressurised
//!   tank under thrust is relieved of compression). Strength falls with
//!   temperature as the weaker side's material's ultimate strength does.
//!   A point attachment (no ring) or a joint whose wall has no allowable has
//!   no capacity data: its load is shown, its ratio is zero and it never
//!   breaks ("if the simulator is not sure, don't trigger the failure").
//! * **Snap.** A joint at or over its ultimate load (ratio ≥ 1) snaps: the
//!   child's side leaves with what hangs from it (a split, `sim::vessel::split`).
//!   Repeated loads are not tracked for fatigue (no data).

use crate::craft::assembly::JointKind;
use crate::craft::ship::{JointDef, ShipDef};
use crate::craft::state::{Effect, ShipState};
use crate::craft::MassProps;
use crate::fluids::fluids;
use crate::math;
use crate::systems::plumbing;
use crate::time::Epoch;
use glam::{DMat3, DVec3};

/// The rigid body's motion as the structure feels it (body axes): the
/// centre of mass, the proper acceleration there (gravity left out), the
/// angular velocity and angular acceleration.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    pub com: DVec3,
    pub accel: DVec3,
    pub omega: DVec3,
    pub alpha: DVec3,
}

/// A force other than gravity on a part (body axes): `force` applied at
/// `at`, and a pure couple `torque`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Push {
    /// The part (an index into the list [`subtree_loads`] takes).
    pub part: usize,
    pub force: DVec3,
    pub at: DVec3,
    pub torque: DVec3,
}

/// The depth of each part in its tree (a root's is 0).
fn depths(parent: &[Option<usize>]) -> Vec<usize> {
    let mut depth = vec![usize::MAX; parent.len()];
    for i in 0..parent.len() {
        let mut chain = Vec::new();
        let mut at = i;
        while depth[at] == usize::MAX {
            chain.push(at);
            match parent[at] {
                Some(p) if chain.len() <= parent.len() => at = p,
                _ => {
                    depth[at] = 0;
                    chain.pop();
                    break;
                }
            }
        }
        let mut d = depth[at];
        for &c in chain.iter().rev() {
            d += 1;
            depth[c] = d;
        }
    }
    depth
}

/// For each part (parents given by index, mass properties with their
/// contents in body axes), the force and the moment about the body origin
/// that its parent must exert on the subtree hanging from it (the part and
/// its descendants) for that subtree to move with the rigid body, beyond
/// the `pushes` on it. A root's is what the whole tree lacks (zero when
/// the motion is the forces' own).
pub fn subtree_loads(parent: &[Option<usize>], mass: &[MassProps], pushes: &[Push], m: &Motion) -> Vec<(DVec3, DVec3)> {
    let n = parent.len();
    let mut out: Vec<(DVec3, DVec3)> = (0..n)
        .map(|i| {
            let p = &mass[i];
            let r = p.com - m.com;
            let f = (m.accel + m.alpha.cross(r) + m.omega.cross(m.omega.cross(r))) * p.mass;
            let spin: DMat3 = p.inertia;
            (f, p.com.cross(f) + spin * m.alpha + m.omega.cross(spin * m.omega))
        })
        .collect();
    for push in pushes.iter().filter(|p| p.part < n) {
        let o = &mut out[push.part];
        o.0 -= push.force;
        o.1 -= push.at.cross(push.force) + push.torque;
    }
    // Deepest first, so each subtree is complete before it joins its parent
    // (a fixed order: the sums are the same bits every time).
    let depth = depths(parent);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(depth[i]), i));
    for i in order {
        if let Some(p) = parent[i].filter(|&p| p < n && p != i) {
            let (f, t) = out[i];
            out[p].0 += f;
            out[p].1 += t;
        }
    }
    out
}

/// The moment about `at` of a load given about the body origin.
pub fn moment_about(force: DVec3, moment_origin: DVec3, at: DVec3) -> DVec3 {
    moment_origin - at.cross(force)
}

/// The peak line loads in a ring joint's wall (N/m), from the force and
/// moment (about its centre) the parent's side exerts on the child's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Section {
    /// Peak axial tension and compression around the ring (each may be
    /// negative: no tension, or no compression, anywhere).
    pub tension: f64,
    pub compression: f64,
    /// Peak shear flow.
    pub shear: f64,
}

/// The section of a ring of `radius` with `normal` (unit, from the parent
/// towards the child) under `force` and `moment` (about its centre) from
/// the parent's side, with `gauge` pressure (Pa) inside it: see the module.
pub fn section(force: DVec3, moment: DVec3, normal: DVec3, radius: f64, gauge: f64) -> Section {
    if radius <= 0.0 {
        return Section::default();
    }
    let r = radius;
    // The parent pulling the child towards itself (against the normal) is
    // tension; the gas in a pressurised section carries p·πR² in
    // compression, so the wall carries that much more tension.
    let tension = -force.dot(normal) + gauge.max(0.0) * math::PI * r * r;
    let bending = (moment - normal * moment.dot(normal)).length();
    let shear = (force - normal * force.dot(normal)).length();
    let torsion = moment.dot(normal).abs();
    let axial = tension / (math::TAU * r);
    let bend = bending / (math::PI * r * r);
    Section {
        tension: axial + bend,
        compression: -axial + bend,
        shear: shear / (math::PI * r) + torsion / (math::TAU * r * r),
    }
}

/// A ring joint's capacity at temperature (N/m): ultimate tension and
/// compression (buckling) line loads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Capacity {
    pub tension: f64,
    pub compression: f64,
}

impl Capacity {
    /// The capacity of a joint whose wall carries `allowable` (N/m, ultimate
    /// in tension) and buckles at `buckling` (N/m; zero: not given, the
    /// ultimate stands), both at room temperature, with its strength at
    /// `factor` of room temperature's.
    pub fn of(allowable: f64, buckling: f64, factor: f64) -> Capacity {
        let f = factor.clamp(0.0, 1.0);
        let compression = if buckling > 0.0 { buckling.min(allowable) } else { allowable };
        Capacity { tension: allowable * f, compression: compression * f }
    }

    /// Whether there is anything to check against.
    pub fn known(&self) -> bool {
        self.tension > 0.0
    }
}

/// The section's load over its capacity (1 is the ultimate load); zero
/// when the capacity is not known.
pub fn ratio(s: &Section, c: &Capacity) -> f64 {
    if !c.known() {
        return 0.0;
    }
    let shear = c.tension / 3f64.sqrt();
    let comp = if c.compression > 0.0 { s.compression / c.compression } else { 0.0 };
    (s.tension / c.tension).max(comp).max(s.shear / shear).max(0.0)
}

/// The strength factor of a material at temperature `k` (its ultimate
/// strength over room temperature's; zero at its destruction temperature).
pub fn strength_factor(material: &crate::materials::Material, k: f64) -> f64 {
    let room = material.strength_at(crate::catalog::ROOM_K).1;
    if room > 0.0 {
        (material.strength_at(k).1 / room).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// SP-8007's increase of the buckling coefficient with internal pressure,
/// Δγ against x = (p/E)(R/t)²: its Figure 6 curve rises from 0 to about
/// 0.6, read here as 0.6·(1 − e^(−1.5√x)) (E: within ~0.05 of the figure
/// from x 0.1 to 10, generous below; a larger Δγ only delays a buckle).
pub fn pressure_knockdown_gain(x: f64) -> f64 {
    0.6 * (1.0 - math::exp(-1.5 * x.max(0.0).sqrt()))
}

/// The buckling line load (N/m) of a wall that buckles at `n0` unpressurised
/// (with the knockdown γ = [`crate::catalog::KNOCKDOWN`]) under `gauge`
/// pressure in a ring of `radius`: n0·(γ + Δγ)/γ, with x = (p/E)(R/t)²
/// = 0.605·γ·p·R/n0 (the wall's thickness and modulus cancel).
pub fn pressurised_buckling(n0: f64, radius: f64, gauge: f64) -> f64 {
    if n0 <= 0.0 || gauge <= 0.0 {
        return n0;
    }
    let gamma = crate::catalog::KNOCKDOWN;
    let x = 0.605 * gamma * gauge * radius / n0;
    n0 * (gamma + pressure_knockdown_gain(x)) / gamma
}

/// The parts of a vessel as its joints see them this tick (body axes),
/// gathered by the vessel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bodies<'a> {
    /// Each part of the piece in the craft's order: its `Part::id`, its
    /// parent (an index into this list), its dry mass properties with its
    /// components (`Craft::part_mass`) and its temperature (K: the mean of
    /// its skin cells).
    pub ids: &'a [u32],
    pub parents: &'a [Option<u32>],
    pub dry: &'a [MassProps],
    pub kelvin: &'a [f64],
    /// The forces other than gravity on the parts over the tick (indices
    /// into `ids`): engines, thrusters, the air, the ground, the clamps.
    pub pushes: &'a [Push],
    /// Ambient pressure (Pa).
    pub ambient: f64,
    /// Each part's hottest interior node (K), the air's dynamic pressure
    /// (Pa) and Mach number: the failures' environment (`systems::failure`;
    /// a skin cell over its own material's limit is the breakage's).
    pub hottest: &'a [f64],
    pub q: f64,
    pub mach: f64,
}

/// Each engine's thrust (N, aligned with the definition's engines) at its
/// gimbal point on its part, along `dir` (body axes: the engines' realised
/// direction, deflecting together) or else its own; `index` maps a
/// `Part::id` to the parts list's index.
pub fn engine_pushes(
    def: &ShipDef,
    per_engine: &[f64],
    dir: Option<DVec3>,
    index: impl Fn(u32) -> Option<usize>,
) -> Vec<Push> {
    (def.engines.iter().zip(per_engine))
        .filter(|(_, &f)| f > 0.0)
        .filter_map(|(e, &f)| {
            let d = dir.unwrap_or(e.data.dir).normalize_or(DVec3::Z);
            Some(Push { part: index(e.part)?, force: d * f, at: e.data.pos, torque: DVec3::ZERO })
        })
        .collect()
}

/// The air's force on each part (body axes) from the hull's total `force`
/// and `moment` (about the body origin; the aero model gives no per-part
/// forces): shared by each part's exposed area facing the flow (Σ e·A·cos θ
/// over its cells, `exposure` from the bake), at that area's centroid; the
/// rest of the moment shared alike as couples. The totals are exact, the
/// split an approximation. `flow` is the direction the air moves past the
/// vessel; `cell_part` gives each cell's part (an index into the list).
pub fn aero_pushes(
    cell_part: &[u32],
    cells: &[crate::aero::CellGeometry],
    exposure: impl Fn(usize) -> f64,
    flow: DVec3,
    (force, moment): (DVec3, DVec3),
    parts: usize,
) -> Vec<Push> {
    let mut w = vec![0.0; parts];
    let mut centre = vec![DVec3::ZERO; parts];
    for (i, (c, &p)) in cells.iter().zip(cell_part).enumerate() {
        let facing = -c.normal.dot(flow);
        let p = p as usize;
        if facing > 0.0 && p < parts {
            let x = exposure(i) * c.area * facing;
            w[p] += x;
            centre[p] += c.centroid * x;
        }
    }
    let total: f64 = w.iter().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut pushes: Vec<Push> = (0..parts)
        .filter(|&p| w[p] > 0.0)
        .map(|p| Push { part: p, force: force * (w[p] / total), at: centre[p] / w[p], torque: DVec3::ZERO })
        .collect();
    let placed: DVec3 = pushes.iter().map(|q| q.at.cross(q.force)).sum();
    let rest = moment - placed;
    for q in &mut pushes {
        q.torque = rest * (w[q.part] / total);
    }
    pushes
}

/// Each part's temperature (K): the mean of its skin cells' (`cell_part`
/// gives each cell's part; a part without cells takes `default`).
pub fn part_kelvin(cell_part: &[u32], skin: &[f64], parts: usize, default: f64) -> Vec<f64> {
    let mut sum = vec![0.0; parts];
    let mut n = vec![0u32; parts];
    for (&p, &k) in cell_part.iter().zip(skin) {
        if let Some(s) = sum.get_mut(p as usize) {
            *s += k;
            n[p as usize] += 1;
        }
    }
    sum.iter().zip(&n).map(|(&s, &n)| if n > 0 { s / f64::from(n) } else { default }).collect()
}

/// Each part's hottest interior node (K; `node_part` gives each node's
/// part; a part without nodes: `default`).
pub fn part_hottest(node_part: &[u32], nodes: &[f64], parts: usize, default: f64) -> Vec<f64> {
    let mut out = vec![f64::NEG_INFINITY; parts];
    for (&p, &k) in node_part.iter().zip(nodes) {
        if let Some(x) = out.get_mut(p as usize) {
            *x = x.max(k);
        }
    }
    out.iter().map(|&k| if k.is_finite() { k } else { default }).collect()
}

/// Each part's mass properties with its contents now: its dry ones, each
/// of its tanks' liquid (a solid cylinder from the region's base, as the
/// vessel's mass model) and ullage gas, and its bottles' gas; the ground's
/// parts (the pad's mount) weigh nothing here (they are not the vehicle's).
pub fn part_masses(def: &ShipDef, state: &ShipState, b: &Bodies) -> Vec<MassProps> {
    let index = |id: u32| b.ids.iter().position(|&x| x == id);
    let mut items: Vec<Vec<MassProps>> = b.dry.iter().map(|&d| vec![d]).collect();
    for (t, s) in def.tanks.iter().zip(&state.tanks) {
        if let Some(k) = index(t.part).filter(|_| s.liquid > 0.0 && t.capacity > 0.0) {
            let (d, h) = (t.region.axis.normalize(), t.region.axis.length() * (s.liquid / t.capacity).min(1.0));
            let inertia = crate::craft::mass::solid_cylinder(s.liquid, t.region.radius, h, d);
            items[k].push(MassProps { mass: s.liquid, com: t.region.base + d * (0.5 * h), inertia });
        }
    }
    // Each ullage's gas at the top of its liquid's region (as `ShipState::contents`).
    for (t, s) in def.tanks.iter().zip(&state.tanks) {
        let kg = s.ullage_moles * fluids().gas(&t.load.pressurant).map_or(0.0, |g| g.molar_mass);
        if let Some(k) = index(t.part).filter(|_| kg > 0.0) {
            items[k].push(MassProps { mass: kg, com: t.region.base + t.region.axis, inertia: DMat3::ZERO });
        }
    }
    for (bd, s) in def.bottles.iter().zip(&state.bottles) {
        let kg = s.moles * fluids().gas(&bd.gas).map_or(0.0, |g| g.molar_mass);
        if let Some(k) = index(bd.part).filter(|_| kg > 0.0) {
            items[k].push(MassProps { mass: kg, com: bd.pos, inertia: DMat3::ZERO });
        }
    }
    let ground = |id: u32| def.parts.iter().any(|p| p.part == id && p.ground);
    (items.iter().zip(b.ids))
        .map(|(list, &id)| {
            let m = MassProps::sum(list);
            if ground(id) {
                MassProps { mass: 0.0, com: m.com, inertia: DMat3::ZERO }
            } else {
                m
            }
        })
        .collect()
}

/// The gauge pressure (Pa) inside a joint's ring: a tank compartment of
/// either part that the joint's plane cuts holds its ullage pressure plus
/// the head of its liquid above the cut under `accel` (body axes, proper),
/// less `ambient`; zero if none does (an unpressurised skirt or interstage).
/// Until the plumbing has regulated the pressures (it sets
/// `Runtime::pressure_regulated`) the ullage holds at least its loading
/// (flight) pressure.
pub fn section_gauge(def: &ShipDef, state: &ShipState, j: &JointDef, accel: DVec3, ambient: f64) -> f64 {
    let mut gauge: f64 = 0.0;
    for (t, s) in def.tanks.iter().zip(&state.tanks) {
        if !(t.part == j.parent || t.part == j.child) {
            continue;
        }
        // The compartment (its ullage too) as a cylinder of its volume from
        // the outlet (`plumbing::geometry`, the one owner).
        let g = plumbing::geometry(t, s);
        let x = (j.pos - t.region.base).dot(g.axis);
        if !(x > 0.0 && x < g.length) {
            continue;
        }
        let head = g.density * accel.dot(g.axis).max(0.0) * (g.height - x).max(0.0);
        // Unregulated (no plumbing step yet): at least the flight pressure
        // the tank is loaded at and its regulator holds.
        let ullage = plumbing::pressure(t, s);
        let ullage = if state.runtime.pressure_regulated { ullage } else { ullage.max(t.load.pressure) };
        gauge = gauge.max(ullage + head - ambient);
    }
    gauge
}

/// The temperature factor of a joint: the weaker of its two parts'
/// materials at their temperatures (1 when a material is unknown).
fn joint_factor(def: &ShipDef, j: &JointDef, kelvin: impl Fn(u32) -> Option<f64>) -> f64 {
    let side = |id: u32| -> f64 {
        let Some(p) = def.parts.iter().find(|p| p.part == id) else { return 1.0 };
        match (crate::materials::materials().get(&p.material), kelvin(id)) {
            (Ok(m), Some(k)) => strength_factor(m, k),
            _ => 1.0,
        }
    };
    side(j.parent).min(side(j.child))
}

/// The rigid body's motion its forces give (body axes): a = ΣF/M and
/// α = I⁻¹(τ − ω×Iω) about the centre of mass, at angular velocity `omega`.
/// The joints are loaded with it, so the whole tree lacks nothing: what a
/// joint carries is the same seen from either side (a tick's integrated
/// motion differs from it by what the gathered forces leave out, such as the
/// thrusters' couples).
pub fn consistent(total: &MassProps, pushes: &[Push], omega: DVec3) -> Motion {
    let force: DVec3 = pushes.iter().map(|p| p.force).sum();
    let torque: DVec3 = pushes.iter().map(|p| (p.at - total.com).cross(p.force) + p.torque).sum();
    let i = total.inertia;
    let alpha = if i.determinant().abs() > 0.0 { i.inverse() * (torque - omega.cross(i * omega)) } else { DVec3::ZERO };
    Motion { com: total.com, accel: force / total.mass, omega, alpha }
}

/// One tick of the joints (see the module), the motion the forces give
/// ([`consistent`]; `m` gives the angular velocity): each intact joint's force, moment and ratio
/// into the ship state's runtime (the pad's hold-down excepted: the held
/// ticks fill it); each part's peak ratio into its history; a joint at or
/// over its ultimate load snaps (not in debug mode), logged, and returned
/// as the effect the vessel acts on.
pub fn step(def: &ShipDef, state: &mut ShipState, b: &Bodies, m: &Motion, at: Epoch, debug: bool) -> Vec<Effect> {
    if def.joints.is_empty() || b.ids.is_empty() {
        return Vec::new();
    }
    let mass = part_masses(def, state, b);
    let total = MassProps::sum(&mass);
    if total.mass <= 0.0 {
        return Vec::new();
    }
    let motion = consistent(&total, b.pushes, m.omega);
    let parents: Vec<Option<usize>> = b.parents.iter().map(|p| p.map(|p| p as usize)).collect();
    let loads = subtree_loads(&parents, &mass, b.pushes, &motion);
    let index = |id: u32| b.ids.iter().position(|&x| x == id);
    let ground = |id: u32| def.parts.iter().any(|p| p.part == id && p.ground);
    let mut effects = Vec::new();
    for (j, d) in def.joints.iter().enumerate() {
        let (Some(c), Some(_)) = (index(d.child), index(d.parent)) else { continue };
        if d.kind == JointKind::HoldDown || ground(d.child) || ground(d.parent) || !state.joints[j].intact {
            continue;
        }
        let (force, about_origin) = loads[c];
        let moment = moment_about(force, about_origin, d.pos);
        let gauge = section_gauge(def, state, d, motion.accel, b.ambient);
        let sec = section(force, moment, d.normal, d.radius, gauge);
        let factor = joint_factor(def, d, |id| index(id).and_then(|k| b.kelvin.get(k).copied()));
        let buckling = if d.buckling > 0.0 { pressurised_buckling(d.buckling, d.radius, gauge) } else { 0.0 };
        let cap = Capacity::of(d.allowable, buckling, factor);
        let r = ratio(&sec, &cap);
        if let Some(run) = state.runtime.joints.get_mut(j) {
            (run.force, run.moment, run.ratio) = (force, moment, r);
        }
        if let Some(p) = state.parts.iter_mut().find(|p| p.part == d.child) {
            p.history.peak_load = p.history.peak_load.max(r);
        }
        if r >= 1.0 && !debug {
            state.joints[j].intact = false;
            let kn = force.length() / 1e3;
            let what = if sec.compression >= sec.tension { "buckled" } else { "snapped" };
            state.runtime.event(
                at,
                format!("{}: {what} at {:.0} % of its ultimate load ({kn:.0} kN)", d.name, r * 100.0),
                true,
            );
            effects.push(Effect::Snap(d.child, force.length()));
        }
    }
    effects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(mass: f64, at: DVec3) -> MassProps {
        MassProps { mass, com: at, inertia: DMat3::ZERO }
    }

    /// A stack of three point masses along Z under thrust at the bottom:
    /// each joint carries what accelerates the masses above it (compression).
    #[test]
    fn a_stack_under_thrust() {
        // Root at the top (as the Falcon 9's capsule): 0 at z 20 (2 t), 1 at
        // z 10 (5 t) under it, 2 at z 0 (10 t) under that with the engine.
        let parent = [None, Some(0), Some(1)];
        let mass = [point(2_000.0, DVec3::Z * 20.0), point(5_000.0, DVec3::Z * 10.0), point(10_000.0, DVec3::ZERO)];
        let thrust = 340_000.0;
        let a = thrust / 17_000.0;
        let com = (DVec3::Z * 20.0 * 2_000.0 + DVec3::Z * 10.0 * 5_000.0) / 17_000.0;
        let m = Motion { com, accel: DVec3::Z * a, ..Motion::default() };
        let push = Push { part: 2, force: DVec3::Z * thrust, at: DVec3::ZERO, torque: DVec3::ZERO };
        let loads = subtree_loads(&parent, &mass, &[push], &m);
        // The root lacks nothing: the motion is the thrust's.
        assert!(loads[0].0.length() < 1e-9 * thrust && loads[0].1.length() < 1e-6 * thrust);
        // Joint 0|1: the parent pushes the child's side down by the top's
        // inertia, 2 t × a; joint 1|2: 7 t × a.
        assert!((loads[1].0 - DVec3::Z * (-2_000.0 * a)).length() < 1e-6, "{:?}", loads[1].0);
        assert!((loads[2].0 - DVec3::Z * (-7_000.0 * a)).length() < 1e-6, "{:?}", loads[2].0);
        // On the axis: no moment about the joints.
        for (i, at) in [(1, DVec3::Z * 15.0), (2, DVec3::Z * 5.0)] {
            assert!(moment_about(loads[i].0, loads[i].1, at).length() < 1e-6);
        }
        // Compression in a 1 m ring at joint 1|2 (normal −Z, parent above).
        let s = section(loads[2].0, DVec3::ZERO, -DVec3::Z, 1.0, 0.0);
        let n = 7_000.0 * a / math::TAU;
        assert!((s.compression - n).abs() < 1e-6 && (s.tension + n).abs() < 1e-6 && s.shear < 1e-9, "{s:?}");
        // 1 bar inside the ring relieves π·10⁵ N of it.
        let p = section(loads[2].0, DVec3::ZERO, -DVec3::Z, 1.0, 1e5);
        assert!((p.compression - (7_000.0 * a - 1e5 * math::PI) / math::TAU).abs() < 1e-6, "{p:?}");
    }

    /// A side load on the top of a cantilevered stack: shear and a bending
    /// moment growing with the lever, as by hand.
    #[test]
    fn a_side_load_bends_the_joint() {
        // Root at the bottom (held, as a vehicle on its clamps): 0 at z 0,
        // 1 (1 t) at z 10. A 10 kN side force at z 15 on part 1, the body at
        // rest: the joint at z 5 carries −10 kN of shear and a moment of
        // 10 kN × 10 m.
        let parent = [None, Some(0)];
        let mass = [point(5_000.0, DVec3::ZERO), point(1_000.0, DVec3::Z * 10.0)];
        let push = Push { part: 1, force: DVec3::X * 1e4, at: DVec3::Z * 15.0, torque: DVec3::ZERO };
        let loads = subtree_loads(&parent, &mass, &[push], &Motion::default());
        let at = DVec3::Z * 5.0;
        let moment = moment_about(loads[1].0, loads[1].1, at);
        assert!((loads[1].0 - DVec3::X * -1e4).length() < 1e-9, "{:?}", loads[1].0);
        // The joint holds the top against the force's moment about it: −(10 m × 10 kN) about Y.
        assert!((moment - DVec3::Y * -1e5).length() < 1e-6, "{moment:?}");
        // A 2 m ring: shear flow V/(πR), bending M/(πR²) both ways.
        let s = section(loads[1].0, moment, DVec3::Z, 2.0, 0.0);
        let bend = 1e5 / (math::PI * 4.0);
        assert!((s.tension - bend).abs() < 1e-6 && (s.compression - bend).abs() < 1e-6, "{s:?}");
        assert!((s.shear - 1e4 / (math::PI * 2.0)).abs() < 1e-9, "{s:?}");
    }

    /// Rotation: a part spun about the root needs the centripetal force
    /// m·ω²·r towards the axis, and angular acceleration its tangential force
    /// and its own I·α.
    #[test]
    fn spin_and_angular_acceleration() {
        let parent = [None, Some(0)];
        let inertia = DMat3::from_diagonal(DVec3::new(10.0, 20.0, 30.0));
        let mass = [point(1e6, DVec3::ZERO), MassProps { mass: 100.0, com: DVec3::X * 4.0, inertia }];
        let m = Motion { omega: DVec3::Z * 0.5, alpha: DVec3::Z * 2.0, ..Motion::default() };
        let loads = subtree_loads(&parent, &mass, &[], &m);
        // a = α×r + ω×(ω×r) = (0, 8, 0) + (−1, 0, 0) at r = 4 X.
        assert!((loads[1].0 - DVec3::new(-100.0, 800.0, 0.0)).length() < 1e-9, "{:?}", loads[1].0);
        // About the origin: r × F + I·α (ω ∥ a principal axis: no gyroscopic term).
        let expect = DVec3::X * 4.0 * 1.0 * 0.0 + DVec3::new(0.0, 0.0, 4.0 * 800.0) + DVec3::Z * 60.0;
        assert!((loads[1].1 - expect).length() < 1e-9, "{:?}", loads[1].1);
    }

    /// The order of the parts does not matter, and a deep tree sums its
    /// subtrees exactly.
    #[test]
    fn subtrees_in_any_order() {
        // 0 ← 1 ← 2, 0 ← 3 ← 4, listed children first.
        let parent = [Some(1), Some(4), None, Some(4), Some(2)];
        let mass: Vec<MassProps> = (0..5).map(|i| point(10.0 * (i + 1) as f64, DVec3::X * i as f64)).collect();
        let m = Motion { accel: DVec3::Y * 3.0, ..Motion::default() };
        let loads = subtree_loads(&parent, &mass, &[], &m);
        let f = |list: &[usize]| list.iter().map(|&i| mass[i].mass * 3.0).sum::<f64>();
        assert_eq!(loads[0].0, DVec3::Y * f(&[0]));
        assert_eq!(loads[1].0, DVec3::Y * f(&[0, 1]));
        assert_eq!(loads[3].0, DVec3::Y * f(&[3]));
        assert_eq!(loads[4].0, DVec3::Y * f(&[0, 1, 3, 4]));
        assert_eq!(loads[2].0, DVec3::Y * f(&[0, 1, 2, 3, 4]));
    }

    #[test]
    fn capacity_and_ratio_table() {
        // (section, capacity, ratio): tension, compression against buckling,
        // shear against the ultimate over √3, unknown capacity.
        let s = |t: f64, c: f64, v: f64| Section { tension: t, compression: c, shear: v };
        let cases = [
            (s(5e5, -5e5, 0.0), Capacity::of(1e6, 2e5, 1.0), 0.5),
            (s(-1e5, 1e5, 0.0), Capacity::of(1e6, 2e5, 1.0), 0.5),
            (s(-1e5, 1e5, 0.0), Capacity::of(1e6, 0.0, 1.0), 0.1),
            (s(0.0, 0.0, 1e6 / 3f64.sqrt()), Capacity::of(1e6, 0.0, 1.0), 1.0),
            (s(5e5, -5e5, 0.0), Capacity::of(1e6, 0.0, 0.5), 1.0),
            (s(5e5, 1e5, 1.0), Capacity::of(0.0, 0.0, 1.0), 0.0),
        ];
        for (sec, cap, expect) in cases {
            assert!((ratio(&sec, &cap) - expect).abs() < 1e-12, "{sec:?} {cap:?}: {}", ratio(&sec, &cap));
        }
    }
}

#[cfg(test)]
#[path = "structure_tests.rs"]
mod falcon9_tests;
