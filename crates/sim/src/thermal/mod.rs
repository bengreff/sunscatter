//! Skin temperature per surface cell and interior volume nodes (D065
//! revised, realism-1 §5b, `docs/design/aero-thermal.md`).
//!
//! Each cell has a heat capacity (skin areal mass × area × specific heat),
//! radiates εσA(T⁴ − T_env⁴), conducts to its neighbours (the cells'
//! conductances) and to the interior node beneath it (a coefficient × the
//! cell's area). Interior nodes ([`Interior`], built from
//! `sim::craft::volume`) conduct to their face neighbours; their heat
//! capacities are an input of each step (they change as propellant drains),
//! and heat sources (engine losses, later reactors and electronics) go
//! into them. [`ThermalNetwork::step`] advances the network by backward
//! Euler, solved by Gauss–Seidel sweeps in cell order, then node order:
//! each cell's update solves its own implicit balance a·T + εσA·T⁴ = b
//! exactly (Newton on the quartic, linearising the radiation from an upper
//! bound, so it never overshoots), with its neighbours at their latest
//! values, over-relaxed at coast steps ([`relaxation`]); a node's is linear. Every coefficient is positive and the system
//! diagonally dominant, so the step is stable at any dt. Sweeps stop when
//! no temperature changes by more than [`CONVERGED`] (relative), at most
//! `max_sweeps`: a deterministic rule (same inputs, same sweeps). Live
//! ticks (20 ms) converge in 2–3 sweeps; energy is conserved to the
//! convergence tolerance.
//!
//! Heat inputs (aerodynamic, sunlight) come from [`heating`].

pub mod heating;

pub use heating::{cell_heat, heating_shape, sutton_graves, HeatInput};

use crate::craft::Cell;

/// Stefan–Boltzmann constant (W/(m²·K⁴), CODATA 2018).
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
/// Most Gauss–Seidel sweeps per step.
pub const DEFAULT_SWEEPS: usize = 128;

/// Over-relaxation of the skin cells' updates for a step of `dt` (s): none
/// for live ticks (converged in 2–4 sweeps); at coast steps the skin's
/// conduction around the craft converges slowly, and over-relaxing halves
/// to fifths the sweeps (measured on the test craft: 60 s 31 → 24 sweeps,
/// 600 s 132 → 60, 3600 s 290 → 60). The fixed point, and so the solution,
/// is the same.
pub fn relaxation(dt: f64) -> f64 {
    if dt < 1.0 {
        1.0
    } else if dt < 300.0 {
        1.3
    } else {
        1.6
    }
}
/// Sweeps stop when the largest change is below this × the hottest temperature.
pub const CONVERGED: f64 = 1e-13;
/// The same for coast steps (minutes to a day long): ~0.03 mK. Their
/// inputs are orbit averages, far coarser.
pub const COAST_CONVERGED: f64 = 1e-5;
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

/// How the interior is divided and joined (built per craft design).
#[derive(Clone, Debug, PartialEq)]
pub struct Interior {
    /// Number of nodes.
    pub nodes: usize,
    /// The node beneath each cell.
    pub cell_node: Vec<u32>,
    /// Conductance from each cell to its node per unit of the cell's area
    /// (W/(m²·K)).
    pub coupling: f64,
    /// Node–node conductances (W/K).
    pub links: Vec<(u32, u32, f64)>,
}

impl Interior {
    /// One node under every cell (tests, simple bodies).
    pub fn single(cells: usize, coupling: f64) -> Self {
        Interior { nodes: 1, cell_node: vec![0; cells], coupling, links: Vec::new() }
    }
}

/// A craft design's thermal network (shared by its vessels).
#[derive(Clone, Debug, PartialEq)]
pub struct ThermalNetwork {
    /// Per cell: heat capacity (J/K), ε·σ·A (W/K⁴), conductance to its node
    /// (W/K), and that node.
    pub capacity: Vec<f64>,
    pub radiation: Vec<f64>,
    pub to_node: Vec<f64>,
    pub cell_node: Vec<u32>,
    /// Cell neighbour links (CSR: `links[start[i]..start[i + 1]]` = (cell, W/K)).
    pub start: Vec<u32>,
    pub links: Vec<(u32, f64)>,
    /// Node neighbour links, both directions (CSR, (node, W/K)).
    node_start: Vec<u32>,
    node_links: Vec<(u32, f64)>,
    /// Cells under each node (CSR).
    under_start: Vec<u32>,
    under: Vec<u32>,
}

/// A vessel's temperatures (K).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ThermalState {
    pub skin: Vec<f64>,
    pub nodes: Vec<f64>,
}

impl ThermalState {
    /// Every cell and node at `t`.
    pub fn uniform(cells: usize, nodes: usize, t: f64) -> Self {
        ThermalState { skin: vec![t; cells], nodes: vec![t; nodes] }
    }
}

/// What overheated first (D065: the part is destroyed cleanly).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Overheat {
    /// The hottest cell above the skin limit.
    Cell(u32),
    /// The hottest node above the node limit.
    Node(u32),
}

/// Index and value of the largest entry (lowest index on ties).
pub fn hottest(t: &[f64]) -> Option<(usize, f64)> {
    t.iter().enumerate().fold(
        None,
        |best: Option<(usize, f64)>, (i, &x)| {
            if best.is_none_or(|(_, b)| x > b) {
                Some((i, x))
            } else {
                best
            }
        },
    )
}

/// The hottest cell above `skin_max`, else the hottest node above `node_max`.
pub fn check(state: &ThermalState, skin_max: f64, node_max: f64) -> Option<Overheat> {
    match (hottest(&state.skin), hottest(&state.nodes)) {
        (Some((i, t)), _) if t > skin_max => Some(Overheat::Cell(i as u32)),
        (_, Some((j, t))) if t > node_max => Some(Overheat::Node(j as u32)),
        _ => None,
    }
}

/// CSR of `n` rows from (row, entry) pairs, in insertion order per row.
fn csr<T: Copy>(n: usize, pairs: impl Iterator<Item = (u32, T)> + Clone) -> (Vec<u32>, Vec<T>) {
    let mut count = vec![0u32; n + 1];
    for (r, _) in pairs.clone() {
        count[r as usize + 1] += 1;
    }
    for i in 0..n {
        count[i + 1] += count[i];
    }
    let mut fill = count.clone();
    let mut out: Vec<Option<T>> = vec![None; count[n] as usize];
    for (r, x) in pairs {
        out[fill[r as usize] as usize] = Some(x);
        fill[r as usize] += 1;
    }
    (count, out.into_iter().map(|x| x.expect("filled")).collect())
}

impl ThermalNetwork {
    /// The network of `cells` (their skin data and neighbour conductances)
    /// over `interior`.
    pub fn new(cells: &[Cell], interior: &Interior) -> Self {
        assert_eq!(cells.len(), interior.cell_node.len(), "one node per cell");
        let mut start = vec![0u32];
        let mut links = Vec::new();
        for c in cells {
            links.extend(c.neighbours.iter().map(|n| (n.cell, n.conductance)));
            start.push(links.len() as u32);
        }
        let both = interior.links.iter().flat_map(|&(a, b, g)| [(a, (b, g)), (b, (a, g))]);
        let (node_start, node_links) = csr(interior.nodes, both);
        let (under_start, under) =
            csr(interior.nodes, interior.cell_node.iter().enumerate().map(|(i, &n)| (n, i as u32)));
        ThermalNetwork {
            capacity: cells.iter().map(|c| c.skin_mass() * c.specific_heat).collect(),
            radiation: cells.iter().map(|c| c.emissivity * STEFAN_BOLTZMANN * c.area).collect(),
            to_node: cells.iter().map(|c| interior.coupling * c.area).collect(),
            cell_node: interior.cell_node.clone(),
            start,
            links,
            node_start,
            node_links,
            under_start,
            under,
        }
    }

    pub fn cells(&self) -> usize {
        self.capacity.len()
    }

    pub fn nodes(&self) -> usize {
        self.node_start.len() - 1
    }

    fn neighbours(&self, i: usize) -> &[(u32, f64)] {
        &self.links[self.start[i] as usize..self.start[i + 1] as usize]
    }

    fn node_neighbours(&self, j: usize) -> &[(u32, f64)] {
        &self.node_links[self.node_start[j] as usize..self.node_start[j + 1] as usize]
    }

    fn cells_under(&self, j: usize) -> &[u32] {
        &self.under[self.under_start[j] as usize..self.under_start[j + 1] as usize]
    }

    /// Heat content Σ C·T over the cells and the nodes (J; conservation
    /// checks), with the nodes' capacities `node_capacity`.
    pub fn heat_content(&self, state: &ThermalState, node_capacity: &[f64]) -> f64 {
        let skin: f64 = self.capacity.iter().zip(&state.skin).map(|(c, t)| c * t).sum();
        skin + node_capacity.iter().zip(&state.nodes).map(|(c, t)| c * t).sum::<f64>()
    }

    /// Advances `state` by `dt` (s): `heat` is the power absorbed by each
    /// cell (W), `node_heat` the power released in each node (W),
    /// `node_capacity` each node's heat capacity now (J/K), `t_env` the
    /// radiative sink temperature (K). Returns the number of sweeps done:
    /// until no temperature changes by more than [`CONVERGED`] (relative),
    /// at most `max_sweeps`.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &self,
        state: &mut ThermalState,
        heat: &[f64],
        node_heat: &[f64],
        node_capacity: &[f64],
        t_env: f64,
        dt: f64,
        max_sweeps: usize,
    ) -> usize {
        self.step_to(state, heat, node_heat, node_capacity, t_env, dt, max_sweeps, CONVERGED)
    }

    /// [`Self::step`] converged to `tolerance` (relative) instead of
    /// [`CONVERGED`] (coast steps: [`COAST_CONVERGED`]).
    #[allow(clippy::too_many_arguments)]
    pub fn step_to(
        &self,
        state: &mut ThermalState,
        heat: &[f64],
        node_heat: &[f64],
        node_capacity: &[f64],
        t_env: f64,
        dt: f64,
        max_sweeps: usize,
        tolerance: f64,
    ) -> usize {
        let (n, m) = (self.cells(), self.nodes());
        assert!(heat.len() == n && state.skin.len() == n, "thermal step: {} cells, {} inputs", n, heat.len());
        assert!(node_heat.len() == m && node_capacity.len() == m && state.nodes.len() == m, "thermal step: nodes");
        let old = state.skin.clone();
        let old_nodes = state.nodes.clone();
        let env4 = t_env * t_env * t_env * t_env;
        let inv_dt = 1.0 / dt;
        let omega = relaxation(dt);
        let mut done = 0;
        while done < max_sweeps {
            done += 1;
            let (mut change, mut hottest) = (0.0f64, 1.0f64);
            for i in 0..n {
                // a·T + r·T⁴ = b with the neighbours and the node at their
                // latest values.
                let tb = state.nodes[self.cell_node[i] as usize];
                let mut a = self.capacity[i] * inv_dt + self.to_node[i];
                let mut b =
                    self.capacity[i] * inv_dt * old[i] + heat[i] + self.radiation[i] * env4 + self.to_node[i] * tb;
                for &(j, g) in self.neighbours(i) {
                    a += g;
                    b += g * state.skin[j as usize];
                }
                let t = cell_temperature(a, self.radiation[i], b, state.skin[i]);
                let t = (state.skin[i] + omega * (t - state.skin[i])).max(0.5 * t);
                change = change.max((t - state.skin[i]).abs());
                hottest = hottest.max(t);
                state.skin[i] = t;
            }
            for j in 0..m {
                let c = node_capacity[j] * inv_dt;
                let (mut diag, mut rhs) = (c, c * old_nodes[j] + node_heat[j]);
                for &(k, g) in self.node_neighbours(j) {
                    diag += g;
                    rhs += g * state.nodes[k as usize];
                }
                for &i in self.cells_under(j) {
                    diag += self.to_node[i as usize];
                    rhs += self.to_node[i as usize] * state.skin[i as usize];
                }
                let t = if diag > 0.0 { rhs / diag } else { old_nodes[j] };
                change = change.max((t - state.nodes[j]).abs());
                hottest = hottest.max(t);
                state.nodes[j] = t;
            }
            if change <= tolerance * hottest {
                break;
            }
        }
        done
    }
}

#[cfg(test)]
mod tests;
