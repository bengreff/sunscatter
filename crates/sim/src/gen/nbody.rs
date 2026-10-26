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

/// Acceleration at `r` (relative to the oblate body's centre) from its J2 term.
pub fn j2_accel(r: DVec3, gm: f64, j2: f64, radius: f64, pole: DVec3) -> DVec3 {
    let d2 = r.length_squared();
    let d = d2.sqrt();
    let z = r.dot(pole);
    let k = -1.5 * j2 * gm * radius * radius / (d2 * d2 * d);
    let zr2 = 5.0 * z * z / d2;
    (r * (1.0 - zr2) + pole * (2.0 * z)) * k
}

/// Acceleration at `r` (relative to the oblate body's centre) from its J4
/// and J6 terms: the gradient of the potential's zonal part
/// `V_n = −(μ/r) J_n (R/r)^n P_n(z/r)` (with `V = μ/r + Σ V_n`, `a = ∇V`;
/// with [`j2_accel`], the zonal field to degree 6).
pub fn j4_j6_accel(r: DVec3, gm: f64, j4: f64, j6: f64, radius: f64, pole: DVec3) -> DVec3 {
    let d2 = r.length_squared();
    let d = d2.sqrt();
    let s = r.dot(pole) / d; // sine of the latitude
    let s2 = s * s;
    let q = radius * radius / d2;
    // a_n = ∇V_n = u J_n q^(n/2) [(n+1) P_n(s) r̂ − P_n'(s) (p̂ − s r̂)],
    // u = μ/r² (for n = 2 this is j2_accel: inward at the equator).
    let p4 = (35.0 * s2 * s2 - 30.0 * s2 + 3.0) / 8.0;
    let dp4 = (140.0 * s2 * s - 60.0 * s) / 8.0;
    let p6 = (231.0 * s2 * s2 * s2 - 315.0 * s2 * s2 + 105.0 * s2 - 5.0) / 16.0;
    let dp6 = (1386.0 * s2 * s2 * s - 1260.0 * s2 * s + 210.0 * s) / 16.0;
    let (c4, c6) = (j4 * q * q, j6 * q * q * q);
    let radial = 5.0 * c4 * p4 + 7.0 * c6 * p6;
    let polar = -(c4 * dp4 + c6 * dp6);
    let u = gm / d2;
    let rhat = r / d;
    (rhat * radial + (pole - rhat * s) * polar) * u
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn j2_matches_textbook_components() {
        // On the equator J2 pulls inward by 1.5 J2 μ R²/r⁴; over the pole it
        // pushes outward by 3 J2 μ R²/r⁴ (less mass under the poles).
        let (gm, j2, rad) = (3.986e14, 1.0826e-3, 6.378e6);
        let r = 7.0e6;
        let eq = j2_accel(DVec3::new(r, 0.0, 0.0), gm, j2, rad, DVec3::Z);
        let pol = j2_accel(DVec3::new(0.0, 0.0, r), gm, j2, rad, DVec3::Z);
        let base = j2 * gm * rad * rad / r.powi(4);
        assert!((eq.x + 1.5 * base).abs() < 1e-12 * base.abs().max(1.0));
        assert!((pol.z - 3.0 * base).abs() < 1e-12 * base.abs().max(1.0));
    }

    /// J2, J4 and J6 are the gradient of the zonal potential
    /// `V_n = −(μ/r) J_n (R/r)^n P_n(z/r)` (acceleration `∇V`, the sign under
    /// which J2 pulls inward at the equator), by finite differences: one
    /// convention for all three, so J4 and J6 cannot have J2's sign flipped.
    #[test]
    fn zonal_terms_are_the_gradient_of_the_zonal_potential() {
        let (gm, rad, j2, j4, j6) = (1.266_865_3e17, 7.1492e7, 1.4697e-2, -5.866e-4, 3.42e-5);
        let pole = DVec3::new(0.1, -0.2, 1.0).normalize();
        let potential = |r: DVec3, jn: [f64; 3]| {
            let d = r.length();
            let s = r.dot(pole) / d;
            let p2 = (3.0 * s * s - 1.0) / 2.0;
            let p4 = (35.0 * s.powi(4) - 30.0 * s * s + 3.0) / 8.0;
            let p6 = (231.0 * s.powi(6) - 315.0 * s.powi(4) + 105.0 * s * s - 5.0) / 16.0;
            let q = rad / d;
            -(gm / d) * (jn[0] * q.powi(2) * p2 + jn[1] * q.powi(4) * p4 + jn[2] * q.powi(6) * p6)
        };
        let grad = |r: DVec3, jn: [f64; 3]| {
            let h = 1.0;
            let f = |dr: DVec3| potential(r + dr * h, jn) - potential(r - dr * h, jn);
            DVec3::new(f(DVec3::X), f(DVec3::Y), f(DVec3::Z)) / (2.0 * h)
        };
        for r in [DVec3::new(4.2e8, 1.0e7, 3.0e7), DVec3::new(1.0e8, -5.0e7, 6.0e7), DVec3::new(0.0, 0.0, 9.0e7)] {
            let a2 = j2_accel(r, gm, j2, rad, pole);
            let g2 = grad(r, [j2, 0.0, 0.0]);
            assert!((a2 - g2).length() < 1e-6 * g2.length(), "J2 {a2} vs {g2}");
            let a46 = j4_j6_accel(r, gm, j4, j6, rad, pole);
            let g46 = grad(r, [0.0, j4, j6]);
            assert!((a46 - g46).length() < 1e-5 * g46.length(), "J4/J6 {a46} vs {g46}");
        }
    }
}
