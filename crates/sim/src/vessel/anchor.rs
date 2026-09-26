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
    let mut zoned: Vec<_> = world.sources.iter().filter_map(|s| s.anchor_zone.map(|z| (s, z))).collect();
    zoned.sort_by(|a, b| a.1.enter.total_cmp(&b.1.enter));
    for (src, zone) in zoned {
        let d = (r - snap.relative(src.node, current).r).length();
        let limit = if src.node == current { zone.exit } else { zone.enter };
        if d < limit {
            return src.node;
        }
    }
    world.eph.root()
}
