//! Every way into each screen, window and menu (D111; the interface map,
//! `docs/technical/interface-map.md`): its key, its click path and the
//! agent's tools (MCP). One table, [`WAYS`], read by the help overlay
//! (`interface::help`), so what the help says cannot drift from the keys:
//! a row names its key by the action or menu it is bound to, and the label
//! is the binding's own (`commands::keys::label`, `MenuId::key_label`).
//! The task menus are listed by `MenuId` itself.

use crate::commands::keys::{self, Action};

/// How a thing is reached by key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A binding of `commands::keys::BINDINGS`.
    Action(Action),
    /// Esc (the one Esc stack, `pause::on_escape`).
    Esc,
    /// No key: by click only.
    None,
}

impl Key {
    /// The key as the player reads it on this machine.
    pub fn label(self) -> String {
        match self {
            Key::Action(a) => keys::label(a),
            Key::Esc => keys::key_name(bevy::prelude::KeyCode::Escape).to_string(),
            Key::None => "—".to_string(),
        }
    }
}

/// One thing and its ways in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Way {
    pub what: &'static str,
    pub key: Key,
    /// Click paths, "or" between them.
    pub clicks: &'static str,
    /// The agent's tools (MCP); empty: none.
    pub mcp: &'static str,
}

/// Every screen, window and panel but the task menus (listed by `MenuId`),
/// in the help's order.
pub const WAYS: &[Way] = &[
    Way {
        what: "Systems screen: the craft by layer, everything about each part, stages and functions",
        key: Key::Action(Action::SystemsScreen),
        clicks: "action bar SYSTEMS; a menu's link; a part card's more ›; a notification's link",
        mcp: "shoot {screen: systems, layer, part}",
    },
    Way {
        what: "The ship computer's programs: each automatic sequence's switch (AUTO, ON PRESS, OFF), what it is \
               doing, its press",
        key: Key::None,
        clicks: "systems screen: PROGRAMS; the Maneuver menu's PROGRAM; Landing ▸ PARACHUTE's DEPLOY",
        mcp: "read {programs: true}, set_switch, press, shoot {screen: systems, tab: programs}",
    },
    Way {
        what: "A part isolated in the systems screen (the rest ghosted, framed)",
        key: Key::None,
        clicks: "click it in the view, in PARTS or in a row about it; Esc or its × goes back",
        mcp: "shoot {screen: systems, part}",
    },
    Way {
        what: "Part card: a part's live essentials and commands",
        key: Key::None,
        clicks: "click the part on the ship, or its icon on the staging bar",
        mcp: "shoot {cards}",
    },
    Way {
        what: "RCS menu: mode, authority, thruster blocks",
        key: Key::Action(Action::Rcs),
        clicks: "the navball's RCS button",
        mcp: "",
    },
    Way { what: "Burn planner", key: Key::None, clicks: "Maneuver menu: Open the burn planner", mcp: "set_plan" },
    Way {
        what: "Tracking station: every vessel and body, switching",
        key: Key::Action(Action::TrackingStation),
        clicks: "pause menu: Tracking station",
        mcp: "go_to_mission_control, shoot {screen: tracking}",
    },
    Way {
        what: "A body's menu: distance, focus",
        key: Key::None,
        clicks: "double-click the body",
        mcp: "set_camera {focus}",
    },
    Way { what: "Pause menu", key: Key::Esc, clicks: "", mcp: "pause, shoot {screen: pause}" },
    Way {
        what: "Settings: orbits, controls, interface, visuals",
        key: Key::None,
        clicks: "pause menu: Settings",
        mcp: "shoot {screen: settings, settings_tab}",
    },
    Way {
        what: "Saves (quicksave and quickload by key)",
        key: Key::Action(Action::QuickSave),
        clicks: "pause menu: Saves",
        mcp: "save_game, load_game",
    },
    Way {
        what: "New flight, revert to launch",
        key: Key::None,
        clicks: "pause menu: New flight… / Revert to launch…",
        mcp: "new_flight",
    },
    Way { what: "Debug mode: the truth", key: Key::None, clicks: "pause menu: Debug mode", mcp: "set_debug" },
    Way {
        what: "Performance overlay",
        key: Key::None,
        clicks: "pause menu: Performance overlay",
        mcp: "profile (developer flag)",
    },
    Way {
        what: "Notifications (shown when a message comes; links to its vessel or screen)",
        key: Key::None,
        clicks: "⏷ / ⏶ fold and open them",
        mcp: "",
    },
    Way {
        what: "This help: every key and every way in",
        key: Key::Action(Action::Help),
        clicks: "menu bar ?; pause menu: Help",
        mcp: "shoot {screen: help}",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every way has a way in (a key or a click), and every key it names is
    /// bound.
    #[test]
    fn every_way_has_a_way_in() {
        for w in WAYS {
            assert!(w.key != Key::None || !w.clicks.is_empty(), "{}", w.what);
            if let Key::Action(a) = w.key {
                assert!(!keys::labels(a, true).is_empty(), "{} names {a:?}, which has no key", w.what);
            }
        }
    }
}
