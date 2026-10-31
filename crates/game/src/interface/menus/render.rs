//! One renderer for every menu (D080, D111): a panel with the menu's
//! title, its summary, and every section under its heading (flat: nothing
//! collapses); the left dock that stacks the open menus, each with a fair
//! share of its height and scrolling within it; the menu bar. Labels
//! are dim small monospace, values monospace and right-aligned in the
//! tone's colour.

use super::{Item, MenuAction, MenuId, MenuSettings, Section, Tone, SUMMARY};
use crate::interface::layout::{self, DockPlace, BAR_AREA, DOCK_WIDTH, GAP};
use crate::interface::theme;
use bevy_egui::egui::{self, pos2, vec2, Align, Align2, Color32, Layout, RichText, Sense, Stroke};

/// Height of a section heading (px).
const HEADER_PX: f32 = 18.0;
/// Width of a pick list's box.
const CHOICE_PX: f32 = 170.0;

/// What the player did in a menu this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuInput {
    pub close: bool,
    pub actions: Vec<MenuAction>,
}

pub fn tone_colour(t: Tone) -> Color32 {
    match t {
        Tone::Normal => theme::TEXT,
        Tone::Dim => theme::DIM,
        Tone::Good => theme::GOOD,
        Tone::Warn => theme::WARN,
        Tone::Alert => theme::ALERT,
    }
}

/// Draws one menu: the title bar, the summary, then each section under its
/// heading (the dock draws the two parts itself, the body scrolling).
#[cfg(test)]
pub fn menu(ui: &mut egui::Ui, menu: MenuId, sections: &[Section]) -> MenuInput {
    let mut input = header(ui, menu);
    body(ui, menu, sections, &mut input);
    input
}

/// A menu's title line (its key, its close button) and the spacing of
/// what follows.
fn header(ui: &mut egui::Ui, menu: MenuId) -> MenuInput {
    let mut input = MenuInput::default();
    ui.spacing_mut().item_spacing = vec2(6.0, 2.0);
    // A section's items sit just inside its header's text (egui's default
    // indent is 18 px, a lot in a 300 px dock).
    ui.spacing_mut().indent = 10.0;
    ui.horizontal(|ui| {
        ui.label(RichText::new(menu.title()).monospace().strong().color(theme::ACCENT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let close = egui::Button::new(RichText::new("×").color(theme::DIM)).frame(false);
            // The RCS menu follows RCS: closing it turns RCS off.
            let hover = if menu == MenuId::Rcs { "RCS off (R)".to_string() } else { menu.with_key("Close") };
            if ui.add(close).on_hover_text(hover).clicked() {
                input.close = true;
            }
            ui.label(RichText::new(menu.key_label()).monospace().small().color(theme::DIM));
        });
    });
    ui.add_space(2.0);
    input
}

/// A menu's sections, each under its heading.
fn body(ui: &mut egui::Ui, menu: MenuId, sections: &[Section], input: &mut MenuInput) {
    for s in sections {
        if s.id != SUMMARY && !s.title.is_empty() {
            heading(ui, &s.title);
        }
        items(ui, menu, s, input);
    }
}

/// A full-width section heading: a rule and the title, dim, wrapped onto
/// a second line rather than cut where it is wider than the menu (a long
/// body's name, or the menu's scroll bar taking its room).
fn heading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(3.0);
    let width = ui.available_width();
    let font = egui::FontId::monospace(11.0);
    let galley = ui.painter().layout(title.to_string(), font, theme::DIM, width);
    let (rect, _) = ui.allocate_exact_size(vec2(width, HEADER_PX.max(galley.size().y + 4.0)), Sense::hover());
    let painter = ui.painter();
    painter.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, theme::BORDER));
    let top = rect.center().y + 1.0 - galley.size().y / 2.0;
    painter.galley(pos2(rect.left(), top), galley, theme::DIM);
}

fn items(ui: &mut egui::Ui, menu: MenuId, s: &Section, input: &mut MenuInput) {
    for item in &s.items {
        match item {
            Item::Row(r) => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&r.label).monospace().small().color(theme::DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let value = RichText::new(&r.value).monospace().color(tone_colour(r.tone));
                        ui.add(egui::Label::new(value).truncate());
                    });
                });
            }
            Item::Note(text, tone) => {
                ui.add(egui::Label::new(RichText::new(text).small().color(tone_colour(*tone))).wrap());
            }
            Item::Choice { label, current, options } => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).monospace().small().color(theme::DIM));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        egui::ComboBox::from_id_salt(("menu_choice", menu, s.id, label))
                            .selected_text(RichText::new(current).monospace())
                            .width(CHOICE_PX)
                            .show_ui(ui, |ui| {
                                for (name, selected, action) in options {
                                    if ui.selectable_label(*selected, name).clicked() {
                                        input.actions.push(action.clone());
                                    }
                                }
                            });
                    });
                });
            }
            Item::Toggle { label, on, value, tone, action } => {
                ui.horizontal(|ui| {
                    let mut checked = *on;
                    let text = RichText::new(label).monospace().small().color(theme::DIM);
                    if ui.checkbox(&mut checked, text).clicked() {
                        input.actions.push(action.clone());
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let value = RichText::new(value).monospace().small().color(tone_colour(*tone));
                        ui.add(egui::Label::new(value).truncate());
                    });
                });
            }
            Item::Button { label, action } => {
                ui.add_space(2.0);
                let button = egui::Button::new(RichText::new(label).small()).min_size(vec2(ui.available_width(), 0.0));
                if ui.add(button).clicked() {
                    input.actions.push(action.clone());
                }
            }
        }
    }
}

/// The inner width of a menu panel in a dock.
fn inner_width() -> f32 {
    let m = theme::panel_frame().inner_margin;
    DOCK_WIDTH - f32::from(m.left) - f32::from(m.right) - 2.0
}

/// The egui area of the dock.
pub fn dock_area() -> egui::Id {
    egui::Id::new("menu_dock")
}

/// The egui memory of a docked menu's natural height (last frame's).
fn natural_id(m: MenuId) -> egui::Id {
    dock_area().with(("natural", m))
}

/// Draws the open menus stacked from `place`, each with its fair share of
/// the dock's height (`layout::fair_heights`, from each one's natural
/// height last frame): a short menu whole, a long one scrolling within
/// its share under its fixed title. The whole dock also scrolls, should
/// the titles alone not fit.
pub fn dock(ctx: &egui::Context, place: DockPlace, menus: &[(MenuId, Vec<Section>)]) -> Vec<(MenuId, MenuInput)> {
    let mut out = Vec::new();
    let natural: Vec<f32> =
        menus.iter().map(|(m, _)| ctx.data(|d| d.get_temp(natural_id(*m))).unwrap_or(f32::INFINITY)).collect();
    let gaps = GAP * menus.len().saturating_sub(1) as f32;
    let heights = layout::fair_heights(&natural, place.max_height - gaps);
    egui::Area::new(dock_area()).fixed_pos(pos2(place.x, place.top)).show(ctx, |ui| {
        ui.set_width(DOCK_WIDTH);
        // An area lays out within its remembered size (egui's default area
        // is 400 px tall): the scroll area could not grow past it, and the
        // dock was cut at 400 px with room to spare.
        ui.set_max_height(place.max_height);
        egui::ScrollArea::vertical().id_salt("menu_dock_scroll").max_height(place.max_height).show(ui, |ui| {
            ui.set_width(DOCK_WIDTH);
            for (k, (m, sections)) in menus.iter().enumerate() {
                if k > 0 {
                    ui.add_space(GAP);
                }
                let top = ui.cursor().top();
                let shown = theme::panel_frame().show(ui, |ui| {
                    ui.set_width(inner_width());
                    let mut input = header(ui, *m);
                    // Above the body: the frame's top and the title; below it, the frame's bottom.
                    let frame = theme::panel_frame();
                    let bottom = f32::from(frame.inner_margin.bottom) + frame.stroke.width;
                    let chrome = ui.cursor().top() - top + bottom;
                    let room = (heights[k] - chrome).max(40.0);
                    let body_out = egui::ScrollArea::vertical()
                        .id_salt(("menu_dock_body", *m))
                        .max_height(room)
                        .show(ui, |ui| body(ui, *m, sections, &mut input));
                    (input, body_out.content_size.y - body_out.inner_rect.height())
                });
                let (input, hidden) = shown.inner;
                let natural = shown.response.rect.height() + hidden.max(0.0);
                ui.ctx().data_mut(|d| d.insert_temp(natural_id(*m), natural));
                out.push((*m, input));
            }
        });
    });
    out
}

/// The menu bar, top centre: each menu's name in normal type with its key
/// as a small badge, an open menu lit (an amber wash and underline);
/// returns the menu clicked.
pub fn bar(ctx: &egui::Context, settings: &MenuSettings) -> Option<BarClick> {
    let mut clicked = None;
    egui::Area::new(egui::Id::new(BAR_AREA)).anchor(Align2::CENTER_TOP, [0.0, layout::MARGIN]).show(ctx, |ui| {
        theme::bar_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for m in MenuId::BAR {
                    let open = settings.is_open(m);
                    let verb = if open { "Close" } else { "Open" };
                    let r = bar_item(ui, m, open).on_hover_text(m.with_key(&format!("{verb} {}", m.title())));
                    if r.clicked() {
                        clicked = Some(BarClick::Menu(m));
                    }
                }
                // The help: every key and every way in, by click too.
                let help = crate::commands::keys::label(crate::commands::keys::Action::Help);
                let text = RichText::new(" ? ").monospace().size(13.0).color(theme::MUTED);
                let r = ui.add(egui::Button::new(text).frame(false));
                if r.on_hover_text(format!("Help: every key and every way in ({help})")).clicked() {
                    clicked = Some(BarClick::Help);
                }
            });
        });
    });
    clicked
}

/// What a click on the menu bar asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarClick {
    Menu(MenuId),
    Help,
}

/// The egui id of a menu's item in the bar.
pub fn bar_item_id(m: MenuId) -> egui::Id {
    egui::Id::new(("menu_bar_item", m))
}

/// One item of the bar: `NAME [K]`.
fn bar_item(ui: &mut egui::Ui, m: MenuId, open: bool) -> egui::Response {
    const PAD: f32 = 9.0;
    let name_colour = if open { theme::TEXT } else { theme::MUTED };
    let name = ui.painter().layout_no_wrap(m.short().to_string(), egui::FontId::monospace(13.0), name_colour);
    let key = ui.painter().layout_no_wrap(m.key_label().to_string(), egui::FontId::monospace(10.0), theme::DIM);
    let badge = vec2(key.size().x + 7.0, key.size().y + 3.0);
    let size = vec2(PAD + name.size().x + 6.0 + badge.x + PAD, 26.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let response = ui.interact(rect, bar_item_id(m), Sense::click());
    let painter = ui.painter();
    if open {
        painter.rect_filled(rect, 4.0, theme::ACCENT.gamma_multiply(0.16));
        let y = rect.bottom() - 1.5;
        painter
            .line_segment([pos2(rect.left() + 5.0, y), pos2(rect.right() - 5.0, y)], Stroke::new(2.0, theme::ACCENT));
    } else if response.hovered() {
        painter.rect_filled(rect, 4.0, Color32::from_white_alpha(14));
    }
    let name_pos = pos2(rect.left() + PAD, rect.center().y - name.size().y * 0.5);
    let badge_rect =
        egui::Rect::from_min_size(pos2(name_pos.x + name.size().x + 6.0, rect.center().y - badge.y * 0.5), badge);
    painter.galley(name_pos, name, name_colour);
    painter.rect_stroke(badge_rect, 3.0, Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
    painter.galley(badge_rect.center() - key.size() * 0.5, key, theme::DIM);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, Pos2, RawInput, Rect};

    fn sections() -> Vec<Section> {
        let mut summary = Section::summary();
        summary.row("APOAPSIS", "301.2 km");
        let mut elements = Section::new("elements", "ELEMENTS");
        elements.row("SEMI-MAJOR AXIS", "6668.0 km");
        let mut target = Section::new("target", "TARGET");
        target.button("Pick", MenuAction::OpenPlanner);
        vec![summary, elements, target]
    }

    /// One frame drawing the Orbital menu in a fixed area, with `events`.
    fn frame(ctx: &egui::Context, events: Vec<Event>) -> MenuInput {
        frame_of(ctx, &sections(), events)
    }

    fn frame_of(ctx: &egui::Context, sections: &[Section], events: Vec<Event>) -> MenuInput {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
            events,
            ..Default::default()
        };
        let mut out = MenuInput::default();
        ctx.begin_pass(input);
        egui::Area::new(egui::Id::new("test_menu")).fixed_pos(pos2(10.0, 10.0)).show(ctx, |ui| {
            ui.set_width(280.0);
            out = menu(ui, MenuId::Orbital, sections);
        });
        ctx.end_pass().drop_without_applying_deltas();
        out
    }

    fn click(at: Pos2) -> [Vec<Event>; 2] {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        [vec![Event::PointerMoved(at), button(true)], vec![button(false)]]
    }

    #[test]
    fn a_click_on_the_bar_names_its_menu() {
        let ctx = egui::Context::default();
        let settings = MenuSettings::default();
        let run = |events: Vec<Event>| {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
                events,
                ..Default::default()
            };
            ctx.begin_pass(input);
            let clicked = bar(&ctx, &settings);
            ctx.end_pass().drop_without_applying_deltas();
            clicked
        };
        // Centred on its own width: settled after a few frames.
        for _ in 0..4 {
            run(vec![]);
        }
        let item = ctx.read_response(bar_item_id(MenuId::Comms)).expect("drawn");
        assert!(item.rect.height() >= 24.0, "readable: {:?}", item.rect);
        let [press, release] = click(item.rect.center());
        run(press);
        assert_eq!(run(release), Some(BarClick::Menu(MenuId::Comms)));
    }

    /// Flat pages: every section's items are drawn, so the last
    /// section's button acts at once.
    #[test]
    fn every_section_is_drawn_and_its_buttons_act() {
        let ctx = egui::Context::default();
        // An area's first frame only measures it: not hit yet.
        frame(&ctx, vec![]);
        frame(&ctx, vec![]);
        let button = ctx.read_response(egui::Id::new("test_menu")).map(|r| r.rect).expect("drawn");
        // The button is the panel's last line.
        let at = pos2(button.center().x, button.bottom() - 8.0);
        let [press, release] = click(at);
        frame(&ctx, press);
        assert_eq!(frame(&ctx, release).actions, [MenuAction::OpenPlanner]);
    }

    /// A switch acts on a click on its box, not on drawing.
    #[test]
    fn a_switch_acts_when_clicked() {
        let ctx = egui::Context::default();
        let mut s = Section::new("blocks", "THRUSTERS");
        let flip = MenuAction::SetRcs(sim::systems::rcs::RcsMode::default());
        s.toggle("RCS#1", true, "on", Tone::Good, flip.clone());
        let all = [s];
        frame_of(&ctx, &all, vec![]);
        assert!(frame_of(&ctx, &all, vec![]).actions.is_empty(), "drawing does nothing");
        let panel = ctx.read_response(egui::Id::new("test_menu")).map(|r| r.rect).expect("drawn");
        // The box is at the left of the panel's last line.
        let [press, release] = click(pos2(panel.left() + 6.0, panel.bottom() - 8.0));
        frame_of(&ctx, &all, press);
        assert_eq!(frame_of(&ctx, &all, release).actions, [flip]);
    }

    /// The dock takes the height its place gives, not egui's default area
    /// height (400 px): menus taller than that showed only their top.
    #[test]
    fn the_dock_uses_its_whole_height() {
        let ctx = egui::Context::default();
        let mut tall = Section::new("tall", "TALL");
        for k in 0..80 {
            tall.row(format!("ROW {k}"), "1");
        }
        let menus = [(MenuId::Orbital, vec![tall])];
        let place = DockPlace { x: 10.0, top: 50.0, max_height: 700.0 };
        for _ in 0..4 {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 900.0))),
                ..Default::default()
            };
            ctx.begin_pass(input);
            dock(&ctx, place, &menus);
            ctx.end_pass().drop_without_applying_deltas();
        }
        let rect = ctx.memory(|m| m.area_rect(dock_area())).expect("drawn");
        assert!((rect.height() - 700.0).abs() < 2.0, "{rect:?}");
    }
}
