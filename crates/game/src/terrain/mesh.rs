//! Terrain chunk geometry on a cube-sphere quadtree (pure CPU code, built
//! on background tasks). Positions are f64 in the body-fixed frame until the
//! final conversion to f32 offsets from the chunk's own centre, so chunk
//! meshes stay precise at any level (the floating-origin rule).

use glam::DVec3;
use std::sync::Arc;

/// Vertices per chunk side (32 × 32 quads).
pub const GRID: usize = 33;

/// Terrain this far below sea level (m) is shaded as water.
const WATER_DEPTH: f64 = 5.0;

/// Height above the reference ellipsoid (m) at a body-fixed unit direction.
pub type HeightFn = Arc<dyn Fn(DVec3) -> f64 + Send + Sync>;

/// A quadtree node: cube face, level and position within the face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkKey {
    pub face: u8,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl ChunkKey {
    pub fn roots() -> [ChunkKey; 6] {
        std::array::from_fn(|f| ChunkKey { face: f as u8, level: 0, x: 0, y: 0 })
    }

    pub fn children(self) -> [ChunkKey; 4] {
        let (l, x, y) = (self.level + 1, self.x * 2, self.y * 2);
        [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| ChunkKey { face: self.face, level: l, x: x + dx, y: y + dy })
    }

    /// Face coordinates (a, b) ∈ [-1, 1]² of grid point (i, j) ∈ [0, 1]².
    pub fn face_coords(self, i: f64, j: f64) -> (f64, f64) {
        let n = f64::from(1u32 << self.level);
        (-1.0 + 2.0 * (f64::from(self.x) + i) / n, -1.0 + 2.0 * (f64::from(self.y) + j) / n)
    }
}

/// Face normal and in-face axes for each of the six cube faces.
fn face_axes(face: u8) -> (DVec3, DVec3, DVec3) {
    match face {
        0 => (DVec3::X, DVec3::Y, DVec3::Z),
        1 => (-DVec3::X, -DVec3::Y, DVec3::Z),
        2 => (DVec3::Y, -DVec3::X, DVec3::Z),
        3 => (-DVec3::Y, DVec3::X, DVec3::Z),
        4 => (DVec3::Z, DVec3::Y, -DVec3::X),
        _ => (-DVec3::Z, DVec3::Y, DVec3::X),
    }
}

/// Unit direction for face coordinates, with the tangent warp that makes
/// cells nearly equal in area.
pub fn direction(face: u8, a: f64, b: f64) -> DVec3 {
    let (n, u, v) = face_axes(face);
    let q = std::f64::consts::FRAC_PI_4;
    (n + u * (a * q).tan() + v * (b * q).tan()).normalize()
}

/// The body's reference shape and surface rules.
#[derive(Clone)]
pub struct Shape {
    pub radius_eq: f64,
    pub radius_polar: f64,
    /// Heights (None: smooth ellipsoid).
    pub height: Option<HeightFn>,
    /// The solid surface is at least sea level (D037).
    pub sea_level: Option<f64>,
    /// Detail-coordinate unit (m); see `ChunkData::detail`.
    pub detail_scale: f64,
}

impl Shape {
    /// Geocentric radius of the ellipsoid along a unit direction.
    pub fn ellipsoid_radius(&self, dir: DVec3) -> f64 {
        let (a, b) = (self.radius_eq, self.radius_polar);
        let s = dir.z;
        (a * b) / (b * b * (1.0 - s * s) + a * a * s * s).sqrt()
    }

    /// Surface height (m) and whether it is water.
    pub fn surface_height(&self, dir: DVec3) -> (f64, bool) {
        let h = self.height.as_ref().map_or(0.0, |f| f(dir));
        match self.sea_level {
            // Shallow basins and coastal land sampled just below sea level
            // are raised to it but shaded as land.
            Some(sea) if h < sea => (sea, h < sea - WATER_DEPTH),
            _ => (h, false),
        }
    }

    pub fn surface_point(&self, dir: DVec3) -> (DVec3, f64, bool) {
        let (h, water) = self.surface_height(dir);
        (dir * (self.ellipsoid_radius(dir) + h), h, water)
    }
}

/// Mesh data for one chunk, ready to upload.
pub struct ChunkData {
    /// Chunk centre on the ellipsoid, body-fixed (m).
    pub center: DVec3,
    /// Bounding radius about `center` (m).
    pub radius: f64,
    /// Largest height deviation from the ellipsoid in this chunk (m).
    pub max_height: f64,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Detail-layer coordinates: face-local metres / `detail_scale`,
    /// wrapped to [0, 256) so they stay precise in f32 (the shader's noise
    /// is periodic with that period).
    pub detail: Vec<[f32; 2]>,
    /// r: water mask, g: height / 10 km.
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

/// Builds a chunk: a (GRID+2)² sample grid (one ring beyond the edge for
/// normals), the GRID² surface vertices, and a skirt around the edge that
/// hangs down to hide cracks between levels.
pub fn build(key: ChunkKey, shape: &Shape) -> ChunkData {
    let g = GRID;
    let s = g + 2;
    let step = 1.0 / (g - 1) as f64;
    let (ca, cb) = key.face_coords(0.5, 0.5);
    let cdir = direction(key.face, ca, cb);
    let center = cdir * shape.ellipsoid_radius(cdir);
    let mut pts = Vec::with_capacity(s * s);
    let mut info = Vec::with_capacity(s * s);
    for j in 0..s {
        for i in 0..s {
            let (a, b) = key.face_coords((i as f64 - 1.0) * step, (j as f64 - 1.0) * step);
            let dir = direction(key.face, a, b);
            let (p, h, water) = shape.surface_point(dir);
            pts.push(p);
            info.push((a, b, h, water, dir));
        }
    }
    let at = |i: usize, j: usize| pts[j * s + i];
    let face_m = shape.radius_eq * std::f64::consts::FRAC_PI_4;
    let wrap = |x: f64| (x / shape.detail_scale).rem_euclid(256.0) as f32;
    let mut out = ChunkData {
        center,
        radius: 0.0,
        max_height: 0.0,
        positions: Vec::with_capacity(g * g + 4 * g),
        normals: Vec::with_capacity(g * g + 4 * g),
        detail: Vec::with_capacity(g * g + 4 * g),
        colors: Vec::with_capacity(g * g + 4 * g),
        indices: Vec::with_capacity((g - 1) * (g - 1) * 6 + 4 * (g - 1) * 6),
    };
    for j in 1..=g {
        for i in 1..=g {
            let p = at(i, j);
            let (a, b, h, water, dir) = info[j * s + i];
            let n = (at(i + 1, j) - at(i - 1, j)).cross(at(i, j + 1) - at(i, j - 1)).normalize_or(dir);
            let n = if n.dot(dir) < 0.0 { -n } else { n };
            out.radius = out.radius.max((p - center).length());
            out.max_height = out.max_height.max(h.abs());
            out.positions.push((p - center).as_vec3().to_array());
            out.normals.push(n.as_vec3().to_array());
            out.detail.push([wrap(a * face_m), wrap(b * face_m)]);
            out.colors.push([f32::from(u8::from(water)), (h / 10_000.0) as f32, 0.0, 1.0]);
        }
    }
    for j in 0..g - 1 {
        for i in 0..g - 1 {
            let a = (j * g + i) as u32;
            let (b, c, d) = (a + 1, a + g as u32, a + g as u32 + 1);
            out.indices.extend_from_slice(&[a, b, d, a, d, c]);
        }
    }
    add_skirt(&mut out, key, shape);
    out
}

/// Skirt: a copy of each edge vertex pushed down along the local up, joined
/// to the edge by a strip facing outwards.
fn add_skirt(out: &mut ChunkData, key: ChunkKey, shape: &Shape) {
    let g = GRID;
    let edge_len = 2.0 * shape.radius_eq * std::f64::consts::FRAC_PI_4 / f64::from(1u32 << key.level);
    let depth = (edge_len / (g - 1) as f64 * 2.0).max(50.0) + out.max_height * 0.05;
    let ring: Vec<usize> = (0..g - 1)
        .chain((0..g - 1).map(|j| j * g + g - 1))
        .chain((0..g - 1).map(|i| (g - 1) * g + g - 1 - i))
        .chain((0..g - 1).map(|j| (g - 1 - j) * g))
        .collect();
    let base = out.positions.len() as u32;
    for &v in &ring {
        let p = DVec3::from_array(out.positions[v].map(f64::from)) + out.center;
        let down = p.normalize() * depth;
        out.positions.push((p - down - out.center).as_vec3().to_array());
        out.normals.push(out.normals[v]);
        out.detail.push(out.detail[v]);
        out.colors.push(out.colors[v]);
    }
    let n = ring.len() as u32;
    for k in 0..n {
        let (a, b) = (ring[k as usize] as u32, ring[((k + 1) % n) as usize] as u32);
        let (c, d) = (base + k, base + (k + 1) % n);
        out.indices.extend_from_slice(&[a, d, b, a, c, d]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere() -> Shape {
        Shape { radius_eq: 1000.0, radius_polar: 1000.0, height: None, sea_level: None, detail_scale: 1.0 }
    }

    #[test]
    fn faces_cover_the_sphere_and_meet_at_edges() {
        // Neighbouring faces share their edge directions exactly.
        for f in 0..6u8 {
            let d = direction(f, 0.0, 0.0);
            assert!((d.length() - 1.0).abs() < 1e-12);
        }
        let e = direction(0, 1.0, 0.3);
        let found = (0..6u8).filter(|&f| f != 0).any(|f| {
            [(-1.0, 0.3), (1.0, 0.3), (0.3, -1.0), (0.3, 1.0), (-1.0, -0.3), (1.0, -0.3), (-0.3, 1.0), (-0.3, -1.0)]
                .iter()
                .any(|&(a, b)| (direction(f, a, b) - e).length() < 1e-12)
        });
        assert!(found);
    }

    #[test]
    fn triangles_face_outwards() {
        let c = build(ChunkKey { face: 3, level: 2, x: 1, y: 2 }, &sphere());
        let p = |i: u32| DVec3::from_array(c.positions[i as usize].map(f64::from)) + c.center;
        let surface = (GRID - 1) * (GRID - 1) * 2;
        for t in c.indices.chunks(3).take(surface) {
            let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            assert!(n.dot(p(t[0])) > 0.0);
        }
        // Skirt triangles face away from the chunk centre.
        let mid = p(((GRID / 2) * GRID + GRID / 2) as u32);
        for t in c.indices.chunks(3).skip(surface) {
            let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            let out = (p(t[0]) + p(t[1]) + p(t[2])) / 3.0 - mid;
            assert!(n.dot(out) > 0.0);
        }
    }

    #[test]
    fn ocean_is_solid_at_sea_level() {
        let shape = Shape { sea_level: Some(0.0), height: Some(Arc::new(|_| -4000.0)), ..sphere() };
        let (h, water) = shape.surface_height(DVec3::X);
        assert_eq!((h, water), (0.0, true));
    }
}
