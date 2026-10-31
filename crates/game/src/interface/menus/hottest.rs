//! The parts nearest their temperature limits (Landing ▸ Thermal), by the
//! systems screen's rule (`systems_screen::rules::nearest_limits`, one
//! owner; interface-map.md finding 13): each part by its skin cell or
//! interior node nearest its own limit, where the craft measures its
//! temperature (D106; every part in debug mode). The parts nothing measures
//! are counted, never ranked by the truth. A craft of parts names each part
//! by its craft-file id.

use crate::systems_screen::rules;

/// How much nearer its limit (share of it) a part must be to take a listed
/// one's place, or to pass it (`interface::steady::top`): on the pad every
/// part sits near the same share and the list would change every refresh.
pub const MARGIN: f64 = 0.02;

/// The parts nearest their limits, nearest first: (name, K, limit K); and
/// how many parts no sensor measures.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hottest {
    pub parts: Vec<(String, f64, f64)>,
    pub unmeasured: usize,
}

/// The `n` parts nearest their limits, given those listed before
/// (`listed`, by name): nearest first, but a listed part stays and keeps
/// its place until another is nearer by [`MARGIN`] (nothing flickers).
pub fn listed(ship: &sim::vessel::Vessel, listed: &[String], n: usize) -> Hottest {
    let all = parts(ship, usize::MAX);
    let scores: Vec<(String, f64)> = all.parts.iter().map(|(name, t, limit)| (name.clone(), t / limit)).collect();
    let keep = crate::interface::steady::top(listed, &scores, n, MARGIN);
    let parts = keep.iter().filter_map(|k| all.parts.iter().find(|p| p.0 == *k).cloned()).collect();
    Hottest { parts, ..all }
}

/// The `n` measured parts nearest their limits, nearest first, and how
/// many are not measured; the launch mount (ground parts) is not the
/// vehicle's. Nothing for a craft without parts.
pub fn parts(ship: &sim::vessel::Vessel, n: usize) -> Hottest {
    use sim::systems::Quantity;
    let Some(def) = ship.ship_def() else { return Hottest::default() };
    let known = ship.knowledge();
    let design = ship.craft.design();
    let temps = rules::vessel_temperatures(ship);
    let named = (design.parts.iter().zip(temps)).filter_map(|(p, (id, (skin, interior)))| {
        let info = def.parts.iter().find(|i| i.part == p.id);
        if info.is_some_and(|i| i.ground) {
            return None;
        }
        let name = info.map_or_else(|| p.name.clone(), |i| i.id.clone());
        let measured = known.knows(Quantity::PartTemperature(id));
        Some((name, rules::nearer(skin, interior).filter(|_| measured)))
    });
    let (hot, unmeasured) = rules::nearest_limits(named, n);
    Hottest { parts: hot.into_iter().map(|(name, (t, limit))| (name, t, limit)).collect(), unmeasured }
}

/// Part `part`'s (`Part::id`) skin cell nearest its own limit: (K, limit K).
pub fn skin_of(ship: &sim::vessel::Vessel, part: u32) -> Option<(f64, f64)> {
    let design = ship.craft.design();
    let skin = &ship.thermal().state.skin;
    let k = design.parts.iter().position(|p| p.id == part)? as u32;
    (design.cell_part.iter().enumerate())
        .filter(|(_, &p)| p == k)
        .filter_map(|(i, _)| Some((*skin.get(i)?, *design.skin_max.get(i)?)))
        .max_by(|a, b| (a.0 / a.1).total_cmp(&(b.0 / b.1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measured parts by their craft-file ids, nearest their limits;
    /// without sensors none is ranked (D106) and all are counted; debug mode
    /// measures every part.
    #[test]
    fn only_the_measured_parts_are_ranked_and_the_rest_counted() {
        let rocket = crate::test_rocket::rocket();
        let mut sim = crate::state::SimState::with_craft(&rocket);
        let def = sim.ship().ship_def().unwrap();
        let vehicle = def.parts.iter().filter(|p| !p.ground).count();
        let blind = parts(sim.ship(), 4);
        assert_eq!((blind.parts.len(), blind.unmeasured), (0, vehicle), "no temperature sensor: {blind:?}");
        let world = sim.world.clone();
        let active = sim.active;
        sim.fleet[active].set_debug(&world, true);
        let hot = parts(sim.ship(), 4);
        assert_eq!((hot.parts.len(), hot.unmeasured), (4.min(vehicle), 0));
        assert!(hot.parts.iter().all(|(name, t, limit)| def.part_named(name).is_some() && t < limit), "{hot:?}");
        let shares: Vec<f64> = hot.parts.iter().map(|p| p.1 / p.2).collect();
        assert!(shares.windows(2).all(|w| w[0] >= w[1]), "nearest first: {shares:?}");
    }

    /// The launch mount's mass is not the vehicle's.
    #[test]
    fn the_vehicle_mass_leaves_out_the_ground() {
        let rocket = crate::test_rocket::rocket();
        let sim = crate::state::SimState::with_craft(&rocket);
        assert_eq!(crate::hud::vehicle_mass(sim.ship()), sim.ship().mass(), "no ground part");
        let test = crate::state::SimState::new();
        assert_eq!(crate::hud::vehicle_mass(test.ship()), test.ship().mass());
        if crate::state::craft_exists("falcon9-crew") {
            let f9 = crate::state::SimState::with_craft(&crate::state::craft_or_fallback("falcon9-crew"));
            let def = f9.ship().ship_def().unwrap();
            let mount: f64 = def.parts.iter().filter(|p| p.ground).map(|p| p.mass).sum();
            assert!(mount > 0.0);
            assert!((f9.ship().mass() - crate::hud::vehicle_mass(f9.ship()) - mount).abs() < 1e-6);
        }
    }
}
