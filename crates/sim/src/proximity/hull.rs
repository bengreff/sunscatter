//! A craft's collision hull for craft-to-craft contact (D082): the exact
//! solids of its primitives, for the signed distance of a point to the
//! craft, and points sampled on its outer surface, which touch other craft.
//!
//! **The solid** is the union of the geometry's convex primitives
//! (`craft::file::Shape`). [`signed_distance`] is exact per primitive
//! (the 2D distance to the profile of a frustum, cylinder or strut about
//! its axis; to the arc and base disc of a sphere cap; to a box). A point
//! inside the union ([`Hull::penetration`]) leaves through the nearest
//! face, of every primitive holding it ([`exits`]), whose outer side is
//! not inside another primitive: a point near a joint is pushed out of the
//! craft, not through the face two primitives share (in a concave corner,
//! where every exit near it is covered, the nearest face).
//!
//! **Sample points** cover every primitive's surface at most [`SPACING`]
//! apart (rings along the sides including both rims, rings on the caps,
//! grids on box faces with their edges), keeping those whose outer side
//! is not inside another primitive (the union surface, as
//! `craft::mesh::union_surface` decides it). Each carries its share of its
//! surface's area (the spring of craft contact is per area) and its
//! primitive (so its part). The thermal cells are too coarse for this (a
//! stress craft has 16 per part: two touching tanks could sink 20 cm into
//! each other between their centroids).
//!
//! Queries go through a bucket grid of the primitives' bounding boxes
//! (like `craft::broad`, which answers only "inside any"), in ascending
//! primitive order, so every answer is deterministic.

use crate::craft::broad::bounds;
use crate::craft::mesh::basis;
use crate::craft::parts::CraftDef;
use crate::craft::Shape;
use crate::math;
use glam::DVec3;

/// Largest distance between neighbouring sample points (m).
pub const SPACING: f64 = 0.2;
/// Fewest points on a ring (thin struts).
const MIN_RING: usize = 8;
/// Offset that decides which side of a surface lies inside another
/// primitive (m), as the union surface does.
const PROBE: f64 = 1e-4;
/// An exposed face this much deeper than a point's nearest face is not its
/// way out (m): the point is in a concave corner of the union.
const CORNER: f64 = SPACING;

/// Signed distance (m, negative inside) from `p` to the solid `shape`, and
/// the outward unit normal at the nearest surface point (the distance's
/// gradient).
pub fn signed_distance(shape: &Shape, p: DVec3) -> (f64, DVec3) {
    match *shape {
        Shape::Frustum { base, axis, r_base, r_top } => revolved(base, axis, r_base, r_top, p),
        Shape::Cylinder { base, axis, radius } => revolved(base, axis, radius, radius, p),
        Shape::Strut { from, to, radius } => revolved(from, to - from, radius, radius, p),
        Shape::SphereCap { center, axis, radius, height } => cap(center, axis, radius, height, p),
        Shape::Box { center, half } => cuboid(center, half, p),
    }
}

/// `p` in the (along, radial) coordinates of an axis through `origin`
/// along unit `w`, with the radial unit vector (any perpendicular on the
/// axis).
fn profile(origin: DVec3, w: DVec3, p: DVec3) -> (f64, f64, DVec3) {
    let rel = p - origin;
    let t = rel.dot(w);
    let radial = rel - w * t;
    let rho = radial.length();
    let u = if rho > 1e-12 { radial / rho } else { basis(w).0 };
    (t, rho, u)
}

/// The nearest point to `q` on the 2D segment `a`–`b`.
fn nearest_on_segment(a: [f64; 2], b: [f64; 2], q: [f64; 2]) -> [f64; 2] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let s = if len2 > 0.0 { (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    [a[0] + dx * s, a[1] + dy * s]
}

/// Signed distance in a profile plane from the nearest boundary point
/// `near` (with the boundary's outward normal `edge` there, used when the
/// point is on it) to `q`; the 2D gradient.
fn signed_2d(q: [f64; 2], near: [f64; 2], edge: [f64; 2], inside: bool) -> (f64, [f64; 2]) {
    let (dx, dy) = (q[0] - near[0], q[1] - near[1]);
    let d = (dx * dx + dy * dy).sqrt();
    if d == 0.0 {
        return (0.0, edge);
    }
    if inside {
        (-d, [-dx / d, -dy / d])
    } else {
        (d, [dx / d, dy / d])
    }
}

/// A solid of revolution: radius `r0` at `base`, `r1` at `base + axis`.
fn revolved(base: DVec3, axis: DVec3, r0: f64, r1: f64, p: DVec3) -> (f64, DVec3) {
    let h = axis.length();
    let w = axis / h;
    let (t, rho, u) = profile(base, w, p);
    let q = [t, rho];
    let inside = t > 0.0 && t < h && rho < r0 + (r1 - r0) * t / h;
    let slant = ((r1 - r0) * (r1 - r0) + h * h).sqrt();
    // The profile's three edges (the axis is not a boundary): bottom disc,
    // side, top disc, with their outward normals.
    let edges = [
        ([0.0, 0.0], [0.0, r0], [-1.0, 0.0]),
        ([0.0, r0], [h, r1], [-(r1 - r0) / slant, h / slant]),
        ([h, r1], [h, 0.0], [1.0, 0.0]),
    ];
    let mut best: Option<(f64, [f64; 2], [f64; 2])> = None;
    for (a, b, n) in edges {
        let near = nearest_on_segment(a, b, q);
        let d2 = (q[0] - near[0]) * (q[0] - near[0]) + (q[1] - near[1]) * (q[1] - near[1]);
        if best.is_none_or(|(bd, _, _)| d2 < bd) {
            best = Some((d2, near, n));
        }
    }
    let (_, near, n) = best.expect("three edges");
    let (d, g) = signed_2d(q, near, n, inside);
    (d, w * g[0] + u * g[1])
}

/// The part of the sphere (`center`, `radius`) at least `radius − height`
/// along `axis` from the centre, closed by its base disc.
fn cap(center: DVec3, axis: DVec3, radius: f64, height: f64, p: DVec3) -> (f64, DVec3) {
    let w = axis.normalize();
    let (t, rho, u) = profile(center, w, p);
    let q = [t, rho];
    let base_t = radius - height;
    let a = (radius * radius - base_t * base_t).max(0.0).sqrt();
    let len = (t * t + rho * rho).sqrt();
    let inside = len < radius && t > base_t;
    // The arc: the circle's nearest point if it lies on the cap, else the rim.
    let arc = if len == 0.0 {
        [radius, 0.0]
    } else if t / len >= base_t / radius {
        [t * radius / len, rho * radius / len]
    } else {
        [base_t, a]
    };
    let disc = [base_t, rho.clamp(0.0, a)];
    let dist2 = |x: [f64; 2]| (q[0] - x[0]) * (q[0] - x[0]) + (q[1] - x[1]) * (q[1] - x[1]);
    let (near, n) =
        if dist2(arc) <= dist2(disc) { (arc, [arc[0] / radius, arc[1] / radius]) } else { (disc, [-1.0, 0.0]) };
    let (d, g) = signed_2d(q, near, n, inside);
    (d, w * g[0] + u * g[1])
}

/// An axis-aligned box.
fn cuboid(center: DVec3, half: DVec3, p: DVec3) -> (f64, DVec3) {
    let d = p - center;
    let q = d.abs() - half;
    let sign = DVec3::new(d.x.signum(), d.y.signum(), d.z.signum());
    if q.max_element() > 0.0 {
        let o = q.max(DVec3::ZERO);
        let len = o.length();
        (len, o * sign / len)
    } else {
        let (k, depth) = if q.x >= q.y && q.x >= q.z {
            (DVec3::X, q.x)
        } else if q.y >= q.z {
            (DVec3::Y, q.y)
        } else {
            (DVec3::Z, q.z)
        };
        (depth, k * sign)
    }
}

/// A point on a hull's surface (body axes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HullPoint {
    pub pos: DVec3,
    /// Its primitive's outward normal there.
    pub normal: DVec3,
    /// The surface area it stands for (m²).
    pub area: f64,
}

/// A craft's collision hull (body axes).
#[derive(Clone, Debug)]
pub struct Hull {
    pub shapes: Vec<Shape>,
    /// The part owning each primitive.
    pub part: Vec<u32>,
    /// The sample points, grouped by primitive: primitive i's are
    /// `points[start[i]..start[i + 1]]`.
    pub points: Vec<HullPoint>,
    pub start: Vec<u32>,
    /// Each primitive's points' bounding sphere (centre, radius).
    pub spheres: Vec<(DVec3, f64)>,
    /// A sphere holding the whole solid (centre, radius).
    pub center: DVec3,
    pub radius: f64,
    grid: Grid,
}

impl Hull {
    /// The hull of a craft definition.
    pub fn new(def: &CraftDef) -> Self {
        let shapes: Vec<Shape> = def.geometry.primitives.iter().map(|p| p.shape).collect();
        let part = def.primitive_part();
        Self::of_shapes(shapes, part)
    }

    /// The hull of `shapes`, primitive i owned by part `part[i]`.
    pub fn of_shapes(shapes: Vec<Shape>, part: Vec<u32>) -> Self {
        let grid = Grid::new(&shapes);
        let mut points = Vec::new();
        let mut start = vec![0u32];
        let mut spheres = Vec::with_capacity(shapes.len());
        for (i, s) in shapes.iter().enumerate() {
            let first = points.len();
            for p in sample(s) {
                if !grid.any_contains(&shapes, p.pos + p.normal * PROBE, Some(i)) {
                    points.push(p);
                }
            }
            start.push(points.len() as u32);
            let (lo, hi) = bounds(s);
            let c = (lo + hi) * 0.5;
            let r = points[first..].iter().map(|p| (p.pos - c).length()).fold(0.0, f64::max);
            spheres.push((c, r + PROBE));
        }
        let (lo, hi) = shapes
            .iter()
            .map(bounds)
            .fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(a, b), x| (a.min(x.0), b.max(x.1)));
        let center = if shapes.is_empty() { DVec3::ZERO } else { (lo + hi) * 0.5 };
        let radius = shapes.iter().map(bounds).map(|(a, b)| (a - center).length().max((b - center).length()));
        let radius = radius.fold(0.0, f64::max);
        Hull { shapes, part, points, start, spheres, center, radius, grid }
    }

    /// The sample points of primitive `i`.
    pub fn points_of(&self, i: usize) -> &[HullPoint] {
        &self.points[self.start[i] as usize..self.start[i + 1] as usize]
    }

    /// Whether any primitive's bounding box comes within `radius` of `c`
    /// (body axes): a sphere that cannot touch the solid is skipped.
    pub fn near_sphere(&self, c: DVec3, radius: f64) -> bool {
        self.grid.near_sphere(c, radius)
    }

    /// Where `p` (body axes) is inside the solid: the depth (m, positive),
    /// the outward unit normal to push it out along, and the primitive
    /// (see the module for the union's rule). `None` outside.
    pub fn penetration(&self, p: DVec3) -> Option<(f64, DVec3, u32)> {
        let cands = self.grid.at(p)?;
        let mut faces: Vec<(f64, DVec3, DVec3, u32)> = Vec::new();
        for &i in cands {
            let (lo, hi) = self.grid.boxes[i as usize];
            let shape = &self.shapes[i as usize];
            if p.cmpge(lo).all() && p.cmple(hi).all() && crate::craft::mesh::contains(shape, p) {
                faces.extend(exits(shape, p).into_iter().map(|(d, q, n)| (d, q, n, i)));
            }
        }
        // Shallowest first (ties: primitive, then face order); an exposed
        // face much deeper than the shallowest means a concave corner,
        // where the shallowest face pushes the point along instead.
        faces.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.3.cmp(&b.3)));
        let limit = faces.first()?.0 + CORNER;
        let exposed = faces
            .iter()
            .take_while(|f| f.0 <= limit)
            .find(|&&(_, q, n, i)| !self.grid.any_contains(&self.shapes, q + n * PROBE, Some(i as usize)));
        exposed.or(faces.first()).map(|&(d, _, n, i)| (d, n, i))
    }
}

/// The ways out of a solid for a point `p` inside it: per face, the depth
/// to it (m), its nearest point and its outward normal there.
pub fn exits(shape: &Shape, p: DVec3) -> Vec<(f64, DVec3, DVec3)> {
    let mut out = Vec::new();
    let mut face = |q: DVec3, n: DVec3| out.push(((q - p).length(), q, n));
    let revolved = |base: DVec3, axis: DVec3, r0: f64, r1: f64, face: &mut dyn FnMut(DVec3, DVec3)| {
        let h = axis.length();
        let w = axis / h;
        let (t, rho, u) = profile(base, w, p);
        let slant = ((r1 - r0) * (r1 - r0) + h * h).sqrt();
        face(base + u * rho.min(r0), -w);
        let s = nearest_on_segment([0.0, r0], [h, r1], [t, rho]);
        face(base + w * s[0] + u * s[1], w * (-(r1 - r0) / slant) + u * (h / slant));
        face(base + w * h + u * rho.min(r1), w);
    };
    match *shape {
        Shape::Frustum { base, axis, r_base, r_top } => revolved(base, axis, r_base, r_top, &mut face),
        Shape::Cylinder { base, axis, radius } => revolved(base, axis, radius, radius, &mut face),
        Shape::Strut { from, to, radius } => revolved(from, to - from, radius, radius, &mut face),
        Shape::SphereCap { center, axis, radius, height } => {
            let w = axis.normalize();
            let (t, rho, u) = profile(center, w, p);
            let base_t = radius - height;
            let a = (radius * radius - base_t * base_t).max(0.0).sqrt();
            let len = (t * t + rho * rho).sqrt();
            let on_arc = len > 0.0 && t / len >= base_t / radius;
            let dir = if on_arc {
                (p - center) / len
            } else if len == 0.0 {
                w
            } else {
                w * (base_t / radius) + u * (a / radius)
            };
            face(center + dir * radius, dir);
            face(center + w * base_t + u * rho.min(a), -w);
        }
        Shape::Box { center, half } => {
            for k in 0..3 {
                for sign in [-1.0, 1.0] {
                    let mut n = DVec3::ZERO;
                    n[k] = sign;
                    let mut q = p;
                    q[k] = center[k] + sign * half[k];
                    face(q, n);
                }
            }
        }
    }
    out
}

/// A sample point of a surface with its area share.
fn point(pos: DVec3, normal: DVec3, area: f64) -> HullPoint {
    HullPoint { pos, normal, area }
}

/// Points on a ring of `radius` about `c` in the plane (u, v), at least
/// [`MIN_RING`] of them (one at the centre if the radius is zero).
fn ring(c: DVec3, u: DVec3, v: DVec3, radius: f64) -> Vec<DVec3> {
    if radius <= 0.0 {
        return vec![c];
    }
    let n = (libm::ceil(math::TAU * radius / SPACING) as usize).max(MIN_RING);
    (0..n)
        .map(|k| {
            let a = math::TAU * k as f64 / n as f64;
            c + (u * math::cos(a) + v * math::sin(a)) * radius
        })
        .collect()
}

/// A disc of `radius` about `c` with normal `n` (rings inside the rim; the
/// rim is the side's).
fn disc(c: DVec3, u: DVec3, v: DVec3, n: DVec3, radius: f64, out: &mut Vec<HullPoint>) {
    let rings = (libm::ceil(radius / SPACING) as usize).max(1);
    let mut pts = Vec::new();
    for k in 0..rings {
        pts.extend(ring(c, u, v, radius * k as f64 / rings as f64));
    }
    let area = math::PI * radius * radius / pts.len() as f64;
    out.extend(pts.into_iter().map(|p| point(p, n, area)));
}

/// Sample points on a primitive's surface (body axes).
fn sample(shape: &Shape) -> Vec<HullPoint> {
    let mut out = Vec::new();
    let revolved = |base: DVec3, axis: DVec3, r0: f64, r1: f64, out: &mut Vec<HullPoint>| {
        let h = axis.length();
        let (u, v, w) = basis(axis);
        let slant = ((r1 - r0) * (r1 - r0) + h * h).sqrt();
        let steps = (libm::ceil(slant / SPACING) as usize).max(1);
        let mut side = Vec::new();
        for j in 0..=steps {
            let f = j as f64 / steps as f64;
            let r = r0 + (r1 - r0) * f;
            for p in ring(base + axis * f, u, v, r) {
                let radial = (p - base - axis * f).normalize_or_zero();
                side.push((p, (radial * h - w * (r1 - r0)) / slant));
            }
        }
        let area = math::PI * (r0 + r1) * slant / side.len() as f64;
        out.extend(side.into_iter().map(|(p, n)| point(p, n, area)));
        disc(base, u, v, -w, r0, out);
        disc(base + axis, u, v, w, r1, out);
    };
    match *shape {
        Shape::Frustum { base, axis, r_base, r_top } => revolved(base, axis, r_base, r_top, &mut out),
        Shape::Cylinder { base, axis, radius } => revolved(base, axis, radius, radius, &mut out),
        Shape::Strut { from, to, radius } => revolved(from, to - from, radius, radius, &mut out),
        Shape::SphereCap { center, axis, radius, height } => {
            let (u, v, w) = basis(axis);
            let base_t = radius - height;
            let top = math::acos((base_t / radius).clamp(-1.0, 1.0));
            let steps = (libm::ceil(radius * top / SPACING) as usize).max(1);
            let mut arc = Vec::new();
            for j in 0..=steps {
                let theta = top * j as f64 / steps as f64;
                let c = center + w * (radius * math::cos(theta));
                for p in ring(c, u, v, radius * math::sin(theta)) {
                    arc.push((p, (p - center) / radius));
                }
            }
            let area = math::TAU * radius * height / arc.len() as f64;
            out.extend(arc.into_iter().map(|(p, n)| point(p, n, area)));
            let a = (radius * radius - base_t * base_t).max(0.0).sqrt();
            disc(center + w * base_t, u, v, -w, a, &mut out);
        }
        Shape::Box { center, half } => {
            for axis in 0..3 {
                for sign in [-1.0, 1.0] {
                    let (i, j) = ((axis + 1) % 3, (axis + 2) % 3);
                    let (ni, nj) = (
                        (libm::ceil(2.0 * half[i] / SPACING) as usize).max(1),
                        (libm::ceil(2.0 * half[j] / SPACING) as usize).max(1),
                    );
                    let mut n = DVec3::ZERO;
                    n[axis] = sign;
                    let area = 4.0 * half[i] * half[j] / ((ni + 1) * (nj + 1)) as f64;
                    for a in 0..=ni {
                        for b in 0..=nj {
                            let mut p = center + n * half[axis];
                            p[i] += half[i] * (2.0 * a as f64 / ni as f64 - 1.0);
                            p[j] += half[j] * (2.0 * b as f64 / nj as f64 - 1.0);
                            out.push(point(p, n, area));
                        }
                    }
                }
            }
        }
    }
    out
}

/// The primitives' bounding boxes in a uniform grid of buckets.
#[derive(Clone, Debug)]
struct Grid {
    boxes: Vec<(DVec3, DVec3)>,
    lo: DVec3,
    size: f64,
    dims: [i64; 3],
    /// Bucket b's primitives: `items[start[b]..start[b + 1]]`, ascending.
    start: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    fn new(shapes: &[Shape]) -> Self {
        let boxes: Vec<(DVec3, DVec3)> = shapes.iter().map(bounds).collect();
        let (lo, hi) = boxes
            .iter()
            .fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(lo, hi), b| (lo.min(b.0), hi.max(b.1)));
        if boxes.is_empty() {
            return Grid { boxes, lo: DVec3::ZERO, size: 1.0, dims: [1; 3], start: vec![0, 0], items: Vec::new() };
        }
        let ext = (hi - lo).max(DVec3::splat(1e-3));
        let volume = ext.x * ext.y * ext.z;
        // About one bucket per primitive, at most a million.
        let size = libm::cbrt(volume / boxes.len() as f64).max(libm::cbrt(volume / 1.0e6));
        let dims = (ext / size).ceil().max(DVec3::ONE).as_i64vec3().to_array();
        let n = (dims[0] * dims[1] * dims[2]) as usize;
        let mut count = vec![0u32; n + 1];
        let mut grid = Grid { boxes, lo, size, dims, start: Vec::new(), items: Vec::new() };
        for i in 0..grid.boxes.len() {
            let (a, b) = grid.boxes[i];
            grid.each_bucket(a, b, |c| count[c + 1] += 1);
        }
        for c in 0..n {
            count[c + 1] += count[c];
        }
        let mut fill = count.clone();
        let mut items = vec![0u32; count[n] as usize];
        for i in 0..grid.boxes.len() {
            let (a, b) = grid.boxes[i];
            grid.each_bucket(a, b, |c| {
                items[fill[c] as usize] = i as u32;
                fill[c] += 1;
            });
        }
        grid.start = count;
        grid.items = items;
        grid
    }

    /// Bucket coordinates of `p`, clamped into the grid.
    fn cell(&self, p: DVec3) -> [i64; 3] {
        let c = ((p - self.lo) / self.size).floor().as_i64vec3().to_array();
        [0, 1, 2].map(|k| c[k].clamp(0, self.dims[k] - 1))
    }

    /// Calls `f` with every bucket the box `a`..`b` overlaps.
    fn each_bucket(&self, a: DVec3, b: DVec3, mut f: impl FnMut(usize)) {
        let (a, b) = (self.cell(a), self.cell(b));
        for k in a[2]..=b[2] {
            for j in a[1]..=b[1] {
                for i in a[0]..=b[0] {
                    f(((k * self.dims[1] + j) * self.dims[0] + i) as usize);
                }
            }
        }
    }

    /// The primitives whose boxes may hold `p` (ascending), `None` outside
    /// the grid.
    fn at(&self, p: DVec3) -> Option<&[u32]> {
        let c = ((p - self.lo) / self.size).floor();
        let d = self.dims;
        let outside = !(c.x >= 0.0 && c.y >= 0.0 && c.z >= 0.0);
        if outside || c.x >= d[0] as f64 || c.y >= d[1] as f64 || c.z >= d[2] as f64 {
            return None;
        }
        let b = ((c.z as i64 * d[1] + c.y as i64) * d[0] + c.x as i64) as usize;
        Some(&self.items[self.start[b] as usize..self.start[b + 1] as usize])
    }

    /// Whether any primitive but `except` strictly contains `p`.
    fn any_contains(&self, shapes: &[Shape], p: DVec3, except: Option<usize>) -> bool {
        self.at(p).is_some_and(|c| {
            c.iter().any(|&i| {
                let (lo, hi) = self.boxes[i as usize];
                Some(i as usize) != except
                    && p.cmpge(lo).all()
                    && p.cmple(hi).all()
                    && crate::craft::mesh::contains(&shapes[i as usize], p)
            })
        })
    }

    /// Whether any box comes within `radius` of `c`.
    fn near_sphere(&self, c: DVec3, radius: f64) -> bool {
        let (a, b) = (c - DVec3::splat(radius), c + DVec3::splat(radius));
        if self.boxes.is_empty() || b.cmplt(self.lo).any() {
            return false;
        }
        let r2 = radius * radius;
        let mut hit = false;
        self.each_bucket(a, b, |bucket| {
            if hit {
                return;
            }
            for &i in &self.items[self.start[bucket] as usize..self.start[bucket + 1] as usize] {
                let (lo, hi) = self.boxes[i as usize];
                if (c.clamp(lo, hi) - c).length_squared() <= r2 {
                    hit = true;
                    return;
                }
            }
        });
        hit
    }
}

#[cfg(test)]
#[path = "hull_tests.rs"]
mod tests;
