//! A ship state split at a separation and merged at a docking (F1 of the
//! fidelity review, D117): every field has its rule, named here one by one.
//! The destructuring is exhaustive, so a new field has no rule until it is
//! given one (the compiler refuses it), and the coverage and round-trip
//! tests (`state_tests`) check the rules.
//!
//! * **An item of a part** (a tank, bottle, engine, thruster, valve,
//!   battery, mechanism, part, component's switch, fate) goes with its part,
//!   in order. The lists stay aligned by index with their [`ShipDef`]'s:
//!   the systems step them in lockstep with the definition, and
//!   [`ShipDef::subset`] keeps the same order, so the merge puts each entry
//!   back where the whole definition has its part.
//! * **A joint** goes with its two parts; a joint the split cuts goes with
//!   its child's piece (the joint whose child part is this, as
//!   `Effect::Separate` names it), held in `severed` until a merge joins
//!   its parts again, with its load at the cut.
//! * **The cabin** goes with its part; the crew's exposure with it
//!   (`Runtime::partition`).
//! * **The vessel's own** (by name): the clamps to the piece keeping a
//!   hold-down joint; the program (the stages, the functions and the
//!   sequence's position) whole to every piece, a copy each: it is the
//!   vessel's as a whole, and each piece reads it through its view
//!   (`craft::program`: what each stage does in that piece), so a merge
//!   gives it back.

use super::assembly::JointKind;
use super::command::Stage;
use super::ship::ShipDef;
use super::state::{JointState, ShipState};

/// The items of `items` whose part (`part`) is kept, in order.
pub(crate) fn by<T: Clone>(items: &[T], part: impl Fn(&T) -> u32, kept: &[u32]) -> Vec<T> {
    items.iter().filter(|x| kept.contains(&part(x))).cloned().collect()
}

/// A list aligned with the whole definition's items (`parts`: each item's
/// part, in order), from the two pieces' lists aligned with theirs: each
/// entry taken in turn from the piece holding its part (`in_a`). Every
/// entry of both is used once, or it is an error.
pub(crate) fn zip_merge<T: Clone>(
    parts: impl Iterator<Item = u32>,
    in_a: &dyn Fn(u32) -> bool,
    a: &[T],
    b: &[T],
    what: &str,
) -> Result<Vec<T>, String> {
    let (mut a, mut b) = (a.iter(), b.iter());
    let mut out = Vec::new();
    for p in parts {
        let from = if in_a(p) { &mut a } else { &mut b };
        out.push(from.next().ok_or_else(|| format!("{what}: a piece lacks part {p}'s entry"))?.clone());
    }
    if a.next().is_some() || b.next().is_some() {
        return Err(format!("{what}: a piece has entries its definition lacks"));
    }
    Ok(out)
}

/// The pieces' entries of a list kept by owner (looked up, not aligned):
/// both pieces' entries in part order (a keyed list's order carries no
/// state; a split keeps it, so a list in part order comes back as it was).
pub(crate) fn keyed<T: Clone>(a: &[T], b: &[T], part: impl Fn(&T) -> u32) -> Vec<T> {
    let mut out: Vec<T> = a.iter().chain(b).cloned().collect();
    out.sort_by_key(|x| part(x));
    out
}

/// Whether a definition keeps the pad's clamps.
fn holds(def: &ShipDef) -> bool {
    def.joints.iter().any(|j| j.kind == JointKind::HoldDown)
}

impl ShipState {
    /// The state of the piece holding the parts `kept` (`Part::id`s, its
    /// definition `piece`, the whole ship's `whole`; `own`: the piece the
    /// vessel keeps): see the module for each field's rule. The program
    /// (stages, functions, the sequence's position) is copied whole: the
    /// piece's view of it (`craft::program`) shows what it does there.
    pub fn partition(&self, kept: &[u32], whole: &ShipDef, piece: &ShipDef, own: bool) -> ShipState {
        let ShipState {
            tanks,
            bottles,
            engines,
            thrusters,
            valves,
            batteries,
            mechanisms,
            joints,
            severed,
            parts,
            power,
            cabin,
            fates,
            clamped,
            sequence,
            stages,
            functions,
            runtime,
        } = self;
        let within = |j: &JointState| kept.contains(&j.parent) && kept.contains(&j.child);
        let cut = |j: &JointState| !kept.contains(&j.parent) && kept.contains(&j.child);
        ShipState {
            tanks: by(tanks, |x| x.part, kept),
            bottles: by(bottles, |x| x.part, kept),
            engines: by(engines, |x| x.part, kept),
            thrusters: by(thrusters, |x| x.part, kept),
            valves: by(valves, |x| x.part, kept),
            batteries: by(batteries, |x| x.part, kept),
            mechanisms: by(mechanisms, |x| x.part, kept),
            joints: joints.iter().filter(|j| within(j)).cloned().collect(),
            severed: (severed.iter().filter(|j| kept.contains(&j.child)))
                .chain(joints.iter().filter(|j| cut(j)))
                .cloned()
                .collect(),
            parts: by(parts, |x| x.part, kept),
            power: by(power, |x| x.part, kept),
            cabin: cabin.clone().filter(|c| kept.contains(&c.part)),
            fates: by(fates, |x| x.part, kept),
            clamped: *clamped && holds(piece),
            // The vessel's as a whole: a copy in every piece.
            sequence: *sequence,
            stages: stages.clone(),
            functions: functions.clone(),
            runtime: runtime.partition(kept, whole, piece, own),
        }
    }

    /// The inverse of [`Self::partition`], for a docking: the state of the
    /// whole (`whole`, whose parts are those of `a_def` and `b_def`, with
    /// unique ids) from the pieces' states, `a` the vessel that keeps its
    /// identity. A joint of the whole neither piece has starts as loaded
    /// (intact, unloaded: a new docking joint). The program: the stages
    /// are `a`'s, then `b`'s whose names `a` lacks, the functions likewise,
    /// and the sequence's position is `a`'s; the pieces of a split keep the
    /// same program (each a copy of the whole's), so their merge gives it
    /// back. Two cabins are refused (one cabin per state).
    pub fn merge(
        a: &ShipState,
        a_def: &ShipDef,
        b: &ShipState,
        b_def: &ShipDef,
        whole: &ShipDef,
    ) -> Result<ShipState, String> {
        let a_parts: Vec<u32> = a_def.parts.iter().map(|p| p.part).collect();
        let in_a = |p: u32| a_parts.contains(&p);
        let ShipState {
            tanks,
            bottles,
            engines,
            thrusters,
            valves,
            batteries,
            mechanisms,
            joints: _,
            severed: _,
            parts,
            power,
            cabin,
            fates,
            clamped,
            sequence,
            stages,
            functions,
            runtime,
        } = a;
        let w = whole;
        let mut severed: Vec<JointState> = a.severed.iter().chain(&b.severed).cloned().collect();
        let (mut ja, mut jb) = (a.joints.iter(), b.joints.iter());
        let mut joints = Vec::new();
        for d in &w.joints {
            let j = match (in_a(d.parent), in_a(d.child)) {
                (true, true) => ja.next().cloned(),
                (false, false) => jb.next().cloned(),
                _ => None,
            };
            let j = j.or_else(|| {
                let i = severed.iter().position(|s| s.parent == d.parent && s.child == d.child)?;
                Some(severed.remove(i))
            });
            joints.push(j.unwrap_or(JointState {
                parent: d.parent,
                child: d.child,
                intact: true,
                ..JointState::default()
            }));
        }
        let union = |x: &[Stage], y: &[Stage]| -> Vec<Stage> {
            x.iter().chain(y.iter().filter(|s| !x.iter().any(|t| t.name == s.name))).cloned().collect()
        };
        if cabin.is_some() && b.cabin.is_some() {
            return Err("both pieces have a cabin (one cabin per ship state)".into());
        }
        Ok(ShipState {
            tanks: zip_merge(w.tanks.iter().map(|x| x.part), &in_a, tanks, &b.tanks, "tanks")?,
            bottles: zip_merge(w.bottles.iter().map(|x| x.part), &in_a, bottles, &b.bottles, "bottles")?,
            engines: zip_merge(w.engines.iter().map(|x| x.part), &in_a, engines, &b.engines, "engines")?,
            thrusters: zip_merge(w.thrusters.iter().map(|x| x.part), &in_a, thrusters, &b.thrusters, "thrusters")?,
            valves: zip_merge(w.valves.iter().map(|x| x.part), &in_a, valves, &b.valves, "valves")?,
            batteries: zip_merge(w.batteries.iter().map(|x| x.part), &in_a, batteries, &b.batteries, "batteries")?,
            mechanisms: zip_merge(w.mechanisms.iter().map(|x| x.part), &in_a, mechanisms, &b.mechanisms, "mechanisms")?,
            joints,
            severed,
            parts: zip_merge(w.parts.iter().map(|x| x.part), &in_a, parts, &b.parts, "parts")?,
            power: zip_merge(w.components.iter().map(|x| x.part), &in_a, power, &b.power, "power")?,
            cabin: cabin.clone().or_else(|| b.cabin.clone()),
            fates: keyed(fates, &b.fates, |f| f.part),
            clamped: *clamped || b.clamped,
            sequence: *sequence,
            stages: union(stages, &b.stages),
            functions: union(functions, &b.functions),
            runtime: crate::systems::Runtime::merge(runtime, a_def, &b.runtime, b_def, whole)?,
        })
    }
}
