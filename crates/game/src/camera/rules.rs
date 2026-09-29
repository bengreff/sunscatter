//! The camera's rules as pure functions (rule 8): orientation, zoom limits,
//! surface clearance and focus. `camera` wires them to the rig each frame.

use super::Focus;
use glam::DVec3;

/// Closest and farthest camera distances from the focus (m).
pub const MIN_DISTANCE: f64 = 1.0;
pub const MAX_DISTANCE: f64 = 5.0e12;
/// Closest camera distance to a vessel, in its bounding radii.
pub const VESSEL_MIN_RADII: f64 = 1.3;
/// Closest camera distance to a body focus (its surface is handled by the
/// clearance rules instead).
pub const BODY_MIN_DISTANCE: f64 = 5.0;
/// Below this altitude (in radii of the body) the camera's yaw reference
/// turns with the ground; higher, it is inertial.
pub const CO_ROTATE_RADII: f64 = 0.02;
/// Seconds over which the camera's up turns to a new reference body.
pub const UP_BLEND_SECONDS: f64 = 1.5;

/// Closest allowed distance to a focus; `radius` is the focused craft's
/// bounding radius (m).
pub fn min_distance(focus: Focus, radius: f64) -> f64 {
    match focus {
        Focus::Ship | Focus::Vessel(_) => (VESSEL_MIN_RADII * radius).max(MIN_DISTANCE),
        Focus::Body(_) => BODY_MIN_DISTANCE,
    }
}

/// The distance `` ` `` returns to: 60 m, or farther for a large craft.
pub fn default_distance(radius: f64) -> f64 {
    (4.0 * radius).max(60.0)
}

/// Height the camera keeps above any surface (m) at a focus distance.
pub fn surface_margin(distance: f64) -> f64 {
    (0.002 * distance).max(2.0)
}

/// Whether a camera move is allowed, from the clearance (m, negative when
/// too close) before and after: it stays clear, or it moves out of a
/// surface it was already too close to (the surface can move under it).
pub fn accept(old: f64, new: f64) -> bool {
    new >= 0.0 || new >= old
}

/// The focus distance after zooming in by `lines` wheel lines, each a factor
/// of `per_line`.
pub fn zoom(distance: f64, lines: f64, per_line: f64) -> f64 {
    (distance * per_line.powf(-lines)).clamp(MIN_DISTANCE, MAX_DISTANCE)
}

/// The distance along the view ray to place the camera: `distance` if clear
/// there, else the farthest clear point between `min` and `distance`, so a
/// surface rising behind the camera pushes it towards the focus. `clear(s)`
/// is the clearance at distance `s`. `None` when no point of the ray is
/// clear (the focus itself is at or under the surface).
pub fn pull_in(distance: f64, min: f64, clear: impl Fn(f64) -> f64) -> Option<f64> {
    if clear(distance) >= 0.0 {
        return Some(distance);
    }
    if clear(min) < 0.0 {
        return None;
    }
    let (mut lo, mut hi) = (min, distance);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if clear(mid) >= 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

/// `p` raised along `up` (unit) just far enough to be clear (`clear(p)` is
/// the clearance at a point): the camera is never left under a surface.
pub fn lift(p: DVec3, up: DVec3, clear: impl Fn(DVec3) -> f64) -> DVec3 {
    let c0 = clear(p);
    if c0 >= 0.0 {
        return p;
    }
    let (mut lo, mut hi) = (0.0, -c0);
    for _ in 0..60 {
        if clear(p + up * hi) >= 0.0 {
            break;
        }
        lo = hi;
        hi = 2.0 * hi + 1.0;
    }
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if clear(p + up * mid) >= 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    p + up * hi
}

/// Where the camera goes: along `dir` (unit) from `target` at `distance`,
/// pulled in towards `min` by a surface in the way, and lifted along `up`
/// when nothing on the ray is clear.
pub fn place(target: DVec3, dir: DVec3, distance: f64, min: f64, up: DVec3, clear: impl Fn(DVec3) -> f64) -> DVec3 {
    match pull_in(distance, min, |s| clear(target + dir * s)) {
        Some(s) => target + dir * s,
        None => lift(target + dir * distance, up, clear),
    }
}

/// `v` normalized, or `fallback` when it has no direction (zero or NaN).
pub fn safe_dir(v: DVec3, fallback: DVec3) -> DVec3 {
    v.try_normalize().unwrap_or(fallback)
}

/// Two unit vectors perpendicular to `up` (and to each other): a fresh
/// basis, used when the camera takes a new focus.
pub fn basis(up: DVec3) -> (DVec3, DVec3) {
    let seed = if up.cross(DVec3::Z).length() > 1e-6 { DVec3::Z } else { DVec3::X };
    let e1 = up.cross(seed).normalize();
    (e1, up.cross(e1))
}

/// Rotates `v` by `angle` (rad) about the unit `axis` (Rodrigues).
pub fn rotate(v: DVec3, axis: DVec3, angle: f64) -> DVec3 {
    let (s, c) = angle.sin_cos();
    v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c))
}

/// The yaw reference carried to a new `up`: turned by `spin` (the frame's
/// rotation since last time: the body the camera stands on turns under
/// it), then projected onto the plane perpendicular to `up` (parallel
/// transport for small steps). It never flips near a pole, unlike
/// [`basis`]; it falls back to [`basis`] only when it lies along `up`.
pub fn carry_reference(reference: DVec3, up: DVec3, spin: Option<(DVec3, f64)>) -> DVec3 {
    let r = spin.map_or(reference, |(axis, angle)| rotate(reference, axis, angle));
    let h = r - up * r.dot(up);
    if h.length_squared() > 1e-12 {
        h.normalize()
    } else {
        basis(up).0
    }
}

/// Unit vector from the focus to the camera: `yaw` from the reference `e1`
/// (perpendicular to `up`) towards `up × e1`, `pitch` above the horizontal.
pub fn view_dir(e1: DVec3, up: DVec3, yaw: f64, pitch: f64) -> DVec3 {
    let e2 = up.cross(e1);
    (e1 * yaw.cos() + e2 * yaw.sin()) * pitch.cos() + up * pitch.sin()
}

/// The unit direction `s` of the way (0..1) from `a` to `b` along the great
/// circle; opposite directions turn about any perpendicular.
pub fn slerp_dir(a: DVec3, b: DVec3, s: f64) -> DVec3 {
    let angle = a.dot(b).clamp(-1.0, 1.0).acos();
    if angle < 1e-9 {
        return b;
    }
    let axis = a.cross(b).try_normalize().unwrap_or_else(|| basis(a).0);
    rotate(a, axis, angle * s).normalize()
}

/// 0 → 0, 1 → 1, flat at both ends; clamped outside.
pub fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// The camera's up while turning from `from` to `to` (both unit), `blend`
/// of [`UP_BLEND_SECONDS`] done (0..1).
pub fn blended_up(from: DVec3, to: DVec3, blend: f64) -> DVec3 {
    slerp_dir(from, to, smoothstep(blend))
}

/// Whether the yaw reference turns with the body under a focus at
/// `altitude` (m) over a body of `radius` (m): near the ground, so a view
/// of a craft on the pad keeps its heading while time passes.
pub fn co_rotates(altitude: f64, radius: f64) -> bool {
    altitude < CO_ROTATE_RADII * radius
}

/// The focus to use: a focused vessel that no longer exists falls back to
/// the active ship.
pub fn resolve_focus(focus: Focus, vessel_exists: bool) -> Focus {
    match focus {
        Focus::Vessel(_) if !vessel_exists => Focus::Ship,
        f => f,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::frame::NodeId;
    use sim::vessel::VesselId;

    fn close(a: DVec3, b: DVec3, tol: f64) -> bool {
        (a - b).length() < tol
    }

    #[test]
    fn zoom_table() {
        // (distance, lines, per line, expected)
        let cases = [
            (1000.0, 1.0, 1.072, 1000.0 / 1.072),
            (1000.0, -1.0, 1.072, 1072.0),
            (1000.0, 0.0, 1.072, 1000.0),
            (1.2, 10.0, 1.072, MIN_DISTANCE),
            (4.0e12, -10.0, 1.072, MAX_DISTANCE),
        ];
        for (d, lines, per_line, expected) in cases {
            assert!((zoom(d, lines, per_line) - expected).abs() < 1e-9 * expected, "{d} {lines}");
        }
        // Two lines at the new speed equal one at the old (1.15 per line).
        assert!((zoom(1000.0, 2.0, 1.072) / (1000.0 / 1.15) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn the_closest_zoom_follows_the_craft_size() {
        // (focus, craft radius, expected)
        let cases = [
            (Focus::Ship, 6.0, 7.8),
            (Focus::Ship, 30.0, 39.0),
            (Focus::Vessel(VesselId(2)), 0.5, MIN_DISTANCE),
            (Focus::Body(NodeId(3)), 6.0, BODY_MIN_DISTANCE),
        ];
        for (focus, radius, expected) in cases {
            assert!((min_distance(focus, radius) - expected).abs() < 1e-12, "{focus:?} {radius}");
        }
        assert_eq!(default_distance(6.0), 60.0);
        assert_eq!(default_distance(30.0), 120.0);
    }

    /// Clearance above a 1,000 m sphere at the origin with a 200 m mountain
    /// around +x, minus the margin.
    fn world_clear(p: DVec3, distance: f64) -> f64 {
        let mountain = 200.0 * (p.normalize().dot(DVec3::X) - 0.9).max(0.0) * 10.0;
        p.length() - 1000.0 - mountain - surface_margin(distance)
    }

    #[test]
    fn zooming_or_rotating_into_terrain_is_refused() {
        // (case, clearance before, after, accepted)
        let cases = [
            ("clear to clear", 50.0, 20.0, true),
            ("zoom into the ground", 5.0, -1.0, false),
            ("already too close, moving out", -3.0, -1.0, true),
            ("already too close, moving in", -3.0, -4.0, false),
            ("exactly at the margin", 1.0, 0.0, true),
        ];
        for (case, old, new, expected) in cases {
            assert_eq!(accept(old, new), expected, "{case}");
        }
    }

    #[test]
    fn orbiting_below_a_mountain_stops_at_its_slope() {
        // Ship 20 m above the plain at +z; the camera swings down towards the
        // mountain at +x, 300 m away.
        let target = DVec3::new(0.0, 0.0, 1020.0);
        let up = DVec3::Z;
        let e1 = basis(up).0;
        let d = 300.0;
        // Yaw towards +x.
        let yaw = DVec3::X.dot(up.cross(e1)).atan2(DVec3::X.dot(e1));
        let clear = |pitch: f64| world_clear(target + view_dir(e1, up, yaw, pitch) * d, d);
        let mut pitch: f64 = 0.8;
        let mut stopped = false;
        while pitch > -1.5 {
            let next = pitch - 0.01;
            if !accept(clear(pitch), clear(next)) {
                stopped = true;
                break;
            }
            pitch = next;
        }
        assert!(stopped, "the camera went into the ground");
        assert!(clear(pitch) >= 0.0 && pitch < 0.2, "stopped at pitch {pitch}");
    }

    #[test]
    fn a_rising_surface_pulls_the_camera_in() {
        let target = DVec3::new(0.0, 0.0, 1050.0);
        let dir = DVec3::new(0.6, 0.0, -0.8);
        let clear = |s: f64| world_clear(target + dir * s, 100.0);
        // 100 m out along a descending ray is underground; 10 m is not.
        assert!(clear(100.0) < 0.0 && clear(10.0) > 0.0);
        let s = pull_in(100.0, 10.0, clear).unwrap();
        assert!(clear(s).abs() < 1e-6 && s < 100.0 && s > 10.0, "{s}");
        assert_eq!(pull_in(30.0, 10.0, |_| 1.0), Some(30.0), "clear: unchanged");
        assert_eq!(pull_in(30.0, 10.0, |_| -1.0), None, "nothing clear on the ray");
    }

    #[test]
    fn the_camera_is_never_left_under_the_ground() {
        // (case, focus, ray direction): the placed camera is always clear.
        let down = DVec3::new(0.6, 0.0, -0.8);
        let cases = [
            ("clear ray", DVec3::new(0.0, 0.0, 1050.0), DVec3::new(0.6, 0.0, 0.8)),
            ("ray into the ground", DVec3::new(0.0, 0.0, 1050.0), down),
            ("focus on the ground", DVec3::new(0.0, 0.0, 1000.5), down),
            ("focus under the ground", DVec3::new(0.0, 0.0, 990.0), down),
            ("focus inside the mountain", DVec3::new(1000.0, 0.0, 0.0), DVec3::X),
        ];
        for (case, target, dir) in cases {
            let clear = |p: DVec3| world_clear(p, 100.0);
            let up = target.normalize();
            let p = place(target, dir, 100.0, 10.0, up, clear);
            assert!(clear(p) >= 0.0 && clear(p) < 1.0 || case == "clear ray", "{case}: {}", clear(p));
            assert!(clear(p) >= 0.0, "{case}");
        }
        assert_eq!(lift(DVec3::new(0.0, 0.0, 2000.0), DVec3::Z, |p| world_clear(p, 1.0)), DVec3::new(0.0, 0.0, 2000.0));
    }

    #[test]
    fn the_yaw_reference_does_not_flip_over_a_pole() {
        // A ship's radial up crossing the +Z pole in small steps (a polar
        // orbit): the carried reference turns smoothly, where the fresh
        // basis swings through 180°.
        let mut reference = basis(DVec3::new(1.0, 0.0, 0.0)).0;
        let mut worst: f64 = 0.0;
        let mut fresh_worst: f64 = 0.0;
        let step = 0.002;
        let mut prev_fresh = basis(DVec3::X).0;
        let mut a: f64 = 0.0;
        while a < std::f64::consts::PI {
            a += step;
            let up = DVec3::new(a.cos(), 0.0, a.sin());
            let next = carry_reference(reference, up, None);
            worst = worst.max(next.angle_between(reference));
            reference = next;
            let fresh = basis(up).0;
            fresh_worst = fresh_worst.max(fresh.angle_between(prev_fresh));
            prev_fresh = fresh;
            assert!(reference.dot(up).abs() < 1e-9 && (reference.length() - 1.0).abs() < 1e-9);
        }
        assert!(worst < 2.0 * step, "carried reference jumped {worst}");
        assert!(fresh_worst > 1.0, "the fresh basis flips at the pole");
    }

    #[test]
    fn the_yaw_reference_turns_with_the_ground_under_it() {
        // Standing on a rotating body: up and the reference turn together,
        // so the view keeps its heading.
        let axis = DVec3::Z;
        let up0 = DVec3::new(0.8, 0.0, 0.6);
        let reference = carry_reference(basis(up0).0, up0, None);
        let angle = 0.3;
        let up1 = rotate(up0, axis, angle);
        let carried = carry_reference(reference, up1, Some((axis, angle)));
        assert!(close(carried, rotate(reference, axis, angle), 1e-12));
        // Along up: falls back to the fresh basis.
        assert!(close(carry_reference(DVec3::Z, DVec3::Z, None), basis(DVec3::Z).0, 1e-12));
    }

    #[test]
    fn up_blends_between_reference_bodies() {
        let (a, b) = (DVec3::X, DVec3::Y);
        // (blend, expected angle from a)
        let quarter = std::f64::consts::FRAC_PI_2;
        let cases = [(-1.0, 0.0), (0.0, 0.0), (0.5, 0.5 * quarter), (1.0, quarter), (2.0, quarter)];
        for (blend, angle) in cases {
            let up = blended_up(a, b, blend);
            assert!((up.angle_between(a) - angle).abs() < 1e-9, "{blend}");
            assert!((up.length() - 1.0).abs() < 1e-12);
        }
        // Opposite directions still pass through a unit vector.
        let mid = blended_up(DVec3::Z, -DVec3::Z, 0.5);
        assert!((mid.length() - 1.0).abs() < 1e-9 && mid.z.abs() < 1e-9);
        // Smooth at both ends.
        assert!(smoothstep(0.01) < 0.001 && smoothstep(0.99) > 0.999);
    }

    #[test]
    fn degenerate_vectors_do_not_make_nan() {
        // (vector, fallback, expected)
        let cases = [
            (DVec3::ZERO, DVec3::Z, DVec3::Z),
            (DVec3::splat(f64::NAN), DVec3::X, DVec3::X),
            (DVec3::new(0.0, 3.0, 0.0), DVec3::Z, DVec3::Y),
        ];
        for (v, fallback, expected) in cases {
            assert_eq!(safe_dir(v, fallback), expected, "{v:?}");
        }
    }

    #[test]
    fn a_deleted_focused_vessel_returns_the_focus_to_the_ship() {
        // (focus, vessel exists, expected)
        let v = Focus::Vessel(VesselId(4));
        let cases = [
            (v, true, v),
            (v, false, Focus::Ship),
            (Focus::Ship, false, Focus::Ship),
            (Focus::Body(NodeId(1)), false, Focus::Body(NodeId(1))),
        ];
        for (focus, exists, expected) in cases {
            assert_eq!(resolve_focus(focus, exists), expected);
        }
    }

    #[test]
    fn the_reference_turns_with_the_ground_only_near_it() {
        // (altitude, radius, co-rotates)
        let cases = [(0.0, 6.371e6, true), (100e3, 6.371e6, true), (400e3, 6.371e6, false), (100e3, 1.737e6, false)];
        for (h, r, expected) in cases {
            assert_eq!(co_rotates(h, r), expected, "{h} {r}");
        }
    }

    #[test]
    fn margin_grows_with_distance() {
        assert_eq!(surface_margin(60.0), 2.0);
        assert_eq!(surface_margin(1.0e6), 2000.0);
    }
}
