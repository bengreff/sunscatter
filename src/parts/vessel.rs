use serde::{Serialize, Deserialize};
use super::{FuelType, Propellant, PartDefinitions, ReactorFuelData, VesselBlueprint};
use std::collections::HashMap;

/// A vessel in flight - runtime representation with physics.
/// Uses struct-level `#[serde(default)]` so old saves missing new fields still load.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FlightVessel {
    // Physics state (same as Ship)
    pub rel_position: [f64; 2],
    pub rel_velocity: [f64; 2],
    pub rotation: f64,
    pub rotational_velocity: f64,
    pub soi_body: usize,

    // Part tree
    pub parts: Vec<FlightPart>,
    pub root_part_index: usize,

    // Aggregated properties (calculated from parts)
    pub total_mass: f64,
    pub dry_mass: f64,
    pub center_of_mass: [f64; 2],
    pub max_thrust_vac: f64,
    pub max_thrust_asl: f64,
    pub moment_of_inertia: f64,

    // Flight state
    pub throttle: f64,
    pub on_rails: bool,
    pub stages: Vec<Vec<usize>>,
    pub current_stage: usize,

    // Ejection force from last decoupler firing (kN), consumed by handle_post_decouple
    pub last_decouple_force: f64,

    // Extra dry mass added as cargo payload (tonnes). Used by trade route dv computation
    // to make cargo reduce ship delta-v. Not consumed as fuel.
    pub extra_dry_mass_tonnes: f64,

    // Ship-wide thermal pool (waste heat from engines/reactors, rejected by radiators)
    pub thermal_pool_temp: f64,        // Kelvin, init AMBIENT_TEMPERATURE
    pub thermal_pool_capacity: f64,    // J/K, recomputed in recalculate_mass

    // Reactor trip cascade: when pool temp passes REACTOR_TRIP_TEMP, all reactors trip and
    // produce no power until manually restarted (after hysteresis cooldown).
    pub reactors_tripped: bool,

    // Life support
    pub food_stored: f64,         // kg of food currently aboard
    pub total_crew: u32,          // living crew count across all pods
    pub starvation_timer: f64,    // seconds continuously at zero food
}

impl Default for FlightVessel {
    fn default() -> Self {
        Self {
            rel_position: [0.0, 0.0],
            rel_velocity: [0.0, 0.0],
            rotation: 0.0,
            rotational_velocity: 0.0,
            soi_body: 0,
            parts: Vec::new(),
            root_part_index: 0,
            total_mass: 0.0,
            dry_mass: 0.0,
            center_of_mass: [0.0, 0.0],
            max_thrust_vac: 0.0,
            max_thrust_asl: 0.0,
            moment_of_inertia: 0.0,
            throttle: 0.0,
            on_rails: false,
            stages: Vec::new(),
            current_stage: 0,
            last_decouple_force: 0.0,
            extra_dry_mass_tonnes: 0.0,
            thermal_pool_temp: 300.0,
            thermal_pool_capacity: 0.0,
            reactors_tripped: false,
            food_stored: 0.0,
            total_crew: 0,
            starvation_timer: 0.0,
        }
    }
}

/// A part in flight.
/// Uses struct-level `#[serde(default)]` so old saves missing new fields still load.
/// When adding fields, ensure the `Default` impl below provides sensible values.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FlightPart {
    pub definition_id: String,
    pub local_position: [f64; 2],  // Relative to vessel center of mass
    pub rotation: f64,
    pub hitbox_half_extents: [f64; 2],
    pub hitbox_y_offset: f64,  // Offset from local_position to hitbox center (engines: top-aligned)

    // Resources currently in this part
    pub resources: HashMap<String, f64>,
    pub max_resources: HashMap<String, f64>,

    // Engine state (if this is an engine)
    pub engine_active: bool,
    pub engine_enabled: bool,  // User toggle: if false, engine won't fire even with fuel
    pub engine_thrust_vac: f64,
    pub engine_thrust_asl: f64,
    pub engine_isp_vac: f64,
    pub engine_isp_asl: f64,
    pub is_throttleable: bool,
    pub propellant_type: Option<Propellant>,
    pub secondary_propellant_type: Option<Propellant>,
    pub secondary_fuel_fraction: f64, // fraction of mass flow that is secondary fuel
    pub mass_flow_rate: f64, // kg/s total (fuel+oxidizer) at full vacuum thrust

    // Gimbal state (if engine has gimbal)
    pub gimbal_angle: f64,      // Current gimbal deflection (radians)
    pub gimbal_range_rad: f64,  // Maximum gimbal deflection (radians, from engine data)

    // RCS state (if this is an RCS thruster)
    pub rcs_thrust: f64,         // kN (0 if not an RCS part)
    pub rcs_torque_multiplier: f64, // Multiplier for rotational torque (default 1.0)
    pub rcs_isp: f64,            // seconds
    pub rcs_mass_flow_rate: f64, // kg/s at full thrust

    // State
    pub destroyed: bool,
    pub decoupled: bool,
    pub crossfeed_enabled: bool, // Whether fuel can flow through this decoupler

    // Thermal state
    pub temperature: f64,         // Kelvin
    pub max_heat_tolerance: f64,  // Kelvin (from PartDefinition)

    // Fairing shell shape (if this is a fairing base)
    pub fairing_shape: Option<crate::parts::FairingShape>,
    // Which half to render (None = full shell, Some = debris half)
    pub fairing_half: Option<crate::parts::FairingHalf>,

    // Electricity (stored separately from resources to avoid mass calculation interference)
    pub electricity: f64,      // Current stored Wh
    pub max_electricity: f64,  // Capacity Wh

    // Solar panel deployment (also used by radiators — see is_radiator)
    pub is_solar_panel: bool,         // true if this part has solar panel data
    pub deploy_fraction: f64,     // 0.0 = retracted, 1.0 = fully deployed
    pub deploy_target: bool,      // desired state (false = retract, true = deploy)
    pub mirror_partner: Option<usize>, // index of mirror partner in parts vec

    // Radiator state (waste heat rejection)
    pub is_radiator: bool,            // true if this part has radiator data
    pub radiator_rejection_watts: f64, // Rated rejection at full deployment (from RadiatorData)
    pub radiator_deploy_time_sec: f64, // Seconds to fully deploy/retract

    // Parachute state
    pub is_parachute: bool,
    pub parachute_deployed: bool,       // currently deployed
    pub parachute_spent: bool,          // used once, permanently disabled
    pub parachute_deploy_fraction: f64, // 0.0-1.0 animation
    pub parachute_deployed_width_m: f64, // meters (from ParachuteData)
    pub parachute_fully_deployed: bool, // true when altitude <= 2000m (full drag)

    // Cargo container manifest
    pub cargo_buildings: Vec<String>,  // BuildingType display names loaded in cargo
    pub cargo_payloads: Vec<crate::colony::ContractPayload>,  // Contract payloads

    // Electric engine power gating
    pub engine_no_power: bool,  // true when engine has fuel but no electricity

    // Shield activation
    pub shield_active: bool,    // true when active shield is drawing power

    // Life support — per-pod crew tracking
    pub crew_count: u32,        // living crew currently in this pod (0 for non-pods)
}

impl Default for FlightPart {
    fn default() -> Self {
        Self {
            definition_id: String::new(),
            local_position: [0.0, 0.0],
            rotation: 0.0,
            hitbox_half_extents: [0.0, 0.0],
            hitbox_y_offset: 0.0,
            resources: HashMap::new(),
            max_resources: HashMap::new(),
            engine_active: false,
            engine_enabled: false,
            engine_thrust_vac: 0.0,
            engine_thrust_asl: 0.0,
            engine_isp_vac: 0.0,
            engine_isp_asl: 0.0,
            is_throttleable: false,
            propellant_type: None,
            secondary_propellant_type: None,
            secondary_fuel_fraction: 0.0,
            mass_flow_rate: 0.0,
            gimbal_angle: 0.0,
            gimbal_range_rad: 0.0,
            rcs_thrust: 0.0,
            rcs_torque_multiplier: 1.0,
            rcs_isp: 0.0,
            rcs_mass_flow_rate: 0.0,
            destroyed: false,
            decoupled: false,
            crossfeed_enabled: false,
            temperature: 0.0,
            max_heat_tolerance: 0.0,
            fairing_shape: None,
            fairing_half: None,
            electricity: 0.0,
            max_electricity: 0.0,
            is_solar_panel: false,
            deploy_fraction: 0.0,
            deploy_target: false,
            mirror_partner: None,
            is_radiator: false,
            radiator_rejection_watts: 0.0,
            radiator_deploy_time_sec: 5.0,
            is_parachute: false,
            parachute_deployed: false,
            parachute_spent: false,
            parachute_deploy_fraction: 0.0,
            parachute_deployed_width_m: 0.0,
            parachute_fully_deployed: false,
            cargo_buildings: Vec::new(),
            cargo_payloads: Vec::new(),
            engine_no_power: false,
            shield_active: false,
            crew_count: 0,
        }
    }
}

impl FlightPart {
    /// Total mass of cargo buildings in this part, in tonnes.
    pub fn cargo_building_mass_tonnes(&self) -> f64 {
        self.cargo_buildings.iter()
            .filter_map(|name| crate::colony::BuildingType::from_display_name(name))
            .map(|bt| bt.total_build_mass())
            .sum::<f64>() * 0.001 // kg -> tonnes
    }

    /// Total mass of contract payloads in this part, in tonnes.
    pub fn cargo_payload_mass_tonnes(&self) -> f64 {
        self.cargo_payloads.iter().map(|p| p.mass_kg).sum::<f64>() * 0.001
    }

    /// Total extra cargo mass (buildings + payloads) in tonnes.
    pub fn cargo_extra_mass_tonnes(&self) -> f64 {
        self.cargo_building_mass_tonnes() + self.cargo_payload_mass_tonnes()
    }
}

impl FlightVessel {
    /// Returns true if the vessel has any non-destroyed, non-decoupled part that provides control.
    pub fn has_control(&self, part_defs: &PartDefinitions) -> bool {
        self.parts.iter().any(|p| {
            !p.destroyed && !p.decoupled &&
            part_defs.get(&p.definition_id)
                .and_then(|d| d.pod.as_ref())
                .map_or(false, |pod| {
                    pod.can_control && (pod.crew_capacity == 0 || p.crew_count > 0)
                })
        })
    }

    /// Check if vessel cargo contains minimum colony buildings (Habitat + Small Solar Farm).
    pub fn has_colony_buildings(&self) -> bool {
        let all_buildings: Vec<&str> = self.parts.iter()
            .filter(|p| !p.decoupled && !p.destroyed)
            .flat_map(|p| p.cargo_buildings.iter().map(|s| s.as_str()))
            .collect();
        all_buildings.iter().any(|b| *b == "Habitat")
            && all_buildings.iter().any(|b| *b == "Small Solar Farm")
    }

    /// Collect all contract payloads from non-decoupled, non-destroyed cargo containers.
    pub fn all_payloads(&self) -> Vec<crate::colony::ContractPayload> {
        self.parts.iter()
            .filter(|p| !p.decoupled && !p.destroyed)
            .flat_map(|p| p.cargo_payloads.iter().cloned())
            .collect()
    }

    /// Check if any non-decoupled, non-destroyed cargo container has contents.
    pub fn has_cargo(&self, part_defs: &PartDefinitions) -> bool {
        self.parts.iter().any(|p| {
            !p.decoupled && !p.destroyed
                && part_defs.get(&p.definition_id).and_then(|d| d.cargo.as_ref()).is_some()
                && (!p.cargo_buildings.is_empty()
                    || !p.cargo_payloads.is_empty()
                    || p.resources.values().any(|&v| v > 0.0))
        })
    }

    /// Extract all cargo from all containers and return (buildings, resources, food_kg).
    /// Clears the cargo containers.
    pub fn extract_all_cargo(&mut self, part_defs: &PartDefinitions)
        -> (Vec<String>, Vec<(String, f64)>, f64)
    {
        let mut buildings = Vec::new();
        let mut resources = Vec::new();
        let mut food_kg = 0.0;

        for part in &mut self.parts {
            if part.decoupled || part.destroyed {
                continue;
            }
            if part_defs.get(&part.definition_id).and_then(|d| d.cargo.as_ref()).is_none() {
                continue;
            }
            // Extract buildings
            buildings.append(&mut part.cargo_buildings);
            // Extract resources
            for (name, amount) in part.resources.drain() {
                if amount > 0.0 {
                    if name == "food" {
                        food_kg += amount;
                    } else {
                        resources.push((name, amount));
                    }
                }
            }
        }
        self.recalculate_mass(part_defs);
        (buildings, resources, food_kg)
    }

    /// Total crew capacity of all active (non-decoupled, non-destroyed) pods.
    pub fn total_crew_capacity(&self, part_defs: &PartDefinitions) -> u32 {
        self.parts.iter()
            .filter(|p| !p.decoupled && !p.destroyed)
            .filter_map(|p| {
                part_defs.get(&p.definition_id)
                    .and_then(|d| d.pod.as_ref())
                    .map(|pod| pod.crew_capacity)
            })
            .sum()
    }

    /// Create a flight vessel from a blueprint
    pub fn from_blueprint(
        blueprint: &VesselBlueprint,
        part_defs: &PartDefinitions,
        spawn_position: [f64; 2],
        spawn_velocity: [f64; 2],
        soi_body: usize,
    ) -> Result<Self, String> {
        blueprint.validate()?;

        let mut parts = Vec::new();
        let mut total_mass = 0.0;
        let mut dry_mass = 0.0;
        let mut center_of_mass = [0.0, 0.0];
        let mut max_thrust_vac = 0.0;
        let mut max_thrust_asl = 0.0;

        // First pass: create parts and calculate total mass
        for bp_part in &blueprint.parts {
            let def = part_defs.get(&bp_part.definition_id)
                .ok_or_else(|| format!("Unknown part: {}", bp_part.definition_id))?;

            dry_mass += def.mass;

            // Sum thrust from engines
            if let Some(ref engine) = def.engine {
                max_thrust_vac += engine.thrust_vac;
                max_thrust_asl += engine.thrust_asl;
            }

            // Create flight part with fuel loaded from blueprint state
            let mut resources = def.resources.clone();
            let mut max_resources = def.resources.clone();

            if let Some(ref tank) = def.tank {
                let fill = if bp_part.fill_fraction > 0.0 {
                    bp_part.fill_fraction
                } else if bp_part.tank_filled {
                    1.0
                } else {
                    0.0
                };
                if fill > 0.0 && bp_part.fuel_type != FuelType::Empty {
                    let (ox_kg, fuel_kg) = tank.propellant_capacity(bp_part.fuel_type);
                    if let Some(fuel_name) = bp_part.fuel_type.fuel_resource_name() {
                        if ox_kg > 0.0 {
                            resources.insert("oxygen".to_string(), ox_kg * fill);
                            max_resources.insert("oxygen".to_string(), ox_kg);
                        }
                        resources.insert(fuel_name.to_string(), fuel_kg * fill);
                        max_resources.insert(fuel_name.to_string(), fuel_kg);
                    }
                }
            }

            // Load cargo container resources from blueprint
            if def.cargo.is_some() {
                for (res_name, amount) in &bp_part.cargo_resources {
                    resources.insert(res_name.clone(), *amount);
                }
            }

            // Calculate part mass including fuel and cargo
            let cargo_building_mass_kg: f64 = bp_part.cargo_buildings.iter()
                .filter_map(|name| crate::colony::BuildingType::from_display_name(name))
                .map(|bt| bt.total_build_mass())
                .sum();
            let cargo_payload_mass_kg: f64 = bp_part.cargo_payloads.iter()
                .map(|p| p.mass_kg)
                .sum();
            let resource_mass_kg: f64 = resources.values().sum();
            let part_mass = def.mass + (resource_mass_kg + cargo_building_mass_kg + cargo_payload_mass_kg) * 0.001; // kg -> tonnes
            total_mass += part_mass;

            // Weight center of mass by part mass
            center_of_mass[0] += bp_part.position[0] * part_mass;
            center_of_mass[1] += bp_part.position[1] * part_mass;

            // Electricity from battery data (starts fully charged)
            let max_elec = def.battery.as_ref().map(|b| b.capacity_wh).unwrap_or(0.0);

            // Create flight part
            let mut flight_part = FlightPart {
                definition_id: bp_part.definition_id.clone(),
                local_position: bp_part.position,
                rotation: bp_part.rotation,
                hitbox_half_extents: [def.flight_hitbox_width_m() / 2.0, def.flight_hitbox_height_m() / 2.0],
                hitbox_y_offset: if def.engine.is_some() {
                    // Engines are top-aligned: shift hitbox up so top aligns with editor hitbox top
                    (def.hitbox_height() - def.flight_hitbox_height_m()) / 2.0
                } else {
                    0.0
                },
                resources,
                max_resources,
                engine_active: false,
                engine_enabled: false,
                engine_thrust_vac: 0.0,
                engine_thrust_asl: 0.0,
                engine_isp_vac: 0.0,
                engine_isp_asl: 0.0,
                is_throttleable: true,
                propellant_type: None,
                secondary_propellant_type: None,
                secondary_fuel_fraction: 0.0,
                mass_flow_rate: 0.0,
                gimbal_angle: 0.0,
                gimbal_range_rad: 0.0,
                rcs_thrust: 0.0,
                rcs_torque_multiplier: 1.0,
                rcs_isp: 0.0,
                rcs_mass_flow_rate: 0.0,
                destroyed: false,
                decoupled: false,
                crossfeed_enabled: bp_part.crossfeed_enabled,
                temperature: 300.0,
                max_heat_tolerance: def.max_heat_tolerance,
                fairing_shape: bp_part.fairing_shape.clone(),
                fairing_half: None,
                electricity: max_elec,
                max_electricity: max_elec,
                is_solar_panel: def.solar_panel.is_some(),
                deploy_fraction: 0.0,
                deploy_target: false,
                mirror_partner: None,
                is_radiator: def.radiator.is_some(),
                radiator_rejection_watts: def.radiator.as_ref().map(|r| r.rejection_watts).unwrap_or(0.0),
                radiator_deploy_time_sec: def.radiator.as_ref().map(|r| r.deploy_time_sec).unwrap_or(5.0),
                is_parachute: def.parachute.is_some(),
                parachute_deployed: false,
                parachute_spent: false,
                parachute_deploy_fraction: 0.0,
                parachute_deployed_width_m: def.parachute.as_ref()
                    .map(|p| p.deployed_width * crate::parts::definition::GRID_SQUARE_SIZE)
                    .unwrap_or(0.0),
                parachute_fully_deployed: false,
                cargo_buildings: bp_part.cargo_buildings.clone(),
                cargo_payloads: bp_part.cargo_payloads.clone(),
                engine_no_power: false,
                shield_active: false,
                crew_count: def.pod.as_ref().map(|p| p.crew_capacity).unwrap_or(0),
            };

            // Set engine data if this is an engine
            if let Some(ref engine) = def.engine {
                flight_part.engine_active = false;
                flight_part.engine_thrust_vac = engine.thrust_vac;
                flight_part.engine_thrust_asl = engine.thrust_asl;
                flight_part.engine_isp_vac = engine.isp_vac;
                flight_part.engine_isp_asl = engine.isp_asl;
                flight_part.is_throttleable = engine.throttleable;
                flight_part.propellant_type = Some(engine.propellant);
                flight_part.secondary_propellant_type = engine.secondary_propellant;
                flight_part.secondary_fuel_fraction = engine.secondary_fuel_fraction;
                flight_part.gimbal_range_rad = engine.gimbal_range.to_radians();
                // m_dot = F / (g0 * Isp), F in Newtons = kN * 1000
                let g0 = 9.80665;
                flight_part.mass_flow_rate = if engine.isp_vac > 0.0 {
                    (engine.thrust_vac * 1000.0) / (g0 * engine.isp_vac)
                } else {
                    0.0
                };
            }

            // Set RCS data if this is an RCS thruster
            if let Some(ref rcs) = def.rcs {
                flight_part.rcs_thrust = rcs.thrust;
                flight_part.rcs_torque_multiplier = rcs.torque_multiplier.unwrap_or(1.0);
                flight_part.rcs_isp = rcs.isp;
                let g0 = 9.80665;
                flight_part.rcs_mass_flow_rate = if rcs.isp > 0.0 {
                    (rcs.thrust * 1000.0) / (g0 * rcs.isp)
                } else {
                    0.0
                };
            }

            parts.push(flight_part);
        }

        // Map mirror partners from blueprint indices to FlightPart indices (1:1 mapping)
        for (i, bp_part) in blueprint.parts.iter().enumerate() {
            if let Some(mirror_idx) = bp_part.mirror_partner_index {
                if mirror_idx < parts.len() {
                    parts[i].mirror_partner = Some(mirror_idx);
                }
            }
        }

        // Normalize center of mass
        if total_mass > 0.0 {
            center_of_mass[0] /= total_mass;
            center_of_mass[1] /= total_mass;
        }

        // Second pass: shift part positions relative to center of mass
        for part in &mut parts {
            part.local_position[0] -= center_of_mass[0];
            part.local_position[1] -= center_of_mass[1];
        }

        // Calculate moment of inertia (simplified: sum of m*r^2)
        let mut moment_of_inertia = 0.0;
        for (i, bp_part) in blueprint.parts.iter().enumerate() {
            let Some(def) = part_defs.get(&bp_part.definition_id) else { continue };
            // Use actual part mass (dry + fuel + cargo buildings)
            let base_mass = def.mass;
            let resource_mass: f64 = parts[i].resources.values().sum::<f64>() * 0.001;
            let part_mass = base_mass + resource_mass + parts[i].cargo_extra_mass_tonnes();
            let r_sq = parts[i].local_position[0].powi(2) + parts[i].local_position[1].powi(2);
            moment_of_inertia += part_mass * r_sq;

            // Add part's own moment of inertia (approximate as rectangle)
            let w = def.width();
            let h = def.height();
            moment_of_inertia += part_mass * (w * w + h * h) / 12.0;
        }

        // Minimum moment of inertia for stability
        moment_of_inertia = moment_of_inertia.max(0.1);

        // Convert blueprint stages to part indices
        let stages = blueprint.stages.clone();

        let initial_crew: u32 = parts.iter().map(|p| p.crew_count).sum();
        let initial_food = initial_crew as f64 * 1.5 * 30.0;

        Ok(FlightVessel {
            rel_position: spawn_position,
            rel_velocity: spawn_velocity,
            rotation: 0.0,
            rotational_velocity: 0.0,
            soi_body,
            parts,
            root_part_index: blueprint.root_part_index,
            total_mass,
            dry_mass,
            center_of_mass: [0.0, 0.0], // Now at origin after shifting
            max_thrust_vac,
            max_thrust_asl,
            moment_of_inertia,
            throttle: 0.0,
            on_rails: false,
            stages,
            current_stage: 0,
            last_decouple_force: 0.0,
            extra_dry_mass_tonnes: 0.0,
            thermal_pool_temp: 300.0,
            thermal_pool_capacity: (dry_mass * 1000.0).max(1.0) * 500.0,
            reactors_tripped: false,
            total_crew: initial_crew,
            food_stored: initial_food,
            starvation_timer: 0.0,
        })
    }

    /// Recalculate mass, center of mass, and moment of inertia
    /// (call after resource consumption or staging)
    pub fn recalculate_mass(&mut self, part_defs: &PartDefinitions) {
        self.total_mass = 0.0;
        self.dry_mass = 0.0;
        let mut com = [0.0, 0.0];

        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }

            let def = part_defs.get(&part.definition_id);
            let base_mass = def.map(|d| d.mass).unwrap_or(0.0);
            let resource_mass: f64 = part.resources.values().sum::<f64>() * 0.001;
            let part_mass = base_mass + resource_mass + part.cargo_extra_mass_tonnes();

            self.total_mass += part_mass;
            self.dry_mass += base_mass;
            com[0] += part.local_position[0] * part_mass;
            com[1] += part.local_position[1] * part_mass;
        }

        // Ship-wide thermal pool capacity: dry mass × average specific heat.
        // 500 J/(kg·K) is a rough average for metallic spacecraft structure.
        self.thermal_pool_capacity = (self.dry_mass * 1000.0).max(1.0) * 500.0;

        if self.total_mass > 0.0 {
            self.center_of_mass[0] = com[0] / self.total_mass;
            self.center_of_mass[1] = com[1] / self.total_mass;
        }

        // Recalculate moment of inertia with current masses
        let mut moi = 0.0;
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            let def = match part_defs.get(&part.definition_id) {
                Some(d) => d,
                None => continue,
            };
            let base_mass = def.mass;
            let resource_mass: f64 = part.resources.values().sum::<f64>() * 0.001;
            let part_mass = base_mass + resource_mass + part.cargo_extra_mass_tonnes();

            let r_sq = part.local_position[0].powi(2) + part.local_position[1].powi(2);
            moi += part_mass * r_sq;

            let w = def.width();
            let h = def.height();
            moi += part_mass * (w * w + h * h) / 12.0;
        }
        self.moment_of_inertia = moi.max(0.1);
    }

    /// Recenter all part local_positions around the new center of mass after decoupling.
    /// Returns the COM offset in vessel-local coordinates (before rotation) so the caller
    /// can shift the ship's world position to keep the vessel in the same physical location.
    pub fn recenter_on_com(&mut self, part_defs: &PartDefinitions) -> [f64; 2] {
        // First, recalculate mass to get the current COM
        self.recalculate_mass(part_defs);

        let com = self.center_of_mass;
        if com[0].abs() < 1e-9 && com[1].abs() < 1e-9 {
            return [0.0, 0.0]; // Already centered
        }

        // Shift all part positions so COM is at origin
        for part in &mut self.parts {
            part.local_position[0] -= com[0];
            part.local_position[1] -= com[1];
        }

        // Reset COM to origin and recalculate MOI with new positions
        self.center_of_mass = [0.0, 0.0];
        let mut moi = 0.0;
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            let def = match part_defs.get(&part.definition_id) {
                Some(d) => d,
                None => continue,
            };
            let base_mass = def.mass;
            let resource_mass: f64 = part.resources.values().sum::<f64>() * 0.001;
            let part_mass = base_mass + resource_mass + part.cargo_extra_mass_tonnes();

            let r_sq = part.local_position[0].powi(2) + part.local_position[1].powi(2);
            moi += part_mass * r_sq;
            let w = def.width();
            let h = def.height();
            moi += part_mass * (w * w + h * h) / 12.0;
        }
        self.moment_of_inertia = moi.max(0.1);

        // Return the old COM offset so the ship position can be corrected
        com
    }

    /// Check if an engine is covered by a non-decoupled decoupler directly below it.
    /// A covered engine has a decoupler whose top edge is near/touching the engine's bottom
    /// edge and whose horizontal extent overlaps the engine's.
    pub fn is_engine_covered(&self, engine_idx: usize, part_defs: &PartDefinitions) -> bool {
        let engine = &self.parts[engine_idx];
        let engine_bottom = engine.local_position[1] - engine.hitbox_half_extents[1];
        let engine_left = engine.local_position[0] - engine.hitbox_half_extents[0];
        let engine_right = engine.local_position[0] + engine.hitbox_half_extents[0];

        for (i, part) in self.parts.iter().enumerate() {
            if i == engine_idx || part.destroyed || part.decoupled {
                continue;
            }
            let Some(def) = part_defs.get(&part.definition_id) else { continue };
            if def.decoupler.is_none() {
                continue;
            }
            let decoupler_top = part.local_position[1] + part.hitbox_half_extents[1];
            let decoupler_left = part.local_position[0] - part.hitbox_half_extents[0];
            let decoupler_right = part.local_position[0] + part.hitbox_half_extents[0];

            // Vertical adjacency: decoupler top near engine bottom
            if (decoupler_top - engine_bottom).abs() < 0.3
                && engine_left < decoupler_right
                && engine_right > decoupler_left
            {
                return true;
            }
        }
        false
    }

    /// Update engine_active flags based on fuel availability within each fuel zone.
    /// Engines without their required propellant type are deactivated.
    pub fn update_engine_states(&mut self, part_defs: &PartDefinitions) {
        let zones = self.compute_fuel_zones(part_defs);

        for i in 0..self.parts.len() {
            if self.parts[i].destroyed || self.parts[i].decoupled {
                continue;
            }
            let propellant = match self.parts[i].propellant_type {
                Some(p) => p,
                None => continue,
            };

            // User-disabled engines are always inactive
            if !self.parts[i].engine_enabled {
                self.parts[i].engine_active = false;
                continue;
            }

            // Engines covered by a decoupler below cannot fire
            if self.is_engine_covered(i, part_defs) {
                self.parts[i].engine_active = false;
                continue;
            }

            let fuel_type = propellant.fuel_type();
            let fuel_name = match fuel_type.fuel_resource_name() {
                Some(n) => n,
                None => {
                    self.parts[i].engine_active = false;
                    continue;
                }
            };

            let engine_zone = zones[i];
            let zone_resource = |name: &str| -> f64 {
                self.parts.iter()
                    .enumerate()
                    .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == engine_zone)
                    .filter_map(|(_, p)| p.resources.get(name))
                    .sum()
            };

            let fuel_available = zone_resource(fuel_name);
            let (ox_per_sq, _) = fuel_type.propellant_per_grid_square();
            let needs_oxygen = ox_per_sq > 0.0;
            let ox_ok = !needs_oxygen || zone_resource("oxygen") > 0.001;

            // Check secondary propellant availability
            let secondary_ok = match self.parts[i].secondary_propellant_type {
                Some(sec) => {
                    let sec_fuel_type = sec.fuel_type();
                    match sec_fuel_type.fuel_resource_name() {
                        Some(sec_name) => zone_resource(sec_name) > 0.001,
                        None => false,
                    }
                }
                None => true,
            };

            let fuel_ok = fuel_available > 0.001 && ox_ok && secondary_ok;
            if fuel_ok {
                let power_req = part_defs.get(&self.parts[i].definition_id)
                    .and_then(|d| d.engine.as_ref())
                    .map(|e| e.power_required)
                    .unwrap_or(0.0);
                if power_req > 0.0 && self.total_electricity() <= 0.0 {
                    self.parts[i].engine_active = false;
                    self.parts[i].engine_no_power = true;
                } else {
                    self.parts[i].engine_active = true;
                    self.parts[i].engine_no_power = false;
                }
            } else {
                self.parts[i].engine_active = false;
                self.parts[i].engine_no_power = false;
            }
        }
    }

    /// Max vacuum thrust from currently active (fueled) engines
    pub fn active_thrust_vac(&self) -> f64 {
        self.parts.iter()
            .filter(|p| p.engine_active && !p.destroyed && !p.decoupled)
            .map(|p| p.engine_thrust_vac)
            .sum()
    }

    /// Max thrust at a given atmospheric pressure (0.0 = vacuum, 1.0 = sea level)
    /// Does NOT apply throttle — returns max capability.
    pub fn active_thrust_at_pressure(&self, pressure: f64) -> f64 {
        self.parts.iter()
            .filter(|p| p.engine_active && !p.destroyed && !p.decoupled)
            .map(|p| p.engine_thrust_vac * (1.0 - pressure) + p.engine_thrust_asl * pressure)
            .sum()
    }

    /// Max sea-level thrust from currently active (fueled) engines
    pub fn active_thrust_asl(&self) -> f64 {
        self.parts.iter()
            .filter(|p| p.engine_active && !p.destroyed && !p.decoupled)
            .map(|p| p.engine_thrust_asl)
            .sum()
    }

    /// Set gimbal angles on all engines based on rotation input.
    /// When rotating left, engines gimbal to create positive (CCW) torque.
    /// When rotating right, engines gimbal to create negative (CW) torque.
    pub fn update_gimbal(&mut self, gimbal_command: f64) {
        let command = gimbal_command.clamp(-1.0, 1.0);
        for part in &mut self.parts {
            if part.destroyed || part.decoupled || part.gimbal_range_rad <= 0.0 {
                continue;
            }
            if !part.engine_active {
                part.gimbal_angle = 0.0;
                continue;
            }
            part.gimbal_angle = command * part.gimbal_range_rad;
        }
    }

    /// Compute net torque from gimbaled engines (kN·m).
    ///
    /// In the vessel's local frame, the structural axis is Y (parts stacked
    /// vertically in the editor) while the physics thrust axis is X. The
    /// nominal (un-gimbaled) engine thrust is along the vessel axis and
    /// produces no torque for centered engines. Gimbal deflection θ rotates
    /// the thrust vector, creating torque:
    ///
    ///   τ = F · [px·(cos θ - 1) - py·sin θ]
    ///
    /// where F = engine_thrust_vac × throttle (kN), and (px, py) is the
    /// engine's local position relative to COM (meters).
    pub fn compute_gimbal_torque(&self) -> f64 {
        let mut torque = 0.0;
        for part in &self.parts {
            if part.destroyed || part.decoupled || !part.engine_active {
                continue;
            }
            if part.gimbal_angle.abs() < 1e-6 || part.gimbal_range_rad <= 0.0 {
                continue;
            }
            let thrust = part.engine_thrust_vac * self.throttle;
            if thrust <= 0.0 {
                continue;
            }
            let px = part.local_position[0];
            let py = part.local_position[1];
            let theta = part.gimbal_angle;
            torque += thrust * (px * (theta.cos() - 1.0) - py * theta.sin());
        }
        torque
    }

    /// Compute maximum possible gimbal torque (kN·m) assuming full deflection.
    /// Used by autopilot to know rotation capability before gimbal is deflected.
    pub fn compute_max_gimbal_torque(&self) -> f64 {
        let mut torque = 0.0;
        for part in &self.parts {
            if part.destroyed || part.decoupled || !part.engine_active {
                continue;
            }
            if part.gimbal_range_rad <= 0.0 {
                continue;
            }
            let thrust = part.engine_thrust_vac * self.throttle;
            if thrust <= 0.0 {
                continue;
            }
            let py = part.local_position[1];
            // Dominant torque term at full deflection: F * |py| * sin(max_angle)
            torque += (thrust * py * part.gimbal_range_rad.sin()).abs();
        }
        torque
    }

    /// Compute total available RCS torque (kN·m) from all non-decoupled RCS thrusters
    /// that have monopropellant available in their fuel zone.
    /// Torque = sum of (thrust * lever_arm) where lever_arm = distance from COM (min 0.25m).
    pub fn compute_rcs_torque(&self, part_defs: &PartDefinitions) -> f64 {
        let zones = self.compute_fuel_zones(part_defs);
        let mut torque = 0.0;

        for (i, part) in self.parts.iter().enumerate() {
            if part.destroyed || part.decoupled || part.rcs_thrust <= 0.0 {
                continue;
            }

            // Check if monopropellant is available in this part's fuel zone
            let zone = zones[i];
            if zone == usize::MAX {
                continue;
            }
            let monoprop_available: f64 = self.parts.iter()
                .enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone)
                .filter_map(|(_, p)| p.resources.get("monopropellant"))
                .sum();
            if monoprop_available < 0.001 {
                continue;
            }

            // Lever arm = distance from COM (min 0.25m to always provide some torque)
            let lever_arm = (part.local_position[0].powi(2) + part.local_position[1].powi(2))
                .sqrt()
                .max(0.25);
            torque += part.rcs_thrust * part.rcs_torque_multiplier * lever_arm;
        }

        torque
    }

    /// Compute total available RCS translation force (kN) from all non-decoupled RCS thrusters
    /// that have monopropellant available in their fuel zone.
    pub fn compute_rcs_translation_force(&self, part_defs: &PartDefinitions) -> f64 {
        let zones = self.compute_fuel_zones(part_defs);
        let mut force = 0.0;

        for (i, part) in self.parts.iter().enumerate() {
            if part.destroyed || part.decoupled || part.rcs_thrust <= 0.0 {
                continue;
            }

            let zone = zones[i];
            if zone == usize::MAX {
                continue;
            }
            let monoprop_available: f64 = self.parts.iter()
                .enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone)
                .filter_map(|(_, p)| p.resources.get("monopropellant"))
                .sum();
            if monoprop_available < 0.001 {
                continue;
            }

            force += part.rcs_thrust;
        }

        force
    }

    /// Consume monopropellant from RCS thrusters during translation.
    /// `translate` is [forward, right] in vessel-local frame, -1..1 each.
    pub fn consume_rcs_translation_fuel(&mut self, dt: f64, translate: [f64; 2], part_defs: &PartDefinitions) {
        let mag = (translate[0].powi(2) + translate[1].powi(2)).sqrt();
        if mag < 0.001 {
            return;
        }

        let zones = self.compute_fuel_zones(part_defs);
        let mut zone_demands: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();

        for (i, part) in self.parts.iter().enumerate() {
            if part.destroyed || part.decoupled || part.rcs_thrust <= 0.0 {
                continue;
            }
            let zone = zones[i];
            if zone == usize::MAX {
                continue;
            }
            let monoprop_available: f64 = self.parts.iter()
                .enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone)
                .filter_map(|(_, p)| p.resources.get("monopropellant"))
                .sum();
            if monoprop_available < 0.001 {
                continue;
            }

            // Scale consumption by translation magnitude (capped at 1)
            let consumption = part.rcs_mass_flow_rate * mag.min(1.0) * dt;
            *zone_demands.entry(zone).or_insert(0.0) += consumption;
        }

        for (&zone, &drain_amount) in &zone_demands {
            let available: f64 = self.parts.iter()
                .enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone)
                .filter_map(|(_, p)| p.resources.get("monopropellant"))
                .sum();
            if drain_amount > 0.0 && available > 0.0 {
                let drain_frac = (drain_amount / available).min(1.0);
                for (j, part) in self.parts.iter_mut().enumerate() {
                    if part.destroyed || part.decoupled || zones[j] != zone {
                        continue;
                    }
                    if let Some(mp) = part.resources.get_mut("monopropellant") {
                        *mp -= *mp * drain_frac;
                        if *mp < 0.001 {
                            *mp = 0.0;
                        }
                    }
                }
            }
        }
    }

    /// Consume monopropellant from RCS thrusters during rotation.
    /// `direction` is +1.0 (CCW), -1.0 (CW), or 0.0 (no rotation).
    /// Returns actual torque available (kN·m).
    pub fn consume_rcs_fuel(&mut self, dt: f64, direction: f64, part_defs: &PartDefinitions) -> f64 {
        if direction.abs() < 0.001 {
            return 0.0;
        }

        let zones = self.compute_fuel_zones(part_defs);
        let mut total_torque = 0.0;

        // Collect RCS fuel demands per zone
        let mut zone_demands: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();

        for (i, part) in self.parts.iter().enumerate() {
            if part.destroyed || part.decoupled || part.rcs_thrust <= 0.0 {
                continue;
            }
            let zone = zones[i];
            if zone == usize::MAX {
                continue;
            }
            let monoprop_available: f64 = self.parts.iter()
                .enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone)
                .filter_map(|(_, p)| p.resources.get("monopropellant"))
                .sum();
            if monoprop_available < 0.001 {
                continue;
            }

            let lever_arm = (part.local_position[0].powi(2) + part.local_position[1].powi(2))
                .sqrt()
                .max(0.25);
            total_torque += part.rcs_thrust * part.rcs_torque_multiplier * lever_arm;

            // Fuel consumption: mass_flow_rate * dt
            let consumption = part.rcs_mass_flow_rate * dt;
            *zone_demands.entry(zone).or_insert(0.0) += consumption;
        }

        // Drain monopropellant from tanks in each zone
        for (&zone, &drain_amount) in &zone_demands {
            let available: f64 = self.parts.iter()
                .enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone)
                .filter_map(|(_, p)| p.resources.get("monopropellant"))
                .sum();
            if drain_amount > 0.0 && available > 0.0 {
                let drain_frac = (drain_amount / available).min(1.0);
                for (j, part) in self.parts.iter_mut().enumerate() {
                    if part.destroyed || part.decoupled || zones[j] != zone {
                        continue;
                    }
                    if let Some(mp) = part.resources.get_mut("monopropellant") {
                        *mp -= *mp * drain_frac;
                        if *mp < 0.001 {
                            *mp = 0.0;
                        }
                    }
                }
            }
        }

        total_torque
    }

    /// Get thrust at a given atmospheric pressure (0.0 = vacuum, 1.0 = sea level)
    pub fn get_thrust(&self, pressure: f64) -> f64 {
        let mut thrust = 0.0;
        for part in &self.parts {
            if part.engine_active && !part.destroyed && !part.decoupled {
                thrust += part.engine_thrust_vac * (1.0 - pressure)
                    + part.engine_thrust_asl * pressure;
            }
        }
        thrust * self.throttle
    }

    /// Get current specific impulse at given pressure
    pub fn get_isp(&self, pressure: f64) -> f64 {
        let mut total_thrust = 0.0;
        let mut weighted_isp = 0.0;

        for part in &self.parts {
            if part.engine_active && !part.destroyed && !part.decoupled {
                let thrust = part.engine_thrust_vac * (1.0 - pressure)
                    + part.engine_thrust_asl * pressure;
                let isp = part.engine_isp_vac * (1.0 - pressure)
                    + part.engine_isp_asl * pressure;

                total_thrust += thrust;
                weighted_isp += thrust * isp;
            }
        }

        if total_thrust > 0.0 {
            weighted_isp / total_thrust
        } else {
            0.0
        }
    }

    /// Consume fuel per-engine and return actual thrust achieved.
    /// Each engine drains oxidizer and its specific fuel type at the tank ratio,
    /// only from tanks in the same fuel zone.
    /// Engines without available fuel are deactivated.
    pub fn consume_fuel(&mut self, dt: f64, pressure: f64, part_defs: &PartDefinitions) -> f64 {
        // Always update which engines have fuel
        self.update_engine_states(part_defs);

        if self.throttle <= 0.0 {
            return 0.0;
        }

        let zones = self.compute_fuel_zones(part_defs);
        let g0 = 9.80665;

        // Phase 1: Collect per-engine fuel demands
        struct EngineDemand {
            fuel_name: &'static str,
            ox_needed: f64,
            fuel_needed: f64,
            secondary_fuel_name: Option<&'static str>,
            secondary_needed: f64,
            thrust: f64,
            zone: usize,
        }
        let mut engine_demands: Vec<EngineDemand> = Vec::new();

        for (i, part) in self.parts.iter().enumerate() {
            if !part.engine_active || part.destroyed || part.decoupled {
                continue;
            }
            let propellant = match part.propellant_type {
                Some(p) => p,
                None => continue,
            };
            let fuel_type = propellant.fuel_type();
            let fuel_name = match fuel_type.fuel_resource_name() {
                Some(n) => n,
                None => continue,
            };

            // Ox:fuel ratio from the tank chemistry
            let (ox_per_sq, fuel_per_sq) = fuel_type.propellant_per_grid_square();
            let total_per_sq = ox_per_sq + fuel_per_sq;
            if total_per_sq <= 0.0 {
                continue;
            }
            let ox_ratio = ox_per_sq / total_per_sq;
            let fuel_ratio = fuel_per_sq / total_per_sq;

            // Engine thrust and ISP at current pressure
            let engine_thrust = part.engine_thrust_vac * (1.0 - pressure)
                + part.engine_thrust_asl * pressure;
            let engine_isp = part.engine_isp_vac * (1.0 - pressure)
                + part.engine_isp_asl * pressure;
            if engine_isp <= 0.0 {
                continue;
            }

            // Mass flow rate at current throttle: m_dot = F / (g0 * Isp)
            let mass_flow = (engine_thrust * 1000.0) / (g0 * engine_isp);
            let total_consumption = mass_flow * self.throttle * dt;

            // Split between primary and secondary propellant
            let primary_fraction = 1.0 - part.secondary_fuel_fraction;
            let primary_consumption = total_consumption * primary_fraction;

            let (secondary_fuel_name, secondary_needed) = match part.secondary_propellant_type {
                Some(sec) => {
                    let sec_ft = sec.fuel_type();
                    match sec_ft.fuel_resource_name() {
                        Some(name) => (Some(name), total_consumption * part.secondary_fuel_fraction),
                        None => (None, 0.0),
                    }
                }
                None => (None, 0.0),
            };

            engine_demands.push(EngineDemand {
                fuel_name,
                ox_needed: primary_consumption * ox_ratio,
                fuel_needed: primary_consumption * fuel_ratio,
                secondary_fuel_name,
                secondary_needed,
                thrust: engine_thrust,
                zone: zones[i],
            });
        }

        if engine_demands.is_empty() {
            return 0.0;
        }

        // Phase 2: Sum drain amounts per (zone, resource)
        let mut zone_ox_drains: HashMap<usize, f64> = HashMap::new();
        let mut zone_fuel_drains: HashMap<(usize, &str), f64> = HashMap::new();
        let mut total_thrust = 0.0;

        for demand in &engine_demands {
            if demand.ox_needed > 0.0 {
                *zone_ox_drains.entry(demand.zone).or_insert(0.0) += demand.ox_needed;
            }
            *zone_fuel_drains.entry((demand.zone, demand.fuel_name)).or_insert(0.0) += demand.fuel_needed;
            if let Some(sec_name) = demand.secondary_fuel_name {
                *zone_fuel_drains.entry((demand.zone, sec_name)).or_insert(0.0) += demand.secondary_needed;
            }
            total_thrust += demand.thrust * self.throttle;
        }

        // Phase 3: Drain resources from tanks within each zone,
        // prioritizing tanks behind crossfeed decouplers (asparagus/onion staging)
        let priorities = self.compute_drain_priorities(part_defs);

        // Drain oxygen per zone (lowest priority tanks first)
        for (&zone, &ox_drain) in &zone_ox_drains {
            // Find min priority among tanks in this zone that have oxygen
            let min_pri = self.parts.iter().enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone)
                .filter(|(_, p)| p.resources.get("oxygen").copied().unwrap_or(0.0) > 0.0)
                .map(|(j, _)| priorities[j])
                .min()
                .unwrap_or(usize::MAX);

            let ox_available: f64 = self.parts.iter()
                .enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone && priorities[*j] == min_pri)
                .filter_map(|(_, p)| p.resources.get("oxygen"))
                .sum();

            if ox_drain > 0.0 && ox_available > 0.0 {
                let drain_frac = (ox_drain / ox_available).min(1.0);
                for (j, part) in self.parts.iter_mut().enumerate() {
                    if part.destroyed || part.decoupled || zones[j] != zone || priorities[j] != min_pri {
                        continue;
                    }
                    if let Some(ox) = part.resources.get_mut("oxygen") {
                        *ox -= *ox * drain_frac;
                        if *ox < 0.001 {
                            *ox = 0.0;
                        }
                    }
                }
            }
        }

        // Drain each fuel type per zone (lowest priority tanks first)
        for (&(zone, fuel_name), &drain_amount) in &zone_fuel_drains {
            let min_pri = self.parts.iter().enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone)
                .filter(|(_, p)| p.resources.get(fuel_name).copied().unwrap_or(0.0) > 0.0)
                .map(|(j, _)| priorities[j])
                .min()
                .unwrap_or(usize::MAX);

            let available: f64 = self.parts.iter()
                .enumerate()
                .filter(|(j, p)| !p.destroyed && !p.decoupled && zones[*j] == zone && priorities[*j] == min_pri)
                .filter_map(|(_, p)| p.resources.get(fuel_name))
                .sum();

            if drain_amount > 0.0 && available > 0.0 {
                let drain_frac = (drain_amount / available).min(1.0);
                for (j, part) in self.parts.iter_mut().enumerate() {
                    if part.destroyed || part.decoupled || zones[j] != zone || priorities[j] != min_pri {
                        continue;
                    }
                    if let Some(fuel) = part.resources.get_mut(fuel_name) {
                        *fuel -= *fuel * drain_frac;
                        if *fuel < 0.001 {
                            *fuel = 0.0;
                        }
                    }
                }
            }
        }

        total_thrust
    }

    /// Resource names that count as propellant
    const PROPELLANT_RESOURCES: &'static [&'static str] = &["oxygen", "rp1", "methane", "hydrogen", "monopropellant"];

    /// Get total fuel available (kg)
    pub fn get_total_fuel(&self) -> f64 {
        let mut total = 0.0;
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            for &name in Self::PROPELLANT_RESOURCES {
                if let Some(&amount) = part.resources.get(name) {
                    total += amount;
                }
            }
        }
        total
    }

    /// Get max total fuel capacity (kg)
    pub fn get_max_fuel(&self) -> f64 {
        let mut total = 0.0;
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            for &name in Self::PROPELLANT_RESOURCES {
                if let Some(&amount) = part.max_resources.get(name) {
                    total += amount;
                }
            }
        }
        total
    }

    /// Get total monopropellant (current_kg, max_kg) across all non-decoupled parts
    pub fn total_monopropellant(&self) -> (f64, f64) {
        let mut current = 0.0;
        let mut max = 0.0;
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            if let Some(&amount) = part.resources.get("monopropellant") {
                current += amount;
            }
            if let Some(&amount) = part.max_resources.get("monopropellant") {
                max += amount;
            }
        }
        (current, max)
    }

    /// Get fuel accessible to currently active engines (current_kg, max_kg).
    /// Uses fuel zones: only counts fuel in zones that contain an active engine.
    pub fn get_stage_fuel(&self, part_defs: &PartDefinitions) -> (f64, f64) {
        let zones = self.compute_fuel_zones(part_defs);

        // Find zone IDs that contain at least one active, non-destroyed, non-decoupled engine
        let mut active_zones = std::collections::HashSet::new();
        for (i, part) in self.parts.iter().enumerate() {
            if part.destroyed || part.decoupled {
                continue;
            }
            if part.engine_active && part.engine_enabled && part.engine_thrust_vac > 0.0 {
                if zones[i] != usize::MAX {
                    active_zones.insert(zones[i]);
                }
            }
        }

        if active_zones.is_empty() {
            return (0.0, 0.0);
        }

        let mut current = 0.0;
        let mut max = 0.0;
        for (i, part) in self.parts.iter().enumerate() {
            if !active_zones.contains(&zones[i]) {
                continue;
            }
            for &name in Self::PROPELLANT_RESOURCES {
                if let Some(&amount) = part.resources.get(name) {
                    current += amount;
                }
                if let Some(&amount) = part.max_resources.get(name) {
                    max += amount;
                }
            }
        }

        (current, max)
    }

    /// Total electricity stored across all non-decoupled battery parts (Wh)
    pub fn total_electricity(&self) -> f64 {
        self.parts.iter()
            .filter(|p| !p.destroyed && !p.decoupled && p.max_electricity > 0.0)
            .map(|p| p.electricity)
            .sum()
    }

    /// Total electricity capacity across all non-decoupled battery parts (Wh)
    pub fn max_electricity(&self) -> f64 {
        self.parts.iter()
            .filter(|p| !p.destroyed && !p.decoupled)
            .map(|p| p.max_electricity)
            .sum()
    }

    /// Electricity fraction (0.0–1.0), or None if no batteries
    pub fn electricity_fraction(&self) -> Option<f64> {
        let max = self.max_electricity();
        if max > 0.0 {
            Some(self.total_electricity() / max)
        } else {
            None
        }
    }

    /// Sum a single fuel resource across all non-destroyed, non-decoupled parts.
    pub fn sum_resource(&self, name: &str) -> f64 {
        self.parts.iter()
            .filter(|p| !p.destroyed && !p.decoupled)
            .map(|p| p.resources.get(name).copied().unwrap_or(0.0))
            .sum()
    }

    /// Drain `amount` kg of `name` proportionally from any non-destroyed,
    /// non-decoupled part that holds it. Caller is responsible for verifying
    /// availability first — this assumes the drain will succeed and silently
    /// drains whatever is present (clamped at 0).
    fn drain_resource(&mut self, name: &str, amount: f64) {
        let total = self.sum_resource(name);
        if total <= 0.0 || amount <= 0.0 {
            return;
        }
        let ratio = (amount / total).min(1.0);
        for part in self.parts.iter_mut() {
            if part.destroyed || part.decoupled { continue; }
            if let Some(avail) = part.resources.get_mut(name) {
                if *avail > 0.0 {
                    *avail = (*avail - *avail * ratio).max(0.0);
                }
            }
        }
    }

    /// Try to consume one timestep's worth of reactor fuel.
    /// Checks both primary and (optional) secondary fuel availability atomically;
    /// returns false (without draining) if either is insufficient.
    /// Returns true after successfully draining both.
    fn try_consume_reactor_fuel(&mut self, fuel: &ReactorFuelData, dt: f64) -> bool {
        let total = fuel.total_kg_s * dt;
        let primary_needed = total * (1.0 - fuel.secondary_fraction);
        let secondary_needed = total * fuel.secondary_fraction;

        let primary_name = match fuel.primary.fuel_resource_name() {
            Some(n) => n,
            None => return false,
        };
        let secondary_name = fuel.secondary.and_then(|f| f.fuel_resource_name());

        if self.sum_resource(primary_name) < primary_needed {
            return false;
        }
        if let Some(sn) = secondary_name {
            if self.sum_resource(sn) < secondary_needed {
                return false;
            }
        }

        self.drain_resource(primary_name, primary_needed);
        if let Some(sn) = secondary_name {
            self.drain_resource(sn, secondary_needed);
        }
        true
    }

    /// Update power: generate from solar/RTG/alternators, consume from pods.
    /// Distributes net charge/drain proportionally across battery parts.
    /// `sun_distance_m` is ship-to-Sun distance in meters.
    /// Returns (generation_watts, consumption_watts).
    pub fn update_power(&mut self, dt: f64, sun_distance_m: f64, part_defs: &PartDefinitions) -> (f64, f64) {
        const AU_M: f64 = 1.496e11; // 1 AU in meters

        let mut generation = 0.0_f64;
        let mut consumption = 0.0_f64;

        // Phase 1: Fuel-consuming reactors (mutable; runs only if fuel available).
        // Skipped entirely while the thermal cascade has tripped reactors offline.
        // Collect demands first to avoid borrowing both immutably and mutably.
        let fueled_reactor_demands: Vec<(ReactorFuelData, f64)> = if self.reactors_tripped {
            Vec::new()
        } else {
            self.parts.iter()
                .filter(|p| !p.destroyed && !p.decoupled)
                .filter_map(|p| {
                    let def = part_defs.get(&p.definition_id)?;
                    let reactor = def.reactor.as_ref()?;
                    let fuel = reactor.fuel.as_ref()?;
                    Some((fuel.clone(), reactor.output_watts))
                })
                .collect()
        };
        for (fuel_data, output_watts) in &fueled_reactor_demands {
            if self.try_consume_reactor_fuel(fuel_data, dt) {
                generation += output_watts;
            }
            // else: out of fuel, reactor produces nothing this tick
        }

        // Phase 2: All other generation/consumption (read-only over parts).
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            let Some(def) = part_defs.get(&part.definition_id) else { continue };

            // Solar panels: inverse-square scaling from 1 AU, gated by deployment
            if let Some(ref solar) = def.solar_panel {
                let distance_ratio = AU_M / sun_distance_m.max(1.0);
                generation += solar.output_1au * distance_ratio * distance_ratio * part.deploy_fraction;
            }

            // RTG: constant output
            if let Some(ref rtg) = def.rtg {
                generation += rtg.output_watts;
            }

            // Reactor: constant output for fuel-less reactors (fission RTG-style).
            // Fuel-consuming reactors (e.g. antimatter) are handled in Phase 1.
            // Tripped reactors (waste-heat cascade) produce no power.
            if let Some(ref reactor) = def.reactor {
                if reactor.fuel.is_none() && !self.reactors_tripped {
                    generation += reactor.output_watts;
                }
            }

            // Engine alternators: only when engine is active
            if let Some(ref engine) = def.engine {
                if engine.alternator_power > 0.0 && part.engine_active {
                    generation += engine.alternator_power;
                }
            }

            // Pod power draw
            if let Some(ref pod) = def.pod {
                if pod.power_draw > 0.0 {
                    consumption += pod.power_draw;
                }
            }

            // Electric engine power draw (when active)
            if let Some(ref engine) = def.engine {
                if engine.power_required > 0.0 && part.engine_active {
                    consumption += engine.power_required;
                }
            }

            // Shield power draw (when active)
            if let Some(ref shield) = def.shield {
                if shield.power_base_watts > 0.0 && part.shield_active {
                    consumption += shield.power_base_watts;
                }
            }
        }

        // Net energy change in Wh for this timestep
        let net_wh = (generation - consumption) * dt / 3600.0;

        // Distribute across battery parts proportionally
        let max_elec = self.max_electricity();
        if max_elec > 0.0 && net_wh.abs() > 1e-12 {
            for part in &mut self.parts {
                if part.destroyed || part.decoupled || part.max_electricity <= 0.0 {
                    continue;
                }
                let fraction = part.max_electricity / max_elec;
                let part_delta = net_wh * fraction;
                part.electricity = (part.electricity + part_delta).clamp(0.0, part.max_electricity);
            }
        }

        // Auto-deactivate shields when batteries are completely drained
        if net_wh < 0.0 && self.total_electricity() <= 0.0 {
            for part in &mut self.parts {
                if part.destroyed || part.decoupled { continue; }
                part.shield_active = false;
            }
        }

        (generation, consumption)
    }

    /// Update life support: greenhouses produce food, crew consumes it, starvation kills crew.
    pub fn update_life_support(&mut self, dt: f64, part_defs: &PartDefinitions) {
        if self.total_crew == 0 { return; }

        const CREW_FOOD_KG_PER_DAY: f64 = 1.5;
        const STARVATION_GRACE_SECS: f64 = 7.0 * 86400.0;
        const KILL_INTERVAL_SECS: f64 = 86400.0;

        let mut production_kg_per_day = 0.0;
        for part in &self.parts {
            if part.destroyed || part.decoupled { continue; }
            let Some(def) = part_defs.get(&part.definition_id) else { continue };
            if let Some(ref gh) = def.greenhouse {
                let has_power = part.electricity > 0.0
                    || def.pod.as_ref().map(|p| p.power_draw <= 0.0).unwrap_or(true);
                if has_power {
                    production_kg_per_day += gh.food_production_rate;
                }
            }
        }

        let consumption_kg_per_day = self.total_crew as f64 * CREW_FOOD_KG_PER_DAY;
        let net_kg_per_sec = (production_kg_per_day - consumption_kg_per_day) / 86400.0;
        self.food_stored = (self.food_stored + net_kg_per_sec * dt).max(0.0);

        if self.food_stored <= 0.0 {
            self.starvation_timer += dt;
            if self.starvation_timer > STARVATION_GRACE_SECS {
                let excess = self.starvation_timer - STARVATION_GRACE_SECS;
                let kills = (excess / KILL_INTERVAL_SECS) as u32;
                let alive: u32 = self.parts.iter()
                    .filter(|p| !p.destroyed && !p.decoupled)
                    .map(|p| p.crew_count)
                    .sum();
                let already_dead = self.parts.iter()
                    .filter(|p| !p.destroyed && !p.decoupled)
                    .filter_map(|p| {
                        let def = part_defs.get(&p.definition_id)?;
                        let cap = def.pod.as_ref()?.crew_capacity;
                        Some(cap.saturating_sub(p.crew_count))
                    })
                    .sum::<u32>();
                let need_to_kill = kills.saturating_sub(already_dead);
                let mut remaining = need_to_kill.min(alive);
                if remaining > 0 {
                    for part in &mut self.parts {
                        if remaining == 0 { break; }
                        if part.destroyed || part.decoupled || part.crew_count == 0 { continue; }
                        let kill = remaining.min(part.crew_count);
                        part.crew_count -= kill;
                        remaining -= kill;
                    }
                    self.total_crew = self.parts.iter()
                        .filter(|p| !p.destroyed && !p.decoupled)
                        .map(|p| p.crew_count)
                        .sum();
                }
            }
        } else {
            self.starvation_timer = 0.0;
        }
    }

    /// Animate solar panel deployment/retraction. Radiators use the same `deploy_fraction`
    /// field with a slower deploy speed (see `radiator_deploy_time_sec`).
    pub fn update_solar_deploy(&mut self, dt: f64) {
        const SOLAR_DEPLOY_SPEED: f64 = 0.5; // fraction per second (2s full deploy)
        for part in &mut self.parts {
            if part.destroyed || part.decoupled { continue; }
            let target = if part.deploy_target { 1.0 } else { 0.0 };
            if (part.deploy_fraction - target).abs() <= 1e-6 { continue; }
            let speed = if part.is_radiator {
                1.0 / part.radiator_deploy_time_sec.max(0.1)
            } else {
                SOLAR_DEPLOY_SPEED
            };
            if part.deploy_fraction < target {
                part.deploy_fraction = (part.deploy_fraction + speed * dt).min(1.0);
            } else {
                part.deploy_fraction = (part.deploy_fraction - speed * dt).max(0.0);
            }
        }
    }

    // --- Waste-heat thermal pool constants ---

    /// Ambient (and minimum) pool temperature, in Kelvin.
    pub const POOL_AMBIENT_TEMP: f64 = 300.0;
    /// Above this pool temperature, all reactors trip offline.
    pub const REACTOR_TRIP_TEMP: f64 = 1200.0;
    /// Below this, the player may manually re-ignite tripped reactors (hysteresis gap).
    pub const REACTOR_RESTART_TEMP: f64 = 800.0;
    /// Above this, excess pool heat spills into per-part temperatures (parts can melt).
    pub const PART_DAMAGE_TEMP: f64 = 1500.0;
    /// Time constant (seconds) controlling how quickly excess pool heat above
    /// `PART_DAMAGE_TEMP` is bled into per-part temperatures.
    const SPILL_TIME_CONSTANT_SEC: f64 = 10.0;

    /// Update the ship-wide thermal pool: accumulate engine + reactor waste heat,
    /// reject via deployed radiators, trip reactors past the threshold, and spill
    /// excess heat into per-part temperatures past the damage threshold.
    ///
    /// Returns the new pool temperature in Kelvin (for HUD display).
    pub fn update_thermal_pool(
        &mut self,
        dt: f64,
        part_defs: &PartDefinitions,
    ) -> f64 {
        if dt <= 0.0 || self.thermal_pool_capacity <= 0.0 {
            return self.thermal_pool_temp;
        }

        // --- Generation: engines firing at current throttle + non-tripped reactors ---
        let mut gen_w = 0.0_f64;
        for part in &self.parts {
            if part.destroyed || part.decoupled { continue; }
            let Some(def) = part_defs.get(&part.definition_id) else { continue; };

            if let Some(ref engine) = def.engine {
                if part.engine_active && engine.waste_heat_watts > 0.0 {
                    gen_w += engine.waste_heat_watts * self.throttle.clamp(0.0, 1.0);
                }
            }

            if let Some(ref reactor) = def.reactor {
                if reactor.waste_heat_watts > 0.0 && !self.reactors_tripped {
                    // Fuel-consuming reactors only generate waste heat while their fuel is
                    // available, but `update_power` already gated that — here we just
                    // mirror the not-tripped state for both reactor families.
                    gen_w += reactor.waste_heat_watts;
                }
            }
        }

        // --- Rejection: sum of deployed radiator capacities, scaled by deploy_fraction ---
        let mut reject_w = 0.0_f64;
        for part in &self.parts {
            if part.destroyed || part.decoupled { continue; }
            if !part.is_radiator { continue; }
            if part.radiator_rejection_watts <= 0.0 { continue; }
            reject_w += part.radiator_rejection_watts * part.deploy_fraction.clamp(0.0, 1.0);
        }

        // --- Pool temperature integration ---
        let net_w = gen_w - reject_w;
        let d_temp = net_w / self.thermal_pool_capacity * dt;
        self.thermal_pool_temp = (self.thermal_pool_temp + d_temp).max(Self::POOL_AMBIENT_TEMP);

        // --- Cascading trip ---
        if !self.reactors_tripped && self.thermal_pool_temp >= Self::REACTOR_TRIP_TEMP {
            self.reactors_tripped = true;
            log::info!(
                "Waste-heat cascade: reactors tripped at pool temp {:.0}K (gen {:.2} GW, reject {:.2} GW)",
                self.thermal_pool_temp, gen_w * 1e-9, reject_w * 1e-9,
            );
        }

        // --- Per-part spillover past damage threshold ---
        if self.thermal_pool_temp > Self::PART_DAMAGE_TEMP {
            // Convert temperature overage back into wattage, drained on a time constant.
            let excess_temp = self.thermal_pool_temp - Self::PART_DAMAGE_TEMP;
            let spill_w = excess_temp * self.thermal_pool_capacity / Self::SPILL_TIME_CONSTANT_SEC;

            // Total thermal mass to distribute against (kg).
            let mut total_thermal_mass = 0.0_f64;
            for part in &self.parts {
                if part.destroyed || part.decoupled { continue; }
                let Some(def) = part_defs.get(&part.definition_id) else { continue; };
                total_thermal_mass += def.width() * Self::SKIN_THERMAL_MASS_PER_METER;
            }
            if total_thermal_mass > 0.0 {
                for part in &mut self.parts {
                    if part.destroyed || part.decoupled { continue; }
                    let Some(def) = part_defs.get(&part.definition_id) else { continue; };
                    let part_thermal_mass = def.width() * Self::SKIN_THERMAL_MASS_PER_METER;
                    if part_thermal_mass <= 0.0 { continue; }
                    let share = part_thermal_mass / total_thermal_mass;
                    let part_dq = spill_w * share * dt;
                    let d_temp_part = part_dq / (part_thermal_mass * def.specific_heat);
                    part.temperature = (part.temperature + d_temp_part)
                        .max(Self::POOL_AMBIENT_TEMP);
                }
                // Remove that energy from the pool so it doesn't double-count.
                let drained = spill_w * dt;
                let d_temp_pool = drained / self.thermal_pool_capacity;
                self.thermal_pool_temp = (self.thermal_pool_temp - d_temp_pool)
                    .max(Self::POOL_AMBIENT_TEMP);
            }
        }

        self.thermal_pool_temp
    }

    /// Manually re-ignite tripped reactors. Returns `true` if the restart succeeded.
    /// Requires the pool temperature to be below `REACTOR_RESTART_TEMP` and consumes
    /// `cost_wh` from stored battery electricity.
    pub fn restart_reactors(&mut self, cost_wh: f64) -> bool {
        if !self.reactors_tripped { return false; }
        if self.thermal_pool_temp > Self::REACTOR_RESTART_TEMP { return false; }
        // Atomic check + drain across all batteries.
        let available = self.total_electricity();
        if available < cost_wh { return false; }
        // Drain proportionally (mirrors update_power's distribution).
        if cost_wh > 0.0 {
            let max_elec = self.max_electricity();
            if max_elec > 0.0 {
                for part in &mut self.parts {
                    if part.destroyed || part.decoupled || part.max_electricity <= 0.0 { continue; }
                    let fraction = part.max_electricity / max_elec;
                    part.electricity = (part.electricity - cost_wh * fraction).max(0.0);
                }
            }
        }
        self.reactors_tripped = false;
        log::info!("Reactors re-ignited (pool at {:.0}K)", self.thermal_pool_temp);
        true
    }

    /// Animate parachute deployment and update full-deployment state based on altitude
    pub fn update_parachute_deploy(&mut self, dt: f64, altitude: f64) {
        const DEPLOY_SPEED: f64 = 1.0; // fraction per second (1s full deploy)
        for part in &mut self.parts {
            if !part.is_parachute || part.destroyed || part.decoupled { continue; }
            let target = if part.parachute_deployed { 1.0 } else { 0.0 };
            if (part.parachute_deploy_fraction - target).abs() > 1e-6 {
                if part.parachute_deploy_fraction < target {
                    part.parachute_deploy_fraction = (part.parachute_deploy_fraction + DEPLOY_SPEED * dt).min(1.0);
                } else {
                    part.parachute_deploy_fraction = (part.parachute_deploy_fraction - DEPLOY_SPEED * dt).max(0.0);
                }
            }
            // Full deployment at or below 2000m altitude
            if part.parachute_deployed {
                part.parachute_fully_deployed = altitude <= 2000.0;
            }
        }
    }

    /// Auto-retract parachutes when leaving atmosphere or landing, marking them spent
    pub fn auto_retract_parachutes(&mut self, in_atmosphere: bool, is_landed: bool) {
        for part in &mut self.parts {
            if !part.is_parachute || part.destroyed || part.decoupled { continue; }
            if part.parachute_deployed && (!in_atmosphere || is_landed) {
                part.parachute_deployed = false;
                part.parachute_spent = true;
                // Instant visual retraction on landing (skip animation)
                if is_landed {
                    part.parachute_deploy_fraction = 0.0;
                }
            }
        }
    }

    /// Total parachute drag width in meters (sum of deployed canopies scaled by deploy fraction)
    pub fn parachute_drag_width(&self) -> f64 {
        let mut total = 0.0;
        for part in &self.parts {
            if part.is_parachute && part.parachute_deployed && !part.destroyed && !part.decoupled {
                total += part.parachute_deployed_width_m * part.parachute_deploy_fraction;
            }
        }
        total
    }

    /// Drag multiplier for parachutes: 100x when any deployed chute is fully deployed (<=2000m), 1x otherwise
    pub fn parachute_drag_multiplier(&self) -> f64 {
        for part in &self.parts {
            if part.is_parachute && part.parachute_deployed && !part.destroyed && !part.decoupled
                && part.parachute_fully_deployed
            {
                return 100.0;
            }
        }
        1.0
    }

    /// Get the bounding half-width of the vessel (max extent from COM in X)
    pub fn bounding_half_width(&self) -> f64 {
        let mut max_extent = 0.0f64;
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            let half_w = if part.is_solar_panel {
                part.hitbox_half_extents[0] * part.deploy_fraction
            } else {
                part.hitbox_half_extents[0]
            };
            let right = part.local_position[0] + half_w;
            let left = part.local_position[0] - half_w;
            max_extent = max_extent.max(right.abs()).max(left.abs());
        }
        max_extent.max(0.5)
    }

    /// Get the bounding half-height of the vessel (max extent from COM in Y)
    pub fn bounding_half_height(&self) -> f64 {
        let mut max_extent = 0.0f64;
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            let half_h = if part.is_solar_panel {
                part.hitbox_half_extents[1] * part.deploy_fraction
            } else {
                part.hitbox_half_extents[1]
            };
            let center_y = part.local_position[1] + part.hitbox_y_offset;
            let top = center_y + half_h;
            let bottom = center_y - half_h;
            max_extent = max_extent.max(top.abs()).max(bottom.abs());
        }
        max_extent.max(1.0)
    }

    /// Distance from COM to the bottom of the vessel (most negative Y extent).
    /// Used for placing the vessel on a surface so the engine touches the ground.
    pub fn bottom_extent(&self) -> f64 {
        let mut min_y = 0.0f64;
        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }
            let bottom = part.local_position[1] + part.hitbox_y_offset - part.hitbox_half_extents[1];
            min_y = min_y.min(bottom);
        }
        -min_y
    }

    /// Check if any part collides with terrain (sphere at origin with given radius).
    /// vessel_pos is the vessel center position relative to the body.
    /// vessel_rotation is the vessel's rotation angle.
    /// body_index is the SOI body index (used for launchpad collision).
    /// Returns Some(surface_angle) if collision detected.
    pub fn check_terrain_collision(
        &self,
        vessel_pos: [f64; 2],
        vessel_rotation: f64,
        body_radius: f64,
        body_index: usize,
        earth_index: usize,
    ) -> Option<f64> {
        use crate::game::{LAUNCHPAD_SURFACE_ANGLE,
                          LAUNCHPAD_HEIGHT, LAUNCHPAD_TOP_WIDTH, LAUNCHPAD_BOTTOM_WIDTH};

        let cos_r = vessel_rotation.cos();
        let sin_r = vessel_rotation.sin();

        // Launchpad collision parameters
        let has_launchpad = body_index == earth_index;
        let lp_top_half = LAUNCHPAD_TOP_WIDTH * 0.5;
        let lp_bot_half = LAUNCHPAD_BOTTOM_WIDTH * 0.5;
        let lp_surface_radius = body_radius + LAUNCHPAD_HEIGHT;

        for part in &self.parts {
            if part.destroyed || part.decoupled {
                continue;
            }

            // Check 4 corners of the hitbox (offset by hitbox_y_offset for top-aligned parts)
            let hx = part.hitbox_half_extents[0];
            let hy = part.hitbox_half_extents[1];
            let center_y = part.local_position[1] + part.hitbox_y_offset;
            let corners = [
                [part.local_position[0] - hx, center_y - hy],
                [part.local_position[0] + hx, center_y - hy],
                [part.local_position[0] + hx, center_y + hy],
                [part.local_position[0] - hx, center_y + hy],
            ];

            for corner in &corners {
                // Rotate corner by vessel rotation
                let world_x = vessel_pos[0] + corner[0] * cos_r - corner[1] * sin_r;
                let world_y = vessel_pos[1] + corner[0] * sin_r + corner[1] * cos_r;

                let dist = (world_x * world_x + world_y * world_y).sqrt();

                // Check ground collision
                if dist < body_radius {
                    return Some(world_y.atan2(world_x));
                }

                // Check launchpad collision
                if has_launchpad && dist < lp_surface_radius {
                    let corner_angle = world_y.atan2(world_x);
                    let angle_diff = corner_angle - LAUNCHPAD_SURFACE_ANGLE;
                    let angle_diff = angle_diff - (angle_diff / std::f64::consts::TAU).round() * std::f64::consts::TAU;
                    // Linear interpolation of width from bottom to top
                    let height_frac = ((dist - body_radius) / LAUNCHPAD_HEIGHT).clamp(0.0, 1.0);
                    let half_width_at_height = lp_bot_half + (lp_top_half - lp_bot_half) * height_frac;
                    let half_angle = half_width_at_height / body_radius;
                    if angle_diff.abs() < half_angle {
                        return Some(corner_angle);
                    }
                }
            }
        }

        None
    }

    /// Find weld connections between parts whose welding hitboxes overlap.
    /// Returns adjacency list: for each part index, the set of part indices it is welded to.
    pub fn find_weld_connections(&self, part_defs: &PartDefinitions) -> Vec<Vec<usize>> {
        let n = self.parts.len();
        let mut connections = vec![Vec::new(); n];

        for i in 0..n {
            if self.parts[i].destroyed || self.parts[i].decoupled {
                continue;
            }
            let Some(def_i) = part_defs.get(&self.parts[i].definition_id) else {
                continue;
            };
            let weld_hw_i = def_i.weld_hitbox_width() / 2.0;
            let weld_hh_i = def_i.weld_hitbox_height() / 2.0;

            for j in (i + 1)..n {
                if self.parts[j].destroyed || self.parts[j].decoupled {
                    continue;
                }
                let Some(def_j) = part_defs.get(&self.parts[j].definition_id) else {
                    continue;
                };
                let weld_hw_j = def_j.weld_hitbox_width() / 2.0;
                let weld_hh_j = def_j.weld_hitbox_height() / 2.0;

                // Check AABB overlap of welding hitboxes
                let dx = (self.parts[i].local_position[0] - self.parts[j].local_position[0]).abs();
                let dy = (self.parts[i].local_position[1] - self.parts[j].local_position[1]).abs();

                if dx < weld_hw_i + weld_hw_j && dy < weld_hh_i + weld_hh_j {
                    connections[i].push(j);
                    connections[j].push(i);
                }
            }
        }

        // Post-process: non-radial decouplers connect upward only via adapter
        let skip: Vec<bool> = self.parts.iter().map(|p| p.destroyed || p.decoupled).collect();
        self.apply_decoupler_adapter_connections(part_defs, &mut connections, &skip);

        connections
    }

    /// Post-process adjacency so non-radial decouplers connect upward only
    /// through their adapter target (closest aligned tank/pod above the ring),
    /// not via welding hitbox overlap. Downward connections remain normal.
    fn apply_decoupler_adapter_connections(
        &self,
        part_defs: &PartDefinitions,
        connections: &mut Vec<Vec<usize>>,
        skip: &[bool],
    ) {
        let n = self.parts.len();
        let tolerance = 0.01;

        for d in 0..n {
            if skip[d] { continue; }
            let Some(def_d) = part_defs.get(&self.parts[d].definition_id) else { continue };
            let Some(ref decoupler) = def_d.decoupler else { continue };
            if decoupler.is_radial { continue; }

            // Ring top in local coordinates (mirrors rendering formula in editor/render.rs)
            let ring_top = self.parts[d].local_position[1]
                - def_d.hitbox_height() / 2.0
                + def_d.height();
            let dec_x = self.parts[d].local_position[0];

            // Find adapter target: closest aligned tank/pod whose bottom >= ring_top
            let mut best_target: Option<usize> = None;
            let mut best_dist = f64::MAX;

            for t in 0..n {
                if skip[t] || t == d { continue; }
                let Some(def_t) = part_defs.get(&self.parts[t].definition_id) else { continue };
                if def_t.tank.is_none() && def_t.pod.is_none() { continue; }
                if (self.parts[t].local_position[0] - dec_x).abs() > tolerance { continue; }

                let target_bottom = self.parts[t].local_position[1] - def_t.hitbox_height() / 2.0;
                if target_bottom < ring_top - tolerance { continue; }

                let dist = target_bottom - ring_top;
                if dist < best_dist {
                    best_dist = dist;
                    best_target = Some(t);
                }
            }

            // Remove all upward welding connections from this decoupler.
            // A neighbor is "above" if its center is above ring_top — even if
            // its welding hitbox extends below ring_top, it should not bridge
            // through the decoupler.
            let upward: Vec<usize> = connections[d].iter().copied()
                .filter(|&nb| self.parts[nb].local_position[1] > ring_top + tolerance)
                .collect();

            for &nb in &upward {
                connections[d].retain(|&x| x != nb);
                connections[nb].retain(|&x| x != d);
            }

            // Add adapter target connection
            if let Some(t) = best_target {
                if !connections[d].contains(&t) {
                    connections[d].push(t);
                }
                if !connections[t].contains(&d) {
                    connections[t].push(d);
                }
            }
        }
    }

    /// Compute fuel zones by flood-filling the weld adjacency graph.
    /// Non-crossfeed decouplers act as barriers: they are visited but don't
    /// propagate fuel flow to their neighbors.
    /// Returns a zone ID per part (usize::MAX for destroyed/decoupled parts).
    pub fn compute_fuel_zones(&self, part_defs: &PartDefinitions) -> Vec<usize> {
        use std::collections::VecDeque;

        let n = self.parts.len();
        let connections = self.find_weld_connections(part_defs);
        let mut zones = vec![usize::MAX; n];
        let mut current_zone = 0;

        for start in 0..n {
            if zones[start] != usize::MAX {
                continue;
            }
            if self.parts[start].destroyed || self.parts[start].decoupled {
                continue;
            }

            let mut queue = VecDeque::new();
            zones[start] = current_zone;
            queue.push_back(start);

            while let Some(idx) = queue.pop_front() {
                // If this part is a non-crossfeed decoupler, assign it to the
                // zone but don't propagate through it.
                let is_barrier = {
                    let def = part_defs.get(&self.parts[idx].definition_id);
                    def.map(|d| d.decoupler.is_some()).unwrap_or(false)
                        && !self.parts[idx].crossfeed_enabled
                };

                if is_barrier {
                    // Barrier parts get visited (assigned a zone) but don't
                    // propagate to neighbors — they might be claimed by
                    // another zone's fill that starts from their other side.
                    continue;
                }

                for &neighbor in &connections[idx] {
                    if zones[neighbor] != usize::MAX {
                        continue;
                    }
                    if self.parts[neighbor].destroyed || self.parts[neighbor].decoupled {
                        continue;
                    }
                    zones[neighbor] = current_zone;
                    queue.push_back(neighbor);
                }
            }

            current_zone += 1;
        }

        zones
    }

    /// Compute drain priority for each part.
    /// Tanks behind crossfeed-enabled decouplers in earlier stages get lower
    /// priority values (drain first). Tanks always reachable from root get
    /// `usize::MAX` (drain last). Used for asparagus/onion staging.
    fn compute_drain_priorities(&self, part_defs: &PartDefinitions) -> Vec<usize> {
        use std::collections::VecDeque;

        let n = self.parts.len();
        let connections = self.find_weld_connections(part_defs);

        // Find root (same logic as decouple_disconnected)
        let root = if self.root_part_index < n
            && !self.parts[self.root_part_index].destroyed
            && !self.parts[self.root_part_index].decoupled
        {
            self.root_part_index
        } else {
            self.parts.iter().enumerate()
                .filter(|(_, p)| !p.destroyed && !p.decoupled)
                .find(|(_, p)| part_defs.get(&p.definition_id).map(|d| d.pod.is_some()).unwrap_or(false))
                .map(|(i, _)| i)
                .or_else(|| self.parts.iter().enumerate()
                    .find(|(_, p)| !p.destroyed && !p.decoupled)
                    .map(|(i, _)| i))
                .unwrap_or(0)
        };

        let mut priorities = vec![usize::MAX; n];

        // For each crossfeed-enabled decoupler in each stage, BFS from root
        // without that decoupler. Parts unreachable get priority = min(current, stage_idx).
        for (stage_idx, stage) in self.stages.iter().enumerate() {
            for &part_idx in stage {
                if part_idx >= n { continue; }
                if self.parts[part_idx].destroyed || self.parts[part_idx].decoupled { continue; }
                let is_crossfeed_decoupler = part_defs.get(&self.parts[part_idx].definition_id)
                    .map(|d| d.decoupler.is_some()).unwrap_or(false)
                    && self.parts[part_idx].crossfeed_enabled;
                if !is_crossfeed_decoupler { continue; }

                // BFS from root, skipping this decoupler
                let mut reached = vec![false; n];
                let mut queue = VecDeque::new();
                if root != part_idx {
                    reached[root] = true;
                    queue.push_back(root);
                }
                while let Some(idx) = queue.pop_front() {
                    for &neighbor in &connections[idx] {
                        if neighbor == part_idx || reached[neighbor] { continue; }
                        if self.parts[neighbor].destroyed || self.parts[neighbor].decoupled { continue; }
                        reached[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }

                // Unreachable parts get priority = min(current, stage_idx)
                for i in 0..n {
                    if !reached[i] && i != part_idx
                        && !self.parts[i].destroyed && !self.parts[i].decoupled
                    {
                        priorities[i] = priorities[i].min(stage_idx);
                    }
                }
            }
        }

        priorities
    }

    /// Calculate delta-v using the Tsiolkovsky rocket equation
    pub fn calculate_delta_v(&self) -> f64 {
        if self.dry_mass <= 0.0 || self.total_mass <= self.dry_mass {
            return 0.0;
        }
        let isp = self.get_isp(0.0); // Vacuum ISP
        if isp <= 0.0 {
            return 0.0;
        }
        let g0 = 9.80665;
        let ve = g0 * isp;
        ve * (self.total_mass / self.dry_mass).ln()
    }

    /// Check collision between a part and a point (for terrain collision)
    pub fn check_part_collision(&self, part_index: usize, world_point: [f64; 2]) -> bool {
        if part_index >= self.parts.len() {
            return false;
        }

        let part = &self.parts[part_index];
        if part.destroyed || part.decoupled {
            return false;
        }

        // Transform world point to part-local coordinates
        let cos_r = self.rotation.cos();
        let sin_r = self.rotation.sin();

        // Vector from vessel to world point
        let dx = world_point[0] - self.rel_position[0];
        let dy = world_point[1] - self.rel_position[1];

        // Rotate to vessel frame
        let local_x = dx * cos_r + dy * sin_r;
        let local_y = -dx * sin_r + dy * cos_r;

        // Offset by part position
        let part_local_x = local_x - part.local_position[0];
        let part_local_y = local_y - part.local_position[1];

        // Check against hitbox (AABB)
        part_local_x.abs() <= part.hitbox_half_extents[0]
            && part_local_y.abs() <= part.hitbox_half_extents[1]
    }

    /// Get world position of a specific part
    pub fn get_part_world_position(&self, part_index: usize) -> [f64; 2] {
        if part_index >= self.parts.len() {
            return self.rel_position;
        }

        let part = &self.parts[part_index];
        let cos_r = self.rotation.cos();
        let sin_r = self.rotation.sin();

        [
            self.rel_position[0] + part.local_position[0] * cos_r - part.local_position[1] * sin_r,
            self.rel_position[1] + part.local_position[0] * sin_r + part.local_position[1] * cos_r,
        ]
    }

    /// Compute fuel zones using simulated decoupled state (for delta-v calculation).
    /// Same algorithm as compute_fuel_zones but uses a provided decoupled vec
    /// instead of reading from part state.
    fn compute_fuel_zones_simulated(
        &self,
        part_defs: &PartDefinitions,
        sim_decoupled: &[bool],
    ) -> Vec<usize> {
        use std::collections::VecDeque;

        let n = self.parts.len();
        // Build adjacency from weld hitbox overlap, skipping simulated-decoupled parts
        let mut connections = vec![Vec::new(); n];
        for i in 0..n {
            if sim_decoupled[i] { continue; }
            let Some(def_i) = part_defs.get(&self.parts[i].definition_id) else { continue };
            let weld_hw_i = def_i.weld_hitbox_width() / 2.0;
            let weld_hh_i = def_i.weld_hitbox_height() / 2.0;
            for j in (i + 1)..n {
                if sim_decoupled[j] { continue; }
                let Some(def_j) = part_defs.get(&self.parts[j].definition_id) else { continue };
                let weld_hw_j = def_j.weld_hitbox_width() / 2.0;
                let weld_hh_j = def_j.weld_hitbox_height() / 2.0;
                let dx = (self.parts[i].local_position[0] - self.parts[j].local_position[0]).abs();
                let dy = (self.parts[i].local_position[1] - self.parts[j].local_position[1]).abs();
                if dx < weld_hw_i + weld_hw_j && dy < weld_hh_i + weld_hh_j {
                    connections[i].push(j);
                    connections[j].push(i);
                }
            }
        }

        // Post-process: non-radial decouplers connect upward only via adapter
        self.apply_decoupler_adapter_connections(part_defs, &mut connections, sim_decoupled);

        // Flood-fill zones; non-crossfeed decouplers are barriers
        let mut zones = vec![usize::MAX; n];
        let mut current_zone = 0;
        for start in 0..n {
            if zones[start] != usize::MAX || sim_decoupled[start] { continue; }
            let mut queue = VecDeque::new();
            zones[start] = current_zone;
            queue.push_back(start);
            while let Some(idx) = queue.pop_front() {
                let is_barrier = {
                    let def = part_defs.get(&self.parts[idx].definition_id);
                    def.map(|d| d.decoupler.is_some()).unwrap_or(false)
                        && !self.parts[idx].crossfeed_enabled
                };
                if is_barrier { continue; }
                for &neighbor in &connections[idx] {
                    if zones[neighbor] != usize::MAX { continue; }
                    zones[neighbor] = current_zone;
                    queue.push_back(neighbor);
                }
            }
            current_zone += 1;
        }
        zones
    }

    /// Compute drain priorities using simulated decoupled state (for delta-v calculation).
    /// Same algorithm as compute_drain_priorities but uses sim_decoupled instead of part state.
    fn compute_drain_priorities_simulated(
        &self,
        part_defs: &PartDefinitions,
        sim_decoupled: &[bool],
    ) -> Vec<usize> {
        use std::collections::VecDeque;

        let n = self.parts.len();

        // Build adjacency (same as compute_fuel_zones_simulated)
        let mut connections = vec![Vec::new(); n];
        for i in 0..n {
            if sim_decoupled[i] { continue; }
            let Some(def_i) = part_defs.get(&self.parts[i].definition_id) else { continue };
            let weld_hw_i = def_i.weld_hitbox_width() / 2.0;
            let weld_hh_i = def_i.weld_hitbox_height() / 2.0;
            for j in (i + 1)..n {
                if sim_decoupled[j] { continue; }
                let Some(def_j) = part_defs.get(&self.parts[j].definition_id) else { continue };
                let weld_hw_j = def_j.weld_hitbox_width() / 2.0;
                let weld_hh_j = def_j.weld_hitbox_height() / 2.0;
                let dx = (self.parts[i].local_position[0] - self.parts[j].local_position[0]).abs();
                let dy = (self.parts[i].local_position[1] - self.parts[j].local_position[1]).abs();
                if dx < weld_hw_i + weld_hw_j && dy < weld_hh_i + weld_hh_j {
                    connections[i].push(j);
                    connections[j].push(i);
                }
            }
        }

        // Post-process: non-radial decouplers connect upward only via adapter
        self.apply_decoupler_adapter_connections(part_defs, &mut connections, sim_decoupled);

        // Find root
        let root = if self.root_part_index < n && !sim_decoupled[self.root_part_index] {
            self.root_part_index
        } else {
            (0..n).filter(|&i| !sim_decoupled[i])
                .find(|&i| part_defs.get(&self.parts[i].definition_id).map(|d| d.pod.is_some()).unwrap_or(false))
                .or_else(|| (0..n).find(|&i| !sim_decoupled[i]))
                .unwrap_or(0)
        };

        let mut priorities = vec![usize::MAX; n];

        for (stage_idx, stage) in self.stages.iter().enumerate() {
            for &part_idx in stage {
                if part_idx >= n || sim_decoupled[part_idx] { continue; }
                let is_crossfeed_decoupler = part_defs.get(&self.parts[part_idx].definition_id)
                    .map(|d| d.decoupler.is_some()).unwrap_or(false)
                    && self.parts[part_idx].crossfeed_enabled;
                if !is_crossfeed_decoupler { continue; }

                // BFS from root, skipping this decoupler
                let mut reached = vec![false; n];
                let mut queue = VecDeque::new();
                if root != part_idx && !sim_decoupled[root] {
                    reached[root] = true;
                    queue.push_back(root);
                }
                while let Some(idx) = queue.pop_front() {
                    for &neighbor in &connections[idx] {
                        if neighbor == part_idx || reached[neighbor] { continue; }
                        if sim_decoupled[neighbor] { continue; }
                        reached[neighbor] = true;
                        queue.push_back(neighbor);
                    }
                }

                for i in 0..n {
                    if !reached[i] && i != part_idx && !sim_decoupled[i] {
                        priorities[i] = priorities[i].min(stage_idx);
                    }
                }
            }
        }

        priorities
    }

    /// Check if an engine is covered by a decoupler using simulated decoupled state.
    /// Used by calculate_stage_delta_v to determine which engines can fire after staging.
    fn is_engine_covered_simulated(
        &self,
        engine_idx: usize,
        part_defs: &PartDefinitions,
        sim_decoupled: &[bool],
    ) -> bool {
        let engine = &self.parts[engine_idx];
        let engine_bottom = engine.local_position[1] - engine.hitbox_half_extents[1];
        let engine_left = engine.local_position[0] - engine.hitbox_half_extents[0];
        let engine_right = engine.local_position[0] + engine.hitbox_half_extents[0];

        for (i, part) in self.parts.iter().enumerate() {
            if i == engine_idx || sim_decoupled[i] {
                continue;
            }
            let Some(def) = part_defs.get(&part.definition_id) else { continue };
            if def.decoupler.is_none() {
                continue;
            }
            let decoupler_top = part.local_position[1] + part.hitbox_half_extents[1];
            let decoupler_left = part.local_position[0] - part.hitbox_half_extents[0];
            let decoupler_right = part.local_position[0] + part.hitbox_half_extents[0];

            if (decoupler_top - engine_bottom).abs() < 0.3
                && engine_left < decoupler_right
                && engine_right > decoupler_left
            {
                return true;
            }
        }
        false
    }

    /// Stefan-Boltzmann constant (W/m^2/K^4)
    const STEFAN_BOLTZMANN: f64 = 5.670374419e-8;

    /// Sutton-Graves convective heating constant for N₂/O₂ atmosphere
    const SUTTON_GRAVES_K: f64 = 1.7415e-4;

    /// Reference thermal mass per meter of exposed width (kg/m).
    /// Models the thin skin layer that absorbs convective heat.
    /// At orbital velocity reentry (~7800 m/s, 80km altitude), gives ~39 K/s
    /// heating rate — destroying 1000K-tolerance parts in ~18 seconds.
    const SKIN_THERMAL_MASS_PER_METER: f64 = 10.0;

    /// Update per-part temperatures from aerodynamic heating and radiative cooling.
    /// `airspeed_dir_local` is the unit vector of airspeed in vessel-local coordinates
    /// (i.e., already rotated by -vessel_rotation).
    pub fn update_part_temperatures(
        &mut self,
        dt: f64,
        density: f64,
        airspeed: f64,
        airspeed_dir_local: [f64; 2],
        part_defs: &PartDefinitions,
    ) {
        if airspeed < 1.0 || density < 1e-15 {
            // No significant heating — just radiative cooling
            for part in &mut self.parts {
                if part.destroyed || part.decoupled { continue; }
                let def = match part_defs.get(&part.definition_id) {
                    Some(d) => d,
                    None => continue,
                };
                if part.temperature > 300.0 {
                    let surface_area = 2.0 * (def.width() + def.height());
                    let q_out = def.emissivity * Self::STEFAN_BOLTZMANN * (part.temperature.powi(4) - 300.0_f64.powi(4)) * surface_area;
                    let thermal_mass_kg = def.width() * Self::SKIN_THERMAL_MASS_PER_METER;
                    if thermal_mass_kg > 0.0 {
                        let d_temp = q_out / (thermal_mass_kg * def.specific_heat) * dt;
                        part.temperature = (part.temperature - d_temp).max(300.0);
                    }
                }
            }
            return;
        }

        // --- 1D interval occlusion along the airspeed axis ---
        // Project each part onto the velocity axis to determine ordering,
        // then compute how much of each part's perpendicular cross-section is exposed.

        // Velocity direction in local frame
        let vx = airspeed_dir_local[0];
        let vy = airspeed_dir_local[1];
        // Perpendicular axis
        let px = -vy;
        let py = vx;

        // For each non-destroyed/decoupled part, compute:
        // - projection_along: position projected onto velocity axis (front = most negative)
        // - perp_interval: [min, max] of the part projected onto perpendicular axis
        // - cross_section_width: width of the part perpendicular to velocity
        struct PartProjection {
            idx: usize,
            proj_along: f64,  // how far along velocity axis (more positive = more forward)
            perp_min: f64,
            perp_max: f64,
            surface_area: f64,
        }

        let mut projections: Vec<PartProjection> = Vec::new();

        for (i, part) in self.parts.iter().enumerate() {
            if part.destroyed || part.decoupled { continue; }
            let def = match part_defs.get(&part.definition_id) {
                Some(d) => d,
                None => continue,
            };

            let half_w = def.width() / 2.0;
            let half_h = def.height() / 2.0;
            let cx = part.local_position[0];
            let cy = part.local_position[1];

            // Project center onto velocity axis
            let proj_along = cx * vx + cy * vy;

            // Part corners projected onto both axes to find extent
            let corners = [
                (cx - half_w, cy - half_h),
                (cx + half_w, cy - half_h),
                (cx + half_w, cy + half_h),
                (cx - half_w, cy + half_h),
            ];

            let mut perp_min = f64::MAX;
            let mut perp_max = f64::MIN;
            for &(cx2, cy2) in &corners {
                let pp = cx2 * px + cy2 * py;
                perp_min = perp_min.min(pp);
                perp_max = perp_max.max(pp);
            }

            let surface_area = 2.0 * (def.width() + def.height());

            projections.push(PartProjection {
                idx: i,
                proj_along,
                perp_min,
                perp_max,
                surface_area,
            });
        }

        // Compute fairing-shielded parts.
        // A part is shielded if it falls within a non-decoupled fairing's envelope.
        let mut fairing_shielded: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for (fi, fpart) in self.parts.iter().enumerate() {
            if fpart.destroyed || fpart.decoupled { continue; }
            let Some(ref shape) = fpart.fairing_shape else { continue; };
            if shape.vertices.is_empty() { continue; }
            let Some(fdef) = part_defs.get(&fpart.definition_id) else { continue; };
            if fdef.fairing.is_none() { continue; }

            let gs = crate::parts::GRID_SQUARE_SIZE;
            let fairing_cx = fpart.local_position[0];
            let fairing_base_top = fpart.local_position[1] + fdef.hitbox_height() / 2.0;
            let base_half_w = fdef.width() / 2.0;

            // Build segment list: [(y_bottom, half_w_bottom, y_top, half_w_top), ...]
            let mut segs: Vec<(f64, f64, f64, f64)> = Vec::new();
            let mut prev_y = fairing_base_top;
            let mut prev_hw = base_half_w;
            for &(hw_grid, y_off_grid) in &shape.vertices {
                let seg_y = fairing_base_top + y_off_grid * gs;
                let seg_hw = hw_grid * gs;
                segs.push((prev_y, prev_hw, seg_y, seg_hw));
                prev_y = seg_y;
                prev_hw = seg_hw;
            }

            // Check each part
            for (pi, ppart) in self.parts.iter().enumerate() {
                if pi == fi || ppart.destroyed || ppart.decoupled { continue; }
                let Some(pdef) = part_defs.get(&ppart.definition_id) else { continue; };
                let part_cx = ppart.local_position[0];
                let part_cy = ppart.local_position[1];
                let part_half_h = pdef.hitbox_height() / 2.0;
                let part_half_w = pdef.hitbox_width() / 2.0;
                let part_bottom = part_cy - part_half_h;
                let part_top = part_cy + part_half_h;

                // Part must be between base top and fairing tip Y
                if part_bottom < fairing_base_top - 0.01 { continue; }
                let tip_y = segs.last().map(|s| s.2).unwrap_or(fairing_base_top);
                if part_top > tip_y + 0.01 { continue; }

                // Check if the part's width fits within the fairing width at the part's center Y
                let mut shielded = true;
                let check_y = part_cy;
                let mut envelope_hw = 0.0_f64;
                let mut found_seg = false;
                for &(y_bot, hw_bot, y_top, hw_top) in &segs {
                    if check_y >= y_bot - 0.01 && check_y <= y_top + 0.01 {
                        let t = if (y_top - y_bot).abs() > 0.001 {
                            (check_y - y_bot) / (y_top - y_bot)
                        } else {
                            0.5
                        };
                        envelope_hw = hw_bot + (hw_top - hw_bot) * t;
                        found_seg = true;
                        break;
                    }
                }
                if !found_seg { shielded = false; }
                if shielded {
                    let dx = (part_cx - fairing_cx).abs();
                    if dx + part_half_w > envelope_hw + 0.01 {
                        shielded = false;
                    }
                }
                if shielded {
                    fairing_shielded.insert(pi);
                }
            }
        }

        // Sort by projection along velocity axis (most forward = most positive first)
        // The leading edge has the largest dot product with the velocity direction
        projections.sort_by(|a, b| b.proj_along.partial_cmp(&a.proj_along).unwrap());

        // Track occluded perpendicular intervals (sorted, non-overlapping)
        let mut occluded: Vec<(f64, f64)> = Vec::new();

        for proj in &projections {
            // Calculate exposed fraction of this part's perpendicular interval
            let total_width = proj.perp_max - proj.perp_min;
            if total_width <= 0.0 { continue; }

            let exposed_width = exposed_interval_width(proj.perp_min, proj.perp_max, &occluded);
            let part = &self.parts[proj.idx];
            let def = match part_defs.get(&part.definition_id) {
                Some(d) => d,
                None => continue,
            };

            // Exposed width for heating (zero if fairing-shielded)
            let exposed_w = if fairing_shielded.contains(&proj.idx) {
                0.0
            } else {
                exposed_width
            };

            // Sutton-Graves heat input: q_in = K * sqrt(density) * airspeed^3 * exposed_width
            let q_in = Self::SUTTON_GRAVES_K * density.sqrt() * airspeed.powi(3) * exposed_w;

            // Radiative cooling (net radiation): q_out = emissivity * sigma * (T^4 - T_ambient^4) * surface_area
            let q_out = def.emissivity * Self::STEFAN_BOLTZMANN * (part.temperature.powi(4) - 300.0_f64.powi(4)) * proj.surface_area;

            let thermal_mass_kg = def.width() * Self::SKIN_THERMAL_MASS_PER_METER;
            if thermal_mass_kg > 0.0 {
                let d_temp = (q_in - q_out) / (thermal_mass_kg * def.specific_heat) * dt;
                self.parts[proj.idx].temperature = (self.parts[proj.idx].temperature + d_temp).max(300.0);
            }

            // Add this part to occluded intervals
            insert_interval(&mut occluded, proj.perp_min, proj.perp_max);
        }
    }

    /// Check for parts exceeding their heat tolerance and destroy them.
    /// Returns indices of destroyed parts (for staging cleanup).
    pub fn destroy_overheated_parts(&mut self) -> Vec<usize> {
        let mut destroyed = Vec::new();
        for (i, part) in self.parts.iter_mut().enumerate() {
            if part.destroyed || part.decoupled { continue; }
            if part.temperature >= part.max_heat_tolerance {
                part.destroyed = true;
                destroyed.push(i);
                log::info!("Part {} destroyed by overheating at {}K (tolerance: {}K)",
                    part.definition_id, part.temperature as i32, part.max_heat_tolerance as i32);
            }
        }
        destroyed
    }

    /// After parts are destroyed, check if the vessel has split into disconnected components.
    /// Uses BFS from root_part_index through weld connections. Any non-destroyed,
    /// non-decoupled parts unreachable from root form debris vessels.
    /// Returns a list of (debris_vessel, com_offset_local) for each disconnected component.
    pub fn check_and_split(&mut self, part_defs: &PartDefinitions) -> Vec<(FlightVessel, [f64; 2])> {
        use std::collections::VecDeque;

        let n = self.parts.len();
        let connections = self.find_weld_connections(part_defs);

        // BFS from root to find all reachable parts
        let mut reachable = vec![false; n];

        // If root part is destroyed, find the first non-destroyed non-decoupled part as new root
        if self.root_part_index < n && (self.parts[self.root_part_index].destroyed || self.parts[self.root_part_index].decoupled) {
            // Try to find a pod first, then any part
            let new_root = self.parts.iter().enumerate()
                .filter(|(_, p)| !p.destroyed && !p.decoupled)
                .find(|(_, p)| {
                    part_defs.get(&p.definition_id).map(|d| d.pod.is_some()).unwrap_or(false)
                })
                .map(|(i, _)| i)
                .or_else(|| {
                    self.parts.iter().enumerate()
                        .find(|(_, p)| !p.destroyed && !p.decoupled)
                        .map(|(i, _)| i)
                });
            match new_root {
                Some(r) => self.root_part_index = r,
                None => return Vec::new(), // All parts destroyed
            }
        }

        let root = self.root_part_index;
        if root >= n || self.parts[root].destroyed || self.parts[root].decoupled {
            return Vec::new();
        }

        let mut queue = VecDeque::new();
        reachable[root] = true;
        queue.push_back(root);
        while let Some(idx) = queue.pop_front() {
            for &neighbor in &connections[idx] {
                if !reachable[neighbor] && !self.parts[neighbor].destroyed && !self.parts[neighbor].decoupled {
                    reachable[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }

        // Find disconnected components (non-reachable, non-destroyed, non-decoupled parts)
        let unreachable_indices: Vec<usize> = (0..n)
            .filter(|&i| !reachable[i] && !self.parts[i].destroyed && !self.parts[i].decoupled)
            .collect();

        if unreachable_indices.is_empty() {
            return Vec::new();
        }

        // Group unreachable parts into connected components
        let mut visited = vec![false; n];
        let mut components: Vec<Vec<usize>> = Vec::new();

        for &start in &unreachable_indices {
            if visited[start] { continue; }
            let mut component = Vec::new();
            let mut q = VecDeque::new();
            visited[start] = true;
            q.push_back(start);
            while let Some(idx) = q.pop_front() {
                component.push(idx);
                for &neighbor in &connections[idx] {
                    if !visited[neighbor] && !self.parts[neighbor].destroyed && !self.parts[neighbor].decoupled {
                        if unreachable_indices.contains(&neighbor) {
                            visited[neighbor] = true;
                            q.push_back(neighbor);
                        }
                    }
                }
            }
            components.push(component);
        }

        // Create debris vessels for each component
        let mut result = Vec::new();
        for component in &components {
            // Calculate COM of this component
            let mut debris_mass = 0.0;
            let mut debris_com = [0.0f64, 0.0];
            for &i in component {
                let part = &self.parts[i];
                let base_mass = part_defs.get(&part.definition_id)
                    .map(|d| d.mass).unwrap_or(0.0);
                let resource_mass: f64 = part.resources.values().sum::<f64>() * 0.001;
                let pm = base_mass + resource_mass + part.cargo_extra_mass_tonnes();
                debris_mass += pm;
                debris_com[0] += part.local_position[0] * pm;
                debris_com[1] += part.local_position[1] * pm;
            }
            if debris_mass <= 0.0 { continue; }
            debris_com[0] /= debris_mass;
            debris_com[1] /= debris_mass;

            // Clone parts, shift to local COM, clear state
            let mut debris_parts: Vec<FlightPart> = component.iter().map(|&i| {
                let mut part = self.parts[i].clone();
                part.local_position[0] -= debris_com[0];
                part.local_position[1] -= debris_com[1];
                part
            }).collect();

            // Disable engines on debris
            for part in &mut debris_parts {
                part.engine_active = false;
                part.engine_enabled = false;
            }

            // Calculate MOI
            let mut moi = 0.0;
            for part in &debris_parts {
                let base_mass = part_defs.get(&part.definition_id)
                    .map(|d| d.mass).unwrap_or(0.0);
                let resource_mass: f64 = part.resources.values().sum::<f64>() * 0.001;
                let pm = base_mass + resource_mass + part.cargo_extra_mass_tonnes();
                let r_sq = part.local_position[0].powi(2) + part.local_position[1].powi(2);
                moi += pm * r_sq;
                let (w, h) = part_defs.get(&part.definition_id)
                    .map(|d| (d.width(), d.height()))
                    .unwrap_or((1.0, 1.0));
                moi += pm * (w * w + h * h) / 12.0;
            }
            moi = moi.max(0.1);

            let dry_mass = debris_parts.iter()
                .map(|p| part_defs.get(&p.definition_id).map(|d| d.mass).unwrap_or(0.0))
                .sum();

            let debris_vessel = FlightVessel {
                rel_position: [0.0, 0.0],
                rel_velocity: [0.0, 0.0],
                rotation: self.rotation,
                rotational_velocity: 0.0,
                soi_body: self.soi_body,
                parts: debris_parts,
                root_part_index: 0,
                total_mass: debris_mass,
                dry_mass,
                center_of_mass: [0.0, 0.0],
                max_thrust_vac: 0.0,
                max_thrust_asl: 0.0,
                moment_of_inertia: moi,
                throttle: 0.0,
                on_rails: false,
                stages: Vec::new(),
                current_stage: 0,
                last_decouple_force: 0.0,
                extra_dry_mass_tonnes: 0.0,
                thermal_pool_temp: 300.0,
                thermal_pool_capacity: (dry_mass * 1000.0).max(1.0) * 500.0,
                reactors_tripped: false,
                food_stored: 0.0,
                total_crew: 0,
                starvation_timer: 0.0,
            };

            result.push((debris_vessel, debris_com));

            // Mark source parts as destroyed in the parent vessel
            for &i in component {
                self.parts[i].destroyed = true;
            }
        }

        result
    }

    /// Calculate per-stage delta-v (vacuum) using the Tsiolkovsky rocket equation.
    /// Simulates staging sequentially: decouplers fire, engines activate.
    /// Fuel zones (divided by non-crossfeed decouplers) determine which fuel
    /// is accessible to active engines in each stage.
    pub fn calculate_stage_delta_v(&self, part_defs: &PartDefinitions) -> Vec<(f64, f64)> {
        use std::collections::{HashSet, VecDeque};

        let g0 = 9.80665;
        let n = self.parts.len();
        let mut stage_dvs = Vec::new();

        // Track state across stages
        let mut decoupled: Vec<bool> = self.parts.iter().map(|p| p.destroyed || p.decoupled).collect();
        let mut engines_enabled: Vec<bool> = vec![false; n];

        // Track remaining resources per part (in kg, matching FlightPart.resources)
        let mut resources_remaining: Vec<HashMap<String, f64>> = self.parts.iter()
            .map(|p| {
                if p.destroyed || p.decoupled {
                    return HashMap::new();
                }
                p.resources.iter()
                    .filter(|(_, &v)| v > 0.0)
                    .map(|(k, &v)| (k.clone(), v))
                    .collect()
            })
            .collect();

        for stage in &self.stages {
            // 1. Fire decouplers in this stage
            for &part_idx in stage {
                if part_idx >= n || decoupled[part_idx] { continue; }
                let Some(def) = part_defs.get(&self.parts[part_idx].definition_id) else { continue };
                if let Some(ref dec_data) = def.decoupler {
                    decoupled[part_idx] = true;
                    if !dec_data.is_radial {
                        // Stack decoupler: Y-based decoupling
                        let decoupler_bottom = self.parts[part_idx].local_position[1]
                            - def.hitbox_height() / 2.0;
                        for i in 0..n {
                            if decoupled[i] { continue; }
                            let Some(other_def) = part_defs.get(&self.parts[i].definition_id) else { continue };
                            let other_top = self.parts[i].local_position[1]
                                + other_def.hitbox_height() / 2.0;
                            if other_top <= decoupler_bottom + 0.01 {
                                decoupled[i] = true;
                            }
                        }
                    }
                    // Radial decouplers: only mark self (done above)
                }
            }

            // 1b. BFS connectivity: mark parts disconnected from root as decoupled
            {
                let mut connections = vec![Vec::new(); n];
                for i in 0..n {
                    if decoupled[i] { continue; }
                    let Some(def_i) = part_defs.get(&self.parts[i].definition_id) else { continue };
                    let weld_hw_i = def_i.weld_hitbox_width() / 2.0;
                    let weld_hh_i = def_i.weld_hitbox_height() / 2.0;
                    for j in (i + 1)..n {
                        if decoupled[j] { continue; }
                        let Some(def_j) = part_defs.get(&self.parts[j].definition_id) else { continue };
                        let weld_hw_j = def_j.weld_hitbox_width() / 2.0;
                        let weld_hh_j = def_j.weld_hitbox_height() / 2.0;
                        let dx = (self.parts[i].local_position[0] - self.parts[j].local_position[0]).abs();
                        let dy = (self.parts[i].local_position[1] - self.parts[j].local_position[1]).abs();
                        if dx < weld_hw_i + weld_hw_j && dy < weld_hh_i + weld_hh_j {
                            connections[i].push(j);
                            connections[j].push(i);
                        }
                    }
                }
                let root = if self.root_part_index < n && !decoupled[self.root_part_index] {
                    self.root_part_index
                } else {
                    (0..n).filter(|&i| !decoupled[i])
                        .find(|&i| part_defs.get(&self.parts[i].definition_id).map(|d| d.pod.is_some()).unwrap_or(false))
                        .or_else(|| (0..n).find(|&i| !decoupled[i]))
                        .unwrap_or(0)
                };
                let mut reachable = vec![false; n];
                let mut queue = VecDeque::new();
                if root < n && !decoupled[root] {
                    reachable[root] = true;
                    queue.push_back(root);
                }
                while let Some(idx) = queue.pop_front() {
                    for &neighbor in &connections[idx] {
                        if !reachable[neighbor] {
                            reachable[neighbor] = true;
                            queue.push_back(neighbor);
                        }
                    }
                }
                for i in 0..n {
                    if !decoupled[i] && !reachable[i] {
                        decoupled[i] = true;
                    }
                }
            }

            // 2. Fire fairings: decouple just the fairing base (parts inside stay)
            for &part_idx in stage {
                if part_idx >= n || decoupled[part_idx] { continue; }
                let Some(def) = part_defs.get(&self.parts[part_idx].definition_id) else { continue };
                if def.fairing.is_some() {
                    decoupled[part_idx] = true;
                }
            }

            // 3. Enable engines in this stage
            for &part_idx in stage {
                if part_idx >= n || decoupled[part_idx] { continue; }
                if self.parts[part_idx].propellant_type.is_some() {
                    engines_enabled[part_idx] = true;
                }
            }

            // 4. Compute fuel zones and drain priorities with simulated decoupled state
            let zones = self.compute_fuel_zones_simulated(part_defs, &decoupled);
            let sim_priorities = self.compute_drain_priorities_simulated(part_defs, &decoupled);

            // 5. Find zones containing active (non-decoupled, non-covered) engines
            let engine_zones: HashSet<usize> = (0..n)
                .filter(|&i| !decoupled[i] && engines_enabled[i] && zones[i] != usize::MAX
                    && !self.is_engine_covered_simulated(i, part_defs, &decoupled))
                .map(|i| zones[i])
                .collect();

            // 6. Per-engine resource demands (kg/s) and thrust contributions
            // Only count engines whose required resources are available in their zone
            let mut zone_resource_demand: HashMap<(usize, &str), f64> = HashMap::new();
            let mut zone_thrust: HashMap<usize, f64> = HashMap::new();
            let mut zone_thrust_over_isp: HashMap<usize, f64> = HashMap::new();

            for i in 0..n {
                if decoupled[i] || !engines_enabled[i] { continue; }
                if zones[i] == usize::MAX || !engine_zones.contains(&zones[i]) { continue; }
                if self.is_engine_covered_simulated(i, part_defs, &decoupled) { continue; }

                let propellant = match self.parts[i].propellant_type {
                    Some(p) => p,
                    None => continue,
                };
                let fuel_type = propellant.fuel_type();
                let fuel_name = match fuel_type.fuel_resource_name() {
                    Some(name) => name,
                    None => continue,
                };
                let (ox_per_sq, fuel_per_sq) = fuel_type.propellant_per_grid_square();
                let total_per_sq = ox_per_sq + fuel_per_sq;
                if total_per_sq <= 0.0 { continue; }

                let mass_flow = self.parts[i].mass_flow_rate;
                if mass_flow <= 0.0 { continue; }
                let primary_fraction = 1.0 - self.parts[i].secondary_fuel_fraction;
                let ox_ratio = ox_per_sq / total_per_sq;
                let fuel_ratio = fuel_per_sq / total_per_sq;
                let z = zones[i];

                // Check if required resources exist in this zone
                let has_fuel = (0..n).any(|j| !decoupled[j] && zones[j] == z
                    && resources_remaining[j].get(fuel_name).copied().unwrap_or(0.0) > 0.0);
                let has_ox = ox_ratio <= 0.0 || (0..n).any(|j| !decoupled[j] && zones[j] == z
                    && resources_remaining[j].get("oxygen").copied().unwrap_or(0.0) > 0.0);
                let has_secondary = match self.parts[i].secondary_propellant_type {
                    Some(sec) => match sec.fuel_type().fuel_resource_name() {
                        Some(sec_name) => (0..n).any(|j| !decoupled[j] && zones[j] == z
                            && resources_remaining[j].get(sec_name).copied().unwrap_or(0.0) > 0.0),
                        None => true,
                    },
                    None => true,
                };
                if !has_fuel || !has_ox || !has_secondary { continue; }

                // Engine has fuel — add its demands and thrust
                let primary_flow = mass_flow * primary_fraction;
                if ox_ratio > 0.0 {
                    *zone_resource_demand.entry((z, "oxygen")).or_insert(0.0) +=
                        primary_flow * ox_ratio;
                }
                *zone_resource_demand.entry((z, fuel_name)).or_insert(0.0) +=
                    primary_flow * fuel_ratio;

                if let Some(sec) = self.parts[i].secondary_propellant_type {
                    if let Some(sec_name) = sec.fuel_type().fuel_resource_name() {
                        *zone_resource_demand.entry((z, sec_name)).or_insert(0.0) +=
                            mass_flow * self.parts[i].secondary_fuel_fraction;
                    }
                }

                *zone_thrust.entry(z).or_insert(0.0) += self.parts[i].engine_thrust_vac;
                if self.parts[i].engine_isp_vac > 0.0 {
                    *zone_thrust_over_isp.entry(z).or_insert(0.0) +=
                        self.parts[i].engine_thrust_vac / self.parts[i].engine_isp_vac;
                }
            }

            // 7. Per-(zone, resource) min drain priority and availability
            let mut zone_res_min_pri: HashMap<(usize, &str), usize> = HashMap::new();
            let mut zone_res_available: HashMap<(usize, &str), f64> = HashMap::new();
            for &(z, res) in zone_resource_demand.keys() {
                let min_pri = (0..n)
                    .filter(|&j| !decoupled[j] && zones[j] == z)
                    .filter(|&j| resources_remaining[j].get(res).copied().unwrap_or(0.0) > 0.0)
                    .map(|j| sim_priorities[j])
                    .min()
                    .unwrap_or(usize::MAX);

                let available: f64 = (0..n)
                    .filter(|&j| !decoupled[j] && zones[j] == z && sim_priorities[j] == min_pri)
                    .filter_map(|j| resources_remaining[j].get(res))
                    .sum();

                zone_res_min_pri.insert((z, res), min_pri);
                zone_res_available.insert((z, res), available);
            }

            // 8. Phase time = min(available / demand) across all demanded resources
            let mut phase_time = f64::MAX;
            for (&key, &demand) in &zone_resource_demand {
                if demand <= 0.0 { continue; }
                let available = zone_res_available.get(&key).copied().unwrap_or(0.0);
                if available > 0.0 {
                    phase_time = phase_time.min(available / demand);
                } else {
                    phase_time = 0.0;
                    break;
                }
            }
            if phase_time == f64::MAX { phase_time = 0.0; }

            // 9. Total consumed mass (kg → tonnes)
            let total_consumed_kg: f64 = zone_resource_demand.values()
                .map(|&d| d * phase_time).sum();
            let total_consumed = total_consumed_kg / 1000.0;

            // Wet mass of all remaining parts (tonnes) + extra cargo payload
            let mut wet_mass = self.extra_dry_mass_tonnes;
            for i in 0..n {
                if decoupled[i] { continue; }
                let base_mass = part_defs.get(&self.parts[i].definition_id)
                    .map(|d| d.mass).unwrap_or(0.0);
                let fuel_mass = resources_remaining[i].values().sum::<f64>() / 1000.0;
                wet_mass += base_mass + fuel_mass;
            }

            // Effective Isp (harmonic weighted mean across zones)
            let total_thrust: f64 = zone_thrust.values().sum();
            let total_thrust_over_isp: f64 = zone_thrust_over_isp.values().sum();
            let effective_isp = if total_thrust_over_isp > 0.0 {
                total_thrust / total_thrust_over_isp
            } else {
                0.0
            };

            // 10. Δv = Isp * g0 * ln(wet / dry)
            let dry_mass = wet_mass - total_consumed;
            let dv = if effective_isp > 0.0 && dry_mass > 0.0 && wet_mass > dry_mass {
                effective_isp * g0 * (wet_mass / dry_mass).ln()
            } else {
                0.0
            };
            stage_dvs.push((dv, phase_time));

            // 11. Drain resources proportionally per (zone, resource)
            for (&(z, res), &demand) in &zone_resource_demand {
                if demand <= 0.0 { continue; }
                let consumed = demand * phase_time;
                if consumed <= 0.0 { continue; }
                let available = zone_res_available.get(&(z, res)).copied().unwrap_or(0.0);
                if available <= 0.0 { continue; }
                let min_pri = zone_res_min_pri.get(&(z, res)).copied().unwrap_or(usize::MAX);

                if consumed >= available {
                    // Resource depleted at this priority level
                    for j in 0..n {
                        if !decoupled[j] && zones[j] == z && sim_priorities[j] == min_pri {
                            if let Some(val) = resources_remaining[j].get_mut(res) {
                                *val = 0.0;
                            }
                        }
                    }
                } else {
                    // Partial drain — distribute proportionally
                    let drain_frac = consumed / available;
                    for j in 0..n {
                        if decoupled[j] || zones[j] != z || sim_priorities[j] != min_pri { continue; }
                        if let Some(val) = resources_remaining[j].get_mut(res) {
                            *val -= *val * drain_frac;
                            if *val < 0.001 { *val = 0.0; }
                        }
                    }
                }
            }
        }

        stage_dvs
    }

    /// Extract decoupled fairing shells into two half-shell debris vessels.
    /// The base disc stays on the vessel; only the shell splits off.
    /// Must be called BEFORE extract_decoupled_parts().
    /// Returns Vec of (debris_vessel, com_offset_local, FairingHalf).
    pub fn extract_fairing_halves(&mut self, part_defs: &PartDefinitions) -> Vec<(FlightVessel, [f64; 2], crate::parts::FairingHalf)> {
        use crate::parts::FairingHalf;
        let mut result = Vec::new();

        // Find decoupled fairing parts that have a shell to split
        let fairing_indices: Vec<usize> = self.parts.iter()
            .enumerate()
            .filter(|(_, p)| p.decoupled && !p.destroyed && p.fairing_shape.is_some() && p.fairing_half.is_none())
            .map(|(i, _)| i)
            .collect();

        for idx in fairing_indices {
            let part = &self.parts[idx];
            let com_offset = part.local_position;
            let base_mass = part_defs.get(&part.definition_id).map(|d| d.mass).unwrap_or(0.0);
            // Shell is ~20% of total fairing mass, split between two halves
            let half_shell_mass = (base_mass * 0.1).max(0.001);

            for &half in &[FairingHalf::Left, FairingHalf::Right] {
                let mut debris_part = part.clone();
                debris_part.local_position = [0.0, 0.0];
                debris_part.decoupled = false;
                debris_part.fairing_half = Some(half);

                let moi = {
                    let (w, h) = part_defs.get(&debris_part.definition_id)
                        .map(|d| (d.width(), d.height()))
                        .unwrap_or((1.0, 1.0));
                    (half_shell_mass * (w * w + h * h) / 12.0).max(0.1)
                };

                let debris_vessel = FlightVessel {
                    rel_position: [0.0, 0.0],
                    rel_velocity: [0.0, 0.0],
                    rotation: self.rotation,
                    rotational_velocity: 0.0,
                    soi_body: self.soi_body,
                    parts: vec![debris_part],
                    root_part_index: 0,
                    total_mass: half_shell_mass,
                    dry_mass: half_shell_mass,
                    center_of_mass: [0.0, 0.0],
                    max_thrust_vac: 0.0,
                    max_thrust_asl: 0.0,
                    moment_of_inertia: moi,
                    throttle: 0.0,
                    on_rails: false,
                    stages: Vec::new(),
                    current_stage: 0,
                    last_decouple_force: 0.0,
                    extra_dry_mass_tonnes: 0.0,
                    thermal_pool_temp: 300.0,
                    thermal_pool_capacity: (half_shell_mass * 1000.0).max(1.0) * 500.0,
                    reactors_tripped: false,
                    food_stored: 0.0,
                    total_crew: 0,
                    starvation_timer: 0.0,
                };

                result.push((debris_vessel, com_offset, half));
            }

            // Keep the base disc on the vessel: un-decouple and strip the shell
            self.parts[idx].decoupled = false;
            self.parts[idx].fairing_shape = None;
        }

        result
    }

    /// Extract decoupled parts into a new FlightVessel (debris).
    /// Call after activate_next_stage() or manual decouple marks parts as `decoupled = true`.
    /// Returns (debris_vessel, com_offset_local) if any decoupled parts were extracted.
    /// The com_offset_local is in vessel-local coordinates (before rotation) so the caller
    /// can place the debris ship at the correct world position.
    pub fn extract_decoupled_parts(&mut self, part_defs: &PartDefinitions) -> Option<(FlightVessel, [f64; 2])> {
        let decoupled_indices: Vec<usize> = self.parts.iter()
            .enumerate()
            .filter(|(_, p)| p.decoupled && !p.destroyed)
            .map(|(i, _)| i)
            .collect();

        if decoupled_indices.is_empty() {
            return None;
        }

        // Compute COM of the decoupled parts (in the current vessel's local frame)
        let mut debris_mass = 0.0;
        let mut debris_com = [0.0f64, 0.0];
        for &i in &decoupled_indices {
            let part = &self.parts[i];
            let base_mass = part_defs.get(&part.definition_id)
                .map(|d| d.mass).unwrap_or(0.0);
            let resource_mass: f64 = part.resources.values().sum::<f64>() * 0.001;
            let pm = base_mass + resource_mass + part.cargo_extra_mass_tonnes();
            debris_mass += pm;
            debris_com[0] += part.local_position[0] * pm;
            debris_com[1] += part.local_position[1] * pm;
        }
        if debris_mass <= 0.0 {
            return None;
        }
        debris_com[0] /= debris_mass;
        debris_com[1] /= debris_mass;

        // Clone decoupled parts, shift them relative to their own COM, clear decoupled flag
        let mut debris_parts: Vec<FlightPart> = decoupled_indices.iter().map(|&i| {
            let mut part = self.parts[i].clone();
            part.local_position[0] -= debris_com[0];
            part.local_position[1] -= debris_com[1];
            part.decoupled = false;
            part
        }).collect();

        // Mark originals as destroyed so they aren't re-extracted on future staging events
        for &i in &decoupled_indices {
            self.parts[i].destroyed = true;
        }

        // Disable all engines on debris (no staging system)
        for part in &mut debris_parts {
            part.engine_active = false;
            part.engine_enabled = false;
        }

        // Calculate moment of inertia for debris
        let mut moi = 0.0;
        for part in &debris_parts {
            let base_mass = part_defs.get(&part.definition_id)
                .map(|d| d.mass).unwrap_or(0.0);
            let resource_mass: f64 = part.resources.values().sum::<f64>() * 0.001;
            let pm = base_mass + resource_mass + part.cargo_extra_mass_tonnes();
            let r_sq = part.local_position[0].powi(2) + part.local_position[1].powi(2);
            moi += pm * r_sq;
            let (w, h) = part_defs.get(&part.definition_id)
                .map(|d| (d.width(), d.height()))
                .unwrap_or((1.0, 1.0));
            moi += pm * (w * w + h * h) / 12.0;
        }
        moi = moi.max(0.1);

        let dry_mass = debris_parts.iter()
            .map(|p| part_defs.get(&p.definition_id).map(|d| d.mass).unwrap_or(0.0))
            .sum();

        let debris_vessel = FlightVessel {
            rel_position: [0.0, 0.0], // Will be set by caller
            rel_velocity: [0.0, 0.0],
            rotation: self.rotation,
            rotational_velocity: 0.0,
            soi_body: self.soi_body,
            parts: debris_parts,
            root_part_index: 0,
            total_mass: debris_mass,
            dry_mass,
            center_of_mass: [0.0, 0.0],
            max_thrust_vac: 0.0,
            max_thrust_asl: 0.0,
            moment_of_inertia: moi,
            throttle: 0.0,
            on_rails: false,
            stages: Vec::new(),   // Debris can't stage
            current_stage: 0,
            last_decouple_force: 0.0,
            extra_dry_mass_tonnes: 0.0,
            thermal_pool_temp: 300.0,
            thermal_pool_capacity: (dry_mass * 1000.0_f64).max(1.0) * 500.0,
            reactors_tripped: false,
            food_stored: 0.0,
            total_crew: 0,
            starvation_timer: 0.0,
        };

        Some((debris_vessel, debris_com))
    }

    /// Activate next stage (enables engines and fires decouplers in that stage)
    pub fn activate_next_stage(&mut self, part_defs: &PartDefinitions, in_atmosphere: bool, is_landed: bool) -> bool {
        if self.current_stage >= self.stages.len() {
            return false;
        }

        let stage_parts = self.stages[self.current_stage].clone();
        self.last_decouple_force = 0.0;

        for &part_idx in &stage_parts {
            if part_idx >= self.parts.len() || self.parts[part_idx].decoupled {
                continue;
            }

            // Enable engines in this stage
            if self.parts[part_idx].propellant_type.is_some() {
                self.parts[part_idx].engine_enabled = true;
            }

            // Deploy parachutes in this stage (only if in atmosphere and not landed)
            if self.parts[part_idx].is_parachute
                && !self.parts[part_idx].parachute_spent
                && !self.parts[part_idx].parachute_deployed
                && in_atmosphere
                && !is_landed
            {
                self.parts[part_idx].parachute_deployed = true;
            }

            // Fire fairings: decouple just the fairing base (parts inside stay)
            let def = part_defs.get(&self.parts[part_idx].definition_id);
            if let Some(def) = def {
                if let Some(ref fairing_data) = def.fairing {
                    if fairing_data.ejection_force > self.last_decouple_force {
                        self.last_decouple_force = fairing_data.ejection_force;
                    }
                    // Mark the fairing base as decoupled — parts above stay connected
                    self.parts[part_idx].decoupled = true;
                }
            }

            // Fire decouplers: decouple all parts below the decoupler
            let def = part_defs.get(&self.parts[part_idx].definition_id);
            if let Some(def) = def {
                if let Some(ref dec_data) = def.decoupler {
                    // Store max ejection force from decouplers in this stage
                    if dec_data.ejection_force > self.last_decouple_force {
                        self.last_decouple_force = dec_data.ejection_force;
                    }

                    // Mark the decoupler itself as decoupled
                    self.parts[part_idx].decoupled = true;

                    if !dec_data.is_radial {
                        // Stack decoupler: Y-based decoupling + adapter/fairing logic
                        let dec_x = self.parts[part_idx].local_position[0];
                        let decoupler_bottom = self.parts[part_idx].local_position[1]
                            - def.hitbox_height() / 2.0;
                        let decoupler_top = self.parts[part_idx].local_position[1]
                            + def.hitbox_height() / 2.0;
                        let dec_half_w = def.width() / 2.0;

                        // Mark all parts whose top edge is at or below the decoupler bottom
                        for i in 0..self.parts.len() {
                            if i == part_idx || self.parts[i].decoupled {
                                continue;
                            }
                            let other_def = part_defs.get(&self.parts[i].definition_id);
                            let other_top = if let Some(od) = other_def {
                                self.parts[i].local_position[1] + od.hitbox_height() / 2.0
                            } else {
                                self.parts[i].local_position[1] + self.parts[i].hitbox_half_extents[1]
                            };
                            if other_top <= decoupler_bottom + 0.01 {
                                self.parts[i].decoupled = true;
                            }
                        }

                        // Also decouple parts beside the adapter/fairing.
                        // The adapter zone spans from the visual ring top to the
                        // tank/pod bottom above. (The ring is shorter than the hitbox;
                        // the gap between ring top and hitbox top is the adapter space.)
                        let ring_top = self.parts[part_idx].local_position[1]
                            - def.hitbox_height() / 2.0 + def.height();

                        // Find the closest aligned tank/pod above to get fairing width
                        let mut adapter_top = decoupler_top;
                        let mut tank_half_w = dec_half_w;
                        let mut best_dist = f64::MAX;
                        for j in 0..self.parts.len() {
                            if self.parts[j].decoupled || self.parts[j].destroyed { continue; }
                            let Some(jdef) = part_defs.get(&self.parts[j].definition_id) else { continue };
                            if jdef.tank.is_none() && jdef.pod.is_none() { continue; }
                            if (self.parts[j].local_position[0] - dec_x).abs() > 0.01 { continue; }
                            let j_bottom = self.parts[j].local_position[1] - jdef.hitbox_height() / 2.0;
                            if j_bottom < ring_top - 0.01 { continue; }
                            let dist = j_bottom - ring_top;
                            if dist < best_dist {
                                best_dist = dist;
                                adapter_top = j_bottom;
                                tank_half_w = jdef.width() / 2.0;
                            }
                        }

                        // Decouple parts beside the fairing. A part qualifies if its
                        // center Y is in the adapter zone and it's off the center axis
                        // but within reach of the fairing edge.
                        let min_half_w = dec_half_w.min(tank_half_w);
                        let max_half_w = dec_half_w.max(tank_half_w);
                        let margin = crate::parts::GRID_SQUARE_SIZE;
                        for j in 0..self.parts.len() {
                            if j == part_idx || self.parts[j].decoupled || self.parts[j].destroyed {
                                continue;
                            }
                            let jy = self.parts[j].local_position[1];
                            let jx = self.parts[j].local_position[0];
                            if jy < ring_top - 0.01 || jy > adapter_top + 0.01 { continue; }
                            let dx = (jx - dec_x).abs();
                            // In the adapter overhang zone (outside the narrower stack width)
                            if dx > min_half_w - 0.01 && dx < max_half_w + margin {
                                self.parts[j].decoupled = true;
                            }
                        }
                    }
                    // Radial decouplers: only mark self as decoupled (done above).
                    // The BFS in decouple_disconnected() handles disconnecting
                    // parts that are only reachable through the decoupled radial.
                }
            }
        }

        // After Y-based decoupling, also decouple any parts that are no longer
        // connected to the vessel root (e.g., RCS blocks side-mounted on a decoupler).
        self.decouple_disconnected(part_defs);

        self.current_stage += 1;
        true
    }

    /// Mark any non-decoupled parts that are disconnected from the root as decoupled.
    /// Uses BFS through weld connections (which skip already-decoupled parts).
    fn decouple_disconnected(&mut self, part_defs: &PartDefinitions) {
        use std::collections::VecDeque;

        let n = self.parts.len();
        let connections = self.find_weld_connections(part_defs);

        // Find a valid root (prefer current root, then any pod, then any part)
        let root = if self.root_part_index < n
            && !self.parts[self.root_part_index].destroyed
            && !self.parts[self.root_part_index].decoupled
        {
            self.root_part_index
        } else {
            match self.parts.iter().enumerate()
                .filter(|(_, p)| !p.destroyed && !p.decoupled)
                .find(|(_, p)| part_defs.get(&p.definition_id).map(|d| d.pod.is_some()).unwrap_or(false))
                .map(|(i, _)| i)
                .or_else(|| self.parts.iter().enumerate()
                    .find(|(_, p)| !p.destroyed && !p.decoupled)
                    .map(|(i, _)| i))
            {
                Some(r) => r,
                None => return,
            }
        };

        // BFS from root
        let mut reachable = vec![false; n];
        let mut queue = VecDeque::new();
        reachable[root] = true;
        queue.push_back(root);
        while let Some(idx) = queue.pop_front() {
            for &neighbor in &connections[idx] {
                if !reachable[neighbor] && !self.parts[neighbor].destroyed && !self.parts[neighbor].decoupled {
                    reachable[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }

        // Mark unreachable parts as decoupled
        for i in 0..n {
            if !reachable[i] && !self.parts[i].destroyed && !self.parts[i].decoupled {
                self.parts[i].decoupled = true;
            }
        }
    }

    /// Create a `StoredShip` from a FlightVessel's current state.
    /// Builds a dry blueprint from non-decoupled, non-destroyed parts with empty tanks.
    pub fn to_stored_ship(
        vessel: &FlightVessel,
        part_defs: &PartDefinitions,
        name: String,
    ) -> crate::colony::trade::StoredShip {
        use super::BlueprintPart;

        // Build blueprint parts from active (non-decoupled, non-destroyed) flight parts
        let active_indices: Vec<usize> = vessel.parts.iter().enumerate()
            .filter(|(_, p)| !p.decoupled && !p.destroyed)
            .map(|(i, _)| i)
            .collect();

        // Map old indices → new indices
        let mut old_to_new = std::collections::HashMap::new();
        for (new_idx, &old_idx) in active_indices.iter().enumerate() {
            old_to_new.insert(old_idx, new_idx);
        }

        let mut blueprint_parts = Vec::new();
        for &old_idx in &active_indices {
            let part = &vessel.parts[old_idx];
            let def = part_defs.get(&part.definition_id);

            // Determine fuel type from the part's propellant resources (heuristic)
            let fuel_type = if def.and_then(|d| d.tank.as_ref()).is_some() {
                // Check what resources the tank has to determine fuel type
                if part.max_resources.contains_key("rp1") {
                    super::FuelType::Rp1
                } else if part.max_resources.contains_key("methane") {
                    super::FuelType::Methane
                } else if part.max_resources.contains_key("hydrogen") {
                    super::FuelType::Hydrogen
                } else if part.max_resources.contains_key("xenon") {
                    super::FuelType::Xenon
                } else if part.max_resources.contains_key("fusion_fuel") || part.max_resources.contains_key("deuterium") {
                    super::FuelType::FusionFuel
                } else if part.max_resources.contains_key("antimatter") {
                    super::FuelType::Antimatter
                } else if part.max_resources.contains_key("nuclear_pulse") {
                    super::FuelType::NuclearPulse
                } else {
                    super::FuelType::Empty
                }
            } else {
                super::FuelType::Empty
            };

            blueprint_parts.push(BlueprintPart {
                definition_id: part.definition_id.clone(),
                position: part.local_position,
                rotation: part.rotation,
                parent_index: None, // We don't track parent in FlightPart
                attachment_type: if old_idx == vessel.root_part_index {
                    super::AttachmentType::Root
                } else {
                    super::AttachmentType::Stack
                },
                stage: 0,
                fuel_type,
                tank_filled: false,
                fill_fraction: 1.0, // For delta-v calculation: assume full tanks
                crossfeed_enabled: part.crossfeed_enabled,
                mirror_partner_index: part.mirror_partner.and_then(|mi| old_to_new.get(&mi).copied()),
                fairing_shape: part.fairing_shape.clone(),
                deployed: part.deploy_target,
                cargo_resources: Vec::new(),
                cargo_buildings: Vec::new(),
                cargo_payloads: Vec::new(),
            });
        }

        let root_index = old_to_new.get(&vessel.root_part_index).copied().unwrap_or(0);

        // Remap stages
        let stages: Vec<Vec<usize>> = vessel.stages.iter()
            .map(|stage| stage.iter().filter_map(|&idx| old_to_new.get(&idx).copied()).collect())
            .collect();

        let blueprint = VesselBlueprint {
            name: name.clone(),
            parts: blueprint_parts,
            root_part_index: root_index,
            stages,
        };

        let dry_mass_kg = crate::colony::economy::blueprint_dry_mass_kg(&blueprint, part_defs);
        let cached_delta_v = crate::colony::transfer::blueprint_total_delta_v(&blueprint, part_defs);

        crate::colony::trade::StoredShip {
            id: 0, // Caller assigns the real ID
            name,
            blueprint_name: None, // Custom recovered ship
            blueprint,
            dry_mass_kg,
            cached_delta_v,
        }
    }
}

/// Calculate the exposed (non-occluded) width of an interval [min, max]
/// given a set of sorted, non-overlapping occluded intervals.
fn exposed_interval_width(min: f64, max: f64, occluded: &[(f64, f64)]) -> f64 {
    let mut exposed = max - min;
    for &(occ_min, occ_max) in occluded {
        // Skip intervals entirely outside our range
        if occ_max <= min || occ_min >= max { continue; }
        // Clamp to our interval
        let overlap_min = occ_min.max(min);
        let overlap_max = occ_max.min(max);
        exposed -= overlap_max - overlap_min;
    }
    exposed.max(0.0)
}

/// Insert an interval into a sorted, non-overlapping interval list, merging overlaps.
fn insert_interval(intervals: &mut Vec<(f64, f64)>, min: f64, max: f64) {
    let mut new_min = min;
    let mut new_max = max;
    let mut i = 0;
    while i < intervals.len() {
        if intervals[i].1 < new_min {
            i += 1;
            continue;
        }
        if intervals[i].0 > new_max {
            break;
        }
        // Overlapping or adjacent — merge
        new_min = new_min.min(intervals[i].0);
        new_max = new_max.max(intervals[i].1);
        intervals.remove(i);
    }
    intervals.insert(i, (new_min, new_max));
}

/// Create a default single-part vessel for testing
pub fn create_default_vessel(
    spawn_position: [f64; 2],
    spawn_velocity: [f64; 2],
    soi_body: usize,
) -> FlightVessel {
    FlightVessel {
        rel_position: spawn_position,
        rel_velocity: spawn_velocity,
        rotation: 0.0,
        rotational_velocity: 0.0,
        soi_body,
        parts: vec![FlightPart {
            definition_id: "default_pod".to_string(),
            local_position: [0.0, 0.0],
            rotation: 0.0,
            hitbox_half_extents: [5.0, 5.0],
            hitbox_y_offset: 0.0,
            resources: HashMap::new(),
            max_resources: HashMap::new(),
            engine_active: true,
            engine_enabled: true,
            engine_thrust_vac: 200.0,
            engine_thrust_asl: 150.0,
            engine_isp_vac: 300.0,
            engine_isp_asl: 250.0,
            is_throttleable: true,
            propellant_type: None,
            secondary_propellant_type: None,
            secondary_fuel_fraction: 0.0,
            mass_flow_rate: 0.0,
            gimbal_angle: 0.0,
            gimbal_range_rad: 0.0,
            rcs_thrust: 0.0,
            rcs_torque_multiplier: 1.0,
            rcs_isp: 0.0,
            rcs_mass_flow_rate: 0.0,
            destroyed: false,
            decoupled: false,
            crossfeed_enabled: false,
            temperature: 300.0,
            max_heat_tolerance: 2000.0,
            fairing_shape: None,
            fairing_half: None,
            electricity: 0.0,
            max_electricity: 0.0,
            is_solar_panel: false,
            deploy_fraction: 0.0,
            deploy_target: false,
            mirror_partner: None,
            is_radiator: false,
            radiator_rejection_watts: 0.0,
            radiator_deploy_time_sec: 5.0,
            is_parachute: false,
            parachute_deployed: false,
            parachute_spent: false,
            parachute_deploy_fraction: 0.0,
            parachute_deployed_width_m: 0.0,
            parachute_fully_deployed: false,
            cargo_buildings: Vec::new(),
            cargo_payloads: Vec::new(),
            engine_no_power: false,
            shield_active: false,
            crew_count: 0,
        }],
        root_part_index: 0,
        total_mass: 2.0,
        dry_mass: 2.0,
        center_of_mass: [0.0, 0.0],
        max_thrust_vac: 200.0,
        max_thrust_asl: 150.0,
        moment_of_inertia: 1.0,
        throttle: 0.0,
        on_rails: false,
        stages: Vec::new(),
        current_stage: 0,
        last_decouple_force: 0.0,
        extra_dry_mass_tonnes: 0.0,
        thermal_pool_temp: 300.0,
        thermal_pool_capacity: 1.0e6,
        reactors_tripped: false,
        food_stored: 0.0,
        total_crew: 0,
        starvation_timer: 0.0,
    }
}
