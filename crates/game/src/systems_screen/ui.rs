//! The systems screen's drawing: one opaque area over the flight view
//! (D107: a screen of its own), in the regions of [`rules::regions`]: the
//! title and layer switch on top; on the left the layer's summary (by
//! default) or the part tree, by a switch; the view in the middle (drag to
//! orbit, scroll to zoom, click to select); on the right the selected part
//! or component with its commands; the stage and function editors at the
//! bottom. No pop-ups (owner, 2026-10-28). Every change to the craft is a
//! `GameCommand`; the rules are `super::{rules, summary, inspect, edit}`.

use super::edit::{self, Edit};
use super::inspect::{self, Group, Selection};
use super::rules::{self, Layer};
use super::view::SystemsView;
use super::{Drafts, Left, SystemsScreen, Tab};
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
        region(ui, regions.left, |ui| {
            let power = crate::interface::menus::power::gather(vessel);
            let x = super::summary::Inputs {
                def,
                state,
                facts: &view.facts,
                known: &known,
                mass: (vessel.mass_props().mass, vessel.craft.mass.dry_mass, vessel.contact_height()),
                power: power.as_ref(),
            };
            left_column(ui, &mut screen, &craft, &x);
        });
        view_image(ui, &mut screen, regions.view, id, (view.framed_radius, view.distance_now));
        region(ui, regions.right, |ui| {
            let x = Selected { def, state, facts: &view.facts, known: &known, vessel: vessel.id() };
            selected(ui, &mut screen, &x, &mut out);
        });
        region(ui, regions.bottom, |ui| editors(ui, &mut screen, def, state, vessel.id(), &mut out));
    });
    if ship.is_some() {
        screen.settle_layer(&view.facts);
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
        let close =
            format!("close ({} / Esc)", crate::commands::keys::label(crate::commands::keys::Action::SystemsScreen));
        if ui.button(close).clicked() {
            screen.close();
        }
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(screen.layer.note()).small().color(theme::DIM));
        if screen.layer == Layer::Thermal {
            ui.add_space(12.0);
            ui.label(egui::RichText::new("cut away").small().color(theme::DIM));
            ui.add(egui::Slider::new(&mut screen.cut, 0.0..=1.0).show_value(false)).on_hover_text(
                "Cut away the side facing you to see the interior nodes: none at the left, all at the right",
            );
        }
    });
}

/// The left column: the craft as a whole in the layer shown (the default)
/// or the parts list, by its switch.
fn left_column(ui: &mut egui::Ui, screen: &mut SystemsScreen, craft: &sim::craft::Craft, x: &super::summary::Inputs) {
    ui.horizontal(|ui| {
        for (left, label) in [(Left::SystemWide, "SYSTEM-WIDE"), (Left::Parts, "PARTS")] {
            if ui.selectable_label(screen.left == left, label).clicked() {
                screen.left = left;
            }
        }
    });
    ui.add_space(4.0);
    match screen.left {
        Left::SystemWide => layer_summary(ui, screen, x),
        Left::Parts => part_list(ui, screen, craft, x.def),
    }
}

/// What the right column reads.
struct Selected<'a> {
    def: &'a sim::craft::ship::ShipDef,
    state: &'a sim::craft::state::ShipState,
    facts: &'a [rules::PartFacts],
    known: &'a crate::format::Known,
    vessel: sim::vessel::VesselId,
}

/// The right column: the selected part or component, its commands on top,
/// then everything about it (`inspect::details`); a link in it selects what
/// it names. A hint while nothing is selected.
fn selected(ui: &mut egui::Ui, screen: &mut SystemsScreen, x: &Selected, out: &mut Vec<GameCommand>) {
    let Some(sel) = screen.selected else {
        ui.label(egui::RichText::new("SELECTED").color(theme::DIM));
        let hint = "Click a part in the view, in the parts list or in a row about one: it is isolated and framed, \
                    and everything about it shows here. Esc or its × goes back to this view.";
        ui.label(egui::RichText::new(hint).small().color(theme::DIM));
        return;
    };
    let def = x.def;
    let title = match sel {
        Selection::Part(id) => def.parts.iter().find(|p| p.part == id).map_or("part".into(), |p| p.id.clone()),
        Selection::Component(i) => def.components.get(i).map_or("component".into(), |c| c.id.clone()),
    };
    let mut clear = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).color(theme::ACCENT));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let x = ui.add(egui::Button::new(egui::RichText::new("×").color(theme::DIM)).frame(false));
            clear = x.on_hover_text("Back to the layer view as it was (Esc)").clicked();
            ui.label(egui::RichText::new("isolated · Esc or × goes back").small().color(theme::DIM));
        });
    });
    let commands = inspect::commands(def, x.state, sel);
    if !commands.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for (text, c) in commands {
                if ui.button(text).clicked() {
                    out.push(GameCommand::Ship { vessel: x.vessel, commands: vec![c] });
                }
            }
        });
        ui.separator();
    }
    let mut opened = None;
    egui::ScrollArea::vertical().id_salt(("systems_selected", sel)).show(ui, |ui| {
        groups(ui, ("selected", sel), &inspect::details(def, x.state, x.facts, sel, x.known), &mut opened);
    });
    if clear {
        screen.select(None);
    } else if opened.is_some() {
        screen.select(opened);
    }
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

/// The 3D view: drag to orbit, scroll to zoom (about what is framed:
/// `framed` its radius and the distance now), click to isolate a part.
fn view_image(
    ui: &mut egui::Ui,
    screen: &mut SystemsScreen,
    rect: egui::Rect,
    id: egui::TextureId,
    framed: (f32, f32),
) {
    let (radius, now) = framed;
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
            let d = screen.distance.unwrap_or(now);
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
    ui.label(egui::RichText::new("Click a part, or a row about one: it shows on the right.").small().color(theme::DIM));
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
/// a part or component is a link that selects it (`open`).
fn groups(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    groups: &[Group],
    open: &mut Option<Selection>,
) {
    // A long value (an unmodelled mass's reason) wraps within the width
    // left of the labels instead of widening the column.
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

/// The stage and function editors, on the craft's view of its program
/// (`sim::craft::program`: after a split, what acts on this piece; edits by
/// the program's indices); the stages already run are shown locked
/// ([`edit::allowed`]); a function runs only where the piece offers it (an
/// abort only while it would save the crew, `program::offered`).
fn editors(
    ui: &mut egui::Ui,
    screen: &mut SystemsScreen,
    def: &sim::craft::ship::ShipDef,
    state: &sim::craft::state::ShipState,
    vessel: sim::vessel::VesselId,
    out: &mut Vec<GameCommand>,
) {
    let sequence = state.sequence as usize;
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
    let locked = if functions { 0 } else { sequence };
    let catalogue = edit::catalogue(def);
    let mut edits: Vec<Edit> = Vec::new();
    let mut run: Option<Vec<sim::craft::command::ShipCommand>> = None;
    let view = sim::craft::program::view(def, list);
    let shown: Vec<usize> = view.iter().map(|v| v.index).collect();
    egui::ScrollArea::vertical().id_salt("systems_editor").max_height((ui.available_height() - 30.0).max(40.0)).show(
        ui,
        |ui| {
            for v in &view {
                let offered = sim::craft::program::offered(def, state, &list[v.index]);
                let at = Entry { shown: &shown, actions: &v.actions, function: functions, locked, offered };
                entry(ui, v.index, &list[v.index], &at, &catalogue, &mut edits, &mut run);
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
        for e in edits.into_iter().filter(|e| edit::allowed(e, locked)) {
            edit::apply(list, e);
        }
        drafts.dirty[k] = true;
    }
    if let Some(actions) = run {
        out.push(GameCommand::Ship { vessel, commands: actions });
    }
    screen.drafts = Some(drafts);
}

/// Where an entry sits in the editor's view: the view's entries (their
/// indices in the program), the entry's commands shown, whether it is a
/// function, how many stages are locked (run), and whether the piece
/// offers it.
struct Entry<'a> {
    shown: &'a [usize],
    actions: &'a [usize],
    function: bool,
    locked: usize,
    offered: Result<(), sim::craft::program::NoEscape>,
}

/// One stage or function (`i`: its index in the program): its name,
/// order, keys and pin, its commands in the view. The first `locked`
/// entries (stages already run) are shown but disabled.
fn entry(
    ui: &mut egui::Ui,
    i: usize,
    s: &Stage,
    at: &Entry,
    catalogue: &[sim::craft::command::ShipCommand],
    edits: &mut Vec<Edit>,
    run: &mut Option<Vec<sim::craft::command::ShipCommand>>,
) {
    let done = i < at.locked;
    ui.horizontal(|ui| {
        if done {
            let tag = egui::RichText::new("RUN").monospace().color(theme::MUTED);
            ui.label(tag).on_hover_text("Already run: locked. AGAIN on the staging bar runs it again.");
        }
        ui.add_enabled_ui(!done, |ui| entry_row(ui, i, s, at, catalogue, edits, run));
    });
    ui.add_enabled_ui(!done, |ui| entry_commands(ui, i, s, at.actions, edits));
    ui.add_space(4.0);
}

/// An entry's name, order, pin and command picker.
fn entry_row(
    ui: &mut egui::Ui,
    i: usize,
    s: &Stage,
    at: &Entry,
    catalogue: &[sim::craft::command::ShipCommand],
    edits: &mut Vec<Edit>,
    run: &mut Option<Vec<sim::craft::command::ShipCommand>>,
) {
    let mut name = s.name.clone();
    if ui.add(egui::TextEdit::singleline(&mut name).desired_width(170.0)).changed() {
        edits.push(Edit::Rename(i, name));
    }
    let (up, down) = edit::moves(at.shown, i);
    for (e, word) in [(up, "up"), (down, "down")] {
        let ok = e.as_ref().is_some_and(|e| edit::allowed(e, at.locked));
        if ui.add_enabled(ok, egui::Button::new(word).small()).clicked() {
            edits.extend(e);
        }
    }
    if ui.small_button("remove").clicked() {
        edits.push(Edit::Remove(i));
    }
    if at.function {
        let mut pinned = s.pinned;
        if ui.checkbox(&mut pinned, "pinned").changed() {
            edits.push(Edit::Pin(i, pinned));
        }
        if s.abort {
            let tag = egui::RichText::new("ABORT").monospace().color(theme::WARN);
            ui.label(tag).on_hover_text("An escape: offered only while it would save the crew");
        }
        let button = ui.add_enabled(at.offered.is_ok(), egui::Button::new("run"));
        let button = match at.offered {
            Err(why) => button.on_disabled_hover_text(format!("It would save no one here: {}", why.describe())),
            Ok(()) => button,
        };
        if button.clicked() {
            *run = Some(at.actions.iter().map(|&j| s.actions[j].clone()).collect());
        }
    }
    egui::ComboBox::from_id_salt(("add", at.function, i)).selected_text("+ command").width(150.0).show_ui(ui, |ui| {
        for c in catalogue {
            if ui.selectable_label(false, label(c)).clicked() {
                edits.push(Edit::AddAction(i, c.clone()));
            }
        }
    });
}

/// An entry's commands in the view (`shown`: their indices in the
/// entry), each with its order and remove buttons.
fn entry_commands(ui: &mut egui::Ui, i: usize, s: &Stage, shown: &[usize], edits: &mut Vec<Edit>) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(14.0);
        for (k, &j) in shown.iter().enumerate() {
            ui.label(egui::RichText::new(label(&s.actions[j])).monospace().color(theme::MUTED));
            // (The default font has no arrow glyphs: plain words.)
            if k > 0 && ui.small_button("<").on_hover_text("earlier").clicked() {
                edits.push(Edit::SwapActions(i, shown[k - 1], j));
            }
            if ui.small_button("x").on_hover_text("remove").clicked() {
                edits.push(Edit::RemoveAction(i, j));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The details wrap within the right column: a long value (an
    /// unmodelled mass's reason) does not widen it.
    #[test]
    fn the_details_keep_to_the_right_column() {
        let ctx = egui::Context::default();
        let mut g = Group::new("PART s1.tank");
        for k in 0..40 {
            g.row(&format!("row {k}"), "12.3 bar / 45.6 bar").opens(Selection::Component(k));
            g.row("unmodelled", "2503.4 kg (Falcon 9 first stage dry mass (Block 5, legs and grid fins): brackets, ring frames, harnesses and margins)");
        }
        let width = rules::RIGHT_WIDTH - 20.0;
        let mut rect = egui::Rect::NOTHING;
        for _ in 0..4 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 900.0))),
                ..Default::default()
            };
            ctx.begin_pass(input);
            let shown = egui::Area::new("t".into()).fixed_pos(egui::pos2(1240.0, 70.0)).show(&ctx, |ui| {
                ui.set_width(width);
                ui.set_max_width(width);
                egui::ScrollArea::vertical().max_height(600.0).show(ui, |ui| {
                    groups(ui, "t", std::slice::from_ref(&g), &mut None);
                });
            });
            rect = shown.response.rect;
            ctx.end_pass().drop_without_applying_deltas();
        }
        assert!(rect.width() <= width + 1.0, "{rect:?}");
    }
}
