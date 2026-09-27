//! Aerodynamics on the craft's surface cells (D061, realism-1 §5a,
//! `docs/design/aero-thermal.md`).
//!
//! [`bake`] runs once per craft design; [`aero_forces`] runs per tick. Both
//! are pure. The runtime blends three regimes:
//!
//! * **Hypersonic continuum (M ≥ 5):** modified Newtonian, Cp = Cp,max·sin²θ
//!   on exposed cells, Cp,max from the Rayleigh pitot formula ([`air::cp_max`]).
//! * **Subsonic (M < 0.8):** drag q·Cd₀·A along the flow (A the projected
//!   area, Cd₀ a craft property; blunt bodies ≈ 0.8): the Newtonian pressure
//!   distribution scaled to that drag, so the centre of pressure is the
//!   shape's. Where the scale would exceed Cp,max (slender shapes), the
//!   normal force stays Newtonian and the extra drag acts at the Newtonian
//!   drag lever.
//! * **Transonic (0.8–1.2):** the drag factor rises linearly to
//!   [`TRANSONIC_PEAK`]·Cd₀ at M 1.2 (blunt capsules: Apollo's Cd goes from
//!   ≈ 0.8 subsonic to ≈ 1.3 at M 1.2); **1.2–5:** linear in M to the
//!   hypersonic force and moment. FAR-style area-distribution drag rise
//!   and skin friction are later.
//! * **Rarefied:** free molecular (fully accommodating, cold wall: the
//!   incoming momentum is absorbed, force 2·q·A along the flow), bridged in
//!   the Knudsen number by Wilmoth's formula ([`bridge`]).
//!
//! Forces and moments are in body axes; moments are about the craft origin
//! (the caller moves them to the centre of mass: τ_cm = τ − r_cm × F).

pub mod air;
pub mod bake;
pub mod geodesic;

pub use bake::{bake, AeroBake, BakeOptions, CellGeometry, DirSums};

use crate::math;
use glam::DVec3;

/// Drag factor at M 1.2 relative to the subsonic Cd₀.
pub const TRANSONIC_PEAK: f64 = 1.6;
/// Hypersonic from this Mach number.
pub const HYPERSONIC_MACH: f64 = 5.0;

/// The free stream seen by the craft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flow {
    /// Direction the air moves relative to the craft, body axes (unit; the
    /// opposite of the craft's airspeed direction).
    pub dir: DVec3,
    /// Dynamic pressure ½ρV² (Pa).
    pub q: f64,
    pub mach: f64,
    /// Mean free path / craft length.
    pub knudsen: f64,
    /// Ratio of specific heats of the air.
    pub gamma: f64,
}

/// Wilmoth's rarefied bridging weight: C = C_cont + (C_fm − C_cont)·b with
/// b = sin²[π(3/8 + ⅛·log₁₀ Kn)] for 10⁻³ < Kn < 10, 0 below, 1 above.
/// Wilmoth, Mitcheltree & Moss, "Low-Density Aerodynamics of the Stardust
/// Sample Return Capsule", J. Spacecraft and Rockets 36(3), 1999 (AIAA
/// 97-2510); the same form is ESA DRAMA's.
pub fn bridge(knudsen: f64) -> f64 {
    if knudsen.is_nan() || knudsen <= 1e-3 {
        0.0
    } else if knudsen >= 10.0 {
        1.0
    } else {
        let s = math::sin(math::PI * (0.375 + 0.125 * math::ln(knudsen) / math::ln(10.0)));
        s * s
    }
}

/// Subsonic drag factor on Cd₀: 1 below M 0.8, rising linearly to
/// [`TRANSONIC_PEAK`] at M 1.2.
fn transonic_factor(mach: f64) -> f64 {
    if mach < 0.8 {
        1.0
    } else {
        1.0 + (TRANSONIC_PEAK - 1.0) * ((mach - 0.8) / 0.4).min(1.0)
    }
}

/// Continuum force and moment per unit q.
fn continuum(s: &DirSums, d: DVec3, mach: f64, gamma: f64, cd0: f64) -> (DVec3, DVec3) {
    let newtonian = |cp: f64| (s.newton * cp, s.newton_moment * cp);
    if mach >= HYPERSONIC_MACH {
        return newtonian(air::cp_max(gamma, mach));
    }
    // Subsonic: the Newtonian distribution scaled so its drag is Cd₀·A
    // (the centre of pressure stays the shape's); where that scale exceeds
    // Cp,max (slender shapes), lift stays Newtonian and the extra drag acts
    // at the Newtonian drag lever.
    let drag_n = s.newton.dot(d);
    let kappa = if drag_n > 1e-12 { transonic_factor(mach) * cd0 * s.area / drag_n } else { 0.0 };
    let k_lift = kappa.min(air::cp_max_limit(gamma));
    let extra = kappa - k_lift;
    let sub = (s.newton * k_lift + d * (drag_n * extra), s.newton_moment * k_lift + s.drag_lever.cross(d) * extra);
    if mach < 1.2 {
        return sub;
    }
    let hyper = newtonian(air::cp_max(gamma, HYPERSONIC_MACH));
    let w = (mach - 1.2) / (HYPERSONIC_MACH - 1.2);
    (sub.0 + (hyper.0 - sub.0) * w, sub.1 + (hyper.1 - sub.1) * w)
}

/// Aerodynamic force and moment (about the craft origin), body axes, at
/// the flow's direction. `cd0` is the craft's subsonic drag coefficient on
/// its projected area (a craft-data field later).
pub fn aero_forces(bake: &AeroBake, flow: &Flow, cd0: f64) -> (DVec3, DVec3) {
    if flow.q <= 0.0 || flow.dir.length_squared() == 0.0 {
        return (DVec3::ZERO, DVec3::ZERO);
    }
    let d = flow.dir.normalize();
    let s = bake.sums_at(d);
    let (f_cont, m_cont) = continuum(&s, d, flow.mach, flow.gamma, cd0);
    let f_fm = d * (2.0 * s.area);
    let m_fm = s.area_moment.cross(d) * 2.0;
    let b = bridge(flow.knudsen);
    let f = f_cont + (f_fm - f_cont) * b;
    let m = m_cont + (m_fm - m_cont) * b;
    (f * flow.q, m * flow.q)
}

#[cfg(test)]
mod tests;
