//! The key help overlay (`?`), replacing the old Controls window. The same
//! list is shown in Settings → Controls.

use super::theme;
use crate::commands::keys::{pressed, Action};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts};

#[derive(Resource, Default)]
pub struct Help {
    pub open: bool,
}

/// (keys, what they do), grouped: every binding of
/// `commands::keys::BINDINGS`, the held flight keys and the menus' keys
/// (tests in `commands::keys` and `menus`). Hold modes and a craft's
/// functions are clicked (on the navball and the action bar).
pub const KEYS: &[(&str, &[(&str, &str)])] = &[
    (
        "Flight",
        &[
            ("Shift / Ctrl", "throttle up / down (hold Alt: fine)"),
            ("Z / X", "full / cut throttle"),
            ("W S / A D / Q E", "pitch / yaw / roll"),
            ("I K / J L / H N", "translate up / down, left / right, forward / back (RCS on)"),
            ("T", "SAS on / off (hold modes: click them on the navball)"),
            ("R", "RCS on / off"),
            ("P", "parachutes: drogue now, mains low"),
            ("Space", "activate the next stage (a craft of parts: its sequence)"),
        ],
    ),
    (
        "Menus",
        &[
            ("1", "Orbital: the orbit now, the target, the ship's clock"),
            ("2", "Landing / Reentry: impact, braking, entry, heat, parachute (shows the impact marker)"),
            ("3", "Maneuver: the planned burns, the burn planner"),
            ("6", "Comms: the link home, a probe's control delay"),
        ],
    ),
    (
        "Time",
        &[
            (". / ,", "warp up / down; , at 1x stops time, . runs it again"),
            ("/", "warp 1x (rails warp needs no thrust)"),
            ("Esc", "pause menu: settings, saves, reset to the pad, help, performance overlay"),
        ],
    ),
    (
        "Camera",
        &[
            ("drag", "orbit the camera"),
            ("scroll", "zoom"),
            ("F", "focus the nearest body"),
            ("`", "focus the ship"),
            ("double-click a body", "its menu (the heaviest under the cursor)"),
        ],
    ),
    (
        "Game",
        &[
            ("[ / ]", "previous / next vessel"),
            ("M", "tracking station"),
            ("Tab", "systems screen: the craft by layer, its commands, stages and functions"),
            ("Cmd+S / Cmd+L", "quicksave / quickload (Ctrl+Alt+S / Ctrl+Alt+L on Windows and Linux)"),
            ("?", "this help"),
        ],
    ),
];

/// The key list as a grid (also used by the settings screen).
pub fn key_grid(ui: &mut egui::Ui) {
    for (group, keys) in KEYS {
        ui.label(egui::RichText::new(*group).color(theme::ACCENT));
        egui::Grid::new(("keys", *group)).num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
            for (k, what) in *keys {
                ui.label(egui::RichText::new(*k).monospace());
                ui.label(egui::RichText::new(*what).color(theme::DIM));
                ui.end_row();
            }
        });
        ui.add_space(4.0);
    }
}

pub fn keys(keys: Res<ButtonInput<KeyCode>>, egui: Res<EguiWantsInput>, mut help: ResMut<Help>) {
    if pressed(&keys, Action::Help) && !egui.wants_any_keyboard_input() {
        help.open = !help.open;
    }
}

pub fn draw(mut contexts: EguiContexts, mut help: ResMut<Help>) -> Result {
    if !help.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut open = true;
    egui::Window::new("Keys (?)")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, key_grid);
    help.open = open;
    Ok(())
}
