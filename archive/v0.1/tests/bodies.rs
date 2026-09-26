mod common;

use sunscatter_app::bodies::SolarSystem;

#[test]
fn earth_surface_gravity() {
    let ss = SolarSystem::new();
    let g = ss.bodies[ss.earth_index].surface_gravity();
    common::assert_close(g, 9.81, 0.05, "Earth surface gravity");
}

#[test]
fn moon_surface_gravity() {
    let ss = SolarSystem::new();
    let g = ss.bodies[ss.moon_index].surface_gravity();
    common::assert_close(g, 1.625, 0.05, "Moon surface gravity");
}

#[test]
fn sgr_a_star_is_compact() {
    let ss = SolarSystem::new();
    assert!(
        ss.bodies[0].is_compact(),
        "Sgr A* (body index 0) should be compact (black hole)"
    );
}

#[test]
fn sun_not_compact() {
    let ss = SolarSystem::new();
    assert!(
        !ss.bodies[ss.sun_index].is_compact(),
        "Sun should not be compact"
    );
}

#[test]
fn all_bodies_nonneg_landing_science() {
    let ss = SolarSystem::new();
    for (i, body) in ss.bodies.iter().enumerate() {
        let val = body.landing_science_value();
        assert!(
            val >= 0.0,
            "Body {} ({}) has negative landing_science_value: {}",
            i,
            body.name,
            val
        );
    }
}

#[test]
fn position_to_sector_sun() {
    use sunscatter_app::bodies::position_to_sector;

    // Compute the Sun's galactic position from its orbital elements
    let (a, e, m0) = (1.996e20_f64, 0.07_f64, 4.8534_f64);
    let big_e = sunscatter_app::galaxy::solve_kepler_nr(m0, e);
    let nu = 2.0
        * ((1.0 + e).sqrt() * (big_e / 2.0).sin())
            .atan2((1.0 - e).sqrt() * (big_e / 2.0).cos());
    let r = a * (1.0 - e * big_e.cos());
    let sun_pos = [r * nu.cos(), r * nu.sin()];

    let sector = position_to_sector(sun_pos).expect("Sun position should map to a valid sector");
    assert!(
        (sector.x as i32 - 500).abs() <= 5,
        "Sun sector x={} should be near 500",
        sector.x
    );
    assert!(
        (sector.y as i32 - 290).abs() <= 5,
        "Sun sector y={} should be near 290",
        sector.y
    );
}

#[test]
fn position_to_sector_out_of_bounds() {
    use sunscatter_app::bodies::position_to_sector;

    let result = position_to_sector([1e25, 1e25]);
    assert!(
        result.is_none(),
        "Extreme position [1e25, 1e25] should be out of bounds"
    );
}

#[test]
fn atmosphere_visible_height_earth() {
    let ss = SolarSystem::new();
    let earth = &ss.bodies[ss.earth_index];
    let atmo = earth.atmosphere.as_ref().expect("Earth should have an atmosphere");
    let vh = atmo.visible_height();
    common::assert_relative(vh, 100_000.0, 0.10, "Earth atmosphere visible_height");
}

#[test]
fn atmosphere_density_exponential_decay() {
    let ss = SolarSystem::new();
    let earth = &ss.bodies[ss.earth_index];
    let atmo = earth.atmosphere.as_ref().expect("Earth should have an atmosphere");

    let density_at_scale_height = atmo.density_at_altitude(atmo.scale_height);
    let expected = atmo.surface_density() / std::f64::consts::E;

    common::assert_relative(
        density_at_scale_height,
        expected,
        0.01,
        "Density at one scale height should be surface_density / e",
    );
}

#[test]
fn earth_surface_density() {
    let ss = SolarSystem::new();
    let earth = &ss.bodies[ss.earth_index];
    let atmo = earth.atmosphere.as_ref().expect("Earth should have an atmosphere");
    let rho = atmo.surface_density();
    common::assert_close(rho, 1.41, 0.1, "Earth surface density (ideal gas at 250K)");
}

#[test]
fn landing_altitude_atmospheric_vs_airless() {
    let ss = SolarSystem::new();

    // Earth (has atmosphere): landing_altitude == atmo.visible_height()
    let earth = &ss.bodies[ss.earth_index];
    let earth_atmo = earth.atmosphere.as_ref().expect("Earth should have an atmosphere");
    let earth_landing = earth.landing_altitude();
    common::assert_close(
        earth_landing,
        earth_atmo.visible_height(),
        1e-6,
        "Earth landing_altitude should equal atmosphere visible_height",
    );

    // Moon (no atmosphere): landing_altitude == radius * 0.01
    let moon = &ss.bodies[ss.moon_index];
    assert!(
        moon.atmosphere.is_none(),
        "Moon should have no atmosphere"
    );
    let moon_landing = moon.landing_altitude();
    common::assert_close(
        moon_landing,
        moon.radius * 0.01,
        1e-6,
        "Moon landing_altitude should equal radius * 0.01",
    );
}

// ============================================================
// Body name guards — hardcoded name lookups must resolve
// ============================================================

#[test]
fn hardcoded_body_names_exist() {
    let ss = SolarSystem::new();
    // Every body name used in string-based lookups across the codebase:
    // game.rs, contracts.rs, transfer.rs, bodies.rs, main.rs
    let required = ["Sagittarius A*", "Sun", "Earth", "Moon", "Mars"];
    for name in &required {
        assert!(
            ss.bodies.iter().any(|b| b.name == *name),
            "Body '{}' not found in SolarSystem — a string-based lookup depends on it",
            name
        );
    }
}

#[test]
fn landing_science_bodies_exist() {
    let ss = SolarSystem::new();
    // All names used in CelestialBody::landing_science_value() match arms
    let science_bodies = [
        "Moon", "Mars", "Mercury", "Venus", "Phobos", "Deimos",
        "Io", "Europa", "Ganymede", "Callisto",
    ];
    for name in &science_bodies {
        assert!(
            ss.bodies.iter().any(|b| b.name == *name),
            "Body '{}' not found — landing_science_value() has a match arm for it",
            name
        );
    }
}
