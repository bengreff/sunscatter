//! Light time: the light-time equation, solved by fixed-point iteration
//! (SPICE's "converged Newtonian" correction: three iterations, since the
//! bodies move at ~1e-4 c).

use super::C;
use glam::DVec3;

/// Iterations of the light-time equation (each gains a factor ~v/c).
const ITERATIONS: usize = 3;

/// Seconds light takes from a transmitter to a receiver at `rx` (at the
/// reception time), given the transmitter's position `tx_before(τ)` τ
/// seconds before reception. Both in one inertial frame.
pub fn light_time(rx: DVec3, tx_before: impl Fn(f64) -> DVec3) -> f64 {
    let mut tau = (rx - tx_before(0.0)).length() / C;
    for _ in 0..ITERATIONS {
        tau = (rx - tx_before(tau)).length() / C;
    }
    tau
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_pair_is_distance_over_c() {
        let tau = light_time(DVec3::ZERO, |_| DVec3::new(C, 0.0, 0.0));
        assert!((tau - 1.0).abs() < 1e-15);
    }

    #[test]
    fn receding_transmitter_solves_the_equation() {
        // Transmitter at x = d + v·(t − t_rx): it was nearer when it sent.
        // Exact: τ = d / (c + v).
        let (d, v) = (3.84e8, 1.0e3);
        let tau = light_time(DVec3::ZERO, |back| DVec3::new(d - v * back, 0.0, 0.0));
        assert!((tau - d / (C + v)).abs() < 1e-12, "{tau}");
    }
}
