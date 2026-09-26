//! Newtonian N-body system for ephemeris generation (point masses, all pairs).

use super::InitialBody;
use crate::integrate::Separable;
use crate::math::Compensated;
use glam::DVec3;

pub struct NBody {
    gm: Vec<f64>,
    r: Vec<Compensated>,
    v: Vec<Compensated>,
}

impl NBody {
    pub fn new(bodies: &[InitialBody]) -> Self {
        NBody {
            gm: bodies.iter().map(|b| b.gm).collect(),
            r: bodies.iter().map(|b| Compensated::new(b.r)).collect(),
            v: bodies.iter().map(|b| Compensated::new(b.v)).collect(),
        }
    }

    /// Accelerations of all bodies (fixed pair order: deterministic).
    pub fn accelerations(&self) -> Vec<DVec3> {
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
        acc
    }

    /// (position, velocity, acceleration) of every body.
    pub fn kinematics(&self) -> Vec<(DVec3, DVec3, DVec3)> {
        let acc = self.accelerations();
        (0..self.gm.len()).map(|i| (self.r[i].value(), self.v[i].value(), acc[i])).collect()
    }
}

impl Separable for NBody {
    fn drift(&mut self, h: f64) {
        for (r, v) in self.r.iter_mut().zip(&self.v) {
            r.add(v.value() * h);
        }
    }

    fn kick(&mut self, h: f64) {
        let acc = self.accelerations();
        for (v, a) in self.v.iter_mut().zip(acc) {
            v.add(a * h);
        }
    }
}
