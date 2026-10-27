//! The ship clock (D012, realism-1 §4a): δ = τ − t, proper time minus the
//! coordinate time (TDB), integrated at
//! [`crate::relativity::proper_time_rate`]; and the clock of a site on a
//! body's surface ([`SiteClock`]: the origin's, which the game shows, D088).
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

/// The clock of a fixed place on a body (a launch site): δ by the landed
/// rule ([`landed_rate`] on the [`LANDED_STEP`] lattice) from a fixed
/// epoch, so its reading is a pure function of time, whatever the frames,
/// the warp or the order of the calls. The game shows the origin's (D088):
/// started from TT at the ephemeris' first epoch, it reads TT (the geoid's
/// time) to within microseconds; ships are shown against it.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SiteClock {
    body: NodeId,
    fixed: Vec3<BodyFixed>,
    /// Where the clock was set, and δ there.
    epoch: Epoch,
    delta0: f64,
    clock: ShipClock,
}

impl SiteClock {
    /// A clock at `fixed` on `body` showing δ = `delta0` at `epoch`.
    pub fn new(body: NodeId, fixed: Vec3<BodyFixed>, epoch: Epoch, delta0: f64) -> Self {
        SiteClock { body, fixed, epoch, delta0, clock: ShipClock::at(epoch, delta0) }
    }

    /// A clock at `fixed` on `body` set to TT at `epoch`
    /// ([`crate::relativity::tdb_minus_tt`]).
    pub fn reading_tt(body: NodeId, fixed: Vec3<BodyFixed>, epoch: Epoch) -> Self {
        Self::new(body, fixed, epoch, -crate::relativity::tdb_minus_tt(epoch))
    }

    /// δ = τ − t at `t` (s). Moving forward continues the lattice; a time
    /// before the last lattice point starts again from the epoch (the same
    /// bits, at the cost of the integration).
    pub fn offset_at(&mut self, world: &World, t: Epoch) -> f64 {
        if t < self.clock.base {
            self.clock = ShipClock::at(self.epoch, self.delta0);
        }
        self.clock.advance_landed(world, self.body, self.fixed, t);
        self.clock.offset
    }

    /// δ at the last time asked for.
    pub fn offset(&self) -> f64 {
        self.clock.offset
    }
}

/// ∫ dδ over `[t, t + dt]` at a body-fixed point (Gauss–Legendre).
fn landed_integral(world: &World, body: NodeId, fixed: Vec3<BodyFixed>, t: Epoch, dt: f64) -> f64 {
    GAUSS3.iter().map(|&(x, w)| w * landed_rate(world, body, fixed, t.add_seconds(x * dt))).sum::<f64>() * dt
}

/// `dδ/dt` at a body-fixed point at `t`: the potential of every source
/// there, with the body's field to the degree a vessel there includes (as
/// a ship flying over it), and the velocity of the point (the body's, plus
/// its rotation).
pub fn landed_rate(world: &World, body: NodeId, fixed: Vec3<BodyFixed>, t: Epoch) -> f64 {
    let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical body").rotation;
    let snap = world.snapshot(t);
    let r = rot.to_inertial(fixed, t).raw();
    let v = rot.omega(t).raw().cross(r);
    let active = ActiveSources::select(world, &snap, body, r);
    ForceContext { world, anchor: body, active: &active, drag: None, thrust: DVec3::ZERO }.accel_rate(&snap, r, v).1
}
