//! Skin temperature per surface cell and one internal node (D065,
//! realism-1 §5b, `docs/design/aero-thermal.md`).
//!
//! Each cell has a heat capacity (skin areal mass × area × specific heat),
//! radiates εσA(T⁴ − T_env⁴), conducts to its neighbours (the cells'
//! conductances) and to the internal node (a coefficient × the cell's area).
//! [`ThermalNetwork::step`] advances the network by backward Euler, solved
//! by Gauss–Seidel sweeps in cell order (then the internal node): each cell's
//! update solves its own implicit balance a·T + εσA·T⁴ = b exactly (Newton on
//! the quartic, linearising the radiation from an upper bound, so it never
//! overshoots), with its neighbours at their latest values. Every
//! coefficient is positive and the system diagonally dominant, so the step
//! is stable at any dt. Sweeps stop when no temperature changes by more
//! than [`CONVERGED`] (relative), at most `max_sweeps`: a deterministic
//! rule (same inputs, same sweeps). Live ticks (20 ms) converge in 2–3
//! sweeps; energy is conserved to the convergence tolerance.
//!
//! Heat inputs (aerodynamic, sunlight) come from [`heating`].

pub mod heating;

pub use heating::{cell_heat, heating_shape, sutton_graves, HeatInput};

use crate::craft::Cell;

/// Stefan–Boltzmann constant (W/(m²·K⁴), CODATA 2018).
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
/// Most Gauss–Seidel sweeps per step.
pub const DEFAULT_SWEEPS: usize = 64;
/// Sweeps stop when the largest change is below this × the hottest temperature.
pub const CONVERGED: f64 = 1e-13;
/// Most Newton iterations per cell update.
const NEWTON: usize = 8;

/// The root T ≥ 0 of a·T + r·T⁴ = b (a, r, b ≥ 0), by Newton from `guess`
/// (the current temperature). The function is convex and increasing, so the
/// first step lands at or above the root (clamped to the upper bounds)
/// and the iterates then decrease monotonically to it: no overshoot at any
/// dt. Stops when a step changes nothing (relative 1e-15).
pub fn cell_temperature(a: f64, r: f64, b: f64, guess: f64) -> f64 {
    if b <= 0.0 {
        return 0.0;
    }
    if r <= 0.0 {
        return if a > 0.0 { b / a } else { guess };
    }
    let newton = |t: f64| {
        let t3 = t * t * t;
        t - (a * t + r * t3 * t - b) / (a + 4.0 * r * t3)
    };
    // Upper bounds of the root: b/a and (b/r)^¼ (the latter only when
    // needed; it costs two square roots).
    let mut t = if guess > 0.0 { newton(guess) } else { f64::INFINITY };
    if a > 0.0 {
        t = t.min(b / a);
    }
    if t.is_infinite() || r * t * t * t * t > b {
        t = t.min((b / r).sqrt().sqrt());
    }
    for _ in 0..NEWTON {
        let next = newton(t);
        if next >= t || t - next <= 1e-15 * t {
            return next.min(t);
        }
        t = next;
    }
    t
}

/// The internal node: the part's interior (one temperature, D065).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InternalNode {
    /// Heat capacity (J/K).
    pub capacity: f64,
    /// Conductance to each cell per unit of its area (W/(m²·K)).
    pub coefficient: f64,
}

/// A craft design's thermal network (shared by its vessels).
#[derive(Clone, Debug, PartialEq)]
pub struct ThermalNetwork {
    /// Per cell: heat capacity (J/K), ε·σ·A (W/K⁴), conductance to the
    /// internal node (W/K).
    pub capacity: Vec<f64>,
    pub radiation: Vec<f64>,
    pub to_internal: Vec<f64>,
    /// Neighbour links (CSR: `links[start[i]..start[i + 1]]` = (cell, W/K)).
    pub start: Vec<u32>,
    pub links: Vec<(u32, f64)>,
    pub internal_capacity: f64,
}

/// A vessel's temperatures (K).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThermalState {
    pub skin: Vec<f64>,
    pub internal: f64,
}

impl ThermalState {
    /// Every cell and the interior at `t`.
    pub fn uniform(cells: usize, t: f64) -> Self {
        ThermalState { skin: vec![t; cells], internal: t }
    }
}

/// What overheated first (D065: the part is destroyed cleanly).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overheat {
    /// The hottest cell above the skin limit.
    Cell(u32),
    Internal,
}

/// The hottest cell above `skin_max` (lowest index on ties), else the
/// interior if above `internal_max`.
pub fn check(state: &ThermalState, skin_max: f64, internal_max: f64) -> Option<Overheat> {
    let hottest =
        state.skin.iter().enumerate().fold(
            None,
            |best: Option<(usize, f64)>, (i, &t)| {
                if best.is_none_or(|(_, b)| t > b) {
                    Some((i, t))
                } else {
                    best
                }
            },
        );
    match hottest {
        Some((i, t)) if t > skin_max => Some(Overheat::Cell(i as u32)),
        _ if state.internal > internal_max => Some(Overheat::Internal),
        _ => None,
    }
}

impl ThermalNetwork {
    /// The network of `cells` (their skin data and neighbour conductances).
    pub fn new(cells: &[Cell], internal: InternalNode) -> Self {
        let mut start = vec![0u32];
        let mut links = Vec::new();
        for c in cells {
            links.extend(c.neighbours.iter().map(|n| (n.cell, n.conductance)));
            start.push(links.len() as u32);
        }
        ThermalNetwork {
            capacity: cells.iter().map(|c| c.skin_mass() * c.specific_heat).collect(),
            radiation: cells.iter().map(|c| c.emissivity * STEFAN_BOLTZMANN * c.area).collect(),
            to_internal: cells.iter().map(|c| internal.coefficient * c.area).collect(),
            start,
            links,
            internal_capacity: internal.capacity,
        }
    }

    pub fn cells(&self) -> usize {
        self.capacity.len()
    }

    fn neighbours(&self, i: usize) -> &[(u32, f64)] {
        &self.links[self.start[i] as usize..self.start[i + 1] as usize]
    }

    /// Heat content Σ C·T over the cells and the interior (J; conservation checks).
    pub fn heat_content(&self, state: &ThermalState) -> f64 {
        let skin: f64 = self.capacity.iter().zip(&state.skin).map(|(c, t)| c * t).sum();
        skin + self.internal_capacity * state.internal
    }

    /// Advances `state` by `dt` (s): `heat` is the power absorbed by each
    /// cell (W), `internal_heat` the power released inside (W), `t_env` the
    /// radiative sink temperature (K).
    /// Returns the number of sweeps done: until no temperature changes by
    /// more than [`CONVERGED`] (relative), at most `max_sweeps`.
    pub fn step(
        &self,
        state: &mut ThermalState,
        heat: &[f64],
        internal_heat: f64,
        t_env: f64,
        dt: f64,
        max_sweeps: usize,
    ) -> usize {
        let n = self.cells();
        assert!(heat.len() == n && state.skin.len() == n, "thermal step: {} cells, {} inputs", n, heat.len());
        let old = state.skin.clone();
        let old_internal = state.internal;
        let env4 = t_env * t_env * t_env * t_env;
        let inv_dt = 1.0 / dt;
        let mut done = 0;
        while done < max_sweeps {
            done += 1;
            let tb = state.internal;
            let (mut change, mut hottest) = (0.0f64, 1.0f64);
            for i in 0..n {
                // a·T + r·T⁴ = b with the neighbours and the interior at
                // their latest values.
                let mut a = self.capacity[i] * inv_dt + self.to_internal[i];
                let mut b =
                    self.capacity[i] * inv_dt * old[i] + heat[i] + self.radiation[i] * env4 + self.to_internal[i] * tb;
                for &(j, g) in self.neighbours(i) {
                    a += g;
                    b += g * state.skin[j as usize];
                }
                let t = cell_temperature(a, self.radiation[i], b, state.skin[i]);
                change = change.max((t - state.skin[i]).abs());
                hottest = hottest.max(t);
                state.skin[i] = t;
            }
            let (mut diag, mut rhs) =
                (self.internal_capacity * inv_dt, self.internal_capacity * inv_dt * old_internal + internal_heat);
            for i in 0..n {
                diag += self.to_internal[i];
                rhs += self.to_internal[i] * state.skin[i];
            }
            let t = if diag > 0.0 { rhs / diag } else { old_internal };
            change = change.max((t - state.internal).abs());
            state.internal = t;
            if change <= CONVERGED * hottest.max(t) {
                break;
            }
        }
        done
    }
}

#[cfg(test)]
mod tests;
