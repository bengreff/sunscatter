//! Drawing the staging bar with egui (its rules are `rows` and `icons`):
//! the rows in a scroll area that keeps the next stage in view, the active
//! stage, the stages run before it (one line each), the air switch; dragging and dropping stages and commands,
//! deleting a stage with a confirmation, clicking an icon.

use super::icons::{self, SIZE};
use super::rows::{drop_edit, Drag, Figures, Place, Row};
use super::{Air, StageBar, AREA, WIDTH};
use crate::interface::{layout, theme};
use bevy_egui::egui::{self, vec2, Align, Align2, Color32, Id, Layout, RichText, Sense, Stroke, StrokeKind};
use sim::craft::command::SequenceEdit;

/// What the player asked of the bar this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Edit(SequenceEdit),
    /// Open the card of the part (`Part::id`).
    Open(u32),
    NextStage,
    /// Run stage `.0` again (D114).
    Again(u32),
}

/// The inner width of a panel the staging bar's width (logical px): its
/// rows, and the action bar above it.
pub fn inner() -> f32 {
    WIDTH - 2.0 * PAD - 2.0
}

/// A panel's inner margin (logical px).
pub const PAD: f32 = 6.0;

/// Draws the bar (see the module) and returns what was asked of it.
pub fn bar(ctx: &egui::Context, rows: &[Row], bar: &mut StageBar) -> Vec<Action> {
    let mut out = Vec::new();
    let screen = ctx.content_rect();
    let done = rows.iter().filter(|r| r.place == Place::Done).count();
    // Up to below the action bar (top right) when it is drawn.
    let top = ctx.memory(|m| m.area_rect(Id::new(crate::interface::action_bar::AREA)));
    let top = top.map_or(screen.min.y + layout::MARGIN, |r| r.max.y + layout::GAP);
    let tall = screen.max.y - layout::MARGIN - top;
    let room = scroll_room(tall, rows.iter().any(|r| r.place == Place::Active), done);
    egui::Area::new(Id::new(AREA))
        .anchor(Align2::RIGHT_BOTTOM, [-layout::MARGIN, -layout::MARGIN])
        .movable(false)
        .show(ctx, |ui| {
            // An area lays out within its remembered size (egui's default
            // is 400 px tall): without this, stages added in flight would
            // not grow the scroll area past it (the notifications' bug).
            ui.set_max_height(tall);
            theme::panel_frame().inner_margin(egui::Margin::same(PAD as i8)).show(ui, |ui| {
                ui.set_width(inner());
                ui.spacing_mut().item_spacing = vec2(4.0, 3.0);
                let pending: Vec<&Row> =
                    rows.iter().filter(|r| matches!(r.place, Place::Next | Place::Later)).collect();
                if pending.is_empty() {
                    ui.label(RichText::new("no stage left").small().color(theme::DIM));
                }
                egui::ScrollArea::vertical().max_height(room).stick_to_bottom(true).show(ui, |ui| {
                    for row in pending {
                        stage(ui, row, bar, &mut out);
                    }
                });
                for row in rows.iter().filter(|r| r.place == Place::Active) {
                    ui.add_space(2.0);
                    stage(ui, row, bar, &mut out);
                }
                for row in rows.iter().filter(|r| r.place == Place::Done) {
                    done_line(ui, row, &mut out);
                }
                ui.add_space(2.0);
                switch(ui, bar);
            });
        });
    dragged(ctx, rows);
    out
}

/// The height of a stage run before the active one (one line).
const DONE_HEIGHT: f32 = 19.0;
/// The room kept for the active stage (its bars and figures) and for the
/// CURRENT AIR / VACUUM switch (logical px).
const ACTIVE_HEIGHT: f32 = 210.0;
const SWITCH_HEIGHT: f32 = 30.0;
/// The stages still to run get at least this much (logical px).
const MIN_ROOM: f32 = 120.0;

/// The height the stages still to run may take before they scroll, in a
/// column `tall` px high: what the active stage (when one runs), the `done`
/// stages run before it and the switch leave. On the pad no stage is
/// active, and the room kept for one hid the top stage (Dragon separation)
/// behind the scroll with the column's top empty (finding 61).
pub fn scroll_room(tall: f32, active: bool, done: usize) -> f32 {
    let active = if active { ACTIVE_HEIGHT } else { 0.0 };
    (tall - SWITCH_HEIGHT - active - DONE_HEIGHT * done as f32).max(MIN_ROOM)
}

/// A stage run before the active one: its number and name, and AGAIN.
fn done_line(ui: &mut egui::Ui, row: &Row, out: &mut Vec<Action>) {
    let Some(j) = row.stage else { return };
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            again(ui, j, out);
            let number = row.number.map(|n| format!("{n}  ")).unwrap_or_default();
            let text = RichText::new(format!("{number}{}", row.name)).small().color(theme::DIM);
            ui.add(egui::Label::new(text).truncate());
        });
    });
}

/// The AGAIN button of stage `j` (D114).
fn again(ui: &mut egui::Ui, j: usize, out: &mut Vec<Action>) {
    let button = ui.add(egui::Button::new(RichText::new("AGAIN").small().color(theme::GOOD)).small());
    let hint =
        "Run this stage again: what can act does (an engine off with an ignition left starts), the rest does nothing";
    if button.on_hover_text(hint).clicked() {
        out.push(Action::Again(j as u32));
    }
}

/// What is being dragged, shown at the pointer.
fn dragged(ctx: &egui::Context, rows: &[Row]) {
    let Some(d) = egui::DragAndDrop::payload::<Drag>(ctx) else { return };
    let Some(at) = ctx.pointer_latest_pos() else { return };
    let text = match *d {
        Drag::Stage(j) => rows.iter().find(|r| r.stage == Some(j)).map(|r| r.name.clone()),
        Drag::Action { stage, action } => (rows.iter().find(|r| r.stage == Some(stage)))
            .and_then(|r| r.items.iter().find(|i| i.action == action))
            .map(|i| i.label.clone()),
    };
    let Some(text) = text else { return };
    egui::Area::new(Id::new("stage_bar_drag")).order(egui::Order::Tooltip).fixed_pos(at + vec2(10.0, 6.0)).show(
        ctx,
        |ui| {
            theme::bar_frame().show(ui, |ui| ui.label(RichText::new(text).small().color(theme::ACCENT)));
        },
    );
}

/// The switch between the current air's figures and the vacuum's (the
/// default, D120).
fn switch(ui: &mut egui::Ui, bar: &mut StageBar) {
    ui.horizontal(|ui| {
        for (air, label) in [(Air::Current, "CURRENT AIR"), (Air::Vacuum, "VACUUM")] {
            let on = bar.air == air;
            let text = RichText::new(label).small().color(if on { theme::ACCENT } else { theme::DIM });
            if ui.add(egui::Button::new(text).selected(on)).clicked() && !on {
                bar.air = air;
            }
        }
    });
}

/// One stage: its header, figures and icons; drops onto it edit the
/// sequence.
fn stage(ui: &mut egui::Ui, row: &Row, bar: &mut StageBar, out: &mut Vec<Action>) {
    let editable = super::rows::editable(row);
    let border = match row.place {
        Place::Next => theme::ACCENT,
        Place::Active => theme::GOOD,
        Place::Later | Place::Done => theme::BORDER,
    };
    let frame =
        egui::Frame::new().stroke(Stroke::new(1.0, border)).corner_radius(4).inner_margin(egui::Margin::same(4));
    let shown = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        header(ui, row, editable, bar, out);
        if let Some(f) = &row.figures {
            figures(ui, f);
        }
        icon_rows(ui, row, editable, bar, out);
    });
    let (Some(j), true) = (row.stage, editable) else { return };
    let r = &shown.response;
    if r.dnd_hover_payload::<Drag>().is_some_and(|d| drop_edit(*d, j).is_some()) {
        ui.painter().rect_stroke(r.rect, 4, Stroke::new(2.0, theme::ACCENT), StrokeKind::Inside);
    }
    if let Some(e) = r.dnd_release_payload::<Drag>().and_then(|d| drop_edit(*d, j)) {
        out.push(Action::Edit(e));
    }
}

/// A stage's number and name (dragged to move the stage), its place, the
/// next stage's run button and the delete with its confirmation. One line
/// high: a right-to-left layout straight in a column centres its content
/// in the column's whole remaining height, which stretched the active
/// stage (drawn outside the scroll area) down the screen's edge.
fn header(ui: &mut egui::Ui, row: &Row, editable: bool, bar: &mut StageBar, out: &mut Vec<Action>) {
    let colour = match row.place {
        Place::Next => theme::ACCENT,
        Place::Active => theme::GOOD,
        Place::Later => theme::TEXT,
        Place::Done => theme::DIM,
    };
    ui.horizontal(|ui| header_line(ui, row, editable, colour, bar, out));
    let Some(j) = row.stage.filter(|&j| bar.confirm == Some(j)) else { return };
    ui.horizontal(|ui| {
        ui.label(RichText::new("Delete stage?").small().color(theme::WARN));
        if ui.small_button("Delete").clicked() {
            out.push(Action::Edit(SequenceEdit::Delete(j as u32)));
            bar.confirm = None;
        }
        if ui.small_button("Keep").clicked() {
            bar.confirm = None;
        }
    });
}

/// The header's line, right to left.
fn header_line(
    ui: &mut egui::Ui,
    row: &Row,
    editable: bool,
    colour: Color32,
    bar: &mut StageBar,
    out: &mut Vec<Action>,
) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if let (true, Some(j)) = (editable, row.stage) {
            let x = ui.add(egui::Button::new(RichText::new("×").small().color(theme::DIM)).frame(false));
            if x.on_hover_text("Delete this stage").clicked() {
                bar.confirm = Some(j);
            }
        }
        if row.place == Place::Next {
            let go = ui.add(egui::Button::new(RichText::new("SPACE").small().color(theme::ACCENT)).small());
            if go.on_hover_text("Run the next stage").clicked() {
                out.push(Action::NextStage);
            }
        }
        if row.place == Place::Active {
            ui.label(RichText::new("ACTIVE").small().color(theme::GOOD));
            if let Some(j) = row.stage {
                again(ui, j, out);
            }
        }
        let number = row.number.map(|n| format!("{n}  ")).unwrap_or_default();
        let text = RichText::new(format!("{number}{}", row.name)).small().strong().color(colour);
        let label =
            ui.add(egui::Label::new(text).truncate().sense(if editable { Sense::drag() } else { Sense::hover() }));
        if let (true, Some(j)) = (editable, row.stage) {
            label.dnd_set_drag_payload(Drag::Stage(j));
        }
    });
}

/// A label on the left and a value on the right, small.
fn pair(ui: &mut egui::Ui, left: &str, right: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(left).small().color(theme::DIM));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(right).small().monospace());
        });
    });
}

/// A burn's figures: a thin bar per propellant, burnout and Δv, and the
/// active stage's thrust, TWR, mass and Isp.
fn figures(ui: &mut egui::Ui, f: &Figures) {
    for b in &f.bars {
        pair(ui, &b.fluid, &b.text);
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 3.0), Sense::hover());
        ui.painter().rect_filled(rect, 1, Color32::from_rgb(35, 44, 54));
        if let Some(fill) = b.fill {
            let full = egui::Rect::from_min_size(rect.min, vec2(rect.width() * fill, rect.height()));
            ui.painter().rect_filled(full, 1, theme::MUTED);
        }
    }
    pair(ui, "burnout", &f.burnout);
    pair(ui, "Δv", &f.dv);
    if let Some(a) = &f.active {
        pair(ui, "thrust", &a.thrust);
        pair(ui, "TWR", &a.twr);
        pair(ui, "mass", &a.mass);
        pair(ui, "mean Isp", &a.isp);
    }
}

/// The icons of what the stage's commands act on, wrapping in rows: a
/// click opens the part's card, a drag moves the command.
fn icon_rows(ui: &mut egui::Ui, row: &Row, editable: bool, bar: &mut StageBar, out: &mut Vec<Action>) {
    let all: Vec<(usize, &str, &icons::Icon)> =
        (row.items.iter()).flat_map(|it| it.icons.iter().map(move |i| (it.action, it.label.as_str(), i))).collect();
    if all.is_empty() {
        return;
    }
    let per_row = (((ui.available_width() + 2.0) / (SIZE + 2.0)).floor() as usize).max(1);
    let lines = all.len().div_ceil(per_row);
    let height = lines as f32 * (SIZE + 2.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    for (k, (action, label, icon)) in all.into_iter().enumerate() {
        let cell = icons::cell(rect.min, k, per_row);
        let id = Id::new(("stage_bar_icon", row.number, row.name.as_str(), k));
        let sense = if editable { Sense::click_and_drag() } else { Sense::click() };
        let resp = ui.interact(cell, id, sense);
        if resp.hovered() {
            ui.painter().rect_filled(cell.expand(1.0), 2, Color32::from_rgb(40, 52, 66));
        }
        icons::paint(ui.painter(), cell, icon.kind, icons::colour(icon.look));
        bar.icons.push((icon.part, cell));
        if let (true, Some(j)) = (editable, row.stage) {
            resp.dnd_set_drag_payload(Drag::Action { stage: j, action });
        }
        if resp.on_hover_text(format!("{label}\n{}", icon.id)).clicked() {
            out.push(Action::Open(icon.part));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::stage_bar::rows::Figures;
    use egui::{Pos2, RawInput, Rect};

    fn row(stage: usize, place: Place, figures: Option<Figures>) -> Row {
        Row { stage: Some(stage), number: Some(stage as u32), name: format!("S{stage}"), place, items: vec![], figures }
    }

    #[test]
    fn the_stages_to_run_take_the_room_the_rest_leaves() {
        // (column height, a stage active, stages run, room)
        let cases = [
            (735.0, false, 0, 705.0), // on the pad: all but the switch
            (735.0, true, 0, 495.0),  // in flight: the active stage's room kept
            (735.0, true, 3, 438.0),  // and a line per stage run
            (300.0, true, 5, MIN_ROOM),
        ];
        for (tall, active, done, want) in cases {
            assert_eq!(scroll_room(tall, active, done), want, "{tall} {active} {done}");
        }
    }

    /// The bar is as tall as its rows: an active stage (drawn below the
    /// scroll area) once stretched it the screen's whole height, empty.
    #[test]
    fn the_bar_is_as_tall_as_its_rows() {
        let figures = Figures { bars: vec![], burnout: "1m".into(), dv: "100 m/s".into(), active: None };
        let rows = [row(1, Place::Next, None), row(0, Place::Active, Some(figures))];
        let ctx = egui::Context::default();
        let mut state = StageBar::default();
        for _ in 0..4 {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
                ..Default::default()
            };
            ctx.begin_pass(input);
            bar(&ctx, &rows, &mut state);
            ctx.end_pass().drop_without_applying_deltas();
        }
        let rect = ctx.memory(|m| m.area_rect(Id::new(AREA))).expect("drawn");
        assert!(rect.height() < 250.0, "{rect:?}");
        assert!((rect.bottom() - (900.0 - layout::MARGIN)).abs() < 2.0, "{rect:?}");
    }
}
