//! One renderer for every menu (D080): a panel with the menu's title, its
//! summary, and a header per section that expands on click; the docks that
//! stack the open menus at their designated places; the menu bar. Labels
//! are dim small monospace, values monospace and right-aligned in the
//! tone's colour.

use super::{Dock, Item, MenuAction, MenuId, MenuSettings, Section, Tone, SUMMARY};
use crate::interface::layout::{self, DockPlace, BAR_AREA, DOCK_WIDTH, GAP};
use crate::interface::theme;
use bevy_egui::egui::{self, pos2, vec2, Align, Align2, Color32, Layout, RichText, Sense, Stroke};

/// Height of a section header (px).
const HEADER_PX: f32 = 20.0;
/// Width of a pick list's box.
const CHOICE_PX: f32 = 170.0;

/// What the player did in a menu this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuInput {
    pub close: bool,
    /// Sections whose header was clicked.
    pub toggled: Vec<&'static str>,
    pub actions: Vec<MenuAction>,
}

pub fn tone_colour(t: Tone) -> Color32 {
    match t {
        Tone::Normal => theme::TEXT,
        Tone::Dim => theme::DIM,
        Tone::Accent => theme::ACCENT,
        Tone::Good => theme::GOOD,
        Tone::Warn => theme::WARN,
        Tone::Alert => theme::ALERT,
    }
}

/// The egui id of a section's header.
pub fn header_id(menu: MenuId, section: &str) -> egui::Id {
    egui::Id::new(("menu_section", menu, section))
}

/// Draws one menu: the title bar, the summary, then each section's header
/// and, when expanded, its items.
pub fn menu(ui: &mut egui::Ui, menu: MenuId, sections: &[Section], settings: &MenuSettings) -> MenuInput {
    let mut input = MenuInput::default();
    ui.spacing_mut().item_spacing = vec2(6.0, 2.0);
    // A section's items sit just inside its header's text (egui's default
    // indent is 18 px, a lot in a 300 px dock).
    ui.spacing_mut().indent = 10.0;
    ui.horizontal(|ui| {
        ui.label(RichText::new(menu.title()).monospace().strong().color(theme::ACCENT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let close = egui::Button::new(RichText::new("×").color(theme::DIM)).frame(false);
            if ui.add(close).on_hover_text(format!("Close ({})", menu.key_label())).clicked() {
                input.close = true;
            }
            ui.label(RichText::new(menu.key_label()).monospace().small().color(theme::DIM));
        });
    });
    ui.add_space(2.0);
    for s in sections {
        if s.id == SUMMARY {
            items(ui, menu, s, &mut input);
            continue;
        }
        let open = settings.is_expanded(menu, s.id);
        if header(ui, header_id(menu, s.id), &s.title, open) {
            input.toggled.push(s.id);
        }
        if open {
            ui.indent(("menu_indent", menu, s.id), |ui| items(ui, menu, s, &mut input));
        }
    }
    input
}

/// A full-width section header with a disclosure triangle; true when
/// clicked.
fn header(ui: &mut egui::Ui, id: egui::Id, title: &str, open: bool) -> bool {
    ui.add_space(3.0);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, HEADER_PX), Sense::hover());
    let response = ui.interact(rect, id, Sense::click());
    let painter = ui.painter();
    painter.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, theme::BORDER));
    let colour = if response.hovered() || open { theme::TEXT } else { theme::DIM };
    // The triangle: pointing right when closed, down when open.
    let c = pos2(rect.left() + 6.0, rect.center().y + 1.0);
    let pts = if open {
        vec![c + vec2(-4.0, -2.5), c + vec2(4.0, -2.5), c + vec2(0.0, 3.0)]
    } else {
        vec![c + vec2(-2.5, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.5, 4.0)]
    };
    painter.add(egui::Shape::convex_polygon(pts, colour, Stroke::NONE));
    let font = egui::FontId::monospace(11.0);
    painter.text(pos2(rect.left() + 16.0, rect.center().y + 1.0), Align2::LEFT_CENTER, title, font, colour);
    response.clicked()
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
            Item::Heading(text, tone) => {
                ui.add_space(3.0);
                ui.label(RichText::new(text).monospace().small().strong().color(tone_colour(*tone)));
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

/// Draws a dock's open menus stacked from `place`, scrolling past its
/// height ([`layout::stack`]); returns what the player did in each.
pub fn dock(
    ctx: &egui::Context,
    dock: Dock,
    place: DockPlace,
    menus: &[(MenuId, Vec<Section>)],
    settings: &MenuSettings,
) -> Vec<(MenuId, MenuInput)> {
    let id = egui::Id::new(("menu_dock", dock == Dock::Left));
    let heights_id = id.with("heights");
    let last: Vec<f32> = ctx.data(|d| d.get_temp(heights_id)).unwrap_or_default();
    let stack = layout::stack(&last, place.max_height);
    let mut out = Vec::new();
    let mut heights = Vec::new();
    egui::Area::new(id).fixed_pos(pos2(place.x, place.top)).show(ctx, |ui| {
        ui.set_width(DOCK_WIDTH);
        let mut body = |ui: &mut egui::Ui| {
            for (k, (m, sections)) in menus.iter().enumerate() {
                if k > 0 {
                    ui.add_space(GAP);
                }
                let shown = theme::panel_frame().show(ui, |ui| {
                    ui.set_width(inner_width());
                    menu(ui, *m, sections, settings)
                });
                heights.push(shown.response.rect.height());
                out.push((*m, shown.inner));
            }
        };
        if stack.scrolls {
            egui::ScrollArea::vertical().id_salt(id).max_height(place.max_height).show(ui, |ui| body(ui));
        } else {
            body(ui);
        }
    });
    ctx.data_mut(|d| d.insert_temp(heights_id, heights));
    out
}

/// The menu bar, top centre: each menu's name and key, lit when open;
/// returns the menu clicked.
pub fn bar(ctx: &egui::Context, settings: &MenuSettings) -> Option<MenuId> {
    let mut clicked = None;
    egui::Area::new(egui::Id::new(BAR_AREA)).anchor(Align2::CENTER_TOP, [0.0, layout::MARGIN]).show(ctx, |ui| {
        theme::bar_frame().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for m in MenuId::ALL {
                    let open = settings.is_open(m);
                    let name =
                        RichText::new(m.short()).monospace().small().color(if open { theme::TEXT } else { theme::DIM });
                    let key = RichText::new(m.key_label()).monospace().small().color(theme::DIM);
                    let button = egui::Button::new((name, key)).selected(open).frame_when_inactive(false);
                    let verb = if open { "Close" } else { "Open" };
                    if ui.add(button).on_hover_text(format!("{verb} {} ({})", m.title(), m.key_label())).clicked() {
                        clicked = Some(m);
                    }
                }
            });
        });
    });
    clicked
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
    fn frame(ctx: &egui::Context, settings: &MenuSettings, events: Vec<Event>) -> MenuInput {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
            events,
            ..Default::default()
        };
        let mut out = MenuInput::default();
        ctx.begin_pass(input);
        egui::Area::new(egui::Id::new("test_menu")).fixed_pos(pos2(10.0, 10.0)).show(ctx, |ui| {
            ui.set_width(280.0);
            out = menu(ui, MenuId::Orbital, &sections(), settings);
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
    fn a_header_click_toggles_its_section_and_collapsed_items_are_not_drawn() {
        let ctx = egui::Context::default();
        let mut settings = MenuSettings::default();
        // An area's first frame only measures it: not hit yet.
        frame(&ctx, &settings, vec![]);
        frame(&ctx, &settings, vec![]);
        let header = ctx.read_response(header_id(MenuId::Orbital, "elements")).expect("the header is drawn");
        let [press, release] = click(header.rect.center());
        frame(&ctx, &settings, press);
        let input = frame(&ctx, &settings, release);
        assert_eq!(input.toggled, ["elements"]);
        assert!(!input.close && input.actions.is_empty());
        // Collapsed: the target's button is not drawn, so it cannot be hit.
        let target = ctx.read_response(header_id(MenuId::Orbital, "target")).expect("drawn");
        let below = target.rect.center() + vec2(0.0, HEADER_PX);
        let [press, release] = click(below);
        frame(&ctx, &settings, press);
        assert!(frame(&ctx, &settings, release).actions.is_empty());
        // Expanded, the button acts.
        settings.toggle_section(MenuId::Orbital, "target");
        frame(&ctx, &settings, vec![]);
        frame(&ctx, &settings, vec![]);
        let target = ctx.read_response(header_id(MenuId::Orbital, "target")).expect("drawn");
        let [press, release] = click(target.rect.center() + vec2(0.0, HEADER_PX));
        frame(&ctx, &settings, press);
        assert_eq!(frame(&ctx, &settings, release).actions, [MenuAction::OpenPlanner]);
    }
}
