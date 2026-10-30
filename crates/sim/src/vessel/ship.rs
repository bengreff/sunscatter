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
use crate::craft::engine::{file_of, ShipThrust};
use crate::craft::parts::combine_engines;
use crate::craft::ship::ShipDef;
use crate::craft::state::{Effect, ShipState};
use crate::craft::{Engine, EngineOutput, MassProps};
use crate::frame::NodeId;
use crate::systems::{command, flight, propulsion, OrderKind};
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
    pub(super) fn ship_parts(&self) -> Option<(&ShipDef, &ShipState)> {
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
    /// loading, the readouts' engine the selected one; without parachutes
    /// there are none to open. The failure fates are drawn from its launch
    /// (its creation epoch).
    pub(super) fn init_ship(&mut self) {
        let Some(def) = self.ship_def() else { return };
        let mut state = ShipState::initial(&def);
        state.runtime.launch = Some(self.time);
        // The fates are drawn at the launch, counting the parts' starts and
        // cycles from here (D105 §6.3).
        let seed = state.runtime.seed;
        crate::systems::failure::draw_fates(&def, &mut state, seed, self.time);
        // Its tanks and bottles hold its propellant (F2).
        self.propellant = 0.0;
        self.ship = Some(state);
        self.sync_engine();
        if !self.craft.design().parts.iter().any(|p| p.chute) {
            self.chute = LOST_CHUTE;
        }
    }

    /// The readouts' engine (`CraftParams::engine`: planned burns, the
    /// navball, landing predictions): the selected main engines together,
    /// else all the main engines; none, a zero-thrust stand-in.
    pub(super) fn sync_engine(&mut self) {
        let Some((def, state)) = self.ship_parts() else { return };
        let main = |i: &usize| def.engines[*i].main;
        let selected: Vec<usize> = (0..def.engines.len()).filter(main).filter(|&i| state.engines[i].selected).collect();
        let all: Vec<usize> = (0..def.engines.len()).filter(main).collect();
        let pick = if selected.is_empty() { all } else { selected };
        let files: Vec<_> = pick.iter().map(|&i| file_of(&def.engines[i].data)).collect();
        self.craft.engine = Engine::from_file(&combine_engines(&files));
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
        self.ship_needs_ticks() || (self.ship.is_some() && controls.translate != DVec3::ZERO)
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

    /// The computers' health (`systems::flight::health`; all working for a
    /// craft without the part system).
    pub fn flight_health(&self) -> flight::Health {
        self.ship_parts().map_or_else(flight::Health::ideal, |(def, state)| flight::health(def, state))
    }

    /// What SAS can do now with `controls` (`systems::flight::sas`): the
    /// navball says it.
    pub fn sas_status(&self, controls: &Controls) -> flight::Sas {
        flight::sas(&self.knowledge(), &self.flight_health(), controls)
    }

    /// What the attitude controller flies in a live tick at `state`
    /// (anchor-relative) with the pilot's `controls` (F3): the controls the
    /// flight software lets through ([`flight::Sas::acting`]), what SAS aims
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
        let acting = flight::sas(&k, &self.flight_health(), controls).acting(controls);
        let view = flight::sense(k, (self.attitude, state), None);
        let aim = view.nav.map_or(Aim::Stability, |nav| self.aim(world, snap, nav, &acting));
        (acting, aim, view.attitude.unwrap_or(self.attitude))
    }

    /// The ship's event log (failures, aborts, stages; newest last).
    pub fn ship_events(&self) -> &[crate::systems::Event] {
        self.ship.as_ref().map_or(&[], |s| &s.runtime.events)
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
        let throttle = controls.throttle;
        let Some((def, state)) = self.ship_parts() else {
            let engine = self.craft.engine;
            let out = engine.tick_output(throttle, p, self.propellant, dt, self.debug);
            let gimbal = Gimbal {
                lever: engine.mount_pos - props.com,
                dir: engine.mount_dir,
                thrust: out.thrust,
                max_angle: engine.gimbal,
            };
            // The RCS draws on the main propellant: dry, only the gimbal turns.
            let rcs_fed = self.debug || self.propellant > 0.0;
            let torque = if rcs_fed { self.craft.torque } else { DVec3::ZERO };
            return (out, Actuators { torque, gimbal: Some(gimbal), limits: None }, None);
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
        let want = (controls.translate.clamp(DVec3::splat(-1.0), DVec3::splat(1.0)) + propulsion::ullage(def, state))
            .clamp_length_max(1.0);
        let push = crate::systems::rcs::translate(def, state, props.com, want, self.debug);
        let limits = Some(limits(def, state, &t, dt, q));
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
        state.runtime.order(at, kind);
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
        self.sync_engine();
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

/// A craft of parts' attitude limits over a tick of `dt` at dynamic
/// pressure `q`: the running engines' gimbal actuators at the slowest one's
/// rate (falcon9.md §2: the Merlins' 20°/s, the MVac's 10°/s) from where
/// the gimbal holds now, and the turn cap.
fn limits(def: &ShipDef, state: &ShipState, t: &ShipThrust, dt: f64, q: f64) -> super::attitude::Limits {
    let rate = (def.engines.iter().zip(&t.per_engine))
        .filter(|(e, &f)| f > 0.0 && e.data.gimbal_rate_dps > 0.0)
        .map(|(e, _)| e.data.gimbal_rate_dps)
        .fold(f64::INFINITY, f64::min);
    let step = if rate.is_finite() { rate * crate::math::PI / 180.0 * dt } else { f64::INFINITY };
    super::attitude::Limits {
        gimbal_from: state.runtime.thrust_dir.unwrap_or(t.dir),
        gimbal_step: step,
        max_rate: turn_cap(q),
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
