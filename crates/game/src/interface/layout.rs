//! Interface settings and the flight view's layout rules (D080), as pure
//! data and functions: the UI scale, the navball size, which menus are open
//! and which sections expanded (persisted in `settings.ron`, `interface`),
//! and where the fixed panels and the docks go for a window size.
//!
//! The places are designated, nothing is dragged: the time panel top left,
//! the menu bar top centre, the navball cluster bottom centre, flight-task
//! menus in the left dock and the ship's in the right one, each stacking
//! its open menus top to bottom and scrolling when they do not fit above
//! the cluster.

use crate::interface::menus::{Dock, MenuSettings};
use bevy::prelude::Resource;
use bevy_egui::egui::Rect;
use serde::{Deserialize, Serialize};

/// Space between a panel and the window's edge (logical px).
pub const MARGIN: f32 = 10.0;
/// Space between panels.
pub const GAP: f32 = 8.0;
/// A dock's width.
pub const DOCK_WIDTH: f32 = 300.0;
/// A dock never gets shorter than this before it scrolls.
pub const MIN_DOCK_HEIGHT: f32 = 120.0;
/// The egui areas of the fixed panels (the docks keep clear of them).
pub const TIME_AREA: &str = "hud_time";
pub const BAR_AREA: &str = "menu_bar";

/// Interface preferences (the settings screen's Interface tab).
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceSettings {
    /// UI zoom on top of the display's own scale.
    pub ui_scale: f32,
    /// Navball diameter (logical px).
    pub navball_size: f32,
    /// Frame rate shown in the time panel.
    pub show_fps: bool,
    /// The open menus and their expanded sections.
    pub menus: MenuSettings,
    /// The MCP server for the player's AI agent (D044, D069): off by default.
    pub agent: AgentSettings,
}

/// Where the player's agent connects (local HTTP only).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AgentSettings {
    pub enabled: bool,
    pub port: u16,
    /// Bearer token the agent must send; empty until first enabled.
    pub token: String,
}

impl Default for AgentSettings {
    fn default() -> Self {
        AgentSettings { enabled: false, port: 7878, token: String::new() }
    }
}

pub const UI_SCALE_RANGE: std::ops::RangeInclusive<f32> = 0.6..=2.0;
pub const NAVBALL_SIZE_RANGE: std::ops::RangeInclusive<f32> = 110.0..=240.0;

impl Default for InterfaceSettings {
    fn default() -> Self {
        InterfaceSettings {
            ui_scale: 1.0,
            navball_size: 150.0,
            show_fps: true,
            menus: MenuSettings::default(),
            agent: AgentSettings::default(),
        }
    }
}

impl InterfaceSettings {
    /// Values out of range (a hand-edited file) pulled back in.
    pub fn sanitized(mut self) -> Self {
        let clamp = |x: f32, r: &std::ops::RangeInclusive<f32>, d: f32| {
            if x.is_finite() {
                x.clamp(*r.start(), *r.end())
            } else {
                d
            }
        };
        self.ui_scale = clamp(self.ui_scale, &UI_SCALE_RANGE, 1.0);
        self.navball_size = clamp(self.navball_size, &NAVBALL_SIZE_RANGE, 150.0);
        self
    }
}

/// Where a dock goes: its left edge and top, and the height past which its
/// menus scroll.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockPlace {
    pub x: f32,
    pub top: f32,
    pub max_height: f32,
}

/// The top of a dock: below the time panel and the menu bar, and for the
/// right dock below the performance overlay (F4) when it shows, whichever
/// reaches lowest (rects as last drawn; `None` if not drawn).
pub fn dock_top(dock: Dock, time: Option<Rect>, bar: Option<Rect>, overlay: Option<Rect>) -> f32 {
    let overlay = overlay.filter(|_| dock == Dock::Right);
    let bottom = [time, bar, overlay].into_iter().flatten().fold(MARGIN, |b, r| b.max(r.max.y));
    bottom + GAP
}

/// A dock's place in `screen`: at the left or right margin, from `top` to
/// the bottom margin, or to above the navball cluster where the dock would
/// overlap it (a narrow window), never shorter than [`MIN_DOCK_HEIGHT`].
pub fn dock_place(dock: Dock, screen: Rect, top: f32, cluster: Option<Rect>) -> DockPlace {
    let x = match dock {
        Dock::Left => screen.min.x + MARGIN,
        Dock::Right => screen.max.x - MARGIN - DOCK_WIDTH,
    };
    let overlaps = |c: &Rect| x < c.max.x && x + DOCK_WIDTH > c.min.x;
    let bottom = match cluster.filter(overlaps) {
        Some(c) => c.min.y - GAP,
        None => screen.max.y - MARGIN,
    };
    DockPlace { x, top, max_height: (bottom - top).max(MIN_DOCK_HEIGHT) }
}

/// A dock's menus stacked top to bottom: where each starts (relative to the
/// dock's top), the stack's height, and whether it scrolls.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    pub tops: Vec<f32>,
    pub height: f32,
    pub scrolls: bool,
}

/// Stacks menus of `heights` with [`GAP`] between them.
pub fn stack(heights: &[f32], max_height: f32) -> Stack {
    let mut tops = Vec::with_capacity(heights.len());
    let mut y = 0.0;
    for (k, h) in heights.iter().enumerate() {
        if k > 0 {
            y += GAP;
        }
        tops.push(y);
        y += h;
    }
    Stack { tops, height: y, scrolls: y > max_height }
}

/// The top of the toasts: centred under the menu bar.
pub fn toasts_top(bar: Option<Rect>) -> f32 {
    bar.map_or(MARGIN + 40.0, |b| b.max.y + GAP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_egui::egui::{pos2, Rect};

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    }

    #[test]
    fn docks_start_below_the_top_panels_and_the_overlay_moves_only_the_right_one() {
        let time = Some(rect(10.0, 10.0, 260.0, 76.0));
        let bar = Some(rect(400.0, 10.0, 880.0, 42.0));
        let overlay = Some(rect(1000.0, 10.0, 1270.0, 120.0));
        // (dock, time, bar, overlay, top)
        let table = [
            (Dock::Left, time, bar, None, 84.0),
            (Dock::Right, time, bar, None, 84.0),
            (Dock::Left, time, bar, overlay, 84.0),
            (Dock::Right, time, bar, overlay, 128.0),
            (Dock::Left, None, None, None, MARGIN + GAP),
        ];
        for (dock, t, b, o, want) in table {
            assert_eq!(dock_top(dock, t, b, o), want, "{dock:?} {o:?}");
        }
    }

    #[test]
    fn docks_reach_the_bottom_unless_they_would_cover_the_navball_cluster() {
        let top = 84.0;
        // The cluster: ~470 px wide, centred, 250 px tall at the bottom.
        let cluster = |w: f32, h: f32| rect(w / 2.0 - 235.0, h - 258.0, w / 2.0 + 235.0, h - 8.0);
        // (window w, h, dock, x, max height, scrolls a 600 px stack)
        let table = [
            (1280.0, 800.0, Dock::Left, 10.0, 706.0, false),
            (1280.0, 800.0, Dock::Right, 970.0, 706.0, false),
            (1920.0, 1080.0, Dock::Right, 1610.0, 986.0, false),
            // Narrow: the docks overlap the cluster and stop above it.
            (900.0, 800.0, Dock::Left, 10.0, 450.0, true),
            (900.0, 800.0, Dock::Right, 590.0, 450.0, true),
            // Tiny: never shorter than the minimum.
            (700.0, 300.0, Dock::Left, 10.0, MIN_DOCK_HEIGHT, true),
        ];
        for (w, h, dock, x, max_height, scrolls) in table {
            let place = dock_place(dock, rect(0.0, 0.0, w, h), top, Some(cluster(w, h)));
            assert_eq!(place, DockPlace { x, top, max_height }, "{w}x{h} {dock:?}");
            assert_eq!(stack(&[350.0, 242.0], place.max_height).scrolls, scrolls, "{w}x{h} {dock:?}");
        }
        // Without the cluster drawn (the tracking station), to the bottom.
        assert_eq!(dock_place(Dock::Left, rect(0.0, 0.0, 900.0, 800.0), top, None).max_height, 706.0);
    }

    #[test]
    fn menus_stack_top_to_bottom_with_a_gap() {
        assert_eq!(stack(&[], 500.0), Stack { tops: vec![], height: 0.0, scrolls: false });
        assert_eq!(
            stack(&[200.0, 100.0, 50.0], 500.0),
            Stack { tops: vec![0.0, 208.0, 316.0], height: 366.0, scrolls: false }
        );
        assert!(stack(&[300.0, 300.0], 600.0).scrolls, "608 px past 600");
        assert!(!stack(&[300.0, 292.0], 600.0).scrolls, "exactly 600 fits");
        assert_eq!(toasts_top(Some(rect(400.0, 10.0, 880.0, 42.0))), 50.0);
    }

    #[test]
    fn round_trips_through_ron_and_sanitizes() {
        let mut s = InterfaceSettings { ui_scale: 1.25, ..Default::default() };
        s.menus.toggle(crate::interface::menus::MenuId::Landing);
        s.menus.toggle_section(crate::interface::menus::MenuId::Landing, "reentry");
        let text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<InterfaceSettings>(&text).unwrap(), s);
        let wild = InterfaceSettings { ui_scale: 40.0, navball_size: f32::NAN, ..Default::default() }.sanitized();
        assert_eq!((wild.ui_scale, wild.navball_size), (2.0, 150.0));
    }

    /// Files saved before D080 (movable panels) still load: their panel
    /// places are ignored.
    #[test]
    fn old_panel_layouts_are_ignored() {
        let old = "(ui_scale: 1.5, navball_size: 180.0, show_fps: false, layout_generation: 3, \
                   panels: {Time: (pos: Some((10.0, 20.0)), visible: true), Flight: (pos: None, visible: false), \
                   Target: (pos: None, visible: true)})";
        let s: InterfaceSettings = ron::from_str(old).expect("loads");
        assert_eq!((s.ui_scale, s.navball_size, s.show_fps), (1.5, 180.0, false));
        assert_eq!(s.menus, MenuSettings::default());
    }
}
