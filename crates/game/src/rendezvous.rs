//! Rendezvous tools (realism-1 §6c): the closest approaches between the
//! active vessel and the navball's target, a vessel (both stored
//! trajectories) or a body (its ephemeris: the pass over the Moon after a
//! transfer burn), from `sim::approach`, refreshed twice a second of real
//! time. Shown on the map (a marker on each line) and in the HUD.

use crate::camera::CameraRig;
use crate::format;
use crate::hud::UiState;
use crate::map::{self, MapView};
use crate::navball::{NavTarget, Navball};
use crate::state::SimState;
use crate::trajectory::Plotter;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use glam::DVec3;
use sim::approach::{closest_approaches, Approach};
use sim::time::Epoch;

/// How often the approaches are recomputed (s of real time).
const REFRESH: f64 = 0.5;
/// Samples over the common computed span.
const SAMPLES: usize = 400;

#[derive(Resource, Default)]
pub struct Rendezvous {
    pub target: Option<NavTarget>,
    /// Nearest first.
    pub approaches: Vec<Approach>,
    last: f64,
}

pub fn update(time: Res<Time>, sim: Res<SimState>, nav: Res<Navball>, mut rv: ResMut<Rendezvous>) {
    let now = time.elapsed_secs_f64();
    let tgt = nav.target;
    if tgt == rv.target && now - rv.last < REFRESH {
        return;
    }
    rv.last = now;
    rv.target = tgt;
    rv.approaches.clear();
    let Some(a) = sim.ship().trajectory() else { return };
    let t0 = sim.clock;
    rv.approaches = match tgt {
        Some(NavTarget::Vessel(id)) => {
            let Some(b) = sim.index_of(id).and_then(|j| sim.fleet[j].trajectory()) else { return };
            let t1 =
                std::cmp::min_by(a.computed_until(), b.computed_until(), |x, y| x.seconds_since(*y).total_cmp(&0.0));
            if t1.seconds_since(t0) <= 0.0 {
                return;
            }
            closest_approaches(&sim.world, |t| a.eval(t), |t| b.eval(t), t0, t1, SAMPLES)
        }
        Some(NavTarget::Body(node)) => {
            let t1 = a.computed_until();
            if t1.seconds_since(t0) <= 0.0 {
                return;
            }
            let body = |_: Epoch| Some((node, DVec3::ZERO, DVec3::ZERO));
            closest_approaches(&sim.world, |t| a.eval(t), body, t0, t1, SAMPLES)
        }
        None => Vec::new(),
    };
}

/// Marks the nearest approach on both lines.
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    ui: Res<UiState>,
    map: Res<MapView>,
    rv: Res<Rendezvous>,
    cam: Query<(&Camera, &Transform, &Projection), With<crate::camera::MainCamera>>,
) -> Result {
    let Some(ca) = rv.approaches.first() else { return Ok(()) };
    // The target's own marker only for a vessel (a body is drawn itself).
    let other = match rv.target {
        Some(NavTarget::Vessel(id)) => match sim.index_of(id) {
            Some(j) => Some(j),
            None => return Ok(()),
        },
        _ => None,
    };
    let Some(view) = map::view(&cam) else { return Ok(()) };
    let Some(plotter) = Plotter::new(&sim, &rig, ui.plot_frame) else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let painter = ctx.layer_painter(egui::LayerId::background());
    let colour = egui::Color32::from_rgb(200, 120, 255);
    for (k, v) in std::iter::once(sim.ship()).chain(other.map(|j| &sim.fleet[j])).enumerate() {
        let Some((anchor, r, _)) = v.trajectory().and_then(|tr| tr.eval(ca.t)) else { continue };
        let c = plotter.plot(anchor, r, ca.t);
        let Some(s) = view.project(c).filter(|&s| !map.occluded(s, c.length())) else {
            continue;
        };
        painter.circle_stroke(egui::pos2(s.x, s.y), 5.0, egui::Stroke::new(2.0, colour));
        if k == 0 {
            painter.text(
                egui::pos2(s.x + 8.0, s.y + 8.0),
                egui::Align2::LEFT_TOP,
                format!(
                    "CA {}\nin {}",
                    format::distance(pass_distance(&sim, rv.target, ca.distance)),
                    format::duration(ca.t.seconds_since(sim.clock))
                ),
                egui::FontId::monospace(12.0),
                colour,
            );
        }
    }
    Ok(())
}

/// The distance shown for an approach: to a vessel, centre to centre; over
/// a body, the height above its equatorial radius.
fn pass_distance(sim: &SimState, target: Option<NavTarget>, distance: f64) -> f64 {
    match target {
        Some(NavTarget::Body(node)) => {
            let radius = sim.world.source(node).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
            distance - radius
        }
        _ => distance,
    }
}

/// One line for the HUD: the nearest approach to the target.
pub fn summary(rv: &Rendezvous, sim: &SimState) -> Option<String> {
    let ca = rv.approaches.first()?;
    let over = if matches!(rv.target, Some(NavTarget::Body(_))) { "pass" } else { "closest" };
    Some(format!(
        "{over} {} in {} at {}",
        format::distance(pass_distance(sim, rv.target, ca.distance)),
        format::duration(ca.t.seconds_since(sim.clock)),
        format::speed(ca.speed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_test_ships_have_an_approach_once_targeted() {
        let mut sim = SimState::new();
        sim.spawn_test_ships(2);
        sim.active = 1;
        // Compute some of both coasts ahead.
        let until = sim.clock.add_seconds(20_000.0);
        let world = sim.world.clone();
        for v in &mut sim.fleet {
            v.extend_coast(&world, until, 100_000);
        }
        let target = sim.fleet[2].id();
        let mut app = App::new();
        app.insert_resource(sim);
        let mut nav = Navball::default();
        nav.target = Some(NavTarget::Vessel(target));
        app.insert_resource(nav);
        app.init_resource::<Rendezvous>();
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, update);
        app.update();
        let rv = app.world().resource::<Rendezvous>();
        let sim = app.world().resource::<SimState>();
        let ca = rv.approaches.first().expect("an approach");
        // The test ships' orbits differ in altitude, inclination and node:
        // the nearest pass is somewhere between touching and opposite sides.
        assert!(ca.distance > 1.0 && ca.distance < 2.0 * 6.9e6, "{}", ca.distance);
        assert!(rv.approaches.windows(2).all(|w| w[0].distance <= w[1].distance), "nearest first");
        assert!(summary(rv, sim).unwrap().starts_with("closest"));
    }

    #[test]
    fn a_body_target_gives_the_pass_above_its_surface() {
        let mut sim = SimState::new();
        sim.spawn_test_ships(1);
        sim.active = 1;
        let until = sim.clock.add_seconds(20_000.0);
        let world = sim.world.clone();
        sim.fleet[1].extend_coast(&world, until, 100_000);
        let moon = sim.world.find("Moon").expect("the Moon").node;
        let mut app = App::new();
        app.insert_resource(sim);
        let mut nav = Navball::default();
        nav.target = Some(NavTarget::Body(moon));
        app.insert_resource(nav);
        app.init_resource::<Rendezvous>();
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, update);
        app.update();
        let rv = app.world().resource::<Rendezvous>();
        let sim = app.world().resource::<SimState>();
        let ca = rv.approaches.first().expect("an approach to the Moon");
        // A ship near Earth passes the Moon at roughly its distance.
        assert!(ca.distance > 3.0e8 && ca.distance < 4.2e8, "{}", ca.distance);
        let radius = sim.world.source(moon).unwrap().physical.as_ref().unwrap().radius_eq;
        assert_eq!(pass_distance(sim, rv.target, ca.distance), ca.distance - radius);
        assert!(summary(rv, sim).unwrap().starts_with("pass"));
    }
}
