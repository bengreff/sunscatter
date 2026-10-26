//! The 10 km bubble in flight (D082): craft near a controllable craft in
//! live physics fly live with it, tick for tick, and collide. The rule is
//! `crate::proximity::rule`; the contact forces `crate::proximity::contact`.
//!
//! [`advance`] runs before the fleet's own advance each frame. With no
//! leader and no member it does nothing (coasting craft near each other
//! stay on rails). Otherwise:
//!
//! * **The lattice** is the tick lattice of the reference (the live leader
//!   or member with the lowest vessel id): epochs `t₀ + k·TICK` from its
//!   time, computed as each vessel computes its own (`add_seconds(TICK)`),
//!   so members' times agree bit for bit.
//! * **Candidates** are the flying craft that could come within the rule's
//!   radius of a leader or member during this frame or the next (their
//!   distance less their relative speed over the frame plus a second, and
//!   5 g); the others are left to the fleet. At every epoch each candidate
//!   is brought to it (a coast evaluated exactly there; a live craft on
//!   another lattice ticked up to it), the rule is evaluated, and joins
//!   and leaves happen there: a coast goes live at the epoch, a live craft
//!   on another lattice flies one shorter tick onto it, a member that
//!   leaves and would coast returns to a coast at the epoch. A candidate
//!   already past the epoch (the fleet moved it ahead last frame) cannot
//!   join before the next epoch; it was evaluated at this one then.
//! * **The shared frame**: members are re-anchored to the reference's
//!   anchor when they join (an exact change of coordinates), so the
//!   relative position of two members is the difference of two
//!   anchor-relative positions (their lowest common ancestor, rule 3;
//!   ~1 nm in low orbit, 0.03 mm at 1 AU from the Sun), and contact is
//!   computed relative to the first member's centre of mass.
//! * **Ticks**: a member whose bounding sphere is far from every other
//!   member's flies its tick alone (exactly as outside a bubble); members
//!   that may touch within the tick fly it together: each one's tick is
//!   started alone, then [`contact::SUBSTEPS`] substeps where the craft
//!   contact forces of every pair at the substep's start are applied as
//!   impulses (equal and opposite), with ground contact, gravity, thrust
//!   and air as in a contact tick, then each tick ends alone.
//! * **Impacts**: a touching point closing faster than a craft's impact
//!   limit destroys the craft (one part) or the part that was hit (D066,
//!   per point; not in debug mode, D064).
//!
//! Everything is on the tick lattice and every sum is in a fixed order
//! (members by index, pairs by index), so frames change nothing: chunked
//! equals single pass once the bubble's members are on its lattice.

use super::live::{Substeps, TickStart};
use super::{attitude, Controls, Destruction, Phase, Vessel, TICK};
use crate::contact;
use crate::frame::NodeId;
use crate::proximity::{self, Body, BubbleState, Candidate, Hull, Near, BUBBLE};
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;
use std::sync::Arc;

/// Members whose bounding spheres come this close (m), beyond their
/// relative motion over a tick, fly that tick's substeps together.
pub const CONTACT_MARGIN: f64 = 0.5;
/// The most acceleration a craft is assumed to have when choosing a
/// frame's candidates (m/s², 5 g).
const MAX_ACCEL: f64 = 50.0;
/// Extra reach of the candidate search (m), and the next frame's span it
/// covers (s).
const SEARCH_SLACK: f64 = 1_000.0;
const NEXT_FRAME: f64 = 1.0;
/// Budget of a candidate's coast between epochs (work units): enough to
/// reach the epoch.
const COAST_BUDGET: usize = usize::MAX / 4;

impl Vessel {
    /// The vessel's bubble state (D082).
    pub fn bubble_state(&self) -> BubbleState {
        self.bubble
    }

    /// Whether a bubble holds the vessel live (D082).
    pub fn in_bubble(&self) -> bool {
        self.bubble.member
    }

    /// Whether the vessel leads a bubble with `controls`: controllable
    /// (D083) and live for its own reasons (it would not coast now).
    pub fn leads(&self, world: &World, controls: &Controls) -> bool {
        matches!(self.phase, Phase::Powered { .. }) && self.controllable() && !self.would_coast(world, controls)
    }

    /// Hit by another craft at `speed` (m/s) on `part`: past the craft's
    /// impact limit, the craft (one part) or the part is destroyed.
    fn collided(&mut self, part: u32, speed: f64) {
        if self.debug || self.destroyed.is_some() || speed <= self.craft.impact_max_speed {
            return;
        }
        let cause = Destruction::Impact { speed, point: u32::MAX };
        if self.craft.design().parts.len() <= 1 {
            self.destroy(cause);
            return;
        }
        let epoch = self.time;
        let b = self.breakup.get_or_insert_with(|| super::Breakup { epoch, destroyed: vec![], torn: vec![] });
        if !b.destroyed.iter().any(|d| d.0 == part) {
            b.destroyed.push((part, cause));
        }
    }
}

fn flying(v: &Vessel) -> bool {
    matches!(v.phase, Phase::Powered { .. } | Phase::Coasting { .. })
}

/// `(r, v)` of `vessel` at `t` relative to `anchor` (through the frame
/// tree: the anchors' offset added when they differ).
fn state_about(world: &World, vessel: &Vessel, t: Epoch, anchor: NodeId) -> (DVec3, DVec3) {
    let (a, r, v) = vessel.state_at(world, t);
    if a == anchor {
        return (r, v);
    }
    let k = world.snapshot(t).relative(a, anchor);
    (r + k.r, v + k.v)
}

/// What one frame of [`advance`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Advance {
    /// The vessels it advanced (indices, ascending): candidates of a bubble.
    pub candidates: Vec<usize>,
    /// The bubble's lattice epoch it reached (`None`: no bubble).
    pub reached: Option<Epoch>,
}

/// Advances the craft a bubble involves towards `target` (see the module);
/// the fleet's own advance then moves the rest (and the candidates that
/// are not members, from where this left them). `controls(i)` are vessel
/// `i`'s controls.
pub fn advance(world: &World, fleet: &mut [Vessel], controls: &dyn Fn(usize) -> Controls, target: Epoch) -> Advance {
    let target = if target > world.end() { world.end() } else { target };
    let n = fleet.len();
    // Landed or crashed craft are never members.
    for v in fleet.iter_mut().filter(|v| v.bubble.member && !flying(v)) {
        v.bubble = BubbleState::default();
    }
    // Leaders now, or about to be (a coast or landing whose engine starts
    // goes live at its own time), and members.
    let starting = |v: &Vessel, c: &Controls| !matches!(v.phase, Phase::Powered { .. }) && v.engine_running(c);
    let mut cores: Vec<usize> = (0..n)
        .filter(|&i| {
            let (v, c) = (&fleet[i], controls(i));
            v.bubble.member || ((v.leads(world, &c) || starting(v, &c)) && v.controllable())
        })
        .collect();
    if cores.is_empty() {
        return Advance::default();
    }
    let reach = |a: &Vessel, b: &Vessel, t: Epoch, span: f64| {
        let anchor = a.state_at(world, t).0;
        let (ra, va) = state_about(world, a, t, anchor);
        let (rb, vb) = state_about(world, b, t, anchor);
        let s = span + NEXT_FRAME;
        (rb - ra).length() - (vb - va).length() * s - 0.5 * MAX_ACCEL * s * s - SEARCH_SLACK
    };
    let near_any = |fleet: &[Vessel], cores: &[usize], i: usize| {
        cores.iter().any(|&c| {
            let t = fleet[c].time;
            c == i || reach(&fleet[c], &fleet[i], t, target.seconds_since(t).max(0.0)) < BUBBLE.leave
        })
    };
    // Engines starting near another craft go live now, as their own
    // advance would at this time.
    for k in 0..cores.len() {
        let i = cores[k];
        let c = controls(i);
        if starting(&fleet[i], &c) && (0..n).any(|j| j != i && flying(&fleet[j]) && near_any(fleet, &[i], j)) {
            let t = fleet[i].time;
            fleet[i].advance(world, t, &c, COAST_BUDGET);
        }
    }
    cores.retain(|&i| matches!(fleet[i].phase, Phase::Powered { .. }));
    let Some(reference) = cores.iter().copied().min_by_key(|&i| fleet[i].id()) else {
        return Advance::default();
    };
    let candidates: Vec<usize> =
        (0..n).filter(|&i| flying(&fleet[i]) && (fleet[i].bubble.member || near_any(fleet, &cores, i))).collect();
    if candidates.len() < 2 && !fleet[reference].bubble.member {
        return Advance::default();
    }
    let anchor = fleet[reference].state(world).0;
    let mut t = fleet[reference].time;
    loop {
        let members = epoch(world, fleet, controls, &candidates, t, anchor);
        let next = t.add_seconds(TICK);
        if next > target || !tick(world, fleet, controls, &members, next) {
            return Advance { candidates, reached: Some(t) };
        }
        t = next;
    }
}

/// At epoch `t`: brings every candidate to it, evaluates the rule, and
/// applies joins and leaves. Returns the members on the lattice at `t`
/// (indices, ascending).
fn epoch(
    world: &World,
    fleet: &mut [Vessel],
    controls: &dyn Fn(usize) -> Controls,
    candidates: &[usize],
    t: Epoch,
    anchor: NodeId,
) -> Vec<usize> {
    for &i in candidates {
        if fleet[i].time < t {
            let c = controls(i);
            fleet[i].advance(world, t, &c, COAST_BUDGET);
        }
    }
    // Craft past the epoch (moved ahead by the fleet) are judged later.
    let here: Vec<usize> = candidates.iter().copied().filter(|&i| fleet[i].time <= t).collect();
    let cands: Vec<Candidate> = here
        .iter()
        .map(|&i| {
            let v = &fleet[i];
            Candidate { flying: flying(v), leader: v.leads(world, &controls(i)), state: v.bubble }
        })
        .collect();
    let pos: Vec<DVec3> = here.iter().map(|&i| state_about(world, &fleet[i], t, anchor).0).collect();
    let member = proximity::members(&BUBBLE, &cands, |a, b| (pos[a] - pos[b]).length());
    let mut out = Vec::new();
    for (k, &i) in here.iter().enumerate() {
        let v = &mut fleet[i];
        let c = controls(i);
        match (v.bubble.member, member[k]) {
            (false, true) => {
                v.bubble = BubbleState { member: true, ..Default::default() };
                v.join(world, anchor);
            }
            (true, false) => {
                v.bubble = BubbleState::default();
                if matches!(v.phase, Phase::Powered { .. }) && v.would_coast(world, &c) {
                    v.start_coast(world);
                }
            }
            _ => {}
        }
        if v.bubble.member && matches!(v.phase, Phase::Powered { .. }) {
            v.align(world, &c, t);
            if v.time == t {
                out.push(i);
            }
        }
    }
    out
}

impl Vessel {
    /// Joins a bubble at its time: a coast goes live there, and a live
    /// vessel is re-anchored to the bubble's `anchor`.
    fn join(&mut self, world: &World, anchor: NodeId) {
        if matches!(self.phase, Phase::Coasting { .. }) {
            self.thermal_to_now(world);
            let state = self.state(world);
            self.go_live(state);
        }
        if let Phase::Powered { anchor: a, r, v } = self.phase {
            if a != anchor {
                let k = world.snapshot(self.time).relative(a, anchor);
                self.phase = Phase::Powered { anchor, r: r + k.r, v: v + k.v };
                self.tick_accel = None;
            }
        }
    }

    /// Brings a live member to the lattice epoch `t`: its own ticks up to
    /// it, then one shorter tick onto it.
    fn align(&mut self, world: &World, controls: &Controls, t: Epoch) {
        if self.time >= t {
            return;
        }
        self.advance(world, t, controls, COAST_BUDGET);
        let dt = t.seconds_since(self.time);
        if dt.is_nan() || dt <= 0.0 || !matches!(self.phase, Phase::Powered { .. }) || self.breakup.is_some() {
            return;
        }
        let design = self.craft.design().clone();
        let Some(start) = self.begin_tick(world, controls, &design, (dt, t)) else { return };
        if let Some(end) = self.fly_tick(world, &start) {
            self.finish_tick(world, start, end, controls);
        }
    }
}

/// Members whose spheres may meet within a tick: connected groups
/// (indices into `members`, ascending; singletons fly alone).
fn groups(fleet: &[Vessel], hulls: &[Arc<Hull>], members: &[usize]) -> Vec<Vec<usize>> {
    let n = members.len();
    let mut root: Vec<usize> = (0..n).collect();
    fn find(root: &mut [usize], i: usize) -> usize {
        let mut i = i;
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    // Each member's bounding sphere, velocity, and the speed its centre
    // turns about the centre of mass.
    let sphere = |m: usize| {
        let v = &fleet[members[m]];
        let Phase::Powered { r, v: vel, .. } = v.phase else { unreachable!("members are live") };
        let lever = v.attitude.q * (hulls[m].center - v.mass_props().com);
        (r + lever, hulls[m].radius, vel, v.attitude.omega.length() * lever.length())
    };
    let spheres: Vec<_> = (0..n).map(sphere).collect();
    for a in 0..n {
        for b in a + 1..n {
            let (ca, ra, va, ta) = spheres[a];
            let (cb, rb, vb, tb) = spheres[b];
            let motion = (vb - va).length() + ta + tb;
            if (cb - ca).length() - ra - rb < CONTACT_MARGIN + motion * TICK {
                let (x, y) = (find(&mut root, a), find(&mut root, b));
                root[x.max(y)] = x.min(y);
            }
        }
    }
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut slot = vec![usize::MAX; n];
    for m in 0..n {
        let r = find(&mut root, m);
        if slot[r] == usize::MAX {
            slot[r] = out.len();
            out.push(Vec::new());
        }
        out[slot[r]].push(m);
    }
    out
}

/// One tick of the bubble's members from `t` to `next`. Returns whether the
/// bubble may go on (every member reached `next` and none broke up).
fn tick(
    world: &World,
    fleet: &mut [Vessel],
    controls: &dyn Fn(usize) -> Controls,
    members: &[usize],
    next: Epoch,
) -> bool {
    let hulls: Vec<Arc<Hull>> = members.iter().map(|&i| proximity::hull_of(&fleet[i].craft.id)).collect();
    for group in groups(fleet, &hulls, members) {
        if let [m] = group[..] {
            let i = members[m];
            fleet[i].advance(world, next, &controls(i), COAST_BUDGET);
            fleet[i].bubble.since_contact = after_tick(fleet[i].bubble.since_contact, false);
        } else {
            let idx: Vec<usize> = group.iter().map(|&m| members[m]).collect();
            let hs: Vec<&Hull> = group.iter().map(|&m| &*hulls[m]).collect();
            if !together(world, fleet, controls, &idx, &hs, next) {
                return false;
            }
        }
    }
    members.iter().all(|&i| fleet[i].time == next && fleet[i].breakup.is_none())
}

/// Speed change a member may gain within a tick beyond its start (m/s:
/// 50 m/s² of thrust), and the slack added to the tick's motion (m).
const TICK_DV: f64 = 1.0;
const TICK_SLACK: f64 = 0.01;

/// How far a point of either body can move relative to the other in a
/// tick (m): the relative speed, the rims' turning, and [`TICK_DV`].
fn tick_margin(a: &Body, b: &Body) -> f64 {
    let rim = |x: &Body| x.omega.length() * (x.hull.radius + (x.hull.center - x.com).length());
    ((b.v - a.v).length() + rim(a) + rim(b) + TICK_DV) * TICK + TICK_SLACK
}

/// A member's ticks since its last craft contact after a tick.
fn after_tick(since: Option<u32>, touched: bool) -> Option<u32> {
    if touched {
        Some(0)
    } else {
        since.map(|s| s.saturating_add(1))
    }
}

/// The tick of members `idx` that may touch: begun alone, flown in joint
/// substeps with the craft contacts, ended alone. Returns false if the
/// tick could not be integrated (the members stay at `t`).
fn together(
    world: &World,
    fleet: &mut [Vessel],
    controls: &dyn Fn(usize) -> Controls,
    idx: &[usize],
    hulls: &[&Hull],
    next: Epoch,
) -> bool {
    let designs: Vec<_> = idx.iter().map(|&i| fleet[i].craft.design().clone()).collect();
    let mut starts: Vec<TickStart> = Vec::with_capacity(idx.len());
    for (k, &i) in idx.iter().enumerate() {
        // A member is held live: it always starts a tick.
        let Some(s) = fleet[i].begin_tick(world, &controls(i), &designs[k], (TICK, next)) else { return false };
        starts.push(s);
    }
    let n = idx.len();
    let mut touched = vec![false; n];
    let mut hits: Vec<Vec<(u32, f64)>> = vec![Vec::new(); n];
    let shared: &[Vessel] = fleet;
    let mut subs: Vec<Substeps> = idx.iter().zip(&starts).map(|(&i, s)| Substeps::new(&shared[i], world, s)).collect();
    let friction: Vec<(f64, f64)> =
        idx.iter().map(|&i| (shared[i].craft.contact.hull.friction, shared[i].craft.contact.stick_speed)).collect();
    // Member m as a contact body, relative to `origin` (the first member's
    // centre of mass with its low part: metres, not orbit radii).
    let body = |m: usize, subs: &[Substeps], origin: (DVec3, DVec3)| {
        let (s, p) = (&subs[m], &starts[m].props);
        Body {
            hull: hulls[m],
            r: (s.r - origin.0) + (s.lo() - origin.1),
            v: s.v,
            q: s.att.q,
            omega: s.att.omega,
            com: p.com,
            mass: p.mass,
            inertia: p.inertia,
        }
    };
    let mut ok = true;
    // The points that can touch this tick, pair by pair (a, b ascending).
    let mut near: Vec<(usize, usize, Near)> = Vec::new();
    let mut origin = (subs[0].r, subs[0].lo());
    for a in 0..n {
        for b in a + 1..n {
            let (ba, bb) = (body(a, &subs, origin), body(b, &subs, origin));
            let margin = tick_margin(&ba, &bb);
            if proximity::sphere_gap(&ba, &bb) < margin {
                near.push((a, b, proximity::near_points(&ba, &bb, margin)));
            }
        }
    }
    'substeps: for k in 0..contact::SUBSTEPS {
        origin = (subs[0].r, subs[0].lo());
        let mut kicks = vec![(DVec3::ZERO, DVec3::ZERO); n];
        for (a, b, near) in &near {
            let (a, b) = (*a, *b);
            let (ba, bb) = (body(a, &subs, origin), body(b, &subs, origin));
            let mu = (friction[a].0 * friction[b].0).sqrt();
            let stick = friction[a].1.max(friction[b].1);
            let f = proximity::pair_forces_near(&ba, &bb, near, mu, stick, subs[a].h());
            if f.count == 0 {
                continue;
            }
            kicks[a].0 += f.force;
            kicks[a].1 += f.torque_a;
            kicks[b].0 -= f.force;
            kicks[b].1 += f.torque_b;
            (touched[a], touched[b]) = (true, true);
            if let Some(im) = f.impact {
                hits[a].push((im.part_a, im.speed));
                hits[b].push((im.part_b, im.speed));
            }
        }
        for (m, sub) in subs.iter_mut().enumerate() {
            if sub.step(k, Some(kicks[m])).is_err() {
                ok = false;
                break 'substeps;
            }
        }
    }
    let ends: Vec<_> = subs.into_iter().map(|s| s.end()).collect();
    if !ok {
        for (k, &i) in idx.iter().enumerate() {
            (fleet[i].attitude, fleet[i].control) = starts[k].before;
        }
        return false;
    }
    for ((k, &i), (start, (end, att, impact))) in idx.iter().enumerate().zip(starts.into_iter().zip(ends)) {
        let v = &mut fleet[i];
        let alone = start.in_vacuum() && start.contact_body.is_none() && !touched[k];
        v.attitude = attitude::finish(att, &mut v.control, &start.cmd, alone);
        if let Some(im) = impact {
            v.impacted(im);
        }
        // The fastest hit on each part (the first such hit, in pair order).
        let mut worst: Vec<(u32, f64)> = Vec::new();
        for &(part, speed) in &hits[k] {
            match worst.iter_mut().find(|w| w.0 == part) {
                Some(w) if speed > w.1 => w.1 = speed,
                Some(_) => {}
                None => worst.push((part, speed)),
            }
        }
        for (part, speed) in worst {
            v.collided(part, speed);
        }
        let c = controls(i);
        v.finish_tick(world, start, end, &c);
        v.bubble.since_contact = after_tick(v.bubble.since_contact, touched[k]);
    }
    true
}

#[cfg(test)]
#[path = "bubble_tests.rs"]
mod tests;
