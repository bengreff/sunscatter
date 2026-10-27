//! The systems screen's drawing: one opaque area over the flight view
//! (D107: a screen of its own), in the regions of [`rules::regions`]: the
//! title and layer switch on top, the part tree on the left, the view in
//! the middle (drag to orbit, scroll to zoom, click to select), the
//! inspector with the selection's commands on the right, the stage and
//! function editors at the bottom. Every change to the craft is a
//! `GameCommand`; the rules are `super::{rules, inspect, edit}`.

use super::edit::{self, Edit};
use super::inspect::{self, Selection};
use super::rules::{self, Layer};
use super::view::SystemsView;
use super::{Drafts, SystemsScreen, Tab};
use crate::commands::keys::{bind_keys, key_label};
use crate::commands::GameCommand;
use crate::interface::theme;
use crate::staging::parts::label;
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiTextureHandle};
use sim::craft::command::Stage;

/// The screen's background.
const FILL: egui::Color32 = egui::Color32::from_rgb(9, 12, 17);

/// Draws the screen while it is open.
pub fn draw(
    mut contexts: EguiContexts,
    mut screen: ResMut<SystemsScreen>,
    view: Res<SystemsView>,
    sim: Res<SimState>,
    mut cmds: MessageWriter<GameCommand>,
    mut texture: Local<Option<egui::TextureId>>,
) -> Result {
    if !screen.open {
        return Ok(());
    }
    let id = *texture.get_or_insert_with(|| contexts.add_image(EguiTextureHandle::Weak(view.image.id())));
    let ctx = contexts.ctx_mut()?;
    let full = ctx.content_rect();
    let regions = rules::regions(full);
    let vessel = sim.ship();
    let craft = sim::craft::craft_of(&vessel.craft.id);
    let ship = vessel.ship_def().zip(vessel.ship().cloned());
    if let Some((_, state)) = &ship {
        let old = screen.drafts.take();
        screen.drafts = Some(Drafts::sync(old, vessel.id(), &craft.id.0, &state.stages, &state.functions));
    }
    let mut out: Vec<GameCommand> = Vec::new();
    egui::Area::new("systems_screen".into()).order(egui::Order::Foreground).fixed_pos(full.min).show(ctx, |ui| {
        ui.painter().rect_filled(full, 0.0, FILL);
        ui.set_min_size(full.size());
        let name = ship.as_ref().map_or_else(|| craft.spec.name.clone(), |(d, _)| d.name.clone());
        region(ui, regions.top, |ui| top_bar(ui, &mut screen, &name, vessel.id().0));
        let Some((def, state)) = ship.as_ref() else {
            region(ui, regions.view, |ui| {
                ui.label(
                    "The test craft is one part (D064): it has no systems to show. Esc > New flight… for the Falcon 9.",
                );
            });
            return;
        };
        region(ui, regions.left, |ui| part_list(ui, &mut screen, &craft, def));
        view_image(ui, &mut screen, regions.view, id, view.radius);
        let known = crate::format::Known::of(vessel);
        region(ui, regions.right, |ui| inspector(ui, &screen, def, state, &view.facts, &known, vessel.id(), &mut out));
        region(ui, regions.bottom, |ui| editors(ui, &mut screen, def, vessel.id(), &mut out));
    });
    for c in out {
        cmds.write(c);
    }
    Ok(())
}

/// Lays out `add` in `rect` with the panel's frame.
fn region(ui: &mut egui::Ui, rect: egui::Rect, add: impl FnOnce(&mut egui::Ui)) {
    let inner = rect.shrink(4.0);
    ui.painter().rect_stroke(inner, 6.0, egui::Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner.shrink(6.0)), |ui| {
        ui.set_clip_rect(inner);
        add(ui);
    });
}

fn top_bar(ui: &mut egui::Ui, screen: &mut SystemsScreen, name: &str, vessel: u64) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("SYSTEMS").size(18.0).color(theme::ACCENT));
        ui.label(egui::RichText::new(format!("{name} · Vessel #{vessel}")).color(theme::TEXT));
        ui.add_space(16.0);
        for layer in Layer::ALL {
            if ui.selectable_label(screen.layer == layer, layer.name()).clicked() {
                screen.layer = layer;
            }
        }
        ui.add_space(16.0);
        if ui.button("close (F8 / Esc)").clicked() {
            screen.open = false;
        }
    });
    ui.label(egui::RichText::new(screen.layer.note()).small().color(theme::DIM));
}

fn part_list(
    ui: &mut egui::Ui,
    screen: &mut SystemsScreen,
    craft: &sim::craft::Craft,
    def: &sim::craft::ship::ShipDef,
) {
    ui.label(egui::RichText::new("PARTS · COMPONENTS").color(theme::DIM));
    egui::ScrollArea::vertical().id_salt("systems_parts").show(ui, |ui| {
        for (depth, k) in rules::tree(&craft.parts) {
            let part = &craft.parts[k];
            let name = def.parts.iter().find(|p| p.part == part.id).map_or(part.name.as_str(), |p| p.id.as_str());
            let sel = Selection::Part(part.id);
            ui.horizontal(|ui| {
                ui.add_space(12.0 * depth as f32);
                if ui.selectable_label(screen.selected == Some(sel), name).clicked() {
                    screen.selected = Some(sel);
                }
            });
            for (i, c) in def.components.iter().enumerate().filter(|(_, c)| c.part == part.id) {
                let sel = Selection::Component(i);
                ui.horizontal(|ui| {
                    ui.add_space(12.0 * depth as f32 + 14.0);
                    let text = egui::RichText::new(format!("· {}", c.id)).color(theme::MUTED);
                    if ui.selectable_label(screen.selected == Some(sel), text).clicked() {
                        screen.selected = Some(sel);
                    }
                });
            }
        }
    });
}

/// The 3D view: drag to orbit, scroll to zoom, click to select.
fn view_image(ui: &mut egui::Ui, screen: &mut SystemsScreen, rect: egui::Rect, id: egui::TextureId, radius: f32) {
    let ppp = ui.ctx().pixels_per_point();
    screen.view_px = Some(UVec2::new((rect.width() * ppp) as u32, (rect.height() * ppp) as u32));
    let image = egui::Image::new(egui::load::SizedTexture::new(id, rect.size())).sense(egui::Sense::click_and_drag());
    let resp = ui.put(rect, image);
    if resp.dragged() {
        let d = resp.drag_delta();
        screen.yaw -= d.x * 0.008;
        screen.pitch = (screen.pitch + d.y * 0.008).clamp(-1.5, 1.5);
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let d = screen.distance.unwrap_or(radius * 2.4);
            screen.distance = Some((d * (1.0 - scroll * 0.002)).clamp(radius * 0.3, radius * 12.0));
        }
    }
    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            screen.click = Some(Vec2::new((p.x - rect.min.x) * ppp, (p.y - rect.min.y) * ppp));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn inspector(
    ui: &mut egui::Ui,
    screen: &SystemsScreen,
    def: &sim::craft::ship::ShipDef,
    state: &sim::craft::state::ShipState,
    facts: &[rules::PartFacts],
    known: &crate::format::Known,
    vessel: sim::vessel::VesselId,
    out: &mut Vec<GameCommand>,
) {
    let Some(sel) = screen.selected else {
        ui.label(egui::RichText::new("Select a part in the view or the list.").color(theme::DIM));
        return;
    };
    let commands = inspect::commands(def, state, sel);
    if !commands.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (text, c) in commands {
                if ui.button(text).clicked() {
                    out.push(GameCommand::Ship { vessel, commands: vec![c] });
                }
            }
        });
        ui.separator();
    }
    egui::ScrollArea::vertical().id_salt("systems_inspector").show(ui, |ui| {
        for g in inspect::details(def, state, facts, sel, known) {
            ui.label(egui::RichText::new(&g.title).color(theme::ACCENT));
            egui::Grid::new(("inspect", g.title.clone())).num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
                for (label, value) in &g.rows {
                    ui.label(egui::RichText::new(label).color(theme::DIM));
                    ui.label(egui::RichText::new(value).monospace());
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
        }
    });
}

fn editors(
    ui: &mut egui::Ui,
    screen: &mut SystemsScreen,
    def: &sim::craft::ship::ShipDef,
    vessel: sim::vessel::VesselId,
    out: &mut Vec<GameCommand>,
) {
    let Some(mut drafts) = screen.drafts.take() else { return };
    ui.horizontal(|ui| {
        for (tab, name) in
            [(Tab::Stages, "STAGES (Space runs the next)"), (Tab::Functions, "FUNCTIONS (pinned: the mini panel)")]
        {
            if ui.selectable_label(screen.tab == tab, name).clicked() {
                screen.tab = tab;
            }
        }
    });
    let functions = screen.tab == Tab::Functions;
    let k = usize::from(functions);
    let list = if functions { &mut drafts.functions } else { &mut drafts.stages };
    let keys: Vec<Option<KeyCode>> = {
        let pinned: Vec<Option<&str>> = list.iter().filter(|s| s.pinned).map(|s| s.key.as_deref()).collect();
        let mut bound = bind_keys(&pinned).into_iter();
        list.iter().map(|s| if s.pinned { bound.next().flatten() } else { None }).collect()
    };
    let catalogue = edit::catalogue(def);
    let mut edits: Vec<Edit> = Vec::new();
    let mut run: Option<Vec<sim::craft::command::ShipCommand>> = None;
    egui::ScrollArea::vertical().id_salt("systems_editor").max_height((ui.available_height() - 30.0).max(40.0)).show(
        ui,
        |ui| {
            for (i, s) in list.iter().enumerate() {
                entry(ui, i, s, functions, keys[i], &catalogue, &mut edits, &mut run);
            }
        },
    );
    ui.horizontal(|ui| {
        if ui.button(if functions { "+ function" } else { "+ stage" }).clicked() {
            edits.push(Edit::Add(if functions { "New function".into() } else { "New stage".into() }));
        }
        let dirty = drafts.dirty[k];
        if ui.add_enabled(dirty, egui::Button::new("save to the vessel")).clicked() {
            let list = if functions { drafts.functions.clone() } else { drafts.stages.clone() };
            out.push(if functions {
                GameCommand::SetFunctions { vessel, functions: list }
            } else {
                GameCommand::SetStages { vessel, stages: list }
            });
            drafts.dirty[k] = false;
        }
        if ui.add_enabled(dirty, egui::Button::new("revert")).clicked() {
            drafts.dirty[k] = false;
        }
        if dirty {
            ui.label(egui::RichText::new("not saved yet").color(theme::WARN));
        }
    });
    let list = if functions { &mut drafts.functions } else { &mut drafts.stages };
    if !edits.is_empty() {
        for e in edits {
            edit::apply(list, e);
        }
        drafts.dirty[k] = true;
    }
    if let Some(actions) = run {
        out.push(GameCommand::Ship { vessel, commands: actions });
    }
    screen.drafts = Some(drafts);
}

/// One stage or function: its name, order, keys and pin, commands.
#[allow(clippy::too_many_arguments)]
fn entry(
    ui: &mut egui::Ui,
    i: usize,
    s: &Stage,
    function: bool,
    key: Option<KeyCode>,
    catalogue: &[sim::craft::command::ShipCommand],
    edits: &mut Vec<Edit>,
    run: &mut Option<Vec<sim::craft::command::ShipCommand>>,
) {
    ui.horizontal(|ui| {
        let mut name = s.name.clone();
        if ui.add(egui::TextEdit::singleline(&mut name).desired_width(170.0)).changed() {
            edits.push(Edit::Rename(i, name));
        }
        if ui.small_button("up").clicked() {
            edits.push(Edit::Up(i));
        }
        if ui.small_button("down").clicked() {
            edits.push(Edit::Down(i));
        }
        if ui.small_button("remove").clicked() {
            edits.push(Edit::Remove(i));
        }
        if function {
            let mut pinned = s.pinned;
            if ui.checkbox(&mut pinned, "pinned").changed() {
                edits.push(Edit::Pin(i, pinned));
            }
            let mut text = s.key.clone().unwrap_or_default();
            ui.label("key");
            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(22.0).char_limit(1)).changed() {
                edits.push(Edit::Key(i, Some(text)));
            }
            if let Some(k) = key {
                ui.label(egui::RichText::new(format!("runs on {}", key_label(k))).color(theme::ACCENT));
            }
            if ui.button("run").clicked() {
                *run = Some(s.actions.clone());
            }
        }
        egui::ComboBox::from_id_salt(("add", function, i)).selected_text("+ command").width(150.0).show_ui(ui, |ui| {
            for c in catalogue {
                if ui.selectable_label(false, label(c)).clicked() {
                    edits.push(Edit::AddAction(i, c.clone()));
                }
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        for (j, c) in s.actions.iter().enumerate() {
            ui.label(egui::RichText::new(label(c)).monospace().color(theme::MUTED));
            // (The default font has no arrow glyphs: plain words.)
            if j > 0 && ui.small_button("<").on_hover_text("earlier").clicked() {
                edits.push(Edit::ActionUp(i, j));
            }
            if ui.small_button("x").on_hover_text("remove").clicked() {
                edits.push(Edit::RemoveAction(i, j));
            }
        }
    });
    ui.add_space(4.0);
}
