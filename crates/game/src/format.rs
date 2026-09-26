//! The one formatter for numbers shown to the player: distance (m, km, Mm,
//! AU), speed (m/s, km/s) and durations (s, min, h, d, y). Fixed decimals
//! so right-aligned monospace readouts do not jiggle.

/// One astronomical unit (m).
const AU: f64 = 1.495_978_707e11;

pub fn distance(m: f64) -> String {
    let a = m.abs();
    if !m.is_finite() {
        "∞".into()
    } else if a < 1.0e4 {
        format!("{m:.0} m")
    } else if a < 1.0e7 {
        format!("{:.1} km", m / 1e3)
    } else if a < 1.0e10 {
        format!("{:.2} Mm", m / 1e6)
    } else {
        format!("{:.3} AU", m / AU)
    }
}

pub fn speed(v: f64) -> String {
    if v.abs() >= 1.0e4 {
        format!("{:.2} km/s", v / 1e3)
    } else {
        format!("{v:.1} m/s")
    }
}

/// A duration (clamped at zero): the two largest units.
pub fn duration(s: f64) -> String {
    let s = s.max(0.0);
    let (m, h, d, y) = (60.0, 3600.0, 86_400.0, 365.25 * 86_400.0);
    if s < m {
        format!("{:.0}s", s.floor())
    } else if s < h {
        format!("{:.0}m {:02.0}s", (s / m).floor(), (s % m).floor())
    } else if s < d {
        format!("{:.0}h {:02.0}m", (s / h).floor(), ((s % h) / m).floor())
    } else if s < y {
        format!("{:.0}d {:02.0}h", (s / d).floor(), ((s % d) / h).floor())
    } else {
        format!("{:.0}y {:03.0}d", (s / y).floor(), ((s % y) / d).floor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        let cases = [
            (5.0, "5 m"),
            (-4_917.0, "-4917 m"),
            (296_400.0, "296.4 km"),
            (6_378_137.0, "6378.1 km"),
            (3.844e8, "384.40 Mm"),
            (1.495_978_707e11, "1.000 AU"),
            (f64::INFINITY, "∞"),
        ];
        for (m, expected) in cases {
            assert_eq!(distance(m), expected, "{m}");
        }
    }

    #[test]
    fn speeds() {
        let cases = [(123.45, "123.5 m/s"), (7_784.0, "7784.0 m/s"), (12_345.0, "12.35 km/s"), (-3.0, "-3.0 m/s")];
        for (v, expected) in cases {
            assert_eq!(speed(v), expected, "{v}");
        }
    }

    #[test]
    fn durations() {
        let cases = [
            (-5.0, "0s"),
            (37.9, "37s"),
            (125.0, "2m 05s"),
            (5_340.0, "1h 29m"),
            (27.32 * 86_400.0, "27d 07h"),
            (400.0 * 86_400.0, "1y 034d"),
        ];
        for (s, expected) in cases {
            assert_eq!(duration(s), expected, "{s}");
        }
    }
}
