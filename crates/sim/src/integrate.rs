//! Integrators.
//!
//! * [`yoshida8_step`]: fixed-step 8th-order symplectic composition for the
//!   celestial N-body generation (bodies are integrated once, ahead of time).
//! * [`Dopri5`]: adaptive Dormand–Prince 5(4) for vessels, with compensated
//!   summation. Its full state is a plain value ([`StepState`]), so integrating
//!   in chunks is bit-identical to integrating in one go.
//! * [`hermite5`]: quintic Hermite interpolation between accepted steps (using
//!   position, velocity and acceleration at both ends). Coast segments store only
//!   step endpoints; this is the "dense output" the game evaluates.

use crate::math::{Compensated, CompensatedScalar};
use glam::DVec3;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Symplectic composition (bodies)
// ---------------------------------------------------------------------------

/// A separable Hamiltonian system: positions drift with velocity, velocities
/// kick with position-dependent acceleration.
pub trait Separable {
    fn drift(&mut self, h: f64);
    fn kick(&mut self, h: f64);
}

/// Yoshida (1990) 8th-order coefficients, solution A.
const YOSHIDA8_W: [f64; 7] = [
    1.042_426_208_699_91,
    1.820_206_309_707_14,
    0.157_739_928_123_617,
    2.440_027_326_167_35,
    -0.007_169_894_197_081_20,
    -2.446_991_823_705_24,
    -1.615_823_741_500_97,
];

/// The 15 symmetric substep weights `w7..w1, w0, w1..w7`.
fn yoshida8_sequence() -> [f64; 15] {
    let w0 = 1.0 - 2.0 * YOSHIDA8_W.iter().sum::<f64>();
    let mut seq = [0.0; 15];
    for (k, w) in YOSHIDA8_W.iter().enumerate() {
        seq[k] = *w;
        seq[14 - k] = *w;
    }
    seq[7] = w0;
    seq
}

/// One step of size `h` of the 8th-order composition of drift-kick-drift
/// leapfrogs. Adjacent half-drifts are merged (16 drifts, 15 kicks).
pub fn yoshida8_step<S: Separable>(s: &mut S, h: f64) {
    let seq = yoshida8_sequence();
    let mut pending_drift = 0.5 * seq[0] * h;
    for k in 0..15 {
        s.drift(pending_drift);
        s.kick(seq[k] * h);
        pending_drift = if k + 1 < 15 { 0.5 * (seq[k] + seq[k + 1]) * h } else { 0.5 * seq[k] * h };
    }
    s.drift(pending_drift);
}

// ---------------------------------------------------------------------------
// Adaptive Dormand–Prince 5(4) (vessels)
// ---------------------------------------------------------------------------

/// Second-order dynamics: `a = f(t, r, v)`. `t` is local seconds since the
/// integration's reference epoch.
pub trait Dynamics {
    fn accel(&self, t: f64, r: DVec3, v: DVec3) -> DVec3;

    /// The acceleration and the rate `ẋ(t, r, v)` of an extra scalar carried
    /// with the state ([`StepState::x`], proper time): integrated with the
    /// same stages but outside error control, so it never changes the steps.
    /// None by default.
    fn accel_rate(&self, t: f64, r: DVec3, v: DVec3) -> (DVec3, f64) {
        (self.accel(t, r, v), 0.0)
    }
}

impl<F: Fn(f64, DVec3, DVec3) -> DVec3> Dynamics for F {
    fn accel(&self, t: f64, r: DVec3, v: DVec3) -> DVec3 {
        self(t, r, v)
    }
}

/// Everything needed to continue integrating. Storing this with a segment and
/// resuming later reproduces a single uninterrupted run bit for bit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepState {
    pub t: f64,
    pub r: Compensated,
    pub v: Compensated,
    /// Acceleration at `(t, r, v)` (first-same-as-last).
    pub a: DVec3,
    /// Step size to attempt next.
    pub h: f64,
    /// The extra scalar ([`Dynamics::accel_rate`]) and its rate at `t`.
    #[serde(default)]
    pub x: CompensatedScalar,
    #[serde(default)]
    pub x_rate: f64,
}

/// One accepted step: endpoints for Hermite interpolation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepSample {
    pub t: f64,
    pub r: DVec3,
    pub v: DVec3,
    pub a: DVec3,
}

#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    /// Absolute tolerance on position (m) and velocity (m/s) components.
    pub abs_r: f64,
    pub abs_v: f64,
    /// Relative tolerance.
    pub rel: f64,
    pub h_min: f64,
    pub h_max: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Tolerance { abs_r: 1e-3, abs_v: 1e-6, rel: 1e-11, h_min: 1e-6, h_max: 86_400.0 }
    }
}

pub struct Dopri5 {
    pub tol: Tolerance,
}

/// The integration cannot continue: the error estimate or the new state is
/// not finite (NaN or infinite state, or a singular force). Returned instead of
/// retrying forever; the state is left at the last accepted step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NonFinite;

// Butcher tableau (Dormand & Prince 1980).
const C: [f64; 7] = [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0];
const A: [[f64; 6]; 7] = [
    [0.0; 6],
    [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0],
    [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0],
    [19372.0 / 6561.0, -25360.0 / 2187.0, 64448.0 / 6561.0, -212.0 / 729.0, 0.0, 0.0],
    [9017.0 / 3168.0, -355.0 / 33.0, 46732.0 / 5247.0, 49.0 / 176.0, -5103.0 / 18656.0, 0.0],
    [35.0 / 384.0, 0.0, 500.0 / 1113.0, 125.0 / 192.0, -2187.0 / 6784.0, 11.0 / 84.0],
];
/// 5th-order minus 4th-order weights (error estimate).
const E: [f64; 7] =
    [71.0 / 57600.0, 0.0, -71.0 / 16695.0, 71.0 / 1920.0, -17253.0 / 339200.0, 22.0 / 525.0, -1.0 / 40.0];

impl Dopri5 {
    pub fn new(tol: Tolerance) -> Self {
        Self { tol }
    }

    /// Initial state at local time `t` with a first step guess `h`.
    /// The extra scalar starts at zero.
    pub fn start<D: Dynamics>(&self, dyn_: &D, t: f64, r: DVec3, v: DVec3, h: f64) -> StepState {
        let (a, x_rate) = dyn_.accel_rate(t, r, v);
        StepState {
            t,
            r: Compensated::new(r),
            v: Compensated::new(v),
            a,
            h: h.clamp(self.tol.h_min, self.tol.h_max),
            x: CompensatedScalar::default(),
            x_rate,
        }
    }

    /// Takes one accepted step (retrying internally on rejection), never going
    /// past `t_limit`. Returns the new endpoint sample, or [`NonFinite`] (with
    /// `st` unchanged) when the step cannot be computed.
    pub fn step<D: Dynamics>(&self, dyn_: &D, st: &mut StepState, t_limit: f64) -> Result<StepSample, NonFinite> {
        loop {
            let remaining = t_limit - st.t;
            let clamped = st.h >= remaining;
            let h = if clamped { remaining } else { st.h };
            let (dr, dv, a1, err, (dx, x_rate)) = self.attempt(dyn_, st, h);
            // A NaN error compares false everywhere: without this the step
            // size grows and the step is retried forever.
            if !err.is_finite() || !(dr.is_finite() && dv.is_finite() && a1.is_finite()) {
                return Err(NonFinite);
            }
            let factor = (0.9 * libm::pow(err.max(1e-10), -0.2)).clamp(0.2, 5.0);
            if err <= 1.0 || h <= self.tol.h_min {
                st.r.add(dr);
                st.v.add(dv);
                st.x.add(dx);
                st.x_rate = x_rate;
                st.t = if clamped { t_limit } else { st.t + h };
                st.a = a1;
                // Keep the step size free of the artificial clamp so resuming
                // after a chunk boundary matches an uninterrupted run.
                if !clamped {
                    st.h = (h * factor).clamp(self.tol.h_min, self.tol.h_max);
                }
                return Ok(StepSample { t: st.t, r: st.r.value(), v: st.v.value(), a: st.a });
            }
            st.h = (h * factor).clamp(self.tol.h_min, self.tol.h_max);
        }
    }

    /// One trial step of size `h`. Returns (Δr, Δv, a at the end, scaled
    /// error, (Δx, ẋ at the end)).
    fn attempt<D: Dynamics>(&self, dyn_: &D, st: &StepState, h: f64) -> (DVec3, DVec3, DVec3, f64, (f64, f64)) {
        let r0 = st.r.value();
        let v0 = st.v.value();
        let mut kr = [DVec3::ZERO; 7];
        let mut kv = [DVec3::ZERO; 7];
        let mut kx = [0.0; 7];
        kr[0] = v0;
        kv[0] = st.a;
        kx[0] = st.x_rate;
        for s in 1..7 {
            let mut dr = DVec3::ZERO;
            let mut dv = DVec3::ZERO;
            for j in 0..s {
                dr += kr[j] * A[s][j];
                dv += kv[j] * A[s][j];
            }
            let rs = r0 + dr * h;
            let vs = v0 + dv * h;
            kr[s] = vs;
            (kv[s], kx[s]) = dyn_.accel_rate(st.t + C[s] * h, rs, vs);
        }
        // The 7th stage is evaluated at the 5th-order solution (FSAL).
        let mut dr = DVec3::ZERO;
        let mut dv = DVec3::ZERO;
        let mut er = DVec3::ZERO;
        let mut ev = DVec3::ZERO;
        let mut dx = 0.0;
        for j in 0..7 {
            if j < 6 {
                dr += kr[j] * A[6][j];
                dv += kv[j] * A[6][j];
                dx += kx[j] * A[6][j];
            }
            er += kr[j] * E[j];
            ev += kv[j] * E[j];
        }
        let (dr, dv, er, ev) = (dr * h, dv * h, er * h, ev * h);
        let r1 = r0 + dr;
        let v1 = v0 + dv;
        let sr = self.tol.abs_r + self.tol.rel * r0.abs().max(r1.abs());
        let sv = self.tol.abs_v + self.tol.rel * v0.abs().max(v1.abs());
        let q = (er / sr).length_squared() + (ev / sv).length_squared();
        let err = (q / 6.0).sqrt();
        (dr, dv, kv[6], err, (dx * h, kx[6]))
    }
}

/// Quintic Hermite interpolation between two samples: returns `(r, v)` at `t`.
pub fn hermite5(s0: &StepSample, s1: &StepSample, t: f64) -> (DVec3, DVec3) {
    let h = s1.t - s0.t;
    if h <= 0.0 {
        return (s0.r, s0.v);
    }
    let s = (t - s0.t) / h;
    let (s2, s3) = (s * s, s * s * s);
    let (s4, s5) = (s3 * s, s3 * s2);
    let h0 = 1.0 - 10.0 * s3 + 15.0 * s4 - 6.0 * s5;
    let h1 = s - 6.0 * s3 + 8.0 * s4 - 3.0 * s5;
    let h2 = 0.5 * s2 - 1.5 * s3 + 1.5 * s4 - 0.5 * s5;
    let h3 = 10.0 * s3 - 15.0 * s4 + 6.0 * s5;
    let h4 = -4.0 * s3 + 7.0 * s4 - 3.0 * s5;
    let h5 = 0.5 * s3 - s4 + 0.5 * s5;
    let r = s0.r * h0 + s0.v * (h * h1) + s0.a * (h * h * h2) + s1.r * h3 + s1.v * (h * h4) + s1.a * (h * h * h5);
    let d0 = -30.0 * s2 + 60.0 * s3 - 30.0 * s4;
    let d1 = 1.0 - 18.0 * s2 + 32.0 * s3 - 15.0 * s4;
    let d2 = s - 4.5 * s2 + 6.0 * s3 - 2.5 * s4;
    let d3 = 30.0 * s2 - 60.0 * s3 + 30.0 * s4;
    let d4 = -12.0 * s2 + 28.0 * s3 - 15.0 * s4;
    let d5 = 1.5 * s2 - 4.0 * s3 + 2.5 * s4;
    let v = (s0.r * d0 + s1.r * d3) / h + s0.v * d1 + s1.v * d4 + (s0.a * d2 + s1.a * d5) * h;
    (r, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kepler::Elements;

    const MU: f64 = 3.986_004_418e14;

    struct TwoBody {
        r: DVec3,
        v: DVec3,
    }
    impl Separable for TwoBody {
        fn drift(&mut self, h: f64) {
            self.r += self.v * h;
        }
        fn kick(&mut self, h: f64) {
            let d = self.r.length();
            self.v += self.r * (-MU / (d * d * d)) * h;
        }
    }

    fn orbit() -> Elements {
        Elements { a: 1.0e7, e: 0.5, i: 0.3, raan: 0.2, argp: 0.1, mean_anomaly: 0.0 }
    }

    fn yoshida_error(steps: usize) -> f64 {
        let el = orbit();
        let (r, v) = el.to_state(MU);
        let period = el.period(MU);
        let mut sys = TwoBody { r, v };
        let h = period / steps as f64;
        for _ in 0..steps {
            yoshida8_step(&mut sys, h);
        }
        (sys.r - r).length()
    }

    #[test]
    fn yoshida8_is_eighth_order() {
        let e1 = yoshida_error(400);
        let e2 = yoshida_error(800);
        let order = (e1 / e2).log2();
        assert!(order > 7.0 && order < 9.5, "observed order {order} ({e1:e} -> {e2:e})");
    }

    fn kepler_dyn() -> impl Fn(f64, DVec3, DVec3) -> DVec3 {
        |_, r: DVec3, _| {
            let d = r.length();
            r * (-MU / (d * d * d))
        }
    }

    #[test]
    fn dopri5_tracks_kepler_orbit() {
        let el = orbit();
        let (r0, v0) = el.to_state(MU);
        let period = el.period(MU);
        let integ = Dopri5::new(Tolerance::default());
        let f = kepler_dyn();
        let mut st = integ.start(&f, 0.0, r0, v0, 10.0);
        let mut steps = 0;
        while st.t < period {
            integ.step(&f, &mut st, period).unwrap();
            steps += 1;
        }
        let err = (st.r.value() - r0).length();
        assert!(err < 5.0, "position error after one orbit: {err} m ({steps} steps)");
    }

    #[test]
    fn chunked_equals_single_pass_bit_for_bit() {
        let el = orbit();
        let (r0, v0) = el.to_state(MU);
        let end = 3.0 * el.period(MU);
        let integ = Dopri5::new(Tolerance::default());
        let f = kepler_dyn();

        let mut single = integ.start(&f, 0.0, r0, v0, 10.0);
        let mut single_samples = Vec::new();
        while single.t < end {
            single_samples.push(integ.step(&f, &mut single, end).unwrap());
        }

        // Resume from stored state after an arbitrary number of steps.
        let mut chunked = integ.start(&f, 0.0, r0, v0, 10.0);
        let mut chunked_samples = Vec::new();
        for chunk in [7usize, 1, 50, 3] {
            let mut resumed = chunked; // a copy, as if loaded from storage
            for _ in 0..chunk {
                chunked_samples.push(integ.step(&f, &mut resumed, end).unwrap());
            }
            chunked = resumed;
        }
        while chunked.t < end {
            chunked_samples.push(integ.step(&f, &mut chunked, end).unwrap());
        }
        assert_eq!(single_samples, chunked_samples);
        assert_eq!(single, chunked);
    }

    #[test]
    fn non_finite_state_is_an_error_not_a_hang() {
        let integ = Dopri5::new(Tolerance::default());
        let f = kepler_dyn();
        let mut st = integ.start(&f, 0.0, DVec3::new(f64::NAN, 7e6, 0.0), DVec3::new(0.0, 0.0, 7e3), 10.0);
        let before = st;
        assert_eq!(integ.step(&f, &mut st, 1e4), Err(NonFinite));
        assert_eq!(st.t, before.t);
        // A singular force (at the centre) fails the same way.
        let mut st = integ.start(&f, 0.0, DVec3::ZERO, DVec3::ZERO, 10.0);
        assert_eq!(integ.step(&f, &mut st, 1e4), Err(NonFinite));
    }

    #[test]
    fn hermite5_reproduces_quintic_exactly() {
        let p = |t: f64| DVec3::new(t.powi(5) - 2.0 * t.powi(3) + t, 3.0 * t.powi(4), 1.0 - t.powi(2));
        let dp = |t: f64| DVec3::new(5.0 * t.powi(4) - 6.0 * t.powi(2) + 1.0, 12.0 * t.powi(3), -2.0 * t);
        let ddp = |t: f64| DVec3::new(20.0 * t.powi(3) - 12.0 * t, 36.0 * t.powi(2), DVec3::splat(-2.0).z);
        let s = |t: f64| StepSample { t, r: p(t), v: dp(t), a: ddp(t) };
        let (s0, s1) = (s(0.5), s(2.0));
        for k in 0..=10 {
            let t = 0.5 + 1.5 * k as f64 / 10.0;
            let (r, v) = hermite5(&s0, &s1, t);
            assert!((r - p(t)).length() < 1e-12, "r at {t}");
            assert!((v - dp(t)).length() < 1e-11, "v at {t}");
        }
    }
}
