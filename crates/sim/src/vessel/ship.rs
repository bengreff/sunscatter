//! A vessel of a craft of catalog parts (D100, D104; ship-model.md §5.1,
//! §5.3): its ship state, the mass properties from every tank's own slug,
//! each engine at its own mount from its own state and level
//! (`systems::propulsion`, the one engine model), the RCS's authority from
//! its thrusters' geometry about the centre of mass, commands and the stage
//! sequence (applied at a tick: [`super::ship_tick`]), and a separation at a
//! joint (a breakup the game applies as any other).
//!
//! The ship state is the only truth of its propellant (F2, [`super::mass`]):
//! coasts, predictions and saves read its tanks and bottles. The readouts'
//! engine (`CraftParams::engine`) is the selected main engines'.

use super::attitude::{Actuators, Attitude, Gimbal};
use super::hold::Aim;
use super::split::{Breakup, LOST_CHUTE};
use super::{Controls, Vessel, TICK};
use crate::craft::command::{ShipCommand, Stage};
use crate::craft::engine::ShipThrust;
use crate::craft::ship::ShipDef;
use crate::craft::state::{Effect, ShipState};
use crate::craft::{EngineOutput, MassProps};
use crate::frame::NodeId;
use crate::systems::{command, propulsion, ship_os, OrderKind};
use crate::world::World;
use glam::DVec3;
use std::sync::Arc;

/// What a live tick of a craft of parts draws, decided at its start.
pub(super) struct ShipTick {
    /// Each tank's draw by the engines (kg/s).
    pub draws: Vec<f64>,
    /// The engines' heat into their nozzles (W, D077).
    pub heat: f64,
    /// RCS flow per body axis at full command (kg/s) and the blocks
    /// firing, with their shares of it.
    pub rcs_flow: DVec3,
    pub blocks: Vec<(usize, f64)>,
    /// The translating thrusters (the pilot's translation and the engines'
    /// settling): force, torque and each block's flow.
    pub push: crate::systems::rcs::Push,
    /// The deployed grid fins, the flow at them, their torque gains and
    /// authority (included in the actuators' torque).
    pub fins: Vec<crate::systems::fins::Fin>,
    pub fin_flow: Option<crate::systems::fins::FinFlow>,
    pub fin_gains: Vec<DVec3>,
    pub fin_auth: DVec3,
}

impl Vessel {
    /// The ship definition and state of a craft of catalog parts.
    pub(crate) fn ship_parts(&self) -> Option<(&ShipDef, &ShipState)> {
        let state = self.ship.as_ref()?;
        let def = self.craft.design().ship.as_deref()?;
        Some((def, state))
    }

    /// The ship state (a craft of catalog parts).
    pub fn ship(&self) -> Option<&ShipState> {
        self.ship.as_ref()
    }

    /// What its RCS can do now (`systems::rcs::authority`; a craft of
    /// catalog parts).
    pub fn rcs_authority(&self) -> Option<crate::systems::rcs::Authority> {
        let (def, state) = self.ship_parts()?;
        Some(crate::systems::rcs::authority(def, state, &self.mass_props(), self.debug))
    }

    /// Whether the pad's hold-down clamps hold it (a hold-down joint
    /// intact): it stays on the pad, fixed to the ground, until they release.
    pub fn clamped(&self) -> bool {
        let Some((def, state)) = self.ship_parts() else { return false };
        let hold = |j: &crate::craft::ship::JointDef| j.kind == crate::craft::assembly::JointKind::HoldDown;
        state.clamped && def.joints.iter().zip(&state.joints).any(|(d, s)| hold(d) && s.intact)
    }

    /// The ship definition (a craft of catalog parts).
    pub fn ship_def(&self) -> Option<Arc<ShipDef>> {
        self.craft.design().ship.clone()
    }

    /// Starts the ship state of a craft of catalog parts: tanks at their
    /// loading, the readouts' engine the selected one, the mechanisms of a
    /// configuration's deployed parts deployed (legs out: `craft::configure`);
    /// without parachutes there are none to open. The failure fates are
    /// drawn from its launch (its creation epoch).
    pub(super) fn init_ship(&mut self) {
        let Some(def) = self.ship_def() else { return };
        let mut state = ShipState::initial(&def);
        state.launch = Some(self.time);
        let deployed = self.craft.id.deployed();
        for (d, m) in def.mechanisms.iter().zip(&mut state.mechanisms) {
            if deployed.contains(&d.part) {
                (m.angle, m.target) = (d.item.deployed_deg, d.item.deployed_deg);
            }
        }
        // The fates are drawn at the launch, counting the parts' starts and
        // cycles from here (D105 §6.3).
        let seed = state.seed;
        crate::systems::failure::draw_fates(&def, &mut state, seed, self.time);
        self.ship = Some(state);
        if !self.craft.design().parts.iter().any(|p| p.chute) {
            self.chute = LOST_CHUTE;
        }
    }

    /// The engines' output now at ambient pressure `p` over a tick (a craft
    /// of catalog parts): from each engine's state and level (the pilot's
    /// throttle acts through the engines' next step, `systems::propulsion`).
    pub(super) fn ship_thrust(&self, _throttle: f64, p: f64) -> Option<ShipThrust> {
        let (def, state) = self.ship_parts()?;
        Some(propulsion::output(def, state, p, TICK, self.debug))
    }

    /// Whether a craft of parts has an engine starting, running or in its
    /// tail-off (`None`: not a craft of parts).
    pub(super) fn ship_engines_on(&self) -> Option<bool> {
        self.ship_parts().map(|(_, s)| propulsion::active(s))
    }

    /// Whether the ship's systems need live ticks now (an engine active, a
    /// mechanism moving, the clamps' release armed).
    pub(super) fn ship_needs_ticks(&self) -> bool {
        self.ship.as_ref().is_some_and(crate::systems::ship::busy)
    }

    /// Whether a craft of parts needs live ticks with `controls`: its
    /// systems busy, or the pilot translating on its thrusters.
    pub(super) fn ship_wants_ticks(&self, controls: &super::Controls) -> bool {
        self.ship_needs_ticks() || (self.ship.is_some() && controls.translate != DVec3::ZERO) || self.program_flying()
    }

    /// Each engine's thrust (N, aligned with the ship definition's engines)
    /// now at ambient pressure `p`: what each flame is drawn from
    /// (`systems::propulsion::output`, the one rule; the throttle argument
    /// is kept for the callers and unused: an engine's level is its state).
    pub fn engine_thrusts(&self, throttle: f64, p: f64) -> Option<Vec<f64>> {
        self.ship_thrust(throttle, p).map(|t| t.per_engine)
    }

    /// What the vessel knows now (D106, `systems::data`): from its working
    /// sensors, or everything exactly in debug mode and for a craft without
    /// the part system (the test craft, the debug craft of D064). An
    /// uncrewed craft's knowledge reaches mission control after the light
    /// time (the game's telemetry delay).
    pub fn knowledge(&self) -> crate::systems::Knowledge {
        match self.ship_parts() {
            Some((def, state)) => crate::systems::data::knowledge(def, state, self.debug),
            None => crate::systems::Knowledge::perfect(),
        }
    }

    /// The computers' health (`systems::ship_os::health`; all working for a
    /// craft without the part system).
    pub fn flight_health(&self) -> ship_os::Health {
        self.ship_parts().map_or_else(ship_os::Health::ideal, |(def, state)| ship_os::health(def, state))
    }

    /// What SAS can do now with `controls` (`systems::ship_os::sas`): the
    /// navball says it.
    pub fn sas_status(&self, controls: &Controls) -> ship_os::Sas {
        ship_os::sas(&self.knowledge(), &self.flight_health(), controls)
    }

    /// What the attitude controller flies in a live tick at `state`
    /// (anchor-relative) with the pilot's `controls` (F3): the controls the
    /// flight software lets through ([`ship_os::Sas::acting`]), what SAS aims
    /// at from the navigation solution (the attitude held without one), and
    /// the attitude as the sensors give it.
    pub(super) fn steer(
        &self,
        world: &World,
        snap: &crate::ephem::Snapshot,
        state: (NodeId, DVec3, DVec3),
        controls: &Controls,
    ) -> (Controls, Aim, Attitude) {
        let k = self.knowledge();
        let acting = ship_os::sas(&k, &self.flight_health(), controls).acting(controls);
        let view = ship_os::sense(k, (self.attitude, state), None);
        let aim = view.nav.map_or(Aim::Stability, |nav| self.aim(world, snap, nav, &acting));
        (acting, aim, view.attitude.unwrap_or(self.attitude))
    }

    /// The ship's event log (failures, aborts, stages; newest last).
    pub fn ship_events(&self) -> &[crate::systems::Event] {
        self.ship.as_ref().map_or(&[], |s| &s.events)
    }

    /// Replaces the stage sequence, the whole program's (the systems
    /// screen, D107; edited through the craft's view, `craft::program`):
    /// every command must name something this craft has or be one the
    /// program already carries (another piece's, which does nothing here),
    /// and the stages already run must stay as they were, first and in
    /// order (D111: only stages still to run change), else nothing changes
    /// and the errors are returned. The sequence's position stays: it
    /// counts exactly the stages run.
    pub fn set_stages(&mut self, stages: Vec<Stage>) -> Result<(), Vec<String>> {
        let def = self.ship_def().ok_or_else(|| vec!["not a craft of parts".to_string()])?;
        check_stages(&def, &stages, self.ship.as_ref().map_or(&[], |s| &s.stages))?;
        let state = self.ship.as_mut().ok_or_else(|| vec!["no ship state".to_string()])?;
        let run = &state.stages[..(state.sequence as usize).min(state.stages.len())];
        if !stages.starts_with(run) {
            let run = if run.len() == 1 { "stage 1 has".into() } else { format!("stages 1 to {} have", run.len()) };
            return Err(vec![format!("{run} run: only the stages after can change")]);
        }
        state.stages = stages;
        Ok(())
    }

    /// Replaces the saved functions (and what is pinned), checked as
    /// [`Self::set_stages`].
    pub fn set_functions(&mut self, functions: Vec<Stage>) -> Result<(), Vec<String>> {
        let def = self.ship_def().ok_or_else(|| vec!["not a craft of parts".to_string()])?;
        check_stages(&def, &functions, self.ship.as_ref().map_or(&[], |s| &s.functions))?;
        self.ship.as_mut().ok_or_else(|| vec!["no ship state".to_string()])?.functions = functions;
        Ok(())
    }

    /// A tick's engine output and attitude actuators, and for a craft of
    /// parts what it draws.
    pub(super) fn actuators(
        &self,
        controls: &super::Controls,
        (p, flow): (f64, Option<crate::systems::fins::FinFlow>),
        dt: f64,
        props: &MassProps,
    ) -> (EngineOutput, Actuators, Option<ShipTick>) {
        let q = flow.map_or(0.0, |f| f.q);
        let Some((def, state)) = self.ship_parts() else {
            // No parts' systems: nothing to push or turn it.
            let none = Actuators { torque: DVec3::ZERO, gimbal: None, limits: None };
            return (EngineOutput { thrust: 0.0, mdot: 0.0 }, none, None);
        };
        let t = propulsion::output(def, state, p, dt, self.debug);
        let gimbal = Gimbal { lever: t.pos - props.com, dir: t.dir, thrust: t.thrust, max_angle: t.gimbal };
        // The thrusters turn the craft unless the pilot's RCS mode keeps
        // them from it (`systems::rcs::thrusters_turn`).
        let turn = crate::systems::rcs::thrusters_turn(controls);
        let fed: Vec<bool> = (0..def.thrusters.len())
            .map(|i| {
                turn && state.thrusters[i].enabled && (self.debug || crate::systems::ship::block_ready(def, state, i))
            })
            .collect();
        let (torque, rcs_flow) = def.rcs_authority(props.com, &fed);
        // A turn on the thrusters alone spends gas: the rate it can afford
        // (a running gimbal turns the stack without it).
        let gimbals = t.thrust > 0.0 && t.gimbal > 0.0;
        let diag = DVec3::new(props.inertia.x_axis.x, props.inertia.y_axis.y, props.inertia.z_axis.z);
        let gas = if self.debug || gimbals { f64::INFINITY } else { gas_cap(def, state, diag, torque, rcs_flow) };
        // Deployed grid fins in the air add their authority.
        let fins = flow.map_or(Vec::new(), |_| crate::systems::fins::deployed(def, state));
        let fin_gains = flow.map_or(Vec::new(), |f| crate::systems::fins::gains(&fins, &f, props.com));
        let fin_auth = crate::systems::fins::authority(&fins, &fin_gains);
        let torque = torque + fin_auth;
        let weights: Vec<(usize, f64)> = (0..def.thrusters.len())
            .filter(|&i| fed[i])
            .map(|i| (i, def.thrusters[i].nozzles.len() as f64 * def.thrusters[i].thrust / def.thrusters[i].isp))
            .collect();
        let total: f64 = weights.iter().map(|w| w.1).sum();
        let blocks = weights.into_iter().map(|(i, w)| (i, w / total)).collect();
        let out = EngineOutput { thrust: t.thrust, mdot: t.mdot };
        let want = (controls.translate.clamp(DVec3::splat(-1.0), DVec3::splat(1.0))
            + ship_os::ullage::push(def, state))
        .clamp_length_max(1.0);
        let push = crate::systems::rcs::translate(def, state, props.com, want, self.debug);
        let limits = Some(limits(def, state, &t, dt, q, gas));
        let tick = ShipTick {
            draws: t.draws,
            heat: t.heat,
            rcs_flow,
            blocks,
            push,
            fins,
            fin_flow: flow,
            fin_gains,
            fin_auth,
        };
        (out, Actuators { torque, gimbal: Some(gimbal), limits }, Some(tick))
    }

    /// Applies a ship command now, at the vessel's time
    /// (`systems::command::apply`: an ignition starts the engine's start
    /// sequence, a deployment moves the mechanism at its rate): a
    /// separation becomes the breakup the game applies (each piece a
    /// vessel), the readouts' engine follows the selection; the effects the
    /// caller acts on (the parachutes' arming) are returned. To apply a
    /// command at an epoch (deterministic, with light delay), use
    /// [`Self::order_at`].
    pub fn command(&mut self, world: &World, cmd: &ShipCommand) -> Result<Vec<Effect>, String> {
        let def = self.ship_def().ok_or("not a craft of parts")?;
        // Its tanks where the coast's law has them now (D119).
        self.law_to_now();
        let at = self.time;
        let state = self.ship.as_mut().ok_or("no ship state")?;
        let effects = command::apply(&def, state, cmd, command::Context { at, sequence: false })?;
        self.after_command(world, &effects);
        Ok(effects)
    }

    /// Runs the next stage of the craft's sequence now (Space): its name and
    /// effects, or `None` when the sequence is done. Its clamp release waits
    /// for every lit engine's rated thrust (`systems::command`).
    pub fn next_stage(&mut self, world: &World) -> Option<(String, Vec<Effect>)> {
        let def = self.ship_def()?;
        let at = self.time;
        let (name, effects) = command::next_stage(&def, self.ship.as_mut()?, at)?;
        self.after_command(world, &effects);
        Some((name, effects))
    }

    /// Queues an order for epoch `at` (D067: from aboard, now; from mission
    /// control, now plus the light time): it is applied at the first tick
    /// (or, coasting or landed, exactly) at or after `at`, so the same
    /// orders give the same flight whatever the frames. An epoch before the
    /// vessel's time is applied at its next tick.
    pub fn order_at(&mut self, at: crate::time::Epoch, kind: OrderKind) -> Result<(), String> {
        let state = self.ship.as_mut().ok_or("not a craft of parts")?;
        state.order(at, kind);
        Ok(())
    }

    /// The part ids whose mechanisms are deployed now (at their deployed
    /// angle): the configuration the vessel's shape is built in
    /// (`CraftId::deployed`).
    pub fn deployed_parts(&self) -> Vec<u32> {
        let Some((def, state)) = self.ship_parts() else { return Vec::new() };
        let mut out: Vec<u32> = (def.mechanisms.iter().zip(&state.mechanisms))
            .filter(|(d, s)| d.item.deployed_deg != d.item.stowed_deg && s.angle == d.item.deployed_deg)
            .map(|(d, _)| d.part)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// After commands or a tick: the readouts' engine; separations, and a
    /// shape whose mechanisms reached or left their deployed angle (legs
    /// out, grid fins in the flow), become the breakup the game applies
    /// (`plan_breakup`: the pieces, or one piece in its new configuration).
    pub(super) fn after_command(&mut self, _world: &World, effects: &[Effect]) {
        let parts = &self.craft.design().parts;
        let separated: Vec<u32> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::Separate(child) => parts.iter().position(|p| p.id == *child).map(|i| i as u32),
                _ => None,
            })
            .collect();
        // A joint that snapped tears its child's side off (no separation
        // system fires).
        let torn: Vec<(u32, f64)> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::Snap(child, load) => parts.iter().position(|p| p.id == *child).map(|i| (i as u32, *load)),
                _ => None,
            })
            .collect();
        // A part that failed destroyed leaves as a wreck of its own.
        let failed: Vec<(u32, super::Destruction)> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::Destroy(part, why) => {
                    parts.iter().position(|p| p.id == *part).map(|i| (i as u32, super::Destruction::Failed(*why)))
                }
                _ => None,
            })
            .collect();
        let moved = self.deployed_parts() != self.craft.id.deployed();
        if separated.is_empty() && torn.is_empty() && failed.is_empty() && !moved {
            return;
        }
        let epoch = self.time;
        let b = self.breakup.get_or_insert(Breakup {
            epoch,
            destroyed: Vec::new(),
            torn: Vec::new(),
            separated: Vec::new(),
        });
        b.separated.extend(separated);
        b.torn.extend(torn);
        for (part, why) in failed {
            if !b.destroyed.iter().any(|d| d.0 == part) {
                b.destroyed.push((part, why));
            }
        }
        b.destroyed.sort_by_key(|d| d.0);
    }
}

/// The fastest turn SAS commands a craft of parts (rad/s): 10°/s up to a
/// dynamic pressure of 1 kPa, then falling as 1/q (a turn at max-Q would
/// load the stack with its angle of attack), never under 0.5°/s.
pub fn turn_cap(q: f64) -> f64 {
    let deg = crate::math::PI / 180.0;
    (10.0 * deg * (1_000.0 / q.max(1_000.0))).max(0.5 * deg)
}

/// The share of the propellant its thrusters have left that a turn on them
/// may spend spinning up and stopping (E: PLANNER-2's default, open for the
/// owner).
pub const TURN_GAS_SHARE: f64 = 0.1;

/// The fastest turn a craft of parts' thrusters can afford (rad/s): the
/// rate whose spin-up and stop about an axis (`2·I·ω/τ` seconds of the
/// couple's `flow`) cost at most [`TURN_GAS_SHARE`] of the propellant they
/// have left ([`rcs_propellant`]), the slowest axis's. `inertia` is the
/// diagonal, `torque` and `flow` the couples' per axis
/// (`ShipDef::rcs_authority`). Infinite when no couple turns the craft.
pub fn gas_cap(def: &ShipDef, state: &ShipState, inertia: DVec3, torque: DVec3, flow: DVec3) -> f64 {
    let left = rcs_propellant(def, state);
    (0..3)
        .filter(|&k| torque[k] > 0.0 && flow[k] > 0.0 && inertia[k] > 0.0)
        .map(|k| TURN_GAS_SHARE * left * torque[k] / (2.0 * inertia[k] * flow[k]))
        .fold(f64::INFINITY, f64::min)
}

/// The propellant a craft of parts' enabled thrusters draw from (kg): their
/// bottles' gas and their tanks' liquid, each counted once.
pub fn rcs_propellant(def: &ShipDef, state: &ShipState) -> f64 {
    let (mut bottles, mut tanks) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    for (d, _) in def.thrusters.iter().zip(&state.thrusters).filter(|(_, s)| s.enabled) {
        match &d.feed {
            crate::craft::ship::Feed::Bottles(b) => bottles.extend(b.iter().copied()),
            crate::craft::ship::Feed::Tanks([o, f]) => tanks.extend(o.iter().chain(f).copied()),
        }
    }
    let gas = |b: u32| state.bottles[b as usize].moles * crate::craft::state::molar_mass(&def.bottles[b as usize].gas);
    bottles.into_iter().map(gas).sum::<f64>() + tanks.into_iter().map(|t| state.tanks[t as usize].liquid).sum::<f64>()
}

impl super::Vessel {
    /// The turn rate a craft of parts' thrusters can afford now with all
    /// its enabled, fed blocks ([`gas_cap`]; rad/s): the cap a planned
    /// burn's schedule turns at. Infinite for the made-up craft and in
    /// debug mode.
    pub fn gas_turn_cap(&self) -> f64 {
        let Some((def, state)) = self.ship_parts().filter(|_| !self.debug) else { return f64::INFINITY };
        let fed: Vec<bool> = (0..def.thrusters.len())
            .map(|i| state.thrusters[i].enabled && crate::systems::ship::block_ready(def, state, i))
            .collect();
        let props = self.mass_props();
        let (torque, flow) = def.rcs_authority(props.com, &fed);
        let diag = DVec3::new(props.inertia.x_axis.x, props.inertia.y_axis.y, props.inertia.z_axis.z);
        gas_cap(def, state, diag, torque, flow)
    }
}

/// A craft of parts' attitude limits over a tick of `dt` at dynamic
/// pressure `q`: the running engines' gimbal actuators at the slowest one's
/// rate (falcon9.md §2: the Merlins' 20°/s, the MVac's 10°/s) from where
/// the gimbal holds now, and the turn cap, under the rate the thrusters'
/// gas affords (`gas`, rad/s).
fn limits(def: &ShipDef, state: &ShipState, t: &ShipThrust, dt: f64, q: f64, gas: f64) -> super::attitude::Limits {
    let rate = (def.engines.iter().zip(&t.per_engine))
        .filter(|(e, &f)| f > 0.0 && e.data.gimbal_rate_dps > 0.0)
        .map(|(e, _)| e.data.gimbal_rate_dps)
        .fold(f64::INFINITY, f64::min);
    let step = if rate.is_finite() { rate * crate::math::PI / 180.0 * dt } else { f64::INFINITY };
    super::attitude::Limits {
        gimbal_from: state.thrust_dir.unwrap_or(t.dir),
        gimbal_step: step,
        max_rate: turn_cap(q).min(gas),
    }
}

/// Every command of `stages` checked against the craft; the errors by stage.
/// Every command of `stages` names something `def` has, or is one the
/// list `carried` already has (another piece's: the program is the whole
/// vessel's, `craft::program`); else each error.
fn check_stages(def: &ShipDef, stages: &[Stage], carried: &[Stage]) -> Result<(), Vec<String>> {
    let had = |a: &ShipCommand| carried.iter().any(|s| s.actions.contains(a));
    let errors: Vec<String> = (stages.iter())
        .flat_map(|s| {
            (s.actions.iter())
                .filter_map(move |a| def.check(a).err().filter(|_| !had(a)).map(|e| format!("{:?}: {e}", s.name)))
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
