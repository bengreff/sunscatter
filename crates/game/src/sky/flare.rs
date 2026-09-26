//! Sun glare and lens flare, painted additively over the 3D view (behind
//! the UI): a soft glow, a horizontal streak and ghosts along the line
//! through the screen centre. Fades with the fraction of the Sun's disc that
//! bodies hide.

use crate::camera::{CameraRig, MainCamera};
use crate::map;
use crate::scene::BodyDefs;
use crate::settings::GraphicsSettings;
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

/// Additive colour (premultiplied with zero alpha).
fn add(rgb: [f32; 3], k: f32) -> egui::Color32 {
    let to = |x: f32| (x * k * 255.0).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_premultiplied(to(rgb[0]), to(rgb[1]), to(rgb[2]), 0)
}

/// A radial gradient disc as a triangle fan.
fn glow(painter: &egui::Painter, c: egui::Pos2, radius: f32, rgb: [f32; 3], k: f32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, add(rgb, k));
    let n = 48;
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(c + egui::vec2(a.cos(), a.sin()) * radius, add(rgb, 0.0));
    }
    for i in 1..=n as u32 {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    settings: Res<GraphicsSettings>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
) -> Result {
    if !settings.flare {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let Some(view) = map::view(&cam) else { return Ok(()) };
    let Some((sun, _)) = defs.light_source else { return Ok(()) };
    let Some(radius) = sim.world.source(sun).and_then(|s| s.physical.as_ref()).map(|p| p.radius_eq) else {
        return Ok(());
    };
    let snap = sim.world.snapshot(sim.clock);
    let sun_pos = snap.relative_r(sun, rig.anchor) - rig.cam_pos;
    let Some(s) = view.project(sun_pos) else { return Ok(()) };
    let vis = super::sun_visibility(&sim, &rig, sun, radius) as f32;
    if vis <= 0.0 {
        return Ok(());
    }
    let screen = ctx.content_rect();
    let centre = screen.center();
    let p = egui::pos2(s.x, s.y);
    // Fade out as the Sun leaves the screen.
    let off = ((p - centre).length() / screen.width().max(1.0)).min(2.0);
    let k = vis * (1.0 - (off - 0.6).max(0.0) / 0.6).clamp(0.0, 1.0);
    if k <= 0.0 {
        return Ok(());
    }
    let painter = ctx.layer_painter(egui::LayerId::background());
    let warm = [1.0, 0.92, 0.8];
    glow(&painter, p, 260.0, warm, 0.22 * k);
    glow(&painter, p, 70.0, [1.0, 0.97, 0.9], 0.55 * k);
    // Horizontal streak.
    let mut streak = egui::Mesh::default();
    let (w, h) = (screen.width() * 0.45, 2.5);
    for (dx, dy, a) in [(-w, 0.0, 0.0), (0.0, -h, 0.35), (w, 0.0, 0.0), (0.0, h, 0.35)] {
        streak.colored_vertex(p + egui::vec2(dx, dy), add([0.8, 0.88, 1.0], a * k));
    }
    streak.colored_vertex(p, add([0.8, 0.88, 1.0], 0.35 * k));
    for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0)] {
        streak.add_triangle(4, a, b);
    }
    painter.add(egui::Shape::mesh(streak));
    // Ghosts along the axis through the screen centre.
    let axis = centre - p;
    for (t, r, rgb, a) in [
        (0.35, 18.0, [0.6, 0.8, 1.0], 0.10),
        (0.7, 40.0, [0.5, 1.0, 0.6], 0.06),
        (1.15, 26.0, [1.0, 0.6, 0.4], 0.08),
        (1.5, 70.0, [0.6, 0.6, 1.0], 0.05),
        (1.9, 12.0, [1.0, 0.9, 0.5], 0.12),
    ] {
        glow(&painter, p + axis * t, r, rgb, a * k);
    }
    Ok(())
}
