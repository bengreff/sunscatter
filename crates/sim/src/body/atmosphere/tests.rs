use super::*;

fn earth() -> Atmosphere {
    crate::body::earth().atmosphere.unwrap()
}

/// U.S. Standard Atmosphere 1976 (NOAA-S/T 76-1562), Table I, geometric
/// altitude: (m, T K, p Pa, ρ kg/m³). 5, 11, 25 and 75 km fall between
/// the data's rows (interpolated).
const US76: [(f64, f64, f64, f64); 10] = [
    (0.0, 288.150, 101_325.0, 1.2250),
    (5_000.0, 255.676, 54_048.0, 0.73643),
    (11_000.0, 216.774, 22_699.0, 0.36480),
    (20_000.0, 216.650, 5_529.3, 0.088910),
    (25_000.0, 221.552, 2_549.2, 0.040084),
    (50_000.0, 270.650, 79.779, 1.0269e-3),
    (75_000.0, 208.399, 2.3881, 3.9921e-5),
    (100_000.0, 195.08, 3.2011e-2, 5.604e-7),
    (400_000.0, 995.83, 1.4518e-6, 2.803e-12),
    (1_000_000.0, 1000.0, 7.5138e-9, 3.561e-15),
];

#[test]
fn earth_matches_the_us_standard_atmosphere_1976() {
    let atm = earth();
    for (h, t, p, rho) in US76 {
        let s = atm.profile(h, 9.80665);
        assert!((s.temperature - t).abs() < 0.02, "{h} m: T {} vs {t}", s.temperature);
        // Log-linear interpolation between rows: within 0.1 % of the standard.
        assert!((s.pressure / p - 1.0).abs() < 1e-3, "{h} m: p {} vs {p}", s.pressure);
        assert!((s.rho / rho - 1.0).abs() < 1e-3, "{h} m: ρ {} vs {rho}", s.rho);
        // The gas law holds with the tabulated molar mass (R = 8.31432 in US 1976).
        let m = s.rho * 8.314_32 * s.temperature / s.pressure;
        assert!((m / s.molar_mass - 1.0).abs() < 3e-3, "{h} m: M {m} vs {}", s.molar_mass);
    }
}

#[test]
fn earth_air_properties() {
    let atm = earth();
    // (m, speed of sound m/s, mean free path m, mean molar mass kg/mol): US
    // 1976 Table I (speed of sound to 86 km only; mean molecular weight
    // 28.40 at 100 km, 15.98 at 400 km).
    let cases = [
        (0.0, Some(340.294), 6.6328e-8, 0.028_964_4),
        (11_000.0, Some(295.154), 2.2274e-7, 0.028_964_4),
        (20_000.0, Some(295.069), 9.1393e-7, 0.028_964_4),
        (50_000.0, Some(329.799), 7.9131e-5, 0.028_964_4),
        (100_000.0, None, 1.421e-1, 0.028_40),
        (400_000.0, None, 1.6e4, 0.015_98),
    ];
    for (h, a, l, m) in cases {
        let s = atm.profile(h, 9.80665);
        if let Some(a) = a {
            let got = s.speed_of_sound(atm.gamma);
            assert!((got - a).abs() < 0.05, "{h} m: a {got} vs {a}");
        }
        assert!((s.mean_free_path / l - 1.0).abs() < 3e-3, "{h} m: λ {} vs {l}", s.mean_free_path);
        assert!((s.molar_mass / m - 1.0).abs() < 2e-3, "{h} m: M {} vs {m}", s.molar_mass);
    }
}

#[test]
fn flight_sees_no_air_above_the_top() {
    let atm = earth();
    assert_eq!(atm.top, 150_000.0);
    assert!(atm.air(149_000.0, 9.8).is_some());
    assert!(atm.air(150_000.0, 9.8).is_none());
    assert_eq!(atm.density(150_000.0), 0.0);
    assert_eq!(atm.pressure(200_000.0), 0.0);
    assert_eq!(atm.density(10_000.0), atm.profile(10_000.0, 9.8).rho);
    // Below sea level: the first row's values; beyond the last row, the
    // last interval continued (still falling).
    assert_eq!(atm.profile(-100.0, 9.8).rho, atm.profile(0.0, 9.8).rho);
    let (a, b) = (atm.profile(1.0e6, 9.8), atm.profile(1.2e6, 9.8));
    assert!(b.rho < a.rho && b.pressure < a.pressure && b.temperature == a.temperature);
}

#[test]
fn density_is_continuous_and_falling() {
    let atm = earth();
    let mut last = f64::INFINITY;
    for i in 0..=200_000 {
        let h = i as f64 * 5.0;
        let s = atm.profile(h, 9.8);
        assert!(s.rho < last, "{h}");
        if last.is_finite() {
            // No jumps: at most a few ‰ per 5 m.
            assert!(s.rho / last > 0.99, "{h}");
        }
        last = s.rho;
    }
}

#[test]
fn bodies_without_a_table_keep_the_exponential_model() {
    let atm = Atmosphere {
        rho0: 1.225,
        scale_height: 7_200.0,
        top: 150_000.0,
        p0: 101_325.0,
        gamma: 1.4,
        molar_mass: 0.029,
        mean_free_path: 6.6e-8,
        collision_diameter: EARTH_AIR_COLLISION_DIAMETER,
        sutton_graves_k: 1.7415e-4,
        radiative_heating: None,
        table: None,
    };
    let s = atm.profile(7_200.0, 9.80665);
    assert!((s.rho - 1.225 / std::f64::consts::E).abs() < 1e-12);
    assert!((s.pressure - 101_325.0 / std::f64::consts::E).abs() < 1e-9);
    assert!((s.mean_free_path - 6.6e-8 * std::f64::consts::E).abs() < 1e-18);
    // T = H·g₀·M/R.
    assert!((s.temperature - 7_200.0 * 9.80665 * 0.029 / air::GAS_CONSTANT).abs() < 1e-9);
    assert_eq!(atm.air(7_200.0, 9.80665), Some(s));
}

#[test]
fn bad_tables_are_rejected() {
    let row = |h: f64| (h, 200.0, 1.0, 1.0, 0.029);
    assert!(AtmosphereTable::try_from(vec![row(0.0)]).is_err());
    assert!(AtmosphereTable::try_from(vec![row(0.0), row(0.0)]).is_err());
    assert!(AtmosphereTable::try_from(vec![row(0.0), (1.0, 200.0, 0.0, 1.0, 0.029)]).is_err());
    assert!(AtmosphereTable::try_from(vec![row(0.0), row(1.0)]).is_ok());
}
