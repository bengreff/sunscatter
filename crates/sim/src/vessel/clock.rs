//! The ship clock (D012, realism-1 §4a): δ = τ − t, proper time minus the
//! game clock (coordinate time), integrated at
//! [`crate::relativity::proper_time_rate`].
//!
//! * Coasts and planned burns carry δ in their segments (the integrator's
//!   extra scalar, outside error control).
//! * Live ticks add each tick's increment (the tick integrator's extra
//!   scalar, or the contact substeps' rates) to a compensated sum.
//! * Landed (and crashed) vessels integrate the rate at their body-fixed
//!   position (the body's potential and motion, and its rotation) on a fixed
//!   lattice of [`LANDED_STEP`] from where they came to rest, by 3-point
//!   Gauss–Legendre per step; the partial step to the vessel's time is added
//!   on top without being stored, so any sequence of advances gives the same
//!   bits.

use crate::forces::{ActiveSources, ForceContext};
use crate::frame::{BodyFixed, NodeId, Vec3};
use crate::math::CompensatedScalar;
use crate::relativity::GAUSS3;
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// Lattice step of a landed vessel's clock (s).
pub const LANDED_STEP: f64 = 600.0;

/// A vessel's proper-time state.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShipClock {
    /// δ = τ − t at the vessel's time (s).
    pub offset: f64,
    /// δ accumulated up to `base` (compensated). Live: `base` is the
    /// vessel's time; landed: the last lattice point.
    acc: CompensatedScalar,
    base: Epoch,
}

impl ShipClock {
    /// A clock showing offset `delta` at `t`.
    pub fn at(t: Epoch, delta: f64) -> Self {
        ShipClock { offset: delta, acc: CompensatedScalar::new(delta), base: t }
    }

    /// Restarts the accumulation at `t` from the current offset (on a
    /// change of phase).
    pub fn restart(&mut self, t: Epoch) {
        *self = ShipClock::at(t, self.offset);
    }

    /// Adds a live tick's increment, ending at `t`.
    pub fn add(&mut self, t: Epoch, increment: f64) {
        self.acc.add(increment);
        self.base = t;
        self.offset = self.acc.value();
    }

    /// Advances a vessel fixed at `fixed` on `body` to `t`.
    pub fn advance_landed(&mut self, world: &World, body: NodeId, fixed: Vec3<BodyFixed>, t: Epoch) {
        while self.base.add_seconds(LANDED_STEP) <= t {
            self.acc.add(landed_integral(world, body, fixed, self.base, LANDED_STEP));
            self.base = self.base.add_seconds(LANDED_STEP);
        }
        let rest = t.seconds_since(self.base);
        let partial = if rest > 0.0 { landed_integral(world, body, fixed, self.base, rest) } else { 0.0 };
        self.offset = self.acc.value() + partial;
    }
}

/// ∫ dδ over `[t, t + dt]` at a body-fixed point (Gauss–Legendre).
fn landed_integral(world: &World, body: NodeId, fixed: Vec3<BodyFixed>, t: Epoch, dt: f64) -> f64 {
    GAUSS3.iter().map(|&(x, w)| w * landed_rate(world, body, fixed, t.add_seconds(x * dt))).sum::<f64>() * dt
}

/// `dδ/dt` at a body-fixed point at `t`: the potential of every source
/// there, and the velocity of the point (the body's, plus its rotation).
pub fn landed_rate(world: &World, body: NodeId, fixed: Vec3<BodyFixed>, t: Epoch) -> f64 {
    let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical body").rotation;
    let snap = world.snapshot(t);
    let r = rot.to_inertial(fixed, t).raw();
    let v = rot.omega(t).raw().cross(r);
    // Every source cut: only the potential is wanted (it counts all of them).
    let none = ActiveSources(Vec::new());
    ForceContext { world, anchor: body, active: &none, drag: None, thrust: DVec3::ZERO }.accel_rate(&snap, r, v).1
}
