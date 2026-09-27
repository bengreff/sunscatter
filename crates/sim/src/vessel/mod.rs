//! A vessel: a craft (realism-1 §3) moving through phases. Translation is
//! that of its centre of mass; rotation is a rigid body ([`attitude`]).
//!
//! * `Landed`  — fixed in a body's rotating frame.
//! * `Powered` — thrust on: fixed 20 ms ticks with controls latched per tick
//!   (deterministic; limited to physics warp by the game).
//! * `Coasting` — on rails: a stored [`Trajectory`] of coasts and planned
//!   burns ([`FlightPlan`]) that is sampled at any warp.
//! * `Crashed` — hit a surface too fast.
//!
//! Rotation input never breaks a coast: neither drag (isotropic until the
//! cell aerodynamics of realism-1 §5) nor gravity depends on attitude, so
//! translation and attitude are independent while unpowered. A planned
//! burn's thrust follows its direction law, not the vessel's attitude.
//!
//! The vessel carries its propellant (burned by the engine in powered ticks
//! and in planned burns) and the **debug mode** flag (D064): infinite
//! propellant, no overheating, infinite impact tolerance, all together.

mod anchor;
mod attitude;
mod burn;
mod id;
mod segment;
mod trajectory;

pub use anchor::preferred_anchor;
pub use attitude::{quat_from_rotvec, quat_z_to, rotvec_of, Attitude, AttitudeControl, RotationBase};
pub use burn::{
    prograde_normal_radial, BurnEnd, BurnLaw, BurnLimits, DirectionLaw, FlightPlan, PlanError, PlannedBurn, G0,
};
pub use id::{VesselId, VesselIds};
pub use segment::{coast_tolerance, CoastStart, EndKind, Sample, Segment, SegmentEnd, SegmentKind};
pub use trajectory::Trajectory;

use crate::craft::{Craft, CraftParams, MassProps};
use crate::forces::{altitude_above, ambient_pressure, ActiveSources, DragModel, ForceContext};
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::integrate::{Dopri5, Tolerance};
use crate::math;
use crate::time::Epoch;
use crate::world::World;
use attitude::{advance_coast, control_tick, Actuators, Gimbal};
use glam::{DMat3, DQuat, DVec3};

/// Physics tick for powered flight and attitude control (s).
pub const TICK: f64 = 0.02;
/// Coast integration horizon (s): effectively unbounded within the game window.
pub const COAST_HORIZON: f64 = 60.0 * 365.25 * 86_400.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Controls {
    /// Throttle in [0, 1].
    pub throttle: f64,
    /// Rotation command in body axes (pitch = x, yaw = y, roll = z), each in
    /// [-1, 1]: a fraction of the torque authority.
    pub rotate: DVec3,
    pub sas: bool,
    pub chute: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Phase {
    Landed { body: NodeId, fixed: Vec3<BodyFixed>, att_fixed: DQuat },
    Powered { anchor: NodeId, r: DVec3, v: DVec3 },
    Coasting { trajectory: Box<Trajectory> },
    Crashed { body: NodeId, fixed: Vec3<BodyFixed>, speed: f64 },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vessel {
    /// Stable identity (never reused; see [`VesselIds`]).
    id: VesselId,
    /// What the vessel is made of (copied from its craft files).
    pub craft: CraftParams,
    pub phase: Phase,
    /// The vessel's current time (it may trail the game clock by < one tick).
    pub time: Epoch,
    pub attitude: Attitude,
    pub chute_deployed: bool,
    /// Propellant at `time` (kg).
    propellant: f64,
    /// Debug mode (D064): infinite propellant, no overheating, infinite
    /// impact tolerance.
    #[serde(default)]
    debug: bool,
    /// Planned burns, flown when coasting (at any warp).
    plan: FlightPlan,
    /// Mean acceleration over the last powered tick, relative to the anchor
    /// (display only: [`Vessel::state_at`] carries a powered vessel from its
    /// own time to the clock with it). `None` outside powered flight.
    #[serde(default)]
    pub tick_accel: Option<DVec3>,
    /// The coast's rotation lattice and the SAS hold ([`AttitudeControl`]).
    #[serde(default)]
    control: AttitudeControl,
}

/// Rotation matrix taking body-fixed axes of `body` to inertial axes at `t`.
fn body_to_inertial(world: &World, body: NodeId, t: Epoch) -> DQuat {
    let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical body").rotation;
    let col = |v: DVec3| rot.to_inertial(Vec3::from_raw(v), t).raw();
    DQuat::from_mat3(&DMat3::from_cols(col(DVec3::X), col(DVec3::Y), col(DVec3::Z))).normalize()
}

impl Vessel {
    /// A vessel standing upright on a body's surface at latitude/longitude (deg).
    pub fn landed_at(
        world: &World,
        id: VesselId,
        body: &str,
        lat_deg: f64,
        lon_deg: f64,
        t: Epoch,
        craft: &Craft,
    ) -> Self {
        let src = world.find(body).expect("known body");
        let p = src.physical.as_ref().expect("physical body");
        let deg = math::PI / 180.0;
        let craft = craft.params();
        let fixed = p.ground_point(lat_deg * deg, lon_deg * deg, craft.contact_height(craft.initial_propellant));
        let att_fixed = quat_z_to(fixed.raw().normalize());
        let mut v = Vessel {
            id,
            propellant: craft.initial_propellant,
            craft,
            phase: Phase::Landed { body: src.node, fixed, att_fixed },
            time: t,
            attitude: Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO },
            chute_deployed: false,
            debug: false,
            plan: FlightPlan::default(),
            tick_accel: None,
            control: AttitudeControl::default(),
        };
        v.sync_landed_attitude(world);
        v
    }

    /// A vessel coasting from `(r, v)` relative to `anchor` at `t`.
    pub fn coasting(world: &World, id: VesselId, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3, craft: &Craft) -> Self {
        let craft = craft.params();
        let mut vessel = Vessel {
            id,
            propellant: craft.initial_propellant,
            craft,
            phase: Phase::Powered { anchor, r, v },
            time: t,
            attitude: Attitude { q: quat_z_to(r.normalize()), omega: DVec3::ZERO },
            chute_deployed: false,
            debug: false,
            plan: FlightPlan::default(),
            tick_accel: None,
            control: AttitudeControl::default(),
        };
        vessel.start_coast(world);
        vessel
    }

    /// The vessel's stable identity.
    pub fn id(&self) -> VesselId {
        self.id
    }

    /// The drag model in use (with the parachute's area once deployed).
    pub fn drag(&self) -> DragModel {
        let extra = if self.chute_deployed { self.craft.chute_cd_area } else { 0.0 };
        DragModel { cd_area: self.craft.cd_area + extra, mass: self.mass() }
    }

    /// Mass at the vessel's time (kg): dry plus propellant.
    pub fn mass(&self) -> f64 {
        self.craft.mass.dry_mass + self.propellant
    }

    /// Propellant at the vessel's time (kg).
    pub fn propellant(&self) -> f64 {
        self.propellant
    }

    /// Mass, centre of mass (body axes) and inertia at the vessel's time.
    pub fn mass_props(&self) -> MassProps {
        self.craft.mass.at(self.propellant)
    }

    /// Height of the centre of mass above the ground when standing (m).
    pub fn contact_height(&self) -> f64 {
        self.craft.contact_height(self.propellant)
    }

    /// Debug mode (D064): infinite propellant, no overheating, infinite
    /// impact tolerance.
    pub fn debug(&self) -> bool {
        self.debug
    }

    /// Turns debug mode on or off. A coast is restarted from now (its burns
    /// depend on it; a planned burn in progress is dropped).
    pub fn set_debug(&mut self, world: &World, on: bool) {
        if self.debug == on {
            return;
        }
        self.debug = on;
        if matches!(self.phase, Phase::Coasting { .. }) {
            self.start_coast(world);
        }
    }

    /// Sets the attitude from outside the dynamics (scripts, tests): the
    /// coast's rotation restarts from it, and SAS holds it if it is not
    /// rotating. Landed vessels keep standing on the ground.
    pub fn set_attitude(&mut self, att: Attitude) {
        if matches!(self.phase, Phase::Landed { .. } | Phase::Crashed { .. }) {
            return;
        }
        self.attitude = att;
        self.control.base = None;
        self.control.hold = (att.omega == DVec3::ZERO).then_some(att.q);
    }

    /// Sets the propellant (clamped to [0, capacity]). A coast is restarted
    /// from now with the new mass.
    pub fn set_propellant(&mut self, world: &World, kg: f64) {
        self.propellant = kg.clamp(0.0, self.craft.mass.capacity);
        if matches!(self.phase, Phase::Coasting { .. }) {
            self.start_coast(world);
        }
    }

    /// Whether the engine runs with these controls (a throttle, and
    /// propellant or debug mode).
    fn engine_running(&self, controls: &Controls) -> bool {
        self.craft.engine.setting(controls.throttle) > 0.0 && (self.debug || self.propellant > 0.0)
    }

    /// Thrust acceleration (inertial axes, m/s²) the engine would give at
    /// `throttle` now, along the nose (display: the navball).
    pub fn thrust_accel(&self, world: &World, throttle: f64) -> DVec3 {
        let (anchor, r, _) = self.state(world);
        let p = ambient_pressure(world, &world.snapshot(self.time), anchor, r);
        let out = self.craft.engine.tick_output(throttle, p, self.propellant, TICK, self.debug);
        self.attitude.q * self.craft.engine.mount_dir * (out.thrust / self.mass())
    }

    /// Limits of planned burns: the propellant, unless debug mode.
    fn burn_limits(&self) -> BurnLimits {
        BurnLimits { dry_mass: self.craft.mass.dry_mass, infinite: self.debug }
    }

    /// The flight plan.
    pub fn plan(&self) -> &FlightPlan {
        &self.plan
    }

    /// Replaces the flight plan. Burns that have started (or been skipped)
    /// cannot change, nor can a changed burn ignite before the vessel's
    /// time. Trajectory segments before the first changed burn are kept bit
    /// for bit; the rest is rebuilt.
    pub fn set_plan(&mut self, world: &World, plan: FlightPlan) -> Result<(), PlanError> {
        if !plan.is_sorted() {
            return Err(PlanError::NotSorted);
        }
        let changed = self.plan.first_difference(&plan);
        if changed == usize::MAX {
            return Ok(());
        }
        let started = match &self.phase {
            Phase::Coasting { trajectory } => trajectory.burns_started(),
            _ => 0,
        };
        if changed < started {
            return Err(PlanError::Started);
        }
        if plan.burns[changed.min(plan.burns.len())..].iter().any(|b| b.t_start.seconds_since(self.time) < 0.0) {
            return Err(PlanError::InThePast);
        }
        if let Phase::Coasting { trajectory } = &mut self.phase {
            trajectory.replan(world, &plan, changed, self.time);
        }
        self.plan = plan;
        Ok(())
    }

    /// The burn being flown, if the vessel is in a planned burn.
    pub fn burn(&self) -> Option<&BurnLaw> {
        match &self.phase {
            Phase::Coasting { trajectory } => match &trajectory.current().kind {
                SegmentKind::Burn(law) => Some(law),
                SegmentKind::Coast => None,
            },
            _ => None,
        }
    }

    /// Landed and crashed vessels rotate rigidly with their body.
    fn sync_landed_attitude(&mut self, world: &World) {
        let (body, att_fixed) = match &self.phase {
            Phase::Landed { body, att_fixed, .. } => (*body, *att_fixed),
            Phase::Crashed { body, fixed, .. } => (*body, quat_z_to(fixed.raw().normalize())),
            _ => return,
        };
        self.control.base = None;
        let b2i = body_to_inertial(world, body, self.time);
        let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical").rotation;
        self.attitude = Attitude { q: (b2i * att_fixed).normalize(), omega: rot.omega(self.time).raw() };
    }

    /// Current (anchor, r, v) at the vessel's own time. Landed/crashed vessels
    /// report their body as anchor.
    pub fn state(&self, world: &World) -> (NodeId, DVec3, DVec3) {
        self.state_at(world, self.time)
    }

    /// (anchor, r, v) at `t`, for display at the game clock, which may lead
    /// the vessel's own time by up to a tick. Landed and crashed vessels and
    /// coasts are evaluated exactly at `t`. A powered vessel is carried from
    /// its last tick with that tick's mean acceleration (a polynomial in the
    /// stored tick, not an integration: rule 4); the next tick replaces it,
    /// off by at most the change in acceleration × dt²/2 (millimetres).
    pub fn state_at(&self, world: &World, t: Epoch) -> (NodeId, DVec3, DVec3) {
        match &self.phase {
            Phase::Landed { body, fixed, .. } | Phase::Crashed { body, fixed, .. } => {
                let rot = world.source(*body).and_then(|s| s.physical.as_ref()).expect("physical").rotation;
                let r = rot.to_inertial(*fixed, t).raw();
                (*body, r, rot.omega(t).raw().cross(r))
            }
            Phase::Powered { anchor, r, v } => {
                let dt = t.seconds_since(self.time);
                match self.tick_accel {
                    Some(a) if dt > 0.0 && dt <= 2.0 * TICK => {
                        (*anchor, *r + *v * dt + a * (0.5 * dt * dt), *v + a * dt)
                    }
                    _ => (*anchor, *r, *v),
                }
            }
            Phase::Coasting { trajectory } => {
                trajectory.eval(t).or_else(|| trajectory.eval(self.time)).expect("vessel time is inside its trajectory")
            }
        }
    }

    /// Advances the vessel towards `target` (never past it, nor past the
    /// ephemeris end). Returns the time actually reached (coasts can be
    /// limited by `max_coast_steps` of new integration work; the game then
    /// simply waits for the segment).
    pub fn advance(&mut self, world: &World, target: Epoch, controls: &Controls, max_coast_steps: usize) -> Epoch {
        let target = if target > world.end() { world.end() } else { target };
        loop {
            let before = (self.time, std::mem::discriminant(&self.phase));
            match &self.phase {
                Phase::Landed { .. } => self.advance_landed(world, target, controls),
                Phase::Crashed { .. } => {
                    self.time = target;
                    self.sync_landed_attitude(world);
                }
                Phase::Powered { .. } => self.advance_powered(world, target, controls),
                Phase::Coasting { .. } => self.advance_coasting(world, target, controls, max_coast_steps),
            }
            // Stop at the target, or when neither time nor phase moved (waiting
            // for a segment, or less than one tick left).
            if self.time >= target || (self.time, std::mem::discriminant(&self.phase)) == before {
                return self.time;
            }
        }
    }

    fn advance_landed(&mut self, world: &World, target: Epoch, controls: &Controls) {
        let Phase::Landed { body, fixed, .. } = self.phase else { unreachable!() };
        let src = world.source(body).expect("body");
        let r = fixed.length();
        let g = src.gm / (r * r);
        if self.engine_running(controls) && self.thrust_accel(world, controls.throttle).length() > g {
            // Lift off: become a powered vessel anchored to the body.
            let (anchor, r, v) = self.state(world);
            self.sync_landed_attitude(world);
            self.phase = Phase::Powered { anchor, r, v };
            self.tick_accel = None;
            self.control.base = None;
            return;
        }
        self.time = target;
        self.sync_landed_attitude(world);
    }

    fn advance_powered(&mut self, world: &World, target: Epoch, controls: &Controls) {
        while self.time.add_seconds(TICK) <= target {
            if !self.engine_running(controls) {
                self.start_coast(world);
                return;
            }
            self.chute_deployed |= controls.chute;
            let Phase::Powered { anchor, r, v } = self.phase else { unreachable!() };
            let props = self.mass_props();
            let engine = self.craft.engine;
            let p = ambient_pressure(world, &world.snapshot(self.time), anchor, r);
            let out = engine.tick_output(controls.throttle, p, self.propellant, TICK, self.debug);
            let gimbal = Gimbal {
                lever: engine.mount_pos - props.com,
                dir: engine.mount_dir,
                thrust: out.thrust,
                max_angle: engine.gimbal,
            };
            let act = Actuators { torque: self.craft.torque, gimbal: Some(gimbal) };
            let before = (self.attitude, self.control);
            let (att, dir) = control_tick(&self.attitude, &props.inertia, &mut self.control, controls, &act, TICK);
            self.attitude = att;
            let thrust = att.q * dir * (out.thrust / props.mass);
            // A tick that cannot be integrated (non-finite state) leaves the
            // vessel where it is, like a failed coast segment.
            let Some((r1, v1)) = self.integrate_tick(world, anchor, r, v, thrust) else {
                (self.attitude, self.control) = before;
                return;
            };
            if !self.debug {
                self.propellant = (self.propellant - out.mdot * TICK).max(0.0);
            }
            self.time = self.time.add_seconds(TICK);
            self.phase = Phase::Powered { anchor, r: r1, v: v1 };
            self.tick_accel = Some((v1 - v) / TICK);
            if self.check_contact(world) {
                return;
            }
        }
    }

    /// Integrates one tick with constant thrust (adaptive substeps, exact end).
    /// `None` if the integration fails (non-finite state or force).
    fn integrate_tick(
        &self,
        world: &World,
        anchor: NodeId,
        r: DVec3,
        v: DVec3,
        thrust: DVec3,
    ) -> Option<(DVec3, DVec3)> {
        let snap = world.snapshot(self.time);
        let active = ActiveSources::select(world, &snap, anchor, r);
        let ctx = ForceContext { world, anchor, active: &active, drag: Some(self.drag()), thrust };
        let t0 = self.time;
        let f = |t: f64, r: DVec3, v: DVec3| ctx.accel(t0.add_seconds(t), r, v);
        let integ = Dopri5::new(Tolerance { h_max: TICK, ..coast_tolerance() });
        let mut st = integ.start(&f, 0.0, r, v, TICK);
        while st.t < TICK {
            integ.step(&f, &mut st, TICK).ok()?;
        }
        Some((st.r.value(), st.v.value()))
    }

    /// Checks ground contact in powered flight; lands or crashes if touching.
    fn check_contact(&mut self, world: &World) -> bool {
        let Phase::Powered { anchor, r, v } = self.phase else { return false };
        let snap = world.snapshot(self.time);
        for src in world.surfaces() {
            let (alt, fixed) = altitude_above(world, &snap, anchor, src.node, r);
            if alt - self.contact_height() < 0.0 {
                let body_kin = snap.relative(src.node, anchor);
                let p = src.physical.as_ref().expect("physical");
                let d = r - body_kin.r;
                let v_ground = body_kin.v + p.rotation.omega(self.time).raw().cross(d);
                let rel_speed = (v - v_ground).length();
                // Moving upward away from the surface (e.g. at liftoff) is not contact.
                if (v - v_ground).dot(d) > 0.0 && rel_speed < self.safe_touchdown_speed() {
                    continue;
                }
                self.touch_down(world, src.node, fixed, rel_speed);
                return true;
            }
        }
        false
    }

    fn touch_down(&mut self, world: &World, body: NodeId, fixed: Vec3<BodyFixed>, speed: f64) {
        let p = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical");
        let dir = fixed.raw().normalize();
        let (lat, lon) = crate::terrain::lat_lon(dir);
        let on_ground = p.ground_point(lat, lon, self.contact_height());
        self.phase = if speed <= self.safe_touchdown_speed() {
            Phase::Landed { body, fixed: on_ground, att_fixed: quat_z_to(dir) }
        } else {
            Phase::Crashed { body, fixed: on_ground, speed }
        };
        self.chute_deployed = false;
        self.sync_landed_attitude(world);
    }

    /// Highest touchdown speed that is a landing (infinite in debug mode).
    fn safe_touchdown_speed(&self) -> f64 {
        if self.debug {
            f64::INFINITY
        } else {
            self.craft.impact_max_speed
        }
    }

    /// Replaces the current phase with a coast starting now.
    fn start_coast(&mut self, world: &World) {
        let state = self.state(world);
        self.start_coast_from(world, state);
    }

    /// Replaces the current phase with a coast from `(anchor, r, v)` now.
    fn start_coast_from(&mut self, world: &World, (anchor, r, v): (NodeId, DVec3, DVec3)) {
        let start = CoastStart {
            anchor,
            r,
            v,
            drag: Some(self.drag()),
            contact_height: self.contact_height(),
            horizon: COAST_HORIZON,
            fixed_anchor: false,
        };
        let trajectory = Trajectory::new(world, self.time, start, self.mass(), &self.plan, self.burn_limits());
        self.phase = Phase::Coasting { trajectory: Box::new(trajectory) };
    }

    fn advance_coasting(&mut self, world: &World, target: Epoch, controls: &Controls, max_steps: usize) {
        if self.engine_running(controls) {
            let (anchor, r, v) = self.state(world);
            self.phase = Phase::Powered { anchor, r, v };
            self.tick_accel = None;
            self.control.base = None;
            return;
        }
        if controls.chute && !self.chute_deployed {
            self.chute_deployed = true;
            self.start_coast(world); // drag changed: a new segment from now
        }
        let Phase::Coasting { trajectory } = &mut self.phase else { unreachable!() };
        trajectory.extend(world, &self.plan, target, max_steps);
        let computed = trajectory.computed_until();
        let at_end = target.seconds_since(computed) >= 0.0;
        let reach = if at_end { computed } else { target };
        // Attitude before pruning: control ticks read the inertia at epochs
        // after the vessel's time, which the trajectory still holds.
        let (model, dry) = (self.craft.mass, self.craft.mass.dry_mass);
        let mass_now = dry + self.propellant;
        let inertia_at = |e: Epoch| model.at(trajectory.mass_at(e).unwrap_or(mass_now) - dry).inertia;
        let times = (self.time, reach);
        let inertia_now = model.at(self.propellant).inertia;
        let torque = self.craft.torque;
        advance_coast(&mut self.attitude, &mut self.control, times, controls, torque, inertia_now, inertia_at);
        trajectory.prune_before(reach);
        self.propellant = (trajectory.mass_at(reach).expect("the vessel's time is in its trajectory") - dry).max(0.0);
        let last = trajectory.last();
        let contact = match last.end {
            Some(SegmentEnd { kind: EndKind::Surface { body }, .. }) if at_end => Some(body),
            _ => None,
        };
        self.time = reach;
        if let Some(body) = contact {
            let (anchor, r, v) = self.state(world);
            let snap = world.snapshot(self.time);
            let (_, fixed) = altitude_above(world, &snap, anchor, body, r);
            let p = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical");
            let kin = snap.relative(body, anchor);
            let v_ground = kin.v + p.rotation.omega(self.time).raw().cross(r - kin.r);
            self.touch_down(world, body, fixed, (v - v_ground).length());
        }
    }

    /// Extends the current coast (if any) until `until`, with at most
    /// `max_steps` new integration steps. The drawn trajectory *is* this
    /// segment, so looking ahead never changes where the vessel will go.
    pub fn extend_coast(&mut self, world: &World, until: Epoch, max_steps: usize) {
        if let Phase::Coasting { trajectory } = &mut self.phase {
            trajectory.extend(world, &self.plan, until, max_steps);
        }
    }

    /// The coast that would follow if thrust stopped now (for predictions
    /// while powered). Not integrated yet; the caller extends it.
    pub fn coast_from_now(&self, world: &World) -> Segment {
        let (anchor, r, v) = self.state(world);
        Segment::new(
            world,
            self.time,
            CoastStart {
                anchor,
                r,
                v,
                drag: Some(self.drag()),
                contact_height: self.contact_height(),
                horizon: COAST_HORIZON,
                fixed_anchor: false,
            },
        )
    }

    /// The current segment (coast or planned burn), if on rails.
    pub fn segment(&self) -> Option<&Segment> {
        self.trajectory().map(Trajectory::current)
    }

    /// The stored trajectory (current and future segments), if on rails.
    pub fn trajectory(&self) -> Option<&Trajectory> {
        match &self.phase {
            Phase::Coasting { trajectory } => Some(trajectory),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
