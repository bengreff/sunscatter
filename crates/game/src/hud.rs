//! HUD (egui) and body picking: flight readouts, controls help, and the
//! double-click body menu. Deliberately minimal; many GUIs come later.

use crate::camera::{self, CameraRig, MainCamera};
use crate::state::{SimState, MAX_PHYSICS_WARP, WARP_LEVELS};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts};
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::vessel::Phase;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum PlotFrame {
    #[default]
    EarthInertial,
    EarthMoonRotating,
}

#[derive(Resource, Default)]
pub struct UiState {
    pub plot_frame: PlotFrame,
    /// Body whose menu is open, and where (logical pixels).
    pub menu: Option<(NodeId, Vec2)>,
    last_click: Option<(f64, Vec2)>,
}

/// Double-click on a body opens its menu. Tab toggles the plotting frame.
#[allow(clippy::too_many_arguments)]
pub fn pick_bodies(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    egui: Res<EguiWantsInput>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    mut ui: ResMut<UiState>,
) {
    if keys.just_pressed(KeyCode::Tab) {
        ui.plot_frame = match ui.plot_frame {
            PlotFrame::EarthInertial => PlotFrame::EarthMoonRotating,
            PlotFrame::EarthMoonRotating => PlotFrame::EarthInertial,
        };
    }
    if !buttons.just_pressed(MouseButton::Left) || egui.wants_any_pointer_input() {
        return;
    }
    let (Ok(window), Ok((cam, cam_tf))) = (window.single(), camera.single()) else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let now = time.elapsed_secs_f64();
    let double = ui.last_click.is_some_and(|(t, p)| now - t < 0.4 && p.distance(cursor) < 8.0);
    ui.last_click = Some((now, cursor));
    if !double {
        return;
    }
    let Ok(ray) = cam.viewport_to_world(cam_tf, cursor) else { return };
    let dir = ray.direction.as_vec3().as_dvec3();
    let snap = sim.world.snapshot(sim.clock);
    let hit = sim
        .world
        .sources
        .iter()
        .filter_map(|s| {
            let radius = s.physical.as_ref()?.radius_eq;
            let c = snap.relative(s.node, rig.anchor).r - rig.cam_pos;
            let dist = c.length();
            // Angular test with a minimum pick size so small discs are clickable.
            let angle = dir.dot(c / dist).clamp(-1.0, 1.0).acos();
            let size = (radius / dist).clamp(0.0, 1.0).asin().max(0.015);
            (angle <= size).then_some((s.node, dist))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((node, _)) = hit {
        ui.menu = Some((node, cursor));
    }
}

pub fn fmt_dist(m: f64) -> String {
    if !m.is_finite() {
        "∞".into()
    } else if m.abs() >= 1.0e6 {
        format!("{:.0} km", m / 1e3)
    } else if m.abs() >= 1.0e4 {
        format!("{:.1} km", m / 1e3)
    } else {
        format!("{m:.0} m")
    }
}

pub fn draw(
    mut contexts: EguiContexts,
    time: Res<Time>,
    sim: Res<SimState>,
    mut rig: ResMut<CameraRig>,
    mut ui: ResMut<UiState>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let ship = sim.ship();
    let (anchor, r, v) = ship.state(&sim.world);
    let snap = sim.world.snapshot(sim.clock);
    let near = camera::nearest_body(&sim);
    let (y, mo, d, h, mi, s) = sim.clock.to_calendar();

    egui::Window::new("Flight").anchor(egui::Align2::LEFT_TOP, [10.0, 10.0]).resizable(false).show(ctx, |ui_| {
        ui_.monospace(format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02}:{:04.1} TDB", s));
        let level = sim.effective_warp();
        let warp = WARP_LEVELS[level];
        let kind = if level > MAX_PHYSICS_WARP { "rails" } else { "physics" };
        let limited = if sim.compute_limited { "  (compute-limited)" } else { "" };
        ui_.monospace(format!("warp {warp}x ({kind}){limited}"));
        ui_.separator();
        let phase = match &ship.phase {
            Phase::Landed { .. } => "LANDED".to_string(),
            Phase::Powered { .. } => "POWERED".to_string(),
            Phase::Coasting { .. } => "COASTING".to_string(),
            Phase::Crashed { speed, .. } => format!("CRASHED at {speed:.0} m/s (R to reset)"),
        };
        ui_.monospace(phase);
        if let Some(body) = near.and_then(|b| sim.world.source(b)) {
            let k = snap.relative(body.node, anchor);
            let rel = r - k.r;
            let p = body.physical.as_ref().expect("surface body");
            let fixed = p.rotation.to_fixed(sim::frame::Vec3::from_raw(rel), sim.clock);
            let alt = p.altitude(fixed);
            let above_ground = p.altitude_above_surface(fixed);
            let v_orb = v - k.v;
            let v_srf = v_orb - p.rotation.omega(sim.clock).raw().cross(rel);
            let el = Elements::from_state(rel, v_orb, body.gm);
            ui_.monospace(format!("{:<6} alt {}  (terrain {})", body.name, fmt_dist(alt), fmt_dist(above_ground)));
            ui_.monospace(format!("surface speed {:.1} m/s", v_srf.length()));
            ui_.monospace(format!("orbital speed {:.1} m/s", v_orb.length()));
            ui_.monospace(format!(
                "Pe {}  Ap {}",
                fmt_dist(el.periapsis() - p.radius_eq),
                fmt_dist(el.apoapsis() - p.radius_eq)
            ));
            ui_.monospace(format!("vertical speed {:.1} m/s", v_srf.dot(rel.normalize())));
        }
        ui_.separator();
        let c = sim.controls;
        ui_.add(egui::ProgressBar::new(c.throttle as f32).text(format!("throttle {:.0}%", c.throttle * 100.0)));
        ui_.monospace(format!(
            "SAS {}   chute {}",
            if c.sas { "on" } else { "off" },
            if ship.chute_deployed { "DEPLOYED" } else { "stowed" }
        ));
        let anchor_name = &sim.world.eph.node(anchor).name;
        let frame = match ui.plot_frame {
            PlotFrame::EarthInertial => "Earth inertial",
            PlotFrame::EarthMoonRotating => "Earth–Moon rotating",
        };
        ui_.monospace(format!("anchor {anchor_name}   plot {frame}"));
        ui_.monospace(format!("{:.0} fps   {} vessel(s)", 1.0 / time.delta_secs().max(1e-6), sim.fleet.len()));
    });

    egui::Window::new("Controls").anchor(egui::Align2::LEFT_BOTTOM, [10.0, -10.0]).default_open(false).show(
        ctx,
        |ui_| {
            for line in [
                "Shift/Ctrl  throttle up/down     Z / X  full / cut",
                "W/S pitch   A/D yaw   Q/E roll   T  SAS",
                "P  deploy parachute              R  reset to pad",
                ". / ,  warp up/down   /  warp 1x (rails warp needs throttle 0)",
                "drag  orbit camera    scroll  zoom",
                "F  focus nearest body   `  focus ship   double-click body  menu",
                "Tab  plotting frame     F2  spawn 10 test ships",
                "[ / ]  previous / next vessel   F7  tracking station",
            ] {
                ui_.monospace(line);
            }
        },
    );

    if let Some((node, pos)) = ui.menu {
        let mut open = true;
        let name = sim.world.eph.node(node).name.clone();
        let dist = (snap.relative(node, anchor).r - r).length();
        egui::Window::new(name).open(&mut open).fixed_pos([pos.x, pos.y]).resizable(false).collapsible(false).show(
            ctx,
            |ui_| {
                ui_.monospace(format!("distance {}", fmt_dist(dist)));
                if ui_.button("Focus").clicked() {
                    camera::focus_body(&mut rig, &sim, node);
                    ui.menu = None;
                }
            },
        );
        if !open {
            ui.menu = None;
        }
    }
    Ok(())
}
