/// Convert a stellar surface temperature (Kelvin) to an RGB color in 0.0–1.0.
/// Covers the full main-sequence range: ~2000K (deep red M-dwarfs) to ~40000K (hot blue O-stars).
/// Uses piecewise linear interpolation between physically-motivated reference points.
pub fn stellar_color(temp_k: f32) -> [f32; 3] {
    // Reference points: (temperature_K, [r, g, b])
    // Based on approximate CIE chromaticity of blackbody radiation.
    const STOPS: &[(f32, [f32; 3])] = &[
        (2000.0, [1.0, 0.35, 0.05]),  // Deep red-orange (late M)
        (3500.0, [1.0, 0.55, 0.20]),  // Orange-red (early M / late K)
        (4500.0, [1.0, 0.75, 0.45]),  // Orange (K)
        (5800.0, [1.0, 0.95, 0.85]),  // Yellow-white (G, Sun-like)
        (7500.0, [0.9, 0.92, 1.0]),   // White (F/A)
        (10000.0, [0.7, 0.8, 1.0]),   // Blue-white (A/B)
        (20000.0, [0.55, 0.65, 1.0]), // Blue (B)
        (40000.0, [0.45, 0.55, 1.0]), // Deep blue (O)
    ];

    let t = temp_k.clamp(STOPS[0].0, STOPS[STOPS.len() - 1].0);

    let mut i = 0;
    while i < STOPS.len() - 2 && t > STOPS[i + 1].0 {
        i += 1;
    }

    let (t0, c0) = STOPS[i];
    let (t1, c1) = STOPS[i + 1];
    let frac = if (t1 - t0).abs() < 1e-6 {
        0.0
    } else {
        (t - t0) / (t1 - t0)
    };

    [
        c0[0] + (c1[0] - c0[0]) * frac,
        c0[1] + (c1[1] - c0[1]) * frac,
        c0[2] + (c1[2] - c0[2]) * frac,
    ]
}
