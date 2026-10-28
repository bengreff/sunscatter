//! The shock layer at the stagnation point (D109): the state of the gas
//! behind a normal shock in chemical equilibrium (temperature, pressure,
//! density, electron density), the shock's standoff from the nose, and a
//! vessel's [`ShockLayer`] in flight.
//!
//! * **The jump:** the Rankine–Hugoniot conservation of mass, momentum and
//!   enthalpy across the shock, ρ₁V = ρ₂u₂, p₂ = p₁ + ρ₁V²(1 − ε),
//!   h₂ = h₁ + ½V²(1 − ε²) with ε = ρ₁/ρ₂, closed by equilibrium air
//!   ([`super::equilibrium`]): fixed-point on ε, the temperature for the
//!   enthalpy at the pressure by the Illinois method. The method of Hunt &
//!   Souders (NASA SP-3093) and Anderson (*Hypersonic and High-Temperature
//!   Gas Dynamics*, §14.6). Real air's temperature saturates where a
//!   perfect gas's grows as V²: dissociation and ionisation take the energy.
//! * **The table:** the jump is solved once per gas, at first use, on a
//!   grid of flight speed (1–16 km/s every 250 m/s) and free-stream density
//!   (10⁻⁸–1 kg/m³ every half decade) from a free stream at 230 K
//!   ([`FREE_STREAM_K`]; the free stream's own enthalpy is under 2 % of the
//!   flow's above 3 km/s), and read bilinearly (the electron density's
//!   logarithm). Air and Titan's nitrogen; not CO₂ (no carbon species).
//! * **Standoff:** Olivier's sphere relation (J. Fluid Mech. 413, 2000),
//!   Δ/R_s = ε/(1 + √(2ε)), R_s = R_n + Δ, at the equilibrium ε: within
//!   10 % of Lobb's ballistic-range fit Δ/R_n = 0.78ε (1964). Inviscid
//!   continuum; the same nose radius as the heating (the bake's).
//! * **Ionisation out of equilibrium:** [`equilibrium_share`], the share of
//!   the equilibrium electron density a layer of a few thousand collisions
//!   reaches.

use super::equilibrium::{self, Mixture, AIR_OXYGEN_FRACTION};
use super::gas::Gas;
use crate::math;
use std::sync::OnceLock;

/// The free-stream temperature of the table (K): the upper atmosphere's
/// 190–270 K, where entries fly.
pub const FREE_STREAM_K: f64 = 230.0;

/// The gas behind a normal shock, in equilibrium.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShockState {
    /// Temperature (K).
    pub temperature: f64,
    /// Pressure (Pa).
    pub pressure: f64,
    /// Density ratio ρ₂/ρ₁.
    pub compression: f64,
    /// Electron number density (1/m³).
    pub electron_density: f64,
}

/// The temperature whose equilibrium enthalpy at `p` is `h` (Illinois
/// regula falsi on a bracket that grows from `guess`).
fn temperature_for(h: f64, p: f64, oxygen: f64, guess: f64, warm: &mut Option<Mixture>) -> Mixture {
    let eval = |t: f64, warm: &mut Option<Mixture>| {
        let m = equilibrium::mixture(t, p, oxygen, warm.as_ref());
        *warm = Some(m);
        (m, m.enthalpy() - h)
    };
    let (mut lo, mut hi) = (guess / 1.3, guess * 1.3);
    let (mut m_lo, mut f_lo) = eval(lo, warm);
    while f_lo > 0.0 && lo > 50.0 {
        hi = lo;
        lo /= 1.5;
        (m_lo, f_lo) = eval(lo, warm);
    }
    let (mut m_hi, mut f_hi) = eval(hi, warm);
    while f_hi < 0.0 && hi < 1e5 {
        lo = hi;
        (m_lo, f_lo) = (m_hi, f_hi);
        hi *= 1.5;
        (m_hi, f_hi) = eval(hi, warm);
    }
    let mut side = 0;
    for _ in 0..100 {
        if hi - lo < 1e-10 * hi {
            break;
        }
        let t = (lo * f_hi - hi * f_lo) / (f_hi - f_lo);
        let (m, f) = eval(t, warm);
        if f == 0.0 {
            return m;
        }
        if f < 0.0 {
            (lo, m_lo, f_lo) = (t, m, f);
            if side == -1 {
                f_hi *= 0.5;
            }
            side = -1;
        } else {
            (hi, m_hi, f_hi) = (t, m, f);
            if side == 1 {
                f_lo *= 0.5;
            }
            side = 1;
        }
    }
    if f_lo.abs() < f_hi.abs() {
        m_lo
    } else {
        m_hi
    }
}

/// The equilibrium state behind a normal shock at flight speed `speed`
/// (m/s) into gas of density `rho` (kg/m³) and temperature `t` (K) whose
/// oxygen mass fraction is `oxygen`. None below Mach 1.2.
pub fn normal_shock(speed: f64, rho: f64, t: f64, oxygen: f64) -> Option<ShockState> {
    let cold = equilibrium::mixture(t, 1.0, oxygen, None);
    let p1 = rho * equilibrium::K_B * t * cold.n.iter().sum::<f64>() / cold.density();
    let h1 = cold.enthalpy();
    if speed * speed < 1.44 * 1.4 * p1 / rho {
        return None;
    }
    let mu = rho * speed * speed;
    let mut eps = 0.1;
    let mut warm = None;
    let mut guess = t * (1.0 + 0.2 * speed * speed / (1.4 * p1 / rho)).min(1e4);
    let mut m = cold;
    for _ in 0..60 {
        let (p2, h2) = (p1 + mu * (1.0 - eps), h1 + 0.5 * speed * speed * (1.0 - eps * eps));
        m = temperature_for(h2, p2, oxygen, guess, &mut warm);
        guess = m.temperature;
        let next = rho / m.density();
        let done = (next - eps).abs() < 1e-11;
        eps = next;
        if done {
            break;
        }
    }
    Some(ShockState {
        temperature: m.temperature,
        pressure: m.pressure(),
        compression: 1.0 / eps,
        electron_density: m.electron_density(),
    })
}

/// The shock's standoff from a sphere of radius `nose_radius` (m) at the
/// density ratio `compression` = ρ₂/ρ₁ (Olivier 2000).
pub fn standoff(nose_radius: f64, compression: f64) -> f64 {
    let eps = 1.0 / compression;
    let k = eps / (1.0 + (2.0 * eps).sqrt());
    nose_radius * k / (1.0 - k)
}

/// Collisions a particle takes to bring dissociation and ionisation to
/// equilibrium: of order 10⁴–10⁵ (Vincenti & Kruger, *Introduction to
/// Physical Gas Dynamics*, ch. VII–VIII; Anderson, *Hypersonic and
/// High-Temperature Gas Dynamics*, ch. 16, against ~10⁴ for vibration),
/// the low end: FIRE II's layer at 1643.5 s is near equilibrium, as its
/// CFD finds.
pub const RELAXATION_COLLISIONS: f64 = 1e4;

/// The share of its equilibrium ionisation the shock layer reaches:
/// 1 − exp(−Z/[`RELAXATION_COLLISIONS`]) with Z = Δ·(ρ₂/ρ₁)/λ∞ the
/// collisions of a particle crossing the layer (`standoff` Δ, m, the mean
/// free path λ∞ shrunk by the compression; binary scaling in ρ∞·Rn). An
/// order-of-magnitude relaxation, not a finite-rate solution: high and
/// thin the layer never ionises (entry interface keeps its links).
pub fn equilibrium_share(standoff: f64, compression: f64, mean_free_path: f64) -> f64 {
    if mean_free_path <= 0.0 {
        return 1.0;
    }
    let collisions = standoff * compression / mean_free_path;
    1.0 - math::exp(-collisions / RELAXATION_COLLISIONS)
}

const SPEED_0: f64 = 1_000.0;
const SPEED_STEP: f64 = 250.0;
const SPEEDS: usize = 61;
const LOG_RHO_0: f64 = -8.0;
const LOG_RHO_STEP: f64 = 0.5;
const RHOS: usize = 17;

/// One gas's jump on the grid: (T, p/(ρ₁V²), ρ₂/ρ₁, ln nₑ).
struct Table {
    rows: Vec<[f64; 4]>,
}

impl Table {
    fn build(oxygen: f64) -> Self {
        let mut rows = vec![[0.0; 4]; SPEEDS * RHOS];
        for j in 0..RHOS {
            let rho = math::pow(10.0, LOG_RHO_0 + LOG_RHO_STEP * j as f64);
            for i in 0..SPEEDS {
                let v = SPEED_0 + SPEED_STEP * i as f64;
                let s = normal_shock(v, rho, FREE_STREAM_K, oxygen).expect("hypersonic over the grid");
                rows[j * SPEEDS + i] =
                    [s.temperature, s.pressure / (rho * v * v), s.compression, math::ln(s.electron_density.max(1.0))];
            }
        }
        Table { rows }
    }

    fn at(&self, speed: f64, rho: f64) -> [f64; 4] {
        let fi = ((speed - SPEED_0) / SPEED_STEP).clamp(0.0, (SPEEDS - 1) as f64);
        let fj = ((math::log10(rho) - LOG_RHO_0) / LOG_RHO_STEP).clamp(0.0, (RHOS - 1) as f64);
        let (i, j) = ((fi as usize).min(SPEEDS - 2), (fj as usize).min(RHOS - 2));
        let (a, b) = (fi - i as f64, fj - j as f64);
        let r = |i: usize, j: usize| self.rows[j * SPEEDS + i];
        let mut out = [0.0; 4];
        for (k, o) in out.iter_mut().enumerate() {
            let lo = r(i, j)[k] + (r(i + 1, j)[k] - r(i, j)[k]) * a;
            let hi = r(i, j + 1)[k] + (r(i + 1, j + 1)[k] - r(i, j + 1)[k]) * a;
            *o = lo + (hi - lo) * b;
        }
        out
    }
}

fn table(gas: Gas) -> Option<&'static Table> {
    static AIR: OnceLock<Table> = OnceLock::new();
    static N2: OnceLock<Table> = OnceLock::new();
    match gas {
        Gas::Air => Some(AIR.get_or_init(|| Table::build(AIR_OXYGEN_FRACTION))),
        Gas::Nitrogen => Some(N2.get_or_init(|| Table::build(0.0))),
        Gas::CarbonDioxide => None,
    }
}

/// The equilibrium jump at flight speed `speed` (m/s) into `gas` of
/// density `rho` (kg/m³), from the gas's table (the free stream at
/// [`FREE_STREAM_K`]); None for CO₂ and below 1 km/s.
pub fn jump(gas: Gas, speed: f64, rho: f64) -> Option<ShockState> {
    if speed < SPEED_0 || rho <= 0.0 {
        return None;
    }
    let [temperature, p, compression, ln_ne] = table(gas)?.at(speed, rho);
    Some(ShockState { temperature, pressure: p * rho * speed * speed, compression, electron_density: math::exp(ln_ne) })
}
