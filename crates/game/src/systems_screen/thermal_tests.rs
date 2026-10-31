use super::*;
use crate::systems_screen::rules::Temps;
use glam::DVec3;

/// A surface of `tris` triangles (each three corners), one patch, normals +Z.
fn surface(tris: &[[DVec3; 3]]) -> Surface {
    let mut s = Surface::default();
    for t in tris {
        let base = s.positions.len() as u32;
        s.positions.extend_from_slice(t);
        s.normals.extend([DVec3::Z; 3]);
        s.triangles.push([base, base + 1, base + 2]);
        s.tri_patch.push(0);
    }
    s
}

fn d(x: f64, y: f64) -> DVec3 {
    DVec3::new(x, y, 0.0)
}

/// An empty volume grid (the test rocket's, with no node given a part:
/// none drawn).
fn grid() -> VolumeGrid {
    sim::craft::design::CraftDesign::new(&crate::test_rocket::rocket()).volume
}

#[test]
fn the_texture_has_a_texel_per_cell_and_node() {
    // (cells, nodes) → (width, height)
    for ((cells, nodes), size) in [((0, 0), (1, 1)), ((5, 4), (3, 3)), ((10, 0), (4, 3)), ((3000, 1000), (64, 63))] {
        let t = Texture::new(cells, nodes);
        assert_eq!((t.width, t.height), size, "{cells} {nodes}");
        assert!(t.texels() >= cells + nodes);
    }
    let t = Texture::new(5, 4);
    assert_eq!(t.uv(0), [1.0 / 6.0, 1.0 / 6.0]);
    assert_eq!(t.uv(4), [0.5, 0.5]);
    assert_eq!((t.node(0), t.uv(t.node(3))), (5, [5.0 / 6.0, 5.0 / 6.0]));
}

/// The cells' boundaries (table): a square of two triangles in one cell
/// has four (the diagonal is inside the cell); in two cells, five; each
/// part draws its own, read from its cell's texel.
#[test]
fn boundaries_are_drawn_between_cells_and_at_the_edges() {
    let square = [[d(0., 0.), d(1., 0.), d(1., 1.)], [d(0., 0.), d(1., 1.), d(0., 1.)]];
    let apart = [d(5., 0.), d(6., 0.), d(5., 1.)];
    // (cells of the three triangles, parts of the cells) → segments per part
    let table: [(&[u32], &[u32], [usize; 2]); 3] =
        [(&[0, 0, 1], &[0, 1], [4, 3]), (&[0, 1, 2], &[0, 0, 1], [5, 3]), (&[0, 1, 2], &[0, 0, 0], [8, 0])];
    let s = surface(&[square[0], square[1], apart]);
    for (tri_cell, cell_part, want) in table {
        let centroids = vec![Vec3::ZERO; cell_part.len()];
        let m = build(&s, tri_cell, &centroids, cell_part, &grid(), &[], 2);
        let got = [m.parts[0].lines.positions.len() / 2, m.parts[1].lines.positions.len() / 2];
        assert_eq!(got, want, "{tri_cell:?} {cell_part:?}");
        let tex = m.texture.unwrap();
        // Every segment reads a cell of its own part.
        for (p, part) in m.parts.iter().enumerate() {
            assert_eq!(part.lines.uvs.len(), part.lines.positions.len());
            for uv in &part.lines.uvs {
                let cell = (0..tex.cells).find(|&c| tex.uv(c) == *uv).expect("a cell's texel");
                assert_eq!(cell_part[cell] as usize, p);
            }
        }
        // The skin: each triangle once, in its cell's part, lifted lines.
        let tris: usize = m.parts.iter().map(|p| p.skin.indices.len() / 3).sum();
        assert_eq!(tris, 3);
        assert!(m.parts[0].lines.positions.iter().all(|p| (p[2] - LINE_LIFT).abs() < 1e-6));
    }
}

/// The test rocket's whole thermal mesh: every surface triangle drawn once
/// in its cell's part reading its cell's texel, every node a cube in its
/// part no bigger than its grid cube.
#[test]
fn the_rocket_draws_every_cell_and_node_once() {
    let craft = crate::test_rocket::rocket();
    let design = sim::craft::design::CraftDesign::new(&craft);
    let centroids: Vec<Vec3> = craft.cells.cells.iter().map(|c| c.centroid.as_vec3()).collect();
    let m = build(
        &craft.surface,
        &craft.cells.tri_cell,
        &centroids,
        &design.cell_part,
        &design.volume,
        &design.node_part,
        craft.parts.len(),
    );
    let tex = m.texture.unwrap();
    assert_eq!((tex.cells, tex.nodes), (design.cells.len(), design.volume.nodes.len()));
    let tris: usize = m.parts.iter().map(|p| p.skin.indices.len() / 3).sum();
    assert_eq!(tris, craft.surface.triangles.len());
    let cubes: usize = m.parts.iter().map(|p| p.nodes.positions.len() / 24).sum();
    assert_eq!(cubes, design.volume.nodes.len());
    assert!(m.parts.iter().any(|p| !p.lines.positions.is_empty()));
    let size = design.volume.size as f32;
    for (k, part) in m.parts.iter().enumerate() {
        for (i, uv) in part.skin.uvs.iter().enumerate().step_by(3) {
            let cell = (0..tex.cells).find(|&c| tex.uv(c) == *uv).expect("a cell's texel");
            assert_eq!(design.cell_part[cell] as usize, k, "triangle {}", i / 3);
        }
        for cube in part.nodes.positions.chunks(24) {
            let (lo, hi) = cube.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
                (lo.min(Vec3::from_array(*p)), hi.max(Vec3::from_array(*p)))
            });
            let edge = hi - lo;
            assert!(edge.max_element() <= size * CUBE_SHARE + 1e-4 && edge.min_element() > 0.0, "{edge}");
        }
    }
}

/// A cube's faces point outward, counter-clockwise from outside.
#[test]
fn a_nodes_cube_faces_outward() {
    let mut m = UvMesh::default();
    cube(&mut m, Vec3::new(1.0, 2.0, 3.0), 2.0, [0.5, 0.5]);
    assert_eq!((m.positions.len(), m.indices.len()), (24, 36));
    for tri in m.indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|k| Vec3::from_array(m.positions[tri[k] as usize]));
        let n = Vec3::from_array(m.normals[tri[0] as usize]);
        assert!((b - a).cross(c - a).dot(n) > 0.0, "{tri:?}");
        assert!(((a + b + c) / 3.0 - Vec3::new(1.0, 2.0, 3.0)).dot(n) > 0.0);
    }
}

/// The cut (table): nothing at depth 0; at a half, the camera's half of
/// the extent across the points; all at 1. Nothing to cut: no cut.
#[test]
fn the_cut_hides_the_cameras_side() {
    let cut = |depth| Cut::across([-2.0, 0.5, 2.0].map(|z| Vec3::new(1.0, 1.0, z)).into_iter(), Vec3::Z, depth);
    assert_eq!((cut(0.3).near, cut(0.3).far), (2.0, -2.0));
    // (depth, point's z) → hidden
    let table = [
        (0.0, 5.0, false),
        (0.0, -5.0, false),
        (0.5, 0.1, true),
        (0.5, -0.1, false),
        (0.25, 0.9, false),
        (0.25, 1.1, true),
        (1.0, -1.9, true),
    ];
    for (depth, z, want) in table {
        assert_eq!(cut(depth).hides(Vec3::new(3.0, -1.0, z)), want, "{depth} {z}");
    }
    assert!(!Cut::across(std::iter::empty(), Vec3::Z, 1.0).hides(Vec3::ZERO));
}

/// The texels (table, D106): a cell of a measured part by its temperature
/// against its limit; an unmeasured one the neutral shade; nodes only
/// behind a cut; what the cut takes clear.
#[test]
fn texels_show_what_is_measured_and_what_the_cut_leaves() {
    // Two cells (parts 0 and 1, the first in front at z 1) and two nodes
    // (parts 0 and 1, at z −1 and +1).
    let mesh = ThermalMesh {
        parts: vec![PartMesh::default(); 2],
        texture: Some(Texture::new(2, 2)),
        cell_centres: vec![Vec3::Z, -Vec3::Z],
        node_centres: vec![-Vec3::Z, Vec3::Z],
        skinless: vec![false, false],
    };
    let temps = Temps {
        skin: &[600.0, 300.0],
        nodes: &[250.0, 280.0],
        cell_part: &[0, 1],
        node_part: &[0, 1],
        skin_max: &[1100.0, 1100.0],
        node_max: &[400.0, 400.0],
    };
    let hot = texel(Some((600.0, 1100.0)), false);
    let grey = texel(None, false);
    let n0 = texel(Some((250.0, 400.0)), false);
    let clear = [0u8; 4];
    let cut = |depth| Cut { toward: Vec3::Z, near: 2.0, far: -2.0, depth };
    // (measured parts, cut depth) → the four texels: cell 0, cell 1, node 0, node 1
    let table = [
        ([true, false], 0.0, [hot, grey, clear, clear]),
        ([false, false], 0.0, [grey, grey, clear, clear]),
        ([true, false], 0.5, [clear, grey, n0, clear]),
        ([true, true], 0.5, [clear, texel(Some((300.0, 1100.0)), false), n0, clear]),
        ([true, false], 1.0, [clear, clear, clear, clear]),
    ];
    let mut out = Vec::new();
    for (measured, depth, want) in table {
        let r = Reading { temps: &temps, measured: &measured };
        texels(&mesh, &r, Some(&cut(depth)), &mut out);
        let got: Vec<[u8; 4]> = out.chunks(4).take(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        assert_eq!(got, want.to_vec(), "{measured:?} {depth}");
    }
    assert_ne!(hot, grey);
    assert_eq!(hot[3], 255);
    // A part with no skin (an engine inside its interstage): its nodes show
    // with nothing cut, and the cut still takes them.
    let bare = ThermalMesh { skinless: vec![false, true], ..mesh };
    let r = Reading { temps: &temps, measured: &[true, true] };
    let mut node1 = |depth| {
        texels(&bare, &r, Some(&cut(depth)), &mut out);
        [out[12], out[13], out[14], out[15]]
    };
    assert_eq!(node1(0.0), texel(Some((280.0, 400.0)), false));
    assert_eq!(node1(0.5), clear);
}
