use std::collections::HashMap;
use crate::bodies::SolarSystem;
use crate::colony::dyson_swarm::DysonSwarm;
use crate::colony::notification::{Notification, NotificationKind};
use crate::colony::tech::TechTree;
use super::buildings::{BuildingType, Colony};
use super::resources::ResourceType;

/// 1 AU in meters, for solar power distance scaling.
const AU: f64 = 1.496e11;

/// Habitability multiplier: score 100 = 1x, score 0 = 2x.
/// Formula: (200 - score) / 100.
pub fn habitability_multiplier(score: u32) -> f64 {
    (200.0 - score as f64) / 100.0
}

/// Storage capacity: 500,000 kg per operational Stockpile.
impl Colony {
    pub fn storage_capacity(&self) -> f64 {
        let stockpile_count = self
            .buildings
            .iter()
            .filter(|b| b.building_type == BuildingType::Stockpile && b.operational)
            .count();
        stockpile_count as f64 * 500_000.0
    }

    /// Food storage capacity in kg.
    /// Habitat: 3,000 kg per operational building.
    /// FoodStorage: 10,000 kg per operational building.
    pub fn food_capacity(&self) -> f64 {
        let mut cap = 0.0;
        for b in &self.buildings {
            if !b.operational {
                continue;
            }
            match b.building_type {
                BuildingType::Habitat => cap += 3_000.0,
                BuildingType::FoodStorage => cap += 10_000.0,
                _ => {}
            }
        }
        cap
    }

    /// Crew capacity: 20 per operational Habitat.
    pub fn crew_capacity(&self) -> u32 {
        self.operational_building_count(BuildingType::Habitat) as u32 * 20
    }

    pub fn operational_building_count(&self, bt: BuildingType) -> usize {
        self.buildings
            .iter()
            .filter(|b| b.building_type == bt && b.operational)
            .count()
    }
}

/// Get the Sun distance for a body (walk parent chain to find the body orbiting the Sun).
pub fn sun_distance(body_index: usize, solar_system: &SolarSystem) -> f64 {
    let sun_idx = solar_system.sun_index;
    let mut idx = body_index;
    // Walk up until we find a body whose parent is the Sun
    loop {
        let body = &solar_system.bodies[idx];
        if let Some(parent) = body.parent {
            if parent == sun_idx {
                // This body orbits the Sun directly — use its SMA
                return body
                    .orbit
                    .map(|o| o.semi_major_axis)
                    .unwrap_or(AU);
            }
            idx = parent;
        } else {
            // Reached root without finding Sun (shouldn't happen for colonizable bodies)
            return AU;
        }
    }
}

/// Compute a map of resource -> production rate (kg/day) for active resource detection.
/// This is a lightweight version — only needs to know which resources have any production.
fn compute_production_map(colony: &Colony, tech_tree: &TechTree) -> HashMap<ResourceType, f64> {
    let mut production = HashMap::new();
    let mining_mult = TechTree::tier_multiplier(tech_tree.line_tier("mining"));
    let atmo_mult = TechTree::tier_multiplier(tech_tree.line_tier("atmospheric_science"));

    for b in &colony.buildings {
        if !b.operational {
            continue;
        }
        match b.building_type {
            BuildingType::Mine => {
                if let Some(res) = b.assigned_resource {
                    *production.entry(res).or_insert(0.0) +=
                        2000.0 * (1.0 - b.degradation) * colony.other_power_fraction * mining_mult;
                }
            }
            BuildingType::AtmosphericCollector => {
                if let Some(res) = b.assigned_resource {
                    *production.entry(res).or_insert(0.0) +=
                        10_000.0 * (1.0 - b.degradation) * colony.other_power_fraction * atmo_mult;
                }
            }
            BuildingType::Factory => {
                if let Some(recipe) = b.assigned_recipe {
                    let batches_per_day = 24.0 / recipe.batch_time_hours();
                    let factory_mult = TechTree::tier_multiplier(
                        tech_tree.line_tier(recipe.efficiency_line_id()),
                    );
                    let factor = batches_per_day
                        * (1.0 - b.degradation)
                        * colony.other_power_fraction
                        * factory_mult;
                    for &(res, amt) in &recipe.outputs() {
                        *production.entry(res).or_insert(0.0) += amt * factor;
                    }
                }
            }
            _ => {}
        }
    }
    production
}

/// Run one simulation tick for a colony.
pub fn simulate_colony_tick(
    colony: &mut Colony,
    days: f64,
    solar_system: &SolarSystem,
    notifications: &mut Vec<Notification>,
    sim_time: f64,
    tech_tree: &TechTree,
    dyson_swarm: Option<&mut DysonSwarm>,
    fleet_manager: Option<&mut super::trade::FleetManager>,
) {
    let hab_score = solar_system.bodies[colony.body_index].habitability_score;
    let hab_mult = habitability_multiplier(hab_score);
    let body_radius_m = solar_system.bodies[colony.body_index].radius;
    let dist = sun_distance(colony.body_index, solar_system);
    let sail_tier = tech_tree.line_tier("sail_technology");

    // === 0. Reactor fuel consumption ===
    // Consume fuel before power calculation. Reactors without fuel don't generate.
    // Track which building indices have fuel available, and atomically consume
    // fuel as we go so reactors sharing the same fuel pool compete correctly.
    // Each check-and-remove is atomic via `remove()` / `remove_all()`, so when
    // the pool runs dry subsequent reactors simply fail to fuel.
    let mut reactor_has_fuel = vec![false; colony.buildings.len()];
    for (i, b) in colony.buildings.iter().enumerate() {
        if !b.operational {
            continue;
        }
        match b.building_type {
            BuildingType::FissionReactor => {
                // 0.5 kg Enriched Uranium per day
                reactor_has_fuel[i] = colony.resources.remove(
                    super::resources::ResourceType::EnrichedUranium,
                    0.5 * days,
                );
            }
            BuildingType::FusionReactor => {
                // 3 kg He-3 + 2 kg Deuterium per day — atomic multi-resource removal
                reactor_has_fuel[i] = colony.resources.remove_all(&[
                    (super::resources::ResourceType::Helium3, 3.0 * days),
                    (super::resources::ResourceType::Deuterium, 2.0 * days),
                ]);
            }
            _ => {}
        }
    }

    // === 1. Power balance (habitat-priority allocation) ===
    let mut total_generation = 0.0_f64;
    let mut habitat_demand = 0.0_f64;
    let mut other_demand = 0.0_f64;

    for (i, b) in colony.buildings.iter().enumerate() {
        if !b.operational {
            continue;
        }
        // Generation
        let output = b.building_type.power_output_kw();
        if output > 0.0 {
            let is_solar = matches!(
                b.building_type,
                BuildingType::SmallSolarFarm
                    | BuildingType::MediumSolarFarm
                    | BuildingType::LargeSolarFarm
            );
            if is_solar {
                total_generation += output * (AU / dist).powi(2) * (1.0 - b.degradation);
            } else if reactor_has_fuel[i] {
                total_generation += output * (1.0 - b.degradation);
            }
            // If reactor has no fuel, it contributes 0 power
        }
        // Demand — split habitat vs other. Size-scaled buildings (Mk IV)
        // multiply by body circumference.
        let draw = b.building_type.power_draw_kw() * b.building_type.size_multiplier(body_radius_m);
        if b.building_type == BuildingType::Habitat {
            habitat_demand += draw;
        } else {
            other_demand += draw;
        }
        // Factory recipe power goes to "other"
        if b.building_type == BuildingType::Factory {
            if let Some(recipe) = b.assigned_recipe {
                other_demand += recipe.power_draw_kw();
            }
        }
    }

    // Swarm receiver power: depends on available laser from DysonSwarm
    colony.receiver_power_kw = 0.0;
    colony.receiver_laser_power_kw = 0.0;
    if let Some(ref swarm) = dyson_swarm {
        let beta = crate::colony::dyson_swarm::lightness_number_at_tier(sail_tier);
        let total_laser_w = swarm.available_laser_power(beta);
        let receiver_cap_w: f64 = colony.buildings.iter()
            .filter(|b| b.operational && b.building_type == BuildingType::ReceiverArray)
            .map(|b| (1.0 - b.degradation) * crate::colony::dyson_swarm::MAX_RECEIVER_INPUT_W)
            .sum();
        if receiver_cap_w > 0.0 && total_laser_w > 0.0 {
            let received_w = total_laser_w.min(receiver_cap_w);
            let electricity_w = received_w * crate::colony::dyson_swarm::RECEIVER_EFFICIENCY;
            colony.receiver_power_kw = electricity_w / 1000.0; // W → kW
            colony.receiver_laser_power_kw = total_laser_w / 1000.0; // W → kW
            total_generation += colony.receiver_power_kw;
        }
    }

    // Habitats get power first, then everything else gets the remainder
    colony.habitat_power_fraction = if habitat_demand == 0.0 {
        1.0
    } else {
        (total_generation / habitat_demand).min(1.0)
    };
    let power_after_habitats = (total_generation - habitat_demand).max(0.0);
    colony.other_power_fraction = if other_demand == 0.0 {
        1.0
    } else {
        (power_after_habitats / other_demand).min(1.0)
    };

    colony.power_generated = total_generation;
    colony.power_consumed = habitat_demand + other_demand;

    // Habitat power loss notification
    if colony.habitat_power_fraction < 1.0 && colony.crew > 0 && !colony.habitat_unpowered_notified
    {
        colony.habitat_unpowered_notified = true;
        notifications.push(Notification {
            kind: NotificationKind::ColonyPowerLoss {
                colony_name: colony.name.clone(),
            },
            time: sim_time,
            read: false,
        });
    }
    if colony.habitat_power_fraction >= 1.0 {
        colony.habitat_unpowered_notified = false;
    }

    // === 2. Maintenance ===
    process_maintenance(colony, days, hab_mult, body_radius_m, tech_tree);

    // === 3. Construction ===
    process_construction(colony, days, hab_mult, body_radius_m, notifications, sim_time, tech_tree);

    // === 4. Mines (per-resource storage caps) ===
    let storage_cap = colony.storage_capacity();

    // Compute production rates for active resource detection
    let production_map = compute_production_map(colony, tech_tree);
    let active_resources = super::resources::compute_active_resources(&colony.resources, &production_map);

    for b in &mut colony.buildings {
        if !b.operational || b.building_type != BuildingType::Mine {
            continue;
        }
        if let Some(resource) = b.assigned_resource {
            let mining_mult = TechTree::tier_multiplier(tech_tree.line_tier("mining"));
            let production = 2000.0 * days * (1.0 - b.degradation) * colony.other_power_fraction * mining_mult;
            let res_cap = colony.storage_allocation.capacity_for(resource, storage_cap, &active_resources);
            let available = (res_cap - colony.resources.get(resource)).max(0.0);
            let capped = production.min(available);
            if capped > 0.0 {
                colony.resources.add(resource, capped);
            }
        }
    }

    // === 4b. Atmospheric Collectors (per-resource storage caps) ===
    for b in &mut colony.buildings {
        if !b.operational || b.building_type != BuildingType::AtmosphericCollector {
            continue;
        }
        if let Some(resource) = b.assigned_resource {
            let atmo_mult = TechTree::tier_multiplier(tech_tree.line_tier("atmospheric_science"));
            let production =
                10_000.0 * days * (1.0 - b.degradation) * colony.other_power_fraction * atmo_mult;
            let res_cap = colony.storage_allocation.capacity_for(resource, storage_cap, &active_resources);
            let available = (res_cap - colony.resources.get(resource)).max(0.0);
            let capped = production.min(available);
            if capped > 0.0 {
                colony.resources.add(resource, capped);
            }
        }
    }

    // === 5. Factories (per-resource storage caps) ===
    for i in 0..colony.buildings.len() {
        let b = &colony.buildings[i];
        if !b.operational || b.building_type != BuildingType::Factory {
            continue;
        }
        let recipe = match b.assigned_recipe {
            Some(r) => r,
            None => continue,
        };
        let degradation = b.degradation;

        // Check colocation requirement
        if let Some(required) = recipe.requires_colocation() {
            if colony.operational_building_count(required) == 0 {
                continue;
            }
        }

        let outputs = recipe.outputs();
        let inputs = recipe.inputs();
        let batch_hours = recipe.batch_time_hours();

        // Throughput: how many batches worth of output per day
        let batches_per_day = 24.0 / batch_hours;
        let factory_mult = TechTree::tier_multiplier(tech_tree.line_tier(recipe.efficiency_line_id()));
        let throughput_factor = batches_per_day * days * (1.0 - degradation) * colony.other_power_fraction * factory_mult;

        // Check if we have enough inputs
        let mut can_produce = true;
        for &(ref res, amount) in &inputs {
            let needed = amount * throughput_factor;
            if colony.resources.get(*res) < needed {
                can_produce = false;
                break;
            }
        }

        if !can_produce {
            continue;
        }

        // Check per-resource storage caps for each output.
        // Storage caps are in kg, but unit-counted resources (MirrorSegment,
        // CollectorStation) are stored as counts. Convert to kg before comparing.
        let mut output_blocked = false;
        for &(ref res, amount) in &outputs {
            let output_amount = amount * throughput_factor;
            let res_cap = colony.storage_allocation.capacity_for(*res, storage_cap, &active_resources);
            // How much of this input resource is being freed?
            let input_freed: f64 = inputs.iter()
                .filter(|(r, _)| *r == *res)
                .map(|(_, a)| a * throughput_factor)
                .sum();
            let current = colony.resources.get(*res);
            let unit_mass = res.storage_mass_per_unit(sail_tier);
            if (current + output_amount - input_freed) * unit_mass > res_cap {
                output_blocked = true;
                break;
            }
        }
        if output_blocked {
            continue;
        }

        // Consume inputs
        for &(ref res, amount) in &inputs {
            colony.resources.remove(*res, amount * throughput_factor);
        }

        // Produce outputs
        for &(ref res, amount) in &outputs {
            colony.resources.add(*res, amount * throughput_factor);
        }
    }

    // === 6. Greenhouses ===
    for b in &mut colony.buildings {
        if !b.operational {
            continue;
        }
        let agri_mult = TechTree::tier_multiplier(tech_tree.line_tier("agriculture"));
        match b.building_type {
            BuildingType::BasicGreenhouse => {
                let max_water = 2_000.0; // kg
                let rate = 0.5 * days * (b.water_fill / max_water).min(1.0) * (1.0 - b.degradation) * colony.other_power_fraction * agri_mult;
                colony.food_stored += rate;
            }
            BuildingType::AdvancedGreenhouse => {
                let max_water = 5_000.0; // kg
                let rate = 2.5 * days * (b.water_fill / max_water).min(1.0) * (1.0 - b.degradation) * colony.other_power_fraction * agri_mult;
                colony.food_stored += rate;
            }
            _ => {}
        }
    }

    // === 6b. Cap food at capacity ===
    let food_cap = colony.food_capacity();
    if food_cap > 0.0 && colony.food_stored > food_cap {
        colony.food_stored = food_cap;
    }

    // === 7. Food consumption ===
    let food_consumed = 0.5 * colony.crew as f64 * days;
    colony.food_stored -= food_consumed;
    if colony.food_stored <= 0.0 {
        colony.food_stored = 0.0;
        if colony.crew > 0 && !colony.food_depleted_notified {
            colony.food_depleted_notified = true;
            notifications.push(Notification {
                kind: NotificationKind::ColonyFoodDepleted {
                    colony_name: colony.name.clone(),
                },
                time: sim_time,
                read: false,
            });
        }
    } else {
        colony.food_depleted_notified = false;
    }

    // === 7b. Crew death from food/power crisis ===
    let crisis = (colony.food_stored <= 0.0 && colony.crew > 0)
        || (colony.habitat_power_fraction < 1.0 && colony.crew > 0);

    if crisis {
        if colony.crew_at_crisis_start.is_none() {
            colony.crew_at_crisis_start = Some(colony.crew);
        }
        let base_crew = colony.crew_at_crisis_start.unwrap_or(colony.crew) as f64;
        let deaths_per_day = base_crew * 0.005; // 1% per 2 days
        colony.crew_death_accumulator += deaths_per_day * days;
        let whole_deaths = colony.crew_death_accumulator.floor() as u32;
        if whole_deaths > 0 {
            colony.crew_death_accumulator -= whole_deaths as f64;
            colony.crew = colony.crew.saturating_sub(whole_deaths);
        }
    } else {
        colony.crew_at_crisis_start = None;
        colony.crew_death_accumulator = 0.0;
    }

    // === 8. Mass driver (ship queue + mirror auto-launch) ===
    if let Some(swarm) = dyson_swarm {
        process_mass_driver(colony, days, swarm, notifications, sim_time, tech_tree, fleet_manager);
        swarm.process_deployments(sim_time);
    }

    // === 9. Science labs ===
    process_science_labs(colony, days, solar_system);
}

/// Process maintenance for all buildings.
fn process_maintenance(colony: &mut Colony, days: f64, hab_mult: f64, body_radius_m: f64, tech_tree: &TechTree) {
    let construction_mult = TechTree::tier_multiplier(tech_tree.line_tier("construction"));
    let life_support_mult = TechTree::tier_multiplier(tech_tree.line_tier("life_support"));

    // Calculate robot maintenance capacity
    let mut robot_maintenance_capacity = 0.0_f64;
    for b in &colony.buildings {
        if !b.operational {
            continue;
        }
        match b.building_type {
            BuildingType::ConstructionRobot => robot_maintenance_capacity += 60_000.0 * days * construction_mult,
            BuildingType::LightConstructionRobot => robot_maintenance_capacity += 15_000.0 * days * construction_mult,
            _ => {}
        }
    }

    // Calculate total maintenance demand.
    // `mult` combines habitability scaling (Habitat/Greenhouse), size scaling
    // (Mk IV Particle Accelerator), and life_support cost reduction for hab buildings.
    let mut total_maintenance_mass = 0.0_f64;
    for b in &colony.buildings {
        let costs = b.building_type.maintenance_cost_per_30d();
        let hab = if b.building_type.affected_by_habitability() {
            hab_mult / life_support_mult
        } else {
            1.0
        };
        let mult = hab * b.building_type.size_multiplier(body_radius_m);
        let mass: f64 = costs.iter().map(|(_, amt)| amt * mult).sum();
        total_maintenance_mass += mass / 30.0 * days;
    }

    let robot_shortfall = if robot_maintenance_capacity > 0.0 && total_maintenance_mass > robot_maintenance_capacity {
        1.0 - robot_maintenance_capacity / total_maintenance_mass
    } else {
        0.0
    };

    // Process per-building maintenance
    for b in &mut colony.buildings {
        let costs = b.building_type.maintenance_cost_per_30d();
        if costs.is_empty() {
            continue;
        }

        let hab = if b.building_type.affected_by_habitability() {
            hab_mult / life_support_mult
        } else {
            1.0
        };
        let mult = hab * b.building_type.size_multiplier(body_radius_m);

        let mut resource_shortfall = 0.0_f64;
        for &(ref res, amount) in &costs {
            let needed = amount * mult / 30.0 * days;
            let available = colony.resources.get(*res);
            if available >= needed {
                colony.resources.remove(*res, needed);
            } else {
                colony.resources.remove(*res, available);
                resource_shortfall = resource_shortfall.max(1.0 - available / needed);
            }
        }

        let shortfall = resource_shortfall.max(robot_shortfall);
        if shortfall > 0.0 {
            b.degradation = (b.degradation + shortfall * days / 30.0).min(1.0);
        }
    }
}

/// Process construction queue.
fn process_construction(
    colony: &mut Colony,
    days: f64,
    hab_mult: f64,
    body_radius_m: f64,
    notifications: &mut Vec<Notification>,
    sim_time: f64,
    tech_tree: &TechTree,
) {
    let construction_mult = TechTree::tier_multiplier(tech_tree.line_tier("construction"));

    // Calculate robot construction capacity (after maintenance)
    let mut robot_construction_capacity = 0.0_f64;
    for b in &colony.buildings {
        if !b.operational {
            continue;
        }
        match b.building_type {
            BuildingType::ConstructionRobot => robot_construction_capacity += 20_000.0 * days * construction_mult,
            BuildingType::LightConstructionRobot => robot_construction_capacity += 5_000.0 * days * construction_mult,
            _ => {}
        }
    }

    // Subtract maintenance demand from robot capacity
    let mut maintenance_demand = 0.0_f64;
    for b in &colony.buildings {
        let costs = b.building_type.maintenance_cost_per_30d();
        let hab = if b.building_type.affected_by_habitability() {
            hab_mult
        } else {
            1.0
        };
        let mult = hab * b.building_type.size_multiplier(body_radius_m);
        let mass: f64 = costs.iter().map(|(_, amt)| amt * mult).sum();
        maintenance_demand += mass / 30.0 * days;
    }

    let remaining_capacity = (robot_construction_capacity - maintenance_demand).max(0.0);
    if remaining_capacity <= 0.0 || colony.construction_queue.is_empty() {
        return;
    }

    // Apply remaining capacity to first queue item
    let item = &mut colony.construction_queue[0];
    item.mass_assembled += remaining_capacity;

    // Complete units in the batch while mass is sufficient
    while item.mass_assembled >= item.total_mass {
        let target = item.effective_target();

        match target {
            super::buildings::ConstructionTarget::Building(building_type) => {
                // Add the new building
                colony.buildings.push(super::buildings::BuildingInstance::new(building_type));

                // Pre-stock food when a Habitat completes
                if building_type == BuildingType::Habitat {
                    colony.food_stored += 1_000.0;
                }
            }
            super::buildings::ConstructionTarget::Ship {
                name,
                blueprint_name,
                blueprint,
                dry_mass_kg,
                cached_delta_v,
            } => {
                // Create a StoredShip and add to colony hangar
                let stored = super::trade::StoredShip {
                    id: colony.next_stored_ship_id,
                    name: name.clone(),
                    blueprint_name: Some(blueprint_name),
                    blueprint,
                    dry_mass_kg,
                    cached_delta_v,
                };
                colony.next_stored_ship_id += 1;
                colony.stored_ships.push(stored);

                notifications.push(Notification {
                    kind: NotificationKind::ShipConstructionComplete {
                        ship_name: name,
                        location: colony.name.clone(),
                    },
                    time: sim_time,
                    read: false,
                });
            }
        }

        item.completed += 1;
        if item.completed >= item.count {
            colony.construction_queue.remove(0);
            break;
        }
        // Reset for next unit in batch
        item.mass_assembled -= item.total_mass;
    }
}

/// Process mass driver: ship queue first (priority), then mirror auto-launch.
///
/// The mass driver accumulates energy from available colony power (the fraction
/// allocated to non-habitat buildings). Ships in the queue are launched first
/// (1,000g accel), then mirrors (10,000g accel) from stockpile.
fn process_mass_driver(
    colony: &mut Colony,
    days: f64,
    swarm: &mut DysonSwarm,
    _notifications: &mut Vec<Notification>,
    sim_time: f64,
    tech_tree: &TechTree,
    fleet_manager: Option<&mut super::trade::FleetManager>,
) {
    // Find best mass driver
    let driver = match colony.best_mass_driver() {
        Some(d) => d,
        None => {
            // No operational driver — refund any queued ships
            if let Some(fm) = fleet_manager {
                for entry in colony.mass_driver_ship_queue.drain(..) {
                    if let Some(ship) = fm.get_ship_mut(entry.trade_ship_id) {
                        ship.state = super::trade::TradeShipState::Stationed;
                    }
                }
            }
            return;
        }
    };

    // Mirror mass depends on sail technology tier
    let sail_tier = tech_tree.line_tier("sail_technology");
    let mirror_mass = crate::colony::dyson_swarm::mirror_mass_at_tier(sail_tier);

    // Power available to mass driver (watts) = power_draw_kw * other_power_fraction * 1000
    let power_w = driver.power_draw_kw() * 1000.0 * colony.other_power_fraction;

    // Max energy storage capacity for this driver tier
    let mirror_launch_v = driver.mass_driver_launch_velocity(mirror_mass, true);
    let mirror_energy = mirror_launch_v
        .map(|v| BuildingType::mass_driver_launch_energy_j(mirror_mass, v))
        .unwrap_or(f64::MAX);
    let capacity = driver.mass_driver_energy_capacity_j().unwrap_or(mirror_energy);

    // Accumulate energy over this tick, capped at storage capacity
    let energy_this_tick = power_w * days * 86_400.0;
    colony.mass_driver_energy_j = (colony.mass_driver_energy_j + energy_this_tick).min(capacity);

    // === 1. Process ship queue (priority over mirrors) ===
    if let Some(fm) = fleet_manager {
        let mut launched_indices = Vec::new();
        for (i, entry) in colony.mass_driver_ship_queue.iter().enumerate() {
            // Check if the required driver tier is still operational
            let tier_operational = colony.buildings.iter().any(|b| {
                b.operational && b.building_type == entry.driver_tier
            });
            if !tier_operational {
                // Refund: set ship back to Stationed
                if let Some(ship) = fm.get_ship_mut(entry.trade_ship_id) {
                    ship.state = super::trade::TradeShipState::Stationed;
                }
                launched_indices.push(i);
                continue;
            }

            // Compute launch energy for ship (1,000g accel)
            let launch_v = match entry.driver_tier.mass_driver_launch_velocity(entry.mass_kg, false) {
                Some(v) => v,
                None => {
                    // Ship too heavy — refund
                    if let Some(ship) = fm.get_ship_mut(entry.trade_ship_id) {
                        ship.state = super::trade::TradeShipState::Stationed;
                    }
                    launched_indices.push(i);
                    continue;
                }
            };
            let energy_needed = BuildingType::mass_driver_launch_energy_j(entry.mass_kg, launch_v);

            if colony.mass_driver_energy_j >= energy_needed {
                colony.mass_driver_energy_j -= energy_needed;
                // Transition ship to InTransit
                if let Some(ship) = fm.get_ship_mut(entry.trade_ship_id) {
                    ship.state = super::trade::TradeShipState::InTransit;
                }
                launched_indices.push(i);
            } else {
                break; // Not enough energy — wait (blocks mirrors too)
            }
        }
        // Remove launched/refunded entries (reverse order to preserve indices)
        for i in launched_indices.into_iter().rev() {
            colony.mass_driver_ship_queue.remove(i);
        }
    }

    // === 2. Fire collector stations (ship-class 1,000g accel) ===
    let collector_mass = crate::colony::dyson_swarm::COLLECTOR_MASS_KG;
    if let Some(collector_launch_v) = driver.mass_driver_launch_velocity(collector_mass, false) {
        let collector_energy = BuildingType::mass_driver_launch_energy_j(collector_mass, collector_launch_v);
        while colony.mass_driver_energy_j >= collector_energy {
            if colony.resources.get(ResourceType::CollectorStation) < 1.0 {
                break;
            }
            colony.resources.remove(ResourceType::CollectorStation, 1.0);
            colony.mass_driver_energy_j -= collector_energy;
            swarm.launch_collector(sim_time);
        }
    }

    // === 3. Fire mirrors ===
    let launch_v = match mirror_launch_v {
        Some(v) => v,
        None => return, // Mirror too heavy for this driver
    };
    let energy_per_launch = BuildingType::mass_driver_launch_energy_j(mirror_mass, launch_v);

    while colony.mass_driver_energy_j >= energy_per_launch {
        if colony.resources.get(ResourceType::MirrorSegment) < 1.0 {
            break;
        }
        colony.resources.remove(ResourceType::MirrorSegment, 1.0);
        colony.mass_driver_energy_j -= energy_per_launch;
        swarm.launch_mirror(sim_time);
        colony.mirrors_launched += 1;
    }
}

/// Process science labs.
fn process_science_labs(colony: &mut Colony, days: f64, solar_system: &SolarSystem) {
    let lab_count = colony.operational_building_count(BuildingType::ScienceLab);
    if lab_count == 0 {
        return;
    }

    let body = &solar_system.bodies[colony.body_index];
    let lv = body.landing_science_value();
    let n = lab_count as f64 * colony.other_power_fraction;

    // Advance lab elapsed time
    colony.lab_elapsed_years += days / 365.0;

    // extracted(t) = 10 * lv * (1 - e^(-N * t / 15))
    let new_extracted = 10.0 * lv * (1.0 - (-n * colony.lab_elapsed_years / 15.0).exp());
    colony.lab_science_extracted = new_extracted;
}
