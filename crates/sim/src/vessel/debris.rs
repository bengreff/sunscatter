//! The debris rule (D083): a piece that broke off is a full vessel until it
//! is dead, uncontrollable and 10 km or more from every controllable craft;
//! then it may be deleted. One pure rule ([`deletable`]); what it reads of a
//! vessel ([`Vessel::controllable`], [`Vessel::dead`]) comes from its parts'
//! placeholders (`sim::craft::parts`) and its power system.
//!
//! * **Controllable**: intact, with a part that can command it (a crew
//!   cabin or probe core) and power (a part that produces or stores it, and
//!   the battery not flat or a panel to recharge it).
//! * **Dead**: destroyed, or no active systems (no power).
//!
//! Deleting a vessel changes nothing for the others (vessels do not
//! attract, D025), so when a frame deletes it does not matter.

use super::Vessel;
use crate::time::Epoch;
use crate::world::World;

/// Debris this far or farther from every controllable craft may go (m).
pub const DEBRIS_DISTANCE: f64 = 10_000.0;

/// D083: delete a vessel iff it is not controllable, it is dead, and the
/// nearest controllable craft (`nearest`, m; `None`: there is none) is at
/// least [`DEBRIS_DISTANCE`] away.
pub fn deletable(controllable: bool, dead: bool, nearest: Option<f64>) -> bool {
    !controllable && dead && nearest.is_none_or(|d| d >= DEBRIS_DISTANCE)
}

impl Vessel {
    /// Whether the vessel has power: a part that produces or stores it, and
    /// a panel or charge left in its power system.
    pub fn powered(&self) -> bool {
        let p = &self.systems.power;
        let source = self.craft.design().parts.iter().any(|q| q.power.source());
        source && (p.spec.panel_area > 0.0 || p.charge > 0.0)
    }

    /// Whether the vessel can be commanded (D083): intact, with a control
    /// part (crew cabin or probe core) and power.
    pub fn controllable(&self) -> bool {
        self.destruction().is_none() && self.craft.design().parts.iter().any(|q| q.control) && self.powered()
    }

    /// Whether the vessel is dead (D083): destroyed, or without power (no
    /// active systems).
    pub fn dead(&self) -> bool {
        self.destruction().is_some() || !self.powered()
    }
}

/// The distance between two vessels at `t` (m), through the frame tree
/// (rule 3: each relative to its own anchor, the anchors' offset added).
pub fn separation(world: &World, t: Epoch, a: &Vessel, b: &Vessel) -> f64 {
    let (ka, ra, _) = a.state_at(world, t);
    let (kb, rb, _) = b.state_at(world, t);
    let offset = if ka == kb { glam::DVec3::ZERO } else { world.snapshot(t).relative_r(ka, kb) };
    (ra + offset - rb).length()
}

/// The fleet's vessels the rule deletes at `t` (indices, ascending),
/// never `keep` (the active vessel).
pub fn debris(world: &World, t: Epoch, fleet: &[Vessel], keep: usize) -> Vec<usize> {
    let control: Vec<usize> = (0..fleet.len()).filter(|&i| fleet[i].controllable()).collect();
    (0..fleet.len())
        .filter(|&i| i != keep && !control.contains(&i) && fleet[i].dead())
        .filter(|&i| {
            let nearest = control.iter().map(|&c| separation(world, t, &fleet[i], &fleet[c])).reduce(f64::min);
            deletable(false, true, nearest)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule as D083 states it: (controllable, dead, nearest controllable
    /// craft) → deleted.
    #[test]
    fn the_debris_rule_table() {
        let cases = [
            (false, true, Some(10_000.0), true),
            (false, true, Some(50_000.0), true),
            (false, true, None, true),
            (false, true, Some(9_999.9), false),
            (false, true, Some(0.0), false),
            (false, false, Some(1e9), false),
            (false, false, None, false),
            (true, true, Some(1e9), false),
            (true, false, None, false),
            (true, true, None, false),
        ];
        for (controllable, dead, nearest, want) in cases {
            assert_eq!(deletable(controllable, dead, nearest), want, "{controllable} {dead} {nearest:?}");
        }
    }
}
