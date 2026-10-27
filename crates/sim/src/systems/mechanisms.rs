//! Mechanisms (D103; ship-model.md §5.4): hinges driven by their actuators
//! at their rates (Falcon 9's legs on helium pneumatics, its grid fins on
//! hydraulics, Dragon's nosecone on an electric drive). A mechanism moves
//! from its angle towards its commanded target at its rate; the craft's
//! shape changes when it reaches its deployed angle (`craft::configure`, a
//! configuration of its own), until then the part is drawn turning.

use crate::craft::ship::ShipDef;
use crate::craft::state::ShipState;

/// The next angle (deg) of a hinge at `angle` driven towards `target` at
/// `rate` deg/s for `dt` s: it stops exactly on the target.
pub fn travel(angle: f64, target: f64, rate: f64, dt: f64) -> f64 {
    let step = rate.max(0.0) * dt;
    if rate <= 0.0 || (target - angle).abs() <= step {
        target
    } else {
        angle + step * (target - angle).signum()
    }
}

/// Moves every mechanism for `dt` seconds.
pub fn step(def: &ShipDef, state: &mut ShipState, dt: f64) {
    for (d, s) in def.mechanisms.iter().zip(state.mechanisms.iter_mut()) {
        s.angle = travel(s.angle, s.target, d.item.rate_dps, dt);
    }
}

/// Whether any mechanism is still moving.
pub fn moving(state: &ShipState) -> bool {
    state.mechanisms.iter().any(|m| m.angle != m.target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travel_table() {
        // (angle, target, rate, dt) → angle
        let cases = [
            (0.0, 120.0, 40.0, 0.02, 0.8),
            (119.5, 120.0, 40.0, 0.02, 120.0),
            (90.0, 0.0, 45.0, 1.0, 45.0),
            (10.0, 10.0, 45.0, 1.0, 10.0),
            (0.0, 100.0, 0.0, 1.0, 100.0), // no rate given: at once
        ];
        for (angle, target, rate, dt, want) in cases {
            assert!((travel(angle, target, rate, dt) - want).abs() < 1e-12, "{angle} → {target}");
        }
        // Falcon 9's legs: 120° at 40°/s in 3 s (150 ticks of 20 ms, one
        // more for the rounding of the sum), ending exactly on the target.
        let mut a = 0.0;
        let mut n = 0;
        while a != 120.0 {
            a = travel(a, 120.0, 40.0, 0.02);
            n += 1;
        }
        assert!(n == 150 || n == 151, "{n}");
    }
}
