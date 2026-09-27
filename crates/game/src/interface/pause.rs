//! The Esc pause menu. Esc pauses (the clock stops, flight keys are ignored,
//! the camera still works) and opens a centred menu: Resume, Settings, Save,
//! Load, Revert flight to launch, Tracking station, Quit to desktop. Revert
//! and Quit ask first. Esc again steps back (confirmation → menu → resume).
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
    ConfirmRevert,
    ConfirmQuit,
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

/// What Esc does, given what is open: the innermost thing closes first.
pub fn on_escape(help_open: bool, settings_open: bool, menu: &PauseMenu) -> EscAction {
    if help_open {
        EscAction::CloseHelp
    } else if settings_open {
        EscAction::CloseSettings
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
    Pause,
    BackToMenu,
    Resume,
}

pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<PauseMenu>,
    mut help: ResMut<super::help::Help>,
    mut settings: ResMut<SettingsUi>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    match on_escape(help.open, settings.open, &menu) {
        EscAction::CloseHelp => help.open = false,
        EscAction::CloseSettings => settings.open = false,
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
    if matches!(sim.ship().phase, Phase::Landed { .. }) && sim.controls.throttle > 0.0 {
        let fresh = snapshot.0.as_ref().is_none_or(|s| {
            let age = sim.clock.seconds_since(s.clock);
            !(0.0..1.0).contains(&age)
        });
        if fresh {
            snapshot.0 =
                Some(SaveGame::capture(&sim.world, sim.clock, &sim.fleet, sim.vessel_ids, sim.active, sim.controls));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut menu: ResMut<PauseMenu>,
    mut settings: ResMut<SettingsUi>,
    mut saves: ResMut<SaveUi>,
    mut station: ResMut<TrackingStation>,
    snapshot: Res<LaunchSnapshot>,
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
                    ui.label(egui::RichText::new("Sunscatter").size(22.0).color(theme::TEXT));
                    ui.add_space(10.0);
                    if button(ui, "Resume") {
                        menu.open = false;
                    }
                    if button(ui, "Settings") {
                        settings.open = true;
                    }
                    let save = button(ui, "Save");
                    let load = button(ui, "Load");
                    if save || load {
                        saves.show_list();
                    }
                    let revert = ui.add_enabled_ui(snapshot.0.is_some(), |ui| button(ui, "Revert flight to launch"));
                    if revert.inner {
                        menu.page = Page::ConfirmRevert;
                    }
                    if button(ui, "Tracking station") {
                        station.open = true;
                        menu.open = false;
                    }
                    if button(ui, "Quit to desktop") {
                        menu.page = Page::ConfirmQuit;
                    }
                }
                Page::ConfirmRevert => {
                    ui.label("Revert to the moment before liftoff?\nEverything since is lost.");
                    ui.add_space(8.0);
                    if button(ui, "Revert") {
                        if let Some(save) = &snapshot.0 {
                            commands.write(GameCommand::Restore(Box::new(save.clone())));
                        }
                        menu.open = false;
                    }
                    if button(ui, "Cancel") {
                        menu.page = Page::Main;
                    }
                }
                Page::ConfirmQuit => {
                    ui.label("Quit to desktop? Unsaved progress is lost.");
                    ui.add_space(8.0);
                    if button(ui, "Quit") {
                        exit.write(AppExit::Success);
                    }
                    if button(ui, "Cancel") {
                        menu.page = Page::Main;
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
        // (help open, settings open, menu, expected)
        let cases = [
            (true, true, &main, EscAction::CloseHelp),
            (false, true, &main, EscAction::CloseSettings),
            (false, true, &closed, EscAction::CloseSettings),
            (false, false, &closed, EscAction::Pause),
            (false, false, &confirm, EscAction::BackToMenu),
            (false, false, &main, EscAction::Resume),
        ];
        for (help, settings, menu, expected) in cases {
            assert_eq!(on_escape(help, settings, menu), expected, "help {help}, settings {settings}, {:?}", menu.page);
        }
    }
}
