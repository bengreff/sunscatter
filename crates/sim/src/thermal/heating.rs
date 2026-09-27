//! Heat into the skin cells: convective stagnation heating spread over the
//! exposed cells, and sunlight (realism-1 §5b).
//!
//! * **Stagnation flux:** Sutton & Graves, "A General Stagnation-Point
//!   Convective Heating Equation for Arbitrary Gas Mixtures", NASA TR R-376
//!   (1971): q̇ = k·√(ρ/Rn)·V³ (SI; k = 1.7415e-4 Earth air, 1.9027e-4
//!   Mars), cold fully catalytic wall; stated accuracy ≈ 10 % for blunt
//!   bodies. The nose radius Rn is the bake's per-direction estimate
//!   ([`crate::aero::DirSums::nose_radius`]). Radiative heating
//!   (Tauber–Sutton, above ~9 km/s) is a later step.
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
