use sunscatter_app::render::formatting::*;

#[test]
fn format_duration_zero() {
    assert_eq!(format_duration(0.0), "0s");
}

#[test]
fn format_duration_mixed_units() {
    assert_eq!(format_duration(3661.0), "1h 1m 1s");
    assert_eq!(format_duration(90061.0), "1d 1h 1m 1s");
}

#[test]
fn format_duration_year_threshold() {
    // 1 year + 1 day + 1 hour + 1 minute
    let secs = 365.0 * 86400.0 + 86400.0 + 3600.0 + 60.0;
    assert_eq!(format_duration(secs), "1y 1d 1h 1m");
}

#[test]
fn format_duration_edge_cases() {
    assert_eq!(format_duration(f64::NAN), "---");
    assert_eq!(format_duration(-1.0), "---");
    assert_eq!(format_duration(f64::INFINITY), "---");
    assert_eq!(format_duration(f64::NEG_INFINITY), "---");
}

#[test]
fn format_duration_no_seconds_omits_seconds() {
    assert_eq!(format_duration_no_seconds(3661.0), "1h 1m");
    assert_eq!(format_duration_no_seconds(90061.0), "1d 1h 1m");
}

#[test]
fn format_duration_no_seconds_edge_cases() {
    assert_eq!(format_duration_no_seconds(f64::NAN), "---");
    assert_eq!(format_duration_no_seconds(-5.0), "---");
}

#[test]
fn format_distance_unit_transitions() {
    assert_eq!(format_distance(500.0), "500 m");
    assert_eq!(format_distance(5000.0), "5.0 km");
    assert_eq!(format_distance(5e9), "5000.0 Mm");
    // AU threshold: >= 0.1 AU
    let au = 1.496e11;
    let result = format_distance(au);
    assert!(result.contains("AU"), "Expected AU, got: {result}");
}

#[test]
fn format_mass_thresholds() {
    let below = format_mass(500_000.0);
    assert!(below.contains("kg"), "got: {below}");
    assert!(!below.contains("e"), "Should not be scientific: {below}");

    let above = format_mass(2e6);
    assert!(above.contains("e"), "Should be scientific: {above}");

    let huge = format_mass(5.972e24);
    assert!(huge.contains("e"), "Should be scientific: {huge}");
}

#[test]
fn format_power_si_transitions() {
    assert_eq!(format_power_si(500.0), "500 W");
    assert_eq!(format_power_si(5000.0), "5.0 kW");
    assert_eq!(format_power_si(5e6), "5.0 MW");
    assert_eq!(format_power_si(5e9), "5.0 GW");
    assert_eq!(format_power_si(5e12), "5.0 TW");
}

#[test]
fn format_pressure_ranges() {
    let high = format_pressure(101_325.0);
    assert!(high.contains("1.00 atm"), "got: {high}");

    let low = format_pressure(500.0);
    assert!(low.contains("atm"), "got: {low}");
}

#[test]
fn blackbody_color_below_threshold() {
    assert!(blackbody_color(400.0).is_none());
    assert!(blackbody_color(0.0).is_none());
}

#[test]
fn blackbody_color_at_threshold() {
    let c = blackbody_color(500.0).expect("Should return Some at 500K");
    assert!(c[0] > 0.0, "Red should be positive");
}

#[test]
fn blackbody_color_progression() {
    let c2000 = blackbody_color(2000.0).unwrap();
    let c4000 = blackbody_color(4000.0).unwrap();
    // At higher temp, green should be more present
    assert!(c4000[1] > c2000[1], "Green should increase with temperature");
}

#[test]
fn apply_heat_tint_below_threshold() {
    let color = [0.5, 0.5, 0.5, 1.0];
    let result = apply_heat_tint(color, 300.0);
    assert_eq!(result, color, "Below 500K should return unchanged color");
}

#[test]
fn apply_heat_tint_above_threshold() {
    let color = [0.5, 0.5, 0.5, 1.0];
    let result = apply_heat_tint(color, 2000.0);
    // Red channel should increase (blended with glow)
    assert!(result[0] > color[0], "Red should increase from heat tint");
    // Alpha should be preserved
    assert_eq!(result[3], 1.0);
}
