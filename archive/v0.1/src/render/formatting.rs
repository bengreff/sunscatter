/// Format a duration in seconds to a human-readable string
pub fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "---".to_string();
    }
    let total = seconds as u64;
    let y = total / (86400 * 365);
    let d = (total / 86400) % 365;
    let h = (total % 86400) / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if y > 0 {
        format!("{}y {}d {}h {}m", y, d, h, m)
    } else if d > 0 {
        format!("{}d {}h {}m {}s", d, h, m, s)
    } else if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

/// Format duration without seconds (for long-running mission timers)
pub fn format_duration_no_seconds(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "---".to_string();
    }
    let total = seconds as u64;
    let y = total / (86400 * 365);
    let d = (total / 86400) % 365;
    let h = (total % 86400) / 3600;
    let m = (total % 3600) / 60;
    if y > 0 {
        format!("{}y {}d {}h", y, d, h)
    } else if d > 0 {
        format!("{}d {}h {}m", d, h, m)
    } else if h > 0 {
        format!("{}h {}m", h, m)
    } else {
        format!("{}m", m)
    }
}

/// Format a distance in meters to a human-readable string with appropriate unit
pub fn format_distance(meters: f64) -> String {
    const AU: f64 = 1.496e11;
    if meters >= AU * 0.1 {
        format!("{:.3} AU", meters / AU)
    } else if meters >= 1e9 {
        format!("{:.1} Mm", meters / 1e6)
    } else if meters >= 1e6 {
        format!("{:.1} km", meters / 1e3)
    } else if meters >= 1e3 {
        format!("{:.1} km", meters / 1e3)
    } else {
        format!("{:.0} m", meters)
    }
}

/// Format mass in kg to a human-readable string with scientific notation
pub fn format_mass(kg: f64) -> String {
    if kg >= 1e24 {
        format!("{:.3e} kg", kg)
    } else if kg >= 1e18 {
        format!("{:.3e} kg", kg)
    } else if kg >= 1e6 {
        format!("{:.3e} kg", kg)
    } else {
        format!("{:.1} kg", kg)
    }
}

/// Format power in Watts to a human-readable string with SI prefixes
pub fn format_power_si(watts: f64) -> String {
    if watts >= 1e12 {
        format!("{:.1} TW", watts / 1e12)
    } else if watts >= 1e9 {
        format!("{:.1} GW", watts / 1e9)
    } else if watts >= 1e6 {
        format!("{:.1} MW", watts / 1e6)
    } else if watts >= 1e3 {
        format!("{:.1} kW", watts / 1e3)
    } else {
        format!("{:.0} W", watts)
    }
}

/// Format pressure in Pascals to a human-readable string (always in atm)
pub fn format_pressure(pa: f64) -> String {
    let atm = pa / 101_325.0;
    if atm >= 0.1 {
        format!("{:.2} atm", atm)
    } else if atm >= 0.001 {
        format!("{:.4} atm", atm)
    } else {
        format!("{:.1e} atm", atm)
    }
}

/// Multi-stop color gradient for porkchop plot.
/// norm: 0.0 (best/lowest dv) to 1.0 (worst/highest dv), already log-scaled.
/// Green -> yellow-green -> yellow -> orange -> red
pub fn porkchop_color(norm: f32) -> egui::Color32 {
    // 5-stop gradient: green, yellow-green, yellow, orange, red
    const STOPS: [(f32, [u8; 3]); 5] = [
        (0.00, [30, 200, 50]),   // green
        (0.25, [120, 210, 40]),  // yellow-green
        (0.50, [220, 210, 30]),  // yellow
        (0.75, [230, 130, 20]),  // orange
        (1.00, [210, 40, 30]),   // red
    ];

    let t = norm.clamp(0.0, 1.0);

    // Find the two stops to interpolate between
    let mut i = 0;
    while i < STOPS.len() - 2 && t > STOPS[i + 1].0 {
        i += 1;
    }

    let (t0, c0) = STOPS[i];
    let (t1, c1) = STOPS[i + 1];
    let frac = if (t1 - t0).abs() < 1e-6 { 0.0 } else { (t - t0) / (t1 - t0) };

    egui::Color32::from_rgb(
        (c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * frac) as u8,
        (c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * frac) as u8,
        (c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * frac) as u8,
    )
}

/// Blackbody glow color for a given temperature.
/// Returns (r, g, b) in 0.0-1.0 following an approximate electromagnetic
/// spectrum: dark red at 500K, cherry red ~1000K, orange ~2000K, bright
/// yellow at 4000K. Below 500K returns None (no glow).
pub fn blackbody_color(temp_k: f64) -> Option<[f32; 3]> {
    if temp_k < 500.0 {
        return None;
    }
    // t: 0.0 at 500K, 1.0 at 4000K
    let t = ((temp_k - 500.0) / 3500.0).min(1.0) as f32;

    // Red: starts dim (0.3) at 500K, reaches full quickly
    let r = (0.3 + 0.7 * (t * 2.0).min(1.0)).min(1.0);
    // Green: stays 0 until ~1000K, then rises to ~0.85 at 4000K
    let g = if t < 0.15 { 0.0 } else { 0.85 * ((t - 0.15) / 0.85).powf(1.5) };
    // Blue: stays 0 (no blue in this range -- yellow is the limit)
    let b = 0.0_f32;

    Some([r, g, b])
}

/// Apply heat tinting to a color based on temperature (Kelvin).
/// Below 500K: no effect. 500K-4000K: blackbody glow from dark red to bright yellow.
pub fn apply_heat_tint(color: [f32; 4], temperature: f64) -> [f32; 4] {
    let glow = match blackbody_color(temperature) {
        Some(g) => g,
        None => return color,
    };
    // Blend factor: how much the glow overrides the base color.
    // At 500K the glow is subtle, by ~1500K it dominates.
    let t = ((temperature - 500.0) / 3500.0).min(1.0) as f32;
    let blend = (t * 1.5).min(1.0);
    [
        color[0] * (1.0 - blend) + glow[0] * blend,
        color[1] * (1.0 - blend) + glow[1] * blend,
        color[2] * (1.0 - blend) + glow[2] * blend,
        color[3],
    ]
}
