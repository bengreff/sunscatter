//! A craft of parts' slow changes through a coast as laws of time (D119;
//! ship-model.md §5.10): what its tanks and bottles hold at any time of a
//! coast is evaluated, not stepped, so that the coast's segment (the mass
//! its perturbations divide by, `vessel::limbo`) and the ship state on the
//! systems lattice read the same law: one mass truth (F2), bit for bit.
//!
//! A law runs in spans of at least [`SPAN`]: each starts at one of the
//! segment's stored samples (`vessel::segment`), so it is the same however
//! the coast is chunked (rule 4). A span holds its conditions: each tank's
//! heat ([`HeatLaw`]) from the sunlight at its start, orbit-averaged with
//! the attitude's turn like the coast thermal lattice
//! (`vessel::coast_heat::law_conditions`), and each liquid's height at its
//! start. A span starts from equilibrium (`plumbing::equilibrate`: the gas
//! at its liquid's temperature, the regulator's top-up, relief). Then each
//! tank, sealed:
//!
//! 1. **Warms or cools** at its mass: m·c·dT/dt = Q(T). In vacuum
//!    Q = S − K·T⁴ (S the sunlight absorbed, K = εσ·A the wetted wall's
//!    emission): with T_e = (S/K)^¼ and x = T/T_e,
//!    1/(T_e⁴ − T⁴) = [1/(T_e² − T²) + 1/(T_e² + T²)]/(2T_e²), so
//!    t(T) = m·c·[F(x) − F(x₀)]/(2K·T_e³), F(x) = atanh x + atan x
//!    (atanh as ½·ln|(1 + x)/(1 − x)| above T_e), inverted by Newton's
//!    method kept inside its bracket; without sunlight
//!    1/T³ = 1/T₀³ + 3K·t/(m·c); in air (the film H·A, [`HeatLaw`])
//!    T relaxes exponentially to T_a + S/(H·A).
//! 2. **Vents its pressurant** from where its pressure (the gas's and the
//!    vapour's) reaches the relief pressure P_r (that temperature found by
//!    bisection): it keeps (P_r − p_v(T))·V_u/(R·T) as T rises by (1). The
//!    vapour the lattice let out with the gas is left to (3): the heat that
//!    would have evaporated it warms the liquid, which then boils the same
//!    mass at saturation (the energy is the same; only its timing moves).
//! 3. **Boils off** at saturation, T_s = T_sat(P_r), no gas left:
//!    L·dm/dt = −Q(m) with Q = a + b·m (the wetted wall and the sunlit
//!    side grow with the height m/(ρ_s·πR²)), so
//!    m(t) = m₀ − (a + b·m₀)·(t/L)·φ(b·t/L), φ(x) = (1 − e⁻ˣ)/x.
//!
//! The coarse grain, against the lattice's 60 s steps of the instant's
//! sunlight: the eclipse swing is averaged over the orbit, as the coast's
//! temperatures are; the height in (1)–(2) and the conditions are held for
//! a span; the regulator acts at span starts. A tank inside a component
//! (its heaters) takes no heat. Power, life support and the liquids'
//! drift stay on the lattice: they change no mass, and power's eclipses
//! and shedding are events no orbit average keeps.

use super::plumbing::{self, HeatLaw};
use crate::craft::ship::ShipDef;
use crate::craft::state::{BottleState, ShipState, TankState};
use crate::fluids::R_GAS;
use crate::math;
use serde::{Deserialize, Serialize};

/// The shortest span (s): the conditions are re-read hourly (a low orbit's
/// average changes by a degree of beta angle a day).
pub const SPAN: f64 = 3_600.0;

/// A coast's law of its tanks and bottles, from local time 0 (its
/// segment's start).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoastLaw {
    spans: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Span {
    /// Local time of its start (s).
    t: f64,
    tanks: Vec<TankLaw>,
    bottles: Vec<BottleState>,
    /// The bottles' gases' molar masses (kg/mol).
    molar: Vec<f64>,
}

/// One tank through a span (see the module).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TankLaw {
    /// Its state at the span's start, and its ledgers then (kg vented, mol
    /// let in).
    start: TankState,
    vented: f64,
    pressurant_in: f64,
    /// m·c (J/K); in vacuum S (W) and K (W/K⁴), in air (H·A W/K, T_a K).
    mc: f64,
    s: f64,
    k: f64,
    air: Option<(f64, f64)>,
    /// Local times (from the span's start) it reaches its relief pressure
    /// and saturation (∞: not in this span).
    relief: f64,
    saturated: f64,
    /// Saturation at the relief pressure (K); the boil-off's Q = a + b·m
    /// (W) up to the mass `full` (kg) that fills the tank's length (the
    /// height held there above it), and latent heat (J/kg); the liquid's
    /// range (K).
    t_s: f64,
    a: f64,
    b: f64,
    full: f64,
    latent: f64,
    range: (f64, f64),
    /// The pressurant's molar mass (kg/mol, 0 if unknown: the consumables').
    molar: f64,
}

impl CoastLaw {
    /// The law of `state`'s tanks and bottles from now (local time 0), its
    /// first span under `heat` (one per tank).
    pub fn start(def: &ShipDef, state: &ShipState, heat: &[HeatLaw]) -> Self {
        let ledgers = state.tanks.iter().map(|t| (t.vented, t.pressurant_in)).collect();
        let span = Span::new(def, 0.0, (state.tanks.clone(), state.bottles.clone(), ledgers), heat);
        CoastLaw { spans: vec![span] }
    }

    /// Whether a span may start at local time `t` (the last one is
    /// [`SPAN`] old).
    pub fn due(&self, t: f64) -> bool {
        t >= self.last().t + SPAN
    }

    /// Starts a span at local time `t` under `heat`.
    pub fn add_span(&mut self, def: &ShipDef, t: f64, heat: &[HeatLaw]) {
        let span = Span::new(def, t, self.contents(def, t), heat);
        self.spans.push(span);
    }

    /// The law from local time `t` on, as a new law whose time 0 is there
    /// (a coast continued in its next segment), under `heat`.
    pub fn continued(&self, def: &ShipDef, t: f64, heat: &[HeatLaw]) -> Self {
        CoastLaw { spans: vec![Span::new(def, 0.0, self.contents(def, t), heat)] }
    }

    /// Drops the spans that end before local time `t`.
    pub fn prune_before(&mut self, t: f64) {
        let keep = self.index(t);
        self.spans.drain(..keep);
    }

    /// The consumables at local time `t` (kg): the sum
    /// ([`crate::craft::state::consumables_sum`]) the ship state takes of
    /// what the law gives there ([`Self::apply`]), so the same bits.
    pub fn total(&self, def: &ShipDef, t: f64) -> f64 {
        let span = self.span(t);
        let tanks = (span.tanks.iter().enumerate()).map(|(i, l)| {
            let (kg, mol) = l.held(def, i, t - span.t);
            (kg, mol, l.molar)
        });
        let bottles = span.bottles.iter().zip(&span.molar).map(|(b, m)| (b.moles, *m));
        crate::craft::state::consumables_sum(tanks, bottles)
    }

    /// Sets `state`'s tanks, bottles and their ledgers to the law's at local
    /// time `t`.
    pub fn apply(&self, def: &ShipDef, state: &mut ShipState, t: f64) {
        let span = self.span(t);
        for (i, l) in span.tanks.iter().enumerate() {
            let (s, vented) = l.at(def, i, t - span.t);
            // The law gives the contents and the ledgers; where the liquid
            // floats is the lattice's (`plumbing_coast::float`).
            let now = &mut state.tanks[i];
            let (drift, drift_speed) = (now.drift, now.drift_speed);
            *now = TankState { drift, drift_speed, vented, pressurant_in: l.pressurant_in, ..s };
        }
        state.bottles.clone_from(&span.bottles);
    }

    /// The tanks, bottles and ledgers at local time `t`.
    fn contents(&self, def: &ShipDef, t: f64) -> (Vec<TankState>, Vec<BottleState>, Vec<(f64, f64)>) {
        let span = self.span(t);
        let (tanks, ledgers) = (span.tanks.iter().enumerate())
            .map(|(i, l)| {
                let (s, vented) = l.at(def, i, t - span.t);
                (s, (vented, l.pressurant_in))
            })
            .unzip();
        (tanks, span.bottles.clone(), ledgers)
    }

    fn index(&self, t: f64) -> usize {
        self.spans.partition_point(|s| s.t <= t).max(1) - 1
    }

    fn span(&self, t: f64) -> &Span {
        &self.spans[self.index(t)]
    }

    fn last(&self) -> &Span {
        self.spans.last().expect("a law has a span")
    }
}

impl Span {
    fn new(
        def: &ShipDef,
        t: f64,
        (mut tanks, mut bottles, ledgers): (Vec<TankState>, Vec<BottleState>, Vec<(f64, f64)>),
        heat: &[HeatLaw],
    ) -> Self {
        let done = plumbing::equilibrate(def, &mut tanks, &mut bottles);
        let tanks = (tanks.into_iter().enumerate())
            .map(|(i, s)| {
                let ((vented, n_in), (out, n)) = (ledgers[i], done[i]);
                TankLaw::new(def, i, s, (vented + out, n_in + n), &heat[i])
            })
            .collect();
        let molar = def.bottles.iter().map(|b| crate::craft::state::molar_mass(&b.gas)).collect();
        Span { t, tanks, bottles, molar }
    }
}

impl TankLaw {
    fn new(def: &ShipDef, i: usize, start: TankState, (vented, pressurant_in): (f64, f64), heat: &HeatLaw) -> Self {
        let t = &def.tanks[i];
        let molar = crate::craft::state::molar_mass(&t.load.pressurant);
        let k0 = start.liquid_k;
        let mut law = TankLaw {
            start,
            vented,
            pressurant_in,
            mc: 0.0,
            s: 0.0,
            k: 0.0,
            air: None,
            relief: f64::INFINITY,
            saturated: f64::INFINITY,
            t_s: k0,
            a: 0.0,
            b: 0.0,
            full: f64::INFINITY,
            latent: 1.0,
            range: (k0, k0),
            molar,
        };
        let Some(l) = plumbing::liquid_of(t) else { return law };
        if law.start.liquid <= 0.0 || heat.held {
            return law;
        }
        let g = plumbing::geometry(t, &law.start);
        let r = g.radius;
        let wetted = 2.0 * math::PI * r * g.height + math::PI * r * r;
        law.mc = law.start.liquid * l.specific_heat;
        law.s = heat.side * g.height + heat.end;
        match heat.air_k {
            Some(air) => law.air = Some((plumbing::H_AIR * wetted, air)),
            None => law.k = heat.emit * wetted,
        }
        law.t_s = l.saturation_temperature(t.pressure);
        law.latent = l.latent_heat;
        law.range = (l.freezing_k, l.critical_k);
        // Boiling: Q at saturation is linear in the height, so in the mass.
        let per_kg = 1.0 / (l.density(law.t_s) * math::PI * r * r);
        law.a = heat.at(r, 0.0, law.t_s);
        law.b = (heat.at(r, 1.0, law.t_s) - law.a) * per_kg;
        law.full = l.density(law.t_s) * t.volume;
        // Relief: the pressure rises with the temperature (vapour, gas, the
        // liquid's swelling); bisection between now and saturation.
        let pressure = |k: f64| plumbing::pressure(t, &law.sealed(k));
        let top = law.t_s;
        let relief_k = if pressure(k0) >= t.pressure {
            Some(k0)
        } else if pressure(top) < t.pressure {
            None
        } else {
            let (mut lo, mut hi) = (k0, top);
            for _ in 0..100 {
                let mid = 0.5 * (lo + hi);
                if mid <= lo || mid >= hi {
                    break;
                }
                if pressure(mid) >= t.pressure {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            Some(hi)
        };
        law.relief = relief_k.map_or(f64::INFINITY, |k| law.time_to(k));
        law.saturated = law.time_to(law.t_s);
        if law.saturated.is_finite() && law.a + law.b * law.start.liquid.min(law.full) <= 0.0 {
            // Saturated but losing heat: it subcools instead (stays as is).
            law.saturated = f64::INFINITY;
        }
        law
    }

    /// The tank sealed at liquid temperature `k` (its gas there too).
    fn sealed(&self, k: f64) -> TankState {
        TankState { liquid_k: k, ullage_k: k, ..self.start.clone() }
    }

    /// Local time (s) the warming (1) takes from the start to `k` (K); 0 if
    /// it is there, ∞ if it never gets there.
    fn time_to(&self, k: f64) -> f64 {
        let k0 = self.start.liquid_k;
        if k <= k0 {
            return 0.0;
        }
        if self.mc <= 0.0 {
            return f64::INFINITY;
        }
        let never = f64::INFINITY;
        if let Some((ha, air)) = self.air {
            let te = air + self.s / ha;
            let ratio = (k - te) / (k0 - te);
            return if ratio > 0.0 && ratio <= 1.0 { -self.mc / ha * math::ln(ratio) } else { never };
        }
        if self.s <= 0.0 {
            return never;
        }
        if self.k <= 0.0 {
            return (k - k0) * self.mc / self.s;
        }
        let te = math::sqrt_pos(math::sqrt_pos(self.s / self.k));
        if k >= te {
            return never;
        }
        self.mc * (big_f(k / te) - big_f(k0 / te)) / (2.0 * self.k * te * te * te)
    }

    /// The liquid's temperature by (1) at local time `tau` (K).
    fn temperature(&self, tau: f64) -> f64 {
        let k0 = self.start.liquid_k;
        if self.mc <= 0.0 || tau <= 0.0 {
            return k0;
        }
        let k = if let Some((ha, air)) = self.air {
            let te = air + self.s / ha;
            te + (k0 - te) * math::exp(-ha * tau / self.mc)
        } else if self.k <= 0.0 {
            k0 + self.s * tau / self.mc
        } else if self.s <= 0.0 {
            1.0 / math::cbrt(1.0 / (k0 * k0 * k0) + 3.0 * self.k * tau / self.mc)
        } else {
            let te = math::sqrt_pos(math::sqrt_pos(self.s / self.k));
            let x0 = k0 / te;
            if x0 == 1.0 {
                return k0;
            }
            let target = big_f(x0) + 2.0 * self.k * te * te * te * tau / self.mc;
            te * invert_f(x0, target)
        };
        k.clamp(self.range.0, self.range.1)
    }

    /// The liquid (kg) and the ullage's gas (mol) at local time `tau`.
    fn held(&self, def: &ShipDef, i: usize, tau: f64) -> (f64, f64) {
        let (m0, n0) = (self.start.liquid, self.start.ullage_moles);
        if tau < self.relief {
            (m0, n0)
        } else if tau < self.saturated {
            (m0, self.gas_kept(def, i, self.temperature(tau).min(self.t_s)))
        } else {
            (self.boiled(tau - self.saturated), 0.0)
        }
    }

    /// The tank's state and its vented ledger (kg) at local time `tau`.
    fn at(&self, def: &ShipDef, i: usize, tau: f64) -> (TankState, f64) {
        let (m, n) = self.held(def, i, tau);
        let k = if tau < self.relief {
            self.temperature(tau)
        } else if tau < self.saturated {
            self.temperature(tau).min(self.t_s)
        } else {
            self.t_s
        };
        let s = TankState { liquid: m, ullage_moles: n, ..self.sealed(k) };
        let vented = self.vented + (self.start.liquid - m) + (self.start.ullage_moles - n) * self.molar;
        (s, vented)
    }

    /// The gas (mol) kept at the relief pressure at liquid temperature `k`
    /// (2): what `plumbing::relieve` keeps.
    fn gas_kept(&self, def: &ShipDef, i: usize, k: f64) -> f64 {
        let t = &def.tanks[i];
        let s = self.sealed(k);
        let g = plumbing::geometry(t, &s);
        let vapour = plumbing::vapour(t, &s, g.ullage);
        ((t.pressure - vapour).max(0.0) * g.ullage / (R_GAS * k)).clamp(0.0, self.start.ullage_moles)
    }

    /// The liquid (kg) `tau` after saturation (3): above the mass that
    /// fills the tank's length the heat is that of the full length (a
    /// constant boil-off down to it), below it linear in the mass.
    fn boiled(&self, tau: f64) -> f64 {
        let m0 = self.start.liquid;
        if tau <= 0.0 {
            return m0;
        }
        let (mut m, mut tau) = (m0, tau);
        if m > self.full {
            let q = self.a + self.b * self.full;
            if q <= 0.0 {
                return m0;
            }
            let down = (m - self.full) * self.latent / q;
            if tau <= down {
                return m - q * tau / self.latent;
            }
            (m, tau) = (self.full, tau - down);
        }
        let q = self.a + self.b * m;
        if q <= 0.0 {
            return m;
        }
        let x = self.b * tau / self.latent;
        let phi = if x == 0.0 { 1.0 } else { -math::expm1(-x) / x };
        (m - q * tau / self.latent * phi).max(0.0)
    }
}

/// F(x) = atanh x + atan x, atanh as ½·ln|(1 + x)/(1 − x)| (see the module).
fn big_f(x: f64) -> f64 {
    0.5 * math::ln(((1.0 + x) / (1.0 - x)).abs()) + math::atan(x)
}

/// The x on the same side of 1 as `x0` where F(x) = `target` (≥ F(x0) below
/// 1, F falling above it): Newton's method, F′ = 2/(1 − x⁴), kept inside
/// the bracket between `x0` and 1 (bisecting when a step leaves it).
fn invert_f(x0: f64, target: f64) -> f64 {
    // f = F − target: negative at x0, +∞ at 1 from either side.
    let (mut neg, mut pos) = (x0, 1.0);
    let mut x = x0;
    for _ in 0..100 {
        let f = big_f(x) - target;
        if f == 0.0 {
            return x;
        }
        if f < 0.0 {
            neg = x;
        } else {
            pos = x;
        }
        let x4 = x * x * x * x;
        let newton = x - f * (1.0 - x4) / 2.0;
        let (lo, hi) = if neg < pos { (neg, pos) } else { (pos, neg) };
        let next = if newton > lo && newton < hi { newton } else { 0.5 * (lo + hi) };
        if next == x || (next - x).abs() <= 1e-15 * x {
            return next;
        }
        x = next;
    }
    x
}

#[cfg(test)]
#[path = "coast_law_tests.rs"]
mod tests;
