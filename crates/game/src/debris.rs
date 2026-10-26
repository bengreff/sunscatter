//! Breakups and debris in the game (D083, `docs/features/parts.md`): the
//! wiring of `sim::vessel::split` and `sim::vessel::debris`; the rules are
//! the sim's.
//!
//! * A vessel that broke up at a live tick stops there, so the clock holds
//!   at it like at any vessel behind. Its pieces' craft (their designs: a
//!   small piece in tens of milliseconds, a large one in a second or so)
//!   are built on a background task; then the split is applied and the
//!   pieces join the fleet. Breakups are applied earliest first
//!   ([`next_breakup`]), so the fleet's order and the vessels' ids do not
//!   depend on the frames.
//! * Every frame, the vessels the debris rule finds are deleted, with a
//!   toast.

use crate::camera::CameraRig;
use crate::interface::toasts::Toasts;
use crate::state::SimState;
use crate::tracking::{self, Tracked};
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};
use sim::craft::Craft;
use sim::time::Epoch;
use sim::vessel::{SplitPlan, Vessel, VesselId};
use std::sync::Arc;

/// A breakup's plan and its pieces' craft, built.
type Built = (SplitPlan, Vec<Arc<Craft>>);

/// The breakup whose pieces are being built: the vessel, its breakup's
/// epoch, the task.
#[derive(Resource, Default)]
pub struct Breakups {
    building: Option<(VesselId, Epoch, Task<Built>)>,
}

/// The fleet index of the earliest pending breakup (the lowest index among
/// equal epochs).
pub fn next_breakup(fleet: &[Vessel]) -> Option<usize> {
    (0..fleet.len())
        .filter_map(|i| fleet[i].breakup().map(|b| (b.epoch, i)))
        .min_by(|a, b| a.0.seconds_since(b.0).total_cmp(&0.0).then(a.1.cmp(&b.1)))
        .map(|(_, i)| i)
}

/// Applies a built breakup, starts building the next, deletes debris.
pub fn update(
    time: Res<Time>,
    mut sim: ResMut<SimState>,
    mut breakups: ResMut<Breakups>,
    mut toasts: ResMut<Toasts>,
    mut tracked: ResMut<Tracked>,
    mut rig: ResMut<CameraRig>,
) {
    let now = time.elapsed_secs_f64();
    let sim = &mut *sim;
    if let Some((id, epoch, task)) = breakups.building.as_mut() {
        if let Some((plan, crafts)) = check_ready(task) {
            let (id, epoch) = (*id, *epoch);
            breakups.building = None;
            let at = sim.index_of(id).filter(|&i| sim.fleet[i].breakup().is_some_and(|b| b.epoch == epoch));
            if let Some(i) = at {
                let pieces = sim.fleet[i].split(&sim.world, &plan, &crafts, &mut sim.vessel_ids);
                let wrecks = pieces.iter().filter(|p| p.destruction().is_some()).count();
                toasts.push(now, format!("Vessel {id} broke up: {} pieces ({wrecks} wrecks)", pieces.len()), true);
                sim.fleet.extend(pieces);
            }
        }
    }
    if breakups.building.is_none() {
        if let Some(i) = next_breakup(&sim.fleet) {
            let v = &sim.fleet[i];
            if let (Some(plan), Some(b)) = (v.plan_breakup(), v.breakup()) {
                let (id, epoch) = (v.id(), b.epoch);
                let task = AsyncComputeTaskPool::get().spawn(async move {
                    let crafts = plan.crafts();
                    (plan, crafts)
                });
                breakups.building = Some((id, epoch, task));
            }
        }
    }
    for i in sim::vessel::debris::debris(&sim.world, sim.clock, &sim.fleet, sim.active).into_iter().rev() {
        let id = sim.fleet[i].id();
        if tracking::delete_vessel(sim, &mut tracked, &mut rig, id) {
            toasts.push(now, format!("Debris {id} deleted: dead, uncontrollable, 10 km from any craft"), false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::craft::test_craft;

    #[test]
    fn no_breakup_no_work() {
        let sim = SimState::new();
        assert_eq!(next_breakup(&sim.fleet), None);
        // The test craft is one part: never debris while it can be commanded.
        assert!(sim.fleet[0].controllable());
        assert_eq!(sim::vessel::debris::debris(&sim.world, sim.clock, &sim.fleet, 0), Vec::<usize>::new());
        assert_eq!(test_craft().parts.len(), 1);
    }
}
