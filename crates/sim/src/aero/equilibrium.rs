//! Air (or nitrogen) in chemical and thermal equilibrium, for the shock
//! layer (D109): the composition at a temperature and pressure from the
//! species' partition functions, and the mixture's density, enthalpy and
//! electron density.
//!
//! Eleven species, N₂ O₂ NO N O N₂⁺ O₂⁺ NO⁺ N⁺ O⁺ e⁻: the high-temperature
//! air model of Hunt & Souders (NASA SP-3093) and of Anderson, *Hypersonic
//! and High-Temperature Gas Dynamics*, ch. 11 (statistical thermodynamics;
//! the eleven-species air of §11.9 and §14.6). Each species' partition
//! function is translational × rigid rotor × harmonic oscillator × its
//! lowest electronic levels; energies are measured from the ground states
//! of N₂ and O₂ by the species' heats of formation at 0 K (JANAF tables,
//! 4th ed., with the NIST ionisation energies). Equilibrium is the law of
//! mass action written with element potentials: n_s = Q_s/V · e^(−θ_f/T) ·
//! a_N^(n_N) · a_O^(n_O) · a_e^(−z), solved by Newton on the logarithms of
//! a_N and a_O for the total number density p/kT and the element ratio;
//! the electron potential follows from charge neutrality in closed form.
//! Argon (0.9 % by volume) is counted as nitrogen. Left out: anharmonicity
//! and the rotation–vibration coupling (a few per cent in the partition
//! functions at 10,000 K), the higher electronic levels, Debye–Hückel.

use crate::math;

/// Boltzmann's constant (J/K).
pub const K_B: f64 = 1.380_649e-23;
const H_PLANCK: f64 = 6.626_070_15e-34;
const AVOGADRO: f64 = 6.022_140_76e23;
const R: f64 = 8.314_462_618;
const M_N: f64 = 0.014_006_7;
const M_O: f64 = 0.015_999_4;
const M_E: f64 = 5.485_799e-7;
/// Oxygen's mass fraction of air (US 1976 sea level; the rest, nitrogen
/// and argon, is counted as nitrogen).
pub const AIR_OXYGEN_FRACTION: f64 = 0.2314;

#[derive(Clone, Copy)]
enum Structure {
    Atom,
    Diatomic { theta_v: f64, theta_r: f64, symmetry: f64 },
}

#[derive(Clone, Copy)]
struct Species {
    n: f64,
    o: f64,
    charge: f64,
    /// Heat of formation at 0 K (kJ/mol) from N₂ and O₂.
    formation: f64,
    structure: Structure,
    /// Electronic levels: (degeneracy, energy in K).
    levels: &'static [(f64, f64)],
}

const fn di(theta_v: f64, theta_r: f64, symmetry: f64) -> Structure {
    Structure::Diatomic { theta_v, theta_r, symmetry }
}

/// The species, in this order: N₂ O₂ NO N O N₂⁺ O₂⁺ NO⁺ N⁺ O⁺ e⁻.
/// θ_v, θ_r: Herzberg's constants (ω_e, B_e) as kelvin; the levels from
/// Moore's tables (NBS 467) and Huber & Herzberg.
const SPECIES: [Species; 11] = [
    Species {
        n: 2.0,
        o: 0.0,
        charge: 0.0,
        formation: 0.0,
        structure: di(3_395.0, 2.88, 2.0),
        levels: &[(1.0, 0.0), (3.0, 72_230.0)],
    },
    Species {
        n: 0.0,
        o: 2.0,
        charge: 0.0,
        formation: 0.0,
        structure: di(2_239.0, 2.08, 2.0),
        levels: &[(3.0, 0.0), (2.0, 11_393.0), (1.0, 18_985.0)],
    },
    Species {
        n: 1.0,
        o: 1.0,
        charge: 0.0,
        formation: 89.77,
        structure: di(2_719.0, 2.45, 1.0),
        levels: &[(2.0, 0.0), (2.0, 174.0), (2.0, 63_240.0)],
    },
    Species {
        n: 1.0,
        o: 0.0,
        charge: 0.0,
        formation: 470.82,
        structure: Structure::Atom,
        levels: &[(4.0, 0.0), (10.0, 27_660.0), (6.0, 41_490.0)],
    },
    Species {
        n: 0.0,
        o: 1.0,
        charge: 0.0,
        formation: 246.79,
        structure: Structure::Atom,
        levels: &[(5.0, 0.0), (3.0, 228.0), (1.0, 326.0), (5.0, 22_830.0), (1.0, 48_620.0)],
    },
    Species {
        n: 2.0,
        o: 0.0,
        charge: 1.0,
        formation: 1_503.3,
        structure: di(3_175.0, 2.78, 2.0),
        levels: &[(2.0, 0.0), (4.0, 13_190.0), (2.0, 36_770.0)],
    },
    Species {
        n: 0.0,
        o: 2.0,
        charge: 1.0,
        formation: 1_164.6,
        structure: di(2_741.0, 2.43, 2.0),
        levels: &[(4.0, 0.0), (8.0, 47_400.0)],
    },
    Species {
        n: 1.0,
        o: 1.0,
        charge: 1.0,
        formation: 983.63,
        structure: di(3_419.0, 2.87, 1.0),
        levels: &[(1.0, 0.0)],
    },
    Species {
        n: 1.0,
        o: 0.0,
        charge: 1.0,
        formation: 1_873.15,
        structure: Structure::Atom,
        levels: &[(1.0, 0.0), (3.0, 70.0), (5.0, 188.0), (5.0, 22_040.0), (1.0, 47_030.0)],
    },
    Species {
        n: 0.0,
        o: 1.0,
        charge: 1.0,
        formation: 1_560.73,
        structure: Structure::Atom,
        levels: &[(4.0, 0.0), (10.0, 38_600.0), (6.0, 58_220.0)],
    },
    Species { n: 0.0, o: 0.0, charge: -1.0, formation: 0.0, structure: Structure::Atom, levels: &[(2.0, 0.0)] },
];
const ELECTRON: usize = 10;

impl Species {
    fn molar_mass(&self) -> f64 {
        self.n * M_N + self.o * M_O - self.charge * M_E
    }

    fn electronic(&self, t: f64) -> (f64, f64) {
        let (mut q, mut e) = (0.0, 0.0);
        for &(g, theta) in self.levels {
            let w = g * math::exp(-theta / t);
            q += w;
            e += w * theta;
        }
        (q, e / q)
    }

    /// ln of the partition function per unit volume (1/m³) with the
    /// energy of formation, and the enthalpy per particle over k (K).
    fn thermo(&self, t: f64) -> (f64, f64) {
        let m = self.molar_mass() / AVOGADRO;
        let theta_f = self.formation * 1e3 / R;
        let (q_el, e_el) = self.electronic(t);
        let mut ln_q =
            1.5 * math::ln(2.0 * math::PI * m * K_B * t / (H_PLANCK * H_PLANCK)) + math::ln(q_el) - theta_f / t;
        let mut h = 2.5 * t + theta_f + e_el;
        if let Structure::Diatomic { theta_v, theta_r, symmetry } = self.structure {
            let x = math::exp(-theta_v / t);
            ln_q += math::ln(t / (symmetry * theta_r)) - math::ln(1.0 - x);
            h += t + theta_v * x / (1.0 - x);
        }
        (ln_q, h)
    }
}

/// A mixture in equilibrium: its temperature and number densities (1/m³,
/// in the order of the species).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mixture {
    pub temperature: f64,
    pub n: [f64; 11],
    /// The element potentials ln a_N, ln a_O (a warm start for the next).
    potentials: (f64, f64),
}

impl Mixture {
    /// Pressure (Pa).
    pub fn pressure(&self) -> f64 {
        self.n.iter().sum::<f64>() * K_B * self.temperature
    }

    /// Density (kg/m³).
    pub fn density(&self) -> f64 {
        self.n.iter().zip(&SPECIES).map(|(n, s)| n * s.molar_mass()).sum::<f64>() / AVOGADRO
    }

    /// Specific enthalpy (J/kg) from the ground states of N₂ and O₂ at 0 K.
    pub fn enthalpy(&self) -> f64 {
        let t = self.temperature;
        let h: f64 = self.n.iter().zip(&SPECIES).map(|(n, s)| n * s.thermo(t).1).sum();
        h * K_B / self.density()
    }

    /// Electron number density (1/m³).
    pub fn electron_density(&self) -> f64 {
        self.n[ELECTRON]
    }

    /// Mean molar mass (kg/mol).
    pub fn molar_mass(&self) -> f64 {
        self.density() * AVOGADRO / self.n.iter().sum::<f64>()
    }
}

fn log_sum_exp(v: impl Iterator<Item = (f64, f64)> + Clone) -> f64 {
    let m = v.clone().map(|(l, _)| l).fold(f64::NEG_INFINITY, f64::max);
    if m == f64::NEG_INFINITY {
        return m;
    }
    m + math::ln(v.map(|(l, w)| w * math::exp(l - m)).sum::<f64>())
}

/// The equilibrium mixture at temperature `t` (K) and pressure `p` (Pa) of
/// a gas whose oxygen mass fraction is `oxygen` (air
/// [`AIR_OXYGEN_FRACTION`]; 0: nitrogen), from the element potentials
/// `start` of a nearby solve if given.
pub fn mixture(t: f64, p: f64, oxygen: f64, start: Option<&Mixture>) -> Mixture {
    let ln_q = SPECIES.map(|s| s.thermo(t).0);
    let has_o = oxygen > 0.0;
    let ln_total = math::ln(p / (K_B * t));
    // N atoms per O atom, and their shares of the atoms.
    let ratio = if has_o { (1.0 - oxygen) / M_N * M_O / oxygen } else { f64::INFINITY };
    let ln_ratio = math::ln(ratio);
    let frac_n = if has_o { ratio / (1.0 + ratio) } else { 1.0 };
    let frac_o = 1.0 - frac_n;
    // A start that undershoots every species: the smaller of the
    // all-molecules and all-atoms potentials.
    let guess = |atoms: f64, mol: usize, atom: usize| {
        let molecules = 0.5 * (ln_total + math::ln(0.5 * atoms) - ln_q[mol]);
        let free = ln_total + math::ln(atoms) - ln_q[atom];
        molecules.min(free)
    };
    let (mut x, mut y) = match start {
        Some(m) => m.potentials,
        None => (guess(frac_n, 0, 3), if has_o { guess(frac_o, 1, 4) } else { -1e30 }),
    };
    if !has_o {
        y = -1e30;
    }
    let mut ln_n = [f64::NEG_INFINITY; 11];
    for _ in 0..200 {
        // The electrons from charge neutrality: 2z = ln Σ_ions − ln Q_e.
        let ions = || {
            (0..ELECTRON)
                .filter(|&i| SPECIES[i].charge > 0.0)
                .map(|i| (ln_q[i] + SPECIES[i].n * x + SPECIES[i].o * y, 1.0))
        };
        let lse = log_sum_exp(ions());
        let (z, dzdx, dzdy) = if lse > -700.0 {
            let (mut dx, mut dy) = (0.0, 0.0);
            for (i, s) in SPECIES.iter().enumerate().take(ELECTRON).filter(|(_, s)| s.charge > 0.0) {
                let w = math::exp(ln_q[i] + s.n * x + s.o * y - lse);
                dx += 0.5 * w * s.n;
                dy += 0.5 * w * s.o;
            }
            (0.5 * (lse - ln_q[ELECTRON]), dx, dy)
        } else {
            (f64::NEG_INFINITY, 0.0, 0.0)
        };
        let (mut sum, mut el_n, mut el_o) = (0.0, 0.0, 0.0);
        let mut j = [[0.0; 2]; 3];
        for (i, s) in SPECIES.iter().enumerate() {
            ln_n[i] = if i == ELECTRON {
                ln_q[i] + z
            } else if s.charge == 0.0 {
                ln_q[i] + s.n * x + s.o * y
            } else if z == f64::NEG_INFINITY {
                f64::NEG_INFINITY
            } else {
                ln_q[i] + s.n * x + s.o * y - s.charge * z
            };
            let ni = if ln_n[i] > -700.0 { math::exp(ln_n[i]) } else { 0.0 };
            let (a, b) = (s.n - s.charge * dzdx, s.o - s.charge * dzdy);
            sum += ni;
            el_n += ni * s.n;
            el_o += ni * s.o;
            j[0][0] += ni * a;
            j[0][1] += ni * b;
            j[1][0] += ni * s.n * a;
            j[1][1] += ni * s.n * b;
            j[2][0] += ni * s.o * a;
            j[2][1] += ni * s.o * b;
        }
        let f1 = math::ln(sum) - ln_total;
        let (f2, j21, j22) = if has_o {
            (
                math::ln(el_n) - math::ln(el_o) - ln_ratio,
                j[1][0] / el_n - j[2][0] / el_o,
                j[1][1] / el_n - j[2][1] / el_o,
            )
        } else {
            (0.0, 0.0, 1.0)
        };
        let (j11, j12) = (j[0][0] / sum, j[0][1] / sum);
        if f1.abs() < 1e-13 && f2.abs() < 1e-13 {
            break;
        }
        let det = j11 * j22 - j12 * j21;
        let dx = ((-f1 * j22 + f2 * j12) / det).clamp(-4.0, 4.0);
        let dy = ((-f2 * j11 + f1 * j21) / det).clamp(-4.0, 4.0);
        x += dx;
        if has_o {
            y += dy;
        }
    }
    let mut n = [0.0; 11];
    for i in 0..11 {
        n[i] = if ln_n[i] > -700.0 { math::exp(ln_n[i]) } else { 0.0 };
    }
    Mixture { temperature: t, n, potentials: (x, y) }
}
