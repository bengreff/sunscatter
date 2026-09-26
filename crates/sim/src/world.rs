//! The simulated world: the ephemeris plus, for each gravity source, the
//! physical data the ship dynamics need (J2, surface, atmosphere, anchoring).

use crate::body::{self, BodyPhysical};
use crate::ephem::{Ephemeris, NodeKind, Snapshot};
use crate::frame::NodeId;
use crate::time::Epoch;
use std::sync::Arc;

/// Distances (m) at which a body becomes / stops being the preferred anchor.
/// Pure precision policy: any value is *correct* (invariance tests), these
/// just keep offsets small near each body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorZone {
    pub enter: f64,
    pub exit: f64,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub node: NodeId,
    pub name: String,
    pub gm: f64,
    /// Physical body data (surface, J2, rotation, atmosphere), if modelled.
    pub physical: Option<BodyPhysical>,
    /// The source's 1PN field acts on vessels (the Sun).
    pub relativistic: bool,
    pub anchor_zone: Option<AnchorZone>,
}

#[derive(Clone, Debug)]
pub struct World {
    pub eph: Arc<Ephemeris>,
    pub sources: Vec<Source>,
    /// Gravity sources whose acceleration at the vessel is below this (m/s²)
    /// at the start of a segment are not simulated for that segment (D023).
    pub cutoff: f64,
}

impl World {
    /// The Solar System world for the Earth–Moon prototype.
    pub fn sol(eph: Arc<Ephemeris>) -> Self {
        let sources = eph
            .bodies()
            .map(|id| {
                let n = eph.node(id);
                let (physical, anchor_zone) = match n.name.as_str() {
                    "Earth" => (Some(body::earth()), Some(AnchorZone { enter: 1.4e9, exit: 1.6e9 })),
                    "Moon" => (Some(body::moon()), Some(AnchorZone { enter: 6.0e7, exit: 7.0e7 })),
                    "Sun" => (Some(body::sun()), None),
                    _ => (None, None),
                };
                Source {
                    node: id,
                    name: n.name.clone(),
                    gm: n.gm,
                    physical,
                    relativistic: n.name == "Sun",
                    anchor_zone,
                }
            })
            .collect();
        debug_assert!(eph.nodes().iter().any(|n| n.kind == NodeKind::Barycenter));
        World { eph, sources, cutoff: 1e-11 }
    }

    pub fn source(&self, node: NodeId) -> Option<&Source> {
        self.sources.iter().find(|s| s.node == node)
    }

    pub fn find(&self, name: &str) -> Option<&Source> {
        self.sources.iter().find(|s| s.name == name)
    }

    pub fn snapshot(&self, t: Epoch) -> Snapshot<'_> {
        self.eph.snapshot(t)
    }

    /// Sources with a solid surface (candidates for landing and impact).
    pub fn surfaces(&self) -> impl Iterator<Item = &Source> {
        self.sources.iter().filter(|s| s.physical.is_some() && s.name != "Sun")
    }
}
