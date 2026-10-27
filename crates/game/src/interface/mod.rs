//! The interface shell: the shared theme, the helper every fixed HUD panel
//! is drawn with, the task menus, the Esc pause menu, and the key help
//! overlay. Where everything goes is pure rules in [`layout`] (D080:
//! designated places, nothing is dragged).

pub mod help;
pub mod layout;
pub mod menus;
pub mod mini_panel;
pub mod pause;
pub mod toasts;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use layout::InterfaceSettings;

/// Panel background, border and text colours: dark, slightly translucent,
/// one accent (clean sci-fi, function first).
pub mod theme {
    use bevy_egui::egui::{Color32, CornerRadius, Frame, Margin, Stroke};

    pub const PANEL: Color32 = Color32::from_rgba_premultiplied(7, 10, 15, 210);
    pub const BORDER: Color32 = Color32::from_rgba_premultiplied(60, 80, 100, 90);
    pub const DIM: Color32 = Color32::from_rgb(130, 150, 170);
    pub const TEXT: Color32 = Color32::from_rgb(225, 235, 245);
    pub const ACCENT: Color32 = Color32::from_rgb(255, 170, 40);
    pub const WARN: Color32 = Color32::from_rgb(255, 120, 100);
    /// Destruction and failures: stronger than a warning.
    pub const ALERT: Color32 = Color32::from_rgb(255, 64, 52);
    /// Working as it should (a canopy open, safe to deploy).
    pub const GOOD: Color32 = Color32::from_rgb(120, 205, 145);
    /// Text that is there but not active (a closed menu's name).
    pub const MUTED: Color32 = Color32::from_rgb(180, 194, 208);

    pub fn panel_frame() -> Frame {
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(8))
    }

    /// The menu bar's frame: the panel's, slimmer.
    pub fn bar_frame() -> Frame {
        panel_frame().inner_margin(Margin::symmetric(6, 4))
    }
}

/// Applies the theme and the UI scale (every frame is cheap: egui only
/// rebuilds when they change).
pub fn apply_style(mut contexts: EguiContexts, iface: Res<InterfaceSettings>, mut styled: Local<bool>) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !*styled {
        *styled = true;
        let mut v = egui::Visuals::dark();
        v.window_fill = theme::PANEL;
        v.panel_fill = theme::PANEL;
        v.window_stroke = egui::Stroke::new(1.0, theme::BORDER);
        v.window_corner_radius = egui::CornerRadius::same(8);
        v.selection.bg_fill = egui::Color32::from_rgb(150, 95, 20);
        v.override_text_color = Some(theme::TEXT);
        ctx.set_visuals(v);
    }
    if (ctx.zoom_factor() - iface.ui_scale).abs() > 1e-3 {
        ctx.set_zoom_factor(iface.ui_scale);
    }
    Ok(())
}

/// Where the HUD's panels were drawn last frame (the navball cluster, the
/// docks with menus in them, the time panel, the menu bar), for marks that
/// must not show through them.
pub fn hud_rects(ctx: &egui::Context) -> Vec<egui::Rect> {
    let ids = [
        egui::Id::new(crate::navball::AREA),
        menus::render::dock_area(menus::Dock::Left),
        menus::render::dock_area(menus::Dock::Right),
        egui::Id::new(layout::TIME_AREA),
        egui::Id::new(layout::BAR_AREA),
    ];
    ctx.memory(|m| {
        let shown = |id: egui::Id| m.areas().visible_last_frame(&egui::LayerId::new(egui::Order::Middle, id));
        ids.into_iter().filter(|id| shown(*id)).filter_map(|id| m.area_rect(id)).collect()
    })
}

/// Draws a HUD panel at its designated place (D080): an egui area with the
/// panel frame, anchored `offset` from a screen corner or edge (`align`).
/// It is not dragged, and its area id (`id`) lets the docks keep clear.
pub fn fixed_panel<R>(
    ctx: &egui::Context,
    id: &'static str,
    align: egui::Align2,
    offset: [f32; 2],
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Area::new(egui::Id::new(id))
        .anchor(align, offset)
        .movable(false)
        .show(ctx, |ui| theme::panel_frame().show(ui, add).inner)
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2, Event, PointerButton, Pos2, RawInput, Rect};

    /// Runs one egui frame drawing the time panel's place, with `events`;
    /// returns where it is.
    fn frame(ctx: &egui::Context, events: Vec<Event>) -> Rect {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 800.0))),
            events,
            ..Default::default()
        };
        ctx.begin_pass(input);
        fixed_panel(ctx, layout::TIME_AREA, egui::Align2::LEFT_TOP, [layout::MARGIN; 2], |ui| {
            ui.set_min_size(vec2(120.0, 60.0));
        });
        ctx.end_pass().drop_without_applying_deltas();
        ctx.memory(|m| m.area_rect(egui::Id::new(layout::TIME_AREA))).expect("drawn")
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    #[test]
    fn a_fixed_panel_stays_in_its_place_when_dragged() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![]);
        let before = frame(&ctx, vec![]);
        assert_eq!(before.min, pos2(layout::MARGIN, layout::MARGIN));
        // Grab the panel's background at (40, 40) and drag it by (200, 100).
        let start = pos2(40.0, 40.0);
        frame(&ctx, vec![Event::PointerMoved(start), button(start, true)]);
        for k in 1..=10 {
            frame(&ctx, vec![Event::PointerMoved(start + vec2(20.0, 10.0) * k as f32)]);
        }
        let after = frame(&ctx, vec![button(start + vec2(200.0, 100.0), false)]);
        assert_eq!(after, before, "designated places: nothing is dragged");
    }
}
