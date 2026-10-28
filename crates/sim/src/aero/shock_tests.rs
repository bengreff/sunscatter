//! The shock layer against published equilibrium-air data and entries.

use super::equilibrium::{mixture, AIR_OXYGEN_FRACTION};
use super::gas::Gas;
use super::shock::*;
use crate::comms::{blacked_out, plasma_frequency};

fn earth_air(h: f64) -> (f64, f64, f64) {
    let a = crate::body::earth().atmosphere.unwrap().profile(h, 9.806_65);
    (a.rho, a.temperature, a.mean_free_path)
}

#[test]
fn the_compression_is_hunt_and_souders() {
    // Their 90° rows (NASA SP-3093): 53.34 km to 7.9 km/s, 60.96 km to
    // 9.8 km/s, 68.58 km above: the table `aero::air` already carries.
    for (h, v) in [
        (53_340.0, 3_048.0),
        (53_340.0, 4_876.8),
        (53_340.0, 6_096.0),
        (53_340.0, 7_924.8),
        (60_960.0, 8_534.4),
        (60_960.0, 9_753.6),
        (68_580.0, 10_972.8),
        (68_580.0, 11_582.4),
    ] {
        let (rho, t, _) = earth_air(h);
        let s = normal_shock(v, rho, t, AIR_OXYGEN_FRACTION).unwrap();
        let published = Gas::Air.shock_density_ratio(v);
        assert!((s.compression / published - 1.0).abs() < 0.01, "{v} m/s: {} against {published}", s.compression);
    }
}

#[test]
fn the_temperature_saturates_where_a_perfect_gas_s_grows() {
    // Anderson, Hypersonic and High-Temperature Gas Dynamics, §14.6: at
    // 36,000 ft/s and 52 km, 11,600 K in equilibrium against 58,128 K for
    // a perfect gas.
    let (rho, t, _) = earth_air(52_000.0);
    let s = normal_shock(10_972.8, rho, t, AIR_OXYGEN_FRACTION).unwrap();
    assert!((s.temperature / 11_600.0 - 1.0).abs() < 0.03, "{}", s.temperature);
    let perfect = |m: f64| t * (2.8 * m * m - 0.4) * (0.4 * m * m + 2.0) / (5.76 * m * m);
    let a = (1.4 * 287.05 * t).sqrt();
    assert!(perfect(10_972.8 / a) > 4.5 * s.temperature);
    // At 1 km/s (Mach 3.1) air is still nearly a perfect gas: within 3 %
    // (vibration takes a little).
    let s = normal_shock(1_000.0, 1e-3, t, AIR_OXYGEN_FRACTION).unwrap();
    let expected = perfect(1_000.0 / a);
    assert!((s.temperature / expected - 1.0).abs() < 0.03, "{} against {expected}", s.temperature);
}

#[test]
fn air_dissociates_then_ionises_in_the_textbook_sequence() {
    // Anderson §11.12 at 1 atm: O₂ gone by ~4,000–5,000 K, N₂ half
    // dissociated near 7,000 K, ionisation of order 1 % by 10,000 K.
    let x = |t: f64| {
        let m = mixture(t, 101_325.0, AIR_OXYGEN_FRACTION, None);
        let total: f64 = m.n.iter().sum();
        m.n.map(|n| n / total)
    };
    assert!(x(1_000.0)[1] > 0.2 && x(1_000.0)[10] < 1e-20);
    assert!(x(5_000.0)[1] < 0.01, "O₂ {}", x(5_000.0)[1]);
    let n = x(7_000.0);
    assert!((0.3..0.7).contains(&(n[3] / (n[3] + 2.0 * n[0]))), "N {n:?}");
    assert!((0.005..0.05).contains(&x(10_000.0)[10]), "e {}", x(10_000.0)[10]);
    // Pure nitrogen (Titan) has no oxygen species.
    let m = mixture(8_000.0, 1e4, 0.0, None);
    assert_eq!((m.n[1], m.n[2], m.n[4]), (0.0, 0.0, 0.0));
    assert!(m.n[3] > 0.0 && m.electron_density() > 0.0);
}

#[test]
fn the_table_reads_the_jump_off_its_grid() {
    for (v, rho) in [(7_430.0, 3.3e-5), (11_100.0, 2.7e-4), (3_210.0, 7e-3)] {
        let (direct, table) =
            (normal_shock(v, rho, FREE_STREAM_K, AIR_OXYGEN_FRACTION).unwrap(), jump(Gas::Air, v, rho).unwrap());
        assert!((table.temperature / direct.temperature - 1.0).abs() < 0.02, "{v}: {table:?} {direct:?}");
        assert!((table.compression / direct.compression - 1.0).abs() < 0.02);
        assert!((table.pressure / direct.pressure - 1.0).abs() < 0.01);
        assert!((table.electron_density / direct.electron_density - 1.0).abs() < 0.3, "{v}: {table:?} {direct:?}");
    }
    assert!(jump(Gas::CarbonDioxide, 6_000.0, 1e-4).is_none());
    assert!(jump(Gas::Air, 900.0, 1e-4).is_none());
    assert!(jump(Gas::Nitrogen, 6_000.0, 1e-4).unwrap().temperature > 4_000.0);
}

#[test]
fn the_standoff_is_olivier_s_and_agrees_with_lobb_s_fit() {
    // Lobb (1964), spheres in a ballistic range: Δ/R ≈ 0.78·ε.
    for compression in [6.0, 10.0, 14.0, 17.6] {
        let d = standoff(1.0, compression);
        assert!((d / (0.78 / compression) - 1.0).abs() < 0.1, "{compression}: {d}");
    }
    assert_eq!(standoff(2.0, 12.0), 2.0 * standoff(1.0, 12.0));
}

#[test]
fn the_plasma_frequency_cuts_s_band_near_6e16_per_cubic_metre() {
    assert!((plasma_frequency(1e18) / 8.98e9 - 1.0).abs() < 1e-3);
    // S band (2.2 GHz): (f/8.98)² = 6.0e16 m⁻³.
    assert!(!blacked_out(2.2e9, 5.9e16) && blacked_out(2.2e9, 6.1e16));
}

/// The sheath's electron density at a sphere's nose of radius `rn` (m) at
/// altitude `h` (m) and speed `v` (m/s): equilibrium times the shares of
/// equilibrium and continuum (as `vessel::aerothermal::shock_layer`).
fn sheath(h: f64, v: f64, rn: f64) -> f64 {
    let (rho, _, lambda) = earth_air(h);
    let s = jump(Gas::Air, v, rho).unwrap();
    let share = equilibrium_share(standoff(rn, s.compression), s.compression, lambda);
    s.electron_density * share * (1.0 - super::bridge(lambda / rn))
}

#[test]
fn a_thin_layer_falls_short_of_equilibrium() {
    // FIRE II at 1643.5 s (53 km, Rn 0.935 m) near equilibrium; at 1634 s
    // (76 km) far from it; entry interface (122 km) not ionised at all.
    let share = |h: f64, rn: f64| {
        let (_, _, lambda) = earth_air(h);
        equilibrium_share(standoff(rn, 15.0), 15.0, lambda)
    };
    assert!(share(53_040.0, 0.935) > 0.4, "{}", share(53_040.0, 0.935));
    assert!(share(76_420.0, 0.935) < 0.1, "{}", share(76_420.0, 0.935));
    assert!(share(122_000.0, 2.0) < 1e-3);
    assert!(share(10_000.0, 1.0) > 0.999);
}

#[test]
fn published_entries_black_out_s_band_at_peak_heating_and_not_at_entry_interface() {
    const S_BAND: f64 = 2.2e9;
    // In blackout at peak heating, as every crewed and sample-return entry
    // was: Stardust (61 km, 11.0 km/s, Rn 0.22 m; Desai et al.), FIRE II
    // at 1643.5 s (53 km, 10.48 km/s, Rn 0.935 m; Cauchon, NASA TM
    // X-1402), a Shuttle-like LEO return (70 km, 6.8 km/s, Rn 1 m).
    for (h, v, rn) in [(61_000.0, 11_000.0, 0.22), (53_040.0, 10_480.0, 0.935), (70_000.0, 6_800.0, 1.0)] {
        let ne = sheath(h, v, rn);
        assert!(blacked_out(S_BAND, ne), "{h} m, {v} m/s: {ne:.2e}");
    }
    // Not at entry interface (122 km), where the links still held.
    assert!(!blacked_out(S_BAND, sheath(122_000.0, 11_000.0, 2.0)));
    // Nor once slowed below the windows' ends (2.5 km/s at 40 km).
    assert!(!blacked_out(S_BAND, sheath(40_000.0, 2_500.0, 2.0)), "{:.2e}", sheath(40_000.0, 2_500.0, 2.0));
}
