//! Stable vessel identities (realism-1 §2, review game 8).
//!
//! A [`VesselId`] never changes for a vessel's life and is never reused: ids
//! come from one counter ([`VesselIds`]) that is saved with the game, so a
//! deleted vessel's id is not handed out again, even across a save and load.

use std::fmt;

/// A vessel's identity, stable across saves (unlike its index in a fleet).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct VesselId(pub u64);

impl fmt::Display for VesselId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// The id counter: the next id to hand out (ids start at 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VesselIds {
    next: u64,
}

impl Default for VesselIds {
    fn default() -> Self {
        VesselIds { next: 1 }
    }
}

impl VesselIds {
    /// A new id, different from every id handed out before.
    pub fn allocate(&mut self) -> VesselId {
        let id = VesselId(self.next);
        self.next += 1;
        id
    }

    /// Whether `id` could have come from this counter (loaded saves are
    /// checked with it).
    pub fn issued(&self, id: VesselId) -> bool {
        id.0 >= 1 && id.0 < self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_never_reused() {
        let mut ids = VesselIds::default();
        let a = ids.allocate();
        let b = ids.allocate();
        assert_ne!(a, b);
        assert!(ids.issued(a) && ids.issued(b));
        assert!(!ids.issued(VesselId(0)) && !ids.issued(VesselId(3)));
        // A saved counter continues where it left off.
        let mut loaded: VesselIds = ron::from_str(&ron::to_string(&ids).unwrap()).unwrap();
        assert_eq!(loaded.allocate(), VesselId(3));
    }
}
