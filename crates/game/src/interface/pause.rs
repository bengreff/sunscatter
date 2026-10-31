//! The Esc pause menu. Esc pauses (the clock stops, flight keys are ignored,
//! the camera still works) and opens a centred menu: Resume, Settings, Save,
//! Load, New flight (a choice of craft: the Falcon 9 Crew, the test craft as
//! the debug craft), Revert to launch, Debug mode (and, in it, Spawn 10
//! test ships), Tracking station, Help, the performance overlay, Quit to
//! desktop. Revert and Quit ask first.
//! Esc again steps back (confirmation → menu → resume). These were F-keys
//! before D111 (no function keys: they need fn on a Mac).
//!
//! Esc always backs out one level ([`on_escape`], the one Esc stack, in
//! [`ESC_ORDER`]): a window over the game first (help, settings, saves, a
//! body's menu), then the planner, the systems screen's isolated part (back
//! to the layer view as it was), the systems screen, the tracking station,
//! and only then the pause menu.
//!
//! "Revert to launch" is the one way back to the pad (D114): it loads the
//! launch save written when the active vessel (or the vessel it split
//! from) was placed on the pad, the whole game as it was then
//! (`saves::launch_of`). A vessel without one (spawned in orbit, from an
//! older save) is put on the pad as a new craft of its kind instead.

use super::theme;
use crate::commands::keys;
use crate::commands::GameCommand;
use crate::saves::SaveUi;
use crate::settings_ui::SettingsUi;
use crate::state::SimState;
use crate::tracking::TrackingStation;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

/// Which page of the pause menu is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Main,
    /// The choice of craft for a new flight.
    NewFlight,
    ConfirmRevert,
    ConfirmQuit,
}

/// A choice on the main page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Resume,
    Settings,
    Saves,
    Page(Page),
    Debug,
    SpawnTestShips,
    TrackingStation,
    Help,
    Overlay,
}

#[derive(Resource, Default)]
pub struct PauseMenu {
    /// Paused, with the menu open.
    pub open: bool,
    pub page: Page,
}

/// What is open that Esc can back out of, besides the pause menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenWindows {
    pub help: bool,
    pub settings: bool,
    pub saves: bool,
    /// A body's menu (a double-click on it).
    pub body_menu: bool,
    pub planner: bool,
    /// The systems screen (D107), and a part isolated in it.
    pub systems: bool,
    pub isolated: bool,
    pub station: bool,
}

/// What Esc does, given what is open: the innermost thing first, one level
/// at a time (the order of [`ESC_ORDER`]).
pub fn on_escape(open: OpenWindows, menu: &PauseMenu) -> EscAction {
    let game = !menu.open;
    if open.help {
        EscAction::CloseHelp
    } else if open.settings {
        EscAction::CloseSettings
    } else if open.saves {
        EscAction::CloseSaves
    } else if open.body_menu && game {
        EscAction::CloseBodyMenu
    } else if open.planner && game {
        EscAction::ClosePlanner
    } else if open.systems && open.isolated && game {
        EscAction::SystemsBack
    } else if open.systems && game {
        EscAction::CloseSystems
    } else if open.station && game {
        EscAction::CloseStation
    } else if game {
        EscAction::Pause
    } else if menu.page != Page::Main {
        EscAction::BackToMenu
    } else {
        EscAction::Resume
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscAction {
    CloseHelp,
    CloseSettings,
    CloseSaves,
    CloseBodyMenu,
    ClosePlanner,
    /// The systems screen's isolated part: back to the layer view as it was.
    SystemsBack,
    CloseSystems,
    CloseStation,
    Pause,
    BackToMenu,
    Resume,
}

/// Esc's levels, innermost first (the help lists them).
pub const ESC_ORDER: [EscAction; 11] = [
    EscAction::CloseHelp,
    EscAction::CloseSettings,
    EscAction::CloseSaves,
    EscAction::CloseBodyMenu,
    EscAction::ClosePlanner,
    EscAction::SystemsBack,
    EscAction::CloseSystems,
    EscAction::CloseStation,
    EscAction::Pause,
    EscAction::BackToMenu,
    EscAction::Resume,
];

impl EscAction {
    /// What it does, as the help says it.
    pub fn about(self) -> &'static str {
        match self {
            EscAction::CloseHelp => "closes the help",
            EscAction::CloseSettings => "closes settings",
            EscAction::CloseSaves => "closes the saves",
            EscAction::CloseBodyMenu => "closes a body's menu",
            EscAction::ClosePlanner => "closes the burn planner",
            EscAction::SystemsBack => "puts back the systems screen's layer view (from an isolated part)",
            EscAction::CloseSystems => "closes the systems screen",
            EscAction::CloseStation => "closes the tracking station",
            EscAction::Pause => "pauses: the pause menu",
            EscAction::BackToMenu => "back to the pause menu's first page",
            EscAction::Resume => "resumes",
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<PauseMenu>,
    mut help: ResMut<super::help::Help>,
    mut settings: ResMut<SettingsUi>,
    mut saves: ResMut<SaveUi>,
    mut planner: ResMut<crate::planner::Planner>,
    mut systems: ResMut<crate::systems_screen::SystemsScreen>,
    (mut station, mut ui): (ResMut<TrackingStation>, ResMut<crate::hud::UiState>),
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let open = OpenWindows {
        help: help.open,
        settings: settings.open,
        saves: saves.open,
        body_menu: ui.menu.is_some(),
        planner: planner.open,
        systems: systems.open,
        isolated: systems.isolated(),
        station: station.open,
    };
    match on_escape(open, &menu) {
        EscAction::CloseHelp => help.open = false,
        EscAction::CloseSettings => settings.open = false,
        EscAction::CloseSaves => saves.open = false,
        EscAction::CloseBodyMenu => ui.menu = None,
        EscAction::ClosePlanner => planner.open = false,
        EscAction::SystemsBack => systems.back(),
        EscAction::CloseSystems => systems.close(),
        EscAction::CloseStation => station.open = false,
        EscAction::Pause => *menu = PauseMenu { open: true, page: Page::Main },
        EscAction::BackToMenu => menu.page = Page::Main,
        EscAction::Resume => menu.open = false,
    }
}

/// The main page's buttons; the choice clicked.
fn main_page(ui: &mut egui::Ui, debug: bool, overlay: bool) -> Option<Choice> {
    let mut choice = None;
    let mut button = |ui: &mut egui::Ui, text: &str, what: Choice| {
        if ui.add_sized([220.0, 30.0], egui::Button::new(text)).clicked() {
            choice = Some(what);
        }
    };
    ui.label(egui::RichText::new("Sunscatter").size(22.0).color(theme::TEXT));
    ui.add_space(10.0);
    button(ui, "Resume", Choice::Resume);
    button(ui, "Settings", Choice::Settings);
    button(ui, &format!("Saves  {}", keys::label(keys::Action::QuickSave)), Choice::Saves);
    button(ui, "New flight…", Choice::Page(Page::NewFlight));
    button(ui, "Revert to launch…", Choice::Page(Page::ConfirmRevert));
    button(ui, if debug { "Debug mode: ON" } else { "Debug mode: off" }, Choice::Debug);
    if debug {
        button(ui, "Spawn 10 test ships", Choice::SpawnTestShips);
    }
    button(ui, &format!("Tracking station  {}", keys::label(keys::Action::TrackingStation)), Choice::TrackingStation);
    button(ui, &format!("Help: keys and ways in  {}", keys::label(keys::Action::Help)), Choice::Help);
    button(ui, if overlay { "Performance overlay: ON" } else { "Performance overlay: off" }, Choice::Overlay);
    button(ui, "Quit to desktop", Choice::Page(Page::ConfirmQuit));
    choice
}

/// A confirmation page: `text`, then `yes` and Cancel; whether `yes` was
/// clicked (Cancel goes back to the main page).
fn confirm(ui: &mut egui::Ui, menu: &mut PauseMenu, text: &str, yes: &str) -> bool {
    let button = |ui: &mut egui::Ui, text: &str| ui.add_sized([220.0, 30.0], egui::Button::new(text)).clicked();
    ui.label(text);
    ui.add_space(8.0);
    let clicked = button(ui, yes);
    if button(ui, "Cancel") {
        menu.page = Page::Main;
    }
    clicked
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut menu: ResMut<PauseMenu>,
    mut windows: (ResMut<SettingsUi>, ResMut<SaveUi>, ResMut<super::help::Help>),
    mut station: ResMut<TrackingStation>,
    (persist, time, mut toasts): (Res<crate::persist::Persist>, Res<Time>, ResMut<super::toasts::Toasts>),
    sim: Res<SimState>,
    mut commands: MessageWriter<GameCommand>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    if !menu.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    // Under the menu bar, not over it.
    let below = ctx.memory(|m| m.area_rect(egui::Id::new(super::layout::BAR_AREA))).map_or(12.0, |r| r.max.y + 6.0);
    egui::Area::new("paused_banner".into()).anchor(egui::Align2::CENTER_TOP, [0.0, below]).show(ctx, |ui| {
        ui.label(egui::RichText::new("PAUSED").monospace().size(20.0).color(theme::ACCENT));
    });
    let (settings, saves, help) = (&mut windows.0, &mut windows.1, &mut windows.2);
    let button = |ui: &mut egui::Ui, text: &str| ui.add_sized([220.0, 30.0], egui::Button::new(text)).clicked();
    egui::Window::new("pause_menu")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(theme::panel_frame().inner_margin(egui::Margin::same(18)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| match menu.page {
                Page::Main => {
                    let debug = sim.ship().debug();
                    match main_page(ui, debug, settings.overlay) {
                        Some(Choice::Resume) => menu.open = false,
                        Some(Choice::Settings) => settings.open = true,
                        Some(Choice::Saves) => saves.show_list(),
                        Some(Choice::Page(p)) => menu.page = p,
                        Some(Choice::Debug) => {
                            commands.write(GameCommand::SetDebug(!debug));
                        }
                        Some(Choice::SpawnTestShips) => {
                            commands.write(GameCommand::SpawnTestShips(10));
                        }
                        Some(Choice::TrackingStation) => {
                            station.open = true;
                            menu.open = false;
                        }
                        Some(Choice::Help) => help.open = true,
                        Some(Choice::Overlay) => settings.overlay = !settings.overlay,
                        None => {}
                    }
                }
                Page::NewFlight => {
                    ui.label("A new flight from the pad: the active vessel\nis replaced. Which craft?");
                    ui.add_space(8.0);
                    for (id, name) in crate::state::craft_choices(crate::state::craft_exists) {
                        if button(ui, name) {
                            commands.write(GameCommand::NewFlight(id.to_string()));
                            menu.open = false;
                        }
                    }
                    if button(ui, "Cancel") {
                        menu.page = Page::Main;
                    }
                }
                Page::ConfirmRevert => {
                    let launch = crate::saves::launch_of(&sim, &persist.dirs.saves);
                    let text = if launch.is_some() {
                        "Revert to launch? The whole game goes back to when\nthis vessel was put on the pad. Everything since is lost."
                    } else {
                        "This vessel has no launch save (it never sat on the pad\nin this game). Put a new one of its kind on the pad?"
                    };
                    if confirm(ui, &mut menu, text, "Revert") {
                        match launch.map(|path| crate::saves::read(&sim, &path)) {
                            Some(Ok(save)) => {
                                commands.write(GameCommand::Restore(Box::new(save)));
                            }
                            Some(Err(e)) => toasts.push(time.elapsed_secs_f64(), format!("Revert failed: {e}"), true),
                            None => {
                                commands.write(GameCommand::Reset);
                            }
                        }
                        menu.open = false;
                    }
                }
                Page::ConfirmQuit => {
                    if confirm(ui, &mut menu, "Quit to desktop? Unsaved progress is lost.", "Quit") {
                        exit.write(AppExit::Success);
                    }
                }
            });
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_closes_the_innermost_thing_first() {
        let closed = PauseMenu::default();
        let main = PauseMenu { open: true, page: Page::Main };
        let confirm = PauseMenu { open: true, page: Page::ConfirmQuit };
        let w = |help, settings, saves, planner| OpenWindows { help, settings, saves, planner, ..Default::default() };
        let none = OpenWindows::default();
        let systems = OpenWindows { systems: true, ..none };
        let isolated = OpenWindows { isolated: true, ..systems };
        let station = OpenWindows { station: true, ..none };
        let body = OpenWindows { body_menu: true, ..none };
        // (open windows, menu, expected)
        let cases = [
            (w(true, true, true, true), &main, EscAction::CloseHelp),
            (w(false, true, true, false), &main, EscAction::CloseSettings),
            (w(false, true, false, false), &closed, EscAction::CloseSettings),
            (w(false, false, true, true), &closed, EscAction::CloseSaves),
            (w(false, false, false, true), &closed, EscAction::ClosePlanner),
            (w(false, false, false, true), &main, EscAction::Resume),
            (systems, &closed, EscAction::CloseSystems),
            (OpenWindows { planner: true, ..systems }, &closed, EscAction::ClosePlanner),
            // An isolated part goes back to the layer view, then the screen closes.
            (isolated, &closed, EscAction::SystemsBack),
            (isolated, &main, EscAction::Resume),
            (OpenWindows { help: true, ..isolated }, &closed, EscAction::CloseHelp),
            // The tracking station closes, rather than pausing over it.
            (station, &closed, EscAction::CloseStation),
            (OpenWindows { systems: true, ..station }, &closed, EscAction::CloseSystems),
            (station, &confirm, EscAction::BackToMenu),
            // A body's menu closes first, before the planner.
            (OpenWindows { planner: true, ..body }, &closed, EscAction::CloseBodyMenu),
            (body, &main, EscAction::Resume),
            (w(false, false, false, false), &closed, EscAction::Pause),
            (w(false, false, false, false), &confirm, EscAction::BackToMenu),
            (w(false, false, false, false), &main, EscAction::Resume),
        ];
        for (open, menu, expected) in cases {
            assert_eq!(on_escape(open, menu), expected, "{open:?}, {:?}", menu.page);
        }
    }

    /// Every Esc level is in the order the help lists, once.
    #[test]
    fn the_esc_order_has_every_level_once() {
        for (i, a) in ESC_ORDER.iter().enumerate() {
            assert!(!ESC_ORDER[i + 1..].contains(a), "{a:?} twice");
            assert!(!a.about().is_empty());
        }
    }
}
