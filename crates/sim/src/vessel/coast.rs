//! Advancing a coasting vessel: its trajectory, attitude and the coast
//! thermal lattice, within a work budget ([`Vessel::advance`]).
//!
//! The attitude follows the controls' hold ([`super::hold`]); the
//! maneuver hold points along the next node (D126), which the maneuver
//! program flies live.
//!
//! Perturbed coasts ([`super::limbo`]): while a segment's pointing knows
//! the attitude ([`super::pointing`]), the attitude is read from it; while
//! it does not (a turn, rotation input), the controller flies it as above.
//! The coast restarts from the current state when the regime changes: a
//! command (checked at every advance), the control tick where a turn
//! settles, a change of the automatic reference. Those epochs do not
//! depend on frames, so the result does not either.

use super::aerothermal;
use super::attitude::{advance_coast, needs_ticks, Attitude, AttitudeControl, FOLLOW_STEP};
use super::burn::{node_direction, DirectionLaw};
use super::clock::ShipClock;
use super::hold::{aim, hold_inputs, maneuver_direction, Aim, AimKey, HoldMode, LazySnapshot};
use super::limbo::Limbo;
use super::pointing::{self, Pointing, Rule};
use super::segment::{EndKind, SegmentEnd};
use super::{Controls, Phase, Trajectory, Vessel, LIVE_STOP, TICK};
use crate::craft::design::Aboard;
use crate::time::Epoch;
use crate::world::World;
use glam::{DMat3, DVec3};

/// Budget units ([`Vessel::advance`]) are about a microsecond of work each
/// on the reference Mac (D028), so a budget bounds a frame's time whatever
/// the craft. One coast integration step (8th order, 12 force
/// evaluations): a perturbed step in low orbit takes ~18 µs with the full
/// Solar System (bare ~11 µs), and covers ~6 minutes there; the gravity
/// fields add their share, 12 × [`crate::forces::field_units`] (D090: ~100
/// units a step in a low lunar orbit, ~12 in LEO).
pub const STEP_UNITS: usize = 15;
/// Units of a systems lattice step (the sunlight with eclipses at the
/// vessel, then the systems).
pub const SYSTEMS_UNITS: usize = 5;
/// Units of a coast thermal lattice point that is passed or skipped.
pub const POINT_UNITS: usize = 1;

/// Units of a coast thermal solve of `design` with `samples` heat-input
/// samples (sunlight and air directions averaged): the network's solve,
/// ~10 sweeps of ~12 ns per unknown (`examples/bench_thermal`), and each
/// sample's heating of every cell, ~3 ns. A 300-part stress craft held
/// prograde (24 samples, 6,042 unknowns): ~1,150; the test craft ~120.
pub fn solve_units(design: &crate::craft::CraftDesign, samples: usize) -> usize {
    let cells = design.cells.len();
    10 + (cells + design.nodes()) / 8 + cells * samples.max(1) / 300
}

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
    /// Units for the lattices (thermal and systems).
    pub thermal: usize,
    /// Units spent so far (integration and lattices).
    pub spent: usize,
}

impl Budget {
    /// A budget of `units` for the integration (in steps of [`STEP_UNITS`],
    /// at least one) and as many for the lattices.
    pub fn new(units: usize) -> Self {
        Budget { steps: (units / STEP_UNITS).max(1), thermal: units, spent: 0 }
    }

    /// Integrates `trajectory` towards `target` within the integration
    /// budget. A step through a body's gravity field costs more (D090): the
    /// budget counts plain steps, so a costlier one uses several.
    fn extend(&mut self, world: &World, trajectory: &mut Trajectory, target: Epoch) {
        let fields = crate::forces::field_units(world, trajectory.current().active_sources());
        let per_step = STEP_UNITS + 12 * fields;
        let steps = trajectory.extend(world, target, (self.steps * STEP_UNITS / per_step).max(1));
        self.steps = self.steps.saturating_sub((steps * per_step).div_ceil(STEP_UNITS));
        self.spent += steps * per_step;
    }

    /// Charges `units` of lattice work.
    fn charge(&mut self, units: usize) {
        self.thermal = self.thermal.saturating_sub(units);
        self.spent = self.spent.saturating_add(units);
    }
}

impl Vessel {
    /// Advances the vessel towards `target` (never past it, nor past the
    /// ephemeris end). Returns the time actually reached: coasts stop when
    /// `budget` work units are spent (the game then holds its clock back
    /// and continues next frame; the result is the same as without a
    /// budget). A unit is about a microsecond of work on the reference Mac
    /// ([`STEP_UNITS`] per coast integration step, [`solve_units`] per
    /// coast thermal solve): the integration may spend `budget` units and
    /// the lattices as many. Live ticks are not budgeted. A coast that
    /// reaches the live boundary stops there.
    pub fn advance(&mut self, world: &World, target: Epoch, controls: &Controls, budget: usize) -> Epoch {
        self.advance_budgeted(world, target, controls, budget, false).0
    }

    /// One frame of the game ([`Self::advance`]), also returning the
    /// budget units spent; and a vessel that goes live inside the call (an
    /// atmosphere, a surface, an engine starting, a landed craft waking)
    /// with more than [`LIVE_STOP`] of it still to go stops there: the game
    /// caps the warp before it flies live ticks (D081), so no rails-warp
    /// frame flies thousands of them. The motion is the same either way.
    pub fn advance_frame(
        &mut self,
        world: &World,
        target: Epoch,
        controls: &Controls,
        budget: usize,
    ) -> (Epoch, usize) {
        self.advance_budgeted(world, target, controls, budget, true)
    }

    fn advance_budgeted(
        &mut self,
        world: &World,
        target: Epoch,
        controls: &Controls,
        budget: usize,
        stop_live: bool,
    ) -> (Epoch, usize) {
        let target = if target > world.end() { world.end() } else { target };
        // A target behind the vessel (it finished a frame ahead of the
        // clock): nothing to do, never a rewind.
        if target < self.time {
            return (self.time, 0);
        }
        // A warp stop not yet taken holds a frame's advance at its epoch
        // (the game drops the warp there, D128).
        if stop_live && self.warp_stop().is_some() {
            return (self.time, 0);
        }
        let mut budget = Budget::new(budget);
        self.systems.start_at(self.time);
        // A wreck has no engine or controls; the pilot's RCS mode gates and
        // scales the inputs (`systems::rcs::pilot`).
        let cut = Controls::default();
        let pilot = crate::systems::rcs::pilot(controls);
        let controls = if self.destroyed.is_some() { &cut } else { &pilot };
        let live_at_start = matches!(self.phase, Phase::Powered { .. });
        loop {
            // Broken up, or in a bubble waiting for a split (D082): nothing
            // moves until the split is applied.
            if self.breakup.is_some() || self.bubble.waiting {
                return (self.time, budget.spent);
            }
            // A craft of parts' orders due now; while it advances continuously
            // it stops at the next one's epoch (`ship_tick`).
            self.apply_orders(world);
            if self.breakup.is_some() {
                return (self.time, budget.spent);
            }
            // A planned burn's program wakes at its own epoch (D129).
            self.wake_program(world, controls);
            let step = match self.order_stop() {
                Some(e) if e < target => e,
                _ => target,
            };
            let before = (self.time, std::mem::discriminant(&self.phase));
            // A coast restarted at its own time (a new attitude regime) moved
            // on; one that reached the live boundary stops there.
            let (mut restarted, mut went_live) = (false, false);
            match &self.phase {
                Phase::Landed { .. } => self.advance_landed(world, step, controls),
                Phase::Crashed { .. } => self.advance_grounded(world, step),
                Phase::Powered { .. } => self.advance_powered(world, step, controls),
                Phase::Coasting { .. } => {
                    let c = self.advance_coasting(world, step, controls, &mut budget);
                    (restarted, went_live) = (c.progressed, c.entered);
                }
            }
            // Stop at the target, when neither time nor phase moved (waiting
            // for a segment, or less than one tick left), or out of budget
            // (integration or lattices) while coasting.
            let spent = (budget.thermal == 0 || budget.steps == 0) && matches!(self.phase, Phase::Coasting { .. });
            let stuck = (self.time, std::mem::discriminant(&self.phase)) == before && !restarted;
            // At the live boundary the game holds its clock and caps the
            // warp (D081) instead of flying live ticks through an atmosphere
            // for the rest of a rails-warp frame; in a frame, the same for
            // any way of going live.
            let woke = stop_live
                && !live_at_start
                && matches!(self.phase, Phase::Powered { .. })
                && target.seconds_since(self.time) > LIVE_STOP;
            let warp_stop = stop_live && self.warp_stop().is_some();
            if self.time >= target || stuck || spent || went_live || woke || warp_stop {
                return (self.time, budget.spent);
            }
        }
    }
}

impl Vessel {
    /// The perturbation model of this vessel's segments, holding `p`.
    pub(super) fn limbo_model(&self, p: Pointing) -> Limbo {
        let mut l = Limbo::new(self.craft.id.clone(), self.craft.design.clone(), p, self.craft.mass.dry_mass);
        l.optics = self.limbo.optics;
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
        if self.engine_running() || self.ship_wants_ticks(controls) {
            self.law_to_now();
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
        let (entered, settled) = self.coast_until(world, target, controls, budget);
        // A turn settled (even at the vessel's time): the next advance
        // restarts the coast with the attitude known.
        Coasted { progressed: restarted || settled || self.time != before, entered }
    }

    /// Restarts the coast if the attitude regime the controls ask for is
    /// not the current coast's pointing: a known pointing whose rule no
    /// longer holds, or an unknown one where the attitude is now known.
    /// Returns whether it restarted.
    fn repoint(&mut self, world: &World, controls: &Controls) -> bool {
        let Phase::Coasting { trajectory } = &self.phase else { return false };
        let seg = trajectory.current();
        if seg.t0.seconds_since(self.time) > 0.0 {
            return false;
        }
        let Some(state) = trajectory.eval(self.time) else { return false };
        let Some(current) = seg.limbo().map(|l| &l.pointing) else {
            // A coast from before perturbed coasts (an older save): restart it.
            self.law_to_now();
            self.start_coast_pointing(world, state, Pointing::Mean);
            return true;
        };
        let props = self.mass_props();
        let inertia_now = props.inertia;
        let torque = thruster_torque(self.thrusters_enabled().as_ref(), props.com);
        let turn = (inertia_now, self.thrust_axis(), torque);
        let fly = Rails::new(world, trajectory, &self.nodes, controls, turn);
        let rule = fly.rule_at(self.time, state.0);
        if !self.repoint && current.is_known() && current.rule().same_regime(&rule) {
            return false;
        }
        let aim = fly.aim_at(controls, self.time);
        let snap = world.snapshot(self.time);
        let lawful =
            pointing::lawful(&rule, (&self.attitude, &self.control), controls, &aim, world, &snap, state, inertia_now);
        if !self.repoint && !current.is_known() && lawful.is_none() {
            return false;
        }
        let p = lawful.unwrap_or(Pointing::Mean);
        if let Some(a) = p.attitude(world, &snap, state.0, state.1, state.2) {
            self.attitude = a;
            self.control = p.control(key_of(&aim), self.time, a, inertia_now);
        }
        self.law_to_now();
        self.start_coast_pointing(world, state, p);
        true
    }

    /// Returns whether it went live at the live boundary, and whether a turn
    /// settled where it stopped.
    fn coast_until(&mut self, world: &World, target: Epoch, controls: &Controls, budget: &mut Budget) -> (bool, bool) {
        // The thrust axis and the thrusters the turns on rails use.
        let axis = self.thrust_axis();
        let thrusters = self.thrusters_enabled();
        let Phase::Coasting { trajectory } = &mut self.phase else { unreachable!() };
        budget.extend(world, trajectory, target);
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
        // The mass is the ship state's at every lattice point (it vents
        // there, F2; its segment integrates its perturbations with the mass
        // it started with).
        let model = self.craft.mass;
        let ship_def = self.craft.design().ship.clone();
        let parts = |ship: Option<&crate::craft::state::ShipState>| {
            let props = ship.zip(ship_def.as_deref()).map(|(s, d)| super::ship_props(&model, d, s));
            props.unwrap_or_else(|| model.dry())
        };
        let held = parts(self.ship.as_ref());
        let (inertia_now, held_torque) = (held.inertia, thruster_torque(thrusters.as_ref(), held.com));
        let fly = Rails::new(world, trajectory, &self.nodes, controls, (inertia_now, axis, held_torque));
        let stop_settled;
        (reach, stop_settled) = fly.regime_stops(self.time, reach);
        let design = self.craft.design().clone();
        let mut t = self.time;
        let (mut stopped, mut halt) = (false, false);
        let mut settled = None;
        loop {
            // The thermal and systems lattices in time order (the attitude
            // is carried forward only); systems first at a shared epoch.
            let e_systems = self.systems.next().filter(|s| *s <= self.thermal.next_lattice());
            let e = e_systems.unwrap_or_else(|| self.thermal.next_lattice());
            if e.seconds_since(reach) > 0.0 {
                break;
            }
            // Out of budget: the vessel stops at its last lattice point, but
            // never between the systems step and the thermal point of one
            // epoch (a vessel at t has every lattice event at t done, as in
            // one call: a frame ending there would otherwise differ).
            // A warp stop raised (`warp_stop`) ends it the same way.
            if (budget.thermal == 0 || halt) && e.seconds_since(t) > 0.0 {
                stopped = true;
                break;
            }
            // What it holds now (its lattice steps so far).
            let now = parts(self.ship.as_ref());
            let inertia_at = |_: Epoch| now.inertia;
            let turn = (now.inertia, axis, thruster_torque(thrusters.as_ref(), now.com));
            let fly = Rails::new(world, trajectory, &self.nodes, controls, turn);
            if e_systems.is_some() {
                settled = fly.attitude(&mut self.attitude, &mut self.control, (t, e), &inertia_at, stop_settled);
                if settled.is_some() {
                    break;
                }
                t = e;
                let (anchor, r, v) = trajectory.eval(e).expect("the lattice epoch is in the trajectory");
                self.systems.step();
                let at = (e, (anchor, r, v), self.attitude.q);
                let law = law_at(trajectory, e);
                super::ship_tick::ship_lattice(self.ship.as_mut(), ship_def.as_deref(), world, at, None, law);
                let ship = (ship_def.as_deref(), self.ship.as_ref());
                halt |= super::warp_stop::watch(&mut self.software, &self.helm, ship, self.debug, e);
                budget.charge(SYSTEMS_UNITS);
                continue;
            }
            if fly.still(&self.attitude, &self.control, t) && self.thermal.passes(e) {
                self.thermal.pass(e);
                budget.charge(POINT_UNITS);
                continue;
            }
            settled = fly.attitude(&mut self.attitude, &mut self.control, (t, e), &inertia_at, stop_settled);
            if settled.is_some() {
                break;
            }
            t = e;
            let state = trajectory.eval(e).expect("the lattice epoch is in the trajectory");
            let pointing = trajectory.segment_at(e).and_then(|s| s.limbo()).map(|l| &l.pointing);
            let known = pointing.filter(|p| p.is_known());
            let mut suns = super::coast_heat::coast_suns(world, e, state, known, &self.attitude);
            suns.air = super::coast_heat::coast_air(world, e, state, known, &self.attitude, design.bake.length);
            let units = solve_units(&design, suns.suns.len() + suns.air.len());
            let held = self.ship.as_ref().zip(ship_def.as_deref()).map(|(s, d)| crate::systems::plumbing::held(d, s));
            let aboard = Aboard(held.unwrap_or_default());
            let solved = aerothermal::coast_step(&design, &mut self.thermal, &suns, &aboard, e, false);
            budget.charge(if solved { units } else { POINT_UNITS });
        }
        if let Some(s) = settled {
            reach = if s > self.time { s } else { self.time };
        } else if stopped {
            reach = t;
        } else {
            let now = parts(self.ship.as_ref());
            let inertia_at = |_: Epoch| now.inertia;
            let turn = (now.inertia, axis, thruster_torque(thrusters.as_ref(), now.com));
            let fly = Rails::new(world, trajectory, &self.nodes, controls, turn);
            if let Some(s) = fly.attitude(&mut self.attitude, &mut self.control, (t, reach), &inertia_at, stop_settled)
            {
                reach = if s > self.time { s } else { self.time };
                settled = Some(s);
            }
        }
        let keep = kept_from(&self.control, reach, trajectory);
        trajectory.prune_before(keep);
        let delta = trajectory.proper_time_at(reach).expect("the vessel's time is in its trajectory");
        self.clock = ShipClock::at(reach, delta);
        let last = trajectory.last();
        let near_surface = matches!(last.end, Some(SegmentEnd { kind: EndKind::Surface { .. }, .. })) && at_end;
        self.time = reach;
        if !stopped && settled.is_none() && (entry.is_some() || near_surface) {
            // In an atmosphere or close to a surface: flown in live ticks.
            self.law_to_now();
            self.thermal_to_now(world);
            let state = self.state(world);
            self.go_live(state);
            return (entry.is_some(), false);
        }
        (false, settled.is_some())
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
        let sun = super::coast_heat::CoastSun::one(sun);
        let aboard = self.aboard();
        aerothermal::coast_step(&design, &mut self.thermal, &sun, &aboard, self.time, true);
    }
}

/// Where a coast keeps its samples from, after reaching `reach`: back to
/// where the next frame's control resumes, a followed hold's base (up to a
/// follow step behind), else at most a tick behind (a lattice point can
/// fall just past the frame's end). Holds aim from the state there.
fn kept_from(control: &AttitudeControl, reach: Epoch, trajectory: &Trajectory) -> Epoch {
    let follows = control.settled.is_some() || control.waiting;
    let tick_back = reach.add_seconds(-TICK);
    let base = control.base.map(|b| b.epoch).and_then(|e| {
        if follows {
            (reach.seconds_since(e) < 2.0 * FOLLOW_STEP).then_some(e)
        } else {
            Some(if e > tick_back { e } else { tick_back })
        }
    });
    let seg_start = trajectory.segment_at(reach).map(|s| s.t0);
    match (base, seg_start) {
        (Some(b), Some(s)) if b < reach => {
            if b > s {
                b
            } else {
                s
            }
        }
        _ => reach,
    }
}

/// The law of a craft of parts' tanks at `e` (D119): the coast segment's
/// there and its local time.
fn law_at(trajectory: &Trajectory, e: Epoch) -> Option<(&crate::systems::coast_law::CoastLaw, f64)> {
    let s = trajectory.segment_at(e)?;
    Some((s.limbo()?.law.as_deref()?, e.seconds_since(s.t0)))
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
    /// A craft of parts' nodes (the maneuver hold points along the next).
    nodes: &'a [crate::systems::maneuver::Node],
    controls: &'a Controls,
    inertia_now: DMat3,
    /// The thrust axis the maneuver hold points ([`Vessel::thrust_axis`]).
    axis: DVec3,
    /// The thrusters' torque authority per body axis (N·m) for its turns
    /// ([`thruster_torque`]).
    torque: DVec3,
}

/// The thrusters' torque authority per body axis (N·m) about `com` of a
/// craft of parts' thrusters that are enabled (`ShipDef::rcs_authority`):
/// what turns it on rails. None without thrusters.
fn thruster_torque(thrusters: Option<&(std::sync::Arc<crate::craft::ship::ShipDef>, Vec<bool>)>, com: DVec3) -> DVec3 {
    thrusters.map_or(DVec3::ZERO, |(def, on)| def.rcs_authority(com, on).0)
}

impl Vessel {
    /// A craft of parts' definition and which of its thrusters are enabled.
    fn thrusters_enabled(&self) -> Option<(std::sync::Arc<crate::craft::ship::ShipDef>, Vec<bool>)> {
        let def = self.ship_def()?;
        let on = self.ship.as_ref()?.thrusters.iter().map(|t| t.enabled).collect();
        Some((def, on))
    }
}

impl<'a> Rails<'a> {
    fn new(
        world: &'a World,
        trajectory: &'a Trajectory,
        nodes: &'a [crate::systems::maneuver::Node],
        controls: &'a Controls,
        (inertia_now, axis, torque): (DMat3, DVec3, DVec3),
    ) -> Self {
        Rails { world, trajectory, nodes, controls, inertia_now, axis, torque }
    }
}

impl Rails<'_> {
    /// Where an advance from `time` must stop before `reach` for the
    /// attitude regime to be checked (`Vessel::repoint`): a coast after a
    /// change of automatic reference. And whether to stop where a turn
    /// settles: the attitude is unknown and the controls ask for a rule
    /// that knows it.
    fn regime_stops(&self, time: Epoch, mut reach: Epoch) -> (Epoch, bool) {
        let tr = self.trajectory;
        if let Some(e) = tr.next_regime_start(time, reach) {
            reach = e;
        }
        let known = tr.segment_at(time).and_then(|s| s.limbo()).map(|l| l.pointing.is_known());
        let stop_settled = known == Some(false) && {
            let anchor = tr.eval(time).map_or(tr.current().initial().anchor, |s| s.0);
            matches!(self.rule_at(time, anchor), Rule::Stability | Rule::Inertial { .. } | Rule::Track(_))
        };
        (reach, stop_settled)
    }

    /// The law of the next node at or after `epoch`.
    fn next_burn(&self, epoch: Epoch) -> Option<DirectionLaw> {
        node_direction(self.nodes, epoch)
    }

    /// The attitude regime the controls ask for at `epoch` ([`pointing::wanted`]).
    /// The automatic reference ([`super::hold::hold_reference`]) is asked
    /// only when the controls name no body.
    fn rule_at(&self, epoch: Epoch, anchor: crate::frame::NodeId) -> Rule {
        let auto = || {
            let (anchor, r) = self.trajectory.eval(epoch).map_or((anchor, DVec3::ZERO), |s| (s.0, s.1));
            super::hold::hold_reference(self.world, &self.world.snapshot(epoch), anchor, r)
        };
        pointing::wanted(self.controls, self.next_burn(epoch).as_ref(), auto, self.axis)
    }

    /// Whether the attitude stays as it is over `(t, e)` (no rotation, no
    /// control ticks): the thermal lattice may skip.
    fn still(&self, att: &Attitude, control: &AttitudeControl, t: Epoch) -> bool {
        att.omega == DVec3::ZERO && !needs_ticks(att, control, self.controls, &self.aim_at(self.controls, t))
    }

    /// What SAS aims at at `epoch` with `controls`.
    fn aim_at(&self, controls: &Controls, epoch: Epoch) -> Aim {
        let Some(state) = self.trajectory.eval(epoch) else { return Aim::Stability };
        if !controls.sas || controls.hold == HoldMode::Stability {
            return Aim::Stability;
        }
        let snap = LazySnapshot::new(self.world, epoch);
        let maneuver =
            (controls.hold == HoldMode::Maneuver).then(|| maneuver_direction(self.nodes, &snap, state)).flatten();
        aim(controls, &hold_inputs(self.world, &snap, state, controls, maneuver), self.axis)
    }

    /// Carries the attitude over `(t, e)`: by [`advance_coast`], or read
    /// from the coast's known pointing. With `stop_settled`, stops where a
    /// turn settles and returns that epoch.
    fn attitude(
        &self,
        att: &mut Attitude,
        control: &mut AttitudeControl,
        (mut t, e): (Epoch, Epoch),
        inertia_at: &impl Fn(Epoch) -> DMat3,
        stop_settled: bool,
    ) -> Option<Epoch> {
        let torque = self.torque;
        while t < e {
            let seg = self.trajectory.segment_at(t)?;
            let end = seg.end.map_or(e, |x| seg.t0.add_seconds(x.t));
            let stop = if end < e { end } else { e };
            match seg.limbo().map(|l| &l.pointing).filter(|p| p.is_known()) {
                Some(p) => {
                    if let Some(a) = seg.limbo_attitude(self.world, stop.seconds_since(seg.t0)) {
                        *att = a;
                        *control = p.control(key_of(&self.aim_at(self.controls, stop)), stop, a, inertia_at(stop));
                    }
                    // The automatic reference changed: the hold turns to the
                    // new body's frame (control ticks), not snapped onto it.
                    if stop == end && seg.end.is_some_and(|x| x.kind == EndKind::Regime) {
                        control.settled = None;
                    }
                    if stop == t {
                        return None;
                    }
                    t = stop;
                }
                None => {
                    let aim_at = |x: Epoch| self.aim_at(self.controls, x);
                    let inertia = (self.inertia_now, inertia_at);
                    let settled =
                        advance_coast(att, control, (t, e), self.controls, torque, inertia, aim_at, stop_settled);
                    if settled.is_some() {
                        return settled;
                    }
                    t = e;
                }
            }
        }
        None
    }
}
