//! N-body system for ephemeris generation: Newtonian point masses (all pairs)
//! plus optional recorded extras — the Sun's first post-Newtonian correction,
//! zonal harmonics (J2, J4, J6) of oblate bodies (with reaction), and
//! perturbers taken from an already generated ephemeris (a moon system
//! integrated in its barycenter's frame, under the Sun's and the other
//! planets' tides). Which extras a system uses is part of its generator
//! config, so the assumption set is explicit (D026).

use super::InitialBody;
use crate::ephem::Ephemeris;
use crate::frame::NodeId;
use crate::gravity::zonal::{j2_accel, j4_j6_accel};
use crate::integrate::Separable;
use crate::math::Compensated;
use glam::DVec3;
use std::sync::Arc;

/// Speed of light (m/s).
pub const C: f64 = 299_792_458.0;

/// A body whose zonal harmonics act on every other body, about its
/// (precessing) pole. `j4` and `j6` of zero leave exactly the J2 arithmetic.
#[derive(Clone, Debug, PartialEq)]
pub struct Oblate {
    pub body: usize,
    pub j2: f64,
    pub j4: f64,
    pub j6: f64,
    pub radius: f64,
    pub rotation: crate::body::Rotation,
}

/// Perturbers that are not integrated here: nodes of an ephemeris generated
/// before (the planets, for a moon system). The system's frame origin is
/// the node `origin` (its barycenter); every body feels each perturber's
/// pull minus the pull on the origin, which moves with the perturbers as a
/// point mass of the system's total mass (the indirect term). With the
/// root as origin the frame is inertial and the pull is direct.
#[derive(Clone, Debug, PartialEq)]
pub struct External {
    pub eph: Arc<Ephemeris>,
    pub origin: NodeId,
    /// (node, GM) of each perturber, in summation order.
    pub perturbers: Vec<(NodeId, f64)>,
    /// Index into `perturbers` of the one whose 1PN field acts on every
    /// body (the Sun, for bodies integrated in the barycentric frame).
    pub relativistic: Option<usize>,
}

/// Forces beyond Newtonian point masses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extras {
    /// Index of the body whose 1PN (Schwarzschild) field acts on all others.
    pub relativistic_center: Option<usize>,
    pub oblate: Vec<Oblate>,
    pub external: Option<External>,
}

impl Extras {
    fn velocity_dependent(&self) -> bool {
        self.relativistic_center.is_some() || self.external.as_ref().is_some_and(|e| e.relativistic.is_some())
    }
}

pub struct NBody {
    /// Epoch of `t = 0`, and the current time (advanced by drifts, as time is
    /// the coordinate conjugate to the Hamiltonian in the split).
    epoch: crate::time::Epoch,
    t: f64,
    gm: Vec<f64>,
    r: Vec<Compensated>,
    v: Vec<Compensated>,
    extras: Extras,
}

impl NBody {
    pub fn new(epoch: crate::time::Epoch, bodies: &[InitialBody], extras: Extras) -> Self {
        NBody {
            epoch,
            t: 0.0,
            gm: bodies.iter().map(|b| b.gm).collect(),
            r: bodies.iter().map(|b| Compensated::new(b.r)).collect(),
            v: bodies.iter().map(|b| Compensated::new(b.v)).collect(),
            extras,
        }
    }

    /// Accelerations of all bodies at the current velocities (fixed evaluation
    /// order: deterministic).
    pub fn accelerations(&self) -> Vec<DVec3> {
        let vel: Vec<DVec3> = self.v.iter().map(Compensated::value).collect();
        let mut acc = self.position_accelerations();
        self.add_velocity_dependent(&mut acc, &vel);
        acc
    }

    /// The position-only part: point masses and J2.
    fn position_accelerations(&self) -> Vec<DVec3> {
        let n = self.gm.len();
        let pos: Vec<DVec3> = self.r.iter().map(Compensated::value).collect();
        let mut acc = vec![DVec3::ZERO; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let d = pos[j] - pos[i];
                let r2 = d.length_squared();
                let inv3 = 1.0 / (r2 * r2.sqrt());
                acc[i] += d * (self.gm[j] * inv3);
                acc[j] -= d * (self.gm[i] * inv3);
            }
        }
        for ob in &self.extras.oblate {
            let pole = ob.rotation.pole(self.epoch.add_seconds(self.t));
            for k in (0..n).filter(|&k| k != ob.body) {
                let d = pos[k] - pos[ob.body];
                let mut a = j2_accel(d, self.gm[ob.body], ob.j2, ob.radius, pole);
                if ob.j4 != 0.0 || ob.j6 != 0.0 {
                    a += j4_j6_accel(d, self.gm[ob.body], ob.j4, ob.j6, ob.radius, pole);
                }
                acc[k] += a;
                acc[ob.body] -= a * (self.gm[k] / self.gm[ob.body]);
            }
        }
        if let Some(ext) = &self.extras.external {
            let t = self.epoch.add_seconds(self.t);
            let root = ext.eph.root();
            let inertial = ext.origin == root;
            // Perturbers relative to the origin through the root, the origin
            // evaluated once (the same bits as walking from each perturber
            // when their common ancestor is the root, as for planets and a
            // moon system's barycenter).
            let origin = ext.eph.relative_r(ext.origin, root, t);
            for &(node, gm) in &ext.perturbers {
                let p = ext.eph.relative_r(node, root, t) - origin;
                let indirect = if inertial { DVec3::ZERO } else { p * (gm / (p.length_squared() * p.length())) };
                for (k, a) in acc.iter_mut().enumerate() {
                    let d = p - pos[k];
                    let d2 = d.length_squared();
                    *a += d * (gm / (d2 * d2.sqrt())) - indirect;
                }
            }
        }
        acc
    }

    /// Adds the velocity-dependent part (1PN) evaluated at velocities `vel`.
    fn add_velocity_dependent(&self, acc: &mut [DVec3], vel: &[DVec3]) {
        if let Some(s) = self.extras.relativistic_center {
            let pos: Vec<DVec3> = self.r.iter().map(Compensated::value).collect();
            for k in (0..self.gm.len()).filter(|&k| k != s) {
                acc[k] += schwarzschild_accel(pos[k] - pos[s], vel[k] - vel[s], self.gm[s]);
            }
        }
        if let Some((ext, i)) = self.extras.external.as_ref().and_then(|e| e.relativistic.map(|i| (e, i))) {
            let (node, gm) = ext.perturbers[i];
            let k_s = ext.eph.relative(node, ext.origin, self.epoch.add_seconds(self.t));
            for (k, a) in acc.iter_mut().enumerate() {
                *a += schwarzschild_accel(self.r[k].value() - k_s.r, vel[k] - k_s.v, gm);
            }
        }
    }

    /// (position, velocity, acceleration) of every body.
    pub fn kinematics(&self) -> Vec<(DVec3, DVec3, DVec3)> {
        let acc = self.accelerations();
        (0..self.gm.len()).map(|i| (self.r[i].value(), self.v[i].value(), acc[i])).collect()
    }
}

/// First post-Newtonian acceleration of a test body in a mass's static field.
pub fn schwarzschild_accel(r: DVec3, v: DVec3, gm: f64) -> DVec3 {
    let d = r.length();
    let k = gm / (C * C * d * d * d);
    (r * (4.0 * gm / d - v.length_squared()) + v * (4.0 * r.dot(v))) * k
}

impl Separable for NBody {
    fn drift(&mut self, h: f64) {
        self.t += h;
        for (r, v) in self.r.iter_mut().zip(&self.v) {
            r.add(v.value() * h);
        }
    }

    /// Velocity-dependent forces make an explicit kick non-symmetric, which
    /// shows up as secular energy drift. The kick is therefore implicit
    /// midpoint in velocity (a fixed number of fixed-point iterations keeps it
    /// deterministic); without such forces it reduces to the explicit kick.
    fn kick(&mut self, h: f64) {
        let base = self.position_accelerations();
        let v0: Vec<DVec3> = self.v.iter().map(Compensated::value).collect();
        let mut acc = base.clone();
        self.add_velocity_dependent(&mut acc, &v0);
        if self.extras.velocity_dependent() {
            for _ in 0..3 {
                let mid: Vec<DVec3> = v0.iter().zip(&acc).map(|(v, a)| *v + *a * (0.5 * h)).collect();
                acc.clone_from(&base);
                self.add_velocity_dependent(&mut acc, &mid);
            }
        }
        for (v, a) in self.v.iter_mut().zip(acc) {
            v.add(a * h);
        }
    }
}
