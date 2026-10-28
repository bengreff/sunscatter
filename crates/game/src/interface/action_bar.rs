//! The action-group bar (D107, D111): in flight, just above the navball
//! cluster, the active craft's functions pinned in the systems screen, each
//! run by a click (no craft-specific keys: abort is one of them), and the
//! way into the systems screen (Tab). Nothing for a craft without a ship
//! definition (the test craft). The docks keep clear of it.

use super::{layout, theme};
use crate::commands::GameCommand;
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sim::craft::command::Stage;

/// The bar's egui area (the docks keep clear of it).
pub const AREA: &str = "action_bar";

/// A pinned function on the bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its index in the vessel's functions.
    pub index: usize,
    pub name: String,
}

/// The bar's entries: the pinned functions in order.
pub fn entries(functions: &[Stage]) -> Vec<Entry> {
    (functions.iter().enumerate())
        .filter(|(_, f)| f.pinned)
        .map(|(index, f)| Entry { index, name: f.name.clone() })
        .collect()
}

/// The bar's drawing and the ship's events.
pub struct ActionBarPlugin;

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, super::events::toast_events.after(crate::state::advance))
            .add_systems(bevy_egui::EguiPrimaryContextPass, draw.after(crate::hud::draw));
    }
}

/// The run of function `index` of the active vessel (its systems log it:
/// `interface::events` notes it).
fn run(sim: &SimState, index: usize) -> Option<GameCommand> {
    let v = sim.ship();
    let f = v.ship()?.functions.get(index)?;
    Some(GameCommand::Function { vessel: v.id(), name: f.name.clone() })
}

/// The bar, above the navball cluster, in the flight view only.
pub fn draw(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    station: Res<crate::tracking::TrackingStation>,
    mut systems: ResMut<crate::systems_screen::SystemsScreen>,
    mut cmds: MessageWriter<GameCommand>,
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
                let into = ui.button(egui::RichText::new("SYSTEMS  Tab").color(theme::ACCENT));
                if into.on_hover_text("The systems screen: every part by layer, stages and functions").clicked() {
                    systems.open_at(None, None);
                }
                if entries.is_empty() {
                    ui.label(egui::RichText::new("pin functions in the systems screen").small().color(theme::DIM));
                }
                for e in &entries {
                    if ui.button(&e.name).on_hover_text("Run this function (pinned in the systems screen)").clicked() {
                        clicked = Some(e.index);
                    }
                }
            });
        });
    });
    if let Some(c) = clicked.and_then(|i| run(&sim, i)) {
        cmds.write(c);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::craft::command::ShipCommand;

    #[test]
    fn only_pinned_functions_show_and_keys_are_ignored() {
        let f = |name: &str, key: Option<&str>, pinned: bool| Stage {
            name: name.into(),
            key: key.map(str::to_string),
            actions: vec![ShipCommand::ArmChutes],
            pinned,
        };
        let functions = [f("Legs", Some("L"), true), f("Hidden", Some("K"), false), f("Abort", None, true)];
        let shown = entries(&functions);
        let got: Vec<(usize, &str)> = shown.iter().map(|e| (e.index, e.name.as_str())).collect();
        assert_eq!(got, [(0, "Legs"), (2, "Abort")]);
        assert!(entries(&[f("x", Some("Z"), false)]).is_empty());
    }

    /// The test rocket's pinned function is sent to the vessel by name; the
    /// Falcon 9 Crew's launch abort is on the bar.
    #[test]
    fn a_pinned_function_runs_on_the_active_vessel_and_abort_is_one() {
        let rocket = crate::test_rocket::rocket();
        let sim = SimState::with_craft(&rocket);
        let e = entries(&sim.ship().ship().unwrap().functions);
        assert_eq!(e.len(), 1, "Legs is pinned, Pair is not");
        let c = run(&sim, e[0].index).unwrap();
        assert_eq!(c, GameCommand::Function { vessel: sim.ship().id(), name: "Legs".into() });
        assert!(run(&sim, 99).is_none());
        let f9 = SimState::with_craft(&crate::state::craft_or_test("falcon9-crew"));
        let names: Vec<String> = entries(&f9.ship().ship().unwrap().functions).into_iter().map(|e| e.name).collect();
        assert!(names.iter().any(|n| n == "Launch abort"), "{names:?}");
    }
}
