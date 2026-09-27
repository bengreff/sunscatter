//! Rendezvous tools (realism-1 §6c): the closest approaches between the
//! active vessel and the navball's target vessel, from both stored
//! trajectories (`sim::approach`), refreshed twice a second of real time.
//! Shown on the map (a marker on each line) and in the HUD.

use crate::camera::CameraRig;
use crate::format;
use crate::hud::UiState;
use crate::map::{self, MapView};
use crate::navball::{NavTarget, Navball};
use crate::state::SimState;
use crate::trajectory::Plotter;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sim::approach::{closest_approaches, Approach};
use sim::vessel::VesselId;

/// How often the approaches are recomputed (s of real time).
const REFRESH: f64 = 0.5;
/// Samples over the common computed span.
const SAMPLES: usize = 400;

#[derive(Resource, Default)]
pub struct Rendezvous {
    pub target: Option<VesselId>,
    /// Nearest first.
    pub approaches: Vec<Approach>,
    last: f64,
}

/// The target vessel of the navball, if any.
fn target(nav: &Navball) -> Option<VesselId> {
    match nav.target {
        Some(NavTarget::Vessel(id)) => Some(id),
        _ => None,
    }
}

pub fn update(time: Res<Time>, sim: Res<SimState>, nav: Res<Navball>, mut rv: ResMut<Rendezvous>) {
    let now = time.elapsed_secs_f64();
    let tgt = target(&nav);
    if tgt == rv.target && now - rv.last < REFRESH {
        return;
    }
    rv.last = now;
    rv.target = tgt;
    rv.approaches.clear();
    let Some(j) = tgt.and_then(|id| sim.index_of(id)) else { return };
    let (Some(a), Some(b)) = (sim.ship().trajectory(), sim.fleet[j].trajectory()) else { return };
    let t0 = sim.clock;
    let t1 = std::cmp::min_by(a.computed_until(), b.computed_until(), |x, y| x.seconds_since(*y).total_cmp(&0.0));
    if t1.seconds_since(t0) <= 0.0 {
        return;
    }
    rv.approaches = closest_approaches(&sim.world, |t| a.eval(t), |t| b.eval(t), t0, t1, SAMPLES);
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
    let Some(j) = rv.target.and_then(|id| sim.index_of(id)) else { return Ok(()) };
    let Some(view) = map::view(&cam) else { return Ok(()) };
    let Some(plotter) = Plotter::new(&sim, &rig, ui.plot_frame) else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let painter = ctx.layer_painter(egui::LayerId::background());
    let colour = egui::Color32::from_rgb(200, 120, 255);
    for (k, v) in [sim.ship(), &sim.fleet[j]].into_iter().enumerate() {
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
                format!("CA {}\nin {}", format::distance(ca.distance), format::duration(ca.t.seconds_since(sim.clock))),
                egui::FontId::monospace(12.0),
                colour,
            );
        }
    }
    Ok(())
}

/// One line for the HUD: the nearest approach to the target.
pub fn summary(rv: &Rendezvous, sim: &SimState) -> Option<String> {
    let ca = rv.approaches.first()?;
    Some(format!(
        "closest {} in {} at {}",
        format::distance(ca.distance),
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
}
