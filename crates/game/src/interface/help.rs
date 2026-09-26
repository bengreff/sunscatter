//! The key help overlay (H), replacing the old Controls window. The same
//! list is shown in Settings → Controls.

use super::theme;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts};

#[derive(Resource, Default)]
pub struct Help {
    pub open: bool,
}

/// (keys, what they do), grouped.
pub const KEYS: &[(&str, &[(&str, &str)])] = &[
    (
        "Flight",
        &[
            ("Shift / Ctrl", "throttle up / down"),
            ("Z / X", "full / cut throttle"),
            ("W S / A D / Q E", "pitch / yaw / roll"),
            ("T", "SAS"),
            ("P", "deploy parachute"),
            ("R", "reset to the pad"),
        ],
    ),
    ("Time", &[(". / ,", "warp up / down"), ("/", "warp 1x (rails warp needs throttle 0)"), ("Esc", "pause menu")]),
    (
        "Camera",
        &[
            ("drag", "orbit the camera"),
            ("scroll", "zoom"),
            ("F", "focus the nearest body"),
            ("`", "focus the ship"),
            ("double-click a body", "its menu"),
            ("Tab", "plotting frame"),
        ],
    ),
    (
        "Game",
        &[
            ("[ / ]", "previous / next vessel"),
            ("F7", "tracking station"),
            ("F5 / F9", "quicksave / quickload"),
            ("F6", "saves"),
            ("F3", "settings"),
            ("F4", "performance overlay"),
            ("F2", "spawn 10 test ships"),
            ("H", "this help"),
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
    if keys.just_pressed(KeyCode::KeyH) && !egui.wants_any_keyboard_input() {
        help.open = !help.open;
    }
}

pub fn draw(mut contexts: EguiContexts, mut help: ResMut<Help>) -> Result {
    if !help.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut open = true;
    egui::Window::new("Keys (H)")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, key_grid);
    help.open = open;
    Ok(())
}
