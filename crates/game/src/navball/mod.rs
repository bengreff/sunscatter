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

pub mod altitude;
mod ball;
mod draw;
pub mod g_meter;
pub mod rules;

pub use draw::AREA;

use crate::state::SimState;
use crate::tracking::Tracked;
use crate::trajectory::apsides::Apsides;
use bevy::prelude::*;
use glam::DVec3;
use rules::{Angles, Local, Markers, Mode, Ship, Side};
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
    /// Never switches the mode at the rails floor (D089).
    pub locked: bool,
    /// The active vessel's side of its reference body's rails floor last
    /// frame: crossing it switches the mode (D089).
    side: Option<(sim::vessel::VesselId, Side)>,
    pub target: Option<NavTarget>,
    /// What ALT reads above (the readout's switch, D111).
    pub altitude: altitude::AltitudeRef,
    /// Each body's datum, found once (a heightmap's lowest point).
    datums: std::collections::HashMap<NodeId, altitude::Datum>,
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
    /// Above its body's datum (m; `altitude::altitude`: the ground under
    /// an apsis ahead is not known): what the readouts show.
    pub above_datum: f64,
    /// From its body's centre (m): the orbit's own size, shown beside the
    /// altitude.
    pub distance: f64,
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
    /// Altitude as the switch reads it (`ALT SEA`, `ALT TER`, …), the
    /// switch's setting.
    pub altitude: (String, f64),
    pub altitude_ref: altitude::AltitudeRef,
    /// Angle of attack and sideslip (deg), in an atmosphere only.
    pub aoa_sideslip: Option<(f64, f64)>,
    /// The G row: load, direction and the craft's limit (D111).
    pub g: g_meter::GReadout,
    /// The next apsides (D076), never osculating.
    pub ap: Option<ApsisReadout>,
    pub pe: Option<ApsisReadout>,
    pub body: String,
    /// The reference body (the attitude hold's, `hold_reference`).
    pub reference: NodeId,
    /// Target name and distance (m).
    pub target: Option<(String, f64)>,
    /// The next planned burn: its direction now (inertial) and seconds to
    /// ignition.
    pub maneuver: Option<(DVec3, f64)>,
    /// What the craft knows (D106): the readouts show a dash for what it
    /// cannot measure and the digits its precision gives.
    pub known: crate::format::Known,
}

/// What can be targeted, in the Orbital menu's picker's order and the
/// agent's `set_target`'s (D044): every body with a surface, then every
/// vessel but the active one.
pub fn target_choices(sim: &SimState) -> Vec<NavTarget> {
    let bodies = sim.world.surfaces().map(|src| NavTarget::Body(src.node));
    let others = sim.fleet.iter().map(|v| v.id()).filter(|&id| id != sim.ship().id()).map(NavTarget::Vessel);
    bodies.chain(others).collect()
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
/// (D075): the speed reference (its mode) and the target (a body exactly,
/// a vessel at its state now). The reference body is left to the sim's
/// rule (`hold_reference` in `sim::vessel`, the dominant body as a pure
/// function of time and state): set from the display's dominance each
/// frame, a change of dominant body would restart a coast at a frame time
/// (rule 4). A control choice, never physics.
fn feed_hold(mut sim: ResMut<SimState>, nav: Res<Navball>) {
    let t = sim.clock;
    let target = nav.target.and_then(|tg| match tg {
        NavTarget::Body(node) => Some(TargetState { anchor: node, epoch: t, r: DVec3::ZERO, v: DVec3::ZERO }),
        NavTarget::Vessel(id) => {
            let i = sim.index_of(id).filter(|&i| i != sim.active)?;
            let (anchor, r, v) = sim.fleet[i].state_at(&sim.world, t);
            Some(TargetState { anchor, epoch: t, r, v })
        }
    });
    let c = sim.controls_mut();
    c.speed = rules::speed_reference(nav.mode);
    c.reference = None;
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
    // The reference is the dominant body (D056's tidal rule) as the
    // attitude hold takes it (`hold_reference`, at this instant): the
    // altitude, speed and markers are relative to the body a prograde hold
    // follows.
    let snap = sim.world.snapshot(t);
    let body = sim::vessel::hold_reference(&sim.world, &snap, anchor, r);
    let src = sim.world.source(body)?;
    let p = src.physical.as_ref()?;
    let k = snap.relative(body, anchor);
    let rel = r - k.r;
    let v_orb = v - k.v;
    let v_srf = v_orb - p.rotation.omega(t).raw().cross(rel);
    let fixed = p.rotation.to_fixed(sim::frame::Vec3::from_raw(rel), t);
    let alt = p.altitude(fixed);
    let ground = alt - p.altitude_above_surface(fixed);
    let datum = nav.datum(body, p);

    let target = nav.target.and_then(|tg| target_state(sim, tracked, tg, anchor, t));
    if nav.target.is_some() && target.is_none() {
        nav.target = None;
    }
    // The mode switches where rails warp does (D062's floor, D089).
    let before = nav.side.filter(|(id, _)| *id == vessel.id()).map(|(_, side)| side);
    let side = rules::side(before, alt, p.rails_floor);
    nav.side = Some((vessel.id(), side));
    nav.mode = rules::auto_mode(nav.mode, nav.locked, before, side, target.is_some());
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
    let datums: Vec<(NodeId, altitude::Datum)> = [next_ap, next_pe]
        .iter()
        .flatten()
        .filter_map(|a| Some((a.body, nav.datum(a.body, sim.world.source(a.body)?.physical.as_ref()?))))
        .collect();
    let readout = |a: Option<&crate::trajectory::apsides::Apsis>| {
        a.map(|a| ApsisReadout {
            altitude: a.altitude,
            distance: a.distance,
            above_datum: {
                let d = datums.iter().find(|(b, _)| *b == a.body).map_or(datum, |(_, d)| *d);
                altitude::altitude(altitude::AltitudeRef::Datum, a.altitude, d, None)
            },
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
        altitude: (altitude::label(nav.altitude, datum), altitude::altitude(nav.altitude, alt, datum, Some(ground))),
        altitude_ref: nav.altitude,
        aoa_sideslip: if in_atmosphere { rules::aoa_sideslip(&ship, v_srf) } else { None },
        g: g_readout(sim, vessel, t),
        ap: readout(next_ap),
        pe: readout(next_pe),
        body: src.name.clone(),
        reference: body,
        target: target.map(|(name, tr, _)| (name, (tr - r).length())),
        maneuver: vessel.plan().burns.iter().find(|b| b.t_start.seconds_since(t) > 0.0).map(|b| {
            let snap = sim.world.snapshot(t);
            (b.law.direction.direction(&snap, anchor, r, v), b.t_start.seconds_since(t))
        }),
        known: crate::format::Known::of(vessel),
    })
}

/// The G row of `vessel` (the active one): its proper acceleration in
/// body axes and, for a craft of parts, its g-limit.
fn g_readout(sim: &SimState, vessel: &Vessel, t: Epoch) -> g_meter::GReadout {
    let a = vessel.attitude.q.inverse() * proper_accel(&sim.world, t, vessel, sim.controls().throttle);
    let def = vessel.ship_def();
    let limit = def.as_ref().zip(vessel.ship()).and_then(|(d, s)| sim::systems::g_limit::g_limit(d, s));
    g_meter::readout(a, limit, |j| def.as_ref().and_then(|d| d.joints.get(j)).map(|j| j.name.clone()))
}

/// Refreshes [`Navball::now`] (and the auto mode) after the simulation
/// step and the apsides.
pub fn update(sim: Res<SimState>, tracked: Res<Tracked>, aps: Res<Apsides>, mut nav: ResMut<Navball>) {
    let nav = &mut *nav;
    nav.now = nav_state(&sim, &tracked, &aps, nav);
}

impl Navball {
    /// The frame the active vessel's line is drawn in (D089), once the
    /// navball has a state.
    pub fn line_frame(&self) -> Option<crate::trajectory::LineFrame> {
        let now = self.now.as_ref()?;
        let target_body = match self.target {
            Some(NavTarget::Body(node)) => Some(node),
            _ => None,
        };
        Some(rules::line_frame(now.mode, now.reference, target_body))
    }

    /// Picks the target (Orbital ▸ Target); without one, target mode falls
    /// back to surface.
    pub fn set_target(&mut self, t: Option<NavTarget>) {
        self.target = t;
        if t.is_none() && self.mode == Mode::Target {
            self.mode = Mode::Surface;
        }
    }

    /// The datum of `body` (found once per body).
    fn datum(&mut self, body: NodeId, p: &sim::body::BodyPhysical) -> altitude::Datum {
        *self.datums.entry(body).or_insert_with(|| altitude::body_datum(p))
    }

    /// The readouts to show: `state` at most every [`READOUT_PERIOD`], at
    /// once when the mode or the altitude's switch changes.
    fn readouts(&mut self, now: f64, state: &NavState) -> &NavState {
        let changed = |s: &NavState| s.mode != state.mode || s.altitude_ref != state.altitude_ref;
        let stale = self.shown.as_ref().is_none_or(|(t, s)| now - t >= READOUT_PERIOD || changed(s));
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
        let pad = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6, -80.6, t, &sim::craft::small::legs_out());
        let g = proper_accel(&w, t, &pad, 0.0).length() / rules::G0;
        assert!((g - 1.0).abs() < 0.01, "pad {g}");

        let earth = w.find("Earth").expect("Earth").clone();
        let el = Elements { a: 6_778_137.0, e: 0.001, i: 0.9, raan: 0.3, argp: 0.0, mean_anomaly: 1.0 };
        let (r, v) = el.to_state(earth.gm);
        let leo = Vessel::coasting(&w, VesselId(1), t, earth.node, r, v, sim::craft::small::rocket_ref());
        let g = proper_accel(&w, t, &leo, 0.0).length() / rules::G0;
        assert!(g < 1e-6, "coasting in vacuum {g}");

        // Its engines lit and run up (a 1 s start) at full throttle.
        let mut burn = leo.clone();
        burn.phase = Phase::Powered { anchor: earth.node, r, v };
        burn.next_stage(&w).expect("the ignition");
        burn.advance(
            &w,
            t.add_seconds(3.0),
            &sim::vessel::Controls { throttle: 1.0, ..Default::default() },
            usize::MAX,
        );
        let expected = burn.craft.engine.thrust_vac / burn.mass() / rules::G0;
        let g = proper_accel(&w, burn.time, &burn, 1.0).length() / rules::G0;
        assert!((g - expected).abs() < 1e-6, "full thrust {g} vs {expected}");
    }
}
