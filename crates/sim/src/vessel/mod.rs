//! A vessel: a craft (realism-1 §3) moving through phases. Translation is
//! that of its centre of mass; rotation is a rigid body ([`attitude`]).
//!
//! * `Landed`  — at rest, frozen in a body's rotating frame with its full
//!   pose (position and tilt): costs nothing at any warp.
//! * `Powered` — live: fixed 20 ms ticks with controls latched per tick
//!   (deterministic; limited to physics warp by the game), while thrusting
//!   or near a surface. Near a surface each tick runs ground-contact
//!   substeps ([`crate::contact`], [`live`]); at rest and standing stably
//!   it freezes into `Landed`. Thrust, rotation input, a change of the
//!   attitude mode or of debug mode, or a pose that no longer holds (checked
//!   once after a load) wakes a landed vessel.
//! * `Coasting` — on rails: a stored [`Trajectory`] of coasts and planned
//!   burns ([`FlightPlan`]) that is sampled at any warp. A coast ends
//!   [`live_height`](Vessel::live_height) above a surface; contact is flown live.
//! * `Crashed` — a wreck at rest. A vessel is destroyed when a contact
//!   point hits too fast (D066) or it overheats (D065) ([`Vessel::destruction`]);
//!   engine and controls are cut, and it flies on until it comes to rest.
//!
//! Inside an atmosphere a vessel is always live: aerodynamics and heating
//! act on its cells every tick ([`aerothermal`]). A coast ends where it
//! descends into an atmosphere; it starts only above every atmosphere.
//! Rotation input never breaks a coast: its drag (for predictions through
//! an atmosphere) is attitude-independent and gravity does not depend on
//! attitude, so translation and attitude are independent while unpowered.
//! A planned burn is flown by the attitude (D075): the maneuver hold turns
//! the craft before ignition and the thrust follows the attitude
//! ([`flown`]). The skin and interior temperatures step on a fixed lattice
//! while coasting.
//!
//! The vessel carries its propellant (burned by the engine in powered ticks
//! and in planned burns) and the **debug mode** flag (D064): infinite
//! propellant, no overheating, infinite impact tolerance, all together.

mod aerothermal;
mod anchor;
mod attitude;
mod breakage;
pub mod bubble;
mod burn;
mod clock;
mod coast;
mod coast_heat;
mod create;
pub mod debris;
mod flown;
mod helm;
mod hold;
mod id;
mod live;
use live::{attitude_mode, Checked};
pub mod limbo;
pub mod pointing;
mod segment;
mod ship;
mod ship_loads;
mod ship_tick;
pub mod split;
mod systems;
mod trajectory;

pub use aerothermal::{
    air_at, coast_sunlight, in_atmosphere, AeroTick, Air, VesselThermal, LATTICE, LEAVE_ATMOSPHERE, MAX_SKIP,
};
pub use coast::{solve_units, POINT_UNITS, STEP_UNITS, SYSTEMS_UNITS};
pub use coast_heat::{coast_air, coast_suns, CoastSun};

pub use anchor::preferred_anchor;
pub use attitude::{quat_from_rotvec, quat_z_to, rotvec_of, Attitude, AttitudeControl, RotationBase};
pub use burn::{
    prograde_normal_radial, BurnEnd, BurnLaw, BurnLimits, DirectionLaw, FlightPlan, PlanError, PlannedBurn, G0,
};
pub use clock::{landed_rate, ShipClock, SiteClock, LANDED_STEP};
pub use flown::BurnCraft;
pub use helm::{fleet_controls, Helm};
pub use hold::{hold_direction, hold_reference, laplace_radius, primary, turn_lead};
pub use hold::{HoldInputs, HoldMode, SpeedReference, TargetState};
pub use id::{VesselId, VesselIds};
pub use segment::{coast_tolerance, CoastStart, EndKind, Sample, Segment, SegmentEnd, SegmentKind};
pub use split::{Breakup, SplitPlan};
pub use trajectory::Trajectory;

use crate::craft::{CraftParams, MassProps};
use crate::forces::{ambient_pressure, DragModel};
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::time::Epoch;
use crate::world::World;
use glam::{DMat3, DQuat, DVec3};

/// Physics tick for powered flight and attitude control (s).
pub const TICK: f64 = 0.02;
/// A vessel that goes live inside [`Vessel::advance_frame`] with more than
/// this (s) of the frame still ahead stops there (more than a physics-warp
/// frame: 4× of the longest 0.1 s frame).
pub const LIVE_STOP: f64 = 1.0;
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
    /// What SAS holds (D075).
    #[serde(default)]
    pub hold: HoldMode,
    /// The velocity prograde and retrograde follow (the navball's mode).
    #[serde(default)]
    pub speed: SpeedReference,
    /// The body prograde and retrograde are relative to, if named (a control
    /// choice, never physics). `None`: the automatic one, [`hold_reference`].
    #[serde(default)]
    pub reference: Option<NodeId>,
    /// What the target modes aim at.
    #[serde(default)]
    pub target: Option<TargetState>,
    /// Translation command in body axes (each in [-1, 1]): a craft of
    /// parts' thrusters push along it (`systems::rcs`; settling burns).
    #[serde(default)]
    pub translate: DVec3,
    /// The pilot's RCS mode (D111, `systems::rcs::pilot`).
    #[serde(default)]
    pub rcs: crate::systems::rcs::RcsMode,
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
    /// Destroyed (`cause`) and come to rest: the wreck is fixed to `body`
    /// in the pose it came to rest in.
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
    /// A part of a craft of parts failed (`sim::systems::failure`).
    Failed(crate::craft::state::PartFailure),
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
            Destruction::Failed(why) => format!("FAILED: {}", why.describe()),
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
    /// The parachutes (`sim::chute`).
    #[serde(default)]
    pub chute: crate::chute::ChuteState,
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
    /// Why the vessel was destroyed, if it was: it flies on with engine
    /// and controls cut until it rests (`Phase::Crashed`).
    #[serde(default)]
    destroyed: Option<Destruction>,
    /// The attitude mode ([`attitude_mode`]) when it came to rest: changing
    /// it wakes a landed vessel. `None`: whatever it is next.
    #[serde(default)]
    rest_mode: Option<Controls>,
    /// Whether a landed pose has been checked to hold since the vessel was
    /// built or loaded.
    #[serde(skip)]
    checked: Checked,
    /// The ship clock: proper time minus coordinate time (D012).
    clock: ShipClock,
    /// Skin and interior temperatures (D065).
    thermal: VesselThermal,
    /// Placeholders of the perturbed coasts ([`limbo`]).
    #[serde(default)]
    pub limbo: limbo::LimboSpec,
    /// The attitude was set from outside while coasting: the coast restarts
    /// from it at the next advance.
    #[serde(default)]
    repoint: bool,
    /// Internal systems on their lattice ([`crate::systems`]).
    #[serde(default)]
    pub systems: crate::systems::Systems,
    /// Parts that broke at a live tick, waiting for [`Vessel::split`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    breakup: Option<split::Breakup>,
    /// Held live in a 10 km bubble (D082, [`bubble`]).
    #[serde(default, skip_serializing_if = "crate::proximity::BubbleState::is_idle")]
    bubble: crate::proximity::BubbleState,
    /// A craft of catalog parts: its tanks, engines and systems
    /// ([`crate::craft::state`]); `propellant` is their consumables' total.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ship: Option<crate::craft::state::ShipState>,
    /// The other pieces of the split this vessel came from, while they
    /// still overlap it: no craft contact between them until they are
    /// clear (a stage's engine starts inside the interstage it leaves).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    clear_of: Vec<VesselId>,
    /// Its own controls, whichever vessel is active (D112), and what has
    /// reached it when uncrewed (D034; `helm`).
    #[serde(default)]
    helm: Helm,
}

/// Rotation matrix taking body-fixed axes of `body` to inertial axes at `t`.
fn body_to_inertial(world: &World, body: NodeId, t: Epoch) -> DQuat {
    let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical body").rotation;
    let col = |v: DVec3| rot.to_inertial(Vec3::from_raw(v), t).raw();
    DQuat::from_mat3(&DMat3::from_cols(col(DVec3::X), col(DVec3::Y), col(DVec3::Z))).normalize()
}

impl Vessel {
    /// The vessel's stable identity.
    pub fn id(&self) -> VesselId {
        self.id
    }

    /// People aboard (0: an uncrewed probe).
    pub fn crew(&self) -> u32 {
        self.crew
    }

    /// Changes the crew count (0 makes the vessel an uncrewed probe).
    pub fn set_crew(&mut self, crew: u32) {
        self.crew = crew;
    }

    /// Proper time minus coordinate time (TDB), δ = τ − t (s), at the
    /// vessel's time (D012); the game shows it against the origin's clock
    /// (D088).
    pub fn proper_time_offset(&self) -> f64 {
        self.clock.offset
    }

    /// Sets the ship clock to δ = `offset` at the vessel's time: a new
    /// vessel synchronised with the clock of the place it starts from
    /// (D088). A coast starts again from the same state, so only its clock
    /// changes.
    pub fn set_clock_offset(&mut self, world: &World, offset: f64) {
        self.clock = ShipClock::at(self.time, offset);
        if matches!(self.phase, Phase::Coasting { .. }) {
            self.start_coast(world);
        }
    }

    /// The drag model in use (with the open parachutes' area).
    pub fn drag(&self) -> DragModel {
        let extra = self.chute.cd_area(&self.craft.chute);
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

    /// The skin cell closest to its material's limit: its temperature and
    /// that limit (K).
    pub fn skin_nearest_limit(&self) -> (f64, f64) {
        self.thermal.skin_nearest_limit(&self.craft.design().skin_max)
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
        match self.ship_parts() {
            Some((def, state)) => {
                let (slugs, points) = state.contents(def);
                self.craft.mass.with_contents(&slugs, &points)
            }
            None => self.craft.mass.at(self.propellant),
        }
    }

    /// Height of the centre of mass above the ground when standing (m).
    pub fn contact_height(&self) -> f64 {
        self.mass_props().com.z - self.craft.bottom_z
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
        match self.phase {
            Phase::Coasting { .. } => self.start_coast(world),
            Phase::Landed { .. } => self.wake(world),
            _ => {}
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
        self.repoint = matches!(self.phase, Phase::Coasting { .. });
    }

    /// Sets the propellant (clamped to [0, capacity]). A coast is restarted
    /// from now with the new mass.
    pub fn set_propellant(&mut self, world: &World, kg: f64) {
        self.propellant = kg.clamp(0.0, self.craft.mass.capacity);
        self.sync_ship_to_propellant();
        if matches!(self.phase, Phase::Coasting { .. }) {
            self.start_coast(world);
        }
    }

    /// Whether the engine runs with these controls (a throttle, and
    /// propellant or debug mode).
    pub fn engine_running(&self, controls: &Controls) -> bool {
        if let Some(on) = self.ship_engines_on() {
            return on;
        }
        let engine = &self.craft.engine;
        engine.thrust_vac > 0.0 && engine.setting(controls.throttle) > 0.0 && (self.debug || self.propellant > 0.0)
    }

    /// Thrust acceleration (inertial axes, m/s²) the engine would give at
    /// `throttle` now, along the nose (display: the navball).
    pub fn thrust_accel(&self, world: &World, throttle: f64) -> DVec3 {
        let (anchor, r, _) = self.state(world);
        let p = ambient_pressure(world, &world.snapshot(self.time), anchor, r);
        if let Some(t) = self.ship_thrust(throttle, p) {
            return self.attitude.q * t.dir * (t.thrust / self.mass());
        }
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
        if self.ship.is_some() && !plan.burns.is_empty() {
            return Err(PlanError::PartsUnsupported);
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

    fn advance_landed(&mut self, world: &World, target: Epoch, controls: &Controls) {
        let mode = attitude_mode(controls);
        let changed = self.rest_mode.is_some_and(|m| m != mode);
        self.rest_mode = Some(mode);
        let holds = std::mem::replace(&mut self.checked, Checked(true)).0 || self.holds_where_landed(world);
        // Held by the clamps with its systems busy (engines spooling, the
        // release armed): ticks in place (`ship_tick`).
        if self.clamped() && self.ship_needs_ticks() {
            self.advance_held(world, target, controls);
            return;
        }
        // Held by the pad's clamps, it stays put whatever runs (the mount is
        // anchored to the ground until the clamps release).
        let free = !self.clamped();
        if free && (self.engine_running(controls) || controls.rotate != DVec3::ZERO || changed || !holds) {
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
        self.systems_grounded(world, target);
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

    /// Replaces the current phase with a coast starting now.
    fn start_coast(&mut self, world: &World) {
        let state = self.state(world);
        self.start_coast_from(world, state);
    }

    /// Replaces the current phase with a coast from `(anchor, r, v)` now.
    /// Its segments are perturbed ([`limbo`]); the first coast's attitude
    /// is unknown until the next advance finds the regime
    /// ([`pointing::Pointing::Mean`]).
    fn start_coast_from(&mut self, world: &World, state: (NodeId, DVec3, DVec3)) {
        self.start_coast_pointing(world, state, pointing::Pointing::Mean);
    }

    /// As [`Self::start_coast_from`], the first coast held by `pointing`.
    fn start_coast_pointing(&mut self, world: &World, (anchor, r, v): (NodeId, DVec3, DVec3), p: pointing::Pointing) {
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
        let seed = (self.attitude, self.control);
        let (plan, limits, craft) = (&self.plan, self.burn_limits(), BurnCraft::of(&self.craft));
        let model = Some(self.limbo_model(p));
        let trajectory = Trajectory::perturbed(world, self.time, start, self.mass(), plan, limits, craft, seed, model);
        self.phase = Phase::Coasting { trajectory: Box::new(trajectory) };
        self.repoint = false;
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
        self.coast_from(world, self.time, self.state(world), self.mass())
    }

    /// The coast this vessel would fly from `(anchor, r, v)` at `t` with
    /// `mass` (for predictions: after a path through the air). Not
    /// integrated yet; the caller extends it.
    pub fn coast_from(&self, world: &World, t: Epoch, (anchor, r, v): (NodeId, DVec3, DVec3), mass: f64) -> Segment {
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
        Segment::new_limbo(world, t, start, mass, self.limbo_model(pointing::Pointing::Mean))
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
mod ship_loads_tests;
#[cfg(test)]
mod ship_systems_tests;
#[cfg(test)]
mod ship_tests;
#[cfg(test)]
mod tests;
