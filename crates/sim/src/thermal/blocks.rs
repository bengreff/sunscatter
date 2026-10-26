//! The blocks of the network solve ([`super::ThermalNetwork::step_to`]):
//! trees of strongly coupled unknowns, each solved exactly in every sweep.
//!
//! Unknowns are the cells (`0..n`) then the nodes (`n..n + m`). A link is
//! *strong* when its conductance is a large part of what holds both ends,
//! g/√(D_u·D_v) ≥ [`STRONG`] with D = C/[`REFERENCE_DT`] + Σ g (a node's
//! capacity is an input of each step, so its D is its conductances only:
//! nodes are judged strongly coupled more readily, which costs nothing).
//! The strong links, strongest first, form a spanning forest (Kruskal;
//! ties by index), and each tree is a block: one pass from the leaves
//! eliminates it onto its root, one pass back gives every unknown, so the
//! chains of thin rings along struts and legs that point Gauss–Seidel
//! relaxes over hundreds of sweeps converge as fast as the rest. The
//! other links are lagged as in Gauss–Seidel (the latest values). This is
//! block Gauss–Seidel on an M-matrix with a regular splitting: it
//! converges for any dt, to the same fixed point.

/// No parent: the root of its tree.
pub(super) const NONE: u32 = u32::MAX;
/// A link is strong at g/√(D_u·D_v) ≥ this.
pub(super) const STRONG: f64 = 0.05;
/// The step (s) the strength is judged at: the coast lattice's (live ticks
/// are diagonally dominant at any blocking).
pub(super) const REFERENCE_DT: f64 = 600.0;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Blocks {
    /// The unknown at each solve position: block after block (by lowest
    /// unknown), each in post-order (every unknown before its parent).
    /// Everything below is indexed by solve position.
    pub order: Vec<u32>,
    /// The blocks' starts (and the end last).
    pub block_start: Vec<u32>,
    /// Its parent in its tree ([`NONE`] for a root), the conductance in its
    /// own row to the parent and in the parent's row to it.
    pub parent: Vec<u32>,
    pub to_parent: Vec<f64>,
    pub from_parent: Vec<f64>,
    /// The sum of all its conductances (the diagonal's share).
    pub sum_g: Vec<f64>,
    /// Its links outside its tree (CSR: (solve position, W/K)).
    pub rest_start: Vec<u32>,
    pub rest: Vec<(u32, f64)>,
}

fn find(up: &mut [u32], mut u: u32) -> u32 {
    while up[u as usize] != u {
        let next = up[up[u as usize] as usize];
        up[u as usize] = next;
        u = next;
    }
    u
}

impl Blocks {
    /// From the rows of the conductance matrix (`rows[u]`: (v, g), any
    /// order, duplicates summed) and each unknown's capacity at
    /// construction (J/K; zero where unknown).
    pub fn new(rows: &[Vec<(u32, f64)>], capacity: &[f64]) -> Self {
        let n = rows.len();
        // Directed entries merged per (u, v).
        let merged: Vec<Vec<(u32, f64)>> = rows
            .iter()
            .map(|r| {
                let mut r = r.clone();
                r.sort_by_key(|e| e.0);
                let mut out: Vec<(u32, f64)> = Vec::with_capacity(r.len());
                for (v, g) in r {
                    match out.last_mut() {
                        Some(last) if last.0 == v => last.1 += g,
                        _ => out.push((v, g)),
                    }
                }
                out
            })
            .collect();
        let sum_g: Vec<f64> = merged.iter().map(|r| r.iter().map(|e| e.1).sum()).collect();
        let d: Vec<f64> = (0..n).map(|u| capacity[u] / REFERENCE_DT + sum_g[u]).collect();
        let entry = |merged: &[Vec<(u32, f64)>], u: usize, v: u32| {
            merged[u].binary_search_by_key(&v, |e| e.0).map_or(0.0, |k| merged[u][k].1)
        };
        // Strong undirected edges, strongest first.
        let mut edges: Vec<(f64, u32, u32)> = Vec::new();
        for u in 0..n {
            for &(v, g) in &merged[u] {
                if (v as usize) <= u {
                    continue;
                }
                let back = entry(&merged, v as usize, u as u32);
                let s = 0.5 * (g + back) / (d[u] * d[v as usize]).sqrt().max(1e-300);
                if s >= STRONG {
                    edges.push((s, u as u32, v));
                }
            }
        }
        edges.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut up: Vec<u32> = (0..n as u32).collect();
        let mut tree: Vec<Vec<u32>> = vec![Vec::new(); n];
        for &(_, u, v) in &edges {
            let (a, b) = (find(&mut up, u), find(&mut up, v));
            if a != b {
                up[a.max(b) as usize] = a.min(b);
                tree[u as usize].push(v);
                tree[v as usize].push(u);
            }
        }
        // Each tree from its lowest unknown, breadth first; post-order is
        // the reverse.
        let mut parent = vec![NONE; n];
        let mut seen = vec![false; n];
        let (mut order, mut block_start) = (Vec::with_capacity(n), vec![0u32]);
        let mut bfs = Vec::new();
        for root in 0..n {
            if seen[root] {
                continue;
            }
            bfs.clear();
            bfs.push(root as u32);
            seen[root] = true;
            let mut k = 0;
            while k < bfs.len() {
                let u = bfs[k] as usize;
                k += 1;
                for &v in &tree[u] {
                    if !seen[v as usize] {
                        seen[v as usize] = true;
                        parent[v as usize] = u as u32;
                        bfs.push(v);
                    }
                }
            }
            order.extend(bfs.iter().rev());
            block_start.push(order.len() as u32);
        }
        let mut pos = vec![0u32; n];
        for (k, &u) in order.iter().enumerate() {
            pos[u as usize] = k as u32;
        }
        let in_tree = |u: usize, v: u32| parent[u] == v || parent[v as usize] == u as u32;
        let (mut rest_start, mut rest) = (vec![0u32], Vec::new());
        for &u in &order {
            let u = u as usize;
            rest.extend(merged[u].iter().filter(|e| !in_tree(u, e.0)).map(|&(v, g)| (pos[v as usize], g)));
            rest_start.push(rest.len() as u32);
        }
        let of_parent = |f: &dyn Fn(usize, usize) -> f64| -> Vec<f64> {
            order
                .iter()
                .map(|&u| u as usize)
                .map(|u| if parent[u] == NONE { 0.0 } else { f(u, parent[u] as usize) })
                .collect()
        };
        let to_parent = of_parent(&|u, p| entry(&merged, u, p as u32));
        let from_parent = of_parent(&|u, p| entry(&merged, p, u as u32));
        let parent =
            order.iter().map(|&u| parent[u as usize]).map(|p| if p == NONE { NONE } else { pos[p as usize] }).collect();
        let sum_g = order.iter().map(|&u| sum_g[u as usize]).collect();
        Blocks { order, block_start, parent, to_parent, from_parent, sum_g, rest_start, rest }
    }
}
