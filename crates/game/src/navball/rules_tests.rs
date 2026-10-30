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
fn the_rails_floor_has_a_band() {
    use Side::*;
    // (previous, altitude, floor, expected): Earth's floor is 150 km.
    let cases = [
        (None, 149_000.0, 150_000.0, Low),
        (None, 150_000.0, 150_000.0, High),
        (Some(High), 150_500.0, 150_000.0, High),
        (Some(Low), 150_500.0, 150_000.0, Low),
        (Some(Low), 151_600.0, 150_000.0, High),
        (Some(High), 149_999.0, 150_000.0, Low),
        // No floor (a giant planet): always high.
        (Some(Low), -1_000.0, 0.0, High),
    ];
    for (previous, alt, floor, expected) in cases {
        assert_eq!(side(previous, alt, floor), expected, "{previous:?} {alt} {floor}");
    }
}

#[test]
fn the_mode_switches_when_crossing_the_rails_floor() {
    use Mode::*;
    use Side::*;
    // (current, locked, side before, side now, has target, expected)
    let cases = [
        // Climbing through the floor: orbit; descending: surface.
        (Surface, false, Some(Low), High, false, Orbit),
        (Orbit, false, Some(High), Low, false, Surface),
        // A choice by hand holds between crossings, either side.
        (Orbit, false, Some(Low), Low, false, Orbit),
        (Surface, false, Some(High), High, false, Surface),
        (Target, false, Some(High), High, true, Target),
        // ... until the next crossing, target included.
        (Target, false, Some(High), Low, true, Surface),
        // A new vessel (no side yet) starts on its side's mode.
        (Orbit, false, None, Low, false, Surface),
        (Surface, false, None, High, false, Orbit),
        // Locked: never switched.
        (Surface, true, Some(Low), High, false, Surface),
        (Orbit, true, None, Low, false, Orbit),
        // Target without a target: the side's mode.
        (Target, false, Some(High), High, false, Orbit),
        (Target, true, Some(Low), Low, false, Surface),
    ];
    for (current, locked, before, now, target, expected) in cases {
        let got = auto_mode(current, locked, before, now, target);
        assert_eq!(got, expected, "{current:?} {locked} {before:?} {now:?} {target}");
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
fn sky_is_blue_and_ground_is_brown() {
    let [r, _, b] = ball_colour(0.5);
    assert!(b > r, "sky");
    let [r, _, b] = ball_colour(-0.5);
    assert!(r > b, "ground");
}

/// Landed with the nose up, the rim is the horizon: noise in attitude must
/// not flip its colour.
#[test]
fn ball_colour_is_continuous_across_the_horizon() {
    let (a, b) = (ball_colour(1e-9), ball_colour(-1e-9));
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < 0.1, "channel {i}: {a:?} vs {b:?}");
    }
    // Still fully sky or ground outside the band.
    assert!(ball_colour(2.0 * HORIZON_BLEND)[2] > 200.0);
    assert!(ball_colour(-2.0 * HORIZON_BLEND)[0] > 150.0);
}

#[test]
fn a_line_on_the_rim_is_drawn_whatever_the_noise() {
    for z in [1e-12, -1e-12, 1e-9, -1e-9, 0.0] {
        assert!(faces_viewer(z), "{z}");
    }
    assert!(!faces_viewer(-0.01));
}

#[test]
fn sas_label_table() {
    use sim::vessel::{Controls, HoldMode, SpeedReference};
    let c = |sas, hold, speed| Controls { sas, hold, speed, ..Controls::default() };
    let cases = [
        (c(false, HoldMode::Prograde, SpeedReference::Orbit), "OFF"),
        (c(true, HoldMode::Stability, SpeedReference::Orbit), "STABILITY"),
        (c(true, HoldMode::Prograde, SpeedReference::Orbit), "PROGRADE (ORBIT Earth)"),
        (c(true, HoldMode::Retrograde, SpeedReference::Surface), "RETROGRADE (SURFACE Earth)"),
        (c(true, HoldMode::Prograde, SpeedReference::Target), "PROGRADE (TARGET)"),
        (c(true, HoldMode::AntiTarget, SpeedReference::Surface), "ANTI-TARGET"),
        (c(true, HoldMode::Maneuver, SpeedReference::Orbit), "MANEUVER"),
    ];
    for (controls, expected) in cases {
        assert_eq!(sas_label(&controls, "Earth"), expected);
    }
    assert_eq!(speed_reference(Mode::Surface), SpeedReference::Surface);
}

#[test]
fn the_sas_button_says_what_the_flight_software_says() {
    use sim::vessel::{Controls, HoldMode};
    let on = Controls { sas: true, hold: HoldMode::Stability, ..Controls::default() };
    let off = Controls::default();
    // (controls, status, label, hover)
    let cases = [
        (on, Sas::Holding, "SAS ON", "SAS: STABILITY"),
        (off, Sas::Off, "SAS OFF", "SAS: OFF"),
        (on, Sas::NoAttitude, "SAS NO REF", "SAS: no attitude reference (STABILITY)"),
        (on, Sas::NoNavigation, "SAS NO NAV", "SAS: no navigation solution: holding attitude (STABILITY)"),
        (off, Sas::NoComputer, "SAS NO CPU", "SAS: no flight computer (OFF)"),
    ];
    for (controls, status, label, hover) in cases {
        assert_eq!(sas_button(&controls, "Earth", status), (label, hover.to_string()), "{status:?}");
    }
}

#[test]
fn the_throttle_bar_sends_only_while_pressed() {
    // A bar from y = 100 (full) to y = 200 (closed).
    // (pressed, pointer y, expected)
    let table = [
        (false, Some(150.0), None),
        (true, None, None),
        (true, Some(200.0), Some(0.0)),
        (true, Some(100.0), Some(1.0)),
        (true, Some(150.0), Some(0.5)),
        (true, Some(175.0), Some(0.25)),
        (true, Some(40.0), Some(1.0)),
        (true, Some(260.0), Some(0.0)),
    ];
    for (pressed, y, want) in table {
        assert_eq!(throttle_from_bar(pressed, y, 100.0, 200.0), want, "{pressed} {y:?}");
    }
    assert_eq!(throttle_from_bar(true, Some(100.0), 100.0, 100.0), None, "a bar with no height");
}

#[test]
fn modes_and_holds_need_what_they_point_at() {
    use sim::vessel::HoldMode;
    assert!(mode_available(Mode::Orbit, false) && mode_available(Mode::Surface, false));
    assert!(!mode_available(Mode::Target, false) && mode_available(Mode::Target, true));
    // (mode, target, burn, available)
    let table = [
        (HoldMode::Stability, false, false, true),
        (HoldMode::Prograde, false, false, true),
        (HoldMode::Retrograde, false, false, true),
        (HoldMode::Target, false, true, false),
        (HoldMode::Target, true, false, true),
        (HoldMode::AntiTarget, false, false, false),
        (HoldMode::AntiTarget, true, false, true),
        (HoldMode::Maneuver, true, false, false),
        (HoldMode::Maneuver, false, true, true),
    ];
    for (mode, target, burn, want) in table {
        assert_eq!(hold_available(mode, target, burn), want, "{mode:?} {target} {burn}");
    }
}

#[test]
fn the_line_frame_follows_the_switch() {
    use crate::trajectory::LineFrame::*;
    use sim::frame::NodeId;
    let (earth, moon) = (NodeId(3), NodeId(4));
    // (mode, target body, expected)
    let cases = [
        (Mode::Orbit, None, About(earth)),
        (Mode::Surface, None, Ground(earth)),
        (Mode::Surface, Some(moon), Ground(earth)),
        (Mode::Target, Some(moon), About(moon)),
        (Mode::Target, None, About(earth)),
    ];
    for (mode, target, expected) in cases {
        assert_eq!(line_frame(mode, earth, target), expected, "{mode:?} {target:?}");
    }
}

#[test]
fn the_throttle_caption_says_min_where_zero_is_the_engines_minimum() {
    // (throttle, a craft of parts' engines lit, caption)
    let table = [(0.0, false, "0%"), (0.0, true, "min"), (0.55, true, "55%"), (1.0, false, "100%")];
    for (t, lit, want) in table {
        assert_eq!(throttle_caption(t, lit), want, "{t} {lit}");
    }
}

#[test]
fn an_apsis_hover_says_above_what_and_from_where() {
    use crate::navball::ApsisReadout;
    let a = |other: Option<&str>| ApsisReadout {
        altitude: 301_200.0,
        above_datum: 301_200.0,
        distance: 6_679_337.0,
        in_s: 725.0,
        other_body: other.map(str::to_string),
    };
    let k = crate::format::Known::default();
    // (apsis, reference body, hover)
    let table = [
        (a(None), "Earth", "301.2 km above Earth's datum\n6679.3 km from Earth's centre"),
        (a(Some("Moon")), "Earth", "301.2 km above Moon's datum\n6679.3 km from Moon's centre"),
    ];
    for (apsis, body, want) in table {
        assert_eq!(apsis_hover(&apsis, body, &k), want);
    }
}
