//! Advancing a coasting vessel: its trajectory, attitude and the coast
//! thermal lattice, within a work budget ([`Vessel::advance`]).

use super::aerothermal;
use super::attitude::{advance_coast, needs_ticks};
use super::burn::BurnLaw;
use super::clock::ShipClock;
use super::segment::{EndKind, SegmentEnd};
use super::{Controls, Phase, Vessel};
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// Budget units of one coast thermal solve (~200 µs against ~7 µs per
/// integration step; [`Vessel::advance`]).
pub const THERMAL_SOLVE_UNITS: usize = 30;

/// What is left of an [`Vessel::advance`] call's work budget.
pub(super) struct Budget {
    /// Coast integration steps.
    pub steps: usize,
    /// Coast thermal work units.
    pub thermal: usize,
}

impl Vessel {
    pub(super) fn advance_coasting(&mut self, world: &World, target: Epoch, controls: &Controls, budget: &mut Budget) {
        if self.engine_running(controls) {
            self.thermal_to_now(world);
            let (anchor, r, v) = self.state(world);
            self.phase = Phase::Powered { anchor, r, v };
            self.tick_accel = None;
            self.control.base = None;
            return;
        }
        if controls.chute && !self.chute_deployed {
            self.chute_deployed = true;
            self.start_coast(world); // drag changed: a new segment from now
        }
        let Phase::Coasting { trajectory } = &mut self.phase else { unreachable!() };
        budget.steps -= trajectory.extend(world, &self.plan, target, budget.steps);
        let computed = trajectory.computed_until();
        let at_end = target.seconds_since(computed) >= 0.0;
        let mut reach = if at_end { computed } else { target };
        // Into an atmosphere: flown live from the entry.
        let entry = trajectory.entry_from(self.time).filter(|(e, _)| e.seconds_since(reach) <= 0.0);
        if let Some((e, _)) = entry {
            reach = e;
        }
        // Attitude and temperatures before pruning: control ticks read the
        // inertia, and the thermal lattice the position, at epochs after the
        // vessel's time, which the trajectory still holds.
        let (model, dry) = (self.craft.mass, self.craft.mass.dry_mass);
        let mass_now = dry + self.propellant;
        let inertia_at = |e: Epoch| model.at(trajectory.mass_at(e).unwrap_or(mass_now) - dry).inertia;
        let inertia_now = model.at(self.propellant).inertia;
        let torque = self.craft.torque;
        let design = self.craft.design().clone();
        let engine = self.craft.engine;
        let heat = |law: &BurnLaw| engine.heat(law.thrust, engine.mdot_max() * law.thrust / engine.thrust_vac);
        self.thermal.record_burns(&trajectory.segments, reach, heat);
        let propellant_at = |e: Epoch| (trajectory.mass_at(e).unwrap_or(mass_now) - dry).max(0.0);
        let mut t = self.time;
        let mut stopped = false;
        loop {
            let e = self.thermal.next_lattice();
            if e.seconds_since(reach) > 0.0 {
                break;
            }
            if budget.thermal == 0 {
                // Out of budget: the vessel stops at its last lattice point.
                stopped = true;
                break;
            }
            let still = self.attitude.omega == DVec3::ZERO && !needs_ticks(&self.attitude, &self.control, controls);
            if still && self.thermal.passes(e) {
                self.thermal.pass(e);
                continue;
            }
            advance_coast(&mut self.attitude, &mut self.control, (t, e), controls, torque, inertia_now, inertia_at);
            t = e;
            let (anchor, r, v) = trajectory.eval(e).expect("the lattice epoch is in the trajectory");
            let sun = self.attitude.q.inverse() * aerothermal::coast_sunlight(world, e, anchor, r, v);
            let solved = aerothermal::coast_step(&design, &mut self.thermal, sun, propellant_at(e), e, false);
            budget.thermal = budget.thermal.saturating_sub(if solved { THERMAL_SOLVE_UNITS } else { 1 });
        }
        if stopped {
            reach = t;
        } else {
            advance_coast(&mut self.attitude, &mut self.control, (t, reach), controls, torque, inertia_now, inertia_at);
        }
        trajectory.prune_before(reach);
        self.propellant = (trajectory.mass_at(reach).expect("the vessel's time is in its trajectory") - dry).max(0.0);
        let delta = trajectory.proper_time_at(reach).expect("the vessel's time is in its trajectory");
        self.clock = ShipClock::at(reach, delta);
        let last = trajectory.last();
        let near_surface = matches!(last.end, Some(SegmentEnd { kind: EndKind::Surface { .. }, .. })) && at_end;
        self.time = reach;
        if !stopped && (entry.is_some() || near_surface) {
            // In an atmosphere or close to a surface: flown in live ticks.
            self.thermal_to_now(world);
            let state = self.state(world);
            self.go_live(state);
        }
    }

    /// Steps the temperatures from their lattice epoch to the vessel's time
    /// (a coast ends between lattice points).
    pub(super) fn thermal_to_now(&mut self, world: &World) {
        if self.thermal.epoch.seconds_since(self.time) >= 0.0 {
            return;
        }
        let (anchor, r, v) = self.state(world);
        let sun = aerothermal::coast_sunlight(world, self.time, anchor, r, v);
        let design = self.craft.design().clone();
        let sun = self.attitude.q.inverse() * sun;
        aerothermal::coast_step(&design, &mut self.thermal, sun, self.propellant, self.time, true);
    }
}
