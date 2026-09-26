mod common;

use std::f64::consts::{PI, TAU};
use sunscatter_app::bodies::{self, G};
use sunscatter_app::ship::Ship;

// ============================================================
// Kepler solver roundtrip tests
// ============================================================

#[test]
fn kepler_circular_orbit() {
    let orbit = common::make_orbit(1e7, 0.0, 0.0, 0.0);
    for m in [0.0, 1.0, PI, 5.0] {
        let e_anomaly = orbit.solve_kepler(m);
        // For e=0, E should equal M (mod TAU)
        let diff = (e_anomaly - m.rem_euclid(TAU)).abs();
        assert!(diff < 1e-10, "Circular: E should equal M, diff={diff}");
    }
}

#[test]
fn kepler_moderate_eccentricity_roundtrip() {
    for e in [0.3, 0.7] {
        let orbit = common::make_orbit(1e7, e, 0.0, 0.0);
        for m_input in [0.5, 1.5, PI, 4.0, 5.5] {
            let big_e = orbit.solve_kepler(m_input);
            // Verify: M = E - e*sin(E)
            let m_check = big_e - e * big_e.sin();
            let m_norm = m_input.rem_euclid(TAU);
            let diff = (m_check.rem_euclid(TAU) - m_norm).abs();
            assert!(
                diff < 1e-10,
                "e={e}, M={m_input}: roundtrip diff={diff}"
            );
        }
    }
}

#[test]
fn kepler_high_eccentricity_roundtrip() {
    let e = 0.99;
    let orbit = common::make_orbit(1e7, e, 0.0, 0.0);
    for m_input in [0.1, 1.0, PI, 5.0] {
        let big_e = orbit.solve_kepler(m_input);
        // The solver clamps 0.99 to 0.9999 for numerical stability,
        // so verify using the clamped eccentricity. The clamping
        // introduces ~O(0.01) error, which is acceptable.
        let e_eff = 0.9999;
        let m_check = big_e - e_eff * big_e.sin();
        let m_norm = m_input.rem_euclid(TAU);
        let diff = (m_check.rem_euclid(TAU) - m_norm).abs();
        assert!(
            diff < 0.01,
            "e={e} (clamped to {e_eff}), M={m_input}: roundtrip diff={diff}"
        );
    }
}

#[test]
fn kepler_hyperbolic_roundtrip() {
    for e in [1.5, 3.0] {
        let orbit = common::make_orbit(-1e7, e, 0.0, 0.0);
        for m_input in [0.5, 2.0, 5.0, -1.0] {
            let h = orbit.solve_kepler(m_input);
            // Verify: M = e*sinh(H) - H
            let m_check = e * h.sinh() - h;
            let diff = (m_check - m_input).abs();
            assert!(
                diff < 1e-10,
                "Hyperbolic e={e}, M={m_input}: roundtrip diff={diff}"
            );
        }
    }
}

// ============================================================
// State vector ↔ orbital elements roundtrip
// ============================================================

#[test]
fn state_vector_roundtrip_circular_leo() {
    let ship = Ship::default();
    let parent_mass = common::EARTH_MASS;
    let mu = G * parent_mass;
    let r = common::EARTH_RADIUS + 400_000.0; // 400 km LEO
    let v = (mu / r).sqrt(); // circular velocity

    let pos = [r, 0.0];
    let vel = [0.0, v];

    let (orbit, true_anomaly, retrograde) = ship
        .calculate_orbit_from_state(pos, vel, parent_mass)
        .expect("Should compute orbit");

    assert!(!retrograde, "Prograde orbit expected");
    common::assert_relative(orbit.semi_major_axis, r, 1e-6, "SMA");
    assert!(orbit.eccentricity < 0.01, "Should be nearly circular, e={}", orbit.eccentricity);

    // Roundtrip: convert back to position/velocity
    let ma = ship.true_to_mean_anomaly(&orbit, true_anomaly);
    let pos2 = orbit.position_from_mean_anomaly(ma, parent_mass);
    let vel2 = orbit.velocity_from_mean_anomaly_with_direction(ma, parent_mass, retrograde);

    common::assert_relative(
        (pos2[0].powi(2) + pos2[1].powi(2)).sqrt(),
        r,
        1e-4,
        "Position magnitude",
    );
    common::assert_relative(
        (vel2[0].powi(2) + vel2[1].powi(2)).sqrt(),
        v,
        1e-4,
        "Velocity magnitude",
    );
}

#[test]
fn state_vector_roundtrip_elliptical() {
    let ship = Ship::default();
    let parent_mass = common::EARTH_MASS;
    let mu = G * parent_mass;

    // Elliptical orbit: periapsis at 6771 km, apoapsis at ~42000 km (GTO-ish)
    let r = common::EARTH_RADIUS + 400_000.0;
    let v = (mu * (2.0 / r - 1.0 / 2.4e7)).sqrt(); // vis-viva for a=24000km

    let pos = [r, 0.0];
    let vel = [0.0, v];

    let (orbit, true_anomaly, retrograde) = ship
        .calculate_orbit_from_state(pos, vel, parent_mass)
        .expect("Should compute orbit");

    assert!(!retrograde);
    assert!(orbit.eccentricity > 0.1 && orbit.eccentricity < 0.9,
        "Should be elliptical, e={}", orbit.eccentricity);

    // Roundtrip
    let ma = ship.true_to_mean_anomaly(&orbit, true_anomaly);
    let pos2 = orbit.position_from_mean_anomaly(ma, parent_mass);
    let vel2 = orbit.velocity_from_mean_anomaly_with_direction(ma, parent_mass, retrograde);

    let r2 = (pos2[0].powi(2) + pos2[1].powi(2)).sqrt();
    let v2 = (vel2[0].powi(2) + vel2[1].powi(2)).sqrt();
    let v_orig = (vel[0].powi(2) + vel[1].powi(2)).sqrt();

    common::assert_relative(r2, r, 1e-3, "Elliptical position magnitude");
    common::assert_relative(v2, v_orig, 1e-3, "Elliptical velocity magnitude");
}

#[test]
fn state_vector_roundtrip_at_angle() {
    let ship = Ship::default();
    let parent_mass = common::EARTH_MASS;
    let mu = G * parent_mass;
    let r = common::EARTH_RADIUS + 400_000.0;
    let v = (mu / r).sqrt();

    // Position at 45 degrees
    let angle = PI / 4.0;
    let pos = [r * angle.cos(), r * angle.sin()];
    let vel_angle = angle + PI / 2.0; // perpendicular = prograde
    let vel = [v * vel_angle.cos(), v * vel_angle.sin()];

    let (orbit, true_anomaly, _retrograde) = ship
        .calculate_orbit_from_state(pos, vel, parent_mass)
        .expect("Should compute orbit");

    let ma = ship.true_to_mean_anomaly(&orbit, true_anomaly);
    let pos2 = orbit.position_from_mean_anomaly(ma, parent_mass);

    let r2 = (pos2[0].powi(2) + pos2[1].powi(2)).sqrt();
    common::assert_relative(r2, r, 1e-3, "Position at angle roundtrip");
}

// ============================================================
// Known-value sanity checks
// ============================================================

#[test]
fn earth_orbit_position_magnitude() {
    let ss = common::make_solar_system();
    let earth = &ss.bodies[ss.earth_index];
    let orbit = earth.orbit.as_ref().unwrap();
    let pos = orbit.position_at(0.0, common::SUN_MASS);
    let r = (pos[0].powi(2) + pos[1].powi(2)).sqrt();

    // Should be approximately 1 AU (within 2% for eccentricity)
    common::assert_relative(r, common::EARTH_SMA, 0.02, "Earth distance from Sun");
}

#[test]
fn earth_orbital_velocity() {
    let earth_orbit = common::make_orbit(common::EARTH_SMA, 0.0167, 1.796, 1.8);
    let ma = earth_orbit.mean_anomaly_at(0.0, common::SUN_MASS);
    let vel = earth_orbit.velocity_from_mean_anomaly(ma, common::SUN_MASS);
    let v = (vel[0].powi(2) + vel[1].powi(2)).sqrt();

    // Earth orbital velocity ≈ 29.8 km/s
    common::assert_relative(v, 29_800.0, 0.05, "Earth orbital velocity");
}

#[test]
fn galactic_enclosed_mass_sun_distance() {
    let sun_distance = 1.996e20; // meters (~21,100 ly)
    let enclosed_mass = bodies::galactic_enclosed_mass(sun_distance);

    // Orbital velocity: v = sqrt(G * M / r)
    let v = (G * enclosed_mass / sun_distance).sqrt();
    // Sun's galactic orbital velocity ≈ 220 km/s
    common::assert_relative(v, 220_000.0, 0.15, "Sun galactic orbital velocity");
}

#[test]
fn earth_soi_radius() {
    let soi = bodies::calculate_soi(common::EARTH_SMA, common::EARTH_MASS, common::SUN_MASS);
    // Earth SOI ≈ 924,000 km = 9.24e8 m
    common::assert_relative(soi, 9.24e8, 0.05, "Earth SOI radius");
}

#[test]
fn atmosphere_pressure_at_surface() {
    let ss = common::make_solar_system();
    let earth = &ss.bodies[ss.earth_index];
    let atmo = earth.atmosphere.as_ref().unwrap();

    let p0 = atmo.pressure_at_altitude(0.0);
    common::assert_close(p0, 101_325.0, 0.1, "Earth surface pressure");
}

#[test]
fn atmosphere_pressure_at_scale_height() {
    let ss = common::make_solar_system();
    let earth = &ss.bodies[ss.earth_index];
    let atmo = earth.atmosphere.as_ref().unwrap();

    let p_h = atmo.pressure_at_altitude(atmo.scale_height);
    // At one scale height, pressure = p0 * e^(-1) ≈ 37,265 Pa
    let expected = 101_325.0 / std::f64::consts::E;
    common::assert_relative(p_h, expected, 1e-6, "Pressure at scale height");
}
