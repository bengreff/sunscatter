//! A vessel's aerodynamics and heating (realism-1 §5, D061, D065): the
//! design's pure models (`sim::aero`, `sim::thermal`) wired to the flight.
//!
//! **Live ticks** (inside an atmosphere every tick is live): the air at the
//! tick's start (density, temperature, speed of sound, mean free path of the
//! body whose atmosphere the vessel is in; the wind −(v − ω×r − v_body)) is
//! frozen for the tick ([`AeroTick`]). The aerodynamic moment about the
//! centre of mass is evaluated at every RK4 stage of the rotation (it
//! depends on the attitude); the force is split into its drag along the
//! flow, kept velocity-dependent inside the translation's integrator as a
//! drag area, and the rest (lift), held constant over the tick. The
//! parachute adds its drag area along the flow, pulling at its mount.
//! Heating: Sutton–Graves at the direction's nose radius, spread by
//! [`thermal::cell_heat`] with the sunlight (eclipses by body spheres,
//! [`crate::light`]); one implicit thermal step per tick.
//!
//! **Coasts** (above every atmosphere): the temperatures step on a fixed
//! [`LATTICE`] from the vessel's thermal epoch (sunlight and radiation
//! only), with the attitude and position at each lattice epoch, so 60 fps
//! and one long jump give the same bits. The sunlight of a coast step is
//! averaged over the vessel's osculating orbit ([`coast_sunlight`]: the
//! orbit's sunlit fraction, for bound orbits of at most
//! [`ORBIT_AVERAGE_PERIOD`]), so the input changes only slowly. While the
//! sunlight in body axes has not changed, lattice points are skipped as
//! long as the temperatures, at the last solve's rate, move by less than
//! [`STEP_CHANGE_K`] (up to [`MAX_SKIP`]), and the next solve spans them
//! (backward Euler is stable at any step; [`coast_step`]).
//! Landed and crashed vessels keep their temperatures (ground heat
//! exchange comes later).
//!
//! Radiation goes to deep space ([`SINK_K`]) everywhere for now: planet IR,
//! albedo and convective cooling come later.

use super::burn::BurnLaw;
use super::segment::{Segment, SegmentKind};
use crate::aero::{self, Flow};
use crate::craft::CraftDesign;
use crate::ephem::Snapshot;
use crate::frame::{NodeId, Vec3};
use crate::thermal::{self, HeatInput, ThermalState, COAST_CONVERGED, DEFAULT_SWEEPS};
use crate::time::Epoch;
use crate::world::World;
use glam::{DQuat, DVec3};

/// The coast thermal lattice (s): a third of the test craft's skin time
/// constant (C/(4εσT³) ≈ 1,600 s at 290 K).
pub const LATTICE: f64 = 600.0;
/// Radiative sink temperature (K): the cosmic background.
pub const SINK_K: f64 = 2.725;
/// Temperature of a new vessel (K).
pub const INITIAL_K: f64 = 290.0;
/// Coast solves are spaced so that each moves no temperature by more
/// than about this (K), at the rate of the last solve.
const STEP_CHANGE_K: f64 = 0.5;
/// While the attitude does not change, the sunlight is compared only every
/// this many lattice points (1 h; the Sun moves 0.04° against a fixed
/// attitude, an orbit's eclipse fraction little more).
pub const SUN_CHECK_POINTS: u64 = 6;
/// Longest run of skipped lattice points (s).
pub const MAX_SKIP: f64 = 86_400.0;
/// Lattice points are skipped only while the sunlight in body axes stays
/// within this fraction of what the last solve had (or of 1 W/m²): about
/// 0.7 K on a sunlit cell.
const SUN_SAME: f64 = 1e-2;
/// Coasts average the sunlight over bound orbits of at most this period (s).
pub const ORBIT_AVERAGE_PERIOD: f64 = 86_400.0;
/// A vessel in an atmosphere starts coasting only this far above its top (m).
pub const LEAVE_ATMOSPHERE: f64 = 1_000.0;

/// A vessel's temperatures and where they stand in time.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VesselThermal {
    pub state: ThermalState,
    /// The time `state` is at.
    pub epoch: Epoch,
    /// Coasts: the last lattice point passed (solved or skipped).
    lattice: Epoch,
    /// Coasts: the sunlight (body axes, W/m²) of the last solve and how
    /// long after it (s) the next solve is due while that sunlight holds.
    hold: Option<(DVec3, f64)>,
    /// Coasts: planned burns not yet stepped past: start, end (once known)
    /// and the engine's heat (W), in start order.
    burns: Vec<(Epoch, Option<Epoch>, f64)>,
}

impl VesselThermal {
    pub fn uniform(cells: usize, nodes: usize, t: f64, epoch: Epoch) -> Self {
        let state = ThermalState::uniform(cells, nodes, t);
        VesselThermal { state, epoch, lattice: epoch, hold: None, burns: Vec::new() }
    }

    /// Hottest skin cell (K).
    pub fn max_skin(&self) -> f64 {
        thermal::hottest(&self.state.skin).map_or(0.0, |h| h.1)
    }

    /// The skin cell closest to its limit (`skin_max`, per cell): its
    /// temperature and limit (K).
    pub fn skin_nearest_limit(&self, skin_max: &[f64]) -> (f64, f64) {
        thermal::nearest_limit(&self.state.skin, skin_max).map_or((0.0, 1.0), |(_, t, m)| (t, m))
    }

    /// Hottest interior node: (K, index).
    pub fn max_node(&self) -> (f64, usize) {
        thermal::hottest(&self.state.nodes).map_or((0.0, 0), |(i, t)| (t, i))
    }

    /// Records the planned burns of `segments` that reach past the last
    /// lattice point and start by `reach`, with the engine heat `heat(law)`
    /// (W), before the trajectory drops them.
    pub fn record_burns(&mut self, segments: &[Segment], reach: Epoch, heat: impl Fn(&BurnLaw) -> f64) {
        for seg in segments {
            let SegmentKind::Burn(law) = &seg.kind else { continue };
            let end = seg.end.map(|e| seg.t0.add_seconds(e.t));
            if seg.t0.seconds_since(reach) > 0.0 || end.is_some_and(|e| e.seconds_since(self.lattice) <= 0.0) {
                continue;
            }
            match self.burns.iter_mut().find(|b| b.0 == seg.t0) {
                Some(b) => b.1 = end,
                None => self.burns.push((seg.t0, end, heat(law))),
            }
        }
        self.burns.sort_by(|a, b| a.0.seconds_since(b.0).total_cmp(&0.0));
    }

    /// Engine heat (J) of the recorded burns over (a, b].
    fn burn_energy(&self, a: Epoch, b: Epoch) -> f64 {
        (self.burns.iter())
            .map(|&(start, end, w)| {
                let from = if start.seconds_since(a) > 0.0 { start } else { a };
                let to = end.filter(|e| e.seconds_since(b) < 0.0).unwrap_or(b);
                w * to.seconds_since(from).max(0.0)
            })
            .sum()
    }

    /// Whether the coast lattice point `e` can be passed without even
    /// evaluating the sunlight, for a vessel whose attitude does not change
    /// (the caller's judgement): the last solve's hold still runs, no
    /// planned burn heats the span, and `e` is not one of the points every
    /// [`SUN_CHECK`] after the last solve where the sunlight is compared.
    pub fn passes(&self, e: Epoch) -> bool {
        let Some((_, span)) = self.hold else { return false };
        let dt = e.seconds_since(self.epoch);
        let k = libm::round(dt / LATTICE) as u64;
        dt < span && !k.is_multiple_of(SUN_CHECK_POINTS) && self.burn_energy(self.epoch, e) == 0.0
    }

    /// Passes lattice point `e` (see [`Self::passes`]).
    pub fn pass(&mut self, e: Epoch) {
        self.lattice = e;
    }

    /// Restarts the clock at `epoch` without stepping (landed vessels).
    pub fn restart(&mut self, epoch: Epoch) {
        self.epoch = epoch;
        self.lattice = epoch;
        self.hold = None;
        self.burns.clear();
    }

    /// The next coast lattice point.
    pub fn next_lattice(&self) -> Epoch {
        self.lattice.add_seconds(LATTICE)
    }
}

/// The sunlight of a coast step at `t` for a vessel at `(r, v)` relative to
/// `anchor` (inertial, W/m²): averaged over its osculating orbit when that
/// is bound, clear of the body and at most [`ORBIT_AVERAGE_PERIOD`] long
/// ([`crate::light::orbit_average_sunlight`]); otherwise the sunlight at
/// the point, eclipses included ([`crate::light::sunlight`]).
pub fn coast_sunlight(world: &World, t: Epoch, anchor: NodeId, r: DVec3, v: DVec3) -> DVec3 {
    let snap = world.snapshot(t);
    crate::light::orbit_average_sunlight(world, &snap, anchor, r, v, ORBIT_AVERAGE_PERIOD)
        .unwrap_or_else(|| crate::light::sunlight(world, &snap, anchor, r))
}

/// The air around a vessel at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air {
    /// The body whose atmosphere it is.
    pub body: NodeId,
    /// kg/m³.
    pub rho: f64,
    /// Velocity of the air relative to the vessel (inertial axes, m/s).
    pub wind: DVec3,
    pub speed_of_sound: f64,
    pub mean_free_path: f64,
    /// Static temperature (K).
    pub temperature: f64,
    pub gamma: f64,
    pub sutton_graves_k: f64,
    pub radiative_heating: Option<thermal::TauberSutton>,
}

impl Air {
    /// Dynamic pressure ½ρV² (Pa).
    pub fn q(&self) -> f64 {
        0.5 * self.rho * self.wind.length_squared()
    }
}

/// The air at `(r, v)` (anchor-relative) at the snapshot's time: the first
/// body (in source order) whose atmosphere has air there.
pub fn air_at(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, v: DVec3) -> Option<Air> {
    for s in &world.sources {
        let Some(p) = s.physical.as_ref() else { continue };
        let Some(atm) = &p.atmosphere else { continue };
        let k = snap.relative(s.node, anchor);
        let d = r - k.r;
        if d.length() - p.radius_eq >= atm.top {
            continue;
        }
        let g0 = s.gm / (p.radius_eq * p.radius_eq);
        let Some(state) = atm.air(p.altitude(p.rotation.to_fixed(Vec3::from_raw(d), snap.t)), g0) else { continue };
        let v_air = k.v + p.rotation.omega(snap.t).raw().cross(d);
        return Some(Air {
            body: s.node,
            rho: state.rho,
            wind: v_air - v,
            speed_of_sound: state.speed_of_sound(atm.gamma),
            mean_free_path: state.mean_free_path,
            temperature: state.temperature,
            gamma: atm.gamma,
            sutton_graves_k: atm.sutton_graves_k,
            radiative_heating: atm.radiative_heating,
        });
    }
    None
}

/// Whether `r` (anchor-relative) is below `margin` above the top of any
/// body's atmosphere (altitude above the ellipsoid, like the density).
pub fn in_atmosphere(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, margin: f64) -> bool {
    world.sources.iter().any(|s| {
        let Some(p) = s.physical.as_ref() else { return false };
        let Some(atm) = &p.atmosphere else { return false };
        let d = r - snap.relative_r(s.node, anchor);
        d.length() - p.radius_eq < atm.top + margin + 1e5
            && p.altitude(p.rotation.to_fixed(Vec3::from_raw(d), snap.t)) < atm.top + margin
    })
}

/// The aerodynamics of one live tick: the air frozen at the tick's start.
pub struct AeroTick<'a> {
    design: &'a CraftDesign,
    /// Direction the air moves relative to the vessel (inertial, unit).
    wind_dir: DVec3,
    q: f64,
    mach: f64,
    knudsen: f64,
    gamma: f64,
    reynolds_per_m: f64,
    temperature: f64,
    speed: f64,
    /// Centre of mass (body axes).
    com: DVec3,
    /// Parachute: Cd·A (m²) and mount (body axes), if deployed.
    chute: Option<(f64, DVec3)>,
}

impl<'a> AeroTick<'a> {
    /// `None` without air or wind.
    pub fn new(design: &'a CraftDesign, air: &Air, com: DVec3, chute: Option<(f64, DVec3)>) -> Option<Self> {
        let speed = air.wind.length();
        if speed.is_nan() || speed <= 0.0 {
            return None;
        }
        Some(AeroTick {
            design,
            wind_dir: air.wind / speed,
            q: air.q(),
            mach: speed / air.speed_of_sound,
            knudsen: aero::air::knudsen(air.mean_free_path, design.bake.length),
            gamma: air.gamma,
            reynolds_per_m: air.rho * speed / aero::air::sutherland_viscosity(air.temperature),
            temperature: air.temperature,
            speed,
            com,
            chute,
        })
    }

    /// The flow in body axes for attitude `q` (body → inertial).
    pub fn flow(&self, q: DQuat) -> Flow {
        let dir = (q.inverse() * self.wind_dir).normalize();
        let (reynolds_per_m, temperature, speed) = (self.reynolds_per_m, self.temperature, self.speed);
        Flow {
            dir,
            q: self.q,
            mach: self.mach,
            knudsen: self.knudsen,
            gamma: self.gamma,
            reynolds_per_m,
            temperature,
            speed,
        }
    }

    /// Hull force (body axes, N) and total torque about the centre of mass
    /// (body axes, N·m, parachute included) at attitude `q`.
    fn hull_and_torque(&self, q: DQuat) -> (DVec3, DVec3, Flow) {
        let q = q.normalize();
        let flow = self.flow(q);
        let (f, m) = aero::aero_forces(&self.design.bake, &flow, self.design.cd0);
        let mut torque = m - self.com.cross(f);
        if let Some((cd_area, mount)) = self.chute {
            torque += (mount - self.com).cross(flow.dir * (self.q * cd_area));
        }
        (f, torque, flow)
    }

    /// Aerodynamic torque about the centre of mass (body axes) at `q`.
    pub fn torque(&self, q: DQuat) -> DVec3 {
        self.hull_and_torque(q).1
    }

    /// For the translation at attitude `q`: the drag area along the flow
    /// (m², hull and parachute; the drag model keeps it velocity-dependent)
    /// and the rest of the hull force (lift) as an acceleration (inertial,
    /// m/s², constant over the tick) for a vessel of `mass`.
    pub fn drag_and_lift(&self, q: DQuat, mass: f64) -> (f64, DVec3) {
        let (f, _, flow) = self.hull_and_torque(q);
        let along = f.dot(flow.dir);
        let lift = f - flow.dir * along;
        let chute = self.chute.map_or(0.0, |c| c.0);
        ((along / self.q).max(0.0) + chute, q.normalize() * lift / mass)
    }
}

/// Stagnation heat flux (W/m²) in `air` at nose radius `rn`: convective
/// (Sutton–Graves) plus shock-layer radiation (Tauber–Sutton, where the
/// atmosphere has it).
pub fn stagnation_flux(air: &Air, rn: f64) -> f64 {
    let v = air.wind.length();
    let convective = thermal::sutton_graves(air.sutton_graves_k, air.rho, rn, v);
    convective + air.radiative_heating.map_or(0.0, |m| thermal::tauber_sutton(m, air.rho, rn, v))
}

/// Scratch buffers of the heat inputs.
struct Buffers {
    heat: Vec<f64>,
    scratch: Vec<f64>,
    node_heat: Vec<f64>,
    node_capacity: Vec<f64>,
}

impl Buffers {
    fn new(design: &CraftDesign) -> Self {
        let (n, m) = (design.cells.len(), design.nodes());
        Buffers { heat: vec![0.0; n], scratch: vec![0.0; n], node_heat: vec![0.0; m], node_capacity: vec![0.0; m] }
    }
}

/// The engine's wall heat `engine_heat` (W) into the nozzle's cells (D077),
/// no heat released in the nodes, and the nodes' capacities with
/// `propellant` kg aboard.
fn engine_and_nodes(design: &CraftDesign, engine_heat: f64, propellant: f64, b: &mut Buffers) {
    for &(i, share) in &design.nozzle_cells {
        b.heat[i as usize] += engine_heat * share;
    }
    b.node_heat.fill(0.0);
    design.node_capacity(propellant, &mut b.node_capacity);
}

/// One live tick of heating: `state` advanced by `dt` with the flow and
/// stagnation flux of `air` (if any), the sunlight `sun_body` (W/m², body
/// axes) and the engine's wall heat `engine_heat` (W, into the nozzle), with
/// `propellant` kg aboard. The flow is taken at attitude `q`.
#[allow(clippy::too_many_arguments)]
pub fn live_step(
    design: &CraftDesign,
    th: &mut VesselThermal,
    tick: Option<(&AeroTick, &Air)>,
    q: DQuat,
    sun_body: DVec3,
    engine_heat: f64,
    propellant: f64,
    dt: f64,
) {
    let mut input = HeatInput { flow_dir: DVec3::ZERO, q_stag: 0.0, sun: sun_body };
    if let Some((aero_tick, air)) = tick {
        let flow = aero_tick.flow(q.normalize());
        let rn = design.bake.nose_radius_at(flow.dir);
        input.flow_dir = flow.dir;
        input.q_stag = stagnation_flux(air, rn);
    }
    let mut b = Buffers::new(design);
    thermal::cell_heat(&design.bake, &design.cells, &input, &mut b.scratch, &mut b.heat);
    engine_and_nodes(design, engine_heat, propellant, &mut b);
    design.network.step(&mut th.state, &b.heat, &b.node_heat, &b.node_capacity, SINK_K, dt, DEFAULT_SWEEPS);
    th.restart(th.epoch.add_seconds(dt));
}

/// The coast step at `epoch` (a lattice point; or, with `last`, the end of
/// the coast) with the sunlight `sun_body` (W/m², body axes) and
/// `propellant` kg aboard: solves from the state's epoch (spanning skipped
/// lattice points; the planned burns' engine heat over that span goes into
/// the nozzle), or skips the point. A point is skipped while the
/// sunlight is the same as at the last solve (within [`SUN_SAME`]), no
/// burn heats the span and the last solve's rate of change says the
/// temperatures have moved less than [`STEP_CHANGE_K`] since (at most
/// [`MAX_SKIP`]): error control on a deterministic lattice. Returns
/// whether the network was solved.
pub fn coast_step(
    design: &CraftDesign,
    th: &mut VesselThermal,
    sun_body: DVec3,
    propellant: f64,
    epoch: Epoch,
    last: bool,
) -> bool {
    let dt = epoch.seconds_since(th.epoch);
    th.lattice = epoch;
    if dt <= 0.0 {
        return false;
    }
    let burn = th.burn_energy(th.epoch, epoch);
    if let Some((s, span)) = th.hold.filter(|_| !last && burn == 0.0) {
        if dt < span && (sun_body - s).length() <= SUN_SAME * s.length().max(1.0) {
            return false;
        }
    }
    let input = HeatInput { flow_dir: DVec3::ZERO, q_stag: 0.0, sun: sun_body };
    let mut b = Buffers::new(design);
    thermal::cell_heat(&design.bake, &design.cells, &input, &mut b.scratch, &mut b.heat);
    engine_and_nodes(design, burn / dt, propellant, &mut b);
    let before = th.state.clone();
    let (heat, node_heat, capacity) = (&b.heat, &b.node_heat, &b.node_capacity);
    design.network.step_to(&mut th.state, heat, node_heat, capacity, SINK_K, dt, DEFAULT_SWEEPS, COAST_CONVERGED);
    let change = |now: &[f64], was: &[f64]| now.iter().zip(was).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    let change = change(&th.state.skin, &before.skin).max(change(&th.state.nodes, &before.nodes));
    // The next solve is due when the temperatures, at this solve's rate,
    // will have moved by STEP_CHANGE_K.
    let span = if change > 0.0 { (STEP_CHANGE_K * dt / change).min(MAX_SKIP) } else { MAX_SKIP };
    th.hold = (!last && burn == 0.0).then_some((sun_body, span));
    th.epoch = epoch;
    th.burns.retain(|b| b.1.is_none_or(|end| end.seconds_since(epoch) > 0.0));
    true
}
