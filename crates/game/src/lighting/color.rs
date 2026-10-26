//! Colours of starlight. The renderer's white is the Sun's light (a
//! daylight white balance: sunlit snow is white), so another star's light
//! is tinted by its temperature relative to the Sun's.

/// The Sun's effective temperature (K): IAU 2015 Resolution B3's nominal
/// value.
pub const SUN_TEMPERATURE_K: f64 = 5772.0;

/// Display colour (sRGB, largest channel 1) of a blackbody at `t_k`
/// kelvin: Helland's 2012 fit to Charity's blackbody colour table (CIE
/// 1964 10° observer, D65 white), 1,000–40,000 K.
pub fn blackbody_srgb(t_k: f64) -> [f64; 3] {
    let t = t_k.clamp(1000.0, 40_000.0) / 100.0;
    let r = if t <= 66.0 { 1.0 } else { (1.292_936_186 * (t - 60.0).powf(-0.133_204_759)).clamp(0.0, 1.0) };
    let g = if t <= 66.0 {
        (0.390_081_579 * t.ln() - 0.631_841_444).clamp(0.0, 1.0)
    } else {
        (1.129_890_861 * (t - 60.0).powf(-0.075_514_849)).clamp(0.0, 1.0)
    };
    let b = if t >= 66.0 {
        1.0
    } else if t <= 19.0 {
        0.0
    } else {
        (0.543_206_789 * (t - 10.0).ln() - 1.196_254_089).clamp(0.0, 1.0)
    };
    let m = r.max(g).max(b);
    [r / m, g / m, b / m]
}

/// sRGB-encoded to linear (IEC 61966-2-1).
pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// A linear colour scaled to Rec. 709 luminance 1 (white if it is black).
pub fn unit_luminance(c: [f64; 3]) -> [f64; 3] {
    let y = super::rules::luminance(c);
    if y > 0.0 {
        c.map(|v| v / y)
    } else {
        [1.0; 3]
    }
}

/// The colour of a star's light relative to the Sun's, linear, luminance
/// 1: white at the Sun's temperature, orange for a red dwarf, blue-white
/// for a hot star.
pub fn star_tint(t_k: f64) -> [f64; 3] {
    let star = blackbody_srgb(t_k).map(srgb_to_linear);
    let sun = blackbody_srgb(SUN_TEMPERATURE_K).map(srgb_to_linear);
    unit_luminance([star[0] / sun[0], star[1] / sun[1], star[2] / sun[2]])
}

/// The colour of the light a body reflects, from its sRGB base colour:
/// linear, luminance 1 (its albedo carries the brightness).
pub fn reflector_tint(base_srgb: [f32; 3]) -> [f64; 3] {
    unit_luminance(base_srgb.map(|c| srgb_to_linear(f64::from(c))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_tints_relative_to_the_sun() {
        // (case, temperature K, expected order of channels)
        let sun = star_tint(SUN_TEMPERATURE_K);
        for c in sun {
            assert!((c - 1.0).abs() < 1e-12, "the Sun is white: {sun:?}");
        }
        let cases = [
            ("red dwarf", 3000.0, true),
            ("K dwarf", 4500.0, true),
            ("A star", 9000.0, false),
            ("B star", 20_000.0, false),
        ];
        for (case, t, warm) in cases {
            let c = star_tint(t);
            assert!((super::super::rules::luminance(c) - 1.0).abs() < 1e-9, "{case}: luminance 1");
            assert_eq!(c[0] > c[2], warm, "{case}: {c:?}");
            assert!(c.iter().all(|v| v.is_finite() && *v >= 0.0), "{case}: {c:?}");
        }
        // Cooler is redder, monotonically.
        let ratio = |t: f64| star_tint(t)[0] / star_tint(t)[2].max(1e-9);
        assert!(ratio(2500.0) > ratio(3500.0) && ratio(3500.0) > ratio(5000.0) && ratio(5000.0) > ratio(8000.0));
    }

    #[test]
    fn reflector_tints_keep_hue_and_unit_luminance() {
        let grey = reflector_tint([0.5, 0.5, 0.5]);
        assert!(grey.iter().all(|c| (c - 1.0).abs() < 1e-9), "{grey:?}");
        let red = reflector_tint([0.8, 0.4, 0.2]);
        assert!(red[0] > red[1] && red[1] > red[2]);
        assert!((super::super::rules::luminance(red) - 1.0).abs() < 1e-9);
        assert_eq!(reflector_tint([0.0; 3]), [1.0; 3]);
    }
}
