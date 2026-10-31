//! Attitude hold modes (D075): the direction the attitude control points
//! the craft at.
//!
//! [`hold_direction`] is the one rule: a pure function of the vessel's
//! state relative to a **reference body** (for prograde and retrograde:
//! orbital or surface velocity), the target's relative state, and the next
//! planned burn. The reference body is a control choice stated with the
//! mode (the game passes the navball's reference, [`Controls::reference`]);
//! it never changes the physics (rule 1).
//!
//! The existing attitude controller ([`super::attitude`]) tracks the
//! direction: stability keeps the attitude where rotation stopped (as
//! before), the others turn the nose (the engine's axis for the maneuver
//! hold) onto the direction with the smallest rotation, so the roll is
//! carried from where the hold began.

use super::attitude::{quat_from_rotvec, Attitude, RotationBase};
use super::burn::FlightPlan;
use super::Controls;
use crate::ephem::{Ephemeris, NodeKind, Snapshot};
use crate::frame::NodeId;
use crate::math;
use crate::time::Epoch;
use crate::world::World;
use glam::{DMat3, DQuat, DVec3};

/// What the attitude control holds while SAS is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HoldMode {
    /// Kill the rotation, then hold the attitude where it stopped.
    #[default]
    Stability,
    /// Nose along the velocity (in the stated [`SpeedReference`]).
    Prograde,
    Retrograde,
    /// Nose towards the target.
    Target,
    AntiTarget,
    /// Engine axis along the next planned burn's thrust direction.
    Maneuver,
}

impl HoldMode {
    pub const ALL: [HoldMode; 6] = [
        HoldMode::Stability,
        HoldMode::Prograde,
        HoldMode::Retrograde,
        HoldMode::Target,
        HoldMode::AntiTarget,
        HoldMode::Maneuver,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HoldMode::Stability => "STABILITY",
            HoldMode::Prograde => "PROGRADE",
            HoldMode::Retrograde => "RETROGRADE",
            HoldMode::Target => "TARGET",
            HoldMode::AntiTarget => "ANTI-TARGET",
            HoldMode::Maneuver => "MANEUVER",
        }
    }

    /// Whether prograde/retrograde's speed reference applies.
    pub fn uses_speed(self) -> bool {
        matches!(self, HoldMode::Prograde | HoldMode::Retrograde)
    }
}

/// Which velocity prograde and retrograde follow (the navball's mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SpeedReference {
    /// Relative to the reference body's centre (inertial axes).
    #[default]
    Orbit,
    /// Relative to the reference body's rotating surface.
    Surface,
    /// Relative to the target.
    Target,
}

impl SpeedReference {
    pub fn label(self) -> &'static str {
        match self {
            SpeedReference::Orbit => "ORBIT",
            SpeedReference::Surface => "SURFACE",
            SpeedReference::Target => "TARGET",
        }
    }
}

/// A target for the hold: its position and velocity relative to `anchor`
/// at `epoch`, carried to other epochs at constant velocity (the game
/// refreshes it every frame). A body is its own anchor with a zero state,
/// exact at every epoch.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TargetState {
    pub anchor: NodeId,
    pub epoch: Epoch,
    pub r: DVec3,
    pub v: DVec3,
}

/// Below this speed (m/s) there is no prograde (the navball's rule too).
pub const MIN_SPEED: f64 = 0.1;

/// What the hold rule reads at an instant (inertial axes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HoldInputs {
    /// Vessel velocity relative to the reference body's centre, and to its
    /// rotating surface.
    pub v_orbit: DVec3,
    pub v_surface: DVec3,
    /// The target relative to the vessel: position and velocity.
    pub target: Option<(DVec3, DVec3)>,
    /// Thrust direction of the next planned burn now.
    pub maneuver: Option<DVec3>,
}

/// The direction to hold (unit, inertial), or `None` when the mode has
/// none (stability, or no velocity, target or planned burn): the
/// controller then holds the attitude (stability).
pub fn hold_direction(mode: HoldMode, speed: SpeedReference, x: &HoldInputs) -> Option<DVec3> {
    let unit = |d: DVec3, min: f64| (d.length() > min).then(|| d.normalize());
    let velocity = match speed {
        SpeedReference::Orbit => Some(x.v_orbit),
        SpeedReference::Surface => Some(x.v_surface),
        // The vessel's velocity relative to the target.
        SpeedReference::Target => x.target.map(|(_, v)| -v),
    };
    match mode {
        HoldMode::Stability => None,
        HoldMode::Prograde => velocity.and_then(|v| unit(v, MIN_SPEED)),
        HoldMode::Retrograde => velocity.and_then(|v| unit(-v, MIN_SPEED)),
        HoldMode::Target => x.target.and_then(|(r, _)| unit(r, 0.0)),
        HoldMode::AntiTarget => x.target.and_then(|(r, _)| unit(-r, 0.0)),
        HoldMode::Maneuver => x.maneuver.and_then(|d| unit(d, 0.0)),
    }
}

/// A snapshot of the ephemeris made only if asked for (it costs every
/// node's position; holds relative to the vessel's own anchor need none).
pub struct LazySnapshot<'a, 'w> {
    world: &'w World,
    pub t: Epoch,
    given: Option<&'a Snapshot<'w>>,
    own: std::cell::OnceCell<Snapshot<'w>>,
}

impl<'a, 'w> LazySnapshot<'a, 'w> {
    pub fn new(world: &'w World, t: Epoch) -> Self {
        LazySnapshot { world, t, given: None, own: std::cell::OnceCell::new() }
    }

    pub fn of(world: &'w World, snap: &'a Snapshot<'w>) -> Self {
        LazySnapshot { world, t: snap.t, given: Some(snap), own: std::cell::OnceCell::new() }
    }

    pub fn get(&self) -> &Snapshot<'w> {
        self.given.unwrap_or_else(|| self.own.get_or_init(|| self.world.snapshot(self.t)))
    }
}

/// The heaviest body directly under barycenter `bary` (the first of equal
/// ones).
fn heaviest(eph: &Ephemeris, bary: NodeId) -> Option<NodeId> {
    let mut best: Option<NodeId> = None;
    for b in eph.bodies().filter(|&b| eph.node(b).parent == Some(bary)) {
        if best.is_none_or(|x| eph.node(b).gm > eph.node(x).gm) {
            best = Some(b);
        }
    }
    best
}

/// The body a node orbits (its primary): its parent body, or for a node
/// under a barycenter that barycenter's heaviest body (or, for that body
/// itself, the barycenter's own primary). Earth → Sun, Moon → Earth,
/// Charon → Pluto, Jupiter → Sun, Sun → none. The one rule of the body
/// hierarchy (D056; the game's display primary is this).
pub fn primary(eph: &Ephemeris, node: NodeId) -> Option<NodeId> {
    let parent = eph.node(node).parent?;
    if eph.node(parent).kind == NodeKind::Body {
        return Some(parent);
    }
    match heaviest(eph, parent) {
        Some(h) if h != node => Some(h),
        _ => primary(eph, parent),
    }
}

/// The heaviest body directly under each node, by node index.
fn heaviest_under(eph: &Ephemeris) -> Vec<Option<NodeId>> {
    let mut heaviest: Vec<Option<NodeId>> = vec![None; eph.nodes().len()];
    for b in eph.bodies() {
        if let Some(p) = eph.node(b).parent {
            let h = &mut heaviest[p.0 as usize];
            if h.is_none_or(|x| eph.node(b).gm > eph.node(x).gm) {
                *h = Some(b);
            }
        }
    }
    heaviest
}

/// Calls `f` with each body whose [`primary`] is `body` (`heaviest` from
/// [`heaviest_under`]): the bodies under it or, when it is the heaviest
/// body under a barycenter, under that barycenter, directly or as the
/// heaviest body of a barycenter there.
fn satellites(eph: &Ephemeris, heaviest: &[Option<NodeId>], body: NodeId, mut f: impl FnMut(NodeId)) {
    let bary = eph.node(body).parent.filter(|&p| eph.node(p).kind != NodeKind::Body);
    let heads = bary.is_some_and(|b| heaviest[b.0 as usize] == Some(body));
    let around = |p: NodeId| p == body || (heads && Some(p) == bary);
    for (i, n) in eph.nodes().iter().enumerate() {
        let id = NodeId(i as u16);
        if id == body || !n.parent.is_some_and(around) {
            continue;
        }
        match n.kind {
            NodeKind::Body => f(id),
            _ => heaviest[i].into_iter().for_each(&mut f),
        }
    }
}

/// The Laplace radius of a body of `gm` a distance `d` from its primary of
/// `gm_primary`: `d·(gm/gm_primary)^(2/5)`, where its pull relative to the
/// primary's tide equals the primary's pull relative to its own (D056).
pub fn laplace_radius(d: f64, gm: f64, gm_primary: f64) -> f64 {
    d * math::exp(0.4 * math::ln(gm / gm_primary))
}

/// The body prograde and retrograde are relative to when the player names
/// none ([`Controls::reference`] `None`): the deepest body whose Laplace
/// sphere ([`laplace_radius`] about its [`primary`] at this instant)
/// contains the vessel at `r` (relative to `anchor`), descending from the
/// root body; the root body (the Sun) if none does. D056's dominance as a
/// pure function of time and state, the same for every anchor (rule 1).
/// A control choice: it says where the craft points, never how it is
/// pulled.
pub fn hold_reference(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> NodeId {
    let eph = &world.eph;
    let root = eph.root();
    let top = if eph.node(root).kind == NodeKind::Body { Some(root) } else { heaviest(eph, root) };
    let Some(mut current) = top else { return anchor };
    let d_star = world.star_distance(snap, anchor, r);
    let heaviest = heaviest_under(eph);
    loop {
        let at_current = snap.relative_r(current, anchor);
        let s = (r - at_current).length();
        let gm = eph.node(current).gm;
        let mut inside: Option<(NodeId, f64)> = None;
        satellites(eph, &heaviest, current, |b| {
            // Inside b's sphere, |r − r_b| < k·d with d = |r_b − r_P| ≤
            // |r − r_b| + s, so d < s/(1 − k) and |r − r_b| < k·s/(1 − k);
            // k ≤ (m/M)^(1/4) for m < M. A body that cannot come that close
            // (a far moon system, a dwarf planet: World::may_reach) is not
            // evaluated.
            let ratio = eph.node(b).gm / gm;
            let k = ratio.sqrt().sqrt();
            let far = |src| k < 1.0 && !world.may_reach(snap, anchor, r, d_star, src, k * s / (1.0 - k));
            if world.source(b).is_some_and(far) {
                return;
            }
            let at = snap.relative_r(b, anchor);
            let q = (r - at).length() / laplace_radius((at - at_current).length(), eph.node(b).gm, gm);
            if q < 1.0 && inside.is_none_or(|(_, best)| q < best) {
                inside = Some((b, q));
            }
        });
        match inside {
            Some((b, _)) => current = b,
            None => return current,
        }
    }
}

/// The hold inputs for a vessel at `(r, v)` relative to `anchor` at the
/// snapshot's epoch, with `controls`' reference body (or the automatic
/// one, [`hold_reference`]; the anchor when the mode reads no velocity)
/// and target. Relative states go through the frame tree (rule 3).
pub fn hold_inputs(
    world: &World,
    snap: &LazySnapshot,
    (anchor, r, v): (NodeId, DVec3, DVec3),
    controls: &Controls,
    maneuver: Option<DVec3>,
) -> HoldInputs {
    // The automatic reference only where the mode reads the velocity.
    let reads_velocity =
        matches!(controls.hold, HoldMode::Prograde | HoldMode::Retrograde) && controls.speed != SpeedReference::Target;
    let body = match controls.reference {
        Some(body) => body,
        None if reads_velocity => hold_reference(world, snap.get(), anchor, r),
        None => anchor,
    };
    let (rel_r, rel_v) = if body == anchor {
        (r, v)
    } else {
        let k = snap.get().relative(body, anchor);
        (r - k.r, v - k.v)
    };
    let spin =
        world.source(body).and_then(|s| s.physical.as_ref()).map_or(DVec3::ZERO, |p| p.rotation.omega(snap.t).raw());
    let target = controls.target.map(|ts| {
        let dt = snap.t.seconds_since(ts.epoch);
        let k = if ts.anchor == anchor { Default::default() } else { snap.get().relative(ts.anchor, anchor) };
        (k.r + ts.r + ts.v * dt - r, k.v + ts.v - v)
    });
    HoldInputs { v_orbit: rel_v, v_surface: rel_v - spin.cross(rel_r), target, maneuver }
}

/// The thrust direction of the plan's next burn (from index `from`, igniting
/// at or after `t`) for a vessel at `(r, v)` relative to `anchor`.
pub fn maneuver_direction(
    plan: &FlightPlan,
    nodes: &[crate::systems::maneuver::Node],
    from: usize,
    snap: &LazySnapshot,
    (anchor, r, v): (NodeId, DVec3, DVec3),
) -> Option<DVec3> {
    let law = plan.next_direction(nodes, from, snap.t)?;
    Some(law.direction_from(|node| snap.get().relative(node, anchor), anchor, r, v))
}

/// What the controller aims at during a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aim {
    /// Damp, then hold the attitude where the rotation stopped.
    Stability,
    /// Turn body axis `axis` onto `dir` (inertial), both unit. `key` names
    /// the rule the direction comes from: a settled hold follows it without
    /// control ticks on rails only while the key stays the same.
    Direction { dir: DVec3, axis: DVec3, key: AimKey },
}

/// Which rule a direction comes from ([`Aim::Direction`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AimKey {
    pub mode: HoldMode,
    pub speed: SpeedReference,
    pub reference: Option<NodeId>,
}

impl AimKey {
    /// The key of a planned burn's (or the turn before it) maneuver hold.
    pub const BURN: AimKey = AimKey { mode: HoldMode::Maneuver, speed: SpeedReference::Orbit, reference: None };
}

/// The aim for `controls` given the hold inputs; `engine_axis` is the
/// thrust direction in body axes (the maneuver hold points it).
pub fn aim(controls: &Controls, inputs: &HoldInputs, engine_axis: DVec3) -> Aim {
    let mode = controls.hold;
    match hold_direction(mode, controls.speed, inputs) {
        None => Aim::Stability,
        Some(dir) => {
            let axis = if mode == HoldMode::Maneuver { engine_axis } else { DVec3::Z };
            let speed = if mode.uses_speed() { controls.speed } else { SpeedReference::Orbit };
            let reference = if mode.uses_speed() { controls.reference } else { None };
            Aim::Direction { dir, axis, key: AimKey { mode, speed, reference } }
        }
    }
}

/// The smallest rotation taking unit `from` onto unit `to` (a half turn
/// about a deterministic perpendicular axis when opposite).
pub fn quat_arc(from: DVec3, to: DVec3) -> DQuat {
    let axis = from.cross(to);
    let s = axis.length();
    let c = from.dot(to);
    if s < 1e-15 {
        return if c > 0.0 { DQuat::IDENTITY } else { quat_from_rotvec(from.any_orthonormal_vector() * math::PI) };
    }
    quat_from_rotvec(axis / s * math::atan2(s, c))
}

/// Time (s) to turn half a revolution from rest and settle, for body
/// inertia `inertia` and attitude-control authority `torque` (per body
/// axis): the lead with which the maneuver hold engages before a planned
/// burn, a whole number of ticks. The controller accelerates at the
/// authority and brakes at half of it (`w = √(α·e)`), taking
/// `3·√(2π/(3α))`, then settles within [`SETTLE`] (critically damped).
pub fn turn_lead(inertia: &DMat3, torque: DVec3) -> f64 {
    let alpha = (torque.x / inertia.x_axis.x).min(torque.y / inertia.y_axis.y);
    if !(alpha.is_finite() && alpha > 0.0) {
        return SETTLE;
    }
    let t = 3.0 * (2.0 * math::PI / (3.0 * alpha)).sqrt() + SETTLE;
    libm::ceil(t / super::TICK) * super::TICK
}

/// [`turn_lead`] with the turn rate capped at `max_rate` (rad/s; the gas
/// a craft's thrusters can afford, `Vessel::gas_turn_cap`): where the cap
/// binds, a half turn at that rate, its spin-up, and its stop at half the
/// authority, then the settling.
pub fn turn_lead_capped(inertia: &DMat3, torque: DVec3, max_rate: f64) -> f64 {
    let lead = turn_lead(inertia, torque);
    let alpha = (torque.x / inertia.x_axis.x).min(torque.y / inertia.y_axis.y);
    if !(alpha.is_finite() && alpha > 0.0 && max_rate.is_finite() && max_rate > 0.0) {
        return lead;
    }
    let capped = math::PI / max_rate + 1.5 * max_rate / alpha + SETTLE;
    lead.max(libm::ceil(capped / super::TICK) * super::TICK)
}

/// Settling time of the hold after a turn (s): the critically damped tail
/// from the braking phase to the snap threshold, with margin.
pub const SETTLE: f64 = 30.0;

/// A settled direction hold followed without control ticks (rails): the
/// base attitude turned by the smallest rotation onto `dir`.
pub fn follow(base: &RotationBase, axis: DVec3, dir: DVec3) -> Attitude {
    let q = (quat_arc(base.att.q * axis, dir) * base.att.q).normalize();
    Attitude { q, omega: base.att.omega }
}

impl super::Vessel {
    /// What SAS aims at for a vessel at `state` (relative to its anchor) at
    /// the snapshot's epoch with `controls`; the maneuver hold follows the
    /// plan's next burn.
    pub(super) fn aim(
        &self,
        world: &World,
        snap: &Snapshot,
        state: (NodeId, DVec3, DVec3),
        controls: &Controls,
    ) -> Aim {
        if !controls.sas || controls.hold == HoldMode::Stability {
            return Aim::Stability;
        }
        let snap = LazySnapshot::of(world, snap);
        // A flying maneuver program's direction, else the plan's next burn or node.
        let maneuver = (controls.hold == HoldMode::Maneuver)
            .then(|| self.program_direction().or_else(|| maneuver_direction(&self.plan, &self.nodes, 0, &snap, state)));
        let inputs = hold_inputs(world, &snap, state, controls, maneuver.flatten());
        aim(controls, &inputs, self.craft.engine.mount_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_direction_table() {
        let x = HoldInputs {
            v_orbit: DVec3::new(0.0, 7_500.0, 0.0),
            v_surface: DVec3::new(0.0, 7_000.0, 100.0),
            target: Some((DVec3::new(1_000.0, 0.0, 0.0), DVec3::new(0.0, -3.0, 4.0))),
            maneuver: Some(DVec3::new(0.0, 0.0, -2.0)),
        };
        let none = HoldInputs { v_orbit: DVec3::new(0.05, 0.0, 0.0), ..HoldInputs::default() };
        use HoldMode::*;
        use SpeedReference as S;
        let cases = [
            (Stability, S::Orbit, &x, None),
            (Prograde, S::Orbit, &x, Some(DVec3::Y)),
            (Retrograde, S::Orbit, &x, Some(-DVec3::Y)),
            (Prograde, S::Surface, &x, Some(DVec3::new(0.0, 7_000.0, 100.0).normalize())),
            // Relative to the target: the vessel's velocity minus the target's.
            (Prograde, S::Target, &x, Some(DVec3::new(0.0, 0.6, -0.8))),
            (Retrograde, S::Target, &x, Some(DVec3::new(0.0, -0.6, 0.8))),
            (Target, S::Surface, &x, Some(DVec3::X)),
            (AntiTarget, S::Orbit, &x, Some(-DVec3::X)),
            (Maneuver, S::Orbit, &x, Some(-DVec3::Z)),
            // Undefined: too slow, no target, no burn.
            (Prograde, S::Orbit, &none, None),
            (Prograde, S::Target, &none, None),
            (Target, S::Orbit, &none, None),
            (Maneuver, S::Orbit, &none, None),
        ];
        for (mode, speed, inputs, expected) in cases {
            let got = hold_direction(mode, speed, inputs);
            match (got, expected) {
                (Some(g), Some(e)) => assert!(g.abs_diff_eq(e, 1e-15), "{mode:?} {speed:?}: {g} vs {e}"),
                (g, e) => assert_eq!(g, e, "{mode:?} {speed:?}"),
            }
        }
    }

    #[test]
    fn arcs_turn_one_direction_onto_another() {
        let dirs = [DVec3::X, DVec3::new(0.3, -0.4, 0.866).normalize(), -DVec3::X, DVec3::Z, -DVec3::Z];
        for a in dirs {
            for b in dirs {
                assert!((quat_arc(a, b) * a - b).length() < 1e-12, "{a} → {b}");
            }
        }
    }

    #[test]
    fn the_turn_lead_grows_with_inertia() {
        let torque = DVec3::new(40e3, 40e3, 20e3);
        let small = turn_lead(&DMat3::from_diagonal(DVec3::new(1e4, 1e4, 1e4)), torque);
        let big = turn_lead(&DMat3::from_diagonal(DVec3::new(1e6, 1e6, 1e4)), torque);
        assert!(small > SETTLE && big > small, "{small} {big}");
        assert_eq!(libm::round(big / super::super::TICK) * super::super::TICK, big);
        assert_eq!(turn_lead(&DMat3::IDENTITY, DVec3::ZERO), SETTLE);
    }

    fn sol() -> World {
        let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), crate::sol::EPHEMERIS_PATH);
        World::sol(std::sync::Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
    }

    /// The rule without shortcuts: satellites from [`primary`], every
    /// sphere evaluated.
    fn exhaustive(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> NodeId {
        let eph = &world.eph;
        let bodies: Vec<NodeId> = eph.bodies().collect();
        let mut current = bodies.iter().copied().find(|&b| primary(eph, b).is_none()).unwrap();
        loop {
            let at_c = snap.relative_r(current, anchor);
            let gm = eph.node(current).gm;
            let mut best: Option<(NodeId, f64)> = None;
            for &b in bodies.iter().filter(|&&b| primary(eph, b) == Some(current)) {
                let at = snap.relative_r(b, anchor);
                let q = (r - at).length() / laplace_radius((at - at_c).length(), eph.node(b).gm, gm);
                if q < 1.0 && best.is_none_or(|(_, x)| q < x) {
                    best = Some((b, q));
                }
            }
            match best {
                Some((b, _)) => current = b,
                None => return current,
            }
        }
    }

    #[test]
    fn satellites_are_the_bodies_whose_primary_it_is() {
        let w = sol();
        let eph = &w.eph;
        for body in eph.bodies() {
            let mut sats = Vec::new();
            satellites(eph, &heaviest_under(eph), body, |b| sats.push(b));
            sats.sort();
            let expected: Vec<NodeId> = eph.bodies().filter(|&b| primary(eph, b) == Some(body)).collect();
            assert_eq!(sats, expected, "{}", eph.node(body).name);
        }
        let id = |n: &str| w.find(n).unwrap().node;
        assert_eq!(primary(eph, id("Moon")), Some(id("Earth")));
        assert_eq!(primary(eph, id("Earth")), Some(id("Sun")));
        assert_eq!(primary(eph, id("Charon")), Some(id("Pluto")));
        assert_eq!(primary(eph, id("Io")), Some(id("Jupiter")));
        assert_eq!(primary(eph, id("Sun")), None);
    }

    /// The automatic reference: the deepest Laplace sphere containing the
    /// vessel, the same whatever the anchor (rule 1), and the same as the
    /// rule evaluated without shortcuts, at the table's points and at
    /// 3,000 points about every body from just above its surface to ten
    /// times its sphere.
    #[test]
    fn the_automatic_reference_is_the_deepest_sphere_whatever_the_anchor() {
        let w = sol();
        let eph = &w.eph;
        let id = |n: &str| w.find(n).unwrap().node;
        let t = crate::sol::sol_epoch().add_seconds(3.0 * 86_400.0);
        let snap = w.snapshot(t);
        let anchors = [eph.root(), id("Sun"), id("Earth"), id("Moon"), id("Jupiter")];
        let check = |about: NodeId, offset: DVec3| {
            let answers: Vec<NodeId> = (anchors.iter().chain([&about]))
                .map(|&a| hold_reference(&w, &snap, a, snap.relative_r(about, a) + offset))
                .collect();
            assert!(answers.iter().all(|&x| x == answers[0]), "{} {offset}: {answers:?}", eph.node(about).name);
            let full = exhaustive(&w, &snap, about, offset);
            assert_eq!(answers[0], full, "{} {offset}", eph.node(about).name);
            answers[0]
        };
        let table = [
            ("Earth", 6.8e6, "Earth"),
            ("Earth", 4.0e8, "Earth"),
            ("Earth", 1.5e9, "Sun"),
            ("Moon", 1.9e6, "Moon"),
            ("Jupiter", 1.0e8, "Jupiter"),
            ("Io", 2.5e6, "Io"),
            ("Pluto", 3.0e6, "Pluto"),
            ("Ceres", 1.0e6, "Ceres"),
            ("Sun", 1.0e12, "Sun"),
        ];
        for (about, d, expected) in table {
            let got = check(id(about), DVec3::new(0.6, -0.48, 0.64) * d);
            assert_eq!(got, id(expected), "{about} at {d} m");
        }
        // A fixed pseudo-random sweep.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        for body in eph.bodies() {
            let radius = w.source(body).and_then(|s| s.physical.as_ref()).map_or(1e3, |p| p.radius_eq);
            let sphere = primary(eph, body)
                .map_or(1e13, |p| laplace_radius(snap.relative_r(body, p).length(), eph.node(body).gm, eph.node(p).gm));
            for _ in 0..70 {
                let d = radius * 1.01 * math::exp(next() * math::ln(10.0 * sphere / radius));
                let dir = DVec3::new(next() - 0.5, next() - 0.5, next() - 0.5).normalize();
                check(body, dir * d);
            }
        }
    }
}
