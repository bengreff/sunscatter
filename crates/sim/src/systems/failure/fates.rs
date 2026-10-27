//! Fates (D105; ship-model.md §6.3): each statistical failure mode of each
//! part draws once a threshold E = −ln u, u from a counter-based hash of
//! (the save's seed, the vessel's launch epoch, the part's id, the mode),
//! and accumulates its hazard H on the tick and the lattice; it fails when
//! H ≥ E. The same ascent from the same launch time gives the same outcome
//! (nothing is re-rolled on a reload: the fates are saved), a different
//! ascent accumulates a different H, and warp changes nothing (H is a sum
//! over the stored ticks and lattice steps).
//!
//! The hazard's rate is the mode's catalog rate (falcon9.md §6.3's low
//! ends) times its wear beyond the rated life and its damage
//! ([`multiplier`]): per hour of operation λ·dt; per start or per cycle the
//! event's own −ln(1 − p) (so one event fails with probability p exactly).

use crate::catalog::{Hazard, Life, Ratings};
use crate::craft::state::History;
use crate::math;
use crate::time::Epoch;

/// SplitMix64's finaliser: a bijection of 64 bits that mixes every input bit
/// into every output bit.
fn mix(z: u64) -> u64 {
    let z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// FNV-1a of a string's bytes (stable across platforms and versions,
/// unlike the standard library's hasher).
fn text(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
}

/// The uniform draw u ∈ (0, 1] of a mode: a counter-based hash of the
/// save's `seed`, the vessel's `launch` epoch, the part's or component's
/// id `owner` and the `mode`'s name (53 bits).
pub fn draw(seed: u64, launch: Epoch, owner: &str, mode: &str) -> f64 {
    let mut h = mix(seed ^ 0x9e37_79b9_7f4a_7c15);
    h = mix(h ^ launch.whole_seconds() as u64);
    h = mix(h ^ launch.fractional_second().to_bits());
    h = mix(h ^ text(owner));
    h = mix(h ^ text(mode));
    ((h >> 11) + 1) as f64 / (1u64 << 53) as f64
}

/// A mode's threshold E = −ln u: exponentially distributed with mean 1, so
/// a mode whose hazard reaches H fails with probability 1 − e^(−H).
pub fn threshold(seed: u64, launch: Epoch, owner: &str, mode: &str) -> f64 {
    -math::ln(draw(seed, launch, owner, mode))
}

/// The hazard of one event of probability `p` (a start, a cycle): −ln(1 − p).
pub fn per_event(p: f64) -> f64 {
    -math::ln(1.0 - p.clamp(0.0, 1.0 - 1e-15))
}

/// Wear beyond a rated life: 1 within it, then (used/rated)² (a wear-out
/// hazard rising as a Weibull of shape 3; E, the shape of TestFlight's
/// curves beyond the rated burn time).
pub fn wear(used: f64, rated: f64) -> f64 {
    if rated > 0.0 && used > rated {
        let x = used / rated;
        x * x
    } else {
        1.0
    }
}

/// A part's hazard multiplier: its wear against each rated life it has
/// (starts, hours or cycles to a 50 % chance of a defect; the rated burn
/// time for its run time) and its accumulated damage (1 + thermal +
/// vibration: a part at the edge of its damage fails twice as readily).
pub fn multiplier(r: &Ratings, h: &History, damage: f64) -> f64 {
    let life = match r.life {
        Some(Life::Starts(n)) => wear(f64::from(h.starts), n),
        Some(Life::Cycles(n)) => wear(f64::from(h.cycles), n),
        Some(Life::Hours(n)) => wear(h.run_time / 3600.0, n),
        None => 1.0,
    };
    let burn = r.burn_time.map_or(1.0, |t| wear(h.run_time, t));
    life * burn * (1.0 + damage.max(0.0))
}

/// The hazard a mode accumulates (before its multiplier): `dt` seconds of
/// operation, `starts` and `cycles` events.
pub fn hazard(h: Hazard, dt: f64, starts: u32, cycles: u32) -> f64 {
    match h {
        Hazard::PerHour(rate) => rate * dt / 3600.0,
        Hazard::PerStart(p) => per_event(p) * f64::from(starts),
        Hazard::PerCycle(p) => per_event(p) * f64::from(cycles),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t0() -> Epoch {
        Epoch::new(845_000_000, 0.25)
    }

    /// The same inputs give the same bits; any input changed gives another
    /// draw; the draws are uniform (a million of them: mean ½ and the
    /// thresholds' mean 1 within their sampling error).
    #[test]
    fn draws_are_counters_not_state() {
        let a = draw(7, t0(), "s1.merlin.9", "ignition");
        assert_eq!(a.to_bits(), draw(7, t0(), "s1.merlin.9", "ignition").to_bits());
        for other in [
            draw(8, t0(), "s1.merlin.9", "ignition"),
            draw(7, t0().add_seconds(1e-6), "s1.merlin.9", "ignition"),
            draw(7, t0(), "s1.merlin.1", "ignition"),
            draw(7, t0(), "s1.merlin.9", "in-flight shutdown"),
        ] {
            assert_ne!(a, other);
        }
        let n = 1_000_000;
        let (mut sum, mut e) = (0.0, 0.0);
        for k in 0..n {
            let u = draw(k, t0(), "part", "mode");
            assert!(u > 0.0 && u <= 1.0);
            sum += u;
            e += -math::ln(u);
        }
        let (mean, mean_e) = (sum / n as f64, e / n as f64);
        assert!((mean - 0.5).abs() < 5.0 * (1.0 / 12.0f64).sqrt() / (n as f64).sqrt(), "{mean}");
        assert!((mean_e - 1.0).abs() < 5.0 / (n as f64).sqrt(), "{mean_e}");
    }

    /// The fraction of fates a hazard H reaches is 1 − e^(−H): a start of
    /// probability p fails p of the vessels.
    #[test]
    fn a_hazard_fails_its_probability() {
        let p = 0.01;
        let n = 200_000;
        let failed = (0..n).filter(|&k| per_event(p) >= threshold(k, t0(), "e", "ignition")).count();
        let f = failed as f64 / n as f64;
        assert!((f - p).abs() < 5.0 * (p * (1.0 - p) / n as f64).sqrt(), "{f}");
    }

    #[test]
    fn hazards_and_multipliers_table() {
        // (hazard, dt, starts, cycles) → H.
        let cases = [
            (Hazard::PerHour(2.2e-3), 3600.0, 0, 0, 2.2e-3),
            (Hazard::PerHour(2.2e-3), 150.0, 1, 0, 2.2e-3 * 150.0 / 3600.0),
            (Hazard::PerStart(1.7e-5), 0.02, 1, 0, -math::ln(1.0 - 1.7e-5)),
            (Hazard::PerStart(1.7e-5), 0.02, 0, 5, 0.0),
            (Hazard::PerCycle(1.6e-4), 0.0, 0, 2, -2.0 * math::ln(1.0 - 1.6e-4)),
        ];
        for (h, dt, s, c, expect) in cases {
            assert!((hazard(h, dt, s, c) - expect).abs() < 1e-18, "{h:?}");
        }
        // A Merlin: 30 starts and 1,800 s rated; at its 45th start (×2.25)
        // and 3,600 s of running (×4), 0.5 damage (×1.5).
        let r = Ratings { life: Some(Life::Starts(30.0)), burn_time: Some(1800.0), ..Ratings::default() };
        let new = History::default();
        assert_eq!(multiplier(&r, &new, 0.0), 1.0);
        let old = History { starts: 45, run_time: 3600.0, ..History::default() };
        assert!((multiplier(&r, &old, 0.5) - 2.25 * 4.0 * 1.5).abs() < 1e-12);
        assert_eq!(wear(10.0, 0.0), 1.0, "no rating: no wear");
    }
}
