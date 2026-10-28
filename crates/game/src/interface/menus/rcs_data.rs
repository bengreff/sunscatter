//! The RCS menu's snapshot of a vessel ([`RcsData`]): its authority
//! (`Vessel::rcs_authority`), every feed of its thruster blocks (a bottle's
//! gas, a tank's liquid, as measured, D106) and every block (enabled, fed,
//! failed or unpowered, its part's temperature). Gathered with the rest of
//! `FlightData`.

use super::rcs::{Block, BlockState, Feed, RcsData};
use sim::craft::ship::Feed as Supply;
use sim::systems::rcs::RcsMode;
use sim::systems::Quantity;
use sim::vessel::Vessel;
use std::collections::BTreeSet;

/// The RCS of `v` with the pilot's mode `mode` (the test craft: the mode
/// only).
pub fn gather(v: &Vessel, mode: RcsMode) -> RcsData {
    let mut r = RcsData { mode, ..Default::default() };
    let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else { return r };
    r.authority = v.rcs_authority();
    let knowledge = v.knowledge();
    let (mut bottles, mut tanks) = (BTreeSet::new(), BTreeSet::new());
    for (t, s) in def.thrusters.iter().zip(&state.thrusters) {
        match &t.feed {
            Supply::Bottles(b) => bottles.extend(b.iter().copied()),
            Supply::Tanks([o, f]) => tanks.extend(o.iter().chain(f).copied()),
        }
        let fed = v.debug() || sim::systems::ship::block_fed(&t.feed, state);
        let block = if state.runtime.out(&t.id) {
            BlockState::Out
        } else if !fed {
            BlockState::Dry
        } else {
            BlockState::Ready
        };
        let measured = knowledge.knows(Quantity::PartTemperature(t.part));
        r.blocks.push(Block {
            id: t.id.clone(),
            nozzles: t.nozzles.len(),
            thrust: t.thrust,
            on: s.enabled,
            state: block,
            kelvin: super::hottest::skin_of(v, t.part).filter(|_| measured),
        });
    }
    for b in bottles {
        let (d, s) = (&def.bottles[b as usize], &state.bottles[b as usize]);
        let molar = sim::fluids::fluids().gas(&d.gas).map_or(0.0, |g| g.molar_mass);
        r.feeds.push(Feed {
            name: d.id.clone(),
            kg: s.moles * molar,
            full: def.bottle_gas(d),
            known: knowledge.knows(Quantity::BottlePressure(b)),
        });
    }
    for t in tanks {
        let (d, s) = (&def.tanks[t as usize], &state.tanks[t as usize]);
        r.feeds.push(Feed {
            name: d.id.clone(),
            kg: s.liquid,
            full: d.capacity,
            known: knowledge.knows(Quantity::TankLevel(t)),
        });
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::systems::rcs::RcsOnly;

    /// The test craft: the mode, no thrusters, no authority to compute.
    #[test]
    fn the_test_craft_has_the_mode_only() {
        let sim = crate::state::SimState::new();
        let mode = RcsMode { on: true, only: RcsOnly::Rotate, fine: true };
        assert_eq!(gather(sim.ship(), mode), RcsData { mode, ..Default::default() });
    }

    /// A craft of parts: every block once, enabled as its file says, fed
    /// and cool on the pad, with its feeds full.
    #[test]
    fn a_craft_of_parts_lists_every_block_and_feed() {
        let rocket = crate::test_rocket::rocket();
        let sim = crate::state::SimState::with_craft(&rocket);
        let r = gather(sim.ship(), RcsMode::default());
        let def = sim.ship().ship_def().unwrap();
        assert_eq!(r.blocks.len(), def.thrusters.len());
        assert!(!r.blocks.is_empty() && r.authority.is_some());
        for (b, t) in r.blocks.iter().zip(&def.thrusters) {
            assert_eq!((b.id.as_str(), b.on, b.nozzles), (t.id.as_str(), t.enabled, t.nozzles.len()));
            assert_eq!(b.state, BlockState::Ready, "{}", b.id);
            // Its part's skin, shown only with a thermocouple on it (D106).
            let skin = super::super::hottest::skin_of(sim.ship(), t.part);
            assert!(skin.is_some_and(|(k, limit)| k < limit), "{}: {skin:?}", b.id);
            let measured = sim.ship().knowledge().knows(Quantity::PartTemperature(t.part));
            assert_eq!(b.kelvin, skin.filter(|_| measured), "{}", b.id);
        }
        assert!(!r.feeds.is_empty());
        for f in &r.feeds {
            assert!(f.full > 0.0 && (f.kg - f.full).abs() < 1e-3 * f.full, "{f:?}");
        }
    }

    /// The Falcon 9 starts its booster's cold gas and Dragon's Dracos
    /// disabled (its sequence enables them): shown, switchable.
    #[test]
    fn the_falcon_9_shows_its_disabled_blocks() {
        if !crate::state::craft_exists("falcon9-crew") {
            return;
        }
        let sim = crate::state::SimState::with_craft(&crate::state::craft_or_test("falcon9-crew"));
        let r = gather(sim.ship(), RcsMode::default());
        let off: Vec<_> = r.blocks.iter().filter(|b| !b.on).map(|b| b.id.as_str()).collect();
        assert!(off.iter().any(|id| id.starts_with("s1.coldgas")), "{off:?}");
        assert!(off.iter().any(|id| id.starts_with("dragon.draco")), "{off:?}");
        assert!(r.blocks.iter().any(|b| b.on), "the second stage's are on");
    }
}
