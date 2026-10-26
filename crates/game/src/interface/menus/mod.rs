//! The flight view's task menus (D080). One registry ([`MenuId`]: title,
//! key, dock, order), pure content per menu (`sections(&FlightData) ->
//! Vec<Section>`, table-tested, rule 8) and one generic renderer
//! ([`render`]) for every menu. A new task is one more `MenuId` and its
//! content function: docking gets a Docking menu, advanced maneuvers go in
//! Orbital.
//!
//! Each menu is a list of sections: the first is its summary, always
//! shown; the others expand on click and their state is saved with the open
//! menus in `settings.ron` ([`MenuSettings`]). A section with nothing to
//! show is not drawn.

pub mod data;
mod landing;
mod orbital;
pub mod render;
mod staging;
mod utilities;
pub mod view;

pub use data::FlightData;

use bevy::prelude::KeyCode;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A task menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MenuId {
    /// Advanced orbital data, the target, maneuvers.
    Orbital,
    /// Impact, braking, entry, parachute, attitude.
    Landing,
    /// A brief status of the ship's systems.
    Utilities,
    /// The stage stack.
    Staging,
}

/// Where a menu sits: flight tasks on the left, the ship on the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dock {
    Left,
    Right,
}

impl MenuId {
    /// Every menu, in menu-bar and dock order.
    pub const ALL: [MenuId; 4] = [MenuId::Orbital, MenuId::Landing, MenuId::Utilities, MenuId::Staging];

    /// The menu's title (its panel).
    pub fn title(self) -> &'static str {
        match self {
            MenuId::Orbital => "ORBITAL",
            MenuId::Landing => "LANDING / REENTRY",
            MenuId::Utilities => "UTILITIES",
            MenuId::Staging => "STAGING",
        }
    }

    /// The menu bar's short name.
    pub fn short(self) -> &'static str {
        match self {
            MenuId::Landing => "LANDING",
            m => m.title(),
        }
    }

    /// The key that opens and closes it.
    pub fn key(self) -> KeyCode {
        match self {
            MenuId::Orbital => KeyCode::KeyO,
            MenuId::Landing => KeyCode::KeyL,
            MenuId::Utilities => KeyCode::KeyU,
            MenuId::Staging => KeyCode::KeyG,
        }
    }

    /// The key as shown (the menu bar, the help).
    pub fn key_label(self) -> &'static str {
        match self {
            MenuId::Orbital => "O",
            MenuId::Landing => "L",
            MenuId::Utilities => "U",
            MenuId::Staging => "G",
        }
    }

    pub fn dock(self) -> Dock {
        match self {
            MenuId::Orbital | MenuId::Landing => Dock::Left,
            MenuId::Utilities | MenuId::Staging => Dock::Right,
        }
    }

    /// What the menu shows, summary first.
    pub fn sections(self, d: &FlightData) -> Vec<Section> {
        let all = match self {
            MenuId::Orbital => orbital::sections(d),
            MenuId::Landing => landing::sections(d),
            MenuId::Utilities => utilities::sections(d),
            MenuId::Staging => staging::sections(d),
        };
        all.into_iter().filter(|s| !s.items.is_empty()).collect()
    }
}

/// How a value reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Normal,
    Dim,
    /// Highlighted (the next stage).
    Accent,
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
    NextStage,
}

/// One line of a section.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Row(Row),
    /// A sub-heading (a stage of the stack).
    Heading(String, Tone),
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

    pub fn heading(&mut self, text: impl Into<String>, tone: Tone) -> &mut Self {
        self.items.push(Item::Heading(text.into(), tone));
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

/// Which menus are open and which of their sections are expanded (saved
/// in `settings.ron`). Nothing is open by default: the flight view is the
/// time panel, the menu bar and the navball cluster.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuSettings {
    pub open: BTreeSet<MenuId>,
    pub expanded: BTreeMap<MenuId, BTreeSet<String>>,
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

    /// The summary is always expanded.
    pub fn is_expanded(&self, menu: MenuId, section: &str) -> bool {
        section == SUMMARY || self.expanded.get(&menu).is_some_and(|s| s.contains(section))
    }

    pub fn toggle_section(&mut self, menu: MenuId, section: &str) {
        let set = self.expanded.entry(menu).or_default();
        if !set.remove(section) {
            set.insert(section.to_string());
        }
        if set.is_empty() {
            self.expanded.remove(&menu);
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
    }

    /// Opens only `menu` (or none) with `sections` expanded (the demo's
    /// views).
    pub fn show_only(&mut self, menu: Option<MenuId>, sections: &[&str]) {
        for m in MenuId::ALL {
            self.set_open(m, Some(m) == menu);
        }
        for (m, s) in menu.iter().flat_map(|m| sections.iter().map(move |s| (*m, *s))) {
            if !self.is_expanded(m, s) {
                self.toggle_section(m, s);
            }
        }
    }

    /// The open menus of `dock`, top to bottom (menu order).
    pub fn in_dock(&self, dock: Dock) -> Vec<MenuId> {
        MenuId::ALL.into_iter().filter(|m| m.dock() == dock && self.is_open(*m)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_has_a_unique_key_and_a_dock() {
        let keys: BTreeSet<_> = MenuId::ALL.iter().map(|m| m.key_label()).collect();
        assert_eq!(keys.len(), MenuId::ALL.len(), "unique keys");
        let codes: std::collections::HashSet<_> = MenuId::ALL.iter().map(|m| m.key()).collect();
        assert_eq!(codes.len(), MenuId::ALL.len());
        for m in MenuId::ALL {
            assert_eq!(format!("{:?}", m.key()), format!("Key{}", m.key_label()), "{m:?}");
            assert!(matches!(m.dock(), Dock::Left | Dock::Right));
            assert!(!m.title().is_empty() && !m.short().is_empty());
        }
        let left: Vec<_> = MenuId::ALL.into_iter().filter(|m| m.dock() == Dock::Left).collect();
        assert_eq!(left, [MenuId::Orbital, MenuId::Landing], "flight tasks on the left");
    }

    /// The menu keys are listed in the help once each, under Menus, and no
    /// other key in the help uses them.
    #[test]
    fn menu_keys_do_not_clash_with_any_other_key() {
        let tokens = |keys: &str| -> Vec<String> {
            keys.split(|c: char| c.is_whitespace() || c == '/').filter(|t| !t.is_empty()).map(str::to_string).collect()
        };
        for m in MenuId::ALL {
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
    fn menus_open_close_and_keep_their_sections() {
        let mut s = MenuSettings::default();
        assert!(MenuId::ALL.iter().all(|m| !s.is_open(*m)), "nothing open by default");
        s.toggle(MenuId::Staging);
        s.toggle(MenuId::Orbital);
        s.toggle(MenuId::Utilities);
        assert_eq!(s.in_dock(Dock::Left), [MenuId::Orbital]);
        assert_eq!(s.in_dock(Dock::Right), [MenuId::Utilities, MenuId::Staging], "menu order, not opening order");
        s.toggle(MenuId::Utilities);
        assert_eq!(s.in_dock(Dock::Right), [MenuId::Staging]);
        assert!(s.is_expanded(MenuId::Orbital, SUMMARY), "the summary always shows");
        assert!(!s.is_expanded(MenuId::Orbital, "elements"));
        s.toggle_section(MenuId::Orbital, "elements");
        assert!(s.is_expanded(MenuId::Orbital, "elements"));
        assert!(!s.is_expanded(MenuId::Landing, "elements"), "per menu");
        let text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<MenuSettings>(&text).unwrap(), s);
        s.toggle_section(MenuId::Orbital, "elements");
        assert_eq!(s.expanded, BTreeMap::new());
        s.toggle_section(MenuId::Orbital, "target");
        s.collapse_all();
        assert!(!s.is_expanded(MenuId::Orbital, "target"));
        s.show_only(Some(MenuId::Landing), &["reentry", "reentry"]);
        assert_eq!(s.open, BTreeSet::from([MenuId::Landing]));
        assert!(s.is_expanded(MenuId::Landing, "reentry"), "expanded once, not toggled back");
        s.show_only(None, &[]);
        assert!(s.open.is_empty() && s.is_expanded(MenuId::Landing, "reentry"));
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
