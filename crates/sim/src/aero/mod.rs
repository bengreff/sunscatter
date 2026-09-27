//! Aerodynamics on the craft's surface cells (D061, realism-1 §5a,
//! `docs/design/aero-thermal.md`).
//!
//! [`bake`] runs once per craft design; [`aero_forces`] runs per tick. Both
//! are pure. The runtime blends three regimes:
//!
//! * **Hypersonic continuum (M ≥ 5):** modified Newtonian, Cp = Cp,max·sin²θ
//!   on exposed cells, Cp,max from the Rayleigh pitot formula ([`air::cp_max`])
//!   with the effective γ of equilibrium air behind the shock at the
//!   flight speed ([`air::real_gas_gamma`]: Cp,max 1.84 → ≈ 1.93 at
//!   11 km/s).
//! * **Subsonic (M < 0.8):** pressure drag q·Cd₀·A along the flow (A the
//!   projected area, Cd₀ a craft property; blunt bodies ≈ 0.8): the Newtonian pressure
//!   distribution scaled to that drag, so the centre of pressure is the
//!   shape's. Where the scale would exceed Cp,max (slender shapes), the
//!   normal force stays Newtonian and the extra drag acts at the Newtonian
//!   drag lever.
//! * **Transonic and supersonic:** the wave drag of the area distribution
//!   along the flow ([`wave`], von Kármán's slender-body integral, capped
//!   for blunt shapes) joins the Cd₀ drag from M 0.8, fully from M 1;
//!   **1.2–5:** linear in M to the hypersonic force and moment.
//! * **Slender bodies and fins** ([`slender`]), full to M 4 and faded out
//!   by M 5 (where Newtonian takes over): where the craft is slender along
//!   the flow, the slender-body normal force (Munk's potential term and
//!   Allen–Perkins crossflow, per section along the long axis) with the
//!   drag as an axial force replaces the distribution's force; fins add
//!   finite-wing lift.
//! * **Skin friction** ([`friction`]) on the wetted cells, in every
//!   continuum regime: a compressible flat plate per cell.
//! * **Rarefied:** free molecular (fully accommodating, cold wall: the
//!   incoming momentum is absorbed, force 2·q·A along the flow), bridged in
//!   the Knudsen number by Wilmoth's formula ([`bridge`]).
//!
//! Forces and moments are in body axes; moments are about the craft origin
//! (the caller moves them to the centre of mass: τ_cm = τ − r_cm × F).

pub mod air;
pub mod bake;
pub mod friction;
pub mod geodesic;
pub mod slender;
pub mod wave;

pub use bake::{bake, AeroBake, BakeOptions, CellGeometry, DirSums};

use crate::math;
use glam::DVec3;

/// Hypersonic from this Mach number.
pub const HYPERSONIC_MACH: f64 = 5.0;
/// Slender-body and fin lift at full strength up to this Mach number,
/// fading to none at [`HYPERSONIC_MACH`].
pub const LIFT_FULL_MACH: f64 = 4.0;

/// The free stream seen by the craft.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    /// Reynolds number per metre ρV/μ (1/m); zero: no skin friction.
    pub reynolds_per_m: f64,
    /// Free-stream static temperature (K).
    pub temperature: f64,
    /// Airspeed (m/s), for the real-gas stagnation pressure; zero: a
    /// perfect gas.
    pub speed: f64,
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

/// Weight of the attached-flow lift models (slender body, fins): 1 to
/// [`LIFT_FULL_MACH`], 0 from [`HYPERSONIC_MACH`], linear between.
fn lift_weight(mach: f64) -> f64 {
    ((HYPERSONIC_MACH - mach) / (HYPERSONIC_MACH - LIFT_FULL_MACH)).clamp(0.0, 1.0)
}

/// Continuum pressure force and moment per unit q (friction apart).
fn continuum(bake: &AeroBake, s: &DirSums, d: DVec3, flow: &Flow, cd0: f64) -> (DVec3, DVec3) {
    let (mach, gamma) = (flow.mach, flow.gamma);
    let newtonian = |cp: f64| (s.newton * cp, s.newton_moment * cp);
    // Newtonian with the equilibrium-air stagnation pressure at entry speeds.
    let g_eff = air::real_gas_gamma(gamma, flow.speed);
    if mach >= HYPERSONIC_MACH {
        return newtonian(air::cp_max(g_eff, mach));
    }
    // Subsonic: the Newtonian distribution scaled so its drag is Cd₀·A
    // (the centre of pressure stays the shape's); where that scale exceeds
    // Cp,max (slender shapes), lift stays Newtonian and the extra drag acts
    // at the Newtonian drag lever.
    // Transonic and supersonic: the wave drag of the area distribution
    // along the flow joins the drag, at the same lever.
    let drag_n = s.newton.dot(d);
    let wave = wave::mach_factor(mach) * bake.wave_at(d).min(wave::WAVE_CAP * s.area);
    let kappa = if drag_n > 1e-12 { (cd0 * s.area + wave) / drag_n } else { 0.0 };
    let k_lift = kappa.min(air::cp_max_limit(gamma));
    let extra = kappa - k_lift;
    let sub = (s.newton * k_lift + d * (drag_n * extra), s.newton_moment * k_lift + s.drag_lever.cross(d) * extra);
    let (mut f, mut m) = if mach < 1.2 {
        sub
    } else {
        let hyper = newtonian(air::cp_max(g_eff, HYPERSONIC_MACH));
        let w = (mach - 1.2) / (HYPERSONIC_MACH - 1.2);
        (sub.0 + (hyper.0 - sub.0) * w, sub.1 + (hyper.1 - sub.1) * w)
    };
    let lift = lift_weight(mach);
    if lift == 0.0 {
        return (f, m);
    }
    // Slender along the flow: the slender-body normal force, and the drag
    // as an axial force at the drag's centre, replace the distribution.
    let fineness = bake.extent_at(d) / (2.0 * (s.area / math::PI).sqrt()).max(1e-9);
    let sw = slender::slenderness(fineness) * lift;
    if sw > 0.0 && drag_n > 1e-12 {
        let drag = f.dot(d);
        let aft = if d.dot(bake.sections.axis) >= 0.0 { bake.sections.axis } else { -bake.sections.axis };
        let axial = aft * drag;
        let (bf, bm) = slender::body_normal(&bake.sections, d);
        let at = s.drag_lever / drag_n;
        f += (bf + axial - f) * sw;
        m += (bm + at.cross(axial) - m) * sw;
    }
    for fin in &bake.fins {
        let (ff, fm) = slender::fin_normal(fin, &bake.geometry, d, mach);
        f += ff * lift;
        m += fm * lift;
    }
    (f, m)
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
    let (f_cont, m_cont) = continuum(bake, &s, d, flow, cd0);
    let plate = friction::Plate::new(flow.reynolds_per_m, flow.mach, flow.gamma, flow.temperature);
    let (f_fr, m_fr) = friction::friction(&bake.geometry, d, &plate);
    let (f_cont, m_cont) = (f_cont + f_fr, m_cont + m_fr);
    let f_fm = d * (2.0 * s.area);
    let m_fm = s.area_moment.cross(d) * 2.0;
    let b = bridge(flow.knudsen);
    let f = f_cont + (f_fm - f_cont) * b;
    let m = m_cont + (m_fm - m_cont) * b;
    (f * flow.q, m * flow.q)
}

#[cfg(test)]
mod lift_tests;
#[cfg(test)]
mod tests;
