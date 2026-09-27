//! Skin friction on the wetted cells (D070): a compressible flat plate per
//! cell, run length from the most upstream point of the craft along the
//! flow.
//!
//! * **Local skin-friction coefficient** (incompressible): laminar Blasius
//!   c_f = 0.664/√Re_x below the transition Reynolds number
//!   [`TRANSITION_RE`], turbulent White's c_f = 0.455/ln²(0.06·Re_x)
//!   above (F. M. White, *Viscous Fluid Flow*, 3rd ed., 2006, eq. 6-78:
//!   within a few per cent of the full law of the wall up to Re 10¹⁰).
//! * **Compressibility**, adiabatic wall (T_aw/T = 1 + r·(γ−1)/2·M², r =
//!   0.85 laminar, 0.89 turbulent): laminar by Eckert's reference
//!   temperature T*/T = 1 + 0.032·M² + 0.58·(T_aw/T − 1), the law at
//!   Re* = Re·(ρ*/ρ)/(μ*/μ) and c_f = c_f*·ρ*/ρ (Eckert, J. Aero. Sci. 22,
//!   1955); turbulent by Van Driest II ([`van_driest`]), the method
//!   Hopkins & Inouye (AIAA J. 9, 1971) found best against flat-plate and
//!   cone data. Viscosity by Sutherland's law.
//! * **Where:** every cell with flow along it, the shear along the flow's
//!   tangential direction t = d − n(n·d) with magnitude |t|; cells facing
//!   downstream more steeply than 45° (bases, the lee of blunt steps) are
//!   in separated flow and get none. Run length x = distance along the flow
//!   from the most upstream point of the cells; each cell takes the mean
//!   coefficient over its extent along the flow ([`Plate::mean`]).

use super::air;
use super::bake::CellGeometry;
use crate::math;
use glam::DVec3;

/// Laminar-to-turbulent transition Reynolds number of the local run length
/// (a smooth flat plate: 5·10⁵).
pub const TRANSITION_RE: f64 = 5.0e5;
/// Cells facing downstream more steeply than this (n·d above) are separated.
const SEPARATED: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Local incompressible skin-friction coefficient at `re` (Re_x).
pub fn cf_incompressible(re: f64) -> f64 {
    if re <= 0.0 {
        0.0
    } else if re < TRANSITION_RE {
        0.664 / re.sqrt()
    } else {
        let l = math::ln(0.06 * re);
        0.455 / (l * l)
    }
}

/// Eckert's reference-temperature factors (laminar) for Mach `mach`,
/// `gamma` and free-stream temperature `temperature` (K), adiabatic wall:
/// (ρ*/ρ, (ρ*/ρ)/(μ*/μ)), so that Re* = Re·second and c_f = c_f*(Re*)·first.
fn eckert(mach: f64, gamma: f64, temperature: f64) -> (f64, f64) {
    let m2 = mach * mach;
    let t_aw = 1.0 + LAMINAR_RECOVERY * 0.5 * (gamma - 1.0) * m2;
    let t_ref = 1.0 + 0.032 * m2 + 0.58 * (t_aw - 1.0);
    let mu_ratio = air::sutherland_viscosity(temperature * t_ref) / air::sutherland_viscosity(temperature);
    (1.0 / t_ref, 1.0 / (t_ref * mu_ratio))
}

/// Van Driest II's transformation (turbulent) for an adiabatic wall:
/// (1/F_c, F_Rx) with c_f = c_f,inc(F_Rx·Re)/F_c. With T_w/T = F =
/// 1 + r(γ−1)/2·M²: A = √((F−1)/F), F_c = (F−1)/asin²A, F_Rx =
/// (μ/μ_w)/F_c (Van Driest, J. Aero. Sci. 23, 1956; White, *Viscous Fluid
/// Flow*, §7-7).
fn van_driest(mach: f64, gamma: f64, temperature: f64) -> (f64, f64) {
    let f = 1.0 + TURBULENT_RECOVERY * 0.5 * (gamma - 1.0) * mach * mach;
    if f - 1.0 < 1e-12 {
        return (1.0, 1.0);
    }
    let asin = math::asin(((f - 1.0) / f).sqrt());
    let fc = (f - 1.0) / (asin * asin);
    let mu = air::sutherland_viscosity(temperature) / air::sutherland_viscosity(temperature * f);
    (1.0 / fc, mu / fc)
}

/// Recovery factors (√Pr laminar, ∛Pr turbulent, Pr = 0.72).
const LAMINAR_RECOVERY: f64 = 0.85;
const TURBULENT_RECOVERY: f64 = 0.89;

/// The compressible skin friction of one flow: each regime's factors,
/// computed once per evaluation.
#[derive(Clone, Copy, Debug)]
pub struct Plate {
    re_per_m: f64,
    laminar: (f64, f64),
    turbulent: (f64, f64),
}

impl Plate {
    /// Reynolds number per metre `re_per_m`, Mach, γ and free-stream
    /// temperature (K).
    pub fn new(re_per_m: f64, mach: f64, gamma: f64, temperature: f64) -> Self {
        Plate { re_per_m, laminar: eckert(mach, gamma, temperature), turbulent: van_driest(mach, gamma, temperature) }
    }

    /// Local skin-friction coefficient (on the free-stream q) at run length
    /// `x` (m).
    pub fn local(&self, x: f64) -> f64 {
        let re = self.re_per_m * x;
        if re <= 0.0 {
            0.0
        } else if re < TRANSITION_RE {
            let (scale, k) = self.laminar;
            0.664 / (re * k).sqrt() * scale
        } else {
            let (scale, k) = self.turbulent;
            let l = math::ln(0.06 * re * k);
            0.455 / (l * l) * scale
        }
    }

    /// Mean skin-friction coefficient over run lengths `a..b` (m): exact
    /// for the laminar law (∫0.664/√Re = 1.328·√Re), the local value at
    /// the middle where any of it is turbulent.
    pub fn mean(&self, a: f64, b: f64) -> f64 {
        let (ra, rb) = (self.re_per_m * a, self.re_per_m * b);
        if rb < TRANSITION_RE && rb > ra {
            let (scale, k) = self.laminar;
            1.328 * ((rb * k).sqrt() - (ra * k).sqrt()) / ((rb - ra) * k) * scale
        } else {
            self.local(0.5 * (a + b))
        }
    }
}

/// Friction force and its moment about the craft origin, per unit q (m²,
/// m³), for the flow direction `d` (unit) and the flow's [`Plate`]. A cell
/// spans run lengths x ± h, h its half extent along the flow
/// ([`CellGeometry::half_extent`]).
pub fn friction(cells: &[CellGeometry], d: DVec3, plate: &Plate) -> (DVec3, DVec3) {
    if plate.re_per_m <= 0.0 || cells.is_empty() {
        return (DVec3::ZERO, DVec3::ZERO);
    }
    let span: Vec<(f64, f64)> = cells.iter().map(|c| (c.centroid.dot(d), c.half_extent(d))).collect();
    let front = span.iter().map(|&(x, h)| x - h).fold(f64::INFINITY, f64::min);
    let (mut f, mut m) = (DVec3::ZERO, DVec3::ZERO);
    for (c, &(x, h)) in cells.iter().zip(&span) {
        let nd = c.normal.dot(d);
        if nd > SEPARATED {
            continue;
        }
        let x = x - front;
        let shear = (d - c.normal * nd) * (plate.mean((x - h).max(0.0), x + h) * c.area);
        f += shear;
        m += c.centroid.cross(shear);
    }
    (f, m)
}
