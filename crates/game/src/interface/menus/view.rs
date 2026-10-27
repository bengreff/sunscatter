//! The menus' Bevy systems: the keys that open and close them, and the
//! frame's drawing (the menu bar and both docks), with the menus' actions
//! applied afterwards. The rules are elsewhere: content in each menu's
//! module, places in `interface::layout`.

use super::{render, Dock, MenuAction, MenuId};
use crate::commands::{GameCommand, InputContext, Keys};
use crate::interface::layout::{self, InterfaceSettings, BAR_AREA, TIME_AREA};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

/// The menus' resources and systems: keys in the input stage, the snapshot
/// after the simulation and its predictions, drawing after the time panel.
pub struct MenusPlugin;

impl Plugin for MenusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<super::FlightData>()
            .add_systems(Update, keys.after(crate::commands::update_context).before(crate::state::read_controls))
            .add_systems(
                Update,
                super::data::update
                    .after(crate::navball::update)
                    .after(crate::rendezvous::update)
                    .after(crate::landing::update),
            )
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                draw.after(crate::hud::draw).before(crate::settings_ui::draw),
            );
    }
}

/// O, L, U and G open and close their menus (also while paused).
pub fn keys(keys: Res<ButtonInput<KeyCode>>, ctx: Res<InputContext>, mut iface: ResMut<InterfaceSettings>) {
    if !ctx.allows(Keys::Hud) {
        return;
    }
    for m in MenuId::ALL {
        if keys.just_pressed(m.key()) {
            iface.menus.toggle(m);
        }
    }
}

/// The menu bar and the docks, in the flight view only.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    data: Res<super::FlightData>,
    mut iface: ResMut<InterfaceSettings>,
    station: Res<crate::tracking::TrackingStation>,
    settings_ui: Res<crate::settings_ui::SettingsUi>,
    bench: Res<crate::bench::Bench>,
    sim: Res<crate::state::SimState>,
    mut nav: ResMut<crate::navball::Navball>,
    mut planner: ResMut<crate::planner::Planner>,
    mut commands: MessageWriter<GameCommand>,
) -> Result {
    if station.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut menus = iface.menus.clone();
    if let Some(m) = render::bar(ctx, &menus) {
        menus.toggle(m);
    }
    let rect = |id: &str| ctx.memory(|m| m.area_rect(egui::Id::new(id)));
    let overlay = if settings_ui.overlay || bench.running() { rect("perf") } else { None };
    // The navball cluster and the mini systems panel above it.
    let cluster = match (rect(crate::navball::AREA), rect(crate::interface::mini_panel::AREA)) {
        (Some(c), Some(m)) => Some(c.union(m)),
        (c, m) => c.or(m),
    };
    let screen = ctx.content_rect();
    let mut actions = Vec::new();
    for dock in [Dock::Left, Dock::Right] {
        let open = menus.in_dock(dock);
        if open.is_empty() {
            continue;
        }
        let top = layout::dock_top(dock, rect(TIME_AREA), rect(BAR_AREA), overlay);
        let place = layout::dock_place(dock, screen, top, cluster);
        let content: Vec<_> = open.iter().map(|m| (*m, m.sections(&data))).collect();
        for (m, input) in render::dock(ctx, dock, place, &content, &menus) {
            if input.close {
                menus.set_open(m, false);
            }
            for s in input.toggled {
                menus.toggle_section(m, s);
            }
            actions.extend(input.actions);
        }
    }
    if menus != iface.menus {
        iface.menus = menus;
    }
    for action in actions {
        match action {
            MenuAction::SetTarget(t) => nav.set_target(t),
            MenuAction::OpenPlanner => planner.open_for(sim.ship(), sim.clock),
            MenuAction::NextStage => {
                commands.write(GameCommand::NextStage);
            }
        }
    }
    Ok(())
}
