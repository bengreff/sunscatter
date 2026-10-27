//! One live tick of a craft of parts' systems (ship-model.md §5.0), after
//! the rigid body has flown it: what the engines and thrusters drew, the
//! mechanisms, the engines' transients and the pad's release, and the motion
//! the systems felt (for the next tick's liquids, joints and sensors).
//!
//! The forces of a tick come from the state at its start
//! ([`super::propulsion::output`]); this step takes the state to the tick's
//! end. Pure: the vessel gathers [`TickEnv`] and acts on the effects.

use super::{plumbing, propulsion};
use crate::catalog::Propellant;
use crate::craft::ship::{Feed, ShipDef};
use crate::craft::state::{Effect, ShipState};
use crate::fluids::fluids;
use crate::time::Epoch;
use glam::DVec3;

/// What a tick's systems read.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickEnv<'a> {
    /// The tick's start and length (s).
    pub at: Epoch,
    pub dt: f64,
    /// Ambient pressure (Pa).
    pub p: f64,
    /// The pilot's throttle.
    pub throttle: f64,
    /// Proper acceleration at the centre of mass, angular velocity and
    /// angular acceleration over the tick (body axes).
    pub accel: DVec3,
    pub omega: DVec3,
    pub alpha: DVec3,
    /// The centre of mass (body axes).
    pub com: DVec3,
    /// Sunlight at the vessel (W/m², along the light, body axes) and the
    /// air's temperature, if in an atmosphere (K).
    pub sun: DVec3,
    pub air_k: Option<f64>,
    /// Debug mode (D064).
    pub debug: bool,
    /// The parts and the forces on them over the tick, for the joints
    /// (`None`: no structure step, as for a vessel that does not move).
    pub bodies: Option<super::structure::Bodies<'a>>,
}

/// Runs a tick (see the module): `draws` is what the engines drew from each
/// tank over it (kg/s, `propulsion::output`), `rcs` the kilograms each
/// thruster block fired. Returns the effects the vessel acts on.
pub fn tick(def: &ShipDef, state: &mut ShipState, env: &TickEnv, draws: &[f64], rcs: &[f64]) -> Vec<Effect> {
    state.runtime.refit(def);
    let before = plumbing::ullages(def, state);
    if !env.debug {
        for (s, d) in state.tanks.iter_mut().zip(draws) {
            s.liquid = (s.liquid - d * env.dt).max(0.0);
        }
        for (i, &kg) in rcs.iter().enumerate() {
            draw_block(def, state, i, kg);
        }
    }
    super::mechanisms::step(def, state, env.dt);
    let r = &mut state.runtime;
    (r.accel, r.omega, r.alpha) = (env.accel, env.omega, env.alpha);
    let a = plumbing::accels(def, state, env.com, env.accel, env.omega, env.alpha);
    plumbing::fast(def, state, &before, &a, env.dt);
    // The slow part covers the time since it last stepped (the lattice's,
    // before the vessel went live), else the tick.
    let end = env.at.add_seconds(env.dt);
    let since = state.runtime.stepped.map_or(env.dt, |s| end.seconds_since(s).max(0.0));
    plumbing::slow(def, state, &plumbing::Surroundings { sun: env.sun, air_k: env.air_k }, since);
    state.runtime.stepped = Some(end);
    let feeds = plumbing::feeds(def, state, &a);
    let penv = propulsion::Env { at: env.at, dt: env.dt, throttle: env.throttle, debug: env.debug };
    let mut effects = propulsion::step(def, state, &penv, &feeds);
    if let Some(b) = &env.bodies {
        let m = super::structure::Motion { com: DVec3::ZERO, accel: env.accel, omega: env.omega, alpha: env.alpha };
        effects.extend(super::structure::step(def, state, b, &m, env.at, env.debug));
    }
    effects.extend(super::failure::step(def, state, env));
    super::power::step_ship(def, state, env.sun, env.dt, env.at);
    effects
}

/// Draws `kg` for thruster block `i` from its feed: a bottle's gas, or its
/// tanks by the mixture ratio (the first holding any).
pub fn draw_block(def: &ShipDef, state: &mut ShipState, i: usize, kg: f64) {
    if kg <= 0.0 {
        return;
    }
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

/// Whether a thruster block's feed holds anything.
pub fn block_fed(feed: &Feed, state: &ShipState) -> bool {
    match feed {
        Feed::Bottles(b) => b.iter().any(|&i| state.bottles[i as usize].moles > 0.0),
        Feed::Tanks([o, f]) => {
            let any = |l: &[u32]| l.iter().any(|&i| state.tanks[i as usize].liquid > 0.0);
            any(o) && any(f)
        }
    }
}

/// Whether the systems need live ticks now: an engine starting, running or
/// in its tail-off, a mechanism moving, the clamps' release armed.
pub fn busy(state: &ShipState) -> bool {
    propulsion::active(state) || super::mechanisms::moving(state) || state.runtime.release_armed
}

/// The ship's slow systems on the lattice (coasting or standing): the
/// plumbing's heat, relief and boil-off, the bottles' and ullages' heat
/// exchange, the liquids' moving or dispersing in free fall, from where
/// they last stepped to `at`, under the proper acceleration `accel` (body
/// axes). Returns the mass vented overboard (kg).
pub fn lattice(def: &ShipDef, state: &mut ShipState, at: Epoch, env: &plumbing::Surroundings, accel: DVec3) -> f64 {
    state.runtime.refit(def);
    let dt = state.runtime.stepped.map_or(0.0, |s| at.seconds_since(s));
    if state.runtime.stepped.is_none() || dt > 0.0 {
        state.runtime.stepped = Some(at);
    }
    if dt <= 0.0 {
        return 0.0;
    }
    let before = plumbing::ullages(def, state);
    let a = plumbing::accels(def, state, DVec3::ZERO, accel, DVec3::ZERO, DVec3::ZERO);
    let vented = plumbing::slow(def, state, env, dt);
    plumbing::fast(def, state, &before, &a, dt);
    super::power::step_ship(def, state, env.sun, dt, at);
    vented
}
