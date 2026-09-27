//! One link: line of sight and data rate.

use super::{Antenna, LinkParams, C, K_B};
use crate::math;
use glam::DVec3;

/// A body that can block a line of sight: centre (common frame) and radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Occluder {
    pub centre: DVec3,
    pub radius: f64,
}

/// Whether the segment `a`–`b` misses every occluder. An end point on or
/// inside a body (a ground site) does not count as blocked by that body:
/// the site's own horizon is checked with the elevation rule instead.
pub fn clear_line_of_sight(a: DVec3, b: DVec3, occluders: &[Occluder]) -> bool {
    let d = b - a;
    let len2 = d.length_squared();
    occluders.iter().all(|o| {
        let r2 = o.radius * o.radius;
        let (ra, rb) = ((a - o.centre).length_squared(), (b - o.centre).length_squared());
        if ra <= r2 * (1.0 + 1e-9) || rb <= r2 * (1.0 + 1e-9) {
            return true;
        }
        if len2 == 0.0 {
            return true;
        }
        // Closest point of the segment to the centre.
        let s = ((o.centre - a).dot(d) / len2).clamp(0.0, 1.0);
        (a + d * s - o.centre).length_squared() > r2
    })
}

/// Whether `to` is above `min_elevation_deg` seen from a ground site at `at`
/// with local up `up` (unit).
pub fn above_horizon(at: DVec3, up: DVec3, to: DVec3, min_elevation_deg: f64) -> bool {
    let dir = (to - at).normalize();
    dir.dot(up) >= math::sin(min_elevation_deg * math::PI / 180.0)
}

/// Shannon rate (bit/s) of a link of length `d` (m) between two antennas,
/// the weaker of the two directions: C = B·log2(1 + P·Gt·Gr·(λ/4πd)² / kTB).
pub fn rate_bps(p: &LinkParams, a: Antenna, b: Antenna, d: f64) -> f64 {
    let lambda = C / p.frequency_hz;
    let x = lambda / (4.0 * math::PI * d.max(1.0));
    let path_gain = x * x;
    let gains = math::exp((a.gain_dbi + b.gain_dbi) / 10.0 * math::ln(10.0));
    let noise = K_B * p.noise_temperature_k * p.bandwidth_hz;
    let snr = a.power_w.min(b.power_w) * gains * path_gain / noise;
    p.bandwidth_hz * math::ln(1.0 + snr) / math::ln(2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> LinkParams {
        LinkParams {
            frequency_hz: 8.4e9,
            bandwidth_hz: 1.0e6,
            noise_temperature_k: 50.0,
            min_rate_bps: 10.0,
            min_elevation_deg: 6.0,
            ground_speed_factor: 0.7,
            umbilical_range_m: 2000.0,
        }
    }

    #[test]
    fn a_body_between_blocks_and_a_body_beside_does_not() {
        let earth = Occluder { centre: DVec3::ZERO, radius: 6.4e6 };
        let (a, b) = (DVec3::new(-1e7, 0.0, 0.0), DVec3::new(1e7, 0.0, 0.0));
        assert!(!clear_line_of_sight(a, b, &[earth]));
        let beside = Occluder { centre: DVec3::new(0.0, 7e6, 0.0), radius: 6.4e6 };
        assert!(clear_line_of_sight(a, b, &[beside]));
        // Short of the body: clear.
        assert!(clear_line_of_sight(a, DVec3::new(-8e6, 0.0, 0.0), &[earth]));
    }

    #[test]
    fn a_ground_site_is_not_blocked_by_its_own_body() {
        let earth = Occluder { centre: DVec3::ZERO, radius: 6.4e6 };
        let site = DVec3::new(6.4e6, 0.0, 0.0);
        assert!(clear_line_of_sight(site, DVec3::new(7e6, 1e6, 0.0), &[earth]));
        // ... but the horizon rule is.
        let up = DVec3::X;
        assert!(above_horizon(site, up, DVec3::new(7e6, 1e6, 0.0), 6.0));
        assert!(!above_horizon(site, up, DVec3::new(6.4e6, 1e6, 0.0), 6.0));
    }

    #[test]
    fn rate_falls_with_distance_and_matches_the_budget() {
        let p = params();
        let dsn = Antenna { gain_dbi: 74.0, power_w: 20_000.0 };
        let craft = Antenna { gain_dbi: 20.0, power_w: 20.0 };
        let moon = rate_bps(&p, dsn, craft, 3.84e8);
        let mars = rate_bps(&p, dsn, craft, 2.25e11);
        assert!(moon > mars && mars > 0.0);
        // Against the formula computed by hand (std maths, test only).
        let lambda = C / 8.4e9;
        let snr =
            20.0 * 10f64.powf(9.4) * (lambda / (4.0 * std::f64::consts::PI * 3.84e8)).powi(2) / (K_B * 50.0 * 1e6);
        assert!((moon - 1e6 * (1.0 + snr).log2()).abs() < 1e-9 * moon);
        // Symmetric.
        assert_eq!(rate_bps(&p, craft, dsn, 3.84e8), moon);
    }
}
