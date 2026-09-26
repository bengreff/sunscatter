//! Map mode: orbit lines, icons, Ap/Pe markers and hover. Currently one
//! global switch (on when the active ship is under ~1 px); being replaced by
//! the per-object rule of D054 (docs/features/map-view-lighting-controls.md).

use crate::camera::{self, CameraRig, MainCamera};
use crate::hud::{fmt_dist, PlotFrame, UiState};
use crate::scene::BodyDefs;
use crate::state::{Prediction, SimState};
use crate::trajectory::{self, Plotter};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use glam::DVec3;
use sim::frame::NodeId;
use sim::kepler::Elements;

/// Nominal ship size for the pixel test (m).
const SHIP_SIZE: f64 = 10.0;
/// Enter map mode below this projected ship size (px); leave above `EXIT_PX`.
const ENTER_PX: f64 = 1.0;
const EXIT_PX: f64 = 2.0;
/// Orbit lines smaller than this on screen (px) are not drawn; they fade in
/// up to `ORBIT_FULL_PX`.
const ORBIT_MIN_PX: f64 = 12.0;
const ORBIT_FULL_PX: f64 = 60.0;
const ORBIT_POINTS: usize = 256;

#[derive(Resource, Default)]
pub struct MapMode {
    pub active: bool,
    /// Forced on (the tracking station).
    pub forced: bool,
}

/// View parameters for projecting camera-relative points to the screen.
pub struct View {
    cam: Camera,
    gt: GlobalTransform,
    /// Pixels per radian at the centre of the view.
    focal: f64,
}

impl View {
    pub fn project(&self, p: DVec3) -> Option<Vec2> {
        self.cam.world_to_viewport(&self.gt, p.as_vec3()).ok()
    }

    pub fn focal(&self) -> f64 {
        self.focal
    }

    /// Projected radius (px) of a sphere of `radius` at camera-relative `p`.
    pub fn radius_px(&self, p: DVec3, radius: f64) -> f64 {
        radius / p.length().max(1e-3) * self.focal
    }
}

/// The current view, from the camera's own transform (not the propagated
/// one, which may lag a frame).
pub fn view(cam: &Query<(&Camera, &Transform, &Projection), With<MainCamera>>) -> Option<View> {
    let (c, t, proj) = cam.single().ok()?;
    let size = c.logical_viewport_size()?;
    let fov = match proj {
        Projection::Perspective(p) => f64::from(p.fov),
        _ => 0.8,
    };
    Some(View { cam: c.clone(), gt: GlobalTransform::from(*t), focal: f64::from(size.y) * 0.5 / (fov * 0.5).tan() })
}

pub fn update_mode(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    mut map: ResMut<MapMode>,
) {
    let Some(v) = view(&cam) else { return };
    let (anchor, r, _) = sim.ship().state(&sim.world);
    let ship = sim.world.snapshot(sim.clock).relative_r(anchor, rig.anchor) + r - rig.cam_pos;
    let px = v.radius_px(ship, SHIP_SIZE);
    map.active = map.forced || if map.active { px < EXIT_PX } else { px < ENTER_PX };
}

/// The smallest anchor zone containing the camera (the body whose system
/// the player is looking at), if any.
fn context_body(sim: &SimState, rig: &CameraRig) -> Option<NodeId> {
    let snap = sim.world.snapshot(sim.clock);
    sim.world.anchor_order.iter().map(|&i| &sim.world.sources[i]).find_map(|s| {
        let zone = s.anchor_zone?;
        let d = (rig.cam_pos - snap.relative_r(s.node, rig.anchor)).length();
        (d < zone.enter).then_some(s.node)
    })
}

/// Orbit lines of every body about its primary, over one period ahead
/// (from the ephemeris where it covers the period, else the osculating conic).
pub fn draw_body_orbits(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    ui: Res<UiState>,
    map: Res<MapMode>,
    defs: Res<BodyDefs>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    mut gizmos: Gizmos,
) {
    let Some(v) = view(&cam) else { return };
    if !map.active {
        return;
    }
    let eph = &sim.world.eph;
    let snap = sim.world.snapshot(sim.clock);
    let context = context_body(&sim, &rig);
    for node in eph.bodies() {
        let Some(p) = crate::relations::primary(eph, node) else { continue };
        // Like KSP: inside a body's zone, show its moons' orbits (and, near
        // a moon, the moon's own orbit); outside all zones, the planets'.
        let has_zone = |n: NodeId| sim.world.source(n).is_some_and(|s| s.anchor_zone.is_some());
        let relevant = match context {
            Some(c) => p == c || (node == c && has_zone(p)),
            None => !has_zone(p),
        };
        if !relevant {
            continue;
        }
        // The rotating frame is Earth–Moon: skip orbits about Earth there.
        if ui.plot_frame == PlotFrame::EarthMoonRotating && eph.node(p).name == "Earth" {
            continue;
        }
        let k = snap.relative(node, p);
        let mu = eph.node(node).gm + eph.node(p).gm;
        let el = Elements::from_state(k.r, k.v, mu);
        if el.e >= 1.0 {
            continue;
        }
        let centre = snap.relative_r(p, rig.anchor) - rig.cam_pos;
        let size_px = v.radius_px(centre, el.a);
        if size_px < ORBIT_MIN_PX {
            continue;
        }
        let alpha = ((size_px - ORBIT_MIN_PX) / (ORBIT_FULL_PX - ORBIT_MIN_PX)).clamp(0.0, 1.0) as f32;
        let [r, g, b] = defs.get(node).map_or([0.7, 0.7, 0.7], |d| d.icon_color);
        let color = Color::srgba(r, g, b, 0.55 * alpha);
        let period = el.period(mu);
        let use_eph = sim.clock.add_seconds(period) < eph.end;
        let points = (0..=ORBIT_POINTS).map(|i| {
            let f = i as f64 / ORBIT_POINTS as f64;
            let rel = if use_eph {
                eph.relative(node, p, sim.clock.add_seconds(f * period)).r
            } else {
                Elements { mean_anomaly: el.mean_anomaly + f * std::f64::consts::TAU, ..el }.to_state(mu).0
            };
            (centre + rel).as_vec3()
        });
        gizmos.linestrip(points, color);
    }
}

/// Something that can be hovered: a name, a screen position and radius.
struct Target {
    name: String,
    pos: Vec2,
    radius: f32,
}

fn egui_color(c: [f32; 3], a: f32) -> egui::Color32 {
    let to = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u8;
    egui::Color32::from_rgba_unmultiplied(to(c[0]), to(c[1]), to(c[2]), to(a))
}

/// Icons, apsis markers and hover highlights, painted behind the windows.
#[allow(clippy::too_many_arguments)]
pub fn draw_overlay(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    pred: Res<Prediction>,
    ui: Res<UiState>,
    map: Res<MapMode>,
    defs: Res<BodyDefs>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    window: Query<&Window, With<PrimaryWindow>>,
    tracked: Res<crate::tracking::Tracked>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let Some(v) = view(&cam) else { return Ok(()) };
    let painter = ctx.layer_painter(egui::LayerId::background());
    let snap = sim.world.snapshot(sim.clock);
    let mut targets = Vec::new();
    let pt = |p: Vec2| egui::pos2(p.x, p.y);

    for node in sim.world.eph.bodies() {
        let c = snap.relative_r(node, rig.anchor) - rig.cam_pos;
        let Some(s) = v.project(c) else { continue };
        let radius = sim.world.source(node).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
        let rpx = v.radius_px(c, radius) as f32;
        let color = defs.get(node).map_or([0.7, 0.7, 0.7], |d| d.icon_color);
        if map.active && rpx < 1.0 {
            painter.circle_filled(pt(s), 4.0, egui_color(color, 1.0));
        }
        targets.push(Target { name: sim.world.eph.node(node).name.clone(), pos: s, radius: rpx.max(6.0) });
    }
    for (i, vessel) in sim.fleet.iter().enumerate() {
        let (anchor, r, _) = vessel.state(&sim.world);
        let c = snap.relative_r(anchor, rig.anchor) + r - rig.cam_pos;
        let Some(s) = v.project(c) else { continue };
        let active = i == sim.active;
        if map.active {
            let (size, color) = if active { (6.0, [1.0, 0.85, 0.2]) } else { (4.0, [0.7, 0.75, 0.8]) };
            let d = |x: f32, y: f32| egui::pos2(s.x + x * size, s.y + y * size);
            let diamond = vec![d(0.0, -1.0), d(1.0, 0.0), d(0.0, 1.0), d(-1.0, 0.0)];
            painter.add(egui::Shape::convex_polygon(diamond, egui_color(color, 1.0), egui::Stroke::NONE));
        }
        let name = if active { format!("{} (active)", tracked.name(i)) } else { tracked.name(i) };
        let rpx = v.radius_px(c, SHIP_SIZE) as f32;
        targets.push(Target { name, pos: s, radius: rpx.max(6.0) });
    }
    if map.active {
        draw_apsides(&painter, &sim, &rig, &pred, &ui, &v);
    }

    // Hover: the nearest target under the cursor (smallest first on ties).
    let cursor = window.single().ok().and_then(Window::cursor_position);
    if let Some(cursor) = cursor.filter(|_| !ctx.is_pointer_over_egui()) {
        let hit = targets
            .iter()
            .filter(|t| t.pos.distance(cursor) <= t.radius + 6.0)
            .min_by(|a, b| a.radius.total_cmp(&b.radius));
        if let Some(t) = hit {
            let ring = egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 220, 255));
            painter.circle_stroke(pt(t.pos), t.radius + 4.0, ring);
            painter.text(
                egui::pos2(t.pos.x + t.radius + 8.0, t.pos.y),
                egui::Align2::LEFT_CENTER,
                &t.name,
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
        }
    }
    Ok(())
}

fn draw_apsides(painter: &egui::Painter, sim: &SimState, rig: &CameraRig, pred: &Prediction, ui: &UiState, v: &View) {
    let Some(seg) = trajectory::active_segment(sim, pred) else { return };
    let Some((t0, t1)) = trajectory::future_span(seg, sim.clock) else { return };
    let Some(primary) = camera::nearest_body(sim) else { return };
    let Some(plotter) = Plotter::new(sim, rig, ui.plot_frame) else { return };
    let radius = sim.world.source(primary).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
    let now = sim.clock.seconds_since(seg.t0);
    let mut seen = (false, false);
    for a in trajectory::apsides(&sim.world, seg, primary, t0, t1, trajectory::TRAJECTORY_POINTS) {
        // Label only the next apoapsis and periapsis.
        let first = if a.is_apo { &mut seen.0 } else { &mut seen.1 };
        if std::mem::replace(first, true) {
            continue;
        }
        let Some((anchor, r, _)) = seg.eval(a.t) else { continue };
        let Some(s) = v.project(plotter.plot(anchor, r, seg.t0.add_seconds(a.t))) else { continue };
        let color =
            if a.is_apo { egui::Color32::from_rgb(120, 200, 255) } else { egui::Color32::from_rgb(255, 170, 90) };
        painter.circle_filled(egui::pos2(s.x, s.y), 4.0, color);
        let label = format!(
            "{} {}\nin {}",
            if a.is_apo { "Ap" } else { "Pe" },
            fmt_dist(a.distance - radius),
            fmt_duration(a.t - now)
        );
        painter.text(
            egui::pos2(s.x + 8.0, s.y - 8.0),
            egui::Align2::LEFT_BOTTOM,
            label,
            egui::FontId::monospace(12.0),
            color,
        );
    }
}

pub fn fmt_duration(s: f64) -> String {
    let s = s.max(0.0);
    if s >= 86_400.0 {
        format!("{:.0}d {:.0}h", (s / 86_400.0).floor(), (s % 86_400.0) / 3600.0)
    } else if s >= 3600.0 {
        format!("{:.0}h {:02.0}m", (s / 3600.0).floor(), ((s % 3600.0) / 60.0).floor())
    } else {
        format!("{:.0}m {:02.0}s", (s / 60.0).floor(), (s % 60.0).floor())
    }
}
