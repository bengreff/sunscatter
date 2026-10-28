//! The systems screen (D107; ship-model.md §8): a screen of its own, like
//! the tracking station, where the active craft is shown alone by layer
//! (physical, structural, thermal, plumbing, electrical, data, health),
//! every part and component can be inspected and commanded, and the stage
//! sequence and saved functions are edited (saved with the vessel, D107:
//! vessel state, not settings) and pinned to the action-group bar.
//!
//! Tab opens and closes it; the action bar's SYSTEMS button, the menus'
//! links and the part cards' "more" open it at a layer and a part
//! ([`SystemsScreen::open_at`]); Esc closes it. While it is open the clock runs, warp keys act and flight
//! keys do not (`commands::InputContext::Systems`).
//!
//! A layer selected shows that layer as a whole ([`summary`], the right
//! column); a click on a part (in the list, the view or a summary's row)
//! opens its pop-up, everything about it in one layout ([`inspect`]), and
//! several can be open at once.
//!
//! Pure rules: [`rules`] (facts, layer colours, tree, camera, pick),
//! [`summary`] (each layer as a whole), [`inspect`] (everything about a
//! selection, its commands), [`edit`] (the editors' pick list and edits).
//! Bevy wiring: [`view`] (the 3D view in an image), [`ui`] (the screen,
//! its pop-ups, its editors).

pub mod edit;
pub mod inspect;
pub mod rules;
pub mod summary;
mod ui;
pub mod view;

use crate::commands::keys::{pressed, Action};
use crate::commands::{InputContext, Keys};
use bevy::prelude::*;
use inspect::Selection;
use rules::Layer;
use sim::craft::command::Stage;
use sim::vessel::VesselId;

/// The editors' tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Stages,
    Functions,
}

/// The stage sequence and functions being edited, for one vessel and craft
/// (`dirty`: changed and not yet saved, per list).
#[derive(Clone, Debug, PartialEq)]
pub struct Drafts {
    pub vessel: VesselId,
    pub craft: String,
    pub stages: Vec<Stage>,
    pub functions: Vec<Stage>,
    pub dirty: [bool; 2],
}

impl Drafts {
    /// The drafts after the vessel's own lists (`stages`, `functions`) are
    /// read: another vessel or craft starts over; a list not being edited
    /// follows the vessel's; one being edited is kept.
    pub fn sync(old: Option<Drafts>, vessel: VesselId, craft: &str, stages: &[Stage], functions: &[Stage]) -> Drafts {
        let mut d = match old.filter(|d| d.vessel == vessel && d.craft == craft) {
            Some(d) => d,
            None => Drafts {
                vessel,
                craft: craft.to_string(),
                stages: Vec::new(),
                functions: Vec::new(),
                dirty: [false; 2],
            },
        };
        if !d.dirty[0] {
            d.stages = stages.to_vec();
        }
        if !d.dirty[1] {
            d.functions = functions.to_vec();
        }
        d
    }
}

/// The screen's state.
#[derive(Resource)]
pub struct SystemsScreen {
    pub open: bool,
    pub layer: Layer,
    pub selected: Option<Selection>,
    /// The orbit camera: yaw and pitch (rad), distance (m; `None`: framed).
    pub yaw: f32,
    pub pitch: f32,
    pub distance: Option<f32>,
    /// The view's size on screen (physical px) and a click in it (px from
    /// its top left), for `view::update`.
    pub view_px: Option<UVec2>,
    pub click: Option<Vec2>,
    pub tab: Tab,
    pub drafts: Option<Drafts>,
    /// The open pop-ups, the latest last.
    pub popups: Vec<Selection>,
    /// Opened at a part without a layer: show the part's main layer once
    /// its facts are known ([`SystemsScreen::settle_layer`]).
    pub pick_layer: bool,
}

/// At most this many pop-ups are open (the oldest closes).
pub const MAX_POPUPS: usize = 4;

impl Default for SystemsScreen {
    fn default() -> Self {
        SystemsScreen {
            open: false,
            layer: Layer::Physical,
            selected: None,
            yaw: 0.6,
            pitch: 0.25,
            distance: None,
            view_px: None,
            click: None,
            tab: Tab::Stages,
            drafts: None,
            popups: Vec::new(),
            pick_layer: false,
        }
    }
}

impl SystemsScreen {
    /// Opens the screen, at `layer` and with `part` selected and its
    /// pop-up open when given; a part without a layer shows the part's
    /// main layer (`Layer::main`). The one way in from the flight view:
    /// the action bar, the menus' links, the part cards' "more".
    pub fn open_at(&mut self, layer: Option<Layer>, part: Option<u32>) {
        self.open = true;
        if let Some(layer) = layer {
            self.layer = layer;
        }
        if let Some(part) = part {
            self.select(Some(Selection::Part(part)));
        }
        self.pick_layer = layer.is_none() && part.is_some();
    }

    /// Selects `sel` (highlighted in the view and the list) and opens its
    /// pop-up, or brings it last (on top); `None` clears the selection.
    pub fn select(&mut self, sel: Option<Selection>) {
        self.selected = sel;
        if let Some(s) = sel {
            self.popups.retain(|p| *p != s);
            self.popups.push(s);
            if self.popups.len() > MAX_POPUPS {
                self.popups.remove(0);
            }
        }
    }

    /// Shows the selected part's main layer once its facts are known, if
    /// [`SystemsScreen::open_at`] asked for it.
    pub fn settle_layer(&mut self, facts: &[rules::PartFacts]) {
        if !self.pick_layer {
            return;
        }
        let Some(Selection::Part(id)) = self.selected else {
            self.pick_layer = false;
            return;
        };
        if let Some(f) = facts.iter().find(|f| f.id == id) {
            self.layer = Layer::main(f);
            self.pick_layer = false;
        }
    }
}

/// The screen's resources and systems.
pub struct SystemsPlugin;

impl Plugin for SystemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SystemsScreen>()
            .init_gizmo_group::<view::NetworkGizmos>()
            .add_systems(Startup, (view::setup, view::setup_gizmos))
            .add_systems(
                Update,
                keys.after(crate::commands::update_context).before(crate::commands::keys::read_controls),
            )
            .add_systems(Update, view::update.after(crate::state::advance))
            .add_systems(bevy_egui::EguiPrimaryContextPass, ui::draw.after(crate::hud::draw));
    }
}

/// Tab opens and closes the screen (not while paused or typing).
pub fn keys(keys: Res<ButtonInput<KeyCode>>, ctx: Res<InputContext>, mut screen: ResMut<SystemsScreen>) {
    if pressed(&keys, Action::SystemsScreen) && ctx.allows(Keys::Menus) && *ctx != InputContext::Paused {
        screen.open = !screen.open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::craft::command::ShipCommand;

    #[test]
    fn drafts_follow_the_vessel_unless_edited() {
        let s =
            |name: &str| Stage { name: name.into(), key: None, actions: vec![ShipCommand::ArmChutes], pinned: false };
        let v = VesselId(4);
        let d = Drafts::sync(None, v, "c", &[s("A")], &[s("F")]);
        assert_eq!((d.stages[0].name.as_str(), d.functions[0].name.as_str(), d.dirty), ("A", "F", [false; 2]));
        // The vessel's lists change (a separation): clean drafts follow.
        let d = Drafts::sync(Some(d), v, "c", &[s("B")], &[]);
        assert_eq!((d.stages[0].name.as_str(), d.functions.len()), ("B", 0));
        // An edited list is kept, the other follows.
        let mut edited = d.clone();
        edited.stages[0].name = "mine".into();
        edited.dirty[0] = true;
        let d = Drafts::sync(Some(edited), v, "c", &[s("C")], &[s("G")]);
        assert_eq!((d.stages[0].name.as_str(), d.functions[0].name.as_str()), ("mine", "G"));
        // Another vessel or craft starts over.
        let other = Drafts::sync(Some(d.clone()), VesselId(5), "c", &[s("X")], &[]);
        assert_eq!((other.stages[0].name.as_str(), other.dirty), ("X", [false; 2]));
        let other = Drafts::sync(Some(d), v, "c@1", &[s("Y")], &[]);
        assert_eq!(other.stages[0].name, "Y");
    }

    /// `open_at` opens at the layer and part given (the part's pop-up
    /// open), and keeps the others; a part without a layer picks its main
    /// layer once its facts are known.
    #[test]
    fn open_at_a_layer_and_a_part() {
        let mut screen = SystemsScreen { selected: Some(Selection::Part(3)), ..SystemsScreen::default() };
        screen.open_at(None, None);
        assert!(screen.open && !screen.pick_layer);
        assert_eq!((screen.layer, screen.selected), (Layer::Physical, Some(Selection::Part(3))));
        screen.open_at(Some(Layer::Electrical), Some(9));
        assert_eq!((screen.layer, screen.selected), (Layer::Electrical, Some(Selection::Part(9))));
        assert_eq!(screen.popups, [Selection::Part(9)]);
        // A part alone: its main layer (a tank: plumbing) once its facts are there.
        screen.open_at(None, Some(4));
        assert!(screen.pick_layer);
        screen.settle_layer(&[]);
        assert_eq!(screen.layer, Layer::Electrical, "not known yet");
        let tank = rules::PartFacts { id: 4, tanks: vec![("LOX".into(), None, 1.0)], ..Default::default() };
        screen.settle_layer(&[tank]);
        assert_eq!((screen.layer, screen.pick_layer), (Layer::Plumbing, false));
        assert_eq!(screen.popups, [Selection::Part(9), Selection::Part(4)]);
    }

    /// Selecting opens a pop-up, or brings it on top; at most
    /// [`MAX_POPUPS`], the oldest closing.
    #[test]
    fn pop_ups_open_on_selection() {
        let mut screen = SystemsScreen::default();
        for p in [1, 2, 1, 3, 4, 5] {
            screen.select(Some(Selection::Part(p)));
        }
        let want: Vec<Selection> = [2, 1, 3, 4, 5].map(Selection::Part)[1..].to_vec();
        assert_eq!(screen.popups, want);
        screen.select(None);
        assert_eq!((screen.selected, screen.popups.len()), (None, MAX_POPUPS));
    }
}
