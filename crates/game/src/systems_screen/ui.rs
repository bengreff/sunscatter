//! The systems screen's drawing: one opaque area over the flight view
//! (D107: a screen of its own), in the regions of [`rules::regions`]: the
//! title and layer switch on top, the part tree on the left, the view in
//! the middle (drag to orbit, scroll to zoom, click to select), the
//! layer's summary on the right, the stage and function editors at the
//! bottom; over them the pop-ups (egui windows, closable, several at
//! once), each a part or component with its commands. Every change to the
//! craft is a `GameCommand`; the rules are `super::{rules, summary,
//! inspect, edit}`.

use super::edit::{self, Edit};
use super::inspect::{self, Group, Selection};
use super::rules::{self, Layer};
use super::view::SystemsView;
use super::{Drafts, SystemsScreen, Tab};
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
    let known = crate::format::Known::of(vessel);
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
        region(ui, regions.right, |ui| {
            let power = crate::interface::menus::power::gather(vessel);
            let x = super::summary::Inputs {
                def,
                state,
                facts: &view.facts,
                known: &known,
                mass: (vessel.mass_props().mass, vessel.craft.mass.dry_mass, vessel.contact_height()),
                power: power.as_ref(),
            };
            layer_summary(ui, &mut screen, &x);
        });
        region(ui, regions.bottom, |ui| editors(ui, &mut screen, def, vessel.id(), &mut out));
    });
    if let Some((def, state)) = ship.as_ref() {
        screen.settle_layer(&view.facts);
        popups(ctx, &mut screen, def, state, &view.facts, &known, regions.view, vessel.id(), &mut out);
    }
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
                screen.pick_layer = false;
            }
        }
        ui.add_space(16.0);
        if ui.button("close (Tab / Esc)").clicked() {
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
                    screen.select(Some(sel));
                }
            });
            for (i, c) in def.components.iter().enumerate().filter(|(_, c)| c.part == part.id) {
                let sel = Selection::Component(i);
                ui.horizontal(|ui| {
                    ui.add_space(12.0 * depth as f32 + 14.0);
                    let text = egui::RichText::new(format!("· {}", c.id)).color(theme::MUTED);
                    if ui.selectable_label(screen.selected == Some(sel), text).clicked() {
                        screen.select(Some(sel));
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

/// The layer as a whole (`summary`); a row about a part opens its pop-up.
fn layer_summary(ui: &mut egui::Ui, screen: &mut SystemsScreen, x: &super::summary::Inputs) {
    ui.label(egui::RichText::new(format!("{} · THE CRAFT AS A WHOLE", screen.layer.name())).color(theme::DIM));
    ui.label(egui::RichText::new("Click a part, or a row about one, for its details.").small().color(theme::DIM));
    ui.add_space(4.0);
    let mut open = None;
    egui::ScrollArea::vertical().id_salt("systems_summary").show(ui, |ui| {
        groups(ui, "summary", &super::summary::summary(screen.layer, x), &mut open);
    });
    if open.is_some() {
        screen.select(open);
    }
}

/// Titled groups of rows: dim labels (a term's hover explains it), values
/// in monospace coloured by their level against their limit; a row about
/// a part or component is a link to its pop-up (`open`).
fn groups(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    groups: &[Group],
    open: &mut Option<Selection>,
) {
    // A long value (an unmodelled mass's reason) wraps within the width
    // left of the labels instead of widening the column or the pop-up.
    let widest = (ui.available_width() * 0.62).max(120.0);
    for g in groups {
        ui.label(egui::RichText::new(&g.title).color(theme::ACCENT));
        let grid = egui::Grid::new((id, g.title.as_str())).num_columns(2).spacing([12.0, 2.0]);
        grid.max_col_width(widest).show(ui, |ui| {
            for r in &g.rows {
                let label = ui.label(egui::RichText::new(&r.label).color(theme::DIM));
                if let Some(h) = &r.hover {
                    label.on_hover_text(h);
                }
                let value = egui::RichText::new(&r.value).monospace().color(theme::level_colour(r.level));
                match r.open {
                    Some(sel) => {
                        if ui.link(value).on_hover_text("Open its details").clicked() {
                            *open = Some(sel);
                        }
                    }
                    None => {
                        ui.add(egui::Label::new(value).wrap());
                    }
                }
                ui.end_row();
            }
        });
        ui.add_space(6.0);
    }
}

/// A pop-up's window: the `n`th open cascades from the view's top left.
fn popup_window<'a>(title: String, sel: Selection, n: usize, view: egui::Rect, open: &'a mut bool) -> egui::Window<'a> {
    let at = view.min + egui::vec2(16.0, 16.0) + egui::vec2(28.0, 28.0) * n as f32;
    egui::Window::new(egui::RichText::new(title).color(theme::ACCENT))
        .id(egui::Id::new(("systems_popup", sel)))
        .order(egui::Order::Tooltip)
        .open(open)
        .collapsible(false)
        .resizable(false)
        .default_pos(at)
        .constrain_to(view)
        .frame(
            egui::Frame::window(&egui::Style::default()).fill(POPUP_FILL).stroke(egui::Stroke::new(1.0, theme::BORDER)),
        )
}

/// A pop-up's background: opaque, a shade above the screen's.
const POPUP_FILL: egui::Color32 = egui::Color32::from_rgb(16, 21, 29);

/// A pop-up's width (logical px).
const POPUP_WIDTH: f32 = 340.0;

/// The open pop-ups: each part or component in its own window over the
/// view (closable, cascading), its commands on top, then everything about
/// it (`inspect::details`).
#[allow(clippy::too_many_arguments)]
fn popups(
    ctx: &egui::Context,
    screen: &mut SystemsScreen,
    def: &sim::craft::ship::ShipDef,
    state: &sim::craft::state::ShipState,
    facts: &[rules::PartFacts],
    known: &crate::format::Known,
    view: egui::Rect,
    vessel: sim::vessel::VesselId,
    out: &mut Vec<GameCommand>,
) {
    let mut closed = Vec::new();
    let mut opened = None;
    for (n, sel) in screen.popups.clone().into_iter().enumerate() {
        let title = match sel {
            Selection::Part(id) => def.parts.iter().find(|p| p.part == id).map_or("part".into(), |p| p.id.clone()),
            Selection::Component(i) => def.components.get(i).map_or("component".into(), |c| c.id.clone()),
        };
        let mut open = true;
        popup_window(title, sel, n, view, &mut open).show(ctx, |ui| {
            // A fixed width (a new window's first, sizing pass could take the screen's).
            ui.set_width(POPUP_WIDTH);
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
            let room = (view.height() - 110.0).max(120.0);
            egui::ScrollArea::vertical().id_salt(("systems_popup_scroll", sel)).max_height(room).show(ui, |ui| {
                groups(ui, ("popup", sel), &inspect::details(def, state, facts, sel, known), &mut opened);
            });
        });
        if !open {
            closed.push(sel);
        }
    }
    screen.popups.retain(|p| !closed.contains(p));
    if screen.selected.is_some_and(|s| closed.contains(&s)) {
        screen.selected = None;
    }
    if opened.is_some() {
        screen.select(opened);
    }
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
            [(Tab::Stages, "STAGES (Space runs the next)"), (Tab::Functions, "FUNCTIONS (pinned: the action bar)")]
        {
            if ui.selectable_label(screen.tab == tab, name).clicked() {
                screen.tab = tab;
            }
        }
    });
    let functions = screen.tab == Tab::Functions;
    let k = usize::from(functions);
    let list = if functions { &mut drafts.functions } else { &mut drafts.stages };
    let catalogue = edit::catalogue(def);
    let mut edits: Vec<Edit> = Vec::new();
    let mut run: Option<Vec<sim::craft::command::ShipCommand>> = None;
    egui::ScrollArea::vertical().id_salt("systems_editor").max_height((ui.available_height() - 30.0).max(40.0)).show(
        ui,
        |ui| {
            for (i, s) in list.iter().enumerate() {
                entry(ui, i, s, functions, &catalogue, &mut edits, &mut run);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A pop-up keeps its width and stays where it opens (in the view).
    #[test]
    fn a_pop_up_is_a_small_window_in_the_view() {
        let ctx = egui::Context::default();
        let view = egui::Rect::from_min_max(egui::pos2(280.0, 70.0), egui::pos2(1220.0, 650.0));
        let mut g = Group::new("PART s1.tank");
        for k in 0..40 {
            g.row(&format!("row {k}"), "12.3 bar / 45.6 bar").opens(Selection::Component(k));
            g.row("ullage pressure", "12.3 bar / 45.6 bar");
            g.row("unmodelled", "2503.4 kg (Falcon 9 first stage dry mass (Block 5, legs and grid fins): brackets, ring frames, harnesses and margins)");
        }
        let sel = Selection::Part(3);
        let mut rect = egui::Rect::NOTHING;
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0))),
                ..Default::default()
            };
            ctx.begin_pass(input);
            let mut open = true;
            let shown = popup_window("s1.tank".into(), sel, 0, view, &mut open).show(&ctx, |ui| {
                ui.set_width(POPUP_WIDTH);
                let room = view.height() - 110.0;
                egui::ScrollArea::vertical()
                    .max_height(room)
                    .show(ui, |ui| groups(ui, "t", std::slice::from_ref(&g), &mut None));
            });
            rect = shown.map(|s| s.response.rect).unwrap_or(rect);
            ctx.end_pass().drop_without_applying_deltas();
        }
        assert!(rect.width() < POPUP_WIDTH + 40.0, "{rect:?}");
        assert!(view.contains_rect(rect), "{rect:?}");
    }
}
