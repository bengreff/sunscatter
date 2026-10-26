//! Advancing a coasting vessel: its trajectory, attitude and the coast
//! thermal lattice, within a work budget ([`Vessel::advance`]).
//!
//! The attitude follows the controls' hold ([`super::hold`]) in coasts and
//! is read from the stored burn in planned burns. From a planned burn's
//! turn lead before ignition the maneuver hold takes over (D075), and at
//! ignition the vessel checks the burn was predicted from the attitude it
//! has ([`Trajectory::refly`]).
//!
//! Perturbed coasts ([`super::limbo`]): while a segment's pointing knows
//! the attitude ([`super::pointing`]), the attitude is read from it; while
//! it does not (a turn, rotation input), the controller flies it as above.
//! The coast restarts from the current state when the regime changes: a
//! command (checked at every advance), the control tick where a turn
//! settles, a burn's end, the start of a planned burn's turn. Those epochs
//! do not depend on frames, so the result does not either.

use super::aerothermal;
use super::attitude::{advance_coast, needs_ticks, Attitude, AttitudeControl, RotationBase, FOLLOW_STEP};
use super::burn::{BurnLaw, DirectionLaw, FlightPlan};
use super::clock::ShipClock;
use super::hold::{aim, hold_inputs, maneuver_direction, Aim, AimKey, HoldMode, LazySnapshot};
use super::limbo::Limbo;
use super::pointing::{self, Pointing, Rule};
use super::segment::{EndKind, SegmentEnd, SegmentKind};
use super::{Controls, Phase, Trajectory, Vessel, TICK};
use crate::craft::CraftParams;
use crate::time::Epoch;
use crate::world::World;
use glam::{DMat3, DVec3};

/// Budget units of one coast thermal solve (~200 µs against ~7 µs per
/// integration step; [`Vessel::advance`]).
pub const THERMAL_SOLVE_UNITS: usize = 30;

/// What a coasting advance did.
pub(super) struct Coasted {
    /// The time moved, or the coast restarted.
    pub progressed: bool,
    /// It went live at the live boundary (an atmosphere).
    pub entered: bool,
}

/// What is left of an [`Vessel::advance`] call's work budget.
pub(super) struct Budget {
    /// Coast integration steps.
    pub steps: usize,
    /// Coast thermal work units.
    pub thermal: usize,
}

impl Vessel {
    /// The perturbation model of this vessel's segments, holding `p`.
    pub(super) fn limbo_model(&self, p: Pointing) -> Limbo {
        let mut l = Limbo::new(self.craft.id.clone(), self.craft.design.clone(), p, self.craft.mass.dry_mass);
        l.optics = self.limbo.optics;
        l.boil_off = if self.debug { 0.0 } else { self.systems.consumables.boil_off };
        l.boundary = self.limbo.boundary;
        l.chute_area = self.chute.cd_area(&self.craft.chute);
        l
    }

    /// Returns whether the vessel made progress (its time moved, or its
    /// coast restarted) and whether it went live at the live boundary.
    pub(super) fn advance_coasting(
        &mut self,
        world: &World,
        target: Epoch,
        controls: &Controls,
        budget: &mut Budget,
    ) -> Coasted {
        if self.engine_running(controls) {
            self.thermal_to_now(world);
            let (anchor, r, v) = self.state(world);
            self.phase = Phase::Powered { anchor, r, v };
            self.tick_accel = None;
            self.control.base = None;
            return Coasted { progressed: true, entered: false };
        }
        // Armed now, opened by the first live tick (no air to fill it here).
        self.chute.armed |= controls.chute;
        let restarted = self.repoint(world, controls);
        let before = self.time;
        let entered = self.coast_until(world, target, controls, budget);
        Coasted { progressed: restarted || self.time != before, entered }
    }

    /// Restarts the coast if the attitude regime the controls ask for is
    /// not the current coast's pointing: a known pointing whose rule no
    /// longer holds, or an unknown one where the attitude is now known.
    /// Returns whether it restarted.
    fn repoint(&mut self, world: &World, controls: &Controls) -> bool {
        let Phase::Coasting { trajectory } = &self.phase else { return false };
        let seg = trajectory.current();
        if !matches!(seg.kind, SegmentKind::Coast) || seg.t0.seconds_since(self.time) > 0.0 {
            return false;
        }
        let Some(state) = trajectory.eval(self.time) else { return false };
        let Some(current) = seg.limbo().map(|l| &l.pointing) else {
            // A coast from before perturbed coasts (an older save): restart it.
            self.start_coast_pointing(world, state, Pointing::Mean);
            return true;
        };
        let inertia_now = self.mass_props().inertia;
        let fly = Rails { world, trajectory, plan: &self.plan, craft: &self.craft, controls, inertia_now };
        let rule = fly.rule_at(self.time, state.0);
        if !self.repoint && current.is_known() && current.rule() == rule {
            return false;
        }
        let aim = fly.aim_at(controls, self.time);
        let eff = fly.effective(controls, self.time);
        let snap = world.snapshot(self.time);
        let lawful =
            pointing::lawful(&rule, (&self.attitude, &self.control), &eff, &aim, world, &snap, state, inertia_now);
        if !self.repoint && !current.is_known() && lawful.is_none() {
            return false;
        }
        let p = lawful.unwrap_or(Pointing::Mean);
        if let Some(a) = p.attitude(world, &snap, state.0, state.1, state.2) {
            self.attitude = a;
            self.control = p.control(key_of(&aim), self.time, a, inertia_now);
        }
        self.start_coast_pointing(world, state, p);
        true
    }

    /// Returns whether it went live at the live boundary.
    fn coast_until(&mut self, world: &World, target: Epoch, controls: &Controls, budget: &mut Budget) -> bool {
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
        // Attitude and temperatures before pruning: control ticks read the
        // inertia, and the thermal lattice the position, at epochs after the
        // vessel's time, which the trajectory still holds.
        let (model, dry) = (self.craft.mass, self.craft.mass.dry_mass);
        let mass_now = dry + self.propellant;
        let inertia_at = |e: Epoch| model.at(trajectory.mass_at(e).unwrap_or(mass_now) - dry).inertia;
        let inertia_now = model.at(self.propellant).inertia;
        let fly = Rails { world, trajectory, plan: &self.plan, craft: &self.craft, controls, inertia_now };
        let stop_settled;
        (reach, stop_settled) = fly.regime_stops(self.time, reach);
        let design = self.craft.design().clone();
        let engine = self.craft.engine;
        let heat = |law: &BurnLaw| engine.heat(law.thrust, engine.mdot_max() * law.thrust / engine.thrust_vac);
        self.thermal.record_burns(&trajectory.segments, reach, heat);
        let propellant_at = |e: Epoch| (trajectory.mass_at(e).unwrap_or(mass_now) - dry).max(0.0);
        let mut t = self.time;
        let mut stopped = false;
        let mut settled = None;
        loop {
            // The thermal and systems lattices in time order (the attitude
            // is carried forward only); systems first at a shared epoch.
            let e_systems = self.systems.next().filter(|s| *s <= self.thermal.next_lattice());
            let e = e_systems.unwrap_or_else(|| self.thermal.next_lattice());
            if e.seconds_since(reach) > 0.0 {
                break;
            }
            if budget.thermal == 0 {
                // Out of budget: the vessel stops at its last lattice point.
                stopped = true;
                break;
            }
            let fly = Rails { world, trajectory, plan: &self.plan, craft: &self.craft, controls, inertia_now };
            if e_systems.is_some() {
                settled = fly.attitude(&mut self.attitude, &mut self.control, (t, e), &inertia_at, stop_settled);
                if settled.is_some() {
                    break;
                }
                t = e;
                let (anchor, r, _) = trajectory.eval(e).expect("the lattice epoch is in the trajectory");
                self.systems.step(&Vessel::systems_env(world, e, (anchor, r), self.attitude.q, propellant_at(e)));
                budget.thermal = budget.thermal.saturating_sub(1);
                continue;
            }
            if fly.still(&self.attitude, &self.control, t, e) && self.thermal.passes(e) {
                self.thermal.pass(e);
                continue;
            }
            settled = fly.attitude(&mut self.attitude, &mut self.control, (t, e), &inertia_at, stop_settled);
            if settled.is_some() {
                break;
            }
            t = e;
            let (anchor, r, v) = trajectory.eval(e).expect("the lattice epoch is in the trajectory");
            let sun = self.attitude.q.inverse() * aerothermal::coast_sunlight(world, e, anchor, r, v);
            let solved = aerothermal::coast_step(&design, &mut self.thermal, sun, propellant_at(e), e, false);
            budget.thermal = budget.thermal.saturating_sub(if solved { THERMAL_SOLVE_UNITS } else { 1 });
        }
        if let Some(s) = settled {
            reach = if s > self.time { s } else { self.time };
        } else if stopped {
            reach = t;
        } else {
            let fly = Rails { world, trajectory, plan: &self.plan, craft: &self.craft, controls, inertia_now };
            if let Some(s) = fly.attitude(&mut self.attitude, &mut self.control, (t, reach), &inertia_at, stop_settled)
            {
                reach = if s > self.time { s } else { self.time };
                settled = Some(s);
            }
        }
        // Keep the samples back to where the next frame's control resumes:
        // a followed hold's base (up to a follow step behind), else at most
        // a tick behind (a lattice point can fall just past the frame's end).
        // Holds aim from the state there.
        let follows = self.control.settled.is_some() || self.control.waiting;
        let tick_back = reach.add_seconds(-TICK);
        let base = self.control.base.map(|b| b.epoch).and_then(|e| {
            if follows {
                (reach.seconds_since(e) < 2.0 * FOLLOW_STEP).then_some(e)
            } else {
                Some(if e > tick_back { e } else { tick_back })
            }
        });
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
        if !stopped && settled.is_none() && (entry.is_some() || near_surface) {
            // In an atmosphere or close to a surface: flown in live ticks.
            self.thermal_to_now(world);
            let state = self.state(world);
            self.go_live(state);
            return entry.is_some();
        }
        false
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

/// The key of a direction hold's aim.
fn key_of(aim: &Aim) -> Option<AimKey> {
    match aim {
        Aim::Direction { key, .. } => Some(*key),
        Aim::Stability => None,
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

    /// Where an advance from `time` must stop before `reach` for the
    /// attitude regime to be checked (`Vessel::repoint`): the next
    /// ignition (the attitude there), a coast after a burn, the start of a
    /// burn's turn from a known pointing. And whether to stop where a turn
    /// settles: the attitude is unknown and the controls ask for a rule
    /// that knows it.
    fn regime_stops(&self, time: Epoch, mut reach: Epoch) -> (Epoch, bool) {
        let tr = self.trajectory;
        if let Some(e) = tr.next_ignition(time, reach) {
            reach = e;
        }
        if let Some(e) = tr.next_coast_after_burn(time, reach) {
            reach = e;
        }
        let known = tr.segment_at(time).and_then(|s| s.limbo()).map(|l| l.pointing.is_known());
        if known == Some(true) {
            if let Some((_, start)) = self.turn(time).filter(|(_, s)| *s > time && *s < reach) {
                reach = start;
            }
        }
        let stop_settled = known == Some(false) && {
            let anchor = tr.eval(time).map_or(tr.current().initial().anchor, |s| s.0);
            matches!(self.rule_at(time, anchor), Rule::Stability | Rule::Inertial { .. } | Rule::Track(_))
        };
        (reach, stop_settled)
    }

    /// Whether `epoch` is inside a planned burn's turn (the maneuver hold
    /// then flies the craft, SAS on).
    fn turning(&self, epoch: Epoch) -> bool {
        self.turn(epoch).is_some_and(|(_, start)| epoch.seconds_since(start) >= 0.0)
    }

    /// The controls that fly the craft at `epoch`: the maneuver hold forced
    /// during a burn's turn.
    fn effective(&self, controls: &Controls, epoch: Epoch) -> Controls {
        if self.turning(epoch) {
            Controls { sas: true, hold: HoldMode::Maneuver, ..*controls }
        } else {
            *controls
        }
    }

    /// The law of the next planned burn after the segment containing `epoch`.
    fn next_burn(&self, epoch: Epoch) -> Option<&DirectionLaw> {
        let seg = self.trajectory.segment_at(epoch)?;
        self.plan.burns.get(self.plan.next_from(seg.plan_index, epoch)).map(|b| &b.law.direction)
    }

    /// The attitude regime the controls ask for at `epoch` ([`pointing::wanted`]).
    fn rule_at(&self, epoch: Epoch, anchor: crate::frame::NodeId) -> Rule {
        let eff = self.effective(self.controls, epoch);
        pointing::wanted(&eff, self.next_burn(epoch), anchor, self.craft.engine.mount_dir)
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
        let turning = self.turning(epoch);
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
    /// maneuver hold from a turn's start, with SAS on) or read from their
    /// known pointing, burns from the stored flight (or a low-thrust burn's
    /// pointing); at a burn's end the control continues from the burn's.
    /// With `stop_settled`, stops where a turn in a coast settles and
    /// returns that epoch.
    fn attitude(
        &self,
        att: &mut Attitude,
        control: &mut AttitudeControl,
        (mut t, e): (Epoch, Epoch),
        inertia_at: &impl Fn(Epoch) -> DMat3,
        stop_settled: bool,
    ) -> Option<Epoch> {
        let torque = self.craft.torque;
        while t < e {
            let seg = self.trajectory.segment_at(t)?;
            let end = seg.end.map_or(e, |x| seg.t0.add_seconds(x.t));
            let stop = if end < e { end } else { e };
            let known = seg.limbo().map(|l| &l.pointing).filter(|p| p.is_known());
            match (seg.kind, known) {
                (SegmentKind::Burn(_), _) => {
                    let local = stop.seconds_since(seg.t0);
                    if let Some(a) = seg.attitude_at(local).or_else(|| seg.limbo_attitude(self.world, local)) {
                        *att = a;
                    }
                    if stop == end {
                        if let Some((a, c)) = seg.flight_end() {
                            let base = RotationBase { epoch: end, att: a, inertia: inertia_at(end) };
                            (*att, *control) = (a, AttitudeControl { base: Some(base), ..c });
                        } else if let Some(p) = known {
                            *control = p.control(Some(AimKey::BURN), end, *att, inertia_at(end));
                        }
                    }
                    if stop == t {
                        return None;
                    }
                    t = stop;
                }
                (SegmentKind::Coast, Some(p)) => {
                    if let Some(a) = seg.limbo_attitude(self.world, stop.seconds_since(seg.t0)) {
                        *att = a;
                        *control = p.control(key_of(&self.aim_at(self.controls, stop)), stop, a, inertia_at(stop));
                    }
                    if stop == t {
                        return None;
                    }
                    t = stop;
                }
                (SegmentKind::Coast, None) => {
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
                    let inertia = (self.inertia_now, inertia_at);
                    let settled =
                        advance_coast(att, control, (t, stop), controls, torque, inertia, aim_at, stop_settled);
                    if settled.is_some() {
                        return settled;
                    }
                    t = stop;
                }
            }
        }
        None
    }
}
