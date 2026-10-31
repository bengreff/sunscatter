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
pub mod crew;
pub mod data;
pub mod hottest;
mod landing;
mod maneuver;
mod orbital;
pub mod power;
pub mod rcs;
mod rcs_data;
pub mod render;
#[cfg(test)]
mod steady_tests;
pub mod view;

pub use data::FlightData;

use crate::interface::steady;
use crate::systems_screen::rules::Layer;
use bevy::prelude::KeyCode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A task menu, in dock order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MenuId {
    /// The pilot's RCS: its mode, authority, propellant and every thruster
    /// block (D111, D112). No key and not in the bar: shown while RCS is on
    /// (the navball's button, R), at the top of the dock.
    Rcs,
    /// The orbit now, the target, the ship's clock.
    Orbital,
    /// Impact, braking, entry, thermal, parachute, attitude.
    Landing,
    /// The planned burns and the planner.
    Maneuver,
    /// The crew, the cabin's air, life support, injuries.
    Crew,
    /// The buses, batteries and what has no power.
    Power,
    /// The links home and the probe's control delay (the old Utilities and
    /// Staging menus open it from older settings).
    #[serde(alias = "Utilities", alias = "Staging")]
    Comms,
}

impl MenuId {
    /// Every menu, in dock order.
    pub const ALL: [MenuId; 7] =
        [MenuId::Rcs, MenuId::Orbital, MenuId::Landing, MenuId::Maneuver, MenuId::Crew, MenuId::Power, MenuId::Comms];
    /// The menus the bar, their keys and the settings open and close (the
    /// RCS menu follows RCS).
    pub const BAR: [MenuId; 6] =
        [MenuId::Orbital, MenuId::Landing, MenuId::Maneuver, MenuId::Crew, MenuId::Power, MenuId::Comms];

    /// The menu's title (its panel).
    pub fn title(self) -> &'static str {
        match self {
            MenuId::Rcs => "RCS",
            MenuId::Orbital => "ORBITAL",
            MenuId::Landing => "LANDING / REENTRY",
            MenuId::Maneuver => "MANEUVER",
            MenuId::Crew => "CREW / ECLSS",
            MenuId::Power => "POWER",
            MenuId::Comms => "COMMS",
        }
    }

    /// The menu bar's short name.
    pub fn short(self) -> &'static str {
        match self {
            MenuId::Landing => "LANDING",
            MenuId::Crew => "CREW",
            m => m.title(),
        }
    }

    /// The number key that opens and closes it (D111: 1–0), if any.
    pub fn key(self) -> Option<KeyCode> {
        match self {
            MenuId::Orbital => Some(KeyCode::Digit1),
            MenuId::Landing => Some(KeyCode::Digit2),
            MenuId::Maneuver => Some(KeyCode::Digit3),
            MenuId::Crew => Some(KeyCode::Digit4),
            MenuId::Power => Some(KeyCode::Digit5),
            MenuId::Comms => Some(KeyCode::Digit6),
            MenuId::Rcs => None,
        }
    }

    /// The key as shown (the menu bar, the help), from [`Self::key`]
    /// itself; empty without one.
    pub fn key_label(self) -> &'static str {
        self.key().map_or("", crate::commands::keys::key_name)
    }

    /// What the menu is for, as the help says it.
    pub fn about(self) -> &'static str {
        match self {
            MenuId::Rcs => "mode, authority, propellant and every thruster block (shown while RCS is on)",
            MenuId::Orbital => "the orbit now, the target, the ship's clock",
            MenuId::Landing => "impact, braking, entry, heat, parachute (shows the impact marker)",
            MenuId::Maneuver => "the planned burns, the burn planner",
            MenuId::Crew => "the crew, the cabin's air, life support, injuries",
            MenuId::Power => "the buses, batteries, what has no power",
            MenuId::Comms => "the link home, a probe's control delay",
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
            MenuId::Rcs => Some(Layer::Plumbing),
            MenuId::Crew => Some(Layer::Health),
            MenuId::Power => Some(Layer::Electrical),
        }
    }

    /// What the menu shows, summary first, then the link into the systems
    /// screen at its layer.
    pub fn sections(self, d: &FlightData) -> Vec<Section> {
        let mut all = match self {
            MenuId::Rcs => rcs::sections(&d.rcs),
            MenuId::Orbital => orbital::sections(d),
            MenuId::Landing => landing::sections(d),
            MenuId::Maneuver => maneuver::sections(d),
            MenuId::Crew => crew::sections(d.crew.as_ref(), &d.known),
            MenuId::Power => power::sections(d.power.as_ref()),
            MenuId::Comms => comms::sections(d),
        };
        let mut more = Section::new("more", "");
        let layer = self.layer();
        let label = layer.map_or("The systems screen".to_string(), |l| format!("The systems screen: {}", l.name()));
        more.button(label, MenuAction::OpenSystems(layer, None));
        all.push(more);
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
    /// A command to the active vessel (a thruster block on or off).
    Ship(sim::craft::command::ShipCommand),
    /// The pilot's RCS mode (`GameCommand::SetRcs`).
    SetRcs(sim::systems::rcs::RcsMode),
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
    /// A switch: a checkbox and its label, a value right-aligned in its
    /// tone; a click does `action`.
    Toggle {
        label: String,
        on: bool,
        value: String,
        tone: Tone,
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

    pub fn toggle(
        &mut self,
        label: impl Into<String>,
        on: bool,
        value: impl Into<String>,
        tone: Tone,
        action: MenuAction,
    ) -> &mut Self {
        self.items.push(Item::Toggle { label: label.into(), on, value: value.into(), tone, action });
        self
    }

    /// The rows (and switches) as (label, value) pairs.
    #[cfg(test)]
    pub fn rows(&self) -> Vec<(&str, &str)> {
        self.items
            .iter()
            .filter_map(|i| match i {
                Item::Row(r) => Some((r.label.as_str(), r.value.as_str())),
                Item::Toggle { label, value, .. } => Some((label.as_str(), value.as_str())),
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
            Item::Toggle { label: l, tone, .. } if l == label => Some(*tone),
            _ => None,
        })
    }
}

/// A menu as shown (`interface::steady`, the one rule: nothing flickers):
/// a section or a line that vanished less than `steady::HOLD` ago stays,
/// in its place, with its last content.
#[derive(Clone, Debug, Default)]
pub struct Steady {
    shown: Vec<Section>,
    sections: steady::Seen<&'static str>,
    items: steady::Seen<(&'static str, String)>,
}

impl Steady {
    /// What to draw now, given the menu's sections now (`now`: real
    /// seconds, so the hold runs while paused).
    pub fn update(&mut self, new: Vec<Section>, now: f64) -> &[Section] {
        let items = &mut self.items;
        self.shown = steady::hold(
            &self.shown,
            new,
            |s| s.id,
            |old, mut s| {
                let id = s.id;
                let before = old.map_or_else(Vec::new, |o| keyed(&o.items));
                let held = steady::hold(&before, keyed(&s.items), |k| (id, k.0.clone()), |_, k| k, items, now);
                s.items = held.into_iter().map(|k| k.1).collect();
                s
            },
            &mut self.sections,
            now,
        );
        &self.shown
    }
}

/// A section's lines with their keys: a row, switch, pick list or button by
/// its label, a note by its place among the notes (its text changes).
fn keyed(items: &[Item]) -> Vec<(String, Item)> {
    let mut notes = 0;
    items
        .iter()
        .map(|i| {
            let key = match i {
                Item::Row(r) => format!("row {}", r.label),
                Item::Toggle { label, .. } => format!("switch {label}"),
                Item::Choice { label, .. } => format!("pick {label}"),
                Item::Button { label, .. } => format!("button {label}"),
                Item::Note(..) => {
                    notes += 1;
                    format!("note {notes}")
                }
            };
            (key, i.clone())
        })
        .collect()
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
        for m in MenuId::BAR {
            self.set_open(m, Some(m) == menu);
        }
    }

    /// The open menus, top to bottom (menu order): the RCS menu first while
    /// RCS is on (`rcs`), never from the settings.
    pub fn in_order(&self, rcs: bool) -> Vec<MenuId> {
        let shown = |m: MenuId| if m == MenuId::Rcs { rcs } else { self.is_open(m) };
        MenuId::ALL.into_iter().filter(|m| shown(*m)).collect()
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
        assert_eq!(s.in_order(false), [MenuId::Orbital, MenuId::Comms]);
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
            for s in crate::interface::help::sections().into_iter().filter(|s| s.title != "Every way in") {
                for (k, _) in &s.rows {
                    if tokens(k).iter().any(|t| t == m.key_label()) {
                        assert!(s.title.starts_with("Menus"), "{m:?}'s key {} is also used by `{k}`", m.key_label());
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
        assert_eq!(s.in_order(false), [MenuId::Orbital, MenuId::Comms], "menu order, not opening order");
        s.toggle(MenuId::Comms);
        assert_eq!(s.in_order(false), [MenuId::Orbital]);
        // The RCS menu follows RCS, on top, whatever the settings say.
        assert_eq!(s.in_order(true), [MenuId::Rcs, MenuId::Orbital]);
        s.set_open(MenuId::Rcs, true);
        assert_eq!(s.in_order(false), [MenuId::Orbital]);
        s.set_open(MenuId::Rcs, false);
        let text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<MenuSettings>(&text).unwrap(), s);
        s.show_only(Some(MenuId::Landing));
        assert_eq!(s.open, BTreeSet::from([MenuId::Landing]));
        s.show_only(None);
        assert!(s.open.is_empty());
    }

    /// A row that comes and goes between refreshes stays drawn (the Comms
    /// menu's VIA on the pad).
    #[test]
    fn a_menu_holds_a_row_that_comes_and_goes() {
        let page = |via: bool| {
            let mut s = Section::summary();
            s.row("HOUSTON", "12 ms");
            if via {
                s.row("  VIA", "Cape Canaveral");
            }
            s.row("  RATE", "4 Mbit/s");
            vec![s]
        };
        let mut steady = Steady::default();
        let mut shown = Vec::new();
        for k in 0..20 {
            let drawn = steady.update(page(k % 2 == 0), 0.1 * k as f64);
            shown.push(drawn[0].rows().iter().map(|r| r.0.to_string()).collect::<Vec<_>>());
        }
        assert!(shown.iter().all(|r| *r == ["HOUSTON", "  VIA", "  RATE"]), "{shown:?}");
        // Gone for good: it goes after the hold.
        let drawn = steady.update(page(false), 2.0 + steady::HOLD);
        assert_eq!(drawn[0].rows().len(), 2);
    }

    /// Every page ends with a link into the systems screen at its layer.
    #[test]
    fn pages_link_into_the_systems_screen() {
        let d = FlightData::default();
        for m in MenuId::ALL {
            let s = m.sections(&d);
            let more = s.last().unwrap();
            let want = MenuAction::OpenSystems(m.layer(), None);
            assert!(more.items.iter().any(|i| matches!(i, Item::Button { action, .. } if *action == want)), "{m:?}");
        }
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
