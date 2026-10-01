//! Heat into the skin cells: convective stagnation heating spread over the
//! exposed cells, and sunlight (realism-1 §5b).
//!
//! * **Stagnation flux:** Sutton & Graves, "A General Stagnation-Point
//!   Convective Heating Equation for Arbitrary Gas Mixtures", NASA TR R-376
//!   (1971): q̇ = k·√(ρ/Rn)·V³ (SI; k = 1.7415e-4 Earth air, 1.9027e-4
//!   Mars), cold fully catalytic wall; stated accuracy ≈ 10 % for blunt
//!   bodies. Both k checked against NASA's TFAWS 2012 aerothermodynamics
//!   course; West & Brandis (AIAA 2020, NTRS 20200002354, Eq. 2) give
//!   1.83e-4 for 97 % CO₂ / 3 % N₂, 4 % lower and inside the stated 10 %. The nose radius Rn is the bake's per-direction estimate
//!   ([`crate::aero::DirSums::nose_radius`]).
//! * **Radiative stagnation flux** (shock-layer radiation, [`tauber_sutton`]):
//!   Tauber & Sutton, "Stagnation-Point Radiative Heating Relations for
//!   Earth and Mars Entries", J. Spacecraft and Rockets 28(1), 1991:
//!   q̇ = C·Rn^a·ρ^b·f(V) (W/cm², Rn m, ρ kg/m³, V m/s). Earth: C =
//!   4.736e4, b = 1.22, a = 1.072e6·V^−1.88·ρ^−0.325 (at most 1), f(V)
//!   tabulated 9–16 km/s (fitted for ρ 6.7e-5–6.3e-4, Rn 0.3–3 m); Mars
//!   (97 % CO₂): C = 2.35e4, a = 0.526, b = 1.19, f(V) 6–9 km/s. Stated
//!   accuracy ±20–30 % in range (equilibrium, no radiative cooling). The
//!   Mars constants match West & Brandis (above; fitted 6.5–9 km/s, ρ
//!   1e-4–1e-3, Rn 1–23 m); the Earth constants and both f(V) tables are
//!   from memory and unchecked (the 1991 paper is not on NTRS).
//!   Added to the convective flux and spread over the cells the same way.
//! * **Distribution:** each cell receives q̇·g, g = f·sin^1.5 θ (a Lees-type
//!   falloff from the stagnation point with the local incidence; f the
//!   cell's exposure), and at least [`LEEWARD_FRACTION`] of q̇ (leeward and
//!   shadowed cells, base flow heating).
//! * **Sunlight:** a flux vector S (W/m², the direction light travels, body
//!   axes); a cell facing it absorbs ε·|S|·cos·A, shadowed by the craft
//!   itself through the same exposure bake (parallel light is a flow
//!   direction). The absorptivity is the emissivity (grey skin) until the
//!   craft data carries a solar absorptivity.

use crate::aero::AeroBake;
use crate::craft::Cell;
use glam::DVec3;

/// Sutton–Graves constant for Earth air (SI: kg^½·m⁻¹ units, q̇ in W/m²).
pub const SUTTON_GRAVES_EARTH: f64 = 1.7415e-4;
/// Sutton–Graves constant for Mars (CO₂/N₂).
pub const SUTTON_GRAVES_MARS: f64 = 1.9027e-4;
/// Tauber & Sutton's radiative heating relation for one atmosphere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TauberSutton {
    Earth,
    Mars,
}

/// Earth f(V) (m/s, W/cm² units of the relation). The points 9, 10, …, 16
/// km/s as reproduced by Brandis & Johnston, "Characterization of
/// Stagnation-Point Heat Flux for Earth Entry", AIAA 2014-2374.
const EARTH_F: [(f64, f64); 19] = [
    (9000.0, 1.5),
    (9250.0, 4.3),
    (9500.0, 9.7),
    (9750.0, 19.5),
    (10000.0, 35.0),
    (10250.0, 55.0),
    (10500.0, 81.0),
    (10750.0, 115.0),
    (11000.0, 151.0),
    (11500.0, 238.0),
    (12000.0, 359.0),
    (12500.0, 495.0),
    (13000.0, 660.0),
    (13500.0, 850.0),
    (14000.0, 1065.0),
    (14500.0, 1313.0),
    (15000.0, 1550.0),
    (15500.0, 1780.0),
    (16000.0, 2040.0),
];

/// Mars (97 % CO₂, 3 % N₂) f(V).
const MARS_F: [(f64, f64); 17] = [
    (6000.0, 0.2),
    (6150.0, 1.0),
    (6300.0, 1.95),
    (6500.0, 3.42),
    (6700.0, 5.1),
    (6900.0, 7.1),
    (7000.0, 8.1),
    (7200.0, 10.2),
    (7400.0, 12.5),
    (7600.0, 14.8),
    (7800.0, 17.1),
    (8000.0, 19.2),
    (8200.0, 21.4),
    (8400.0, 24.1),
    (8600.0, 26.0),
    (8800.0, 28.9),
    (9000.0, 32.8),
];

/// f(V) of a table: zero below its first speed, linear between entries,
/// the last interval continued above.
fn velocity_function(table: &[(f64, f64)], v: f64) -> f64 {
    if v < table[0].0 {
        return 0.0;
    }
    let k = table.partition_point(|e| e.0 <= v).clamp(1, table.len() - 1);
    let (a, b) = (table[k - 1], table[k]);
    a.1 + (b.1 - a.1) * (v - a.0) / (b.0 - a.0)
}

/// Stagnation-point radiative heat flux (W/m²) by Tauber–Sutton at density
/// `rho` (kg/m³), nose radius `nose_radius` (m) and airspeed `speed` (m/s).
pub fn tauber_sutton(model: TauberSutton, rho: f64, nose_radius: f64, speed: f64) -> f64 {
    if rho <= 0.0 || nose_radius <= 0.0 {
        return 0.0;
    }
    let pow = |x: f64, e: f64| crate::math::exp(e * crate::math::ln(x));
    let (c, a, b, f) = match model {
        TauberSutton::Earth => {
            let a = (1.072e6 * pow(speed, -1.88) * pow(rho, -0.325)).min(1.0);
            (4.736e4, a, 1.22, velocity_function(&EARTH_F, speed))
        }
        TauberSutton::Mars => (2.35e4, 0.526, 1.19, velocity_function(&MARS_F, speed)),
    };
    if f <= 0.0 {
        return 0.0;
    }
    c * pow(nose_radius, a) * pow(rho, b) * f * 1e4
}

/// Heating of leeward and shadowed cells as a fraction of the stagnation flux.
pub const LEEWARD_FRACTION: f64 = 0.03;

/// Stagnation-point convective heat flux (W/m²) at density `rho` (kg/m³),
/// nose radius `nose_radius` (m) and airspeed `speed` (m/s).
pub fn sutton_graves(k: f64, rho: f64, nose_radius: f64, speed: f64) -> f64 {
    if rho <= 0.0 || nose_radius <= 0.0 {
        return 0.0;
    }
    k * (rho / nose_radius).sqrt() * speed * speed * speed
}

/// Fraction of the stagnation flux a cell receives: exposure × sin^1.5 θ,
/// at least [`LEEWARD_FRACTION`].
pub fn heating_shape(sin_theta: f64, exposure: f64) -> f64 {
    let s = sin_theta.max(0.0);
    (exposure * s * s.sqrt()).max(LEEWARD_FRACTION)
}

/// Inputs of [`cell_heat`] (body axes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeatInput {
    /// Direction the air moves relative to the craft (unit).
    pub flow_dir: DVec3,
    /// Stagnation convective flux (W/m²), e.g. [`sutton_graves`].
    pub q_stag: f64,
    /// Sunlight flux vector (W/m², along the light); zero in shadow.
    pub sun: DVec3,
}

/// Power absorbed by each cell (W) into `out`; `scratch` is a per-cell
/// buffer (exposures).
pub fn cell_heat(bake: &AeroBake, cells: &[Cell], input: &HeatInput, scratch: &mut [f64], out: &mut [f64]) {
    out.fill(0.0);
    if input.q_stag > 0.0 && input.flow_dir.length_squared() > 0.0 {
        let d = input.flow_dir.normalize();
        bake.exposure_at(d, scratch);
        for ((o, c), &f) in out.iter_mut().zip(cells).zip(scratch.iter()) {
            *o += input.q_stag * heating_shape(-c.normal.dot(d), f) * c.area;
        }
    }
    let flux = input.sun.length();
    if flux > 0.0 {
        let l = input.sun / flux;
        bake.exposure_at(l, scratch);
        for ((o, c), &f) in out.iter_mut().zip(cells).zip(scratch.iter()) {
            let cos = -c.normal.dot(l);
            if cos > 0.0 {
                *o += c.emissivity * flux * cos * f * c.area;
            }
        }
    }
}
