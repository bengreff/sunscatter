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
//! A tick ends the live phase when the vessel
//! * coasts: engine off and [`contact::LEAVE`] above the live height;
//! * crashes: a contact point closes faster than the impact limit (not in
//!   debug mode, D064);
//! * rests: [`contact::REST_TICKS`] ticks in a row touching with every speed
//!   below the rest thresholds and no input; it then freezes into `Landed`
//!   with its pose.

use super::attitude::{self, Actuators, Command, Gimbal};
use super::{Controls, Phase, Vessel, TICK};
use crate::contact::{self, Ground, Pose};
use crate::craft::{ContactKind, MassProps};
use crate::forces::{altitude_above, ambient_pressure, ActiveSources, ForceContext};
use crate::frame::NodeId;
use crate::rigid;
use crate::world::World;
use glam::{DQuat, DVec3};

/// How a contact tick ended early.
enum Hit {
    /// Destroyed at substep `k`: the state then and the impact.
    Crash { k: usize, r: DVec3, att: super::Attitude, impact: contact::Impact },
    /// The state stopped being finite.
    Failed,
}

/// A completed contact tick.
struct ContactTick {
    r: DVec3,
    v: DVec3,
    att: super::Attitude,
    touching: bool,
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
            if !engine_on && near.is_none_or(|(_, h)| h > live + contact::LEAVE) {
                self.start_coast(world);
                return;
            }
            self.chute_deployed |= controls.chute;
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
            let cmd = attitude::command(&self.attitude, &props.inertia, &mut self.control, controls, &act, TICK);
            let (r1, v1, touching) = match contact_body {
                None => {
                    let next = rigid::tick(&self.attitude, &props.inertia, cmd.torque, TICK);
                    let att = attitude::finish(next, &mut self.control, &cmd, true);
                    let thrust = att.q * cmd.thrust_dir * (out.thrust / props.mass);
                    // A tick that cannot be integrated (non-finite state) leaves the
                    // vessel where it is, like a failed coast segment.
                    let Some((r1, v1)) = self.integrate_tick(world, anchor, r, v, thrust) else {
                        (self.attitude, self.control) = before;
                        return;
                    };
                    self.attitude = att;
                    (r1, v1, false)
                }
                Some(body) => match self.contact_tick(world, anchor, body, (r, v), &cmd, out.thrust, &props) {
                    Ok(t) => {
                        self.attitude = attitude::finish(t.att, &mut self.control, &cmd, false);
                        (t.r, t.v, t.touching)
                    }
                    Err(Hit::Failed) => {
                        (self.attitude, self.control) = before;
                        return;
                    }
                    Err(Hit::Crash { k, r, att, impact }) => {
                        self.time = self.time.add_seconds(TICK / contact::SUBSTEPS as f64 * k as f64);
                        self.crash(world, anchor, body, r, att, impact);
                        return;
                    }
                },
            };
            if !self.debug {
                self.propellant = (self.propellant - out.mdot * TICK).max(0.0);
            }
            self.time = self.time.add_seconds(TICK);
            self.phase = Phase::Powered { anchor, r: r1, v: v1 };
            self.tick_accel = Some((v1 - v) / TICK);
            if let Some(body) = contact_body {
                let input = engine_on || controls.rotate != DVec3::ZERO;
                if self.rest(world, anchor, body, touching, input) {
                    return;
                }
            }
        }
    }

    /// One tick of contact substeps from `(r, v)` at the vessel's time with
    /// the command `cmd` and engine thrust `thrust` (N).
    #[allow(clippy::too_many_arguments)]
    fn contact_tick(
        &self,
        world: &World,
        anchor: NodeId,
        body: NodeId,
        (mut r, mut v): (DVec3, DVec3),
        cmd: &Command,
        thrust: f64,
        props: &MassProps,
    ) -> Result<ContactTick, Hit> {
        let h = TICK / contact::SUBSTEPS as f64;
        let active = ActiveSources::select(world, &world.snapshot(self.time), anchor, r);
        let drag = Some(self.drag());
        let mut att = self.attitude;
        let mut touching = false;
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
                return Err(Hit::Crash { k, r, att, impact });
            }
            touching = c.touching;
            let thrust_accel = att.q * cmd.thrust_dir * (thrust / props.mass);
            let ctx = ForceContext { world, anchor, active: &active, drag, thrust: thrust_accel };
            let a = ctx.accel_with(&snap, r, v) + c.force / props.mass;
            let torque = cmd.torque + att.q.inverse() * c.torque;
            if !(a.is_finite() && torque.is_finite()) {
                return Err(Hit::Failed);
            }
            v += a * h;
            r += v * h;
            att = rigid::tick(&att, &props.inertia, torque, h);
        }
        Ok(ContactTick { r, v, att, touching })
    }

    /// Destroyed by `impact` at `r` (anchor-relative) with attitude `att`.
    fn crash(
        &mut self,
        world: &World,
        anchor: NodeId,
        body: NodeId,
        r: DVec3,
        att: super::Attitude,
        impact: contact::Impact,
    ) {
        let ground = Ground::new(world, &world.snapshot(self.time), anchor, body);
        self.phase = Phase::Crashed {
            body,
            fixed: ground.fixed(r),
            speed: impact.speed,
            att_fixed: fixed_attitude(&ground, att.q),
            point: impact.point,
        };
        self.chute_deployed = false;
        self.tick_accel = None;
        self.rest_ticks = 0;
        self.sync_landed_attitude(world);
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
