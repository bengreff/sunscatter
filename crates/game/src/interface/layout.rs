//! Interface settings and the flight view's layout rules (D080), as pure
//! data and functions: the UI scale, the navball size, which menus are open
//! (persisted in `settings.ron`, `interface`), and where the fixed panels
//! and the dock go for a window size.
//!
//! The places are designated, nothing is dragged: the time panel top left,
//! the menu bar top centre, the navball cluster bottom centre with the
//! action bar above it, the staging bar on the right edge
//! (`interface::stage_bar`), every open menu in the left dock, stacked
//! top to bottom and scrolling within the dock's height, and the
//! notifications window (`interface::toasts`) under the dock, bottom left.

use crate::interface::menus::MenuSettings;
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
    /// The open menus.
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

/// The top of the dock: below the time panel and the menu bar, whichever
/// reaches lowest (rects as last drawn; `None` if not drawn).
pub fn dock_top(time: Option<Rect>, bar: Option<Rect>) -> f32 {
    let bottom = [time, bar].into_iter().flatten().fold(MARGIN, |b, r| b.max(r.max.y));
    bottom + GAP
}

/// The dock's place in `screen`: at the left margin, from `top` to the
/// bottom margin, or to above the notifications window (`corner`, bottom
/// left) and the navball cluster where the dock would overlap them (the
/// cluster in a narrow window), never shorter than [`MIN_DOCK_HEIGHT`].
/// Its menus scroll within that height.
pub fn dock_place(screen: Rect, top: f32, cluster: Option<Rect>, corner: Option<Rect>) -> DockPlace {
    let x = screen.min.x + MARGIN;
    let overlaps = |c: &Rect| x < c.max.x && x + DOCK_WIDTH > c.min.x;
    let bottom = [cluster, corner]
        .into_iter()
        .flatten()
        .filter(overlaps)
        .fold(screen.max.y - MARGIN, |b, c| b.min(c.min.y - GAP));
    DockPlace { x, top, max_height: (bottom - top).max(MIN_DOCK_HEIGHT) }
}

/// Whether a mark at `r` falls behind any of the HUD's panels (`hud`, as
/// last drawn): such a mark is not drawn, so it does not show through
/// their translucent backgrounds.
pub fn covered(r: Rect, hud: &[Rect]) -> bool {
    hud.iter().any(|h| h.intersects(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_egui::egui::{pos2, Rect};

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    }

    #[test]
    fn the_dock_starts_below_the_top_panels() {
        let time = Some(rect(10.0, 10.0, 260.0, 76.0));
        let bar = Some(rect(400.0, 10.0, 880.0, 42.0));
        // (time, bar, top)
        let table = [(time, bar, 84.0), (None, bar, 50.0), (None, None, MARGIN + GAP)];
        for (t, b, want) in table {
            assert_eq!(dock_top(t, b), want, "{t:?} {b:?}");
        }
    }

    /// The dock (table): from its top to the bottom margin at 1600×900 and
    /// the reference Mac (3024×1964 physical, 1512×982 logical at scale
    /// 2), above the navball cluster where it would cover it; its menus
    /// scroll within that height whatever they hold.
    #[test]
    fn the_dock_reaches_the_bottom_unless_it_would_cover_the_navball_cluster() {
        let top = 84.0;
        // The cluster: ~470 px wide, centred, 250 px tall at the bottom.
        let cluster = |w: f32, h: f32| rect(w / 2.0 - 235.0, h - 258.0, w / 2.0 + 235.0, h - 8.0);
        // (window w, h, max height)
        let table = [
            (1600.0, 900.0, 806.0),
            (1512.0, 982.0, 888.0),
            (1280.0, 800.0, 706.0),
            // Narrow: the dock overlaps the cluster and stops above it.
            (900.0, 800.0, 450.0),
            // Tiny: never shorter than the minimum.
            (700.0, 300.0, MIN_DOCK_HEIGHT),
        ];
        for (w, h, max_height) in table {
            let place = dock_place(rect(0.0, 0.0, w, h), top, Some(cluster(w, h)), None);
            assert_eq!(place, DockPlace { x: MARGIN, top, max_height }, "{w}x{h}");
            assert!(place.top + place.max_height <= h - MARGIN || max_height == MIN_DOCK_HEIGHT, "{w}x{h} fits");
        }
        // Without the cluster drawn (the tracking station), to the bottom.
        assert_eq!(dock_place(rect(0.0, 0.0, 900.0, 800.0), top, None, None).max_height, 706.0);
    }

    /// The notifications window in the bottom-left corner: the dock ends
    /// above it, and above the cluster too where both are in the way.
    #[test]
    fn the_dock_ends_above_the_notifications() {
        let top = 84.0;
        let corner = |h: f32, tall: f32| rect(MARGIN, h - MARGIN - tall, MARGIN + DOCK_WIDTH, h - MARGIN);
        let screen = |w: f32, h: f32| rect(0.0, 0.0, w, h);
        let narrow_cluster = rect(200.0, 542.0, 700.0, 792.0);
        // (screen, cluster, corner, max height)
        let table = [
            (screen(1600.0, 900.0), None, Some(corner(900.0, 140.0)), 900.0 - MARGIN - 140.0 - GAP - top),
            // A narrow window: the cluster reaches higher than the window.
            (screen(900.0, 800.0), Some(narrow_cluster), Some(corner(800.0, 140.0)), 542.0 - GAP - top),
            // The window elsewhere (not in the dock's column): no effect.
            (screen(1600.0, 900.0), None, Some(rect(900.0, 700.0, 1200.0, 890.0)), 900.0 - MARGIN - top),
            // A tall window on a short screen: the dock keeps its minimum.
            (screen(1280.0, 300.0), None, Some(corner(300.0, 140.0)), MIN_DOCK_HEIGHT),
        ];
        for (s, cluster, c, want) in table {
            assert_eq!(dock_place(s, top, cluster, c).max_height, want, "{s:?} {c:?}");
        }
    }

    #[test]
    fn marks_behind_a_panel_are_hidden() {
        let cluster = rect(405.0, 542.0, 875.0, 792.0);
        let dock = rect(10.0, 84.0, 310.0, 500.0);
        let hud = [cluster, dock];
        let mark = |x: f32, y: f32| Rect::from_center_size(pos2(x, y), bevy_egui::egui::vec2(18.0, 18.0));
        // (mark, covered)
        let table = [
            (mark(640.0, 600.0), true),               // behind the speed digits
            (mark(200.0, 300.0), true),               // behind the left dock
            (mark(640.0, 300.0), false),              // open sky
            (mark(400.0, 600.0), true),               // half over the cluster's edge
            (mark(390.0, 600.0), false),              // just clear of it
            (rect(330.0, 590.0, 450.0, 604.0), true), // a label reaching into it
        ];
        for (r, want) in table {
            assert_eq!(covered(r, &hud), want, "{r:?}");
        }
        assert!(!covered(mark(640.0, 600.0), &[]), "no panels drawn: nothing hidden");
    }

    #[test]
    fn round_trips_through_ron_and_sanitizes() {
        let mut s = InterfaceSettings { ui_scale: 1.25, ..Default::default() };
        s.menus.toggle(crate::interface::menus::MenuId::Landing);
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
