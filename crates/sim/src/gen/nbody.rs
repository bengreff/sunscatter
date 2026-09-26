//! N-body system for ephemeris generation: Newtonian point masses (all pairs)
//! plus optional recorded extras — the Sun's first post-Newtonian correction
//! and zonal J2 of oblate bodies (with reaction). Which extras a system uses is
//! part of its generator config, so the assumption set is explicit (D026).

use super::InitialBody;
use crate::integrate::Separable;
use crate::math::Compensated;
use glam::DVec3;

/// Speed of light (m/s).
pub const C: f64 = 299_792_458.0;

/// A body whose J2 acts on every other body, about its (precessing) pole.
#[derive(Clone, Debug, PartialEq)]
pub struct Oblate {
    pub body: usize,
    pub j2: f64,
    pub radius: f64,
    pub rotation: crate::body::Rotation,
}

/// Forces beyond Newtonian point masses.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Extras {
    /// Index of the body whose 1PN (Schwarzschild) field acts on all others.
    pub relativistic_center: Option<usize>,
    pub oblate: Vec<Oblate>,
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
                let a = j2_accel(pos[k] - pos[ob.body], self.gm[ob.body], ob.j2, ob.radius, pole);
                acc[k] += a;
                acc[ob.body] -= a * (self.gm[k] / self.gm[ob.body]);
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
        if self.extras.relativistic_center.is_some() {
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
}
