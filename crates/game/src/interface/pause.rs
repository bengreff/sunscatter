//! The Esc pause menu. Esc pauses (the clock stops, flight keys are ignored,
//! the camera still works) and opens a centred menu: Resume, Settings, Save,
//! Load, New flight (a choice of craft: the Falcon 9 Crew, the test craft as
//! the debug craft), Revert flight to launch, Reset to the pad, Debug mode
//! (and, in it, Spawn 10 test ships), Tracking station, Help, the
//! performance overlay, Quit to desktop. Revert, Reset and Quit ask first.
//! Esc again steps back (confirmation → menu → resume). These were F-keys
//! before D111 (no function keys: they need fn on a Mac).
//!
//! "Revert flight to launch" restores the whole game as it was the moment
//! the active vessel was last about to lift off (a snapshot taken then).

use super::theme;
use crate::commands::GameCommand;
use crate::saves::SaveUi;
use crate::settings_ui::SettingsUi;
use crate::state::SimState;
use crate::tracking::TrackingStation;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sim::save::SaveGame;
use sim::vessel::Phase;

/// Which page of the pause menu is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Main,
    /// The choice of craft for a new flight.
    NewFlight,
    ConfirmRevert,
    ConfirmReset,
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

/// The game as it was when the active vessel was last about to lift off.
#[derive(Resource, Default)]
pub struct LaunchSnapshot(pub Option<SaveGame>);

/// Windows Esc can close, besides the pause menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpenWindows {
    pub help: bool,
    pub settings: bool,
    pub saves: bool,
    pub planner: bool,
    /// The systems screen (D107).
    pub systems: bool,
}

/// What Esc does, given what is open: the innermost thing closes first.
pub fn on_escape(open: OpenWindows, menu: &PauseMenu) -> EscAction {
    if open.help {
        EscAction::CloseHelp
    } else if open.settings {
        EscAction::CloseSettings
    } else if open.saves {
        EscAction::CloseSaves
    } else if open.planner && !menu.open {
        EscAction::ClosePlanner
    } else if open.systems && !menu.open {
        EscAction::CloseSystems
    } else if !menu.open {
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
    ClosePlanner,
    CloseSystems,
    Pause,
    BackToMenu,
    Resume,
}

pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<PauseMenu>,
    mut help: ResMut<super::help::Help>,
    mut settings: ResMut<SettingsUi>,
    mut saves: ResMut<SaveUi>,
    mut planner: ResMut<crate::planner::Planner>,
    mut systems: ResMut<crate::systems_screen::SystemsScreen>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    let open = OpenWindows {
        help: help.open,
        settings: settings.open,
        saves: saves.open,
        planner: planner.open,
        systems: systems.open,
    };
    match on_escape(open, &menu) {
        EscAction::CloseHelp => help.open = false,
        EscAction::CloseSettings => settings.open = false,
        EscAction::CloseSaves => saves.open = false,
        EscAction::ClosePlanner => planner.open = false,
        EscAction::CloseSystems => systems.open = false,
        EscAction::Pause => *menu = PauseMenu { open: true, page: Page::Main },
        EscAction::BackToMenu => menu.page = Page::Main,
        EscAction::Resume => menu.open = false,
    }
}

/// Keeps the launch snapshot: while the active vessel sits on the ground
/// with the throttle open (it lifts off as soon as thrust beats gravity), the
/// game is captured at most once per simulated second, so a revert returns
/// to within a second before liftoff.
pub fn track_launch(sim: Res<SimState>, mut snapshot: ResMut<LaunchSnapshot>) {
    if matches!(sim.ship().phase, Phase::Landed { .. }) && sim.controls().throttle > 0.0 {
        let fresh = snapshot.0.as_ref().is_none_or(|s| {
            let age = sim.clock.seconds_since(s.clock);
            !(0.0..1.0).contains(&age)
        });
        if fresh {
            snapshot.0 = Some(crate::saves::capture(&sim));
        }
    }
}

/// The main page's buttons; the choice clicked.
fn main_page(ui: &mut egui::Ui, debug: bool, can_revert: bool, overlay: bool) -> Option<Choice> {
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
    button(ui, "Save", Choice::Saves);
    button(ui, "Load", Choice::Saves);
    button(ui, "New flight…", Choice::Page(Page::NewFlight));
    ui.add_enabled_ui(can_revert, |ui| button(ui, "Revert flight to launch", Choice::Page(Page::ConfirmRevert)));
    button(ui, "Reset to the pad…", Choice::Page(Page::ConfirmReset));
    button(ui, if debug { "Debug mode: ON" } else { "Debug mode: off" }, Choice::Debug);
    if debug {
        button(ui, "Spawn 10 test ships", Choice::SpawnTestShips);
    }
    button(ui, "Tracking station  M", Choice::TrackingStation);
    button(ui, "Help: the keys  ?", Choice::Help);
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
    snapshot: Res<LaunchSnapshot>,
    sim: Res<SimState>,
    mut commands: MessageWriter<GameCommand>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    if !menu.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    egui::Area::new("paused_banner".into()).anchor(egui::Align2::CENTER_TOP, [0.0, 12.0]).show(ctx, |ui| {
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
                    match main_page(ui, debug, snapshot.0.is_some(), settings.overlay) {
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
                    if confirm(
                        ui,
                        &mut menu,
                        "Revert to the moment before liftoff?\nEverything since is lost.",
                        "Revert",
                    ) {
                        if let Some(save) = &snapshot.0 {
                            commands.write(GameCommand::Restore(Box::new(save.clone())));
                        }
                        menu.open = false;
                    }
                }
                Page::ConfirmReset => {
                    let text = "Reset the active vessel to the pad?\nIts flight is lost (New flight picks the craft).";
                    if confirm(ui, &mut menu, text, "Reset") {
                        commands.write(GameCommand::Reset);
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
        let w = |help, settings, saves, planner| OpenWindows { help, settings, saves, planner, systems: false };
        let systems = OpenWindows { systems: true, ..w(false, false, false, false) };
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
            (w(false, false, false, false), &closed, EscAction::Pause),
            (w(false, false, false, false), &confirm, EscAction::BackToMenu),
            (w(false, false, false, false), &main, EscAction::Resume),
        ];
        for (open, menu, expected) in cases {
            assert_eq!(on_escape(open, menu), expected, "{open:?}, {:?}", menu.page);
        }
    }
}
