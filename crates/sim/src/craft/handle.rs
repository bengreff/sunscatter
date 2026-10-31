//! Typed handles (D121): ids are strings only at the edges (files, the
//! interface, MCP, the player's events); inside, a function's owner is an
//! [`Owner`] and a part is its `Part::id`, found in a definition's lists by
//! a binary search, since the lists are in design order and a split's
//! subset keeps it. The one resolution from a string is
//! [`ShipDef::owner_named`].

use super::ship::{ComponentDef, ShipDef};
use crate::catalog::{FailureMode, Ratings};
use serde::{Deserialize, Serialize};

/// What owns a function (an engine, a valve, a computer, a mechanism, …):
/// a part (its own functions) or a component, each by its number in the
/// design (`Part::id`, `ComponentDef::n`), which every piece of a split
/// keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Owner {
    Part(u32),
    Component(u32),
}

impl ShipDef {
    /// Where part `part` (`Part::id`) sits in the per-part lists (this
    /// definition's `parts`, and its ship state's).
    pub fn part_index(&self, part: u32) -> Option<usize> {
        self.parts.binary_search_by_key(&part, |p| p.part).ok()
    }

    /// Where component `n` (`ComponentDef::n`) sits in `components` (and
    /// the state's power switches).
    pub fn component_index(&self, n: u32) -> Option<usize> {
        self.components.binary_search_by_key(&n, |c| c.n).ok()
    }

    /// The owner's id, for the edges ("" when this definition lacks it).
    pub fn owner_id(&self, o: Owner) -> &str {
        match o {
            Owner::Part(p) => self.part_index(p).map_or("", |i| self.parts[i].id.as_str()),
            Owner::Component(n) => self.component_index(n).map_or("", |i| self.components[i].id.as_str()),
        }
    }

    /// The owner whose id is `id` (the one resolution from a string: files,
    /// the interface, MCP, tests).
    pub fn owner_named(&self, id: &str) -> Option<Owner> {
        (self.parts.iter().find(|p| p.id == id).map(|p| Owner::Part(p.part)))
            .or_else(|| self.components.iter().find(|c| c.id == id).map(|c| Owner::Component(c.n)))
    }

    /// The part an owner sits on (`None` when this definition lacks it).
    pub fn owner_part(&self, o: Owner) -> Option<u32> {
        match o {
            Owner::Part(p) => self.part_index(p).map(|_| p),
            Owner::Component(n) => self.component_index(n).map(|i| self.components[i].part),
        }
    }

    /// The component an owner is (`None` for a part).
    pub fn component_of(&self, o: Owner) -> Option<&ComponentDef> {
        match o {
            Owner::Component(n) => self.component_index(n).map(|i| &self.components[i]),
            Owner::Part(_) => None,
        }
    }

    /// An owner's ratings.
    pub fn ratings_of(&self, o: Owner) -> Option<&Ratings> {
        match o {
            Owner::Part(p) => self.part_index(p).map(|i| &self.parts[i].ratings),
            Owner::Component(n) => self.component_index(n).map(|i| &self.components[i].ratings),
        }
    }

    /// An owner's statistical failure modes.
    pub fn failures_of(&self, o: Owner) -> &[FailureMode] {
        match o {
            Owner::Part(p) => self.part_index(p).map_or(&[], |i| self.parts[i].failures.as_slice()),
            Owner::Component(n) => self.component_index(n).map_or(&[], |i| self.components[i].failures.as_slice()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn falcon9() -> std::sync::Arc<ShipDef> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
        super::super::load_definition(&path).unwrap().ship.unwrap()
    }

    /// Every part and component resolves to its handle and back, in the
    /// whole craft and in a split's piece (which keeps the numbers), and
    /// the binary searches find what a linear search finds.
    #[test]
    fn handles_resolve_both_ways_in_the_whole_and_in_a_piece() {
        let def = falcon9();
        let dragon: Vec<u32> = (def.parts.iter()).filter(|p| p.id.starts_with("dragon.")).map(|p| p.part).collect();
        let piece = def.subset(&dragon);
        assert!(!piece.components.is_empty() && piece.parts.len() < def.parts.len());
        for d in [&*def, &piece] {
            for (i, p) in d.parts.iter().enumerate() {
                assert_eq!(d.part_index(p.part), Some(i));
                assert_eq!(d.owner_named(&p.id), Some(Owner::Part(p.part)));
                assert_eq!(d.owner_id(Owner::Part(p.part)), p.id);
                assert_eq!(d.owner_part(Owner::Part(p.part)), Some(p.part));
            }
            for (i, c) in d.components.iter().enumerate() {
                assert_eq!(d.component_index(c.n), Some(i));
                assert_eq!(d.owner_named(&c.id), Some(Owner::Component(c.n)));
                assert_eq!(d.owner_id(Owner::Component(c.n)), c.id);
                assert_eq!(d.owner_part(Owner::Component(c.n)), Some(c.part));
            }
        }
        // The piece's numbers are the whole's.
        for c in &piece.components {
            assert_eq!(def.owner_id(Owner::Component(c.n)), c.id);
        }
        // What the piece lacks resolves to nothing.
        let booster = def.parts.iter().find(|p| p.id.starts_with("s1.")).unwrap();
        assert_eq!((piece.part_index(booster.part), piece.owner_id(Owner::Part(booster.part))), (None, ""));
        assert_eq!(piece.owner_named(&booster.id), None);
        // Every item's owner is its definition's.
        for e in &def.engines {
            assert_eq!(def.owner_part(e.owner), Some(e.part), "{}", e.id);
        }
        for m in &def.mechanisms {
            assert_eq!(def.owner_id(m.owner), m.id);
        }
    }
}
