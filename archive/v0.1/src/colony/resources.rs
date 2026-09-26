use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All resource types in the game economy.
/// Raw resources are mined or collected; processed resources are manufactured in factories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    // Raw resources
    MetalOre,
    Regolith,
    Water,
    LithiumOre,
    Hydrocarbons,
    AtmosphericCo2,
    GasGiantAtmosphere,
    RareEarthElements,
    UraniumOre,
    // Processed resources
    StructuralMetal,
    HighTempAlloys,
    Electronics,
    Superconductors,
    PrecisionInstruments,
    // Fuels
    Rp1,
    Methane,
    LiquidHydrogen,
    Lox,
    Xenon,
    Deuterium,
    Tritium,
    EnrichedUranium,
    NuclearPulseUnits,
    Helium3,
    Antimatter,
    // Manufactured
    MirrorSegment,
    CollectorStation,
    // Consumables
    Food,
}

impl ResourceType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::MetalOre => "Metal Ore",
            Self::Regolith => "Regolith",
            Self::Water => "Water",
            Self::LithiumOre => "Lithium Ore",
            Self::Hydrocarbons => "Hydrocarbons",
            Self::AtmosphericCo2 => "Atmospheric CO₂",
            Self::GasGiantAtmosphere => "Gas Giant Atmosphere",
            Self::RareEarthElements => "Rare Earth Elements",
            Self::UraniumOre => "Uranium Ore",
            Self::StructuralMetal => "Structural Metal",
            Self::HighTempAlloys => "High-Temp Alloys",
            Self::Electronics => "Electronics",
            Self::Superconductors => "Superconductors",
            Self::PrecisionInstruments => "Precision Instruments",
            Self::Rp1 => "RP-1",
            Self::Methane => "Methane",
            Self::LiquidHydrogen => "Liquid Hydrogen",
            Self::Lox => "LOX",
            Self::Xenon => "Xenon",
            Self::Deuterium => "Deuterium",
            Self::Tritium => "Tritium",
            Self::EnrichedUranium => "Enriched Uranium",
            Self::NuclearPulseUnits => "Nuclear Pulse Units",
            Self::Helium3 => "Helium-3",
            Self::Antimatter => "Antimatter",
            Self::MirrorSegment => "Mirror Segment",
            Self::CollectorStation => "Collector Station",
            Self::Food => "Food",
        }
    }

    /// All resource types.
    pub fn all() -> &'static [ResourceType] {
        &[
            Self::MetalOre, Self::Regolith, Self::Water, Self::LithiumOre,
            Self::Hydrocarbons, Self::AtmosphericCo2, Self::GasGiantAtmosphere,
            Self::RareEarthElements, Self::UraniumOre,
            Self::StructuralMetal, Self::HighTempAlloys, Self::Electronics,
            Self::Superconductors, Self::PrecisionInstruments,
            Self::Rp1, Self::Methane, Self::LiquidHydrogen, Self::Lox,
            Self::Xenon, Self::Deuterium, Self::Tritium, Self::EnrichedUranium,
            Self::NuclearPulseUnits, Self::Helium3, Self::Antimatter,
            Self::MirrorSegment,
            Self::CollectorStation,
            Self::Food,
        ]
    }

    /// Returns true for resources that are fully refined ship fuels
    /// (loaded into fuel tanks, not cargo containers).
    pub fn is_ship_fuel(&self) -> bool {
        matches!(
            self,
            Self::Rp1
                | Self::Methane
                | Self::LiquidHydrogen
                | Self::Lox
                | Self::Xenon
                | Self::Helium3
                | Self::Antimatter
        )
    }

    /// Look up a ResourceType by its display name.
    pub fn from_display_name(name: &str) -> Option<ResourceType> {
        Self::all().iter().find(|rt| rt.display_name() == name).copied()
    }

    /// Mass in kg per inventory unit. Most resources are stored in kg (returns 1.0).
    /// MirrorSegment and CollectorStation are stored as unit counts.
    pub fn storage_mass_per_unit(&self, sail_tier: u32) -> f64 {
        match self {
            Self::MirrorSegment => crate::colony::dyson_swarm::mirror_mass_at_tier(sail_tier),
            Self::CollectorStation => crate::colony::dyson_swarm::COLLECTOR_MASS_KG,
            _ => 1.0,
        }
    }

    /// Earth purchase price in $/kg. None if the resource cannot be purchased on Earth.
    pub fn earth_price(&self) -> Option<f64> {
        match self {
            Self::MetalOre => Some(5.0),
            Self::Regolith => None,
            Self::Water => Some(2.0),
            Self::LithiumOre => Some(50.0),
            Self::Hydrocarbons => Some(1.0),
            Self::AtmosphericCo2 => None,
            Self::GasGiantAtmosphere => None,
            Self::RareEarthElements => Some(500.0),
            Self::UraniumOre => Some(100.0),
            Self::StructuralMetal => Some(100.0),
            Self::HighTempAlloys => Some(1_000.0),
            Self::Electronics => Some(10_000.0),
            Self::Superconductors => Some(50_000.0),
            Self::PrecisionInstruments => Some(200_000.0),
            Self::Rp1 => Some(1.0),
            Self::Methane => Some(2.0),
            Self::LiquidHydrogen => Some(6.0),
            Self::Lox => Some(0.50),
            Self::Xenon => Some(3_000.0),
            Self::Deuterium => Some(20.0),
            Self::Tritium => Some(30_000.0),
            Self::EnrichedUranium => Some(15_000.0),
            Self::NuclearPulseUnits => Some(100_000.0),
            Self::Helium3 => None,
            Self::Antimatter => None,
            Self::MirrorSegment => None,
            Self::CollectorStation => None,
            Self::Food => Some(50.0),
        }
    }
}

/// A collection of resources tracked by mass (kg).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceInventory {
    #[serde(default)]
    resources: HashMap<ResourceType, f64>,
}

impl ResourceInventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, resource: ResourceType) -> f64 {
        self.resources.get(&resource).copied().unwrap_or(0.0)
    }

    pub fn set(&mut self, resource: ResourceType, amount: f64) {
        if amount <= 0.0 {
            self.resources.remove(&resource);
        } else {
            self.resources.insert(resource, amount);
        }
    }

    pub fn add(&mut self, resource: ResourceType, amount: f64) {
        if amount <= 0.0 {
            return;
        }
        let entry = self.resources.entry(resource).or_insert(0.0);
        *entry += amount;
    }

    /// Remove up to `amount` kg of a resource. Returns false if insufficient.
    pub fn remove(&mut self, resource: ResourceType, amount: f64) -> bool {
        if amount <= 0.0 {
            return true;
        }
        let current = self.get(resource);
        if current < amount - 1e-9 {
            return false;
        }
        self.set(resource, (current - amount).max(0.0));
        true
    }

    pub fn has_enough(&self, resource: ResourceType, amount: f64) -> bool {
        self.get(resource) >= amount - 1e-9
    }

    pub fn has_enough_all(&self, requirements: &[(ResourceType, f64)]) -> bool {
        requirements.iter().all(|&(r, amt)| self.has_enough(r, amt))
    }

    /// Atomically remove all listed resources. Returns false (and removes nothing) if any is insufficient.
    pub fn remove_all(&mut self, requirements: &[(ResourceType, f64)]) -> bool {
        if !self.has_enough_all(requirements) {
            return false;
        }
        for &(r, amt) in requirements {
            self.remove(r, amt);
        }
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ResourceType, &f64)> {
        self.resources.iter()
    }

    pub fn total_mass(&self) -> f64 {
        self.resources.values().sum()
    }

    /// Total storage mass in kg, accounting for unit-counted resources
    /// (MirrorSegment, CollectorStation) whose inventory values are counts, not kg.
    pub fn total_storage_mass(&self, sail_tier: u32) -> f64 {
        self.resources
            .iter()
            .map(|(rt, &amt)| amt * rt.storage_mass_per_unit(sail_tier))
            .sum()
    }
}

/// Per-resource storage allocation within a colony's total stockpile capacity.
///
/// Resources can be "pinned" to a fixed percentage of total storage, or left as
/// "auto" to share the remainder equally. Food is excluded (has its own capacity).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageAllocation {
    /// Manually pinned percentages (resource -> percent of total storage, 0–100).
    pinned: HashMap<ResourceType, f64>,
}

impl Default for StorageAllocation {
    fn default() -> Self {
        Self {
            pinned: HashMap::new(),
        }
    }
}

impl StorageAllocation {
    /// Returns the max kg this resource can occupy in storage.
    pub fn capacity_for(
        &self,
        resource: ResourceType,
        total_capacity: f64,
        active_resources: &[ResourceType],
    ) -> f64 {
        let pct = self.effective_pct(resource, active_resources);
        total_capacity * pct / 100.0
    }

    /// Pin a resource at a specific percentage of total storage.
    /// Clamps to [0, 100 - other_pinned_total].
    pub fn set_pinned(&mut self, resource: ResourceType, percent: f64) {
        let other_pinned: f64 = self
            .pinned
            .iter()
            .filter(|(&r, _)| r != resource)
            .map(|(_, &p)| p)
            .sum();
        let clamped = percent.clamp(0.0, (100.0 - other_pinned).max(0.0));
        if clamped < 0.01 {
            self.pinned.remove(&resource);
        } else {
            self.pinned.insert(resource, clamped);
        }
    }

    /// Remove manual override, return to auto allocation.
    pub fn unpin(&mut self, resource: ResourceType) {
        self.pinned.remove(&resource);
    }

    /// Whether a resource is pinned.
    pub fn is_pinned(&self, resource: ResourceType) -> bool {
        self.pinned.contains_key(&resource)
    }

    /// Compute the effective percentage for a single resource.
    fn effective_pct(&self, resource: ResourceType, active_resources: &[ResourceType]) -> f64 {
        if let Some(&pct) = self.pinned.get(&resource) {
            return pct;
        }
        // Auto-allocated: split remainder equally among non-pinned active resources
        let pinned_total: f64 = self.pinned.values().sum();
        let remainder = (100.0 - pinned_total).max(0.0);
        let auto_count = active_resources
            .iter()
            .filter(|r| !self.pinned.contains_key(r))
            .count();
        if auto_count == 0 {
            return 0.0;
        }
        // Only give share if this resource is in the active set
        if active_resources.contains(&resource) {
            remainder / auto_count as f64
        } else {
            0.0
        }
    }

    /// Compute all effective percentages for display.
    pub fn effective_pcts(
        &self,
        active_resources: &[ResourceType],
    ) -> HashMap<ResourceType, f64> {
        let mut result = HashMap::new();
        for &res in active_resources {
            result.insert(res, self.effective_pct(res, active_resources));
        }
        result
    }
}

/// Compute the set of "active" resources for storage allocation.
/// A resource is active if it has stock > 0 OR has production > 0.
/// Food is excluded (has its own capacity system).
pub fn compute_active_resources(
    resources: &ResourceInventory,
    production: &HashMap<ResourceType, f64>,
) -> Vec<ResourceType> {
    let mut active = Vec::new();
    for &rt in ResourceType::all() {
        if rt == ResourceType::Food {
            continue;
        }
        let has_stock = resources.get(rt) > 0.001;
        let has_production = production.get(&rt).copied().unwrap_or(0.0) > 0.001;
        if has_stock || has_production {
            active.push(rt);
        }
    }
    active
}

/// The player's company — manages money and R&D spending.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Company {
    /// Current cash balance in dollars
    pub money: f64,
    /// Annual R&D budget in dollars/year
    pub rd_budget: f64,
}

impl Default for Company {
    fn default() -> Self {
        Self {
            money: 25_000_000.0,
            rd_budget: 1_000_000.0,
        }
    }
}
