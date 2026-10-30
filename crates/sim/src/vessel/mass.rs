//! A vessel's mass, its one truth (F2 of the fidelity review;
//! ship-model.md §5.1): a craft of catalog parts holds its propellant only
//! in its ship state (each tank's liquid and ullage, each bottle's gas), so
//! its mass, its centre of mass and inertia and its thermal nodes' heat
//! capacities are derived from the tanks where their liquids sit
//! (`systems::plumbing`: the slug's drift); a made-up craft (the blocks)
//! keeps one propellant total, its segments' mass law.

use super::{Phase, Vessel};
use crate::craft::{MassModel, MassProps};
use crate::world::World;

impl Vessel {
    /// Mass at the vessel's time (kg): dry plus propellant.
    pub fn mass(&self) -> f64 {
        self.craft.mass.dry_mass + self.propellant()
    }

    /// Propellant at the vessel's time (kg): a craft of parts' consumables
    /// (its tanks' liquids and ullages, its bottles' gas), else the made-up
    /// craft's own.
    pub fn propellant(&self) -> f64 {
        match self.ship_parts() {
            Some((def, state)) => state.consumables(def),
            None => self.propellant,
        }
    }

    /// Mass, centre of mass (body axes) and inertia at the vessel's time.
    pub fn mass_props(&self) -> MassProps {
        match self.ship_parts() {
            Some((def, state)) => ship_props(&self.craft.mass, def, state),
            None => self.craft.mass.at(self.propellant),
        }
    }

    /// What its tanks hold for the thermal nodes' heat capacities: a craft
    /// of parts' tanks and bottles each by its own contents where they sit,
    /// else the made-up craft's propellant.
    pub(super) fn aboard(&self) -> crate::craft::design::Aboard {
        use crate::craft::design::Aboard;
        match self.ship_parts() {
            Some((def, state)) => Aboard::Tanks(crate::systems::plumbing::held(def, state)),
            None => Aboard::Fill(self.propellant),
        }
    }

    /// Sets a made-up craft's propellant (clamped to [0, capacity]). A
    /// coast is restarted from now with the new mass. A craft of parts has
    /// no one total to set (F2): its tanks are set one by one
    /// ([`Self::set_tank`]); here it is left as it is.
    pub fn set_propellant(&mut self, world: &World, kg: f64) {
        debug_assert!(self.ship.is_none(), "a craft of parts sets its tanks one by one (set_tank)");
        if self.ship.is_some() {
            return;
        }
        self.propellant = kg.clamp(0.0, self.craft.mass.capacity);
        if matches!(self.phase, Phase::Coasting { .. }) {
            self.start_coast(world);
        }
    }

    /// Sets the liquid of a craft of parts' tank `tank` (an index into its
    /// `ShipDef::tanks`) to `kg`, clamped to [0, its capacity], at the
    /// temperature it has; its ullage keeps its gas. A coast is restarted
    /// from now with the new mass. Returns whether there is such a tank.
    pub fn set_tank(&mut self, world: &World, tank: usize, kg: f64) -> bool {
        let Some(def) = self.ship_def() else { return false };
        self.law_to_now();
        let (Some(t), Some(state)) = (def.tanks.get(tank), self.ship.as_mut()) else { return false };
        state.tanks[tank].liquid = kg.clamp(0.0, t.capacity);
        if matches!(self.phase, Phase::Coasting { .. }) {
            self.start_coast(world);
        }
        true
    }

    /// Sets the liquid of every tank of a craft of parts to `fraction`
    /// (clamped to [0, 1]) of its capacity ([`Self::set_tank`] for each).
    pub fn set_fill(&mut self, world: &World, fraction: f64) {
        let Some(def) = self.ship_def() else { return };
        for (i, t) in def.tanks.iter().enumerate() {
            self.set_tank(world, i, fraction.clamp(0.0, 1.0) * t.capacity);
        }
    }
}

/// A craft of parts' mass properties from its ship state: the dry parts of
/// `model`, each tank's liquid where the plumbing has it and its ullage's
/// gas, each bottle's gas.
pub(crate) fn ship_props(
    model: &MassModel,
    def: &crate::craft::ship::ShipDef,
    state: &crate::craft::state::ShipState,
) -> MassProps {
    let (slugs, points) = crate::systems::plumbing::contents(def, state);
    model.with_slugs(&slugs, &points)
}
