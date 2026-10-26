//! The time panel (date, warp arrows, the DEBUG badge), the flight state
//! line's rule, the craft readouts' formulas the menus use, body picking
//! and the double-click body menu. The navball cluster is in
//! [`crate::navball`], the task menus in [`crate::interface::menus`].

use crate::camera::{self, CameraRig, MainCamera};
use crate::interface::layout::{self, InterfaceSettings};
use crate::interface::{fixed_panel, theme};
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
    if keys.just_pressed(KeyCode::Tab) && !egui.wants_any_keyboard_input() {
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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WarpLimit {
    /// Rails warp needs the throttle closed and the ship not under thrust.
    Throttle,
    /// Coast integration can't keep up at this warp.
    Compute,
    /// Below a body's rails floor (D062): the floor altitude (m).
    Floor(f64),
    /// Flown live near the ground.
    Live,
    /// Another vessel (id) is flying live.
    OtherLive(u64),
}

/// What the warp indicator shows: the level in effect, whether it is rails
/// or physics warp, and why it is below the requested level, if it is.
pub fn warp_status(
    requested: usize,
    effective: usize,
    compute_limited: bool,
    floor: Option<f64>,
) -> (usize, bool, Option<WarpLimit>) {
    let rails = effective > MAX_PHYSICS_WARP;
    let limit = if effective < requested {
        Some(floor.map_or(WarpLimit::Throttle, WarpLimit::Floor))
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

/// The time panel, top left (fixed, D080): the date, the warp arrows and
/// status, the frame rate, and a DEBUG badge in debug mode (D064).
pub fn draw(
    mut contexts: EguiContexts,
    fps: Res<FpsMeter>,
    sim: Res<SimState>,
    mut commands: MessageWriter<crate::commands::GameCommand>,
    iface: Res<InterfaceSettings>,
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
    let margin = [layout::MARGIN, layout::MARGIN];
    fixed_panel(ctx, layout::TIME_AREA, egui::Align2::LEFT_TOP, margin, |ui_| {
        ui_.horizontal(|ui_| {
            ui_.monospace(format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:04.1} TDB"));
            if sim.ship().debug() {
                let badge = egui::RichText::new("DEBUG").monospace().small().strong().color(theme::WARN);
                ui_.label(badge).on_hover_text("Debug mode (D064): infinite propellant, no overheating, unbreakable");
            }
        });
        let block = sim.rails_block();
        let floor = match block {
            Some(crate::state::RailsBlock::Floor { floor, .. }) => Some(floor),
            _ => None,
        };
        let (level, rails, mut limit) = warp_status(sim.warp, sim.effective_warp(), sim.compute_limited, floor);
        match block {
            Some(crate::state::RailsBlock::Live) if limit == Some(WarpLimit::Throttle) => limit = Some(WarpLimit::Live),
            Some(crate::state::RailsBlock::OtherLive(id)) if limit == Some(WarpLimit::Throttle) => {
                limit = Some(WarpLimit::OtherLive(id.0));
            }
            _ => {}
        }
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
            Some(WarpLimit::Throttle) => "  limited: throttle".to_string(),
            Some(WarpLimit::Compute) => format!("  limited: compute, {} achieved", warp_label(sim.achieved_warp)),
            Some(WarpLimit::Floor(f)) => format!("  limited: below {}", crate::format::distance(f)),
            Some(WarpLimit::Live) => "  limited: flying live".to_string(),
            Some(WarpLimit::OtherLive(id)) => format!("  limited: Vessel {id} is flying live"),
            None => String::new(),
        };
        ui_.horizontal(|ui_| {
            ui_.monospace(format!("warp {} ({kind})", warp_label(WARP_LEVELS[level])));
            if !why.is_empty() {
                ui_.label(egui::RichText::new(why).monospace().small().color(theme::WARN));
            }
            if show_fps {
                ui_.label(dim(&format!("  {} fps, sim {:.1} ms", fps.text(), sim.sim_ms)));
            }
        });
    });
    if let Some(i) = set_warp {
        commands.write(crate::commands::GameCommand::SetWarp(i));
    }
    Ok(())
}

/// A temperature with its limit: "812 K / 1100 K" (warning shown by the
/// caller's colour; here the text).
pub fn temperature(t: f64, limit: f64) -> String {
    let mark = if limit > 0.0 && t >= 0.9 * limit { " !" } else { "" };
    format!("{t:.0} K / {limit:.0} K{mark}")
}

/// The ship clock's offset from the game clock (proper minus coordinate
/// time, D012), in the unit that shows it.
pub fn clock_offset(s: f64) -> String {
    let a = s.abs();
    if a < 1e-3 {
        format!("{:+.2} µs", s * 1e6)
    } else if a < 1.0 {
        format!("{:+.3} ms", s * 1e3)
    } else {
        format!("{s:+.3} s")
    }
}

/// Rocket-equation Δv (m/s) of `propellant` (kg) burnt from mass `mass`.
pub fn delta_v(isp: f64, mass: f64, propellant: f64) -> f64 {
    let dry = (mass - propellant).max(1e-9);
    isp * sim::vessel::G0 * (mass / dry).ln()
}

/// Thrust-to-weight ratio in local gravity `g` (m/s²); `None` in free space.
pub fn twr(thrust: f64, mass: f64, g: f64) -> Option<f64> {
    (g > 1e-6).then(|| thrust / (mass * g))
}

/// The double-click body menu (in flight and in the tracking station).
pub fn draw_body_menu(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    mut rig: ResMut<CameraRig>,
    mut ui: ResMut<UiState>,
) -> Result {
    let Some((node, pos)) = ui.menu else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let (anchor, r, _) = sim.ship().state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let mut open = true;
    let name = sim.world.eph.node(node).name.clone();
    let dist = (snap.relative(node, anchor).r - r).length();
    egui::Window::new(name).open(&mut open).fixed_pos([pos.x, pos.y]).resizable(false).collapsible(false).show(
        ctx,
        |ui_| {
            ui_.monospace(format!("distance {}", crate::format::distance(dist)));
            if ui_.button("Focus").clicked() {
                camera::focus_body(&mut rig, &sim, node);
                ui.menu = None;
            }
        },
    );
    if !open {
        ui.menu = None;
    }
    Ok(())
}

/// The flight state under the navball, and whether it is an alert: a
/// wreck's cause (at once, while it still moves), LANDED, POWERED (only a
/// running engine: live, or a planned burn on rails), ON PARACHUTE, or
/// FLIGHT (gliding, falling or coasting with the engine off).
pub fn state_word(destroyed: Option<String>, landed: bool, engine: bool, chute: bool) -> (String, bool) {
    match (destroyed, landed, engine, chute) {
        (Some(cause), ..) => (format!("{cause} (R R to reset)"), true),
        (None, true, ..) => ("LANDED".into(), false),
        (None, false, true, _) => ("POWERED".into(), false),
        (None, false, false, true) => ("ON PARACHUTE".into(), false),
        (None, false, false, false) => ("FLIGHT".into(), false),
    }
}

/// [`state_word`] for the active vessel.
pub fn state_line(sim: &SimState) -> (String, bool) {
    let ship = sim.ship();
    let destroyed = match (&ship.phase, ship.destruction()) {
        (_, Some(cause)) => Some(cause.describe()),
        (Phase::Crashed { cause, .. }, None) => Some(cause.describe()),
        _ => None,
    };
    let landed = matches!(ship.phase, Phase::Landed { .. });
    state_word(destroyed, landed, crate::scene::engine_on(sim, sim.active), ship.chute.open())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_line_is_powered_only_with_the_engine_running() {
        let wreck = || Some("CRASHED at 40 m/s".to_string());
        // (destroyed, landed, engine, chute, expected, alert)
        let table = [
            (None, false, true, false, "POWERED", false),
            (None, false, true, true, "POWERED", false),
            (None, false, false, true, "ON PARACHUTE", false),
            (None, false, false, false, "FLIGHT", false),
            (None, true, false, false, "LANDED", false),
            (wreck(), false, false, true, "CRASHED at 40 m/s (R R to reset)", true),
            (wreck(), true, false, false, "CRASHED at 40 m/s (R R to reset)", true),
        ];
        for (destroyed, landed, engine, chute, want, alert) in table {
            assert_eq!(state_word(destroyed, landed, engine, chute), (want.to_string(), alert));
        }
    }

    #[test]
    fn temperatures_warn_near_the_limit() {
        assert_eq!(temperature(812.4, 1100.0), "812 K / 1100 K");
        assert_eq!(temperature(1000.0, 1100.0), "1000 K / 1100 K !");
    }

    #[test]
    fn clock_offsets_read_in_their_unit() {
        assert_eq!(clock_offset(-3.2e-7), "-0.32 µs");
        assert_eq!(clock_offset(0.0123), "+12.300 ms");
        assert_eq!(clock_offset(-1.5), "-1.500 s");
    }

    #[test]
    fn delta_v_and_twr_follow_the_rocket_equation() {
        // 20 t with 16 t of propellant at Isp 320 s: 320·g0·ln 5.
        let dv = delta_v(320.0, 20_000.0, 16_000.0);
        assert!((dv - 320.0 * sim::vessel::G0 * 5f64.ln()).abs() < 1e-9);
        assert!((dv - 5051.0).abs() < 1.0, "{dv}");
        assert_eq!(delta_v(320.0, 4_000.0, 0.0), 0.0);
        let t = twr(300e3, 20_000.0, 9.81).unwrap();
        assert!((t - 1.529).abs() < 1e-3);
        assert_eq!(twr(300e3, 20_000.0, 0.0), None);
    }

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
            assert_eq!(warp_status(req, eff, compute, None), expected, "{req} {eff} {compute}");
        }
        assert_eq!(warp_status(6, 3, false, Some(150e3)), (3, false, Some(WarpLimit::Floor(150e3))));
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
