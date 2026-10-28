//! The parts nearest their temperature limits (Landing ▸ Thermal): per
//! part, the skin cell nearest its own limit; for a craft of parts each
//! part by its craft-file id (the design's `cell_part`), for the test craft
//! each of its geometry's primitives by name.

/// How much nearer its limit (share of it) a part must be to take a listed
/// one's place, or to pass it (`interface::steady::top`): on the pad every
/// part sits near the same share and the list would change every refresh.
pub const MARGIN: f64 = 0.02;

/// The `n` parts nearest their limits, given those listed before
/// (`listed`, by name): nearest first, but a listed part stays and keeps
/// its place until another is nearer by [`MARGIN`] (nothing flickers).
pub fn listed(ship: &sim::vessel::Vessel, listed: &[String], n: usize) -> Vec<(String, f64, f64)> {
    let all = parts(ship, usize::MAX);
    let scores: Vec<(String, f64)> = all.iter().map(|(name, t, limit)| (name.clone(), t / limit)).collect();
    let keep = crate::interface::steady::top(listed, &scores, n, MARGIN);
    keep.iter().filter_map(|k| all.iter().find(|p| p.0 == *k).cloned()).collect()
}

/// The `n` parts nearest their limits, nearest first: (name, K, limit K).
pub fn parts(ship: &sim::vessel::Vessel, n: usize) -> Vec<(String, f64, f64)> {
    let design = ship.craft.design();
    let skin = &ship.thermal().state.skin;
    let def = ship.ship_def();
    // A craft of parts: by part; the test craft: by primitive, named in its file.
    let (owner, names): (Vec<usize>, Vec<String>) = match &def {
        Some(def) => (
            design.cell_part.iter().map(|&k| k as usize).collect(),
            (design.parts.iter())
                .map(|p| def.parts.iter().find(|i| i.part == p.id).map_or_else(|| p.name.clone(), |i| i.id.clone()))
                .collect(),
        ),
        None => {
            let names = if ship.craft.id.0 == sim::craft::TEST_CRAFT {
                sim::craft::test_craft().geometry.primitives.iter().map(|p| p.name.clone()).collect()
            } else {
                Vec::new()
            };
            (design.cells.iter().map(|c| c.primitive as usize).collect(), names)
        }
    };
    let mut best: Vec<Option<(f64, f64)>> = Vec::new();
    for (i, &p) in owner.iter().enumerate() {
        let (Some(&t), Some(&limit)) = (skin.get(i), design.skin_max.get(i)) else { continue };
        if best.len() <= p {
            best.resize(p + 1, None);
        }
        if best[p].is_none_or(|(bt, bl)| t / limit > bt / bl) {
            best[p] = Some((t, limit));
        }
    }
    let mut out: Vec<(String, f64, f64)> = (best.into_iter().enumerate())
        .filter_map(|(p, b)| {
            let (t, limit) = b?;
            Some((names.get(p).cloned().unwrap_or_else(|| format!("part {}", p + 1)), t, limit))
        })
        .collect();
    // Ties by name (a stable order).
    out.sort_by(|a, b| (b.1 / b.2).total_cmp(&(a.1 / a.2)).then_with(|| a.0.cmp(&b.0)));
    out.truncate(n);
    out
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

    /// A craft of parts names its hottest parts by their craft-file ids.
    #[test]
    fn a_craft_of_parts_lists_its_parts_by_id() {
        let rocket = crate::test_rocket::rocket();
        let sim = crate::state::SimState::with_craft(&rocket);
        let hot = parts(sim.ship(), 4);
        assert_eq!(hot.len(), 4);
        let def = sim.ship().ship_def().unwrap();
        assert!(hot.iter().all(|(name, t, limit)| def.part_named(name).is_some() && t < limit), "{hot:?}");
        // The test craft by its primitives' names.
        let test = crate::state::SimState::new();
        let names: Vec<String> = parts(test.ship(), 2).into_iter().map(|p| p.0).collect();
        assert!(names.iter().all(|n| !n.starts_with("part ")), "{names:?}");
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
            let f9 = crate::state::SimState::with_craft(&crate::state::craft_or_test("falcon9-crew"));
            let def = f9.ship().ship_def().unwrap();
            let mount: f64 = def.parts.iter().filter(|p| p.ground).map(|p| p.mass).sum();
            assert!(mount > 0.0);
            assert!((f9.ship().mass() - crate::hud::vehicle_mass(f9.ship()) - mount).abs() < 1e-6);
        }
    }
}
