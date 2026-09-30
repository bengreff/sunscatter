//! Surface cells (D065, realism-1 §3a): the craft's surface divided into
//! cells that carry aero, heating and contact.
//!
//! Cells are built top-down from the union surface ([`super::mesh`]): each
//! smooth face starts as one cell, and the cell with the largest error is
//! split in two (along its longest extent, at equal area) until the target
//! count is reached. The error is `area × (1 + γ·spread)`, where `spread`
//! is the angular spread of the cell's normals, so cells end up smaller
//! where the surface curves tightly (nose, feet, struts) and larger on flat
//! or gently curved faces. Cells never span two faces, so edges between
//! faces stay cell boundaries.
//!
//! Each cell stores its thermal data (skin areal mass, specific heat,
//! emissivity), its neighbours with the conductance between them (through
//! shared edges; where primitives meet without shared vertices, through the
//! nearest cell across the joint), and whether it is a contact point (a
//! foot, or on the convex hull of the cell centroids).

use super::file::{Primitive, Shape, Skin};
use super::mesh::{edge_key, Surface};
use crate::math;
use glam::DVec3;
use std::collections::{BTreeMap, HashMap};

/// How to build cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellOptions {
    /// Highest number of cells (at least one per face). Q1 of realism-1: 512
    /// until the aero and heating budget is measured.
    pub target: usize,
    /// Weight γ of the normal spread (rad) in the split error.
    pub curvature_weight: f64,
    /// Farthest a joint between primitives is bridged (m).
    pub joint_reach: f64,
}

impl Default for CellOptions {
    fn default() -> Self {
        CellOptions { target: 512, curvature_weight: 4.0, joint_reach: 0.3 }
    }
}

/// A thermal link to a neighbouring cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Neighbour {
    pub cell: u32,
    /// Conductance through the skin (W/K).
    pub conductance: f64,
}

/// One surface cell (body axes).
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub centroid: DVec3,
    /// Outward unit normal (area-weighted mean of its triangles).
    pub normal: DVec3,
    /// m².
    pub area: f64,
    /// Index of the primitive it belongs to.
    pub primitive: u32,
    /// Skin areal mass (kg/m²), specific heat (J/(kg·K)), emissivity.
    pub areal_mass: f64,
    pub specific_heat: f64,
    pub emissivity: f64,
    /// The skin material's temperature limit (K) and conductance to its
    /// node per unit area (W/(m²·K)), where the material sets its own.
    pub max_k: Option<f64>,
    pub coupling: Option<f64>,
    /// A contact point (a foot, or on the convex hull).
    pub contact: bool,
    pub neighbours: Vec<Neighbour>,
}

impl Cell {
    /// Skin mass (kg): areal mass × area.
    pub fn skin_mass(&self) -> f64 {
        self.areal_mass * self.area
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ContactKind {
    /// A landing-gear foot (soft, with a stroke; §3e).
    Foot,
    /// A hull point on the convex hull (stiff).
    Hull,
}

/// A point that can touch the ground (the hitbox, §5c).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ContactPoint {
    pub pos: DVec3,
    pub kind: ContactKind,
    /// The cell it belongs to.
    pub cell: u32,
}

/// The craft's cells.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cells {
    pub cells: Vec<Cell>,
    /// Cell of each surface triangle.
    pub tri_cell: Vec<u32>,
    /// Feet first (in primitive order), then hull points (in cell order).
    pub contacts: Vec<ContactPoint>,
}

/// Per-triangle centroid, normal and area.
struct Tris {
    c: Vec<DVec3>,
    n: Vec<DVec3>,
    a: Vec<f64>,
}

impl Tris {
    fn of(s: &Surface) -> Self {
        let mut t = Tris { c: Vec::new(), n: Vec::new(), a: Vec::new() };
        for i in 0..s.triangles.len() {
            let (c, n, a) = s.triangle(i);
            t.c.push(c);
            t.n.push(n);
            t.a.push(a);
        }
        t
    }

    fn error(&self, group: &[u32], gamma: f64) -> f64 {
        if group.len() < 2 {
            return -1.0;
        }
        let (area, vector) = group.iter().fold((0.0, DVec3::ZERO), |(a, v), &t| {
            (a + self.a[t as usize], v + self.n[t as usize] * self.a[t as usize])
        });
        let spread = (2.0 * (1.0 - (vector.length() / area).min(1.0))).sqrt();
        area * (1.0 + gamma * spread)
    }

    /// Splits a group at equal area across its longest extent.
    fn split(&self, group: &mut Vec<u32>) -> Vec<u32> {
        let (lo, hi) = group.iter().fold((DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)), |(lo, hi), &t| {
            (lo.min(self.c[t as usize]), hi.max(self.c[t as usize]))
        });
        let ext = hi - lo;
        let axis = if ext.x >= ext.y && ext.x >= ext.z {
            0
        } else if ext.y >= ext.z {
            1
        } else {
            2
        };
        group.sort_by(|&p, &q| self.c[p as usize][axis].total_cmp(&self.c[q as usize][axis]).then(p.cmp(&q)));
        let half = group.iter().map(|&t| self.a[t as usize]).sum::<f64>() / 2.0;
        let mut acc = 0.0;
        let mut k = 0;
        while k < group.len() - 1 && acc + self.a[group[k] as usize] <= half {
            acc += self.a[group[k] as usize];
            k += 1;
        }
        let k = k.clamp(1, group.len() - 1);
        let rest = group.split_off(k);
        group.sort_unstable();
        let mut rest = rest;
        rest.sort_unstable();
        rest
    }
}

/// Where a foot touches: its base (frustum, cylinder), far end (strut),
/// pole (sphere cap) or the centre of its −Z face (box).
pub fn foot_point(shape: &Shape) -> DVec3 {
    match *shape {
        Shape::Frustum { base, .. } | Shape::Cylinder { base, .. } => base,
        Shape::Strut { to, .. } => to,
        Shape::SphereCap { center, axis, radius, .. } => center + axis.normalize() * radius,
        Shape::Box { center, half } => center - DVec3::Z * half.z,
    }
}

/// Evenly spread unit directions (a Fibonacci sphere).
pub fn sphere_directions(n: usize) -> Vec<DVec3> {
    let golden = math::PI * (3.0 - 5.0f64.sqrt());
    (0..n)
        .map(|i| {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let phi = golden * i as f64;
            DVec3::new(r * math::cos(phi), r * math::sin(phi), z)
        })
        .collect()
}

/// Directions sampled to find the convex hull's points.
const HULL_DIRECTIONS: usize = 2000;

/// Builds the cells of a union surface.
pub fn build_cells(s: &Surface, primitives: &[Primitive], default_skin: &Skin, opts: &CellOptions) -> Cells {
    let tris = Tris::of(s);
    // One group per face, then split the worst until the target.
    let mut groups: Vec<Vec<u32>> = vec![Vec::new(); s.patches.len()];
    for (t, &p) in s.tri_patch.iter().enumerate() {
        groups[p as usize].push(t as u32);
    }
    groups.retain(|g| !g.is_empty());
    let mut errors: Vec<f64> = groups.iter().map(|g| tris.error(g, opts.curvature_weight)).collect();
    while groups.len() < opts.target {
        let (worst, &e) =
            errors.iter().enumerate().fold((0, &f64::MIN), |best, cur| if cur.1 > best.1 { cur } else { best });
        if e < 0.0 {
            break;
        }
        let rest = tris.split(&mut groups[worst]);
        errors[worst] = tris.error(&groups[worst], opts.curvature_weight);
        errors.push(tris.error(&rest, opts.curvature_weight));
        groups.push(rest);
    }
    let mut tri_cell = vec![0u32; s.triangles.len()];
    let mut cells: Vec<Cell> = groups
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let (mut area, mut moment, mut vector) = (0.0, DVec3::ZERO, DVec3::ZERO);
            for &t in g {
                let t = t as usize;
                tri_cell[t] = i as u32;
                area += tris.a[t];
                moment += tris.c[t] * tris.a[t];
                vector += tris.n[t] * tris.a[t];
            }
            let primitive = s.patches[s.tri_patch[g[0] as usize] as usize].primitive;
            let skin = primitives[primitive as usize].skin.unwrap_or(*default_skin);
            Cell {
                centroid: moment / area,
                normal: vector.normalize_or_zero(),
                area,
                primitive,
                areal_mass: skin.areal_mass,
                specific_heat: skin.specific_heat,
                emissivity: skin.emissivity,
                max_k: skin.max_k,
                coupling: skin.coupling,
                contact: false,
                neighbours: Vec::new(),
            }
        })
        .collect();
    link_neighbours(s, &tris, &tri_cell, primitives, default_skin, opts, &mut cells);
    let contacts = find_contacts(primitives, &mut cells);
    Cells { cells, tri_cell, contacts }
}

/// Fills the neighbour lists: shared edges, then joints between primitives
/// (edges left open by the union, bridged to the nearest triangle of
/// another primitive; each side contributes half its edge length).
fn link_neighbours(
    s: &Surface,
    tris: &Tris,
    tri_cell: &[u32],
    primitives: &[Primitive],
    default_skin: &Skin,
    opts: &CellOptions,
    cells: &mut [Cell],
) {
    let prim_of = |t: usize| s.patches[s.tri_patch[t] as usize].primitive;
    let mut edges: BTreeMap<[[i64; 3]; 2], (Vec<u32>, f64, DVec3)> = BTreeMap::new();
    for (t, tri) in s.triangles.iter().enumerate() {
        let p = tri.map(|i| s.positions[i as usize]);
        for k in 0..3 {
            let (a, b) = (p[k], p[(k + 1) % 3]);
            edges
                .entry(edge_key(a, b))
                .or_insert_with(|| (Vec::new(), (b - a).length(), (a + b) * 0.5))
                .0
                .push(t as u32);
        }
    }
    // Shared length between cell pairs.
    let mut shared: BTreeMap<(u32, u32), f64> = BTreeMap::new();
    let mut add = |i: u32, j: u32, len: f64| {
        if i != j {
            *shared.entry((i.min(j), i.max(j))).or_default() += len;
        }
    };
    // Grid of triangle centroids for the joint search.
    let size = opts.joint_reach;
    let key = |p: DVec3| (p / size).floor().as_i64vec3().to_array();
    let mut grid: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    for (t, &c) in tris.c.iter().enumerate() {
        grid.entry(key(c)).or_default().push(t as u32);
    }
    for (ts, len, mid) in edges.values() {
        match ts.as_slice() {
            [a, b] => add(tri_cell[*a as usize], tri_cell[*b as usize], *len),
            [a] => {
                let own = prim_of(*a as usize);
                let k = key(*mid);
                let mut best: Option<(f64, u32)> = None;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let Some(list) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else { continue };
                            for &t in list {
                                if prim_of(t as usize) == own {
                                    continue;
                                }
                                let d = (tris.c[t as usize] - *mid).length();
                                if d <= size && best.is_none_or(|(bd, bt)| d < bd || (d == bd && t < bt)) {
                                    best = Some((d, t));
                                }
                            }
                        }
                    }
                }
                if let Some((_, t)) = best {
                    add(tri_cell[*a as usize], tri_cell[t as usize], 0.5 * len);
                }
            }
            _ => {}
        }
    }
    let sheet = |c: &Cell| {
        let skin = primitives[c.primitive as usize].skin.unwrap_or(*default_skin);
        skin.conductivity * skin.thickness
    };
    for (&(i, j), &len) in &shared {
        let (ci, cj) = (&cells[i as usize], &cells[j as usize]);
        let (ki, kj) = (sheet(ci), sheet(cj));
        let k = if ki + kj > 0.0 { 2.0 * ki * kj / (ki + kj) } else { 0.0 };
        let d = (ci.centroid - cj.centroid).length().max(1e-3);
        let conductance = k * len / d;
        cells[i as usize].neighbours.push(Neighbour { cell: j, conductance });
        cells[j as usize].neighbours.push(Neighbour { cell: i, conductance });
    }
    for c in cells.iter_mut() {
        c.neighbours.sort_by_key(|n| n.cell);
    }
}

/// Contact points: one per foot, then the cells whose centroids are on the
/// convex hull of all centroids and feet (found as the extreme point along
/// many directions). Marks the cells.
fn find_contacts(primitives: &[Primitive], cells: &mut [Cell]) -> Vec<ContactPoint> {
    let mut contacts = Vec::new();
    for (p, prim) in primitives.iter().enumerate().filter(|(_, p)| p.foot) {
        let pos = foot_point(&prim.shape);
        let nearest = cells.iter().enumerate().filter(|(_, c)| c.primitive == p as u32).fold(
            None,
            |best: Option<(f64, usize)>, (i, c)| {
                let d = (c.centroid - pos).length();
                if best.is_none_or(|(bd, _)| d < bd) {
                    Some((d, i))
                } else {
                    best
                }
            },
        );
        if let Some((_, i)) = nearest {
            cells[i].contact = true;
            contacts.push(ContactPoint { pos, kind: ContactKind::Foot, cell: i as u32 });
        }
    }
    let feet: Vec<DVec3> = contacts.iter().map(|c| c.pos).collect();
    let is_foot = |c: &Cell| primitives[c.primitive as usize].foot;
    let mut hull = vec![false; cells.len()];
    for d in sphere_directions(HULL_DIRECTIONS) {
        let foot_best = feet.iter().map(|f| f.dot(d)).fold(f64::MIN, f64::max);
        let mut best: Option<(f64, usize)> = None;
        for (i, c) in cells.iter().enumerate() {
            let x = c.centroid.dot(d);
            if !is_foot(c) && best.is_none_or(|(b, _)| x > b) {
                best = Some((x, i));
            }
        }
        if let Some((x, i)) = best {
            if x > foot_best {
                hull[i] = true;
            }
        }
    }
    for (i, c) in cells.iter_mut().enumerate() {
        if hull[i] {
            c.contact = true;
            contacts.push(ContactPoint { pos: c.centroid, kind: ContactKind::Hull, cell: i as u32 });
        }
    }
    contacts
}

/// FNV-1a hash of every cell's data (determinism checks).
pub fn cells_hash(cells: &Cells) -> u64 {
    let mut bytes = Vec::new();
    let mut put = |x: f64| bytes.extend_from_slice(&x.to_bits().to_le_bytes());
    for c in &cells.cells {
        for x in [c.centroid, c.normal].iter().flat_map(|v| v.to_array()) {
            put(x);
        }
        for x in [
            c.area,
            c.areal_mass,
            c.specific_heat,
            c.emissivity,
            f64::from(c.primitive),
            f64::from(u8::from(c.contact)),
        ] {
            put(x);
        }
        for n in &c.neighbours {
            put(f64::from(n.cell));
            put(n.conductance);
        }
    }
    for p in &cells.contacts {
        for x in p.pos.to_array() {
            put(x);
        }
        put(f64::from(p.cell));
    }
    crate::ephem::fnv1a64(&bytes)
}

#[cfg(test)]
#[path = "cells_tests.rs"]
mod tests;
