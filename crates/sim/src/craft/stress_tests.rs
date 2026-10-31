use super::*;

/// Coherence of a cell: |Σ n·a| / Σ a over its triangles (1 flat, 0 a
/// closed tube whose normals cancel).
fn coherence(c: &Craft) -> Vec<f64> {
    let mut sums = vec![(DVec3::ZERO, 0.0); c.cells.cells.len()];
    for t in 0..c.surface.triangles.len() {
        let (_, normal, area) = c.surface.triangle(t);
        let s = &mut sums[c.cells.tri_cell[t] as usize];
        *s = (s.0 + normal * area, s.1 + area);
    }
    sums.iter().map(|(v, a)| v.length() / a).collect()
}

/// The stress craft of catalog parts through the whole pipeline (cells,
/// mass model, the design's bake and thermal network): its core (12 parts)
/// and a ring of boosters (40).
#[test]
fn stress_craft_build_and_bake_through_the_pipeline() {
    for n in [12, 40] {
        let c = &*craft(n);
        let owner = c.def().primitive_part();
        let design = c.design();
        assert_eq!((design.bake.cells, design.network.cells()), (c.cells.cells.len(), c.cells.cells.len()), "{n}");
        assert!(design.nodes() > 0 && design.mean_drag_area > 0.0, "{n}");
        // No large cell is a tube whose normals cancel (a whole tank side);
        // every cell belongs to its primitive's part.
        for (cell, k) in c.cells.cells.iter().zip(coherence(c)) {
            assert!(cell.area < 0.5 || k > 0.5, "{n} parts: a {} m² cell's normals cancel ({k})", cell.area);
        }
        for (i, cell) in c.cells.cells.iter().enumerate() {
            assert_eq!(design.cell_part[i], owner[cell.primitive as usize], "{n}");
        }
        // Every node is tagged with a part that fills it.
        for (j, node) in design.volume.nodes.iter().enumerate() {
            let p = design.node_part[j];
            assert!(node.parts.iter().any(|&(q, _)| q == p), "{n}: node {j}");
        }
        // The mass model sums the parts' shells and components: near the
        // axis (the capsule's equipment is off it), its centre near the
        // volume centroids'.
        let m = &c.mass;
        let length = c.surface.positions.iter().map(|p| p.z).fold(f64::MIN, f64::max) - c.params().bottom_z;
        assert!(m.dry_com.truncate().length() < 1e-2 * length, "{n}: shell centre {}", m.dry_com);
        let off = (m.dry_com - volume_com(&c.def())).length();
        assert!(off < 0.1 * length, "{n}: shell centre {} vs volume {} ({length} m)", m.dry_com, volume_com(&c.def()));
        assert_eq!(c.part_mass.len(), n);
    }
}

/// Parts a plan spends: the capsule, the core's tanks, its thrust
/// structure and engines, its legs, and every booster.
fn spent(p: &super::PartsPlan) -> usize {
    1 + p.core_tanks
        + 5 * usize::from(p.engines)
        + 4 * usize::from(p.legs)
        + p.boosters.iter().map(|t| 6 + t).sum::<usize>()
}

#[test]
fn the_plan_of_catalog_parts_spends_exactly_n() {
    use super::parts_plan as plan;
    // (n, core tanks, rings, tanks of the first and last booster)
    let table = [
        (1, 0, 0, None),
        (6, 5, 0, None),
        (7, 1, 0, None),
        (10, 4, 0, None),
        (12, 2, 0, None),
        (40, 2, 1, Some((1, 1))),
        (300, 2, 4, Some((2, 1))),
        (1_000, 3, 7, Some((3, 2))),
        (5_000, 10, 12, Some((10, 9))),
    ];
    for (n, core, rings, first_last) in table {
        let p = plan(n);
        assert_eq!(spent(&p), n, "{n}: {p:?}");
        assert_eq!((p.core_tanks, p.rings, p.boosters.len()), (core, rings, 2 * rings * (rings + 1)), "{n}");
        assert_eq!(p.boosters.first().zip(p.boosters.last()).map(|(a, b)| (*a, *b)), first_last, "{n}");
    }
    for n in 1..=700 {
        assert_eq!(spent(&plan(n)), n, "{n}");
    }
    assert_eq!(super::file(137), super::file(137));
}

#[test]
fn a_stress_craft_of_catalog_parts_builds_with_its_stacks_and_sequence() {
    let c = super::craft(40);
    let ship = c.ship.clone().expect("a craft of parts");
    assert_eq!((c.parts.len(), c.spec.crew), (40, 1));
    // The core's four engines and the four boosters' sixteen, each fed.
    assert_eq!(ship.engines.len(), 20);
    let names: Vec<&str> = ship.sequence.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Ignition", "Boosters", "Separation"]);
    // Quarter-turn symmetric: the boosters' noses at 2.3 m, 90° apart.
    let noses: Vec<DVec3> =
        (c.parts.iter().zip(&c.part_mass)).filter(|(p, _)| p.name.ends_with(".nose")).map(|(_, m)| m.com).collect();
    assert_eq!(noses.len(), 4);
    for w in noses.windows(2) {
        assert!(
            (w[0].truncate().length() - w[1].truncate().length()).abs() < 1e-9 && (w[0].z - w[1].z).abs() < 1e-9,
            "{noses:?}"
        );
    }
    // Its pieces resolve from their ids (a booster alone).
    let nose = c.parts.iter().find(|p| p.name == "b1.nose").unwrap().id;
    assert_eq!(crate::craft::craft_of(&CraftId::piece("stress-40", &[nose])).parts.len(), 1);
}
