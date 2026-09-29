//! One link: line of sight and data rate.

use super::{Antenna, LinkParams, C, K_B};
use crate::math;
use glam::DVec3;

/// A body that can block a line of sight: its reference ellipsoid (an
/// oblate spheroid) with centre in the common frame, equatorial and polar
/// radii, and unit pole in the common frame's axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Occluder {
    pub centre: DVec3,
    pub radius: f64,
    pub polar_radius: f64,
    pub pole: DVec3,
}

impl Occluder {
    /// A sphere.
    pub fn sphere(centre: DVec3, radius: f64) -> Self {
        Occluder { centre, radius, polar_radius: radius, pole: DVec3::Z }
    }

    /// `v` (relative to the centre) in the space where the spheroid is the
    /// sphere of the equatorial radius: the polar component stretched by a/b.
    fn scaled(&self, v: DVec3) -> DVec3 {
        v + self.pole * (v.dot(self.pole) * (self.radius / self.polar_radius - 1.0))
    }

    /// Outward normal (not unit) of the level spheroid through `v`.
    fn normal(&self, v: DVec3) -> DVec3 {
        let k = self.radius / self.polar_radius;
        v + self.pole * (v.dot(self.pole) * (k * k - 1.0))
    }
}

/// Whether the segment `a`–`b` misses every occluder. An end point on or
/// inside a body (a ground site, a vessel on low ground) sees across that
/// body only above its tangent plane there; a ground site's own horizon mask
/// is checked separately by the elevation rule. Every other body blocks the
/// segment where it crosses the body's ellipsoid.
pub fn clear_line_of_sight(a: DVec3, b: DVec3, occluders: &[Occluder]) -> bool {
    occluders.iter().all(|o| {
        let (va, vb) = (a - o.centre, b - o.centre);
        let (sa, sb) = (o.scaled(va), o.scaled(vb));
        let r2 = o.radius * o.radius;
        let inside = |s: DVec3| s.length_squared() <= r2 * (1.0 + 1e-9);
        let (ia, ib) = (inside(sa), inside(sb));
        if ia || ib {
            // Each end on or in the body must look above its tangent plane.
            return (!ia || (b - a).dot(o.normal(va)) >= 0.0) && (!ib || (a - b).dot(o.normal(vb)) >= 0.0);
        }
        let d = sb - sa;
        let len2 = d.length_squared();
        if len2 == 0.0 {
            return true;
        }
        // Closest point of the (scaled) segment to the centre.
        let s = (-sa.dot(d) / len2).clamp(0.0, 1.0);
        (sa + d * s).length_squared() > r2
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
        let earth = Occluder::sphere(DVec3::ZERO, 6.4e6);
        let (a, b) = (DVec3::new(-1e7, 0.0, 0.0), DVec3::new(1e7, 0.0, 0.0));
        assert!(!clear_line_of_sight(a, b, &[earth]));
        let beside = Occluder::sphere(DVec3::new(0.0, 7e6, 0.0), 6.4e6);
        assert!(clear_line_of_sight(a, b, &[beside]));
        // Short of the body: clear.
        assert!(clear_line_of_sight(a, DVec3::new(-8e6, 0.0, 0.0), &[earth]));
    }

    #[test]
    fn a_ground_site_is_not_blocked_by_its_own_body() {
        let earth = Occluder::sphere(DVec3::ZERO, 6.4e6);
        let site = DVec3::new(6.4e6, 0.0, 0.0);
        assert!(clear_line_of_sight(site, DVec3::new(7e6, 1e6, 0.0), &[earth]));
        // ... but the horizon rule is.
        let up = DVec3::X;
        assert!(above_horizon(site, up, DVec3::new(7e6, 1e6, 0.0), 6.0));
        assert!(!above_horizon(site, up, DVec3::new(6.4e6, 1e6, 0.0), 6.0));
    }

    #[test]
    fn the_occluder_is_the_oblate_ellipsoid() {
        let earth = Occluder { centre: DVec3::ZERO, radius: 6.378e6, polar_radius: 6.357e6, pole: DVec3::Z };
        // Two craft 3 km above the pole, 2,000 km apart: inside the
        // equatorial-radius sphere, clear of the ellipsoid.
        let (a, b) = (DVec3::new(-1e6, 0.0, 6.36e6), DVec3::new(1e6, 0.0, 6.36e6));
        assert!(clear_line_of_sight(a, b, &[earth]));
        assert!(!clear_line_of_sight(a, b, &[Occluder::sphere(DVec3::ZERO, 6.378e6)]));
        // Low craft on opposite sides of the pole region, far apart: blocked.
        let (c, d) = (DVec3::new(-5e6, 0.0, 3.96e6), DVec3::new(5e6, 0.0, 3.96e6));
        assert!(!clear_line_of_sight(c, d, &[earth]));
        // A point on the surface sees up, not down through the body.
        let pad = DVec3::new(6.378e6, 0.0, 0.0);
        assert!(clear_line_of_sight(pad, pad + DVec3::new(100.0, 0.0, 0.0), &[earth]));
        assert!(!clear_line_of_sight(pad, DVec3::new(-7e6, 0.0, 0.0), &[earth]));
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
