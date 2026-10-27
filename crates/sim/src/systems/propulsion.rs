//! Engines (D104; ship-model.md §5.3; falcon9.md §2): one model for every
//! engine of a craft of parts, from its published data.
//!
//! * **State machine** per engine: *off* → *starting* (an ignition: the
//!   ground's supply on the pad, else one of its charges; thrust and flows
//!   ramp over its start time as a smooth step) → *running* (its throttle
//!   held within its range: a running engine cannot go below its minimum;
//!   zero throttle is not off) → *shutting down* (the tail-off: thrust
//!   decays exponentially with the published shutdown impulse,
//!   τ = I / F_vac) → *off*; or *failed*.
//! * **Steady map**: at flow level τ (a fraction of full flow) and ambient
//!   pressure p, F = τ·F_vac − p·Aₑ and ṁ = τ·ṁ_full (an ideal nozzle: the
//!   exit pressure scales with the chamber pressure, so the back-pressure
//!   term does not; falcon9.md §2.1), Isp = F/(ṁ·g₀); never negative.
//! * **Feeds**: each engine draws its oxidizer and fuel by its mixture ratio
//!   from the first of its feed tanks holding liquid; tanks that cannot
//!   supply a tick share out what they hold; an engine whose tanks run dry
//!   flames out (not a failure).
//! * **The pad's rule** (falcon9.md §1.2, §4.2): an armed release lets the
//!   clamps go at the first tick every engine it waits for runs at its
//!   rated thrust; an engine that fails or stops first aborts the launch
//!   (every engine shut down, the clamps kept).

use super::plumbing::Feed;
use crate::catalog::EngineData;
use crate::craft::command::ShipCommand;
use crate::craft::engine::{ShipThrust, P_SEA_LEVEL};
use crate::craft::ship::{EngineDef, ShipDef};
use crate::craft::state::{Effect, EngineStatus, ShipState};
use crate::math;
use crate::time::Epoch;
use crate::vessel::G0;
use glam::DVec3;

/// The start transient's shape: a smooth step from 0 to 1 over `x` in [0, 1].
pub fn ramp(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Full mass flow (kg/s).
pub fn full_flow(d: &EngineData) -> f64 {
    d.thrust_vac / (d.isp_vac * G0)
}

/// Nozzle exit area (m²): the geometry's, else from the Isp pair.
pub fn exit_area(d: &EngineData) -> f64 {
    if d.exit_area > 0.0 {
        d.exit_area
    } else {
        d.thrust_vac * (1.0 - d.isp_sl / d.isp_vac) / P_SEA_LEVEL
    }
}

/// Thrust (N) and mass flow (kg/s) at flow level `level` and ambient
/// pressure `p` (Pa).
pub fn steady(d: &EngineData, level: f64, p: f64) -> (f64, f64) {
    let level = level.max(0.0);
    ((level * d.thrust_vac - p * exit_area(d)).max(0.0), level * full_flow(d))
}

/// The level a running engine holds at the pilot's `throttle` and its own
/// setting `own`: within [min, max] of its range.
pub fn setting(d: &EngineData, throttle: f64, own: f64) -> f64 {
    (throttle * own).clamp(d.throttle_min, d.throttle_max)
}

/// The tail-off's time constant (s): the shutdown impulse over full
/// thrust (0: it stops at once).
pub fn tail_off(d: &EngineData) -> f64 {
    if d.thrust_vac > 0.0 {
        (d.shutdown_impulse / d.thrust_vac).max(0.0)
    } else {
        0.0
    }
}

/// The engines lit now (starting or running; indices into the definition's
/// engines).
pub fn lit(state: &ShipState) -> Vec<u32> {
    (state.engines.iter().enumerate())
        .filter(|(_, e)| matches!(e.status, EngineStatus::Starting | EngineStatus::Running))
        .map(|(i, _)| i as u32)
        .collect()
}

/// Whether any engine is doing something (starting, running or in its
/// tail-off): the vessel flies live ticks.
pub fn active(state: &ShipState) -> bool {
    (state.engines.iter())
        .any(|e| matches!(e.status, EngineStatus::Starting | EngineStatus::Running | EngineStatus::ShuttingDown))
}

/// The first tank of a feed list holding liquid (any, when `infinite`).
fn first(list: &[u32], state: &ShipState, infinite: bool) -> Option<usize> {
    list.iter().map(|&t| t as usize).find(|&t| infinite || state.tanks[t].liquid > 0.0)
}

/// The engines' output over a tick of `dt` at their levels now and ambient
/// pressure `p` (`infinite`: debug mode, no draw limits): thrust and flow
/// summed, the thrust-weighted mount and direction, each tank's draw, each
/// engine's thrust.
pub fn output(def: &ShipDef, state: &ShipState, p: f64, dt: f64, infinite: bool) -> ShipThrust {
    let mut draws = vec![0.0; def.tanks.len()];
    let mut running: Vec<(usize, f64, f64, [Option<usize>; 2])> = Vec::new();
    for (i, (e, s)) in def.engines.iter().zip(&state.engines).enumerate() {
        let level = state.runtime.engines.get(i).map_or(0.0, |r| r.level);
        if level <= 0.0 || matches!(s.status, EngineStatus::Off | EngineStatus::Failed) {
            continue;
        }
        let tanks = [first(&e.feeds[0], state, infinite), first(&e.feeds[1], state, infinite)];
        if !infinite && tanks.iter().any(Option::is_none) {
            continue;
        }
        let (thrust, mdot) = steady(&e.data, level, p);
        for (t, share) in tanks.iter().zip(shares(&e.data)) {
            if let Some(t) = t {
                draws[*t] += mdot * share;
            }
        }
        running.push((i, thrust, mdot, tanks));
    }
    // Tanks that cannot supply the whole tick share out what they hold.
    let supply: Vec<f64> = (0..draws.len())
        .map(|t| {
            if infinite || draws[t] * dt <= state.tanks[t].liquid {
                1.0
            } else {
                state.tanks[t].liquid / (draws[t] * dt)
            }
        })
        .collect();
    let mut out = ShipThrust {
        draws: vec![0.0; def.tanks.len()],
        per_engine: vec![0.0; def.engines.len()],
        gimbal: f64::INFINITY,
        ..ShipThrust::default()
    };
    let (mut pos, mut dir) = (DVec3::ZERO, DVec3::ZERO);
    for (i, thrust, mdot, tanks) in running {
        let e = &def.engines[i];
        let f = tanks.iter().flatten().map(|&t| supply[t]).fold(1.0, f64::min);
        let (thrust, mdot) = (thrust * f, mdot * f);
        for (t, share) in tanks.iter().zip(shares(&e.data)) {
            if let Some(t) = t {
                out.draws[*t] += mdot * share;
            }
        }
        out.per_engine[i] = thrust;
        out.thrust += thrust;
        out.mdot += mdot;
        out.heat += nozzle_heat(&e.data, thrust, mdot);
        pos += e.data.pos * thrust;
        dir += e.data.dir.normalize() * thrust;
        if thrust > 0.0 {
            out.gimbal = out.gimbal.min(e.data.gimbal_deg * math::PI / 180.0);
        }
    }
    if out.thrust > 0.0 {
        out.pos = pos / out.thrust;
        out.dir = dir.normalize();
    } else {
        out.dir = DVec3::Z;
        out.gimbal = 0.0;
    }
    out
}

/// Oxidizer and fuel shares of an engine's flow.
fn shares(d: &EngineData) -> [f64; 2] {
    let mr = d.mixture_ratio;
    [mr / (1.0 + mr), 1.0 / (1.0 + mr)]
}

/// Heat into a radiatively cooled nozzle's wall (W, D077): its share of
/// the jet power ½·F·vₑ.
pub fn nozzle_heat(d: &EngineData, thrust: f64, mdot: f64) -> f64 {
    if mdot > 0.0 {
        d.nozzle_heat_fraction * 0.5 * thrust * thrust / mdot
    } else {
        0.0
    }
}

/// What an engine's step reads besides its own state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    pub at: Epoch,
    pub dt: f64,
    /// The pilot's throttle.
    pub throttle: f64,
    /// Debug mode (D064): no failures, no depletion.
    pub debug: bool,
}

/// Why an engine stops (not by command).
#[derive(Clone, Debug, PartialEq)]
pub enum Stop {
    /// Its start was aborted (the ignition is spent; it can try again).
    Abort(String),
    /// It stopped running (a flameout or the controller's cut; restartable).
    Flameout(String),
    /// It failed and will not run again.
    Fail(String),
}

/// Stops engine `i` for `why` now (the controller's cut: no tail-off).
pub fn stop(def: &ShipDef, state: &mut ShipState, i: usize, why: Stop, at: Epoch) {
    let (status, text, alarm) = match why {
        Stop::Abort(t) => (EngineStatus::Off, format!("start aborted: {t}"), true),
        Stop::Flameout(t) => (EngineStatus::Off, t, false),
        Stop::Fail(t) => (EngineStatus::Failed, format!("failed: {t}"), true),
    };
    let e = &mut state.engines[i];
    e.status = status;
    e.timer = 0.0;
    let run = &mut state.runtime.engines[i];
    run.level = 0.0;
    run.cutoff = 0.0;
    run.reason = Some(text.clone());
    state.runtime.event(at, format!("{}: {text}", def.engines[i].id), alarm);
}

/// Advances every engine over a tick (`env`) with its tanks' conditions
/// (`feeds`, `plumbing::feeds`): the transients, depletion, gas ingestion
/// and cavitation, the run time; then the pad's armed release. Returns the
/// effects the vessel acts on (the clamps' release).
pub fn step(def: &ShipDef, state: &mut ShipState, env: &Env, feeds: &[Feed]) -> Vec<Effect> {
    state.runtime.refit(def);
    for i in 0..def.engines.len() {
        if !env.debug && inlet_trouble(def, state, i, env, feeds) {
            continue;
        }
        step_engine(def, state, i, env);
    }
    release(def, state, env.at)
}

/// An engine starting or running on a tank whose outlet is not covered
/// ingests gas; below its suction pressure it cavitates (D105 §6.2,
/// falcon9.md §3.4–3.5): a start is aborted (the ignition spent, it may try
/// again once settled), a running engine is cut by its controller (benign:
/// restartable). Returns whether the engine stopped.
fn inlet_trouble(def: &ShipDef, state: &mut ShipState, i: usize, env: &Env, feeds: &[Feed]) -> bool {
    let d = &def.engines[i];
    let status = state.engines[i].status;
    if !matches!(status, EngineStatus::Starting | EngineStatus::Running) {
        return false;
    }
    let level = state.runtime.engines[i].level;
    let mut trouble = None;
    for (list, need) in d.feeds.iter().zip([d.data.npsp_ox, d.data.npsp_fuel]) {
        let Some(t) = first(list, state, false) else { continue };
        let Some(f) = feeds.get(t) else { continue };
        if !f.covered {
            trouble = Some(format!("gas ingestion: {} not settled", def.tanks[t].id));
        } else if need > 0.0 && f.npsp(level, d.data.pos) < need {
            let npsp = f.npsp(level, d.data.pos) / 1e3;
            trouble = Some(format!("cavitation: NPSP {npsp:.0} kPa from {} < {:.0} kPa", def.tanks[t].id, need / 1e3));
        }
        if trouble.is_some() {
            break;
        }
    }
    let Some(why) = trouble else { return false };
    let stop_as = if status == EngineStatus::Starting { Stop::Abort(why) } else { Stop::Flameout(why) };
    stop(def, state, i, stop_as, env.at);
    true
}

fn step_engine(def: &ShipDef, state: &mut ShipState, i: usize, env: &Env) {
    let d: &EngineDef = &def.engines[i];
    let e = &state.engines[i];
    let (status, timer, own) = (e.status, e.timer + env.dt, e.throttle);
    if matches!(status, EngineStatus::Off | EngineStatus::Failed) {
        state.runtime.engines[i].level = 0.0;
        return;
    }
    let dry = |list: &[u32]| first(list, state, env.debug).is_none();
    if status != EngineStatus::ShuttingDown && (dry(&d.feeds[0]) || dry(&d.feeds[1])) {
        stop(def, state, i, Stop::Flameout("propellant depleted".into()), env.at);
        return;
    }
    let target = setting(&d.data, env.throttle, own);
    let (next, level) = match status {
        // (Within a nanosecond: ticks of 20 ms do not sum to whole seconds.)
        EngineStatus::Starting if timer >= d.data.start_time - 1e-9 => (EngineStatus::Running, target),
        EngineStatus::Starting => (EngineStatus::Starting, target * ramp(timer / d.data.start_time)),
        EngineStatus::Running => (EngineStatus::Running, target),
        _ => {
            let tau = tail_off(&d.data);
            let cutoff = state.runtime.engines[i].cutoff;
            if tau <= 0.0 || timer >= 5.0 * tau {
                (EngineStatus::Off, 0.0)
            } else {
                (EngineStatus::ShuttingDown, cutoff * math::exp(-timer / tau))
            }
        }
    };
    let e = &mut state.engines[i];
    e.timer = if next == status { timer } else { 0.0 };
    e.status = next;
    state.runtime.engines[i].level = level;
    if matches!(next, EngineStatus::Running | EngineStatus::Starting) {
        if let Some(p) = state.parts.iter_mut().find(|p| p.part == d.part) {
            p.history.run_time += env.dt;
        }
    }
}

/// The pad's armed release (see the module).
fn release(def: &ShipDef, state: &mut ShipState, at: Epoch) -> Vec<Effect> {
    if !state.runtime.release_armed || !state.clamped {
        state.runtime.release_armed = false;
        return Vec::new();
    }
    if state.runtime.release_engines.is_empty() {
        state.runtime.release_engines = lit(state);
        if state.runtime.release_engines.is_empty() {
            return Vec::new();
        }
    }
    let waits: Vec<usize> = state.runtime.release_engines.iter().map(|&i| i as usize).collect();
    let bad =
        waits.iter().find(|&&i| !matches!(state.engines[i].status, EngineStatus::Starting | EngineStatus::Running));
    if let Some(&i) = bad {
        let why = state.runtime.engines[i].reason.clone().unwrap_or_else(|| "stopped".into());
        for j in lit(state) {
            let (e, run) = (&mut state.engines[j as usize], &mut state.runtime.engines[j as usize]);
            e.status = EngineStatus::ShuttingDown;
            e.timer = 0.0;
            run.cutoff = run.level;
            run.reason = Some("pad abort".into());
        }
        state.runtime.release_armed = false;
        state.runtime.release_engines.clear();
        state.runtime.event(at, format!("Pad abort: {} {why}; clamps held", def.engines[i].id), true);
        return Vec::new();
    }
    let rated = |i: usize| {
        state.engines[i].status == EngineStatus::Running
            && state.runtime.engines[i].level >= def.engines[i].data.throttle_max * (1.0 - 1e-9)
    };
    if !waits.iter().all(|&i| rated(i)) {
        return Vec::new();
    }
    state.runtime.release_armed = false;
    state.runtime.release_engines.clear();
    state.runtime.event(at, "Liftoff: the clamps released at full thrust", false);
    state.apply(def, &ShipCommand::ReleaseClamps).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merlin() -> EngineData {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        let def = crate::craft::load_definition(&path).unwrap();
        let ship = def.ship.unwrap();
        ship.engines.iter().find(|e| e.id == "s1.merlin.9").unwrap().data.clone()
    }

    /// The steady map against falcon9.md §2.1: 914 kN in vacuum, 845 kN at
    /// sea level (Aₑ 0.681 m²), 299.7 kg/s; at 40 % at sea level
    /// 0.4·914 − 69.0 = 296.6 kN (the back pressure does not throttle).
    #[test]
    fn the_steady_map_table() {
        let d = merlin();
        // (level, p) → (thrust kN, flow kg/s)
        let cases = [
            (1.0, 0.0, 914.0, 299.7),
            (1.0, P_SEA_LEVEL, 845.0, 299.7),
            (0.4, P_SEA_LEVEL, 296.6, 119.9),
            (0.4, 0.0, 365.6, 119.9),
            (0.05, P_SEA_LEVEL, 0.0, 15.0), // fully separated: no thrust
        ];
        for (level, p, thrust, flow) in cases {
            let (f, m) = steady(&d, level, p);
            assert!((f / 1e3 - thrust).abs() < 0.2, "{level} {p}: {f}");
            assert!((m - flow).abs() < 0.1, "{level} {p}: {m}");
        }
        // Range: zero throttle is the minimum, never off.
        assert_eq!((setting(&d, 0.0, 1.0), setting(&d, 0.7, 1.0), setting(&d, 2.0, 1.0)), (0.4, 0.7, 1.0));
        assert!((tail_off(&d) - 2.1e5 / 914e3).abs() < 1e-12);
        assert_eq!((ramp(-1.0), ramp(0.5), ramp(2.0)), (0.0, 0.5, 1.0));
    }
}
