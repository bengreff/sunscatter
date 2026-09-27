//! Powered-landing aids (realism-1 §6b): where and when the active vessel
//! meets the surface, and when to start braking.
//!
//! The impact is read from the stored trajectory: a coast ends at the
//! surface (`EndKind::Surface`, found on the detailed terrain, D059), so the
//! prediction is exactly what the vessel will do if nothing changes.

use crate::format;
use crate::interface::theme;
use crate::map;
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use glam::DVec3;
use sim::frame::{NodeId, Vec3};
use sim::time::Epoch;
use sim::vessel::{EndKind, Phase};

/// Below this height above the terrain the landing panel shows (m).
pub const PANEL_BELOW: f64 = 20_000.0;

/// Where the stored trajectory meets a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub body: NodeId,
    pub t: Epoch,
    /// Speed relative to the ground at impact (m/s).
    pub speed: f64,
    /// Position relative to the vessel's current anchor frame at impact:
    /// (anchor, r).
    pub anchor: NodeId,
    pub r: DVec3,
}

/// The impact of vessel `i`'s stored trajectory, if it ends on a surface.
pub fn impact(sim: &SimState, i: usize) -> Option<Impact> {
    let tr = sim.fleet[i].trajectory()?;
    let last = tr.last();
    let end = last.end?;
    let EndKind::Surface { body } = end.kind else { return None };
    let t_end = last.t0.add_seconds(end.t);
    let (anchor, r, v) = last.eval(end.t)?;
    let src = sim.world.source(body)?;
    let p = src.physical.as_ref()?;
    let k = sim.world.snapshot(t_end).relative(body, anchor);
    let rel = r - k.r;
    let v_srf = v - k.v - p.rotation.omega(t_end).raw().cross(rel);
    // The coast ends where live contact flight takes over, `live_height`
    // above the ground; the lowest point still has to fall the rest.
    let vessel = &sim.fleet[i];
    let lowest = vessel.mass_props().com.z - vessel.craft.bottom_z;
    let drop = (vessel.live_height() - lowest).max(0.0);
    let (up, g) = (rel.normalize(), src.gm / rel.length_squared());
    let (dt, v_down) = fall(-v_srf.dot(up), g, drop);
    let speed = (v_srf - up * v_srf.dot(up)).length().hypot(v_down);
    Some(Impact { body, t: t_end.add_seconds(dt), speed, anchor, r: r - up * drop })
}

/// Falling `drop` metres from a downward speed `v_down` under gravity `g`:
/// the time taken and the downward speed at the end.
pub fn fall(v_down: f64, g: f64, drop: f64) -> (f64, f64) {
    let v_end = (v_down * v_down + 2.0 * g * drop).sqrt();
    let dt = if g > 0.0 { (v_end - v_down) / g } else { drop / v_down.max(1e-6) };
    (dt, v_end)
}

/// Height (m) above the ground at which a vertical descent at `v_down`
/// (m/s, positive down) must start braking at full thrust to stop at the
/// ground, with thrust acceleration `a` and gravity `g` (m/s²): v²/(2(a−g)).
/// `None` if the craft cannot stop (a ≤ g).
pub fn braking_height(v_down: f64, a: f64, g: f64) -> Option<f64> {
    (a > g).then(|| v_down.max(0.0).powi(2) / (2.0 * (a - g)))
}

/// The landing panel and the impact marker.
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    cam: Query<(&Camera, &Transform, &Projection), With<crate::camera::MainCamera>>,
    rig: Res<crate::camera::CameraRig>,
    station: Res<crate::tracking::TrackingStation>,
) -> Result {
    if station.open {
        return Ok(());
    }
    let ship = sim.ship();
    if matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. }) {
        return Ok(());
    }
    let i = sim.active;
    let body = sim.dominant_of(i);
    let Some(src) = sim.world.source(body) else { return Ok(()) };
    let Some(p) = src.physical.as_ref() else { return Ok(()) };
    let (anchor, r, v) = ship.state_at(&sim.world, sim.clock);
    let k = sim.world.snapshot(sim.clock).relative(body, anchor);
    let rel = r - k.r;
    let fixed = p.rotation.to_fixed(Vec3::from_raw(rel), sim.clock);
    let height = p.altitude_above_surface(fixed);
    let hit = impact(&sim, i);
    let ctx = contexts.ctx_mut()?;
    // The impact point, in any view.
    if let (Some(hit), Some(view)) = (hit, map::view(&cam)) {
        let c = sim.world.snapshot(sim.clock).relative_r(hit.anchor, rig.anchor) + hit.r - rig.cam_pos;
        if let Some(s) = view.project(c) {
            let painter = ctx.layer_painter(egui::LayerId::background());
            let red = egui::Color32::from_rgb(255, 80, 60);
            painter.circle_stroke(egui::pos2(s.x, s.y), 7.0, egui::Stroke::new(2.0, red));
            painter.text(
                egui::pos2(s.x + 10.0, s.y),
                egui::Align2::LEFT_CENTER,
                format!("impact in {}", format::duration(hit.t.seconds_since(sim.clock))),
                egui::FontId::monospace(12.0),
                red,
            );
        }
    }
    if height > PANEL_BELOW {
        return Ok(());
    }
    let up = rel.normalize();
    let v_srf = v - k.v - p.rotation.omega(sim.clock).raw().cross(rel);
    let v_up = v_srf.dot(up);
    let v_horizontal = (v_srf - up * v_up).length();
    let g = src.gm / rel.length_squared();
    let a = ship.craft.engine.thrust_vac / ship.mass_props().mass;
    egui::Window::new("Landing").anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0]).resizable(false).show(ctx, |ui| {
        let dim = |t: &str| egui::RichText::new(t).monospace().small().color(theme::DIM);
        egui::Grid::new("landing_grid").num_columns(2).show(ui, |ui| {
            let mut row = |label: &str, value: String| {
                ui.label(dim(label));
                ui.monospace(value);
                ui.end_row();
            };
            row("RADAR ALT", format::distance(height));
            row("V/S", format!("{v_up:+.1} m/s"));
            row("H SPEED", format!("{v_horizontal:.1} m/s"));
            row("TWR", format!("{:.2}", a / g));
            match hit {
                Some(h) => {
                    row("IMPACT IN", format::duration(h.t.seconds_since(sim.clock)));
                    row("AT", format!("{:.1} m/s", h.speed));
                }
                None => row("IMPACT", "none".into()),
            }
            match braking_height(-v_up, a, g) {
                Some(b) => {
                    let colour = if height <= b * 1.2 { theme::WARN } else { theme::TEXT };
                    ui.label(dim("BRAKE AT"));
                    ui.label(egui::RichText::new(format::distance(b)).monospace().color(colour));
                    ui.end_row();
                }
                None => row("BRAKE", "cannot stop".into()),
            }
        });
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braking_height_is_the_stopping_distance() {
        // 100 m/s down on the Moon (1.62 m/s²) at 3 m/s² net... a = 4.62.
        let h = braking_height(100.0, 4.62, 1.62).unwrap();
        assert!((h - 100.0 * 100.0 / 6.0).abs() < 1e-9);
        assert_eq!(braking_height(-5.0, 4.0, 1.0), Some(0.0), "going up: no braking needed");
        assert_eq!(braking_height(10.0, 1.0, 1.62), None, "cannot stop");
    }

    #[test]
    fn falling_the_last_metres() {
        let (dt, v) = fall(10.0, 1.62, 15.0);
        assert!((v - (100.0f64 + 2.0 * 1.62 * 15.0).sqrt()).abs() < 1e-12);
        assert!((10.0 * dt + 0.5 * 1.62 * dt * dt - 15.0).abs() < 1e-9);
    }

    #[test]
    fn a_falling_ship_predicts_its_impact() {
        let mut sim = SimState::new();
        // A ship 50 km above Earth falling straight down.
        let earth = sim.world.find("Earth").unwrap().clone();
        let re = earth.physical.as_ref().unwrap().radius_eq;
        let id = sim.vessel_ids.allocate();
        let r = DVec3::X * (re + 50_000.0);
        let v = sim::vessel::Vessel::coasting(
            &sim.world,
            id,
            sim.clock,
            earth.node,
            r,
            DVec3::ZERO,
            sim::craft::test_craft(),
        );
        sim.fleet.push(v);
        let i = sim.fleet.len() - 1;
        let world = sim.world.clone();
        let until = sim.clock.add_seconds(600.0);
        sim.fleet[i].extend_coast(&world, until, 100_000);
        let hit = impact(&sim, i).expect("it hits the ground");
        // Free fall from 50 km with drag lower down: between one and three
        // minutes, fast.
        let dt = hit.t.seconds_since(sim.clock);
        assert!(dt > 60.0 && dt < 200.0, "{dt}");
        assert!(hit.speed > 100.0, "{}", hit.speed);
    }
}
