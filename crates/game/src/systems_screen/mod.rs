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
//! The left column shows the layer as a whole ([`summary`], by default) or
//! the parts list, by its switch ([`Left`]). A click on a part (in the
//! list, the view or a summary's row) selects it and isolates it: the
//! other parts are ghosted, the camera frames it, and the right column
//! shows everything about it in one layout ([`inspect`]); there are no
//! pop-ups (owner, 2026-10-28). Esc, or the right column's ×, puts the
//! layer view back as it was ([`SystemsScreen::back`]: the layer, the
//! camera, the left column, no selection; owner, 2026-10-31); a second Esc
//! closes the screen.
//!
//! The thermal layer draws the thermal network itself ([`thermal`]): each
//! skin cell by its own temperature, the interior nodes behind a cut-away.
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
pub mod thermal;
mod ui;
pub mod view;

use crate::commands::keys::{pressed, Action};
use crate::commands::{InputContext, Keys};
use bevy::prelude::*;
use inspect::Selection;
use rules::Layer;
use sim::craft::command::Stage;
use sim::vessel::VesselId;

/// What the left column shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Left {
    /// The craft as a whole in the layer shown.
    #[default]
    SystemWide,
    Parts,
}

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

/// The orbit camera's pose: yaw and pitch (rad), distance (m; `None`:
/// framed to what it looks at).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: Option<f32>,
}

/// The layer view as it was before a part was isolated, to put back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Back {
    pub layer: Layer,
    pub pose: Pose,
    pub left: Left,
    pub selected: Option<Selection>,
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
    pub left: Left,
    /// Opened at a part without a layer: show the part's main layer once
    /// its facts are known ([`SystemsScreen::settle_layer`]).
    pub pick_layer: bool,
    /// A part is isolated (the selection's): the layer view to put back.
    pub back: Option<Back>,
    /// The thermal layer's cut-away: 0 nothing cut, 1 all (the share of
    /// the depth of what the camera looks at, from its side).
    pub cut: f32,
}

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
            left: Left::SystemWide,
            pick_layer: false,
            back: None,
            cut: DEFAULT_CUT,
        }
    }
}

/// The thermal layer's cut-away when the screen starts: half the craft
/// toward the camera cut away, so the interior nodes show with the skin
/// cells (INTERFACE's default, open for the owner).
pub const DEFAULT_CUT: f32 = 0.5;

impl SystemsScreen {
    /// Opens the screen, at `layer` and with `part` selected (shown in the
    /// right column) when given; a part without a layer shows the part's
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

    /// Selects `sel` and isolates its part (a component's part): the
    /// first selection keeps the layer view to put back, and the camera
    /// frames the part. `None` puts the layer view back ([`Self::back`]).
    pub fn select(&mut self, sel: Option<Selection>) {
        let Some(sel) = sel else {
            self.back();
            return;
        };
        if self.back.is_none() {
            self.back = Some(Back { layer: self.layer, pose: self.pose(), left: self.left, selected: self.selected });
        }
        if self.selected != Some(sel) {
            self.distance = None;
        }
        self.selected = Some(sel);
    }

    /// Whether a part is isolated.
    pub fn isolated(&self) -> bool {
        self.back.is_some()
    }

    /// The camera's pose now.
    pub fn pose(&self) -> Pose {
        Pose { yaw: self.yaw, pitch: self.pitch, distance: self.distance }
    }

    /// Puts back the layer view an isolation started from: its layer,
    /// camera, left column and selection (Esc, or the right column's ×).
    pub fn back(&mut self) {
        let Some(b) = self.back.take() else {
            self.selected = None;
            return;
        };
        self.layer = b.layer;
        (self.yaw, self.pitch, self.distance) = (b.pose.yaw, b.pose.pitch, b.pose.distance);
        self.left = b.left;
        self.selected = b.selected;
        self.pick_layer = false;
    }

    /// Closes the screen; an isolated part is put back first, so the screen
    /// opens again on the layer view.
    pub fn close(&mut self) {
        if self.isolated() {
            self.back();
        }
        self.open = false;
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
            // The layer view to go back to is the part's layer too.
            if let Some(b) = self.back.as_mut() {
                b.layer = self.layer;
            }
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
        if screen.open {
            screen.close();
        } else {
            screen.open = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::craft::command::ShipCommand;

    #[test]
    fn drafts_follow_the_vessel_unless_edited() {
        let s = |name: &str| Stage {
            name: name.into(),
            key: None,
            actions: vec![ShipCommand::ArmChutes],
            pinned: false,
            abort: false,
        };
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

    /// `open_at` opens at the layer and part given (the part selected); a
    /// part without a layer picks its main layer once its facts are known.
    #[test]
    fn open_at_a_layer_and_a_part() {
        let mut screen = SystemsScreen { selected: Some(Selection::Part(3)), ..SystemsScreen::default() };
        screen.open_at(None, None);
        assert!(screen.open && !screen.pick_layer);
        assert_eq!((screen.layer, screen.selected), (Layer::Physical, Some(Selection::Part(3))));
        screen.open_at(Some(Layer::Electrical), Some(9));
        assert_eq!((screen.layer, screen.selected), (Layer::Electrical, Some(Selection::Part(9))));
        // A part alone: its main layer (a tank: plumbing) once its facts are there.
        screen.open_at(None, Some(4));
        assert!(screen.pick_layer);
        screen.settle_layer(&[]);
        assert_eq!(screen.layer, Layer::Electrical, "not known yet");
        let tank = rules::PartFacts { id: 4, tanks: vec![("LOX".into(), None, 1.0)], ..Default::default() };
        screen.settle_layer(&[tank]);
        assert_eq!(
            (screen.layer, screen.pick_layer, screen.selected),
            (Layer::Plumbing, false, Some(Selection::Part(4)))
        );
    }

    /// The left column starts on the craft as a whole; a selection is one
    /// part or component (no pop-ups), the last one clicked.
    #[test]
    fn the_left_starts_system_wide_and_one_thing_is_selected() {
        let mut screen = SystemsScreen::default();
        assert_eq!(screen.left, Left::SystemWide);
        for p in [1, 2, 1, 3] {
            screen.select(Some(Selection::Part(p)));
        }
        assert_eq!(screen.selected, Some(Selection::Part(3)));
        screen.select(None);
        assert_eq!(screen.selected, None);
    }

    /// Isolating a part and going back (table): each step's action, then
    /// (isolated, layer, yaw, distance, left, selection). The layer view is
    /// kept from the first isolation and put back whole, whatever changed
    /// meanwhile; the camera frames each newly isolated part.
    #[test]
    fn isolation_goes_back_to_the_layer_view_as_it_was() {
        #[derive(Clone, Copy, Debug)]
        enum Do {
            Select(u32),
            Component(usize),
            SetLayer(Layer),
            Orbit(f32, f32),
            SetLeft(Left),
            Back,
            Clear,
        }
        use Do::*;
        let mut s =
            SystemsScreen { open: true, layer: Layer::Thermal, yaw: 0.6, distance: Some(80.0), ..Default::default() };
        let p = |id| Some(Selection::Part(id));
        type Want = (bool, Layer, f32, Option<f32>, Left, Option<Selection>);
        let steps: [(Do, Want); 12] = [
            (Select(4), (true, Layer::Thermal, 0.6, None, Left::SystemWide, p(4))),
            (Orbit(1.0, 12.0), (true, Layer::Thermal, 1.0, Some(12.0), Left::SystemWide, p(4))),
            (SetLayer(Layer::Plumbing), (true, Layer::Plumbing, 1.0, Some(12.0), Left::SystemWide, p(4))),
            (SetLeft(Left::Parts), (true, Layer::Plumbing, 1.0, Some(12.0), Left::Parts, p(4))),
            // Another part: framed anew, the same layer view kept.
            (Select(7), (true, Layer::Plumbing, 1.0, None, Left::Parts, p(7))),
            (Select(7), (true, Layer::Plumbing, 1.0, None, Left::Parts, p(7))),
            (Component(2), (true, Layer::Plumbing, 1.0, None, Left::Parts, Some(Selection::Component(2)))),
            (Back, (false, Layer::Thermal, 0.6, Some(80.0), Left::SystemWide, None)),
            // Nothing isolated: Esc's back changes nothing.
            (Back, (false, Layer::Thermal, 0.6, Some(80.0), Left::SystemWide, None)),
            (Select(2), (true, Layer::Thermal, 0.6, None, Left::SystemWide, p(2))),
            // The right column's ×.
            (Clear, (false, Layer::Thermal, 0.6, Some(80.0), Left::SystemWide, None)),
            (Select(3), (true, Layer::Thermal, 0.6, None, Left::SystemWide, p(3))),
        ];
        for (i, (step, want)) in steps.into_iter().enumerate() {
            match step {
                Select(id) => s.select(p(id)),
                Component(c) => s.select(Some(Selection::Component(c))),
                SetLayer(l) => s.layer = l,
                Orbit(yaw, d) => (s.yaw, s.distance) = (yaw, Some(d)),
                SetLeft(l) => s.left = l,
                Back => s.back(),
                Clear => s.select(None),
            }
            assert_eq!((s.isolated(), s.layer, s.yaw, s.distance, s.left, s.selected), want, "step {i}: {step:?}");
        }
        // Closing puts the layer view back, so the screen opens on it.
        s.close();
        assert_eq!((s.open, s.isolated(), s.selected, s.distance), (false, false, None, Some(80.0)));
    }

    /// Opened at a part from flight (a card's "more", a menu's link): the
    /// part isolated on its main layer, and going back stays on that layer.
    #[test]
    fn opened_at_a_part_goes_back_to_its_layer() {
        let mut s = SystemsScreen::default();
        s.open_at(None, Some(4));
        assert!(s.isolated());
        let tank = rules::PartFacts { id: 4, tanks: vec![("LOX".into(), None, 1.0)], ..Default::default() };
        s.settle_layer(&[tank]);
        assert_eq!(s.layer, Layer::Plumbing);
        s.back();
        assert_eq!((s.layer, s.selected, s.open), (Layer::Plumbing, None, true));
    }
}
