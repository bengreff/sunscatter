//! Anchor policy: which node a vessel's state is stored relative to.
//!
//! This is a *precision* choice only (motion-model §2). The physics subtracts
//! the anchor's exact kinematic acceleration, so any policy gives the same
//! trajectory to within round-off; this one keeps offsets small near bodies.

use crate::ephem::Snapshot;
use crate::frame::NodeId;
use crate::world::World;
use glam::DVec3;

/// The preferred anchor for a vessel at `r` (relative to `current`): the
/// smallest anchor zone containing it, with hysteresis; otherwise the root.
pub fn preferred_anchor(world: &World, snap: &Snapshot, current: NodeId, r: DVec3) -> NodeId {
    for &i in &world.anchor_order {
        let src = &world.sources[i];
        let Some(zone) = src.anchor_zone else { continue };
        let d = (r - snap.relative_r(src.node, current)).length();
        let limit = if src.node == current { zone.exit } else { zone.enter };
        if d < limit {
            return src.node;
        }
    }
    world.eph.root()
}
