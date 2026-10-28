//! The navball cluster (D080), always on, at a fixed place bottom centre:
//! the speed under its `ORBIT | SURFACE | TARGET` switch, the altitude
//! (its label the datum / terrain switch), G (load, direction, limit), Ap
//! and Pe left of the ball, SAS with its hold modes, RCS and the throttle
//! bar right of it, and the flight state under it. Everything
//! else is in the task menus. Hidden in the tracking station.

use super::altitude::AltitudeRef;
use super::rules::{self, Mode};
use super::{g_meter, ApsisReadout, NavState, Navball};
use crate::commands::GameCommand;
use crate::format::{duration as fmt_duration, DASH};
use crate::interface::layout::InterfaceSettings;
use crate::interface::theme;
use crate::state::SimState;
use crate::tracking::TrackingStation;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use egui::{vec2, Align2, Color32, RichText, Stroke};
use sim::systems::Quantity;
use sim::vessel::HoldMode;

/// Width of the readouts left of the ball and of the controls right of it.
const SIDE_PX: f32 = 150.0;
/// Width of the hold-mode buttons.
const HOLD_PX: f32 = 92.0;
/// The throttle bar's width.
const BAR_PX: f32 = 14.0;

/// The cluster's egui area (the docks keep clear of its rect).
pub const AREA: &str = "navball_cluster";

/// A user action on the cluster, applied after drawing.
enum Action {
    SetMode(Mode),
    ToggleLock,
    CycleMode,
    ToggleAltitude,
    Command(GameCommand),
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    time: Res<Time>,
    sim: Res<SimState>,
    station: Res<TrackingStation>,
    iface: Res<InterfaceSettings>,
    mut nav: ResMut<Navball>,
    mut commands: MessageWriter<GameCommand>,
) -> Result {
    if station.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let Some(state) = nav.now.clone() else { return Ok(()) };
    let shown = nav.readouts(time.elapsed_secs_f64(), &state).clone();
    let locked = nav.locked;
    let ball = iface.navball_size;
    let mut actions = Vec::new();
    crate::interface::fixed_panel(ctx, AREA, Align2::CENTER_BOTTOM, [0.0, -8.0], |ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
        let has_target = shown.target.is_some();
        centred_row(ui, egui::Id::new("nav_switch"), |ui| switch(ui, shown.mode, locked, has_target, &mut actions));
        ui.vertical_centered(|ui| {
            let speed = shown.known.speed(Quantity::Velocity, shown.speed);
            ui.label(RichText::new(speed).monospace().size(20.0).color(theme::TEXT));
            let about = shown.target.as_ref().filter(|_| shown.mode == Mode::Target).map_or(&shown.body, |t| &t.0);
            ui.label(RichText::new(format!("{} · {about}", shown.mode.label())).monospace().small().color(theme::DIM));
        });
        ui.add_space(2.0);
        ui.horizontal_top(|ui| {
            fixed_column(ui, SIDE_PX, ball, |ui| readouts(ui, &shown, &mut actions));
            let (rect, response) = ui.allocate_exact_size(vec2(ball, ball), egui::Sense::click());
            super::ball::paint_ball(ui.painter(), rect, &state);
            if response.on_hover_text("Click to change the speed mode").clicked() {
                actions.push(Action::CycleMode);
            }
            fixed_column(ui, SIDE_PX, ball, |ui| controls(ui, &sim, &shown, ball, &mut actions));
        });
        let (line, alert) = crate::hud::state_line(&sim);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(line).monospace().color(if alert { theme::ALERT } else { theme::ACCENT }));
        });
    });
    for action in actions {
        match action {
            // A choice by hand holds until the next crossing of the rails
            // floor (D089), or for good while locked.
            Action::SetMode(mode) => nav.mode = mode,
            Action::ToggleLock => nav.locked = !nav.locked,
            Action::CycleMode => nav.mode = nav.mode.next(nav.target.is_some()),
            Action::ToggleAltitude => nav.altitude = nav.altitude.other(),
            Action::Command(c) => {
                commands.write(c);
            }
        }
    }
    Ok(())
}

/// A row centred in the available width, from its width last frame (egui
/// lays out left to right; the first frame is left-aligned).
fn centred_row(ui: &mut egui::Ui, id: egui::Id, add: impl FnOnce(&mut egui::Ui)) {
    let width = ui.ctx().data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let pad = ((ui.available_width() - width) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        ui.add_space(pad);
        let start = ui.cursor().min.x;
        add(ui);
        let used = ui.min_rect().max.x - start;
        ui.ctx().data_mut(|d| d.insert_temp(id, used));
    });
}

/// A column of fixed width and height.
fn fixed_column(ui: &mut egui::Ui, width: f32, height: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(vec2(width, height), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.set_width(width);
        ui.set_min_height(height);
        add(ui);
    });
}

/// The speed mode switch, `ORBIT | SURFACE | TARGET`, and AUTO/LOCK.
fn switch(ui: &mut egui::Ui, mode: Mode, locked: bool, has_target: bool, actions: &mut Vec<Action>) {
    ui.spacing_mut().item_spacing.x = 2.0;
    for m in [Mode::Orbit, Mode::Surface, Mode::Target] {
        let text = RichText::new(m.label()).monospace().small();
        let button = egui::Button::new(text).selected(m == mode);
        let r = ui.add_enabled(rules::mode_available(m, has_target), button);
        let hint = match m {
            Mode::Orbit => "Speed and markers relative to the body's centre",
            Mode::Surface => "Speed and markers relative to the rotating surface",
            Mode::Target => "Speed and markers relative to the target (pick one in Orbital ▸ Target)",
        };
        if r.on_hover_text(hint).clicked() && m != mode {
            actions.push(Action::SetMode(m));
        }
    }
    ui.add_space(6.0);
    let lock = RichText::new(if locked { "LOCK" } else { "AUTO" }).monospace().small().color(theme::DIM);
    let r = ui.add(egui::Button::new(lock).frame(false));
    let hover = "AUTO: SURFACE below the altitude where rails warp stops (Earth: 150 km), ORBIT above; a mode \
                 picked by hand holds until the next crossing. LOCK: never switches. The orbit line follows the \
                 mode: over the ground in SURFACE, about the target body in TARGET.";
    if r.on_hover_text(hover).clicked() {
        actions.push(Action::ToggleLock);
    }
}

/// Altitude (its label the switch), G, Ap and Pe with the time to each
/// (D076), as the craft knows them (D106: a dash for what it cannot
/// measure).
fn readouts(ui: &mut egui::Ui, s: &NavState, actions: &mut Vec<Action>) {
    let k = &s.known;
    let orbit = k.orbit();
    let apsis = |name: &str, a: &Option<ApsisReadout>| match a {
        Some(a) => [
            (
                a.other_body.as_ref().map_or(name.to_string(), |b| format!("{name} {b}")),
                k.distance(Quantity::Position, a.above_datum),
            ),
            ("  in".to_string(), k.or_dash(orbit, || fmt_duration(a.in_s))),
        ],
        None => [(name.to_string(), DASH.to_string()), ("  in".to_string(), DASH.to_string())],
    };
    // Above the terrain from an altimeter (or a fix), above the datum from the position.
    let alt_q = if s.altitude_ref == AltitudeRef::Terrain { Quantity::Altitude } else { Quantity::Position };
    let mut rows = vec![];
    rows.extend(apsis("Ap", &s.ap));
    rows.extend(apsis("Pe", &s.pe));
    ui.add_space(10.0);
    let dim = |text: &str| RichText::new(text).monospace().small().color(theme::DIM);
    let value = |ui: &mut egui::Ui, text: String, colour: Color32| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(text).monospace().color(colour))
        })
        .inner
    };
    egui::Grid::new("nav_readouts").num_columns(2).spacing([8.0, 3.0]).min_col_width(40.0).show(ui, |ui| {
        let switch = ui.add(egui::Button::new(dim(&s.altitude.0)).frame(false));
        let hint = "Click: above the datum (sea level; without a sea the lowest ground; on a giant its 1-bar \
                    level) or above the terrain under the craft (TER). Ap and Pe are above the datum.";
        if switch.on_hover_text(hint).clicked() {
            actions.push(Action::ToggleAltitude);
        }
        value(ui, k.distance(alt_q, s.altitude.1), theme::TEXT);
        ui.end_row();
        ui.label(dim("G"));
        let known = k.knows(Quantity::Acceleration);
        let limit = s.g.limit.as_ref().map(|(l, _)| *l);
        let colour = match g_meter::level(s.g.load, limit) {
            g_meter::Level::Normal => theme::TEXT,
            g_meter::Level::Warn => theme::ACCENT,
            g_meter::Level::Over => theme::ALERT,
        };
        let text = if known { g_meter::text(&s.g) } else { DASH.to_string() };
        value(ui, text, if known { colour } else { theme::TEXT }).on_hover_text(g_meter::hover(&s.g));
        ui.end_row();
        for (label, text) in rows {
            ui.label(dim(&label));
            value(ui, text, theme::TEXT);
            ui.end_row();
        }
    });
}

/// SAS, the hold modes (click only, D111), and the throttle bar.
fn controls(ui: &mut egui::Ui, sim: &SimState, s: &NavState, height: f32, actions: &mut Vec<Action>) {
    let c = *sim.controls();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let sas = hold_button(ui, if c.sas { "SAS ON" } else { "SAS OFF" }, "T", c.sas, true);
            // The hold stated in full, with prograde's speed reference (D075).
            if sas.on_hover_text(format!("SAS: {}", rules::sas_label(&c, &s.body))).clicked() {
                actions.push(Action::Command(GameCommand::SetSas(!c.sas)));
            }
            let rcs = hold_button(ui, if c.rcs.on { "RCS ON" } else { "RCS OFF" }, "R", c.rcs.on, true);
            let hint = "RCS: the thrusters translate (I K / J L / H N) and turn the craft; off, the rotation keys \
                        use gimbals and fins only (SAS still fires them). Its menu: blocks, feeds, modes.";
            if rcs.on_hover_text(hint).clicked() {
                let mode = sim::systems::rcs::RcsMode { on: !c.rcs.on, ..c.rcs };
                actions.push(Action::Command(GameCommand::SetRcs(mode)));
            }
            let has_burn = s.maneuver.is_some();
            for mode in HoldMode::ALL {
                let active = c.sas && c.hold == mode;
                let enabled = active || rules::hold_available(mode, s.target.is_some(), has_burn);
                let r = hold_button(ui, short(mode), "", active, enabled);
                if r.on_hover_text(mode.label()).clicked() {
                    actions.push(Action::Command(GameCommand::SetHold(mode)));
                }
            }
        });
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let hint = "Shift / Ctrl, Z / X. A craft of parts' lit engines hold within their range: \
                        0 % is each one's minimum (a Merlin's 40 %), not off; shut them down to stop them.";
            ui.label(RichText::new("THR").monospace().small().color(theme::DIM)).on_hover_text(hint);
            let running = sim.ship().engine_running(&c);
            if let Some(t) = throttle_bar(ui, c.throttle, running, (height - 36.0).max(40.0)) {
                actions.push(Action::Command(GameCommand::SetThrottle(t)));
            }
            let lit = sim.ship().ship().is_some() && running;
            let caption = rules::throttle_caption(c.throttle, lit);
            ui.label(RichText::new(caption).monospace().small().color(theme::TEXT)).on_hover_text(hint);
        });
    });
}

/// A hold button: the mode on the left, a key (if any) dim on the right,
/// lit when active.
fn hold_button(ui: &mut egui::Ui, name: &str, key: &str, lit: bool, enabled: bool) -> egui::Response {
    let colour = if lit { theme::TEXT } else { theme::DIM };
    let atoms = (
        RichText::new(name).monospace().small().color(colour),
        egui::Atom::grow(),
        RichText::new(key).monospace().small().color(theme::DIM),
    );
    ui.add_enabled(enabled, egui::Button::new(atoms).selected(lit).min_size(vec2(HOLD_PX, 0.0)))
}

/// The short name on a hold button.
fn short(mode: HoldMode) -> &'static str {
    match mode {
        HoldMode::Stability => "STAB",
        HoldMode::Prograde => "PRO",
        HoldMode::Retrograde => "RETRO",
        HoldMode::Target => "TGT",
        HoldMode::AntiTarget => "ANTI",
        HoldMode::Maneuver => "MNV",
    }
}

/// The vertical throttle bar: filled from the bottom, ticks at quarters;
/// a click or a drag on it sets the throttle ([`rules::throttle_from_bar`]).
fn throttle_bar(ui: &mut egui::Ui, throttle: f64, running: bool, height: f32) -> Option<f64> {
    let (rect, response) = ui.allocate_exact_size(vec2(BAR_PX, height), egui::Sense::click_and_drag());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, Color32::from_black_alpha(140));
    let top = rect.bottom() - rect.height() * throttle.clamp(0.0, 1.0) as f32;
    let fill = egui::Rect::from_min_max(egui::pos2(rect.left(), top), rect.max);
    let colour = if running { theme::ACCENT } else { theme::DIM.gamma_multiply(0.6) };
    painter.rect_filled(fill, 3.0, colour);
    for k in 1..4 {
        let y = rect.bottom() - rect.height() * k as f32 / 4.0;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.left() + 4.0, y)],
            Stroke::new(1.0, theme::BORDER),
        );
    }
    painter.rect_stroke(rect, 3.0, Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
    let response = response.on_hover_text("Throttle: click or drag; Shift / Ctrl (Alt: fine), Z full, X cut");
    let y = response.interact_pointer_pos().map(|p| p.y);
    rules::throttle_from_bar(response.is_pointer_button_down_on(), y, rect.top(), rect.bottom())
}
