//! The staging bar (D111): a thin column at the flight view's right edge,
//! the stages still to run stacked last to next (the next at the bottom),
//! then the active stage, then the switch between the current air's and
//! the vacuum's figures. Each stage shows where it applies a bar per
//! propellant, the burnout time and the Δv; the active one also thrust,
//! TWR, mass now and at burnout, mean Isp (`sim::systems::estimate`, asked
//! a few times a second, never per frame). Each lists what its commands
//! act on as icons (`icons`): a click opens the part's card
//! (`interface::part_card`), a drag moves the command to another stage; a
//! stage's header drags the stage; × deletes it (confirmed). Edits are ship
//! commands (`ShipCommand::Sequence`, D108), sent by the commands' route.
//!
//! The right edge is the bar's (NAV's menus live in the left dock; part
//! cards stack to its left): [`WIDTH`] is its width for their layout.

pub mod draw;
pub mod icons;
pub mod rows;

use crate::commands::GameCommand;
use crate::state::SimState;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use sim::craft::command::Stage;
use sim::systems::estimate::{self, Estimates, Inputs};
use sim::vessel::VesselId;

/// The bar's width (logical px), for the layout of what sits beside it.
pub const WIDTH: f32 = 184.0;
/// The bar's egui area.
pub const AREA: &str = "stage_bar";
/// How often the estimates are taken again (s of real time).
const REFRESH: f64 = 0.25;

/// The bar's state: the air switch, the last estimates, a delete waiting
/// for its confirmation, and where each icon was drawn (for the cards'
/// lines).
#[derive(Resource, Default)]
pub struct StageBar {
    /// Vacuum figures instead of the current air's.
    pub vacuum: bool,
    pub cache: Option<Cached>,
    /// The stage (index into the sequence) whose delete waits.
    pub confirm: Option<usize>,
    /// Each icon's square last frame: (part id, rect).
    pub icons: Vec<(u32, egui::Rect)>,
}

/// Estimates taken for a vessel, with what they were taken from.
pub struct Cached {
    pub vessel: VesselId,
    pub at: f64,
    pub vacuum: bool,
    pub sequence: u32,
    pub stages: Vec<Stage>,
    pub estimates: Estimates,
}

/// Whether estimates taken from (`vessel`, `sequence`, `stages`, `vacuum`)
/// at `at` still stand at `now`: the same vessel, sequence and switch, and
/// younger than [`REFRESH`].
pub fn fresh(c: &Cached, vessel: VesselId, sequence: u32, stages: &[Stage], vacuum: bool, now: f64) -> bool {
    c.vessel == vessel && c.sequence == sequence && c.vacuum == vacuum && c.stages == stages && now - c.at < REFRESH
}

pub struct StageBarPlugin;

impl Plugin for StageBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StageBar>()
            .add_systems(Update, refresh.after(crate::state::advance))
            .add_systems(bevy_egui::EguiPrimaryContextPass, show.after(crate::hud::draw));
    }
}

/// Takes the active vessel's estimates again when they are stale.
pub fn refresh(sim: Res<SimState>, time: Res<Time>, mut bar: ResMut<StageBar>) {
    let v = sim.ship();
    let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else {
        bar.cache = None;
        return;
    };
    let now = time.elapsed_secs_f64();
    if bar.cache.as_ref().is_some_and(|c| fresh(c, v.id(), state.sequence, &state.stages, bar.vacuum, now)) {
        return;
    }
    let (anchor, r, v_rel) = v.state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let motion = sim::landing::nearest_surface(&sim.world, &snap, anchor, r)
        .map(|b| sim::landing::surface_motion(&sim.world, &snap, anchor, b, r, v_rel));
    let knowledge = v.knowledge();
    let hull = estimate::hull(&v.craft.design().parts);
    let known = knowledge.knows(sim::systems::Quantity::Position);
    let inputs = Inputs {
        hull: &hull,
        knowledge: &knowledge,
        throttle: sim.controls().throttle,
        p: if bar.vacuum { 0.0 } else { motion.map_or(0.0, |m| m.pressure) },
        g: motion.map(|m| m.gravity).filter(|_| known),
    };
    let estimates = estimate::estimate(&def, state, &inputs);
    let vacuum = bar.vacuum;
    bar.cache = Some(Cached {
        vessel: v.id(),
        at: now,
        vacuum,
        sequence: state.sequence,
        stages: state.stages.clone(),
        estimates,
    });
}

/// Draws the bar in the flight view and sends what it asks for.
pub fn show(
    mut contexts: EguiContexts,
    sim: Res<SimState>,
    station: Res<crate::tracking::TrackingStation>,
    systems: Res<crate::systems_screen::SystemsScreen>,
    mut bar: ResMut<StageBar>,
    mut cards: ResMut<crate::interface::part_card::PartCards>,
    mut cmds: MessageWriter<GameCommand>,
) -> Result {
    bar.icons.clear();
    if station.open || systems.open {
        return Ok(());
    }
    let v = sim.ship();
    let rows = match (v.ship_def(), v.ship()) {
        (Some(def), Some(state)) => {
            let Some(cache) = bar.cache.as_ref().filter(|c| c.vessel == v.id()) else { return Ok(()) };
            let chute_parts: Vec<u32> = v.craft.design().parts.iter().filter(|p| p.chute).map(|p| p.id).collect();
            rows::rows(&def, state, &cache.estimates, (&chute_parts, icons::chute_look(&v.chute)))
        }
        // The test craft: its engine and parachutes (`staging::stack`).
        _ => rows::plain(&crate::staging::stack_of(v, sim.controls())),
    };
    let ctx = contexts.ctx_mut()?;
    let out = draw::bar(ctx, &rows, &mut bar);
    for a in out {
        match a {
            draw::Action::Edit(e) => {
                let commands = vec![sim::craft::command::ShipCommand::Sequence(e)];
                cmds.write(GameCommand::Ship { vessel: v.id(), commands });
            }
            draw::Action::Open(part) => cards.open(v.id(), part, crate::interface::part_card::From::Bar),
            draw::Action::NextStage => {
                cmds.write(GameCommand::NextStage);
            }
            draw::Action::Again(stage) => {
                cmds.write(GameCommand::StageAgain { vessel: v.id(), stage });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_are_taken_again_when_anything_they_read_changes() {
        let stages = vec![Stage { name: "a".into(), key: None, actions: vec![], pinned: false }];
        let c = Cached {
            vessel: VesselId(1),
            at: 10.0,
            vacuum: false,
            sequence: 0,
            stages: stages.clone(),
            estimates: Estimates::default(),
        };
        let mut other = stages.clone();
        other[0].name = "b".into();
        // (vessel, sequence, stages, vacuum, now, fresh)
        let table = [
            (1, 0, &stages, false, 10.1, true),
            (1, 0, &stages, false, 10.3, false),
            (2, 0, &stages, false, 10.1, false),
            (1, 1, &stages, false, 10.1, false),
            (1, 0, &other, false, 10.1, false),
            (1, 0, &stages, true, 10.1, false),
        ];
        for (vessel, sequence, s, vacuum, now, want) in table {
            assert_eq!(
                fresh(&c, VesselId(vessel), sequence, s, vacuum, now),
                want,
                "{vessel} {sequence} {vacuum} {now}"
            );
        }
    }
}
