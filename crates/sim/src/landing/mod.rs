//! Landing prediction (playtest-1 §C): where, when and how fast a vessel
//! meets the ground if it keeps flying as it is, and when a full-thrust
//! braking burn must start to stop just above the ground.
//!
//! The prediction integrates the vessel's own force model, not a coast
//! approximation: N-body gravity with J2 and the tidal cutoff
//! ([`ForceContext`]), drag and lift from the craft's aerodynamic bake
//! ([`AeroTick`]) at the actual Mach and Knudsen number and an assumed
//! attitude (surface-retrograde by default), the parachute if deployed,
//! and the current throttle with thrust at ambient pressure along that
//! attitude and the mass falling with the propellant burned. The live tick
//! freezes the aerodynamic coefficients over each 20 ms tick; here they are
//! evaluated at every stage of an adaptive Dormand–Prince integration
//! (deterministic, [`crate::math`] only), which agrees with the tick to
//! well below a metre over a descent.
//!
//! The prediction ends when the lowest contact point (at the assumed
//! attitude) meets the physical surface (`surface_height`, found by
//! bisection on the step's interpolant like a coast's contact), within its
//! own horizon ([`Limits`]), independent of the drawn orbit line.
//!
//! The impact is reported in the body-fixed frame, so the ground point can
//! be drawn where it is *now* (carried by the body's rotation), and the
//! surface-relative velocity is split along the local vertical (radial
//! from the body's centre).

mod model;

use crate::craft::engine::P_SEA_LEVEL;
use crate::craft::CraftParams;
use crate::ephem::Snapshot;
use crate::forces::ambient_pressure;
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::integrate::{hermite5, StepSample};
use crate::time::Epoch;
use crate::vessel::limbo::LiveBoundary;
use crate::vessel::{EndKind, Phase, Segment, Vessel};
use crate::world::World;
use glam::{DQuat, DVec3};
use model::{End, Model, Run};

/// Default prediction horizon for unbound paths (s).
pub const DEFAULT_HORIZON: f64 = 86_400.0;
/// Integration steps of one prediction, at most.
pub const MAX_STEPS: usize = 40_000;
/// The braking solution's ignition time is found to this precision (s):
/// at 250 m/s a quarter of a metre in the stop height.
pub const IGNITION_PRECISION: f64 = 1e-3;

/// How the craft is assumed to point during a prediction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AssumedAttitude {
    /// Thrust (the engine's mount direction) against the velocity relative
    /// to the reference body's surface; straight up when at rest.
    SurfaceRetrograde,
    /// Nose (the engine's mount direction) into the relative wind: a
    /// statically stable craft left to itself; straight up at rest.
    SurfacePrograde,
    /// A fixed inertial attitude (body → inertial).
    Inertial(DQuat),
}

/// Where a prediction starts and what the craft does meanwhile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandingStart {
    pub t: Epoch,
    pub anchor: NodeId,
    pub r: DVec3,
    pub v: DVec3,
    /// Propellant at `t` the engines burn (kg). Infinite when the craft
    /// does not measure it (no level sensor on a tank they draw from): the
    /// prediction assumes they do not run dry, and holds the mass.
    pub propellant: f64,
    /// The engines the throttle runs, lumped ([`Thrust::of_vessel`]).
    pub thrust: Thrust,
    /// Its mass (kg) and centre of mass (body axes) at `t`; the mass falls
    /// with the flow and the centre is held.
    pub mass: (f64, DVec3),
    /// Debug mode (D064): the propellant never runs out.
    pub infinite: bool,
    /// Throttle held throughout (0: engine off).
    pub throttle: f64,
    /// The parachutes, armed if the command is given.
    pub chute: crate::chute::ChuteState,
    pub attitude: AssumedAttitude,
    /// The body the surface-relative quantities refer to (retrograde,
    /// vertical and horizontal speed, the braking stop).
    pub body: NodeId,
}

impl LandingStart {
    /// A flying vessel's prediction start with `throttle` and its chutes
    /// (armed by `chute_command`), from the vessel's time. `None` for landed and crashed vessels, and for one whose
    /// flight software has no navigation solution (`ship_os::can_predict`).
    pub fn of_vessel(
        world: &World,
        vessel: &Vessel,
        throttle: f64,
        chute_command: bool,
        attitude: AssumedAttitude,
    ) -> Option<Self> {
        // What the flight software knows (F3): a navigation solution, and the
        // attitude when it assumes the current one held.
        let inertial = matches!(attitude, AssumedAttitude::Inertial(_));
        if !crate::systems::ship_os::can_predict(&vessel.knowledge(), inertial) {
            return None;
        }
        let mut chute = if vessel.destruction().is_none() { vessel.chute } else { Default::default() };
        chute.armed |= chute_command;
        let (t, (anchor, r, v), mass, throttle) = match &vessel.phase {
            Phase::Landed { .. } | Phase::Crashed { .. } => return None,
            Phase::Coasting { .. } | Phase::Powered { .. } => {
                (vessel.time, vessel.state(world), vessel.mass(), throttle)
            }
        };
        let body = nearest_surface(world, &world.snapshot(t), anchor, r)?;
        let infinite = vessel.debug();
        let (propellant, thrust, mass) = propulsion_of(vessel, mass);
        Some(LandingStart { t, anchor, r, v, propellant, thrust, mass, infinite, throttle, chute, attitude, body })
    }

    /// Thrust-to-weight ratio at full throttle with thrust at ambient
    /// pressure `pressure` against local gravity `gravity`, at the start.
    pub fn twr(&self, pressure: f64, gravity: f64) -> f64 {
        self.thrust.output(1.0, pressure).0 / (self.mass.0 * gravity)
    }
}

/// The engines a prediction burns, lumped into one (the software's model,
/// D128): thrust and flow at a throttle setting, along one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thrust {
    /// Thrust at full throttle in vacuum (N), and its fall with the ambient
    /// pressure (m²: a fixed nozzle's exit area, linear in pressure).
    pub vac: f64,
    pub exit_area: f64,
    /// Mass flow at full throttle (kg/s).
    pub mdot: f64,
    /// The lowest setting above off.
    pub min_throttle: f64,
    /// The thrust's direction (body axes, unit).
    pub axis: DVec3,
}

impl Thrust {
    /// A vessel's main engines (one owner): for a craft of parts, the stage
    /// estimate of the engines its throttle runs (those selected, else all
    /// main engines; `estimate::lit`, the staging bar's walk) at full
    /// throttle in vacuum and at sea level, along their mean axis (none
    /// without parts' systems). Also the propellant they burn (kg; to the
    /// estimate's burnout; infinite if that is not measured, see
    /// [`LandingStart::propellant`]).
    pub fn of_vessel(vessel: &Vessel) -> (Self, f64) {
        let Some((def, _)) = vessel.ship_parts() else {
            let none = Thrust { vac: 0.0, exit_area: 0.0, mdot: 0.0, min_throttle: 0.0, axis: DVec3::Z };
            return (none, 0.0);
        };
        let engines = vessel.burn_engines();
        let (vac, sl) = (vessel.burn_estimate_at(&engines, 0.0), vessel.burn_estimate_at(&engines, P_SEA_LEVEL));
        let force =
            |b: &Option<crate::systems::estimate::Burn>| b.as_ref().and_then(|b| b.thrust).map_or(0.0, |t| t.value);
        let (f_vac, f_sl) = (force(&vac), force(&sl));
        let isp = vac.as_ref().map_or(0.0, |b| b.isp);
        let burnable = match vac.as_ref() {
            Some(b) => b.burnout_mass.map_or(f64::INFINITY, |m1| b.mass.value - m1.value),
            None => 0.0,
        };
        let thrust = Thrust {
            vac: f_vac,
            exit_area: (f_vac - f_sl).max(0.0) / P_SEA_LEVEL,
            mdot: if isp > 0.0 { f_vac / (isp * crate::vessel::G0) } else { 0.0 },
            min_throttle: engines.iter().map(|&e| def.engines[e as usize].data.throttle_min).fold(0.0, f64::max),
            axis: vessel.thrust_axis(),
        };
        (thrust, burnable.max(0.0))
    }

    /// Thrust (N) and mass flow (kg/s) at `throttle` and ambient pressure
    /// `p` (Pa).
    pub fn output(&self, throttle: f64, p: f64) -> (f64, f64) {
        let s = if throttle > 0.0 { throttle.clamp(self.min_throttle, 1.0) } else { 0.0 };
        (s * (self.vac - self.exit_area * p).max(0.0), s * self.mdot)
    }
}

/// What a prediction from `vessel` at mass `mass` (kg) burns: the
/// propellant its engines burn, their lumped thrust, and its mass and
/// centre of mass (see [`LandingStart`]).
pub fn propulsion_of(vessel: &Vessel, mass: f64) -> (f64, Thrust, (f64, DVec3)) {
    let (thrust, burnable) = Thrust::of_vessel(vessel);
    let spent = vessel.mass() - mass;
    ((burnable - spent).max(0.0), thrust, (mass, vessel.mass_props().com))
}

/// The surface body whose ground is nearest `r` (anchor-relative).
pub fn nearest_surface(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> Option<NodeId> {
    world
        .surfaces()
        .map(|s| {
            let p = s.physical.as_ref().expect("surfaces are physical");
            let d = r - snap.relative_r(s.node, anchor);
            (s.node, p.altitude_above_surface(p.rotation.to_fixed(Vec3::from_raw(d), snap.t)))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(n, _)| n)
}

/// How far a prediction looks ahead.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Seconds after the start.
    pub horizon: f64,
    pub max_steps: usize,
}

impl Limits {
    /// Two revolutions of the osculating orbit about the reference body if
    /// it is bound, else [`DEFAULT_HORIZON`] (at most that either way).
    pub fn two_orbits(world: &World, start: &LandingStart) -> Self {
        let snap = world.snapshot(start.t);
        let gm = world.source(start.body).map_or(0.0, |s| s.gm);
        let k = snap.relative(start.body, start.anchor);
        let (r, v) = ((start.r - k.r).length(), (start.v - k.v).length_squared());
        let energy = 0.5 * v - gm / r;
        let horizon = if energy < 0.0 {
            let a = -gm / (2.0 * energy);
            2.0 * std::f64::consts::TAU * (a * a * a / gm).sqrt()
        } else {
            DEFAULT_HORIZON
        };
        Limits { horizon: horizon.min(DEFAULT_HORIZON), max_steps: MAX_STEPS }
    }
}

/// Where the path meets the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub body: NodeId,
    pub t: Epoch,
    /// Body-fixed ground point under the centre of mass at impact.
    pub ground: Vec3<BodyFixed>,
    /// Geocentric latitude and longitude of the ground point (rad).
    pub lat: f64,
    pub lon: f64,
    /// Surface height there above the reference ellipsoid (m).
    pub height: f64,
    /// Surface-relative velocity at impact: along the local vertical (m/s,
    /// positive up), horizontal, and the total.
    pub v_vertical: f64,
    pub v_horizontal: f64,
    pub speed: f64,
}

impl Impact {
    /// The ground point relative to `anchor` at time `t` (inertial axes):
    /// where it is then, carried by the body's rotation.
    pub fn ground_at(&self, world: &World, t: Epoch, anchor: NodeId) -> DVec3 {
        let p = world.source(self.body).and_then(|s| s.physical.as_ref()).expect("a surface body");
        world.snapshot(t).relative_r(self.body, anchor) + p.rotation.to_inertial(self.ground, t).raw()
    }
}

/// The predicted path and where it ends.
#[derive(Clone, Debug, PartialEq)]
pub struct Descent {
    pub start: LandingStart,
    /// Accepted steps (local time from `start.t`, `start.anchor` frame).
    samples: Vec<StepSample>,
    pub impact: Option<Impact>,
}

impl Descent {
    /// The state `(r, v)` at local time `t` (within the path).
    fn state(&self, t: f64) -> (DVec3, DVec3) {
        // A path that ends where it starts (already touching) has one sample.
        if let [only] = self.samples.as_slice() {
            return (only.r, only.v);
        }
        let i = self.samples.partition_point(|s| s.t <= t).clamp(1, self.samples.len() - 1);
        let (a, b) = (&self.samples[i - 1], &self.samples[i]);
        if a.t == b.t {
            return (b.r, b.v);
        }
        hermite5(a, b, t.clamp(a.t, b.t))
    }

    /// Seconds from the start to the end of the computed path.
    pub fn duration(&self) -> f64 {
        self.samples.last().map_or(0.0, |s| s.t)
    }
}

/// Predicts the path from `start` until the lowest contact point meets a
/// surface, or `limits` run out.
pub fn predict_impact(world: &World, craft: &CraftParams, start: &LandingStart, limits: Limits) -> Descent {
    let model = Model::new(world, craft, start);
    let run = Run::from_start(start, start.throttle);
    let flight = model.fly(&run, false, limits.horizon, limits.max_steps);
    let impact = match flight.end {
        End::Surface { t, body } => {
            let last = flight.samples.last().expect("a contact sample");
            Some(model.impact(start.t.add_seconds(t), start.anchor, body, last.r, last.v))
        }
        _ => None,
    };
    Descent { start: *start, samples: flight.samples, impact }
}

/// A vessel's path through air that needs live flight, for its orbit line
/// (D089): the vessel's own force model, as [`predict_impact`].
#[derive(Clone, Debug, PartialEq)]
pub struct AirPath {
    /// The path, a finished segment the line samples (rule 4).
    pub segment: Segment,
    /// It ends where the air is clear for coasting again: a coast follows
    /// from its last sample.
    pub cleared: bool,
}

/// The path from `start` through the air by the vessel's own force model
/// (drag and lift from its aerodynamic bake against the air turning with
/// the body, at the assumed attitude), until the lowest contact point meets
/// a surface, or the air is clear for coasting by `boundary` (the vessel's
/// own hand-over rule, `vessel::limbo::may_coast`), or `limits` run out.
/// The ship clock is held at `proper_time` along it.
pub fn air_path(
    world: &World,
    craft: &CraftParams,
    start: &LandingStart,
    boundary: LiveBoundary,
    limits: Limits,
    proper_time: f64,
) -> AirPath {
    let model = Model::new(world, craft, start).until_clear(boundary);
    let flight = model.fly(&Run::from_start(start, start.throttle), false, limits.horizon, limits.max_steps);
    let (end, cleared) = match flight.end {
        End::Surface { body, .. } => (EndKind::Surface { body }, false),
        End::Clear => (EndKind::Horizon, true),
        End::Failed => (EndKind::Failed, false),
        _ => (EndKind::Horizon, false),
    };
    let mass = model.props(start.propellant).0;
    let segment = Segment::recorded(world, start.t, start.anchor, &flight.samples, end, mass, proper_time);
    AirPath { segment, cleared }
}

/// When to start braking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Braking {
    /// Ignition of the full-thrust burn.
    pub t_ignite: Epoch,
    /// Height of the centre of mass above the surface at ignition (m).
    pub ignite_height: f64,
    /// When the surface-relative speed reaches zero.
    pub t_stop: Epoch,
    /// Height of the lowest contact point above the surface then (m).
    pub stop_height: f64,
}

/// The latest ignition of a full-thrust burn against the surface-relative
/// velocity (thrust at ambient pressure, throttle limits, falling mass,
/// gravity, the body's rotation and drag) that stops the descent with the
/// lowest contact point `margin` metres above the surface: shooting from
/// the predicted path, bisecting the ignition time. `None` without an
/// impact, or if even igniting at the start does not stop in time.
pub fn braking_solution(world: &World, craft: &CraftParams, descent: &Descent, margin: f64) -> Option<Braking> {
    let impact = descent.impact?;
    let start = &descent.start;
    let t_end = impact.t.seconds_since(start.t);
    let mut braking = *start;
    braking.attitude = AssumedAttitude::SurfaceRetrograde;
    let model = Model::new(world, craft, &braking);
    let mdot = start.thrust.output(start.throttle, 0.0).1;
    // The stop height (lowest point above the ground) igniting at local
    // time `ti`; a crash counts as minus the contact speed.
    let shoot = |ti: f64| -> (f64, f64) {
        let (r, v) = descent.state(ti);
        let propellant = if start.infinite { start.propellant } else { (start.propellant - mdot * ti).max(0.0) };
        let run = Run { t0: start.t.add_seconds(ti), anchor: start.anchor, r, v, propellant, throttle: 1.0 };
        // Long enough to stop from any speed the craft can stop from.
        let horizon = (t_end - ti).max(0.0) + 3_600.0;
        let flight = model.fly(&run, true, horizon, MAX_STEPS);
        let last = *flight.samples.last().expect("a sample");
        match flight.end {
            End::Stopped { t } => (model.clearance_exact(&run, t, last.r, last.v), t),
            End::Surface { t, body } => {
                let hit = model.impact(run.t0.add_seconds(t), start.anchor, body, last.r, last.v);
                (-hit.speed, t)
            }
            _ => (f64::NEG_INFINITY, 0.0),
        }
    };
    let (h0, stop0) = shoot(0.0);
    if h0 < margin {
        return None;
    }
    let (mut lo, mut hi, mut at_lo) = (0.0, t_end, (h0, stop0));
    while hi - lo > IGNITION_PRECISION {
        let mid = 0.5 * (lo + hi);
        let s = shoot(mid);
        if s.0 >= margin {
            (lo, at_lo) = (mid, s);
        } else {
            hi = mid;
        }
    }
    let t_ignite = start.t.add_seconds(lo);
    let (r, _) = descent.state(lo);
    let snap = world.snapshot(t_ignite);
    let ignite_height = crate::forces::altitude_above(world, &snap, start.anchor, start.body, r).0;
    Some(Braking { t_ignite, ignite_height, t_stop: t_ignite.add_seconds(at_lo.1), stop_height: at_lo.0 })
}

/// Height of the lowest contact point above the reference body's surface
/// at the start, at the assumed attitude (m).
pub fn clearance(world: &World, craft: &CraftParams, start: &LandingStart) -> f64 {
    let model = Model::new(world, craft, start);
    model.clearance_exact(&Run::from_start(start, start.throttle), 0.0, start.r, start.v)
}

/// A vessel's motion relative to a body's surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceMotion {
    /// Height of the centre of mass above the surface (m).
    pub height: f64,
    /// Along the local vertical (radial), positive up (m/s).
    pub v_vertical: f64,
    pub v_horizontal: f64,
    /// Local gravity GM/d² (m/s²).
    pub gravity: f64,
    /// Ambient pressure (Pa).
    pub pressure: f64,
}

/// The motion of `(r, v)` (anchor-relative, at the snapshot's time)
/// relative to `body`'s surface.
pub fn surface_motion(
    world: &World,
    snap: &Snapshot,
    anchor: NodeId,
    body: NodeId,
    r: DVec3,
    v: DVec3,
) -> SurfaceMotion {
    let src = world.source(body).expect("a source");
    let p = src.physical.as_ref().expect("a physical body");
    let k = snap.relative(body, anchor);
    let d = r - k.r;
    let v_srf = v - k.v - p.rotation.omega(snap.t).raw().cross(d);
    let up = d.normalize();
    let v_vertical = v_srf.dot(up);
    SurfaceMotion {
        height: p.altitude_above_surface(p.rotation.to_fixed(Vec3::from_raw(d), snap.t)),
        v_vertical,
        v_horizontal: (v_srf - up * v_vertical).length(),
        gravity: src.gm / d.length_squared(),
        pressure: ambient_pressure(world, snap, anchor, r),
    }
}

/// A vessel's thrust-to-weight ratio now at full throttle of the engines
/// its throttle runs ([`Thrust::of_vessel`]), with thrust at ambient
/// pressure `pressure` against local gravity `gravity`.
pub fn twr(vessel: &Vessel, pressure: f64, gravity: f64) -> f64 {
    Thrust::of_vessel(vessel).0.output(1.0, pressure).0 / (vessel.mass() * gravity)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod thrust_tests;
