use super::*;
use crate::craft::mesh::contains;
use crate::craft::{small, stress};

fn shapes_of(def: &CraftDef) -> Vec<Shape> {
    def.geometry.primitives.iter().map(|p| p.shape).collect()
}

/// For every primitive of the test craft and a stress craft, at points
/// through and around it: the sign agrees with the inside test, the
/// gradient leads back to the surface, and the distance is the least to
/// the surface (no sample of the surface is closer, and one is within the
/// samples' spacing).
#[test]
fn signed_distance_is_the_distance_to_each_solid() {
    let defs = [small::rocket_ref().def(), stress::def(40)];
    let mut checked = 0;
    for def in &defs {
        for shape in shapes_of(def) {
            let surface = sample(&shape);
            let (lo, hi) = bounds(&shape);
            let (lo, hi) = (lo - DVec3::splat(0.3), hi + DVec3::splat(0.3));
            let n = 9;
            for i in 0..n {
                for j in 0..n {
                    for k in 0..n {
                        let f = DVec3::new(i as f64, j as f64, k as f64) / (n - 1) as f64;
                        let p = lo + (hi - lo) * f + DVec3::new(1e-7, 2e-7, 3e-7);
                        let (d, g) = signed_distance(&shape, p);
                        assert!((g.length() - 1.0).abs() < 1e-9, "{shape:?} at {p}: gradient {g}");
                        if d.abs() > 1e-9 {
                            assert_eq!(d < 0.0, contains(&shape, p), "{shape:?} at {p}: {d}");
                        }
                        let back = signed_distance(&shape, p - g * d).0;
                        assert!(back.abs() < 1e-9, "{shape:?} at {p}: {d} along {g} lands {back} off");
                        let nearest = surface.iter().map(|s| (s.pos - p).length()).fold(f64::MAX, f64::min);
                        assert!(d.abs() <= nearest + 1e-9, "{shape:?} at {p}: {d} but a sample at {nearest}");
                        assert!(nearest <= d.abs() + 0.75 * SPACING, "{shape:?} at {p}: {d}, samples at {nearest}");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 20_000, "{checked}");
}

/// The sample points lie on their primitives' surfaces and on the union's
/// outer surface, cover it to within the spacing (every triangle of the
/// craft's union surface has one near), and their areas add up to it.
#[test]
fn sample_points_cover_the_union_surface() {
    let craft = small::rocket_ref();
    let hull = Hull::new(&craft.def());
    for p in &hull.points {
        let i = p.primitive as usize;
        let d = signed_distance(&hull.shapes[i], p.pos).0;
        assert!(d.abs() < 1e-9, "primitive {i}: a point {d} m off its surface");
        assert!(!hull.grid.any_contains(&hull.shapes, p.pos + p.normal * PROBE, Some(i)));
    }
    let s = &craft.surface;
    let mut worst: f64 = 0.0;
    for t in 0..s.triangles.len() {
        let (c, _, _) = s.triangle(t);
        let near = hull.points.iter().map(|p| (p.pos - c).length()).fold(f64::MAX, f64::min);
        worst = worst.max(near);
    }
    assert!(worst < SPACING, "a surface triangle {worst} m from every sample point");
    let area: f64 = hull.points.iter().map(|p| p.area).sum();
    assert!((area / s.area() - 1.0).abs() < 0.1, "points' area {area} m² against the surface's {}", s.area());
    assert!(hull.points.len() > 1_000 && hull.points.len() < 4_000, "{} points", hull.points.len());
    // The bounding sphere holds every solid.
    for shape in &hull.shapes {
        let (lo, hi) = bounds(shape);
        assert!((lo - hull.center).length() <= hull.radius && (hi - hull.center).length() <= hull.radius);
    }
}

/// A point inside the union leaves through the outer surface: near the
/// tank's top (where the capsule sits on it) sideways, not up into the
/// capsule; in a stowed leg beyond the tank, through its outer side;
/// outside, nothing.
#[test]
fn penetration_leaves_through_the_outer_surface() {
    let hull = Hull::new(&small::rocket_ref().def());
    // (point, depth, normal)
    let cases = [
        // 4 cm inside the tank's side, 1 cm below the capsule: out the side.
        (DVec3::new(0.96, 0.0, -0.01), 0.04, DVec3::X),
        // Under the tank's skin, between the legs.
        (DVec3::new(0.0, -0.99, -1.0), 0.01, -DVec3::Y),
        // In leg 1 (a strut of radius 0.0875 m on x = 1.075 m) beyond the
        // tank: out its outer side, not through the tank.
        (DVec3::new(1.12, 0.0, -3.5), 0.0425, DVec3::X),
        // 1 cm into the capsule's flat top along the axis.
        (DVec3::new(0.0, 0.0, 1.49), 0.01, DVec3::Z),
    ];
    for (p, depth, n) in cases {
        let (d, normal, _) = hull.penetration(p).unwrap_or_else(|| panic!("{p} not inside"));
        assert!((d - depth).abs() < 1e-9 && (normal - n).length() < 1e-9, "{p}: {d} along {normal}");
    }
    for p in [DVec3::new(1.1, 0.0, 0.0), DVec3::new(0.0, 0.0, 1.51), DVec3::new(0.0, 0.0, -20.0), DVec3::splat(1e6)] {
        assert_eq!(hull.penetration(p), None, "{p}");
    }
}

/// The stress craft's hulls: points scale with the surface, and the tree
/// holds every point once, each node's sphere holding its points.
#[test]
fn stress_craft_hulls() {
    for n in [1, 50, 300] {
        let def = stress::def(n);
        let t = std::time::Instant::now();
        let hull = Hull::new(&def);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        eprintln!("stress-{n}: {} primitives, {} points, built in {ms:.0} ms", hull.shapes.len(), hull.points.len());
        assert_eq!(hull.part.len(), hull.shapes.len());
        assert!(hull.part.iter().all(|&p| (p as usize) < def.parts.len()));
        let mut seen = vec![0u32; hull.points.len()];
        let mut stack = vec![(0usize, None::<(DVec3, f64)>)];
        while let Some((k, parent)) = stack.pop() {
            let node = hull.tree[k];
            if node.count == 0 {
                stack.push((node.first as usize, Some((node.center, node.radius))));
                stack.push((node.first as usize + 1, Some((node.center, node.radius))));
            } else {
                for j in node.first..node.first + node.count {
                    seen[j as usize] += 1;
                    let p = hull.points[j as usize].pos;
                    assert!((p - node.center).length() <= node.radius);
                    assert!(parent.is_some_and(|(c, r)| (p - c).length() <= r) || k == 0);
                }
            }
        }
        assert!(seen.iter().all(|&s| s == 1), "every point in one leaf");
    }
}
