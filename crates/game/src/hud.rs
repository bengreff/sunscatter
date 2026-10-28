//! The time panel (date, warp arrows, the DEBUG badge), the flight state
//! line's rule, the craft readouts' formulas the menus use, body picking
//! and the double-click body menu. The navball cluster is in
//! [`crate::navball`], the task menus in [`crate::interface::menus`].

use crate::camera::{self, CameraRig};
use crate::interface::layout::{self, InterfaceSettings};
use crate::interface::{fixed_panel, theme};
use crate::state::{SimState, MAX_PHYSICS_WARP, WARP_LEVELS};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts};
use sim::frame::NodeId;
use sim::vessel::Phase;

#[derive(Resource, Default)]
pub struct UiState {
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

/// Double-click on a body opens its menu: the heaviest body under the
/// cursor (D054, `map_view::pick_body`), a planet over its moons.
pub fn pick_bodies(
    buttons: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    egui: Res<EguiWantsInput>,
    window: Query<&Window, With<PrimaryWindow>>,
    map: Res<crate::map::MapView>,
    mut ui: ResMut<UiState>,
) {
    if !buttons.just_pressed(MouseButton::Left) || egui.wants_any_pointer_input() {
        return;
    }
    let Ok(window) = window.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let now = time.elapsed_secs_f64();
    let double = ui.last_click.is_some_and(|(t, p)| now - t < 0.4 && p.distance(cursor) < 8.0);
    ui.last_click = Some((now, cursor));
    if !double {
        return;
    }
    if let Some(node) = map.pick_body(cursor) {
        ui.menu = Some((node, cursor));
    }
}

/// Why warp is lower than requested.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WarpLimit {
    /// Rails warp needs the ship to produce no thrust (D111:
    /// `state::produces_thrust`), whatever the throttle.
    Thrust,
    /// Coast integration can't keep up at this warp.
    Compute,
    /// Below a body's rails floor (D062): the floor altitude (m).
    Floor(f64),
    /// Flown live near the ground.
    Live,
    /// Another vessel (id) is flying live.
    OtherLive(u64),
    /// Held live near a live controllable vessel (id; D082).
    Near(u64),
    /// Held on the pad's clamps with its systems live (`state::held_live`).
    Held,
    /// Another vessel (id) is held on the pad with its systems live.
    OtherHeld(u64),
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
        Some(floor.map_or(WarpLimit::Thrust, WarpLimit::Floor))
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
    // The origin's clock (D088): the launch site's proper time, TT.
    let (y, mo, d, h, mi, s) = sim.site_time().to_calendar();
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
            ui_.monospace(format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:04.1} TT")).on_hover_text(format!(
                "The clock at {}, the origin (D088): its proper time, which is TT (the geoid's time) to within \
                 microseconds. Ship clocks are shown against it (Utilities ▸ Clock); the physics runs on TDB.",
                sim.origin
            ));
            ui_.label(dim(&sim.origin));
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
            Some(crate::state::RailsBlock::Live) if limit == Some(WarpLimit::Thrust) => limit = Some(WarpLimit::Live),
            Some(crate::state::RailsBlock::OtherLive(id)) if limit == Some(WarpLimit::Thrust) => {
                limit = Some(WarpLimit::OtherLive(id.0));
            }
            Some(crate::state::RailsBlock::Near(id)) if limit == Some(WarpLimit::Thrust) => {
                limit = Some(WarpLimit::Near(id.0));
            }
            Some(crate::state::RailsBlock::Held) if limit == Some(WarpLimit::Thrust) => limit = Some(WarpLimit::Held),
            Some(crate::state::RailsBlock::OtherHeld(id)) if limit == Some(WarpLimit::Thrust) => {
                limit = Some(WarpLimit::OtherHeld(id.0));
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
            Some(WarpLimit::Thrust) => "  limited: thrust".to_string(),
            Some(WarpLimit::Compute) => format!("  limited: compute, {} achieved", warp_label(sim.achieved_warp)),
            Some(WarpLimit::Floor(f)) => format!("  limited: below {}", crate::format::distance(f)),
            Some(WarpLimit::Live) => "  limited: flying live".to_string(),
            Some(WarpLimit::OtherLive(id)) => format!("  limited: Vessel {id} is flying live"),
            Some(WarpLimit::Near(id)) => format!("  limited: near Vessel {id}"),
            Some(WarpLimit::Held) => "  limited: held live on the pad (engines lit or release armed)".to_string(),
            Some(WarpLimit::OtherHeld(id)) => format!("  limited: Vessel {id} held live on the pad"),
            None => String::new(),
        };
        ui_.horizontal(|ui_| {
            if sim.paused {
                let stopped = egui::RichText::new("TIME STOPPED").monospace().strong().color(theme::WARN);
                ui_.label(stopped).on_hover_text("`.` (or any warp) runs it again at 1x");
            } else {
                ui_.monospace(format!("warp {} ({kind})", warp_label(WARP_LEVELS[level])));
            }
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

/// The vehicle's mass (kg): the vessel's less the ground it still holds
/// (a craft of parts' `PartInfo::ground` parts and their components: the
/// pad's launch mount until the clamps release), so mass, thrust-to-weight
/// and Δv readouts are the vehicle's.
pub fn vehicle_mass(v: &sim::vessel::Vessel) -> f64 {
    let Some(def) = v.ship_def() else { return v.mass() };
    let ground: Vec<u32> = def.parts.iter().filter(|p| p.ground).map(|p| p.part).collect();
    let parts: f64 = def.parts.iter().filter(|p| p.ground).map(|p| p.mass).sum();
    let components: f64 = def.components.iter().filter(|c| ground.contains(&c.part)).map(|c| c.mass).sum();
    v.mass() - parts - components
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

/// Held by the pad's clamps (D104): what the state line says of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pad {
    /// Nothing lit.
    Idle,
    /// Engines lit, the release not armed.
    Lit,
    /// The release armed at full throttle: it lets go once every lit
    /// engine reaches its rated thrust.
    Releasing,
    /// The release armed with the throttle below 100 %: the engines never
    /// reach their rated thrust, so it never lets go (they burn on the pad).
    ThrottleUp,
}

/// What the pad's state line says: from engines lit, the release armed
/// (`Runtime::release_armed`) and the pilot's throttle.
pub fn pad_state(lit: bool, armed: bool, throttle: f64) -> Pad {
    match (armed, lit) {
        (true, _) if throttle < 1.0 => Pad::ThrottleUp,
        (true, _) => Pad::Releasing,
        (false, true) => Pad::Lit,
        (false, false) => Pad::Idle,
    }
}

/// The flight state under the navball, and whether it is an alert: a
/// wreck's cause (at once, while it still moves), CLAMPED on the pad (with
/// its engines lit or not, the release armed: it waits for their rated
/// thrust, an alert below full throttle), LANDED, POWERED (only a running
/// engine: live, or a planned burn on rails), ON PARACHUTE, or FLIGHT
/// (gliding, falling or coasting with the engine off).
pub fn state_word(
    destroyed: Option<String>,
    pad: Option<Pad>,
    landed: bool,
    engine: bool,
    chute: bool,
) -> (String, bool) {
    match (destroyed, landed, engine, chute) {
        (Some(cause), ..) => (format!("{cause} (R R to reset)"), true),
        (None, ..) if pad == Some(Pad::ThrottleUp) => ("CLAMPED · THROTTLE UP TO RELEASE".into(), true),
        (None, ..) if pad == Some(Pad::Releasing) => ("CLAMPED · RELEASING AT FULL THRUST".into(), false),
        (None, ..) if pad == Some(Pad::Lit) => ("CLAMPED · ENGINES LIT".into(), false),
        (None, ..) if pad == Some(Pad::Idle) => ("CLAMPED".into(), false),
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
    let lit = |s: &sim::craft::state::ShipState| {
        use sim::craft::state::EngineStatus::{Running, Starting};
        s.engines.iter().any(|e| matches!(e.status, Starting | Running))
    };
    let pad = ship.ship().filter(|_| ship.clamped());
    let pad = pad.map(|s| pad_state(lit(s), s.runtime.release_armed, sim.controls.throttle));
    state_word(destroyed, pad, landed, crate::scene::engine_on(sim, sim.active), ship.chute.open())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_state_line_is_powered_only_with_the_engine_running() {
        let wreck = || Some("CRASHED at 40 m/s".to_string());
        // (destroyed, pad, landed, engine, chute, expected, alert)
        let table = [
            (None, None, false, true, false, "POWERED", false),
            (None, None, false, true, true, "POWERED", false),
            (None, None, false, false, true, "ON PARACHUTE", false),
            (None, None, false, false, false, "FLIGHT", false),
            (None, None, true, false, false, "LANDED", false),
            (None, Some(Pad::Idle), true, false, false, "CLAMPED", false),
            (None, Some(Pad::Lit), true, false, false, "CLAMPED · ENGINES LIT", false),
            (None, Some(Pad::Releasing), true, false, false, "CLAMPED · RELEASING AT FULL THRUST", false),
            (None, Some(Pad::ThrottleUp), true, false, false, "CLAMPED · THROTTLE UP TO RELEASE", true),
            (wreck(), None, false, false, true, "CRASHED at 40 m/s (R R to reset)", true),
            (wreck(), Some(Pad::Lit), true, false, false, "CRASHED at 40 m/s (R R to reset)", true),
        ];
        for (destroyed, pad, landed, engine, chute, want, alert) in table {
            assert_eq!(state_word(destroyed, pad, landed, engine, chute), (want.to_string(), alert));
        }
    }

    /// The release lets go only at every lit engine's rated thrust: armed
    /// below full throttle, the line says to throttle up (review H3).
    #[test]
    fn the_pad_line_asks_for_full_throttle_while_the_release_waits() {
        // (lit, armed, throttle, expected)
        let table = [
            (false, false, 0.0, Pad::Idle),
            (false, false, 1.0, Pad::Idle),
            (true, false, 0.0, Pad::Lit),
            (true, false, 1.0, Pad::Lit),
            (true, true, 0.0, Pad::ThrottleUp),
            (true, true, 0.99, Pad::ThrottleUp),
            (true, true, 1.0, Pad::Releasing),
            (false, true, 0.5, Pad::ThrottleUp),
            (false, true, 1.0, Pad::Releasing),
        ];
        for (lit, armed, throttle, want) in table {
            assert_eq!(pad_state(lit, armed, throttle), want, "{lit} {armed} {throttle}");
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
            (6, 3, false, (3, false, Some(WarpLimit::Thrust))),
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
