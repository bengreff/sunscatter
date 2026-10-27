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
//! a4 = −(μ/r²) Σ ρⁿ (n+m+1) Ānm (C̄ r_m + S̄ i_m) − u a3
//! ```
//!
//! (from `∂/∂u Ānm ∝ Ān,m+1`; the radial term is Pines'
//! `−Σ ρⁿ An+1,m+1 Dnm` through `An+1,m+1 = (n+m+1) Anm + u An,m+1`, so no
//! column runs past the field's degree).
//! The columns `P̄'nm = ρⁿ Ānm` follow the stable forward recursion in `n`
//! from the diagonal, summed as they are computed (degree ascending); each
//! order's degree sum is formed first (lumped), then multiplied by its `r_m`,
//! `i_m`, orders from the highest down: a fixed order (rule 2).

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
    // Orders from the highest down, two at a time (their columns'
    // recursions are independent chains, which the processor overlaps):
    // each column is summed as it is computed, with the column above it.
    let (mut buf_a, mut buf_b) = ([0.0; B], [0.0; B]);
    let (mut upper, mut lower) = (&mut buf_a, &mut buf_b);
    let x = Ctx { field, rho_u, rho2, n_max, diag: &diag };
    x.column(m_max + 1, upper);
    let mut acc = Accumulated::default();
    let mut m = m_max;
    while m >= 1 {
        let (high, low) = x.pair(m, upper, lower);
        acc.add(m, &high, &re, &im, potential);
        acc.add(m - 1, &low, &re, &im, potential);
        std::mem::swap(&mut upper, &mut lower);
        if m == 1 {
            break;
        }
        m -= 2;
    }
    if m_max.is_multiple_of(2) {
        // Order 0 alone (the zonal path, or the last of an odd count).
        acc.add(0, &x.single(0, upper), &re, &im, potential);
    }
    let Accumulated { a1, a2, a3, h, pot } = acc;
    let g = gm / (r * r);
    [a1 * g, a2 * g, a3 * g, -(h + u * a3) * g, pot * gm / r]
}

/// One order's lumped degree sums: `Σ P̄'nm (C̄nm, S̄nm)` (`sc`, `ss`), the
/// same weighted by `n+m+1` (`hc`, `hs`), and `Σ P̄'n,m+1 (fC̄nm, fS̄nm)`
/// (`tc`, `ts`).
#[derive(Default)]
struct Sums {
    sc: f64,
    ss: f64,
    hc: f64,
    hs: f64,
    tc: f64,
    ts: f64,
}

#[derive(Default)]
struct Accumulated {
    a1: f64,
    a2: f64,
    a3: f64,
    /// The radial term's `(n+m+1)` sum.
    h: f64,
    pot: f64,
}

impl Accumulated {
    /// Adds order `m`'s sums, times its `r_m`, `i_m`.
    fn add(&mut self, m: usize, k: &Sums, re: &[f64], im: &[f64], potential: bool) {
        if potential {
            self.pot += re[m] * k.sc + im[m] * k.ss;
        }
        if m > 0 {
            let mf = m as f64;
            self.a1 += mf * (re[m - 1] * k.sc + im[m - 1] * k.ss);
            self.a2 += mf * (re[m - 1] * k.ss - im[m - 1] * k.sc);
        }
        self.a3 += re[m] * k.tc + im[m] * k.ts;
        self.h += re[m] * k.hc + im[m] * k.hs;
    }
}

struct Ctx<'a> {
    field: &'a Field,
    rho_u: f64,
    rho2: f64,
    n_max: usize,
    diag: &'a [f64],
}

impl Ctx<'_> {
    /// The terms of column `m`, degrees `m..=n_max`.
    fn terms(&self, m: usize) -> &[super::Term] {
        let start = self.field.columns[m];
        &self.field.terms[start..start + self.n_max + 1 - m]
    }

    /// Column `m` of `P̄'nm = ρⁿ Ānm`, degrees `m..=n_max`, into `buf[n]`.
    fn column(&self, m: usize, buf: &mut [f64]) {
        let (n_max, rho_u, rho2) = (self.n_max, self.rho_u, self.rho2);
        buf[m] = self.diag[m];
        if m >= n_max {
            return;
        }
        let terms = self.terms(m);
        buf[m + 1] = rho_u * terms[1].alpha * buf[m];
        for n in m + 2..=n_max {
            let k = &terms[n - m];
            buf[n] = rho_u * k.alpha * buf[n - 1] - rho2 * k.beta * buf[n - 2];
        }
    }

    /// Order `m`'s sums from its column (computed here) and `upper`
    /// (column `m + 1`, degrees `m + 1..=n_max`).
    fn single(&self, m: usize, upper: &[f64]) -> Sums {
        let (n_max, rho_u, rho2) = (self.n_max, self.rho_u, self.rho2);
        let terms = self.terms(m);
        // Degree m (the diagonal), then m+1..=n_max by the recursion; terms
        // below degree 2 have zero coefficients and add nothing.
        let (mut p2, mut p1) = (0.0, self.diag[m]);
        let mut w = (2 * m + 1) as f64;
        let k = &terms[0];
        let mut sums = Sums { sc: p1 * k.c, ss: p1 * k.s, hc: w * p1 * k.c, hs: w * p1 * k.s, ..Sums::default() };
        for (k, &u0) in terms[1..].iter().zip(&upper[m + 1..=n_max]) {
            let p = rho_u * k.alpha * p1 - rho2 * k.beta * p2;
            (p2, p1) = (p1, p);
            w += 1.0;
            let q = w * p;
            sums.sc += p * k.c;
            sums.ss += p * k.s;
            sums.hc += q * k.c;
            sums.hs += q * k.s;
            sums.tc += u0 * k.fc;
            sums.ts += u0 * k.fs;
        }
        sums
    }

    /// Orders `m` and `m − 1` (`m ≥ 1`) from `upper` (column `m + 1`):
    /// column `m` is summed as it is computed and feeds order `m − 1`'s
    /// `∂/∂u` term; column `m − 1` goes to `lower` (degrees
    /// `m − 1..=n_max`) for the next pair.
    fn pair(&self, m: usize, upper: &[f64], lower: &mut [f64]) -> (Sums, Sums) {
        let (n_max, rho_u, rho2) = (self.n_max, self.rho_u, self.rho2);
        let (ta, tb) = (self.terms(m), self.terms(m - 1));
        // Column m at degree m; column m − 1 at degrees m − 1 and m.
        let (mut a2, mut a1) = (0.0, self.diag[m]);
        let bd = self.diag[m - 1];
        let bm = rho_u * tb[1].alpha * bd;
        (lower[m - 1], lower[m]) = (bd, bm);
        let (mut b2, mut b1) = (bd, bm);
        let (mut wa, mut wb) = ((2 * m + 1) as f64, (2 * m) as f64);
        let (ka, kd, kb) = (&ta[0], &tb[0], &tb[1]);
        let mut high = Sums { sc: a1 * ka.c, ss: a1 * ka.s, hc: wa * a1 * ka.c, hs: wa * a1 * ka.s, ..Sums::default() };
        let (qd, qb) = ((wb - 1.0) * bd, wb * bm);
        let mut low = Sums {
            sc: bd * kd.c + bm * kb.c,
            ss: bd * kd.s + bm * kb.s,
            hc: qd * kd.c + qb * kb.c,
            hs: qd * kd.s + qb * kb.s,
            tc: a1 * kb.fc,
            ts: a1 * kb.fs,
        };
        let rows = ta[1..].iter().zip(&tb[2..]).zip(&upper[m + 1..=n_max]).zip(lower[m + 1..=n_max].iter_mut());
        for (((ka, kb), &u0), out) in rows {
            let a = rho_u * ka.alpha * a1 - rho2 * ka.beta * a2;
            (a2, a1) = (a1, a);
            let b = rho_u * kb.alpha * b1 - rho2 * kb.beta * b2;
            (b2, b1) = (b1, b);
            *out = b;
            wa += 1.0;
            wb += 1.0;
            let (qa, qb) = (wa * a, wb * b);
            high.sc += a * ka.c;
            high.ss += a * ka.s;
            high.hc += qa * ka.c;
            high.hs += qa * ka.s;
            high.tc += u0 * ka.fc;
            high.ts += u0 * ka.fs;
            low.sc += b * kb.c;
            low.ss += b * kb.s;
            low.hc += qb * kb.c;
            low.hs += qb * kb.s;
            low.tc += a * kb.fc;
            low.ts += a * kb.fs;
        }
        (high, low)
    }
}
