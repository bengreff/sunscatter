//! A vessel: a craft (realism-1 §3) moving through phases. Translation is
//! that of its centre of mass; rotation is a rigid body ([`attitude`]).
//!
//! * `Landed`  — at rest, frozen in a body's rotating frame with its full
//!   pose (position and tilt): costs nothing at any warp.
//! * `Powered` — live: fixed 20 ms ticks with controls latched per tick
//!   (deterministic; limited to physics warp by the game), while thrusting
//!   or near a surface. Near a surface each tick runs ground-contact
//!   substeps ([`crate::contact`], [`live`]); at rest it freezes into
//!   `Landed`, and thrust or rotation input wakes a landed vessel.
//! * `Coasting` — on rails: a stored [`Trajectory`] of coasts and planned
//!   burns ([`FlightPlan`]) that is sampled at any warp. A coast ends
//!   [`live_height`](Vessel::live_height) above a surface; contact is flown live.
//! * `Crashed` — destroyed: a contact point hit too fast (D066), or
//!   overheated (D065).
//!
//! Inside an atmosphere a vessel is always live: aerodynamics and heating
//! act on its cells every tick ([`aerothermal`]). A coast ends where it
//! descends into an atmosphere; it starts only above every atmosphere.
//! Rotation input never breaks a coast: its drag (for predictions through
//! an atmosphere) is attitude-independent and gravity does not depend on
//! attitude, so translation and attitude are independent while unpowered.
//! A planned burn's thrust follows its direction law, not the vessel's
//! attitude. The skin and interior temperatures step on a fixed lattice
//! while coasting.
//!
//! The vessel carries its propellant (burned by the engine in powered ticks
//! and in planned burns) and the **debug mode** flag (D064): infinite
//! propellant, no overheating, infinite impact tolerance, all together.

mod aerothermal;
mod anchor;
mod attitude;
mod burn;
mod clock;
mod coast;
mod id;
mod live;
mod segment;
mod trajectory;

pub use aerothermal::{air_at, coast_sunlight, in_atmosphere, AeroTick, Air, VesselThermal, LATTICE, MAX_SKIP};
pub use coast::THERMAL_SOLVE_UNITS;

pub use anchor::preferred_anchor;
pub use attitude::{quat_from_rotvec, quat_z_to, rotvec_of, Attitude, AttitudeControl, RotationBase};
pub use burn::{
    prograde_normal_radial, BurnEnd, BurnLaw, BurnLimits, DirectionLaw, FlightPlan, PlanError, PlannedBurn, G0,
};
pub use clock::{landed_rate, ShipClock, LANDED_STEP};
pub use id::{VesselId, VesselIds};
pub use segment::{coast_tolerance, CoastStart, EndKind, Sample, Segment, SegmentEnd, SegmentKind};
pub use trajectory::Trajectory;

use crate::craft::{Craft, CraftParams, MassProps};
use crate::forces::{ambient_pressure, ActiveSources, DragModel, ForceContext};
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::integrate::{Dopri5, Dynamics, Tolerance};
use crate::math;
use crate::time::Epoch;
use crate::world::World;
use coast::Budget;
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
    Landed {
        body: NodeId,
        fixed: Vec3<BodyFixed>,
        att_fixed: DQuat,
    },
    Powered {
        anchor: NodeId,
        r: DVec3,
        v: DVec3,
    },
    Coasting {
        trajectory: Box<Trajectory>,
    },
    /// Destroyed (`cause`). The wreck stays where it was destroyed, fixed
    /// to `body` (the ground it hit, or the body whose air burned it up),
    /// in the pose it had.
    Crashed {
        body: NodeId,
        fixed: Vec3<BodyFixed>,
        att_fixed: DQuat,
        cause: Destruction,
    },
}

/// Why a vessel was destroyed.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Destruction {
    /// Contact point `point` (an index into `CraftParams::contacts`) closed
    /// at `speed` (m/s) along the ground normal (D066).
    Impact { speed: f64, point: u32 },
    /// A skin cell or an interior node above its limit at `temperature`
    /// (K) (D065).
    Overheat { at: crate::thermal::Overheat, temperature: f64 },
}

impl Destruction {
    /// One line for the player.
    pub fn describe(&self) -> String {
        match *self {
            Destruction::Impact { speed, .. } => format!("CRASHED at {speed:.0} m/s"),
            Destruction::Overheat { at: crate::thermal::Overheat::Cell(_), temperature } => {
                format!("BURNED UP: skin at {temperature:.0} K")
            }
            Destruction::Overheat { at: crate::thermal::Overheat::Node(_), temperature } => {
                format!("OVERHEATED: interior at {temperature:.0} K")
            }
        }
    }
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
    /// People aboard (0: an uncrewed probe, controlled from elsewhere with
    /// light delay, D034, D067).
    crew: u32,
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
    /// Live ticks in a row at rest on the ground ([`crate::contact::rest_ticks`]).
    #[serde(default)]
    rest_ticks: u32,
    /// The ship clock: proper time minus coordinate time (D012).
    clock: ShipClock,
    /// Skin and interior temperatures (D065).
    thermal: VesselThermal,
}

/// The forces of a live tick from `t0`, with the proper-time rate.
struct TickForces<'a> {
    ctx: ForceContext<'a>,
    t0: Epoch,
}

impl Dynamics for TickForces<'_> {
    fn accel(&self, t: f64, r: DVec3, v: DVec3) -> DVec3 {
        self.ctx.accel(self.t0.add_seconds(t), r, v)
    }

    fn accel_rate(&self, t: f64, r: DVec3, v: DVec3) -> (DVec3, f64) {
        self.ctx.accel_rate(&self.ctx.world.snapshot(self.t0.add_seconds(t)), r, v)
    }
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
        let crew = craft.spec.crew;
        let (cells, nodes) = (craft.design().cells.len(), craft.design().nodes());
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
            crew,
            debug: false,
            plan: FlightPlan::default(),
            tick_accel: None,
            control: AttitudeControl::default(),
            rest_ticks: 0,
            clock: ShipClock::at(t, 0.0),
            thermal: VesselThermal::uniform(cells, nodes, aerothermal::INITIAL_K, t),
        };
        v.sync_landed_attitude(world);
        if !v.holds_where_landed(world) {
            v.wake(world);
        }
        v
    }

    /// A vessel coasting from `(r, v)` relative to `anchor` at `t`.
    pub fn coasting(world: &World, id: VesselId, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3, craft: &Craft) -> Self {
        let crew = craft.spec.crew;
        let (cells, nodes) = (craft.design().cells.len(), craft.design().nodes());
        let craft = craft.params();
        let mut vessel = Vessel {
            id,
            propellant: craft.initial_propellant,
            craft,
            phase: Phase::Powered { anchor, r, v },
            time: t,
            attitude: Attitude { q: quat_z_to(r.normalize()), omega: DVec3::ZERO },
            chute_deployed: false,
            crew,
            debug: false,
            plan: FlightPlan::default(),
            tick_accel: None,
            control: AttitudeControl::default(),
            rest_ticks: 0,
            clock: ShipClock::at(t, 0.0),
            thermal: VesselThermal::uniform(cells, nodes, aerothermal::INITIAL_K, t),
        };
        // Inside an atmosphere it is flown live.
        if !in_atmosphere(world, &world.snapshot(t), anchor, r, aerothermal::LEAVE_ATMOSPHERE) {
            vessel.start_coast(world);
        }
        vessel
    }

    /// The vessel's stable identity.
    pub fn id(&self) -> VesselId {
        self.id
    }

    /// Proper time minus coordinate time, δ = τ − t (s), at the vessel's
    /// time: what the ship clock shows minus the game clock (D012).
    /// People aboard (0: an uncrewed probe).
    pub fn crew(&self) -> u32 {
        self.crew
    }

    /// Changes the crew count (0 makes the vessel an uncrewed probe).
    pub fn set_crew(&mut self, crew: u32) {
        self.crew = crew;
    }

    pub fn proper_time_offset(&self) -> f64 {
        self.clock.offset
    }

    /// The drag model in use (with the parachute's area once deployed).
    pub fn drag(&self) -> DragModel {
        let extra = if self.chute_deployed { self.craft.chute_cd_area } else { 0.0 };
        DragModel { cd_area: self.craft.cd_area + extra, mass: self.mass() }
    }

    /// Skin and interior temperatures at the vessel's thermal epoch.
    pub fn thermal(&self) -> &VesselThermal {
        &self.thermal
    }

    /// The hottest skin cell (K).
    pub fn max_skin_temperature(&self) -> f64 {
        self.thermal.max_skin()
    }

    /// The hottest interior node: its temperature (K) and centre (body
    /// axes, m).
    pub fn max_node_temperature(&self) -> (f64, DVec3) {
        let (t, i) = self.thermal.max_node();
        (t, self.craft.design().volume.nodes[i].centre)
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
            Phase::Landed { body, att_fixed, .. } | Phase::Crashed { body, att_fixed, .. } => (*body, *att_fixed),
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
    /// ephemeris end). Returns the time actually reached: coasts stop when
    /// `budget` work units are spent (the game then holds its clock back
    /// and continues next frame; the result is the same as without a
    /// budget). A unit is about one coast integration step (~7 µs): the
    /// integration may spend `budget` steps and the coast thermal lattice
    /// `budget` units (a lattice point one, a thermal solve
    /// [`THERMAL_SOLVE_UNITS`]). Live ticks are not budgeted.
    pub fn advance(&mut self, world: &World, target: Epoch, controls: &Controls, budget: usize) -> Epoch {
        let target = if target > world.end() { world.end() } else { target };
        let mut budget = Budget { steps: budget, thermal: budget };
        loop {
            let before = (self.time, std::mem::discriminant(&self.phase));
            match &self.phase {
                Phase::Landed { .. } => self.advance_landed(world, target, controls),
                Phase::Crashed { .. } => self.advance_grounded(world, target),
                Phase::Powered { .. } => self.advance_powered(world, target, controls),
                Phase::Coasting { .. } => self.advance_coasting(world, target, controls, &mut budget),
            }
            // Stop at the target, when neither time nor phase moved (waiting
            // for a segment, or less than one tick left), or out of budget
            // while coasting.
            let spent = budget.thermal == 0 && matches!(self.phase, Phase::Coasting { .. });
            if self.time >= target || (self.time, std::mem::discriminant(&self.phase)) == before || spent {
                return self.time;
            }
        }
    }

    fn advance_landed(&mut self, world: &World, target: Epoch, controls: &Controls) {
        if self.engine_running(controls) || controls.rotate != DVec3::ZERO {
            self.wake(world);
            return;
        }
        self.advance_grounded(world, target);
    }

    /// Landed or crashed: fixed to the body until `target`.
    fn advance_grounded(&mut self, world: &World, target: Epoch) {
        if let Phase::Landed { body, fixed, .. } | Phase::Crashed { body, fixed, .. } = self.phase {
            self.clock.advance_landed(world, body, fixed, target);
        }
        self.time = target;
        self.sync_landed_attitude(world);
    }

    /// Leaves `Landed` for live ticks from the landed state.
    fn wake(&mut self, world: &World) {
        let (anchor, r, v) = self.state(world);
        self.sync_landed_attitude(world);
        self.phase = Phase::Powered { anchor, r, v };
        self.tick_accel = None;
        self.control = AttitudeControl::default();
        self.rest_ticks = 0;
        self.clock.restart(self.time);
        self.thermal.restart(self.time);
    }

    /// Integrates one tick with a constant acceleration `thrust` (thrust
    /// and lift) and `drag` (adaptive substeps, exact end). Returns the
    /// state and the tick's proper-time increment; `None` if the
    /// integration fails (non-finite state or force).
    fn integrate_tick(
        &self,
        world: &World,
        (anchor, r, v): (NodeId, DVec3, DVec3),
        thrust: DVec3,
        drag: DragModel,
    ) -> Option<(DVec3, DVec3, f64)> {
        let snap = world.snapshot(self.time);
        let active = ActiveSources::select(world, &snap, anchor, r);
        let f = TickForces {
            ctx: ForceContext { world, anchor, active: &active, drag: Some(drag), thrust },
            t0: self.time,
        };
        let integ = Dopri5::new(Tolerance { h_max: TICK, ..coast_tolerance() });
        let mut st = integ.start(&f, 0.0, r, v, TICK);
        while st.t < TICK {
            integ.step(&f, &mut st, TICK).ok()?;
        }
        Some((st.r.value(), st.v.value(), st.x.value()))
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
            contact_height: self.live_height(),
            horizon: COAST_HORIZON,
            fixed_anchor: false,
            proper_time: self.clock.offset,
        };
        let trajectory = Trajectory::new(world, self.time, start, self.mass(), &self.plan, self.burn_limits());
        self.phase = Phase::Coasting { trajectory: Box::new(trajectory) };
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
                contact_height: self.live_height(),
                horizon: COAST_HORIZON,
                fixed_anchor: false,
                proper_time: self.clock.offset,
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
