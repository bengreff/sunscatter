//! Starlight where physics needs it (realism-1 §5b): the flux of each star
//! at a point and the part of its disc other bodies hide (eclipses, by
//! body spheres). The game's lighting (D055) uses the same eclipse rule.
//! Trig goes through `sim::math` (rule 2).

use crate::ephem::Snapshot;
use crate::frame::NodeId;
use crate::kepler::Ellipse;
use crate::math;
use crate::world::World;
use glam::DVec3;

/// Irradiance (W/m²) at distance `d` (m) from a star of luminosity `l` (W).
pub fn star_flux(luminosity: f64, d: f64) -> f64 {
    luminosity / (4.0 * math::PI * d * d)
}

/// Area of overlap of two discs of radii `r1`, `r2` whose centres are `d`
/// apart (any consistent unit, here angles in radians).
fn disc_overlap(r1: f64, r2: f64, d: f64) -> f64 {
    if d >= r1 + r2 {
        return 0.0;
    }
    if d <= (r1 - r2).abs() {
        let r = r1.min(r2);
        return math::PI * r * r;
    }
    let a1 = math::acos(((d * d + r1 * r1 - r2 * r2) / (2.0 * d * r1)).clamp(-1.0, 1.0));
    let a2 = math::acos(((d * d + r2 * r2 - r1 * r1) / (2.0 * d * r2)).clamp(-1.0, 1.0));
    r1 * r1 * (a1 - math::sin(a1) * math::cos(a1)) + r2 * r2 * (a2 - math::sin(a2) * math::cos(a2))
}

/// Fraction (0..1) of a star's disc visible from `at`, with the star at
/// `star` (radius `star_r`) and sphere occluders `(centre, radius)`. Discs
/// are compared as angles on the sky; overlapping occluders are not
/// double-counted beyond hiding the whole disc.
pub fn eclipse_factor(at: DVec3, star: DVec3, star_r: f64, occluders: &[(DVec3, f64)]) -> f64 {
    let to_star = star - at;
    let ds = to_star.length();
    let s_ang = math::asin((star_r / ds).clamp(0.0, 1.0));
    let s_area = math::PI * s_ang * s_ang;
    let mut hidden = 0.0;
    for &(c, r) in occluders {
        let to_c = c - at;
        let dc = to_c.length();
        // Behind us, farther than the star, or containing the point: skip
        // (a point inside a body is its surface; the body itself is lit by
        // the shader's Lambert term, not eclipsed).
        if dc <= r || dc >= ds || to_c.dot(to_star) <= 0.0 {
            continue;
        }
        let o_ang = math::asin((r / dc).clamp(0.0, 1.0));
        let sep = math::acos(to_c.normalize().dot(to_star / ds).clamp(-1.0, 1.0));
        hidden += disc_overlap(s_ang, o_ang, sep);
    }
    (1.0 - hidden / s_area).clamp(0.0, 1.0)
}

/// Sunlight at a vessel at `r` (relative to `anchor`): the flux vector
/// (W/m², along the direction the light travels, inertial axes), summed
/// over the stars (sources with a luminosity), each dimmed by the bodies
/// in front of it (spheres of equatorial radius). Zero in full shadow.
pub fn sunlight(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3) -> DVec3 {
    let mut flux = DVec3::ZERO;
    for star in &world.sources {
        let Some(p) = star.physical.as_ref().filter(|p| p.luminosity > 0.0) else { continue };
        let at_star = snap.relative_r(star.node, anchor);
        let from_star = r - at_star;
        let d = from_star.length();
        if d <= p.radius_eq {
            continue;
        }
        let mut occluders = Vec::new();
        for s in &world.sources {
            if s.node == star.node {
                continue;
            }
            if let Some(b) = &s.physical {
                occluders.push((snap.relative_r(s.node, anchor), b.radius_eq));
            }
        }
        let seen = eclipse_factor(r, at_star, p.radius_eq, &occluders);
        if seen > 0.0 {
            flux += from_star / d * (star_flux(p.luminosity, d) * seen);
        }
    }
    flux
}

/// Samples of an orbit (uniform in eccentric anomaly) for [`orbit_sunlit_fraction`].
const ORBIT_SAMPLES: usize = 24;
/// Bisections of each shadow boundary between samples.
const EDGE_BISECTIONS: usize = 12;

/// Fraction of an orbit's time spent in sunlight: `orbit` about a body of
/// `radius`, the star in the unit direction `to_star` from the body (far
/// away: a cylindrical shadow; the penumbra, seconds in low orbit, is left
/// out). The shadow boundaries are found by bisection between samples in
/// eccentric anomaly and timed by Kepler's equation; a shadow passage
/// shorter than the sample spacing (15° of eccentric anomaly) is missed.
pub fn orbit_sunlit_fraction(orbit: &Ellipse, to_star: DVec3, radius: f64) -> f64 {
    let lit = |big_e: f64| {
        let x = orbit.position(big_e);
        let along = x.dot(to_star);
        along >= 0.0 || (x - to_star * along).length_squared() >= radius * radius
    };
    let step = math::TAU / ORBIT_SAMPLES as f64;
    let mut dark = 0.0;
    // Where each sample interval's state changes, the boundary; the dark
    // time accumulates from each entry to the next exit.
    let mut entered: Option<f64> = if lit(0.0) { None } else { Some(0.0) };
    let mut prev = lit(0.0);
    for k in 1..=ORBIT_SAMPLES {
        let e1 = step * k as f64;
        let now = lit(e1);
        if now != prev {
            let (mut lo, mut hi) = (e1 - step, e1);
            for _ in 0..EDGE_BISECTIONS {
                let mid = 0.5 * (lo + hi);
                if lit(mid) == prev {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let edge = 0.5 * (lo + hi);
            match entered.take() {
                Some(start) => dark += orbit.mean_anomaly(edge) - orbit.mean_anomaly(start),
                None => entered = Some(edge),
            }
        }
        prev = now;
    }
    if let Some(start) = entered {
        dark += orbit.mean_anomaly(math::TAU) - orbit.mean_anomaly(start);
    }
    (1.0 - dark / math::TAU).clamp(0.0, 1.0)
}

/// Sunlight at a vessel at `(r, v)` relative to `anchor`, averaged over its
/// osculating orbit about `anchor`: each star's flux vector at `r`, times
/// the orbit's sunlit fraction behind `anchor` (the only occluder counted).
/// `None` unless the orbit is bound, clears the body's equatorial radius
/// and takes at most `max_period` (s), or when `anchor` is itself a star:
/// the caller then uses [`sunlight`].
pub fn orbit_average_sunlight(
    world: &World,
    snap: &Snapshot,
    anchor: NodeId,
    r: DVec3,
    v: DVec3,
    max_period: f64,
) -> Option<DVec3> {
    let body = world.source(anchor)?;
    let p = body.physical.as_ref().filter(|p| p.luminosity == 0.0)?;
    let orbit = Ellipse::from_state(r, v, body.gm)?;
    if orbit.periapsis() <= p.radius_eq || orbit.period(body.gm) > max_period {
        return None;
    }
    let mut flux = DVec3::ZERO;
    for star in &world.sources {
        let Some(s) = star.physical.as_ref().filter(|s| s.luminosity > 0.0) else { continue };
        let at_star = snap.relative_r(star.node, anchor);
        let from_star = r - at_star;
        let d = from_star.length();
        let lit = orbit_sunlit_fraction(&orbit, at_star.normalize(), p.radius_eq);
        flux += from_star / d * (star_flux(s.luminosity, d) * lit);
    }
    Some(flux)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AU: f64 = 1.495_978_707e11;
    const SUN_R: f64 = 6.957e8;
    const EARTH_R: f64 = 6.371e6;

    #[test]
    fn solar_constant_at_one_au() {
        let s = star_flux(3.828e26, AU);
        assert!((s - 1361.0).abs() < 1.0, "{s}");
        assert!((star_flux(3.828e26, 2.0 * AU) - s / 4.0).abs() < 1e-9 * s);
    }

    #[test]
    fn eclipse_table() {
        let sun = DVec3::new(-AU, 0.0, 0.0);
        let occ = [(DVec3::ZERO, EARTH_R)];
        // (case, point, expected visible fraction)
        let cases = [
            ("behind Earth's centre, in the umbra", DVec3::new(2.0 * EARTH_R, 0.0, 0.0), 0.0),
            ("LEO on the night side", DVec3::new(6.778e6, 0.0, 0.0), 0.0),
            ("Moon distance on the axis: umbra", DVec3::new(3.84e8, 0.0, 0.0), 0.0),
            ("well outside the shadow", DVec3::new(3.84e8, 2.0e7, 0.0), 1.0),
            ("LEO on the day side", DVec3::new(-6.778e6, 0.0, 0.0), 1.0),
            ("LEO over the terminator", DVec3::new(0.0, 6.778e6, 0.0), 1.0),
            ("far off to the side", DVec3::new(0.0, 1.0e9, 0.0), 1.0),
        ];
        for (case, p, expected) in cases {
            let f = eclipse_factor(p, sun, SUN_R, &occ);
            assert!((f - expected).abs() <= 1e-9, "{case}: {f}");
        }
        // In the penumbra the factor is strictly between 0 and 1, and grows
        // outwards.
        let edge = |y: f64| eclipse_factor(DVec3::new(3.84e8, y, 0.0), sun, SUN_R, &occ);
        let (a, b) = (edge(6.0e6), edge(7.5e6));
        assert!(a > 0.0 && a < b && b < 1.0, "{a} {b}");
    }

    /// A circular orbit of radius `r` whose plane is tilted `beta` from the
    /// sunlight (the beta angle), about a body of radius `EARTH_R`.
    fn circular(r: f64, beta: f64) -> (Ellipse, DVec3) {
        let orbit = Ellipse { a: r, e: 0.0, p: DVec3::X, q: DVec3::Y };
        let to_star = DVec3::new(math::cos(beta), 0.0, math::sin(beta));
        (orbit, to_star)
    }

    #[test]
    fn circular_orbit_eclipse_fraction_matches_the_beta_angle_formula() {
        // Eclipse fraction of a circular orbit at height h, beta angle β:
        // (1/π)·acos(√(h² + 2Rh) / (r·cos β)) while |β| < asin(R/r), else 0
        // (e.g. Wertz & Larson, Space Mission Analysis and Design, §5.1).
        // At 400 km, β = 0: 0.390 (36 min of a 92.4 min orbit).
        let r = EARTH_R + 400e3;
        let h = r - EARTH_R;
        for beta_deg in [0.0f64, 20.0, 45.0, 60.0, 69.0, 72.0, 85.0] {
            let beta = beta_deg.to_radians();
            let x = (h * h + 2.0 * EARTH_R * h).sqrt() / (r * math::cos(beta));
            let eclipse = if x < 1.0 { math::acos(x) / math::PI } else { 0.0 };
            let (orbit, to_star) = circular(r, beta);
            let lit = orbit_sunlit_fraction(&orbit, to_star, EARTH_R);
            assert!((lit - (1.0 - eclipse)).abs() < 1e-5, "β {beta_deg}: {lit} vs {}", 1.0 - eclipse);
        }
        let (orbit, to_star) = circular(r, 0.0);
        assert!((1.0 - orbit_sunlit_fraction(&orbit, to_star, EARTH_R) - 0.3896).abs() < 1e-3);
    }

    #[test]
    fn eccentric_orbit_sunlit_fraction_matches_time_sampling() {
        // e = 0.3, periapsis 500 km: the fraction of time lit, by brute
        // force in mean anomaly (Kepler solves) against the bisection.
        let rp = EARTH_R + 500e3;
        let (a, e) = (rp / 0.7, 0.3);
        for (argp, beta) in [(0.0f64, 0.0f64), (2.5, 0.2), (4.0, -0.4)] {
            let p = DVec3::new(math::cos(argp), math::sin(argp), 0.0);
            let orbit = Ellipse { a, e, p, q: DVec3::Z.cross(p) };
            let to_star = DVec3::new(math::cos(beta), 0.0, math::sin(beta));
            let n = 200_000;
            let lit = (0..n)
                .filter(|&k| {
                    let m = math::TAU * (k as f64 + 0.5) / n as f64;
                    let big_e = crate::kepler::solve_elliptic(m, e);
                    let x = orbit.position(big_e);
                    let along = x.dot(to_star);
                    along >= 0.0 || (x - to_star * along).length() >= EARTH_R
                })
                .count() as f64
                / n as f64;
            let got = orbit_sunlit_fraction(&orbit, to_star, EARTH_R);
            assert!((got - lit).abs() < 2e-5, "argp {argp}: {got} vs {lit}");
        }
    }
}
