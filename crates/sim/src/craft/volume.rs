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
    /// Heights along their tank (from its base, m) of the node's samples
    /// inside a tank.
    pub tank: Vec<f64>,
    /// The tank of each of those samples (an index into the grid's tanks).
    pub tank_of: Vec<u32>,
    /// Inside samples per part (part, count), by part (craft of parts; a
    /// sample belongs to the part of the first primitive holding it).
    pub parts: Vec<(u32, u32)>,
}

impl VolumeNode {
    /// The part holding most of the node (the lowest on ties).
    pub fn part(&self) -> u32 {
        self.parts.iter().fold((0, 0), |best, &(p, n)| if n > best.1 { (p, n) } else { best }).0
    }
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
    /// Per tank: height (m), capacity (kg) and the samples inside it.
    tank_height: Vec<f64>,
    tank_samples: Vec<usize>,
    /// Per tank: the nodes holding its samples and their heights, in node
    /// then sample order ([`Self::propellant`] reads only its own).
    by_tank: Vec<Vec<(u32, Vec<f64>)>>,
}

impl VolumeGrid {
    /// The grid over a craft of parts: `owner` gives each primitive's part,
    /// `tanks` (region, capacity) the tanks (a sample in several is in the
    /// first).
    pub fn with_parts(
        primitives: &[Primitive],
        owner: &[u32],
        bounds: (DVec3, DVec3),
        tanks: &[(Tank, f64)],
        size: f64,
    ) -> Self {
        let (lo, hi) = bounds;
        let n = ((hi - lo) / size).ceil().max(DVec3::ONE);
        let (nx, ny, nz) = (n.x as i32, n.y as i32, n.z as i32);
        let step = size / SAMPLES as f64;
        let sample_volume = step * step * step;
        let shapes: Vec<(DVec3, f64, f64)> =
            tanks.iter().map(|(t, _)| (t.axis.normalize(), t.axis.length(), t.radius)).collect();
        let in_tank = |p: DVec3| {
            tanks.iter().zip(&shapes).enumerate().find_map(|(k, ((tank, _), &(dir, height, radius)))| {
                let a = (p - tank.base).dot(dir);
                let radial = (p - tank.base - dir * a).length();
                (a >= 0.0 && a <= height && radial <= radius).then_some((k as u32, a))
            })
        };
        let broad = super::broad::Broad::new(primitives.iter().map(|p| &p.shape));
        let mut nodes = Vec::new();
        let mut cubes = Vec::new();
        let mut index = BTreeMap::new();
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let corner = lo + DVec3::new(i as f64, j as f64, k as f64) * size;
                    let (mut count, mut sum, mut tank_heights, mut tank_of) =
                        (0usize, DVec3::ZERO, Vec::new(), Vec::new());
                    let mut parts: Vec<(u32, u32)> = Vec::new();
                    for s in 0..SAMPLES * SAMPLES * SAMPLES {
                        let (a, b, c) = (s % SAMPLES, (s / SAMPLES) % SAMPLES, s / (SAMPLES * SAMPLES));
                        let p = corner + DVec3::new(a as f64 + 0.5, b as f64 + 0.5, c as f64 + 0.5) * step;
                        if let Some(q) = broad.first_containing(p) {
                            count += 1;
                            sum += p;
                            if let Some((t, h)) = in_tank(p) {
                                tank_heights.push(h);
                                tank_of.push(t);
                            }
                            let part = owner[q as usize];
                            match parts.binary_search_by_key(&part, |x| x.0) {
                                Ok(at) => parts[at].1 += 1,
                                Err(at) => parts.insert(at, (part, 1)),
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
                            tank_of,
                            parts,
                        });
                    }
                }
            }
        }
        if nodes.is_empty() {
            // Nothing as thick as a sample spacing (a piece of thin parts):
            // one node at the bounds' centre, filled by no sample.
            let centre = (lo + hi) * 0.5;
            nodes.push(VolumeNode { centre, volume: sample_volume, tank: vec![], tank_of: vec![], parts: vec![] });
            let c = ((centre - lo) / size).floor();
            index.insert([c.x as i32, c.y as i32, c.z as i32], 0);
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
        let mut tank_samples = vec![0; tanks.len()];
        for t in nodes.iter().flat_map(|n| &n.tank_of) {
            tank_samples[*t as usize] += 1;
        }
        let by_tank = (0..tanks.len() as u32)
            .map(|k| {
                (nodes.iter().enumerate())
                    .map(|(i, n)| (i as u32, heights(n, Some(k)).collect::<Vec<f64>>()))
                    .filter(|(_, h)| !h.is_empty())
                    .collect()
            })
            .collect();
        VolumeGrid {
            size,
            origin: lo,
            nodes,
            links,
            index,
            sample_volume,
            tank_height: shapes.iter().map(|s| s.1).collect(),
            tank_samples,
            by_tank,
        }
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
}

impl VolumeGrid {
    /// How many tanks the grid's samples are sorted into.
    pub fn tanks(&self) -> usize {
        self.tank_height.len()
    }

    /// Adds `amount` (any quantity: kg, J/K) in tank `k` between the heights
    /// `from` and `to` along it (a slug where it sits, F2), shared out over
    /// the tank's samples by the part of each one's span inside the band;
    /// a band beside every sample goes to the node of the one nearest it.
    /// Returns whether the tank has samples of its own (a bottle inside
    /// another tank has none: its samples are that tank's).
    pub fn band(&self, k: usize, amount: f64, (from, to): (f64, f64), out: &mut [f64]) -> bool {
        let Some(list) = self.by_tank.get(k).filter(|l| !l.is_empty()) else { return false };
        let step = self.size / SAMPLES as f64;
        let below = |x: f64, h: f64| ((x - h) / step + 0.5).clamp(0.0, 1.0);
        let weight = |h: f64| below(to, h) - below(from, h);
        let total: f64 = list.iter().flat_map(|(_, h)| h.iter().copied()).map(weight).sum();
        if total <= 0.0 {
            let mid = 0.5 * (from + to);
            let near = (list.iter())
                .map(|(i, h)| (h.iter().map(|x| (x - mid).abs()).fold(f64::MAX, f64::min), *i))
                .fold((f64::MAX, 0), |b, x| if x.0 < b.0 { x } else { b });
            out[near.1 as usize] += amount;
            return true;
        }
        for (i, h) in list {
            out[*i as usize] += amount * h.iter().copied().map(weight).sum::<f64>() / total;
        }
        true
    }
}

/// The heights of a node's samples in tank `tank` (`None`: every tank).
fn heights(n: &VolumeNode, tank: Option<u32>) -> impl Iterator<Item = f64> + '_ {
    n.tank.iter().zip(&n.tank_of).filter(move |(_, &k)| tank.is_none_or(|t| t == k)).map(|(&h, _)| h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::small;

    /// The small rocket's interior at its node size (0.5 m): its parts'
    /// solids, its tank's two compartments.
    fn grid() -> VolumeGrid {
        let c = small::rocket_ref();
        let def = c.def();
        VolumeGrid::with_parts(&c.geometry.primitives, &def.primitive_part(), bounds(c), &def.tanks(), 0.5)
    }

    fn bounds(c: &crate::craft::Craft) -> (DVec3, DVec3) {
        let s = &c.surface;
        s.positions
            .iter()
            .fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
    }

    #[test]
    fn the_small_rocket_s_interior_matches_its_solids() {
        let g = grid();
        let c = small::rocket_ref();
        // The capsule's frustum, the tank and the thrust structure; the
        // engines and stowed legs (small): the grid's volume within a
        // sample layer.
        let pi = std::f64::consts::PI;
        let frustum = |h: f64, a: f64, b: f64| pi * h * (a * a + a * b + b * b) / 3.0;
        let main = frustum(1.5, 1.0, 0.4) + pi * 4.5 + pi * 0.5;
        let analytic: f64 = c.geometry.primitives.iter().map(|p| crate::craft::mesh::analytic_volume(&p.shape)).sum();
        assert!(analytic > main);
        let v = g.volume();
        assert!((v / main - 1.0).abs() < 0.1, "{v} vs {main}");
        println!("{} nodes, {} links, {v:.1} m³", g.nodes.len(), g.links.len());
        assert!(g.nodes.len() > 100 && g.nodes.len() < 400, "{} nodes", g.nodes.len());
        // Every link joins face neighbours with a positive open area.
        for &(a, b, area) in &g.links {
            assert!(a < b && area > 0.0 && area <= 0.25);
        }
    }

    /// A slug where it sits (F2): all of it in the nodes whose samples meet
    /// its band along the tank (a sample either side), wherever the band.
    #[test]
    fn a_band_goes_where_it_sits() {
        let g = grid();
        let tanks = small::rocket_ref().def().tanks();
        let mut out = vec![0.0; g.nodes.len()];
        // The tank's two compartments have samples of their own; the
        // nitrogen bottle inside it has none (its samples are the tank's).
        let own: Vec<bool> = (0..tanks.len()).map(|k| !g.by_tank[k].is_empty()).collect();
        assert_eq!(own.iter().filter(|&&o| !o).count(), 1, "{own:?}");
        for (k, (tank, _)) in tanks.iter().enumerate() {
            let l = tank.axis.length();
            for (from, to) in [(0.0, 0.1 * l), (0.3 * l, 0.5 * l), (0.85 * l, l), (2.0 * l, 2.1 * l)] {
                out.fill(0.0);
                assert_eq!(g.band(k, 1000.0, (from, to), &mut out), own[k]);
                if !own[k] {
                    continue;
                }
                let total: f64 = out.iter().sum();
                assert!((total - 1000.0).abs() < 1e-9, "tank {k}, {from}..{to}: {total}");
                let step = g.size / SAMPLES as f64;
                for (n, m) in g.nodes.iter().zip(&out) {
                    if *m > 0.0 && to < l {
                        let near = heights(n, Some(k as u32)).any(|h| h > from - step && h < to + step);
                        assert!(near, "tank {k}, {from}..{to}: node at {} holds {m}", n.centre);
                    }
                }
            }
        }
        assert!(!g.band(tanks.len(), 1000.0, (0.0, 1.0), &mut out), "no such tank");
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
        assert!(g.nodes[far as usize].centre.z > 0.75, "the capsule's end");
    }

    /// Two tanks filled to the same level: each holds its own contents,
    /// from its own base; the samples carry their tank and part.
    #[test]
    fn two_tanks_fill_together_by_capacity() {
        let prim = |base: DVec3, h: f64| Primitive {
            name: String::new(),
            shape: crate::craft::Shape::Cylinder { base, axis: DVec3::Z * h, radius: 1.0 },
            foot: false,
            skin: None,
            nozzle: false,
        };
        let prims = [prim(DVec3::ZERO, 4.0), prim(DVec3::new(3.0, 0.0, 0.0), 2.0)];
        let tanks = [
            (Tank { base: DVec3::ZERO, axis: DVec3::Z * 4.0, radius: 1.0 }, 3_000.0),
            (Tank { base: DVec3::new(3.0, 0.0, 0.0), axis: DVec3::Z * 2.0, radius: 1.0 }, 1_000.0),
        ];
        let bounds = (DVec3::new(-1.0, -1.0, 0.0), DVec3::new(4.0, 1.0, 4.0));
        let g = VolumeGrid::with_parts(&prims, &[0, 1], bounds, &tanks, 0.5);
        for n in &g.nodes {
            assert!(n.parts.len() == 1 && n.tank_of.iter().all(|&t| t == n.parts[0].0), "{:?}", n.parts);
        }
        let mut out = vec![0.0; g.nodes.len()];
        for fill in [1.0, 0.5, 0.25] {
            out.fill(0.0);
            for (k, (cap, h)) in [(3_000.0, 4.0), (1_000.0, 2.0)].into_iter().enumerate() {
                assert!(g.band(k, cap * fill, (0.0, h * fill), &mut out), "tank {k} has samples");
            }
            let in_tank =
                |t: u32| -> f64 { g.nodes.iter().zip(&out).filter(|(n, _)| n.parts[0].0 == t).map(|x| x.1).sum() };
            let total = 4_000.0 * fill;
            assert!((in_tank(0) - 0.75 * total).abs() < 1e-9 && (in_tank(1) - 0.25 * total).abs() < 1e-9, "{fill}");
            for (n, m) in g.nodes.iter().zip(&out) {
                let height = if n.parts[0].0 == 0 { 4.0 } else { 2.0 };
                assert!(*m == 0.0 || n.centre.z < height * fill + 0.5, "{fill}: {} holds {m}", n.centre);
            }
        }
    }
}
