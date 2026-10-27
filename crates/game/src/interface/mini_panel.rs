//! The mini systems panel (D107): in flight, the active craft's pinned
//! functions, each run by a click or its key, and the way into the systems
//! screen (F8). A function keeps the key it asks for unless a game key or
//! an earlier pinned function has it; then it gets a free one
//! (`commands::keys::bind_keys`), shown on its button. A small fixed dock
//! just above the navball cluster (D080: designated places; the docks keep
//! clear of it); nothing for a craft without a ship definition (the test
//! craft).

use super::{layout, theme};
use crate::commands::keys::{bind_keys, key_label};
use crate::commands::{GameCommand, InputContext, Keys};
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sim::craft::command::Stage;

/// The panel's egui area (the docks keep clear of it).
pub const AREA: &str = "mini_panel";

/// A pinned function on the panel.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its index in the vessel's functions.
    pub index: usize,
    pub name: String,
    pub key: Option<KeyCode>,
}

/// The panel's entries: the pinned functions in order, with their keys.
pub fn entries(functions: &[Stage]) -> Vec<Entry> {
    let pinned: Vec<(usize, &Stage)> = functions.iter().enumerate().filter(|(_, f)| f.pinned).collect();
    let keys = bind_keys(&pinned.iter().map(|(_, f)| f.key.as_deref()).collect::<Vec<_>>());
    pinned.into_iter().zip(keys).map(|((index, f), key)| Entry { index, name: f.name.clone(), key }).collect()
}

/// The panel's keys and drawing.
pub struct MiniPanelPlugin;

impl Plugin for MiniPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, keys.after(crate::commands::update_context).before(crate::state::read_controls))
            .add_systems(bevy_egui::EguiPrimaryContextPass, draw.after(crate::hud::draw));
    }
}

/// The run of function `index` of the active vessel.
fn run(sim: &SimState, index: usize) -> Option<(GameCommand, String)> {
    let v = sim.ship();
    let f = v.ship()?.functions.get(index)?;
    Some((GameCommand::Ship { vessel: v.id(), commands: f.actions.clone() }, format!("Function: {}", f.name)))
}

/// A pinned function's key runs it (in flight).
pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    ctx: Res<InputContext>,
    sim: Res<SimState>,
    time: Res<Time>,
    mut cmds: MessageWriter<GameCommand>,
    mut toasts: ResMut<super::toasts::Toasts>,
) {
    if !ctx.allows(Keys::Flight) {
        return;
    }
    let Some(state) = sim.ship().ship() else { return };
    for e in entries(&state.functions) {
        if e.key.is_some_and(|k| keys.just_pressed(k)) {
            if let Some((c, text)) = run(&sim, e.index) {
                cmds.write(c);
                toasts.push(time.elapsed_secs_f64(), text, false);
            }
        }
    }
}

/// The panel, above the navball cluster, in the flight view only.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    station: Res<crate::tracking::TrackingStation>,
    mut systems: ResMut<crate::systems_screen::SystemsScreen>,
    time: Res<Time>,
    mut cmds: MessageWriter<GameCommand>,
    mut toasts: ResMut<super::toasts::Toasts>,
) -> Result {
    let Some(state) = sim.ship().ship() else { return Ok(()) };
    if station.open || systems.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let cluster = ctx.memory(|m| m.area_rect(egui::Id::new(crate::navball::AREA)));
    let screen = ctx.content_rect();
    let at = cluster.map_or(egui::pos2(screen.center().x, screen.max.y - 270.0), |c| {
        egui::pos2(c.center().x, c.min.y - layout::GAP)
    });
    let entries = entries(&state.functions);
    let mut clicked: Option<usize> = None;
    egui::Area::new(AREA.into()).pivot(egui::Align2::CENTER_BOTTOM).fixed_pos(at).show(ctx, |ui| {
        theme::bar_frame().show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.set_max_width(560.0);
                if ui.button(egui::RichText::new("SYSTEMS  F8").color(theme::ACCENT)).clicked() {
                    systems.open = true;
                }
                if entries.is_empty() {
                    ui.label(egui::RichText::new("pin functions in the systems screen").small().color(theme::DIM));
                }
                for e in &entries {
                    let key = e.key.map(|k| format!("[{}] ", key_label(k))).unwrap_or_default();
                    if ui.button(format!("{key}{}", e.name)).clicked() {
                        clicked = Some(e.index);
                    }
                }
            });
        });
    });
    if let Some((c, text)) = clicked.and_then(|i| run(&sim, i)) {
        cmds.write(c);
        toasts.push(time.elapsed_secs_f64(), text, false);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::craft::command::ShipCommand;

    #[test]
    fn only_pinned_functions_show_with_keys_that_never_shadow_the_game() {
        let f = |name: &str, key: Option<&str>, pinned: bool| Stage {
            name: name.into(),
            key: key.map(str::to_string),
            actions: vec![ShipCommand::ArmChutes],
            pinned,
        };
        let functions = [
            f("Legs", Some("L"), true),
            f("Hidden", Some("K"), false),
            f("Chutes", Some("C"), true),
            f("Abort", None, true),
        ];
        let e = entries(&functions);
        let got: Vec<(usize, &str, Option<String>)> =
            e.iter().map(|e| (e.index, e.name.as_str(), e.key.map(key_label))).collect();
        assert_eq!(
            got,
            [(0, "Legs", Some("7".into())), (2, "Chutes", Some("C".into())), (3, "Abort", Some("8".into()))]
        );
        assert!(entries(&[f("x", Some("Z"), false)]).is_empty());
    }

    /// The test rocket's pinned function runs through the ship command.
    #[test]
    fn a_pinned_function_runs_its_commands_on_the_active_vessel() {
        let rocket = crate::test_rocket::rocket();
        let sim = SimState::with_craft(&rocket);
        let e = entries(&sim.ship().ship().unwrap().functions);
        assert_eq!(e.len(), 1, "Legs is pinned, Pair is not");
        let (c, text) = run(&sim, e[0].index).unwrap();
        assert_eq!(text, "Function: Legs");
        assert_eq!(
            c,
            GameCommand::Ship { vessel: sim.ship().id(), commands: vec![ShipCommand::Deploy("leg#*".into())] }
        );
        assert!(run(&sim, 99).is_none());
    }
}
