//! Gravity fields of bodies beyond the point mass: fully normalized
//! spherical harmonics in the body's own rotating frame (D090).
//!
//! The potential outside the reference sphere of radius `R` is
//!
//! ```text
//! V = (GM/r) Σ_{n≥2} (R/r)^n Σ_{m≤n} P̄nm(sin φ) (C̄nm cos mλ + S̄nm sin mλ)
//! ```
//!
//! with geodesy (4π) normalization, `P̄nm = sqrt((2 − δm0)(2n+1)(n−m)!/(n+m)!) Pnm`
//! (no Condon–Shortley phase), the convention of every published model used
//! here. The point mass (`n = 0`) is the source's own pull ([`crate::forces`]);
//! `n = 1` vanishes about the centre of mass.
//!
//! **Evaluation** ([`pines`]): Pines' singularity-free formulation in
//! Cartesian direction cosines (Pines 1973, AIAA J. 11, 1508; normalized as in
//! Lundberg & Schutz 1988, JGCD 11, 31, and Eckman et al. 2014, NASA/TP-2014-218519),
//! with the radius ratio folded into the column recursion and the degree sums
//! lumped per order before the order's trigonometric polynomials (Holmes &
//! Featherstone 2002, J. Geodesy 76, 279). No division by `cos φ`, so the poles
//! are ordinary points; every quantity is a polynomial in the direction
//! cosines, so there is no trigonometry at all (rule 2). Sums run in a fixed
//! order (orders from the highest down, degrees up within each).
//!
//! **One owner** (rule 5): every vessel force from a body's figure goes
//! through [`Field`]; a body with only `j2` in its `body.ron` has the
//! degree-2 zonal field. A zonal field (or the zonal part below the first
//! tesseral degree) needs only the pole, not the body's spin. The ephemeris
//! generator's closed-form J2/J4/J6 between bodies ([`zonal`]) is kept as it
//! is because the shipped ephemeris must regenerate bit for bit; it is tested
//! against this model.
//!
//! **Truncation** ([`Field::included_degree`], `forces::field_degree`): a
//! vessel includes degree `n` of a body only where the sum of the
//! acceleration bounds of all the degrees above could exceed a threshold at
//! the vessel (the world cutoff times [`crate::world::FAINT_ERROR_FRACTION`],
//! as the former J2 rule), so a far vessel pays for nothing and a low one
//! for every degree the data has. The bound is rigorous ([`Field::bound`]).

pub mod file;
mod pines;
pub mod zonal;

#[cfg(test)]
mod tests;

use crate::body::Rotation;
use crate::time::Epoch;
use glam::{DMat3, DVec3};

/// The highest degree a field may have (stack buffers in [`pines`]; the
/// unscaled recursion is far from overflow there).
pub const MAX_DEGREE: usize = 600;

/// One `(n, m)` entry of a field, stored by column (fixed order `m`, degree
/// `n` ascending), with the recursion's constants for `P̄'nm = ρⁿ Ānm`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Term {
    /// `P̄'nm = ρ u α P̄'n−1,m − ρ² β P̄'n−2,m`.
    alpha: f64,
    beta: f64,
    c: f64,
    s: f64,
    /// `C̄nm`, `S̄nm` times `Nnm / Nn,m+1` (the `∂/∂u` term).
    fc: f64,
    fs: f64,
}

/// A body's gravity field beyond its point mass (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// The model's name (`"J2"` for a body with only `body.ron`'s J2).
    pub model: String,
    /// Reference radius of the coefficients (m).
    pub radius: f64,
    /// The model's GM (m³/s²), which scales its harmonics; `None`: the
    /// source's own GM.
    pub gm: Option<f64>,
    degree: usize,
    /// Highest order with a non-zero coefficient.
    max_order: usize,
    /// Lowest degree with a non-zero tesseral or sectoral coefficient
    /// (`degree + 1` if none): below it the field is zonal.
    tesseral_from: usize,
    /// From the body-fixed frame of the body's rotation model to the
    /// coefficients' frame (`None`: the same).
    to_field: Option<DMat3>,
    /// Start of column `m` in `terms` (entry `n` at `columns[m] + n − m`),
    /// for `m = 0..=max_order + 1`, each to degree `degree`.
    columns: Vec<usize>,
    terms: Vec<Term>,
    /// `P̄'mm = ρ diag[m] P̄'m−1,m−1`.
    diag: Vec<f64>,
    /// `(2n+1) √(n+1) ‖c_n‖` per degree, `‖c_n‖² = Σ_m C̄nm² + S̄nm²`
    /// ([`Self::bound`]).
    strength: Vec<f64>,
}

/// Fully normalized coefficients of one degree and order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coefficient {
    pub n: usize,
    pub m: usize,
    pub c: f64,
    pub s: f64,
}

/// The factor `N_nm` of the geodesy normalization (`P̄nm = N_nm Pnm`),
/// for converting unnormalized coefficients (`C̄ = C / N`).
pub fn normalization(n: usize, m: usize) -> f64 {
    // (n−m)!/(n+m)! as a product, exact enough for the low degrees where
    // published coefficients are unnormalized.
    let mut ratio = 1.0;
    for k in (n - m + 1)..=(n + m) {
        ratio /= k as f64;
    }
    let delta = if m == 0 { 1.0 } else { 2.0 };
    (delta * (2 * n + 1) as f64 * ratio).sqrt()
}

impl Field {
    /// A field from fully normalized coefficients (degrees below 2 are
    /// ignored). `to_field` rotates the body-fixed frame of the body's
    /// rotation model into the coefficients' frame.
    pub fn new(
        model: &str,
        radius: f64,
        gm: Option<f64>,
        coefficients: &[Coefficient],
        to_field: Option<DMat3>,
    ) -> Result<Self, String> {
        if !(radius > 0.0 && radius.is_finite()) || gm.is_some_and(|g| !(g > 0.0 && g.is_finite())) {
            return Err("a field needs a positive radius and GM".into());
        }
        let used: Vec<&Coefficient> = coefficients.iter().filter(|k| k.n >= 2 && (k.c != 0.0 || k.s != 0.0)).collect();
        if let Some(k) = coefficients.iter().find(|k| k.m > k.n || !(k.c.is_finite() && k.s.is_finite())) {
            return Err(format!("invalid coefficient ({}, {})", k.n, k.m));
        }
        if let Some(k) = coefficients.iter().find(|k| k.m == 0 && k.s != 0.0) {
            return Err(format!("S̄({}, 0) must be zero", k.n));
        }
        let degree = used.iter().map(|k| k.n).max().ok_or("a field needs a coefficient of degree 2 or more")?;
        if degree > MAX_DEGREE {
            return Err(format!("degree {degree} exceeds {MAX_DEGREE}"));
        }
        let max_order = used.iter().map(|k| k.m).max().unwrap_or(0);
        let tesseral_from = used.iter().filter(|k| k.m > 0).map(|k| k.n).min().unwrap_or(degree + 1);
        let mut columns = Vec::with_capacity(max_order + 2);
        let mut terms = Vec::new();
        for m in 0..=max_order + 1 {
            columns.push(terms.len());
            for n in m..=degree {
                terms.push(recursion(n, m));
            }
        }
        let mut field = Field {
            model: model.into(),
            radius,
            gm,
            degree,
            max_order,
            tesseral_from,
            to_field,
            columns,
            terms,
            diag: (0..=max_order + 1).map(diagonal).collect(),
            strength: vec![0.0; degree + 1],
        };
        let mut norm2 = vec![0.0; degree + 1];
        for k in &used {
            let i = field.index(k.n, k.m);
            let t = &mut field.terms[i];
            if t.c != 0.0 || t.s != 0.0 {
                return Err(format!("coefficient ({}, {}) given twice", k.n, k.m));
            }
            let f = ratio_f(k.n, k.m);
            *t = Term { c: k.c, s: k.s, fc: f * k.c, fs: f * k.s, ..*t };
            norm2[k.n] += k.c * k.c + k.s * k.s;
        }
        for n in 2..=degree {
            field.strength[n] = (2 * n + 1) as f64 * ((n + 1) as f64).sqrt() * norm2[n].sqrt();
        }
        Ok(field)
    }

    /// The degree-2 zonal field of a body with only a J2 (referenced to
    /// `radius`, the source's GM).
    pub fn j2(j2: f64, radius: f64) -> Self {
        let c = Coefficient { n: 2, m: 0, c: -j2 / 5f64.sqrt(), s: 0.0 };
        Field::new("J2", radius, None, &[c], None).expect("a valid J2 field")
    }

    /// The highest degree of the data.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Fully normalized `(C̄nm, S̄nm)` (zero beyond the data).
    pub fn coefficient(&self, n: usize, m: usize) -> (f64, f64) {
        if n < 2 || n > self.degree || m > n.min(self.max_order) {
            return (0.0, 0.0);
        }
        let t = &self.terms[self.index(n, m)];
        (t.c, t.s)
    }

    /// The number of `(n, m)` terms an evaluation to `degree` computes:
    /// two columns for a zonal field (or the zonal part below the first
    /// tesseral degree), every order's column otherwise (its cost goes with
    /// this, ~1.6 ns a term on the reference Mac).
    pub fn evaluation_terms(&self, degree: usize) -> usize {
        let n = degree.min(self.degree);
        if n < 2 {
            0
        } else if n < self.tesseral_from {
            2 * (n + 1)
        } else {
            let m = n.min(self.max_order) + 1;
            (m + 1) * (n + 1) - m * (m + 1) / 2
        }
    }

    fn index(&self, n: usize, m: usize) -> usize {
        self.columns[m] + n - m
    }

    /// An upper bound on the acceleration of degree `n` anywhere at distance
    /// `r` from the centre: `(GM/r²) (R/r)ⁿ (2n+1) √(n+1) ‖c_n‖`. From the
    /// addition theorem, a degree-`n` surface harmonic and its surface
    /// gradient are at most `‖c_n‖ √(2n+1)` and `‖c_n‖ √(n(n+1)(2n+1))`;
    /// the radial derivative brings `n+1`.
    pub fn bound(&self, gm: f64, n: usize, r: f64) -> f64 {
        if n < 2 || n > self.degree {
            return 0.0;
        }
        gm / (r * r) * crate::math::powi(self.radius / r, n as i32) * self.strength[n]
    }

    /// The lowest degree `N` such that the bounds of every degree above it,
    /// summed, stay at or under `threshold` at distance `r` (0: the field is
    /// negligible there). With a zero threshold, the data's degree.
    pub fn included_degree(&self, gm: f64, r: f64, threshold: f64) -> usize {
        if threshold <= 0.0 {
            return self.degree;
        }
        // The remainder by subtraction from the total is off by a few ulps
        // of the total (J2's, at most ~1e-2 m/s²), far below any threshold.
        let rho = self.radius / r;
        let g = gm / (r * r);
        let mut total = 0.0;
        let mut p = rho;
        for n in 2..=self.degree {
            p *= rho;
            total += g * p * self.strength[n];
        }
        if total <= threshold {
            return 0;
        }
        let mut rest = total;
        let mut p = rho;
        for n in 2..=self.degree {
            p *= rho;
            rest -= g * p * self.strength[n];
            if rest <= threshold {
                return n;
            }
        }
        self.degree
    }

    /// Acceleration (inertial axes, m/s²) and potential (m²/s², positive)
    /// of the field to degree `degree` at `d` from the body's centre
    /// (inertial axes) at `t`, `gm` scaling the harmonics. The potential is
    /// computed only with `potential` (else 0).
    pub fn accel(
        &self,
        gm: f64,
        d: DVec3,
        rotation: &Rotation,
        t: Epoch,
        degree: usize,
        potential: bool,
    ) -> (DVec3, f64) {
        let n = degree.min(self.degree);
        if n < 2 {
            return (DVec3::ZERO, 0.0);
        }
        let r = d.length();
        let rhat = d / r;
        if n < self.tesseral_from {
            // Zonal: symmetric about the pole, so the pole is the only axis needed.
            let pole = rotation.pole(t);
            let [_, _, a3, a4, u] = pines::sums(self, gm, r, [0.0, 0.0, rhat.dot(pole)], n, 0, potential);
            return (pole * a3 + rhat * a4, u);
        }
        let to_inertial = rotation.matrix(t);
        let fixed = to_inertial.transpose() * rhat;
        let x = self.to_field.map_or(fixed, |m| m * fixed);
        let [a1, a2, a3, a4, u] = pines::sums(self, gm, r, [x.x, x.y, x.z], n, n.min(self.max_order), potential);
        let a = DVec3::new(a1, a2, a3) + x * a4;
        let a_fixed = self.to_field.map_or(a, |m| m.transpose() * a);
        (to_inertial * a_fixed, u)
    }

    /// Acceleration and potential in the coefficients' own frame at `x`
    /// (m), to degree `degree` (all orders): for tests and tools.
    pub fn accel_in_field_frame(&self, gm: f64, x: DVec3, degree: usize) -> (DVec3, f64) {
        let n = degree.min(self.degree);
        if n < 2 {
            return (DVec3::ZERO, 0.0);
        }
        let r = x.length();
        let u = x / r;
        let [a1, a2, a3, a4, pot] = pines::sums(self, gm, r, [u.x, u.y, u.z], n, n.min(self.max_order), true);
        (DVec3::new(a1, a2, a3) + u * a4, pot)
    }
}

/// The column recursion's constants for `(n, m)` (`n ≥ m + 1`; zero on the
/// diagonal, which [`diagonal`] seeds).
fn recursion(n: usize, m: usize) -> Term {
    if n <= m {
        return Term::default();
    }
    let (nf, mf) = (n as f64, m as f64);
    let alpha = ((2.0 * nf + 1.0) * (2.0 * nf - 1.0) / ((nf - mf) * (nf + mf))).sqrt();
    let beta = if n >= m + 2 {
        ((2.0 * nf + 1.0) * (nf + mf - 1.0) * (nf - mf - 1.0) / ((2.0 * nf - 3.0) * (nf + mf) * (nf - mf))).sqrt()
    } else {
        0.0
    };
    Term { alpha, beta, ..Term::default() }
}

/// `Āmm / Ām−1,m−1`: `√3` for `m = 1`, `√((2m+1)/(2m))` above (1 for `m = 0`).
fn diagonal(m: usize) -> f64 {
    match m {
        0 => 1.0,
        1 => 3f64.sqrt(),
        _ => ((2 * m + 1) as f64 / (2 * m) as f64).sqrt(),
    }
}

/// `N_nm / N_n,m+1 = √((2 − δm0)/2 · (n−m)(n+m+1))`.
fn ratio_f(n: usize, m: usize) -> f64 {
    let half = if m == 0 { 0.5 } else { 1.0 };
    (half * ((n - m) * (n + m + 1)) as f64).sqrt()
}
