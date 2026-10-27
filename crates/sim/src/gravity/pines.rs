//! The evaluation of a [`Field`]: Pines' formulation, normalized.
//!
//! With direction cosines `(s, t, u)` of the point, `ρ = R/r`,
//! `r_m + i i_m = (s + i t)^m` and `Ānm(u)` the normalized `m`-th
//! derivative of the Legendre polynomial `P_n` (a polynomial: the
//! `cos^m φ` of `P̄nm` lives in `r_m`, `i_m`), the potential is
//! `V = (μ/r) Σ ρⁿ Ānm (C̄nm r_m + S̄nm i_m)` and its gradient
//! `a1 x̂ + a2 ŷ + a3 ẑ + a4 r̂` with
//!
//! ```text
//! a1 = (μ/r²) Σ ρⁿ m Ānm (C̄ r_m−1 + S̄ i_m−1)
//! a2 = (μ/r²) Σ ρⁿ m Ānm (S̄ r_m−1 − C̄ i_m−1)
//! a3 = (μ/r²) Σ ρⁿ (Nnm/Nn,m+1) Ān,m+1 (C̄ r_m + S̄ i_m)
//! a4 = −(μ/r²) Σ ρⁿ (Nnm/Nn+1,m+1) Ān+1,m+1 (C̄ r_m + S̄ i_m)
//! ```
//!
//! (from `∂/∂u Ānm ∝ Ān,m+1` and `(n+m+1) Anm + u An,m+1 = An+1,m+1`).
//! The columns `P̄'nm = ρⁿ Ānm` follow the stable forward recursion in `n`
//! from the diagonal; each order's degree sum is formed first (lumped), then
//! multiplied by its `r_m`, `i_m`.

use super::Field;

/// `[a1, a2, a3, a4, V]` (m/s², m²/s²) for a point at distance `r` in
/// direction `dir` (unit, in the coefficients' frame), to degree `n_max`
/// and order `m_max`; `V` only with `potential`. The acceleration is
/// `a1 x̂ + a2 ŷ + a3 ẑ + a4 r̂`.
pub(super) fn sums(
    field: &Field,
    gm: f64,
    r: f64,
    dir: [f64; 3],
    n_max: usize,
    m_max: usize,
    potential: bool,
) -> [f64; 5] {
    // Buffers on the stack, sized to the degree (a J2 field touches 16).
    match n_max {
        0..=13 => sums_in::<16>(field, gm, r, dir, n_max, m_max, potential),
        14..=61 => sums_in::<64>(field, gm, r, dir, n_max, m_max, potential),
        62..=253 => sums_in::<256>(field, gm, r, dir, n_max, m_max, potential),
        _ => sums_in::<{ super::MAX_DEGREE + 2 }>(field, gm, r, dir, n_max, m_max, potential),
    }
}

fn sums_in<const B: usize>(
    field: &Field,
    gm: f64,
    r: f64,
    [s, t, u]: [f64; 3],
    n_max: usize,
    m_max: usize,
    potential: bool,
) -> [f64; 5] {
    debug_assert!(n_max + 2 <= B && m_max <= n_max);
    let rho = field.radius / r;
    let (rho_u, rho2) = (rho * u, rho * rho);
    // Diagonal P̄'mm and (s + i t)^m.
    let mut diag = [0.0; B];
    diag[0] = 1.0;
    for m in 1..=m_max + 1 {
        diag[m] = rho * field.diag[m] * diag[m - 1];
    }
    let (mut re, mut im) = ([0.0; B], [0.0; B]);
    re[0] = 1.0;
    for m in 1..=m_max {
        re[m] = s * re[m - 1] - t * im[m - 1];
        im[m] = s * im[m - 1] + t * re[m - 1];
    }
    // Orders from the highest down: column m and the one above it.
    let (mut buf_a, mut buf_b) = ([0.0; B], [0.0; B]);
    let (mut upper, mut col) = (&mut buf_a, &mut buf_b);
    column(field, m_max + 1, diag[m_max + 1], rho_u, rho2, n_max + 1, upper);
    let (mut a1, mut a2, mut a3, mut a4, mut pot) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for m in (0..=m_max).rev() {
        column(field, m, diag[m], rho_u, rho2, n_max + 1, col);
        upper[m] = 0.0; // Ām,m+1 = 0
        let terms = &field.terms[field.columns[m]..];
        let (mut sc, mut ss, mut tc, mut ts, mut qc, mut qs) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for n in (m.max(2)..=n_max).rev() {
            let k = &terms[n - m];
            let (p, pu, pq) = (col[n], upper[n], upper[n + 1]);
            sc += p * k.c;
            ss += p * k.s;
            tc += pu * k.fc;
            ts += pu * k.fs;
            qc += pq * k.gc;
            qs += pq * k.gs;
        }
        if potential {
            pot += re[m] * sc + im[m] * ss;
        }
        if m > 0 {
            let mf = m as f64;
            a1 += mf * (re[m - 1] * sc + im[m - 1] * ss);
            a2 += mf * (re[m - 1] * ss - im[m - 1] * sc);
        }
        a3 += re[m] * tc + im[m] * ts;
        a4 += re[m] * qc + im[m] * qs;
        std::mem::swap(&mut upper, &mut col);
    }
    let g = gm / (r * r);
    [a1 * g, a2 * g, a3 * g, -(a4 * g) / rho, pot * gm / r]
}

/// Column `m` of `P̄'nm = ρⁿ Ānm`, `n = m..=n_hi`, into `buf[n]`.
fn column(field: &Field, m: usize, diag: f64, rho_u: f64, rho2: f64, n_hi: usize, buf: &mut [f64]) {
    let terms = &field.terms[field.columns[m]..];
    buf[m] = diag;
    if n_hi > m {
        buf[m + 1] = rho_u * terms[1].alpha * diag;
    }
    for n in m + 2..=n_hi {
        let k = &terms[n - m];
        buf[n] = rho_u * k.alpha * buf[n - 1] - rho2 * k.beta * buf[n - 2];
    }
}
