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
    /// Ambient pressure (Pa).
    pub p: f64,
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
        continuous.then(|| self.ship.as_ref()?.runtime.next_order()).flatten()
    }

    /// Applies every order due at the vessel's time (see the module).
    pub(super) fn apply_orders(&mut self, world: &World) {
        let at = self.time;
        let Some(state) = self.ship.as_ref() else { return };
        if state.runtime.next_order().is_none_or(|e| e > at) {
            return;
        }
        let Some(def) = self.ship_def() else { return };
        let state = self.ship.as_mut().expect("checked");
        let mut effects = Vec::new();
        for order in state.runtime.due(at) {
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
            let state = self.ship.as_mut().expect("a craft of parts");
            let out = propulsion::output(&def, state, p, TICK, debug);
            // The clamps hold the vehicle (all but the mount) still on the
            // ground: their force on it is m·a_proper less the thrust.
            let mount: f64 = def.parts.iter().filter(|x| x.ground).map(|x| x.mass).sum();
            let on_vehicle = accel * (props.mass - mount) - out.dir * out.thrust;
            let mut effects = Vec::new();
            for (j, d) in def.joints.iter().enumerate().filter(|(_, d)| d.kind == JointKind::HoldDown) {
                let capacity = d.allowable * 2.0 * crate::math::PI * d.radius;
                let run = &mut state.runtime.joints[j];
                run.force = -on_vehicle;
                run.moment = DVec3::ZERO;
                run.ratio = if capacity > 0.0 { on_vehicle.length() / capacity } else { 0.0 };
                if run.ratio >= 1.0 && !debug && state.joints[j].intact {
                    state.runtime.event(
                        at,
                        format!("{}: torn out at {:.1} MN", d.name, on_vehicle.length() / 1e6),
                        true,
                    );
                    effects.extend(
                        state.apply(&def, &crate::craft::command::ShipCommand::ReleaseClamps).unwrap_or_default(),
                    );
                }
            }
            let env =
                sys::TickEnv { at, dt: TICK, p, throttle: controls.throttle, accel, omega, alpha: DVec3::ZERO, debug };
            effects.extend(sys::tick(&def, state, &env, &out.draws, &[]));
            self.propellant = state.consumables(&def);
            self.advance_grounded(world, at.add_seconds(TICK));
            self.after_command(world, &effects);
            if effects.contains(&Effect::ReleaseClamps) {
                self.wake(world);
                return;
            }
        }
    }

    /// After a live tick of a craft of parts (`m` its motion, the vessel's
    /// time still its start): the systems' step with what the tick drew.
    /// Returns the consumables left (kg) and the effects to act on at the
    /// tick's end.
    pub(super) fn ship_after_tick(
        &mut self,
        world: &World,
        tick: &ShipTick,
        rcs: DVec3,
        m: &TickMotion,
        throttle: f64,
    ) -> (f64, Vec<Effect>) {
        let at = self.time;
        let mid = at.add_seconds(0.5 * m.dt);
        let g = gravity(world, mid, m.anchor, (m.r0 + m.r1) * 0.5, (m.v0 + m.v1) * 0.5);
        let q = self.attitude.q;
        let accel = q.inverse() * ((m.v1 - m.v0) / m.dt - g);
        let omega = self.attitude.body_rate();
        let alpha = (omega - q.inverse() * m.omega0) / m.dt;
        let rcs_kg = rcs.dot(tick.rcs_flow) * m.dt;
        let (debug, p) = (self.debug, m.p);
        let (Some(def), Some(state)) = (self.ship_def(), self.ship.as_mut()) else { return (self.propellant, vec![]) };
        let mut per_block = vec![0.0; def.thrusters.len()];
        for &(i, share) in &tick.blocks {
            per_block[i] = rcs_kg * share;
        }
        let env = sys::TickEnv { at, dt: m.dt, p, throttle, accel, omega, alpha, debug };
        let effects = sys::tick(&def, state, &env, &tick.draws, &per_block);
        (state.consumables(&def), effects)
    }
}
