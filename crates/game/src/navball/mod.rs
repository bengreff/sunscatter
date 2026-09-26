//! The navball (attitude indicator), flight view only: a ball showing the
//! horizon and heading of the reference body's local frame, rotated by the
//! ship's attitude, with velocity/orbit/target markers and flight readouts.
//!
//! The rules (markers, mode switch, angles, ball projection) are pure
//! functions in [`rules`]; this module gathers state from the sim and
//! [`draw`] paints it with egui (the ball is an egui mesh, no extra camera).

mod draw;
pub mod rules;

use crate::relations;
use crate::state::SimState;
use crate::tracking::Tracked;
use bevy::prelude::*;
use glam::DVec3;
use rules::{Angles, Local, Markers, Mode, Ship};
use sim::forces::{ActiveSources, ForceContext};
use sim::frame::NodeId;
use sim::time::Epoch;
use sim::vessel::{Phase, Vessel};
use sim::world::World;

/// Readouts refresh at most this often (s) so numbers don't jiggle.
const READOUT_PERIOD: f64 = 0.1;

pub struct NavballPlugin;

impl Plugin for NavballPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Navball>()
            .add_systems(bevy_egui::EguiPrimaryContextPass, draw::draw.after(crate::hud::draw));
    }
}

/// What the navball measures against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavTarget {
    Body(NodeId),
    /// A vessel by its tracking id (stable across fleet changes).
    Vessel(u64),
}

/// Navball settings and the throttled readouts.
#[derive(Resource, Default)]
pub struct Navball {
    pub mode: Mode,
    /// Holds the surface/orbit mode instead of switching at 36 km.
    pub locked: bool,
    pub target: Option<NavTarget>,
    /// Readouts as last shown, and when (app seconds).
    shown: Option<(f64, NavState)>,
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
    pub time_to_ap: Option<f64>,
    pub time_to_pe: Option<f64>,
    pub body: String,
    /// Target name and distance (m).
    pub target: Option<(String, f64)>,
}

/// A target's name, position and velocity relative to `anchor` at `t`.
fn target_state(
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
            let i = (0..sim.fleet.len()).find(|&i| tracked.id(i) == Some(id)).filter(|&i| i != sim.active)?;
            let (a, r, v) = sim.fleet[i].state_at(&sim.world, t);
            let k = snap.relative(a, anchor);
            Some((tracked.name(i), k.r + r, k.v + v))
        }
    }
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
            let p = &vessel.params;
            let thrust = vessel.attitude.nose() * (throttle.clamp(0.0, 1.0) * p.max_thrust / p.mass);
            ctx(Some(vessel.drag()), thrust).accel_with(&snap, r, v) - gravity
        }
        Phase::Coasting { .. } => ctx(Some(vessel.drag()), DVec3::ZERO).accel_with(&snap, r, v) - gravity,
    }
}

/// Gathers the navball state for the active vessel, updating the mode.
pub fn nav_state(sim: &SimState, tracked: &Tracked, nav: &mut Navball) -> Option<NavState> {
    let vessel = sim.ship();
    let t = sim.clock;
    let (anchor, r, v) = vessel.state_at(&sim.world, t);
    // The reference is the dominant body (D056's tidal rule): the altitude,
    // speed and markers are relative to it.
    let eph = &sim.world.eph;
    let body = relations::Dominance::new(eph, t).of(eph, t, anchor, r, None, None);
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
    let in_atmosphere = p.atmosphere.is_some_and(|a| alt < a.top);
    let orbit = relations::orbit_about(&sim.world, t, anchor, r, v, body);
    let (time_to_ap, time_to_pe) = orbit.map_or((None, None), |o| rules::time_to_apsides(&o.elements, o.mu));
    Some(NavState {
        mode: nav.mode,
        ship,
        local,
        markers: rules::markers(rel, v_mode, target.as_ref().map(|(_, tr, _)| *tr - r)),
        angles: rules::attitude_angles(&ship, &local),
        speed: v_mode.length(),
        altitude: rules::mode_altitude(nav.mode, above_terrain, alt),
        vertical_speed: rules::vertical_speed(rel, v_srf),
        aoa_sideslip: if in_atmosphere { rules::aoa_sideslip(&ship, v_srf) } else { None },
        g_load: rules::g_load(proper_accel(&sim.world, t, vessel, sim.controls.throttle)),
        time_to_ap,
        time_to_pe,
        body: src.name.clone(),
        target: target.map(|(name, tr, _)| (name, (tr - r).length())),
    })
}

impl Navball {
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
    use sim::kepler::Elements;
    use sim::vessel::VesselParams;
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
        let pad = Vessel::landed_at(&w, "Earth", 28.6, -80.6, t, VesselParams::block());
        let g = rules::g_load(proper_accel(&w, t, &pad, 0.0));
        assert!((g - 1.0).abs() < 0.01, "pad {g}");

        let earth = w.find("Earth").expect("Earth").clone();
        let el = Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 0.3, argp: 0.0, mean_anomaly: 1.0 };
        let (r, v) = el.to_state(earth.gm);
        let leo = Vessel::coasting(&w, t, earth.node, r, v, VesselParams::block());
        let g = rules::g_load(proper_accel(&w, t, &leo, 0.0));
        assert!(g < 1e-6, "coasting in vacuum {g}");

        let mut burn = leo.clone();
        burn.phase = Phase::Powered { anchor: earth.node, r, v };
        let expected = VesselParams::block().max_thrust / VesselParams::block().mass / rules::G0;
        let g = rules::g_load(proper_accel(&w, t, &burn, 1.0));
        assert!((g - expected).abs() < 1e-6, "full thrust {g} vs {expected}");
    }
}
