//! The help overlay (`?`, the menu bar's `?`, the pause menu's Help): every
//! key and every way in, generated from the tables the input itself uses,
//! so it cannot drift ([`sections`]): the bindings and held keys
//! (`commands::keys`), the task menus (`MenuId`), Esc's levels
//! (`pause::ESC_ORDER`) and the ways into each screen
//! (`interface::access::WAYS`). The key part is also shown in Settings →
//! Controls ([`key_grid`]).

use super::theme;
use crate::commands::keys::{self, pressed, Action, Group};
use crate::interface::access::{Key, WAYS};
use crate::interface::menus::MenuId;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{egui, EguiContexts};

#[derive(Resource, Default)]
pub struct Help {
    pub open: bool,
}

/// What the mouse does in the flight view (no table drives it: the camera
/// and the pick read the mouse directly).
const MOUSE: [(&str, &str); 4] = [
    ("drag", "orbit the camera"),
    ("scroll", "zoom"),
    ("click a part", "its card (a second click closes it)"),
    ("double-click a body", "its menu: distance, focus"),
];

/// One section of the help: a title and its (keys, what) rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub title: &'static str,
    pub rows: Vec<(String, String)>,
}

/// The help's sections, from the input's tables: the keys by group, the
/// menus, Esc's levels, and the ways into each screen.
pub fn sections() -> Vec<Section> {
    let group = |g: Group| -> Vec<(String, String)> {
        Action::ALL.iter().filter(|a| a.group() == g).map(|a| (keys::label(*a), a.about().to_string())).collect()
    };
    let mut flight: Vec<(String, String)> = keys::held_rows().into_iter().map(|(k, w)| (k, w.to_string())).collect();
    flight.extend(group(Group::Flight));
    let menus =
        MenuId::BAR.iter().map(|m| (m.key_label().to_string(), format!("{}: {}", m.title(), m.about()))).collect();
    let mut camera = group(Group::Camera);
    camera.extend(MOUSE.iter().map(|(k, w)| (k.to_string(), w.to_string())));
    let esc = super::pause::ESC_ORDER.iter().map(|a| a.about()).collect::<Vec<_>>().join("; ");
    let ways = WAYS
        .iter()
        .map(|w| {
            let mut how = vec![];
            if w.key != Key::None {
                how.push(w.key.label());
            }
            if !w.clicks.is_empty() {
                how.push(w.clicks.to_string());
            }
            if !w.mcp.is_empty() {
                how.push(format!("agent: {}", w.mcp));
            }
            (w.what.to_string(), how.join(" · "))
        })
        .collect();
    vec![
        Section { title: "Flight", rows: flight },
        Section { title: "Menus (or click them on the menu bar)", rows: menus },
        Section { title: "Time", rows: group(Group::Time) },
        Section { title: "Camera", rows: camera },
        Section { title: "Screens", rows: group(Group::Screens) },
        Section { title: "Esc backs out one level", rows: vec![(Key::Esc.label(), format!("innermost first: {esc}"))] },
        Section { title: "Every way in", rows: ways },
    ]
}

/// The sections as grids, the keys only or with the ways in (Settings →
/// Controls shows the keys).
fn grid(ui: &mut egui::Ui, sections: &[Section]) {
    for s in sections {
        ui.label(egui::RichText::new(s.title).color(theme::ACCENT));
        egui::Grid::new(("help", s.title)).num_columns(2).spacing([16.0, 2.0]).min_col_width(150.0).show(ui, |ui| {
            for (k, what) in &s.rows {
                ui.label(egui::RichText::new(k).monospace());
                ui.add(egui::Label::new(egui::RichText::new(what).color(theme::DIM)).wrap());
                ui.end_row();
            }
        });
        ui.add_space(4.0);
    }
}

/// The key list as a grid (the settings screen's Controls tab).
pub fn key_grid(ui: &mut egui::Ui) {
    let all = sections();
    grid(ui, &all[..all.len() - 1]);
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
    let height = ctx.content_rect().height() * 0.8;
    let mut open = true;
    egui::Window::new(format!("Keys and every way in ({})", keys::label(Action::Help)))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(760.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(theme::panel_frame().fill(theme::SOLID))
        .show(ctx, |ui| {
            ui.set_max_height(height);
            egui::ScrollArea::vertical().max_height(height).show(ui, |ui| grid(ui, &sections()));
        });
    help.open = open;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::keys::BINDINGS;

    fn all_rows() -> Vec<(String, String)> {
        sections().into_iter().flat_map(|s| s.rows).collect()
    }

    /// Every binding is in the help with its own label, every action once;
    /// every menu with its key; every Esc level; every way in.
    #[test]
    fn the_help_lists_every_key_and_way_in() {
        let rows = all_rows();
        let has = |k: &str| rows.iter().any(|(key, _)| key == k);
        for (_, a) in BINDINGS {
            assert!(Action::ALL.contains(a), "{a:?} is not in Action::ALL");
            assert!(has(&keys::label(*a)), "{a:?} ({}) is not in the help", keys::label(*a));
        }
        for a in Action::ALL {
            assert!(!keys::labels(a, true).is_empty() && !keys::labels(a, false).is_empty(), "{a:?} has no key");
            assert_eq!(Action::ALL.iter().filter(|b| **b == a).count(), 1, "{a:?} twice");
        }
        for m in MenuId::BAR {
            assert!(rows.iter().any(|(k, w)| k == m.key_label() && w.starts_with(m.title())), "{m:?}");
        }
        let esc = &rows.iter().find(|(k, _)| k == "Esc").expect("Esc").1;
        for a in crate::interface::pause::ESC_ORDER {
            assert!(esc.contains(a.about()), "{a:?}");
        }
        for w in WAYS {
            assert!(rows.iter().any(|(k, _)| k == w.what), "{}", w.what);
        }
    }

    /// The labels follow the bindings, on a Mac and elsewhere (table).
    #[test]
    fn labels_come_from_the_bindings() {
        // (action, on a Mac, elsewhere)
        let table = [
            (Action::QuickSave, "Cmd+S", "Ctrl+Alt+S"),
            (Action::Help, "?", "?"),
            (Action::SystemsScreen, "Tab", "Tab"),
            (Action::TrackingStation, "M", "M"),
            (Action::Warp1x, "/", "/"),
            (Action::FocusShip, "`", "`"),
            (Action::NextStage, "Space", "Space"),
        ];
        for (a, mac, other) in table {
            assert_eq!((keys::labels(a, true).as_str(), keys::labels(a, false).as_str()), (mac, other), "{a:?}");
        }
        assert_eq!(keys::held_rows()[1].0, "W S / A D / Q E");
        assert_eq!(keys::held_rows()[2].0, "J L / I K / N H");
    }
}
