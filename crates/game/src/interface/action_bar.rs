//! The action-group bar (D107, D111): in flight, at the top right as wide
//! as the staging bar below it (owner, 2026-10-28: "Move the systems
//! mini-tab to the top right corner, same width as staging"), the way into
//! the systems screen (Tab) and the active craft's functions pinned in the
//! systems screen, each run by a click (no craft-specific keys: abort is one
//! of them, shown only while it would save the crew, D120:
//! `sim::craft::program::offered`), wrapping onto more lines. Nothing for a
//! craft without a ship definition (the test craft). The staging bar stops
//! below it.

use super::{layout, theme};
use crate::commands::GameCommand;
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sim::craft::ship::ShipDef;
use sim::craft::state::ShipState;

/// The bar's egui area (the docks keep clear of it).
pub const AREA: &str = "action_bar";

/// A pinned function on the bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Its index in the vessel's functions.
    pub index: usize,
    pub name: String,
}

/// The bar's entries: the pinned functions in order, of those in the
/// craft's view (`sim::craft::program`: after a split, those acting on
/// this piece) that it offers (an abort only while it would save the crew).
pub fn entries(def: &ShipDef, state: &ShipState) -> Vec<Entry> {
    let functions = &state.functions;
    (sim::craft::program::view(def, functions).into_iter())
        .filter(|v| functions[v.index].pinned)
        .filter(|v| sim::craft::program::offered(def, state, &functions[v.index]).is_ok())
        .map(|v| Entry { index: v.index, name: functions[v.index].name.clone() })
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
    let (Some(def), Some(state)) = (sim.ship().ship_def(), sim.ship().ship()) else { return Ok(()) };
    if station.open || systems.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let entries = entries(&def, state);
    let mut clicked: Option<usize> = None;
    let corner = [-layout::MARGIN, layout::MARGIN];
    egui::Area::new(AREA.into()).anchor(egui::Align2::RIGHT_TOP, corner).movable(false).show(ctx, |ui| {
        let pad = super::stage_bar::draw::PAD as i8;
        theme::panel_frame().inner_margin(egui::Margin::same(pad)).show(ui, |ui| {
            ui.set_width(super::stage_bar::draw::inner());
            ui.horizontal_wrapped(|ui| {
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
    use sim::craft::command::{ShipCommand, Stage};

    #[test]
    fn only_pinned_functions_show_and_keys_are_ignored() {
        let f = |name: &str, key: Option<&str>, pinned: bool| Stage {
            name: name.into(),
            key: key.map(str::to_string),
            actions: vec![ShipCommand::ArmChutes],
            pinned,
            abort: false,
        };
        let rocket = crate::test_rocket::rocket();
        let def = rocket.ship.as_deref().unwrap();
        let mut state = ShipState::initial(def);
        // A function named "Abort" is a function like any other; one marked
        // as an abort that would save no one here is not shown.
        state.functions = vec![f("Legs", Some("L"), true), f("Hidden", Some("K"), false), f("Abort", None, true)];
        // Another piece's function (nothing here it acts on): not shown.
        state
            .functions
            .push(Stage { actions: vec![ShipCommand::Deploy("nothing#*".into())], ..f("Other", None, true) });
        state.functions.push(Stage { abort: true, ..f("Escape", None, true) });
        let shown = entries(def, &state);
        let got: Vec<(usize, &str)> = shown.iter().map(|e| (e.index, e.name.as_str())).collect();
        assert_eq!(got, [(0, "Legs"), (2, "Abort")]);
        state.functions = vec![f("x", Some("Z"), false)];
        assert!(entries(def, &state).is_empty());
    }

    /// The test rocket's pinned function is sent to the vessel by name; the
    /// Falcon 9 Crew's launch abort is on the bar while it would save the
    /// crew (D120): on the stack and the upper stack, not on the booster
    /// after the stage separation, nor once Dragon's separation has fired.
    #[test]
    fn a_pinned_function_runs_on_the_active_vessel_and_abort_is_one() {
        let rocket = crate::test_rocket::rocket();
        let sim = SimState::with_craft(&rocket);
        let e = entries(&sim.ship().ship_def().unwrap(), sim.ship().ship().unwrap());
        assert_eq!(e.len(), 1, "Legs is pinned, Pair is not");
        let c = run(&sim, e[0].index).unwrap();
        assert_eq!(c, GameCommand::Function { vessel: sim.ship().id(), name: "Legs".into() });
        assert!(run(&sim, 99).is_none());
        let f9 = SimState::with_craft(&crate::state::craft_or_test("falcon9-crew"));
        let def = f9.ship().ship_def().unwrap();
        let piece = |keep: &dyn Fn(&str) -> bool| {
            def.subset(&def.parts.iter().filter(|p| keep(&p.id)).map(|p| p.part).collect::<Vec<_>>())
        };
        let upper = |id: &str| id.starts_with("s2.") || id.starts_with("dragon.");
        let mut fired = f9.ship().ship().unwrap().clone();
        let j = def.joints.iter().position(|j| j.name == "dragon separation").unwrap();
        fired.joints[j].intact = false;
        // (case, piece, its state or the initial one, the abort on the bar)
        let table: [(&str, ShipDef, Option<ShipState>, bool); 4] = [
            ("the stack on the pad", (*def).clone(), None, true),
            ("Dragon's separation fired", (*def).clone(), Some(fired), false),
            ("the upper stack", piece(&upper), None, true),
            ("the booster", piece(&|id| !upper(id)), None, false),
        ];
        for (case, d, state, want) in table {
            let state = state.unwrap_or_else(|| ShipState::initial(&d));
            let names: Vec<String> = entries(&d, &state).into_iter().map(|e| e.name).collect();
            assert_eq!(names.iter().any(|n| n == "Launch abort"), want, "{case}: {names:?}");
        }
    }
}
