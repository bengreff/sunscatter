//! A craft's configuration (D103: rigid until it breaks, mechanisms move):
//! the shape with some mechanisms deployed — landing legs out with their
//! feet as contact points, grid fins in the flow, a nosecone open. Each
//! configuration is a craft of its own (its cells, contact points, mass
//! properties and design), named by its id: `base[#parts]@deployed` (the
//! part ids whose mechanisms are deployed, as ranges), built once per
//! process like any piece (`super::registry`).
//!
//! A deployed part turns about its mechanism's hinge by the deployed angle
//! (`super::place::hinge`); a leg's foot becomes a contact point (its pad's
//! base, vertical once deployed). The ship definition's mechanisms and aero
//! surfaces of those parts move with them.

use super::parts::CraftDef;
use super::place;
use super::CraftId;
use std::sync::Arc;

impl CraftId {
    /// The part ids whose mechanisms this configuration has deployed.
    pub fn deployed(&self) -> Vec<u32> {
        self.0.split_once('@').map_or_else(Vec::new, |(_, list)| ranges(list).unwrap_or_default())
    }

    /// This craft (or piece) with the parts `deployed` (part ids) deployed;
    /// none: as built.
    pub fn with_deployed(&self, deployed: &[u32]) -> CraftId {
        let plain = self.0.split('@').next().unwrap_or("");
        if deployed.is_empty() {
            return CraftId(plain.to_string());
        }
        let list = CraftId::piece("", deployed).0;
        CraftId(format!("{plain}@{}", list.trim_start_matches('#')))
    }
}

/// Parses `a-b,c` into ids.
pub(super) fn ranges(list: &str) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for range in list.split(',').filter(|r| !r.is_empty()) {
        let (a, b) = range.split_once('-').unwrap_or((range, range));
        let (a, b): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
        out.extend(a..=b);
    }
    Some(out)
}

impl CraftDef {
    /// This definition with the parts `deployed` (part ids) turned about
    /// their mechanisms' hinges to their deployed angles: their primitives,
    /// a leg's foot as a contact point, their mechanisms and aero surfaces
    /// in the ship definition. A part without a mechanism, or not in the
    /// craft, is an error.
    pub fn deploy(&self, deployed: &[u32]) -> Result<CraftDef, String> {
        let Some(ship) = &self.ship else {
            return Err("only a craft of catalog parts has mechanisms".into());
        };
        let mut def = self.clone();
        let mut ship = (**ship).clone();
        for &id in deployed {
            let Some(index) = def.parts.iter().position(|p| p.id == id) else {
                return Err(format!("part {id} is not in the craft"));
            };
            let Some(m) = ship.mechanisms.iter_mut().find(|m| m.part == id) else {
                return Err(format!("part {:?} has no mechanism", def.parts[index].name));
            };
            let turn = place::hinge(m.item.pos, m.item.axis, m.item.deployed_deg - m.item.stowed_deg);
            let foot = m.item.foot.is_some();
            m.item.foot = m.item.foot.map(|f| turn.point(f));
            for &q in &def.parts[index].primitives {
                let p = &mut def.geometry.primitives[q as usize];
                p.shape = turn.shape(&p.shape).map_err(|e| format!("part {:?}: {e}", def.parts[index].name))?;
                p.foot = foot && p.name == "foot";
            }
            for s in ship.surfaces.iter_mut().filter(|s| s.part == id) {
                s.item.pos = turn.point(s.item.pos);
                s.item.axis = turn.dir(s.item.axis);
            }
        }
        def.ship = Some(Arc::new(ship));
        Ok(def)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::craft::build_tests::{built, CRAFT};
    use crate::craft::{CellOptions, ContactKind, Craft, Resolution};
    use glam::DVec3;

    #[test]
    fn ids_carry_the_deployed_parts() {
        let id = CraftId("falcon9-crew".into());
        let legs = id.with_deployed(&[27, 28, 29, 30]);
        assert_eq!(legs.0, "falcon9-crew@27-30");
        assert_eq!((legs.base(), legs.parts(), legs.deployed()), ("falcon9-crew", None, vec![27, 28, 29, 30]));
        let piece = CraftId::piece("falcon9-crew", &[8, 9, 10]).with_deployed(&[9]);
        assert_eq!(piece.0, "falcon9-crew#8-10@9");
        assert_eq!((piece.base(), piece.parts(), piece.deployed()), ("falcon9-crew", Some(vec![8, 9, 10]), vec![9]));
        assert_eq!(piece.with_deployed(&[]).0, "falcon9-crew#8-10");
    }

    /// The test rocket's legs deployed (120° about their hinges): each
    /// foot's pad is down and out, a contact point of kind foot; the legs'
    /// mass moves out, the rest stays.
    #[test]
    fn deployed_legs_stand_on_their_feet() {
        let def = built(CRAFT).unwrap().def;
        let legs: Vec<u32> = def.parts.iter().filter(|p| p.name.starts_with("leg#")).map(|p| p.id).collect();
        let out = def.deploy(&legs).unwrap();
        let opts = CellOptions::for_parts(11);
        let (stowed, open) = (
            Craft::from_def(CraftId("a".into()), def.clone(), &Resolution::default(), &opts),
            Craft::from_def(CraftId("b".into()), out.clone(), &Resolution::default(), &opts),
        );
        let feet: Vec<DVec3> =
            open.cells.contacts.iter().filter(|c| c.kind == ContactKind::Foot).map(|c| c.pos).collect();
        assert_eq!(feet.len(), 4);
        assert!(stowed.cells.contacts.iter().all(|c| c.kind != ContactKind::Foot));
        // Leg 1 on the +X mount at z −4.75: the foot 2 m along the leg turned
        // 120° from +Z, 0.075 m out from the hinge.
        let hinge = DVec3::new(1.0, 0.0, -4.75);
        let foot = place::hinge(hinge, DVec3::Y, 120.0).point(hinge + DVec3::new(0.075, 0.0, 2.0));
        assert!(feet.iter().any(|f| (*f - foot).length() < 1e-9), "{feet:?} vs {foot}");
        assert!(out
            .ship
            .as_ref()
            .unwrap()
            .mechanisms
            .iter()
            .any(|m| m.item.foot.is_some_and(|f| (f - foot).length() < 1e-9)));
        // Same parts and masses; the legs' centre of mass moves out and down.
        assert_eq!(stowed.mass.dry_mass, open.mass.dry_mass);
        assert!(open.mass.dry_com.z < stowed.mass.dry_com.z);
        assert!(def.deploy(&[0]).is_err(), "the capsule has no mechanism");
    }
}
