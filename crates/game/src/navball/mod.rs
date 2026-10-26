//! The navball (attitude indicator), flight view only: a ball showing the
//! horizon and heading of the reference body's local frame, rotated by the
//! ship's attitude, with velocity/orbit/target markers and flight readouts.
//!
//! The rules (markers, mode switch, angles, ball projection, the throttle
//! bar) are pure functions in [`rules`]; this module gathers state from the
//! sim once a frame ([`update`]), [`draw`] lays out the cluster (D080: the
//! speed and its switch, altitude, V/S, Ap and Pe, SAS and its hold modes,
//! the throttle, the flight state) and [`ball`] paints the ball (an egui
//! mesh, no extra camera).

mod ball;
mod draw;
pub mod rules;

pub use draw::AREA;

use crate::state::SimState;
use crate::tracking::Tracked;
use crate::trajectory::apsides::Apsides;
use bevy::prelude::*;
use glam::DVec3;
use rules::{Angles, Local, Markers, Mode, Ship};
use sim::forces::{ActiveSources, ForceContext};
use sim::frame::NodeId;
use sim::time::Epoch;
use sim::vessel::{Phase, TargetState, Vessel};
use sim::world::World;

/// Readouts refresh at most this often (s) so numbers don't jiggle.
const READOUT_PERIOD: f64 = 0.1;

pub struct NavballPlugin;

impl Plugin for NavballPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Navball>()
            .add_systems(Update, feed_hold.before(crate::state::advance))
            .add_systems(Update, update.after(crate::trajectory::apsides::update))
            .add_systems(bevy_egui::EguiPrimaryContextPass, draw::draw.after(crate::hud::draw));
    }
}

/// What the navball measures against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavTarget {
    Body(NodeId),
    /// A vessel by its stable id.
    Vessel(sim::vessel::VesselId),
}

/// Navball settings and the throttled readouts.
#[derive(Resource, Default)]
pub struct Navball {
    pub mode: Mode,
    /// Holds the surface/orbit mode instead of switching at 36 km.
    pub locked: bool,
    pub target: Option<NavTarget>,
    /// The state this frame (`update`), for the cluster and the menus.
    pub now: Option<NavState>,
    /// Readouts as last shown, and when (app seconds).
    shown: Option<(f64, NavState)>,
}

/// The next apoapsis or periapsis on the predicted trajectory (D076).
#[derive(Clone, Debug, PartialEq)]
pub struct ApsisReadout {
    /// Above the body's reference ellipsoid under it (m).
    pub altitude: f64,
    /// Seconds from now.
    pub in_s: f64,
    /// Its body's name when it is not the reference body.
    pub other_body: Option<String>,
}

/// Everything the navball draws, for one instant.
#[derive(Clone, Debug)]
pub struct NavState {
    pub mode: Mode,
    pub ship: Ship,
    pub local: Local,
    pub markers: Markers,
    pub angles: Angles,
    /// Speed in the current mode (m/s), relative to the reference body (its
    /// surface or centre) or the target.
    pub speed: f64,
    /// Altitude for the current mode, with its label.
    pub altitude: (&'static str, f64),
    pub vertical_speed: f64,
    /// Angle of attack and sideslip (deg), in an atmosphere only.
    pub aoa_sideslip: Option<(f64, f64)>,
    pub g_load: f64,
    /// The next apsides (D076), never osculating.
    pub ap: Option<ApsisReadout>,
    pub pe: Option<ApsisReadout>,
    pub body: String,
    /// Target name and distance (m).
    pub target: Option<(String, f64)>,
    /// The next planned burn: its direction now (inertial) and seconds to
    /// ignition.
    pub maneuver: Option<(DVec3, f64)>,
}

/// A target's name, position and velocity relative to `anchor` at `t`.
pub(crate) fn target_state(
    sim: &SimState,
    tracked: &Tracked,
    target: NavTarget,
    anchor: NodeId,
    t: Epoch,
) -> Option<(String, DVec3, DVec3)> {
    let snap = sim.world.snapshot(t);
    match target {
        NavTarget::Body(node) => {
            let k = snap.relative(node, anchor);
            Some((sim.world.eph.node(node).name.clone(), k.r, k.v))
        }
        NavTarget::Vessel(id) => {
            let i = sim.index_of(id).filter(|&i| i != sim.active)?;
            let (a, r, v) = sim.fleet[i].state_at(&sim.world, t);
            let k = snap.relative(a, anchor);
            Some((tracked.name(id), k.r + r, k.v + v))
        }
    }
}

/// States the navball's references in the controls for the attitude hold
/// (D075): the speed reference (its mode), the reference body (its
/// dominant body) and the target (a body exactly, a vessel at its state
/// now). A control choice, never physics.
fn feed_hold(mut sim: ResMut<SimState>, nav: Res<Navball>) {
    let t = sim.clock;
    let reference = sim.dominant_of(sim.active);
    let target = nav.target.and_then(|tg| match tg {
        NavTarget::Body(node) => Some(TargetState { anchor: node, epoch: t, r: DVec3::ZERO, v: DVec3::ZERO }),
        NavTarget::Vessel(id) => {
            let i = sim.index_of(id).filter(|&i| i != sim.active)?;
            let (anchor, r, v) = sim.fleet[i].state_at(&sim.world, t);
            Some(TargetState { anchor, epoch: t, r, v })
        }
    });
    let c = &mut sim.controls;
    c.speed = rules::speed_reference(nav.mode);
    c.reference = Some(reference);
    c.target = target;
}

/// Proper (non-gravitational) acceleration of `vessel` at its state
/// (`anchor`, `r`, `v`) at `t`: thrust and drag, or the ground's push when
/// landed. Its length over g0 is the g-load.
pub fn proper_accel(world: &World, t: Epoch, vessel: &Vessel, throttle: f64) -> DVec3 {
    let (anchor, r, v) = vessel.state_at(world, t);
    let snap = world.snapshot(t);
    let active = ActiveSources::select(world, &snap, anchor, r);
    let ctx = |drag, thrust| ForceContext { world, anchor, active: &active, drag, thrust };
    let gravity = ctx(None, DVec3::ZERO).accel_with(&snap, r, v);
    match &vessel.phase {
        // Landed and crashed vessels report their body as anchor and co-rotate.
        Phase::Landed { .. } | Phase::Crashed { .. } => {
            let omega = world
                .source(anchor)
                .and_then(|s| s.physical.as_ref())
                .map_or(DVec3::ZERO, |p| p.rotation.omega(t).raw());
            omega.cross(omega.cross(r)) - gravity
        }
        Phase::Powered { .. } => {
            let thrust = vessel.thrust_accel(world, throttle);
            ctx(Some(vessel.drag()), thrust).accel_with(&snap, r, v) - gravity
        }
        Phase::Coasting { .. } => ctx(Some(vessel.drag()), DVec3::ZERO).accel_with(&snap, r, v) - gravity,
    }
}

/// Gathers the navball state for the active vessel, updating the mode.
pub fn nav_state(sim: &SimState, tracked: &Tracked, aps: &Apsides, nav: &mut Navball) -> Option<NavState> {
    let vessel = sim.ship();
    let t = sim.clock;
    let (anchor, r, v) = vessel.state_at(&sim.world, t);
    // The reference is the dominant body (D056's tidal rule): the altitude,
    // speed and markers are relative to it.
    let body = sim.dominant_of(sim.active);
    let src = sim.world.source(body)?;
    let p = src.physical.as_ref()?;
    let k = sim.world.snapshot(t).relative(body, anchor);
    let rel = r - k.r;
    let v_orb = v - k.v;
    let v_srf = v_orb - p.rotation.omega(t).raw().cross(rel);
    let fixed = p.rotation.to_fixed(sim::frame::Vec3::from_raw(rel), t);
    let alt = p.altitude(fixed);
    let above_terrain = p.altitude_above_surface(fixed);

    let target = nav.target.and_then(|tg| target_state(sim, tracked, tg, anchor, t));
    if nav.target.is_some() && target.is_none() {
        nav.target = None;
    }
    nav.mode = rules::auto_mode(nav.mode, nav.locked, alt, target.is_some());
    let v_mode = match (nav.mode, &target) {
        (Mode::Target, Some((_, _, tv))) => v - *tv,
        (Mode::Orbit, _) => v_orb,
        _ => v_srf,
    };
    let local = rules::local_frame(rel, p.rotation.pole(t));
    let ship = rules::ship_axes(vessel.attitude.q);
    let in_atmosphere = p.atmosphere.as_ref().is_some_and(|a| alt < a.top);
    let grounded = matches!(vessel.phase, Phase::Landed { .. } | Phase::Crashed { .. });
    // The next apsides on the predicted trajectory (D076).
    let (next_ap, next_pe) = aps.get(vessel.id()).map_or((None, None), |a| a.next_after(t));
    let readout = |a: Option<&crate::trajectory::apsides::Apsis>| {
        a.map(|a| ApsisReadout {
            altitude: a.altitude,
            in_s: a.t.seconds_since(t),
            other_body: (a.body != body).then(|| sim.world.eph.node(a.body).name.clone()),
        })
    };
    Some(NavState {
        mode: nav.mode,
        ship,
        local,
        markers: if grounded {
            rules::markers(rel, DVec3::ZERO, target.as_ref().map(|(_, tr, _)| *tr - r))
        } else {
            rules::markers(rel, v_mode, target.as_ref().map(|(_, tr, _)| *tr - r))
        },
        angles: rules::attitude_angles(&ship, &local),
        speed: v_mode.length(),
        altitude: rules::mode_altitude(nav.mode, above_terrain, alt),
        vertical_speed: rules::vertical_speed(rel, v_srf),
        aoa_sideslip: if in_atmosphere { rules::aoa_sideslip(&ship, v_srf) } else { None },
        g_load: rules::g_load(proper_accel(&sim.world, t, vessel, sim.controls.throttle)),
        ap: readout(next_ap),
        pe: readout(next_pe),
        body: src.name.clone(),
        target: target.map(|(name, tr, _)| (name, (tr - r).length())),
        maneuver: vessel.plan().burns.iter().find(|b| b.t_start.seconds_since(t) > 0.0).map(|b| {
            let snap = sim.world.snapshot(t);
            (b.law.direction.direction(&snap, anchor, r, v), b.t_start.seconds_since(t))
        }),
    })
}

/// Refreshes [`Navball::now`] (and the auto mode) after the simulation
/// step and the apsides.
pub fn update(sim: Res<SimState>, tracked: Res<Tracked>, aps: Res<Apsides>, mut nav: ResMut<Navball>) {
    let nav = &mut *nav;
    nav.now = nav_state(&sim, &tracked, &aps, nav);
}

impl Navball {
    /// Picks the target (Orbital ▸ Target); without one, target mode falls
    /// back to surface.
    pub fn set_target(&mut self, t: Option<NavTarget>) {
        self.target = t;
        if t.is_none() && self.mode == Mode::Target {
            self.mode = Mode::Surface;
        }
    }

    /// The readouts to show: `state` at most every [`READOUT_PERIOD`].
    fn readouts(&mut self, now: f64, state: &NavState) -> &NavState {
        let stale = self.shown.as_ref().is_none_or(|(t, s)| now - t >= READOUT_PERIOD || s.mode != state.mode);
        if stale {
            self.shown = Some((now, state.clone()));
        }
        &self.shown.as_ref().expect("just set").1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::craft::test_craft;
    use sim::kepler::Elements;
    use sim::vessel::VesselId;
    use std::sync::Arc;

    fn world() -> World {
        let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sim::sol::EPHEMERIS_PATH);
        let eph = sim::ephem::Ephemeris::from_bytes(&std::fs::read(path).expect("ephemeris")).expect("valid");
        World::sol(Arc::new(eph))
    }

    #[test]
    fn g_load_is_one_on_the_pad_zero_in_orbit_and_thrust_when_powered() {
        let w = world();
        let t = sim::sol::sol_epoch();
        let pad = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6, -80.6, t, test_craft());
        let g = rules::g_load(proper_accel(&w, t, &pad, 0.0));
        assert!((g - 1.0).abs() < 0.01, "pad {g}");

        let earth = w.find("Earth").expect("Earth").clone();
        let el = Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 0.3, argp: 0.0, mean_anomaly: 1.0 };
        let (r, v) = el.to_state(earth.gm);
        let leo = Vessel::coasting(&w, VesselId(1), t, earth.node, r, v, test_craft());
        let g = rules::g_load(proper_accel(&w, t, &leo, 0.0));
        assert!(g < 1e-6, "coasting in vacuum {g}");

        let mut burn = leo.clone();
        burn.phase = Phase::Powered { anchor: earth.node, r, v };
        let expected = burn.craft.engine.thrust_vac / burn.mass() / rules::G0;
        let g = rules::g_load(proper_accel(&w, t, &burn, 1.0));
        assert!((g - expected).abs() < 1e-6, "full thrust {g} vs {expected}");
    }
}
