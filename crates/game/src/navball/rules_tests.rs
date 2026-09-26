use super::*;
use glam::DMat3;

const EPS: f64 = 1e-9;

/// On the equator at longitude 0 of a body spinning about +Z: up = X,
/// north = Z, east = Y.
fn local() -> Local {
    local_frame(DVec3::X * 6.4e6, DVec3::Z)
}

/// A ship from its forward and top directions (right × up = −forward, as the
/// body axes say).
fn ship(forward: DVec3, up: DVec3) -> Ship {
    let right = forward.cross(up);
    let q = DQuat::from_mat3(&DMat3::from_cols(right, -up, forward));
    ship_axes(q)
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

#[test]
fn local_frame_is_right_handed_east_north_up() {
    let l = local();
    assert!((l.up - DVec3::X).length() < EPS);
    assert!((l.north - DVec3::Z).length() < EPS);
    assert!((l.east - DVec3::Y).length() < EPS);
    // At the pole north is still a unit horizontal vector.
    let p = local_frame(DVec3::Z * 6.4e6, DVec3::Z);
    assert!(close(p.north.length(), 1.0, EPS) && close(p.north.dot(p.up), 0.0, EPS));
}

#[test]
fn ship_axes_match_the_controls() {
    // Identity attitude: nose +Z, right +X, top −Y.
    let s = ship_axes(DQuat::IDENTITY);
    assert_eq!((s.forward, s.right, s.up), (DVec3::Z, DVec3::X, DVec3::NEG_Y));
    // The test helper reproduces requested axes.
    let s = ship(DVec3::Z, DVec3::X);
    assert!((s.forward - DVec3::Z).length() < EPS && (s.up - DVec3::X).length() < EPS);
    assert!((s.right - DVec3::Y).length() < EPS);
}

#[test]
fn heading_pitch_roll_for_known_attitudes() {
    let (s30, c30) = (0.5, 0.75f64.sqrt());
    let r45 = 0.5f64.sqrt();
    // (case, forward, top, heading, pitch, roll); local: up X, north Z, east Y.
    let cases = [
        ("nose up on the pad, belly north", DVec3::X, DVec3::NEG_Z, 0.0, 90.0, 0.0),
        ("nose up on the pad, belly east", DVec3::X, DVec3::NEG_Y, 90.0, 90.0, 0.0),
        ("level, north", DVec3::Z, DVec3::X, 0.0, 0.0, 0.0),
        ("level, east", DVec3::Y, DVec3::X, 90.0, 0.0, 0.0),
        ("level, south", DVec3::NEG_Z, DVec3::X, 180.0, 0.0, 0.0),
        ("level, west", DVec3::NEG_Y, DVec3::X, 270.0, 0.0, 0.0),
        ("45° up, north", DVec3::new(r45, 0.0, r45), DVec3::new(r45, 0.0, -r45), 0.0, 45.0, 0.0),
        ("nose down, north", DVec3::NEG_X, DVec3::Z, 0.0, -90.0, 0.0),
        ("north, rolled 30° right", DVec3::Z, DVec3::new(c30, s30, 0.0), 0.0, 0.0, 30.0),
        ("north, rolled 30° left", DVec3::Z, DVec3::new(c30, -s30, 0.0), 0.0, 0.0, -30.0),
        ("north, inverted", DVec3::Z, DVec3::NEG_X, 0.0, 0.0, 180.0),
    ];
    for (name, f, u, heading, pitch, roll) in cases {
        let a = attitude_angles(&ship(f, u), &local());
        let dh = (a.heading - heading + 180.0).rem_euclid(360.0) - 180.0;
        let droll = (a.roll - roll + 180.0).rem_euclid(360.0) - 180.0;
        assert!(close(dh, 0.0, 1e-6), "{name}: heading {}", a.heading);
        assert!(close(a.pitch, pitch, 1e-6), "{name}: pitch {}", a.pitch);
        assert!(close(droll, 0.0, 1e-6), "{name}: roll {}", a.roll);
    }
}

#[test]
fn nose_up_on_the_pad_centres_the_zenith_and_puts_the_horizon_on_the_rim() {
    let l = local();
    let s = ship(l.up, -l.north);
    let (x, y, z) = ball_project(&s, l.up);
    assert!(close(x, 0.0, EPS) && close(y, 0.0, EPS) && close(z, 1.0, EPS));
    for k in 0..12 {
        let (x, y, z) = ball_project(&s, l.direction(0.0, k as f64 * 0.5));
        assert!(close(z, 0.0, EPS) && close(x.hypot(y), 1.0, EPS), "horizon point {k}");
    }
    // Belly north: north is at the bottom of the ball (behind the top).
    let (_, y, _) = ball_project(&s, l.direction(0.3, 0.0));
    assert!(y < 0.0);
}

#[test]
fn level_flight_puts_the_horizon_through_the_centre_with_sky_above() {
    let l = local();
    let s = ship(l.north, l.up);
    let (x, y, z) = ball_project(&s, l.north);
    assert!(close(x, 0.0, EPS) && close(y, 0.0, EPS) && close(z, 1.0, EPS));
    assert!(close(ball_project(&s, l.up).1, 1.0, EPS), "zenith at the top");
    assert!(close(ball_project(&s, l.east).0, 1.0, EPS), "east to the right when facing north");
    // Rolled right: the sky side tilts to the left on the ball, like a window.
    let rolled = ship(l.north, (l.up * 0.8 + l.east * 0.6).normalize());
    assert!(ball_project(&rolled, l.up).0 < 0.0);
}

#[test]
fn unproject_inverts_project_on_the_front_hemisphere() {
    let s = ship(DVec3::new(1.0, 2.0, 0.5).normalize(), DVec3::new(-2.0, 1.0, 0.0).normalize());
    for (x, y) in [(0.0, 0.0), (0.3, -0.2), (-0.7, 0.6), (0.0, 0.99)] {
        let (px, py, pz) = ball_project(&s, ball_unproject(&s, x, y));
        assert!(close(px, x, EPS) && close(py, y, EPS) && pz >= 0.0);
    }
}

#[test]
fn prograde_is_the_normalised_velocity_and_radial_is_perpendicular() {
    let r = DVec3::new(7.0e6, 0.0, 0.0);
    for v in [
        DVec3::new(0.0, 7_500.0, 0.0),
        DVec3::new(1_000.0, 7_000.0, 500.0),
        DVec3::new(-300.0, 2.0, 9_000.0),
        DVec3::new(0.2, 0.0, 0.0) + DVec3::Y * 0.1,
    ] {
        let m = markers(r, v, None);
        let p = m.prograde.expect("moving");
        assert!((p - v.normalize()).length() < EPS);
        let (n, rad) = (m.normal.expect("normal"), m.radial_out.expect("radial"));
        assert!(close(rad.dot(p), 0.0, EPS) && close(n.dot(p), 0.0, EPS) && close(n.dot(rad), 0.0, EPS));
        assert!(close(n.length(), 1.0, EPS) && close(rad.length(), 1.0, EPS));
        assert!(rad.dot(r) > 0.0, "radial out points away from the body");
        assert!(n.dot(r.cross(v)) > 0.0, "normal along the angular momentum");
    }
    // Circular orbit: prograde +Y, normal +Z, radial out +X.
    let m = markers(r, DVec3::Y * 7_500.0, None);
    assert!((m.normal.unwrap() - DVec3::Z).length() < EPS);
    assert!((m.radial_out.unwrap() - DVec3::X).length() < EPS);
}

#[test]
fn markers_are_undefined_without_velocity_and_target_points_at_the_target() {
    let r = DVec3::X * 6.4e6;
    let m = markers(r, DVec3::ZERO, Some(DVec3::new(0.0, 3.0, 4.0)));
    assert!(m.prograde.is_none() && m.normal.is_none() && m.radial_out.is_none());
    assert!((m.target.unwrap() - DVec3::new(0.0, 0.6, 0.8)).length() < EPS);
    // Straight up: prograde defined, but no orbital plane.
    let m = markers(r, DVec3::X * 100.0, None);
    assert!(m.prograde.is_some() && m.normal.is_none());
}

#[test]
fn mode_switches_at_36_km_unless_locked_or_targeting() {
    use Mode::*;
    // (current, locked, altitude, has_target, expected)
    let cases = [
        (Surface, false, 10_000.0, false, Surface),
        (Surface, false, 40_000.0, false, Orbit),
        (Orbit, false, 35_999.0, false, Surface),
        (Orbit, false, 36_000.0, false, Orbit),
        (Surface, true, 400_000.0, false, Surface),
        (Orbit, true, 0.0, false, Orbit),
        (Target, false, 0.0, true, Target),
        (Target, true, 400_000.0, true, Target),
        (Target, false, 0.0, false, Surface),
        (Target, true, 400_000.0, false, Orbit),
    ];
    for (current, locked, alt, target, expected) in cases {
        assert_eq!(auto_mode(current, locked, alt, target), expected, "{current:?} {locked} {alt} {target}");
    }
    assert_eq!(Surface.next(false), Orbit);
    assert_eq!(Orbit.next(false), Surface);
    assert_eq!(Orbit.next(true), Target);
    assert_eq!(Target.next(true), Surface);
}

#[test]
fn angle_of_attack_and_sideslip() {
    let s = ship(DVec3::Z, DVec3::X);
    // (velocity, aoa, sideslip)
    let cases = [
        (DVec3::Z * 100.0, 0.0, 0.0),
        (DVec3::new(-1.0, 0.0, 1.0) * 100.0, 45.0, 0.0),
        (DVec3::new(1.0, 0.0, 1.0) * 100.0, -45.0, 0.0),
        (DVec3::new(0.0, 1.0, 1.0) * 100.0, 0.0, 45.0),
    ];
    for (v, aoa, slip) in cases {
        let (a, b) = aoa_sideslip(&s, v).expect("moving");
        assert!(close(a, aoa, 1e-9) && close(b, slip, 1e-9), "{v}: {a} {b}");
    }
    assert!(aoa_sideslip(&s, DVec3::Z * 0.5).is_none());
}

#[test]
fn vertical_speed_and_g_load() {
    let r = DVec3::new(0.0, 6.4e6, 0.0);
    assert!(close(vertical_speed(r, DVec3::new(50.0, 12.0, -3.0)), 12.0, EPS));
    assert!(close(g_load(DVec3::new(0.0, 0.0, -G0 * 3.0)), 3.0, EPS));
    assert_eq!(g_load(DVec3::ZERO), 0.0);
}

#[test]
fn time_to_apsides_follows_the_mean_anomaly() {
    let mu = 3.986_004_418e14;
    let el = |e: f64, a: f64, m: f64| Elements { a, e, i: 0.3, raan: 0.0, argp: 0.0, mean_anomaly: m };
    let circ = el(0.1, 7.0e6, 0.0);
    let period = circ.period(mu);
    let (ap, pe) = time_to_apsides(&circ, mu);
    assert!(close(pe.unwrap(), 0.0, 1e-6) && close(ap.unwrap(), period / 2.0, 1e-6));
    let (ap, pe) = time_to_apsides(&el(0.1, 7.0e6, PI), mu);
    assert!(close(ap.unwrap(), 0.0, 1e-6) && close(pe.unwrap(), period / 2.0, 1e-6));
    let (ap, pe) = time_to_apsides(&el(0.1, 7.0e6, 1.5 * PI), mu);
    assert!(close(ap.unwrap(), 0.75 * period, 1e-6) && close(pe.unwrap(), 0.25 * period, 1e-6));
    // Hyperbolic: periapsis ahead only before it.
    let hyp = el(1.5, -2.0e7, -1.0);
    let (ap, pe) = time_to_apsides(&hyp, mu);
    assert!(ap.is_none() && close(pe.unwrap(), 1.0 / hyp.mean_motion(mu), 1e-6));
    assert_eq!(time_to_apsides(&el(1.5, -2.0e7, 1.0), mu), (None, None));
}
