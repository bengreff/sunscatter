//! Advancing a coasting vessel: its trajectory, attitude and the coast
//! thermal lattice, within a work budget ([`Vessel::advance`]).
//!
//! The attitude follows the controls' hold ([`super::hold`]) in coasts and
//! is read from the stored burn in planned burns. From a planned burn's
//! turn lead before ignition the maneuver hold takes over (D075), and at
//! ignition the vessel checks the burn was predicted from the attitude it
//! has ([`Trajectory::refly`]).

use super::aerothermal;
use super::attitude::{advance_coast, needs_ticks, Attitude, AttitudeControl, RotationBase, FOLLOW_STEP};
use super::burn::{BurnLaw, FlightPlan};
use super::clock::ShipClock;
use super::hold::{aim, hold_inputs, maneuver_direction, Aim, AimKey, HoldMode, LazySnapshot};
use super::segment::{EndKind, SegmentEnd, SegmentKind};
use super::{Controls, Phase, Trajectory, Vessel};
use crate::craft::CraftParams;
use crate::time::Epoch;
use crate::world::World;
use glam::{DMat3, DVec3};

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
        // At a burn's ignition: flown from the attitude the vessel has.
        let control = AttitudeControl { base: None, ..self.control };
        if trajectory.current().t0 == self.time && matches!(trajectory.current().kind, SegmentKind::Burn(_)) {
            self.attitude = trajectory.refly(world, self.attitude, control);
            self.control = control;
        }
        budget.steps -= trajectory.extend(world, &self.plan, target, budget.steps);
        let computed = trajectory.computed_until();
        let at_end = target.seconds_since(computed) >= 0.0;
        let mut reach = if at_end { computed } else { target };
        // Into an atmosphere: flown live from the entry.
        let entry = trajectory.entry_from(self.time).filter(|(e, _)| e.seconds_since(reach) <= 0.0);
        if let Some((e, _)) = entry {
            reach = e;
        }
        // Stop at the next ignition to check the attitude there.
        if let Some(e) = trajectory.next_ignition(self.time, reach) {
            reach = e;
        }
        // Attitude and temperatures before pruning: control ticks read the
        // inertia, and the thermal lattice the position, at epochs after the
        // vessel's time, which the trajectory still holds.
        let (model, dry) = (self.craft.mass, self.craft.mass.dry_mass);
        let mass_now = dry + self.propellant;
        let inertia_at = |e: Epoch| model.at(trajectory.mass_at(e).unwrap_or(mass_now) - dry).inertia;
        let inertia_now = model.at(self.propellant).inertia;
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
            let fly = Rails { world, trajectory, plan: &self.plan, craft: &self.craft, controls, inertia_now };
            if fly.still(&self.attitude, &self.control, t, e) && self.thermal.passes(e) {
                self.thermal.pass(e);
                continue;
            }
            fly.attitude(&mut self.attitude, &mut self.control, (t, e), &inertia_at);
            t = e;
            let (anchor, r, v) = trajectory.eval(e).expect("the lattice epoch is in the trajectory");
            let sun = self.attitude.q.inverse() * aerothermal::coast_sunlight(world, e, anchor, r, v);
            let solved = aerothermal::coast_step(&design, &mut self.thermal, sun, propellant_at(e), e, false);
            budget.thermal = budget.thermal.saturating_sub(if solved { THERMAL_SOLVE_UNITS } else { 1 });
        }
        if stopped {
            reach = t;
        } else {
            let fly = Rails { world, trajectory, plan: &self.plan, craft: &self.craft, controls, inertia_now };
            fly.attitude(&mut self.attitude, &mut self.control, (t, reach), &inertia_at);
        }
        // Keep the samples back to the attitude's base (a followed hold's is
        // up to a follow step behind): holds aim from the state there.
        let follows = self.control.settled.is_some() || self.control.waiting;
        let base =
            self.control.base.map(|b| b.epoch).filter(|&e| follows && reach.seconds_since(e) < 2.0 * FOLLOW_STEP);
        let seg_start = trajectory.segment_at(reach).map(|s| s.t0);
        let keep = match (base, seg_start) {
            (Some(b), Some(s)) if b < reach => {
                if b > s {
                    b
                } else {
                    s
                }
            }
            _ => reach,
        };
        trajectory.prune_before(keep);
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

/// The attitude on rails: what it needs of the vessel.
struct Rails<'a> {
    world: &'a World,
    trajectory: &'a Trajectory,
    plan: &'a FlightPlan,
    craft: &'a CraftParams,
    controls: &'a Controls,
    inertia_now: DMat3,
}

impl Rails<'_> {
    /// The next burn after the coast containing `t`: its ignition and the
    /// epoch its turn starts (the maneuver hold's lead before it).
    fn turn(&self, t: Epoch) -> Option<(Epoch, Epoch)> {
        let seg = self.trajectory.segment_at(t)?;
        if !matches!(seg.kind, SegmentKind::Coast) {
            return None;
        }
        let ignition = self.plan.burns.get(self.plan.next_from(seg.plan_index, t))?.t_start;
        let mass = self.trajectory.mass_at(t)?;
        let lead = self.trajectory.craft()?.turn_lead(mass);
        Some((ignition, ignition.add_seconds(-lead)))
    }

    /// Whether the attitude stays as it is over `(t, e)` (no rotation, no
    /// control ticks, no turn or burn): the thermal lattice may skip.
    fn still(&self, att: &Attitude, control: &AttitudeControl, t: Epoch, e: Epoch) -> bool {
        att.omega == DVec3::ZERO
            && !needs_ticks(att, control, self.controls, &self.aim_at(self.controls, t))
            && self.turn(t).is_none_or(|(_, start)| start.seconds_since(e) > 0.0)
            && self.trajectory.segment_at(t).is_some_and(|s| matches!(s.kind, SegmentKind::Coast))
    }

    /// What SAS aims at at `epoch` with `controls` (the maneuver hold once
    /// its turn has started: the key of a planned burn).
    fn aim_at(&self, controls: &Controls, epoch: Epoch) -> Aim {
        let Some(seg) = self.trajectory.segment_at(epoch) else { return Aim::Stability };
        let Some(state) = self.trajectory.eval(epoch) else { return Aim::Stability };
        let turning = self.turn(epoch).is_some_and(|(_, start)| epoch.seconds_since(start) >= 0.0);
        if !turning && (!controls.sas || controls.hold == HoldMode::Stability) {
            return Aim::Stability;
        }
        let snap = LazySnapshot::new(self.world, epoch);
        let maneuver = (turning || controls.hold == HoldMode::Maneuver)
            .then(|| maneuver_direction(self.plan, seg.plan_index, &snap, state))
            .flatten();
        let axis = self.craft.engine.mount_dir;
        match (turning, maneuver) {
            (true, Some(dir)) => Aim::Direction { dir: dir.normalize(), axis, key: AimKey::BURN },
            _ => aim(controls, &hold_inputs(self.world, &snap, state, controls, maneuver), axis),
        }
    }

    /// Carries the attitude over `(t, e)`: coasts by [`advance_coast`] (the
    /// maneuver hold from a turn's start, with SAS on), burns from the
    /// stored flight; at a burn's end the control continues from the
    /// burn's.
    fn attitude(
        &self,
        att: &mut Attitude,
        control: &mut AttitudeControl,
        (mut t, e): (Epoch, Epoch),
        inertia_at: &impl Fn(Epoch) -> DMat3,
    ) {
        let torque = self.craft.torque;
        while t < e {
            let Some(seg) = self.trajectory.segment_at(t) else { return };
            match seg.kind {
                SegmentKind::Burn(_) => {
                    let end = seg.end.map_or(e, |x| seg.t0.add_seconds(x.t));
                    let stop = if end < e { end } else { e };
                    if let Some(a) = seg.attitude_at(stop.seconds_since(seg.t0)) {
                        *att = a;
                    }
                    if stop == end {
                        if let Some((a, c)) = seg.flight_end() {
                            let base = RotationBase { epoch: end, att: a, inertia: inertia_at(end) };
                            (*att, *control) = (a, AttitudeControl { base: Some(base), ..c });
                        }
                    }
                    if stop == t {
                        return;
                    }
                    t = stop;
                }
                SegmentKind::Coast => {
                    let turn = self.turn(t).map(|(_, start)| start);
                    let forced = Controls { sas: true, hold: HoldMode::Maneuver, ..*self.controls };
                    let (stop, controls) = match turn {
                        Some(start) if start.seconds_since(t) > 0.0 => {
                            (if start < e { start } else { e }, self.controls)
                        }
                        Some(_) => (e, &forced),
                        None => (e, self.controls),
                    };
                    let aim_at = |x: Epoch| self.aim_at(controls, x);
                    advance_coast(att, control, (t, stop), controls, torque, self.inertia_now, inertia_at, aim_at);
                    t = stop;
                }
            }
        }
    }
}
