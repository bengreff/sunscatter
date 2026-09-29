//! Interior volume nodes (D065 revised): a coarse grid of cubes over the
//! craft, a node wherever the craft has volume.
//!
//! Each cube is sampled at `SAMPLES³` points; a sample is inside the craft
//! when any primitive contains it (the solids themselves, not the surface
//! mesh, so thin seams cannot leak). A node's volume is its inside samples'
//! share of the cube, its centre their centroid. Face-neighbour nodes share
//! a face whose open area is the smaller of the two inside fractions × the
//! face. Samples inside the tank cylinder keep their height along the tank,
//! so the propellant at any fill level is shared out by where it sits
//! ([`VolumeGrid::propellant`]).

use super::file::{Primitive, Tank};
use super::mesh::contains;
use glam::DVec3;
use std::collections::BTreeMap;

/// Samples per cube edge.
pub const SAMPLES: usize = 4;

/// One interior node.
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeNode {
    /// Centroid of the inside samples (body axes, m).
    pub centre: DVec3,
    /// m³.
    pub volume: f64,
    /// Heights along the tank (from its base, m) of the node's samples
    /// inside the tank.
    pub tank: Vec<f64>,
}

/// The interior grid of a craft.
#[derive(Clone, Debug, PartialEq)]
pub struct VolumeGrid {
    /// Cube edge (m).
    pub size: f64,
    /// Corner of cube (0, 0, 0) (body axes).
    pub origin: DVec3,
    pub nodes: Vec<VolumeNode>,
    /// Face-neighbour pairs (i < j) and their open face area (m²).
    pub links: Vec<(u32, u32, f64)>,
    /// Cube index → node.
    index: BTreeMap<[i32; 3], u32>,
    /// Volume of one sample (m³).
    pub sample_volume: f64,
    /// Tank height (m) and the samples inside it.
    tank_height: f64,
    tank_samples: usize,
}

impl VolumeGrid {
    /// The grid of cubes of edge `size` over the primitives' solids.
    pub fn new(primitives: &[Primitive], bounds: (DVec3, DVec3), tank: &Tank, size: f64) -> Self {
        let (lo, hi) = bounds;
        let n = ((hi - lo) / size).ceil().max(DVec3::ONE);
        let (nx, ny, nz) = (n.x as i32, n.y as i32, n.z as i32);
        let step = size / SAMPLES as f64;
        let sample_volume = step * step * step;
        let (dir, height) = (tank.axis.normalize(), tank.axis.length());
        let in_tank = |p: DVec3| {
            let a = (p - tank.base).dot(dir);
            let radial = (p - tank.base - dir * a).length();
            (a >= 0.0 && a <= height && radial <= tank.radius).then_some(a)
        };
        let mut nodes = Vec::new();
        let mut cubes = Vec::new();
        let mut index = BTreeMap::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let corner = lo + DVec3::new(i as f64, j as f64, k as f64) * size;
                    let (mut count, mut sum, mut tank_heights) = (0usize, DVec3::ZERO, Vec::new());
                    for s in 0..SAMPLES * SAMPLES * SAMPLES {
                        let (a, b, c) = (s % SAMPLES, (s / SAMPLES) % SAMPLES, s / (SAMPLES * SAMPLES));
                        let p = corner + DVec3::new(a as f64 + 0.5, b as f64 + 0.5, c as f64 + 0.5) * step;
                        if primitives.iter().any(|q| contains(&q.shape, p)) {
                            count += 1;
                            sum += p;
                            if let Some(h) = in_tank(p) {
                                tank_heights.push(h);
                            }
                        }
                    }
                    if count > 0 {
                        index.insert([i, j, k], nodes.len() as u32);
                        cubes.push((count, [i, j, k]));
                        nodes.push(VolumeNode {
                            centre: sum / count as f64,
                            volume: count as f64 * sample_volume,
                            tank: tank_heights,
                        });
                    }
                }
            }
        }
        let per_cube = (SAMPLES * SAMPLES * SAMPLES) as f64;
        let mut links = Vec::new();
        for (a, &(count, [i, j, k])) in cubes.iter().enumerate() {
            for d in [[1, 0, 0], [0, 1, 0], [0, 0, 1]] {
                if let Some(&b) = index.get(&[i + d[0], j + d[1], k + d[2]]) {
                    let fraction = (count as f64).min(cubes[b as usize].0 as f64) / per_cube;
                    links.push((a as u32, b, fraction * size * size));
                }
            }
        }
        let tank_samples = nodes.iter().map(|n| n.tank.len()).sum();
        VolumeGrid { size, origin: lo, nodes, links, index, sample_volume, tank_height: height, tank_samples }
    }

    /// Total interior volume (m³).
    pub fn volume(&self) -> f64 {
        self.nodes.iter().map(|n| n.volume).sum()
    }

    /// The node whose cube contains `p`, if any.
    pub fn node_at(&self, p: DVec3) -> Option<u32> {
        let c = ((p - self.origin) / self.size).floor();
        self.index.get(&[c.x as i32, c.y as i32, c.z as i32]).copied()
    }

    /// The node containing `p`, else the one with the nearest centre
    /// (lowest index on ties).
    pub fn nearest(&self, p: DVec3) -> u32 {
        self.node_at(p).unwrap_or_else(|| {
            let d = |i: usize| (self.nodes[i].centre - p).length_squared();
            (0..self.nodes.len()).fold(0, |best, i| if d(i) < d(best) { i } else { best }) as u32
        })
    }

    /// Propellant per node (kg) for `mass` kg filling the tank from its
    /// base to the level `fill` × height: each tank sample below the level
    /// holds an equal share (the sample straddling it a partial one).
    pub fn propellant(&self, mass: f64, fill: f64, out: &mut [f64]) {
        out.fill(0.0);
        if mass <= 0.0 || self.tank_samples == 0 {
            return;
        }
        let level = self.tank_height * fill.clamp(0.0, 1.0);
        let step = self.size / SAMPLES as f64;
        let weight = |h: f64| ((level - h) / step + 0.5).clamp(0.0, 1.0);
        let total: f64 = self.nodes.iter().flat_map(|n| n.tank.iter()).map(|&h| weight(h)).sum();
        if total <= 0.0 {
            // Below the lowest sample: all in the node holding it.
            let lowest = (0..self.nodes.len())
                .filter(|&i| !self.nodes[i].tank.is_empty())
                .map(|i| (self.nodes[i].tank.iter().fold(f64::MAX, |m, &h| m.min(h)), i))
                .fold((f64::MAX, 0), |b, x| if x.0 < b.0 { x } else { b });
            out[lowest.1] = mass;
            return;
        }
        for (o, n) in out.iter_mut().zip(&self.nodes) {
            *o = mass * n.tank.iter().map(|&h| weight(h)).sum::<f64>() / total;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::test_craft;

    fn grid() -> VolumeGrid {
        let c = test_craft();
        let s = &c.surface;
        let (lo, hi) = s
            .positions
            .iter()
            .fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        VolumeGrid::new(&c.geometry.primitives, (lo, hi), &c.geometry.tank, 1.0)
    }

    #[test]
    fn the_test_craft_interior_matches_its_solids() {
        let g = grid();
        let c = test_craft();
        // The skirt, the body above it (π·1.04²·8.5) and the cabin cone; the
        // bell, nose cap, fins and legs (small): the grid's volume within
        // a sample layer.
        let pi = std::f64::consts::PI;
        let frustum = |h: f64, a: f64, b: f64| pi * h * (a * a + a * b + b * b) / 3.0;
        let main = frustum(1.5, 1.52, 1.04) + pi * 1.04 * 1.04 * 8.5 + frustum(2.4, 1.04, 0.49);
        let analytic: f64 = c.geometry.primitives.iter().map(|p| crate::craft::mesh::analytic_volume(&p.shape)).sum();
        assert!(analytic > main);
        let v = g.volume();
        assert!((v / main - 1.0).abs() < 0.1, "{v} vs {main}");
        println!("{} nodes, {} links, {v:.1} m³", g.nodes.len(), g.links.len());
        assert!(g.nodes.len() > 40 && g.nodes.len() < 200);
        // Every link joins face neighbours with a positive open area.
        for &(a, b, area) in &g.links {
            assert!(a < b && area > 0.0 && area <= 1.0);
        }
    }

    #[test]
    fn propellant_fills_from_the_tank_base() {
        let g = grid();
        let mut out = vec![0.0; g.nodes.len()];
        for fill in [1.0, 0.5, 0.1, 1e-4] {
            g.propellant(1000.0, fill, &mut out);
            let total: f64 = out.iter().sum();
            assert!((total - 1000.0).abs() < 1e-9, "{fill}: {total}");
            // Nothing above the level (plus a sample).
            let level = -1.0 + 4.7 * fill + 0.25;
            for (n, m) in g.nodes.iter().zip(&out) {
                if *m > 0.0 {
                    let lowest = n.tank.iter().fold(f64::MAX, |a, &h| a.min(h)) - 1.0;
                    assert!(lowest <= level, "{fill}: node at {} holds {m}", n.centre);
                }
            }
        }
        g.propellant(0.0, 0.0, &mut out);
        assert!(out.iter().all(|&m| m == 0.0));
    }

    #[test]
    fn points_find_their_node() {
        let g = grid();
        for (i, n) in g.nodes.iter().enumerate() {
            // A cube's centroid of inside samples lies in its own cube.
            assert_eq!(g.node_at(n.centre), Some(i as u32));
            assert_eq!(g.nearest(n.centre), i as u32);
        }
        assert_eq!(g.node_at(DVec3::new(50.0, 0.0, 0.0)), None);
        let far = g.nearest(DVec3::new(0.0, 0.0, 50.0));
        assert!(g.nodes[far as usize].centre.z > 1.5, "the nose end");
    }
}
