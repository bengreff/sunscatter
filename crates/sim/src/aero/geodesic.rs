//! Icosahedral geodesic grid of unit directions (the bake's flow
//! directions) and barycentric location of an arbitrary direction in it.
//!
//! Level `k` subdivides each icosahedron face into 4ᵏ triangles:
//! 10·4ᵏ + 2 vertices (level 3: 642). Vertices are created in a fixed
//! order, so the grid is identical on every platform (only `sqrt`, which is
//! IEEE-exact, is used).

use glam::DVec3;
use std::collections::BTreeMap;

/// A geodesic sphere: unit vertices, triangular faces (counter-clockwise
/// seen from outside) and the faces around each vertex.
#[derive(Clone, Debug, PartialEq)]
pub struct Geodesic {
    pub dirs: Vec<DVec3>,
    pub faces: Vec<[u32; 3]>,
    /// Faces touching each vertex (CSR: `vertex_faces[start[v]..start[v + 1]]`).
    start: Vec<u32>,
    vertex_faces: Vec<u32>,
}

/// Three grid directions and their weights (non-negative, summing to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    pub dirs: [u32; 3],
    pub w: [f64; 3],
}

fn det(a: DVec3, b: DVec3, c: DVec3) -> f64 {
    a.dot(b.cross(c))
}

impl Geodesic {
    /// The grid at `level` subdivisions.
    pub fn new(level: u32) -> Self {
        let p = (1.0 + 5.0f64.sqrt()) / 2.0;
        let mut dirs: Vec<DVec3> = [
            (-1.0, p, 0.0),
            (1.0, p, 0.0),
            (-1.0, -p, 0.0),
            (1.0, -p, 0.0),
            (0.0, -1.0, p),
            (0.0, 1.0, p),
            (0.0, -1.0, -p),
            (0.0, 1.0, -p),
            (p, 0.0, -1.0),
            (p, 0.0, 1.0),
            (-p, 0.0, -1.0),
            (-p, 0.0, 1.0),
        ]
        .iter()
        .map(|&(x, y, z)| DVec3::new(x, y, z).normalize())
        .collect();
        let mut faces: Vec<[u32; 3]> = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];
        for _ in 0..level {
            let mut mid: BTreeMap<(u32, u32), u32> = BTreeMap::new();
            let mut midpoint = |a: u32, b: u32, dirs: &mut Vec<DVec3>| {
                *mid.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    dirs.push((dirs[a as usize] + dirs[b as usize]).normalize());
                    (dirs.len() - 1) as u32
                })
            };
            let mut next = Vec::with_capacity(faces.len() * 4);
            for &[a, b, c] in &faces {
                let ab = midpoint(a, b, &mut dirs);
                let bc = midpoint(b, c, &mut dirs);
                let ca = midpoint(c, a, &mut dirs);
                next.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
            }
            faces = next;
        }
        let mut lists = vec![Vec::new(); dirs.len()];
        for (f, face) in faces.iter().enumerate() {
            for &v in face {
                lists[v as usize].push(f as u32);
            }
        }
        let mut start = vec![0u32];
        let mut vertex_faces = Vec::new();
        for l in lists {
            vertex_faces.extend(l);
            start.push(vertex_faces.len() as u32);
        }
        Geodesic { dirs, faces, start, vertex_faces }
    }

    /// Index of the grid direction nearest `d` (lowest index on ties).
    pub fn nearest(&self, d: DVec3) -> u32 {
        let mut best = (f64::MIN, 0u32);
        for (i, g) in self.dirs.iter().enumerate() {
            let x = g.dot(d);
            if x > best.0 {
                best = (x, i as u32);
            }
        }
        best.1
    }

    /// Barycentric weights of `d` (unit) in face `f`, if it lies inside.
    fn in_face(&self, f: u32, d: DVec3) -> Option<Weights> {
        let face = self.faces[f as usize];
        let [a, b, c] = face.map(|i| self.dirs[i as usize]);
        let w = [det(d, b, c), det(a, d, c), det(a, b, d)];
        let sum = w[0] + w[1] + w[2];
        if sum <= 0.0 || w.iter().any(|&x| x < -1e-12 * sum) {
            return None;
        }
        let w = w.map(|x| x.max(0.0) / sum);
        Some(Weights { dirs: face, w })
    }

    /// The face containing `d` and its barycentric weights: the faces around
    /// the nearest vertex first, then every face; the nearest vertex alone
    /// if rounding leaves `d` in none.
    pub fn locate(&self, d: DVec3) -> Weights {
        let d = d.normalize();
        let v = self.nearest(d);
        let around = &self.vertex_faces[self.start[v as usize] as usize..self.start[v as usize + 1] as usize];
        around
            .iter()
            .copied()
            .chain(0..self.faces.len() as u32)
            .find_map(|f| self.in_face(f, d))
            .unwrap_or(Weights { dirs: [v, v, v], w: [1.0, 0.0, 0.0] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_three_has_642_directions_and_1280_faces() {
        for (level, n) in [(0, 12), (1, 42), (2, 162), (3, 642)] {
            let g = Geodesic::new(level);
            assert_eq!(g.dirs.len(), n);
            assert_eq!(g.faces.len(), 20 * 4usize.pow(level));
            // Outward winding: every face's normal points away from the centre.
            for f in &g.faces {
                let [a, b, c] = f.map(|i| g.dirs[i as usize]);
                assert!(det(a, b, c) > 0.0);
            }
        }
    }

    #[test]
    fn located_weights_reproduce_the_direction() {
        let g = Geodesic::new(3);
        for d in crate::craft::cells::sphere_directions(500) {
            let w = g.locate(d);
            assert!((w.w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
            let p: DVec3 = (0..3).map(|k| g.dirs[w.dirs[k] as usize] * w.w[k]).sum();
            // The flat-triangle interpolant lies just inside the sphere.
            assert!(p.normalize().dot(d) > 1.0 - 1e-12, "{d}");
        }
        // At a grid vertex, the weight is all on it.
        let w = g.locate(g.dirs[100]);
        let k = w.dirs.iter().position(|&i| i == 100).unwrap();
        assert!((w.w[k] - 1.0).abs() < 1e-12);
    }
}
