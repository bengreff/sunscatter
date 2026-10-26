//! Procedural relief for bodies without a global DEM (D086: "procedural
//! surfaces from their measured statistics, stated as such"). Baked once,
//! offline, into the body's `height.png`, so the simulation and the renderer
//! share it like any measured heightmap (D047); the simulation's procedural
//! detail (D059) adds what is below its resolution.
//!
//! The height above the body's reference spheroid is the sum of:
//! 1. **Shape:** the triaxial ellipsoid's radius minus the spheroid's
//!    (`a` towards longitude 0, the sub-planet point of a synchronous moon;
//!    `b` towards 90° E; `c` polar), for irregular or tidally stretched
//!    bodies whose `body.ron` keeps an oblate spheroid (`sqrt(ab)`, `c`).
//! 2. **Craters:** the IAU Gazetteer's named craters at their real positions
//!    and diameters ([`crate::gazetteer`]), then a random population with a
//!    cumulative size-frequency distribution `N(>D) = n1 · D^−slope` per km²
//!    (D in km) down to about three pixels, largest first, each a stamped
//!    profile ([`crater_profile`]): a parabolic bowl of depth `0.2 D` (simple
//!    craters) or a flat floor with a central peak and depth growing as
//!    `D^0.3` above the simple–complex transition, capped at `max_depth_m`
//!    (viscous relaxation on icy bodies); a raised rim; an ejecta blanket
//!    falling off as `r^−3` to 3 radii (Pike 1977; Melosh 1989, ch. 6 and 7;
//!    Schenk 2002 for icy satellites).
//! 3. **Regional relief:** value-noise fBm with RMS `relief_m` from
//!    `relief_wavelength_m` down to two pixels.
//!
//! Deterministic from the seed (this runs offline; the committed file is
//! what the game and simulation read).

use std::f64::consts::PI;

use crate::gazetteer;

/// A crater population.
#[derive(Clone, Copy, Debug)]
pub struct Craters {
    /// Cumulative density of craters larger than 1 km, per km².
    pub n1: f64,
    /// Exponent of the cumulative distribution (`N ∝ D^−slope`).
    pub slope: f64,
    /// Largest random crater (km).
    pub d_max_km: f64,
    /// Simple-to-complex transition diameter (km).
    pub transition_km: f64,
    /// Depth cap (m): relaxed large craters on icy bodies.
    pub max_depth_m: f64,
    /// Fraction of random craters young enough to keep bright ejecta.
    pub fresh: f64,
}

/// One body's procedural relief.
#[derive(Clone, Copy, Debug)]
pub struct Relief {
    /// Triaxial radii (m), or `None` for the spheroid itself.
    pub axes: Option<(f64, f64, f64)>,
    pub craters: Option<Craters>,
    /// Gazetteer target name (e.g. `"CALLISTO"`) for the named craters.
    pub gazetteer: Option<&'static str>,
    /// RMS of the regional fBm (m).
    pub relief_m: f64,
    /// Longest wavelength of the regional fBm (m).
    pub relief_wavelength_m: f64,
    pub seed: u64,
}

/// A crater on the sphere.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crater {
    pub lat: f64,
    pub lon: f64,
    pub diameter_m: f64,
    /// Has bright ejecta (raises the procedural colour's brightness).
    pub fresh: bool,
}

/// Brightness factor of fresh ejecta at `x` crater radii from the centre.
pub fn ejecta_brightness(x: f64) -> f64 {
    1.0 + 0.6 * (1.0 - x / 2.5).max(0.0)
}

/// Generated relief: heights (m) above the spheroid and a brightness factor
/// for the procedural colour, both `w × h`.
pub struct Generated {
    pub heights: Vec<f32>,
    pub brightness: Vec<f32>,
}

/// SplitMix64: a small deterministic generator.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Depth and rim height (m) of a crater of diameter `d` (m).
pub fn crater_shape(d: f64, c: &Craters) -> (f64, f64) {
    let dt = c.transition_km * 1000.0;
    let (depth, rim) =
        if d <= dt { (0.2 * d, 0.04 * d) } else { (0.2 * dt * (d / dt).powf(0.3), 0.04 * dt * (d / dt).powf(0.4)) };
    let depth = depth.min(c.max_depth_m);
    (depth, rim.min(0.5 * depth))
}

/// Height (m) of a crater's profile at `x` = distance from the centre over
/// the radius: floor `−depth`, rim `+rim` at `x = 1`, ejecta to `x = 3`;
/// `complex` craters have a flat floor to `x = 0.5` and a central peak.
pub fn crater_profile(x: f64, depth: f64, rim: f64, complex: bool) -> f64 {
    if x >= 3.0 {
        return 0.0;
    }
    if x >= 1.0 {
        let tail = 1.0 / 27.0;
        return rim * (x.powi(-3) - tail) / (1.0 - tail);
    }
    if complex {
        let floor = 0.5;
        let t = ((x - floor) / (1.0 - floor)).max(0.0);
        let peak = 0.4 * depth * (-(x / 0.12).powi(2)).exp();
        -depth + (depth + rim) * t * t + peak
    } else {
        -depth + (depth + rim) * x * x
    }
}

/// How much of the older surface survives under a new crater at `x` radii:
/// none inside 0.8 radii, all beyond 1.2 (the ejecta add to it).
pub fn overprint_keep(x: f64) -> f64 {
    let t = ((x - 0.8) / 0.4).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Random craters for a body of radius `radius` (m), from `d_min` (m) up to
/// the population's largest, sorted largest first.
pub fn random_craters(c: &Craters, radius: f64, d_min: f64, rng: &mut Rng) -> Vec<Crater> {
    let area_km2 = 4.0 * PI * (radius / 1000.0).powi(2);
    let (lo, hi) = (d_min / 1000.0, c.d_max_km.max(d_min / 1000.0));
    let n = (c.n1 * (lo.powf(-c.slope) - hi.powf(-c.slope)) * area_km2).round() as usize;
    let mut out: Vec<Crater> = (0..n)
        .map(|_| {
            // Inverse transform of the truncated power law.
            let u = rng.unit();
            let a = lo.powf(-c.slope);
            let b = hi.powf(-c.slope);
            let d_km = (a - u * (a - b)).powf(-1.0 / c.slope);
            let lat = (2.0 * rng.unit() - 1.0).asin().to_degrees();
            let lon = 360.0 * rng.unit() - 180.0;
            let fresh = rng.unit() < c.fresh;
            Crater { lat, lon, diameter_m: d_km * 1000.0, fresh }
        })
        .collect();
    out.sort_by(|p, q| q.diameter_m.total_cmp(&p.diameter_m));
    out
}

/// Unit vector of latitude/longitude in degrees.
fn unit(lat: f64, lon: f64) -> [f64; 3] {
    let (la, lo) = (lat.to_radians(), lon.to_radians());
    [la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()]
}

/// Adds `craters` to a `w × h` height grid of a body of radius `radius`,
/// and the fresh ones' ejecta to the brightness grid.
pub fn stamp(z: &mut [f32], bright: &mut [f32], w: usize, h: usize, radius: f64, craters: &[Crater], pop: &Craters) {
    let rows: Vec<(f64, f64)> = (0..h)
        .map(|j| {
            let lat = (90.0 - (j as f64 + 0.5) * 180.0 / h as f64).to_radians();
            (lat.sin(), lat.cos())
        })
        .collect();
    let cols: Vec<(f64, f64)> = (0..w)
        .map(|i| {
            let lon = (-180.0 + (i as f64 + 0.5) * 360.0 / w as f64).to_radians();
            (lon.sin(), lon.cos())
        })
        .collect();
    let mut foot: Vec<(usize, f64)> = Vec::new();
    for c in craters {
        let (depth, rim) = crater_shape(c.diameter_m, pop);
        let complex = c.diameter_m > pop.transition_km * 1000.0;
        let r_ang = 0.5 * c.diameter_m / radius;
        let reach = (3.0 * r_ang).min(PI);
        let p = unit(c.lat, c.lon);
        let j0 = (((90.0 - c.lat - reach.to_degrees()) / 180.0 * h as f64).floor().max(0.0)) as usize;
        let j1 = (((90.0 - c.lat + reach.to_degrees()) / 180.0 * h as f64).ceil() as usize).min(h);
        let cos_reach = reach.cos();
        let lat_c = c.lat.to_radians();
        foot.clear();
        for j in j0..j1 {
            let (sl, cl) = rows[j];
            // Longitude half-width at this row (all columns near the poles).
            let num = cos_reach - sl * lat_c.sin();
            let den = cl * lat_c.cos();
            let dlon = if den <= 1e-12 || num / den <= -1.0 {
                PI
            } else if num / den >= 1.0 {
                continue;
            } else {
                (num / den).acos()
            };
            let span = ((dlon / (2.0 * PI)) * w as f64).ceil() as i64 + 1;
            let ic = ((c.lon + 180.0) / 360.0 * w as f64).floor() as i64;
            let (a, b) = if 2 * span + 1 >= w as i64 { (0, w as i64 - 1) } else { (ic - span, ic + span) };
            for ii in a..=b {
                let i = ii.rem_euclid(w as i64) as usize;
                let (so, co) = cols[i];
                let q = [cl * co, cl * so, sl];
                let dot = (p[0] * q[0] + p[1] * q[1] + p[2] * q[2]).clamp(-1.0, 1.0);
                let x = dot.acos() / r_ang;
                if x < 3.0 {
                    foot.push((j * w + i, x));
                }
            }
        }
        // Overprinting: inside its rim a crater replaces the older surface,
        // measured from that surface's mean under the crater, so saturated
        // surfaces keep realistic depths instead of accumulating holes.
        let (sum, n) =
            foot.iter().filter(|f| f.1 < 1.0).fold((0.0, 0.0), |(s, n), &(k, _)| (s + f64::from(z[k]), n + 1.0));
        let base = if n > 0.0 { sum / n } else { 0.0 };
        for &(k, x) in &foot {
            let keep = overprint_keep(x);
            z[k] = (f64::from(z[k]) * keep + base * (1.0 - keep) + crater_profile(x, depth, rim, complex)) as f32;
            if c.fresh {
                bright[k] = bright[k].max(ejecta_brightness(x) as f32);
            }
        }
    }
}

/// 3D value noise in [−1, 1] at `p` (lattice spacing 1).
fn value_noise(p: [f64; 3], seed: u64) -> f64 {
    let f = p.map(f64::floor);
    let t = [p[0] - f[0], p[1] - f[1], p[2] - f[2]].map(|t| t * t * (3.0 - 2.0 * t));
    let corner = |dx: i64, dy: i64, dz: i64| {
        let mut r = Rng::new(
            seed ^ ((f[0] as i64 + dx) as u64).wrapping_mul(0x8DA6_B343)
                ^ ((f[1] as i64 + dy) as u64).wrapping_mul(0xD816_3841)
                ^ ((f[2] as i64 + dz) as u64).wrapping_mul(0xCB1A_B31F),
        );
        2.0 * r.unit() - 1.0
    };
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x00 = lerp(corner(0, 0, 0), corner(1, 0, 0), t[0]);
    let x10 = lerp(corner(0, 1, 0), corner(1, 1, 0), t[0]);
    let x01 = lerp(corner(0, 0, 1), corner(1, 0, 1), t[0]);
    let x11 = lerp(corner(0, 1, 1), corner(1, 1, 1), t[0]);
    lerp(lerp(x00, x10, t[1]), lerp(x01, x11, t[1]), t[2])
}

/// Regional fBm (m) at unit direction `u` on a body of radius `radius`.
fn regional(u: [f64; 3], radius: f64, r: &Relief, shortest: f64) -> f64 {
    let mut wl = r.relief_wavelength_m;
    let (mut amp, mut sum, mut norm, mut k) = (1.0, 0.0, 0.0, 0u64);
    while wl >= shortest && k < 16 {
        let s = radius / wl;
        sum += amp * value_noise([u[0] * s, u[1] * s, u[2] * s], r.seed.wrapping_add(k));
        norm += amp * amp;
        amp *= 0.5;
        wl *= 0.5;
        k += 1;
    }
    // Value noise has an RMS of about 0.33 per octave.
    if norm > 0.0 {
        r.relief_m * sum / (0.33 * norm.sqrt())
    } else {
        0.0
    }
}

/// The triaxial radius minus the spheroid's at a unit direction.
pub fn shape_height(u: [f64; 3], axes: (f64, f64, f64), spheroid_radius: f64) -> f64 {
    let (a, b, c) = axes;
    let r = 1.0 / ((u[0] / a).powi(2) + (u[1] / b).powi(2) + (u[2] / c).powi(2)).sqrt();
    r - spheroid_radius
}

/// Bakes the relief on a `w × h` grid for a body with reference spheroid `s`.
pub fn generate(r: &Relief, s: crate::planets::Spheroid, w: usize, h: usize) -> Generated {
    let radius = s.a;
    let px = 2.0 * PI * radius / w as f64;
    let mut z = vec![0.0f32; w * h];
    let mut bright = vec![1.0f32; w * h];
    if let Some(pop) = &r.craters {
        let mut list: Vec<Crater> = r.gazetteer.map_or(Vec::new(), |t| {
            gazetteer::craters(t)
                .into_iter()
                .map(|f| Crater { lat: f.lat, lon: f.lon, diameter_m: f.diameter_km * 1000.0, fresh: false })
                .filter(|c| c.diameter_m >= 3.0 * px)
                .collect()
        });
        let named = list.len();
        list.sort_by(|p, q| q.diameter_m.total_cmp(&p.diameter_m));
        let mut rng = Rng::new(r.seed);
        let random = random_craters(pop, radius, 3.0 * px, &mut rng);
        println!("  craters: {named} named (IAU Gazetteer), {} random from {:.1} km", random.len(), 3.0 * px / 1000.0);
        // Random craters above the largest named one are left out, so the
        // big real ones dominate; the rest interleave by size.
        let mut all: Vec<Crater> = list.into_iter().chain(random).collect();
        all.sort_by(|p, q| q.diameter_m.total_cmp(&p.diameter_m));
        stamp(&mut z, &mut bright, w, h, radius, &all, pop);
        // Nested craters deepen without limit here (the ejecta's infill of
        // saturated surfaces is not modelled): soft-limit the crater relief
        // to 1.5 depth caps.
        let limit = 1.5 * pop.max_depth_m;
        for v in &mut z {
            *v = (limit * (f64::from(*v) / limit).tanh()) as f32;
        }
    }
    for j in 0..h {
        let lat = 90.0 - (j as f64 + 0.5) * 180.0 / h as f64;
        let reference = s.radius(lat);
        for i in 0..w {
            let lon = -180.0 + (i as f64 + 0.5) * 360.0 / w as f64;
            let u = unit(lat, lon);
            let mut v = f64::from(z[j * w + i]);
            if r.relief_m > 0.0 {
                v += regional(u, radius, r, 2.0 * px);
            }
            if let Some(axes) = r.axes {
                v += shape_height(u, axes, reference);
            }
            z[j * w + i] = v as f32;
        }
    }
    Generated { heights: z, brightness: bright }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POP: Craters =
        Craters { n1: 0.1, slope: 2.0, d_max_km: 300.0, transition_km: 15.0, max_depth_m: 4000.0, fresh: 0.1 };

    #[test]
    fn profiles_have_a_floor_a_rim_and_no_far_effect() {
        let (d, rim) = crater_shape(10_000.0, &POP);
        assert_eq!((d, rim), (2000.0, 400.0));
        assert_eq!(crater_profile(0.0, d, rim, false), -d);
        assert!((crater_profile(1.0, d, rim, false) - rim).abs() < 1e-9);
        assert_eq!(crater_profile(3.0, d, rim, false), 0.0);
        assert!(crater_profile(2.9, d, rim, false).abs() < 0.01 * rim);
        // Continuous at the rim and monotonic outside it.
        assert!((crater_profile(0.999_999, d, rim, false) - crater_profile(1.000_001, d, rim, false)).abs() < 1.0);
        assert!(crater_profile(1.5, d, rim, false) > crater_profile(2.0, d, rim, false));
        // Complex craters: shallower than 0.2 D, capped, with a central peak above the floor.
        let (d2, r2) = crater_shape(100_000.0, &POP);
        assert!(d2 <= POP.max_depth_m && r2 <= 0.5 * d2);
        assert!(0.2 * 20_000.0 * (5.0f64).powf(0.3) > POP.max_depth_m, "the cap binds here");
        assert!(crater_profile(0.0, d2, r2, true) > crater_profile(0.3, d2, r2, true));
        assert_eq!(crater_profile(0.3, d2, r2, true), -d2 + 0.4 * d2 * (-(0.3f64 / 0.12).powi(2)).exp());
    }

    #[test]
    fn random_population_follows_the_distribution() {
        let mut rng = Rng::new(7);
        let radius = 500_000.0;
        let list = random_craters(&POP, radius, 5_000.0, &mut rng);
        let area = 4.0 * PI * 500.0f64.powi(2);
        let expect = |d: f64| POP.n1 * (d.powf(-2.0) - 300.0f64.powf(-2.0)) * area;
        assert_eq!(list.len(), expect(5.0).round() as usize);
        let above_10 = list.iter().filter(|c| c.diameter_m > 10_000.0).count() as f64;
        assert!((above_10 - expect(10.0)).abs() < 4.0 * expect(10.0).sqrt(), "{above_10} vs {}", expect(10.0));
        assert!(list.windows(2).all(|p| p[0].diameter_m >= p[1].diameter_m));
        // Same seed, same craters.
        assert_eq!(list, random_craters(&POP, radius, 5_000.0, &mut Rng::new(7)));
    }

    #[test]
    fn a_stamped_crater_is_centred_where_it_was_placed() {
        let (w, h) = (360, 180);
        let radius = 1_000_000.0;
        let mut z = vec![0.0f32; w * h];
        let c = Crater { lat: 30.5, lon: -99.5, diameter_m: 200_000.0, fresh: true };
        let mut bright = vec![1.0f32; w * h];
        // A simple (bowl) crater: deepest at its centre.
        stamp(&mut z, &mut bright, w, h, radius, &[c], &Craters { transition_km: 1000.0, ..POP });
        assert_eq!(bright[59 * w + 80], ejecta_brightness(0.0) as f32);
        assert_eq!(bright[59 * w + 180], 1.0);
        let (k, lo) =
            z.iter().enumerate().fold((0, f32::MAX), |(bk, bv), (k, &v)| if v < bv { (k, v) } else { (bk, bv) });
        let (i, j) = (k % w, k / w);
        assert_eq!((i, j), (80, 59), "deepest pixel");
        assert!(lo < 0.0);
        // Nothing beyond three radii (17.2° here).
        assert_eq!(z[(59) * w + 180], 0.0);
        // A crater over the pole wraps every longitude without panicking.
        let mut zp = vec![0.0f32; w * h];
        let polar = Crater { lat: 89.0, lon: 10.0, diameter_m: 300_000.0, fresh: false };
        stamp(&mut zp, &mut bright, w, h, radius, &[polar], &POP);
        assert!(zp[0] < 0.0 && zp[w / 2] < 0.0);
    }

    #[test]
    fn triaxial_shape_heights() {
        let (a, b, c) = (207_800.0f64, 196_700.0f64, 190_600.0f64);
        let eq = (a * b).sqrt();
        assert!((shape_height([1.0, 0.0, 0.0], (a, b, c), eq) - (a - eq)).abs() < 1e-6);
        assert!((shape_height([0.0, 1.0, 0.0], (a, b, c), eq) - (b - eq)).abs() < 1e-6);
        assert!(shape_height([0.0, 0.0, 1.0], (a, b, c), c).abs() < 1e-6);
    }

    #[test]
    fn regional_relief_has_the_requested_size() {
        let r = Relief {
            axes: None,
            craters: None,
            gazetteer: None,
            relief_m: 500.0,
            relief_wavelength_m: 400_000.0,
            seed: 3,
        };
        let s = crate::planets::Spheroid { a: 1_000_000.0, c: 1_000_000.0 };
        let z = generate(&r, s, 512, 256).heights;
        let rms = (z.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>() / z.len() as f64).sqrt();
        assert!(rms > 200.0 && rms < 1000.0, "rms {rms}");
        assert_eq!(z, generate(&r, s, 512, 256).heights);
    }
}
