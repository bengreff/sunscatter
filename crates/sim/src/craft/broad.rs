//! Broad phase for point-in-primitive tests (hot spot 5 of the scale
//! baseline, `docs/archive/research/scale-baseline-2026-10-26.md`): the union surface
//! ([`super::mesh::union_surface`]) and the interior grid
//! ([`super::volume::VolumeGrid`]) ask whether points lie inside any
//! primitive, which was a scan of every primitive per point (O(points ×
//! primitives): 70 % of a 1,000-part build).
//!
//! [`Broad`] keeps each primitive's bounding box in a uniform grid of
//! buckets; a query visits only the primitives whose box covers the
//! point's bucket, in ascending index order, and tests their solids with
//! [`contains`]. The answers are exactly the scan's (the boxes are
//! conservative and only skip primitives that cannot contain the point),
//! so every surface and grid built through it is the same bits.

use super::file::Shape;
use super::mesh::{basis, contains};
use glam::DVec3;

/// Slack added to every bounding box (m): the boxes are computed in f64 and
/// must cover every point a solid strictly contains.
const SLACK: f64 = 1e-6;
/// Most buckets in the grid.
const MAX_BUCKETS: f64 = 2.0e6;

/// Axis-aligned bounds of a primitive's solid (conservative).
pub fn bounds(shape: &Shape) -> (DVec3, DVec3) {
    // A disc of radius r about c with unit normal w spans r·√(1 − w_k²)
    // along each coordinate axis k.
    let disc = |c: DVec3, w: DVec3, r: f64| {
        let e = (DVec3::ONE - w * w).max(DVec3::ZERO);
        let half = DVec3::new(e.x.sqrt(), e.y.sqrt(), e.z.sqrt()) * r;
        (c - half, c + half)
    };
    let frustum = |base: DVec3, axis: DVec3, r0: f64, r1: f64| {
        let (_, _, w) = basis(axis);
        let (a, b) = (disc(base, w, r0), disc(base + axis, w, r1));
        (a.0.min(b.0), a.1.max(b.1))
    };
    let (lo, hi) = match *shape {
        Shape::Frustum { base, axis, r_base, r_top } => frustum(base, axis, r_base, r_top),
        Shape::Cylinder { base, axis, radius } => frustum(base, axis, radius, radius),
        Shape::Strut { from, to, radius } => frustum(from, to - from, radius, radius),
        Shape::SphereCap { center, radius, .. } => (center - DVec3::splat(radius), center + DVec3::splat(radius)),
        Shape::Box { center, half } => (center - half, center + half),
    };
    (lo - DVec3::splat(SLACK), hi + DVec3::splat(SLACK))
}

/// Primitives' bounding boxes in a grid of buckets.
#[derive(Clone, Debug)]
pub struct Broad {
    shapes: Vec<Shape>,
    boxes: Vec<(DVec3, DVec3)>,
    lo: DVec3,
    /// Bucket edge (m).
    size: f64,
    dims: [i64; 3],
    /// Bucket b's primitives: `items[start[b]..start[b + 1]]`, ascending.
    start: Vec<u32>,
    items: Vec<u32>,
}

impl Broad {
    /// The broad phase of `shapes` (indices are their positions).
    pub fn new<'a>(shapes: impl IntoIterator<Item = &'a Shape>) -> Self {
        let shapes: Vec<Shape> = shapes.into_iter().copied().collect();
        let boxes: Vec<(DVec3, DVec3)> = shapes.iter().map(bounds).collect();
        let (lo, hi) = boxes
            .iter()
            .fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(lo, hi), b| (lo.min(b.0), hi.max(b.1)));
        if shapes.is_empty() {
            return Broad {
                shapes,
                boxes,
                lo: DVec3::ZERO,
                size: 1.0,
                dims: [1; 3],
                start: vec![0, 0],
                items: Vec::new(),
            };
        }
        // About one bucket per primitive's worth of the bounds' volume, at
        // most MAX_BUCKETS.
        let ext = (hi - lo).max(DVec3::splat(1e-3));
        let volume = ext.x * ext.y * ext.z;
        let size = libm::cbrt(volume / shapes.len() as f64).max(libm::cbrt(volume / MAX_BUCKETS));
        let dims = (ext / size).ceil().max(DVec3::ONE).as_i64vec3().to_array();
        let n = (dims[0] * dims[1] * dims[2]) as usize;
        let mut count = vec![0u32; n + 1];
        let each = |b: &(DVec3, DVec3), f: &mut dyn FnMut(usize)| {
            let (a, z) = (Self::cell_of(lo, size, dims, b.0), Self::cell_of(lo, size, dims, b.1));
            for k in a[2]..=z[2] {
                for j in a[1]..=z[1] {
                    for i in a[0]..=z[0] {
                        f(((k * dims[1] + j) * dims[0] + i) as usize);
                    }
                }
            }
        };
        for b in &boxes {
            each(b, &mut |c| count[c + 1] += 1);
        }
        for c in 0..n {
            count[c + 1] += count[c];
        }
        let mut fill = count.clone();
        let mut items = vec![0u32; count[n] as usize];
        for (p, b) in boxes.iter().enumerate() {
            each(b, &mut |c| {
                items[fill[c] as usize] = p as u32;
                fill[c] += 1;
            });
        }
        Broad { shapes, boxes, lo, size, dims, start: count, items }
    }

    /// The bucket coordinates of `p`, clamped into the grid.
    fn cell_of(lo: DVec3, size: f64, dims: [i64; 3], p: DVec3) -> [i64; 3] {
        let c = ((p - lo) / size).floor().as_i64vec3().to_array();
        [c[0].clamp(0, dims[0] - 1), c[1].clamp(0, dims[1] - 1), c[2].clamp(0, dims[2] - 1)]
    }

    /// The primitives whose boxes may hold `p`, ascending (empty outside
    /// every box's bucket).
    fn candidates(&self, p: DVec3) -> &[u32] {
        let c = ((p - self.lo) / self.size).floor();
        let d = self.dims;
        if !(c.x >= 0.0 && c.y >= 0.0 && c.z >= 0.0) || c.x >= d[0] as f64 || c.y >= d[1] as f64 || c.z >= d[2] as f64 {
            return &[];
        }
        let b = ((c.z as i64 * d[1] + c.y as i64) * d[0] + c.x as i64) as usize;
        &self.items[self.start[b] as usize..self.start[b + 1] as usize]
    }

    fn in_box(&self, i: u32, p: DVec3) -> bool {
        let (lo, hi) = self.boxes[i as usize];
        p.cmpge(lo).all() && p.cmple(hi).all()
    }

    /// Whether any primitive but `except` strictly contains `p`.
    pub fn any_contains(&self, p: DVec3, except: Option<usize>) -> bool {
        self.candidates(p)
            .iter()
            .any(|&i| Some(i as usize) != except && self.in_box(i, p) && contains(&self.shapes[i as usize], p))
    }

    /// The lowest-index primitive that strictly contains `p`.
    pub fn first_containing(&self, p: DVec3) -> Option<u32> {
        self.candidates(p).iter().copied().find(|&i| self.in_box(i, p) && contains(&self.shapes[i as usize], p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::stress::layout;

    /// Against the scan of every primitive, at points spread through and
    /// around a stress craft and the test craft: the same answers.
    #[test]
    fn the_broad_phase_answers_as_the_full_scan() {
        let craft = crate::craft::test_craft();
        for shapes in [
            layout(120).geometry.primitives.iter().map(|p| p.shape).collect::<Vec<_>>(),
            craft.geometry.primitives.iter().map(|p| p.shape).collect(),
        ] {
            let broad = Broad::new(&shapes);
            let (lo, hi) = shapes
                .iter()
                .map(bounds)
                .fold((DVec3::splat(1e9), DVec3::splat(-1e9)), |(a, b), x| (a.min(x.0), b.max(x.1)));
            let (lo, hi) = (lo - DVec3::ONE, hi + DVec3::ONE);
            let n = 40;
            let mut inside = 0;
            for i in 0..n {
                for j in 0..n {
                    for k in 0..n {
                        let f = DVec3::new(i as f64, j as f64, k as f64) / (n - 1) as f64;
                        let p = lo + (hi - lo) * f;
                        let scan = shapes.iter().position(|s| contains(s, p)).map(|i| i as u32);
                        assert_eq!(broad.first_containing(p), scan, "{p}");
                        assert_eq!(broad.any_contains(p, None), scan.is_some());
                        if let Some(s) = scan {
                            let others = shapes.iter().enumerate().any(|(q, x)| q != s as usize && contains(x, p));
                            assert_eq!(broad.any_contains(p, Some(s as usize)), others);
                            inside += 1;
                        }
                    }
                }
            }
            assert!(inside > 100, "{inside} points inside");
        }
    }

    #[test]
    fn bounds_cover_their_solids() {
        let shapes = layout(60).geometry.primitives.iter().map(|p| p.shape).collect::<Vec<_>>();
        for s in &shapes {
            let (lo, hi) = bounds(s);
            let mut s2 = crate::craft::mesh::Surface::default();
            crate::craft::mesh::tessellate(&mut s2, 0, s, &crate::craft::Resolution::default());
            for p in &s2.positions {
                assert!(p.cmpge(lo).all() && p.cmple(hi).all(), "{s:?}: {p} outside {lo}..{hi}");
            }
        }
    }
}
