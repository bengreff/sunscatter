//! The per-design aerodynamic bake (docs/design/aero-thermal.md, "The
//! bake"): for every direction of a geodesic grid, which part of each cell
//! the flow reaches first (its exposure), and the effective nose radius.
//!
//! The force sums ([`DirSums`]) are formed at run time over the cells, with
//! the exposure interpolated between the three grid directions around the
//! flow and the incidence from the actual flow direction (~1 µs for 512
//! cells). Interpolating baked sums instead was measured 2 % off on a cone
//! at 5° incidence (the Newtonian sums are quadratic in the direction); with
//! exposure interpolation the only interpolated quantity is the shadowing.
//!
//! Exposure is found with a small z-buffer: the surface triangles facing the
//! flow are rasterised onto a square grid orthogonal to it (pixel centres
//! sampled, depth interpolated), each pixel keeping the nearest cell. A
//! cell's exposure is the fraction of the pixels its own triangles cover
//! that it wins; both counts come from the same raster, so on a convex
//! body every front-facing cell is exposed exactly (1). Cells too small to
//! cover a pixel centre are judged by the pixel under their centroid.

use super::geodesic::Geodesic;
use crate::craft::mesh::basis;
use crate::craft::{Cell, Cells, Surface};
use glam::DVec3;

/// Bake resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeOptions {
    /// Geodesic subdivision level (3: 642 directions).
    pub level: u32,
    /// Raster size in pixels per side.
    pub raster: usize,
}

impl Default for BakeOptions {
    fn default() -> Self {
        BakeOptions { level: 3, raster: 64 }
    }
}

/// The sums for one flow direction **d** (body axes; moments about the
/// craft origin). With w = A·f (area × exposure) and s = sin θ = −n·d over
/// exposed cells (s > 0):
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DirSums {
    /// Newtonian force per unit q·Cp,max: Σ w s² (−n) (m²).
    pub newton: DVec3,
    /// Its moment: Σ w s² c × (−n) (m³).
    pub newton_moment: DVec3,
    /// Σ w s³ c (m³): the drag part of the Newtonian moment is this × d.
    pub drag_lever: DVec3,
    /// Projected area Σ w s (m²).
    pub area: f64,
    /// Σ w s c (m³): the projected area's first moment (free-molecular
    /// moment = 2·this × d; its centroid is this / area).
    pub area_moment: DVec3,
    /// Effective nose radius for stagnation heating (m).
    pub nose_radius: f64,
}

/// What the aerodynamics needs of a cell (body axes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellGeometry {
    pub centroid: DVec3,
    pub normal: DVec3,
    pub area: f64,
}

impl CellGeometry {
    pub fn of(c: &Cell) -> Self {
        CellGeometry { centroid: c.centroid, normal: c.normal, area: c.area }
    }
}

/// Everything baked for one craft design (shared by its vessels).
#[derive(Clone, Debug, PartialEq)]
pub struct AeroBake {
    pub grid: Geodesic,
    pub geometry: Vec<CellGeometry>,
    /// Exposure per direction per cell, 0..=255 for 0..=1 (direction-major).
    pub exposure: Vec<u8>,
    /// Effective nose radius per direction (m).
    pub nose_radius: Vec<f64>,
    pub cells: usize,
    /// Reference length for the Knudsen number: the bounding-sphere diameter (m).
    pub length: f64,
}

/// Per-direction working buffers.
struct Raster {
    n: usize,
    depth: Vec<f64>,
    owner: Vec<u32>,
    stamp: Vec<u32>,
    covered: Vec<u32>,
}

const NONE: u32 = u32::MAX;

/// Stagnation-region cells for the nose radius: incidence sin θ at least this.
const NOSE_MIN_SIN: f64 = 0.7;

/// Bakes a craft design from its surface and cells.
pub fn bake(surface: &Surface, cells: &Cells, opts: &BakeOptions) -> AeroBake {
    let grid = Geodesic::new(opts.level);
    let nc = cells.cells.len();
    // Bounding sphere (box centre; not minimal, just fixed).
    let (lo, hi) = surface
        .positions
        .iter()
        .fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let centre = (lo + hi) * 0.5;
    let radius = surface.positions.iter().map(|p| (*p - centre).length()).fold(0.0, f64::max).max(1e-6);
    let tri_normal: Vec<DVec3> = (0..surface.triangles.len()).map(|t| surface.triangle(t).1).collect();
    // Triangles grouped by cell (so each cell's covered pixels are counted once).
    let mut order: Vec<u32> = (0..surface.triangles.len() as u32).collect();
    order.sort_by_key(|&t| (cells.tri_cell[t as usize], t));
    let n = opts.raster;
    let mut r =
        Raster { n, depth: vec![0.0; n * n], owner: vec![NONE; n * n], stamp: vec![NONE; n * n], covered: vec![0; nc] };
    let geometry: Vec<CellGeometry> = cells.cells.iter().map(CellGeometry::of).collect();
    let mut nose_radius = Vec::with_capacity(grid.dirs.len());
    let mut exposure = Vec::with_capacity(grid.dirs.len() * nc);
    let mut proj = vec![DVec3::ZERO; surface.positions.len()];
    let mut won = vec![0u32; nc];
    let mut f = vec![0.0; nc];
    for &d in &grid.dirs {
        let (u, v, _) = basis(d);
        let scale = n as f64 / (2.0 * radius);
        for (p, q) in surface.positions.iter().zip(proj.iter_mut()) {
            let x = *p - centre;
            *q = DVec3::new((x.dot(u) + radius) * scale, (x.dot(v) + radius) * scale, x.dot(d));
        }
        r.depth.fill(f64::INFINITY);
        r.owner.fill(NONE);
        r.stamp.fill(NONE);
        r.covered.fill(0);
        for &t in &order {
            if tri_normal[t as usize].dot(d) >= 0.0 {
                continue;
            }
            let tri = surface.triangles[t as usize].map(|i| proj[i as usize]);
            r.fill(tri, cells.tri_cell[t as usize]);
        }
        won.fill(0);
        for &o in &r.owner {
            if o != NONE {
                won[o as usize] += 1;
            }
        }
        let pixel = 2.0 * radius / n as f64;
        for (i, c) in cells.cells.iter().enumerate() {
            f[i] = if c.normal.dot(d) >= 0.0 {
                0.0
            } else if r.covered[i] > 0 {
                (f64::from(won[i]) / f64::from(r.covered[i])).min(1.0)
            } else {
                let x = c.centroid - centre;
                let px = ((x.dot(u) + radius) * scale).floor().clamp(0.0, (n - 1) as f64) as usize;
                let py = ((x.dot(v) + radius) * scale).floor().clamp(0.0, (n - 1) as f64) as usize;
                let front = r.depth[py * n + px];
                if front < x.dot(d) - 2.0 * pixel {
                    0.0
                } else {
                    1.0
                }
            };
            exposure.push((f[i] * 255.0).round() as u8);
        }
        // From the quantised exposure, as the run time sees it.
        let fq = |i: usize| f64::from(exposure[exposure.len() - nc + i]) / 255.0;
        let area = direction_sums(&geometry, d, fq).area;
        nose_radius.push(nose(&geometry, d, fq, area));
    }
    AeroBake { grid, geometry, exposure, nose_radius, cells: nc, length: 2.0 * radius }
}

/// The sums for direction `d` given each cell's exposure (no nose radius).
pub fn direction_sums(cells: &[CellGeometry], d: DVec3, f: impl Fn(usize) -> f64) -> DirSums {
    let mut s = DirSums::default();
    for (i, c) in cells.iter().enumerate() {
        let sin = -c.normal.dot(d);
        if sin <= 0.0 {
            continue;
        }
        let w = c.area * f(i);
        if w <= 0.0 {
            continue;
        }
        s.newton -= c.normal * (w * sin * sin);
        s.newton_moment -= c.centroid.cross(c.normal) * (w * sin * sin);
        s.drag_lever += c.centroid * (w * sin * sin * sin);
        s.area += w * sin;
        s.area_moment += c.centroid * (w * sin);
    }
    s
}

/// Effective nose radius for direction `d`: the regression r = r₀ + Rn·t of
/// the lateral offsets r on the lateral normal components t over the
/// stagnation region (exposed cells with sin θ ≥ 0.7; exact for a sphere,
/// c = O + Rn·n). Bounded by twice the radius of the projected area's disc
/// (a flat face). Without a smooth stagnation region (an edge or a point
/// facing the flow), the size of the best-facing exposed cell.
fn nose(cells: &[CellGeometry], d: DVec3, f: impl Fn(usize) -> f64, area: f64) -> f64 {
    let (mut sw, mut sr, mut st) = (0.0, DVec3::ZERO, DVec3::ZERO);
    let mut stag: Vec<(f64, DVec3, DVec3)> = Vec::new();
    for (i, c) in cells.iter().enumerate() {
        let (sin, w) = (-c.normal.dot(d), c.area * f(i));
        if sin >= NOSE_MIN_SIN && f(i) >= 0.5 {
            let r = c.centroid - d * c.centroid.dot(d);
            let t = c.normal - d * c.normal.dot(d);
            sw += w;
            sr += r * w;
            st += t * w;
            stag.push((w, r, t));
        }
    }
    let cap = 2.0 * (area / crate::math::PI).max(0.0).sqrt();
    if stag.len() >= 3 && sw > 0.0 {
        let (rm, tm) = (sr / sw, st / sw);
        let (num, den) = stag.iter().fold((0.0, 0.0), |(num, den), &(w, r, t)| {
            let (dr, dt) = (r - rm, t - tm);
            (num + w * dr.dot(dt), den + w * dt.dot(dt))
        });
        if den > 1e-12 && num > 0.0 {
            (num / den).min(cap)
        } else {
            cap
        }
    } else {
        let best = cells
            .iter()
            .enumerate()
            .filter(|(i, _)| f(*i) > 0.0)
            .map(|(_, c)| (-c.normal.dot(d), c.area))
            .fold((f64::MIN, 0.0), |b, x| if x.0 > b.0 { x } else { b });
        (best.1 / crate::math::PI).max(0.0).sqrt().min(cap)
    }
}

impl Raster {
    /// Rasterises one projected triangle (x, y in pixels, z depth) of `cell`.
    fn fill(&mut self, p: [DVec3; 3], cell: u32) {
        let n = self.n as f64;
        let x0 = (p[0].x.min(p[1].x).min(p[2].x) - 0.5).ceil().max(0.0);
        let x1 = (p[0].x.max(p[1].x).max(p[2].x) - 0.5).floor().min(n - 1.0);
        let y0 = (p[0].y.min(p[1].y).min(p[2].y) - 0.5).ceil().max(0.0);
        let y1 = (p[0].y.max(p[1].y).max(p[2].y) - 0.5).floor().min(n - 1.0);
        if x0 > x1 || y0 > y1 {
            return;
        }
        let edge = |a: DVec3, b: DVec3, x: f64, y: f64| (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x);
        let area = edge(p[0], p[1], p[2].x, p[2].y);
        if area == 0.0 {
            return;
        }
        for py in y0 as usize..=y1 as usize {
            let y = py as f64 + 0.5;
            for px in x0 as usize..=x1 as usize {
                let x = px as f64 + 0.5;
                let w0 = edge(p[1], p[2], x, y) / area;
                let w1 = edge(p[2], p[0], x, y) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let k = py * self.n + px;
                if self.stamp[k] != cell {
                    self.stamp[k] = cell;
                    self.covered[cell as usize] += 1;
                }
                let z = w0 * p[0].z + w1 * p[1].z + w2 * p[2].z;
                if z < self.depth[k] {
                    self.depth[k] = z;
                    self.owner[k] = cell;
                }
            }
        }
    }
}

impl AeroBake {
    /// FNV-1a hash of everything baked (determinism checks).
    pub fn hash(&self) -> u64 {
        let mut bytes = Vec::new();
        let mut put = |x: f64| bytes.extend_from_slice(&x.to_bits().to_le_bytes());
        for d in &self.grid.dirs {
            d.to_array().into_iter().for_each(&mut put);
        }
        for g in &self.geometry {
            for v in [g.centroid, g.normal] {
                v.to_array().into_iter().for_each(&mut put);
            }
            put(g.area);
        }
        self.nose_radius.iter().for_each(|&r| put(r));
        put(self.length);
        bytes.extend_from_slice(&self.exposure);
        crate::ephem::fnv1a64(&bytes)
    }

    /// The sums at flow direction `d` (unit, body axes): over the cells,
    /// with the exposure interpolated barycentrically between the three
    /// grid directions around `d`; the nose radius interpolated likewise.
    pub fn sums_at(&self, d: DVec3) -> DirSums {
        let w = self.grid.locate(d);
        let rows = w.dirs.map(|k| &self.exposure[k as usize * self.cells..][..self.cells]);
        let f = |i: usize| {
            (w.w[0] * f64::from(rows[0][i]) + w.w[1] * f64::from(rows[1][i]) + w.w[2] * f64::from(rows[2][i])) / 255.0
        };
        let mut s = direction_sums(&self.geometry, d, f);
        s.nose_radius = (0..3).map(|k| w.w[k] * self.nose_radius[w.dirs[k] as usize]).sum();
        s
    }

    /// Each cell's exposure (0..1) at flow direction `d`, interpolated like
    /// [`Self::sums_at`]. `out` has one entry per cell.
    pub fn exposure_at(&self, d: DVec3, out: &mut [f64]) {
        let w = self.grid.locate(d);
        out.fill(0.0);
        for k in 0..3 {
            let row = &self.exposure[w.dirs[k] as usize * self.cells..][..self.cells];
            for (o, &e) in out.iter_mut().zip(row) {
                *o += w.w[k] * f64::from(e) / 255.0;
            }
        }
    }
}
