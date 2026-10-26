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
//! flow are rasterised onto a square grid orthogonal to it, spanning the
//! craft's projection (pixel centres
//! sampled, depth interpolated), each pixel keeping the nearest cell. A
//! cell's exposure is the fraction of the pixels its own triangles cover
//! that it wins; both counts come from the same raster, so on a convex
//! body every front-facing cell is exposed exactly (1). Cells too small to
//! cover a pixel centre are judged by the pixel under their centroid.

use super::geodesic::Geodesic;
use super::slender::{BodySections, Fin, WING_RATIO};
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
    /// Over the bases, the cells turned downstream more steeply than 45°
    /// (separated; [`super::base`]): Σ A·n (m², its component along the
    /// flow the projected base area) and its moment Σ c × A·n (m³): a
    /// uniform base suction pulls along them.
    pub base: DVec3,
    pub base_moment: DVec3,
}

/// What the aerodynamics needs of a cell (body axes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellGeometry {
    pub centroid: DVec3,
    pub normal: DVec3,
    pub area: f64,
    /// Second moment of the cell's area about its centroid (m⁴): xx, yy,
    /// zz, xy, xz, yz (from its triangles; zero from [`Self::of`]).
    pub spread: [f64; 6],
}

impl CellGeometry {
    pub fn of(c: &Cell) -> Self {
        CellGeometry { centroid: c.centroid, normal: c.normal, area: c.area, spread: [0.0; 6] }
    }

    /// Half the cell's extent along the unit direction `d`: √(3·σ²) with σ²
    /// the area's variance along `d` (exact for a rectangle).
    pub fn half_extent(&self, d: DVec3) -> f64 {
        let [xx, yy, zz, xy, xz, yz] = self.spread;
        let m =
            xx * d.x * d.x + yy * d.y * d.y + zz * d.z * d.z + 2.0 * (xy * d.x * d.y + xz * d.x * d.z + yz * d.y * d.z);
        if self.area > 0.0 {
            (3.0 * m.max(0.0) / self.area).sqrt()
        } else {
            0.0
        }
    }
}

/// Each cell's second moment of area about its centroid, from the surface
/// triangles (a triangle's own: A/12·Σ dᵢdᵢᵀ over its vertices' offsets
/// from its centroid, plus A·ccᵀ for its offset c from the cell's).
fn spreads(surface: &Surface, cells: &Cells, geometry: &mut [CellGeometry]) {
    let add = |m: &mut [f64; 6], v: DVec3, w: f64| {
        m[0] += w * v.x * v.x;
        m[1] += w * v.y * v.y;
        m[2] += w * v.z * v.z;
        m[3] += w * v.x * v.y;
        m[4] += w * v.x * v.z;
        m[5] += w * v.y * v.z;
    };
    for (t, tri) in surface.triangles.iter().enumerate() {
        let (c, _, a) = surface.triangle(t);
        let g = &mut geometry[cells.tri_cell[t] as usize];
        for &i in tri {
            add(&mut g.spread, surface.positions[i as usize] - c, a / 12.0);
        }
        add(&mut g.spread, c - g.centroid, a);
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
    /// Reference length for the Knudsen number (m): the maximum body
    /// diameter, as Wilmoth's bridging was fitted (Kn = λ/D_b; Wilmoth,
    /// Mitcheltree & Moss, AIAA 97-2510, nomenclature), generalised as the
    /// diameter of the disc of the craft's largest projected area over the
    /// grid's directions (a capsule's heat-shield diameter; a slender
    /// body's broadside silhouette).
    pub length: f64,
    /// The craft's extent along each grid direction (m).
    pub extent: Vec<f64>,
    /// Wave drag per unit q (m², uncapped) along each grid direction, from
    /// the area distribution along it ([`super::wave`]).
    pub wave: Vec<f64>,
    /// Cross-section areas along the long axis (slender-body lift).
    pub sections: BodySections,
    /// Thin parts (fin lift).
    pub fins: Vec<Fin>,
    /// The boundary layer's 3D relief along each grid direction: x_eff/x
    /// (laminar, turbulent) at [`super::relief::STATIONS`] slabs
    /// (direction-major).
    pub relief: Vec<(f32, f32)>,
}

/// The exposures of the three grid directions around a flow direction and
/// their barycentric weights.
pub struct Exposure<'a> {
    rows: [&'a [u8]; 3],
    w: [f64; 3],
}

impl Exposure<'_> {
    /// Cell `i`'s exposure (0..1).
    pub fn at(&self, i: usize) -> f64 {
        let (r, w) = (&self.rows, &self.w);
        (w[0] * f64::from(r[0][i]) + w[1] * f64::from(r[1][i]) + w[2] * f64::from(r[2][i])) / 255.0
    }
}

/// The relief rows of the three grid directions around a flow direction,
/// their weights and the craft's extent along it.
pub struct Relief<'a> {
    rows: [&'a [(f32, f32)]; 3],
    w: [f64; 3],
    extent: f64,
}

impl Relief<'_> {
    /// x_eff/x (laminar, turbulent) at run length `x` (m), linear between
    /// the slabs' middles.
    pub fn at(&self, x: f64) -> (f64, f64) {
        let n = super::relief::STATIONS;
        let u = (x / self.extent.max(1e-9) * n as f64 - 0.5).clamp(0.0, (n - 1) as f64);
        let (k, f) = ((u.floor() as usize).min(n - 2), u - u.floor().min((n - 2) as f64));
        let (mut lam, mut turb) = (0.0, 0.0);
        for (row, w) in self.rows.iter().zip(self.w) {
            let (a, b) = (row[k], row[k + 1]);
            lam += w * (f64::from(a.0) * (1.0 - f) + f64::from(b.0) * f);
            turb += w * (f64::from(a.1) * (1.0 - f) + f64::from(b.1) * f);
        }
        (lam, turb)
    }
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

/// What every direction of the bake reads.
struct Shared<'a> {
    surface: &'a Surface,
    cells: &'a Cells,
    geometry: &'a [CellGeometry],
    /// Each cell's vector area Σ A·n (m²).
    vector: &'a [DVec3],
    tri_normal: &'a [DVec3],
    /// Triangles grouped by cell (so each cell's covered pixels are counted once).
    order: &'a [u32],
    centre: DVec3,
    raster: usize,
}

/// One direction's bake.
struct Direction {
    exposure: Vec<u8>,
    nose_radius: f64,
    extent: f64,
    wave: f64,
    relief: [(f32, f32); super::relief::STATIONS],
    /// Projected area (for the Knudsen length).
    area: f64,
}

/// A thread's working buffers.
struct Scratch {
    raster: Raster,
    proj: Vec<DVec3>,
    won: Vec<u32>,
    f: Vec<f64>,
}

impl Scratch {
    fn new(shared: &Shared) -> Self {
        let (n, nc) = (shared.raster, shared.cells.cells.len());
        Scratch {
            raster: Raster {
                n,
                depth: vec![0.0; n * n],
                owner: vec![NONE; n * n],
                stamp: vec![NONE; n * n],
                covered: vec![0; nc],
            },
            proj: vec![DVec3::ZERO; shared.surface.positions.len()],
            won: vec![0; nc],
            f: vec![0.0; nc],
        }
    }
}

/// Bakes a craft design from its surface and cells. The directions are
/// independent: they are baked on all available cores, each into its own
/// slot (the same bits on any number of threads).
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
    let mut order: Vec<u32> = (0..surface.triangles.len() as u32).collect();
    order.sort_by_key(|&t| (cells.tri_cell[t as usize], t));
    let mut geometry: Vec<CellGeometry> = cells.cells.iter().map(CellGeometry::of).collect();
    spreads(surface, cells, &mut geometry);
    let tris: Vec<(DVec3, DVec3, f64)> = (0..surface.triangles.len()).map(|t| surface.triangle(t)).collect();
    let sections = BodySections::new(&tris, &surface.positions);
    let mut vector = vec![DVec3::ZERO; nc];
    for (t, &(_, n, a)) in tris.iter().enumerate() {
        vector[cells.tri_cell[t] as usize] += n * a;
    }
    let mut fins = super::fin_search::find_fins(surface, cells, &geometry, WING_RATIO * 2.0 * radius);
    super::slender::attach(&mut fins, &geometry, &sections);
    let shared = Shared {
        surface,
        cells,
        geometry: &geometry,
        vector: &vector,
        tri_normal: &tri_normal,
        order: &order,
        centre,
        raster: opts.raster,
    };
    let dirs = &grid.dirs;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 16).min(dirs.len().max(1));
    let chunk = dirs.len().div_ceil(threads);
    let mut baked: Vec<Option<Direction>> = Vec::with_capacity(dirs.len());
    baked.resize_with(dirs.len(), || None);
    std::thread::scope(|s| {
        for (block, slots) in dirs.chunks(chunk.max(1)).zip(baked.chunks_mut(chunk.max(1))) {
            let shared = &shared;
            s.spawn(move || {
                let mut scratch = Scratch::new(shared);
                for (&d, slot) in block.iter().zip(slots) {
                    *slot = Some(direction(shared, &mut scratch, d));
                }
            });
        }
    });
    let mut exposure = Vec::with_capacity(dirs.len() * nc);
    let (mut nose_radius, mut extent, mut wave) = (Vec::new(), Vec::new(), Vec::new());
    let mut relief = Vec::with_capacity(dirs.len() * super::relief::STATIONS);
    let mut max_area: f64 = 0.0;
    for b in baked.into_iter().map(|b| b.expect("every direction baked")) {
        exposure.extend_from_slice(&b.exposure);
        nose_radius.push(b.nose_radius);
        extent.push(b.extent);
        wave.push(b.wave);
        relief.extend(b.relief);
        max_area = f64::max(max_area, b.area);
    }
    let length = if max_area > 0.0 { 2.0 * (max_area / crate::math::PI).sqrt() } else { 2.0 * radius };
    AeroBake { grid, geometry, exposure, nose_radius, cells: nc, length, extent, wave, sections, fins, relief }
}

/// Bakes one direction `d`.
fn direction(shared: &Shared, scratch: &mut Scratch, d: DVec3) -> Direction {
    let (surface, cells, centre, n) = (shared.surface, shared.cells, shared.centre, shared.raster);
    let nc = cells.cells.len();
    let Scratch { raster: r, proj, won, f } = scratch;
    let (u, v, _) = basis(d);
    // The raster spans the craft's projection (square pixels), so a
    // slender body seen end-on is as well resolved as seen broadside.
    let (mut u0, mut u1, mut v0, mut v1) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for (p, q) in surface.positions.iter().zip(proj.iter_mut()) {
        let x = *p - centre;
        *q = DVec3::new(x.dot(u), x.dot(v), x.dot(d));
        (u0, u1) = (u0.min(q.x), u1.max(q.x));
        (v0, v1) = (v0.min(q.y), v1.max(q.y));
        (lo, hi) = (lo.min(q.z), hi.max(q.z));
    }
    let span = (u1 - u0).max(v1 - v0).max(1e-9) * (1.0 + 1e-9);
    let scale = n as f64 / span;
    for q in proj.iter_mut() {
        (q.x, q.y) = ((q.x - u0) * scale, (q.y - v0) * scale);
    }
    // The area distribution along d, and the perimeter's, from the cells.
    let areas = super::wave::area_along_cells(shared.geometry, shared.vector, centre, d, lo, hi);
    let wave = super::wave::wave_drag_area(&areas, (hi - lo) / super::wave::STATIONS as f64);
    let relief = super::relief::ratios_cells(shared.geometry, shared.vector, centre, d, lo, hi);
    r.depth.fill(f64::INFINITY);
    r.owner.fill(NONE);
    r.stamp.fill(NONE);
    r.covered.fill(0);
    for &t in shared.order {
        if shared.tri_normal[t as usize].dot(d) >= 0.0 {
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
    let pixel = span / n as f64;
    let mut exposure = Vec::with_capacity(nc);
    for (i, c) in cells.cells.iter().enumerate() {
        f[i] = if c.normal.dot(d) >= 0.0 {
            0.0
        } else if r.covered[i] > 0 {
            (f64::from(won[i]) / f64::from(r.covered[i])).min(1.0)
        } else {
            let x = c.centroid - centre;
            let px = ((x.dot(u) - u0) * scale).floor().clamp(0.0, (n - 1) as f64) as usize;
            let py = ((x.dot(v) - v0) * scale).floor().clamp(0.0, (n - 1) as f64) as usize;
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
    let fq = |i: usize| f64::from(exposure[i]) / 255.0;
    let area = direction_sums(shared.geometry, d, fq).area;
    let nose_radius = nose(shared.geometry, d, fq, area);
    Direction { exposure, nose_radius, extent: hi - lo, wave, relief, area }
}

/// The sums for direction `d` given each cell's exposure (no nose radius).
pub fn direction_sums(cells: &[CellGeometry], d: DVec3, f: impl Fn(usize) -> f64) -> DirSums {
    let mut s = DirSums::default();
    for (i, c) in cells.iter().enumerate() {
        let sin = -c.normal.dot(d);
        if -sin > super::friction::SEPARATED {
            s.base += c.normal * c.area;
            s.base_moment += c.centroid.cross(c.normal * c.area);
        }
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
/// c = O + Rn·n) at the front: within half the projected disc's diameter
/// of the most upstream such cell (a sphere's region is 0.3·Rn deep), so
/// faces far downstream (a rocket's fin tops) are not part of the nose.
/// A face flatter than a hemisphere over the craft's projected disc takes
/// Zoby & Sullivan's effective radius ([`effective_nose_radius`]).
/// Without a smooth stagnation region (an edge or a point facing the flow),
/// the size of the best-facing exposed cell.
fn nose(cells: &[CellGeometry], d: DVec3, f: impl Fn(usize) -> f64, area: f64) -> f64 {
    let cap = 2.0 * (area / crate::math::PI).max(0.0).sqrt();
    let facing = |i: usize, c: &CellGeometry| -c.normal.dot(d) >= NOSE_MIN_SIN && f(i) >= 0.5;
    let front = cells
        .iter()
        .enumerate()
        .filter(|&(i, c)| facing(i, c))
        .map(|(_, c)| -c.centroid.dot(d))
        .fold(f64::NEG_INFINITY, f64::max);
    let (mut sw, mut sr, mut st) = (0.0, DVec3::ZERO, DVec3::ZERO);
    let mut stag: Vec<(f64, DVec3, DVec3)> = Vec::new();
    for (i, c) in cells.iter().enumerate() {
        let w = c.area * f(i);
        if facing(i, c) && -c.centroid.dot(d) >= front - 0.5 * cap {
            let r = c.centroid - d * c.centroid.dot(d);
            let t = c.normal - d * c.normal.dot(d);
            sw += w;
            sr += r * w;
            st += t * w;
            stag.push((w, r, t));
        }
    }
    if stag.len() >= 3 && sw > 0.0 {
        let (rm, tm) = (sr / sw, st / sw);
        let (num, den) = stag.iter().fold((0.0, 0.0), |(num, den), &(w, r, t)| {
            let (dr, dt) = (r - rm, t - tm);
            (num + w * dr.dot(dt), den + w * dt.dot(dt))
        });
        let r_n = if den > 1e-12 && num > 0.0 { num / den } else { f64::INFINITY };
        effective_nose_radius(0.5 * cap, r_n)
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

/// r_B/r_eff against r_B/r_N for spherical segments of base radius r_B
/// and sphere radius r_N with a sharp corner (a flat face at 0): the
/// stagnation-point velocity gradient relative to a hemisphere of the base
/// radius. Zoby & Sullivan, "Effects of Corner Radius on Stagnation-Point
/// Velocity Gradients on Blunt Axisymmetric Bodies", NASA TM X-1067
/// (1965), fig. 4, read from the figure (±0.02).
const ZOBY_SULLIVAN: [(f64, f64); 5] = [(0.0, 0.29), (0.4, 0.50), (0.6, 0.60), (0.8, 0.81), (1.0, 1.0)];

/// The effective nose radius (for stagnation heating, ∝ 1/√r) of a face of
/// sphere radius `r_n` (∞: flat) over a body whose projected disc has
/// radius `r_b`: r_N where the face is at least as curved as a hemisphere
/// over r_B, else r_B over Zoby & Sullivan's velocity-gradient ratio (a
/// flat face 3.45·r_B, not r_B: its stagnation gradient is the
/// hemisphere's × 0.29), never above r_N.
pub fn effective_nose_radius(r_b: f64, r_n: f64) -> f64 {
    if r_b <= 0.0 {
        return r_n;
    }
    let x = if r_n.is_finite() { r_b / r_n } else { 0.0 };
    if x >= 1.0 {
        return r_n;
    }
    let t = &ZOBY_SULLIVAN;
    let k = t.partition_point(|e| e.0 <= x).clamp(1, t.len() - 1);
    let (a, b) = (t[k - 1], t[k]);
    let ratio = a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0);
    (r_b / ratio).min(r_n)
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
            g.spread.into_iter().for_each(&mut put);
        }
        self.nose_radius.iter().for_each(|&r| put(r));
        put(self.length);
        self.extent.iter().chain(&self.wave).for_each(|&x| put(x));
        let b = &self.sections;
        b.axis
            .to_array()
            .into_iter()
            .chain(b.point.to_array())
            .chain([b.x0, b.dx])
            .chain(b.area.iter().copied())
            .for_each(&mut put);
        for f in &self.fins {
            let (n, c) = (f.normal.to_array(), f.centroid.to_array());
            n.into_iter().chain(c).chain([f.area, f.body_radius, f.semispan]).for_each(&mut put);
            f.cells.iter().for_each(|&c| put(f64::from(c)));
        }
        for &(l, t) in &self.relief {
            bytes.extend_from_slice(&l.to_bits().to_le_bytes());
            bytes.extend_from_slice(&t.to_bits().to_le_bytes());
        }
        bytes.extend_from_slice(&self.exposure);
        crate::ephem::fnv1a64(&bytes)
    }

    /// The sums at flow direction `d` (unit, body axes): over the cells,
    /// with the exposure interpolated barycentrically between the three
    /// grid directions around `d`; the nose radius interpolated likewise.
    pub fn sums_at(&self, d: DVec3) -> DirSums {
        let e = self.exposure_weights(d);
        let mut s = direction_sums(&self.geometry, d, |i| e.at(i));
        s.nose_radius = self.nose_radius_at(d);
        s
    }

    /// The boundary layer's 3D relief around flow direction `d` (unit, body
    /// axes): [`Relief::at`] gives x_eff/x (laminar, turbulent) at a run
    /// length, interpolated like the exposure.
    pub fn relief_at(&self, d: DVec3) -> Relief<'_> {
        let w = self.grid.locate(d);
        let n = super::relief::STATIONS;
        let extent = (0..3).map(|k| w.w[k] * self.extent[w.dirs[k] as usize]).sum();
        Relief { rows: w.dirs.map(|k| &self.relief[k as usize * n..][..n]), w: w.w, extent }
    }

    /// The exposure rows and barycentric weights around flow direction `d`
    /// (unit, body axes): [`Exposure::at`] interpolates a cell's exposure.
    pub fn exposure_weights(&self, d: DVec3) -> Exposure<'_> {
        let w = self.grid.locate(d);
        Exposure { rows: w.dirs.map(|k| &self.exposure[k as usize * self.cells..][..self.cells]), w: w.w }
    }

    /// The craft's extent along `d` (unit, body axes), interpolated like
    /// [`Self::sums_at`].
    pub fn extent_at(&self, d: DVec3) -> f64 {
        let w = self.grid.locate(d);
        (0..3).map(|k| w.w[k] * self.extent[w.dirs[k] as usize]).sum()
    }

    /// Wave drag per unit q (m², uncapped) along `d`, interpolated like
    /// [`Self::sums_at`].
    pub fn wave_at(&self, d: DVec3) -> f64 {
        let w = self.grid.locate(d);
        (0..3).map(|k| w.w[k] * self.wave[w.dirs[k] as usize]).sum()
    }

    /// The effective nose radius at flow direction `d` (unit, body axes),
    /// interpolated like [`Self::sums_at`] (without forming the sums).
    pub fn nose_radius_at(&self, d: DVec3) -> f64 {
        let w = self.grid.locate(d);
        (0..3).map(|k| w.w[k] * self.nose_radius[w.dirs[k] as usize]).sum()
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
