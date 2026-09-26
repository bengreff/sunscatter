//! Chebyshev segment tables.
//!
//! A table covers `[start, start + n_segments * seg_len)` with equal segments of
//! an integer number of seconds (so segment lookup is exact integer arithmetic
//! on the split epoch). Within a segment the position is a Chebyshev series in
//! `tau ∈ [-1, 1]`; velocity and acceleration are its analytic derivatives, so
//! the kinematics are exactly consistent with the position function.
//!
//! Coefficients are built by interpolation at Chebyshev–Gauss–Lobatto nodes,
//! which include both segment endpoints: adjacent segments agree at their shared
//! boundary to within the interpolation of the same true value.

use super::Kinematics;
use crate::math;
use crate::time::Epoch;
use glam::DVec3;

#[derive(Clone, Debug, PartialEq)]
pub struct ChebTable {
    /// Start of the first segment. Must have a zero fractional second.
    pub start: Epoch,
    /// Segment length in whole seconds.
    pub seg_len: i64,
    /// Polynomial degree (coefficients per axis = degree + 1).
    pub degree: usize,
    /// Layout: `[segment][axis x,y,z][coefficient]`.
    pub coeffs: Vec<f64>,
}

impl ChebTable {
    pub fn n_segments(&self) -> usize {
        self.coeffs.len() / (3 * (self.degree + 1))
    }

    /// Segment index and `tau` for time `t` (clamped to the table: outside the
    /// window the first/last segment is extrapolated). Vessels never ask past
    /// the end ([`crate::world::World::end`]); unit tests count such calls.
    fn locate(&self, t: Epoch) -> (usize, f64) {
        debug_assert_eq!(self.start.fractional_second(), 0.0);
        let offset = t.whole_seconds() - self.start.whole_seconds();
        #[cfg(test)]
        {
            let len = self.n_segments() as i64 * self.seg_len;
            if offset > len || (offset == len && t.fractional_second() > 0.0) {
                PAST_END_CALLS.with(|c| c.set(c.get() + 1));
            }
        }
        let k = offset.div_euclid(self.seg_len).clamp(0, self.n_segments() as i64 - 1);
        let seg_start = self.start.whole_seconds() + k * self.seg_len;
        let local = (t.whole_seconds() - seg_start) as f64 + t.fractional_second();
        (k as usize, 2.0 * local / self.seg_len as f64 - 1.0)
    }

    /// Position only. Uses exactly the arithmetic of [`ChebTable::eval`]'s
    /// position part, so the two agree bit for bit.
    pub fn eval_r(&self, t: Epoch) -> DVec3 {
        let (k, tau) = self.locate(t);
        let n = self.degree + 1;
        let mut tn = [0.0; MAX_DEGREE + 1];
        tn[0] = 1.0;
        if n > 1 {
            tn[1] = tau;
        }
        for j in 2..n {
            tn[j] = 2.0 * tau * tn[j - 1] - tn[j - 2];
        }
        let mut out = [0.0f64; 3];
        for axis in 0..3 {
            let c = &self.coeffs[(k * 3 + axis) * n..(k * 3 + axis + 1) * n];
            let mut p = 0.0;
            for j in 0..n {
                p += c[j] * tn[j];
            }
            out[axis] = p;
        }
        DVec3::from_array(out)
    }

    pub fn eval(&self, t: Epoch) -> Kinematics {
        let (k, tau) = self.locate(t);
        let n = self.degree + 1;
        let (tn, dn, ddn) = cheb_basis(tau, self.degree);
        let scale = 2.0 / self.seg_len as f64;
        let mut out = [[0.0f64; 3]; 3];
        for axis in 0..3 {
            let c = &self.coeffs[(k * 3 + axis) * n..(k * 3 + axis + 1) * n];
            let (mut p, mut d, mut dd) = (0.0, 0.0, 0.0);
            for j in 0..n {
                p += c[j] * tn[j];
                d += c[j] * dn[j];
                dd += c[j] * ddn[j];
            }
            out[0][axis] = p;
            out[1][axis] = d * scale;
            out[2][axis] = dd * scale * scale;
        }
        Kinematics { r: DVec3::from_array(out[0]), v: DVec3::from_array(out[1]), a: DVec3::from_array(out[2]) }
    }
}

#[cfg(test)]
thread_local! {
    /// Evaluations past a table's end on this thread (test instrumentation).
    pub(crate) static PAST_END_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Highest supported polynomial degree (fixed-size, allocation-free evaluation).
pub const MAX_DEGREE: usize = 31;
type Basis = [f64; MAX_DEGREE + 1];

/// Chebyshev polynomials `T_j(tau)` and their first and second derivatives with
/// respect to `tau`, for `j = 0..=degree`.
pub fn cheb_basis(tau: f64, degree: usize) -> (Basis, Basis, Basis) {
    assert!(degree <= MAX_DEGREE, "Chebyshev degree {degree} > {MAX_DEGREE}");
    let n = degree + 1;
    let mut t = [0.0; MAX_DEGREE + 1];
    let mut d = [0.0; MAX_DEGREE + 1];
    let mut dd = [0.0; MAX_DEGREE + 1];
    t[0] = 1.0;
    if n > 1 {
        t[1] = tau;
        d[1] = 1.0;
    }
    for j in 2..n {
        t[j] = 2.0 * tau * t[j - 1] - t[j - 2];
        d[j] = 2.0 * t[j - 1] + 2.0 * tau * d[j - 1] - d[j - 2];
        dd[j] = 4.0 * d[j - 1] + 2.0 * tau * dd[j - 1] - dd[j - 2];
    }
    (t, d, dd)
}

/// The Chebyshev–Gauss–Lobatto nodes `cos(pi k / degree)`, k = 0..=degree
/// (from +1 down to -1).
pub fn lobatto_nodes(degree: usize) -> Vec<f64> {
    (0..=degree).map(|k| math::cos(math::PI * k as f64 / degree as f64)).collect()
}

/// Coefficients of the degree-`n` polynomial interpolating `values[k]` at
/// `lobatto_nodes(n)[k]`.
pub fn interpolate_lobatto(values: &[f64]) -> Vec<f64> {
    let n = values.len() - 1;
    let mut c = vec![0.0; n + 1];
    for (j, cj) in c.iter_mut().enumerate() {
        let mut sum = 0.0;
        for (k, v) in values.iter().enumerate() {
            let w = if k == 0 || k == n { 0.5 } else { 1.0 };
            sum += w * v * math::cos(math::PI * (j * k) as f64 / n as f64);
        }
        *cj = 2.0 * sum / n as f64;
    }
    c[0] *= 0.5;
    c[n] *= 0.5;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a table for f(t) = (A sin wt, A cos wt, B t) over a few segments.
    fn table(degree: usize) -> (ChebTable, impl Fn(f64) -> (DVec3, DVec3, DVec3)) {
        let (amp, w, b) = (3.8e8, 2.66e-6, 10.0);
        let f = move |t: f64| {
            let (s, c) = (math::sin(w * t), math::cos(w * t));
            (
                DVec3::new(amp * s, amp * c, b * t),
                DVec3::new(amp * w * c, -amp * w * s, b),
                DVec3::new(-amp * w * w * s, -amp * w * w * c, 0.0),
            )
        };
        let seg_len = 4 * 86_400;
        let nodes = lobatto_nodes(degree);
        let mut coeffs = Vec::new();
        for k in 0..10 {
            let t0 = (k * seg_len) as f64;
            for axis in 0..3 {
                let vals: Vec<f64> =
                    nodes.iter().map(|x| f(t0 + (x + 1.0) * 0.5 * seg_len as f64).0.to_array()[axis]).collect();
                coeffs.extend(interpolate_lobatto(&vals));
            }
        }
        (ChebTable { start: Epoch::J2000, seg_len, degree, coeffs }, f)
    }

    #[test]
    fn table_reproduces_position_velocity_acceleration() {
        let (tab, f) = table(16);
        for i in 0..400 {
            let t = i as f64 * 8_000.0 + 123.456;
            let k = tab.eval(Epoch::J2000.add_seconds(t));
            let (r, v, a) = f(t);
            assert!((k.r - r).length() < 1e-3, "r err {} at {t}", (k.r - r).length());
            assert!((k.v - v).length() < 1e-8, "v err at {t}");
            assert!((k.a - a).length() < 1e-12, "a err at {t}");
            assert_eq!(k.r, tab.eval_r(Epoch::J2000.add_seconds(t)), "position paths must agree bitwise");
        }
    }

    #[test]
    fn segments_meet_at_boundaries() {
        let (tab, _) = table(12);
        let boundary = Epoch::J2000.add_seconds((4 * 86_400) as f64);
        let before = tab.eval(boundary.add_seconds(-1e-6)).r;
        let after = tab.eval(boundary).r;
        assert!((before - after).length() < 1.0, "jump {}", (before - after).length());
    }
}
