use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::bodies::SolarSystem;
use crate::colony::tech::DiscoveryTracker;
use crate::parts::FlightPart;
use crate::ship::ShipState;

const POOL_SIZE: usize = 5;

const SUBORBITAL_TEST_PARTS: &[(&str, &str)] = &[
    ("probe_tiny", "Tiny Probe Core"),
    ("engine_hummingbird", "Hummingbird"),
    ("engine_gecko", "Gecko"),
    ("engine_firefly", "Firefly"),
    ("fairing_tiny", "AE-FF0 Fairing"),
    ("heatshield_tiny", "HS-1 Heat Shield"),
];

const SUBORBITAL_TEST_ALTITUDES: &[f64] = &[100_000.0, 200_000.0, 500_000.0];

/// Destinations for payload delivery and tourism contracts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Destination {
    Suborbital,
    LowEarthOrbit,
    LunarOrbit,
    LunarSurface,
    MarsOrbit,
    MarsSurface,
}

impl Destination {
    pub fn display_name(&self) -> &'static str {
        match self {
            Destination::Suborbital => "Suborbital",
            Destination::LowEarthOrbit => "Low Earth Orbit",
            Destination::LunarOrbit => "Lunar Orbit",
            Destination::LunarSurface => "Lunar Surface",
            Destination::MarsOrbit => "Mars Orbit",
            Destination::MarsSurface => "Mars Surface",
        }
    }

    /// Price per kg for payload delivery to this destination.
    pub fn price_per_kg(&self) -> f64 {
        match self {
            Destination::Suborbital => 3_000.0,
            Destination::LowEarthOrbit => 3_000.0,
            Destination::LunarOrbit => 20_000.0,
            Destination::LunarSurface => 50_000.0,
            Destination::MarsOrbit => 80_000.0,
            Destination::MarsSurface => 100_000.0,
        }
    }

    /// Price per seat for tourism (only valid for Suborbital, LEO, Lunar).
    pub fn tourism_price_per_seat(&self) -> Option<f64> {
        match self {
            Destination::Suborbital => Some(750_000.0),
            Destination::LowEarthOrbit => Some(60_000_000.0),
            Destination::LunarOrbit => Some(125_000_000.0),
            _ => None,
        }
    }

    /// Whether the objective condition is met for this destination.
    pub fn is_met(
        &self,
        ship_state: &ShipState,
        soi_body: usize,
        altitude: f64,
        is_suborbital: bool,
        solar_system: &SolarSystem,
    ) -> bool {
        let earth_idx = solar_system.earth_index;
        let moon_idx = solar_system.moon_index;
        let mars_idx = solar_system.bodies.iter()
            .position(|b| b.name == "Mars")
            .unwrap_or(usize::MAX);

        match self {
            Destination::Suborbital => {
                soi_body == earth_idx
                    && altitude > 100_000.0
                    && matches!(ship_state, ShipState::Flying)
            }
            Destination::LowEarthOrbit => {
                soi_body == earth_idx
                    && !is_suborbital
                    && matches!(ship_state, ShipState::Flying)
            }
            Destination::LunarOrbit => {
                soi_body == moon_idx
                    && !is_suborbital
                    && matches!(ship_state, ShipState::Flying)
            }
            Destination::LunarSurface => {
                matches!(ship_state, ShipState::Landed { body_index, .. } if *body_index == moon_idx)
            }
            Destination::MarsOrbit => {
                soi_body == mars_idx
                    && !is_suborbital
                    && matches!(ship_state, ShipState::Flying)
            }
            Destination::MarsSurface => {
                matches!(ship_state, ShipState::Landed { body_index, .. } if *body_index == mars_idx)
            }
        }
    }

    /// Whether this destination is unlocked based on discovery progress.
    pub fn is_unlocked(&self, discoveries: &DiscoveryTracker, solar_system: &SolarSystem) -> bool {
        let moon_idx = solar_system.moon_index;
        let mars_idx = solar_system.bodies.iter()
            .position(|b| b.name == "Mars")
            .unwrap_or(usize::MAX);

        match self {
            Destination::Suborbital => true,
            Destination::LowEarthOrbit => discoveries.first_suborbital,
            Destination::LunarOrbit => discoveries.first_orbit,
            Destination::LunarSurface => discoveries.body_orbited.contains(&moon_idx),
            Destination::MarsOrbit => discoveries.body_landed.contains(&moon_idx),
            Destination::MarsSurface => discoveries.body_orbited.contains(&mars_idx),
        }
    }

    /// All destination variants (for iteration).
    pub fn all() -> &'static [Destination] {
        &[
            Destination::Suborbital,
            Destination::LowEarthOrbit,
            Destination::LunarOrbit,
            Destination::LunarSurface,
            Destination::MarsOrbit,
            Destination::MarsSurface,
        ]
    }
}

/// What kind of contract this is.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContractKind {
    Payload {
        mass_kg: f64,
        destination: Destination,
    },
    Tourism {
        passengers: u32,
        destination: Destination,
        /// Set to true when the ship reaches the destination during flight.
        destination_reached: bool,
    },
    SuborbitalTest {
        part_id: String,
        part_name: String,
        target_altitude: f64,
    },
}

/// A single contract instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Contract {
    pub id: u64,
    pub kind: ContractKind,
    pub payout: f64,
    pub name: String,
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            id: 0,
            kind: ContractKind::Payload { mass_kg: 0.0, destination: Destination::Suborbital },
            payout: 0.0,
            name: String::new(),
        }
    }
}

impl Contract {
    pub fn destination(&self) -> Destination {
        match &self.kind {
            ContractKind::Payload { destination, .. } => *destination,
            ContractKind::Tourism { destination, .. } => *destination,
            ContractKind::SuborbitalTest { .. } => Destination::Suborbital,
        }
    }

    pub fn description(&self) -> String {
        match &self.kind {
            ContractKind::Payload { mass_kg, destination } => {
                format!("Deliver {:.0} kg to {}", mass_kg, destination.display_name())
            }
            ContractKind::Tourism { passengers, destination, .. } => {
                let dest_name = destination.display_name();
                if *passengers == 1 {
                    format!("Fly 1 tourist to {} and return safely", dest_name)
                } else {
                    format!("Fly {} tourists to {} and return safely", passengers, dest_name)
                }
            }
            ContractKind::SuborbitalTest { part_name, target_altitude, .. } => {
                format!("Carry a {} above {:.0} km altitude", part_name, target_altitude / 1000.0)
            }
        }
    }
}

/// A payload object placed into a cargo container for a contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ContractPayload {
    pub contract_id: u64,
    pub name: String,
    pub mass_kg: f64,
}

impl Default for ContractPayload {
    fn default() -> Self {
        Self {
            contract_id: 0,
            name: String::new(),
            mass_kg: 0.0,
        }
    }
}

/// One-time government milestones, checked automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GovernmentMilestone {
    FirstSuborbital,
    FirstOrbital,
    FirstCrewedOrbital,
    FirstLunarOrbit,
    FirstLunarLanding,
    FirstCrewedLunar,
    FirstMarsOrbit,
    FirstMarsLanding,
    FirstCrewedMars,
}

impl GovernmentMilestone {
    pub fn payout(&self) -> f64 {
        match self {
            GovernmentMilestone::FirstSuborbital => 5_000_000.0,
            GovernmentMilestone::FirstOrbital => 20_000_000.0,
            GovernmentMilestone::FirstCrewedOrbital => 50_000_000.0,
            GovernmentMilestone::FirstLunarOrbit => 100_000_000.0,
            GovernmentMilestone::FirstLunarLanding => 200_000_000.0,
            GovernmentMilestone::FirstCrewedLunar => 500_000_000.0,
            GovernmentMilestone::FirstMarsOrbit => 300_000_000.0,
            GovernmentMilestone::FirstMarsLanding => 500_000_000.0,
            GovernmentMilestone::FirstCrewedMars => 1_000_000_000.0,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GovernmentMilestone::FirstSuborbital => "First Suborbital Flight",
            GovernmentMilestone::FirstOrbital => "First Orbital Flight",
            GovernmentMilestone::FirstCrewedOrbital => "First Crewed Orbital Flight",
            GovernmentMilestone::FirstLunarOrbit => "First Lunar Orbit",
            GovernmentMilestone::FirstLunarLanding => "First Lunar Landing",
            GovernmentMilestone::FirstCrewedLunar => "First Crewed Lunar Mission",
            GovernmentMilestone::FirstMarsOrbit => "First Mars Orbit",
            GovernmentMilestone::FirstMarsLanding => "First Mars Landing",
            GovernmentMilestone::FirstCrewedMars => "First Crewed Mars Mission",
        }
    }

    pub fn all() -> &'static [GovernmentMilestone] {
        &[
            GovernmentMilestone::FirstSuborbital,
            GovernmentMilestone::FirstOrbital,
            GovernmentMilestone::FirstCrewedOrbital,
            GovernmentMilestone::FirstLunarOrbit,
            GovernmentMilestone::FirstLunarLanding,
            GovernmentMilestone::FirstCrewedLunar,
            GovernmentMilestone::FirstMarsOrbit,
            GovernmentMilestone::FirstMarsLanding,
            GovernmentMilestone::FirstCrewedMars,
        ]
    }
}

/// Manages contract pool, active contracts, milestones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractManager {
    /// Pool of contracts available for the player to accept.
    #[serde(default)]
    pub available: Vec<Contract>,
    /// Contracts the player has accepted and is working on.
    #[serde(default)]
    pub active: Vec<Contract>,
    /// IDs of completed contracts (prevents re-accept).
    #[serde(default)]
    pub completed_ids: Vec<u64>,
    /// How many completions per destination (for size scaling).
    #[serde(default)]
    pub completions_by_dest: HashMap<Destination, u32>,
    /// Government milestones already awarded.
    #[serde(default)]
    pub awarded_milestones: HashSet<GovernmentMilestone>,
    /// Next unique contract ID.
    #[serde(default = "default_next_id")]
    pub next_id: u64,
    /// Whether the contract board window is open (editor UI).
    #[serde(default)]
    pub show_board: bool,
}

fn default_next_id() -> u64 { 1 }

impl Default for ContractManager {
    fn default() -> Self {
        Self {
            available: Vec::new(),
            active: Vec::new(),
            completed_ids: Vec::new(),
            completions_by_dest: HashMap::new(),
            awarded_milestones: HashSet::new(),
            next_id: 1,
            show_board: false,
        }
    }
}

impl ContractManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Accept a contract from the available pool.
    pub fn accept(&mut self, contract_id: u64) {
        if let Some(pos) = self.available.iter().position(|c| c.id == contract_id) {
            let contract = self.available.remove(pos);
            self.active.push(contract);
        }
    }

    /// Cancel an active contract. Returns payloads that should be removed from cargo.
    pub fn cancel(&mut self, contract_id: u64) -> Vec<u64> {
        let mut removed_ids = Vec::new();
        if let Some(pos) = self.active.iter().position(|c| c.id == contract_id) {
            removed_ids.push(contract_id);
            self.active.remove(pos);
        }
        removed_ids
    }

    /// Complete a contract: remove from active, record completion, return payout.
    fn complete_contract(&mut self, contract_id: u64) -> f64 {
        if let Some(pos) = self.active.iter().position(|c| c.id == contract_id) {
            let contract = self.active.remove(pos);
            let dest = contract.destination();
            let payout = contract.payout;
            self.completed_ids.push(contract.id);
            *self.completions_by_dest.entry(dest).or_insert(0) += 1;
            payout
        } else {
            0.0
        }
    }

    /// Check payload contracts against current vessel state.
    /// Returns list of (contract_name, payout) for completed contracts.
    pub fn check_payload_contracts(
        &mut self,
        vessel_payloads: &[ContractPayload],
        ship_state: &ShipState,
        soi_body: usize,
        altitude: f64,
        is_suborbital: bool,
        solar_system: &SolarSystem,
    ) -> Vec<(String, f64)> {
        let mut completed = Vec::new();

        // Find active payload contracts whose destination is met and whose payload is on the vessel
        let contract_ids_to_complete: Vec<u64> = self.active.iter()
            .filter_map(|c| {
                if let ContractKind::Payload { destination, .. } = &c.kind {
                    // Check if vessel has a payload for this contract
                    let has_payload = vessel_payloads.iter().any(|p| p.contract_id == c.id);
                    if has_payload && destination.is_met(ship_state, soi_body, altitude, is_suborbital, solar_system) {
                        return Some(c.id);
                    }
                }
                None
            })
            .collect();

        for id in contract_ids_to_complete {
            let name = self.active.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default();
            let payout = self.complete_contract(id);
            if payout > 0.0 {
                completed.push((name, payout));
            }
        }

        completed
    }

    /// Check suborbital test flight contracts against current vessel state.
    /// Returns list of (contract_name, payout) for completed contracts.
    pub fn check_suborbital_test_contracts(
        &mut self,
        parts: &[FlightPart],
        ship_state: &ShipState,
        soi_body: usize,
        altitude: f64,
        solar_system: &SolarSystem,
    ) -> Vec<(String, f64)> {
        let earth_idx = solar_system.earth_index;

        // Must be flying in Earth SOI
        if soi_body != earth_idx || !matches!(ship_state, ShipState::Flying) {
            return Vec::new();
        }

        let mut completed = Vec::new();

        let ids_to_complete: Vec<u64> = self.active.iter()
            .filter_map(|c| {
                if let ContractKind::SuborbitalTest { part_id, target_altitude, .. } = &c.kind {
                    if altitude >= *target_altitude {
                        // Check vessel has a matching non-decoupled, non-destroyed part
                        let has_part = parts.iter().any(|p| {
                            !p.decoupled && !p.destroyed && p.definition_id == *part_id
                        });
                        if has_part {
                            return Some(c.id);
                        }
                    }
                }
                None
            })
            .collect();

        for id in ids_to_complete {
            let name = self.active.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default();
            let payout = self.complete_contract(id);
            if payout > 0.0 {
                completed.push((name, payout));
            }
        }

        completed
    }

    /// Update tourism destination_reached flags based on current ship state.
    pub fn check_tourism_destinations(
        &mut self,
        ship_state: &ShipState,
        soi_body: usize,
        altitude: f64,
        is_suborbital: bool,
        has_crew: bool,
        solar_system: &SolarSystem,
    ) {
        if !has_crew {
            return;
        }
        for contract in &mut self.active {
            if let ContractKind::Tourism { destination, destination_reached, .. } = &mut contract.kind {
                if !*destination_reached && destination.is_met(ship_state, soi_body, altitude, is_suborbital, solar_system) {
                    *destination_reached = true;
                    log::info!("Tourism contract '{}': destination reached!", contract.name);
                }
            }
        }
    }

    /// On vessel recovery, complete all tourism contracts with destination_reached == true.
    /// Returns list of (contract_name, payout) for completed contracts.
    pub fn check_tourism_recovery(&mut self) -> Vec<(String, f64)> {
        let mut completed = Vec::new();

        let ids_to_complete: Vec<u64> = self.active.iter()
            .filter_map(|c| {
                if let ContractKind::Tourism { destination_reached, .. } = &c.kind {
                    if *destination_reached {
                        return Some(c.id);
                    }
                }
                None
            })
            .collect();

        for id in ids_to_complete {
            let name = self.active.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default();
            let payout = self.complete_contract(id);
            if payout > 0.0 {
                completed.push((name, payout));
            }
        }

        completed
    }

    /// Check government milestones and return newly awarded ones.
    pub fn check_milestones(
        &mut self,
        discoveries: &DiscoveryTracker,
        has_crew: bool,
        solar_system: &SolarSystem,
    ) -> Vec<(GovernmentMilestone, f64)> {
        let moon_idx = solar_system.moon_index;
        let mars_idx = solar_system.bodies.iter()
            .position(|b| b.name == "Mars")
            .unwrap_or(usize::MAX);

        let mut awarded = Vec::new();

        let checks: &[(GovernmentMilestone, bool)] = &[
            (GovernmentMilestone::FirstSuborbital, discoveries.first_suborbital),
            (GovernmentMilestone::FirstOrbital, discoveries.first_orbit),
            (GovernmentMilestone::FirstCrewedOrbital, has_crew && discoveries.first_crewed_orbit),
            (GovernmentMilestone::FirstLunarOrbit, discoveries.body_orbited.contains(&moon_idx)),
            (GovernmentMilestone::FirstLunarLanding, discoveries.body_landed.contains(&moon_idx)),
            (GovernmentMilestone::FirstCrewedLunar, has_crew && discoveries.first_crewed_lunar),
            (GovernmentMilestone::FirstMarsOrbit, discoveries.body_orbited.contains(&mars_idx)),
            (GovernmentMilestone::FirstMarsLanding, discoveries.body_landed.contains(&mars_idx)),
            (GovernmentMilestone::FirstCrewedMars, has_crew && discoveries.first_crewed_mars),
        ];

        for &(milestone, condition) in checks {
            if condition && !self.awarded_milestones.contains(&milestone) {
                self.awarded_milestones.insert(milestone);
                awarded.push((milestone, milestone.payout()));
            }
        }

        awarded
    }

    /// Sync awarded_milestones to match current discovery state.
    /// Inserts milestones without payouts for discoveries that are already true.
    /// Called on save load to prevent re-awarding milestones from old saves
    /// where discoveries happened before the milestone system existed.
    pub fn sync_milestones_from_discoveries(
        &mut self,
        discoveries: &DiscoveryTracker,
        solar_system: &SolarSystem,
    ) {
        let moon_idx = solar_system.moon_index;
        let mars_idx = solar_system.bodies.iter()
            .position(|b| b.name == "Mars")
            .unwrap_or(usize::MAX);

        let checks: &[(GovernmentMilestone, bool)] = &[
            (GovernmentMilestone::FirstSuborbital, discoveries.first_suborbital),
            (GovernmentMilestone::FirstOrbital, discoveries.first_orbit),
            (GovernmentMilestone::FirstCrewedOrbital, discoveries.first_crewed_orbit),
            (GovernmentMilestone::FirstLunarOrbit, discoveries.body_orbited.contains(&moon_idx)),
            (GovernmentMilestone::FirstLunarLanding, discoveries.body_landed.contains(&moon_idx)),
            (GovernmentMilestone::FirstCrewedLunar, discoveries.first_crewed_lunar),
            (GovernmentMilestone::FirstMarsOrbit, discoveries.body_orbited.contains(&mars_idx)),
            (GovernmentMilestone::FirstMarsLanding, discoveries.body_landed.contains(&mars_idx)),
            (GovernmentMilestone::FirstCrewedMars, discoveries.first_crewed_mars),
        ];

        for &(milestone, condition) in checks {
            if condition && !self.awarded_milestones.contains(&milestone) {
                self.awarded_milestones.insert(milestone);
                log::info!("Synced milestone from save: {}", milestone.display_name());
            }
        }
    }

    /// Get payloads for all active payload contracts (for editor UI).
    pub fn active_payload_contracts(&self) -> Vec<ContractPayload> {
        self.active.iter()
            .filter_map(|c| {
                if let ContractKind::Payload { mass_kg, .. } = &c.kind {
                    Some(ContractPayload {
                        contract_id: c.id,
                        name: c.name.clone(),
                        mass_kg: *mass_kg,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Fill the available pool up to POOL_SIZE.
    pub fn refill_pool(&mut self, discoveries: &DiscoveryTracker, solar_system: &SolarSystem, sim_time: f64) {
        while self.available.len() < POOL_SIZE {
            if let Some(contract) = self.generate_contract(discoveries, solar_system, sim_time) {
                self.available.push(contract);
            } else {
                break; // No eligible destinations
            }
        }
    }

    /// Add exactly one new contract to the pool (if below POOL_SIZE).
    pub fn refill_one(&mut self, discoveries: &DiscoveryTracker, solar_system: &SolarSystem, sim_time: f64) {
        if self.available.len() < POOL_SIZE {
            if let Some(contract) = self.generate_contract(discoveries, solar_system, sim_time) {
                self.available.push(contract);
            }
        }
    }

    /// Generate a single contract using deterministic PRNG.
    fn generate_contract(
        &mut self,
        discoveries: &DiscoveryTracker,
        solar_system: &SolarSystem,
        sim_time: f64,
    ) -> Option<Contract> {
        let seed = simple_hash(self.next_id, sim_time);

        // Collect eligible destinations
        let eligible: Vec<Destination> = Destination::all().iter()
            .copied()
            .filter(|d| d.is_unlocked(discoveries, solar_system))
            .collect();

        if eligible.is_empty() {
            return None;
        }

        // Pick destination
        let dest_idx = (seed % eligible.len() as u64) as usize;
        let destination = eligible[dest_idx];

        // Decide payload vs tourism (~70% payload, ~30% tourism)
        let is_tourism = (seed / 7) % 10 < 3 && destination.tourism_price_per_seat().is_some();

        let id = self.next_id;
        self.next_id += 1;

        if is_tourism {
            let price_per_seat = destination.tourism_price_per_seat().unwrap();
            let completions = self.completions_by_dest.get(&destination).copied().unwrap_or(0);
            let passengers = tourism_passenger_count(completions, seed);
            let payout = price_per_seat * passengers as f64;
            let name = format!("{} Tourism #{}", destination.display_name(), id);

            Some(Contract {
                id,
                kind: ContractKind::Tourism {
                    passengers,
                    destination,
                    destination_reached: false,
                },
                payout,
                name,
            })
        } else if destination == Destination::Suborbital {
            // Suborbital non-tourism contracts are test flights
            let part_idx = (seed / 11) as usize % SUBORBITAL_TEST_PARTS.len();
            let (part_id, part_name) = SUBORBITAL_TEST_PARTS[part_idx];
            let alt_idx = (seed / 19) as usize % SUBORBITAL_TEST_ALTITUDES.len();
            let target_altitude = SUBORBITAL_TEST_ALTITUDES[alt_idx];
            let payout = match alt_idx {
                0 => 300_000.0,
                1 => 600_000.0,
                _ => 1_200_000.0,
            };
            let name = format!("{} Test Flight #{}", part_name, id);

            Some(Contract {
                id,
                kind: ContractKind::SuborbitalTest {
                    part_id: part_id.to_string(),
                    part_name: part_name.to_string(),
                    target_altitude,
                },
                payout,
                name,
            })
        } else {
            let completions = self.completions_by_dest.get(&destination).copied().unwrap_or(0);
            let (min_kg, max_kg) = payload_mass_range(destination, completions);
            let mass_kg = interpolate_mass(min_kg, max_kg, seed);
            let payout = mass_kg * destination.price_per_kg();
            let name = format!("{} Delivery #{}", destination.display_name(), id);

            Some(Contract {
                id,
                kind: ContractKind::Payload {
                    mass_kg,
                    destination,
                },
                payout,
                name,
            })
        }
    }
}

/// Payload mass range based on destination and completion count.
fn payload_mass_range(destination: Destination, completions: u32) -> (f64, f64) {
    let (base_min, base_max) = match destination {
        Destination::Suborbital => (100.0, 500.0),
        Destination::LowEarthOrbit => (500.0, 2_000.0),
        Destination::LunarOrbit => (200.0, 1_000.0),
        Destination::LunarSurface => (100.0, 500.0),
        Destination::MarsOrbit => (100.0, 500.0),
        Destination::MarsSurface => (50.0, 300.0),
    };

    let scale = if completions < 5 {
        1.0
    } else if completions < 15 {
        // Linear interpolation 1x -> 5x over completions 5-14
        1.0 + (completions - 5) as f64 * (4.0 / 9.0)
    } else {
        // Linear interpolation 5x -> 25x over completions 15+
        let extra = (completions - 15).min(20) as f64;
        5.0 + extra * (20.0 / 20.0)
    };

    (base_min * scale, base_max * scale)
}

/// Tourism passenger count based on experience.
fn tourism_passenger_count(completions: u32, seed: u64) -> u32 {
    let (min, max) = if completions < 3 {
        (1, 2)
    } else if completions < 10 {
        (2, 4)
    } else {
        (4, 6)
    };
    let range = max - min + 1;
    min + ((seed / 13) % range as u64) as u32
}

/// Interpolate a mass value within a range using a seed.
fn interpolate_mass(min_kg: f64, max_kg: f64, seed: u64) -> f64 {
    // Round to nearest 50 kg
    let t = ((seed / 17) % 100) as f64 / 100.0;
    let raw = min_kg + t * (max_kg - min_kg);
    (raw / 50.0).round() * 50.0
}

/// Simple deterministic hash from two values (no external dependency).
fn simple_hash(id: u64, sim_time: f64) -> u64 {
    let time_bits = sim_time.to_bits();
    let mut h = id.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    h ^= time_bits;
    h = h.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    // Extra mixing
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h
}
