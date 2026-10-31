//! Where a craft comes from: a [`CraftId`] names a definition, which builds
//! the craft and its design (problem 3 of the scale baseline: a vessel of a
//! craft not under `data/craft` could not be reloaded from a save).
//!
//! An id is resolved, in order, as
//! * a definition registered at run time ([`register`]: a craft made in the
//!   game, later);
//! * `stress-<n>`: the stress craft of n catalog parts ([`super::stress`]);
//! * `small-<name>`: a small craft of parts built in code ([`super::small`]);
//! * a directory under `data/craft` ([`super::default_craft_dir`]), one
//!   part for the whole ship;
//!
//! and a **piece** `base#parts` ([`CraftId::piece`]) is the subset of its
//! base craft's parts listed (their ids in the base, as ranges `a-b,c`):
//! what is left of a craft, or what broke off it, after a split
//! (`sim::vessel::split`). Building is deterministic, so a craft rebuilt
//! after a load is the same bits as the one the vessel flew with. Built
//! craft are kept per id for the process ([`craft_of`]; the design inside
//! is built on first use).

use super::cells::CellOptions;
use super::parts::CraftDef;
use super::{Craft, CraftId, Resolution};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

impl CraftId {
    /// The craft this one is a piece of (itself for a whole craft).
    pub fn base(&self) -> &str {
        self.0.split(['#', '@']).next().unwrap_or("")
    }

    /// The parts of the base craft this piece holds (their ids, ascending);
    /// `None` for a whole craft.
    pub fn parts(&self) -> Option<Vec<u32>> {
        let plain = self.0.split('@').next().unwrap_or("");
        let (_, list) = plain.split_once('#')?;
        super::configure::ranges(list)
    }

    /// The piece of craft `base` holding the parts with ids `parts`.
    pub fn piece(base: &str, parts: &[u32]) -> CraftId {
        let mut ids = parts.to_vec();
        ids.sort_unstable();
        ids.dedup();
        let mut ranges: Vec<String> = Vec::new();
        let mut i = 0;
        while i < ids.len() {
            let mut j = i;
            while j + 1 < ids.len() && ids[j + 1] == ids[j] + 1 {
                j += 1;
            }
            ranges.push(if i == j { ids[i].to_string() } else { format!("{}-{}", ids[i], ids[j]) });
            i = j + 1;
        }
        CraftId(format!("{base}#{}", ranges.join(",")))
    }
}

fn registered() -> &'static Mutex<BTreeMap<String, CraftDef>> {
    static DEFS: OnceLock<Mutex<BTreeMap<String, CraftDef>>> = OnceLock::new();
    DEFS.get_or_init(Default::default)
}

/// Registers a definition under `id` (a base id, without `#`), for craft
/// that are neither files nor stress craft. Registering the same id twice
/// keeps the first: a craft id names one definition for the process.
pub fn register(id: &CraftId, def: CraftDef) {
    registered().lock().expect("craft registry").entry(id.0.clone()).or_insert(def);
}

/// The definition `id` names (see the module), or why there is none.
pub fn definition(id: &CraftId) -> Result<CraftDef, String> {
    let base = id.base();
    let whole = if let Some(def) = registered().lock().expect("craft registry").get(base) {
        def.clone()
    } else if let Some(def) = super::stress::definition(base) {
        def?
    } else if let Some(def) = super::small::definition(base) {
        def?
    } else if base == super::TEST_CRAFT {
        super::test_craft().def()
    } else {
        super::load_definition(&super::default_craft_dir().join(base)).map_err(|e| e.to_string())?
    };
    let piece = match id.parts() {
        None => whole,
        Some(ids) => {
            let local: Option<Vec<u32>> =
                ids.iter().map(|&p| whole.parts.iter().position(|q| q.id == p).map(|i| i as u32)).collect();
            match local {
                Some(local) if !local.is_empty() => whole.subset(&local),
                _ => return Err(format!("craft {:?}: parts not in {base:?}", id.0)),
            }
        }
    };
    let deployed = id.deployed();
    if deployed.is_empty() {
        Ok(piece)
    } else {
        piece.deploy(&deployed).map_err(|e| format!("craft {:?}: {e}", id.0))
    }
}

/// How the cells of craft `id` (definition `def`) are built: a craft file
/// the default (as [`super::parse_craft`]), anything made of parts (a
/// stress craft, a piece, a registered craft) by area and part count
/// ([`CellOptions::for_parts`]).
pub fn cell_options(id: &CraftId, def: &CraftDef) -> CellOptions {
    let file = id.parts().is_none()
        && def.ship.is_none()
        && !id.0.starts_with("stress-")
        && !registered().lock().expect("craft registry").contains_key(&id.0);
    if file {
        CellOptions::default()
    } else {
        CellOptions::for_parts(def.parts.len())
    }
}

/// The craft `id` names, built once per process (the test craft is
/// [`super::test_craft`]'s). Panics if it cannot be resolved.
pub fn craft_of(id: &CraftId) -> Arc<Craft> {
    static CRAFT: OnceLock<Mutex<BTreeMap<String, Arc<Craft>>>> = OnceLock::new();
    let cache = CRAFT.get_or_init(Default::default);
    if let Some(c) = cache.lock().expect("craft cache").get(&id.0) {
        return c.clone();
    }
    // Built outside the lock: other ids may build meanwhile.
    let built = if id.0 == super::TEST_CRAFT {
        let c = super::test_craft();
        c.design();
        c.clone()
    } else {
        let def = definition(id).unwrap_or_else(|e| panic!("{e}"));
        let opts = cell_options(id, &def);
        Craft::from_def(id.clone(), def, &Resolution::default(), &opts)
    };
    cache.lock().expect("craft cache").entry(id.0.clone()).or_insert_with(|| Arc::new(built)).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piece_ids_round_trip() {
        let cases: [(&[u32], &str); 4] = [
            (&[3], "stress-300#3"),
            (&[0, 1, 2, 3, 7, 9, 10], "stress-300#0-3,7,9-10"),
            (&[10, 9, 9, 0], "stress-300#0,9-10"),
            (&[5, 6], "stress-300#5-6"),
        ];
        for (parts, text) in cases {
            let id = CraftId::piece("stress-300", parts);
            assert_eq!(id.0, text);
            let mut want = parts.to_vec();
            want.sort_unstable();
            want.dedup();
            assert_eq!(id.parts(), Some(want));
            assert_eq!(id.base(), "stress-300");
        }
        assert_eq!(CraftId("small-rocket".into()).parts(), None);
        assert_eq!(CraftId("stress-3#x".into()).parts(), None);
    }

    /// Every kind of id resolves; a piece of a piece names its parts in the
    /// base craft, and resolves to the same definition as the piece taken
    /// directly.
    #[test]
    fn ids_resolve_to_definitions() {
        let small = definition(&super::super::small::id("rocket")).unwrap();
        assert_eq!(small.parts.len(), super::super::small::rocket_ref().parts.len());
        let whole = definition(&CraftId("stress-12".into())).unwrap();
        assert_eq!(whole.parts.len(), 12);
        assert_eq!(Some(Ok(whole.clone())), super::super::stress::definition("stress-12"));
        let piece = definition(&CraftId::piece("stress-12", &[1, 4, 5, 6])).unwrap();
        assert_eq!(piece.parts.iter().map(|p| p.id).collect::<Vec<_>>(), vec![1, 4, 5, 6]);
        assert_eq!(piece, whole.subset(&[1, 4, 5, 6]));
        let again = piece.subset(&[1, 2]);
        assert_eq!(again, definition(&CraftId::piece("stress-12", &[4, 5])).unwrap());
        assert!(definition(&CraftId("no-such-craft".into())).is_err());
        assert!(definition(&CraftId::piece("stress-12", &[40])).is_err());
    }

    /// A stress craft and a piece of one rebuilt from their ids are the
    /// craft they were built as: the same design bits (what a saved vessel
    /// reloads).
    #[test]
    fn a_stress_craft_and_its_pieces_rebuild_from_their_ids() {
        let id = super::super::stress::id(12);
        let def = definition(&id).unwrap();
        let built = Craft::from_def(id.clone(), def.clone(), &Resolution::default(), &cell_options(&id, &def));
        let again = craft_of(&built.id);
        assert_eq!(again.as_ref(), &built);
        assert_eq!(**again.design(), **built.design());
        assert!(Arc::ptr_eq(&again, &craft_of(&built.id)), "built once");
        let id = CraftId::piece("stress-12", &[4, 5, 6]);
        let def = built.def().subset(&[4, 5, 6]);
        let piece = Craft::from_def(id.clone(), def.clone(), &Resolution::default(), &cell_options(&id, &def));
        assert_eq!(**craft_of(&id).design(), **piece.design());
        assert_eq!(*super::super::design_of(&id), **piece.design());
    }
}
