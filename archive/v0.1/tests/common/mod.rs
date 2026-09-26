use sunscatter_app::bodies::{Orbit, SolarSystem};

// Physical constants
pub const EARTH_MASS: f64 = 5.972e24;
pub const SUN_MASS: f64 = 1.989e30;
pub const EARTH_SMA: f64 = 1.496e11; // meters
pub const EARTH_RADIUS: f64 = 6.371e6; // meters
pub const MOON_SMA: f64 = 3.844e8; // meters
pub const MARS_SMA: f64 = 2.279e11; // meters
pub const G: f64 = 6.67430e-11;

pub fn make_orbit(sma: f64, e: f64, arg_peri: f64, ma0: f64) -> Orbit {
    Orbit {
        semi_major_axis: sma,
        eccentricity: e,
        argument_of_periapsis: arg_peri,
        mean_anomaly_at_epoch: ma0,
    }
}

pub fn make_solar_system() -> SolarSystem {
    SolarSystem::new()
}

pub fn assert_close(actual: f64, expected: f64, tolerance: f64, msg: &str) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tolerance,
        "{msg}: expected {expected:.6e}, got {actual:.6e} (diff {diff:.6e}, tol {tolerance:.6e})"
    );
}

pub fn assert_relative(actual: f64, expected: f64, rel_tol: f64, msg: &str) {
    let diff = (actual - expected).abs();
    let scale = expected.abs().max(1e-30);
    assert!(
        diff / scale <= rel_tol,
        "{msg}: expected {expected:.6e}, got {actual:.6e} (rel diff {:.4e}, tol {rel_tol:.4e})",
        diff / scale
    );
}
