//! Landing aids (playtest-1 §C): the landing panel and the impact marker,
//! read from `sim::landing` (the vessel's own force model integrated to
//! the ground, surface-retrograde attitude, current throttle and chute).
//!
//! The prediction runs on a background task a few times a second and is
//! swapped in when complete. The marker is the body-fixed impact point
//! carried by the body's rotation to the current clock.

use crate::format;
use crate::interface::theme;
use crate::map;
use crate::state::SimState;
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};
use bevy_egui::{egui, EguiContexts};
use sim::craft::CraftParams;
use sim::landing::{self, AssumedAttitude, Braking, Impact, LandingStart, Limits};
use sim::time::Epoch;
use sim::vessel::{Phase, Vessel, VesselId};
use sim::world::World;

/// Below this radar altitude the landing panel shows (m).
pub const PANEL_BELOW: f64 = 20_000.0;
/// The panel also shows when the impact is this close (s).
pub const PANEL_WITHIN: f64 = 900.0;
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

/// The prediction start for a vessel flying with `throttle` and `chute`.
pub fn start_of(world: &World, vessel: &Vessel, throttle: f64, chute: bool) -> Option<LandingStart> {
    LandingStart::of_vessel(world, vessel, throttle, chute, AssumedAttitude::SurfaceRetrograde)
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
    let Some(start) = start_of(&sim.world, ship, sim.controls.throttle, sim.controls.chute) else {
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

/// The landing panel and the impact marker.
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    cam: Query<(&Camera, &Transform, &Projection), With<crate::camera::MainCamera>>,
    rig: Res<crate::camera::CameraRig>,
    station: Res<crate::tracking::TrackingStation>,
    pred: Res<LandingPrediction>,
) -> Result {
    if station.open {
        return Ok(());
    }
    let ship = sim.ship();
    if matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. }) {
        return Ok(());
    }
    let latest = pred.latest.filter(|l| l.vessel == ship.id());
    let hit = latest.and_then(|l| l.impact).filter(|h| from_now(h.t, sim.clock) > 0.0);
    let ctx = contexts.ctx_mut()?;
    // The impact point, where that ground is now, in any view.
    if let (Some(hit), Some(view)) = (hit, map::view(&cam)) {
        let c = hit.ground_at(&sim.world, sim.clock, rig.anchor) - rig.cam_pos;
        if let Some(s) = view.project(c) {
            let painter = ctx.layer_painter(egui::LayerId::background());
            let red = egui::Color32::from_rgb(255, 80, 60);
            painter.circle_stroke(egui::pos2(s.x, s.y), 7.0, egui::Stroke::new(2.0, red));
            painter.text(
                egui::pos2(s.x + 10.0, s.y),
                egui::Align2::LEFT_CENTER,
                format!("impact in {}", format::duration(from_now(hit.t, sim.clock))),
                egui::FontId::monospace(12.0),
                red,
            );
        }
    }
    let (anchor, r, v) = ship.state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let Some(body) = landing::nearest_surface(&sim.world, &snap, anchor, r) else { return Ok(()) };
    let m = landing::surface_motion(&sim.world, &snap, anchor, body, r, v);
    let radar = m.height - ship.contact_height();
    let near_impact = hit.is_some_and(|h| from_now(h.t, sim.clock) < PANEL_WITHIN);
    if radar > PANEL_BELOW && !near_impact {
        return Ok(());
    }
    let twr = landing::twr(&ship.craft, ship.propellant(), m.pressure, m.gravity);
    let braking = latest.and_then(|l| l.braking);
    egui::Window::new("Landing").anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0]).resizable(false).show(ctx, |ui| {
        let dim = |t: &str| egui::RichText::new(t).monospace().small().color(theme::DIM);
        egui::Grid::new("landing_grid").num_columns(2).show(ui, |ui| {
            let mut row = |label: &str, value: String, warn: bool| {
                ui.label(dim(label));
                ui.label(egui::RichText::new(value).monospace().color(if warn { theme::WARN } else { theme::TEXT }));
                ui.end_row();
            };
            row("RADAR ALT", format::distance(radar), false);
            row("V/S", format!("{:+.1} m/s", m.v_vertical), false);
            row("H SPEED", format!("{:.1} m/s", m.v_horizontal), false);
            row("TWR", format!("{twr:.2}"), twr < 1.0);
            match hit {
                Some(h) => {
                    row("IMPACT IN", format::duration(from_now(h.t, sim.clock)), false);
                    row("IMPACT V/S", format!("{:+.1} m/s", h.v_vertical), false);
                    row("IMPACT H", format!("{:.1} m/s", h.v_horizontal), false);
                    row("AT", format!("{:+.3}° {:+.3}°", h.lat.to_degrees(), h.lon.to_degrees()), false);
                }
                None => row("IMPACT", "none".into(), false),
            }
            match (hit, braking) {
                (_, Some(b)) => {
                    let t = from_now(b.t_ignite, sim.clock);
                    row("BURN IN", burn_in(t), t < 10.0);
                    row("IGNITE AT", format::distance(b.ignite_height), false);
                }
                (Some(h), None) if from_now(h.t, sim.clock) < BRAKING_WITHIN => {
                    row("BRAKE", "cannot stop".into(), true);
                }
                _ => {}
            }
        });
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let start = start_of(&sim.world, ship, 0.0, false).expect("flying");
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
