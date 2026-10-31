//! The physical triggers of a tick (ship-model.md §6.2, §6.5): time at
//! temperature, vibration, bursts, and the blasts they make.

use super::blast::{self, Pair};
use super::damage::{self, ThermalRating};
use crate::craft::ship::ShipDef;
use crate::craft::state::{PartFailure, ShipState};
use crate::fluids::fluids;
use crate::systems::structure::Bodies;
use crate::time::Epoch;
use glam::DVec3;

/// A part's thermal rating: its rated (service) temperature, its
/// destruction temperature (its own, else its material's), its hold.
pub fn thermal_rating(def: &ShipDef, part: u32) -> Option<ThermalRating> {
    let p = def.parts.iter().find(|p| p.part == part)?;
    let material = crate::materials::materials().get(&p.material).ok();
    let destruction_k = p.ratings.destruction_k.or(material.map(|m| m.destruction_k))?;
    let rated_k = p.ratings.rated_k.or(material.map(|m| m.service_k))?;
    let hold_s = p.ratings.time_at_temperature.unwrap_or(damage::DEFAULT_HOLD_S);
    Some(ThermalRating { rated_k, destruction_k, hold_s })
}

/// Time at temperature over a tick: each part's interior at its hottest
/// node (`b.hottest`) takes its damage; at its destruction temperature or at
/// a damage of 1 it is destroyed. Each part's peak interior temperature
/// goes into its history. (Its skin cells are judged against their own
/// materials' destruction temperatures by `vessel::breakage`: a cell's
/// service temperature is not data.)
pub fn thermal(def: &ShipDef, state: &mut ShipState, b: &Bodies, dt: f64) -> Vec<(u32, PartFailure)> {
    let mut out = Vec::new();
    for (k, &id) in b.ids.iter().enumerate() {
        let Some(&t) = b.hottest.get(k) else { continue };
        let Some(s) = state.parts.iter_mut().find(|p| p.part == id) else { continue };
        s.history.peak_k = s.history.peak_k.max(t);
        if s.integrity <= 0.0 {
            continue;
        }
        let Some(r) = thermal_rating(def, id) else { continue };
        s.thermal_damage = (s.thermal_damage + r.damage(t, dt)).min(1.0);
        if s.thermal_damage >= 1.0 {
            out.push((id, PartFailure::Melted { k: t }));
        }
    }
    out
}

/// Vibration over a tick: each part's environment (the running engines at
/// their mounts by their level, buffeting from `b.q` and `b.mach`) against
/// its qualification (Miner's rule). Returns the parts whose damage reached
/// 1 this tick.
pub fn vibration(def: &ShipDef, state: &mut ShipState, b: &Bodies, dt: f64) -> Vec<u32> {
    let index = |id: u32| b.ids.iter().position(|&x| x == id);
    let sources: Vec<(usize, f64)> = def
        .engines
        .iter()
        .zip(&state.engines)
        .filter(|(_, r)| r.level > 0.0)
        .filter_map(|(e, r)| Some((index(e.part)?, damage::ENGINE_GRMS * r.level.min(1.0))))
        .collect();
    let own = vec![damage::buffet(b.q, b.mach); b.ids.len()];
    let parents: Vec<Option<usize>> = b.parents.iter().map(|p| p.map(|p| p as usize)).collect();
    let g = damage::environment(&parents, &own, &sources);
    let mut out = Vec::new();
    for (k, &id) in b.ids.iter().enumerate() {
        let Some(p) = def.parts.iter().find(|p| p.part == id) else { continue };
        let (Some(qual), Some(qual_s)) = (p.ratings.vibration_grms, p.ratings.vibration_s) else { continue };
        let Some(s) = state.parts.iter_mut().find(|s| s.part == id) else { continue };
        let before = s.vibration_damage;
        s.vibration_damage += damage::miner(g[k], qual, qual_s, dt);
        if before < 1.0 && s.vibration_damage >= 1.0 {
            out.push(id);
        }
    }
    out
}

/// An explosion: its centre (body axes) and its TNT equivalent (kg).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Explosion {
    pub at: DVec3,
    pub tnt: f64,
}

/// The distance from `x` to a segment from `a` along `axis`, less `radius`.
fn to_cylinder(x: DVec3, a: DVec3, axis: DVec3, radius: f64) -> f64 {
    let len2 = axis.length_squared();
    let t = if len2 > 0.0 { ((x - a).dot(axis) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((x - (a + axis * t)).length() - radius).max(0.0)
}

/// A tank's ullage pressure (Pa): the plumbing's.
fn tank_pressure(def: &ShipDef, state: &ShipState, i: usize) -> f64 {
    crate::systems::plumbing::pressure(&def.tanks[i], &state.tanks[i])
}

/// A bottle's pressure (Pa).
fn bottle_pressure(def: &ShipDef, state: &ShipState, i: usize) -> f64 {
    let (d, s) = (&def.bottles[i], &state.bottles[i]);
    crate::fluids::pressure(s.moles, d.volume, s.k)
}

/// The gas energy of tank `i` (its ullage) as TNT (kg), in `ambient`.
fn tank_tnt(def: &ShipDef, state: &ShipState, i: usize, ambient: f64) -> f64 {
    let (t, s) = (&def.tanks[i], &state.tanks[i]);
    let volume = crate::systems::plumbing::geometry(t, s).ullage;
    let gamma = fluids().gas(&t.load.pressurant).map_or(1.4, |g| g.gamma);
    blast::tnt_of(blast::gas_energy(tank_pressure(def, state, i), volume, gamma, ambient))
}

/// The propellants a destroyed part releases together: its tanks' liquids
/// when they hold an oxidizer and a fuel of a published pair, at rest (a
/// rupture in flight: LOX/RP-1 0, hypergols 2 %), as TNT (kg).
fn mixing_tnt(def: &ShipDef, state: &ShipState, part: u32) -> f64 {
    let held: Vec<(&str, f64)> = (def.tanks.iter().zip(&state.tanks))
        .filter(|(t, _)| t.part == part)
        .map(|(t, s)| (t.fluid.as_str(), s.liquid))
        .collect();
    let mut best = 0.0f64;
    for &(ox, mo) in &held {
        for &(fu, mf) in &held {
            if let Some(pair) = Pair::of(ox, fu) {
                best = best.max(blast::yield_factor(pair, 0.0) * (mo + mf));
            }
        }
    }
    best
}

/// Bursts and their blasts over a tick (see the module): every tank and
/// bottle over its burst pressure bursts; each burst is an explosion whose
/// overpressure reaches the tank parts and bottles by distance, and those it
/// overpowers burst in turn. Returns the parts destroyed (with why) and
/// logs each event; burst bottles lose their gas.
pub fn bursts(def: &ShipDef, state: &mut ShipState, ambient: f64, at: Epoch) -> Vec<(u32, PartFailure)> {
    let mut destroyed: Vec<(u32, PartFailure)> = Vec::new();
    let mut burst_bottles = vec![false; def.bottles.len()];
    let mut queue: Vec<Explosion> = Vec::new();
    for (i, t) in def.tanks.iter().enumerate() {
        let p = tank_pressure(def, state, i);
        if t.burst > 0.0 && p >= t.burst && !destroyed.iter().any(|d| d.0 == t.part) {
            state.runtime.event(at, format!("{}: burst at {:.1} bar", t.id, p / 1e5), true);
            destroyed.push((t.part, PartFailure::Burst { pa: p }));
            let tnt = tank_tnt(def, state, i, ambient) + mixing_tnt(def, state, t.part);
            queue.push(Explosion { at: t.region.base + t.region.axis * 0.5, tnt });
        }
    }
    for (i, d) in def.bottles.iter().enumerate() {
        let p = bottle_pressure(def, state, i);
        if d.burst > 0.0 && p >= d.burst {
            state.runtime.event(at, format!("{}: burst at {:.0} bar", d.id, p / 1e5), true);
            burst_bottles[i] = true;
            let gamma = fluids().gas(&d.gas).map_or(1.4, |g| g.gamma);
            queue.push(Explosion { at: d.pos, tnt: blast::tnt_of(blast::gas_energy(p, d.volume, gamma, ambient)) });
        }
    }
    // The cascade: each explosion against what has not failed yet.
    let mut k = 0;
    while k < queue.len() && k < 256 {
        let e = queue[k];
        k += 1;
        for (i, t) in def.tanks.iter().enumerate() {
            if destroyed.iter().any(|d| d.0 == t.part) {
                continue;
            }
            let rating = def.parts.iter().find(|p| p.part == t.part).and_then(|p| p.ratings.burst_pressure);
            let Some(rating) = rating.or((t.burst > 0.0).then_some(t.burst)) else { continue };
            // Inside a tank the blast travels in its own gas and liquid.
            let medium = ambient.max(tank_pressure(def, state, i));
            let r = to_cylinder(e.at, t.region.base, t.region.axis, t.region.radius);
            let p = blast::overpressure(r, e.tnt, medium);
            if p >= rating {
                state.runtime.event(at, format!("{}: ruptured by a blast of {:.1} bar", t.id, p / 1e5), true);
                destroyed.push((t.part, PartFailure::Blast { pa: p }));
                let tnt = tank_tnt(def, state, i, ambient) + mixing_tnt(def, state, t.part);
                queue.push(Explosion { at: t.region.base + t.region.axis * 0.5, tnt });
            }
        }
        for (i, d) in def.bottles.iter().enumerate() {
            if burst_bottles[i] || d.burst <= 0.0 {
                continue;
            }
            // A bottle in a tank's liquid feels the blast through it.
            let medium = (def.tanks.iter().enumerate())
                .filter(|(_, t)| t.part == d.part)
                .map(|(j, _)| tank_pressure(def, state, j))
                .fold(ambient, f64::max);
            let p = blast::overpressure((d.pos - e.at).length(), e.tnt, medium);
            if p >= d.burst {
                state.runtime.event(at, format!("{}: ruptured by a blast", d.id), true);
                burst_bottles[i] = true;
                let gamma = fluids().gas(&d.gas).map_or(1.4, |g| g.gamma);
                let q = bottle_pressure(def, state, i);
                queue.push(Explosion { at: d.pos, tnt: blast::tnt_of(blast::gas_energy(q, d.volume, gamma, ambient)) });
            }
        }
    }
    for (i, b) in burst_bottles.iter().enumerate() {
        if *b {
            state.bottles[i].moles = 0.0;
        }
    }
    destroyed
}
