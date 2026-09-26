//! Integer-hash lattice noise for terrain detail (D059).
//!
//! 3D gradient noise ("improved Perlin": quintic fade, 12 edge gradients)
//! whose lattice gradients come from an integer hash of the cell corner, so
//! the result is plain IEEE arithmetic: no trig, no tables that differ per
//! platform, bit-identical everywhere. Evaluated on 3D points (a direction
//! scaled by the body radius), so there are no map or cube-face seams.

use glam::DVec3;

/// 64-bit mix of a lattice point and a seed (SplitMix64 finaliser).
#[inline]
fn hash(x: i64, y: i64, z: i64, seed: u64) -> u64 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

/// Dot product of one of the 12 cube-edge gradients (chosen by `h`) with `(x, y, z)`.
#[inline]
fn grad(h: u64, x: f64, y: f64, z: f64) -> f64 {
    // Top 32 bits scaled to 0..12 (uniform, no modulo bias).
    match ((h >> 32) * 12) >> 32 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

/// `6t⁵ − 15t⁴ + 10t³`: C² interpolation weight.
#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Gradient noise at `p` (lattice units), roughly in [−1, 1], zero at lattice
/// points, C² continuous. `seed` selects an independent field.
pub fn gradient(p: DVec3, seed: u64) -> f64 {
    let (fx, fy, fz) = (p.x.floor(), p.y.floor(), p.z.floor());
    let (x, y, z) = (p.x - fx, p.y - fy, p.z - fz);
    let (ix, iy, iz) = (fx as i64, fy as i64, fz as i64);
    let g = |dx: i64, dy: i64, dz: i64| {
        grad(hash(ix + dx, iy + dy, iz + dz, seed), x - dx as f64, y - dy as f64, z - dz as f64)
    };
    let (u, v, w) = (fade(x), fade(y), fade(z));
    let x00 = lerp(g(0, 0, 0), g(1, 0, 0), u);
    let x10 = lerp(g(0, 1, 0), g(1, 1, 0), u);
    let x01 = lerp(g(0, 0, 1), g(1, 0, 1), u);
    let x11 = lerp(g(0, 1, 1), g(1, 1, 1), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_at_lattice_points_and_bounded() {
        for (i, p) in [(0.0, 0.0, 0.0), (5.0, -3.0, 12.0), (-1e6, 2e6, 3.0)].into_iter().enumerate() {
            assert_eq!(gradient(DVec3::from(p), i as u64), 0.0);
        }
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        let mut sum2 = 0.0;
        let n = 20_000;
        for k in 0..n {
            let t = k as f64;
            let p = DVec3::new(t * 0.137, t * 0.071 + 3.3, t * 0.029 - 7.7);
            let v = gradient(p, 7);
            lo = lo.min(v);
            hi = hi.max(v);
            sum2 += v * v;
        }
        let rms = (sum2 / n as f64).sqrt();
        assert!(lo > -1.2 && hi < 1.2 && lo < -0.5 && hi > 0.5, "{lo} {hi}");
        assert!(rms > 0.15 && rms < 0.4, "{rms}");
    }

    #[test]
    fn seeds_give_independent_fields() {
        let p = DVec3::new(0.3, 0.6, 0.2);
        assert_ne!(gradient(p, 1), gradient(p, 2));
    }

    #[test]
    fn continuous_across_cell_faces() {
        for p in [DVec3::new(1.0, 0.3, 0.4), DVec3::new(-2.0, -5.0, 0.5), DVec3::new(3e6, 2.5, -1.0)] {
            let a = gradient(p - DVec3::splat(1e-9), 3);
            let b = gradient(p + DVec3::splat(1e-9), 3);
            assert!((a - b).abs() < 1e-7, "{p}: {a} {b}");
        }
    }
}
