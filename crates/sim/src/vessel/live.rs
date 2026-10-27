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
//! * rests ([`contact::rest_ticks`]): [`contact::REST_TICKS`] ticks in a
//!   row touching, still and (intact) standing stably, with no input; it
//!   then freezes into `Landed` with its pose, or a wreck into `Crashed`.
//!
//! A contact point hitting faster than the impact limit, or a skin cell or
//! the interior above its limit, destroys the craft (not in debug mode,
//! D064, D065): its engine and controls are cut, and it flies on (falls,
//! bounces, tumbles) until it comes to rest.

use super::aerothermal::{self, air_at, AeroTick, Air, LEAVE_ATMOSPHERE};
use super::attitude::{self, Command};
use super::coast_tolerance;
use super::{Attitude, AttitudeControl, Controls, Destruction, FlightPlan, Phase, Vessel, TICK};
use crate::chute;
use crate::contact::{self, Ground, Pose};
use crate::craft::{CraftDesign, EngineOutput, MassProps};
use crate::forces::{altitude_above, ambient_pressure, ActiveSources, DragModel, ForceContext};
use crate::frame::NodeId;
use crate::integrate::{Dopri5, Dynamics, Tolerance};
use crate::math::Compensated;
use crate::rigid;
use crate::time::Epoch;
use crate::world::World;
use glam::{DQuat, DVec3};

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

/// A contact tick that could not be completed: the state stopped being
/// finite.
pub(super) struct Failed;

/// A live tick's start ([`Vessel::begin_tick`]): the state, the controls
/// latched into the engine output and the attitude command, and the tick's
/// air, all from the vessel at the tick's start.
pub(super) struct TickStart<'d> {
    pub(super) anchor: NodeId,
    pub(super) r: DVec3,
    pub(super) v: DVec3,
    /// The low part of the position a bubble's member carries between
    /// ticks (compensated summation, rule 3), and whether it does.
    lo: DVec3,
    compensate: bool,
    /// The tick's length (s; [`TICK`] but for a tick that aligns a vessel
    /// with a bubble's lattice) and its end.
    pub(super) dt: f64,
    pub(super) until: Epoch,
    engine_on: bool,
    /// The surface it may touch, if within its live height of one.
    pub(super) contact_body: Option<NodeId>,
    pub(super) props: MassProps,
    design: &'d CraftDesign,
    air: Option<Air>,
    aero: Option<AeroTick<'d>>,
    drag: DragModel,
    lift: DVec3,
    sun_body: DVec3,
    att0: DQuat,
    /// Ambient pressure (Pa).
    p: f64,
    out: EngineOutput,
    /// A craft of parts: what the tick draws from each tank and bottle.
    ship: Option<super::ship::ShipTick>,
    pub(super) cmd: Command,
    /// Attitude and control before the command (restored if the tick fails).
    pub(super) before: (Attitude, AttitudeControl),
}

impl TickStart<'_> {
    /// The aerodynamic torque (body axes) at attitude `q`.
    pub(super) fn aero_torque(&self, q: DQuat) -> DVec3 {
        self.aero.as_ref().map_or(DVec3::ZERO, |a| a.torque(q))
    }

    /// Whether the command was the only torque but contact (no air).
    pub(super) fn in_vacuum(&self) -> bool {
        self.aero.is_none()
    }
}

/// Where a tick took the vessel.
pub(super) struct TickEnd {
    pub(super) r: DVec3,
    /// The position's low part (zero unless compensated).
    pub(super) lo: DVec3,
    pub(super) v: DVec3,
    /// On the ground during the last substep.
    pub(super) touching: bool,
    /// Proper-time increment over the tick.
    pub(super) delta: f64,
}

/// A tick's contact substeps for one vessel ([`contact::SUBSTEPS`] of its
/// `dt`): ground contact if it is near a surface, and the craft contacts
/// a bubble adds (`super::bubble`). Semi-implicit Euler for the
/// translation, RK4 for the rotation.
pub(super) struct Substeps<'a> {
    vessel: &'a Vessel,
    world: &'a World,
    start: &'a TickStart<'a>,
    active: ActiveSources,
    h: f64,
    pub(super) r: DVec3,
    /// The compensated position of a bubble's member (`r` its value).
    r_c: Option<Compensated>,
    pub(super) v: DVec3,
    pub(super) att: Attitude,
    touching: bool,
    delta: f64,
    /// The impact that destroyed the craft (or its part) during the tick.
    impact: Option<contact::Impact>,
    /// Destroyed (now or before): engine and controls cut.
    dead: bool,
    whole: bool,
}

impl<'a> Substeps<'a> {
    pub(super) fn new(vessel: &'a Vessel, world: &'a World, start: &'a TickStart<'a>) -> Self {
        let active = ActiveSources::select(world, &world.snapshot(vessel.time), start.anchor, start.r);
        Substeps {
            vessel,
            world,
            start,
            active,
            h: start.dt / contact::SUBSTEPS as f64,
            r: start.r,
            r_c: start.compensate.then_some(Compensated { sum: start.r, comp: start.lo }),
            v: start.v,
            att: vessel.attitude,
            touching: false,
            delta: 0.0,
            impact: None,
            dead: vessel.destroyed.is_some(),
            whole: vessel.craft.design().parts.len() <= 1,
        }
    }

    /// The substep's length (s).
    pub(super) fn h(&self) -> f64 {
        self.h
    }

    /// The position's low part (zero unless compensated).
    pub(super) fn lo(&self) -> DVec3 {
        self.r_c.map_or(DVec3::ZERO, |c| low_part(&c))
    }

    /// Substep `k`, with `kick`, a contact force and torque about the
    /// centre of mass (inertial) from another craft at the substep's
    /// start, applied as an impulse before the drift.
    pub(super) fn step(&mut self, k: usize, kick: Option<(DVec3, DVec3)>) -> Result<(), Failed> {
        let (vessel, s, world, h) = (self.vessel, self.start, self.world, self.h);
        let props = &s.props;
        let snap = world.snapshot(vessel.time.add_seconds(h * k as f64));
        let ground = s.contact_body.map(|body| Ground::new(world, &snap, s.anchor, body));
        let (r, v, att) = (self.r, self.v, self.att);
        let pose = Pose { r, v, q: att.q, omega: att.omega, com: props.com, mass: props.mass, inertia: props.inertia };
        let craft = &vessel.craft;
        let c = ground.as_ref().map_or(contact::ContactForces::default(), |g| {
            contact::forces(g, &pose, &craft.contacts, &craft.contact, craft.impact_max_speed, h)
        });
        self.touching = c.touching;
        let thrust = if self.dead { 0.0 } else { s.out.thrust };
        let thrust_accel = att.q * s.cmd.thrust_dir * (thrust / props.mass) + s.lift;
        let ctx =
            ForceContext { world, anchor: s.anchor, active: &self.active, drag: Some(s.drag), thrust: thrust_accel };
        let (a, rate) = ctx.accel_rate(&snap, r, v);
        let a = a + c.force / props.mass;
        let command = if self.dead { DVec3::ZERO } else { s.cmd.torque };
        let torque = command + att.q.inverse() * c.torque + s.aero_torque(att.q);
        if !(a.is_finite() && torque.is_finite()) {
            return Err(Failed);
        }
        self.delta += rate * h;
        let mut v = v + a * h;
        let mut att = att;
        if let Some((force, tau)) = kick {
            if !(force.is_finite() && tau.is_finite()) {
                return Err(Failed);
            }
            v += force * (h / props.mass);
            att.omega += att.q * (props.inertia.inverse() * (att.q.inverse() * tau)) * h;
        }
        let r = match &mut self.r_c {
            Some(c) => {
                c.add(v * h);
                c.value()
            }
            None => r + v * h,
        };
        let att = rigid::tick(&att, &props.inertia, torque, h);
        (self.r, self.v, self.att) = (r, v, att);
        // An impact: the point past its free travel is still closing
        // too fast after the substep's contact force (its closing speed
        // at the start of the substep, the event's peak, is reported).
        if let (Some(im), Some(g)) =
            (c.impact.filter(|_| !vessel.debug && !self.dead && self.impact.is_none()), &ground)
        {
            let after = Pose { r, v, q: att.q, omega: att.omega, ..pose };
            let point = &craft.contacts[im.point as usize];
            if contact::closing_speed(g, &after, point) > craft.impact_max_speed {
                self.impact = Some(im);
                // A craft of parts loses the part (`breakage`), not its engine.
                self.dead = self.whole;
            }
        }
        Ok(())
    }

    /// Where the substeps took the vessel, its attitude (before SAS's
    /// bookkeeping) and the impact, if one destroyed the craft or a part.
    pub(super) fn end(self) -> (TickEnd, Attitude, Option<contact::Impact>) {
        let lo = self.lo();
        (TickEnd { r: self.r, lo, v: self.v, touching: self.touching, delta: self.delta }, self.att, self.impact)
    }
}

impl Vessel {
    /// Why the vessel was destroyed, if it was (a wreck still moving, or at
    /// rest in `Phase::Crashed`).
    pub fn destruction(&self) -> Option<Destruction> {
        match self.phase {
            Phase::Crashed { cause, .. } => Some(cause),
            _ => self.destroyed,
        }
    }

    /// Destroyed by `cause`: engine and controls are cut (the flight plan
    /// and the parachute go with them); it flies on until it rests.
    pub(super) fn destroy(&mut self, cause: Destruction) {
        self.destroyed = Some(cause);
        self.chute = Default::default();
        self.plan = FlightPlan::default();
        self.control = AttitudeControl::default();
    }

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

    /// Integrates one tick of `dt` with a constant acceleration `thrust`
    /// (thrust and lift) and `drag` (adaptive substeps, exact end), from a
    /// position with low part `lo` (zero but for a bubble's members).
    /// Returns the position, its low part, the velocity and the tick's
    /// proper-time increment; `None` if the integration fails (non-finite
    /// state or force).
    fn integrate_tick(
        &self,
        world: &World,
        (anchor, r, v): (NodeId, DVec3, DVec3),
        lo: DVec3,
        (thrust, drag): (DVec3, DragModel),
        dt: f64,
    ) -> Option<(DVec3, DVec3, DVec3, f64)> {
        let snap = world.snapshot(self.time);
        let active = ActiveSources::select(world, &snap, anchor, r);
        let f = TickForces {
            ctx: ForceContext { world, anchor, active: &active, drag: Some(drag), thrust },
            t0: self.time,
        };
        let integ = Dopri5::new(Tolerance { h_max: dt, ..coast_tolerance() });
        let mut st = integ.start(&f, 0.0, r, v, dt);
        st.r.comp += lo;
        while st.t < dt {
            integ.step(&f, &mut st, dt).ok()?;
        }
        Some((st.r.value(), low_part(&st.r), st.v.value(), st.x.value()))
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

    /// Whether the vessel, landed where it is, holds there: [`contact::holds`]
    /// on its contact points within [`contact::SUPPORT_CLEARANCE`] of the
    /// ground. Scripted placements on ground too steep for its friction,
    /// and saved poses that no longer hold, go live instead.
    pub(super) fn holds_where_landed(&self, world: &World) -> bool {
        let Phase::Landed { body, .. } = self.phase else { return true };
        let (anchor, r, v) = self.state(world);
        let ground = Ground::new(world, &world.snapshot(self.time), anchor, body);
        let props = self.mass_props();
        let pose = Pose {
            r,
            v,
            q: self.attitude.q,
            omega: self.attitude.omega,
            com: props.com,
            mass: props.mass,
            inertia: props.inertia,
        };
        let c = &self.craft;
        let support = contact::support(&ground, &pose, &c.contacts, &c.contact, contact::SUPPORT_CLEARANCE);
        contact::holds(r, -(r - ground.center).normalize(), &support, 0.0)
    }

    pub(super) fn advance_powered(&mut self, world: &World, target: crate::time::Epoch, controls: &Controls) {
        while self.time.add_seconds(TICK) <= target {
            // Orders due by this tick's start (a craft of parts).
            self.apply_orders(world);
            if self.breakup.is_some() {
                return;
            }
            let design = self.craft.design().clone();
            let until = self.time.add_seconds(TICK);
            let Some(start) = self.begin_tick(world, controls, &design, (TICK, until)) else {
                self.start_coast(world);
                return;
            };
            let Some(end) = self.fly_tick(world, &start) else { return };
            if self.finish_tick(world, start, end, controls) {
                return;
            }
        }
    }

    /// Whether a live vessel at `(anchor, r, v)` with the engine `engine_on`
    /// and the nearest surface `near` (body, height) would return to a coast
    /// now: engine off, [`contact::LEAVE`] above its live height and out of
    /// air that needs live flight (the live boundary).
    fn coasts(
        &self,
        world: &World,
        snap: &crate::ephem::Snapshot,
        (anchor, r, v): (NodeId, DVec3, DVec3),
        engine_on: bool,
        near: Option<(NodeId, f64)>,
    ) -> bool {
        let live = self.live_height();
        !engine_on
            && near.is_none_or(|(_, h)| h > live + contact::LEAVE)
            && super::limbo::may_coast(&self.limbo.boundary, world, snap, anchor, (r, v), LEAVE_ATMOSPHERE)
    }

    /// Whether a live vessel would return to a coast now with `controls`
    /// (it is live for its own reasons otherwise: thrust, air, the ground).
    pub(super) fn would_coast(&self, world: &World, controls: &Controls) -> bool {
        let Phase::Powered { anchor, r, v } = self.phase else { return true };
        let near = self.nearest_surface(world, anchor, r);
        self.coasts(world, &world.snapshot(self.time), (anchor, r, v), self.engine_running(controls), near)
    }

    /// Starts a live tick of `dt` ending at `until` with `controls`: the
    /// parachutes step, the air and the attitude command are found. `None`:
    /// the vessel returns to a coast instead (never while a bubble holds it
    /// live, D082).
    pub(super) fn begin_tick<'d>(
        &mut self,
        world: &World,
        controls: &Controls,
        design: &'d CraftDesign,
        (dt, until): (f64, Epoch),
    ) -> Option<TickStart<'d>> {
        let Phase::Powered { anchor, r, v } = self.phase else { unreachable!() };
        let engine_on = self.engine_running(controls) || self.ship_needs_ticks();
        let near = self.nearest_surface(world, anchor, r);
        let live = self.live_height();
        let contact_body = near.filter(|&(_, h)| h <= live).map(|(b, _)| b);
        let snap = world.snapshot(self.time);
        if !self.bubble.member && self.coasts(world, &snap, (anchor, r, v), engine_on, near) {
            return None;
        }
        let props = self.mass_props();
        let p = ambient_pressure(world, &snap, anchor, r);
        let air = air_at(world, &snap, anchor, r, v);
        if self.destroyed.is_none() {
            let speed = air.as_ref().map_or(0.0, |a| a.wind.length());
            let q = air.as_ref().map_or(0.0, |a| 0.5 * a.rho * speed * speed);
            let height = near.map_or(f64::INFINITY, |(_, h)| h);
            let flow = chute::Flow { q, speed, height };
            self.chute.step(&self.craft.chute, controls.chute, flow, dt, self.debug);
        }
        let chute_area = self.chute.cd_area(&self.craft.chute);
        let chute = (chute_area > 0.0).then_some((chute_area, self.craft.chute.mount));
        let aero = (air.as_ref().and_then(|a| AeroTick::new(design, a, props.com, chute)))
            .map(|t| t.with_layer(self.attitude.q, &self.thermal.state.skin));
        let (cd_area, lift) =
            aero.as_ref().map_or((0.0, DVec3::ZERO), |a| a.drag_and_lift(self.attitude.q, props.mass));
        let drag = DragModel { cd_area, mass: props.mass };
        let sun_body = self.attitude.q.inverse() * crate::light::sunlight(world, &snap, anchor, r);
        let att0 = self.attitude.q;
        let (out, act, ship) = self.actuators(controls.throttle, p, dt, &props);
        let before = (self.attitude, self.control);
        let aim = self.aim(world, &snap, (anchor, r, v), controls);
        let cmd = attitude::command(&self.attitude, &props.inertia, &mut self.control, controls, &act, &aim, dt);
        let compensate = self.bubble.member;
        Some(TickStart {
            anchor,
            r,
            v,
            lo: if compensate { self.bubble.lo } else { DVec3::ZERO },
            compensate,
            dt,
            until,
            engine_on,
            contact_body,
            props,
            design,
            air,
            aero,
            drag,
            lift,
            sun_body,
            att0,
            p,
            out,
            ship,
            cmd,
            before,
        })
    }

    /// Flies a tick on its own: away from surfaces the translation
    /// adaptively with the tick's constant thrust and the rotation with the
    /// command and the air's torque; near one, contact substeps. `None` if
    /// the tick cannot be integrated (non-finite state): the vessel stays
    /// where it is, like a failed coast segment.
    pub(super) fn fly_tick(&mut self, world: &World, s: &TickStart) -> Option<TickEnd> {
        let Some(_) = s.contact_body else {
            let torque = |q: DQuat| s.cmd.torque + s.aero_torque(q);
            let next = rigid::tick_with(&self.attitude, &s.props.inertia, torque, s.dt);
            let att = attitude::finish(next, &mut self.control, &s.cmd, s.aero.is_none());
            let thrust = att.q * s.cmd.thrust_dir * (s.out.thrust / s.props.mass) + s.lift;
            let state = (s.anchor, s.r, s.v);
            let Some((r1, lo, v1, delta)) = self.integrate_tick(world, state, s.lo, (thrust, s.drag), s.dt) else {
                (self.attitude, self.control) = s.before;
                return None;
            };
            self.attitude = att;
            let lo = if s.compensate { lo } else { DVec3::ZERO };
            return Some(TickEnd { r: r1, lo, v: v1, touching: false, delta });
        };
        let mut sub = Substeps::new(self, world, s);
        let ok = (0..contact::SUBSTEPS).all(|k| sub.step(k, None).is_ok());
        let (end, att, impact) = sub.end();
        if !ok {
            (self.attitude, self.control) = s.before;
            return None;
        }
        self.attitude = attitude::finish(att, &mut self.control, &s.cmd, false);
        if let Some(im) = impact {
            self.impacted(im);
        }
        Some(end)
    }

    /// Ends a tick: propellant and temperatures over it, the new state and
    /// time, the limits (D065, D066, D083) and systems, and rest on the
    /// ground. Returns whether the vessel stopped (it broke up or came to
    /// rest).
    pub(super) fn finish_tick(&mut self, world: &World, s: TickStart, end: TickEnd, controls: &Controls) -> bool {
        let TickStart {
            anchor,
            r,
            v,
            dt,
            p,
            before,
            until,
            engine_on,
            contact_body,
            design,
            air,
            aero,
            cmd,
            out,
            att0,
            sun_body,
            ship,
            ..
        } = s;
        let TickEnd { r: r1, lo, v: v1, touching, delta } = end;
        let (engine_heat, propellant, effects) = match &ship {
            Some(tick) => {
                let m = super::ship_tick::TickMotion { anchor, dt, r0: r, r1, v0: v, v1, omega0: before.0.omega, p };
                let (left, effects) = self.ship_after_tick(world, tick, cmd.rcs, &m, controls.throttle);
                (tick.heat, left, effects)
            }
            None => {
                let engine = self.craft.engine;
                let mdot = out.mdot + cmd.rcs.dot(self.craft.rcs_flow);
                let mdot = mdot + self.systems.consumables.boil_off;
                let left = if self.debug { self.propellant } else { (self.propellant - mdot * dt).max(0.0) };
                (engine.heat(out.thrust, out.mdot), left, Vec::new())
            }
        };
        let heat_tick = aero.as_ref().zip(air.as_ref());
        aerothermal::live_step(design, &mut self.thermal, heat_tick, att0, sun_body, engine_heat, propellant, dt);
        self.propellant = propellant;
        self.time = until;
        self.clock.add(self.time, delta);
        self.phase = Phase::Powered { anchor, r: r1, v: v1 };
        if self.bubble.member {
            self.bubble.lo = lo;
        }
        self.tick_accel = Some((v1 - v) / dt);
        if ship.is_some() {
            self.after_command(world, &effects);
        }
        let flow = aero.as_ref().map(|a| a.flow(att0));
        self.check_limits(world, flow.as_ref(), contact_body.map(|b| (anchor, b)));
        self.systems_after_tick(world);
        if self.breakup.is_some() {
            return true;
        }
        if let Some(body) = contact_body {
            let input = engine_on || controls.rotate != DVec3::ZERO;
            if self.rest(world, anchor, body, touching, input, controls) {
                return true;
            }
        }
        false
    }

    /// After a tick: destroyed if a temperature is above its limit (not in
    /// debug mode, D064).
    pub(super) fn check_overheat(&mut self) {
        if self.debug || self.destroyed.is_some() {
            return;
        }
        let (design, t) = (self.craft.design(), &self.craft.thermal);
        let Some(hot) = crate::thermal::check(&self.thermal.state, &design.skin_max, t.internal_max_k) else {
            return;
        };
        let temperature = match hot {
            crate::thermal::Overheat::Cell(i) => self.thermal.state.skin[i as usize],
            crate::thermal::Overheat::Node(j) => self.thermal.state.nodes[j as usize],
        };
        self.destroy(Destruction::Overheat { at: hot, temperature });
    }

    /// Counts rest ticks after a contact tick; freezes into `Landed` (a
    /// wreck into `Crashed`) and returns true once the vessel has rested
    /// long enough. `controls` are remembered: changing the attitude mode
    /// wakes a landed vessel.
    fn rest(
        &mut self,
        world: &World,
        anchor: NodeId,
        body: NodeId,
        touching: bool,
        input: bool,
        controls: &Controls,
    ) -> bool {
        let Phase::Powered { r, v, .. } = self.phase else { return false };
        let ground = Ground::new(world, &world.snapshot(self.time), anchor, body);
        let props = self.mass_props();
        let (v_rel, w_rel) = (v - ground.velocity(r), self.attitude.omega - ground.omega);
        let kinetic = contact::kinetic_per_mass(props.mass, &props.inertia, self.attitude.q, v_rel, w_rel);
        // Stability is checked only when the rest could count (it needs
        // the support points' ground normals).
        let still = touching && !input && kinetic < contact::REST_ENERGY;
        let stable = still
            && (self.destroyed.is_some() || {
                let pose = Pose {
                    r,
                    v,
                    q: self.attitude.q,
                    omega: self.attitude.omega,
                    com: props.com,
                    mass: props.mass,
                    inertia: props.inertia,
                };
                let c = &self.craft;
                let support = contact::support(&ground, &pose, &c.contacts, &c.contact, 0.0);
                contact::holds(r, -(r - ground.center).normalize(), &support, contact::REST_MARGIN)
            });
        self.rest_ticks = contact::rest_ticks(self.rest_ticks, touching, input, kinetic, stable);
        if self.rest_ticks < contact::REST_TICKS {
            return false;
        }
        let (fixed, att_fixed) = (ground.fixed(r), fixed_attitude(&ground, self.attitude.q));
        self.phase = match self.destroyed {
            Some(cause) => Phase::Crashed { body, fixed, att_fixed, cause },
            None => Phase::Landed { body, fixed, att_fixed },
        };
        self.chute = Default::default();
        self.tick_accel = None;
        self.rest_ticks = 0;
        self.control = Default::default();
        self.rest_mode = Some(attitude_mode(controls));
        self.sync_landed_attitude(world);
        true
    }
}

/// The low part of a compensated sum: what its value leaves out.
fn low_part(c: &Compensated) -> DVec3 {
    let hi = c.value();
    (c.sum - hi) + c.comp
}

/// Whether a landed pose has been checked since the vessel was built or
/// loaded: bookkeeping, not state (every value compares equal; a check
/// that passes changes nothing).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Checked(pub(super) bool);

impl PartialEq for Checked {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// The part of the controls that sets the attitude mode (SAS and its hold):
/// a landed vessel wakes when it changes.
pub(super) fn attitude_mode(c: &Controls) -> Controls {
    Controls { throttle: 0.0, rotate: DVec3::ZERO, chute: false, reference: None, target: None, ..*c }
}

/// Body → body-fixed attitude of a vessel with body → inertial `q`.
fn fixed_attitude(ground: &Ground, q: DQuat) -> DQuat {
    (DQuat::from_mat3(&ground.to_fixed) * q).normalize()
}
