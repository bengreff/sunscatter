//! Rails: Keplerian elements relative to the parent node plus linear drift
//! rates (the "orbit + drift term" model, cf. JPL's approximate planetary
//! elements). Used for bodies whose motion the generator measured to fit this
//! model within tolerance.

use super::Kinematics;
use crate::kepler::Elements;
use crate::math;
use crate::time::Epoch;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rails {
    pub epoch: Epoch,
    /// Gravitational parameter used for the conic (parent + body).
    pub mu: f64,
    /// Elements at `epoch`.
    pub el: Elements,
    /// Linear rates of every element (per second). `rates.mean_anomaly` is the
    /// full mean-anomaly rate (mean motion plus any drift).
    pub rates: Elements,
}

impl Rails {
    pub fn elements_at(&self, t: Epoch) -> Elements {
        let dt = t.seconds_since(self.epoch);
        let (e0, r) = (&self.el, &self.rates);
        Elements {
            a: e0.a + r.a * dt,
            e: e0.e + r.e * dt,
            i: e0.i + r.i * dt,
            raan: math::wrap_tau(e0.raan + r.raan * dt),
            argp: math::wrap_tau(e0.argp + r.argp * dt),
            mean_anomaly: math::wrap_tau(e0.mean_anomaly + r.mean_anomaly * dt),
        }
    }

    /// Position and velocity from the drifting conic. The acceleration is the
    /// conic's (−μr/r³); the drift terms' own contribution is second order in
    /// the (tiny) rates and is neglected.
    pub fn eval(&self, t: Epoch) -> Kinematics {
        let (r, v) = self.elements_at(t).to_state(self.mu);
        let d = r.length();
        Kinematics { r, v, a: r * (-self.mu / (d * d * d)) }
    }
}
