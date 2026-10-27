//! Accumulated damage (D105; ship-model.md §5.9, §6.2): time at
//! temperature and vibration, each a Miner's sum that fails its part at 1.
//!
//! * **Thermal** ([`ThermalRating`]): at or above its destruction
//!   temperature a part is destroyed at once; below its rated (service)
//!   temperature it takes no damage; between them its life at temperature
//!   T follows a Larson–Miller line, T·(C + log₁₀ t) constant, through two
//!   anchors: it holds its rated time at the rated temperature and one
//!   second at the destruction temperature, so log₁₀ t(T) = C·(T_d/T − 1)
//!   with C = T_r·log₁₀ t_r/(T_d − T_r). Damage grows by dt/t(T).
//!   ("4000 K for complete failure; below it, integrated heat.")
//! * **Vibration** ([`miner`], [`environment`]): each part's random
//!   vibration (g rms) is the largest its sources give it — a running
//!   engine at its mount, attenuated by a transmissibility across each joint
//!   on the way; buffeting from the dynamic pressure, strongest
//!   transonic — and its damage grows by (G/G_qual)^b·dt/t_qual against its
//!   qualification level and duration (Miner's rule, b ≈ 4 for random
//!   fatigue of metal structure, falcon9.md §6.4). A part without a
//!   qualification rating takes none (no data, no failure).

use crate::math;

/// What a part's time at temperature is judged by (K, s).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThermalRating {
    pub rated_k: f64,
    pub destruction_k: f64,
    /// How long it holds at its rated temperature.
    pub hold_s: f64,
}

/// The hold at the rated temperature when a part's ratings give none:
/// 1,000 hours (a service temperature is a long-term one; generous, so an
/// unknown hold never fails a part early).
pub const DEFAULT_HOLD_S: f64 = 3.6e6;

impl ThermalRating {
    /// The Larson–Miller constant through the two anchors.
    fn c(&self) -> f64 {
        self.rated_k * math::log10(self.hold_s.max(1.0)) / (self.destruction_k - self.rated_k)
    }

    /// The life (s) at temperature `k`: infinite at or below the rated
    /// temperature, zero at or above the destruction temperature.
    pub fn life(&self, k: f64) -> f64 {
        if self.destruction_k.partial_cmp(&self.rated_k) != Some(std::cmp::Ordering::Greater) || k <= self.rated_k {
            return f64::INFINITY;
        }
        if k >= self.destruction_k {
            return 0.0;
        }
        math::pow(10.0, self.c() * (self.destruction_k / k - 1.0))
    }

    /// The damage of `dt` seconds at `k` (∞ at the destruction temperature).
    pub fn damage(&self, k: f64, dt: f64) -> f64 {
        let life = self.life(k);
        if life == f64::INFINITY {
            0.0
        } else if life <= 0.0 {
            f64::INFINITY
        } else {
            dt / life
        }
    }
}

/// Miner's exponent for random vibration fatigue (E, falcon9.md §6.4).
pub const MINER_EXPONENT: f64 = 4.0;

/// The damage of `dt` seconds at `g` rms against a qualification of
/// `qual` g rms for `qual_s` seconds: (g/qual)^b·dt/qual_s.
pub fn miner(g: f64, qual: f64, qual_s: f64, dt: f64) -> f64 {
    if !(qual > 0.0 && qual_s > 0.0) || g <= 0.0 {
        return 0.0;
    }
    math::powi(g / qual, MINER_EXPONENT as i32) * dt / qual_s
}

/// Random vibration at an engine's mount at full thrust (g rms;
/// engine-mounted equipment, E, falcon9.md §6.4), scaled by its level.
pub const ENGINE_GRMS: f64 = 20.0;
/// What a bolted joint passes on (E: 0.5–0.7, −3 to −6 dB).
pub const TRANSMISSIBILITY: f64 = 0.6;
/// Buffeting and acoustics on the outer structure at the reference
/// dynamic pressure, transonic (g rms; E: the payload envelope's 5.13 g rms
/// covers all flight events, falcon9.md §6.4).
pub const BUFFET_GRMS: f64 = 5.0;
/// The reference dynamic pressure (Pa): Falcon 9's max-Q (falcon9.md §4.3).
pub const BUFFET_Q: f64 = 35_000.0;

/// Buffeting's share at Mach `mach`: 1 transonic (0.8–1.2), half well
/// sub- or supersonic, smoothly between (E: the classic transonic peak of
/// launch-vehicle buffet).
pub fn buffet_shape(mach: f64) -> f64 {
    let x = if mach < 0.8 {
        (0.8 - mach) / 0.3
    } else if mach > 1.2 {
        (mach - 1.2) / 0.8
    } else {
        0.0
    };
    0.5 + 0.5 * math::exp(-x * x)
}

/// The buffeting a part exposed to the flow feels at dynamic pressure `q`
/// and Mach `mach` (g rms), proportional to q.
pub fn buffet(q: f64, mach: f64) -> f64 {
    BUFFET_GRMS * (q.max(0.0) / BUFFET_Q) * buffet_shape(mach)
}

/// Each part's vibration (g rms): the largest of its `own` level (buffeting)
/// and every `source` (part index, g rms at it) attenuated by
/// [`TRANSMISSIBILITY`] per joint between them along the tree (`parents`
/// by index).
pub fn environment(parents: &[Option<usize>], own: &[f64], sources: &[(usize, f64)]) -> Vec<f64> {
    let n = parents.len();
    let mut g: Vec<f64> = (0..n).map(|i| own.get(i).copied().unwrap_or(0.0)).collect();
    // Neighbours along the tree (parent and children).
    let mut near: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, p) in parents.iter().enumerate() {
        if let Some(p) = p.filter(|&p| p < n && p != i) {
            near[i].push(p);
            near[p].push(i);
        }
    }
    for &(at, level) in sources.iter().filter(|s| s.0 < n && s.1 > 0.0) {
        // Breadth first: each joint passes on TRANSMISSIBILITY of it.
        let mut seen = vec![false; n];
        let mut front = vec![at];
        seen[at] = true;
        let mut level = level;
        while !front.is_empty() && level > 1e-3 {
            let mut next = Vec::new();
            for &i in &front {
                g[i] = g[i].max(level);
                for &j in &near[i] {
                    if !seen[j] {
                        seen[j] = true;
                        next.push(j);
                    }
                }
            }
            front = next;
            level *= TRANSMISSIBILITY;
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Larson–Miller line through its anchors: the rated hold at the
    /// rated temperature, a second at destruction, monotone between; no
    /// damage below the rating, destruction at once at the limit.
    #[test]
    fn time_at_temperature_table() {
        // Al-Li 2195 with the default hold: 422 K rated, 833 K destroyed.
        let r = ThermalRating { rated_k: 422.0, destruction_k: 833.0, hold_s: DEFAULT_HOLD_S };
        assert_eq!(r.life(400.0), f64::INFINITY);
        assert!((r.life(422.0 + 1e-9) / DEFAULT_HOLD_S - 1.0).abs() < 1e-6);
        assert!((r.life(833.0 - 1e-9) - 1.0).abs() < 1e-6);
        assert_eq!(r.life(833.0), 0.0);
        let mut last = f64::INFINITY;
        for k in [450.0, 500.0, 600.0, 700.0, 800.0] {
            let t = r.life(k);
            assert!(t < last, "{k}: {t}");
            last = t;
        }
        // Halfway in 1/T between the anchors, halfway in log t.
        let mid = 2.0 / (1.0 / 422.0 + 1.0 / 833.0);
        assert!((math::log10(r.life(mid)) - 0.5 * math::log10(DEFAULT_HOLD_S)).abs() < 1e-9);
        assert_eq!(r.damage(300.0, 1.0), 0.0);
        assert_eq!(r.damage(900.0, 0.02), f64::INFINITY);
        // A heat shield rated for 4,000 K's destruction holds 300 s at its
        // 3,000 K service temperature: at 3,500 K a fraction of a minute.
        let shield = ThermalRating { rated_k: 3000.0, destruction_k: 4000.0, hold_s: 300.0 };
        let t = shield.life(3500.0);
        assert!(t > 1.0 && t < 60.0, "{t}");
    }

    #[test]
    fn miner_table() {
        // At the qualification level for the qualification time: 1.
        assert!((miner(26.0, 26.0, 450.0, 450.0) - 1.0).abs() < 1e-12);
        // Half the level: 1/16 the rate.
        assert!((miner(13.0, 26.0, 450.0, 450.0) - 1.0 / 16.0).abs() < 1e-12);
        // A Merlin's ascent: 20 g rms for 150 s against 26 g rms for 450 s.
        let d = miner(20.0, 26.0, 450.0, 150.0);
        assert!((d - (20.0f64 / 26.0).powi(4) / 3.0).abs() < 1e-12, "{d}");
        // No rating, no damage.
        assert_eq!(miner(50.0, 0.0, 0.0, 10.0), 0.0);
    }

    /// Two engines on a thrust structure under a tank: each engine's level
    /// at it, the structure the larger engine's × 0.6, the tank × 0.36;
    /// buffeting where it is larger.
    #[test]
    fn vibration_through_the_joints() {
        // 0 tank (root) ← 1 octaweb ← 2, 3 engines.
        let parents = [None, Some(0), Some(1), Some(1)];
        let g = environment(&parents, &[2.0, 0.0, 0.0, 0.0], &[(2, 20.0), (3, 10.0)]);
        assert_eq!(g[2], 20.0);
        assert_eq!(g[3], 10.0, "its own, louder than the other's through the octaweb (7.2)");
        assert!((g[1] - 12.0).abs() < 1e-12);
        assert!((g[0] - 7.2).abs() < 1e-12);
        let quiet = environment(&parents, &[9.0, 0.0, 0.0, 0.0], &[(2, 5.0)]);
        assert_eq!(quiet[0], 9.0);
        // Buffeting: transonic peak at max-Q.
        assert!((buffet(BUFFET_Q, 1.0) - BUFFET_GRMS).abs() < 1e-12);
        assert!(buffet(BUFFET_Q, 3.0) < 0.6 * BUFFET_GRMS && buffet(BUFFET_Q, 0.3) < 0.6 * BUFFET_GRMS);
        assert_eq!(buffet(0.0, 1.0), 0.0);
    }
}
