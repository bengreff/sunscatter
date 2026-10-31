//! The plumbing through a coast (D119, `super::super::coast_law`): each
//! tank's heat as a law of its liquid's height and temperature under
//! conditions held over a span, and what the tanks and bottles start a span
//! from (their gas relaxed, topped up and relieved).

use super::*;
use crate::craft::state::BottleState;

/// A tank's heat through a coast's span (D119, `super::coast_law`), its
/// conditions held: the sunlight its wetted wall absorbs, `side` W per
/// metre of the liquid's height plus `end` W through its end, averaged over
/// the span's sunlight samples as [`heat_in`] takes one; less `emit`·A·T⁴
/// (εσ, the wetted area A) in vacuum, or with the film `H_AIR`·A·(air − T)
/// in air. Nothing reaches a tank its component's heaters hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HeatLaw {
    pub side: f64,
    pub end: f64,
    pub emit: f64,
    pub air_k: Option<f64>,
    pub held: bool,
}

impl HeatLaw {
    /// Heat into the liquid (W) at height `h` (m) and temperature `k` (K) in
    /// a tank of radius `r`.
    pub fn at(&self, r: f64, h: f64, k: f64) -> f64 {
        if self.held {
            return 0.0;
        }
        let wetted = 2.0 * math::PI * r * h + math::PI * r * r;
        let sunlit = self.side * h + self.end;
        match self.air_k {
            Some(air) => H_AIR * wetted * (air - k) + sunlit,
            None => sunlit - self.emit * wetted * math::powi(k, 4),
        }
    }
}

/// Tank `i`'s [`HeatLaw`] under the sunlight samples `suns` (W/m², along
/// the light, body axes; their mean is taken) and the air's temperature.
pub fn heat_law(def: &ShipDef, i: usize, suns: &[DVec3], air_k: Option<f64>) -> HeatLaw {
    let t = &def.tanks[i];
    let (alpha, epsilon) = optics(def, t);
    let (r, axis) = (t.region.radius, t.region.axis.normalize());
    let (mut side, mut end) = (0.0, 0.0);
    for sun in suns {
        let flux = sun.length();
        if flux > 0.0 {
            let c = (*sun / flux).dot(axis).abs().min(1.0);
            side += flux * math::sqrt_pos(1.0 - c * c);
            end += flux * c;
        }
    }
    let n = suns.len().max(1) as f64;
    HeatLaw {
        side: alpha * 2.0 * r * side / n,
        end: alpha * math::PI * r * r * end / n,
        emit: epsilon * STEFAN_BOLTZMANN,
        air_k,
        held: heated_inside(t),
    }
}

/// Brings the tanks and bottles to what a coast's span starts from (D119):
/// each ullage's gas and each bottle in a liquid at that liquid's
/// temperature (their relaxation, τ 300 s and 60 s, done), each ullage
/// topped up to its loading pressure from its bottles at that temperature
/// (the regulator), then relieved ([`relieve`]). Returns, per tank, the
/// mass vented (kg) and the pressurant let in (mol).
pub fn equilibrate(def: &ShipDef, tanks: &mut [TankState], bottles: &mut [BottleState]) -> Vec<(f64, f64)> {
    for (t, s) in def.tanks.iter().zip(tanks.iter_mut()) {
        keep_physical(t, s);
        s.ullage_k = s.liquid_k;
    }
    for (b, bs) in def.bottles.iter().zip(bottles.iter_mut()) {
        let around = (def.tanks.iter().zip(tanks.iter())).find(|(t, s)| {
            let g = geometry(t, s);
            t.part == b.part && (b.pos - t.region.base).dot(g.axis) <= g.height
        });
        if let Some((_, s)) = around {
            bs.k = s.liquid_k;
        }
    }
    (0..def.tanks.len())
        .map(|i| {
            let n = top_up(def, tanks, bottles, i);
            let out = relieve(&def.tanks[i], &mut tanks[i]);
            keep_physical(&def.tanks[i], &mut tanks[i]);
            (out, n)
        })
        .collect()
}

/// The regulator's top-up of tank `i`'s ullage, at its liquid's temperature,
/// to its loading pressure from the bottles plumbed to it that hold more
/// (each blowing down isentropically, never below that pressure). Returns
/// the moles let in.
fn top_up(def: &ShipDef, tanks: &mut [TankState], bottles: &mut [BottleState], i: usize) -> f64 {
    let t = &def.tanks[i];
    if fluids().gas(&t.load.pressurant).is_err() {
        return 0.0;
    }
    let g = geometry(t, &tanks[i]);
    let k = tanks[i].liquid_k;
    let want = (t.load.pressure - vapour(t, &tanks[i], g.ullage)) * g.ullage / (R_GAS * k) - tanks[i].ullage_moles;
    let mut let_in = 0.0;
    for b in bottles_of(def, i) {
        let (bd, bs) = (&def.bottles[b], &mut bottles[b]);
        let Ok(gas) = fluids().gas(&bd.gas) else { continue };
        if bd.gas != t.load.pressurant || bs.moles <= 0.0 || want - let_in <= 0.0 {
            continue;
        }
        // Moles above the loading pressure, isentropically: p ∝ n^γ.
        let p = bs.moles * R_GAS * bs.k / bd.volume;
        if p <= t.load.pressure {
            continue;
        }
        let floor = bs.moles * math::pow(t.load.pressure / p, 1.0 / gas.gamma);
        let dn = (want - let_in).min(bs.moles - floor);
        let n1 = bs.moles - dn;
        bs.k *= math::pow(n1 / bs.moles, gas.gamma - 1.0);
        bs.moles = n1;
        let_in += dn;
    }
    tanks[i].ullage_moles += let_in;
    let_in
}

/// The liquids floating in free fall for `dt` (a coast's lattice step):
/// [`settle`] at no acceleration, dispersing towards each tank's middle.
pub fn float(def: &ShipDef, state: &mut ShipState, dt: f64) {
    for i in 0..def.tanks.len() {
        let g = geometry(&def.tanks[i], &state.tanks[i]);
        settle(&mut state.tanks[i], &def.tanks[i], &g, 0.0, dt);
    }
}
