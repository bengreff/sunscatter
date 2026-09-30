use super::super::file::Primitive;
use super::super::mesh::{analytic_volume, contains, union_surface, Resolution};
use super::super::{test_craft, Shape};
use super::*;
use std::collections::VecDeque;

fn craft_cells(target: usize) -> (Surface, Cells) {
    let g = &test_craft().geometry;
    let s = union_surface(&g.primitives, &Resolution::default());
    let cells = build_cells(&s, &g.primitives, &g.skin, &CellOptions { target, ..CellOptions::default() });
    (s, cells)
}

#[test]
fn cell_count_respects_the_target() {
    for target in [64, 200, 512, 1024] {
        let (_, c) = craft_cells(target);
        assert!(c.cells.len() <= target, "{} cells for target {target}", c.cells.len());
        assert!(c.cells.len() == target, "{} cells for target {target}: the target is reachable", c.cells.len());
    }
}

#[test]
fn cells_cover_the_surface_and_it_is_closed_with_outward_normals() {
    let (s, c) = craft_cells(512);
    let total: f64 = c.cells.iter().map(|c| c.area).sum();
    assert!((total / s.area() - 1.0).abs() < 1e-12);
    // Closed: the vector area of a closed surface is zero (the union's
    // joints are jagged at the triangle scale, hence the tolerance).
    let vector: DVec3 = c.cells.iter().map(|c| c.normal * c.area).sum();
    assert!(vector.length() < 5e-3 * total, "Σ n·a = {vector} for area {total}");
    // Outward: by the divergence theorem ⅓ Σ (c·n) a is the enclosed
    // volume, positive and close to the union's (the capsule stack, the
    // parts of the bell, legs and feet outside it).
    let volume: f64 = (0..s.triangles.len())
        .map(|t| {
            let (c, n, a) = s.triangle(t);
            c.dot(n) * a / 3.0
        })
        .sum();
    let prims = &test_craft().geometry.primitives;
    let v = |name: &str| analytic_volume(&prims.iter().find(|p| p.name == name).unwrap().shape);
    let pi = std::f64::consts::PI;
    // The union, by hand: the skirt (z −6 to −4.5), the body above it
    // (π·1.04²·8.5), the cabin cone, the nose cap beyond the cone's end
    // (0.396 of its 0.5 m sphere), the bell below the skirt (r 0.8 → 0.391
    // over 1 m), the feet and the fins beyond the skirt's widest radius
    // (1.28 m of each 2 m span). At most: plus the legs, the fins' remaining parts and
    // the whole nose cap.
    let frustum = |h: f64, a: f64, b: f64| pi * h * (a * a + a * b + b * b) / 3.0;
    let feet: f64 = prims.iter().filter(|p| p.foot).map(|p| analytic_volume(&p.shape)).sum();
    let legs: f64 = prims.iter().filter(|p| p.name.starts_with("leg")).map(|p| analytic_volume(&p.shape)).sum();
    let fins: f64 = prims.iter().filter(|p| p.name.starts_with("fin")).map(|p| analytic_volume(&p.shape)).sum();
    let fins_out = fins * 1.28 / 2.0;
    let nose_out = pi * 0.396 * 0.396 * (1.5 - 0.396) / 3.0;
    let lo = v("skirt") + pi * 1.04 * 1.04 * 8.5 + v("cabin") + nose_out + frustum(1.0, 0.8, 0.391) + feet + fins_out;
    let hi = lo + legs + (fins - fins_out) + v("nose");
    assert!(volume > 0.99 * lo && volume < 1.01 * hi, "volume {volume}, expected {lo}..{hi}");
    // Each triangle faces out of its own primitive: just inside it along −n,
    // outside everything along +n (away from the joints).
    let mut wrong = 0.0;
    for t in 0..s.triangles.len() {
        let (ctr, n, a) = s.triangle(t);
        let own = &prims[s.patches[s.tri_patch[t] as usize].primitive as usize].shape;
        let outside = !prims.iter().any(|p| contains(&p.shape, ctr + n * 5e-3));
        if !(contains(own, ctr - n * 5e-3) && outside) {
            wrong += a;
        }
    }
    assert!(wrong < 2e-3 * total, "{wrong} m² of {total} face inward or sit in a joint");
}

#[test]
fn cells_are_smaller_where_the_surface_curves() {
    let (_, c) = craft_cells(512);
    let prims = &test_craft().geometry.primitives;
    let mean_area = |name: &str| {
        let p = prims.iter().position(|p| p.name == name).unwrap() as u32;
        let v: Vec<f64> = c.cells.iter().filter(|c| c.primitive == p).map(|c| c.area).collect();
        v.iter().sum::<f64>() / v.len() as f64
    };
    // Against the flat fins; the body (radius 1 m) curves too.
    let flat = mean_area("fin +X");
    for (name, ratio) in [("body", 0.9), ("nose", 0.6), ("foot +X", 0.6), ("leg +Y", 0.4), ("engine bell", 0.9)] {
        println!("{name}: {:.3} m² (fin {flat:.3} m²)", mean_area(name));
        assert!(mean_area(name) < ratio * flat, "{name}: {} m² vs fin {flat} m²", mean_area(name));
    }
    assert!(mean_area("nose") < mean_area("body"));
}

#[test]
fn neighbours_are_symmetric_and_connect_every_cell() {
    let (_, c) = craft_cells(512);
    for (i, cell) in c.cells.iter().enumerate() {
        assert!(!cell.neighbours.is_empty(), "cell {i} has no neighbour");
        for n in &cell.neighbours {
            assert!(n.conductance > 0.0 && n.conductance.is_finite());
            let back = c.cells[n.cell as usize].neighbours.iter().find(|m| m.cell == i as u32);
            assert_eq!(back.map(|m| m.conductance), Some(n.conductance), "{i} ↔ {}", n.cell);
        }
    }
    let mut seen = vec![false; c.cells.len()];
    let mut queue = VecDeque::from([0usize]);
    seen[0] = true;
    while let Some(i) = queue.pop_front() {
        for n in &c.cells[i].neighbours {
            if !std::mem::replace(&mut seen[n.cell as usize], true) {
                queue.push_back(n.cell as usize);
            }
        }
    }
    assert!(seen.iter().all(|&s| s), "{} cells unreachable", seen.iter().filter(|&&s| !s).count());
}

#[test]
fn contact_points_are_the_feet_and_the_hull() {
    let (_, c) = craft_cells(512);
    let prims = &test_craft().geometry.primitives;
    let feet: Vec<_> = c.contacts.iter().filter(|p| p.kind == ContactKind::Foot).collect();
    assert_eq!(feet.len(), 4);
    for f in &feet {
        assert_eq!(f.pos.z, -7.7);
        assert!(prims[c.cells[f.cell as usize].primitive as usize].foot);
    }
    let hull: Vec<_> = c.contacts.iter().filter(|p| p.kind == ContactKind::Hull).collect();
    assert!(hull.len() > 20, "{} hull points", hull.len());
    // The nose tip (6.8 m), the skirt's rim and the fin tips are on the hull.
    let top = hull.iter().map(|p| p.pos.z).fold(f64::MIN, f64::max);
    assert!(top > 6.5, "highest hull point at {top} m");
    assert!(hull.iter().any(|p| p.pos.truncate().length() > 1.3 && p.pos.z < -5.5), "no skirt rim point");
    assert!(hull.iter().any(|p| p.pos.truncate().length() > 2.6), "no fin tip point");
    // Every contact point's cell is flagged, and only those.
    let flagged = c.cells.iter().filter(|c| c.contact).count();
    assert_eq!(flagged, c.contacts.len());
}

/// A fixed shape (independent of the craft data), for cross-platform
/// determinism: CI runs this on macOS (ARM64) and Windows (x86-64).
#[test]
fn cells_match_golden_hash() {
    let prim = |shape, foot| Primitive { name: String::new(), shape, foot, skin: None, nozzle: false };
    let prims = [
        prim(Shape::Cylinder { base: DVec3::ZERO, axis: DVec3::Z * 2.0, radius: 1.0 }, false),
        prim(Shape::SphereCap { center: DVec3::Z * 1.5, axis: DVec3::Z, radius: 1.2, height: 1.0 }, false),
        prim(Shape::Strut { from: DVec3::new(0.8, 0.0, 0.5), to: DVec3::new(1.6, 0.3, -0.8), radius: 0.05 }, false),
        prim(Shape::Box { center: DVec3::new(1.6, 0.3, -0.85), half: DVec3::new(0.2, 0.2, 0.05) }, true),
    ];
    let skin = test_craft().geometry.skin;
    let build = || {
        let s = union_surface(&prims, &Resolution::default());
        build_cells(&s, &prims, &skin, &CellOptions { target: 128, ..CellOptions::default() })
    };
    let (a, b) = (build(), build());
    assert_eq!(a, b);
    let hash = cells_hash(&a);
    println!("cells golden hash: {hash:#018x}");
    assert_eq!(hash, CELLS_GOLDEN, "cells changed (or differ on this platform)");
}

const CELLS_GOLDEN: u64 = 0xc0153423964cc021;

#[test]
fn render_mesh_indexes_its_vertices() {
    let s = union_surface(&test_craft().geometry.primitives, &Resolution::default());
    let m = s.render_mesh();
    assert_eq!(m.indices.len(), 3 * s.triangles.len());
    assert_eq!(m.positions.len(), m.normals.len());
    assert!(m.indices.iter().all(|&i| (i as usize) < m.positions.len()));
    assert!(m.normals.iter().all(|n| (glam::Vec3::from(*n).length() - 1.0).abs() < 1e-5));
    let used = m.positions.len();
    assert!(used < s.positions.len(), "vertices of removed triangles are dropped");
}

#[test]
#[ignore = "timing; run with --ignored --nocapture"]
fn build_time() {
    let g = &test_craft().geometry;
    let t0 = std::time::Instant::now();
    let s = union_surface(&g.primitives, &Resolution::default());
    let t1 = std::time::Instant::now();
    let c = build_cells(&s, &g.primitives, &g.skin, &CellOptions::default());
    let t2 = std::time::Instant::now();
    println!(
        "{} triangles in {:?}, {} cells in {:?}, {} contact points",
        s.triangles.len(),
        t1 - t0,
        c.cells.len(),
        t2 - t1,
        c.contacts.len()
    );
}
