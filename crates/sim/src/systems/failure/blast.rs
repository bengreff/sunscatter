//! Explosions (D105; ship-model.md §6.5; falcon9.md §6.5): the energy of
//! what was released and the overpressure it reaches neighbours with.
//!
//! * A burst gas vessel releases its isentropic expansion energy,
//!   E = p·V/(γ−1)·[1 − (p_a/p)^((γ−1)/γ)] ([`gas_energy`]).
//! * Mixing propellants release a TNT equivalent, a yield factor (kg TNT
//!   per kg of propellant involved) by propellant pair and impact speed
//!   (FAA AC 450.137-1 Table 1, Project PYRO's screening values,
//!   [`yield_factor`]): LOX/RP-1 is 0 at rest (a deflagration and fireball,
//!   not a detonation; D105's low end for a rupture in flight), hypergols
//!   2 %.
//! * The blast's incident overpressure at a distance R from W kg of TNT
//!   follows the scaled distance Z = R/W^⅓ ([`overpressure`]: Kinney &
//!   Graham's closed form of the Kingery–Bulmash free-air data, checked
//!   against DoD 6055.09's K-factors). It damages a neighbour whose rating
//!   it exceeds (a tank's or a bottle's burst pressure), so failures cascade.

use crate::math;

/// TNT's energy (J/kg).
pub const TNT_J_PER_KG: f64 = 4.184e6;

/// The isentropic expansion energy (J) of `volume` m³ of gas at `p` Pa with
/// ratio of specific heats `gamma` into `ambient` Pa (zero if not above it).
pub fn gas_energy(p: f64, volume: f64, gamma: f64, ambient: f64) -> f64 {
    if p <= ambient || p <= 0.0 || gamma <= 1.0 {
        return 0.0;
    }
    let k = (gamma - 1.0) / gamma;
    p * volume / (gamma - 1.0) * (1.0 - math::pow(ambient.max(0.0) / p, k))
}

/// Propellant pairs with published yields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pair {
    LoxKerosene,
    Hypergolic,
    LoxHydrogen,
}

impl Pair {
    /// The pair of an oxidizer and a fuel by their fluid ids, if published.
    pub fn of(oxidizer: &str, fuel: &str) -> Option<Pair> {
        match (oxidizer, fuel) {
            ("LOX", "RP-1") => Some(Pair::LoxKerosene),
            ("NTO", "MMH") | ("NTO", "UDMH") | ("NTO", "Aerozine-50") => Some(Pair::Hypergolic),
            ("LOX", "LH2") => Some(Pair::LoxHydrogen),
            _ => None,
        }
    }
}

/// Linear interpolation in a table of (speed m/s, yield), held at its ends.
fn table(points: &[(f64, f64)], v: f64) -> f64 {
    let v = v.max(0.0);
    if v <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        let ((a, ya), (b, yb)) = (w[0], w[1]);
        if v <= b {
            return ya + (yb - ya) * (v - a) / (b - a);
        }
    }
    points[points.len() - 1].1
}

/// The TNT yield factor (kg TNT per kg of propellant involved) of a pair at
/// impact speed `v` (m/s; 0 for a rupture in flight): FAA AC 450.137-1
/// (2024) Table 1 (falcon9.md §6.5).
pub fn yield_factor(pair: Pair, v: f64) -> f64 {
    match pair {
        // 0 at rest to 0.215 at 52 m/s (170 ft/s), linear.
        Pair::LoxKerosene => table(&[(0.0, 0.0), (52.0, 0.215)], v),
        Pair::Hypergolic => table(
            &[
                (0.0, 0.020),
                (30.0, 0.085),
                (61.0, 0.135),
                (91.0, 0.180),
                (122.0, 0.220),
                (152.0, 0.245),
                (183.0, 0.260),
                (213.0, 0.275),
                (244.0, 0.280),
            ],
            v,
        ),
        // 0.16 to 26 m/s, 1.50 at 227 m/s and over.
        Pair::LoxHydrogen => table(&[(26.0, 0.16), (227.0, 1.50)], v),
    }
}

/// The incident overpressure (Pa) at `r` m from `tnt` kg of TNT in air at
/// `ambient` Pa (free air; Kinney & Graham 1985, eq. 6-1:
/// p/p_a = 808·[1 + (Z/4.5)²] / √([1 + (Z/0.048)²][1 + (Z/0.32)²][1 + (Z/1.35)²]),
/// Z = R/W^⅓ in m/kg^⅓). In vacuum there is no air to carry a blast: 0.
pub fn overpressure(r: f64, tnt: f64, ambient: f64) -> f64 {
    if tnt <= 0.0 || ambient <= 0.0 {
        return 0.0;
    }
    let z = r.max(1e-3) / math::cbrt(tnt);
    let sq = |a: f64| 1.0 + (z / a) * (z / a);
    ambient * 808.0 * sq(4.5) / (sq(0.048) * sq(0.32) * sq(1.35)).sqrt()
}

/// The overpressure of a burst vessel's gas in vacuum or air: its energy as
/// TNT (kg).
pub fn tnt_of(energy: f64) -> f64 {
    energy.max(0.0) / TNT_J_PER_KG
}

#[cfg(test)]
mod tests {
    use super::*;

    /// falcon9.md §6.5: a stage 2 helium COPV, 0.19 m³ at 37.9 MPa → 9.8 MJ
    /// (2.3 kg TNT); a 0.43 m³ stage 1 bottle → 22 MJ; a 0.2 m³ nitrogen
    /// bottle at 34.5 MPa → 14 MJ.
    #[test]
    fn burst_energies_from_the_data_sheet() {
        let p_a = 101_325.0;
        let he = gas_energy(37.9e6, 0.19, 5.0 / 3.0, p_a);
        assert!((he / 9.8e6 - 1.0).abs() < 0.01, "{he}");
        assert!((tnt_of(he) - 2.3).abs() < 0.05);
        assert!((gas_energy(37.9e6, 0.43, 5.0 / 3.0, p_a) / 22e6 - 1.0).abs() < 0.02);
        assert!((gas_energy(34.5e6, 0.2, 1.4, p_a) / 14e6 - 1.0).abs() < 0.02);
        assert_eq!(gas_energy(1e5, 1.0, 1.4, 101_325.0), 0.0, "below ambient");
    }

    #[test]
    fn yields_table() {
        let cases = [
            (Pair::LoxKerosene, 0.0, 0.0),
            (Pair::LoxKerosene, 26.0, 0.1075),
            (Pair::LoxKerosene, 100.0, 0.215),
            (Pair::Hypergolic, 0.0, 0.02),
            (Pair::Hypergolic, 45.5, 0.11),
            (Pair::Hypergolic, 1000.0, 0.28),
            (Pair::LoxHydrogen, 0.0, 0.16),
            (Pair::LoxHydrogen, 300.0, 1.5),
        ];
        for (pair, v, y) in cases {
            assert!((yield_factor(pair, v) - y).abs() < 1e-12, "{pair:?} {v}: {}", yield_factor(pair, v));
        }
        assert_eq!(Pair::of("LOX", "RP-1"), Some(Pair::LoxKerosene));
        assert_eq!(Pair::of("NTO", "MMH"), Some(Pair::Hypergolic));
        assert_eq!(Pair::of("water", "RP-1"), None);
    }

    /// Against Kingery–Bulmash as DoD 6055.09 tabulates it for a
    /// hemispherical surface burst (a surface burst is a free-air burst of
    /// 1.8 W): K = R/W^⅓ of 9, 18, 24 and 40 ft/lb^⅓ give 12, 3.5, 2.3 and
    /// 1.2 psi, within 15 %.
    #[test]
    fn overpressure_against_kingery_bulmash() {
        let p_a = 101_325.0;
        let ft_per_lb3_to_m_per_kg3 = 0.3048 / (0.453_592_37f64).powf(1.0 / 3.0);
        for (k, psi) in [(9.0, 12.0), (18.0, 3.5), (24.0, 2.3), (40.0, 1.2)] {
            // 1 kg of TNT on the surface: R = K·1 kg^⅓ as free air of 1.8 kg.
            let r = k * ft_per_lb3_to_m_per_kg3;
            let p = overpressure(r, 1.8, p_a) / 6_894.76;
            assert!((p / psi - 1.0).abs() < 0.15, "K{k}: {p:.2} psi against {psi}");
        }
        // Scaling: the same Z, the same overpressure.
        assert!((overpressure(2.0, 1.0, p_a) / overpressure(20.0, 1000.0, p_a) - 1.0).abs() < 1e-12);
        assert_eq!(overpressure(1.0, 10.0, 0.0), 0.0, "no air, no blast");
        assert!(overpressure(1.0, 1.0, p_a) > overpressure(2.0, 1.0, p_a));
    }
}
