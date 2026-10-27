//! The vessel's internal systems on their lattice ([`crate::systems`]):
//! the environment at each lattice epoch from the phase the vessel is in.
//!
//! * Coasting: the stored trajectory at the epoch and the attitude carried
//!   there (`super::coast` interleaves the systems lattice with the thermal
//!   one, so the attitude is advanced in time order).
//! * Live: the state at the end of the tick that holds the epoch.
//! * Landed and crashed: fixed to the body, which turns under the Sun.

use super::{body_to_inertial, Phase, Vessel};
use crate::forces::radiation;
use crate::frame::NodeId;
use crate::systems::Env;
use crate::time::Epoch;
use crate::world::World;
use glam::{DQuat, DVec3};

impl Vessel {
    /// The systems' environment at `e` for a vessel at `(r, v)` relative to
    /// `anchor` with attitude `q` and `propellant` kg aboard.
    pub(super) fn systems_env(world: &World, e: Epoch, (anchor, r): (NodeId, DVec3), q: DQuat, propellant: f64) -> Env {
        let sun = radiation::sunlight(world, &world.snapshot(e), anchor, r);
        Env { sun, attitude: q, propellant }
    }

    /// After a live tick: the lattice points the tick reached.
    pub(super) fn systems_after_tick(&mut self, world: &World) {
        let Phase::Powered { anchor, r, .. } = self.phase else { return };
        let (q, propellant, time) = (self.attitude.q, self.propellant, self.time);
        self.systems.advance(time, |e| Vessel::systems_env(world, e, (anchor, r), q, propellant));
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
        let propellant = self.propellant;
        let def = self.ship_def();
        while let Some(e) = self.systems.next().filter(|e| e.seconds_since(target) <= 0.0) {
            let r = rot.to_inertial(fixed, e).raw();
            let q = (body_to_inertial(world, body, e) * att_fixed).normalize();
            self.systems.step(&Vessel::systems_env(world, e, (body, r), q, propellant));
            // A craft of parts' slow systems (its liquids warm on the pad).
            let v = rot.omega(e).raw().cross(r);
            let left = super::ship_tick::ship_lattice(
                self.ship.as_mut(),
                def.as_deref(),
                world,
                (e, (body, r, v), q),
                Some(body),
            );
            if let Some(left) = left {
                self.propellant = left;
            }
        }
    }
}

impl Vessel {
    /// Sets the propellant boil-off (kg/s, [`crate::systems::Consumables`]):
    /// a mass flow the trajectory integrates, so a coast restarts from now
    /// with it.
    pub fn set_boil_off(&mut self, world: &World, rate: f64) {
        self.systems.consumables.boil_off = rate.max(0.0);
        if matches!(self.phase, Phase::Coasting { .. }) {
            self.start_coast(world);
        }
    }
}
