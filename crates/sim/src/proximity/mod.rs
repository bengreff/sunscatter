//! Proximity (D082): the 10 km bubble of live physics around a live
//! controllable craft, and craft-to-craft contact.
//!
//! * [`rule`]: which craft fly live because they are near a leader (a
//!   pure rule, evaluated on the bubble's tick lattice).
//! * [`hull`]: a craft's collision hull: its primitives' exact solids and
//!   points sampled on its outer surface.
//! * [`contact`]: the contact forces between two craft for one substep,
//!   equal and opposite.
//!
//! The lockstep stepping of a bubble's members (one tick lattice, a
//! shared local frame, joint contact substeps) is in `sim::vessel::bubble`.
//! Design and numbers: `docs/features/proximity.md`.

pub mod contact;
pub mod hull;
pub mod rule;

pub use contact::{pair_forces, sphere_gap, Body, PairForces, PairImpact};
pub use hull::Hull;
pub use rule::{members, BubbleRule, BubbleState, Candidate, BUBBLE};

use crate::craft::{registry, CraftId};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The hull of craft `id` ([`registry::definition`]), built once per
/// process. Panics if the id cannot be resolved.
pub fn hull_of(id: &CraftId) -> Arc<Hull> {
    static HULLS: OnceLock<Mutex<BTreeMap<String, Arc<Hull>>>> = OnceLock::new();
    let cache = HULLS.get_or_init(Default::default);
    if let Some(h) = cache.lock().expect("hull cache").get(&id.0) {
        return h.clone();
    }
    let def = registry::definition(id).unwrap_or_else(|e| panic!("hull of {:?}: {e}", id.0));
    let hull = Arc::new(Hull::new(&def));
    cache.lock().expect("hull cache").entry(id.0.clone()).or_insert(hull).clone()
}
