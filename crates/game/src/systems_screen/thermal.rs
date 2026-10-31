//! The thermal layer's mesh (D065, D106; D123's exception for the
//! interface): what the thermal network is, drawn on the craft. Each skin
//! cell is coloured on the surface by its own temperature, the cells'
//! boundaries drawn as a grid; the interior nodes (the volume grid's cubes,
//! `node_size`) are drawn as cubes behind a cut-away the player moves.
//!
//! The meshes are built once per configuration from the simulation's own
//! surface and cells (`Craft::surface`, `Cells::tri_cell`) and the volume
//! grid (`CraftDesign::volume`), per part so mechanisms turn them and
//! isolation hides them. Their vertices point into one texture of one
//! texel per cell and per node ([`Texture`]), so each frame only the
//! texture changes ([`texels`]): cheap for a craft of many cells. A texel's
//! alpha of 0 hides its cell or node (the cut, through the material's alpha
//! mask).
//!
//! Colours follow D106: a cell or node of a part whose temperature a sensor
//! measures is coloured blue to red against its limit (`rules::heat_colour`);
//! the rest show the grid in a neutral "not measured" shade. Debug mode
//! knows everything, so it colours everything.

use bevy::prelude::Vec3;
use glam::DVec3;
use sim::craft::mesh::{edge_key, Surface};
use sim::craft::volume::VolumeGrid;
use std::collections::HashMap;

/// The shade of a cell or node whose temperature nothing measures (linear).
pub const NOT_MEASURED: [f32; 3] = [0.30, 0.31, 0.33];
/// How far the cells' boundary lines sit above the surface (m), so they are
/// not lost in it.
pub const LINE_LIFT: f32 = 0.02;
/// A node's cube against its grid cell: a little smaller, so the grid's
/// gaps show.
pub const CUBE_SHARE: f32 = 0.88;

/// Where cell and node texels are: cells first, then nodes, row by row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub cells: usize,
    pub nodes: usize,
}

impl Texture {
    /// The smallest near-square texture with a texel for each cell and node.
    pub fn new(cells: usize, nodes: usize) -> Texture {
        let n = (cells + nodes).max(1);
        let width = ((n as f64).sqrt().ceil() as u32).max(1);
        let height = (n as u32).div_ceil(width).max(1);
        Texture { width, height, cells, nodes }
    }

    /// The centre of texel `i` in UV coordinates.
    pub fn uv(&self, i: usize) -> [f32; 2] {
        let (x, y) = (i as u32 % self.width, i as u32 / self.width);
        [(x as f32 + 0.5) / self.width as f32, (y as f32 + 0.5) / self.height as f32]
    }

    /// Texel of node `n`.
    pub fn node(&self, n: usize) -> usize {
        self.cells + n
    }

    pub fn texels(&self) -> usize {
        (self.width * self.height) as usize
    }
}

/// A triangle mesh whose vertices read a texel each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UvMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Line segments (pairs of points), each reading a texel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lines {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
}

/// One part's share of the thermal mesh (craft axes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PartMesh {
    /// Its skin cells' triangles.
    pub skin: UvMesh,
    /// Its cells' boundaries.
    pub lines: Lines,
    /// Its interior nodes as cubes.
    pub nodes: UvMesh,
}

/// The whole craft's thermal mesh: per part, and where each cell's and
/// node's centre is (craft axes, for the cut).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThermalMesh {
    pub parts: Vec<PartMesh>,
    pub texture: Option<Texture>,
    pub cell_centres: Vec<Vec3>,
    pub node_centres: Vec<Vec3>,
    /// Per part: it has no skin cell (enclosed by others, as an engine in
    /// its interstage), so nothing covers its nodes.
    pub skinless: Vec<bool>,
}

/// The thermal mesh of a craft: `surface` and `tri_cell` its cells (the
/// simulation's), `cell_part` and `node_part` the part (an index into the
/// craft's `parts`) of each cell and node, `centroids` each cell's.
pub fn build(
    surface: &Surface,
    tri_cell: &[u32],
    centroids: &[Vec3],
    cell_part: &[u32],
    volume: &VolumeGrid,
    node_part: &[u32],
    parts: usize,
) -> ThermalMesh {
    let tex = Texture::new(cell_part.len(), volume.nodes.len());
    let mut out: Vec<PartMesh> = vec![PartMesh::default(); parts];
    let part_of_cell = |c: u32| cell_part.get(c as usize).map(|&p| p as usize).filter(|&p| p < parts);
    // Skin: each triangle in its cell's part, its three corners reading the
    // cell's texel (one colour per cell).
    // Each edge (by its corners' positions: patches do not share vertices)
    // with its corners and the triangles along it.
    type Edge = (DVec3, DVec3, Vec<usize>);
    let mut edges: HashMap<[[i64; 3]; 2], Edge> = HashMap::new();
    for (t, tri) in surface.triangles.iter().enumerate() {
        let Some(&cell) = tri_cell.get(t) else { continue };
        let Some(p) = part_of_cell(cell) else { continue };
        let m = &mut out[p].skin;
        let uv = tex.uv(cell as usize);
        for &v in tri {
            m.indices.push(m.positions.len() as u32);
            m.positions.push(surface.positions[v as usize].as_vec3().to_array());
            m.normals.push(surface.normals[v as usize].as_vec3().to_array());
            m.uvs.push(uv);
        }
        for k in 0..3 {
            let (a, b) = (surface.positions[tri[k] as usize], surface.positions[tri[(k + 1) % 3] as usize]);
            edges.entry(edge_key(a, b)).or_insert_with(|| (a, b, Vec::new())).2.push(t);
        }
    }
    // Boundaries: an edge between two cells, or the end of the surface,
    // lifted along its first triangle's normal; in that triangle's part
    // and reading its cell (so the cut hides it with the cell).
    let mut boundary: Vec<(&[[i64; 3]; 2], &Edge)> = (edges.iter())
        .filter(|(_, (_, _, ts))| ts.len() == 1 || ts.iter().any(|&t| tri_cell[t] != tri_cell[ts[0]]))
        .collect();
    boundary.sort_by(|a, b| a.0.cmp(b.0));
    for (_, (a, b, ts)) in boundary {
        let t = ts[0];
        let cell = tri_cell[t];
        let Some(p) = part_of_cell(cell) else { continue };
        let lift = surface.triangle(t).1.as_vec3() * LINE_LIFT;
        let l = &mut out[p].lines;
        l.positions.push((a.as_vec3() + lift).to_array());
        l.positions.push((b.as_vec3() + lift).to_array());
        l.uvs.extend([tex.uv(cell as usize); 2]);
    }
    // Nodes: a cube each, as big as its inside share (`VolumeNode::volume`
    // over the cube's) at the centre of its inside samples.
    let full = volume.size.powi(3).max(1e-12);
    for (n, node) in volume.nodes.iter().enumerate() {
        let Some(&p) = node_part.get(n) else { continue };
        let Some(m) = out.get_mut(p as usize).map(|m| &mut m.nodes) else { continue };
        let edge = volume.size as f32 * CUBE_SHARE * ((node.volume / full).clamp(0.0, 1.0) as f32).cbrt();
        cube(m, node.centre.as_vec3(), edge, tex.uv(tex.node(n)));
    }
    ThermalMesh {
        parts: out,
        texture: Some(tex),
        cell_centres: centroids.to_vec(),
        node_centres: volume.nodes.iter().map(|n| n.centre.as_vec3()).collect(),
        skinless: (0..parts).map(|p| !cell_part.iter().any(|&c| c as usize == p)).collect(),
    }
}

/// Appends a cube of edge `edge` at `centre` (a face each way, outward),
/// every corner reading `uv`.
fn cube(m: &mut UvMesh, centre: Vec3, edge: f32, uv: [f32; 2]) {
    let h = edge / 2.0;
    let e = [Vec3::X, Vec3::Y, Vec3::Z];
    for axis in 0..3 {
        for sign in [1.0f32, -1.0] {
            let n = e[axis] * sign;
            // Counter-clockwise seen from outside: u × v is the axis (the
            // next two, in turn), swapped for the negative face.
            let (u, v) = (e[(axis + 1) % 3], e[(axis + 2) % 3]);
            let (u, v) = if sign > 0.0 { (u, v) } else { (v, u) };
            let base = m.positions.len() as u32;
            for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                m.positions.push((centre + (n + u * a + v * b) * h).to_array());
                m.normals.push(n.to_array());
                m.uvs.push(uv);
            }
            m.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

/// The cut-away: everything on the camera's side of a plane is hidden.
/// The plane faces the camera (`toward`, a unit vector in craft axes) and
/// moves across what is framed: `near` and `far` its extent along
/// `toward` (the camera's side is `near`), `depth` how far in the plane
/// is (0: nothing cut; 1: all cut).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cut {
    pub toward: Vec3,
    pub near: f32,
    pub far: f32,
    pub depth: f32,
}

impl Cut {
    /// The cut across `points` (craft axes) facing `toward`.
    pub fn across(points: impl Iterator<Item = Vec3>, toward: Vec3, depth: f32) -> Cut {
        let (far, near) =
            points.map(|p| p.dot(toward)).fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
        if far > near {
            return Cut { toward, near: 0.0, far: 0.0, depth: 0.0 };
        }
        Cut { toward, near, far, depth }
    }

    /// Whether point `p` (craft axes) is cut away.
    pub fn hides(&self, p: Vec3) -> bool {
        self.depth > 0.0 && p.dot(self.toward) > self.near - self.depth * (self.near - self.far)
    }
}

/// One texel's colour (sRGB bytes, alpha 255 shown, 0 hidden): its
/// temperature against its limit where it is measured, else the neutral
/// shade.
pub fn texel(measured: Option<(f64, f64)>, hidden: bool) -> [u8; 4] {
    if hidden {
        return [0; 4];
    }
    let [r, g, b] = measured.map_or(NOT_MEASURED, |(k, limit)| super::rules::heat_colour(k, limit));
    use bevy::color::ColorToPacked;
    let c = bevy::color::Color::linear_rgb(r, g, b).to_srgba().to_u8_array();
    [c[0], c[1], c[2], 255]
}

/// What the texels read: the temperatures and limits of the cells and
/// nodes, and which parts (indices) the craft measures.
pub struct Reading<'a> {
    pub temps: &'a super::rules::Temps<'a>,
    /// Per part index: a sensor measures its temperature (debug: all).
    pub measured: &'a [bool],
}

/// Every texel of `mesh`'s texture into `out` (RGBA, one per texel; the
/// unused end stays clear): each cell by its own temperature where
/// measured, nodes only behind a cut (with nothing cut they sit inside
/// the skin), anything on the camera's side of the cut hidden.
pub fn texels(mesh: &ThermalMesh, r: &Reading, cut: Option<&Cut>, out: &mut Vec<u8>) {
    let Some(tex) = mesh.texture else {
        out.clear();
        return;
    };
    out.clear();
    out.resize(tex.texels() * 4, 0);
    let t = r.temps;
    let known = |part: Option<&u32>| part.is_some_and(|&p| r.measured.get(p as usize).copied().unwrap_or(false));
    let hides = |p: Option<&Vec3>| cut.is_some_and(|c| p.is_some_and(|p| c.hides(*p)));
    for i in 0..tex.cells {
        let m = (known(t.cell_part.get(i))).then(|| t.skin.get(i).copied().zip(t.skin_max.get(i).copied())).flatten();
        out[i * 4..i * 4 + 4].copy_from_slice(&texel(m, hides(mesh.cell_centres.get(i))));
    }
    let cut_in = cut.is_some_and(|c| c.depth > 0.0);
    for n in 0..tex.nodes {
        let m = (known(t.node_part.get(n))).then(|| t.nodes.get(n).copied().zip(t.node_max.get(n).copied())).flatten();
        let bare = t.node_part.get(n).is_some_and(|&p| mesh.skinless.get(p as usize).copied().unwrap_or(false));
        let hidden = !(cut_in || bare) || hides(mesh.node_centres.get(n));
        let k = tex.node(n) * 4;
        out[k..k + 4].copy_from_slice(&texel(m, hidden));
    }
}

#[cfg(test)]
#[path = "thermal_tests.rs"]
mod tests;
