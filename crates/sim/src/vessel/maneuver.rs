//! A craft of parts' planned burns (D126, D129): the maneuver program
//! (`systems::maneuver`, layer 2) run on the vessel. Nodes do not touch the
//! stored trajectory: the vessel stops at the program's wake epoch like an
//! order, whatever the warp, and flies live from there; each live tick the
//! program's step runs first and its commands lie over the pilot's
//! controls ([`Vessel::tick_controls`], the one place, used by every tick
//! loop).

use super::{PlanError, Vessel};
use crate::craft::command::ShipCommand;
use crate::systems::maneuver::{self, Node, Program, Step};
use crate::systems::{command, estimate, flight};
use crate::time::Epoch;
use crate::world::World;
use glam::DVec3;

/// What the planner shows of a node before it is flown (one owner): a
/// prediction, never the stored trajectory.
#[derive(Clone, Debug, PartialEq)]
pub struct NodePrediction {
    /// The state just after the node, if its Δv were given at once
    /// (relative to the node's reference body): the orbit the program
    /// closes the loop on.
    pub after: (crate::frame::NodeId, DVec3, DVec3),
    /// Ignition, and the burn's duration from the stage estimate (s).
    pub ignite: Epoch,
    pub duration: Option<f64>,
    /// The Δv the engines have (m/s), if measured.
    pub available: Option<f64>,
    pub warnings: Vec<String>,
}

impl Vessel {
    /// The maneuver program of the next node (waiting, flying, or the last
    /// one ended).
    pub fn program(&self) -> Option<&Program> {
        self.program.as_ref()
    }

    /// The engines a planned burn lights: the main engines the throttle
    /// refers to (those selected, else all), still aboard.
    fn burn_engines(&self) -> Vec<u32> {
        let Some((def, state)) = self.ship_parts() else { return Vec::new() };
        let main: Vec<u32> = (0..def.engines.len() as u32).filter(|&i| def.engines[i as usize].main).collect();
        let selected: Vec<u32> = main.iter().copied().filter(|&i| state.engines[i as usize].selected).collect();
        if selected.is_empty() {
            main
        } else {
            selected
        }
    }

    /// The stage estimate of a burn of `engines` lit now at full throttle
    /// in vacuum (`estimate::lit`).
    fn burn_estimate(&self, engines: &[u32]) -> Option<estimate::Burn> {
        let (def, state) = self.ship_parts()?;
        let k = self.knowledge();
        let hull = estimate::hull(&self.craft.design().parts);
        let inp = estimate::Inputs { hull: &hull, knowledge: &k, throttle: 1.0, p: 0.0, g: None };
        estimate::lit(def, state, &inp, engines)
    }

    /// The program of node `i` of the plan, scheduled from the stage
    /// estimate now (`maneuver::schedule`); a wake already past is now.
    /// Nodes already past (missed) are skipped.
    fn schedule_node(&self, i: usize) -> Option<Program> {
        let i = (i..self.nodes.len()).find(|&j| self.nodes[j].t >= self.time)?;
        let node = &self.nodes[i];
        let (def, _) = self.ship_parts()?;
        let engines = self.burn_engines();
        let burn = self.burn_estimate(&engines);
        let full: f64 = engines.iter().map(|&e| def.engines[e as usize].data.thrust_vac).sum();
        let thrust = burn.as_ref().and_then(|b| b.thrust.map(|t| t.value)).unwrap_or(full);
        let isp = burn.as_ref().map_or(0.0, |b| b.isp);
        let mdot = if isp > 0.0 { thrust / (isp * super::G0) } else { 0.0 };
        let mass = burn.as_ref().map_or_else(|| self.mass(), |b| b.mass.value);
        let start = engines.iter().map(|&e| def.engines[e as usize].data.start_time).fold(0.0, f64::max);
        let props = self.mass_props();
        let turn = self.rcs_authority().map_or(super::hold::SETTLE, |a| {
            let i = props.inertia;
            super::turn_lead(&i, DVec3::new(a.rotate.x * i.x_axis.x, a.rotate.y * i.y_axis.y, a.rotate.z * i.z_axis.z))
        });
        let (wake, ignite) = maneuver::schedule(node, (thrust, mdot, mass), start, turn);
        let wake = if wake < self.time { self.time } else { wake };
        Some(Program::new(i, (wake, ignite), engines))
    }

    /// The planned burns of a craft of parts (D126): its nodes, kept after
    /// they are flown.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Replaces a craft of parts' nodes, by the rules of
    /// [`Vessel::set_plan`]: in order; a node flown or being flown cannot
    /// change, and a changed one cannot be in the past. They leave the
    /// stored trajectory as it is. The next node's program is scheduled
    /// anew unless it flies.
    pub fn set_nodes(&mut self, nodes: Vec<Node>) -> Result<(), PlanError> {
        if self.ship.is_none() {
            return Err(PlanError::NodesNeedParts);
        }
        if !nodes.windows(2).all(|w| w[0].t <= w[1].t) {
            return Err(PlanError::NotSorted);
        }
        let same = self.nodes.iter().zip(&nodes).take_while(|(a, b)| a == b).count();
        if same == self.nodes.len() && same == nodes.len() {
            return Ok(());
        }
        let started = self.program.as_ref().map_or(0, |p| if p.step == Step::Wait { p.node } else { p.node + 1 });
        if same < started {
            return Err(PlanError::Started);
        }
        if nodes[same..].iter().any(|n| n.t < self.time) {
            return Err(PlanError::InThePast);
        }
        self.nodes = nodes;
        if self.program.as_ref().is_none_or(|p| !p.step.flying()) {
            if let Some(p) = self.schedule_node(started) {
                self.program = Some(p);
            }
        }
        Ok(())
    }

    /// After a split, the kept piece's program for the next node not yet
    /// flown (`old`: the whole's; a burn in flight ends with the split, as
    /// its engines' indices were the whole's).
    pub(super) fn reschedule(&mut self, old: Option<&Program>) {
        let next = old.map_or(0, |p| if p.step == Step::Wait { p.node } else { p.node + 1 });
        self.program = self.schedule_node(next);
    }

    /// The epoch the next program wakes, while it waits.
    pub(super) fn program_stop(&self) -> Option<Epoch> {
        self.program.as_ref().filter(|p| p.step == Step::Wait).map(|p| p.wake)
    }

    /// Whether a program flies the craft now (it needs live ticks).
    pub(super) fn program_flying(&self) -> bool {
        self.program.as_ref().is_some_and(|p| p.step.flying())
    }

    /// The navigation solution relative to `node`'s reference body, and the
    /// body's μ: what the program steers on.
    fn node_frame(&self, world: &World, view: &flight::View, node: &Node) -> (Option<(DVec3, DVec3)>, f64) {
        let mu = world.source(node.reference).map_or(0.0, |s| s.gm);
        let rel = view.nav.map(|(anchor, r, v)| {
            let k = world.eph.relative(anchor, node.reference, self.time);
            (r + k.r, v + k.v)
        });
        (rel, mu)
    }

    /// Wakes the waiting program when its epoch has come, with the pilot's
    /// `controls` as its baseline.
    pub(super) fn wake_program(&mut self, world: &World, controls: &super::Controls) {
        let Some(p) = self.program.as_ref().filter(|p| p.step == Step::Wait && p.wake <= self.time) else { return };
        let Some(node) = self.nodes.get(p.node).copied() else { return };
        let Some((def, state)) = self.ship_parts() else { return };
        let view = flight::sense(self.knowledge(), (self.attitude, self.state(world)), Some((def, state)));
        let (rel, mu) = self.node_frame(world, &view, &node);
        let to_node = node.t.seconds_since(self.time);
        let mut p = self.program.take().expect("checked");
        p.wake_up(&node, rel, mu, to_node, controls);
        self.program = Some(p);
        self.program_log();
    }

    /// The controls a live tick flies with the pilot's `controls`: a flying
    /// program's step runs (its ignitions applied now) and its commands lie
    /// over them, unless the pilot's input conflicts with it (D126): then
    /// the burn is the pilot's from this tick on.
    pub(super) fn tick_controls(&mut self, world: &World, controls: &super::Controls) -> super::Controls {
        if !self.program_flying() {
            return *controls;
        }
        let mut p = self.program.take().expect("flying");
        let (Some(node), Some(def)) = (self.nodes.get(p.node).copied(), self.ship_def()) else {
            p.step = Step::Short;
            self.program = Some(p);
            self.program_log();
            return *controls;
        };
        let state = self.ship.as_ref().expect("a craft of parts");
        let k = self.knowledge();
        let settled = flight::feeds_read(&def, state, &k, &p.engines);
        let view = flight::sense(k, (self.attitude, self.state(world)), Some((&def, state)));
        let (rel, mu) = self.node_frame(world, &view, &node);
        let axis = p.engines.iter().map(|&i| def.engines[i as usize].data.dir.normalize()).sum::<DVec3>().normalize();
        let can_settle = crate::systems::rcs::can_push(&def, state, axis);
        let burn = (p.step == Step::Burn).then(|| self.burn_estimate(&p.engines)).flatten();
        let mass = burn.as_ref().map_or_else(|| self.mass(), |b| b.mass.value);
        let thrust = burn.as_ref().and_then(|b| b.thrust.map(|t| t.value)).unwrap_or(0.0);
        let tail_impulse = p.engines.iter().map(|&i| def.engines[i as usize].data.shutdown_impulse).sum();
        let level_min = p.engines.iter().map(|&i| def.engines[i as usize].data.throttle_min).fold(0.0, f64::max);
        let x = maneuver::Inputs {
            t: self.time,
            view: &view,
            node: &node,
            rel,
            mu,
            settled,
            axis,
            can_settle,
            mass,
            thrust,
            tail_impulse,
            level_min,
            dt: super::TICK,
        };
        let before = p.step;
        let cmds = maneuver::step(&x, &mut p);
        if p.pilot.is_some_and(|base| maneuver::conflicts(&base, controls, &cmds)) {
            p.step = Step::Manual;
            self.program = Some(p);
            self.program_log();
            return *controls;
        }
        let ignite = cmds.ignite.clone();
        let ended = p.step != before && !p.step.flying();
        self.program = Some(p);
        if !ignite.is_empty() {
            let at = self.time;
            let state = self.ship.as_mut().expect("a craft of parts");
            let cx = command::Context { at, sequence: false };
            let mut effects = Vec::new();
            for i in ignite {
                let owner = def.engines[i as usize].owner.clone();
                effects.extend(command::apply(&def, state, &ShipCommand::Ignite(owner), cx).unwrap_or_default());
            }
            self.after_command(world, &effects);
        }
        if ended {
            self.program_log();
        }
        cmds.over(controls)
    }

    /// The ship's log of the program's step, and the next node's program
    /// when it ended.
    fn program_log(&mut self) {
        let Some(p) = self.program.as_ref() else { return };
        let n = p.node + 1;
        let left = p.left.map_or(String::new(), |l| format!(" ({l:.2} m/s left)"));
        let (text, alarm) = match p.step {
            Step::Turn => (format!("burn {n}: turning to the burn"), false),
            Step::Done => (format!("burn {n}: on target{left}"), false),
            Step::Manual => (format!("burn {n}: taken over by the pilot"), false),
            Step::Short if p.target.is_none() => (format!("burn {n}: no navigation solution: not flown"), true),
            Step::Short => (format!("burn {n}: ended short{left}"), true),
            _ => return,
        };
        let (at, ended, next) = (self.time, p.step.ended(), p.node + 1);
        if let Some(state) = self.ship.as_mut() {
            state.runtime.event(at, text, alarm);
        }
        if ended {
            if let Some(next) = self.schedule_node(next) {
                self.program = Some(next);
            }
        }
    }

    /// The maneuver hold's direction from a flying program (inertial).
    pub(super) fn program_direction(&self) -> Option<DVec3> {
        self.program.as_ref().filter(|p| p.step.flying()).and_then(|p| p.dir)
    }

    /// What the planner shows of node `i` before it is flown: the orbit it
    /// gives from the stored coast's state at the node, and the stage
    /// estimate's ignition and duration (see [`NodePrediction`]).
    pub fn node_prediction(&self, world: &World, i: usize) -> Option<NodePrediction> {
        let node = *self.nodes.get(i)?;
        let (def, state) = self.ship_parts()?;
        // The stored coast at the node where it is computed that far, else
        // the state now carried there along its conic.
        let traj = self.trajectory().filter(|tr| tr.computed_until() >= node.t);
        let (r, v) = match traj.and_then(|tr| tr.eval(node.t)) {
            Some((anchor, r, v)) => {
                let k = world.eph.relative(anchor, node.reference, node.t);
                (r + k.r, v + k.v)
            }
            None => {
                let (anchor, r, v) = self.state(world);
                let k = world.eph.relative(anchor, node.reference, self.time);
                let mu = world.source(node.reference)?.gm;
                crate::kepler::propagate(r + k.r, v + k.v, mu, node.t.seconds_since(self.time))?
            }
        };
        let (p, n, radial) = super::prograde_normal_radial(r, v);
        let after = (node.reference, r, v + p * node.dv.x + n * node.dv.y + radial * node.dv.z);
        let program = self.schedule_node(i)?;
        let burn = self.burn_estimate(&program.engines);
        let available = burn.as_ref().and_then(|b| b.dv.map(|d| d.value));
        let duration = burn.as_ref().and_then(|b| {
            let (f, m0) = (b.thrust?.value, b.mass.value);
            let mdot = if b.isp > 0.0 { f / (b.isp * super::G0) } else { 0.0 };
            let dv = node.dv.length();
            Some(if mdot > 0.0 { m0 / mdot * (1.0 - crate::math::exp(-dv * mdot / f)) } else { dv * m0 / f })
        });
        let mut warnings = Vec::new();
        let axis = program.engines.iter().map(|&e| def.engines[e as usize].data.dir.normalize()).sum::<DVec3>();
        if axis.length() > 0.0 && !crate::systems::rcs::can_push(def, state, axis.normalize()) {
            warnings.push("no thrusters push along the engines: the propellant cannot be settled".into());
        }
        if available.is_some_and(|a| a < node.dv.length()) {
            warnings.push(format!(
                "the engines have {:.0} m/s of the {:.0} m/s",
                available.unwrap_or(0.0),
                node.dv.length()
            ));
        }
        if program.wake <= self.time && i == self.program.as_ref().map_or(0, |p| p.node) {
            warnings.push("too soon: the burn starts before the craft is turned and settled".into());
        }
        Some(NodePrediction { after, ignite: program.ignite, duration, available, warnings })
    }
}
