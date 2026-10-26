//! Crater-shaped detail below the heightmap's resolution (D059) for cratered
//! bodies: the fractal noise of [`super::detail`] makes rough ground, not
//! craters, so a body's `terrain_detail` can add a field of bowls.
//!
//! Per octave, space is cut into cubic cells of the octave's size (the first
//! is the map's sample spacing, each next one `lacunarity` times smaller);
//! a hash of the cell decides whether it holds a crater (probability
//! `density`), where its centre lies and its radius (0.1 to 1/3 of the cell),
//! so a crater's footprint (1.5 radii with its rim) never leaves the half
//! cell around it and a point needs only the 8 cells nearest to it. Points
//! are on the body's surface, so a crater is the circle where its ball meets
//! the surface. Profile: a parabolic bowl `depth_ratio × diameter` deep,
//! a rim 0.2 of the depth high at the edge, falling to zero at 1.5 radii.
//! Plain IEEE arithmetic (floor, sqrt, integer hashes): bit-identical on
//! every platform.

use super::noise;
use glam::DVec3;

/// Crater detail parameters, body data (`terrain_detail.craters` in `body.ron`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CraterDetail {
    /// Number of crater octaves, from the detail's `wavelength` down by its
    /// `lacunarity`.
    pub octaves: u32,
    /// Fraction of cells holding a crater (0..=1); about 0.5 is saturated.
    pub density: f64,
    /// Depth over diameter (fresh simple craters ~0.2, degraded ones less).
    pub depth_ratio: f64,
}

/// Seed offset so the crater field is independent of the fractal's octaves.
const SEED: u64 = 0x00c4_a7e5;
/// Rim height over depth.
const RIM: f64 = 0.2;
/// Footprint (rim and ejecta) in crater radii.
const REACH: f64 = 1.5;

/// Height (m) of a crater's profile at `x` crater radii from its centre.
pub fn profile(x: f64, depth: f64) -> f64 {
    let rim = RIM * depth;
    if x < 1.0 {
        -depth + (depth + rim) * x * x
    } else if x < REACH {
        let t = 1.0 - (x - 1.0) / (REACH - 1.0);
        rim * t * t
    } else {
        0.0
    }
}

/// Uniform in [0, 1) from 53 bits of `h`.
#[inline]
fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// The crater field (m) at body-fixed point `p` (m): `cell` is the first
/// octave's cell size (m), `lacunarity` the ratio between octaves.
pub fn field(c: &CraterDetail, p: DVec3, cell: f64, lacunarity: f64, seed: u64) -> f64 {
    let mut sum = 0.0;
    let mut size = cell;
    for k in 0..c.octaves {
        let q = p / size;
        let base = q.floor();
        let f = q - base;
        // Per axis: this cell and the neighbour on the side of the point.
        let side = |t: f64| if t < 0.5 { -1i64 } else { 1 };
        let (sx, sy, sz) = (side(f.x), side(f.y), side(f.z));
        let (bx, by, bz) = (base.x as i64, base.y as i64, base.z as i64);
        let s = seed.wrapping_add(SEED).wrapping_add(u64::from(k));
        for corner in 0..8u32 {
            let ix = bx + if corner & 1 == 0 { 0 } else { sx };
            let iy = by + if corner & 2 == 0 { 0 } else { sy };
            let iz = bz + if corner & 4 == 0 { 0 } else { sz };
            let h = noise::hash(ix, iy, iz, s);
            if unit(h) >= c.density {
                continue;
            }
            let h2 = noise::hash(ix, iy, iz, s ^ 0x5bd1_e995);
            let h3 = noise::hash(ix, iy, iz, s ^ 0x27d4_eb2f);
            // Centre anywhere in the cell; a radius of 0.1..1/3 cell keeps
            // the footprint (1.5 radii) within half a cell of it.
            let centre = DVec3::new(
                ix as f64 + unit(h2),
                iy as f64 + unit(h2.rotate_left(21)),
                iz as f64 + unit(h2.rotate_left(42)),
            );
            let r = 0.1 + (1.0 / 3.0 - 0.1) * unit(h3);
            let x = (q - centre).length() / r;
            if x < REACH {
                sum += profile(x, c.depth_ratio * 2.0 * r * size);
            }
        }
        size /= lacunarity;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: CraterDetail = CraterDetail { octaves: 4, density: 0.5, depth_ratio: 0.2 };

    #[test]
    fn profile_is_a_bowl_with_a_rim_and_continuous() {
        assert_eq!(profile(0.0, 100.0), -100.0);
        assert!((profile(1.0, 100.0) - 20.0).abs() < 1e-9);
        assert_eq!(profile(1.5, 100.0), 0.0);
        assert_eq!(profile(3.0, 100.0), 0.0);
        for x in [0.999_999_9, 1.499_999_9] {
            assert!((profile(x, 100.0) - profile(x + 2e-7, 100.0)).abs() < 1e-3, "{x}");
        }
    }

    #[test]
    fn craters_dent_the_surface_and_are_deterministic() {
        // Sample a line across many cells: some points in bowls (negative),
        // some on rims (positive), many untouched; the same twice.
        let (mut neg, mut pos) = (0, 0);
        let mut a = Vec::new();
        for i in 0..20_000 {
            let p = DVec3::new(1.0e6 + i as f64 * 7.3, 2.0e5 + i as f64 * 1.1, -3.0e5);
            let h = field(&C, p, 2_000.0, 2.0, 0);
            neg += usize::from(h < 0.0);
            pos += usize::from(h > 0.0);
            a.push(h.to_bits());
        }
        assert!(neg > 1_000 && pos > 1_000 && neg + pos < 19_000, "neg {neg} pos {pos}");
        let b: Vec<u64> = (0..20_000)
            .map(|i| {
                let p = DVec3::new(1.0e6 + i as f64 * 7.3, 2.0e5 + i as f64 * 1.1, -3.0e5);
                field(&C, p, 2_000.0, 2.0, 0).to_bits()
            })
            .collect();
        assert_eq!(a, b);
        // Deepest possible: a bowl in every octave at once.
        assert!(a.iter().map(|&h| f64::from_bits(h)).all(|h| h > -0.2 * 2.0 / 3.0 * 2_000.0 * 2.0));
    }

    #[test]
    fn no_density_no_craters_and_the_field_is_continuous() {
        let none = CraterDetail { density: 0.0, ..C };
        assert_eq!(field(&none, DVec3::new(1.0e6, 2.0, 3.0), 2_000.0, 2.0, 0), 0.0);
        // Steps of 1 mm change the height by little (no seams at cell faces).
        let mut prev = field(&C, DVec3::new(1.0e6, 0.0, 0.0), 2_000.0, 2.0, 0);
        for i in 1..200_000 {
            let h = field(&C, DVec3::new(1.0e6 + i as f64 * 1e-3 * 37.0, i as f64 * 1e-3 * 11.0, 0.0), 2_000.0, 2.0, 0);
            assert!((h - prev).abs() < 0.5, "jump {} at {i}", h - prev);
            prev = h;
        }
    }
}
