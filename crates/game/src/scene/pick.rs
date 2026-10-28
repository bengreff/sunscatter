//! Rays against craft (the one owner): which part of a craft a ray from
//! the camera first meets, and how far along it (D111: clicking a part in
//! flight opens its card; the systems screen's picking; the sun flare
//! hidden behind craft, D110). Each part's mesh is in the craft's body axes
//! with its own transform there (a mechanism's hinge), and the craft is
//! placed in the view by its root transform; the ray is taken into each
//! part's axes once, never the vertices out of them. Pure, with table
//! tests (rule 8). Also here: when a press and release of the mouse are a
//! click, not a camera drag ([`is_click`]).

use bevy::math::{Vec2, Vec3};
use bevy::prelude::Transform;
use sim::craft::RenderMesh;

/// The nearest hit of a ray (origin, unit direction) with a triangle
/// (Möller–Trumbore, either face): the distance along the ray.
fn triangle(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    let t = e2.dot(q) * inv;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0).then_some(t)
}

/// The nearest hit of a ray with a mesh in the ray's axes.
pub fn mesh_hit(o: Vec3, d: Vec3, mesh: &RenderMesh) -> Option<f32> {
    let at = |i: u32| Vec3::from_array(mesh.positions[i as usize]);
    (mesh.indices.as_chunks::<3>().0.iter())
        .filter_map(|t| triangle(o, d, at(t[0]), at(t[1]), at(t[2])))
        .min_by(f32::total_cmp)
}

/// The part a ray (origin, unit direction, in the view's axes) first meets
/// and the distance along the ray: the craft placed by `placement` (its
/// body axes into the view's), each part's mesh with its transform in the
/// body axes (`parts`). Rigid transforms keep distances.
pub fn ray_hit(
    origin: Vec3,
    dir: Vec3,
    placement: Transform,
    parts: &[(&RenderMesh, Transform)],
) -> Option<(usize, f32)> {
    let body = placement.compute_affine().inverse();
    let (o, d) = (body.transform_point3(origin), body.transform_vector3(dir));
    let mut best: Option<(usize, f32)> = None;
    for (k, (mesh, t)) in parts.iter().enumerate() {
        let own = t.compute_affine().inverse();
        if let Some(hit) = mesh_hit(own.transform_point3(o), own.transform_vector3(d), mesh) {
            if best.is_none_or(|(_, b)| hit < b) {
                best = Some((k, hit));
            }
        }
    }
    best
}

/// Most a click's pointer moves (logical px) and lasts (s); beyond either
/// it was a drag of the camera.
const CLICK_PX: f32 = 5.0;
const CLICK_S: f64 = 0.35;

/// Whether a press at (`t0`, `p0`) and its release at (`t1`, `p1`) are a
/// click.
pub fn is_click(t0: f64, p0: Vec2, t1: f64, p1: Vec2) -> bool {
    t1 - t0 <= CLICK_S && p0.distance(p1) <= CLICK_PX
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Quat;

    /// A unit square facing +Z at height `z`.
    fn square(z: f32) -> RenderMesh {
        RenderMesh {
            positions: vec![[-1.0, -1.0, z], [1.0, -1.0, z], [1.0, 1.0, z], [-1.0, 1.0, z]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    #[test]
    fn a_ray_meets_the_nearest_part() {
        let (low, high) = (square(0.0), square(2.0));
        let parts = [(&low, Transform::IDENTITY), (&high, Transform::from_xyz(1.5, 0.0, 0.0))];
        let down = -Vec3::Z;
        // (origin, hit)
        let table = [
            (Vec3::new(0.0, 0.0, 5.0), Some((0, 5.0))),
            (Vec3::new(1.0, 0.0, 5.0), Some((1, 3.0))),
            (Vec3::new(2.4, 0.0, 5.0), Some((1, 3.0))),
            (Vec3::new(3.0, 0.0, 5.0), None),
            (Vec3::new(0.0, 0.0, -5.0), None),
        ];
        for (origin, want) in table {
            assert_eq!(ray_hit(origin, down, Transform::IDENTITY, &parts), want, "{origin}");
        }
    }

    #[test]
    fn the_craft_is_placed_and_turned_in_the_view() {
        let low = square(0.0);
        let parts = [(&low, Transform::IDENTITY)];
        // The craft's +Z turned to the view's +X, its origin at (10, 0, 0):
        // the square now faces +X at x = 10.
        let placement =
            Transform::from_xyz(10.0, 0.0, 0.0).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        let hit = ray_hit(Vec3::new(20.0, 0.5, 0.5), -Vec3::X, placement, &parts);
        assert!(hit.is_some_and(|(k, d)| k == 0 && (d - 10.0).abs() < 1e-4), "{hit:?}");
        assert_eq!(ray_hit(Vec3::new(20.0, 2.0, 0.0), -Vec3::X, placement, &parts), None);
    }

    #[test]
    fn a_click_is_short_and_still() {
        let p = Vec2::new(100.0, 100.0);
        // (seconds, moved px, click)
        let table = [(0.1, 0.0, true), (0.3, 4.0, true), (0.5, 0.0, false), (0.1, 12.0, false)];
        for (dt, moved, want) in table {
            assert_eq!(is_click(1.0, p, 1.0 + dt, p + Vec2::new(moved, 0.0)), want, "{dt} {moved}");
        }
    }
}
