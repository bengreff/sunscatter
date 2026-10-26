use super::*;
use crate::craft::mesh::contains;
use crate::craft::{stress, test_craft};

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
    let defs = [test_craft().def(), stress::layout(40).def()];
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
    let craft = test_craft();
    let hull = Hull::new(&craft.def());
    for i in 0..hull.shapes.len() {
        for p in hull.points_of(i) {
            let d = signed_distance(&hull.shapes[i], p.pos).0;
            assert!(d.abs() < 1e-9, "primitive {i}: a point {d} m off its surface");
            assert!(!hull.grid.any_contains(&hull.shapes, p.pos + p.normal * PROBE, Some(i)));
        }
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
    assert!(hull.points.len() > 2_000 && hull.points.len() < 6_000, "{} points", hull.points.len());
    // The bounding sphere holds every solid.
    for shape in &hull.shapes {
        let (lo, hi) = bounds(shape);
        assert!((lo - hull.center).length() <= hull.radius && (hi - hull.center).length() <= hull.radius);
    }
}

/// A point inside the union leaves through the outer surface: near the
/// body's top (where the cabin sits on it) sideways, not up into the
/// cabin; in a fin, through its nearer side; outside, nothing.
#[test]
fn penetration_leaves_through_the_outer_surface() {
    let hull = Hull::new(&test_craft().def());
    // (point, depth, normal)
    let cases = [
        // 4 cm inside the body's side, 1 cm below the cabin: out the side.
        (DVec3::new(1.0, 0.0, 3.99), 0.04, DVec3::X),
        // Under the skin of the body at mid-height.
        (DVec3::new(0.0, -1.03, 0.0), 0.01, -DVec3::Y),
        // In fin +X beyond the body: out its nearer side face.
        (DVec3::new(2.0, 0.03, -4.6), 0.01, DVec3::Y),
        // 1 cm into the nose cap along the axis.
        (DVec3::new(0.0, 0.0, 6.786), 0.01, DVec3::Z),
    ];
    for (p, depth, n) in cases {
        let (d, normal, _) = hull.penetration(p).unwrap_or_else(|| panic!("{p} not inside"));
        assert!((d - depth).abs() < 1e-9 && (normal - n).length() < 1e-9, "{p}: {d} along {normal}");
    }
    for p in [DVec3::new(1.1, 0.0, 0.0), DVec3::new(0.0, 0.0, 6.8), DVec3::new(0.0, 0.0, -20.0), DVec3::splat(1e6)] {
        assert_eq!(hull.penetration(p), None, "{p}");
    }
}

/// The stress craft's hulls: points scale with the surface, and each
/// primitive's points sit inside its sphere.
#[test]
fn stress_craft_hulls() {
    for n in [1, 50, 300] {
        let def = stress::layout(n).def();
        let t = std::time::Instant::now();
        let hull = Hull::new(&def);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        eprintln!("stress-{n}: {} primitives, {} points, built in {ms:.0} ms", hull.shapes.len(), hull.points.len());
        assert_eq!(hull.part.len(), hull.shapes.len());
        assert!(hull.part.iter().all(|&p| (p as usize) < def.parts.len()));
        for i in 0..hull.shapes.len() {
            let (c, r) = hull.spheres[i];
            assert!(hull.points_of(i).iter().all(|p| (p.pos - c).length() <= r));
        }
    }
}
