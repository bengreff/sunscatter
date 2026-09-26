//! Built-in dev/test save fixture. Always appears in the Load Game list
//! under a synthetic ID and is generated fresh from code each time it's
//! loaded — never persisted to disk by the game itself (though once loaded,
//! the player can save under any name and it becomes a regular save).
//!
//! Contents:
//! - All tech tree nodes unlocked, all tech lines at tier 15.
//! - 1 trillion in company funds.
//! - Two self-sustaining colonies: Mercury (mass driver Mk III base) and
//!   Moon (secondary base).
//! - Mature Dyson swarm orbiting the Sun (50 mirrors, 2 collectors,
//!   several in-transit).
//! - A bare test probe spawned on Earth.
//!
//! Lifted heavily from `tests/generate_mass_driver_save.rs` so the fixture
//! matches what that test produces — except funds are bumped to 1e12.

use std::collections::{HashMap, HashSet};

use crate::bodies::SolarSystem;
use crate::colony::{
    BuildingInstance, BuildingType, Colony, ColonyManager, Company,
    ContractManager, DeployingMirror, DysonSwarm, FactoryRecipe, FleetManager,
    ResourceInventory, ResourceType, ScienceState,
};
use crate::ship::Ship;

use super::{SaveFileInfo, SaveGame, SavedVessel};

/// Sentinel save_id used to identify this fixture in the save list. The
/// double-underscore prefix avoids collision with anything `sanitize_save_name`
/// would produce from a human-typed name.
pub const FIXTURE_ID: &str = "__test_fixture__";

/// Display name shown in the Load Game list.
pub const FIXTURE_NAME: &str = "Test Fixture (dev)";

/// Synthetic `SaveFileInfo` for the save list. Pretends the file was
/// modified "now" so it sorts to a stable position.
pub fn fixture_info() -> SaveFileInfo {
    SaveFileInfo {
        name: FIXTURE_NAME.to_string(),
        save_id: FIXTURE_ID.to_string(),
        vessel_count: 1,
        simulation_time: 86400.0 * 365.0,
        modified: std::time::SystemTime::UNIX_EPOCH,
    }
}

/// Construct the full fixture as an in-memory `SaveGame`.
pub fn build() -> SaveGame {
    // Solar system used both for body-index lookup and for ship spawn.
    let solar_system = SolarSystem::new();
    let mercury_idx = solar_system
        .bodies
        .iter()
        .position(|b| b.name == "Mercury")
        .expect("Mercury body not found in solar system");
    let moon_idx = solar_system.moon_index;
    let sun_idx = solar_system.sun_index;

    // ------------------------------------------------------------------
    // Mercury Colony — primary mass driver base
    // ------------------------------------------------------------------
    let mut mercury = Colony::new(mercury_idx, "Mercury Base".to_string());

    for _ in 0..2 {
        mercury.buildings.push(make_building(BuildingType::Habitat));
    }
    mercury.crew = 40;

    for _ in 0..7 {
        mercury.buildings.push(make_greenhouse(5_000.0));
    }
    mercury.buildings.push(make_building(BuildingType::FoodStorage));
    mercury.food_stored = 16_000.0;

    for _ in 0..2 {
        mercury.buildings.push(make_building(BuildingType::SmallSolarFarm));
    }

    for _ in 0..17 {
        mercury.buildings.push(make_mine(ResourceType::MetalOre));
    }
    mercury.buildings.push(make_mine(ResourceType::RareEarthElements));

    for _ in 0..13 {
        mercury.buildings.push(make_factory(FactoryRecipe::MetalSmelting));
    }
    for _ in 0..30 {
        mercury.buildings.push(make_factory(FactoryRecipe::AlloyForging));
    }
    for _ in 0..29 {
        mercury.buildings.push(make_factory(FactoryRecipe::ElectronicsManufacturing));
    }
    for _ in 0..74 {
        mercury.buildings.push(make_factory(FactoryRecipe::SuperconductorFabrication));
    }
    for _ in 0..2 {
        mercury.buildings.push(make_factory(FactoryRecipe::MirrorSegmentAssembly));
    }
    mercury.buildings.push(make_factory(FactoryRecipe::CollectorStationAssembly));
    mercury.buildings.push(make_factory(FactoryRecipe::PrecisionInstrumentsManufacturing));

    mercury.buildings.push(make_building(BuildingType::MassDriverMk3));

    for _ in 0..17 {
        mercury.buildings.push(make_building(BuildingType::ReceiverArray));
    }
    mercury.buildings.push(make_building_degraded(BuildingType::ReceiverArray, 0.30));
    mercury.buildings.push(make_building_degraded(BuildingType::ReceiverArray, 0.50));

    for _ in 0..2 {
        mercury.buildings.push(make_building(BuildingType::ConstructionRobot));
    }
    for _ in 0..2 {
        mercury.buildings.push(make_building(BuildingType::Stockpile));
    }
    mercury.buildings.push(make_building(BuildingType::Launchpad));

    let mut mercury_res = ResourceInventory::new();
    mercury_res.add(ResourceType::MetalOre, 500_000.0);
    mercury_res.add(ResourceType::RareEarthElements, 50_000.0);
    mercury_res.add(ResourceType::Regolith, 50_000.0);
    mercury_res.add(ResourceType::StructuralMetal, 200_000.0);
    mercury_res.add(ResourceType::HighTempAlloys, 50_000.0);
    mercury_res.add(ResourceType::Electronics, 50_000.0);
    mercury_res.add(ResourceType::Superconductors, 50_000.0);
    mercury_res.add(ResourceType::PrecisionInstruments, 5_000.0);
    mercury_res.add(ResourceType::MirrorSegment, 50.0);
    mercury_res.add(ResourceType::CollectorStation, 10.0);
    mercury_res.add(ResourceType::Food, 10_000.0);
    mercury.resources = mercury_res;

    // ------------------------------------------------------------------
    // Moon Colony — secondary base
    // ------------------------------------------------------------------
    let mut moon = Colony::new(moon_idx, "Lunar Station".to_string());

    for _ in 0..2 {
        moon.buildings.push(make_building(BuildingType::Habitat));
    }
    moon.crew = 30;

    for _ in 0..5 {
        moon.buildings.push(make_greenhouse(5_000.0));
    }
    moon.buildings.push(make_building(BuildingType::FoodStorage));
    moon.food_stored = 16_000.0;

    moon.buildings.push(make_building(BuildingType::SmallSolarFarm));

    for _ in 0..4 {
        moon.buildings.push(make_mine(ResourceType::MetalOre));
    }
    moon.buildings.push(make_mine(ResourceType::RareEarthElements));
    moon.buildings.push(make_mine(ResourceType::Water));

    for _ in 0..3 {
        moon.buildings.push(make_factory(FactoryRecipe::MetalSmelting));
    }
    moon.buildings.push(make_factory(FactoryRecipe::AlloyForging));
    for _ in 0..2 {
        moon.buildings.push(make_factory(FactoryRecipe::ElectronicsManufacturing));
    }

    for _ in 0..3 {
        moon.buildings.push(make_building(BuildingType::ReceiverArray));
    }

    moon.buildings.push(make_building(BuildingType::ConstructionRobot));
    moon.buildings.push(make_building(BuildingType::Stockpile));
    moon.buildings.push(make_building(BuildingType::Launchpad));

    let mut moon_res = ResourceInventory::new();
    moon_res.add(ResourceType::MetalOre, 100_000.0);
    moon_res.add(ResourceType::RareEarthElements, 50_000.0);
    moon_res.add(ResourceType::Water, 50_000.0);
    moon_res.add(ResourceType::StructuralMetal, 50_000.0);
    moon_res.add(ResourceType::HighTempAlloys, 20_000.0);
    moon_res.add(ResourceType::Electronics, 20_000.0);
    moon_res.add(ResourceType::Food, 5_000.0);
    moon.resources = moon_res;

    let mut colony_manager = ColonyManager::default();
    colony_manager.colonies.push(mercury);
    colony_manager.colonies.push(moon);

    // ------------------------------------------------------------------
    // Tech tree — every node unlocked, every line at tier 15
    // ------------------------------------------------------------------
    let tech_unlocked: HashSet<String> = [
        "basic_rocketry", "structural_engineering", "kerolox_propulsion",
        "methalox_propulsion", "hydrolox_propulsion", "crewed_spaceflight",
        "medium_launch", "medium_hydrolox", "advanced_life_support",
        "heavy_lift", "large_cryogenic", "colony_engineering",
        "nuclear_thermal", "advanced_ntr", "ion_propulsion", "advanced_electric",
        "compact_fission", "heavy_fission", "super_heavy", "extended_missions",
        "mpd_propulsion", "deep_space_hab", "science_laboratory",
        "nuclear_pulse", "interstellar_fission", "passive_shielding",
        "mass_driver_tech", "fusion_probe", "fusion_full", "fusion_power",
        "active_shielding",
        "advanced_fusion", "geodesic_shielding", "swarm_power",
        "am_catalyzed", "am_production", "am_power", "heavy_mass_driver",
        "am_torch", "advanced_am_power", "ring_accelerator",
        "planetary_mass_driver", "photon_drive", "bulk_am_storage",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut tech_line_tiers: HashMap<String, u32> = HashMap::new();
    for line in [
        "mining", "metallurgy", "construction", "agriculture", "electronics_mfg",
        "life_support", "chemical_processing", "sail_technology",
        "atmospheric_science", "isotope_extraction", "precision_mfg",
        "nuclear_engineering", "swarm_power_delivery",
    ] {
        tech_line_tiers.insert(line.to_string(), 15);
    }

    let science = ScienceState {
        available: 5_000.0,
        cumulative_discovery: 15_000.0,
        cumulative_rd: 20_000.0,
        cumulative_lab: 5_000.0,
        ..Default::default()
    };

    // 1 trillion funds — main differentiator from the existing test save.
    let company = Company {
        money: 1_000_000_000_000.0,
        rd_budget: 10_000_000.0,
    };

    // Test probe on Earth's surface.
    let ship = Ship::spawn_on_earth(&solar_system);
    let vessel = SavedVessel {
        id: 1,
        name: "Test Probe".to_string(),
        ship,
        vessel: None,
        maneuver_nodes: Vec::new(),
        is_debris: false,
        interstellar_star_key: None,
    };

    // Dyson swarm — partially built around the Sun.
    let sim_time = 86400.0 * 365.0;
    let swarm = DysonSwarm {
        mirror_count: 50,
        deploying: vec![
            DeployingMirror { arrival_time: sim_time + 3_600.0 },
            DeployingMirror { arrival_time: sim_time + 7_200.0 },
            DeployingMirror { arrival_time: sim_time + 40_000.0 },
            DeployingMirror { arrival_time: sim_time + 60_000.0 },
            DeployingMirror { arrival_time: sim_time + 80_000.0 },
        ],
        collector_count: 2,
        deploying_collectors: vec![
            DeployingMirror { arrival_time: sim_time + 50_000.0 },
            DeployingMirror { arrival_time: sim_time + 120_000.0 },
        ],
    };
    let mut dyson_swarms = HashMap::new();
    dyson_swarms.insert(sun_idx, swarm);

    SaveGame {
        version: 1,
        name: FIXTURE_NAME.to_string(),
        simulation_time: sim_time,
        vessels: vec![vessel],
        next_vessel_id: 2,
        debris_counter: 0,
        blueprints: Vec::new(),
        editor_vessel_name: String::new(),
        colonies: colony_manager,
        company,
        science,
        tech_unlocked,
        tech_line_tiers,
        notifications: Vec::new(),
        contracts: ContractManager::default(),
        fleet: FleetManager::default(),
        dyson_swarm: DysonSwarm::default(),
        dyson_swarms,
        editor_blueprint: None,
    }
}

// ------------------- small construction helpers ----------------------

fn make_building(bt: BuildingType) -> BuildingInstance {
    BuildingInstance {
        building_type: bt,
        assigned_resource: None,
        assigned_recipe: None,
        operational: true,
        degradation: 0.0,
        water_fill: 0.0,
    }
}

fn make_building_degraded(bt: BuildingType, degradation: f64) -> BuildingInstance {
    BuildingInstance {
        building_type: bt,
        assigned_resource: None,
        assigned_recipe: None,
        operational: true,
        degradation,
        water_fill: 0.0,
    }
}

fn make_mine(resource: ResourceType) -> BuildingInstance {
    BuildingInstance {
        building_type: BuildingType::Mine,
        assigned_resource: Some(resource),
        assigned_recipe: None,
        operational: true,
        degradation: 0.0,
        water_fill: 0.0,
    }
}

fn make_factory(recipe: FactoryRecipe) -> BuildingInstance {
    BuildingInstance {
        building_type: BuildingType::Factory,
        assigned_resource: None,
        assigned_recipe: Some(recipe),
        operational: true,
        degradation: 0.0,
        water_fill: 0.0,
    }
}

fn make_greenhouse(water_fill: f64) -> BuildingInstance {
    BuildingInstance {
        building_type: BuildingType::AdvancedGreenhouse,
        assigned_resource: None,
        assigned_recipe: None,
        operational: true,
        degradation: 0.0,
        water_fill,
    }
}
