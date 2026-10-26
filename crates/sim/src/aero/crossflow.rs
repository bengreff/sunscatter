//! The viscous crossflow of a slender body (D070): the drag coefficient of
//! a circular cylinder in the flow across it, and the finite-length factor
//! η, against the crossflow Mach and Reynolds numbers M_n = M·sin α and
//! Re_n = Re_d·sin α (Jorgensen, "Prediction of Static Aerodynamic
//! Characteristics for Slender Bodies Alone and With Lifting Surfaces to
//! Very High Angles of Attack", NASA TR R-474, 1977, eqs. 2.3–2.6: the
//! viscous normal force per unit length 2r·η·C_dn·q·sin²α).
//!
//! * **C_dn(M_n)**, subcritical: R-474 fig. 1 (the wind-tunnel curve and
//!   the points extrapolated to zero diameter; read from the figure, ±0.02):
//!   1.2 to M_n 0.2, rising through the transonic peak (≈ 2.1 at 0.96) and
//!   falling to 1.25 by M_n 5.5 (modified Newtonian: (2/3)·Cp,max).
//! * **Reynolds number** (fig. 2, M_n ≤ 0.4): 1.2 below the critical
//!   Re_n ≈ 2·10⁵, dropping to 0.27–0.30 at 4–5·10⁵ (the drag crisis),
//!   recovering to 0.55–0.6 by 5·10⁶–10⁷. The drop happens only below
//!   M_n ≈ 0.5 (p. 15): full at M_n 0.4, none from 0.6, linear between.
//! * **η(M_n)** (fig. 6, a body of fineness 10–12): its subsonic value at
//!   low M_n (Allen & Perkins' fineness table, [`super::slender::crossflow_eta`]),
//!   rising to ≈ 1 by M_n 0.8, a dip to 0.77 at M_n 1 (where C_dn peaks:
//!   the product, measured directly on bodies at α 45–60°, is smooth,
//!   fig. 5), ≈ 1 for supersonic M_n; scaled from its 0.70 at M_n 0 for
//!   other finenesses.

/// Subcritical C_dn against M_n (R-474 fig. 1).
const CD_MACH: [(f64, f64); 25] = [
    (0.0, 1.20),
    (0.2, 1.20),
    (0.3, 1.22),
    (0.4, 1.27),
    (0.5, 1.37),
    (0.6, 1.48),
    (0.7, 1.55),
    (0.8, 1.50),
    (0.9, 1.57),
    (0.96, 2.10),
    (1.0, 1.98),
    (1.1, 1.80),
    (1.2, 1.69),
    (1.3, 1.56),
    (1.4, 1.57),
    (1.6, 1.50),
    (1.8, 1.45),
    (2.0, 1.41),
    (2.4, 1.36),
    (2.8, 1.34),
    (3.2, 1.32),
    (3.6, 1.31),
    (4.0, 1.29),
    (4.8, 1.26),
    (5.5, 1.25),
];

/// C_dn against Re_n at low M_n (R-474 fig. 2), log-linear in Re_n.
const CD_REYNOLDS: [(f64, f64); 13] = [
    (1.0e4, 1.12),
    (2.0e4, 1.20),
    (1.8e5, 1.22),
    (2.0e5, 1.13),
    (2.5e5, 1.05),
    (3.0e5, 0.90),
    (3.5e5, 0.74),
    (4.0e5, 0.30),
    (5.0e5, 0.27),
    (1.0e6, 0.35),
    (2.0e6, 0.45),
    (5.0e6, 0.55),
    (1.0e7, 0.60),
];

/// η against M_n for a body of fineness 10–12 (R-474 fig. 6).
const ETA_MACH: [(f64, f64); 8] =
    [(0.0, 0.70), (0.4, 0.72), (0.6, 0.82), (0.8, 1.0), (0.9, 1.0), (1.0, 0.77), (1.2, 0.95), (1.4, 0.99)];

/// The subcritical value C_dn takes at low M_n, against which the Reynolds
/// table is a ratio.
const SUBCRITICAL: f64 = 1.2;

/// Linear interpolation in a table, held at its ends.
fn lookup(table: &[(f64, f64)], x: f64) -> f64 {
    if x <= table[0].0 {
        return table[0].1;
    }
    let k = table.partition_point(|e| e.0 <= x);
    if k >= table.len() {
        return table[table.len() - 1].1;
    }
    let (a, b) = (table[k - 1], table[k]);
    a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0)
}

/// Interpolation linear in ln x, held at the table's ends.
fn log_lookup(table: &[(f64, f64)], x: f64) -> f64 {
    if x <= table[0].0 {
        return table[0].1;
    }
    let k = table.partition_point(|e| e.0 <= x);
    if k >= table.len() {
        return table[table.len() - 1].1;
    }
    let ln = crate::math::ln;
    let (a, b) = (table[k - 1], table[k]);
    a.1 + (b.1 - a.1) * (ln(x) - ln(a.0)) / (ln(b.0) - ln(a.0))
}

/// Crossflow drag coefficient of a circular cylinder at crossflow Mach
/// number `mach_n` and Reynolds number `re_n` (on the body's diameter; zero:
/// unknown, subcritical).
pub fn coefficient(mach_n: f64, re_n: f64) -> f64 {
    let cd = lookup(&CD_MACH, mach_n.max(0.0));
    if re_n <= 0.0 {
        return cd;
    }
    let ratio = log_lookup(&CD_REYNOLDS, re_n) / SUBCRITICAL;
    let w = ((0.6 - mach_n) / 0.2).clamp(0.0, 1.0);
    cd * (1.0 - w * (1.0 - ratio))
}

/// The crossflow proportionality factor η of a body whose subsonic factor
/// (fineness table) is `eta_subsonic`, at crossflow Mach number `mach_n`.
pub fn eta(eta_subsonic: f64, mach_n: f64) -> f64 {
    let base = ETA_MACH[0].1;
    let rise = (lookup(&ETA_MACH, mach_n.max(0.0)) - base) / (1.0 - base);
    eta_subsonic + (1.0 - eta_subsonic) * rise
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crossflow_drag_follows_jorgensen() {
        // Fig. 1 points, subcritical.
        for (m, cd) in [(0.0, 1.2), (0.5, 1.37), (1.0, 1.98), (2.0, 1.41), (4.0, 1.29), (8.0, 1.25)] {
            assert!((coefficient(m, 0.0) - cd).abs() < 1e-12, "M_n {m}");
        }
        // Fig. 2 at low M_n: the drag crisis between Re_n 2·10⁵ and 4·10⁵,
        // the partial recovery above.
        assert!((coefficient(0.2, 1e5) - 1.215).abs() < 0.01);
        assert!((coefficient(0.2, 4.5e5) - 0.285).abs() < 0.01);
        assert!((coefficient(0.2, 1e7) - 0.6).abs() < 1e-9);
        // No crisis in compressible crossflow (M_n ≥ 0.6).
        assert_eq!(coefficient(0.7, 1e6), coefficient(0.7, 0.0));
        // η·C_dn measured on bodies at α 45–60° (fig. 5), fineness ≈ 10
        // (η 0.69 subsonic): within 10 %.
        for (m, want) in [(0.6, 1.20), (0.8, 1.52), (1.0, 1.53), (1.3, 1.56), (1.5, 1.49), (1.8, 1.41)] {
            let got = eta(0.69, m) * coefficient(m, 0.0);
            assert!((got / want - 1.0).abs() < 0.1, "M_n {m}: {got} vs {want}");
        }
        // η: the subsonic value at low M_n, ≈ 1 supersonic.
        assert_eq!(eta(0.6, 0.0), 0.6);
        assert!((eta(0.6, 2.0) - 0.987).abs() < 0.01);
    }
}
