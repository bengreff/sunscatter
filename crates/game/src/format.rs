//! The one formatter for numbers shown to the player: distance (m, km, Mm,
//! AU), speed (m/s, km/s), durations (s, min, h, d, y), signal delays and
//! data rates. Fixed decimals
//! so right-aligned monospace readouts do not jiggle.

use sim::systems::{Knowledge, Quantity, Reading};

/// One astronomical unit (m).
const AU: f64 = 1.495_978_707e11;

/// The unit a distance is shown in: its size in metres, name and decimals.
fn distance_unit(m: f64) -> (f64, &'static str, usize) {
    let a = m.abs();
    if a < 1.0e4 {
        (1.0, "m", 0)
    } else if a < 1.0e7 {
        (1e3, "km", 1)
    } else if a < 1.0e10 {
        (1e6, "Mm", 2)
    } else {
        (AU, "AU", 3)
    }
}

/// The unit a speed is shown in: its size in m/s, name and decimals.
fn speed_unit(v: f64) -> (f64, &'static str, usize) {
    if v.abs() >= 1.0e4 {
        (1e3, "km/s", 2)
    } else {
        (1.0, "m/s", 1)
    }
}

pub fn distance(m: f64) -> String {
    if !m.is_finite() {
        return "∞".into();
    }
    let (scale, unit, d) = distance_unit(m);
    format!("{:.*} {unit}", d, m / scale)
}

/// A speed known exactly (`Known::speed` gives one as the craft knows it).
#[cfg(test)]
pub fn speed(v: f64) -> String {
    let (scale, unit, d) = speed_unit(v);
    format!("{:.*} {unit}", d, v / scale)
}

/// What the interface shows for a quantity the craft cannot measure (D106).
pub const DASH: &str = "—";

/// Decimals of a value shown in a unit of `scale` SI units when it is known
/// to `precision` (SI; 0: exactly): down to the precision's decade, at
/// most `max` (the format's own), never fewer than none.
pub fn decimals(precision: f64, scale: f64, max: usize) -> usize {
    // Zero (exact) or not a number: the format's own digits.
    if precision.is_nan() || precision <= 0.0 {
        return max;
    }
    let d = -(precision / scale).log10().floor();
    (d.max(0.0) as usize).min(max)
}

/// A reading (D106) with a unit chosen for its size: a dash when unknown,
/// else the value rounded to its precision with no more decimals than the
/// precision gives in that unit.
fn read(r: Option<Reading>, unit_of: fn(f64) -> (f64, &'static str, usize)) -> String {
    let Some(r) = r else { return DASH.into() };
    let v = r.shown();
    if !v.is_finite() {
        return "∞".into();
    }
    let (scale, unit, max) = unit_of(v);
    format!("{:.*} {unit}", decimals(r.precision, scale, max), v / scale)
}

/// The unit a mass is shown in: kg, then tonnes.
fn mass_unit(kg: f64) -> (f64, &'static str, usize) {
    if kg.abs() < 1.0e4 {
        (1.0, "kg", 0)
    } else {
        (1e3, "t", 1)
    }
}

/// The unit a force is shown in: N, kN, MN.
fn force_unit(n: f64) -> (f64, &'static str, usize) {
    let a = n.abs();
    if a < 1.0e4 {
        (1.0, "N", 0)
    } else if a < 1.0e7 {
        (1e3, "kN", 1)
    } else {
        (1e6, "MN", 2)
    }
}

/// What tanks hold of their capacity, in the capacity's unit (`kg/kg`), the
/// held to its reading's precision, a dash when no sensor measures it (one
/// rule: the staging bar's bars, the part card's tanks, the systems
/// screen's tank and its Plumbing summary).
pub fn amount(kg: Option<Reading>, capacity: f64) -> String {
    let (scale, unit, max) = if capacity < 1.0e4 { (1.0, "kg", 0) } else { (1e3, "t", 1) };
    let held = kg.map_or(DASH.to_string(), |r| format!("{:.*}", decimals(r.precision, scale, max), r.shown() / scale));
    format!("{held}/{:.*} {unit}", max, capacity / scale)
}

/// A mass as it is known (D106: an estimate's reading, a dash when unknown).
pub fn mass(r: Option<Reading>) -> String {
    read(r, mass_unit)
}

/// A force as it is known.
pub fn force(r: Option<Reading>) -> String {
    read(r, force_unit)
}

/// A speed (a Δv) as it is known.
pub fn speed_of(r: Option<Reading>) -> String {
    read(r, speed_unit)
}

/// What the craft knows (D106: `Vessel::knowledge`, from its working
/// sensors) for the readouts: everything by default (debug mode).
#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    pub knowledge: Knowledge,
    /// Its propellant: every tank's level (so its mass, Δv and TWR).
    pub propellant: bool,
    /// Its parts' temperatures: every part's.
    pub temperatures: bool,
}

impl Default for Known {
    fn default() -> Self {
        Known::from(Knowledge::perfect())
    }
}

impl From<Knowledge> for Known {
    /// Vehicle quantities only (no tanks or parts read).
    fn from(knowledge: Knowledge) -> Self {
        let perfect = knowledge.perfect;
        Known { knowledge, propellant: perfect, temperatures: perfect }
    }
}

impl Known {
    /// What vessel `v` knows now.
    pub fn of(v: &sim::vessel::Vessel) -> Known {
        let knowledge = v.knowledge();
        let Some(def) = v.ship_def() else { return Known::from(knowledge) };
        let n = |len: usize| 0..len as u32;
        let propellant = n(def.tanks.len()).all(|i| knowledge.knows(Quantity::TankLevel(i)));
        let temperatures = def.parts.iter().all(|p| knowledge.knows(Quantity::PartTemperature(p.part)));
        Known { knowledge, propellant, temperatures }
    }

    pub fn knows(&self, q: Quantity) -> bool {
        self.knowledge.knows(q)
    }

    /// The orbit and all that comes from it (apsides, elements, the
    /// target's relative motion, an impact): known with the position and
    /// the velocity.
    pub fn orbit(&self) -> bool {
        self.knows(Quantity::Position) && self.knows(Quantity::Velocity)
    }

    /// A distance as `q` is known.
    pub fn distance(&self, q: Quantity, m: f64) -> String {
        read(self.knowledge.read(q, m), distance_unit)
    }

    /// A speed as `q` is known.
    pub fn speed(&self, q: Quantity, v: f64) -> String {
        read(self.knowledge.read(q, v), speed_unit)
    }

    /// A value of `q` shown in a unit of `scale` SI units (`unit`) with at
    /// most `max` decimals and no more than its precision gives; a dash
    /// when unknown.
    pub fn value(&self, q: Quantity, v: f64, scale: f64, max: usize, unit: &str) -> String {
        match self.knowledge.read(q, v) {
            Some(r) => format!("{:.*} {unit}", decimals(r.precision, scale, max), r.shown() / scale),
            None => DASH.into(),
        }
    }

    /// `text` when `known`, else a dash (values whose digits are the
    /// format's own: angles, durations, ratios).
    pub fn or_dash(&self, known: bool, text: impl FnOnce() -> String) -> String {
        if known {
            text()
        } else {
            DASH.into()
        }
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

/// A signal delay: milliseconds below one second, then seconds, then the
/// duration format.
pub fn delay(s: f64) -> String {
    if s < 1.0 {
        format!("{:.0} ms", s * 1e3)
    } else if s < 60.0 {
        format!("{s:.2} s")
    } else {
        duration(s)
    }
}

/// A data rate (bit/s).
pub fn rate(bps: f64) -> String {
    if !bps.is_finite() {
        "∞".into()
    } else if bps >= 1e6 {
        format!("{:.1} Mbit/s", bps / 1e6)
    } else if bps >= 1e3 {
        format!("{:.1} kbit/s", bps / 1e3)
    } else {
        format!("{bps:.0} bit/s")
    }
}

/// From this fraction of its limit a value reads amber.
pub const WARN_FRACTION: f64 = 0.8;

/// How a value reads against its limit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Level {
    #[default]
    Normal,
    /// From [`WARN_FRACTION`] of the limit.
    Warn,
    /// At or over the limit.
    Over,
}

/// The one rule for a value against its limit (the navball's G row, the
/// systems screen's `value / limit` rows, the part cards): amber from
/// [`WARN_FRACTION`] of it, red at it, normal without one.
pub fn limit_level(value: f64, limit: Option<f64>) -> Level {
    match limit {
        Some(l) if value >= l => Level::Over,
        Some(l) if value >= WARN_FRACTION * l => Level::Warn,
        _ => Level::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_and_rates() {
        assert_eq!(delay(0.0213), "21 ms");
        assert_eq!(delay(1.2831), "1.28 s");
        assert_eq!(delay(750.0), "12m 30s");
        assert_eq!(rate(4.26e6), "4.3 Mbit/s");
        assert_eq!(rate(1500.0), "1.5 kbit/s");
        assert_eq!(rate(12.0), "12 bit/s");
        assert_eq!(rate(f64::INFINITY), "∞");
    }

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
    fn readings_show_a_dash_unknown_and_the_digits_their_precision_gives() {
        let known = |p: Option<f64>| {
            let mut k = Knowledge::default();
            if let Some(p) = p {
                k.learn(Quantity::Position, p);
                k.learn(Quantity::Velocity, p / 30.0);
            }
            Known::from(k)
        };
        // (precision of the position, metres, shown)
        let table = [
            (None, 296_400.0, "—"),
            (Some(0.0), 296_421.7, "296.4 km"),
            (Some(3.0), 6_771_234.7, "6771.2 km"),
            (Some(30.0), 6_771_234.7, "6771.2 km"),
            (Some(300.0), 6_771_234.7, "6771.2 km"),
            (Some(3000.0), 6_771_234.7, "6771 km"),
            (Some(3.0), 1_234.5, "1235 m"),
            (Some(30.0), 1_234.5, "1230 m"),
        ];
        for (p, m, want) in table {
            assert_eq!(known(p).distance(Quantity::Position, m), want, "{p:?} {m}");
        }
        assert_eq!(known(Some(3.0)).speed(Quantity::Velocity, 7_812.345), "7812.3 m/s");
        assert_eq!(known(Some(30.0)).speed(Quantity::Velocity, 7_812.345), "7812 m/s");
        assert_eq!(known(None).speed(Quantity::Velocity, 7_812.0), "—");
        assert!(known(Some(3.0)).orbit() && !known(None).orbit());
        assert_eq!(Known::default().distance(Quantity::Position, 296_421.7), distance(296_421.7), "perfect: as before");
        assert_eq!(known(None).or_dash(false, || "x".into()), "—");
        // (precision, unit size, the format's decimals, decimals)
        for (p, scale, max, want) in
            [(0.0, 1.0, 1, 1), (0.1, 1.0, 1, 1), (3.0, 1e3, 1, 1), (3000.0, 1e3, 1, 0), (5e4, 1.0, 1, 0)]
        {
            assert_eq!(decimals(p, scale, max), want, "{p} {scale}");
        }
    }

    #[test]
    fn masses_and_forces_as_known() {
        let r = |value: f64, precision: f64| Some(Reading { value, precision });
        // (text, want)
        let table = [
            (mass(r(559_860.4, 0.0)), "559.9 t"),
            (mass(r(559_860.4, 2_600.0)), "560 t"),
            (mass(r(8_312.0, 0.0)), "8312 kg"),
            (mass(None), DASH),
            (force(r(7_607_000.0, 0.0)), "7607.0 kN"),
            (force(r(81_000_000.0, 0.0)), "81.00 MN"),
            (force(r(400.0, 0.0)), "400 N"),
            (speed_of(r(3_990.24, 0.0)), "3990.2 m/s"),
            (speed_of(r(3_990.24, 12.0)), "3990 m/s"),
            (speed_of(None), DASH),
        ];
        for (got, want) in table {
            assert_eq!(got, want);
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

    /// A value against its limit (table): amber from 80 % of it, red at
    /// it, normal without one.
    #[test]
    fn amber_from_eighty_percent_red_at_the_limit() {
        let table = [
            (1.0, Some(7.5), Level::Normal),
            (6.0, Some(7.5), Level::Warn),
            (7.5, Some(7.5), Level::Over),
            (9.0, Some(7.5), Level::Over),
            (30.0, None, Level::Normal),
        ];
        for (v, limit, want) in table {
            assert_eq!(limit_level(v, limit), want, "{v} {limit:?}");
        }
    }
}
