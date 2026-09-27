//! Interface settings and panel layout, as pure data: which panels exist,
//! where each sits by default, where the player moved it, whether it is
//! shown, and the UI scale. Persisted in `settings.ron` (`interface`).

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A HUD panel. Every one is a movable window whose place is saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PanelId {
    /// Date, time and warp (top left).
    Time,
    /// Flight readouts (bottom left).
    Flight,
    /// The navball (bottom centre).
    Navball,
    /// Hovered or targeted object (top right).
    Target,
    /// Anchor, plotting frame, fps and vessel count (off by default).
    Debug,
}

/// A screen corner or edge centre, for default placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    LeftTop,
    RightTop,
    LeftBottom,
    CenterBottom,
}

impl PanelId {
    pub const ALL: [PanelId; 5] = [PanelId::Time, PanelId::Flight, PanelId::Navball, PanelId::Target, PanelId::Debug];

    pub fn title(self) -> &'static str {
        match self {
            PanelId::Time => "Time",
            PanelId::Flight => "Flight",
            PanelId::Navball => "Navball",
            PanelId::Target => "Target",
            PanelId::Debug => "Debug",
        }
    }

    /// Default placement: an anchor and an offset (px) from it, inwards.
    pub fn default_place(self) -> (Anchor, [f32; 2]) {
        match self {
            PanelId::Time => (Anchor::LeftTop, [10.0, 10.0]),
            PanelId::Flight => (Anchor::LeftBottom, [10.0, 10.0]),
            PanelId::Navball => (Anchor::CenterBottom, [0.0, 8.0]),
            PanelId::Target => (Anchor::RightTop, [10.0, 10.0]),
            PanelId::Debug => (Anchor::LeftTop, [10.0, 120.0]),
        }
    }

    pub fn default_visible(self) -> bool {
        self != PanelId::Debug
    }
}

/// One panel's saved state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelLayout {
    /// Top-left corner (logical px) once the player has moved it.
    pub pos: Option<[f32; 2]>,
    pub visible: bool,
}

impl Default for PanelLayout {
    fn default() -> Self {
        PanelLayout { pos: None, visible: true }
    }
}

/// Interface preferences (the settings screen's Interface tab).
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InterfaceSettings {
    /// UI zoom on top of the display's own scale.
    pub ui_scale: f32,
    /// Navball diameter (logical px).
    pub navball_size: f32,
    /// Frame rate shown in the Time panel.
    pub show_fps: bool,
    /// Bumped by "Reset layout" so windows forget where egui put them.
    pub layout_generation: u32,
    pub panels: BTreeMap<PanelId, PanelLayout>,
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
            layout_generation: 0,
            panels: BTreeMap::new(),
            agent: AgentSettings::default(),
        }
    }
}

impl InterfaceSettings {
    pub fn panel(&self, id: PanelId) -> PanelLayout {
        self.panels.get(&id).copied().unwrap_or(PanelLayout { pos: None, visible: id.default_visible() })
    }

    pub fn set_visible(&mut self, id: PanelId, visible: bool) {
        let p = self.panel(id);
        self.panels.insert(id, PanelLayout { visible, ..p });
    }

    /// Records where the player moved a panel (ignores sub-pixel changes).
    pub fn set_pos(&mut self, id: PanelId, pos: [f32; 2]) {
        let p = self.panel(id);
        if p.pos.is_none_or(|old| (old[0] - pos[0]).abs() > 0.5 || (old[1] - pos[1]).abs() > 0.5) {
            self.panels.insert(id, PanelLayout { pos: Some(pos.map(f32::round)), ..p });
        }
    }

    /// Every panel back to its default place (visibility is kept).
    pub fn reset_layout(&mut self) {
        for p in self.panels.values_mut() {
            p.pos = None;
        }
        self.layout_generation = self.layout_generation.wrapping_add(1);
    }

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

/// A saved position pulled back so the whole panel is on screen (or, if it
/// is larger than the screen, its top-left corner is).
pub fn clamp_to_screen(pos: [f32; 2], size: [f32; 2], screen: [f32; 2]) -> [f32; 2] {
    [0, 1].map(|i| pos[i].min(screen[i] - size[i]).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_positions_stay_on_screen() {
        let screen = [1000.0, 800.0];
        let size = [200.0, 100.0];
        // (saved, expected)
        let cases = [
            ([50.0, 60.0], [50.0, 60.0]),
            ([950.0, 60.0], [800.0, 60.0]),
            ([-40.0, 790.0], [0.0, 700.0]),
            ([5000.0, -5000.0], [800.0, 0.0]),
        ];
        for (saved, expected) in cases {
            assert_eq!(clamp_to_screen(saved, size, screen), expected, "{saved:?}");
        }
        // Larger than the screen: the corner stays visible.
        assert_eq!(clamp_to_screen([300.0, 300.0], [2000.0, 100.0], screen), [0.0, 300.0]);
    }

    #[test]
    fn moving_hiding_and_resetting_panels() {
        let mut s = InterfaceSettings::default();
        assert!(s.panel(PanelId::Time).visible && !s.panel(PanelId::Debug).visible);
        s.set_pos(PanelId::Time, [100.2, 50.7]);
        assert_eq!(s.panel(PanelId::Time).pos, Some([100.0, 51.0]));
        let before = s.clone();
        s.set_pos(PanelId::Time, [100.3, 50.9]);
        assert_eq!(s, before, "sub-pixel moves are not changes");
        s.set_visible(PanelId::Debug, true);
        s.set_visible(PanelId::Time, false);
        s.reset_layout();
        assert_eq!(s.panel(PanelId::Time), PanelLayout { pos: None, visible: false });
        assert!(s.panel(PanelId::Debug).visible);
        assert_eq!(s.layout_generation, 1);
    }

    #[test]
    fn round_trips_through_ron_and_sanitizes() {
        let mut s = InterfaceSettings { ui_scale: 1.25, ..Default::default() };
        s.set_pos(PanelId::Navball, [700.0, 650.0]);
        s.set_visible(PanelId::Debug, true);
        let text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<InterfaceSettings>(&text).unwrap(), s);
        let wild = InterfaceSettings { ui_scale: 40.0, navball_size: f32::NAN, ..Default::default() }.sanitized();
        assert_eq!((wild.ui_scale, wild.navball_size), (2.0, 150.0));
    }
}
