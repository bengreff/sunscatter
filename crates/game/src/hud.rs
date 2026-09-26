//! HUD panels (egui): Time (date, warp arrows), Flight (readouts), Debug,
//! drawn as movable windows through [`crate::interface::panel`]; body
//! picking and the double-click body menu.

use crate::camera::{self, CameraRig, MainCamera};
use crate::interface::layout::{InterfaceSettings, PanelId};
use crate::interface::{panel, theme};
use crate::state::{SimState, MAX_PHYSICS_WARP, WARP_LEVELS};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts};
use sim::frame::NodeId;
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

/// The fps readout is averaged over windows at least this long (s).
pub const FPS_WINDOW: f64 = 0.5;

/// A steady fps readout: frames over the last window of at least
/// [`FPS_WINDOW`] seconds, divided by its length, updated once per window.
#[derive(Resource, Default)]
pub struct FpsMeter {
    frames: u32,
    elapsed: f64,
    fps: Option<f64>,
}

impl FpsMeter {
    pub fn tick(&mut self, dt: f64) {
        self.frames += 1;
        self.elapsed += dt;
        if self.elapsed >= FPS_WINDOW {
            self.fps = Some(f64::from(self.frames) / self.elapsed);
            (self.frames, self.elapsed) = (0, 0.0);
        }
    }

    /// The last complete window's rate (`None` before the first one ends).
    pub fn fps(&self) -> Option<f64> {
        self.fps
    }

    /// For display: "60" or "--".
    pub fn text(&self) -> String {
        self.fps().map_or_else(|| "--".into(), |f| format!("{f:.0}"))
    }
}

pub fn measure_fps(time: Res<Time>, mut meter: ResMut<FpsMeter>) {
    meter.tick(time.delta_secs_f64());
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

/// Why warp is lower than requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarpLimit {
    /// Rails warp needs the throttle closed and the ship not under thrust.
    Throttle,
    /// Coast integration can't keep up at this warp.
    Compute,
}

/// What the warp indicator shows: the level in effect, whether it is rails
/// or physics warp, and why it is below the requested level, if it is.
pub fn warp_status(requested: usize, effective: usize, compute_limited: bool) -> (usize, bool, Option<WarpLimit>) {
    let rails = effective > MAX_PHYSICS_WARP;
    let limit = if effective < requested {
        Some(WarpLimit::Throttle)
    } else if compute_limited {
        Some(WarpLimit::Compute)
    } else {
        None
    };
    (effective, rails, limit)
}

fn warp_label(x: f64) -> String {
    if x >= 1e3 {
        format!("{:.0}kx", x / 1e3)
    } else {
        format!("{x:.0}x")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    fps: Res<FpsMeter>,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut ui: ResMut<UiState>,
    mut iface: ResMut<InterfaceSettings>,
    station: Res<crate::tracking::TrackingStation>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let (y, mo, d, h, mi, s) = sim.clock.to_calendar();
    let dim = |t: &str| egui::RichText::new(t).monospace().small().color(theme::DIM);

    // The tracking station is its own screen: no flight HUD.
    if station.open {
        return Ok(());
    }
    let show_fps = iface.show_fps;
    let mut set_warp = None;
    panel(ctx, &mut iface, PanelId::Time, |ui_| {
        ui_.monospace(format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:04.1} TDB"));
        let (level, rails, limit) = warp_status(sim.warp, sim.effective_warp(), sim.compute_limited);
        ui_.horizontal(|ui_| {
            ui_.spacing_mut().item_spacing.x = 1.0;
            for (i, &w) in WARP_LEVELS.iter().enumerate() {
                let (glyph, color) = if i <= level {
                    ("▶", if i > MAX_PHYSICS_WARP { theme::ACCENT } else { theme::TEXT })
                } else if i <= sim.warp {
                    ("▶", theme::WARN)
                } else {
                    ("▷", theme::DIM)
                };
                let arrow =
                    egui::Label::new(egui::RichText::new(glyph).monospace().color(color)).sense(egui::Sense::click());
                if ui_.add(arrow).on_hover_text(warp_label(w)).clicked() {
                    set_warp = Some(i);
                }
            }
        });
        let kind = if rails { "rails" } else { "physics" };
        let why = match limit {
            Some(WarpLimit::Throttle) => "  limited: throttle",
            Some(WarpLimit::Compute) => "  limited: compute",
            None => "",
        };
        ui_.horizontal(|ui_| {
            ui_.monospace(format!("warp {} ({kind})", warp_label(WARP_LEVELS[level])));
            if !why.is_empty() {
                ui_.label(egui::RichText::new(why).monospace().small().color(theme::WARN));
            }
            if show_fps {
                ui_.label(dim(&format!("  {} fps", fps.text())));
            }
        });
    });
    if let Some(i) = set_warp {
        sim.warp = i;
    }
    let sim = sim.into_inner();
    let (anchor, r, _) = sim.ship().state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    // Readouts are relative to the dominant body, like the navball's.
    let eph = &sim.world.eph;
    let dominant = crate::relations::Dominance::new(eph, sim.clock).of(eph, sim.clock, anchor, r, None, None);

    panel(ctx, &mut iface, PanelId::Flight, |ui_| flight_panel(ui_, sim, Some(dominant)));

    panel(ctx, &mut iface, PanelId::Debug, |ui_| {
        let anchor_name = &sim.world.eph.node(anchor).name;
        let frame = match ui.plot_frame {
            PlotFrame::EarthInertial => "Earth inertial",
            PlotFrame::EarthMoonRotating => "Earth–Moon rotating",
        };
        ui_.monospace(format!("anchor {anchor_name}   plot {frame}"));
        ui_.monospace(format!("{} fps   {} vessel(s)", fps.text(), sim.fleet.len()));
    });

    if let Some((node, pos)) = ui.menu {
        let mut open = true;
        let name = sim.world.eph.node(node).name.clone();
        let dist = (snap.relative(node, anchor).r - r).length();
        egui::Window::new(name).open(&mut open).fixed_pos([pos.x, pos.y]).resizable(false).collapsible(false).show(
            ctx,
            |ui_| {
                ui_.monospace(format!("distance {}", crate::format::distance(dist)));
                if ui_.button("Focus").clicked() {
                    camera::focus_body(&mut rig, sim, node);
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

/// The Flight panel: phase, altitudes, speeds, apsides, throttle, SAS, chute.
fn flight_panel(ui_: &mut egui::Ui, sim: &SimState, near: Option<sim::frame::NodeId>) {
    let dim = |t: &str| egui::RichText::new(t).monospace().small().color(theme::DIM);
    let ship = sim.ship();
    let (anchor, r, v) = ship.state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let phase = match &ship.phase {
        Phase::Landed { .. } => "LANDED".to_string(),
        Phase::Powered { .. } => "POWERED".to_string(),
        Phase::Coasting { .. } => "COASTING".to_string(),
        Phase::Crashed { speed, .. } => format!("CRASHED at {speed:.0} m/s (R to reset)"),
    };
    ui_.label(egui::RichText::new(phase).monospace().color(theme::ACCENT));
    if let Some(body) = near.and_then(|b| sim.world.source(b)) {
        let k = snap.relative(body.node, anchor);
        let rel = r - k.r;
        let p = body.physical.as_ref().expect("surface body");
        let fixed = p.rotation.to_fixed(sim::frame::Vec3::from_raw(rel), sim.clock);
        let alt = p.altitude(fixed);
        let above_ground = p.altitude_above_surface(fixed);
        let v_orb = v - k.v;
        let v_srf = v_orb - p.rotation.omega(sim.clock).raw().cross(rel);
        let el = crate::relations::orbit_about(&sim.world, sim.clock, anchor, r, v, body.node)
            .expect("nearest body is a source")
            .elements;
        egui::Grid::new("flight_grid").num_columns(2).spacing([10.0, 2.0]).show(ui_, |ui_| {
            let mut row = |label: &str, value: String| {
                ui_.label(dim(label));
                ui_.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui_| ui_.monospace(value));
                ui_.end_row();
            };
            row("BODY", body.name.clone());
            row("ALT (SEA)", crate::format::distance(alt));
            row("ALT (TERRAIN)", crate::format::distance(above_ground));
            row("SURFACE V", format!("{:.1} m/s", v_srf.length()));
            row("ORBIT V", format!("{:.1} m/s", v_orb.length()));
            row("V/S", format!("{:+.1} m/s", v_srf.dot(rel.normalize())));
            row("Ap", crate::format::distance(el.apoapsis() - p.radius_eq));
            row("Pe", crate::format::distance(el.periapsis() - p.radius_eq));
        });
    }
    let c = sim.controls;
    ui_.add(egui::ProgressBar::new(c.throttle as f32).text(format!("throttle {:.0}%", c.throttle * 100.0)));
    ui_.monospace(format!(
        "SAS {}   chute {}",
        if c.sas { "on" } else { "off" },
        if ship.chute_deployed { "DEPLOYED" } else { "stowed" }
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_indicator_shows_the_level_kind_and_limit() {
        // (requested, effective, compute-limited, expected)
        let cases = [
            (0, 0, false, (0, false, None)),
            (3, 3, false, (3, false, None)),
            (6, 6, false, (6, true, None)),
            (6, 3, false, (3, false, Some(WarpLimit::Throttle))),
            (9, 9, true, (9, true, Some(WarpLimit::Compute))),
        ];
        for (req, eff, compute, expected) in cases {
            assert_eq!(warp_status(req, eff, compute), expected, "{req} {eff} {compute}");
        }
    }

    #[test]
    fn fps_is_frames_over_at_least_half_a_second() {
        // Frame times exact in binary, so window ends are exact.
        let mut m = FpsMeter::default();
        for _ in 0..3 {
            m.tick(0.125);
        }
        assert_eq!(m.fps(), None, "not a full window yet");
        m.tick(0.125);
        assert_eq!(m.fps(), Some(8.0));
        // One slow frame lowers the next window's average, not to 1/dt.
        m.tick(0.125);
        m.tick(0.125);
        assert_eq!(m.fps(), Some(8.0), "held between updates");
        m.tick(0.25);
        assert_eq!(m.fps(), Some(6.0));
    }
}
