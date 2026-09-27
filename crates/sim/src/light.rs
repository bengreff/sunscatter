//! Starlight where physics needs it (realism-1 §5b): the flux of each star
//! at a point and the part of its disc other bodies hide (eclipses, by
//! body spheres). The game's lighting (D055) uses the same eclipse rule.
//! Trig goes through `sim::math` (rule 2).

use crate::ephem::Snapshot;
use crate::frame::NodeId;
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
}
