//! The data links (D106, D128): whether a function is in, and which
//! computer reaches which part. Native and layer-neutral: the parts' own
//! logic (layer 1: an engine controller reading its own links, the
//! sensors' data paths) and the ship computer (layer 2, `systems::ship_os`)
//! both read it, so neither calls the other.

use crate::craft::ship::{Owner, ShipDef};
use crate::craft::state::ShipState;

/// Whether the function of `owner` (a component, or a part's own) works:
/// its part intact, its function neither lost (a failure) nor without
/// power, and (a component) switched on.
pub fn works(def: &ShipDef, state: &ShipState, owner: Owner) -> bool {
    let part = def.owner_part(owner).and_then(|p| def.part_index(p));
    let intact = part.is_none_or(|i| state.parts.get(i).is_none_or(|p| p.integrity > 0.0));
    let switched = match owner {
        Owner::Component(n) => def.component_index(n).is_none_or(|i| state.power.get(i).is_none_or(|s| s.on)),
        Owner::Part(_) => true,
    };
    intact && !state.out(owner) && switched
}

/// Whether a working computer is linked to `owner`: `None` when no data
/// link names it.
fn computer_for(def: &ShipDef, state: &ShipState, owner: Owner) -> Option<bool> {
    let mut links = def.links.iter().filter(|l| l.to.iter().any(|(to, _)| *to == owner)).peekable();
    links.peek()?;
    Some(links.any(|l| works(def, state, l.from.0)))
}

/// Whether sensor `owner`'s data reach a working computer (D106): a link
/// from one, or a craft without data links (its data paths are not
/// modelled).
pub fn read_by_computer(def: &ShipDef, state: &ShipState, owner: Owner) -> bool {
    def.links.is_empty() || computer_for(def, state, owner) == Some(true)
}

/// Whether actuator `owner` can be commanded through the data links: a
/// working computer linked to it, or none linked at all (commanded
/// directly).
pub fn commanded(def: &ShipDef, state: &ShipState, owner: Owner) -> bool {
    computer_for(def, state, owner) != Some(false)
}

/// An engine's own controller, as its links give it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Controller {
    /// Its own function in: the engine's part intact, not lost or unpowered.
    pub works: bool,
    /// A computer linked to it works, or none is linked.
    pub commanded: bool,
}

/// Engine `i`'s controller.
pub fn controller(def: &ShipDef, state: &ShipState, i: usize) -> Controller {
    let owner = def.engines[i].owner;
    Controller { works: works(def, state, owner), commanded: commanded(def, state, owner) }
}
