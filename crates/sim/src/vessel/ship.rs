//! A vessel of a craft of catalog parts (D100, D104; ship-model.md §5.1,
//! §5.3): its ship state, the mass properties from every tank's own fill,
//! each engine at its own mount with its own Isp, throttle range and feed
//! (thrust and moments summed, `craft::engine::ship_thrust`), the RCS's
//! authority from its thrusters' geometry about the centre of mass, the
//! consumption from each tank and bottle, commands and the stage sequence,
//! and a separation at a joint (a breakup the game applies as any other).
//!
//! The vessel's `propellant` is the ship state's consumables (liquid and
//! gas): coasts, predictions and saves see the same mass (a coast changes
//! it only through its continuous laws; the tanks keep their shares,
//! `ShipState::scale_to`). The pilot's throttle and the readouts' engine
//! (`CraftParams::engine`) are the selected main engines'.

use super::attitude::{Actuators, Gimbal};
use super::split::{Breakup, LOST_CHUTE};
use super::{Vessel, TICK};
use crate::catalog::Propellant;
use crate::craft::command::ShipCommand;
use crate::craft::engine::{file_of, ship_thrust, ShipThrust};
use crate::craft::parts::combine_engines;
use crate::craft::ship::{Feed, ShipDef};
use crate::craft::state::{Effect, ShipState};
use crate::craft::{Engine, EngineOutput, MassProps};
use crate::fluids::fluids;
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

    /// The ship definition (a craft of catalog parts).
    pub fn ship_def(&self) -> Option<Arc<ShipDef>> {
        self.craft.design().ship.clone()
    }

    /// Starts the ship state of a craft of catalog parts: tanks at their
    /// loading, the readouts' engine the selected one; without parachutes
    /// there are none to open.
    pub(super) fn init_ship(&mut self) {
        let Some(def) = self.ship_def() else { return };
        let state = ShipState::initial(&def);
        self.propellant = state.consumables(&def);
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

    /// The tanks and bottles at `self.propellant` in total (after a coast's
    /// continuous laws or a set propellant).
    pub(super) fn sync_ship_to_propellant(&mut self) {
        let Some(def) = self.ship_def() else { return };
        if let Some(state) = self.ship.as_mut() {
            state.scale_to(&def, self.propellant);
        }
    }

    /// The running engines' output at `throttle` and ambient pressure `p`
    /// over a tick (a craft of catalog parts).
    pub(super) fn ship_thrust(&self, throttle: f64, p: f64) -> Option<ShipThrust> {
        let (def, state) = self.ship_parts()?;
        Some(ship_thrust(def, state, throttle, p, TICK, self.debug))
    }

    /// A tick's engine output and attitude actuators, and for a craft of
    /// parts what it draws.
    pub(super) fn actuators(
        &self,
        throttle: f64,
        p: f64,
        dt: f64,
        props: &MassProps,
    ) -> (EngineOutput, Actuators, Option<ShipTick>) {
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
            return (out, Actuators { torque, gimbal: Some(gimbal) }, None);
        };
        let t = ship_thrust(def, state, throttle, p, dt, self.debug);
        let gimbal = Gimbal { lever: t.pos - props.com, dir: t.dir, thrust: t.thrust, max_angle: t.gimbal };
        let fed: Vec<bool> = (def.thrusters.iter().zip(&state.thrusters))
            .map(|(d, s)| s.enabled && (self.debug || block_fed(&d.feed, state)))
            .collect();
        let (torque, rcs_flow) = def.rcs_authority(props.com, &fed);
        let weights: Vec<(usize, f64)> = (0..def.thrusters.len())
            .filter(|&i| fed[i])
            .map(|i| (i, def.thrusters[i].nozzles.len() as f64 * def.thrusters[i].thrust / def.thrusters[i].isp))
            .collect();
        let total: f64 = weights.iter().map(|w| w.1).sum();
        let blocks = weights.into_iter().map(|(i, w)| (i, w / total)).collect();
        let out = EngineOutput { thrust: t.thrust, mdot: t.mdot };
        let tick = ShipTick { draws: t.draws, heat: t.heat, rcs_flow, blocks };
        (out, Actuators { torque, gimbal: Some(gimbal) }, Some(tick))
    }

    /// Draws a tick's engines and RCS (`rcs`: the command's use per axis)
    /// from the tanks and bottles; returns the consumables left (kg).
    pub(super) fn consume(&mut self, tick: &ShipTick, rcs: DVec3, dt: f64) -> f64 {
        let Some(def) = self.ship_def() else { return self.propellant };
        let debug = self.debug;
        let Some(state) = self.ship.as_mut() else { return self.propellant };
        if !debug {
            for (s, d) in state.tanks.iter_mut().zip(&tick.draws) {
                s.liquid = (s.liquid - d * dt).max(0.0);
            }
            let rcs_kg = rcs.dot(tick.rcs_flow) * dt;
            for &(i, share) in &tick.blocks {
                draw_block(&def, state, i, rcs_kg * share);
            }
        }
        state.consumables(&def)
    }

    /// Applies a ship command now (`crate::craft::state::ShipState::apply`):
    /// a separation becomes the breakup the game applies (each piece a
    /// vessel), the readouts' engine follows the selection; the effects
    /// the caller acts on (the parachutes' arming) are returned.
    pub fn command(&mut self, world: &World, command: &ShipCommand) -> Result<Vec<Effect>, String> {
        let def = self.ship_def().ok_or("not a craft of parts")?;
        let effects = self.ship.as_mut().ok_or("no ship state")?.apply(&def, command)?;
        self.after_command(world, &effects);
        Ok(effects)
    }

    /// Runs the next stage of the craft's sequence (Space): its name and
    /// effects, or `None` when the sequence is done.
    pub fn next_stage(&mut self, world: &World) -> Option<(String, Vec<Effect>)> {
        let def = self.ship_def()?;
        let (name, effects) = self.ship.as_mut()?.next_stage(&def)?;
        self.after_command(world, &effects);
        Some((name, effects))
    }

    fn after_command(&mut self, world: &World, effects: &[Effect]) {
        self.sync_engine();
        let parts = &self.craft.design().parts;
        let separated: Vec<u32> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::Separate(child) => parts.iter().position(|p| p.id == *child).map(|i| i as u32),
                _ => None,
            })
            .collect();
        if separated.is_empty() {
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
        let _ = world;
    }
}

/// Whether a thruster block's feed holds anything.
fn block_fed(feed: &Feed, state: &ShipState) -> bool {
    match feed {
        Feed::Bottles(b) => b.iter().any(|&i| state.bottles[i as usize].moles > 0.0),
        Feed::Tanks([o, f]) => {
            let any = |l: &[u32]| l.iter().any(|&i| state.tanks[i as usize].liquid > 0.0);
            any(o) && any(f)
        }
    }
}

/// Draws `kg` for thruster block `i` from its feed: a bottle's gas, or its
/// tanks by the mixture ratio (the first holding any).
fn draw_block(def: &ShipDef, state: &mut ShipState, i: usize, kg: f64) {
    let t = &def.thrusters[i];
    match (&t.feed, &t.propellant) {
        (Feed::Bottles(b), Propellant::ColdGas(gas)) => {
            let molar = fluids().gas(gas).map_or(1.0, |g| g.molar_mass);
            if let Some(&j) = b.iter().find(|&&j| state.bottles[j as usize].moles > 0.0) {
                let s = &mut state.bottles[j as usize];
                s.moles = (s.moles - kg / molar).max(0.0);
            }
        }
        (Feed::Tanks([o, f]), Propellant::Biprop { mixture_ratio, .. }) => {
            let mr = *mixture_ratio;
            for (list, share) in [(o, mr / (1.0 + mr)), (f, 1.0 / (1.0 + mr))] {
                if let Some(&j) = list.iter().find(|&&j| state.tanks[j as usize].liquid > 0.0) {
                    let s = &mut state.tanks[j as usize];
                    s.liquid = (s.liquid - kg * share).max(0.0);
                }
            }
        }
        _ => {}
    }
}
