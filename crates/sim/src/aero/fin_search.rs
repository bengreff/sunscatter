//! The fin search of the bake (`aero::bake`): thin, flat parts of the
//! craft that lift as wings ([`super::slender::Fin`]).

use super::bake::CellGeometry;
use super::slender::Fin;
use crate::craft::{Cells, Surface};
use glam::DVec3;

/// The fins: cells whose opposite face (anti-parallel, straight through
/// the part along −n, on the same primitive) is closer than `thin`,
/// grouped by primitive; a group
/// is a fin when its cells lie in two parallel planes (each side's normals
/// within 8° of their mean) and it is thin for its size (thickness under
/// [`FIN_THICKNESS`] × √planform).
pub(super) fn find_fins(surface: &Surface, cells: &Cells, geometry: &[CellGeometry], thin: f64) -> Vec<Fin> {
    let mut by_primitive: Vec<(u32, Vec<u32>, f64)> = Vec::new();
    // A fin's opposite face is its own primitive's: each cell is cast only
    // against those triangles (O(cells × triangles per primitive)).
    let primitives = cells.cells.iter().map(|c| c.primitive as usize + 1).max().unwrap_or(0);
    let mut own: Vec<Vec<u32>> = vec![Vec::new(); primitives];
    for (t, &cell) in cells.tri_cell.iter().enumerate() {
        own[cells.cells[cell as usize].primitive as usize].push(t as u32);
    }
    for (i, g) in geometry.iter().enumerate() {
        let n = g.normal;
        let mut nearest = f64::INFINITY;
        for &t in &own[cells.cells[i].primitive as usize] {
            let tri = &surface.triangles[t as usize];
            let [a, b, c] = tri.map(|k| surface.positions[k as usize]);
            let tn = (b - a).cross(c - a);
            if tn.dot(n) >= -0.9 * tn.length() {
                continue;
            }
            if let Some(t) = ray_triangle(g.centroid, -n, a, b, c) {
                if t > 1e-6 {
                    nearest = nearest.min(t);
                }
            }
        }
        if nearest < thin {
            let p = cells.cells[i].primitive;
            match by_primitive.iter_mut().find(|e| e.0 == p) {
                Some(e) => {
                    e.1.push(i as u32);
                    e.2 = e.2.max(nearest);
                }
                None => by_primitive.push((p, vec![i as u32], nearest)),
            }
        }
    }
    by_primitive
        .into_iter()
        .filter_map(|(_, list, thickness)| {
            let n0 = geometry[list[0] as usize].normal;
            let (mut normal, mut area, mut centroid, mut total) = (DVec3::ZERO, 0.0, DVec3::ZERO, 0.0);
            for &i in &list {
                let g = &geometry[i as usize];
                if g.normal.dot(n0) > 0.0 {
                    normal += g.normal * g.area;
                    area += g.area;
                }
                centroid += g.centroid * g.area;
                total += g.area;
            }
            if area <= 0.0 || normal.length_squared() == 0.0 {
                return None;
            }
            let normal = normal.normalize();
            let flat = list.iter().all(|&i| geometry[i as usize].normal.dot(normal).abs() > FIN_FLAT);
            (flat && thickness < FIN_THICKNESS * area.sqrt()).then(|| Fin {
                cells: list,
                normal,
                area,
                centroid: centroid / total,
                body_radius: 0.0,
                semispan: 0.0,
            })
        })
        .collect()
}

/// A fin's two sides are planes: normals within this cosine (8°) of the mean.
const FIN_FLAT: f64 = 0.99;
/// A fin is thinner than this × the square root of its planform area.
const FIN_THICKNESS: f64 = 0.1;

/// Distance along the unit ray `dir` from `o` to triangle abc (Möller–Trumbore).
fn ray_triangle(o: DVec3, dir: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-15 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = dir.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(e2.dot(q) * inv)
}
