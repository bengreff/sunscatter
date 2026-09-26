//! Integration tests for colony simulation invariants.
//!
//! These cover regressions for bugs fixed in the Phase A code review:
//! - Reactor fuel consumption atomicity (C4)
//! - Blueprint output determinism (C5)
//! - Independent per-building degradation (C6)

use std::collections::HashMap;
use sunscatter_app::bodies::SolarSystem;
use sunscatter_app::colony::{
    BuildingInstance, BuildingType, Colony, ResourceType, TechTree,
};
use sunscatter_app::colony::simulation::simulate_colony_tick;
use sunscatter_app::parts::{
    parts_to_blueprint, AttachmentType, FuelType, PlacedPart, PlacedPartId,
};

// ---- C4: Reactor atomicity ----

/// Three Fission reactors share a pool of 1.0 kg Enriched Uranium.
/// Each reactor needs 0.5 kg/day. Exactly two should fuel, one should starve.
/// Power generation should reflect only the two fueled reactors.
#[test]
fn reactor_fuel_is_consumed_atomically_per_building() {
    let solar_system = SolarSystem::new();
    let earth_idx = solar_system.earth_index;
    let mut colony = Colony::new(earth_idx, "Test".to_string());

    for _ in 0..3 {
        colony
            .buildings
            .push(BuildingInstance::new(BuildingType::FissionReactor));
    }
    colony.resources.add(ResourceType::EnrichedUranium, 1.0);

    let tech = TechTree::default();
    let mut notifications = Vec::new();

    simulate_colony_tick(
        &mut colony,
        1.0,
        &solar_system,
        &mut notifications,
        0.0,
        &tech,
        None,
        None,
    );

    // All uranium consumed (two reactors × 0.5 kg). Pool cannot go negative.
    let remaining = colony.resources.get(ResourceType::EnrichedUranium);
    assert!(
        remaining.abs() < 1e-9,
        "expected 0 kg EnrichedUranium remaining, got {}",
        remaining
    );

    // Only two reactors generate power (2 × 500 MW = 1 GW = 1_000_000 kW).
    // Earth has no habitat demand here, so total_generation is unaffected
    // by power balance clamping.
    let expected_power = 2.0 * 500_000.0;
    assert!(
        (colony.power_generated - expected_power).abs() < 1e-6,
        "expected {} kW power generated, got {}",
        expected_power,
        colony.power_generated
    );
}

/// Fusion reactors need BOTH He-3 AND Deuterium. If either is short, the
/// reactor must consume neither (atomic multi-resource removal).
#[test]
fn fusion_reactor_fuel_is_all_or_nothing() {
    let solar_system = SolarSystem::new();
    let earth_idx = solar_system.earth_index;
    let mut colony = Colony::new(earth_idx, "Test".to_string());
    colony
        .buildings
        .push(BuildingInstance::new(BuildingType::FusionReactor));

    // Plenty of Deuterium but zero He-3 — the reactor should consume nothing.
    colony.resources.add(ResourceType::Deuterium, 100.0);
    colony.resources.add(ResourceType::Helium3, 0.0);

    let tech = TechTree::default();
    let mut notifications = Vec::new();

    simulate_colony_tick(
        &mut colony,
        1.0,
        &solar_system,
        &mut notifications,
        0.0,
        &tech,
        None,
        None,
    );

    let deuterium_after = colony.resources.get(ResourceType::Deuterium);
    assert!(
        (deuterium_after - 100.0).abs() < 1e-9,
        "Fusion reactor should consume zero Deuterium when He-3 is missing, \
         but Deuterium dropped from 100.0 to {}",
        deuterium_after
    );
    assert_eq!(
        colony.power_generated, 0.0,
        "Unfueled fusion reactor should produce 0 kW, got {}",
        colony.power_generated
    );
}

// ---- C5: Blueprint determinism ----

/// Building a blueprint from a HashMap must produce a byte-stable RON
/// serialization regardless of the order in which parts were inserted.
/// Regression: `parts_to_blueprint` previously iterated HashMap entries
/// in nondeterministic order, causing saves to diff between runs.
#[test]
fn parts_to_blueprint_output_is_sorted_by_id() {
    let mut parts: HashMap<PlacedPartId, PlacedPart> = HashMap::new();

    // Insert in a scrambled order; output should still be ordered by ID.
    let ids: [PlacedPartId; 5] = [7, 2, 9, 1, 5];
    for &id in &ids {
        let mut part = PlacedPart::new(id, format!("part_{}", id), [id as f64, 0.0]);
        part.attachment_type = if id == 1 {
            AttachmentType::Root
        } else {
            AttachmentType::Stack
        };
        part.parent_id = if id == 1 { None } else { Some(1) };
        part.fuel_type = FuelType::Empty;
        parts.insert(id, part);
    }

    let stages: Vec<Vec<PlacedPartId>> = vec![vec![1, 2, 5, 7, 9]];
    let bp = parts_to_blueprint(&parts, 1, "test".to_string(), &stages);

    // Blueprint parts must be ordered by original PlacedPartId.
    let sorted_ids = [1u64, 2, 5, 7, 9];
    for (i, expected_id) in sorted_ids.iter().enumerate() {
        assert_eq!(
            bp.parts[i].definition_id,
            format!("part_{}", expected_id),
            "Part at index {} should be part_{}, got {}",
            i,
            expected_id,
            bp.parts[i].definition_id
        );
    }

    // root_part_index should point at id=1 (which is at sorted index 0).
    assert_eq!(bp.root_part_index, 0);
}

/// Serialize the same HashMap twice (with different internal orderings)
/// and verify the RON output is byte-identical.
#[test]
fn parts_to_blueprint_is_ron_byte_stable() {
    // Build once with ascending insertion order
    let mut parts_a: HashMap<PlacedPartId, PlacedPart> = HashMap::new();
    for id in [1u64, 2, 3, 4, 5] {
        let mut p = PlacedPart::new(id, format!("p{}", id), [0.0, id as f64]);
        p.attachment_type = if id == 1 {
            AttachmentType::Root
        } else {
            AttachmentType::Stack
        };
        p.parent_id = if id == 1 { None } else { Some(1) };
        parts_a.insert(id, p);
    }

    // Build again with descending insertion order
    let mut parts_b: HashMap<PlacedPartId, PlacedPart> = HashMap::new();
    for id in [5u64, 4, 3, 2, 1] {
        let mut p = PlacedPart::new(id, format!("p{}", id), [0.0, id as f64]);
        p.attachment_type = if id == 1 {
            AttachmentType::Root
        } else {
            AttachmentType::Stack
        };
        p.parent_id = if id == 1 { None } else { Some(1) };
        parts_b.insert(id, p);
    }

    let stages = vec![vec![1u64, 2, 3, 4, 5]];
    let bp_a = parts_to_blueprint(&parts_a, 1, "bp".to_string(), &stages);
    let bp_b = parts_to_blueprint(&parts_b, 1, "bp".to_string(), &stages);

    let ron_a = ron::ser::to_string(&bp_a).expect("serialize bp_a");
    let ron_b = ron::ser::to_string(&bp_b).expect("serialize bp_b");
    assert_eq!(
        ron_a, ron_b,
        "RON output must be byte-identical regardless of HashMap insertion order"
    );
}

// ---- C6: Independent per-building degradation ----

/// Two identical Habitats share a maintenance pool that only covers one.
/// The first building consumed should stay pristine (0 degradation),
/// while the second must accrue shortfall-based degradation.
/// Regression: earlier code averaged shortfall across all buildings of a type,
/// causing both to degrade equally even though resources fed one completely.
#[test]
fn habitat_degradation_is_independent_per_building() {
    let solar_system = SolarSystem::new();
    let earth_idx = solar_system.earth_index;
    let mut colony = Colony::new(earth_idx, "Test".to_string());

    // Two operational Habitats, zero crew (avoid food/life-support side effects).
    for _ in 0..2 {
        colony
            .buildings
            .push(BuildingInstance::new(BuildingType::Habitat));
    }

    // Habitat maintenance per 30 days: 40 kg StructuralMetal + 8 kg Electronics.
    // For a 1-day tick that means each habitat needs 40/30 and 8/30. Provide
    // exactly enough for ONE habitat so one starves completely.
    colony.resources.add(ResourceType::StructuralMetal, 40.0 / 30.0);
    colony.resources.add(ResourceType::Electronics, 8.0 / 30.0);

    let tech = TechTree::default();
    let mut notifications = Vec::new();

    simulate_colony_tick(
        &mut colony,
        1.0,
        &solar_system,
        &mut notifications,
        0.0,
        &tech,
        None,
        None,
    );

    let deg0 = colony.buildings[0].degradation;
    let deg1 = colony.buildings[1].degradation;

    // The first habitat drained the pool — it should be pristine.
    assert!(
        deg0.abs() < 1e-9,
        "First habitat should have 0 degradation (fully supplied), got {}",
        deg0
    );
    // The second habitat got nothing — it must have degraded.
    assert!(
        deg1 > 0.0,
        "Second habitat should have positive degradation (starved), got {}",
        deg1
    );
    // And they must differ — the regression averaged them to the same value.
    assert!(
        (deg0 - deg1).abs() > 1e-6,
        "Degradation should differ between fed and starved habitats, both = {}",
        deg0
    );
}

// ---- Storage Allocation ----

/// Per-resource storage caps should prevent one resource from filling global storage.
/// When MetalOre allocation is capped, mines should stop producing at that cap
/// while leaving room for other resources.
#[test]
fn per_resource_storage_cap_limits_mine_output() {
    let solar_system = SolarSystem::new();
    let earth_idx = solar_system.earth_index;
    let mut colony = Colony::new(earth_idx, "Test".to_string());

    // Power source
    colony.buildings.push(BuildingInstance::new(BuildingType::FissionReactor));
    colony.resources.add(ResourceType::EnrichedUranium, 100.0);
    // Storage
    colony.buildings.push(BuildingInstance::new(BuildingType::Stockpile)); // 500,000 kg

    // Two mines: one for MetalOre, one for Water
    let mut mine_metal = BuildingInstance::new(BuildingType::Mine);
    mine_metal.assigned_resource = Some(ResourceType::MetalOre);
    colony.buildings.push(mine_metal);

    let mut mine_water = BuildingInstance::new(BuildingType::Mine);
    mine_water.assigned_resource = Some(ResourceType::Water);
    colony.buildings.push(mine_water);

    // Pin MetalOre at 10% of storage = 50,000 kg
    colony.storage_allocation.set_pinned(ResourceType::MetalOre, 10.0);

    // Pre-fill MetalOre to near its cap
    colony.resources.add(ResourceType::MetalOre, 49_000.0);

    let tech = TechTree::default();
    let mut notifications = Vec::new();

    // Run a 1-day tick. MetalOre mine should produce at most ~1,000 kg (cap - current).
    // Water mine should produce its full 2,000 kg/day.
    simulate_colony_tick(
        &mut colony, 1.0, &solar_system, &mut notifications, 0.0, &tech, None, None,
    );

    let metal = colony.resources.get(ResourceType::MetalOre);
    let water = colony.resources.get(ResourceType::Water);

    // MetalOre should be capped around 50,000 (cap is 10% of 500,000)
    assert!(
        metal <= 50_000.0 + 1.0,
        "MetalOre should be capped at ~50,000, got {:.0}",
        metal
    );
    // Water should have produced freely (its allocation share should be large)
    assert!(
        water > 1_000.0,
        "Water mine should have produced, got {:.0}",
        water
    );
}

/// Auto-allocated resources should share remaining space equally.
#[test]
fn storage_allocation_auto_splits_evenly() {
    use sunscatter_app::colony::{StorageAllocation, compute_active_resources, ResourceInventory};

    let alloc = StorageAllocation::default();
    let mut inv = ResourceInventory::new();
    inv.add(ResourceType::MetalOre, 1.0);
    inv.add(ResourceType::Water, 1.0);
    inv.add(ResourceType::LithiumOre, 1.0);

    let production = HashMap::new();
    let active = compute_active_resources(&inv, &production);

    // 3 active resources, no pinning => each gets 100/3 ≈ 33.3%
    let cap_metal = alloc.capacity_for(ResourceType::MetalOre, 300_000.0, &active);
    let cap_water = alloc.capacity_for(ResourceType::Water, 300_000.0, &active);
    let cap_lithium = alloc.capacity_for(ResourceType::LithiumOre, 300_000.0, &active);

    assert!(
        (cap_metal - 100_000.0).abs() < 1.0,
        "Expected ~100,000 for MetalOre, got {:.0}", cap_metal
    );
    assert!(
        (cap_water - 100_000.0).abs() < 1.0,
        "Expected ~100,000 for Water, got {:.0}", cap_water
    );
    assert!(
        (cap_lithium - 100_000.0).abs() < 1.0,
        "Expected ~100,000 for LithiumOre, got {:.0}", cap_lithium
    );
}

/// Pinning one resource should reduce the auto-allocated share for others.
#[test]
fn storage_allocation_pinning_reduces_others() {
    use sunscatter_app::colony::{StorageAllocation, compute_active_resources, ResourceInventory};

    let mut alloc = StorageAllocation::default();
    alloc.set_pinned(ResourceType::MetalOre, 50.0); // Pin metal at 50%

    let mut inv = ResourceInventory::new();
    inv.add(ResourceType::MetalOre, 1.0);
    inv.add(ResourceType::Water, 1.0);

    let production = HashMap::new();
    let active = compute_active_resources(&inv, &production);

    // MetalOre pinned at 50%, Water gets the remaining 50%
    let cap_metal = alloc.capacity_for(ResourceType::MetalOre, 100_000.0, &active);
    let cap_water = alloc.capacity_for(ResourceType::Water, 100_000.0, &active);

    assert!(
        (cap_metal - 50_000.0).abs() < 1.0,
        "Expected 50,000 for pinned MetalOre, got {:.0}", cap_metal
    );
    assert!(
        (cap_water - 50_000.0).abs() < 1.0,
        "Expected 50,000 for auto Water, got {:.0}", cap_water
    );
}

/// Factory producing MirrorSegments should be capped by storage allocation
/// when the mirror count × mirror_mass exceeds the per-resource cap in kg.
/// Regression: the old code compared unit counts (1.0, 2.0, ...) directly
/// against kg caps (50,000 kg), so the cap never triggered.
#[test]
fn factory_mirror_output_capped_by_storage_allocation() {
    use sunscatter_app::colony::FactoryRecipe;

    let solar_system = SolarSystem::new();
    let earth_idx = solar_system.earth_index;
    let mut colony = Colony::new(earth_idx, "Test".to_string());

    // Power source
    colony.buildings.push(BuildingInstance::new(BuildingType::FissionReactor));
    colony.resources.add(ResourceType::EnrichedUranium, 1000.0);
    // Storage: 1 Stockpile = 500,000 kg
    colony.buildings.push(BuildingInstance::new(BuildingType::Stockpile));

    // Factory producing mirrors
    let mut factory = BuildingInstance::new(BuildingType::Factory);
    factory.assigned_recipe = Some(FactoryRecipe::MirrorSegmentAssembly);
    colony.buildings.push(factory);

    // Provide ample inputs for many batches
    colony.resources.add(ResourceType::StructuralMetal, 100_000.0);
    colony.resources.add(ResourceType::HighTempAlloys, 50_000.0);
    colony.resources.add(ResourceType::Electronics, 20_000.0);
    colony.resources.add(ResourceType::Superconductors, 10_000.0);

    // Pin MirrorSegment at 5% of storage = 25,000 kg cap.
    // At tier 0, each mirror = 3,500 kg, so cap = 7 mirrors (7 × 3,500 = 24,500 < 25,000).
    colony.storage_allocation.set_pinned(ResourceType::MirrorSegment, 5.0);

    let tech = TechTree::default();
    let mut notifications = Vec::new();

    // Run 50 one-day ticks. Factory does 0.5 batches/day = 0.5 mirrors/tick.
    // Without cap: 25 mirrors. With 25,000 kg cap: stops at 7 mirrors (24,500 kg).
    for day in 0..50 {
        simulate_colony_tick(
            &mut colony, 1.0, &solar_system, &mut notifications, day as f64 * 86400.0, &tech, None, None,
        );
    }

    let mirrors = colony.resources.get(ResourceType::MirrorSegment);
    let mirror_mass = sunscatter_app::colony::dyson_swarm::mirror_mass_at_tier(0);
    let total_mirror_kg = mirrors * mirror_mass;
    let cap_kg = 500_000.0 * 0.05; // 25,000 kg

    assert!(
        total_mirror_kg <= cap_kg + mirror_mass,
        "Mirror storage mass ({:.0} kg, {} mirrors) should not exceed cap ({:.0} kg)",
        total_mirror_kg, mirrors, cap_kg,
    );
    // Should have produced some mirrors (not zero)
    assert!(
        mirrors >= 1.0,
        "Factory should have produced at least 1 mirror, got {:.1}",
        mirrors,
    );
    // Should be well below the uncapped amount (25 mirrors)
    assert!(
        mirrors < 10.0,
        "Factory should be capped well below 25 mirrors, got {:.1}",
        mirrors,
    );
}

