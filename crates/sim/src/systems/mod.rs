//! A vessel's internal systems on a fixed time lattice
//! (`docs/technical/limbo.md` §3, D081, D033).
//!
//! The systems step at the epochs `start + k·STEP`, whatever the frames:
//! each step reads the vessel's state at its own lattice epoch (from the
//! stored trajectory and its pointing while coasting, the tick that holds
//! the epoch while live, the body's rotation while landed) and advances by
//! exactly [`STEP`]. So a year of systems is the same bits advanced in one
//! call or in frames at any warp, and a save taken between lattice points
//! continues exactly (the state is the last lattice point's).
//!
//! What a system may and may not do: a system that changes what the
//! trajectory integrates (mass, thrust) must give the segment a continuous
//! law it can integrate exactly (a craft of parts' tanks on a coast:
//! [`coast_law`]), never step it; anything else (power, life support, the
//! liquids' boil-off, comms later) steps on the lattice. Their state is the
//! ship state's ([`crate::craft::state`]); [`Systems`] is the lattice's
//! clock, and the vessel steps the ship's slow systems at each of its
//! points ([`ship::lattice`], live ticks step them every tick instead).

pub mod coast_law;
pub mod command;
pub mod crew;
pub mod data;
pub mod estimate;
pub mod exhaust;
pub mod failure;
pub mod fins;
pub mod g_limit;
pub mod links;
pub mod maneuver;
pub mod mechanisms;
pub mod plumbing;
pub mod power;
pub mod propulsion;
pub mod rcs;
pub mod ship;
pub mod ship_os;
pub mod structure;

pub use crate::craft::state::{Event, Order, OrderKind};
pub use data::{Knowledge, Quantity, Reading};

use crate::time::Epoch;

/// The systems lattice spacing (s): resolves a low orbit's eclipse
/// (≈ 35 min) to a minute.
pub const STEP: f64 = 60.0;

/// Where a vessel's systems lattice stands.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Systems {
    /// The last lattice point stepped; `None` until the vessel first
    /// advances (the lattice then starts at its time).
    pub epoch: Option<Epoch>,
}

impl Systems {
    /// Systems whose lattice starts at `epoch`.
    pub fn starting(epoch: Epoch) -> Self {
        Systems { epoch: Some(epoch) }
    }

    /// The next lattice point (`None` before the lattice has started).
    pub fn next(&self) -> Option<Epoch> {
        self.epoch.map(|e| e.add_seconds(STEP))
    }

    /// Starts the lattice at `epoch` if it has not started.
    pub fn start_at(&mut self, epoch: Epoch) {
        self.epoch.get_or_insert(epoch);
    }

    /// Moves to the next lattice point (its systems stepped there).
    pub fn step(&mut self) {
        if let Some(next) = self.next() {
            self.epoch = Some(next);
        }
    }

    /// Moves over every lattice point up to `until` (a live tick's: the
    /// ticks stepped the systems). Returns the number of points.
    pub fn advance(&mut self, until: Epoch) -> usize {
        let mut n = 0;
        while self.next().is_some_and(|e| e.seconds_since(until) <= 0.0) {
            self.step();
            n += 1;
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_chunking_of_the_lattice_gives_the_same_bits() {
        let t0 = Epoch::new(1_000_000_000, 0.25);
        let mut one = Systems::starting(t0);
        let mut chunked = one.clone();
        assert_eq!(one.advance(t0.add_seconds(86_400.0)), 1_440);
        let mut t = t0;
        for k in 0..2_000 {
            t = t.add_seconds(if k % 3 == 0 { 7.3 } else { 61.9 });
            if t.seconds_since(t0) > 86_400.0 {
                t = t0.add_seconds(86_400.0);
            }
            chunked.advance(t);
        }
        chunked.advance(t0.add_seconds(86_400.0));
        assert_eq!(one, chunked);
        assert_eq!(one.epoch, Some(t0.add_seconds(86_400.0)));
        assert_eq!(Systems::default().advance(t0), 0, "not started");
    }
}
