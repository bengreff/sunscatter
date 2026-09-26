//! Simulation time.
//!
//! The global clock is barycentric dynamical time (TDB), stored as whole seconds
//! since J2000 (2000-01-01T12:00:00 TDB) plus a fractional second in `[0, 1)`.
//! A single f64 of seconds would resolve only ~4 µs after 1,000 years; the split
//! keeps sub-nanosecond resolution across the whole game window.
//!
//! Local computations (integrator steps, Chebyshev segments) use plain `f64`
//! seconds *relative to a nearby epoch*, obtained with [`Epoch::seconds_since`].

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub const SECONDS_PER_DAY: i64 = 86_400;
/// Seconds per Julian year (365.25 days).
pub const SECONDS_PER_JULIAN_YEAR: f64 = 31_557_600.0;
/// Julian date of J2000.
pub const J2000_JD: f64 = 2_451_545.0;

/// A point in TDB time. Always normalized so that `0.0 <= frac < 1.0`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Epoch {
    secs: i64,
    frac: f64,
}

impl Epoch {
    pub const J2000: Epoch = Epoch { secs: 0, frac: 0.0 };

    /// Builds an epoch from whole seconds and a fractional part (any sign/size).
    pub fn new(secs: i64, frac: f64) -> Self {
        let whole = libm::floor(frac);
        let secs = secs + whole as i64;
        let mut frac = frac - whole;
        let mut secs = secs;
        // `frac - floor(frac)` can round up to exactly 1.0 for tiny negatives.
        if frac >= 1.0 {
            frac -= 1.0;
            secs += 1;
        }
        Epoch { secs, frac }
    }

    /// Seconds since J2000 as an `f64`. Lossy far from J2000; use only for display
    /// or when the precision loss is acceptable.
    pub fn to_seconds_f64(self) -> f64 {
        self.secs as f64 + self.frac
    }

    pub fn whole_seconds(self) -> i64 {
        self.secs
    }

    pub fn fractional_second(self) -> f64 {
        self.frac
    }

    /// `self - earlier` in seconds. Exact integer part, so precise for any span
    /// whose result fits comfortably in an f64.
    pub fn seconds_since(self, earlier: Epoch) -> f64 {
        (self.secs - earlier.secs) as f64 + (self.frac - earlier.frac)
    }

    /// Adds `dt` seconds.
    pub fn add_seconds(self, dt: f64) -> Epoch {
        let whole = libm::floor(dt);
        Epoch::new(self.secs + whole as i64, self.frac + (dt - whole))
    }

    /// Builds an epoch from a TDB calendar date (proleptic Gregorian).
    pub fn from_calendar(year: i64, month: u32, day: u32, hour: u32, min: u32, sec: f64) -> Self {
        let days = days_from_civil(year, month, day) - days_from_civil(2000, 1, 1);
        let whole = days * SECONDS_PER_DAY + i64::from(hour) * 3600 + i64::from(min) * 60 - 12 * 3600;
        Epoch::new(whole, 0.0).add_seconds(sec)
    }

    /// The TDB calendar date: (year, month, day, hour, minute, second).
    pub fn to_calendar(self) -> (i64, u32, u32, u32, u32, f64) {
        let since_midnight_j2000 = self.secs + 12 * 3600;
        let days = since_midnight_j2000.div_euclid(SECONDS_PER_DAY);
        let rem = since_midnight_j2000.rem_euclid(SECONDS_PER_DAY);
        let (y, m, d) = civil_from_days(days + days_from_civil(2000, 1, 1));
        let hour = (rem / 3600) as u32;
        let min = ((rem % 3600) / 60) as u32;
        let sec = (rem % 60) as f64 + self.frac;
        (y, m, d, hour, min, sec)
    }

    /// Days since J2000 as an `f64` (for rotation models and display).
    pub fn days_since_j2000(self) -> f64 {
        self.secs as f64 / SECONDS_PER_DAY as f64 + self.frac / SECONDS_PER_DAY as f64
    }
}

impl PartialOrd for Epoch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.secs.cmp(&other.secs) {
            Ordering::Equal => self.frac.partial_cmp(&other.frac),
            ord => Some(ord),
        }
    }
}

/// Days from 1970-01-01 to the given civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`].
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j2000_is_noon_jan_1_2000() {
        assert_eq!(Epoch::from_calendar(2000, 1, 1, 12, 0, 0.0), Epoch::J2000);
        assert_eq!(Epoch::J2000.to_calendar(), (2000, 1, 1, 12, 0, 0.0));
    }

    #[test]
    fn calendar_round_trip_across_millennium() {
        for &(y, m, d) in &[(1969, 7, 20), (2030, 1, 1), (2524, 2, 29), (3030, 12, 31)] {
            let e = Epoch::from_calendar(y, m, d, 7, 42, 13.25);
            assert_eq!(e.to_calendar(), (y, m, d, 7, 42, 13.25));
        }
    }

    #[test]
    fn resolution_holds_after_1000_years() {
        let far = Epoch::from_calendar(3030, 1, 1, 0, 0, 0.0);
        let tick = far.add_seconds(1e-9);
        let dt = tick.seconds_since(far);
        assert!((dt - 1e-9).abs() < 1e-15, "dt = {dt}");
    }

    #[test]
    fn add_and_difference_are_consistent_for_negative_values() {
        let e = Epoch::from_calendar(2030, 1, 1, 0, 0, 0.0);
        let back = e.add_seconds(-1234.75);
        assert!((e.seconds_since(back) - 1234.75).abs() < 1e-12);
        assert!(back < e);
        assert!(back.fractional_second() >= 0.0 && back.fractional_second() < 1.0);
    }

    #[test]
    fn ordering_uses_fraction_when_seconds_match() {
        let a = Epoch::new(10, 0.25);
        let b = Epoch::new(10, 0.5);
        assert!(a < b);
        assert!(Epoch::new(9, 0.99) < a);
    }
}
