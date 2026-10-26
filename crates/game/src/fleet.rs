//! Advancing the fleet each frame (`state::advance`): every vessel towards
//! the frame's target in chunks of work, on the compute pool's threads,
//! the vessel furthest behind first, until each gets there or the frame's
//! time for the simulation is used up.
//!
//! Vessels are independent (D025) and each is advanced by one thread at a
//! time, in calls whose chunking never changes the result (rule 4: a
//! budgeted advance equals one call), so neither the threads nor the
//! deadline change any vessel's motion: only how far the vessels get this
//! frame, and so the clock (held at the slowest; the warp then shows as
//! limited by compute, with the rate actually achieved). The results are
//! collected per vessel index, never reduced across threads in an order
//! that depends on them (rule 2).

use bevy::app::{TaskPoolOptions, TaskPoolThreadAssignmentPolicy};
use bevy::tasks::ComputeTaskPool;
use sim::time::Epoch;
use sim::vessel::{Controls, Phase, Vessel, TICK};
use sim::world::World;
use std::sync::Mutex;
use std::time::Instant;

/// Work units (about a microsecond each on the reference Mac, D028; see
/// `sim::vessel::STEP_UNITS`) per advance call: one chunk of a vessel's
/// frame. A vessel that needs more gets more chunks while the frame lasts.
pub const CHUNK_UNITS: usize = 2_000;
/// Most chunks one vessel gets in a frame (a guard: a vessel that keeps
/// spending without moving is left for the next frame).
pub const MAX_CHUNKS: usize = 256;
/// Wall time (s) the fleet's advance may take in a frame before the clock
/// is held back; the rest of the 16.7 ms frame (D028) draws.
pub const SIM_FRAME_TIME: f64 = 0.008;

/// The app's thread pools: Bevy's default gives a 10-core machine (the
/// reference M2 Pro) 3 IO, 3 async and 4 compute threads; the fleet runs
/// on the compute pool, so IO keeps one thread (asset loading at startup),
/// async compute (terrain chunks, the powered prediction) two, and compute
/// the rest (7 there).
pub fn task_pools() -> TaskPoolOptions {
    let d = TaskPoolOptions::default();
    TaskPoolOptions {
        io: TaskPoolThreadAssignmentPolicy { min_threads: 1, max_threads: 1, ..d.io },
        async_compute: TaskPoolThreadAssignmentPolicy { min_threads: 1, max_threads: 2, ..d.async_compute },
        ..d
    }
}

/// Where a vessel stands in a frame's scheduling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Short of the target, waiting for a chunk.
    Ready,
    /// A thread is advancing it.
    Busy,
    /// At the target, or nothing more to do this frame.
    Done,
}

/// The scheduling state of a frame (a pure rule: which vessel goes next).
#[derive(Clone, Debug)]
pub struct Queue {
    pub time: Vec<Epoch>,
    pub slot: Vec<Slot>,
    pub chunks: Vec<usize>,
}

impl Queue {
    /// The vessels at `times` for a frame to `target`: those already at or
    /// past it (finished first in an earlier frame whose target was further
    /// on) are done, never advanced back to it.
    pub fn new(times: impl Iterator<Item = Epoch>, target: Epoch) -> Self {
        let time: Vec<Epoch> = times.collect();
        let slot = time.iter().map(|&t| if t >= target { Slot::Done } else { Slot::Ready }).collect();
        Queue { slot, chunks: vec![0; time.len()], time }
    }

    /// The next vessel to advance: the ready one furthest behind (the clock
    /// waits for it), the lowest index on ties. Marks it busy.
    pub fn pick(&mut self) -> Option<usize> {
        let i = (0..self.time.len())
            .filter(|&i| self.slot[i] == Slot::Ready)
            .min_by(|&a, &b| self.time[a].seconds_since(self.time[b]).total_cmp(&0.0).then(a.cmp(&b)))?;
        self.slot[i] = Slot::Busy;
        Some(i)
    }

    /// Vessel `i` reached `reached` from `before` with `spent` units of a
    /// chunk; `more` says whether it can still move towards the target.
    pub fn finish(&mut self, i: usize, before: Epoch, reached: Epoch, spent: usize, more: bool) {
        self.time[i] = reached;
        self.chunks[i] += 1;
        let moved = reached > before || spent > 0;
        self.slot[i] = if more && moved && self.chunks[i] < MAX_CHUNKS { Slot::Ready } else { Slot::Done };
    }
}

/// What a frame's advance did.
#[derive(Clone, Debug, Default)]
pub struct FleetAdvance {
    /// Per vessel: the time reached, and whether its motion has ended for
    /// good (a finished trajectory: it must not hold the clock).
    pub reached: Vec<Epoch>,
    pub stopped: Vec<bool>,
}

/// Whether a vessel's motion has ended for good at a time before the clock
/// (its trajectory failed or reached the ephemeris end): it stays there and
/// must not hold the clock.
pub fn stopped(vessel: &Vessel) -> bool {
    matches!(&vessel.phase, Phase::Coasting { trajectory } if trajectory.finished())
}

/// Whether a coasting vessel at `reached` can still move towards `target`
/// in this frame (a live vessel trails by less than a tick, or stopped at a
/// hand-over to live flight: the clock waits for it; landed vessels reach
/// the target in one call).
fn can_continue(vessel: &Vessel, reached: Epoch, target: Epoch) -> bool {
    target.seconds_since(reached) > TICK && matches!(vessel.phase, Phase::Coasting { .. }) && !stopped(vessel)
}

/// Advances every vessel of `fleet` towards `target` with `controls(i)`,
/// in chunks of [`CHUNK_UNITS`] on the compute pool's threads, the vessel
/// furthest behind first, until each reaches the target (or can go no
/// further this frame), or until `deadline` (a chunk under way finishes).
pub fn advance_fleet(
    world: &World,
    fleet: &mut [Vessel],
    controls: &(dyn Fn(usize) -> Controls + Sync),
    target: Epoch,
    deadline: Option<Instant>,
) -> FleetAdvance {
    let n = fleet.len();
    let queue = Mutex::new(Queue::new(fleet.iter().map(|v| v.time), target));
    let slots: Vec<Mutex<&mut Vessel>> = fleet.iter_mut().map(Mutex::new).collect();
    let work = || loop {
        let next = {
            let mut q = queue.lock().expect("queue");
            if deadline.is_some_and(|d| Instant::now() >= d) {
                None
            } else {
                q.pick()
            }
        };
        let Some(i) = next else { return };
        let mut v = slots[i].lock().expect("vessel");
        let before = v.time;
        let (reached, spent) = v.advance_frame(world, target, &controls(i), CHUNK_UNITS);
        let more = can_continue(&v, reached, target);
        drop(v);
        queue.lock().expect("queue").finish(i, before, reached, spent, more);
    };
    let pool = ComputeTaskPool::get();
    let threads = pool.thread_num().max(1).min(n.max(1));
    if threads <= 1 {
        work();
    } else {
        pool.scope(|s| {
            for _ in 0..threads {
                s.spawn(async { work() });
            }
        });
    }
    drop(slots);
    let reached = queue.into_inner().expect("queue").time;
    FleetAdvance { reached, stopped: fleet.iter().map(stopped).collect() }
}

/// The clock after a frame's advance: the target, held back to the
/// slowest vessel that has not stopped for good and trails it by more than
/// a tick (a live vessel trails by less).
pub fn clock_after(adv: &FleetAdvance, target: Epoch) -> Epoch {
    let mut clock = target;
    for (&r, &stopped) in adv.reached.iter().zip(&adv.stopped) {
        if holds_clock(r, target, stopped) && r < clock {
            clock = r;
        }
    }
    clock
}

/// Whether a vessel that reached `reached` holds the clock back from
/// `target`: powered vessels trail by less than a tick; a coast that could
/// not be integrated far enough holds it; a stopped vessel never does.
pub fn holds_clock(reached: Epoch, target: Epoch, stopped: bool) -> bool {
    !stopped && reached.seconds_since(target) < -TICK
}

/// How much more than the clock moved last frame a frame may ask for: a
/// little while compute limits it, twice otherwise (a warp change ramps
/// up over a few frames: 1x to 1,000,000x in 20, a third of a second).
pub const LIMITED_GROWTH: f64 = 1.25;
pub const GROWTH: f64 = 2.0;

/// The simulated span (s) a frame asks for: `wanted` (dt × warp), at most
/// [`GROWTH`] × what the clock moved last frame (`last`; [`LIMITED_GROWTH`]
/// while compute limits the clock). The vessels that finish first lead the
/// drawn clock (they are drawn at their own time) by a fraction of a
/// frame's progress, never by a whole 1,000,000x span (4.6 h). A frame that
/// did not move (paused, a vessel gone live at the clock) asks for `wanted`.
pub fn frame_span(wanted: f64, limited: bool, last: f64) -> f64 {
    if last > 0.0 {
        wanted.min(if limited { LIMITED_GROWTH } else { GROWTH } * last)
    } else {
        wanted
    }
}

/// The warp achieved, smoothed: `last` moved towards the frame's rate
/// (simulated over real seconds) with a time constant of about half a
/// second of frames.
pub fn achieved_warp(last: f64, simulated: f64, real: f64) -> f64 {
    if real <= 0.0 {
        return last;
    }
    let rate = simulated / real;
    let k = (real / 0.5).min(1.0);
    last + k * (rate - last)
}

#[cfg(test)]
#[path = "fleet_tests.rs"]
mod tests;
