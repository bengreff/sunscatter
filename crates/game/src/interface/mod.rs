//! The interface shell: the shared theme, the helper every HUD panel is drawn
//! with (a movable window whose place is saved), the Esc pause menu, and the
//! key help overlay. What is laid out where is pure data in [`layout`].

pub mod help;
pub mod layout;
pub mod pause;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use layout::{Anchor, InterfaceSettings, PanelId};

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

    pub fn panel_frame() -> Frame {
        Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(8))
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

fn align(anchor: Anchor) -> egui::Align2 {
    match anchor {
        Anchor::LeftTop => egui::Align2::LEFT_TOP,
        Anchor::RightTop => egui::Align2::RIGHT_TOP,
        Anchor::LeftBottom => egui::Align2::LEFT_BOTTOM,
        Anchor::CenterBottom => egui::Align2::CENTER_BOTTOM,
    }
}

/// Draws HUD panel `id` as a movable window without a title bar (drag it by
/// its background) at its saved place, or its default one; when the player
/// drags it, the new place is saved. Nothing is drawn if it is hidden.
pub fn panel<R>(
    ctx: &egui::Context,
    iface: &mut InterfaceSettings,
    id: PanelId,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    let saved = iface.panel(id);
    if !saved.visible {
        return None;
    }
    let screen = ctx.content_rect();
    let window_id = egui::Id::new(("hud_panel", id, iface.layout_generation));
    let last_size = ctx.data(|d| d.get_temp::<egui::Vec2>(window_id.with("size")));
    let mut window =
        egui::Window::new(id.title()).id(window_id).title_bar(false).resizable(false).frame(theme::panel_frame());
    window = match saved.pos {
        Some(p) => {
            let size = last_size.map_or([0.0, 0.0], |s| [s.x, s.y]);
            let [x, y] = layout::clamp_to_screen(p, size, [screen.width(), screen.height()]);
            window.pivot(egui::Align2::LEFT_TOP).default_pos([screen.min.x + x, screen.min.y + y])
        }
        None => {
            let (anchor, [ox, oy]) = id.default_place();
            let a = align(anchor);
            let corner = a.pos_in_rect(&screen);
            let inward = egui::vec2(
                match a.x() {
                    egui::Align::Min => ox,
                    egui::Align::Center => ox,
                    egui::Align::Max => -ox,
                },
                match a.y() {
                    egui::Align::Min => oy,
                    egui::Align::Center => oy,
                    egui::Align::Max => -oy,
                },
            );
            window.pivot(a).default_pos(corner + inward)
        }
    };
    let shown = window.show(ctx, add)?;
    let rect = shown.response.rect;
    ctx.data_mut(|d| d.insert_temp(window_id.with("size"), rect.size()));
    // Saved only while the pointer is down (the player dragging), so egui
    // keeping windows on a smaller screen does not overwrite the choice.
    let prev = ctx.data(|d| d.get_temp::<egui::Pos2>(window_id.with("pos")));
    ctx.data_mut(|d| d.insert_temp(window_id.with("pos"), rect.min));
    if prev.is_some_and(|p| p != rect.min) && ctx.input(|i| i.pointer.any_down()) {
        iface.set_pos(id, [rect.min.x - screen.min.x, rect.min.y - screen.min.y]);
    }
    shown.inner
}
