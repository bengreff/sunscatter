//! The flight view's task menus (D080, D111). One registry ([`MenuId`]:
//! title, number key, order), pure content per menu (`sections(&FlightData)
//! -> Vec<Section>`, table-tested, rule 8) and one generic renderer
//! ([`render`]) for every menu. A new task is one more `MenuId` and its
//! content function: docking gets a Docking menu on 7.
//!
//! Each menu is one flat page (D111): its summary, then every section under
//! its heading, nothing collapsed; a section with nothing to show is not
//! drawn. All of them stack in one dock on the left that always scrolls
//! (the right edge is the staging bar's). Which are open is saved in
//! `settings.ron` ([`MenuSettings`]).

mod comms;
pub mod data;
mod hottest;
mod landing;
mod maneuver;
mod orbital;
pub mod render;
pub mod view;

pub use data::FlightData;

use crate::systems_screen::rules::Layer;
use bevy::prelude::KeyCode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A task menu, in dock order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MenuId {
    /// The orbit now, the target, the ship's clock.
    Orbital,
    /// Impact, braking, entry, thermal, parachute, attitude.
    Landing,
    /// The planned burns and the planner.
    Maneuver,
    /// The links home and the probe's control delay (the old Utilities and
    /// Staging menus open it from older settings).
    #[serde(alias = "Utilities", alias = "Staging")]
    Comms,
}

impl MenuId {
    /// Every menu, in menu-bar and dock order.
    pub const ALL: [MenuId; 4] = [MenuId::Orbital, MenuId::Landing, MenuId::Maneuver, MenuId::Comms];

    /// The menu's title (its panel).
    pub fn title(self) -> &'static str {
        match self {
            MenuId::Orbital => "ORBITAL",
            MenuId::Landing => "LANDING / REENTRY",
            MenuId::Maneuver => "MANEUVER",
            MenuId::Comms => "COMMS",
        }
    }

    /// The menu bar's short name.
    pub fn short(self) -> &'static str {
        match self {
            MenuId::Landing => "LANDING",
            m => m.title(),
        }
    }

    /// The number key that opens and closes it (D111: 1–0), if any.
    pub fn key(self) -> Option<KeyCode> {
        match self {
            MenuId::Orbital => Some(KeyCode::Digit1),
            MenuId::Landing => Some(KeyCode::Digit2),
            MenuId::Maneuver => Some(KeyCode::Digit3),
            MenuId::Comms => Some(KeyCode::Digit6),
        }
    }

    /// The key as shown (the menu bar, the help); empty without one.
    pub fn key_label(self) -> &'static str {
        match self {
            MenuId::Orbital => "1",
            MenuId::Landing => "2",
            MenuId::Maneuver => "3",
            MenuId::Comms => "6",
        }
    }

    /// `text` with the menu's key after it, if it has one: "Close (1)".
    pub fn with_key(self, text: &str) -> String {
        match self.key_label() {
            "" => text.to_string(),
            k => format!("{text} ({k})"),
        }
    }

    /// The systems screen's layer its page links to at its end.
    pub fn layer(self) -> Option<Layer> {
        match self {
            MenuId::Orbital | MenuId::Maneuver => None,
            MenuId::Landing => Some(Layer::Thermal),
            MenuId::Comms => Some(Layer::Data),
        }
    }

    /// What the menu shows, summary first, then the link into the systems
    /// screen at its layer.
    pub fn sections(self, d: &FlightData) -> Vec<Section> {
        let mut all = match self {
            MenuId::Orbital => orbital::sections(d),
            MenuId::Landing => landing::sections(d),
            MenuId::Maneuver => maneuver::sections(d),
            MenuId::Comms => comms::sections(d),
        };
        if d.parts {
            let mut more = Section::new("more", "");
            let layer = self.layer();
            let label = layer.map_or("The systems screen".to_string(), |l| format!("The systems screen: {}", l.name()));
            more.button(label, MenuAction::OpenSystems(layer, None));
            all.push(more);
        }
        all.into_iter().filter(|s| !s.items.is_empty()).collect()
    }
}

/// How a value reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Normal,
    Dim,
    Good,
    /// Near a limit, or something to act on.
    Warn,
    /// Over a limit, failed, destroyed.
    Alert,
}

/// A labelled value.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub tone: Tone,
}

/// Something a menu control does, applied after drawing.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    SetTarget(Option<crate::navball::NavTarget>),
    OpenPlanner,
    /// Opens the systems screen at a layer and a part (`SystemsScreen::open_at`).
    OpenSystems(Option<Layer>, Option<u32>),
}

/// One line of a section.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Row(Row),
    /// A line of text: an explanation, "no impact predicted".
    Note(String, Tone),
    /// A pick list: the label, the current choice, and each option with
    /// whether it is selected.
    Choice {
        label: String,
        current: String,
        options: Vec<(String, bool, MenuAction)>,
    },
    Button {
        label: String,
        action: MenuAction,
    },
}

/// A part of a menu: the summary (always shown, no header) or a section
/// that expands on click.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// Stable id (saved with the expanded state).
    pub id: &'static str,
    pub title: String,
    pub items: Vec<Item>,
}

/// The summary section's id.
pub const SUMMARY: &str = "summary";

impl Section {
    pub fn new(id: &'static str, title: impl Into<String>) -> Self {
        Section { id, title: title.into(), items: Vec::new() }
    }

    pub fn summary() -> Self {
        Section::new(SUMMARY, "")
    }

    pub fn row(&mut self, label: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.toned(label, value, Tone::Normal)
    }

    pub fn toned(&mut self, label: impl Into<String>, value: impl Into<String>, tone: Tone) -> &mut Self {
        self.items.push(Item::Row(Row { label: label.into(), value: value.into(), tone }));
        self
    }

    pub fn note(&mut self, text: impl Into<String>, tone: Tone) -> &mut Self {
        self.items.push(Item::Note(text.into(), tone));
        self
    }

    pub fn button(&mut self, label: impl Into<String>, action: MenuAction) -> &mut Self {
        self.items.push(Item::Button { label: label.into(), action });
        self
    }

    /// The rows as (label, value) pairs.
    #[cfg(test)]
    pub fn rows(&self) -> Vec<(&str, &str)> {
        self.items
            .iter()
            .filter_map(|i| match i {
                Item::Row(r) => Some((r.label.as_str(), r.value.as_str())),
                _ => None,
            })
            .collect()
    }

    /// The value of the row labelled `label`, if there is one.
    #[cfg(test)]
    pub fn value(&self, label: &str) -> Option<&str> {
        self.rows().into_iter().find(|(l, _)| *l == label).map(|(_, v)| v)
    }

    /// The tone of the row labelled `label`, if there is one.
    #[cfg(test)]
    pub fn tone(&self, label: &str) -> Option<Tone> {
        self.items.iter().find_map(|i| match i {
            Item::Row(r) if r.label == label => Some(r.tone),
            _ => None,
        })
    }
}

/// The section `id` of `sections`, if drawn.
#[cfg(test)]
pub fn find<'a>(sections: &'a [Section], id: &str) -> Option<&'a Section> {
    sections.iter().find(|s| s.id == id)
}

/// Which menus are open (saved in `settings.ron`; older settings' expanded
/// sections are ignored). Nothing is open by default: the flight view is
/// the time panel, the menu bar and the navball cluster.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuSettings {
    pub open: BTreeSet<MenuId>,
}

impl MenuSettings {
    pub fn is_open(&self, menu: MenuId) -> bool {
        self.open.contains(&menu)
    }

    pub fn set_open(&mut self, menu: MenuId, open: bool) {
        if open {
            self.open.insert(menu);
        } else {
            self.open.remove(&menu);
        }
    }

    pub fn toggle(&mut self, menu: MenuId) {
        self.set_open(menu, !self.is_open(menu));
    }

    /// Opens only `menu`, or none (the demo's views).
    pub fn show_only(&mut self, menu: Option<MenuId>) {
        for m in MenuId::ALL {
            self.set_open(m, Some(m) == menu);
        }
    }

    /// The open menus, top to bottom (menu order).
    pub fn in_order(&self) -> Vec<MenuId> {
        MenuId::ALL.into_iter().filter(|m| self.is_open(*m)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_has_a_unique_number_key() {
        let keyed: Vec<MenuId> = MenuId::ALL.into_iter().filter(|m| m.key().is_some()).collect();
        let keys: BTreeSet<_> = keyed.iter().map(|m| m.key_label()).collect();
        assert_eq!(keys.len(), keyed.len(), "unique keys");
        for m in MenuId::ALL {
            let code = m.key().map(|k| format!("{k:?}"));
            let label = (!m.key_label().is_empty()).then(|| format!("Digit{}", m.key_label()));
            assert_eq!(code, label, "{m:?}");
            assert!(!m.title().is_empty() && !m.short().is_empty());
        }
    }

    /// Older settings load: the old menus' names open Comms, expanded
    /// sections are ignored.
    #[test]
    fn older_settings_still_load() {
        let old = "(open: [Orbital, Utilities, Staging], expanded: {Landing: [\"reentry\"]})";
        let s: MenuSettings = ron::from_str(old).expect("loads");
        assert_eq!(s.in_order(), [MenuId::Orbital, MenuId::Comms]);
    }

    /// The menu keys are listed in the help once each, under Menus, and no
    /// other key in the help uses them.
    #[test]
    fn menu_keys_do_not_clash_with_any_other_key() {
        let tokens = |keys: &str| -> Vec<String> {
            keys.split(|c: char| c.is_whitespace() || c == '/').filter(|t| !t.is_empty()).map(str::to_string).collect()
        };
        for m in MenuId::ALL.into_iter().filter(|m| m.key().is_some()) {
            let mut listed = 0;
            for (group, keys) in crate::interface::help::KEYS {
                for (k, _) in *keys {
                    if tokens(k).iter().any(|t| t == m.key_label()) {
                        assert_eq!(*group, "Menus", "{m:?}'s key {} is also used by `{k}`", m.key_label());
                        listed += 1;
                    }
                }
            }
            assert_eq!(listed, 1, "{m:?} is in the help once");
        }
    }

    #[test]
    fn menus_open_close_in_menu_order() {
        let mut s = MenuSettings::default();
        assert!(MenuId::ALL.iter().all(|m| !s.is_open(*m)), "nothing open by default");
        s.toggle(MenuId::Comms);
        s.toggle(MenuId::Orbital);
        assert_eq!(s.in_order(), [MenuId::Orbital, MenuId::Comms], "menu order, not opening order");
        s.toggle(MenuId::Comms);
        assert_eq!(s.in_order(), [MenuId::Orbital]);
        let text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<MenuSettings>(&text).unwrap(), s);
        s.show_only(Some(MenuId::Landing));
        assert_eq!(s.open, BTreeSet::from([MenuId::Landing]));
        s.show_only(None);
        assert!(s.open.is_empty());
    }

    /// A craft of parts' pages end with a link into the systems screen at
    /// their layer; the test craft's do not.
    #[test]
    fn pages_link_into_the_systems_screen() {
        let mut d = FlightData::default();
        assert!(MenuId::Landing.sections(&d).iter().all(|s| s.id != "more"));
        d.parts = true;
        let s = MenuId::Landing.sections(&d);
        let more = s.last().unwrap();
        let want = MenuAction::OpenSystems(Some(Layer::Thermal), None);
        assert!(more.items.iter().any(|i| matches!(i, Item::Button { action, .. } if *action == want)));
    }

    /// Before the first snapshot (and in any degenerate state) no value
    /// reads NaN or infinity.
    #[test]
    fn no_menu_shows_nan_or_infinity_before_the_first_snapshot() {
        let d = FlightData::default();
        for m in MenuId::ALL {
            for s in m.sections(&d) {
                for (label, value) in s.rows() {
                    let bad = |t: &str| t.contains("NaN") || t.contains("inf") || t.contains('∞');
                    assert!(!bad(value) && !bad(label), "{m:?} {}: {label} = {value}", s.id);
                }
            }
        }
    }

    #[test]
    fn empty_sections_are_not_drawn() {
        // A craft with nothing in an atmosphere and no impact: Landing has
        // no Braking or Reentry section.
        let d = FlightData::default();
        let ids: Vec<_> = MenuId::Landing.sections(&d).iter().map(|s| s.id).collect();
        assert!(!ids.contains(&"braking") && !ids.contains(&"reentry"), "{ids:?}");
        for m in MenuId::ALL {
            let sections = m.sections(&d);
            assert!(sections.iter().all(|s| !s.items.is_empty()), "{m:?}");
            assert!(sections.iter().filter(|s| s.id == SUMMARY).count() <= 1);
            let at = sections.iter().position(|s| s.id == SUMMARY);
            assert!(at.is_none_or(|k| k == 0), "{m:?}: the summary comes first");
        }
    }
}
