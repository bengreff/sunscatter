mod common;

use sunscatter_app::ship::{lorentz_gamma, gravitational_time_factor, relativistic_cruise_velocity, SPEED_OF_LIGHT};

#[test]
fn lorentz_gamma_zero_speed() {
    common::assert_close(lorentz_gamma(0.0), 1.0, 1e-12, "gamma at v=0");
}

#[test]
fn lorentz_gamma_below_threshold() {
    // 1000 m/s is well below the 0.01c threshold
    common::assert_close(lorentz_gamma(1000.0), 1.0, 1e-12, "gamma at 1000 m/s");
}

#[test]
fn lorentz_gamma_half_c() {
    let v = 0.5 * SPEED_OF_LIGHT;
    let expected = 1.0 / (1.0 - 0.25_f64).sqrt(); // 1/sqrt(0.75) ≈ 1.1547
    common::assert_relative(lorentz_gamma(v), expected, 1e-6, "gamma at 0.5c");
}

#[test]
fn lorentz_gamma_09c() {
    let v = 0.9 * SPEED_OF_LIGHT;
    let expected = 1.0 / (1.0 - 0.81_f64).sqrt(); // 1/sqrt(0.19) ≈ 2.294
    common::assert_relative(lorentz_gamma(v), expected, 1e-4, "gamma at 0.9c");
}

#[test]
fn gravitational_time_factor_non_compact() {
    // Non-compact body → always 1.0 regardless of parameters
    let factor = gravitational_time_factor(1e20, 1e7, false);
    common::assert_close(factor, 1.0, 1e-12, "Non-compact body");
}

#[test]
fn gravitational_time_factor_compact_far() {
    // Compact body but very far away → potential below threshold → 1.0
    let factor = gravitational_time_factor(1e10, 1e20, true);
    common::assert_close(factor, 1.0, 1e-12, "Compact body, far away");
}

#[test]
fn relativistic_cruise_velocity_zero() {
    common::assert_close(relativistic_cruise_velocity(0.0), 0.0, 1e-12, "cruise v at dv=0");
}

#[test]
fn relativistic_cruise_velocity_extreme() {
    // High Newtonian Δv → asymptotically approaches c
    // Use 3e9 m/s (~10c Newtonian) where tanh is still < 1.0 in f64
    let v = relativistic_cruise_velocity(3e9);
    assert!(v < SPEED_OF_LIGHT, "Must be below c, got {v}");
    assert!(v > 0.99999 * SPEED_OF_LIGHT, "Should be very close to c at high dv");
}

#[test]
fn relativistic_cruise_velocity_low() {
    // At low Δv, tanh(x) ≈ x, so result ≈ Δv
    let dv = 1000.0;
    let v = relativistic_cruise_velocity(dv);
    common::assert_relative(v, dv, 1e-6, "Low dv should equal Newtonian");
}
