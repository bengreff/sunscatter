//! Landing aids (playtest-1 §C): the landing prediction and the impact
//! marker, read from `sim::landing` (the vessel's own force model
//! integrated to the ground with the attitude its controls will hold, the
//! current throttle and chute). Its numbers are shown in the
//! Landing/Reentry menu (`interface::menus`), and the marker only while
//! that menu is open (D080).
//!
//! The prediction runs on a background task a few times a second and is
//! swapped in when complete. The marker is the body-fixed impact point
//! carried by the body's rotation to the current clock.

use crate::format;
use crate::map;
use crate::state::SimState;
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};
use bevy_egui::{egui, EguiContexts};
use glam::DQuat;
use sim::craft::CraftParams;
use sim::landing::{self, AssumedAttitude, Braking, Impact, LandingStart, Limits};
use sim::time::Epoch;
use sim::vessel::{Controls, HoldMode, Phase, SpeedReference, Vessel, VesselId};
use sim::world::World;

/// Braking solutions are computed for impacts this close (s).
pub const BRAKING_WITHIN: f64 = 1_800.0;
/// The braking solution stops the lowest contact point this high (m).
pub const BRAKING_MARGIN: f64 = 10.0;
/// Seconds between background predictions.
const EVERY: f64 = 0.25;

/// One prediction for a vessel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub vessel: VesselId,
    pub impact: Option<Impact>,
    pub braking: Option<Braking>,
}

/// The attitude the prediction assumes from the controls: what the hold
/// will do (a surface prograde/retrograde hold follows the surface
/// velocity; any other hold keeps the current attitude, which is exact for
/// stability and a fair guess for the rest), and with SAS off a stable
/// craft weathervanes nose into the wind (D074).
pub fn assumed_attitude(c: &Controls, q: DQuat) -> AssumedAttitude {
    match (c.sas, c.hold, c.speed) {
        (false, _, _) => AssumedAttitude::SurfacePrograde,
        (true, HoldMode::Retrograde, SpeedReference::Surface) => AssumedAttitude::SurfaceRetrograde,
        (true, HoldMode::Prograde, SpeedReference::Surface) => AssumedAttitude::SurfacePrograde,
        (true, _, _) => AssumedAttitude::Inertial(q),
    }
}

/// The prediction start for a vessel flying with `controls`.
pub fn start_of(world: &World, vessel: &Vessel, controls: &Controls) -> Option<LandingStart> {
    let attitude = assumed_attitude(controls, vessel.attitude.q);
    LandingStart::of_vessel(world, vessel, controls.throttle, controls.chute, attitude)
}

/// Predicts the impact from `start` and, when it is near, the braking
/// solution.
pub fn predict(world: &World, craft: &CraftParams, vessel: VesselId, start: &LandingStart) -> Landing {
    let descent = landing::predict_impact(world, craft, start, Limits::two_orbits(world, start));
    let braking = descent
        .impact
        .filter(|h| h.t.seconds_since(start.t) < BRAKING_WITHIN)
        .and_then(|_| landing::braking_solution(world, craft, &descent, BRAKING_MARGIN));
    Landing { vessel, impact: descent.impact, braking }
}

/// The latest prediction for the active vessel, and the one being computed.
#[derive(Resource, Default)]
pub struct LandingPrediction {
    pub latest: Option<Landing>,
    task: Option<Task<Landing>>,
    since_last: f64,
}

/// Starts a background prediction for the active vessel every [`EVERY`]
/// seconds (one at a time) and swaps in the finished one.
pub fn update(time: Res<Time>, sim: Res<SimState>, mut pred: ResMut<LandingPrediction>) {
    if let Some(task) = pred.task.as_mut() {
        if let Some(done) = check_ready(task) {
            pred.latest = Some(done);
            pred.task = None;
        }
    }
    let ship = sim.ship();
    if pred.latest.is_some_and(|l| l.vessel != ship.id()) {
        pred.latest = None;
    }
    pred.since_last += time.delta_secs_f64();
    if pred.task.is_some() || pred.since_last < EVERY {
        return;
    }
    pred.since_last = 0.0;
    let Some(start) = start_of(&sim.world, ship, sim.controls()) else {
        pred.latest = None;
        return;
    };
    let (world, craft, id) = (sim.world.clone(), ship.craft.clone(), ship.id());
    pred.task = Some(AsyncComputeTaskPool::get().spawn(async move { predict(&world, &craft, id, &start) }));
}

/// Signed seconds from the clock to `t`.
fn from_now(t: Epoch, clock: Epoch) -> f64 {
    t.seconds_since(clock)
}

/// "BURN IN" text: the time to ignition, or NOW once it has passed.
pub fn burn_in(seconds: f64) -> String {
    if seconds > 0.0 {
        format::duration(seconds)
    } else {
        "NOW".into()
    }
}

/// Whether the impact marker is drawn (D080): only while the
/// Landing/Reentry menu is open, in the flight view, for a vessel still
/// flying towards a predicted impact.
pub fn impact_marker_shown(landing_open: bool, station_open: bool, grounded: bool, impact_ahead: bool) -> bool {
    landing_open && !station_open && !grounded && impact_ahead
}

/// Whether the impact marker was drawn this frame (the demo checks it).
#[derive(Resource, Default)]
pub struct ImpactMarker {
    pub drawn: bool,
}

/// The impact marker: the predicted impact point where that ground is now,
/// in any view, while the Landing/Reentry menu is open.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    cam: Query<(&Camera, &Transform, &Projection), With<crate::camera::MainCamera>>,
    rig: Res<crate::camera::CameraRig>,
    station: Res<crate::tracking::TrackingStation>,
    pred: Res<LandingPrediction>,
    iface: Res<crate::interface::layout::InterfaceSettings>,
    mut marker: ResMut<ImpactMarker>,
) -> Result {
    marker.drawn = false;
    let ship = sim.ship();
    let grounded = matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. });
    let latest = pred.latest.filter(|l| l.vessel == ship.id());
    let hit = latest.and_then(|l| l.impact).filter(|h| from_now(h.t, sim.clock) > 0.0);
    let open = iface.menus.is_open(crate::interface::menus::MenuId::Landing);
    let Some(hit) = hit.filter(|_| impact_marker_shown(open, station.open, grounded, true)) else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let Some(view) = map::view(&cam) else { return Ok(()) };
    let c = hit.ground_at(&sim.world, sim.clock, rig.anchor) - rig.cam_pos;
    let Some(s) = view.project(c) else { return Ok(()) };
    // Not through the HUD's translucent panels: hidden where it falls
    // behind the navball cluster, a dock or a top panel.
    let hud = crate::interface::hud_rects(ctx);
    let at = egui::pos2(s.x, s.y);
    if crate::interface::layout::covered(egui::Rect::from_center_size(at, egui::vec2(18.0, 18.0)), &hud) {
        return Ok(());
    }
    let painter = ctx.layer_painter(egui::LayerId::background());
    let red = egui::Color32::from_rgb(255, 80, 60);
    painter.circle_stroke(at, 7.0, egui::Stroke::new(2.0, red));
    let text = format!("impact in {}", format::duration(from_now(hit.t, sim.clock)));
    let label = painter.layout_no_wrap(text, egui::FontId::monospace(12.0), red);
    let label_rect = egui::Rect::from_min_size(egui::pos2(s.x + 10.0, s.y - label.size().y * 0.5), label.size());
    if !crate::interface::layout::covered(label_rect, &hud) {
        painter.galley(label_rect.min, label, red);
    }
    marker.drawn = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_assumed_attitude_follows_the_controls() {
        let q = DQuat::from_xyzw(0.0, 0.6, 0.0, 0.8);
        let c = |sas, hold, speed| Controls { sas, hold, speed, ..Controls::default() };
        let cases = [
            (c(false, HoldMode::Retrograde, SpeedReference::Surface), AssumedAttitude::SurfacePrograde),
            (c(true, HoldMode::Retrograde, SpeedReference::Surface), AssumedAttitude::SurfaceRetrograde),
            (c(true, HoldMode::Prograde, SpeedReference::Surface), AssumedAttitude::SurfacePrograde),
            (c(true, HoldMode::Retrograde, SpeedReference::Orbit), AssumedAttitude::Inertial(q)),
            (c(true, HoldMode::Stability, SpeedReference::Surface), AssumedAttitude::Inertial(q)),
            (c(true, HoldMode::Maneuver, SpeedReference::Orbit), AssumedAttitude::Inertial(q)),
        ];
        for (controls, want) in cases {
            assert_eq!(assumed_attitude(&controls, q), want, "{controls:?}");
        }
    }

    #[test]
    fn the_impact_marker_shows_only_with_the_landing_menu_open() {
        // (landing open, station open, grounded, impact ahead, shown)
        let table = [
            (true, false, false, true, true),
            (false, false, false, true, false),
            (true, true, false, true, false),
            (true, false, true, true, false),
            (true, false, false, false, false),
            (false, true, true, false, false),
        ];
        for (open, station, grounded, ahead, want) in table {
            assert_eq!(
                impact_marker_shown(open, station, grounded, ahead),
                want,
                "{open} {station} {grounded} {ahead}"
            );
        }
    }

    #[test]
    fn burn_in_counts_down_then_says_now() {
        assert_eq!(burn_in(-1.0), "NOW");
        assert_eq!(burn_in(0.0), "NOW");
        assert_eq!(burn_in(65.0), format::duration(65.0));
    }

    #[test]
    fn a_falling_ship_predicts_its_impact_and_braking() {
        let mut sim = SimState::new();
        // 3 km above the Moon's surface falling at 50 m/s.
        let moon = sim.world.find("Moon").unwrap().clone();
        let p = moon.physical.as_ref().unwrap();
        let fixed = p.ground_point(0.1, 0.2, 3_000.0);
        let r = p.rotation.to_inertial(fixed, sim.clock).raw();
        let v = p.rotation.omega(sim.clock).raw().cross(r) - r.normalize() * 50.0;
        let id = sim.vessel_ids.allocate();
        let ship = Vessel::coasting(&sim.world, id, sim.clock, moon.node, r, v, sim::craft::test_craft());
        sim.fleet.push(ship);
        let ship = sim.fleet.last().unwrap();
        let start = start_of(&sim.world, ship, &Controls::default()).expect("flying");
        let l = predict(&sim.world, &ship.craft, id, &start);
        let hit = l.impact.expect("it lands");
        let dt = hit.t.seconds_since(sim.clock);
        assert!(dt > 20.0 && dt < 60.0, "{dt}");
        assert!(hit.v_vertical < -50.0 && hit.v_horizontal < 1.0, "{hit:?}");
        let b = l.braking.expect("a full tank can stop");
        assert!(b.t_ignite > sim.clock && b.t_ignite < hit.t);
        assert!(b.ignite_height > 0.0 && b.ignite_height < 3_000.0);
    }
}
