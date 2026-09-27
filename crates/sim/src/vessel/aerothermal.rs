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
//! and one long jump give the same bits. While the network has settled and
//! the sunlight in body axes has not changed, lattice points are skipped
//! for up to [`MAX_SKIP`] and the next solve spans them (backward Euler is
//! stable at any step; [`coast_step`]): a vessel in steady sunlight solves
//! once an hour.
//! Landed and crashed vessels keep their temperatures (ground heat
//! exchange comes later).
//!
//! Radiation goes to deep space ([`SINK_K`]) everywhere for now: planet IR,
//! albedo and convective cooling come later.

use crate::aero::{self, Flow};
use crate::craft::CraftDesign;
use crate::ephem::Snapshot;
use crate::frame::{NodeId, Vec3};
use crate::thermal::{self, HeatInput, ThermalState, DEFAULT_SWEEPS};
use crate::time::Epoch;
use crate::world::World;
use glam::{DQuat, DVec3};

/// The coast thermal lattice (s).
pub const LATTICE: f64 = 60.0;
/// Radiative sink temperature (K): the cosmic background.
pub const SINK_K: f64 = 2.725;
/// Temperature of a new vessel (K).
pub const INITIAL_K: f64 = 290.0;
/// A solve that changed no temperature by more than this per lattice step
/// (K) has settled.
const SETTLED_K: f64 = 1e-2;
/// Longest run of skipped lattice points (s).
pub const MAX_SKIP: f64 = 3_600.0;
/// A settled network is not stepped while the sunlight in body axes stays
/// within this fraction of what it was solved with (or of 1 W/m²).
const SUN_SAME: f64 = 1e-3;
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
    /// Coasts: the sunlight (body axes, W/m²) of the last solved lattice
    /// step, if that step settled.
    settled: Option<DVec3>,
}

impl VesselThermal {
    pub fn uniform(cells: usize, t: f64, epoch: Epoch) -> Self {
        VesselThermal { state: ThermalState::uniform(cells, t), epoch, lattice: epoch, settled: None }
    }

    /// Hottest skin cell (K).
    pub fn max_skin(&self) -> f64 {
        self.state.skin.iter().fold(0.0, |m, &t| m.max(t))
    }

    /// Restarts the clock at `epoch` without stepping (landed vessels).
    pub fn restart(&mut self, epoch: Epoch) {
        self.epoch = epoch;
        self.lattice = epoch;
        self.settled = None;
    }

    /// The next coast lattice point.
    pub fn next_lattice(&self) -> Epoch {
        self.lattice.add_seconds(LATTICE)
    }
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
    pub gamma: f64,
    pub sutton_graves_k: f64,
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
        let Some(atm) = p.atmosphere else { continue };
        let k = snap.relative(s.node, anchor);
        let d = r - k.r;
        if d.length() - p.radius_eq >= atm.top {
            continue;
        }
        let rho = atm.density(p.altitude(p.rotation.to_fixed(Vec3::from_raw(d), snap.t)));
        if rho <= 0.0 {
            continue;
        }
        let v_air = k.v + p.rotation.omega(snap.t).raw().cross(d);
        let g0 = s.gm / (p.radius_eq * p.radius_eq);
        let temperature = atm.temperature(g0);
        return Some(Air {
            body: s.node,
            rho,
            wind: v_air - v,
            speed_of_sound: aero::air::speed_of_sound(atm.gamma, temperature, atm.molar_mass),
            mean_free_path: atm.mean_free_path_at(rho),
            gamma: atm.gamma,
            sutton_graves_k: atm.sutton_graves_k,
        });
    }
    None
}

/// Whether `r` (anchor-relative) is below `margin` above the top of any
/// body's atmosphere (altitude above the ellipsoid, like the density).
pub fn in_atmosphere(world: &World, snap: &Snapshot, anchor: NodeId, r: DVec3, margin: f64) -> bool {
    world.sources.iter().any(|s| {
        let Some(p) = s.physical.as_ref() else { return false };
        let Some(atm) = p.atmosphere else { return false };
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
            com,
            chute,
        })
    }

    /// The flow in body axes for attitude `q` (body → inertial).
    pub fn flow(&self, q: DQuat) -> Flow {
        let dir = (q.inverse() * self.wind_dir).normalize();
        Flow { dir, q: self.q, mach: self.mach, knudsen: self.knudsen, gamma: self.gamma }
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

/// Scratch buffers of the heat inputs.
struct Buffers {
    heat: Vec<f64>,
    scratch: Vec<f64>,
}

impl Buffers {
    fn new(n: usize) -> Self {
        Buffers { heat: vec![0.0; n], scratch: vec![0.0; n] }
    }
}

/// One live tick of heating: `state` advanced by `dt` with the flow and
/// stagnation flux of `air` (if any) and the sunlight `sun_body` (W/m²,
/// body axes). The flow is taken at attitude `q`.
pub fn live_step(
    design: &CraftDesign,
    th: &mut VesselThermal,
    tick: Option<(&AeroTick, &Air)>,
    q: DQuat,
    sun_body: DVec3,
    dt: f64,
) {
    let mut input = HeatInput { flow_dir: DVec3::ZERO, q_stag: 0.0, sun: sun_body };
    if let Some((aero_tick, air)) = tick {
        let flow = aero_tick.flow(q.normalize());
        let rn = design.bake.nose_radius_at(flow.dir);
        input.flow_dir = flow.dir;
        input.q_stag = thermal::sutton_graves(air.sutton_graves_k, air.rho, rn, air.wind.length());
    }
    let mut b = Buffers::new(design.cells.len());
    thermal::cell_heat(&design.bake, &design.cells, &input, &mut b.scratch, &mut b.heat);
    design.network.step(&mut th.state, &b.heat, 0.0, SINK_K, dt, DEFAULT_SWEEPS);
    th.restart(th.epoch.add_seconds(dt));
}

/// The coast step at `epoch` (a lattice point; or, with `last`, the end of
/// the coast) with the sunlight `sun_body` (W/m², body axes): solves from
/// the state's epoch (spanning skipped lattice points), or skips while
/// settled under the same sunlight for less than [`MAX_SKIP`]. Returns
/// whether the network was solved.
pub fn coast_step(design: &CraftDesign, th: &mut VesselThermal, sun_body: DVec3, epoch: Epoch, last: bool) -> bool {
    let dt = epoch.seconds_since(th.epoch);
    th.lattice = epoch;
    if dt <= 0.0 {
        return false;
    }
    if let Some(s) = th.settled.filter(|_| !last && dt < MAX_SKIP) {
        if (sun_body - s).length() <= SUN_SAME * s.length().max(1.0) {
            return false;
        }
    }
    let input = HeatInput { flow_dir: DVec3::ZERO, q_stag: 0.0, sun: sun_body };
    let mut b = Buffers::new(design.cells.len());
    thermal::cell_heat(&design.bake, &design.cells, &input, &mut b.scratch, &mut b.heat);
    let before = th.state.clone();
    design.network.step(&mut th.state, &b.heat, 0.0, SINK_K, dt, DEFAULT_SWEEPS);
    let change = (th.state.skin.iter().zip(&before.skin))
        .map(|(a, b)| (a - b).abs())
        .fold((th.state.internal - before.internal).abs(), f64::max);
    th.settled = (!last && change <= SETTLED_K * (dt / LATTICE)).then_some(sun_body);
    th.epoch = epoch;
    true
}
