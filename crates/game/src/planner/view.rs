//! The planner in the 3D/map view: a click anywhere on the drawn line (any
//! future segment, after earlier burns too) adds a burn there, and each
//! draft burn has prograde, normal and radial handles (both signs) to drag
//! its Δv ([`super::drag_dv`]). Aboard, a released drag sets the plan at
//! once so the re-predicted line shows the result; from elsewhere the plan
//! is sent from the window (light delay, D067).

use super::{drag_dv, nearest_on_screen, plan_from, DraftBurn, Drag, Planner};
use crate::camera::{CameraRig, MainCamera};
use crate::commands::GameCommand;
use crate::comms::{Comms, Location};
use crate::hud::UiState;
use crate::map::View;
use crate::state::{Prediction, SimState};
use crate::trajectory::{apsides::vessel_segments, eval_at, lines::Lines, Plotter};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use glam::DVec3;
use sim::time::Epoch;

/// Handle distance from the burn's node on screen (px), and grab radius.
const HANDLE_PX: f32 = 56.0;
const GRAB_PX: f32 = 9.0;
/// Handle colours: prograde, normal, radial.
const COLOURS: [egui::Color32; 3] = [
    egui::Color32::from_rgb(230, 220, 60),
    egui::Color32::from_rgb(220, 80, 230),
    egui::Color32::from_rgb(70, 210, 230),
];
const NAMES: [&str; 3] = ["pro", "nrm", "rad"];

/// One handle on screen.
struct Handle {
    axis: usize,
    sign: f32,
    /// Outward screen direction (unit) and position.
    dir: Vec2,
    at: Vec2,
}

/// Screen position of burn `b`'s node and its handles.
fn handles(
    sim: &SimState,
    pred: &Prediction,
    i: usize,
    plotter: &Plotter,
    v: &View,
    b: &DraftBurn,
) -> Option<(Vec2, Vec<Handle>)> {
    let segs = vessel_segments(sim, pred, i);
    let (anchor, r, vel) = eval_at(&segs, b.t_start)?;
    let c = plotter.plot(anchor, r, b.t_start);
    let s = v.project(c)?;
    let rel = sim.world.eph.relative(b.reference, anchor, b.t_start);
    let (p, n, rad) = sim::vessel::prograde_normal_radial(r - rel.r, vel - rel.v);
    // Axes are directions: offset the plotted point a little along each.
    let step = 0.05 * c.length();
    let mut out = Vec::new();
    for (axis, a) in [p, n, rad].into_iter().enumerate() {
        let Some(e) = v.project(c + a * step) else { continue };
        let d = e - s;
        if d.length() < 1e-3 {
            continue;
        }
        let dir = d.normalize();
        for sign in [1.0, -1.0] {
            out.push(Handle { axis, sign, dir: dir * sign, at: s + dir * sign * HANDLE_PX });
        }
    }
    Some((s, out))
}

/// With the planner open: handles on each draft burn (egui widgets, so a
/// drag on one does not turn the camera), and a click (press and release
/// in place) on the drawn line to add a burn.
#[allow(clippy::too_many_arguments)]
pub fn pick_on_line(
    mut contexts: EguiContexts,
    mouse: Res<ButtonInput<MouseButton>>,
    sim: Res<SimState>,
    pred: Res<Prediction>,
    rig: Res<CameraRig>,
    ui: Res<UiState>,
    lines: Res<Lines>,
    comms: Res<Comms>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    window: Query<&Window>,
    mut planner: ResMut<Planner>,
    mut commands: MessageWriter<GameCommand>,
) -> Result {
    let Some(id) = planner.vessel.filter(|_| planner.open) else { return Ok(()) };
    let Some(i) = sim.index_of(id) else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let (Some(v), Some(plotter)) = (crate::map::view(&cam), Plotter::new(&sim, &rig, ui.plot_frame)) else {
        return Ok(());
    };
    let painter = ctx.layer_painter(egui::LayerId::background());
    let pos = |p: Vec2| egui::pos2(p.x, p.y);

    // Handles of the burns still ahead.
    let mut released = false;
    for k in 0..planner.draft.len() {
        let b = planner.draft[k];
        if b.t_start.seconds_since(sim.clock) <= 0.0 {
            continue;
        }
        let Some((s, hs)) = handles(&sim, &pred, i, &plotter, &v, &b) else { continue };
        painter.circle_stroke(pos(s), 6.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
        let font = egui::FontId::monospace(11.0);
        painter.text(
            pos(s + Vec2::new(8.0, 8.0)),
            egui::Align2::LEFT_TOP,
            format!("burn {}", k + 1),
            font,
            egui::Color32::WHITE,
        );
        for h in &hs {
            let colour = COLOURS[h.axis];
            painter.line_segment([pos(s), pos(h.at)], egui::Stroke::new(1.0, colour.gamma_multiply(0.6)));
            if h.sign > 0.0 {
                let label = format!("{} {:+.1}", NAMES[h.axis], b.dv[h.axis]);
                painter.text(
                    pos(h.at + h.dir * 16.0),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::monospace(10.0),
                    colour,
                );
            }
            let area = egui::Area::new(egui::Id::new(("burn_handle", k, h.axis, h.sign > 0.0)))
                .fixed_pos(pos(h.at - Vec2::splat(GRAB_PX)))
                .order(egui::Order::Background);
            let resp = area
                .show(ctx, |ui| {
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(2.0 * GRAB_PX, 2.0 * GRAB_PX), egui::Sense::drag());
                    let c = rect.center();
                    if h.sign > 0.0 {
                        ui.painter().circle_filled(c, 6.0, colour);
                    } else {
                        ui.painter().circle_stroke(c, 6.0, egui::Stroke::new(2.0, colour));
                    }
                    if resp.hovered() || resp.dragged() {
                        ui.painter().circle_stroke(c, 8.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
                    }
                    resp
                })
                .inner;
            let cursor = resp.interact_pointer_pos().map(|p| Vec2::new(p.x, p.y));
            if resp.drag_started() {
                // The axis's positive screen direction: dragging a negative
                // handle outward lowers the component.
                let (from, dir) = (cursor.unwrap_or(h.at), h.dir * h.sign);
                planner.drag = Some(Drag { burn: k, axis: h.axis, dir, from, dv0: b.dv[h.axis] });
            }
            if let (Some(d), Some(c)) = (planner.drag.filter(|d| d.burn == k && d.axis == h.axis), cursor) {
                if resp.dragged() {
                    planner.draft[k].dv[d.axis] = d.dv0 + drag_dv(f64::from((c - d.from).dot(d.dir)));
                }
            }
            if resp.drag_stopped() {
                planner.drag = None;
                released = true;
            }
        }
    }
    // Aboard, the plan is set when a drag ends (re-predicted at once).
    if released && comms.location == Location::Vessel(id) {
        let plan = plan_from(&sim.fleet[i], &planner.draft, sim.clock);
        commands.write(GameCommand::SetPlan { vessel: id, plan });
    }

    // A click on the drawn line adds a burn there, about the dominant body
    // at that point; a press that moves (turning the camera) does not.
    let cursor = window.single().ok().and_then(Window::cursor_position);
    if mouse.just_pressed(MouseButton::Left) {
        planner.press = cursor.filter(|_| !ctx.is_pointer_over_egui());
    }
    if !mouse.just_released(MouseButton::Left) {
        return Ok(());
    }
    let (Some(press), Some(cursor)) = (planner.press.take(), cursor) else { return Ok(()) };
    if press.distance(cursor) > 4.0 {
        return Ok(());
    }
    let Some(line) = lines.get(id) else { return Ok(()) };
    let mut best: Option<(Epoch, f32)> = None;
    for piece in &line.pieces {
        let run: Vec<(Epoch, Vec2)> =
            piece.points.iter().filter_map(|p| Some((p.t, v.project(p.p.as_dvec3())?))).collect();
        if let Some((t, d)) = nearest_on_screen(&run, cursor, 8.0) {
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((t, d));
            }
        }
    }
    let Some((t, _)) = best.filter(|(t, _)| t.seconds_since(sim.clock) > 1.0) else { return Ok(()) };
    let segs = vessel_segments(&sim, &pred, i);
    let reference = eval_at(&segs, t)
        .map_or(sim.dominant_of(i), |(anchor, r, _)| sim.dominance.of(&sim.world.eph, t, anchor, r, None, None));
    planner.draft.push(DraftBurn { t_start: t, dv: DVec3::ZERO, reference });
    planner.draft.sort_by(|a, b| a.t_start.seconds_since(b.t_start).total_cmp(&0.0));
    Ok(())
}
