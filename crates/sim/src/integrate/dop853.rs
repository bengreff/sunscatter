//! Dormand–Prince 8(5,3) (Hairer, Nørsett & Wanner, *Solving ODEs I*, the
//! `DOP853` code): 12 stages per step plus the first-same-as-last, an
//! 8th-order solution with the 5th- and 3rd-order embedded error estimate,
//! and the same plain-value state as [`super::Dopri5`] ([`StepState`]), so
//! integrating in chunks is bit-identical to integrating in one go.
//!
//! For the smooth, tight-tolerance problems of coasting vessels it takes
//! far larger steps than the 5th-order method at the same error.

// The coefficients as published (they round to the nearest f64).
#![allow(clippy::excessive_precision)]

use super::{Dynamics, NonFinite, StepSample, StepState, Tolerance};
use crate::math::{Compensated, CompensatedScalar};
use glam::DVec3;

/// Stage times.
const C: [f64; 12] = [
    0.0,
    0.526001519587677318785587544488e-1,
    0.789002279381515978178381316732e-1,
    0.118350341907227396726757197510,
    0.281649658092772603273242802490,
    0.333333333333333333333333333333,
    0.25,
    0.307692307692307692307692307692,
    0.651282051282051282051282051282,
    0.6,
    0.857142857142857142857142857142,
    1.0,
];

/// Stage coefficients, row `s` for stage `s` (zero-based; zeros where the
/// method has none).
const A: [[f64; 11]; 12] = [
    [0.0; 11],
    [5.26001519587677318785587544488e-2, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [
        1.97250569845378994544595329183e-2,
        5.91751709536136983633785987549e-2,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ],
    [
        2.95875854768068491816892993775e-2,
        0.0,
        8.87627564304205475450678981324e-2,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ],
    [
        2.41365134159266685502369798665e-1,
        0.0,
        -8.84549479328286085344864962717e-1,
        9.24834003261792003115737966543e-1,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ],
    [
        3.7037037037037037037037037037e-2,
        0.0,
        0.0,
        1.70828608729473871279604482173e-1,
        1.25467687566822425016691814123e-1,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ],
    [
        3.7109375e-2,
        0.0,
        0.0,
        1.70252211019544039314978060272e-1,
        6.02165389804559606850219397283e-2,
        -1.7578125e-2,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ],
    [
        3.70920001185047927108779319836e-2,
        0.0,
        0.0,
        1.70383925712239993810214054705e-1,
        1.07262030446373284651809199168e-1,
        -1.53194377486244017527936158236e-2,
        8.27378916381402288758473766002e-3,
        0.0,
        0.0,
        0.0,
        0.0,
    ],
    [
        6.24110958716075717114429577812e-1,
        0.0,
        0.0,
        -3.36089262944694129406857109825,
        -8.68219346841726006818189891453e-1,
        2.75920996994467083049415600797e1,
        2.01540675504778934086186788979e1,
        -4.34898841810699588477366255144e1,
        0.0,
        0.0,
        0.0,
    ],
    [
        4.77662536438264365890433908527e-1,
        0.0,
        0.0,
        -2.48811461997166764192642586468,
        -5.90290826836842996371446475743e-1,
        2.12300514481811942347288949897e1,
        1.52792336328824235832596922938e1,
        -3.32882109689848629194453265587e1,
        -2.03312017085086261358222928593e-2,
        0.0,
        0.0,
    ],
    [
        -9.3714243008598732571704021658e-1,
        0.0,
        0.0,
        5.18637242884406370830023853209,
        1.09143734899672957818500254654,
        -8.14978701074692612513997267357,
        -1.85200656599969598641566180701e1,
        2.27394870993505042818970056734e1,
        2.49360555267965238987089396762,
        -3.0467644718982195003823669022,
        0.0,
    ],
    [
        2.27331014751653820792359768449,
        0.0,
        0.0,
        -1.05344954667372501984066689879e1,
        -2.00087205822486249909675718444,
        -1.79589318631187989172765950534e1,
        2.79488845294199600508499808837e1,
        -2.85899827713502369474065508674,
        -8.87285693353062954433549289258,
        1.23605671757943030647266201528e1,
        6.43392746015763530355970484046e-1,
    ],
];

/// 8th-order weights.
const B: [f64; 12] = [
    5.42937341165687622380535766363e-2,
    0.0,
    0.0,
    0.0,
    0.0,
    4.45031289275240888144113950566,
    1.89151789931450038304281599044,
    -5.8012039600105847814672114227,
    3.1116436695781989440891606237e-1,
    -1.52160949662516078556178806805e-1,
    2.01365400804030348374776537501e-1,
    4.47106157277725905176885569043e-2,
];

/// 8th- minus 5th-order weights (the main error estimate).
const E5: [f64; 12] = [
    0.1312004499419488073250102996e-1,
    0.0,
    0.0,
    0.0,
    0.0,
    -0.1225156446376204440720569753e1,
    -0.4957589496572501915214079952,
    0.1664377182454986536961530415e1,
    -0.3503288487499736816886487290,
    0.3341791187130174790297318841,
    0.8192320648511571246570742613e-1,
    -0.2235530786388629525884427845e-1,
];

/// The 3rd-order weights (on stages 1, 9 and 12) of the second estimate.
const BHH: [f64; 3] =
    [0.244094488188976377952755905512, 0.733846688281611857341361741547, 0.220588235294117647058823529412e-1];

pub struct Dop853 {
    pub tol: Tolerance,
}

impl Dop853 {
    pub fn new(tol: Tolerance) -> Self {
        Self { tol }
    }

    /// Initial state at local time `t` with a first step guess `h`.
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

    /// One accepted step, as [`super::Dopri5::step`].
    pub fn step<D: Dynamics>(&self, dyn_: &D, st: &mut StepState, t_limit: f64) -> Result<StepSample, NonFinite> {
        loop {
            let remaining = t_limit - st.t;
            let clamped = st.h >= remaining;
            let h = if clamped { remaining } else { st.h };
            let (dr, dv, a1, err, (dx, x_rate)) = self.attempt(dyn_, st, h);
            if !err.is_finite() || !(dr.is_finite() && dv.is_finite() && a1.is_finite()) {
                return Err(NonFinite);
            }
            let factor = (0.9 * libm::pow(err.max(1e-16), -0.125)).clamp(0.333, 6.0);
            if err <= 1.0 || h <= self.tol.h_min {
                st.r.add(dr);
                st.v.add(dv);
                st.x.add(dx);
                st.x_rate = x_rate;
                st.t = if clamped { t_limit } else { st.t + h };
                st.a = a1;
                if !clamped {
                    st.h = (h * factor).clamp(self.tol.h_min, self.tol.h_max);
                }
                return Ok(StepSample { t: st.t, r: st.r.value(), v: st.v.value(), a: st.a });
            }
            st.h = (h * factor.min(1.0)).clamp(self.tol.h_min, self.tol.h_max);
        }
    }

    /// One trial step of size `h`: (Δr, Δv, a at the end, scaled error,
    /// (Δx, ẋ at the end)).
    fn attempt<D: Dynamics>(&self, dyn_: &D, st: &StepState, h: f64) -> (DVec3, DVec3, DVec3, f64, (f64, f64)) {
        let r0 = st.r.value();
        let v0 = st.v.value();
        let mut kr = [DVec3::ZERO; 12];
        let mut kv = [DVec3::ZERO; 12];
        let mut kx = [0.0; 12];
        kr[0] = v0;
        kv[0] = st.a;
        kx[0] = st.x_rate;
        for s in 1..12 {
            let mut dr = DVec3::ZERO;
            let mut dv = DVec3::ZERO;
            for j in 0..s {
                if A[s][j] != 0.0 {
                    dr += kr[j] * A[s][j];
                    dv += kv[j] * A[s][j];
                }
            }
            let rs = r0 + dr * h;
            let vs = v0 + dv * h;
            kr[s] = vs;
            (kv[s], kx[s]) = dyn_.accel_rate(st.t + C[s] * h, rs, vs);
        }
        let (mut dr, mut dv, mut dx) = (DVec3::ZERO, DVec3::ZERO, 0.0);
        let (mut e5r, mut e5v) = (DVec3::ZERO, DVec3::ZERO);
        for j in 0..12 {
            if B[j] != 0.0 {
                dr += kr[j] * B[j];
                dv += kv[j] * B[j];
                dx += kx[j] * B[j];
            }
            if E5[j] != 0.0 {
                e5r += kr[j] * E5[j];
                e5v += kv[j] * E5[j];
            }
        }
        let e3r = dr - kr[0] * BHH[0] - kr[8] * BHH[1] - kr[11] * BHH[2];
        let e3v = dv - kv[0] * BHH[0] - kv[8] * BHH[1] - kv[11] * BHH[2];
        let (dr, dv) = (dr * h, dv * h);
        let r1 = r0 + dr;
        let v1 = v0 + dv;
        let (a1, x_rate) = dyn_.accel_rate(st.t + h, r1, v1);
        let sr = self.tol.abs_r + self.tol.rel * r0.abs().max(r1.abs());
        let sv = self.tol.abs_v + self.tol.rel * v0.abs().max(v1.abs());
        let q5 = (e5r / sr).length_squared() + (e5v / sv).length_squared();
        let q3 = (e3r / sr).length_squared() + (e3v / sv).length_squared();
        let den = q5 + 0.01 * q3;
        // A NaN (a non-finite state's scale) must stay NaN: the caller
        // stops on it.
        let err = if den == 0.0 { 0.0 } else { h.abs() * q5 * (1.0 / (6.0 * den)).sqrt() };
        (dr, dv, a1, err, (dx * h, x_rate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrate::Dopri5;
    use crate::kepler::Elements;

    const MU: f64 = 3.986_004_418e14;

    fn orbit() -> Elements {
        Elements { a: 7_000_000.0, e: 0.1, i: 0.5, raan: 0.3, argp: 1.0, mean_anomaly: 0.0 }
    }

    fn kepler_dyn() -> impl Fn(f64, DVec3, DVec3) -> DVec3 {
        |_, r: DVec3, _| r * (-MU / (r.length() * r.length_squared()))
    }

    /// Position error after one orbit in fixed steps of `period / n`.
    fn fixed_step_error(n: usize) -> f64 {
        let el = orbit();
        let (r0, v0) = el.to_state(MU);
        let period = el.period(MU);
        let h = period / n as f64;
        let tol = Tolerance { abs_r: 1e30, abs_v: 1e30, rel: 0.0, h_min: h, h_max: h };
        let integ = Dop853::new(tol);
        let f = kepler_dyn();
        let mut st = integ.start(&f, 0.0, r0, v0, h);
        while st.t < period {
            integ.step(&f, &mut st, period).unwrap();
        }
        (st.r.value() - r0).length()
    }

    #[test]
    fn dop853_is_eighth_order() {
        let (e1, e2) = (fixed_step_error(30), fixed_step_error(60));
        let order = (e1 / e2).log2();
        assert!((7.5..9.5).contains(&order), "observed order {order} ({e1} m, {e2} m)");
    }

    #[test]
    fn dop853_takes_far_fewer_steps_than_dopri5_at_the_same_tolerance() {
        let el = orbit();
        let (r0, v0) = el.to_state(MU);
        let period = el.period(MU);
        let tol = Tolerance { abs_r: 1e-2, abs_v: 1e-5, rel: 1e-11, h_min: 1e-4, h_max: 1e5 };
        let f = kepler_dyn();
        let (mut n5, mut n8) = (0, 0);
        let five = Dopri5::new(tol);
        let mut st = five.start(&f, 0.0, r0, v0, 10.0);
        while st.t < 10.0 * period {
            five.step(&f, &mut st, 10.0 * period).unwrap();
            n5 += 1;
        }
        let err5 = (st.r.value() - r0).length();
        let eight = Dop853::new(tol);
        let mut st = eight.start(&f, 0.0, r0, v0, 10.0);
        while st.t < 10.0 * period {
            eight.step(&f, &mut st, 10.0 * period).unwrap();
            n8 += 1;
        }
        let err8 = (st.r.value() - r0).length();
        assert!(err8 <= err5.max(0.05), "error after 10 orbits: {err8} m (Dopri5 {err5} m)");
        assert!(n8 * 12 < n5 * 6, "evaluations: {} against Dopri5's {}", n8 * 12, n5 * 6);
    }

    #[test]
    fn non_finite_state_is_an_error_not_a_hang() {
        let integ = Dop853::new(Tolerance::default());
        let f = kepler_dyn();
        let mut st = integ.start(&f, 0.0, DVec3::new(f64::NAN, 7e6, 0.0), DVec3::new(0.0, 0.0, 7e3), 10.0);
        let before = st;
        assert_eq!(integ.step(&f, &mut st, 1e4), Err(NonFinite));
        assert_eq!(st.t, before.t);
        let mut st = integ.start(&f, 0.0, DVec3::ZERO, DVec3::ZERO, 10.0);
        assert_eq!(integ.step(&f, &mut st, 1e4), Err(NonFinite));
        // A force that ignores the position (finite stages, NaN state).
        let free = |_: f64, _: DVec3, _: DVec3| DVec3::ZERO;
        let mut st = integ.start(&free, 0.0, DVec3::new(f64::NAN, 7e6, 0.0), DVec3::new(0.0, 0.0, 7e3), 10.0);
        assert_eq!(integ.step(&free, &mut st, 1e4), Err(NonFinite));
    }

    #[test]
    fn chunked_equals_single_pass_bit_for_bit() {
        let el = orbit();
        let (r0, v0) = el.to_state(MU);
        let end = 3.0 * el.period(MU);
        let integ = Dop853::new(Tolerance::default());
        let f = kepler_dyn();
        let mut single = integ.start(&f, 0.0, r0, v0, 10.0);
        let mut single_samples = Vec::new();
        while single.t < end {
            single_samples.push(integ.step(&f, &mut single, end).unwrap());
        }
        let mut chunked = integ.start(&f, 0.0, r0, v0, 10.0);
        let mut chunked_samples = Vec::new();
        for chunk in [7usize, 1, 5, 3] {
            let mut resumed = chunked;
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
}
