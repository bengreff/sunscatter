//! The vessel's internal systems on their lattice ([`crate::systems`]):
//! the environment at each lattice epoch from the phase the vessel is in.
//!
//! * Coasting: the stored trajectory at the epoch and the attitude carried
//!   there (`super::coast` interleaves the systems lattice with the thermal
//!   one, so the attitude is advanced in time order).
//! * Live: the state at the end of the tick that holds the epoch.
//! * Landed and crashed: fixed to the body, which turns under the Sun.

use super::{body_to_inertial, Phase, Vessel};
use crate::time::Epoch;
use crate::world::World;

impl Vessel {
    /// After a live tick: the lattice points the tick reached (its ticks
    /// stepped the ship's systems).
    pub(super) fn systems_after_tick(&mut self) {
        if matches!(self.phase, Phase::Powered { .. }) && self.systems.advance(self.time) > 0 {
            // The risks at the tick that crossed a lattice epoch (D128).
            self.watch_now();
        }
    }

    /// Landed or crashed until `target`: the lattice points on the ground.
    pub(super) fn systems_grounded(&mut self, world: &World, target: Epoch) {
        let (body, fixed, att_fixed) = match &self.phase {
            Phase::Landed { body, fixed, att_fixed } | Phase::Crashed { body, fixed, att_fixed, .. } => {
                (*body, *fixed, *att_fixed)
            }
            _ => return,
        };
        let rot = world.source(body).and_then(|s| s.physical.as_ref()).expect("physical body").rotation;
        let def = self.ship_def();
        while let Some(e) = self.systems.next().filter(|e| e.seconds_since(target) <= 0.0) {
            let r = rot.to_inertial(fixed, e).raw();
            let q = (body_to_inertial(world, body, e) * att_fixed).normalize();
            self.systems.step();
            // A craft of parts' slow systems (its liquids warm on the pad).
            let v = rot.omega(e).raw().cross(r);
            let at = (e, (body, r, v), q);
            super::ship_tick::ship_lattice(self.ship.as_mut(), def.as_deref(), world, at, Some(body), None);
            let ship = (def.as_deref(), self.ship.as_ref());
            super::warp_stop::watch(&mut self.software, &self.helm, ship, self.debug, e);
        }
    }
}

impl Vessel {
    /// Brings a craft of parts' tanks and bottles to its coast's law at the
    /// vessel's time (D119, `systems::coast_law`), between lattice points:
    /// before a command acts on them, and before the coast is left or
    /// restarted, so a new law (or the live ticks) goes on from there.
    pub(super) fn law_to_now(&mut self) {
        let Phase::Coasting { trajectory } = &self.phase else { return };
        let Some(seg) = trajectory.segment_at(self.time) else { return };
        let Some((l, law)) = seg.limbo().and_then(|l| Some((l, l.law.as_deref()?))) else { return };
        let (Some(def), Some(ship)) = (l.ship_def(), self.ship.as_mut()) else { return };
        law.apply(def, ship, self.time.seconds_since(seg.t0));
    }
}
