//! The menus' Bevy systems: the keys that open and close them, and the
//! frame's drawing (the menu bar and both docks), with the menus' actions
//! applied afterwards. The rules are elsewhere: content in each menu's
//! module, places in `interface::layout`.

use super::{render, MenuAction, MenuId};
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
            .add_systems(
                Update,
                keys.after(crate::commands::update_context).before(crate::commands::keys::read_controls),
            )
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

/// The number keys open and close their menus (also while paused; not
/// under a saves' chord).
pub fn keys(keys: Res<ButtonInput<KeyCode>>, ctx: Res<InputContext>, mut iface: ResMut<InterfaceSettings>) {
    if !ctx.allows(Keys::Hud) || crate::commands::keys::Held::of(&keys).chord() {
        return;
    }
    for m in MenuId::BAR {
        if m.key().is_some_and(|k| keys.just_pressed(k)) {
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
    mut systems: ResMut<crate::systems_screen::SystemsScreen>,
    sim: Res<crate::state::SimState>,
    mut nav: ResMut<crate::navball::Navball>,
    mut planner: ResMut<crate::planner::Planner>,
    mut cmds: MessageWriter<GameCommand>,
    real: Res<Time<Real>>,
    mut steady: Local<std::collections::HashMap<MenuId, super::Steady>>,
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
    // The navball cluster (the action bar is at the top right).
    let cluster = rect(crate::navball::AREA);
    let screen = ctx.content_rect();
    let mut actions = Vec::new();
    let open = menus.in_order(sim.controls().rcs.on);
    if !open.is_empty() {
        let top = layout::dock_top(rect(TIME_AREA), rect(BAR_AREA));
        let corner = rect(crate::interface::toasts::AREA);
        let place = layout::dock_place(screen, top, cluster, corner);
        // Nothing flickers (`interface::steady`): each menu as held.
        let now = real.elapsed_secs_f64();
        let content: Vec<_> =
            open.iter().map(|m| (*m, steady.entry(*m).or_default().update(m.sections(&data), now).to_vec())).collect();
        for (m, input) in render::dock(ctx, place, &content) {
            if input.close && m == MenuId::Rcs {
                actions.push(MenuAction::SetRcs(sim::systems::rcs::RcsMode { on: false, ..sim.controls().rcs }));
            } else if input.close {
                menus.set_open(m, false);
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
            MenuAction::OpenSystems(layer, part) => systems.open_at(layer, part),
            MenuAction::Ship(c) => {
                cmds.write(GameCommand::Ship { vessel: sim.ship().id(), commands: vec![c] });
            }
            MenuAction::SetRcs(mode) => {
                cmds.write(GameCommand::SetRcs(mode));
            }
        }
    }
    Ok(())
}
