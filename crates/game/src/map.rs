//! Map view, drawn: icons, body orbit lines, Ap/Pe markers and hover. Which
//! objects get them is decided per object by the rule in [`map_view`]
//! (D054); this module gathers each object's sizes and draws the result.

use crate::camera::{self, CameraRig, MainCamera};
use crate::hud::{fmt_dist, PlotFrame, UiState};
use crate::map_view::{self, Object, ObjectId, Visibility};
use crate::scene::BodyDefs;
use crate::state::{Prediction, SimState};
use crate::tracking::{Tracked, TrackingStation};
use crate::trajectory::{self, Plotter};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts};
use glam::DVec3;
use sim::kepler::Elements;

/// A vessel's bounding radius for the map-view rule (m).
pub const VESSEL_RADIUS: f64 = 10.0;
/// Orbit lines fade in from the map-view threshold up to this size (px).
const ORBIT_FADE_PX: f64 = 12.0;
const ORBIT_POINTS: usize = 256;

/// This frame's map-view classification of every body and vessel.
#[derive(Resource, Default)]
pub struct MapView {
    objects: Vec<Object>,
    vis: Vec<Visibility>,
    /// Pixels per metre at the focus distance.
    k: f64,
}

impl MapView {
    pub fn get(&self, id: ObjectId) -> Visibility {
        self.objects.iter().position(|o| o.id == id).map_or_else(Visibility::default, |i| self.vis[i])
    }

    pub fn in_map(&self, id: ObjectId) -> bool {
        self.get(id).in_map
    }
}

/// View parameters for projecting camera-relative points to the screen.
pub struct View {
    cam: Camera,
    gt: GlobalTransform,
    /// Pixels per radian at the centre of the view.
    focal: f64,
    /// Viewport height (logical px) and vertical field of view (rad).
    height: f64,
    fov: f64,
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
    let height = f64::from(size.y);
    Some(View { cam: c.clone(), gt: GlobalTransform::from(*t), focal: height * 0.5 / (fov * 0.5).tan(), height, fov })
}

/// Gathers every object's sizes and classifies it (after the camera moves).
pub fn update(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    tracked: Res<Tracked>,
    station: Res<TrackingStation>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    mut map: ResMut<MapView>,
) {
    let Some(v) = view(&cam) else { return };
    let (world, eph) = (&sim.world, &sim.world.eph);
    let snap = world.snapshot(sim.clock);
    let object = |id, c: DVec3, radius: f64, orbit_radius, mass| Object {
        id,
        radius,
        orbit_radius,
        mass,
        screen: v.project(c),
        depth: c.length(),
        disc_px: v.radius_px(c, radius),
        pinned: false,
    };
    let mut objects = Vec::new();
    for node in eph.bodies() {
        let c = snap.relative_r(node, rig.anchor) - rig.cam_pos;
        let radius = world.source(node).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
        let orbit = crate::relations::body_orbit(eph, sim.clock, node).map(|(_, o)| o.radius());
        objects.push(object(ObjectId::Body(node), c, radius, orbit, eph.node(node).gm));
    }
    for (i, vessel) in sim.fleet.iter().enumerate() {
        let (anchor, r, _) = vessel.state_at(world, sim.clock);
        let c = snap.relative_r(anchor, rig.anchor) + r - rig.cam_pos;
        let orbit = crate::relations::vessel_orbit(world, sim.clock, vessel).map(|o| o.radius());
        let mut o = object(ObjectId::Vessel(i), c, VESSEL_RADIUS, orbit, 0.0);
        // The tracking station never hides a tracked vessel.
        o.pinned = station.open && (i == sim.active || tracked.is_tracked(i));
        objects.push(o);
    }
    let k = map_view::scale(v.height, v.fov, rig.distance);
    let vis = map_view::classify(&objects, k, |id| map.in_map(id));
    *map = MapView { objects, vis, k };
}

/// Orbit lines of the bodies in map view, about their primaries, over one
/// period ahead (from the ephemeris where it covers the period, else the
/// osculating conic).
pub fn draw_body_orbits(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    ui: Res<UiState>,
    map: Res<MapView>,
    defs: Res<BodyDefs>,
    mut gizmos: Gizmos,
) {
    let eph = &sim.world.eph;
    let snap = sim.world.snapshot(sim.clock);
    for node in eph.bodies() {
        if !map.in_map(ObjectId::Body(node)) {
            continue;
        }
        let Some((p, orbit)) = crate::relations::body_orbit(eph, sim.clock, node) else { continue };
        // The rotating frame is Earth–Moon: skip orbits about Earth there.
        if ui.plot_frame == PlotFrame::EarthMoonRotating && eph.node(p).name == "Earth" {
            continue;
        }
        let Some(period) = orbit.period() else { continue };
        let (el, mu) = (orbit.elements, orbit.mu);
        let fade = (map.k * el.a - map_view::ORBIT_ENTER_PX) / (ORBIT_FADE_PX - map_view::ORBIT_ENTER_PX);
        let alpha = fade.clamp(0.15, 1.0) as f32;
        let [r, g, b] = defs.get(node).map_or([0.7, 0.7, 0.7], |d| d.icon_color);
        let color = Color::srgba(r, g, b, 0.55 * alpha);
        let centre = snap.relative_r(p, rig.anchor) - rig.cam_pos;
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

fn egui_color(c: [f32; 3], a: f32) -> egui::Color32 {
    let to = |x: f32| (x.clamp(0.0, 1.0) * 255.0) as u8;
    egui::Color32::from_rgba_unmultiplied(to(c[0]), to(c[1]), to(c[2]), to(a))
}

/// Icons, apsis markers and the hover ring, painted behind the windows.
#[allow(clippy::too_many_arguments)]
pub fn draw_overlay(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    pred: Res<Prediction>,
    ui: Res<UiState>,
    map: Res<MapView>,
    defs: Res<BodyDefs>,
    cam: Query<(&Camera, &Transform, &Projection), With<MainCamera>>,
    window: Query<&Window, With<PrimaryWindow>>,
    tracked: Res<Tracked>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let Some(v) = view(&cam) else { return Ok(()) };
    let painter = ctx.layer_painter(egui::LayerId::background());
    let pt = |p: Vec2| egui::pos2(p.x, p.y);

    for (o, vis) in map.objects.iter().zip(&map.vis) {
        let Some(s) = o.screen.filter(|_| vis.icon) else { continue };
        match o.id {
            ObjectId::Body(node) => {
                let color = defs.get(node).map_or([0.7, 0.7, 0.7], |d| d.icon_color);
                painter.circle_filled(pt(s), 4.0, egui_color(color, 1.0));
            }
            ObjectId::Vessel(i) => {
                let (size, color) = if i == sim.active { (6.0, [1.0, 0.85, 0.2]) } else { (4.0, [0.7, 0.75, 0.8]) };
                let d = |x: f32, y: f32| egui::pos2(s.x + x * size, s.y + y * size);
                let diamond = vec![d(0.0, -1.0), d(1.0, 0.0), d(0.0, 1.0), d(-1.0, 0.0)];
                painter.add(egui::Shape::convex_polygon(diamond, egui_color(color, 1.0), egui::Stroke::NONE));
            }
        }
    }
    if map.in_map(ObjectId::Vessel(sim.active)) {
        draw_apsides(&painter, &sim, &rig, &pred, &ui, &v, &map);
    }

    let cursor = window.single().ok().and_then(Window::cursor_position);
    let hovered =
        cursor.filter(|_| !ctx.is_pointer_over_egui()).and_then(|c| map_view::hover(&map.objects, &map.vis, c));
    if let Some(o) = hovered.map(|i| &map.objects[i]) {
        let Some(s) = o.screen else { return Ok(()) };
        let name = match o.id {
            ObjectId::Body(node) => sim.world.eph.node(node).name.clone(),
            ObjectId::Vessel(i) if i == sim.active => format!("{} (active)", tracked.name(i)),
            ObjectId::Vessel(i) => tracked.name(i),
        };
        let ring = egui::Stroke::new(1.5, egui::Color32::from_rgb(120, 220, 255));
        painter.circle_stroke(pt(s), 10.0, ring);
        painter.text(
            egui::pos2(s.x + 14.0, s.y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }
    Ok(())
}

fn draw_apsides(
    painter: &egui::Painter,
    sim: &SimState,
    rig: &CameraRig,
    pred: &Prediction,
    ui: &UiState,
    v: &View,
    map: &MapView,
) {
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
        let c = plotter.plot(anchor, r, seg.t0.add_seconds(a.t));
        let Some(s) = v.project(c).filter(|&s| !map_view::occluded(&map.objects, s, c.length())) else { continue };
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
