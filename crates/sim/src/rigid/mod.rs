//! Rigid-body rotation (realism-1 §3d): Euler's equations.
//!
//! * [`tick`] — one fixed step with a constant body torque (RK4 on the
//!   attitude quaternion and body angular velocity), for control ticks;
//!   [`tick_with`] with an attitude-dependent torque (aerodynamics).
//! * [`FreeRotation`] — the exact torque-free motion (closed form: constant
//!   spin, the symmetric top, or Jacobi elliptic functions for an
//!   asymmetric body), evaluated at any time without stepping, so coasts
//!   rotate correctly at any warp.
//! * [`principal_axes`] — Jacobi eigen-decomposition of an inertia tensor.
//!
//! Attitudes are [`Attitude`]s: `q` rotates body axes to inertial axes and
//! `omega` is the angular velocity in inertial axes. Tensors are in body
//! axes, about the centre of mass. Trig goes through `sim::math` (rule 2).

pub mod elliptic;
mod free;

pub use free::FreeRotation;

use crate::vessel::Attitude;
use glam::{DMat3, DQuat, DVec3};

/// Principal moments (ascending) and axes (columns, a proper rotation:
/// body = axes · principal) of a symmetric tensor, by cyclic Jacobi
/// rotations (square roots only: deterministic).
pub fn principal_axes(tensor: DMat3) -> (DVec3, DMat3) {
    let mut a = tensor.transpose().to_cols_array_2d(); // a[row][col]
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let off = a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2];
        let diag = a[0][0] * a[0][0] + a[1][1] * a[1][1] + a[2][2] * a[2][2];
        if off <= 1e-32 * diag {
            break;
        }
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q] == 0.0 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for k in 0..3 {
                let (x, y) = (a[k][p], a[k][q]);
                a[k][p] = c * x - s * y;
                a[k][q] = s * x + c * y;
            }
            for k in 0..3 {
                let (x, y) = (a[p][k], a[q][k]);
                a[p][k] = c * x - s * y;
                a[q][k] = s * x + c * y;
            }
            for row in &mut v {
                let (x, y) = (row[p], row[q]);
                row[p] = c * x - s * y;
                row[q] = s * x + c * y;
            }
        }
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| a[i][i].total_cmp(&a[j][j]).then(i.cmp(&j)));
    let col = |i: usize| DVec3::new(v[0][i], v[1][i], v[2][i]);
    let (c0, c1, mut c2) = (col(order[0]), col(order[1]), col(order[2]));
    if c0.cross(c1).dot(c2) < 0.0 {
        c2 = -c2;
    }
    (DVec3::new(a[order[0]][order[0]], a[order[1]][order[1]], a[order[2]][order[2]]), DMat3::from_cols(c0, c1, c2))
}

/// Angular momentum (inertial axes) of an attitude with body inertia `inertia`.
pub fn angular_momentum(att: &Attitude, inertia: &DMat3) -> DVec3 {
    att.q * (*inertia * (att.q.inverse() * att.omega))
}

/// Rotational kinetic energy.
pub fn kinetic_energy(att: &Attitude, inertia: &DMat3) -> f64 {
    let w = att.q.inverse() * att.omega;
    0.5 * w.dot(*inertia * w)
}

/// `q ⊗ (w, 0) / 2`: the quaternion rate for body angular velocity `w`.
fn q_rate(q: DQuat, w: DVec3) -> DQuat {
    q * DQuat::from_xyzw(w.x, w.y, w.z, 0.0) * 0.5
}

/// One step of `dt` with a constant body-axes `torque` (RK4 on Euler's
/// equations `I ω̇ = τ − ω × Iω` and `q̇ = q ⊗ ω / 2`, body axes).
pub fn tick(att: &Attitude, inertia: &DMat3, torque: DVec3, dt: f64) -> Attitude {
    tick_with(att, inertia, |_| torque, dt)
}

/// [`tick`] with a torque that depends on the attitude: `torque(q)` (body
/// axes) is evaluated at each RK4 stage's quaternion (not normalised), so
/// attitude-dependent torques such as aerodynamic moments are integrated to
/// the same order as the rotation.
pub fn tick_with(att: &Attitude, inertia: &DMat3, torque: impl Fn(DQuat) -> DVec3, dt: f64) -> Attitude {
    let inv = inertia.inverse();
    let w_rate = |q: DQuat, w: DVec3| inv * (torque(q) - w.cross(*inertia * w));
    let (q0, w0) = (att.q, att.q.inverse() * att.omega);
    let (k1q, k1w) = (q_rate(q0, w0), w_rate(q0, w0));
    let (q1, w1) = (q0 + k1q * (0.5 * dt), w0 + k1w * (0.5 * dt));
    let (k2q, k2w) = (q_rate(q1, w1), w_rate(q1, w1));
    let (q2, w2) = (q0 + k2q * (0.5 * dt), w0 + k2w * (0.5 * dt));
    let (k3q, k3w) = (q_rate(q2, w2), w_rate(q2, w2));
    let (q3, w3) = (q0 + k3q * dt, w0 + k3w * dt);
    let (k4q, k4w) = (q_rate(q3, w3), w_rate(q3, w3));
    let q = (q0 + (k1q + k2q * 2.0 + k3q * 2.0 + k4q) * (dt / 6.0)).normalize();
    let w = w0 + (k1w + k2w * 2.0 + k3w * 2.0 + k4w) * (dt / 6.0);
    Attitude { q, omega: q * w }
}

#[cfg(test)]
mod tests;
