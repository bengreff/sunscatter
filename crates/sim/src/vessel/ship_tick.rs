//! A craft of parts' systems in the vessel's phases (ship-model.md §5.0):
//! orders applied at their epochs, the pad's held ticks, and the systems'
//! step after every live tick.
//!
//! * **Orders** ([`Vessel::order_at`]) are applied at the first tick at or
//!   after their epoch while the vessel flies ticks (live, or held on the
//!   pad), and exactly at it while it advances continuously (coasting, or
//!   standing idle): the vessel stops there. So orders at given epochs give
//!   the same flight in any frames.
//! * **Held on the pad**: while the hold-down clamps hold it and its
//!   systems are busy (engines starting or running, the release armed), the
//!   vessel ticks in place, fixed to the ground: the engines spool up and
//!   burn their propellant, the clamps carry what holds it there
//!   (m·a_proper − F_thrust, the joint's load: the clamps fail above their
//!   capacity), and the armed release lets go at full thrust.
//! * **After a live tick**: the proper acceleration the tick gave
//!   ((v₁ − v₀)/dt less gravity at its midpoint), the angular velocity and
//!   its rate, then `systems::ship::tick`.

use super::ship::ShipTick;
use super::ship_loads::GroundAt;
use super::{Controls, Phase, Vessel, TICK};
use crate::craft::assembly::JointKind;
use crate::craft::state::Effect;
use crate::forces::{ambient_pressure, ActiveSources, ForceContext};
use crate::frame::NodeId;
use crate::systems::{command, propulsion, ship as sys};
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// The motion of a live tick, for the systems.
pub(super) struct TickMotion {
    pub anchor: NodeId,
    pub dt: f64,
    pub r0: DVec3,
    pub r1: DVec3,
    pub v0: DVec3,
    pub v1: DVec3,
    /// Angular velocity at the tick's start (inertial).
    pub omega0: DVec3,
    /// Ambient pressure (Pa), sunlight (W/m², body axes) and the air's
    /// temperature (K, in an atmosphere).
    pub p: f64,
    pub sun: DVec3,
    pub air_k: Option<f64>,
    /// The air's loads over it (at its start's attitude) and the ground
    /// under it, for the joints (`ship_loads`).
    pub air: Option<super::aerothermal::AeroLoads>,
    pub ground: Option<NodeId>,
    /// The engines' thrust direction the tick flew (body axes, after the
    /// gimbal).
    pub thrust_dir: DVec3,
}

impl Vessel {
    /// A craft of parts' other effectors over a tick, after the attitude
    /// command: the translating thrusters (`systems::rcs`) and the grid
    /// fins (`systems::fins`: the command's share of their authority
    /// allocated to deflections at their actuators' rate, their real force
    /// and torque, the flow's incidence included, in its place). Their
    /// torques go into the command's, their forces (body axes) into the
    /// tick's constant acceleration `lift` (inertial), which is returned
    /// with the fins' deflections (aligned with the tick's fins; held when
    /// the tick commits, [`Self::hold_effectors`]).
    pub(super) fn ship_effectors(
        &self,
        ship: Option<&ShipTick>,
        cmd: &mut super::attitude::Command,
        lift: DVec3,
        props: &crate::craft::MassProps,
        dt: f64,
    ) -> (DVec3, Vec<f64>) {
        let Some(t) = ship else { return (lift, Vec::new()) };
        let q = self.attitude.q;
        cmd.torque += t.push.torque;
        let mut lift = lift + q * t.push.force / props.mass;
        if let (Some(flow), Some(state)) = (t.fin_flow, self.ship.as_ref()) {
            if !t.fins.is_empty() {
                use crate::systems::fins;
                let want = cmd.u * t.fin_auth;
                let now = fins::held(state, &t.fins);
                let d = fins::allocate(&t.fins, &t.fin_gains, want, &now, dt);
                let (force, torque) = fins::forces(&t.fins, &flow, props.com, &d);
                cmd.torque += torque - want;
                lift += q * force / props.mass;
                return (lift, d);
            }
        }
        (lift, Vec::new())
    }

    /// A committed tick's effectors: the grid fins hold their deflections
    /// (`fins`, aligned with the tick's), and the gimbal the thrust where
    /// the command left it (its actuators start the next tick from it).
    pub(super) fn hold_effectors(&mut self, ship: Option<&ShipTick>, cmd: &super::attitude::Command, fins: &[f64]) {
        let def = self.ship_def();
        let Some(s) = self.ship.as_mut() else { return };
        s.thrust_dir = Some(cmd.thrust_dir);
        if let (Some(t), Some(def)) = (ship, def) {
            if !fins.is_empty() {
                crate::systems::fins::hold(&def, s, &t.fins, fins);
            }
        }
    }
}

/// The gravitational acceleration (relative to `anchor`) at `r` at `t`.
fn gravity(world: &World, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3) -> DVec3 {
    let snap = world.snapshot(t);
    let active = ActiveSources::select(world, &snap, anchor, r);
    ForceContext { world, anchor, active: &active, drag: None, thrust: DVec3::ZERO }.accel_with(&snap, r, v)
}

impl Vessel {
    /// The epoch of the next order while the vessel advances continuously
    /// (coasting, crashed, or standing idle): it stops there to apply it.
    pub(super) fn order_stop(&self) -> Option<Epoch> {
        let continuous = match self.phase {
            Phase::Coasting { .. } | Phase::Crashed { .. } => true,
            Phase::Landed { .. } => !(self.clamped() && self.ship_needs_ticks()),
            Phase::Powered { .. } => false,
        };
        let order = continuous.then(|| self.ship.as_ref()?.next_order()).flatten();
        // A planned burn's program wakes at its epoch the same way.
        let wake = continuous.then(|| self.program_stop()).flatten();
        match (order, wake) {
            (Some(o), Some(w)) => Some(if w < o { w } else { o }),
            (o, w) => o.or(w),
        }
    }

    /// Applies every order due at the vessel's time (see the module).
    pub(super) fn apply_orders(&mut self, world: &World) {
        let at = self.time;
        let Some(state) = self.ship.as_ref() else { return };
        if state.next_order().is_none_or(|e| e > at) {
            return;
        }
        let Some(def) = self.ship_def() else { return };
        self.law_to_now();
        let state = self.ship.as_mut().expect("checked");
        let mut effects = Vec::new();
        for order in state.due(at) {
            effects.extend(command::carry_out(&def, state, &order.kind, at));
        }
        self.after_command(world, &effects);
    }

    /// Held on the pad (see the module) until `target`, a tick at a time;
    /// returns when the clamps release (the vessel wakes into live ticks
    /// with the mount's separation pending), when nothing needs ticks any
    /// more, or with less than a tick to go.
    pub(super) fn advance_held(&mut self, world: &World, target: Epoch, controls: &Controls) {
        while self.time.add_seconds(TICK) <= target {
            self.apply_orders(world);
            if self.breakup.is_some() || !self.clamped() || !self.ship_needs_ticks() {
                return;
            }
            let Phase::Landed { body, .. } = self.phase else { return };
            let (anchor, r, v) = self.state(world);
            let snap = world.snapshot(self.time);
            let p = ambient_pressure(world, &snap, anchor, r);
            let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical body").rotation;
            let w = rot.omega(self.time).raw();
            let proper = w.cross(w.cross(r)) - gravity(world, self.time, anchor, r, v);
            let q = self.attitude.q;
            let (accel, omega) = (q.inverse() * proper, q.inverse() * w);
            let props = self.mass_props();
            let (debug, at) = (self.debug, self.time);
            let def = self.ship_def().expect("a craft of parts");
            let out = propulsion::output(&def, self.ship.as_ref().expect("a craft of parts"), p, TICK, debug);
            // The clamps hold the vehicle (all but the mount) still on the
            // ground: their force on it is m·a_proper less the thrust.
            let mount: f64 = def.parts.iter().filter(|x| x.ground).map(|x| x.mass).sum();
            let on_vehicle = accel * (props.mass - mount) - out.dir * out.thrust;
            let loads = self.gather_held(&out, on_vehicle, p);
            let state = self.ship.as_mut().expect("a craft of parts");
            let mut effects = Vec::new();
            for (j, d) in def.joints.iter().enumerate().filter(|(_, d)| d.kind == JointKind::HoldDown) {
                let capacity = d.allowable * 2.0 * crate::math::PI * d.radius;
                let run = &mut state.joints[j];
                run.force = -on_vehicle;
                run.moment = DVec3::ZERO;
                run.ratio = if capacity > 0.0 { on_vehicle.length() / capacity } else { 0.0 };
                if run.ratio >= 1.0 && !debug && state.joints[j].intact {
                    state.event(at, format!("{}: torn out at {:.1} MN", d.name, on_vehicle.length() / 1e6), true);
                    effects.extend(
                        state.apply(&def, &crate::craft::command::ShipCommand::ReleaseClamps).unwrap_or_default(),
                    );
                }
            }
            let (alpha, bodies) = (DVec3::ZERO, Some(loads.bodies()));
            let sun = q.inverse() * crate::light::sunlight(world, &snap, anchor, r);
            let air_k = super::aerothermal::air_at(world, &snap, anchor, r, v).map(|a| a.temperature);
            let (throttle, com) = (controls.throttle, props.com);
            let env = sys::TickEnv { at, dt: TICK, p, throttle, accel, omega, alpha, com, sun, air_k, debug, bodies };
            effects.extend(sys::tick(&def, state, &env, &out.draws, &[]));
            self.advance_grounded(world, at.add_seconds(TICK));
            self.after_command(world, &effects);
            if effects.contains(&Effect::ReleaseClamps) {
                self.wake(world);
                return;
            }
        }
    }

    /// The tanks' liquids after the structure was jolted by `dv` (inertial,
    /// a separation system's push): they keep their velocity, so each moves
    /// against its tank by the push along the tank's axis (the pushers
    /// settle the stage they push forward, and float the one pushed back).
    pub(super) fn jolt(&mut self, dv: DVec3) {
        let Some(def) = self.ship_def() else { return };
        let body = self.attitude.q.inverse() * dv;
        let Some(state) = self.ship.as_mut() else { return };
        for (t, s) in def.tanks.iter().zip(state.tanks.iter_mut()) {
            s.drift_speed -= body.dot(t.region.axis.normalize());
        }
    }

    /// After a live tick of a craft of parts (`m` its motion, the vessel's
    /// time still its start): the systems' step with what the tick drew.
    /// Returns the effects to act on at the tick's end.
    pub(super) fn ship_after_tick(
        &mut self,
        world: &World,
        tick: &ShipTick,
        rcs: DVec3,
        m: &TickMotion,
        throttle: f64,
    ) -> Vec<Effect> {
        let at = self.time;
        let mid = at.add_seconds(0.5 * m.dt);
        let g = gravity(world, mid, m.anchor, (m.r0 + m.r1) * 0.5, (m.v0 + m.v1) * 0.5);
        let q = self.attitude.q;
        let accel = q.inverse() * ((m.v1 - m.v0) / m.dt - g);
        let omega = self.attitude.body_rate();
        let alpha = (omega - q.inverse() * m.omega0) / m.dt;
        let rcs_kg = rcs.dot(tick.rcs_flow) * m.dt;
        let (debug, p, com) = (self.debug, m.p, self.mass_props().com);
        // The joints' loads: the engines as the tick flew them (the state at
        // its start), the air, the ground at its end.
        let loads = self.ship_parts().map(|(def, state)| {
            let out = propulsion::output(def, state, p, m.dt, debug);
            let ground = m.ground.map(|body| GroundAt { anchor: m.anchor, body, r: m.r1, v: m.v1 });
            let mut g = self.gather_live(world, (&out, m.thrust_dir), m.air.as_ref(), ground.as_ref(), p);
            g.push(self.fin_pushes(&tick.fins, tick.fin_flow));
            g
        });
        let (Some(def), Some(state)) = (self.ship_def(), self.ship.as_mut()) else { return vec![] };
        let mut per_block = vec![0.0; def.thrusters.len()];
        for &(i, share) in &tick.blocks {
            per_block[i] = rcs_kg * share;
        }
        for (b, f) in per_block.iter_mut().zip(&tick.push.flows) {
            *b += f * m.dt;
        }
        let bodies = loads.as_ref().map(|g| g.bodies());
        let (sun, air_k) = (m.sun, m.air_k);
        let env = sys::TickEnv { at, dt: m.dt, p, throttle, accel, omega, alpha, com, sun, air_k, debug, bodies };
        sys::tick(&def, state, &env, &tick.draws, &per_block)
    }
}

/// A craft of parts' slow systems at lattice epoch `e` (`systems::ship::
/// lattice`: boil-off and venting, its liquids warming, settled or
/// floating), at `(anchor, r, v)` with attitude `q`: standing on `grounded`
/// the ground holds its liquids down (proper acceleration ω×(ω×r) − g);
/// coasting they float. Its tanks and bottles are its propellant: what
/// they vent leaves the vessel (F2). On a coast with a law of its tanks
/// (`law`: it and the local time of `e`, D119), they are the law's there.
pub(super) fn ship_lattice(
    ship: Option<&mut crate::craft::state::ShipState>,
    def: Option<&crate::craft::ship::ShipDef>,
    world: &World,
    (e, (anchor, r, v), q): (Epoch, (NodeId, DVec3, DVec3), glam::DQuat),
    grounded: Option<NodeId>,
    law: Option<(&crate::systems::coast_law::CoastLaw, f64)>,
) {
    let (Some(state), Some(def)) = (ship, def) else { return };
    let snap = world.snapshot(e);
    let sun = q.inverse() * crate::light::sunlight(world, &snap, anchor, r);
    let air_k = super::aerothermal::air_at(world, &snap, anchor, r, v).map(|a| a.temperature);
    let accel = match grounded {
        Some(body) => {
            let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical body").rotation;
            let w = rot.omega(e).raw();
            q.inverse() * (w.cross(w.cross(r)) - gravity(world, e, anchor, r, v))
        }
        None => DVec3::ZERO,
    };
    let env = crate::systems::plumbing::Surroundings { sun, air_k };
    match law {
        Some(law) => sys::coast_lattice(def, state, e, &env, law),
        None => {
            sys::lattice(def, state, e, &env, accel);
        }
    }
}
