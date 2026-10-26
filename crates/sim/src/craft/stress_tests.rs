use super::*;
use crate::craft::file::{validate_craft, validate_geometry};

/// FNV-1a over every number and name of a layout (golden determinism).
fn layout_hash(l: &StressLayout) -> u64 {
    let mut bytes = Vec::new();
    let mut put = |x: f64| bytes.extend_from_slice(&x.to_bits().to_le_bytes());
    for p in &l.geometry.primitives {
        let v: Vec<f64> = match p.shape {
            Shape::Frustum { base, axis, r_base, r_top } => {
                [base, axis].iter().flat_map(|v| v.to_array()).chain([r_base, r_top]).collect()
            }
            Shape::Cylinder { base, axis, radius } => {
                [base, axis].iter().flat_map(|v| v.to_array()).chain([radius]).collect()
            }
            Shape::SphereCap { center, axis, radius, height } => {
                [center, axis].iter().flat_map(|v| v.to_array()).chain([radius, height]).collect()
            }
            Shape::Box { center, half } => [center, half].iter().flat_map(|v| v.to_array()).collect(),
            Shape::Strut { from, to, radius } => [from, to].iter().flat_map(|v| v.to_array()).chain([radius]).collect(),
        };
        let s = p.skin.expect("every stress primitive names its skin");
        v.into_iter().chain([s.areal_mass, s.specific_heat, s.max_k.unwrap_or(0.0)]).for_each(&mut put);
        put(f64::from(u8::from(p.foot) + 2 * u8::from(p.nozzle)));
    }
    for q in &l.parts {
        let e = q.engine.map_or([0.0; 3], |e| [e.thrust_vac, e.isp_vac, e.isp_sl]);
        q.com
            .to_array()
            .into_iter()
            .chain(e)
            .chain([q.dry_mass, q.propellant_capacity, q.heat_capacity])
            .for_each(&mut put);
        put(f64::from(q.parent.unwrap_or(u32::MAX)));
    }
    let s = &l.spec;
    [s.dry_mass, s.engine.thrust_vac, s.engine.isp_vac, s.engine.isp_sl, s.contact.foot.stiffness]
        .into_iter()
        .for_each(&mut put);
    crate::ephem::fnv1a64(&bytes)
}

/// Every count from 1 to 1000: exactly that many parts, valid craft files,
/// each primitive owned by one part, the parts a tree rooted at part 0, the
/// craft file's sums, engines sized for the lift-off ratio, and the dry
/// mass balanced on the axis (quarter-turn symmetry).
#[test]
fn every_count_lays_out_a_valid_craft_of_exactly_that_many_parts() {
    for n in 1..=1000 {
        let l = layout(n);
        assert_eq!(l.parts.len(), n);
        validate_craft(&l.spec).unwrap_or_else(|e| panic!("{n} parts: {e}"));
        validate_geometry(&l.geometry).unwrap_or_else(|e| panic!("{n} parts: {e}"));
        let owned: usize = l.parts.iter().map(|p| p.primitives.len()).sum();
        let owner = part_of_primitive(&l.parts, l.geometry.primitives.len());
        assert!(owned == l.geometry.primitives.len() && owner.iter().all(|&o| o != u32::MAX), "{n}: ownership");
        for (i, p) in l.parts.iter().enumerate() {
            assert_eq!(p.parent.is_none(), i == 0, "{n}: only part 0 is the root");
            let (mut at, mut steps) = (i, 0);
            while let Some(up) = l.parts[at].parent {
                at = up as usize;
                steps += 1;
                assert!(steps <= n, "{n}: a cycle through part {i}");
            }
            assert!(p.dry_mass > 0.0 && p.heat_capacity > 0.0 && p.max_temperature > 0.0, "{n}: part {i}");
        }
        let sum = |f: fn(&StressPart) -> f64| l.parts.iter().map(f).sum::<f64>();
        let (dry, capacity) = (sum(|p| p.dry_mass), sum(|p| p.propellant_capacity));
        assert!((dry / l.spec.dry_mass - 1.0).abs() < 1e-12, "{n}: dry mass");
        assert!((capacity / l.spec.propellant.capacity - 1.0).abs() < 1e-12, "{n}: capacity");
        let thrust = sum(|p| p.engine.map_or(0.0, |e| e.thrust_vac));
        if n >= 2 {
            assert!((thrust / l.spec.engine.thrust_vac - 1.0).abs() < 1e-12, "{n}: thrust");
            let twr = thrust / ((dry + capacity) * G0);
            assert!((twr / LIFTOFF_TWR - 1.0).abs() < 1e-12, "{n}: lift-off ratio {twr}");
        }
        let com = dry_com(&l.parts);
        assert!(com.truncate().length() < 1e-9 * (1.0 + com.z.abs()), "{n}: dry centre of mass {com}");
        let avionics = l.parts.iter().filter(|p| p.kind == StressKind::Avionics).count();
        assert!(avionics <= 3, "{n}: {avionics} avionics rings make up the count");
    }
}

#[test]
fn layouts_are_deterministic_and_grow_as_documented() {
    assert_eq!(layout(137), layout(137));
    // Golden: the same bits on every platform (libm trig, no fused ops).
    let h = layout_hash(&layout(300));
    assert_eq!(h, layout_hash(&layout(300)));
    assert_eq!(h, 0x5951_bac0_addd_6a15, "golden hash of the 300-part layout: {h:#x}");
    // The sizes the module documentation gives.
    let count = |l: &StressLayout, k: StressKind| l.parts.iter().filter(|p| p.kind == k).count();
    for (n, core_tanks, boosters, booster_tanks, smalls) in [(300, 8, 12, 6, 160), (1000, 18, 24, 14, 532)] {
        let l = layout(n);
        let core = l.parts.iter().filter(|p| p.kind == StressKind::Tank && p.com.truncate().length() < 1e-9).count();
        let small = [StressKind::Panel, StressKind::Rcs, StressKind::Battery, StressKind::Antenna];
        let engines = count(&l, StressKind::Engine);
        assert_eq!((core, engines - 1), (core_tanks, boosters), "{n}");
        assert_eq!(count(&l, StressKind::Tank) - core, boosters * booster_tanks, "{n}");
        assert_eq!(small.iter().map(|&k| count(&l, k)).sum::<usize>(), smalls, "{n}");
    }
}

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

/// Small craft through the whole pipeline (cells, mass model, the design's
/// bake and thermal network): one part, the full core (12 parts), and a
/// booster ring with small parts (64 parts, coarse tessellation to keep the
/// debug-mode test fast).
#[test]
fn stress_craft_build_and_bake_through_the_pipeline() {
    let coarse = Resolution { max_edge: 0.6, max_angle: 30.0 * math::PI / 180.0 };
    for (n, res) in [(1, Resolution::default()), (12, Resolution::default()), (64, coarse)] {
        let s = layout(n).build(&res, &cell_options(n));
        let c = &s.craft;
        let owner = part_of_primitive(&s.parts, c.geometry.primitives.len());
        let design = c.design();
        assert_eq!((design.bake.cells, design.network.cells()), (c.cells.cells.len(), c.cells.cells.len()), "{n}");
        assert!(design.nodes() > 0 && design.mean_drag_area > 0.0, "{n}");
        // No large cell is a tube whose normals cancel (a whole tank side;
        // thin struts are split into rings by the cell builder, the test
        // craft's legs too); every cell's limit is its part's.
        for (cell, k) in c.cells.cells.iter().zip(coherence(c)) {
            assert!(cell.area < 0.5 || k > 0.5, "{n} parts: a {} m² cell's normals cancel ({k})", cell.area);
        }
        for (cell, limit) in c.cells.cells.iter().zip(&design.skin_max) {
            assert_eq!(*limit, s.parts[owner[cell.primitive as usize] as usize].max_temperature, "{n}");
        }
        // The shell mass model: balanced on the axis up to the union's
        // trimming (struts are tessellated from different seams per lane),
        // its centre near the parts' (the skin carries a share of each
        // part's mass over its surface; hidden faces are removed).
        let m = &c.mass;
        let length = c.surface.positions.iter().map(|p| p.z).fold(f64::MIN, f64::max) - c.params().bottom_z;
        assert!(m.dry_com.truncate().length() < 1e-3 * length, "{n}: shell centre {}", m.dry_com);
        let off = (m.dry_com - dry_com(&s.parts)).length();
        assert!(off < 0.1 * length, "{n}: shell centre {} vs parts {} ({length} m)", m.dry_com, dry_com(&s.parts));
        // Feet: the legs' soles are the lowest points.
        let feet = c.cells.contacts.iter().filter(|p| p.kind == super::super::ContactKind::Foot).count();
        assert_eq!(feet, if n >= 12 { 4 } else { 0 }, "{n}");
        if n >= 12 {
            assert_eq!(c.params().bottom_z, FOOT_Z, "{n}");
        }
    }
}
