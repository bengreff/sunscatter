//! Craft geometry → triangle surface (realism-1 §3a).
//!
//! Each primitive is tessellated into a closed triangle surface (outward
//! normals, counter-clockwise seen from outside); where primitives overlap,
//! triangles lying inside another primitive are removed, leaving the outer
//! surface of their union. The result feeds the surface cells
//! ([`super::cells`]) and the render mesh ([`RenderMesh`]).
//!
//! Angles go through `sim::math` (rule 2): the surface is bit-identical on
//! every platform.

use super::file::{Primitive, Shape};
use crate::math;
use glam::DVec3;

/// Tessellation fineness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolution {
    /// Longest edge along a surface (m).
    pub max_edge: f64,
    /// Largest angle between segments around an axis (rad).
    pub max_angle: f64,
}

impl Default for Resolution {
    fn default() -> Self {
        Resolution { max_edge: 0.15, max_angle: 10.0 * math::PI / 180.0 }
    }
}

/// A patch: one smooth face of one primitive (a side, a cap, a box face).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patch {
    pub primitive: u32,
}

/// A triangle surface in body axes (f64; the simulation's copy).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Surface {
    pub positions: Vec<DVec3>,
    /// Per-vertex normals (smooth across curved patches, for drawing).
    pub normals: Vec<DVec3>,
    pub triangles: Vec<[u32; 3]>,
    /// Patch of each triangle.
    pub tri_patch: Vec<u32>,
    pub patches: Vec<Patch>,
}

/// The mesh the game draws: plain arrays, ready for any engine.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

impl Surface {
    /// (centroid, unit normal, area) of triangle `t`.
    pub fn triangle(&self, t: usize) -> (DVec3, DVec3, f64) {
        let [a, b, c] = self.triangles[t].map(|i| self.positions[i as usize]);
        let n = (b - a).cross(c - a);
        let len = n.length();
        ((a + b + c) / 3.0, if len > 0.0 { n / len } else { DVec3::ZERO }, 0.5 * len)
    }

    pub fn area(&self) -> f64 {
        (0..self.triangles.len()).map(|t| self.triangle(t).2).sum()
    }

    /// The render mesh: only the vertices the triangles use, in f32.
    pub fn render_mesh(&self) -> RenderMesh {
        let mut remap = vec![u32::MAX; self.positions.len()];
        let mut mesh = RenderMesh::default();
        for tri in &self.triangles {
            for &i in tri {
                let i = i as usize;
                if remap[i] == u32::MAX {
                    remap[i] = mesh.positions.len() as u32;
                    mesh.positions.push(self.positions[i].as_vec3().to_array());
                    mesh.normals.push(self.normals[i].as_vec3().to_array());
                }
                mesh.indices.push(remap[i]);
            }
        }
        mesh
    }

    /// Appends a patch and returns its index.
    fn patch(&mut self, primitive: u32) -> u32 {
        self.patches.push(Patch { primitive });
        (self.patches.len() - 1) as u32
    }

    fn vertex(&mut self, p: DVec3, n: DVec3) -> u32 {
        self.positions.push(p);
        self.normals.push(n);
        (self.positions.len() - 1) as u32
    }

    /// Adds triangle (a, b, c) unless degenerate, oriented so that its face
    /// normal agrees with the vertices' outward normals.
    fn tri(&mut self, patch: u32, a: u32, b: u32, c: u32) {
        let [pa, pb, pc] = [a, b, c].map(|i| self.positions[i as usize]);
        let face = (pb - pa).cross(pc - pa);
        if face.length_squared() < 1e-24 {
            return;
        }
        let outward = self.normals[a as usize] + self.normals[b as usize] + self.normals[c as usize];
        self.triangles.push(if face.dot(outward) >= 0.0 { [a, b, c] } else { [a, c, b] });
        self.tri_patch.push(patch);
    }
}

/// Right-handed orthonormal (u, v, w) with w along `axis`, built without
/// trig: u from the coordinate axis least aligned with `axis`.
pub fn basis(axis: DVec3) -> (DVec3, DVec3, DVec3) {
    let w = axis.normalize();
    let a = w.abs();
    let e = if a.x <= a.y && a.x <= a.z {
        DVec3::X
    } else if a.y <= a.z {
        DVec3::Y
    } else {
        DVec3::Z
    };
    let u = (e - w * e.dot(w)).normalize();
    (u, w.cross(u), w)
}

fn segments(radius: f64, res: &Resolution) -> usize {
    let by_edge = libm::ceil(math::TAU * radius / res.max_edge) as usize;
    let by_angle = libm::ceil(math::TAU / res.max_angle) as usize;
    by_edge.max(by_angle).max(8)
}

fn steps(length: f64, max: f64) -> usize {
    (libm::ceil(length / max) as usize).max(1)
}

/// A surface of revolution about `axis` through `origin`: profile points
/// (radius, height along the axis, outward normal as (radial, axial)).
/// Rings of `n` vertices; rings of radius 0 are poles.
fn lathe(s: &mut Surface, patch: u32, origin: DVec3, axis: DVec3, profile: &[(f64, f64, (f64, f64))], n: usize) {
    let (u, v, w) = basis(axis);
    let trig: Vec<(f64, f64)> = (0..n)
        .map(|k| (math::cos(math::TAU * k as f64 / n as f64), math::sin(math::TAU * k as f64 / n as f64)))
        .collect();
    let rings: Vec<u32> = profile
        .iter()
        .map(|&(r, h, (nr, nh))| {
            let first = s.positions.len() as u32;
            for &(c, sn) in &trig {
                let radial = u * c + v * sn;
                s.vertex(origin + w * h + radial * r, (radial * nr + w * nh).normalize());
            }
            first
        })
        .collect();
    for j in 0..profile.len() - 1 {
        let (r0, r1) = (rings[j], rings[j + 1]);
        for k in 0..n {
            let k1 = (k + 1) % n;
            let (a, b) = (r0 + k as u32, r0 + k1 as u32);
            let (c, d) = (r1 + k as u32, r1 + k1 as u32);
            if profile[j].0 > 0.0 {
                s.tri(patch, a, b, d);
            }
            if profile[j + 1].0 > 0.0 {
                s.tri(patch, a, d, c);
            }
        }
    }
}

/// A flat disc of radius `r` at height `h`, facing `sign` × axis, with `n`
/// segments (those of the side it closes, so the rim vertices coincide).
struct Disc {
    r: f64,
    h: f64,
    sign: f64,
    n: usize,
}

fn disc(s: &mut Surface, primitive: u32, origin: DVec3, axis: DVec3, d: Disc, res: &Resolution) {
    if d.r <= 0.0 {
        return;
    }
    let patch = s.patch(primitive);
    let rings = steps(d.r, res.max_edge);
    let profile: Vec<_> = (0..=rings)
        .map(|i| (if i == rings { d.r } else { d.r * i as f64 / rings as f64 }, d.h, (0.0, d.sign)))
        .collect();
    lathe(s, patch, origin, axis, &profile, d.n);
}

fn frustum(s: &mut Surface, primitive: u32, base: DVec3, axis: DVec3, r0: f64, r1: f64, res: &Resolution) {
    let h = axis.length();
    let (dr, slant) = (r1 - r0, ((r1 - r0) * (r1 - r0) + h * h).sqrt());
    let normal = (h / slant, -dr / slant);
    let n = segments(r0.max(r1), res);
    let rings = steps(slant, res.max_edge);
    let profile: Vec<_> = (0..=rings)
        .map(|i| {
            if i == rings {
                (r1, h, normal)
            } else {
                let f = i as f64 / rings as f64;
                (r0 + dr * f, h * f, normal)
            }
        })
        .collect();
    let patch = s.patch(primitive);
    lathe(s, patch, base, axis, &profile, n);
    disc(s, primitive, base, axis, Disc { r: r0, h: 0.0, sign: -1.0, n }, res);
    disc(s, primitive, base, axis, Disc { r: r1, h, sign: 1.0, n }, res);
}

fn sphere_cap(s: &mut Surface, primitive: u32, center: DVec3, axis: DVec3, r: f64, height: f64, res: &Resolution) {
    let alpha = math::acos(((r - height) / r).clamp(-1.0, 1.0));
    let rings = steps(r * alpha, res.max_edge).max(steps(alpha, res.max_angle));
    let profile: Vec<_> = (0..=rings)
        .map(|i| {
            let phi = alpha * i as f64 / rings as f64;
            let (sn, c) = (math::sin(phi), math::cos(phi));
            (r * sn, r * c, (sn, c))
        })
        .collect();
    let (patch, n) = (s.patch(primitive), segments(r, res));
    lathe(s, patch, center, axis, &profile, n);
    disc(s, primitive, center, axis, Disc { r: r * math::sin(alpha), h: r * math::cos(alpha), sign: -1.0, n }, res);
}

fn cuboid(s: &mut Surface, primitive: u32, center: DVec3, half: DVec3, res: &Resolution) {
    for axis in 0..3 {
        for sign in [-1.0, 1.0] {
            let n = DVec3::AXES[axis] * sign;
            let (i, j) = ((axis + 1) % 3, (axis + 2) % 3);
            let (ei, ej) = (DVec3::AXES[i] * half[i], DVec3::AXES[j] * half[j]);
            let (ni, nj) = (steps(2.0 * half[i], res.max_edge), steps(2.0 * half[j], res.max_edge));
            let patch = s.patch(primitive);
            let first = s.positions.len() as u32;
            for a in 0..=ni {
                for b in 0..=nj {
                    let (fa, fb) = (2.0 * a as f64 / ni as f64 - 1.0, 2.0 * b as f64 / nj as f64 - 1.0);
                    s.vertex(center + n * half[axis] + ei * fa + ej * fb, n);
                }
            }
            let at = |a: usize, b: usize| first + (a * (nj + 1) + b) as u32;
            for a in 0..ni {
                for b in 0..nj {
                    s.tri(patch, at(a, b), at(a + 1, b), at(a + 1, b + 1));
                    s.tri(patch, at(a, b), at(a + 1, b + 1), at(a, b + 1));
                }
            }
        }
    }
}

/// Appends the closed surface of one primitive.
pub fn tessellate(s: &mut Surface, primitive: u32, shape: &Shape, res: &Resolution) {
    match *shape {
        Shape::Frustum { base, axis, r_base, r_top } => frustum(s, primitive, base, axis, r_base, r_top, res),
        Shape::Cylinder { base, axis, radius } => frustum(s, primitive, base, axis, radius, radius, res),
        Shape::Strut { from, to, radius } => frustum(s, primitive, from, to - from, radius, radius, res),
        Shape::SphereCap { center, axis, radius, height } => {
            sphere_cap(s, primitive, center, axis, radius, height, res)
        }
        Shape::Box { center, half } => cuboid(s, primitive, center, half, res),
    }
}

/// Whether `p` is strictly inside the solid.
pub fn contains(shape: &Shape, p: DVec3) -> bool {
    let in_frustum = |base: DVec3, axis: DVec3, r0: f64, r1: f64| {
        let h2 = axis.length_squared();
        let t = (p - base).dot(axis) / h2;
        if !(t > 0.0 && t < 1.0) {
            return false;
        }
        let radial = (p - base - axis * t).length();
        radial < r0 + (r1 - r0) * t
    };
    match *shape {
        Shape::Frustum { base, axis, r_base, r_top } => in_frustum(base, axis, r_base, r_top),
        Shape::Cylinder { base, axis, radius } => in_frustum(base, axis, radius, radius),
        Shape::Strut { from, to, radius } => in_frustum(from, to - from, radius, radius),
        Shape::SphereCap { center, axis, radius, height } => {
            let d = p - center;
            d.length_squared() < radius * radius && d.dot(axis.normalize()) > radius - height
        }
        Shape::Box { center, half } => {
            let d = (p - center).abs();
            d.x < half.x && d.y < half.y && d.z < half.z
        }
    }
}

/// Offset used to decide which side of a face lies inside another primitive (m).
const INSIDE_PROBE: f64 = 1e-4;

/// The outer surface of the primitives' union: every primitive tessellated,
/// then triangles whose outer side lies inside another primitive removed
/// (overlaps and coincident faces between touching primitives).
pub fn union_surface(primitives: &[Primitive], res: &Resolution) -> Surface {
    let mut s = Surface::default();
    for (i, p) in primitives.iter().enumerate() {
        tessellate(&mut s, i as u32, &p.shape, res);
    }
    let keep: Vec<bool> = (0..s.triangles.len())
        .map(|t| {
            let (c, n, _) = s.triangle(t);
            let own = s.patches[s.tri_patch[t] as usize].primitive as usize;
            let probe = c + n * INSIDE_PROBE;
            !primitives.iter().enumerate().any(|(j, p)| j != own && contains(&p.shape, probe))
        })
        .collect();
    let mut k = keep.iter();
    s.triangles.retain(|_| *k.next().expect("one flag per triangle"));
    let mut k = keep.iter();
    s.tri_patch.retain(|_| *k.next().expect("one flag per triangle"));
    s
}

/// Surface area of a primitive's solid (analytic).
pub fn analytic_area(shape: &Shape) -> f64 {
    let frustum = |h: f64, r0: f64, r1: f64| {
        let slant = ((r1 - r0) * (r1 - r0) + h * h).sqrt();
        math::PI * ((r0 + r1) * slant + r0 * r0 + r1 * r1)
    };
    match *shape {
        Shape::Frustum { axis, r_base, r_top, .. } => frustum(axis.length(), r_base, r_top),
        Shape::Cylinder { axis, radius, .. } => frustum(axis.length(), radius, radius),
        Shape::Strut { from, to, radius } => frustum((to - from).length(), radius, radius),
        Shape::SphereCap { radius, height, .. } => {
            let base_r2 = radius * radius - (radius - height) * (radius - height);
            math::TAU * radius * height + math::PI * base_r2
        }
        Shape::Box { half, .. } => 8.0 * (half.x * half.y + half.y * half.z + half.z * half.x),
    }
}

/// Volume of a primitive's solid (analytic).
pub fn analytic_volume(shape: &Shape) -> f64 {
    let frustum = |h: f64, r0: f64, r1: f64| math::PI * h * (r0 * r0 + r0 * r1 + r1 * r1) / 3.0;
    match *shape {
        Shape::Frustum { axis, r_base, r_top, .. } => frustum(axis.length(), r_base, r_top),
        Shape::Cylinder { axis, radius, .. } => frustum(axis.length(), radius, radius),
        Shape::Strut { from, to, radius } => frustum((to - from).length(), radius, radius),
        Shape::SphereCap { radius, height, .. } => math::PI * height * height * (3.0 * radius - height) / 3.0,
        Shape::Box { half, .. } => 8.0 * half.x * half.y * half.z,
    }
}

/// Key of a vertex position for matching coincident vertices (0.1 mm grid).
pub fn position_key(p: DVec3) -> [i64; 3] {
    p.to_array().map(|x| libm::round(x * 1e4) as i64)
}

/// Edge key: the two vertex keys, ordered.
pub fn edge_key(a: DVec3, b: DVec3) -> [[i64; 3]; 2] {
    let (ka, kb) = (position_key(a), position_key(b));
    if ka <= kb {
        [ka, kb]
    } else {
        [kb, ka]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn shapes() -> Vec<Shape> {
        let z = DVec3::Z;
        vec![
            Shape::Frustum { base: DVec3::new(0.3, -0.2, 1.0), axis: z * 2.0, r_base: 1.5, r_top: 0.5 },
            Shape::Frustum { base: DVec3::ZERO, axis: DVec3::new(1.0, 2.0, -0.5), r_base: 0.0, r_top: 0.7 },
            Shape::Cylinder { base: DVec3::ZERO, axis: z * 3.2, radius: 2.0 },
            Shape::SphereCap { center: DVec3::ZERO, axis: z, radius: 0.875, height: 0.35 },
            Shape::SphereCap { center: DVec3::X, axis: -DVec3::Y, radius: 1.0, height: 1.0 },
            Shape::SphereCap { center: DVec3::ZERO, axis: z, radius: 1.0, height: 1.8 },
            Shape::Box { center: DVec3::new(1.0, 2.0, 3.0), half: DVec3::new(0.5, 1.0, 0.25) },
            Shape::Strut { from: DVec3::new(1.9, 0.0, -2.2), to: DVec3::new(3.0, 0.0, -4.52), radius: 0.08 },
        ]
    }

    fn single(shape: &Shape) -> Surface {
        let mut s = Surface::default();
        tessellate(&mut s, 0, shape, &Resolution::default());
        s
    }

    #[test]
    fn primitive_area_and_volume_match_the_analytic_values() {
        for shape in shapes() {
            let s = single(&shape);
            let (area, exact) = (s.area(), analytic_area(&shape));
            assert!((area / exact - 1.0).abs() < 0.01, "{shape:?}: area {area} vs {exact}");
            // Divergence theorem: V = ⅓ Σ (c·n) a, positive only if normals point out.
            let vol: f64 = (0..s.triangles.len())
                .map(|t| {
                    let (c, n, a) = s.triangle(t);
                    c.dot(n) * a / 3.0
                })
                .sum();
            let exact_v = analytic_volume(&shape);
            // Thin caps lose a little more volume than area to the chords.
            assert!((vol / exact_v - 1.0).abs() < 0.03, "{shape:?}: volume {vol} vs {exact_v}");
        }
    }

    #[test]
    fn primitive_surfaces_are_closed() {
        for shape in shapes() {
            let s = single(&shape);
            let mut edges: BTreeMap<_, u32> = BTreeMap::new();
            for tri in &s.triangles {
                let p = tri.map(|i| s.positions[i as usize]);
                for k in 0..3 {
                    *edges.entry(edge_key(p[k], p[(k + 1) % 3])).or_default() += 1;
                }
            }
            let open = edges.values().filter(|&&n| n != 2).count();
            assert_eq!(open, 0, "{shape:?}: {open} edges not shared by exactly two triangles");
            let vector_area: DVec3 = (0..s.triangles.len()).map(|t| s.triangle(t).1 * s.triangle(t).2).sum();
            assert!(vector_area.length() < 1e-9 * s.area(), "{shape:?}: Σ n·a = {vector_area}");
        }
    }

    #[test]
    fn inside_test_table() {
        let cyl = Shape::Cylinder { base: DVec3::ZERO, axis: DVec3::Z * 2.0, radius: 1.0 };
        let cap = Shape::SphereCap { center: DVec3::ZERO, axis: DVec3::Z, radius: 1.0, height: 0.5 };
        let bx = Shape::Box { center: DVec3::ZERO, half: DVec3::ONE };
        let cases = [
            (cyl, DVec3::new(0.5, 0.5, 1.0), true),
            (cyl, DVec3::new(0.8, 0.8, 1.0), false),
            (cyl, DVec3::new(0.0, 0.0, 2.1), false),
            (cyl, DVec3::new(0.0, 0.0, -0.1), false),
            (cap, DVec3::new(0.0, 0.0, 0.9), true),
            (cap, DVec3::new(0.0, 0.0, 0.4), false),
            (cap, DVec3::new(0.8, 0.0, 0.55), true),
            (bx, DVec3::new(0.9, -0.9, 0.9), true),
            (bx, DVec3::new(1.1, 0.0, 0.0), false),
        ];
        for (shape, p, inside) in cases {
            assert_eq!(contains(&shape, p), inside, "{shape:?} {p}");
        }
    }

    #[test]
    fn stacked_cylinders_lose_their_shared_caps() {
        let prim = |z0: f64, h: f64| Primitive {
            name: String::new(),
            shape: Shape::Cylinder { base: DVec3::Z * z0, axis: DVec3::Z * h, radius: 1.0 },
            foot: false,
            skin: None,
            nozzle: false,
        };
        let s = union_surface(&[prim(0.0, 1.0), prim(1.0, 2.0)], &Resolution::default());
        let exact = analytic_area(&Shape::Cylinder { base: DVec3::ZERO, axis: DVec3::Z * 3.0, radius: 1.0 });
        assert!((s.area() / exact - 1.0).abs() < 0.01, "area {} vs {exact}", s.area());
        // A cylinder inside another vanishes entirely.
        let inner = Primitive {
            shape: Shape::Cylinder { base: DVec3::Z * 0.5, axis: DVec3::Z, radius: 0.5 },
            ..prim(0.0, 1.0)
        };
        let s2 = union_surface(&[prim(0.0, 3.0), inner], &Resolution::default());
        assert!((s2.area() / exact - 1.0).abs() < 0.01, "area {} vs {exact}", s2.area());
    }

    #[test]
    fn basis_is_right_handed_and_orthonormal() {
        for a in [DVec3::Z, DVec3::X, -DVec3::Y, DVec3::new(1.0, 2.0, -0.5)] {
            let (u, v, w) = basis(a);
            assert!(
                (u.cross(v) - w).length() < 1e-15 && u.dot(w).abs() < 1e-15 && (w - a.normalize()).length() < 1e-15
            );
        }
    }
}
