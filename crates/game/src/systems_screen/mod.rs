//! The systems screen (D107; ship-model.md §8): a screen of its own, like
//! the tracking station, where the active craft is shown alone by layer
//! (physical, structural, thermal, plumbing, electrical, data, health),
//! every part and component can be inspected and commanded, and the stage
//! sequence and saved functions are edited (saved with the vessel, D107:
//! vessel state, not settings) and pinned to the mini systems panel.
//!
//! F8 opens and closes it (also the mini panel's SYSTEMS button); Esc
//! closes it. While it is open the clock runs, warp keys act and flight
//! keys do not (`commands::InputContext::Systems`).
//!
//! Pure rules: [`rules`] (facts, layer colours, tree, camera, pick),
//! [`inspect`] (everything about a selection, its commands), [`edit`] (the
//! editors' pick list and edits). Bevy wiring: [`view`] (the 3D view in an
//! image), [`ui`] (the screen, its editors).

pub mod edit;
pub mod inspect;
pub mod rules;
mod ui;
pub mod view;

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
        }
    }
}

/// The screen's resources and systems.
pub struct SystemsPlugin;

impl Plugin for SystemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SystemsScreen>()
            .add_systems(Startup, view::setup)
            .add_systems(Update, keys.after(crate::commands::update_context).before(crate::state::read_controls))
            .add_systems(Update, view::update.after(crate::state::advance))
            .add_systems(bevy_egui::EguiPrimaryContextPass, ui::draw.after(crate::hud::draw));
    }
}

/// F8 opens and closes the screen (not while paused or typing).
pub fn keys(keys: Res<ButtonInput<KeyCode>>, ctx: Res<InputContext>, mut screen: ResMut<SystemsScreen>) {
    if keys.just_pressed(KeyCode::F8) && ctx.allows(Keys::Menus) && *ctx != InputContext::Paused {
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
}
