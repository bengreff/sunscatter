//! The staging bar (D111): a thin column at the flight view's right edge,
//! the stages still to run stacked last to next (the next at the bottom),
//! then the active stage, then the switch between the vacuum's figures
//! (the default, D120) and the current air's. Each stage shows where it
//! applies a bar per propellant, the burnout time and the Δv; the active
//! one also thrust, TWR, mass now and at burnout, mean Isp
//! (`sim::systems::estimate`, asked a few times a second, never per
//! frame). Each lists what its commands
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

/// The air the figures are taken in (D120: vacuum by default, the current
/// air a choice; never chosen stage by stage).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Air {
    #[default]
    Vacuum,
    /// The ambient pressure now, held over every burn.
    Current,
}

/// The ambient pressure the estimates take (Pa) in `air`, the pressure
/// around the vessel now being `now`.
pub fn ambient(air: Air, now: f64) -> f64 {
    match air {
        Air::Vacuum => 0.0,
        Air::Current => now,
    }
}

/// The bar's state: the air switch, the last estimates, a delete waiting
/// for its confirmation, and where each icon was drawn (for the cards'
/// lines).
#[derive(Resource, Default)]
pub struct StageBar {
    pub air: Air,
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
    pub air: Air,
    pub sequence: u32,
    pub stages: Vec<Stage>,
    pub estimates: Estimates,
}

/// Whether estimates taken from (`vessel`, `sequence`, `stages`, `air`)
/// at `at` still stand at `now`: the same vessel, sequence and switch, and
/// younger than [`REFRESH`].
pub fn fresh(c: &Cached, vessel: VesselId, sequence: u32, stages: &[Stage], air: Air, now: f64) -> bool {
    c.vessel == vessel && c.sequence == sequence && c.air == air && c.stages == stages && now - c.at < REFRESH
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
    let Some(state) = v.ship() else {
        bar.cache = None;
        return;
    };
    let now = time.elapsed_secs_f64();
    if bar.cache.as_ref().is_some_and(|c| fresh(c, v.id(), state.sequence, &state.stages, bar.air, now)) {
        return;
    }
    let Some(estimates) = estimates_of(&sim, sim.active, bar.air) else { return };
    let air = bar.air;
    bar.cache = Some(Cached {
        vessel: v.id(),
        at: now,
        air,
        sequence: state.sequence,
        stages: state.stages.clone(),
        estimates,
    });
}

/// Vessel `i`'s stage estimates in `air` (`sim::systems::estimate`, from
/// what it knows), as the bar shows them for the active vessel and the
/// agent reads them for any (`get_vessel`); `None` for a vessel without
/// parts.
pub fn estimates_of(sim: &SimState, i: usize, air: Air) -> Option<Estimates> {
    let v = &sim.fleet[i];
    let (def, state) = (v.ship_def()?, v.ship()?);
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
        throttle: v.controls().throttle,
        p: ambient(air, motion.map_or(0.0, |m| m.pressure)),
        g: motion.map(|m| m.gravity).filter(|_| known),
    };
    Some(estimate::estimate(&def, state, &inputs))
}

/// What is aboard vessel `v` now as it knows it (`estimate::aboard`: the
/// mass the bar's burn starts from, to the sensors' precision, the pad's
/// mount left out), for every readout of its mass; `None` for a vessel
/// without a ship definition.
pub fn aboard_of(v: &sim::vessel::Vessel) -> Option<estimate::Aboard> {
    let (def, state) = (v.ship_def()?, v.ship()?);
    let hull = estimate::hull(&v.craft.design().parts);
    Some(estimate::aboard(&def, state, &hull, &v.knowledge()))
}

/// Vessel `v`'s rows on the bar with `estimates` (its parachutes' look
/// from their state); `None` for a vessel without a ship definition.
pub fn rows_of(v: &sim::vessel::Vessel, estimates: &Estimates) -> Option<Vec<rows::Row>> {
    let (def, state) = (v.ship_def()?, v.ship()?);
    let chute_parts: Vec<u32> = v.craft.design().parts.iter().filter(|p| p.chute).map(|p| p.id).collect();
    Some(rows::rows(&def, state, estimates, (&chute_parts, icons::chute_look(&v.chute))))
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
    let Some(cache) = bar.cache.as_ref().filter(|c| c.vessel == v.id()) else { return Ok(()) };
    let rows = rows_of(v, &cache.estimates).unwrap_or_default();
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
                cmds.write(GameCommand::Stage(v.id()));
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
        let stages = vec![Stage { name: "a".into(), key: None, actions: vec![], pinned: false, abort: false }];
        let c = Cached {
            vessel: VesselId(1),
            at: 10.0,
            air: Air::Vacuum,
            sequence: 0,
            stages: stages.clone(),
            estimates: Estimates::default(),
        };
        let mut other = stages.clone();
        other[0].name = "b".into();
        // (vessel, sequence, stages, air, now, fresh)
        let table = [
            (1, 0, &stages, Air::Vacuum, 10.1, true),
            (1, 0, &stages, Air::Vacuum, 10.3, false),
            (2, 0, &stages, Air::Vacuum, 10.1, false),
            (1, 1, &stages, Air::Vacuum, 10.1, false),
            (1, 0, &other, Air::Vacuum, 10.1, false),
            (1, 0, &stages, Air::Current, 10.1, false),
        ];
        for (vessel, sequence, s, air, now, want) in table {
            assert_eq!(fresh(&c, VesselId(vessel), sequence, s, air, now), want, "{vessel} {sequence} {air:?} {now}");
        }
    }

    /// D120: the bar opens in vacuum (the pad's air does not cut the second
    /// stage's Δv to nothing); the current air is the switch's other side.
    #[test]
    fn the_figures_are_in_vacuum_by_default() {
        assert_eq!(StageBar::default().air, Air::Vacuum);
        // (air, pressure around the vessel now, the estimates' ambient)
        let table = [
            (Air::Vacuum, 101_325.0, 0.0),
            (Air::Vacuum, 0.0, 0.0),
            (Air::Current, 101_325.0, 101_325.0),
            (Air::Current, 2_000.0, 2_000.0),
        ];
        for (air, now, want) in table {
            assert_eq!(ambient(air, now), want, "{air:?} {now}");
        }
    }
}
