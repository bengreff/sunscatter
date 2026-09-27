//! Clipping drawn polylines to the space in front of the camera.
//!
//! Bevy's gizmo lines expand a segment with a vertex behind the camera into
//! a huge screen-space triangle: an orbit line wrapping around the camera
//! (the Moon's orbit seen from a few thousand km off Earth) flashed as a grey
//! shape while zooming. Lines are split into runs that stay at least `near`
//! in front of the camera, cut exactly where they cross that plane.

use bevy::math::Vec3;

/// Splits camera-relative `points` into runs in front of the camera
/// (`forward` unit, depth ≥ `near`), each with at least two points.
pub fn front_runs(points: impl IntoIterator<Item = Vec3>, forward: Vec3, near: f32) -> Vec<Vec<Vec3>> {
    let mut runs = Vec::new();
    let mut run: Vec<Vec3> = Vec::new();
    let mut prev: Option<(Vec3, f32)> = None;
    for p in points {
        let d = p.dot(forward) - near;
        if let Some((q, dq)) = prev {
            if (dq >= 0.0) != (d >= 0.0) {
                // Where the segment crosses the plane.
                let cut = q + (p - q) * (dq / (dq - d));
                run.push(cut);
                if d < 0.0 {
                    if run.len() > 1 {
                        runs.push(std::mem::take(&mut run));
                    }
                    run.clear();
                }
            }
        }
        if d >= 0.0 {
            run.push(p);
        }
        prev = Some((p, d));
    }
    if run.len() > 1 {
        runs.push(run);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: Vec3 = Vec3::NEG_Z;

    #[test]
    fn a_line_in_front_is_unchanged() {
        let pts = vec![Vec3::new(0.0, 0.0, -5.0), Vec3::new(1.0, 0.0, -6.0), Vec3::new(2.0, 0.0, -7.0)];
        assert_eq!(front_runs(pts.clone(), F, 1.0), vec![pts]);
    }

    #[test]
    fn a_line_behind_is_dropped() {
        let pts = vec![Vec3::new(0.0, 0.0, 5.0), Vec3::new(1.0, 0.0, 6.0)];
        assert!(front_runs(pts, F, 1.0).is_empty());
    }

    #[test]
    fn a_line_through_the_camera_plane_is_cut_at_it() {
        // In front, behind, in front again: two runs, each ending/starting
        // exactly on the near plane.
        let pts = vec![Vec3::new(0.0, 0.0, -3.0), Vec3::new(0.0, 2.0, 1.0), Vec3::new(0.0, 4.0, -3.0)];
        let runs = front_runs(pts, F, 1.0);
        assert_eq!(runs.len(), 2);
        for run in &runs {
            assert!(run.iter().all(|p| p.dot(F) >= 1.0 - 1e-5), "{run:?}");
        }
        assert!((runs[0][1].dot(F) - 1.0).abs() < 1e-5);
        assert!((runs[1][0].dot(F) - 1.0).abs() < 1e-5);
    }
}
