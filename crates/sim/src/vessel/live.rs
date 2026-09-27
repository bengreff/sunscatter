//! Live ticks (the `Powered` phase): fixed 20 ms ticks while thrusting or
//! near a surface, with rigid-body ground contact (realism-1 §3e, D066).
//!
//! Away from surfaces a tick integrates the translation adaptively with a
//! constant thrust and the rotation with the control torque. Within
//! [`Vessel::live_height`] of a surface (so any contact point may be within
//! [`contact::NEAR`] of it) the tick is [`contact::SUBSTEPS`] fixed substeps
//! with the contact forces and torques of [`contact::forces`]: semi-implicit
//! Euler for the translation, RK4 for the rotation. Everything is on the
//! vessel's own tick lattice with controls latched per tick, so 60 fps and
//! one long jump give the same bits.
//!
//! Every tick also carries the aerodynamics and heating of
//! [`super::aerothermal`] (in an atmosphere: forces and moments on the
//! cells; everywhere: sunlight and radiation).
//!
//! A tick ends the live phase when the vessel
//! * coasts: engine off, [`contact::LEAVE`] above the live height and
//!   [`LEAVE_ATMOSPHERE`] above every atmosphere;
//! * crashes: a contact point closes faster than the impact limit (not in
//!   debug mode, D064);
//! * burns up: a skin cell or the interior exceeds its limit (not in debug
//!   mode, D064, D065);
//! * rests: [`contact::REST_TICKS`] ticks in a row touching with every speed
//!   below the rest thresholds and no input; it then freezes into `Landed`
//!   with its pose.

use super::aerothermal::{self, air_at, in_atmosphere, AeroTick, LEAVE_ATMOSPHERE};
use super::attitude::{self, Actuators, Command, Gimbal};
use super::{Controls, Destruction, Phase, Vessel, TICK};
use crate::contact::{self, Ground, Pose};
use crate::craft::{ContactKind, MassProps};
use crate::forces::{altitude_above, ambient_pressure, ActiveSources, DragModel, ForceContext};
use crate::frame::NodeId;
use crate::rigid;
use crate::world::World;
use glam::{DQuat, DVec3};

/// How a contact tick ended early.
enum Hit {
    /// Destroyed at substep `k`: the state then, the impact, and the
    /// proper-time increment until then.
    Crash { k: usize, r: DVec3, att: super::Attitude, impact: contact::Impact, delta: f64 },
    /// The state stopped being finite.
    Failed,
}

/// A completed contact tick.
struct ContactTick {
    r: DVec3,
    v: DVec3,
    att: super::Attitude,
    touching: bool,
    /// Proper-time increment over the tick.
    delta: f64,
}

impl Vessel {
    /// Height of the centre of mass above a surface below which the vessel
    /// flies live ticks with contact (m): its farthest contact point plus
    /// [`contact::NEAR`]. Coasts end here.
    pub fn live_height(&self) -> f64 {
        self.craft.contact_reach(self.propellant) + contact::NEAR
    }

    /// Becomes live (ticks) from `(anchor, r, v)` at the vessel's time.
    pub(super) fn go_live(&mut self, (anchor, r, v): (NodeId, DVec3, DVec3)) {
        self.phase = Phase::Powered { anchor, r, v };
        self.tick_accel = None;
        self.control.base = None;
        self.rest_ticks = 0;
    }

    /// The surface nearest the centre of mass at `r` (anchor-relative) at
    /// the vessel's time, and the height above it, if any is within 100 km.
    fn nearest_surface(&self, world: &World, anchor: NodeId, r: DVec3) -> Option<(NodeId, f64)> {
        let snap = world.snapshot(self.time);
        let mut best: Option<(NodeId, f64)> = None;
        for src in world.surfaces() {
            let radius = src.physical.as_ref().map_or(0.0, |p| p.radius_eq);
            if (r - snap.relative_r(src.node, anchor)).length() > radius + 100_000.0 {
                continue;
            }
            let (h, _) = altitude_above(world, &snap, anchor, src.node, r);
            if best.is_none_or(|(_, b)| h < b) {
                best = Some((src.node, h));
            }
        }
        best
    }

    /// Whether the vessel, landed where it is, holds there
    /// ([`contact::holds`] on its feet): scripted placements on ground too
    /// steep for its friction start live instead.
    pub(super) fn holds_where_landed(&self, world: &World) -> bool {
        let Phase::Landed { body, fixed, att_fixed } = self.phase else { return true };
        let p = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical");
        let com = self.mass_props().com;
        let mu = self.craft.contact.foot.friction;
        let support: Vec<_> = (self.craft.contacts.iter())
            .filter(|c| c.kind == ContactKind::Foot)
            .map(|c| {
                let x = fixed.raw() + att_fixed * (c.pos - com);
                let n = contact::ground_normal(
                    |y| p.altitude_above_surface(crate::frame::Vec3::from_raw(y)),
                    x,
                    contact::NORMAL_STEP,
                );
                (x, n, mu)
            })
            .collect();
        contact::holds(fixed.raw(), -fixed.raw().normalize(), &support)
    }

    pub(super) fn advance_powered(&mut self, world: &World, target: crate::time::Epoch, controls: &Controls) {
        while self.time.add_seconds(TICK) <= target {
            let Phase::Powered { anchor, r, v } = self.phase else { unreachable!() };
            let engine_on = self.engine_running(controls);
            let near = self.nearest_surface(world, anchor, r);
            let live = self.live_height();
            let contact_body = near.filter(|&(_, h)| h <= live).map(|(b, _)| b);
            let snap = world.snapshot(self.time);
            if !engine_on
                && near.is_none_or(|(_, h)| h > live + contact::LEAVE)
                && !in_atmosphere(world, &snap, anchor, r, LEAVE_ATMOSPHERE)
            {
                self.start_coast(world);
                return;
            }
            self.chute_deployed |= controls.chute;
            let props = self.mass_props();
            let engine = self.craft.engine;
            let p = ambient_pressure(world, &snap, anchor, r);
            let design = self.craft.design().clone();
            let air = air_at(world, &snap, anchor, r, v);
            let chute = self.chute_deployed.then_some((self.craft.chute_cd_area, self.craft.chute_mount));
            let aero = air.as_ref().and_then(|a| AeroTick::new(&design, a, props.com, chute));
            let (cd_area, lift) =
                aero.as_ref().map_or((0.0, DVec3::ZERO), |a| a.drag_and_lift(self.attitude.q, props.mass));
            let drag = DragModel { cd_area, mass: props.mass };
            let aero_torque = |q: DQuat| aero.as_ref().map_or(DVec3::ZERO, |a| a.torque(q));
            let sun_body = self.attitude.q.inverse() * crate::light::sunlight(world, &snap, anchor, r);
            let heat_tick = aero.as_ref().zip(air.as_ref());
            let att0 = self.attitude.q;
            let out = engine.tick_output(controls.throttle, p, self.propellant, TICK, self.debug);
            let gimbal = Gimbal {
                lever: engine.mount_pos - props.com,
                dir: engine.mount_dir,
                thrust: out.thrust,
                max_angle: engine.gimbal,
            };
            let act = Actuators { torque: self.craft.torque, gimbal: Some(gimbal) };
            let before = (self.attitude, self.control);
            let cmd = attitude::command(&self.attitude, &props.inertia, &mut self.control, controls, &act, TICK);
            let (r1, v1, touching, delta) = match contact_body {
                None => {
                    let next = rigid::tick_with(&self.attitude, &props.inertia, |q| cmd.torque + aero_torque(q), TICK);
                    let att = attitude::finish(next, &mut self.control, &cmd, aero.is_none());
                    let thrust = att.q * cmd.thrust_dir * (out.thrust / props.mass) + lift;
                    // A tick that cannot be integrated (non-finite state) leaves the
                    // vessel where it is, like a failed coast segment.
                    let Some((r1, v1, delta)) = self.integrate_tick(world, (anchor, r, v), thrust, drag) else {
                        (self.attitude, self.control) = before;
                        return;
                    };
                    self.attitude = att;
                    (r1, v1, false, delta)
                }
                Some(body) => match self.contact_tick(
                    world,
                    (anchor, body),
                    (r, v),
                    &cmd,
                    out.thrust,
                    &props,
                    (drag, lift, &aero_torque),
                ) {
                    Ok(t) => {
                        self.attitude = attitude::finish(t.att, &mut self.control, &cmd, false);
                        (t.r, t.v, t.touching, t.delta)
                    }
                    Err(Hit::Failed) => {
                        (self.attitude, self.control) = before;
                        return;
                    }
                    Err(Hit::Crash { k, r, att, impact, delta }) => {
                        self.time = self.time.add_seconds(TICK / contact::SUBSTEPS as f64 * k as f64);
                        self.clock.add(self.time, delta);
                        self.thermal.restart(self.time);
                        let cause = Destruction::Impact { speed: impact.speed, point: impact.point };
                        self.crash(world, anchor, body, r, att, cause);
                        return;
                    }
                },
            };
            let engine_heat = engine.heat(out.thrust, out.mdot);
            let propellant = if self.debug { self.propellant } else { (self.propellant - out.mdot * TICK).max(0.0) };
            aerothermal::live_step(
                &design,
                &mut self.thermal,
                heat_tick,
                att0,
                sun_body,
                engine_heat,
                propellant,
                TICK,
            );
            if !self.debug {
                self.propellant = (self.propellant - out.mdot * TICK).max(0.0);
            }
            self.time = self.time.add_seconds(TICK);
            self.clock.add(self.time, delta);
            self.phase = Phase::Powered { anchor, r: r1, v: v1 };
            self.tick_accel = Some((v1 - v) / TICK);
            if self.overheated(world, anchor, r1, air.map(|a| a.body)) {
                return;
            }
            if let Some(body) = contact_body {
                let input = engine_on || controls.rotate != DVec3::ZERO;
                if self.rest(world, anchor, body, touching, input) {
                    return;
                }
            }
        }
    }

    /// One tick of contact substeps from `(r, v)` at the vessel's time with
    /// the command `cmd`, engine thrust `thrust` (N) and the tick's
    /// aerodynamics (drag model, lift acceleration, torque at an attitude).
    #[allow(clippy::too_many_arguments)]
    fn contact_tick(
        &self,
        world: &World,
        (anchor, body): (NodeId, NodeId),
        (mut r, mut v): (DVec3, DVec3),
        cmd: &Command,
        thrust: f64,
        props: &MassProps,
        (drag, lift, aero_torque): (DragModel, DVec3, &dyn Fn(DQuat) -> DVec3),
    ) -> Result<ContactTick, Hit> {
        let h = TICK / contact::SUBSTEPS as f64;
        let active = ActiveSources::select(world, &world.snapshot(self.time), anchor, r);
        let drag = Some(drag);
        let mut att = self.attitude;
        let mut touching = false;
        let mut delta = 0.0;
        for k in 0..contact::SUBSTEPS {
            let snap = world.snapshot(self.time.add_seconds(h * k as f64));
            let ground = Ground::new(world, &snap, anchor, body);
            let pose =
                Pose { r, v, q: att.q, omega: att.omega, com: props.com, mass: props.mass, inertia: props.inertia };
            let c = contact::forces(
                &ground,
                &pose,
                &self.craft.contacts,
                &self.craft.contact,
                self.craft.impact_max_speed,
                h,
            );
            if let Some(impact) = c.impact.filter(|_| !self.debug) {
                return Err(Hit::Crash { k, r, att, impact, delta });
            }
            touching = c.touching;
            let thrust_accel = att.q * cmd.thrust_dir * (thrust / props.mass) + lift;
            let ctx = ForceContext { world, anchor, active: &active, drag, thrust: thrust_accel };
            let (a, rate) = ctx.accel_rate(&snap, r, v);
            let a = a + c.force / props.mass;
            let torque = cmd.torque + att.q.inverse() * c.torque + aero_torque(att.q);
            if !(a.is_finite() && torque.is_finite()) {
                return Err(Hit::Failed);
            }
            delta += rate * h;
            v += a * h;
            r += v * h;
            att = rigid::tick(&att, &props.inertia, torque, h);
        }
        Ok(ContactTick { r, v, att, touching, delta })
    }

    /// Destroyed (`cause`) at `r` (anchor-relative) with attitude `att`;
    /// the wreck is fixed to `body`.
    fn crash(
        &mut self,
        world: &World,
        anchor: NodeId,
        body: NodeId,
        r: DVec3,
        att: super::Attitude,
        cause: Destruction,
    ) {
        let ground = Ground::new(world, &world.snapshot(self.time), anchor, body);
        self.phase = Phase::Crashed { body, fixed: ground.fixed(r), att_fixed: fixed_attitude(&ground, att.q), cause };
        self.chute_deployed = false;
        self.tick_accel = None;
        self.rest_ticks = 0;
        self.sync_landed_attitude(world);
    }

    /// After a tick: destroyed if a temperature is above its limit (not in
    /// debug mode, D064). The wreck is fixed to the body whose air it was
    /// in (else the nearest surface's). Returns whether it was destroyed.
    fn overheated(&mut self, world: &World, anchor: NodeId, r: DVec3, air: Option<NodeId>) -> bool {
        if self.debug {
            return false;
        }
        let t = &self.craft.thermal;
        let Some(hot) = crate::thermal::check(&self.thermal.state, t.skin_max_k, t.internal_max_k) else {
            return false;
        };
        let temperature = match hot {
            crate::thermal::Overheat::Cell(i) => self.thermal.state.skin[i as usize],
            crate::thermal::Overheat::Node(j) => self.thermal.state.nodes[j as usize],
        };
        let cause = Destruction::Overheat { at: hot, temperature };
        let Some(body) = air.or_else(|| self.nearest_surface(world, anchor, r).map(|(b, _)| b)) else {
            // Nothing to fix the wreck to (deep space): it keeps flying.
            return false;
        };
        let att = self.attitude;
        self.crash(world, anchor, body, r, att, cause);
        true
    }

    /// Counts rest ticks after a contact tick; freezes into `Landed` (and
    /// returns true) once the vessel has rested long enough.
    fn rest(&mut self, world: &World, anchor: NodeId, body: NodeId, touching: bool, input: bool) -> bool {
        let Phase::Powered { r, v, .. } = self.phase else { return false };
        let ground = Ground::new(world, &world.snapshot(self.time), anchor, body);
        let v_rel = v - ground.velocity(r);
        let w_rel = self.attitude.omega - ground.omega;
        self.rest_ticks = contact::rest_ticks(self.rest_ticks, touching, input, v_rel, w_rel);
        if self.rest_ticks < contact::REST_TICKS {
            return false;
        }
        self.phase =
            Phase::Landed { body, fixed: ground.fixed(r), att_fixed: fixed_attitude(&ground, self.attitude.q) };
        self.chute_deployed = false;
        self.tick_accel = None;
        self.rest_ticks = 0;
        self.control = Default::default();
        self.sync_landed_attitude(world);
        true
    }
}

/// Body → body-fixed attitude of a vessel with body → inertial `q`.
fn fixed_attitude(ground: &Ground, q: DQuat) -> DQuat {
    (DQuat::from_mat3(&ground.to_fixed) * q).normalize()
}
