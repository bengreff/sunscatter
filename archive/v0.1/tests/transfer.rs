mod common;

use std::f64::consts::{PI, TAU};
use sunscatter_app::bodies::G;
use sunscatter_app::ship::transfer::{solve_lambert_2d, normalize_angle, compute_hohmann};

// ============================================================
// Lambert solver
// ============================================================

#[test]
fn lambert_earth_to_mars_hohmann() {
    let mu = G * common::SUN_MASS;
    let r_earth = common::EARTH_SMA;
    let r_mars = common::MARS_SMA;

    // Near-Hohmann transfer: depart [r_earth, 0], arrive slightly off 180°
    // (exact 180° is degenerate in 2D Lambert — ambiguous transfer plane)
    let a_t = (r_earth + r_mars) / 2.0;
    let tof = PI * (a_t.powi(3) / mu).sqrt();

    let r1 = [r_earth, 0.0];
    // Offset arrival by ~5° from exact anti-podal to avoid degeneracy
    let arrival_angle = PI - 0.09;
    let r2 = [r_mars * arrival_angle.cos(), r_mars * arrival_angle.sin()];

    let result = solve_lambert_2d(r1, r2, tof, mu, true);
    assert!(result.is_some(), "Lambert should converge for near-Hohmann transfer");

    let lambert = result.unwrap();
    // Earth's circular velocity at [r_earth, 0] is [0, v_earth]
    let v_earth = (mu / r_earth).sqrt();
    let dv_x = lambert.v1[0] - 0.0;
    let dv_y = lambert.v1[1] - v_earth;
    let dv = (dv_x.powi(2) + dv_y.powi(2)).sqrt();

    // Departure Δv for Earth→Mars transfer ≈ 2.9 km/s (within 30%)
    common::assert_relative(dv, 2_900.0, 0.30, "Lambert departure delta-v");
}

#[test]
fn lambert_degenerate_same_position() {
    let mu = G * common::SUN_MASS;
    let pos = [common::EARTH_SMA, 0.0];
    // Same position → 0° or 360° transfer, should fail (degenerate)
    let result = solve_lambert_2d(pos, pos, 86400.0 * 180.0, mu, true);
    assert!(result.is_none(), "Same position should be degenerate");
}

#[test]
fn lambert_negative_tof() {
    let mu = G * common::SUN_MASS;
    let r1 = [common::EARTH_SMA, 0.0];
    let r2 = [-common::MARS_SMA, 0.0];
    let result = solve_lambert_2d(r1, r2, -1000.0, mu, true);
    assert!(result.is_none(), "Negative TOF should return None");
}

#[test]
fn lambert_zero_length_vectors() {
    let mu = G * common::SUN_MASS;
    let result = solve_lambert_2d([0.0, 0.0], [common::MARS_SMA, 0.0], 86400.0 * 180.0, mu, true);
    assert!(result.is_none(), "Zero-length r1 should return None");

    let result = solve_lambert_2d([common::EARTH_SMA, 0.0], [0.0, 0.0], 86400.0 * 180.0, mu, true);
    assert!(result.is_none(), "Zero-length r2 should return None");
}

// ============================================================
// normalize_angle
// ============================================================

#[test]
fn normalize_angle_basic() {
    common::assert_close(normalize_angle(0.0), 0.0, 1e-12, "0");
    common::assert_close(normalize_angle(PI + 0.1), -(PI - 0.1), 1e-12, "PI+0.1");
    common::assert_close(normalize_angle(-PI - 0.1), PI - 0.1, 1e-12, "-PI-0.1");
}

#[test]
fn normalize_angle_multiples() {
    common::assert_close(normalize_angle(TAU), 0.0, 1e-12, "TAU");
    common::assert_close(normalize_angle(-TAU), 0.0, 1e-12, "-TAU");
    common::assert_close(normalize_angle(3.0 * TAU + 1.0), 1.0, 1e-12, "3*TAU+1");
}

// ============================================================
// Hohmann transfer (compute_hohmann)
// ============================================================

#[test]
fn hohmann_earth_to_moon() {
    let parent_mass = common::EARTH_MASS;

    // Ship in circular LEO at 400 km
    let ship_sma = common::EARTH_RADIUS + 400_000.0;
    let ship_orbit = common::make_orbit(ship_sma, 0.001, 0.0, 0.0);

    // Moon orbit
    let target_orbit = common::make_orbit(common::MOON_SMA, 0.0549, 0.0, 0.0);

    let result = compute_hohmann(
        &ship_orbit,
        false, // prograde
        0.0,   // ship mean anomaly
        &target_orbit,
        parent_mass,
        0.0,   // sim_time
    );

    assert!(result.is_some(), "Hohmann should succeed for Earth→Moon");
    let h = result.unwrap();

    // Departure Δv should be reasonable (< 5 km/s)
    assert!(h.departure_delta_v < 5_000.0,
        "Departure Δv too high: {}", h.departure_delta_v);
    assert!(h.departure_delta_v > 1_000.0,
        "Departure Δv too low: {}", h.departure_delta_v);

    // Transfer time should be reasonable (hours to days)
    assert!(h.transfer_time > 3600.0, "Transfer time too short");
    assert!(h.transfer_time < 86400.0 * 10.0, "Transfer time too long");
}
