//! A vessel: point-mass translation plus attitude, moving through phases
//!
//! * `Landed`  — fixed in a body's rotating frame.
//! * `Powered` — thrust on: fixed 20 ms ticks with controls latched per tick
//!   (deterministic; limited to physics warp by the game).
//! * `Coasting` — no thrust: a stored [`Segment`] that is sampled at any warp.
//! * `Crashed` — hit a surface too fast.
//!
//! Rotation input never breaks a coast: in the prototype neither drag
//! (isotropic) nor gravity depends on attitude, so translation and attitude are
//! independent while unpowered.

mod anchor;
mod attitude;
mod segment;

pub use anchor::preferred_anchor;
pub use attitude::{quat_from_rotvec, quat_z_to, Attitude};
pub use segment::{coast_tolerance, CoastStart, EndKind, Sample, Segment, SegmentEnd};

use crate::forces::{altitude_above, ActiveSources, DragModel, ForceContext};
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::integrate::{Dopri5, Tolerance};
use crate::math;
use crate::time::Epoch;
use crate::world::World;
use glam::{DMat3, DQuat, DVec3};

/// Physics tick for powered flight and attitude control (s).
pub const TICK: f64 = 0.02;
/// Coast integration horizon (s): effectively unbounded within the game window.
pub const COAST_HORIZON: f64 = 60.0 * 365.25 * 86_400.0;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VesselParams {
    pub mass: f64,
    pub cd_area: f64,
    pub chute_cd_area: f64,
    pub max_thrust: f64,
    /// Maximum angular acceleration from reaction control (rad/s²).
    pub max_ang_accel: f64,
    /// Distance from the centre of mass to the bottom (contact) point (m).
    pub contact_height: f64,
    /// Highest touchdown speed relative to the ground that counts as landed.
    pub safe_touchdown_speed: f64,
}

impl VesselParams {
    /// The prototype block: 3 × 3 × 10 m, 20 t, magic 600 kN thrust.
    pub fn block() -> Self {
        VesselParams {
            mass: 20_000.0,
            cd_area: 7.0,
            // ~4× Apollo's three mains, for 20 t: ~7 m/s at sea level.
            chute_cd_area: 6_000.0,
            max_thrust: 600_000.0,
            max_ang_accel: 0.5,
            contact_height: 5.0,
            safe_touchdown_speed: 10.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Controls {
    /// Throttle in [0, 1].
    pub throttle: f64,
    /// Rotation command in body axes (pitch = x, yaw = y, roll = z), each in [-1, 1].
    pub rotate: DVec3,
    pub sas: bool,
    pub chute: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Phase {
    Landed { body: NodeId, fixed: Vec3<BodyFixed>, att_fixed: DQuat },
    Powered { anchor: NodeId, r: DVec3, v: DVec3 },
    Coasting { segment: Box<Segment> },
    Crashed { body: NodeId, fixed: Vec3<BodyFixed>, speed: f64 },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Vessel {
    pub params: VesselParams,
    pub phase: Phase,
    /// The vessel's current time (it may trail the game clock by < one tick).
    pub time: Epoch,
    pub attitude: Attitude,
    pub chute_deployed: bool,
    /// Mean acceleration over the last powered tick, relative to the anchor
    /// (display only: [`Vessel::state_at`] carries a powered vessel from its
    /// own time to the clock with it). `None` outside powered flight.
    #[serde(default)]
    pub tick_accel: Option<DVec3>,
}

/// Rotation matrix taking body-fixed axes of `body` to inertial axes at `t`.
fn body_to_inertial(world: &World, body: NodeId, t: Epoch) -> DQuat {
    let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical body").rotation;
    let col = |v: DVec3| rot.to_inertial(Vec3::from_raw(v), t).raw();
    DQuat::from_mat3(&DMat3::from_cols(col(DVec3::X), col(DVec3::Y), col(DVec3::Z))).normalize()
}

impl Vessel {
    /// A vessel standing upright on a body's surface at latitude/longitude (deg).
    pub fn landed_at(world: &World, body: &str, lat_deg: f64, lon_deg: f64, t: Epoch, params: VesselParams) -> Self {
        let src = world.find(body).expect("known body");
        let p = src.physical.as_ref().expect("physical body");
        let deg = math::PI / 180.0;
        let fixed = p.ground_point(lat_deg * deg, lon_deg * deg, params.contact_height);
        let att_fixed = quat_z_to(fixed.raw().normalize());
        let mut v = Vessel {
            params,
            phase: Phase::Landed { body: src.node, fixed, att_fixed },
            time: t,
            attitude: Attitude { q: DQuat::IDENTITY, omega: DVec3::ZERO },
            chute_deployed: false,
            tick_accel: None,
        };
        v.sync_landed_attitude(world);
        v
    }

    /// A vessel coasting from `(r, v)` relative to `anchor` at `t`.
    pub fn coasting(world: &World, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3, params: VesselParams) -> Self {
        let mut vessel = Vessel {
            params,
            phase: Phase::Powered { anchor, r, v },
            time: t,
            attitude: Attitude { q: quat_z_to(r.normalize()), omega: DVec3::ZERO },
            chute_deployed: false,
            tick_accel: None,
        };
        vessel.start_coast(world);
        vessel
    }

    /// The drag model in use (with the parachute's area once deployed).
    pub fn drag(&self) -> DragModel {
        let extra = if self.chute_deployed { self.params.chute_cd_area } else { 0.0 };
        DragModel { cd_area: self.params.cd_area + extra, mass: self.params.mass }
    }

    /// Landed and crashed vessels rotate rigidly with their body.
    fn sync_landed_attitude(&mut self, world: &World) {
        let (body, att_fixed) = match &self.phase {
            Phase::Landed { body, att_fixed, .. } => (*body, *att_fixed),
            Phase::Crashed { body, fixed, .. } => (*body, quat_z_to(fixed.raw().normalize())),
            _ => return,
        };
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
            Phase::Coasting { segment } => {
                let local = t.seconds_since(segment.t0);
                let own = self.time.seconds_since(segment.t0);
                let covered = local >= 0.0 && local <= segment.computed_until();
                segment.eval(if covered { local } else { own }).expect("vessel time is inside its segment")
            }
        }
    }

    /// Advances the vessel towards `target` (never past it). Returns the time
    /// actually reached (coasts can be limited by `max_coast_steps` of new
    /// integration work; the game then simply waits for the segment).
    pub fn advance(&mut self, world: &World, target: Epoch, controls: &Controls, max_coast_steps: usize) -> Epoch {
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
        if controls.throttle * self.params.max_thrust / self.params.mass > g {
            // Lift off: become a powered vessel anchored to the body.
            let (anchor, r, v) = self.state(world);
            self.sync_landed_attitude(world);
            self.phase = Phase::Powered { anchor, r, v };
            self.tick_accel = None;
            return;
        }
        self.time = target;
        self.sync_landed_attitude(world);
    }

    fn advance_powered(&mut self, world: &World, target: Epoch, controls: &Controls) {
        if controls.throttle <= 0.0 {
            self.start_coast(world);
            return;
        }
        while self.time.add_seconds(TICK) <= target {
            self.chute_deployed |= controls.chute;
            let attitude_before = self.attitude;
            self.attitude.control_tick(controls.rotate, controls.sas, self.params.max_ang_accel, TICK);
            let thrust =
                self.attitude.nose() * (controls.throttle.clamp(0.0, 1.0) * self.params.max_thrust / self.params.mass);
            let Phase::Powered { anchor, r, v } = self.phase else { unreachable!() };
            // A tick that cannot be integrated (non-finite state) leaves the
            // vessel where it is, like a failed coast segment.
            let Some((r1, v1)) = self.integrate_tick(world, anchor, r, v, thrust) else {
                self.attitude = attitude_before;
                return;
            };
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
            if alt - self.params.contact_height < 0.0 {
                let body_kin = snap.relative(src.node, anchor);
                let p = src.physical.as_ref().expect("physical");
                let d = r - body_kin.r;
                let v_ground = body_kin.v + p.rotation.omega(self.time).raw().cross(d);
                let rel_speed = (v - v_ground).length();
                // Moving upward away from the surface (e.g. at liftoff) is not contact.
                if (v - v_ground).dot(d) > 0.0 && rel_speed < self.params.safe_touchdown_speed {
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
        let on_ground = p.ground_point(lat, lon, self.params.contact_height);
        self.phase = if speed <= self.params.safe_touchdown_speed {
            Phase::Landed { body, fixed: on_ground, att_fixed: quat_z_to(dir) }
        } else {
            Phase::Crashed { body, fixed: on_ground, speed }
        };
        self.chute_deployed = false;
        self.sync_landed_attitude(world);
    }

    /// Replaces the current phase with a coast starting now.
    fn start_coast(&mut self, world: &World) {
        let (anchor, r, v) = self.state(world);
        let seg = Segment::new(
            world,
            self.time,
            CoastStart {
                anchor,
                r,
                v,
                drag: Some(self.drag()),
                contact_height: self.params.contact_height,
                horizon: COAST_HORIZON,
                fixed_anchor: false,
            },
        );
        self.phase = Phase::Coasting { segment: Box::new(seg) };
    }

    fn advance_coasting(&mut self, world: &World, target: Epoch, controls: &Controls, max_steps: usize) {
        if controls.throttle > 0.0 {
            let (anchor, r, v) = self.state(world);
            self.phase = Phase::Powered { anchor, r, v };
            self.tick_accel = None;
            return;
        }
        if controls.chute && !self.chute_deployed {
            self.chute_deployed = true;
            self.start_coast(world); // drag changed: a new segment from now
        }
        let Phase::Coasting { segment } = &mut self.phase else { unreachable!() };
        let want = target.seconds_since(segment.t0);
        let mut budget = max_steps;
        while segment.computed_until() < want && !segment.finished() && budget > 0 {
            let chunk = budget.min(256);
            segment.extend(world, chunk);
            budget -= chunk;
        }
        let reach = want.min(segment.computed_until());
        segment.prune_before(reach);
        let end = segment.end;
        let new_time = segment.t0.add_seconds(reach);
        self.advance_attitude(new_time, controls);
        self.time = new_time;
        if let Some(SegmentEnd { t, kind: EndKind::Surface { body } }) = end {
            if reach >= t {
                let (anchor, r, v) = self.state(world);
                let snap = world.snapshot(self.time);
                let (_, fixed) = altitude_above(world, &snap, anchor, body, r);
                let p = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical");
                let kin = snap.relative(body, anchor);
                let v_ground = kin.v + p.rotation.omega(self.time).raw().cross(r - kin.r);
                self.touch_down(world, body, fixed, (v - v_ground).length());
            }
        }
    }

    /// Attitude over a coast: control ticks while input/SAS damping needs them,
    /// then torque-free rotation for the rest.
    fn advance_attitude(&mut self, until: Epoch, controls: &Controls) {
        let mut t = self.time;
        while self.attitude.needs_ticks(controls.rotate, controls.sas) && t.add_seconds(TICK) <= until {
            self.attitude.control_tick(controls.rotate, controls.sas, self.params.max_ang_accel, TICK);
            t = t.add_seconds(TICK);
        }
        self.attitude = self.attitude.propagate_free(until.seconds_since(t));
    }

    /// Extends the current coast (if any) until `until`, with at most
    /// `max_steps` new integration steps. The drawn trajectory *is* this
    /// segment, so looking ahead never changes where the vessel will go.
    pub fn extend_coast(&mut self, world: &World, until: Epoch, max_steps: usize) {
        if let Phase::Coasting { segment } = &mut self.phase {
            let want = until.seconds_since(segment.t0);
            let mut budget = max_steps;
            while segment.computed_until() < want && !segment.finished() && budget > 0 {
                let chunk = budget.min(256);
                segment.extend(world, chunk);
                budget -= chunk;
            }
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
                contact_height: self.params.contact_height,
                horizon: COAST_HORIZON,
                fixed_anchor: false,
            },
        )
    }

    /// The current coast segment, if coasting (for drawing the trajectory).
    pub fn segment(&self) -> Option<&Segment> {
        match &self.phase {
            Phase::Coasting { segment } => Some(segment),
            _ => None,
        }
    }
}
