use super::*;
use crate::craft::file::validate_craft;

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
        let e = q
            .engine
            .map_or([0.0; 6], |e| [e.thrust_vac, e.isp_vac, e.isp_sl, e.mount.pos.x, e.mount.pos.y, e.mount.pos.z]);
        let tank = q.tank.map_or(0.0, |t| t.capacity);
        let power = [q.power.produced, q.power.consumed, q.power.battery];
        e.into_iter()
            .chain([q.dry_mass, tank, q.interior_specific_heat.unwrap_or(0.0), q.node_max_k.unwrap_or(0.0)])
            .chain(power)
            .chain([q.max_load, f64::from(q.crew), f64::from(u8::from(q.control) + 2 * u8::from(q.chute))])
            .for_each(&mut put);
        put(f64::from(q.parent.unwrap_or(u32::MAX)));
    }
    let s = &l.spec;
    [s.dry_mass, s.engine.thrust_vac, s.engine.isp_vac, s.engine.isp_sl, s.contact.foot.stiffness]
        .into_iter()
        .for_each(&mut put);
    crate::ephem::fnv1a64(&bytes)
}

/// Every count from 1 to 1000: exactly that many parts, a valid definition
/// (each primitive owned by one part, the parts a tree rooted at part 0),
/// the craft file's aggregates, engines sized for the lift-off ratio, and
/// the dry mass balanced on the axis (quarter-turn symmetry).
#[test]
fn every_count_lays_out_a_valid_craft_of_exactly_that_many_parts() {
    for n in 1..=1000 {
        let l = layout(n);
        assert_eq!((l.parts.len(), l.kinds.len()), (n, n));
        let def = l.def();
        super::super::file::validate_shapes(&l.geometry).unwrap_or_else(|e| panic!("{n} parts: {e}"));
        def.validate().unwrap_or_else(|e| panic!("{n} parts: {e}"));
        if n >= 2 {
            validate_craft(&l.spec).unwrap_or_else(|e| panic!("{n} parts: {e}"));
        }
        for (i, p) in l.parts.iter().enumerate() {
            assert_eq!(p.parent.is_none(), i == 0, "{n}: only part 0 is the root");
            assert_eq!(p.id, i as u32, "{n}");
        }
        let sum = |f: fn(&Part) -> f64| l.parts.iter().map(f).sum::<f64>();
        let (dry, capacity) = (sum(|p| p.dry_mass), sum(|p| p.tank.map_or(0.0, |t| t.capacity)));
        assert!((dry / l.spec.dry_mass - 1.0).abs() < 1e-12, "{n}: dry mass");
        assert!((capacity / l.spec.propellant.capacity - 1.0).abs() < 1e-12, "{n}: capacity");
        let thrust = sum(|p| p.engine.map_or(0.0, |e| e.thrust_vac));
        if n >= 2 {
            assert!((thrust / l.spec.engine.thrust_vac - 1.0).abs() < 1e-12, "{n}: thrust");
            let twr = thrust / ((dry + capacity) * G0);
            assert!((twr / LIFTOFF_TWR - 1.0).abs() < 1e-12, "{n}: lift-off ratio {twr}");
        }
        let com = volume_com(&def);
        assert!(com.truncate().length() < 1e-9 * (1.0 + com.z.abs()), "{n}: dry centre of mass {com}");
        let avionics = l.kinds.iter().filter(|&&k| k == StressKind::Avionics).count();
        assert!(avionics <= 3, "{n}: {avionics} avionics rings make up the count");
        // One part carries the thrusters and parachutes; the pod is crewed.
        assert_eq!(l.parts.iter().filter(|p| p.rcs.is_some() && p.chute).count(), 1, "{n}");
        assert_eq!(l.spec.crew, if n >= 3 { 3 } else { 0 }, "{n}");
    }
}

#[test]
fn layouts_are_deterministic_and_grow_as_documented() {
    assert_eq!(layout(137), layout(137));
    // Golden: the same bits on every platform (libm trig, no fused ops).
    let h = layout_hash(&layout(300));
    assert_eq!(h, layout_hash(&layout(300)));
    assert_eq!(h, 0x60ea_2023_f279_24be, "golden hash of the 300-part layout: {h:#x}");
    // The sizes the module documentation gives.
    let count = |l: &StressLayout, k: StressKind| l.kinds.iter().filter(|&&x| x == k).count();
    for (n, core_tanks, boosters, booster_tanks, smalls) in [(300, 8, 12, 6, 160), (1000, 18, 24, 14, 532)] {
        let l = layout(n);
        let core = (l.parts.iter().zip(&l.kinds))
            .filter(|(p, &k)| k == StressKind::Tank && p.tank.unwrap().region.base.truncate().length() < 1e-9)
            .count();
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
        let owner = c.def().primitive_part();
        let design = c.design();
        assert_eq!((design.bake.cells, design.network.cells()), (c.cells.cells.len(), c.cells.cells.len()), "{n}");
        assert!(design.nodes() > 0 && design.mean_drag_area > 0.0, "{n}");
        // No large cell is a tube whose normals cancel (a whole tank side;
        // thin struts are split into rings by the cell builder, the test
        // craft's legs too); every cell's limit is its part's.
        for (cell, k) in c.cells.cells.iter().zip(coherence(c)) {
            assert!(cell.area < 0.5 || k > 0.5, "{n} parts: a {} m² cell's normals cancel ({k})", cell.area);
        }
        let limit = |p: usize| c.geometry.primitives[c.parts[p].primitives[0] as usize].skin.unwrap().max_k.unwrap();
        for (i, (cell, max)) in c.cells.cells.iter().zip(&design.skin_max).enumerate() {
            assert_eq!(design.cell_part[i], owner[cell.primitive as usize], "{n}");
            assert_eq!(*max, limit(design.cell_part[i] as usize), "{n}");
        }
        // Every node is tagged with a part that fills it; per-part limits.
        for (j, node) in design.volume.nodes.iter().enumerate() {
            let p = design.node_part[j];
            assert!(node.parts.iter().any(|&(q, _)| q == p), "{n}: node {j}");
            let want = c.parts[p as usize].node_max_k.unwrap_or(c.spec.thermal.internal_max_k);
            assert_eq!(design.node_max[j], want, "{n}");
        }
        // The mass model sums the parts' shells: balanced on the axis up to
        // the tessellation (struts are tessellated from different seams per
        // lane), its centre near the volume centroids' (shells over each
        // part's own surface).
        let m = &c.mass;
        let length = c.surface.positions.iter().map(|p| p.z).fold(f64::MIN, f64::max) - c.params().bottom_z;
        assert!(m.dry_com.truncate().length() < 1e-3 * length, "{n}: shell centre {}", m.dry_com);
        let off = (m.dry_com - volume_com(&c.def())).length();
        assert!(off < 0.05 * length, "{n}: shell centre {} vs volume {} ({length} m)", m.dry_com, volume_com(&c.def()));
        assert_eq!(c.part_mass.len(), n);
        // Each part's interior (dry mass less skin) heat capacity is in the
        // nodes, thin parts' under their cells.
        let mut want = 0.0;
        for (i, p) in c.parts.iter().enumerate() {
            let skin: f64 = (c.cells.cells.iter().zip(&design.cell_part))
                .filter(|(_, &q)| q as usize == i)
                .map(|(cell, _)| cell.skin_mass())
                .sum();
            let heat = p.interior_specific_heat.unwrap_or(c.spec.thermal.interior_specific_heat);
            want += (p.dry_mass - skin).max(0.0) * heat;
        }
        let got: f64 = design.dry_capacity.iter().sum();
        assert!((got / want - 1.0).abs() < 1e-9, "{n}: interior heat capacity {got} vs {want}");
        // Full, every tank holds its capacity: the heat capacity of the
        // propellant in the nodes is the capacity's.
        let mut cap = vec![0.0; design.nodes()];
        design.volume.propellant(c.spec.propellant.capacity, 1.0, &mut cap);
        let total: f64 = cap.iter().sum();
        assert!((total / c.spec.propellant.capacity - 1.0).abs() < 1e-9, "{n}: {total}");
        // Feet: the legs' soles are the lowest points.
        let feet = c.cells.contacts.iter().filter(|p| p.kind == super::super::ContactKind::Foot).count();
        assert_eq!(feet, if n >= 12 { 4 } else { 0 }, "{n}");
        if n >= 12 {
            assert_eq!(c.params().bottom_z, FOOT_Z, "{n}");
        }
    }
}
