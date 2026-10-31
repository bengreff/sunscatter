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
//!   A point attachment (no ring), a part on a mechanism (it hangs by its
//!   hinge and actuator or strut: a leg's A-frame, a grid fin; its feet's
//!   overload is the contact's, D066) or a joint whose wall has no
//!   allowable has no capacity data: its load is shown, its ratio is zero
//!   and it never breaks ("if the simulator is not sure, don't trigger the
//!   failure").
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

/// Whether the craft gives a joint a capacity to check its load against:
/// a ring with an allowable line load, not a part hung by a mechanism's
/// hinge and actuator (a leg's A-frame and strut, a grid fin's hinge). The
/// one owner of the rule: the load's ratio is zero without it, and the
/// interface shows a dash (D106).
pub fn rated(def: &ShipDef, d: &JointDef) -> bool {
    d.allowable > 0.0 && !def.mechanisms.iter().any(|m| m.part == d.child)
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
/// forces), split along the body's `axis` (its slender axis):
/// * **axial**: the forebody's pressure on each part's upstream cells
///   (Newtonian, q·Cp,max·e·A·cos²θ, at most the whole axial force) and the
///   rest (skin friction, base suction) by wetted area — so a stack's drag
///   is not all on its nose;
/// * **normal**: by each part's exposed area facing the crossflow
///   (e·A·cos θ to the flow's normal part), shifted together along the axis
///   so the normal forces' moment is the model's;
/// * whatever moment is left (roll, small) as couples shared by exposure.
///
/// The totals are the model's exactly; the split is an approximation.
/// `flow` is the direction the air moves past the vessel, `q` its dynamic
/// pressure and `cp_max` its stagnation pressure coefficient; `cell_part`
/// gives each cell's part (an index into the list).
#[allow(clippy::too_many_arguments)]
pub fn aero_pushes(
    cell_part: &[u32],
    cells: &[crate::aero::CellGeometry],
    exposure: impl Fn(usize) -> f64,
    (flow, q, cp_max): (DVec3, f64, f64),
    axis: DVec3,
    (force, moment): (DVec3, DVec3),
    parts: usize,
) -> Vec<Push> {
    let a = axis.normalize_or(DVec3::Z);
    let axial = a * force.dot(a);
    let normal = force - axial;
    let cross = flow - a * flow.dot(a);
    let cross = if cross.length() > 1e-9 { cross.normalize() } else { DVec3::ZERO };
    // Per part: forebody pressure (axial), wetted area, crossflow area.
    let (mut fore, mut wet, mut side) = (vec![0.0; parts], vec![0.0; parts], vec![0.0; parts]);
    let (mut c_wet, mut c_side, mut c_fore) =
        (vec![DVec3::ZERO; parts], vec![DVec3::ZERO; parts], vec![DVec3::ZERO; parts]);
    for (i, (c, &p)) in cells.iter().zip(cell_part).enumerate() {
        let p = p as usize;
        if p >= parts {
            continue;
        }
        wet[p] += c.area;
        c_wet[p] += c.centroid * c.area;
        let facing = -c.normal.dot(flow);
        if facing > 0.0 {
            let x = q * cp_max * exposure(i) * c.area * facing * facing * (-c.normal).dot(a);
            fore[p] += x;
            c_fore[p] += c.centroid * x.abs();
        }
        let crossing = -c.normal.dot(cross);
        if crossing > 0.0 {
            let x = exposure(i).max(0.0) * c.area * crossing;
            side[p] += x;
            c_side[p] += c.centroid * x;
        }
    }
    let total_axial = force.dot(a);
    let fore_total: f64 = fore.iter().sum();
    // The forebody takes its Newtonian share, never more than the whole.
    let scale = if fore_total.abs() > 1e-9 && total_axial * fore_total > 0.0 {
        (total_axial / fore_total).min(1.0)
    } else {
        0.0
    };
    let rest = total_axial - fore_total * scale;
    let wet_total: f64 = wet.iter().sum();
    let side_total: f64 = side.iter().sum();
    let mut pushes = Vec::new();
    for p in 0..parts {
        let f_fore = fore[p] * scale;
        let f_rest = if wet_total > 0.0 { rest * wet[p] / wet_total } else { 0.0 };
        let at = if fore[p] != 0.0 && f_fore.abs() > f_rest.abs() {
            c_fore[p] / fore[p].abs()
        } else if wet[p] > 0.0 {
            c_wet[p] / wet[p]
        } else {
            continue;
        };
        if f_fore != 0.0 {
            let at_fore = if fore[p] != 0.0 { c_fore[p] / fore[p].abs() } else { at };
            pushes.push(Push { part: p, force: a * f_fore, at: at_fore, torque: DVec3::ZERO });
        }
        if f_rest != 0.0 && wet[p] > 0.0 {
            pushes.push(Push { part: p, force: a * f_rest, at: c_wet[p] / wet[p], torque: DVec3::ZERO });
        }
        if side_total > 0.0 && side[p] > 0.0 {
            let n = normal * (side[p] / side_total);
            pushes.push(Push { part: p, force: n, at: c_side[p] / side[p], torque: DVec3::ZERO });
        }
    }
    if side_total <= 0.0 && normal.length() > 0.0 && wet_total > 0.0 {
        // No crossflow area (pure axial flow): the normal force by wetted area.
        for p in (0..parts).filter(|&p| wet[p] > 0.0) {
            pushes.push(Push {
                part: p,
                force: normal * (wet[p] / wet_total),
                at: c_wet[p] / wet[p],
                torque: DVec3::ZERO,
            });
        }
    }
    // Shift the normal forces along the axis so their moment is the model's.
    let placed: DVec3 = pushes.iter().map(|x| x.at.cross(x.force)).sum();
    let lever = a.cross(normal);
    if lever.length_squared() > 1e-18 {
        let shift = lever.dot(moment - placed) / lever.length_squared();
        for x in pushes.iter_mut().filter(|x| x.force.cross(a).length() > 0.0) {
            x.at += a * shift;
        }
    }
    let placed: DVec3 = pushes.iter().map(|x| x.at.cross(x.force)).sum();
    let left = moment - placed;
    let weight: f64 = wet_total;
    if weight > 0.0 {
        for p in (0..parts).filter(|&p| wet[p] > 0.0) {
            pushes.push(Push { part: p, force: DVec3::ZERO, at: c_wet[p] / wet[p], torque: left * (wet[p] / weight) });
        }
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
/// `ShipState::pressure_regulated`) the ullage holds at least its loading
/// (flight) pressure.
pub fn section_gauge(def: &ShipDef, state: &ShipState, j: &JointDef, accel: DVec3, ambient: f64) -> f64 {
    gauge_at(&gauge_terms(def, state, j, accel, ambient), 1.0)
}

/// [`section_gauge`] as the acceleration scales: per compartment the joint
/// cuts, its ullage less `ambient` and its head under `accel` (Pa); at
/// `k`·`accel` the gauge is [`gauge_at`]`(terms, k)`.
pub fn gauge_terms(def: &ShipDef, state: &ShipState, j: &JointDef, accel: DVec3, ambient: f64) -> Vec<(f64, f64)> {
    let mut terms = Vec::new();
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
        let ullage = if state.pressure_regulated { ullage } else { ullage.max(t.load.pressure) };
        terms.push((ullage - ambient, head));
    }
    terms
}

/// The gauge of [`gauge_terms`] at `k` times their acceleration: the
/// largest compartment's, never below zero.
pub fn gauge_at(terms: &[(f64, f64)], k: f64) -> f64 {
    terms.iter().map(|&(c, h)| c + h * k).fold(0.0, f64::max)
}

/// The proper acceleration's own scale the structure's limit is sought to
/// (m/s²: 100 g). No craft holds more; a limit beyond it is not shown.
pub const LIMIT_SEARCH: f64 = 100.0 * crate::vessel::G0;

/// The least scale `k` in (0, `max`] of a joint's load as it stands at
/// which `ratio_at(k)` (its load over its ultimate at `k` times the load)
/// reaches 1: doubling from 1/1024 to bracket it, then halving the bracket
/// to a part in 10⁴ (already there at 1/1024: that). `None` if it is not
/// reached by `max` (a joint without capacity data never is).
pub fn ultimate_scale(ratio_at: impl Fn(f64) -> f64, max: f64) -> Option<f64> {
    let mut k = 1.0 / 1024.0;
    if ratio_at(k) >= 1.0 {
        return Some(k);
    }
    let mut lo = k;
    loop {
        k *= 2.0;
        if k > max {
            return None;
        }
        if ratio_at(k) >= 1.0 {
            break;
        }
        lo = k;
    }
    let mut hi = k;
    while hi - lo > 1e-4 * hi {
        let mid = 0.5 * (lo + hi);
        if ratio_at(mid) >= 1.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
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
/// into the ship state (the pad's hold-down excepted: the held
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
        let terms = gauge_terms(def, state, d, motion.accel, b.ambient);
        let factor = joint_factor(def, d, |id| index(id).and_then(|k| b.kelvin.get(k).copied()));
        let rated = rated(def, d);
        // The section and its ratio with the load as it stands scaled by k
        // (the liquids' heads with it): k = 1 now.
        let scaled = |k: f64| {
            let gauge = gauge_at(&terms, k);
            let sec = section(force * k, moment * k, d.normal, d.radius, gauge);
            let buckling = if d.buckling > 0.0 { pressurised_buckling(d.buckling, d.radius, gauge) } else { 0.0 };
            let cap = if rated { Capacity::of(d.allowable, buckling, factor) } else { Capacity::default() };
            (sec, ratio(&sec, &cap))
        };
        let (sec, r) = scaled(1.0);
        let a = motion.accel.length();
        let to_ultimate = (a > 1e-3).then(|| ultimate_scale(|k| scaled(k).1, LIMIT_SEARCH / a)).flatten();
        if let Some(run) = state.joints.get_mut(j) {
            (run.force, run.moment, run.ratio, run.to_ultimate) = (force, moment, r, to_ultimate);
        }
        if let Some(p) = state.parts.iter_mut().find(|p| p.part == d.child) {
            p.history.peak_load = p.history.peak_load.max(r);
        }
        if r >= 1.0 && !debug {
            state.joints[j].intact = false;
            let kn = force.length() / 1e3;
            let what = if sec.compression >= sec.tension { "buckled" } else { "snapped" };
            state.event(
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
#[path = "structure_hand_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "structure_tests.rs"]
mod falcon9_tests;
