//! The maneuver program (D126, D128 layer 2): a planned burn flown to the
//! orbit its node gives, closed-loop, by the commands a player issues.
//!
//! **A node** is a Δv along prograde, normal and radial-out at an epoch,
//! relative to a body (the axes' reference: a control choice, never
//! physics, rule 1). **The target** is the orbit the node gives: the
//! navigation state carried to the node along its conic (`kepler::propagate`,
//! the software's model of the orbit), plus the node's Δv in its axes there;
//! taken afresh each tick of the turn and latched when settling begins, so
//! the two-body model spans only the settling and the burn (J2 moves a LEO
//! state ~1 cm/s per second from its conic).
//!
//! **Inputs** are only what the flight software sees ([`super::ship_os::View`]:
//! the attitude, the navigation solution, the engine controllers' readouts,
//! the feeds' settled flags where a level sensor reads them), the node, the
//! stage estimate of its engines (`estimate::lit`, the staging bar's walk)
//! and the craft's static definition. **Outputs** ([`Commands`]) are what a
//! player issues: SAS on with the maneuver hold along a direction, the
//! throttle, a translation, ignitions. The engines' start, settling wait and
//! tail-off are their own logic (layer 1, `systems::propulsion`).
//!
//! **Steps** ([`Step`]): *wait* until the wake epoch; *turn* the thrust
//! axis onto the burn direction; *settle* the propellant on the thrusters
//! (translation along the thrust axis) while a sensor says a feed is
//! unsettled, or for [`SETTLE_LEAD`] without one; *ignite* at the ignition
//! epoch (settled, or [`SETTLE_WAIT`] after it regardless: the start then
//! does what the physics says); *burn* along v_go, the target's velocity at
//! the node minus the current state's carried there; in its last ticks
//! ([`last_ticks`]) throttle so the tick still committed, the next ones and
//! the tail-off end on the target, then cut off (or when v_go turns against
//! the burn); *tail-off* until the engines are off; *done*. An engine that
//! stops on its own ends it *short*; the pilot's conflicting input
//! ([`conflicts`]) ends it *manual*.
//!
//! Numbers marked (E) are estimates awaiting the owner.

use super::ship_os::View;
use crate::craft::state::EngineStatus;
use crate::frame::NodeId;
use crate::time::Epoch;
use crate::vessel::{prograde_normal_radial, Attitude, Controls, HoldMode};
use glam::DVec3;
use serde::{Deserialize, Serialize};

/// The thrust axis within this of the burn direction (rad) counts as
/// aligned (E: 2°).
pub const ALIGN: f64 = 0.035;
/// And turning slower than this (rad/s; E: 0.5°/s).
pub const ALIGN_RATE: f64 = 0.009;
/// How long settling takes without a level sensor, and the lead the
/// schedule gives it (s, E: the Falcon 9 second stage's nitrogen push
/// before a restart is seconds long; falcon9.md §3.5).
pub const SETTLE_LEAD: f64 = 20.0;
/// Past the ignition epoch, the program ignites even on unsettled
/// propellant after this (s, E).
pub const SETTLE_WAIT: f64 = 60.0;
/// In a burn's last seconds the steering holds its direction (s; E: about
/// the attitude hold's response time). Near the cutoff v_go is small, and
/// errors across it swing its direction faster than any turn could follow:
/// a gimballed stage chasing it ends the burn spinning.
pub const STEER_HOLD: f64 = 2.0;

/// A planned burn as the player sets it (D126): Δv along prograde, normal
/// and radial-out (`dv.x`, `y`, `z`, m/s) at epoch `t`, relative to body
/// `reference`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub t: Epoch,
    pub dv: DVec3,
    pub reference: NodeId,
}

/// Where the program is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Step {
    /// Before the wake epoch.
    #[default]
    Wait,
    Turn,
    Settle,
    Burn,
    TailOff,
    /// Cut off on the target.
    Done,
    /// Taken over by the pilot (D126).
    Manual,
    /// Ended early: an engine stopped on its own, never started, or the
    /// navigation solution was lost.
    Short,
}

impl Step {
    /// Whether the program is flying the craft (from the wake to the end).
    pub fn flying(self) -> bool {
        matches!(self, Step::Turn | Step::Settle | Step::Burn | Step::TailOff)
    }

    /// Whether it has ended.
    pub fn ended(self) -> bool {
        matches!(self, Step::Done | Step::Manual | Step::Short)
    }
}

/// The program's saved state for one node of the plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Program {
    /// The node's index in the plan.
    pub node: usize,
    /// When it wakes and when it ignites (the schedule, [`schedule`]).
    pub wake: Epoch,
    pub ignite: Epoch,
    pub step: Step,
    /// The engines it lights (indices into `ShipDef::engines`).
    pub engines: Vec<u32>,
    /// The target state at the node relative to its reference: from the
    /// navigation solution at each tick of the turn, latched when settling
    /// begins (the settling's push is part of the burn).
    pub target: Option<(DVec3, DVec3)>,
    /// The burn direction now (inertial, unit), and at ignition.
    pub dir: Option<DVec3>,
    pub first: Option<DVec3>,
    /// The pilot's controls at the wake: what input is measured against.
    pub pilot: Option<Controls>,
    /// When settling began and when the ignition was commanded.
    pub settle_from: Option<Epoch>,
    pub ignited: Option<Epoch>,
    /// Whether an engine has reported it started.
    pub lit_seen: bool,
    /// |v_go| at the cutoff (m/s), for the log.
    pub left: Option<f64>,
    /// The throttle it commanded at its last tick (what a burn ended by a
    /// separation hands the pilot, [`handover`]).
    #[serde(default)]
    pub throttle: Option<f64>,
}

/// What the program commands this tick: what a player would set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commands {
    /// SAS on with the maneuver hold along this (inertial, unit).
    pub hold: Option<DVec3>,
    pub throttle: Option<f64>,
    /// Translation in body axes.
    pub translate: Option<DVec3>,
    /// Engines to ignite now.
    pub ignite: Vec<u32>,
}

impl Commands {
    /// The pilot's controls with these laid over them.
    pub fn over(&self, pilot: &Controls) -> Controls {
        let mut c = *pilot;
        if self.hold.is_some() {
            c.sas = true;
            c.hold = HoldMode::Maneuver;
        }
        if let Some(t) = self.throttle {
            c.throttle = t;
        }
        if let Some(t) = self.translate {
            c.translate = t;
        }
        c
    }
}

/// What the program reads at a tick.
#[derive(Clone, Copy, Debug)]
pub struct Inputs<'a> {
    pub t: Epoch,
    pub view: &'a View,
    pub node: &'a Node,
    /// The navigation solution relative to the node's reference body, and
    /// that body's μ (the software's model of it).
    pub rel: Option<(DVec3, DVec3)>,
    pub mu: f64,
    /// Whether every feed of its engines is settled, where a level sensor
    /// reads them all.
    pub settled: Option<bool>,
    /// Its engines' thrust axis (body axes, unit).
    pub axis: DVec3,
    /// Whether the thrusters can push along that axis.
    pub can_settle: bool,
    /// The mass (kg) and its engines' full thrust (N), from the estimate.
    pub mass: f64,
    pub thrust: f64,
    /// The sum of its engines' shutdown impulses (N·s), at full level.
    pub tail_impulse: f64,
    /// The lowest level its engines run at (a fraction of full flow).
    pub level_min: f64,
    /// The tick (s).
    pub dt: f64,
}

/// The target at the node: `rel` (relative to the reference, at `dt`
/// seconds before the node) carried to the node, plus `dv` in its
/// prograde, normal and radial-out axes there.
pub fn target(rel: (DVec3, DVec3), mu: f64, dt: f64, dv: DVec3) -> Option<(DVec3, DVec3)> {
    let (r, v) = crate::kepler::propagate(rel.0, rel.1, mu, dt)?;
    let (p, n, radial) = prograde_normal_radial(r, v);
    Some((r, v + p * dv.x + n * dv.y + radial * dv.z))
}

/// The velocity still to gain: the target's velocity at the node minus
/// that of the state `rel` carried there (`dt` seconds ahead).
pub fn v_go(target: (DVec3, DVec3), rel: (DVec3, DVec3), mu: f64, dt: f64) -> Option<DVec3> {
    let (_, v) = crate::kepler::propagate(rel.0, rel.1, mu, dt)?;
    Some(target.1 - v)
}

/// Whether body axis `axis` points along `dir` within [`ALIGN`], turning
/// slower than [`ALIGN_RATE`].
pub fn aligned(att: &Attitude, axis: DVec3, dir: DVec3) -> bool {
    let along = (att.q * axis).normalize().dot(dir.normalize()).clamp(-1.0, 1.0);
    along >= crate::math::cos(ALIGN) && att.omega.length() <= ALIGN_RATE
}

/// Whether v_go has turned against the burn's first direction: overshot.
pub fn overshot(go: DVec3, first: Option<DVec3>) -> bool {
    first.is_some_and(|f| go.dot(f) <= 0.0)
}

/// The throttle in a burn's last ticks, or `None`: cut off now. A command
/// reaches the engines a tick late, so this tick's thrust is committed: `a`
/// (m/s) is what is left along v_go after it. `d` is a tick of
/// full thrust and `tail` the tail-off from full level (m/s; the tail-off
/// scales with the level it starts from), `now` the level this tick and
/// `min` the lowest. Of cutting now (the tail-off from `now`) and one or
/// two more ticks at one level then cutting (the tail-off from that
/// level), it takes the end nearest the target; with more than two ticks
/// left that is full throttle.
pub fn last_ticks(a: f64, d: f64, tail: f64, now: f64, min: f64) -> Option<f64> {
    if d.is_nan() || d <= 0.0 {
        // No estimate of the thrust: burn until it has overshot.
        return (a > 0.0).then_some(1.0);
    }
    let mut best = (None, (a - now * tail).abs());
    for n in [1.0, 2.0] {
        let per = n * d + tail;
        let u = (a / per).clamp(min.clamp(0.0, 1.0), 1.0);
        let err = (a - u * per).abs();
        if err < best.1 {
            best = (Some(u), err);
        }
    }
    best.0
}

/// The mean level of the engines `engines` by their readouts (0 for one
/// nothing reads).
fn level(view: &View, engines: &[u32]) -> f64 {
    if engines.is_empty() {
        return 0.0;
    }
    let sum: f64 =
        engines.iter().map(|&i| view.engines.get(i as usize).and_then(Option::as_ref).map_or(0.0, |r| r.level)).sum();
    sum / engines.len() as f64
}

/// Whether the pilot's controls `now` conflict with the program (D126):
/// against the `baseline` (theirs at the wake), a change that is not what
/// the program commands (`c`), in something it commands. The throttle,
/// SAS and the hold it commands throughout; the translation only while it
/// settles; any rotation input is against the hold. The rest (chutes, the
/// RCS mode, the speed reference, the target) it does not command.
pub fn conflicts(baseline: &Controls, now: &Controls, c: &Commands) -> bool {
    let throttle = now.throttle != baseline.throttle && c.throttle.is_none_or(|t| now.throttle != t);
    let rotate = now.rotate != baseline.rotate && now.rotate != DVec3::ZERO;
    let sas = now.sas != baseline.sas && !now.sas;
    let hold = now.hold != baseline.hold && now.hold != HoldMode::Maneuver;
    let translate = now.translate != baseline.translate && c.translate.is_some_and(|t| now.translate != t);
    throttle || rotate || sas || hold || translate
}

/// The pilot's controls when a burn is handed over (D126: taken over, or
/// ended by a separation): the pilot's `now`, except in what the program
/// commanded (`c`, its last tick's) and the pilot left as it was at the
/// wake (`baseline`). There the burn goes on as it was: the throttle at
/// the program's, and SAS on holding the attitude where it is (the
/// stability hold: the burn's attitude, or where a rotation input leaves
/// it), until the pilot changes them.
pub fn handover(baseline: &Controls, now: &Controls, c: &Commands) -> Controls {
    let mut out = *now;
    if let Some(t) = c.throttle.filter(|_| now.throttle == baseline.throttle) {
        out.throttle = t;
    }
    if c.hold.is_some() && now.sas == baseline.sas && now.hold == baseline.hold {
        out.sas = true;
        out.hold = HoldMode::Stability;
    }
    out
}

/// The schedule of a node of Δv `dv` (m/s) for a burn of thrust `thrust`
/// (N) and flow `mdot` (kg/s) from mass `m0`, engines starting in
/// `start` (s), and a turn taking `turn` (s): ignition so half the Δv is
/// given before the node, and the wake before it by the turn and the
/// settling lead. Returns (wake, ignition).
pub fn schedule(node: &Node, (thrust, mdot, m0): (f64, f64, f64), start: f64, turn: f64) -> (Epoch, Epoch) {
    let half = 0.5 * node.dv.length();
    let t_half = if !(thrust > 0.0 && m0 > 0.0) {
        0.0
    } else if mdot > 0.0 {
        m0 / mdot * (1.0 - crate::math::exp(-half * mdot / thrust))
    } else {
        half * m0 / thrust
    };
    let ignite = node.t.add_seconds(-(t_half + 0.5 * start));
    (ignite.add_seconds(-(turn + SETTLE_LEAD)), ignite)
}

/// Whether the burn direction follows v_go this tick (|v_go| `go`, m/s,
/// at an acceleration `accel`, m/s², from the stage estimate): through the
/// turn and the settling, and in the burn until the time to go falls under
/// [`STEER_HOLD`]; never in the tail-off.
pub fn steers(step: Step, go: f64, accel: f64) -> bool {
    match step {
        Step::TailOff => false,
        Step::Burn => go > STEER_HOLD * accel.max(0.0),
        _ => true,
    }
}

/// Whether engine `i`'s readout says it burns (starting or running).
fn burning(view: &View, i: u32) -> bool {
    view.engines
        .get(i as usize)
        .and_then(Option::as_ref)
        .is_some_and(|r| matches!(r.status, EngineStatus::Starting | EngineStatus::Running))
}

/// Whether engine `i`'s readout says it is off (or failed), or nothing
/// reads it.
fn quiet(view: &View, i: u32) -> bool {
    view.engines
        .get(i as usize)
        .and_then(Option::as_ref)
        .is_none_or(|r| matches!(r.status, EngineStatus::Off | EngineStatus::Failed))
}

/// One tick of the program (see the module): it moves `p` on and returns
/// what it commands. At the wake the caller latches the target and the
/// pilot's baseline ([`Program::wake_up`]).
pub fn step(x: &Inputs, p: &mut Program) -> Commands {
    let c = commands(x, p);
    p.throttle = c.throttle;
    c
}

/// [`step`]'s commands.
fn commands(x: &Inputs, p: &mut Program) -> Commands {
    if !p.step.flying() {
        return Commands::default();
    }
    let to_node = x.node.t.seconds_since(x.t);
    if p.step == Step::Turn {
        // Still coasting: the target from the latest state, so the
        // software's two-body model spans only the settling and the burn.
        if let Some(t) = x.rel.and_then(|rel| target(rel, x.mu, to_node, x.node.dv)) {
            p.target = Some(t);
        }
    }
    let go = x.rel.zip(p.target).and_then(|(rel, target)| v_go(target, rel, x.mu, to_node));
    let accel = if x.mass > 0.0 { x.thrust / x.mass } else { 0.0 };
    if let Some(g) = go.filter(|g| g.length() > 0.0 && steers(p.step, g.length(), accel)) {
        p.dir = Some(g.normalize());
    }
    let hold = p.dir;
    let since = |e: Option<Epoch>| e.map_or(0.0, |e| x.t.seconds_since(e));
    match p.step {
        Step::Turn => {
            let ready = x.view.attitude.zip(hold).is_some_and(|(a, d)| aligned(&a, x.axis, d));
            let ignition = x.t.seconds_since(p.ignite);
            if (ready && ignition >= -SETTLE_LEAD) || ignition >= 0.0 {
                p.step = Step::Settle;
                p.settle_from = Some(x.t);
            }
            Commands { hold, throttle: Some(0.0), ..Commands::default() }
        }
        Step::Settle => {
            let settled = x.settled.unwrap_or(since(p.settle_from) >= SETTLE_LEAD);
            let ignition = x.t.seconds_since(p.ignite);
            let push = x.can_settle.then_some(x.axis);
            if (settled && ignition >= 0.0) || ignition >= SETTLE_WAIT {
                p.step = Step::Burn;
                p.first = p.dir;
                p.ignited = Some(x.t);
                return Commands { hold, throttle: Some(1.0), translate: push, ignite: p.engines.clone() };
            }
            Commands { hold, throttle: Some(0.0), translate: push, ..Commands::default() }
        }
        Step::Burn => {
            p.lit_seen |= p.engines.iter().any(|&i| burning(x.view, i));
            let stopped = p.lit_seen && !p.engines.iter().any(|&i| burning(x.view, i));
            let never = !p.lit_seen && since(p.ignited) > super::propulsion::SETTLE_LIMIT + 1.0;
            if stopped || never {
                p.step = Step::Short;
                p.left = go.map(|g| g.length());
                return Commands::default();
            }
            let Some(g) = go else {
                // No navigation: nothing to close the loop on.
                p.step = Step::TailOff;
                return Commands { hold, throttle: Some(0.0), ..Commands::default() };
            };
            let (tail, tick) =
                if x.mass > 0.0 { (x.tail_impulse / x.mass, x.thrust / x.mass * x.dt) } else { (0.0, 0.0) };
            // The thrust axis now (the attitude's), else the burn direction.
            let along = x.view.attitude.map(|a| (a.q * x.axis).normalize()).or(hold).unwrap_or(g.normalize_or_zero());
            let now = level(x.view, &p.engines);
            // Left along v_go after this tick's committed thrust.
            let a = g.length() - now * tick * along.dot(g.normalize_or_zero());
            let throttle = if overshot(g, p.first) { None } else { last_ticks(a, tick, tail, now, x.level_min) };
            match throttle {
                None if p.lit_seen => {
                    p.step = Step::TailOff;
                    // What the committed tick and the tail-off leave.
                    p.left = Some((g - along * now * (tick + tail)).length());
                    Commands { hold, throttle: Some(0.0), ..Commands::default() }
                }
                // Not lit yet: its start is the engines' own.
                None => Commands { hold, throttle: Some(1.0), ..Commands::default() },
                Some(u) => Commands { hold, throttle: Some(u), ..Commands::default() },
            }
        }
        Step::TailOff => {
            if p.engines.iter().all(|&i| quiet(x.view, i)) {
                p.step = if p.left.is_some() { Step::Done } else { Step::Short };
                return Commands::default();
            }
            Commands { hold, throttle: Some(0.0), ..Commands::default() }
        }
        Step::Wait | Step::Done | Step::Manual | Step::Short => Commands::default(),
    }
}

impl Program {
    /// The program of node `node`, scheduled ([`schedule`]) to wake and
    /// ignite at those epochs with `engines`.
    pub fn new(node: usize, (wake, ignite): (Epoch, Epoch), engines: Vec<u32>) -> Self {
        Program {
            node,
            wake,
            ignite,
            step: Step::Wait,
            engines,
            target: None,
            dir: None,
            first: None,
            pilot: None,
            settle_from: None,
            ignited: None,
            lit_seen: false,
            left: None,
            throttle: None,
        }
    }

    /// The controls flown with the pilot's `pilot` after its last tick:
    /// while it flies, its hold along the burn and its throttle laid over
    /// them ([`Commands::over`], as the vessel flies them); else the
    /// pilot's. What the navball shows (its hold buttons read the pilot's
    /// STAB while the program held the burn).
    pub fn flown(&self, pilot: &Controls) -> Controls {
        if !self.step.flying() {
            return *pilot;
        }
        Commands { hold: self.dir, throttle: self.throttle, ..Commands::default() }.over(pilot)
    }

    /// Wakes the program at its wake epoch: the target from the navigation
    /// solution `rel` (relative to the node's reference) `to_node` seconds
    /// before the node, and the pilot's controls as the baseline. Without
    /// a navigation solution it cannot fly: it ends short.
    pub fn wake_up(&mut self, node: &Node, rel: Option<(DVec3, DVec3)>, mu: f64, to_node: f64, pilot: &Controls) {
        self.pilot = Some(*pilot);
        self.target = rel.and_then(|rel| target(rel, mu, to_node, node.dv));
        self.step = if self.target.is_some() { Step::Turn } else { Step::Short };
    }
}

#[cfg(test)]
#[path = "maneuver_tests.rs"]
mod tests;
