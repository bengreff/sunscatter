//! Breakups and debris in the game (D083, `docs/features/parts.md`): the
//! wiring of `sim::vessel::split` and `sim::vessel::debris`; the rules are
//! the sim's.
//!
//! * A vessel that broke up at a live tick stops there, so the clock holds
//!   at it like at any vessel behind. Its pieces' craft (their designs: a
//!   small piece in tens of milliseconds, a large one in a second or so)
//!   are built on a background task; then the split is applied and the
//!   pieces join the fleet. Breakups are applied earliest first (by epoch,
//!   then vessel id: [`next_breakup`]), each only once no vessel can still
//!   break up before it ([`ready_breakup`]: every other vessel is at or
//!   past its epoch, waits for its own split, or has stopped for good), so
//!   the fleet's order and the vessels' ids do not depend on the frames.
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

/// The breakup whose pieces are being built (the vessel, its breakup's
/// epoch, the task), and one built, waiting until it is ready.
#[derive(Resource, Default)]
pub struct Breakups {
    building: Option<(VesselId, Epoch, Task<Built>)>,
    built: Option<(VesselId, Epoch, Built)>,
}

/// What the breakup order reads of a vessel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standing {
    pub id: VesselId,
    pub time: Epoch,
    /// Its pending breakup's epoch.
    pub breakup: Option<Epoch>,
    /// Its motion has ended for good (`crate::fleet::stopped`).
    pub stopped: bool,
}

/// The fleet's standings.
pub fn standings(fleet: &[Vessel]) -> Vec<Standing> {
    (fleet.iter())
        .map(|v| Standing {
            id: v.id(),
            time: v.time,
            breakup: v.breakup().map(|b| b.epoch),
            stopped: crate::fleet::stopped(v),
        })
        .collect()
}

/// The index of the earliest pending breakup (by epoch, then the lowest
/// vessel id).
pub fn next_breakup(fleet: &[Standing]) -> Option<usize> {
    (0..fleet.len())
        .filter_map(|i| fleet[i].breakup.map(|e| (e, fleet[i].id, i)))
        .min_by(|a, b| a.0.seconds_since(b.0).total_cmp(&0.0).then(a.1 .0.cmp(&b.1 .0)))
        .map(|(_, _, i)| i)
}

/// [`next_breakup`], once no vessel can still break up before it: every
/// other vessel is at or past its epoch, waits for its own split (at a
/// later epoch, or a later id), or has stopped for good. One frame or
/// several, the breakups are applied in the same order.
pub fn ready_breakup(fleet: &[Standing]) -> Option<usize> {
    let i = next_breakup(fleet)?;
    let epoch = fleet[i].breakup?;
    fleet.iter().all(|v| v.breakup.is_some() || v.stopped || v.time >= epoch).then_some(i)
}

/// The toast of vessel `id`'s breakup by `plan` that made `new` vessels
/// (the pieces but the first, which stays the vessel), and whether it is an
/// alert: none when nothing came off (a craft taking a new configuration);
/// a separation the craft fired on command (nothing destroyed or torn) is
/// told as one, not as a breakup.
pub fn breakup_toast(id: VesselId, plan: &SplitPlan, new: usize, separation: bool) -> Option<(String, bool)> {
    let wrecks = plan.destroyed.iter().filter(|d| d.is_some()).count();
    if new == 0 {
        None
    } else if separation && wrecks == 0 {
        let pieces = if new == 1 { "a new vessel".to_string() } else { format!("{new} new vessels") };
        Some((format!("Vessel {id} separated: {pieces}"), false))
    } else {
        Some((format!("Vessel {id} broke up: {} pieces ({wrecks} wrecks)", new + 1), true))
    }
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
        if let Some(built) = check_ready(task) {
            breakups.built = Some((*id, *epoch, built));
            breakups.building = None;
        }
    }
    let standing = standings(&sim.fleet);
    let is = |i: Option<usize>, id: VesselId, epoch: Epoch| {
        i.is_some_and(|i| standing[i].id == id && standing[i].breakup == Some(epoch))
    };
    let next = next_breakup(&standing);
    if let Some((id, epoch, _)) = breakups.built.as_ref() {
        let (id, epoch) = (*id, *epoch);
        if is(ready_breakup(&standing), id, epoch) {
            let (_, _, (plan, crafts)) = breakups.built.take().expect("built");
            let i = sim.index_of(id).expect("a ready breakup's vessel");
            let separation = sim.fleet[i]
                .breakup()
                .is_some_and(|b| !b.separated.is_empty() && b.destroyed.is_empty() && b.torn.is_empty());
            let pieces = sim.fleet[i].split(&sim.world, &plan, &crafts, &mut sim.vessel_ids);
            if let Some((text, alert)) = breakup_toast(id, &plan, pieces.len(), separation) {
                toasts.push(now, text, alert);
            }
            sim.fleet.extend(pieces);
        } else if !is(next, id, epoch) {
            // An earlier breakup came up (or this one went): built again
            // in its turn (the craft are kept per id).
            breakups.built = None;
        }
    }
    if breakups.building.is_none() && breakups.built.is_none() {
        if let Some(i) = next_breakup(&standings(&sim.fleet)) {
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
        assert_eq!(next_breakup(&standings(&sim.fleet)), None);
        // The test craft is one part: never debris while it can be commanded.
        assert!(sim.fleet[0].controllable());
        assert_eq!(sim::vessel::debris::debris(&sim.world, sim.clock, &sim.fleet, 0), Vec::<usize>::new());
        assert_eq!(test_craft().parts.len(), 1);
    }

    /// The order the rule applies breakups in, one per call, over frames
    /// given as each vessel's (time, breakup epoch) at the frame's end (a
    /// split vessel's breakup is gone in the frames after).
    fn applied(frames: &[Vec<(f64, Option<f64>)>]) -> Vec<u64> {
        let t = |s: f64| sim::sol::sol_epoch().add_seconds(s);
        let mut done: Vec<u64> = Vec::new();
        for frame in frames {
            loop {
                let fleet: Vec<Standing> = (frame.iter().enumerate())
                    .map(|(k, &(time, b))| Standing {
                        id: VesselId(k as u64 + 1),
                        time: t(time),
                        breakup: b.filter(|_| !done.contains(&(k as u64 + 1))).map(t),
                        stopped: false,
                    })
                    .collect();
                let Some(i) = ready_breakup(&fleet) else { break };
                done.push(fleet[i].id.0);
            }
        }
        done
    }

    /// Two breakups, in one frame or in several (vessel 1 behind, compute
    /// limited, breaking up after vessel 2 did at a later epoch): the same
    /// order, the earliest epoch first (review of 2026-10-26, PARTS 6).
    #[test]
    fn breakups_apply_in_the_same_order_in_one_frame_or_several() {
        // Vessel 1 breaks up at 8 s, vessel 2 at 10 s, vessel 3 never; after
        // its split vessel 1 must pass 10 s before vessel 2's split (its
        // pieces could break up before).
        let one = applied(&[
            vec![(8.0, Some(8.0)), (10.0, Some(10.0)), (20.0, None)],
            vec![(9.0, Some(8.0)), (10.0, Some(10.0)), (20.0, None)],
            vec![(20.0, Some(8.0)), (10.0, Some(10.0)), (20.0, None)],
        ]);
        assert_eq!(one, [1, 2]);
        let frames = applied(&[
            vec![(7.0, None), (10.0, Some(10.0)), (12.0, None)],
            vec![(8.0, Some(8.0)), (10.0, Some(10.0)), (14.0, None)],
            vec![(20.0, Some(8.0)), (10.0, Some(10.0)), (20.0, None)],
        ]);
        assert_eq!(frames, one);
        // Equal epochs: the lower id first, whatever the fleet order.
        let s = |id: u64, b: f64| Standing {
            id: VesselId(id),
            time: sim::sol::sol_epoch().add_seconds(b),
            breakup: Some(sim::sol::sol_epoch().add_seconds(b)),
            stopped: false,
        };
        assert_eq!(next_breakup(&[s(5, 3.0), s(2, 3.0), s(9, 1.0)]), Some(2));
        assert_eq!(next_breakup(&[s(5, 3.0), s(2, 3.0)]), Some(1));
        // A vessel stopped for good behind the epoch does not hold it.
        let behind = Standing { breakup: None, ..s(1, 0.0) };
        assert_eq!(ready_breakup(&[behind, s(2, 3.0)]), None);
        assert_eq!(ready_breakup(&[Standing { stopped: true, ..behind }, s(2, 3.0)]), Some(1));
    }

    #[test]
    fn a_breakup_toast_counts_every_piece_and_never_none() {
        let id = |p: u32| sim::craft::CraftId::piece("stress-40", &[p]);
        let wreck = Some(sim::vessel::Destruction::Impact { speed: 9.0, point: 0 });
        let one = SplitPlan { pieces: vec![vec![0, 1]], ids: vec![id(0)], destroyed: vec![None] };
        assert_eq!(breakup_toast(VesselId(3), &one, 0, false), None, "nothing came off");
        assert_eq!(breakup_toast(VesselId(3), &one, 0, true), None, "a new configuration");
        let three = SplitPlan {
            pieces: vec![vec![0], vec![1], vec![2]],
            ids: vec![id(0), id(1), id(2)],
            destroyed: vec![None, None, wreck],
        };
        let toast = |plan: &SplitPlan, new, separation| breakup_toast(VesselId(3), plan, new, separation);
        assert_eq!(toast(&three, 2, false), Some(("Vessel #3 broke up: 3 pieces (1 wrecks)".into(), true)));
        assert_eq!(toast(&three, 2, true), Some(("Vessel #3 broke up: 3 pieces (1 wrecks)".into(), true)), "a wreck");
        let two = SplitPlan { pieces: vec![vec![0], vec![1]], ids: vec![id(0), id(1)], destroyed: vec![None, None] };
        assert_eq!(toast(&two, 1, true), Some(("Vessel #3 separated: a new vessel".into(), false)));
    }
}
