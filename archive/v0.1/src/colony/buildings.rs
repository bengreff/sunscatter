use serde::{Deserialize, Serialize};

use super::resources::{ResourceInventory, ResourceType, StorageAllocation};
use super::trade::{StoredShip, StoredShipId, TradeShipId, TradeRouteId};
use crate::parts::VesselBlueprint;

/// An entry in the colony mass driver ship queue.
/// Ships wait here until the mass driver accumulates enough energy to launch them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MassDriverShipEntry {
    pub trade_ship_id: TradeShipId,
    pub route_id: TradeRouteId,
    pub mass_kg: f64,
    pub driver_tier: BuildingType,
}

/// All colony building types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingType {
    Habitat,
    BasicGreenhouse,
    AdvancedGreenhouse,
    SmallSolarFarm,
    MediumSolarFarm,
    LargeSolarFarm,
    FissionReactor,
    FusionReactor,
    Mine,
    AtmosphericCollector,
    Factory,
    Launchpad,
    MassDriverMk1,
    MassDriverMk2,
    MassDriverMk3,
    MassDriverMk4,
    LightConstructionRobot,
    ConstructionRobot,
    ScienceLab,
    Stockpile,
    FoodStorage,
    ParticleAcceleratorMk1,
    ParticleAcceleratorMk2,
    ParticleAcceleratorMk3,
    ParticleAcceleratorMk4,
    Hangar,
    ReceiverArray,
}

impl BuildingType {
    /// All building types.
    pub fn all() -> &'static [BuildingType] {
        &[
            Self::Habitat,
            Self::BasicGreenhouse,
            Self::AdvancedGreenhouse,
            Self::SmallSolarFarm,
            Self::MediumSolarFarm,
            Self::LargeSolarFarm,
            Self::FissionReactor,
            Self::FusionReactor,
            Self::Mine,
            Self::AtmosphericCollector,
            Self::Factory,
            Self::Launchpad,
            Self::MassDriverMk1,
            Self::MassDriverMk2,
            Self::MassDriverMk3,
            Self::MassDriverMk4,
            Self::LightConstructionRobot,
            Self::ConstructionRobot,
            Self::ScienceLab,
            Self::Stockpile,
            Self::FoodStorage,
            Self::ParticleAcceleratorMk1,
            Self::ParticleAcceleratorMk2,
            Self::ParticleAcceleratorMk3,
            Self::ParticleAcceleratorMk4,
            Self::Hangar,
            Self::ReceiverArray,
        ]
    }

    /// Look up a BuildingType by its display name.
    pub fn from_display_name(name: &str) -> Option<BuildingType> {
        Self::all().iter().find(|bt| bt.display_name() == name).copied()
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Habitat => "Habitat",
            Self::BasicGreenhouse => "Basic Greenhouse",
            Self::AdvancedGreenhouse => "Advanced Greenhouse",
            Self::SmallSolarFarm => "Small Solar Farm",
            Self::MediumSolarFarm => "Medium Solar Farm",
            Self::LargeSolarFarm => "Large Solar Farm",
            Self::FissionReactor => "Fission Reactor",
            Self::FusionReactor => "Fusion Reactor",
            Self::Mine => "Mine",
            Self::AtmosphericCollector => "Atmospheric Collector",
            Self::Factory => "Factory",
            Self::Launchpad => "Launchpad",
            Self::MassDriverMk1 => "Mass Driver Mk I",
            Self::MassDriverMk2 => "Mass Driver Mk II",
            Self::MassDriverMk3 => "Mass Driver Mk III",
            Self::MassDriverMk4 => "Mass Driver Mk IV",
            Self::LightConstructionRobot => "Light Construction Robot",
            Self::ConstructionRobot => "Construction Robot",
            Self::ScienceLab => "Science Lab",
            Self::Stockpile => "Stockpile",
            Self::FoodStorage => "Food Storage",
            Self::ParticleAcceleratorMk1 => "Particle Accelerator Mk I",
            Self::ParticleAcceleratorMk2 => "Particle Accelerator Mk II",
            Self::ParticleAcceleratorMk3 => "Particle Accelerator Mk III",
            Self::ParticleAcceleratorMk4 => "Particle Accelerator Mk IV",
            Self::Hangar => "Hangar",
            Self::ReceiverArray => "Receiver Array",
        }
    }

    /// Base build cost in kg: Vec of (ResourceType, amount).
    /// For Habitat and greenhouses, these are pre-multiplier costs.
    pub fn build_cost(&self) -> Vec<(ResourceType, f64)> {
        use ResourceType::*;
        match self {
            Self::Habitat => vec![
                (StructuralMetal, 8_000.0),
                (Electronics, 1_000.0),
            ],
            Self::BasicGreenhouse => vec![
                (StructuralMetal, 5_000.0),
                (Electronics, 3_000.0),
            ],
            Self::AdvancedGreenhouse => vec![
                (StructuralMetal, 5_000.0),
                (Electronics, 3_000.0),
            ],
            Self::SmallSolarFarm => vec![
                (StructuralMetal, 10_000.0),
                (Electronics, 5_000.0),
            ],
            Self::MediumSolarFarm => vec![
                (StructuralMetal, 100_000.0),
                (Electronics, 50_000.0),
            ],
            Self::LargeSolarFarm => vec![
                (StructuralMetal, 1_000_000.0),
                (Electronics, 500_000.0),
            ],
            Self::FissionReactor => vec![
                (StructuralMetal, 200_000.0),
                (HighTempAlloys, 100_000.0),
                (Electronics, 100_000.0),
                (Superconductors, 50_000.0),
            ],
            Self::FusionReactor => vec![
                (StructuralMetal, 500_000.0),
                (HighTempAlloys, 200_000.0),
                (Electronics, 300_000.0),
                (Superconductors, 400_000.0),
                (PrecisionInstruments, 200.0),
            ],
            Self::Mine => vec![
                (StructuralMetal, 20_000.0),
                (HighTempAlloys, 5_000.0),
                (Electronics, 5_000.0),
            ],
            Self::AtmosphericCollector => vec![
                (StructuralMetal, 15_000.0),
                (HighTempAlloys, 3_000.0),
                (Electronics, 8_000.0),
            ],
            Self::Factory => vec![
                (StructuralMetal, 50_000.0),
                (HighTempAlloys, 10_000.0),
                (Electronics, 30_000.0),
            ],
            Self::Launchpad => vec![
                (StructuralMetal, 30_000.0),
                (HighTempAlloys, 5_000.0),
                (Electronics, 5_000.0),
            ],
            Self::MassDriverMk1 => vec![
                (StructuralMetal, 150_000.0),
                (HighTempAlloys, 50_000.0),
                (Electronics, 100_000.0),
                (Superconductors, 200_000.0),
            ],
            Self::MassDriverMk2 => vec![
                (StructuralMetal, 1_500_000.0),
                (HighTempAlloys, 500_000.0),
                (Electronics, 1_000_000.0),
                (Superconductors, 2_000_000.0),
            ],
            Self::MassDriverMk3 => vec![
                (StructuralMetal, 15_000_000.0),
                (HighTempAlloys, 5_000_000.0),
                (Electronics, 10_000_000.0),
                (Superconductors, 20_000_000.0),
                (PrecisionInstruments, 500.0),
            ],
            Self::MassDriverMk4 => vec![
                (StructuralMetal, 60_000_000.0),
                (HighTempAlloys, 20_000_000.0),
                (Electronics, 40_000_000.0),
                (Superconductors, 80_000_000.0),
                (PrecisionInstruments, 2_000.0),
            ],
            Self::LightConstructionRobot => vec![
                (StructuralMetal, 3_000.0),
                (HighTempAlloys, 1_500.0),
                (Electronics, 5_000.0),
            ],
            Self::ConstructionRobot => vec![
                (StructuralMetal, 10_000.0),
                (HighTempAlloys, 5_000.0),
                (Electronics, 15_000.0),
            ],
            Self::ScienceLab => vec![
                (StructuralMetal, 30_000.0),
                (HighTempAlloys, 5_000.0),
                (Electronics, 20_000.0),
            ],
            Self::Stockpile => vec![
                (StructuralMetal, 20_000.0),
            ],
            Self::FoodStorage => vec![
                (StructuralMetal, 5_000.0),
                (Electronics, 500.0),
            ],
            Self::ParticleAcceleratorMk1 => vec![
                (StructuralMetal, 1_500_000.0),
                (HighTempAlloys, 500_000.0),
                (Electronics, 1_000_000.0),
                (Superconductors, 2_000_000.0),
                (PrecisionInstruments, 500.0),
            ],
            Self::ParticleAcceleratorMk2 => vec![
                (StructuralMetal, 15_000_000.0),
                (HighTempAlloys, 5_000_000.0),
                (Electronics, 10_000_000.0),
                (Superconductors, 20_000_000.0),
                (PrecisionInstruments, 5_000.0),
            ],
            Self::ParticleAcceleratorMk3 => vec![
                (StructuralMetal, 150_000_000.0),
                (HighTempAlloys, 50_000_000.0),
                (Electronics, 100_000_000.0),
                (Superconductors, 200_000_000.0),
                (PrecisionInstruments, 50_000.0),
            ],
            // Mk IV costs are per-km and scale with body circumference.
            // Base values per km stored here; simulation multiplies by circumference.
            Self::ParticleAcceleratorMk4 => vec![
                (StructuralMetal, 150_000.0),
                (HighTempAlloys, 50_000.0),
                (Electronics, 100_000.0),
                (Superconductors, 200_000.0),
                (PrecisionInstruments, 50.0),
            ],
            Self::Hangar => vec![
                (StructuralMetal, 50_000.0),
                (HighTempAlloys, 10_000.0),
                (Electronics, 5_000.0),
            ],
            Self::ReceiverArray => vec![
                (StructuralMetal, 100_000.0),
                (HighTempAlloys, 20_000.0),
                (Electronics, 30_000.0),
                (Superconductors, 10_000.0),
            ],
        }
    }

    /// Power draw in kW. Factory power varies by recipe and is not included here.
    pub fn power_draw_kw(&self) -> f64 {
        match self {
            Self::Habitat => 10.0,
            Self::BasicGreenhouse => 50.0,
            Self::AdvancedGreenhouse => 50.0,
            Self::SmallSolarFarm => 0.0,
            Self::MediumSolarFarm => 0.0,
            Self::LargeSolarFarm => 0.0,
            Self::FissionReactor => 0.0,
            Self::FusionReactor => 0.0,
            Self::Mine => 100.0,
            Self::AtmosphericCollector => 100.0,
            Self::Factory => 0.0, // Varies by recipe
            Self::Launchpad => 10.0,
            Self::MassDriverMk1 => 10_000.0,       // 10 MW
            Self::MassDriverMk2 => 100_000.0,      // 100 MW
            Self::MassDriverMk3 => 1_000_000.0,    // 1 GW
            Self::MassDriverMk4 => 100_000_000.0,  // 100 GW
            Self::LightConstructionRobot => 100.0,
            Self::ConstructionRobot => 500.0,
            Self::ScienceLab => 200.0,
            Self::Stockpile => 0.0,
            Self::FoodStorage => 5.0,
            Self::ParticleAcceleratorMk1 => 50_000_000.0, // 50 GW
            Self::ParticleAcceleratorMk2 => 500_000_000.0, // 500 GW
            Self::ParticleAcceleratorMk3 => 5_000_000_000.0, // 5 TW
            Self::ParticleAcceleratorMk4 => 50_000.0, // 50 GW per km (scaled by circumference)
            Self::Hangar => 50.0,
            Self::ReceiverArray => 0.0,
        }
    }

    /// Power output in kW. Solar farms output at 1 AU; actual output scales with distance.
    pub fn power_output_kw(&self) -> f64 {
        match self {
            Self::SmallSolarFarm => 10_000.0, // 10 MW @ 1 AU
            Self::MediumSolarFarm => 100_000.0, // 100 MW @ 1 AU
            Self::LargeSolarFarm => 1_000_000.0, // 1 GW @ 1 AU
            Self::FissionReactor => 500_000.0, // 500 MW
            Self::FusionReactor => 5_000_000.0, // 5 GW
            Self::ReceiverArray => 45_000_000.0, // 45 GW (50 GW laser input × 0.90 efficiency)
            _ => 0.0,
        }
    }

    /// Maintenance cost per 30 days in kg: Vec of (ResourceType, amount).
    /// For Habitat and greenhouses, these are pre-multiplier costs.
    pub fn maintenance_cost_per_30d(&self) -> Vec<(ResourceType, f64)> {
        use ResourceType::*;
        match self {
            Self::Habitat => vec![
                (StructuralMetal, 40.0),
                (Electronics, 8.0),
            ],
            Self::BasicGreenhouse => vec![
                (StructuralMetal, 25.0),
                (Electronics, 13.0),
            ],
            Self::AdvancedGreenhouse => vec![
                (StructuralMetal, 25.0),
                (Electronics, 13.0),
            ],
            Self::SmallSolarFarm => vec![
                (StructuralMetal, 13.0),
                (Electronics, 5.0),
            ],
            Self::MediumSolarFarm => vec![
                (StructuralMetal, 125.0),
                (Electronics, 50.0),
            ],
            Self::LargeSolarFarm => vec![
                (StructuralMetal, 1_250.0),
                (Electronics, 500.0),
            ],
            Self::FissionReactor => vec![
                (StructuralMetal, 750.0),
                (HighTempAlloys, 250.0),
            ],
            Self::FusionReactor => vec![
                (StructuralMetal, 1_250.0),
                (HighTempAlloys, 500.0),
                (Superconductors, 250.0),
                (PrecisionInstruments, 2.0),
            ],
            Self::Mine => vec![
                (StructuralMetal, 75.0),
                (HighTempAlloys, 13.0),
            ],
            Self::AtmosphericCollector => vec![
                (StructuralMetal, 50.0),
                (Electronics, 13.0),
            ],
            Self::Factory => vec![
                (StructuralMetal, 125.0),
                (HighTempAlloys, 25.0),
                (Electronics, 25.0),
            ],
            Self::Launchpad => vec![
                (StructuralMetal, 125.0),
                (HighTempAlloys, 25.0),
            ],
            Self::MassDriverMk1 => vec![
                (StructuralMetal, 375.0),
                (HighTempAlloys, 125.0),
                (Superconductors, 500.0),
            ],
            Self::MassDriverMk2 => vec![
                (StructuralMetal, 3_750.0),
                (HighTempAlloys, 1_250.0),
                (Superconductors, 5_000.0),
            ],
            Self::MassDriverMk3 => vec![
                (StructuralMetal, 37_500.0),
                (HighTempAlloys, 12_500.0),
                (Superconductors, 50_000.0),
            ],
            Self::MassDriverMk4 => vec![
                (StructuralMetal, 150_000.0),
                (HighTempAlloys, 50_000.0),
                (Superconductors, 200_000.0),
            ],
            Self::LightConstructionRobot => vec![
                (StructuralMetal, 15.0),
                (HighTempAlloys, 8.0),
                (Electronics, 15.0),
            ],
            Self::ConstructionRobot => vec![
                (StructuralMetal, 50.0),
                (HighTempAlloys, 25.0),
                (Electronics, 50.0),
            ],
            Self::ScienceLab => vec![
                (StructuralMetal, 100.0),
                (Electronics, 25.0),
            ],
            Self::Stockpile => vec![],
            Self::FoodStorage => vec![
                (StructuralMetal, 10.0),
            ],
            Self::ParticleAcceleratorMk1 => vec![
                (StructuralMetal, 12_500.0),
                (HighTempAlloys, 5_000.0),
                (Electronics, 7_500.0),
                (Superconductors, 12_500.0),
                (PrecisionInstruments, 5.0),
            ],
            Self::ParticleAcceleratorMk2 => vec![
                (StructuralMetal, 125_000.0),
                (HighTempAlloys, 50_000.0),
                (Electronics, 75_000.0),
                (Superconductors, 125_000.0),
                (PrecisionInstruments, 50.0),
            ],
            Self::ParticleAcceleratorMk3 => vec![
                (StructuralMetal, 1_250_000.0),
                (HighTempAlloys, 500_000.0),
                (Electronics, 750_000.0),
                (Superconductors, 1_250_000.0),
                (PrecisionInstruments, 500.0),
            ],
            // Per-km maintenance; scaled by body circumference in simulation
            Self::ParticleAcceleratorMk4 => vec![
                (StructuralMetal, 250.0),
                (HighTempAlloys, 75.0),
                (Electronics, 125.0),
                (Superconductors, 250.0),
                (PrecisionInstruments, 0.05),
            ],
            Self::Hangar => vec![
                (StructuralMetal, 125.0),
                (HighTempAlloys, 25.0),
            ],
            Self::ReceiverArray => vec![
                (StructuralMetal, 250.0),
                (HighTempAlloys, 50.0),
                (Electronics, 75.0),
                (Superconductors, 25.0),
            ],
        }
    }

    /// Total build mass in kg (sum of build_cost amounts + pre-stocked supplies).
    /// Habitat includes 1,000 kg of pre-stocked food.
    pub fn total_build_mass(&self) -> f64 {
        let material_mass: f64 = self.build_cost().iter().map(|(_, amt)| amt).sum();
        match self {
            Self::Habitat => material_mass + 1_000.0,
            _ => material_mass,
        }
    }

    /// Whether this building's costs/power/maintenance are affected by the habitability multiplier.
    /// Only Habitats and Greenhouses are affected.
    pub fn affected_by_habitability(&self) -> bool {
        matches!(
            self,
            Self::Habitat | Self::BasicGreenhouse | Self::AdvancedGreenhouse
        )
    }

    /// Scale factor applied to `build_cost`, `total_build_mass`, `power_draw_kw`,
    /// and `maintenance_cost_per_30d` for buildings whose footprint scales with
    /// the host body's size. The Mk IV Particle Accelerator wraps the entire
    /// equator, so its base values are stored per kilometer and the simulation
    /// multiplies them by the body's circumference in km.
    /// Returns 1.0 for all other buildings.
    pub fn size_multiplier(&self, body_radius_m: f64) -> f64 {
        match self {
            Self::ParticleAcceleratorMk4 => {
                let radius_km = body_radius_m / 1000.0;
                radius_km * 2.0 * std::f64::consts::PI
            }
            _ => 1.0,
        }
    }
    /// Mass driver track length in meters, or None if not a mass driver.
    pub fn mass_driver_track_m(&self) -> Option<f64> {
        match self {
            Self::MassDriverMk1 => Some(2_000.0),
            Self::MassDriverMk2 => Some(10_000.0),
            Self::MassDriverMk3 => Some(50_000.0),
            Self::MassDriverMk4 => Some(200_000.0),
            _ => None,
        }
    }

    /// Mass driver max payload in kg, or None if not a mass driver.
    pub fn mass_driver_max_payload_kg(&self) -> Option<f64> {
        match self {
            Self::MassDriverMk1 => Some(10_000.0),
            Self::MassDriverMk2 => Some(50_000.0),
            Self::MassDriverMk3 => Some(200_000.0),
            Self::MassDriverMk4 => Some(200_000.0),
            _ => None,
        }
    }

    /// Mass driver system mass in kg, or None if not a mass driver.
    pub fn mass_driver_system_mass_kg(&self) -> Option<f64> {
        match self {
            Self::MassDriverMk1 => Some(500_000.0),
            Self::MassDriverMk2 => Some(5_000_000.0),
            Self::MassDriverMk3 => Some(50_000_000.0),
            Self::MassDriverMk4 => Some(200_000_000.0),
            _ => None,
        }
    }

    /// Whether this building is a mass driver.
    pub fn is_mass_driver(&self) -> bool {
        self.mass_driver_track_m().is_some()
    }

    /// Compute mass driver launch velocity (m/s) for a given payload mass (kg).
    /// `is_mirror` selects 10,000g accel (mirror segment) vs 1,000g (blueprint ship).
    /// Returns None if not a mass driver or payload exceeds max.
    pub fn mass_driver_launch_velocity(&self, payload_mass_kg: f64, is_mirror: bool) -> Option<f64> {
        let track = self.mass_driver_track_m()?;
        let max_payload = self.mass_driver_max_payload_kg()?;
        if payload_mass_kg > max_payload + 1e-3 {
            return None;
        }
        let accel = if is_mirror { 98_100.0 } else { 9_810.0 }; // m/s²
        // v = sqrt(2 * a * s)
        Some((2.0 * accel * track).sqrt())
    }

    /// Kinetic energy (joules) to launch a payload at a given velocity.
    /// Accounts for 90% superconducting efficiency.
    pub fn mass_driver_launch_energy_j(payload_mass_kg: f64, velocity_ms: f64) -> f64 {
        let ke = 0.5 * payload_mass_kg * velocity_ms * velocity_ms;
        ke / 0.90 // η = 0.90 superconducting efficiency
    }

    /// Maximum energy storage capacity (joules) for a mass driver.
    /// Must exceed the worst-case launch: max_payload at max acceleration.
    /// Returns None if not a mass driver.
    pub fn mass_driver_energy_capacity_j(&self) -> Option<f64> {
        match self {
            Self::MassDriverMk1 => Some(1e12),            // 1 TJ   (worst launch: 766 GJ mirror)
            Self::MassDriverMk2 => Some(10e12),           // 10 TJ  (worst launch: 5.45 TJ ship)
            Self::MassDriverMk3 => Some(150e12),          // 150 TJ (worst launch: 109 TJ ship)
            Self::MassDriverMk4 => Some(500e12),          // 500 TJ (worst launch: 436 TJ ship)
            _ => None,
        }
    }

    /// Recharge time (seconds) given launch energy (J) and available power (W).
    pub fn mass_driver_recharge_time_s(energy_j: f64, power_w: f64) -> f64 {
        if power_w <= 0.0 {
            return f64::INFINITY;
        }
        energy_j / power_w
    }
}

/// Factory recipes that can be assigned to a Factory building.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FactoryRecipe {
    MetalSmelting,
    AlloyForging,
    ElectronicsManufacturing,
    SuperconductorFabrication,
    PrecisionInstrumentsManufacturing,
    Electrolysis,
    DeuteriumExtraction,
    SabatierReaction,
    MethanePurification,
    KeroseneRefining,
    UraniumEnrichment,
    TritiumBreeding,
    NpuAssembly,
    RegolithHe3Extraction,
    GasGiantHe3Separation,
    MirrorSegmentAssembly,
    CollectorStationAssembly,
}

impl FactoryRecipe {
    pub fn all() -> &'static [FactoryRecipe] {
        &[
            Self::MetalSmelting,
            Self::AlloyForging,
            Self::ElectronicsManufacturing,
            Self::SuperconductorFabrication,
            Self::PrecisionInstrumentsManufacturing,
            Self::Electrolysis,
            Self::DeuteriumExtraction,
            Self::SabatierReaction,
            Self::MethanePurification,
            Self::KeroseneRefining,
            Self::UraniumEnrichment,
            Self::TritiumBreeding,
            Self::NpuAssembly,
            Self::RegolithHe3Extraction,
            Self::GasGiantHe3Separation,
            Self::MirrorSegmentAssembly,
            Self::CollectorStationAssembly,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::MetalSmelting => "Metal Smelting",
            Self::AlloyForging => "Alloy Forging",
            Self::ElectronicsManufacturing => "Electronics Manufacturing",
            Self::SuperconductorFabrication => "Superconductor Fabrication",
            Self::PrecisionInstrumentsManufacturing => "Precision Instruments Mfg",
            Self::Electrolysis => "Electrolysis",
            Self::DeuteriumExtraction => "Deuterium Extraction",
            Self::SabatierReaction => "Sabatier Reaction",
            Self::MethanePurification => "Methane Purification",
            Self::KeroseneRefining => "Kerosene Refining",
            Self::UraniumEnrichment => "Uranium Enrichment",
            Self::TritiumBreeding => "Tritium Breeding",
            Self::NpuAssembly => "NPU Assembly",
            Self::RegolithHe3Extraction => "Regolith He-3 Extraction",
            Self::GasGiantHe3Separation => "Gas Giant He-3 Separation",
            Self::MirrorSegmentAssembly => "Mirror Segment Assembly",
            Self::CollectorStationAssembly => "Collector Station Assembly",
        }
    }

    /// Inputs consumed per batch: Vec of (ResourceType, kg).
    pub fn inputs(&self) -> Vec<(ResourceType, f64)> {
        use ResourceType::*;
        match self {
            Self::MetalSmelting => vec![(MetalOre, 1_000.0)],
            Self::AlloyForging => vec![(MetalOre, 200.0), (StructuralMetal, 40.0)],
            Self::ElectronicsManufacturing => vec![
                (StructuralMetal, 10.0),
                (HighTempAlloys, 2.0),
                (RareEarthElements, 1.0),
            ],
            Self::SuperconductorFabrication => vec![
                (StructuralMetal, 6.0),
                (HighTempAlloys, 2.0),
                (Electronics, 1.0),
            ],
            Self::PrecisionInstrumentsManufacturing => vec![
                (Electronics, 50.0),
                (Superconductors, 10.0),
                (HighTempAlloys, 5.0),
            ],
            Self::Electrolysis => vec![(Water, 200.0)],
            Self::DeuteriumExtraction => vec![(Water, 10_000.0)],
            Self::SabatierReaction => vec![(AtmosphericCo2, 88.0), (LiquidHydrogen, 16.0)],
            Self::MethanePurification => vec![(Hydrocarbons, 200.0)],
            Self::KeroseneRefining => vec![(Hydrocarbons, 400.0)],
            Self::UraniumEnrichment => vec![(UraniumOre, 100.0)],
            Self::TritiumBreeding => vec![(LithiumOre, 20.0)],
            Self::NpuAssembly => vec![
                (StructuralMetal, 100.0),
                (HighTempAlloys, 40.0),
                (Tritium, 0.5),
            ],
            Self::RegolithHe3Extraction => vec![(Regolith, 20_000.0)],
            Self::GasGiantHe3Separation => vec![(GasGiantAtmosphere, 10_000.0)],
            Self::MirrorSegmentAssembly => vec![
                (StructuralMetal, 2_500.0),
                (HighTempAlloys, 500.0),
                (Electronics, 300.0),
                (Superconductors, 200.0),
            ],
            Self::CollectorStationAssembly => vec![
                (StructuralMetal, 2_000.0),
                (HighTempAlloys, 800.0),
                (Electronics, 1_200.0),
                (Superconductors, 700.0),
                (PrecisionInstruments, 300.0),
            ],
        }
    }

    /// Outputs produced per batch: Vec of (ResourceType, kg).
    pub fn outputs(&self) -> Vec<(ResourceType, f64)> {
        use ResourceType::*;
        match self {
            Self::MetalSmelting => vec![(StructuralMetal, 200.0)],
            Self::AlloyForging => vec![(HighTempAlloys, 30.0)],
            Self::ElectronicsManufacturing => vec![(Electronics, 8.0)],
            Self::SuperconductorFabrication => vec![(Superconductors, 2.0)],
            Self::PrecisionInstrumentsManufacturing => vec![(PrecisionInstruments, 1.0)],
            Self::Electrolysis => vec![(LiquidHydrogen, 22.0), (Lox, 178.0)],
            Self::DeuteriumExtraction => vec![
                (Deuterium, 2.0),
                (LiquidHydrogen, 1_098.0),
                (Lox, 8_900.0),
            ],
            Self::SabatierReaction => vec![(Methane, 32.0), (Water, 72.0)],
            Self::MethanePurification => vec![(Methane, 180.0)],
            Self::KeroseneRefining => vec![(Rp1, 100.0)],
            Self::UraniumEnrichment => vec![(EnrichedUranium, 1.0)],
            Self::TritiumBreeding => vec![(Tritium, 1.0)],
            Self::NpuAssembly => vec![(NuclearPulseUnits, 50.0)],
            Self::RegolithHe3Extraction => vec![(Helium3, 0.2)],
            Self::GasGiantHe3Separation => vec![(Helium3, 1.0)],
            Self::MirrorSegmentAssembly => vec![(MirrorSegment, 1.0)],
            Self::CollectorStationAssembly => vec![(CollectorStation, 1.0)],
        }
    }

    /// Power draw in kW while this recipe is running.
    pub fn power_draw_kw(&self) -> f64 {
        match self {
            Self::MetalSmelting => 150.0,
            Self::AlloyForging => 250.0,
            Self::ElectronicsManufacturing => 300.0,
            Self::SuperconductorFabrication => 500.0,
            Self::PrecisionInstrumentsManufacturing => 500.0,
            Self::Electrolysis => 50.0,
            Self::DeuteriumExtraction => 100.0,
            Self::SabatierReaction => 75.0,
            Self::MethanePurification => 30.0,
            Self::KeroseneRefining => 75.0,
            Self::UraniumEnrichment => 500.0,
            Self::TritiumBreeding => 200.0,
            Self::NpuAssembly => 500.0,
            Self::RegolithHe3Extraction => 500.0,
            Self::GasGiantHe3Separation => 200.0,
            Self::MirrorSegmentAssembly => 500.0,
            Self::CollectorStationAssembly => 500.0,
        }
    }

    /// Batch time in hours.
    pub fn batch_time_hours(&self) -> f64 {
        match self {
            Self::MetalSmelting => 12.0,
            Self::AlloyForging => 24.0,
            Self::ElectronicsManufacturing => 24.0,
            Self::SuperconductorFabrication => 48.0,
            Self::PrecisionInstrumentsManufacturing => 120.0,
            Self::Electrolysis => 8.0,
            Self::DeuteriumExtraction => 120.0,
            Self::SabatierReaction => 12.0,
            Self::MethanePurification => 4.0,
            Self::KeroseneRefining => 12.0,
            Self::UraniumEnrichment => 48.0,
            Self::TritiumBreeding => 48.0,
            Self::NpuAssembly => 240.0,
            Self::RegolithHe3Extraction => 24.0,
            Self::GasGiantHe3Separation => 24.0,
            Self::MirrorSegmentAssembly => 48.0,
            Self::CollectorStationAssembly => 48.0,
        }
    }

    /// The tech efficiency line ID that governs this recipe's throughput multiplier.
    pub fn efficiency_line_id(&self) -> &'static str {
        match self {
            Self::MetalSmelting | Self::AlloyForging => "metallurgy",
            Self::ElectronicsManufacturing | Self::SuperconductorFabrication => "electronics_mfg",
            Self::PrecisionInstrumentsManufacturing => "precision_mfg",
            Self::Electrolysis | Self::DeuteriumExtraction | Self::SabatierReaction
            | Self::MethanePurification | Self::KeroseneRefining => "chemical_processing",
            Self::UraniumEnrichment | Self::TritiumBreeding | Self::NpuAssembly => "nuclear_engineering",
            Self::RegolithHe3Extraction | Self::GasGiantHe3Separation => "isotope_extraction",
            Self::MirrorSegmentAssembly => "construction",
            Self::CollectorStationAssembly => "construction",
        }
    }

    /// Recipe ID string matching recipe_gates in tree.ron.
    pub fn recipe_id(&self) -> &'static str {
        match self {
            Self::MetalSmelting => "MetalSmelting",
            Self::AlloyForging => "AlloyForging",
            Self::ElectronicsManufacturing => "ElectronicsManufacturing",
            Self::SuperconductorFabrication => "SuperconductorFabrication",
            Self::PrecisionInstrumentsManufacturing => "PrecisionInstrumentsManufacturing",
            Self::Electrolysis => "Electrolysis",
            Self::DeuteriumExtraction => "DeuteriumExtraction",
            Self::SabatierReaction => "SabatierReaction",
            Self::MethanePurification => "MethanePurification",
            Self::KeroseneRefining => "KeroseneRefining",
            Self::UraniumEnrichment => "UraniumEnrichment",
            Self::TritiumBreeding => "TritiumBreeding",
            Self::NpuAssembly => "NpuAssembly",
            Self::RegolithHe3Extraction => "RegolithHe3Extraction",
            Self::GasGiantHe3Separation => "GasGiantHe3Separation",
            Self::MirrorSegmentAssembly => "MirrorSegmentAssembly",
            Self::CollectorStationAssembly => "CollectorStationAssembly",
        }
    }

    /// If Some, this recipe requires the given building type to be co-located on the colony.
    pub fn requires_colocation(&self) -> Option<BuildingType> {
        match self {
            Self::TritiumBreeding => Some(BuildingType::FissionReactor),
            Self::NpuAssembly => Some(BuildingType::FissionReactor),
            _ => None,
        }
    }
}

/// A single building instance within a colony.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildingInstance {
    pub building_type: BuildingType,
    /// For mines: which resource to extract. For atmospheric collectors: which atmospheric resource.
    pub assigned_resource: Option<ResourceType>,
    /// For factories: which recipe to run.
    pub assigned_recipe: Option<FactoryRecipe>,
    /// Whether the building is currently operational (powered and maintained).
    pub operational: bool,
    /// Degradation level (0.0 = pristine, 1.0 = fully degraded/non-functional).
    pub degradation: f64,
    /// Water fill level for greenhouses (kg). Scales food output linearly.
    pub water_fill: f64,
}

impl Default for BuildingInstance {
    fn default() -> Self {
        Self {
            building_type: BuildingType::Habitat,
            assigned_resource: None,
            assigned_recipe: None,
            operational: true,
            degradation: 0.0,
            water_fill: 0.0,
        }
    }
}

impl BuildingInstance {
    pub fn new(building_type: BuildingType) -> Self {
        Self {
            building_type,
            assigned_resource: None,
            assigned_recipe: None,
            operational: true,
            degradation: 0.0,
            water_fill: 0.0,
        }
    }
}

/// What a construction queue item is building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstructionTarget {
    Building(BuildingType),
    Ship {
        name: String,
        blueprint_name: String,
        blueprint: VesselBlueprint,
        dry_mass_kg: f64,
        cached_delta_v: f64,
    },
}

/// An item in the colony construction queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConstructionQueueItem {
    pub building_type: BuildingType,
    /// Resources that have been reserved from colony inventory for this construction.
    pub reserved_resources: ResourceInventory,
    /// Mass assembled so far (kg) — for the current unit in a batch.
    pub mass_assembled: f64,
    /// Total mass to assemble per unit (kg).
    pub total_mass: f64,
    /// What is being constructed. None = legacy (use building_type).
    #[serde(default)]
    pub target: Option<ConstructionTarget>,
    /// Number of buildings to construct in this batch (default 1 for save compat).
    #[serde(default = "default_one_u32")]
    pub count: u32,
    /// Number of buildings completed so far in this batch.
    #[serde(default)]
    pub completed: u32,
}

impl ConstructionQueueItem {
    /// Returns the effective construction target, falling back to Building(self.building_type)
    /// for backward compatibility with old saves.
    pub fn effective_target(&self) -> ConstructionTarget {
        self.target.clone().unwrap_or(ConstructionTarget::Building(self.building_type))
    }
}

impl Default for ConstructionQueueItem {
    fn default() -> Self {
        Self {
            building_type: BuildingType::Habitat,
            reserved_resources: ResourceInventory::default(),
            mass_assembled: 0.0,
            total_mass: 0.0,
            target: None,
            count: 1,
            completed: 0,
        }
    }
}

/// A colony on a celestial body.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Colony {
    /// Index of the body this colony is on.
    pub body_index: usize,
    pub name: String,
    pub buildings: Vec<BuildingInstance>,
    pub resources: ResourceInventory,
    pub crew: u32,
    pub food_stored: f64,
    pub power_generated: f64,
    pub power_consumed: f64,
    pub construction_queue: Vec<ConstructionQueueItem>,
    /// Whether this is an orbital station (gas giant scooping stations).
    pub is_orbital_station: bool,
    /// Cumulative science extracted by labs on this body.
    pub lab_science_extracted: f64,
    /// Cumulative time (years) that labs have been running on this colony.
    #[serde(default)]
    pub lab_elapsed_years: f64,
    /// Whether a food-depleted notification has already been sent (prevents spam).
    #[serde(default)]
    pub food_depleted_notified: bool,
    /// Fraction of habitat power demand that is met (0.0–1.0).
    #[serde(default = "default_one")]
    pub habitat_power_fraction: f64,
    /// Fraction of non-habitat building power demand that is met (0.0–1.0).
    #[serde(default = "default_one")]
    pub other_power_fraction: f64,
    /// Whether a habitat-unpowered notification has been sent (dedup flag).
    #[serde(default)]
    pub habitat_unpowered_notified: bool,
    /// Crew count when a food/power crisis began (for linear death rate).
    #[serde(default)]
    pub crew_at_crisis_start: Option<u32>,
    /// Fractional death accumulator — deaths only applied when this reaches >= 1.0.
    #[serde(default)]
    pub crew_death_accumulator: f64,
    /// Ships stored in this colony's hangar(s).
    #[serde(default)]
    pub stored_ships: Vec<StoredShip>,
    /// Next ID for stored ships at this colony.
    #[serde(default)]
    pub next_stored_ship_id: StoredShipId,
    /// Energy accumulated in the mass driver capacitor (joules).
    #[serde(default)]
    pub mass_driver_energy_j: f64,
    /// Total mirrors launched from this colony (lifetime stat).
    #[serde(default)]
    pub mirrors_launched: u64,
    /// Ships queued for mass driver launch (trade ships waiting for energy).
    #[serde(default)]
    pub mass_driver_ship_queue: Vec<MassDriverShipEntry>,
    /// Per-resource storage allocation (pinned percentages).
    #[serde(default)]
    pub storage_allocation: StorageAllocation,
    /// Actual receiver array power output (kW), computed by simulation.
    /// Accounts for laser availability and degradation.
    #[serde(default)]
    pub receiver_power_kw: f64,
    /// Total laser power available to this colony's receivers (kW), computed by simulation.
    /// Used in the power card to explain receiver saturation.
    #[serde(default)]
    pub receiver_laser_power_kw: f64,
}

fn default_one() -> f64 {
    1.0
}

fn default_one_u32() -> u32 {
    1
}

impl Default for Colony {
    fn default() -> Self {
        Self {
            body_index: 0,
            name: String::new(),
            buildings: Vec::new(),
            resources: ResourceInventory::default(),
            crew: 0,
            food_stored: 0.0,
            power_generated: 0.0,
            power_consumed: 0.0,
            construction_queue: Vec::new(),
            is_orbital_station: false,
            lab_science_extracted: 0.0,
            lab_elapsed_years: 0.0,
            food_depleted_notified: false,
            habitat_power_fraction: 1.0,
            other_power_fraction: 1.0,
            habitat_unpowered_notified: false,
            crew_at_crisis_start: None,
            crew_death_accumulator: 0.0,
            stored_ships: Vec::new(),
            next_stored_ship_id: 0,
            mass_driver_energy_j: 0.0,
            mirrors_launched: 0,
            mass_driver_ship_queue: Vec::new(),
            storage_allocation: StorageAllocation::default(),
            receiver_power_kw: 0.0,
            receiver_laser_power_kw: 0.0,
        }
    }
}

impl Colony {
    pub fn new(body_index: usize, name: String) -> Self {
        Self {
            body_index,
            name,
            buildings: Vec::new(),
            resources: ResourceInventory::new(),
            crew: 0,
            food_stored: 0.0,
            power_generated: 0.0,
            power_consumed: 0.0,
            construction_queue: Vec::new(),
            is_orbital_station: false,
            lab_science_extracted: 0.0,
            lab_elapsed_years: 0.0,
            food_depleted_notified: false,
            habitat_power_fraction: 1.0,
            other_power_fraction: 1.0,
            habitat_unpowered_notified: false,
            crew_at_crisis_start: None,
            crew_death_accumulator: 0.0,
            stored_ships: Vec::new(),
            next_stored_ship_id: 0,
            mass_driver_energy_j: 0.0,
            mirrors_launched: 0,
            mass_driver_ship_queue: Vec::new(),
            storage_allocation: StorageAllocation::default(),
            receiver_power_kw: 0.0,
            receiver_laser_power_kw: 0.0,
        }
    }

    /// Find the best (highest tier) operational mass driver in this colony.
    pub fn best_mass_driver(&self) -> Option<BuildingType> {
        let mut best: Option<BuildingType> = None;
        for b in &self.buildings {
            if !b.operational || !b.building_type.is_mass_driver() {
                continue;
            }
            let track = b.building_type.mass_driver_track_m().unwrap_or(0.0);
            if best.map_or(true, |bt| bt.mass_driver_track_m().unwrap_or(0.0) < track) {
                best = Some(b.building_type);
            }
        }
        best
    }

    /// Whether this colony has any operational mass driver.
    pub fn has_mass_driver(&self) -> bool {
        self.buildings.iter().any(|b| b.operational && b.building_type.is_mass_driver())
    }

    /// Check whether the colony can queue a building (has enough resources).
    /// `body_radius_m` is used to scale size-dependent buildings (Mk IV accelerator).
    pub fn can_queue_building(&self, bt: BuildingType, hab_score: u32, body_radius_m: f64) -> bool {
        let costs = bt.build_cost();
        let hab = if bt.affected_by_habitability() {
            crate::colony::simulation::habitability_multiplier(hab_score)
        } else {
            1.0
        };
        let mult = hab * bt.size_multiplier(body_radius_m);
        costs.iter().all(|&(res, amount)| {
            self.resources.get(res) >= amount * mult
        })
    }

    /// Queue a building for construction, consuming resources from inventory.
    /// `body_radius_m` is used to scale size-dependent buildings (Mk IV accelerator).
    /// `count` is the batch size: 0 means "all affordable".
    pub fn queue_building(&mut self, bt: BuildingType, hab_score: u32, body_radius_m: f64, count: u32) -> Result<(), String> {
        let costs = bt.build_cost();
        let hab = if bt.affected_by_habitability() {
            crate::colony::simulation::habitability_multiplier(hab_score)
        } else {
            1.0
        };
        let mult = hab * bt.size_multiplier(body_radius_m);

        // Determine how many we can afford
        let mut max_affordable = u32::MAX;
        for &(res, amount) in &costs {
            let per_unit = amount * mult;
            if per_unit > 0.0 {
                let affordable = (self.resources.get(res) / per_unit) as u32;
                max_affordable = max_affordable.min(affordable);
            }
        }

        let actual_count = if count == 0 {
            max_affordable
        } else {
            count.min(max_affordable)
        };

        if actual_count == 0 {
            // Find which resource is insufficient for the error message
            for &(res, amount) in &costs {
                let needed = amount * mult;
                if self.resources.get(res) < needed {
                    return Err(format!("Not enough {}", res.display_name()));
                }
            }
            return Err("Not enough resources".to_string());
        }

        // Consume resources for the full batch
        let mut reserved = ResourceInventory::new();
        for &(res, amount) in &costs {
            let needed = amount * mult * actual_count as f64;
            self.resources.remove(res, needed);
            reserved.add(res, needed);
        }

        let unit_mass = bt.total_build_mass() * mult;

        // Merge into existing queue item of the same building type if present
        if let Some(existing) = self.construction_queue.iter_mut().find(|item| {
            matches!(item.effective_target(), ConstructionTarget::Building(existing_bt) if existing_bt == bt)
        }) {
            existing.count += actual_count;
            for (res, amount) in reserved.iter() {
                existing.reserved_resources.add(*res, *amount);
            }
        } else {
            self.construction_queue.push(ConstructionQueueItem {
                building_type: bt,
                reserved_resources: reserved,
                mass_assembled: 0.0,
                total_mass: unit_mass,
                target: None,
                count: actual_count,
                completed: 0,
            });
        }

        Ok(())
    }

    /// Days of food remaining at current crew level.
    pub fn food_days_remaining(&self) -> f64 {
        if self.crew == 0 {
            return f64::INFINITY;
        }
        self.food_stored / (0.5 * self.crew as f64)
    }

    /// Total hangar capacity in kg (200,000 kg per operational Hangar building).
    pub fn hangar_capacity(&self) -> f64 {
        self.buildings
            .iter()
            .filter(|b| b.building_type == BuildingType::Hangar && b.operational)
            .count() as f64
            * 200_000.0
    }

    /// Total mass of ships currently stored in hangars (kg).
    pub fn hangar_used(&self) -> f64 {
        self.stored_ships.iter().map(|s| s.dry_mass_kg).sum()
    }

    /// Whether the colony has at least one operational Hangar building.
    pub fn has_hangar(&self) -> bool {
        self.buildings
            .iter()
            .any(|b| b.building_type == BuildingType::Hangar && b.operational)
    }

    /// Store a ship in the colony hangar. Assigns a new ID. Returns Err if insufficient capacity.
    pub fn store_ship(&mut self, mut ship: StoredShip) -> Result<(), String> {
        let remaining = self.hangar_capacity() - self.hangar_used();
        if ship.dry_mass_kg > remaining + 1e-3 {
            return Err("Insufficient hangar capacity".to_string());
        }
        ship.id = self.next_stored_ship_id;
        self.next_stored_ship_id += 1;
        self.stored_ships.push(ship);
        Ok(())
    }

    /// Remove a ship from the colony hangar by ID. Returns the ship if found.
    pub fn remove_ship(&mut self, id: StoredShipId) -> Option<StoredShip> {
        let idx = self.stored_ships.iter().position(|s| s.id == id)?;
        Some(self.stored_ships.remove(idx))
    }

    /// Find a stored ship matching a blueprint name. Returns its ID if found.
    pub fn find_matching_ship(&self, blueprint_name: &str) -> Option<StoredShipId> {
        self.stored_ships
            .iter()
            .find(|s| s.blueprint_name.as_deref() == Some(blueprint_name))
            .map(|s| s.id)
    }

    /// Scrap a ship, converting its dry mass to resources via material breakdown.
    /// Returns the resources recovered (for display).
    pub fn scrap_ship(
        &mut self,
        id: StoredShipId,
        part_defs: &crate::parts::PartDefinitions,
    ) -> Option<Vec<(ResourceType, f64)>> {
        let ship = self.remove_ship(id)?;
        let costs =
            crate::colony::economy::blueprint_material_costs(&ship.blueprint, part_defs);
        for &(res, amount) in &costs {
            self.resources.add(res, amount);
        }
        Some(costs)
    }

    /// Queue ship construction. Validates and consumes material resources upfront.
    pub fn queue_ship_construction(
        &mut self,
        name: String,
        blueprint_name: String,
        blueprint: &VesselBlueprint,
        part_defs: &crate::parts::PartDefinitions,
    ) -> Result<(), String> {
        let costs = crate::colony::economy::blueprint_material_costs(blueprint, part_defs);

        // Check resources
        for &(res, amount) in &costs {
            if self.resources.get(res) < amount {
                return Err(format!("Not enough {} (need {:.0} kg)", res.display_name(), amount));
            }
        }

        // Consume resources
        let mut reserved = ResourceInventory::new();
        for &(res, amount) in &costs {
            self.resources.remove(res, amount);
            reserved.add(res, amount);
        }

        let dry_mass_kg = crate::colony::economy::blueprint_dry_mass_kg(blueprint, part_defs);
        let cached_delta_v = crate::colony::transfer::blueprint_total_delta_v(blueprint, part_defs);

        self.construction_queue.push(ConstructionQueueItem {
            building_type: BuildingType::Habitat, // Placeholder for backward compat
            reserved_resources: reserved,
            mass_assembled: 0.0,
            total_mass: dry_mass_kg,
            target: Some(ConstructionTarget::Ship {
                name,
                blueprint_name,
                blueprint: blueprint.clone(),
                dry_mass_kg,
                cached_delta_v,
            }),
            count: 1,
            completed: 0,
        });

        Ok(())
    }
}

/// Manages all colonies in the game.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ColonyManager {
    pub colonies: Vec<Colony>,
}

impl ColonyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_by_body(&self, body_index: usize) -> Option<&Colony> {
        self.colonies.iter().find(|c| c.body_index == body_index)
    }

    pub fn get_by_body_mut(&mut self, body_index: usize) -> Option<&mut Colony> {
        self.colonies.iter_mut().find(|c| c.body_index == body_index)
    }

    pub fn has_colony(&self, body_index: usize) -> bool {
        self.colonies.iter().any(|c| c.body_index == body_index)
    }

    pub fn add_colony(&mut self, colony: Colony) {
        self.colonies.push(colony);
    }
}
