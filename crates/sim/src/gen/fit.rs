//! Rails fitting: osculating elements at each sample, then an independent
//! least-squares line per element (angles unwrapped). The fit is measured by
//! the maximum position error over the samples; the classifier compares that
//! with the node's tolerance.

use crate::ephem::Rails;
use crate::integrate::StepSample;
use crate::kepler::Elements;
use crate::math::{self, TAU};
use crate::time::Epoch;

pub struct RailsFit {
    pub rails: Rails,
    pub max_err: f64,
}

pub fn fit_rails(samples: &[StepSample], epoch: Epoch, mu: f64) -> RailsFit {
    let els: Vec<Elements> = samples.iter().map(|s| Elements::from_state(s.r, s.v, mu)).collect();
    let ts: Vec<f64> = samples.iter().map(|s| s.t).collect();
    let series = |f: fn(&Elements) -> f64, angle: bool| -> (f64, f64) {
        let mut ys: Vec<f64> = els.iter().map(f).collect();
        if angle {
            unwrap(&mut ys);
        }
        line_fit(&ts, &ys)
    };
    let (a, da) = series(|e| e.a, false);
    let (e, de) = series(|e| e.e, false);
    let (i, di) = series(|e| e.i, false);
    let (raan, draan) = series(|e| e.raan, true);
    let (argp, dargp) = series(|e| e.argp, true);
    let (m, dm) = series(|e| e.mean_anomaly, true);
    let rails = Rails {
        epoch,
        mu,
        el: Elements {
            a,
            e,
            i,
            raan: math::wrap_tau(raan),
            argp: math::wrap_tau(argp),
            mean_anomaly: math::wrap_tau(m),
        },
        rates: Elements { a: da, e: de, i: di, raan: draan, argp: dargp, mean_anomaly: dm },
    };
    // NaN (e.g. degenerate elements) must never read as a perfect fit.
    let max_err = samples.iter().map(|s| (rails.eval(epoch.add_seconds(s.t)).r - s.r).length()).fold(0.0, |m, e| {
        if e.is_nan() {
            f64::INFINITY
        } else {
            m.max(e)
        }
    });
    RailsFit { rails, max_err }
}

fn unwrap(ys: &mut [f64]) {
    let mut offset = 0.0;
    for k in 1..ys.len() {
        let raw = ys[k] + offset;
        let d = raw - ys[k - 1];
        if d > math::PI {
            offset -= TAU;
        } else if d < -math::PI {
            offset += TAU;
        }
        ys[k] += offset;
    }
}

/// Least-squares line `y = c0 + c1 t` (returns c0 at t = 0 and the slope).
fn line_fit(ts: &[f64], ys: &[f64]) -> (f64, f64) {
    let n = ts.len() as f64;
    if ts.len() < 2 {
        return (ys.first().copied().unwrap_or(0.0), 0.0);
    }
    let mt = ts.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (t, y) in ts.iter().zip(ys) {
        sxy += (t - mt) * (y - my);
        sxx += (t - mt) * (t - mt);
    }
    let slope = sxy / sxx;
    (my - slope * mt, slope)
}
