//! Exact torque-free rotation of a rigid body.
//!
//! The angular momentum `L` is fixed in inertial space and the body's
//! angular velocity follows Euler's equations. Three closed forms:
//!
//! * **constant spin** — a spherical body, or a spin about a principal axis;
//! * **symmetric top** (two principal moments equal within 1e-9):
//!   `R(t) = Rot(L/I⊥ · t) · R₀ · Rot_body(s · ν t)`, `ν = ω_s (1 − I_s/I⊥)`;
//! * **asymmetric body** (Landau & Lifshitz §37): in principal axes ordered
//!   so that axis 3 is the one the angular momentum circulates about,
//!   `ω = (A₁ cn u, σ₂ A₂ sn u, σ₃ A₃ dn u)` with `u = λt + u₀`. The attitude
//!   is `R(t) = W₀ · Rot_e(ψ(t)) · G(t)`, where `G` is the smallest rotation
//!   taking the body direction of `L` to `e = σ₃ ê₃` and
//!   `ψ̇ = (2E/|L| + ω·e) / (1 + L̂·e)`, a function of `dn u` only, integrated
//!   by Gauss–Legendre over one period `2K` and a remainder
//!   (Celledoni, Fassò, Säfström & Zanna 2008 take the same route with an
//!   elliptic integral of the third kind).
//!
//! Each evaluation is a pure function of the starting state and the elapsed
//! time: nothing accumulates, so any warp and frame rate give the same bits.

use super::elliptic::{complete_k, gauss_legendre, incomplete_f, sn_cn_dn};
use super::principal_axes;
use crate::math;
use crate::vessel::{quat_from_rotvec, Attitude};
use glam::{DMat3, DQuat, DVec3};

/// Relative difference below which two principal moments count as equal.
const SYMMETRY_TOL: f64 = 1e-9;
/// Quadrature panels per period of ψ̇.
const PANELS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct FreeRotation {
    q0: DQuat,
    kind: Kind,
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    /// Constant angular velocity (inertial axes).
    Constant(DVec3),
    Symmetric {
        /// L / I⊥ (inertial axes).
        omega_l: DVec3,
        /// Symmetry axis × ν (body axes).
        body_spin: DVec3,
    },
    Asymmetric(Box<Asymmetric>),
}

#[derive(Clone, Debug, PartialEq)]
struct Asymmetric {
    /// Working axes → body axes.
    q_wb: DQuat,
    /// W₀ (see the module doc).
    w0: DQuat,
    j: DVec3,
    amp: DVec3,
    sigma2: f64,
    sigma3: f64,
    lambda: f64,
    m: f64,
    u0: f64,
    /// 2E / |L| and |L|.
    two_e_over_l: f64,
    l: f64,
    /// Period of ψ̇ in u (2K) and ∫ ψ̇ du over it.
    period: f64,
    per_period: f64,
    /// ∫₀^u₀ ψ̇ du.
    g0: f64,
}

/// The smallest rotation taking unit `n` to unit `e` (n·e > −1).
fn min_rotation(n: DVec3, e: DVec3) -> DQuat {
    let x = n.cross(e);
    DQuat::from_xyzw(x.x, x.y, x.z, 1.0 + n.dot(e)).normalize()
}

impl FreeRotation {
    /// The torque-free motion from `att` for body inertia `inertia` (about
    /// the centre of mass).
    pub fn new(att: &Attitude, inertia: &DMat3) -> Self {
        let q0 = att.q;
        let w_body = q0.inverse() * att.omega;
        if w_body == DVec3::ZERO {
            return FreeRotation { q0, kind: Kind::Constant(DVec3::ZERO) };
        }
        let (moments, axes) = principal_axes(*inertia);
        let (i1, i2, i3) = (moments.x, moments.y, moments.z);
        let wp = axes.transpose() * w_body; // principal components
        let tol = SYMMETRY_TOL * i3;
        let about_axis = |k: usize| (0..3).filter(|&i| i != k).all(|i| wp[i].abs() <= 1e-15 * wp.length());
        if (i3 - i1) <= tol || (0..3).any(about_axis) {
            return FreeRotation { q0, kind: Kind::Constant(att.omega) };
        }
        let symmetric = if i2 - i1 <= tol {
            Some((axes.z_axis, 0.5 * (i1 + i2), i3))
        } else if i3 - i2 <= tol {
            Some((axes.x_axis, 0.5 * (i2 + i3), i1))
        } else {
            None
        };
        if let Some((s, i_perp, i_s)) = symmetric {
            let w_s = s.dot(w_body);
            let l_body = (w_body - s * w_s) * i_perp + s * (i_s * w_s);
            let kind = Kind::Symmetric { omega_l: q0 * l_body / i_perp, body_spin: s * (w_s * (1.0 - i_s / i_perp)) };
            return FreeRotation { q0, kind };
        }
        let l2 = (wp * moments).length_squared();
        let two_e = wp.dot(wp * moments);
        // Working axes: principal as is if L circulates about the largest
        // moment's axis, else (3, −2, 1) so that it is axis 3 either way.
        let (q_wb, j, w) = if l2 >= two_e * i2 {
            (DQuat::from_mat3(&axes), moments, wp)
        } else {
            let m = DMat3::from_cols(axes.z_axis, -axes.y_axis, axes.x_axis);
            (DQuat::from_mat3(&m), DVec3::new(i3, i2, i1), DVec3::new(wp.z, -wp.y, wp.x))
        };
        let q_wb = q_wb.normalize();
        let pos = |x: f64| x.max(0.0);
        let amp = DVec3::new(
            (pos((two_e * j.z - l2) / (j.x * (j.z - j.x)))).sqrt(),
            (pos((two_e * j.z - l2) / (j.y * (j.z - j.y)))).sqrt(),
            (pos((l2 - two_e * j.x) / (j.z * (j.z - j.x)))).sqrt(),
        );
        let lambda = pos((j.z - j.y) * (l2 - two_e * j.x) / (j.x * j.y * j.z)).sqrt();
        let m = (pos((j.y - j.x) * (two_e * j.z - l2) / ((j.z - j.y) * (l2 - two_e * j.x)))).min(1.0 - 1e-12);
        let sigma3 = if w.z >= 0.0 { 1.0 } else { -1.0 };
        let sigma2 = sigma3 * (j.z - j.y).signum();
        let cn0 = if amp.x > 0.0 { w.x / amp.x } else { 1.0 };
        let sn0 = if amp.y > 0.0 { sigma2 * w.y / amp.y } else { 0.0 };
        let u0 = incomplete_f(math::atan2(sn0, cn0), m);
        let l = l2.sqrt();
        let e = DVec3::Z * sigma3;
        let n0 = (w * j) / l;
        let w0 = q0 * q_wb * min_rotation(n0, e).inverse();
        let period = 2.0 * complete_k(m);
        let mut a = Asymmetric {
            q_wb,
            w0,
            j,
            amp,
            sigma2,
            sigma3,
            lambda,
            m,
            u0,
            two_e_over_l: two_e / l,
            l,
            period,
            per_period: 0.0,
            g0: 0.0,
        };
        a.per_period = gauss_legendre(|u| a.psi_rate(u), 0.0, period, PANELS);
        a.g0 = a.integral(u0);
        FreeRotation { q0, kind: Kind::Asymmetric(Box::new(a)) }
    }

    /// The attitude `dt` seconds after the start.
    pub fn at(&self, dt: f64) -> Attitude {
        match &self.kind {
            Kind::Constant(w) => Attitude { q: (quat_from_rotvec(*w * dt) * self.q0).normalize(), omega: *w },
            Kind::Symmetric { omega_l, body_spin } => {
                let q = (quat_from_rotvec(*omega_l * dt) * self.q0 * quat_from_rotvec(*body_spin * dt)).normalize();
                let s = body_spin.normalize_or_zero();
                let nu = body_spin.length();
                Attitude { q, omega: *omega_l + q * s * nu }
            }
            Kind::Asymmetric(a) => a.at(dt),
        }
    }
}

impl Asymmetric {
    /// ω in working axes at Jacobi argument `u`.
    fn omega(&self, u: f64) -> DVec3 {
        let (sn, cn, dn) = sn_cn_dn(u, self.m);
        DVec3::new(self.amp.x * cn, self.sigma2 * self.amp.y * sn, self.sigma3 * self.amp.z * dn)
    }

    /// ψ̇ at `u`: (2E/L + ω·e) / (1 + L̂·e), with ω·e = A₃ dn, L̂·e = J₃A₃ dn/L.
    fn psi_rate(&self, u: f64) -> f64 {
        let dn = sn_cn_dn(u, self.m).2;
        let we = self.amp.z * dn;
        (self.two_e_over_l + we) / (1.0 + self.j.z * we / self.l)
    }

    /// ∫₀^u ψ̇ du.
    fn integral(&self, u: f64) -> f64 {
        let whole = libm::floor(u / self.period);
        let rest = u - whole * self.period;
        let panels = (libm::ceil(rest / self.period * PANELS as f64) as usize).max(1);
        whole * self.per_period + gauss_legendre(|x| self.psi_rate(x), 0.0, rest, panels)
    }

    fn at(&self, dt: f64) -> Attitude {
        let u = self.u0 + self.lambda * dt;
        let w = self.omega(u);
        let psi = (self.integral(u) - self.g0) / self.lambda;
        let n = (w * self.j) / self.l;
        let e = DVec3::Z * self.sigma3;
        let q = (self.w0 * quat_from_rotvec(e * psi) * min_rotation(n, e) * self.q_wb.inverse()).normalize();
        Attitude { q, omega: q * (self.q_wb * w) }
    }
}
