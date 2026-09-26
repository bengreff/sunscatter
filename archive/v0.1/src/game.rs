use crate::bodies::SolarSystem;
use crate::colony::{ColonyManager, Company, ContractManager, FleetManager, Notification, NotificationKind, ScienceState, TechTree};
use crate::editor::EditorState;
use crate::galaxy::GalaxyState;
use crate::parts::{BlueprintRegistry, FlightVessel, PartDefinitions};
use crate::render::ManeuverNode;
use crate::ship::{Ship, ShipInput};

/// Launchpad constants
pub const LAUNCHPAD_SURFACE_ANGLE: f64 = std::f64::consts::FRAC_PI_2;
pub const LAUNCHPAD_HEIGHT: f64 = 10.0; // meters
pub const LAUNCHPAD_TOP_WIDTH: f64 = 100.0; // meters
pub const LAUNCHPAD_BOTTOM_WIDTH: f64 = 120.0; // meters

/// The current game mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    TitleScreen,
    MainMenu,
    Editor,
    Flight,
    TrackingStation,
    Colony,
    ColonyOverview,
    Management,
    TechTree,
}

/// UI state for the title screen dialogs
#[derive(Debug, Clone, Default)]
pub struct TitleScreenUiState {
    pub show_new_game: bool,
    pub show_load_game: bool,
    pub new_game_name: String,
    /// Cached save file list, populated once when load dialog opens.
    pub save_list: Vec<crate::save::SaveFileInfo>,
    /// Save ID pending delete confirmation (None = no dialog shown).
    pub confirm_delete: Option<String>,
}

/// Unique identifier for a vessel (active or inactive)
pub type VesselId = u64;

/// An inactive vessel tracked by the game (on-rails propagation)
#[derive(Clone)]
pub struct TrackedVessel {
    pub id: VesselId,
    pub name: String,
    pub ship: Ship,
    pub vessel: Option<FlightVessel>,
    pub maneuver_nodes: Vec<ManeuverNode>,
    pub is_debris: bool,
}

/// Flight-specific state
pub struct FlightState {
    // Active vessel (direct access — existing code unchanged)
    pub ship: Ship,
    pub ship_input: ShipInput,
    pub tracking_ship: bool,
    pub vessel: Option<FlightVessel>,

    // Active vessel identity
    pub active_vessel_id: VesselId,
    pub active_vessel_name: String,
    /// Maneuver nodes owned by the active vessel
    pub active_maneuver_nodes: Vec<ManeuverNode>,

    // Inactive vessels (on-rails only)
    pub inactive_vessels: Vec<TrackedVessel>,
    pub next_vessel_id: VesselId,
    pub debris_counter: u32,
}

impl FlightState {
    pub fn new(ship: Ship) -> Self {
        Self {
            ship,
            ship_input: ShipInput::default(),
            tracking_ship: true,
            vessel: None,
            active_vessel_id: 0,
            active_vessel_name: "Ship".to_string(),
            active_maneuver_nodes: Vec::new(),
            inactive_vessels: Vec::new(),
            next_vessel_id: 1,
            debris_counter: 0,
        }
    }

    /// Package the active vessel into a TrackedVessel (for shelving/switching).
    pub fn active_as_tracked(&self, solar_system: &SolarSystem) -> TrackedVessel {
        let mut saved_ship = self.ship.clone();
        saved_ship.enter_rails_mode(solar_system);

        TrackedVessel {
            id: self.active_vessel_id,
            name: self.active_vessel_name.clone(),
            ship: saved_ship,
            vessel: self.vessel.clone(),
            maneuver_nodes: self.active_maneuver_nodes.clone(),
            is_debris: false,
        }
    }

    /// Load a TrackedVessel as the active vessel.
    pub fn load_as_active(&mut self, tv: TrackedVessel, solar_system: &SolarSystem) {
        self.ship = tv.ship;
        self.ship.exit_rails_mode(solar_system);
        self.vessel = tv.vessel;
        self.active_vessel_id = tv.id;
        self.active_vessel_name = tv.name;
        self.active_maneuver_nodes = tv.maneuver_nodes;
        self.ship_input = ShipInput::default();
        self.tracking_ship = true;
    }

    /// Total number of vessels (active + inactive)
    pub fn vessel_count(&self) -> usize {
        1 + self.inactive_vessels.len()
    }

    /// Get all vessel IDs sorted (active first, then inactive by ID)
    pub fn all_vessel_ids(&self) -> Vec<VesselId> {
        let mut ids = vec![self.active_vessel_id];
        for v in &self.inactive_vessels {
            ids.push(v.id);
        }
        ids.sort();
        ids
    }

    /// Switch to a different vessel by ID.
    /// Maneuver nodes are saved/restored via `active_maneuver_nodes`.
    pub fn switch_to_vessel(
        &mut self,
        target_id: VesselId,
        solar_system: &SolarSystem,
        part_defs: &PartDefinitions,
    ) -> Result<(), String> {
        // Find target in inactive vessels
        let target_pos = self.inactive_vessels.iter()
            .position(|v| v.id == target_id)
            .ok_or_else(|| format!("Vessel {} not found", target_id))?;

        let target = self.inactive_vessels.remove(target_pos);

        // Save current active vessel as inactive
        let is_debris = self.vessel.as_ref()
            .map_or(false, |v| !v.has_control(part_defs));
        let mut saved = self.active_as_tracked(solar_system);
        saved.is_debris = is_debris;
        saved.vessel = self.vessel.take(); // Take instead of clone for active
        self.inactive_vessels.push(saved);

        // Load target as active
        self.load_as_active(target, solar_system);

        Ok(())
    }

    /// Save the active vessel into inactive_vessels (for leaving flight mode).
    /// The active vessel is put on rails first.
    pub fn shelve_active_vessel(
        &mut self,
        solar_system: &SolarSystem,
    ) {
        let saved = self.active_as_tracked(solar_system);
        self.inactive_vessels.push(saved);
    }

    /// Load a vessel from inactive_vessels as the active vessel (for entering flight mode).
    pub fn activate_vessel(
        &mut self,
        target_id: VesselId,
        solar_system: &SolarSystem,
    ) -> Result<(), String> {
        let target_pos = self.inactive_vessels.iter()
            .position(|v| v.id == target_id)
            .ok_or_else(|| format!("Vessel {} not found", target_id))?;

        let target = self.inactive_vessels.remove(target_pos);
        self.load_as_active(target, solar_system);

        Ok(())
    }

    /// Create a debris vessel from extracted decoupled parts.
    /// `com_offset` is in vessel-local coordinates (before rotation).
    /// `ejection_force_kn` applies a separation impulse pushing debris away from the upper stage.
    pub fn create_debris_vessel(
        &mut self,
        debris_vessel: FlightVessel,
        com_offset: [f64; 2],
        ejection_force_kn: f64,
        solar_system: &SolarSystem,
        part_defs: &PartDefinitions,
    ) {
        // Local-to-world rotation for part positions (heading - PI/2, matches rendering)
        let local_rot = self.ship.rotation - std::f64::consts::FRAC_PI_2;
        let world_offset_x = com_offset[0] * local_rot.cos() - com_offset[1] * local_rot.sin();
        let world_offset_y = com_offset[0] * local_rot.sin() + com_offset[1] * local_rot.cos();

        let mut debris_ship = self.ship.clone();
        debris_ship.rel_position[0] += world_offset_x;
        debris_ship.rel_position[1] += world_offset_y;
        debris_ship.throttle = 0.0;
        debris_ship.on_rails = false;
        debris_ship.cached_orbit = None;
        debris_ship.cached_trajectory = None;
        debris_ship.color = [1.0, 1.0, 1.0, 1.0]; // Same as active vessels

        // Apply separation impulse: F*dt/m, using a 0.1s impulse duration
        // Direction: use COM offset to determine push direction.
        // For stack decouplers the offset is mostly vertical (pushes down),
        // for radial decouplers the offset is mostly horizontal (pushes sideways).
        if ejection_force_kn > 0.0 && debris_vessel.total_mass > 0.0 {
            let impulse_duration = 0.1; // seconds
            let force_newtons = ejection_force_kn * 1000.0;
            let mass_kg = debris_vessel.total_mass * 1000.0;
            let dv = force_newtons * impulse_duration / mass_kg;

            // Rotate COM offset from local to world coordinates
            let dir_mag = (world_offset_x.powi(2) + world_offset_y.powi(2)).sqrt();
            if dir_mag > 0.01 {
                // Push debris along the COM offset direction (away from vessel center)
                debris_ship.rel_velocity[0] += dv * world_offset_x / dir_mag;
                debris_ship.rel_velocity[1] += dv * world_offset_y / dir_mag;
            } else {
                // Fallback: opposite to vessel heading
                let sep_dir = self.ship.rotation + std::f64::consts::PI;
                debris_ship.rel_velocity[0] += dv * sep_dir.cos();
                debris_ship.rel_velocity[1] += dv * sep_dir.sin();
            }
        }

        // Put debris on rails immediately
        debris_ship.enter_rails_mode(solar_system);

        self.debris_counter += 1;
        let name = format!("Debris {}", self.debris_counter);
        let id = self.next_vessel_id;
        self.next_vessel_id += 1;

        let is_debris = !debris_vessel.has_control(part_defs);
        self.inactive_vessels.push(TrackedVessel {
            id,
            name: name.clone(),
            ship: debris_ship,
            vessel: Some(debris_vessel),
            maneuver_nodes: Vec::new(),
            is_debris,
        });

        log::info!("Created {} vessel: {} (id={})", if is_debris { "debris" } else { "tracked" }, name, id);
    }

    /// Create a fairing half debris vessel with a fixed perpendicular velocity.
    /// `com_offset` is the local position of the fairing part.
    /// `sign` is -1.0 for left half, +1.0 for right half.
    pub fn create_fairing_debris(
        &mut self,
        debris_vessel: FlightVessel,
        com_offset: [f64; 2],
        sep_speed: f64,
        sign: f64,
        solar_system: &SolarSystem,
    ) {
        let local_rot = self.ship.rotation - std::f64::consts::FRAC_PI_2;

        let world_offset_x = com_offset[0] * local_rot.cos() - com_offset[1] * local_rot.sin();
        let world_offset_y = com_offset[0] * local_rot.sin() + com_offset[1] * local_rot.cos();

        let mut debris_ship = self.ship.clone();
        debris_ship.rel_position[0] += world_offset_x;
        debris_ship.rel_position[1] += world_offset_y;
        debris_ship.throttle = 0.0;
        debris_ship.on_rails = false;
        debris_ship.cached_orbit = None;
        debris_ship.cached_trajectory = None;
        debris_ship.color = [1.0, 1.0, 1.0, 1.0];

        // Perpendicular velocity in local X direction
        let perp_x = local_rot.cos() * sign;
        let perp_y = local_rot.sin() * sign;
        debris_ship.rel_velocity[0] += sep_speed * perp_x;
        debris_ship.rel_velocity[1] += sep_speed * perp_y;

        debris_ship.enter_rails_mode(solar_system);

        self.debris_counter += 1;
        let name = format!("Debris {}", self.debris_counter);
        let id = self.next_vessel_id;
        self.next_vessel_id += 1;

        self.inactive_vessels.push(TrackedVessel {
            id,
            name: name.clone(),
            ship: debris_ship,
            vessel: Some(debris_vessel),
            maneuver_nodes: Vec::new(),
            is_debris: true, // Fairing halves are always debris
        });
    }

    /// Remove debris vessels that are far from all controllable vessels.
    /// Debris is deleted when >2km from every controllable vessel (or in a different SOI).
    pub fn cleanup_distant_debris(&mut self) {
        const CLEANUP_DISTANCE: f64 = 2000.0; // meters

        // Gather positions/SOIs of all controllable vessels:
        // 1. Active vessel (always controllable)
        let active_pos = self.ship.rel_position;
        let active_soi = self.ship.soi_body;

        // 2. Non-debris inactive vessels
        let controllable: Vec<([f64; 2], usize)> = self.inactive_vessels.iter()
            .filter(|v| !v.is_debris)
            .map(|v| (v.ship.rel_position, v.ship.soi_body))
            .collect();

        self.inactive_vessels.retain(|v| {
            if !v.is_debris {
                return true; // Keep non-debris
            }

            let debris_pos = v.ship.rel_position;
            let debris_soi = v.ship.soi_body;

            // Check distance to active vessel
            if debris_soi == active_soi {
                let dx = debris_pos[0] - active_pos[0];
                let dy = debris_pos[1] - active_pos[1];
                if (dx * dx + dy * dy).sqrt() <= CLEANUP_DISTANCE {
                    return true; // Close to active vessel, keep
                }
            }

            // Check distance to non-debris inactive vessels
            for &(ctrl_pos, ctrl_soi) in &controllable {
                if debris_soi == ctrl_soi {
                    let dx = debris_pos[0] - ctrl_pos[0];
                    let dy = debris_pos[1] - ctrl_pos[1];
                    if (dx * dx + dy * dy).sqrt() <= CLEANUP_DISTANCE {
                        return true; // Close to a controllable vessel, keep
                    }
                }
            }

            log::info!("Auto-deleted distant debris: {} (id={})", v.name, v.id);
            false // Too far from all controllable vessels, delete
        });
    }

    /// Remove inactive vessels that are landed on the launchpad.
    /// Called before launching a new vessel to clear the pad.
    pub fn recover_vessels_on_launchpad(&mut self, solar_system: &crate::bodies::SolarSystem, launch_body: usize) {
        let body_radius = solar_system.bodies[launch_body].radius;
        let half_angle = (LAUNCHPAD_BOTTOM_WIDTH * 0.5) / body_radius;

        self.inactive_vessels.retain(|v| {
            let on_launch_body = v.ship.soi_body == launch_body;
            let is_landed = matches!(v.ship.state, crate::ship::ShipState::Landed { body_index, .. } if body_index == launch_body);
            if on_launch_body && is_landed {
                let angle = v.ship.rel_position[1].atan2(v.ship.rel_position[0]);
                let angle_diff = angle - LAUNCHPAD_SURFACE_ANGLE;
                let angle_diff = angle_diff - (angle_diff / std::f64::consts::TAU).round() * std::f64::consts::TAU;
                if angle_diff.abs() < half_angle {
                    log::info!("Auto-recovered vessel on launchpad: {} (id={})", v.name, v.id);
                    return false; // Remove
                }
            }
            true // Keep
        });
    }
}

/// Returns true if the given body supports vessel recovery.
/// Earth always supports recovery. Colonies with a Launchpad also support recovery.
pub fn is_recoverable_body(
    body_index: usize,
    solar_system: &crate::bodies::SolarSystem,
    colony_manager: &crate::colony::ColonyManager,
) -> bool {
    if body_index == solar_system.earth_index {
        return true;
    }
    // Colony with a launchpad
    if let Some(colony) = colony_manager.get_by_body(body_index) {
        return colony.buildings.iter().any(|b| {
            b.building_type == crate::colony::BuildingType::Launchpad && b.operational
        });
    }
    false
}

/// Central game state container
pub struct Game {
    pub mode: GameMode,
    pub paused: bool,
    pub warp_index: usize,
    pub solar_system: SolarSystem,
    pub flight: FlightState,
    pub editor: EditorState,
    pub part_definitions: PartDefinitions,
    pub blueprints: BlueprintRegistry,
    /// Current save game name (None when on title screen / no game loaded)
    pub save_name: Option<String>,
    /// Whether a launch save exists for the current flight (enables "Revert to Launch")
    pub has_launch_save: bool,
    /// Title screen UI state
    pub title_screen: TitleScreenUiState,
    // Colony system state
    pub colony_manager: ColonyManager,
    pub company: Company,
    pub science: ScienceState,
    pub tech_tree: TechTree,
    pub contracts: ContractManager,
    /// Fleet manager for trade routes and ships
    pub fleet: FleetManager,
    /// Colony notifications (persisted in saves)
    pub notifications: Vec<Notification>,
    /// Body index of the colony currently being viewed in Colony mode
    pub colony_view_body_index: Option<usize>,
    /// Which mode to return to when leaving Colony mode
    pub colony_return_mode: Option<GameMode>,
    /// Which mode to return to when leaving the Tech Tree screen
    pub tech_tree_return_mode: Option<GameMode>,
    /// Dyson swarm state per star (keyed by star body index)
    pub dyson_swarms: std::collections::HashMap<usize, crate::colony::DysonSwarm>,
    /// Procedural galaxy star system
    pub galaxy: GalaxyState,
}

impl Game {
    pub fn new() -> Self {
        // Load part definitions (embedded on wasm, filesystem on desktop).
        let part_definitions = PartDefinitions::load_default();

        // Load blueprints from the platform user-data dir (desktop) or
        // IndexedDB (wasm). Both start empty on first run; the player
        // populates them via Save Blueprint / Import.
        #[cfg(not(target_arch = "wasm32"))]
        let mut blueprints = BlueprintRegistry::new(crate::save::paths::blueprints_dir());
        #[cfg(target_arch = "wasm32")]
        let mut blueprints = BlueprintRegistry::new("");
        if let Err(e) = blueprints.load_all() {
            log::error!("Failed to load blueprints: {}", e);
        }

        // Initialize solar system
        let solar_system = SolarSystem::new();

        // Create initial ship
        let ship = Ship::spawn_on_earth(&solar_system);
        let flight = FlightState::new(ship);

        // Create editor state
        let editor = EditorState::new();

        // Load tech tree (embedded on wasm, filesystem on desktop).
        let tech_tree = TechTree::load_default()
            .unwrap_or_else(|e| {
                log::error!("Failed to load tech tree: {}", e);
                TechTree::default()
            });

        let mut contracts = ContractManager::new();
        let discoveries = crate::colony::DiscoveryTracker::default();
        contracts.refill_pool(&discoveries, &solar_system, solar_system.time);

        Self {
            mode: GameMode::TitleScreen,
            paused: false,
            warp_index: 0,
            solar_system,
            flight,
            editor,
            part_definitions,
            blueprints,
            save_name: None,
            has_launch_save: false,
            title_screen: TitleScreenUiState::default(),
            colony_manager: ColonyManager::new(),
            company: Company::default(),
            science: ScienceState::default(),
            tech_tree,
            contracts,
            fleet: FleetManager::new(),
            notifications: Vec::new(),
            colony_view_body_index: None,
            colony_return_mode: None,
            tech_tree_return_mode: None,
            dyson_swarms: std::collections::HashMap::new(),
            galaxy: GalaxyState::new(),
        }
    }

    /// Switch to title screen (pre-game)
    pub fn enter_title_screen(&mut self) {
        self.mode = GameMode::TitleScreen;
        self.paused = false;
        self.save_name = None;
        self.title_screen = TitleScreenUiState::default();
        log::info!("Entered title screen");
    }

    /// Reset game state for a new game
    pub fn reset_for_new_game(&mut self, name: String) {
        self.solar_system.time = 0.0;
        self.solar_system.init_sectors();
        self.warp_index = 0;
        let ship = Ship::spawn_on_earth(&self.solar_system);
        self.flight = FlightState::new(ship);
        self.editor = EditorState::new();
        self.save_name = Some(name);
        self.mode = GameMode::MainMenu;
        self.paused = false;
        // Reset colony state
        self.colony_manager = ColonyManager::new();
        self.company = Company::default();
        self.science = ScienceState::default();
        self.fleet = FleetManager::new();
        self.notifications = Vec::new();
        self.colony_view_body_index = None;
        self.colony_return_mode = None;
        self.tech_tree_return_mode = None;
        self.tech_tree = TechTree::load_default()
            .unwrap_or_else(|e| {
                log::error!("Failed to load tech tree: {}", e);
                TechTree::default()
            });
        self.dyson_swarms = std::collections::HashMap::new();
        // Reset galaxy state
        self.galaxy = GalaxyState::new();
        // Reset contracts (milestones, active/available contracts)
        self.contracts = ContractManager::new();
        self.contracts.refill_pool(
            &self.science.discoveries,
            &self.solar_system,
            self.solar_system.time,
        );
        log::info!("Started new game: {:?}", self.save_name);
    }

    /// Get the current simulation time (seconds since epoch)
    pub fn time(&self) -> f64 {
        self.solar_system.time
    }

    /// Switch to main menu
    pub fn enter_main_menu(&mut self) {
        self.mode = GameMode::MainMenu;
        self.paused = false;
        self.has_launch_save = false;
        log::info!("Entered main menu");
    }

    /// Switch to editor mode
    pub fn enter_editor(&mut self) {
        self.mode = GameMode::Editor;
        self.paused = false;
        self.has_launch_save = false;
        log::info!("Entered editor mode");
    }

    /// Switch to flight mode (without launching - resume existing flight)
    pub fn enter_flight(&mut self) {
        self.mode = GameMode::Flight;
        self.paused = false;
        log::info!("Entered flight mode");
    }

    /// Switch to tracking station
    pub fn enter_tracking_station(&mut self) {
        self.mode = GameMode::TrackingStation;
        self.paused = false;
        log::info!("Entered tracking station");
    }

    /// Switch to colony management screen
    pub fn enter_colony(&mut self, body_index: usize, from_mode: GameMode) {
        self.colony_view_body_index = Some(body_index);
        self.colony_return_mode = Some(from_mode);
        self.mode = GameMode::Colony;
        self.paused = false;
        log::info!("Entered colony screen for body {}", body_index);
    }

    /// Leave colony management screen, returning to the previous mode
    pub fn leave_colony(&mut self) {
        let return_mode = self.colony_return_mode.take().unwrap_or(GameMode::TrackingStation);
        self.colony_view_body_index = None;
        self.mode = return_mode;
        self.paused = false;
        log::info!("Left colony screen, returning to {:?}", return_mode);
    }

    /// Switch to colony overview screen
    pub fn enter_colony_overview(&mut self) {
        self.mode = GameMode::ColonyOverview;
        self.paused = false;
        log::info!("Entered colony overview");
    }

    /// Switch to management screen
    pub fn enter_management(&mut self) {
        self.mode = GameMode::Management;
        self.paused = false;
        log::info!("Entered management screen");
    }

    /// Switch to tech tree screen, remembering where to return
    pub fn enter_tech_tree(&mut self, from: GameMode) {
        self.tech_tree_return_mode = Some(from);
        self.mode = GameMode::TechTree;
        self.paused = false;
        log::info!("Entered tech tree from {:?}", from);
    }

    /// Leave tech tree screen, returning to the previous mode
    pub fn leave_tech_tree(&mut self) {
        let return_mode = self.tech_tree_return_mode.take().unwrap_or(GameMode::Management);
        self.mode = return_mode;
        self.paused = false;
        log::info!("Left tech tree, returning to {:?}", return_mode);
    }

    /// Get current time warp multiplier
    pub fn time_warp(&self, warp_levels: &[f64]) -> f64 {
        warp_levels.get(self.warp_index).copied().unwrap_or(1.0)
    }

    /// Launch a vessel from the editor
    pub fn launch_from_editor(&mut self, body_index: usize) -> Result<(), String> {
        // Build blueprint from editor state
        let blueprint = self.editor.to_blueprint(&self.part_definitions)?;

        // Check that the vessel has at least one controllable part (pod/probe core)
        let has_control = blueprint.parts.iter().any(|p| {
            self.part_definitions.get(&p.definition_id)
                .and_then(|d| d.pod.as_ref())
                .map_or(false, |pod| pod.can_control)
        });
        if !has_control {
            return Err("Vessel has no controllable part (add a pod or probe core)".to_string());
        }

        // Check funds
        let cost = self.editor.calculate_vessel_cost(&self.part_definitions);
        if cost > self.company.money {
            return Err(format!(
                "Insufficient funds: vessel costs {} but you only have {}",
                crate::colony::format_money(cost),
                crate::colony::format_money(self.company.money),
            ));
        }
        self.company.money -= cost;

        // Get spawn position (on launchpad of target body)
        let launch_body = &self.solar_system.bodies[body_index];
        let surface_angle = LAUNCHPAD_SURFACE_ANGLE;

        // Create flight vessel first to get bounding height
        let vessel = FlightVessel::from_blueprint(
            &blueprint,
            &self.part_definitions,
            [0.0, 0.0],
            [0.0, 0.0],
            body_index,
        )?;

        let surface_distance = launch_body.radius + LAUNCHPAD_HEIGHT + vessel.bottom_extent();

        let spawn_position = [
            surface_distance * surface_angle.cos(),
            surface_distance * surface_angle.sin(),
        ];

        // Recreate vessel at correct position
        let mut vessel = FlightVessel::from_blueprint(
            &blueprint,
            &self.part_definitions,
            spawn_position,
            [0.0, 0.0],
            body_index,
        )?;

        let _weld_connections = vessel.find_weld_connections(&self.part_definitions);

        self.flight.ship.rel_position = spawn_position;
        self.flight.ship.rel_velocity = [0.0, 0.0];
        self.flight.ship.rotation = surface_angle;
        self.flight.ship.rotational_velocity = 0.0;
        self.flight.ship.soi_body = body_index;
        self.flight.ship.throttle = 0.0;
        self.flight.ship.on_rails = false;
        self.flight.ship.cached_orbit = None;
        self.flight.ship.temperature = crate::ship::AMBIENT_TEMPERATURE;
        self.flight.ship.heat_flux = 0.0;
        self.flight.ship.proper_time = 0.0;
        self.flight.ship.mission_time = 0.0;
        self.flight.ship.state = crate::ship::ShipState::Landed {
            body_index,
            surface_angle,
        };

        // Sync vessel state
        vessel.rel_position = spawn_position;
        vessel.rotation = surface_angle;
        self.flight.vessel = Some(vessel);

        // Switch to flight mode
        self.mode = GameMode::Flight;
        self.warp_index = 0;
        self.flight.tracking_ship = true;
        self.flight.ship_input = ShipInput::default();

        // Assign vessel identity
        self.flight.active_vessel_id = self.flight.next_vessel_id;
        self.flight.active_vessel_name = blueprint.name.clone();
        self.flight.next_vessel_id += 1;

        if let Some(ref v) = self.flight.vessel {
            log::info!("Launched vessel: {} ({} parts, {} stages, current_stage={})",
                blueprint.name, v.parts.len(), v.stages.len(), v.current_stage);
            for (i, stage) in v.stages.iter().enumerate() {
                let part_names: Vec<String> = stage.iter().map(|&idx| {
                    if idx < v.parts.len() {
                        format!("{}({})", idx, &v.parts[idx].definition_id)
                    } else {
                        format!("{}(INVALID)", idx)
                    }
                }).collect();
                log::info!("  Stage {}: {:?}", i, part_names);
            }
            let decoupled_count = v.parts.iter().filter(|p| p.decoupled).count();
            let destroyed_count = v.parts.iter().filter(|p| p.destroyed).count();
            if decoupled_count > 0 || destroyed_count > 0 {
                log::warn!("  WARNING: {} decoupled, {} destroyed parts at launch!", decoupled_count, destroyed_count);
            }
        }
        Ok(())
    }

    /// Load a blueprint into the editor.
    /// Returns Err if the blueprint contains parts not yet unlocked in the tech tree.
    pub fn load_blueprint(&mut self, name: &str) -> Result<(), String> {
        let blueprint = self.blueprints.get(name)
            .ok_or_else(|| format!("Blueprint not found: {}", name))?
            .clone();

        // Validate all parts are unlocked
        let locked: Vec<String> = blueprint.parts.iter()
            .filter_map(|p| {
                let def = self.part_definitions.get(&p.definition_id)?;
                if !self.tech_tree.is_part_available(&def.name) {
                    Some(def.name.clone())
                } else {
                    None
                }
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        if !locked.is_empty() {
            return Err(format!("Blueprint contains locked parts: {}", locked.join(", ")));
        }

        self.editor.load_blueprint(&blueprint, &self.part_definitions);
        self.mode = GameMode::Editor;
        Ok(())
    }

    /// Save the current editor state as a blueprint
    pub fn save_blueprint(&mut self, name: String) -> Result<(), String> {
        let mut blueprint = self.editor.to_blueprint(&self.part_definitions)?;
        blueprint.name = name;
        self.blueprints.save(blueprint)
    }

    /// Create a new vessel in the editor
    pub fn new_vessel(&mut self) {
        self.editor.clear();
        self.mode = GameMode::Editor;
    }

    /// Update the game simulation
    pub fn update(&mut self, dt: f64, warp_levels: &[f64]) {
        if self.paused {
            return;
        }

        let time_warp = self.time_warp(warp_levels);

        // Solar system advances in all modes
        self.solar_system.update(dt * time_warp);

        // Ship physics only in flight mode
        if self.mode == GameMode::Flight {
            self.flight.ship.update(
                dt * time_warp,
                time_warp,
                &self.flight.ship_input,
                &self.solar_system,
                None,
                false,
                self.flight.vessel.is_some(),
            );

            if let Some(ref mut vessel) = self.flight.vessel {
                vessel.rel_position = self.flight.ship.rel_position;
                vessel.rel_velocity = self.flight.ship.rel_velocity;
                vessel.rotation = self.flight.ship.rotation;
                vessel.throttle = self.flight.ship.throttle;
            }
        }
    }

    /// Toggle pause state
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if self.paused {
            self.warp_index = 0;
        }
    }

    /// Establish a colony on the given body using cargo container contents.
    pub fn establish_colony(&mut self, body_index: usize) -> Result<(), String> {
        use crate::colony::{BuildingInstance, BuildingType, Colony, ResourceType};
        use crate::colony::notification::{Notification, NotificationKind};
        use crate::ship::ShipState;

        // Validate: ship must be landed on the target body
        match self.flight.ship.state {
            ShipState::Landed { body_index: landed_bi, .. } if landed_bi == body_index => {}
            _ => return Err("Ship is not landed on this body".to_string()),
        }

        // Check no existing colony
        if self.colony_manager.has_colony(body_index) {
            return Err("A colony already exists on this body".to_string());
        }

        // Check not Earth
        if body_index == self.solar_system.earth_index {
            return Err("Cannot establish a colony on Earth".to_string());
        }

        // Check not a gas giant
        if self.solar_system.bodies[body_index].is_gas_giant {
            return Err("Cannot establish a surface colony on a gas giant".to_string());
        }

        // Check vessel has minimum colony buildings in cargo
        let vessel = self.flight.vessel.as_mut()
            .ok_or("No vessel available")?;
        if !vessel.has_colony_buildings() {
            return Err("Cargo must contain at least a Habitat and Small Solar Farm".to_string());
        }

        // Get crew capacity
        let crew = vessel.total_crew_capacity(&self.part_definitions);

        // Extract all cargo from containers (empties them, doesn't destroy parts)
        let (building_names, resources, food_kg) = vessel.extract_all_cargo(&self.part_definitions);

        // Create colony
        let body_name = self.solar_system.bodies[body_index].name.clone();
        let mut colony = Colony::new(body_index, format!("{} Colony", body_name));

        // Add buildings from cargo as operational
        for name in &building_names {
            if let Some(bt) = BuildingType::from_display_name(name) {
                colony.buildings.push(BuildingInstance::new(bt));
            }
        }

        // Add a stockpile if none was included
        if !building_names.iter().any(|n| n == "Stockpile") {
            colony.buildings.push(BuildingInstance::new(BuildingType::Stockpile));
        }

        // Transfer resources from cargo to colony
        for (name, amount) in &resources {
            if let Some(rt) = ResourceType::from_display_name(name) {
                colony.resources.add(rt, *amount);
            }
        }

        // Set food and crew
        colony.food_stored = food_kg;
        // Pre-stock food from Habitats (1,000 kg each)
        let habitat_count = colony.buildings.iter()
            .filter(|b| b.building_type == BuildingType::Habitat).count();
        colony.food_stored += habitat_count as f64 * 1_000.0;
        colony.crew = crew;

        self.colony_manager.add_colony(colony);

        // Push notification
        let colony_name = format!("{} Colony", body_name);
        self.notifications.push(Notification {
            kind: NotificationKind::ColonyEstablished { colony_name },
            time: self.solar_system.time,
            read: false,
        });

        log::info!("Colony established on {} with {} crew, {} buildings from cargo",
            body_name, crew, building_names.len());
        Ok(())
    }

    /// Update all colonies for the given simulation time step.
    /// Check and award one-time discovery milestones (suborbital, orbit, per-body).
    pub fn check_discovery_milestones(&mut self) {
        use crate::colony::economy::{
            SCIENCE_SUBORBITAL, SCIENCE_FIRST_ORBIT, SCIENCE_GEOSTATIONARY,
            body_distance_au, orbit_science_reward, landing_science_reward,
        };

        let ship = &self.flight.ship;
        let earth_idx = self.solar_system.earth_index;
        let discoveries = &mut self.science.discoveries;

        // Compute distance from body center
        let dist = (ship.rel_position[0].powi(2) + ship.rel_position[1].powi(2)).sqrt();
        let body = &self.solar_system.bodies[ship.soi_body];
        let altitude = dist - body.radius;

        // --- Earth milestones ---
        if ship.soi_body == earth_idx {
            // Suborbital: altitude > 100 km
            if !discoveries.first_suborbital && altitude > 100_000.0 {
                if matches!(ship.state, crate::ship::ShipState::Flying) {
                    discoveries.first_suborbital = true;
                    self.science.available += SCIENCE_SUBORBITAL;
                    self.science.cumulative_discovery += SCIENCE_SUBORBITAL;
                    log::info!("Discovery: First suborbital flight! +{} science", SCIENCE_SUBORBITAL);
                }
            }

            // First orbit: in orbit (not suborbital) in Earth SOI
            if !discoveries.first_orbit
                && matches!(ship.state, crate::ship::ShipState::Flying)
                && !ship.is_suborbital(&self.solar_system)
            {
                discoveries.first_orbit = true;
                self.science.available += SCIENCE_FIRST_ORBIT;
                self.science.cumulative_discovery += SCIENCE_FIRST_ORBIT;
                log::info!("Discovery: First Earth orbit! +{} science", SCIENCE_FIRST_ORBIT);
            }

            // Geostationary: altitude > 35,786 km in Earth SOI + in orbit
            if !discoveries.geostationary
                && altitude > 35_786_000.0
                && matches!(ship.state, crate::ship::ShipState::Flying)
                && !ship.is_suborbital(&self.solar_system)
            {
                discoveries.geostationary = true;
                self.science.available += SCIENCE_GEOSTATIONARY;
                self.science.cumulative_discovery += SCIENCE_GEOSTATIONARY;
                log::info!("Discovery: Geostationary altitude! +{} science", SCIENCE_GEOSTATIONARY);
            }
        }

        // --- Per-body orbit ---
        if matches!(ship.state, crate::ship::ShipState::Flying)
            && !ship.is_suborbital(&self.solar_system)
            && !discoveries.body_orbited.contains(&ship.soi_body)
        {
            // Don't count Earth orbit here (handled above as first_orbit)
            if ship.soi_body != earth_idx {
                discoveries.body_orbited.insert(ship.soi_body);
                let dist_au = body_distance_au(ship.soi_body, &self.solar_system);
                let reward = orbit_science_reward(dist_au);
                self.science.available += reward;
                self.science.cumulative_discovery += reward;
                let name = &self.solar_system.bodies[ship.soi_body].name;
                log::info!("Discovery: Orbit around {}! +{:.0} science", name, reward);
            }
        }

        // --- Per-body landing ---
        if let crate::ship::ShipState::Landed { body_index, .. } = ship.state {
            if body_index != earth_idx && !discoveries.body_landed.contains(&body_index) {
                discoveries.body_landed.insert(body_index);
                let dist_au = body_distance_au(body_index, &self.solar_system);
                let reward = landing_science_reward(dist_au);
                self.science.available += reward;
                self.science.cumulative_discovery += reward;
                let name = &self.solar_system.bodies[body_index].name;
                log::info!("Discovery: Landed on {}! +{:.0} science", name, reward);
            }
        }
    }

    /// Check active contracts for completion and award payouts.
    pub fn check_contracts(&mut self) {
        let ship = &self.flight.ship;
        let dist = (ship.rel_position[0].powi(2) + ship.rel_position[1].powi(2)).sqrt();
        let body = &self.solar_system.bodies[ship.soi_body];
        let altitude = dist - body.radius;
        let is_suborbital = ship.is_suborbital(&self.solar_system);

        let has_crew = self.flight.vessel.as_ref()
            .map_or(false, |v| v.total_crew_capacity(&self.part_definitions) > 0);

        // Check payload contracts
        let vessel_payloads = self.flight.vessel.as_ref()
            .map(|v| v.all_payloads())
            .unwrap_or_default();

        let payload_completions = self.contracts.check_payload_contracts(
            &vessel_payloads,
            &ship.state,
            ship.soi_body,
            altitude,
            is_suborbital,
            &self.solar_system,
        );

        for (name, payout) in &payload_completions {
            self.company.money += payout;
            log::info!("Contract completed: {} — {}", name, crate::colony::format_money(*payout));
            self.notifications.push(Notification {
                kind: NotificationKind::ContractCompleted { name: name.clone(), payout: *payout },
                time: self.solar_system.time,
                read: false,
            });
        }

        // Add one new contract per completion
        for _ in &payload_completions {
            self.contracts.refill_one(
                &self.science.discoveries,
                &self.solar_system,
                self.solar_system.time,
            );
        }

        // Check suborbital test flight contracts
        let vessel_parts = self.flight.vessel.as_ref()
            .map(|v| v.parts.as_slice())
            .unwrap_or(&[]);

        let test_completions = self.contracts.check_suborbital_test_contracts(
            vessel_parts,
            &ship.state,
            ship.soi_body,
            altitude,
            &self.solar_system,
        );

        for (name, payout) in &test_completions {
            self.company.money += payout;
            log::info!("Contract completed: {} — {}", name, crate::colony::format_money(*payout));
            self.notifications.push(Notification {
                kind: NotificationKind::ContractCompleted { name: name.clone(), payout: *payout },
                time: self.solar_system.time,
                read: false,
            });
        }

        for _ in &test_completions {
            self.contracts.refill_one(
                &self.science.discoveries,
                &self.solar_system,
                self.solar_system.time,
            );
        }

        // Check tourism destination flags
        self.contracts.check_tourism_destinations(
            &ship.state,
            ship.soi_body,
            altitude,
            is_suborbital,
            has_crew,
            &self.solar_system,
        );
    }

    /// Check and award government milestones. Also update crewed discovery tracking.
    pub fn check_government_milestones(&mut self) {
        let ship = &self.flight.ship;
        let has_crew = self.flight.vessel.as_ref()
            .map_or(false, |v| v.total_crew_capacity(&self.part_definitions) > 0);

        if has_crew {
            let moon_idx = self.solar_system.moon_index;
            let mars_idx = self.solar_system.bodies.iter()
                .position(|b| b.name == "Mars")
                .unwrap_or(usize::MAX);
            let is_suborbital = ship.is_suborbital(&self.solar_system);
            let earth_idx = self.solar_system.earth_index;

            // Track crewed discoveries
            if !self.science.discoveries.first_crewed_orbit
                && ship.soi_body == earth_idx
                && !is_suborbital
                && matches!(ship.state, crate::ship::ShipState::Flying)
            {
                self.science.discoveries.first_crewed_orbit = true;
            }

            if !self.science.discoveries.first_crewed_lunar {
                // Crewed lunar = orbit or landing at moon
                let at_moon = ship.soi_body == moon_idx && matches!(ship.state, crate::ship::ShipState::Flying);
                let landed_moon = matches!(ship.state, crate::ship::ShipState::Landed { body_index, .. } if body_index == moon_idx);
                if at_moon || landed_moon {
                    self.science.discoveries.first_crewed_lunar = true;
                }
            }

            if !self.science.discoveries.first_crewed_mars {
                let at_mars = ship.soi_body == mars_idx && matches!(ship.state, crate::ship::ShipState::Flying);
                let landed_mars = matches!(ship.state, crate::ship::ShipState::Landed { body_index, .. } if body_index == mars_idx);
                if at_mars || landed_mars {
                    self.science.discoveries.first_crewed_mars = true;
                }
            }
        }

        // Check milestones
        let awarded = self.contracts.check_milestones(
            &self.science.discoveries,
            has_crew,
            &self.solar_system,
        );

        for (milestone, payout) in awarded {
            self.company.money += payout;
            let name = milestone.display_name().to_string();
            log::info!("Milestone achieved: {} — {}", name, crate::colony::format_money(payout));
            self.notifications.push(Notification {
                kind: NotificationKind::MilestoneAchieved { name, payout },
                time: self.solar_system.time,
                read: false,
            });
        }
    }

    /// Generate science from R&D budget spending over dt_sim seconds.
    pub fn update_rd_science(&mut self, dt_sim: f64) {
        if dt_sim <= 0.0 || self.company.rd_budget <= 0.0 {
            return;
        }

        let years = dt_sim / (86400.0 * 365.0);
        let budget = self.company.rd_budget;

        // Deduct R&D budget cost from money (stop if broke)
        let cost = budget * years;
        if self.company.money <= 0.0 {
            return;
        }
        let actual_cost = cost.min(self.company.money);
        self.company.money -= actual_cost;
        let actual_years = if cost > 0.0 { years * (actual_cost / cost) } else { 0.0 };

        // base_rate = 25 * ln(1 + budget/1M) per year
        let base_rate = 25.0 * (1.0 + budget / 1_000_000.0).ln();

        // effective_rate = base_rate * max(0, 1 - cumulative_rd/50000) per year
        let exhaustion = (1.0 - self.science.cumulative_rd / 50_000.0).max(0.0);
        let effective_rate = base_rate * exhaustion;

        let science_earned = effective_rate * actual_years;
        if science_earned > 0.0 {
            self.science.available += science_earned;
            self.science.cumulative_rd += science_earned;
        }
    }

    pub fn update_colonies(&mut self, dt_sim: f64) {
        if dt_sim <= 0.0 {
            return;
        }

        let total_days = dt_sim / 86400.0;

        let mut notifications = Vec::new();
        let sim_time = self.solar_system.time;

        // Track previous lab science for delta computation
        let prev_lab_science: Vec<f64> = self.colony_manager.colonies.iter()
            .map(|c| c.lab_science_extracted)
            .collect();

        let num_ticks = (total_days.ceil() as usize).clamp(1, 1000);
        let days_per_tick = total_days / num_ticks as f64;

        for _ in 0..num_ticks {
            for colony in &mut self.colony_manager.colonies {
                let swarm = self.solar_system.parent_star(colony.body_index)
                    .map(|si| self.dyson_swarms.entry(si).or_default());
                crate::colony::simulation::simulate_colony_tick(
                    colony,
                    days_per_tick,
                    &self.solar_system,
                    &mut notifications,
                    sim_time,
                    &self.tech_tree,
                    swarm,
                    Some(&mut self.fleet),
                );
            }
        }

        // Remove empty phantom swarms created by entry().or_default()
        self.dyson_swarms.retain(|_, s| {
            s.mirror_count > 0 || !s.deploying.is_empty()
                || s.collector_count > 0 || !s.deploying_collectors.is_empty()
        });

        // Compute science deltas and add to available science
        for (i, colony) in self.colony_manager.colonies.iter().enumerate() {
            if i < prev_lab_science.len() {
                let delta = colony.lab_science_extracted - prev_lab_science[i];
                if delta > 0.0 {
                    self.science.available += delta;
                    self.science.cumulative_lab += delta;
                }
            }
        }

        // Update fleet transit and automation
        let body_names: Vec<String> = self.solar_system.bodies.iter().map(|b| b.name.clone()).collect();
        self.fleet.update_fleet(
            total_days,
            sim_time,
            &mut self.colony_manager,
            &mut self.company,
            self.solar_system.earth_index,
            &body_names,
            &mut notifications,
            &self.blueprints,
            &self.part_definitions,
        );
        self.fleet.check_automation(
            sim_time,
            &mut self.colony_manager,
            &mut self.company,
            self.solar_system.earth_index,
            &self.solar_system,
            &body_names,
            &mut notifications,
            &self.blueprints,
            &self.part_definitions,
        );

        self.notifications.extend(notifications);
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

/// Format simulation_time (seconds since Jan 1, 2030 00:00 UTC) as a date string.
pub fn format_date(simulation_time: f64) -> String {
    // Jan 1, 2030 = day 0
    const EPOCH_YEAR: i32 = 2030;

    let total_seconds = simulation_time as i64;
    let days_total = total_seconds / 86400;
    let remaining_secs = total_seconds % 86400;
    let hours = remaining_secs / 3600;
    let minutes = (remaining_secs % 3600) / 60;

    fn is_leap(y: i64) -> bool {
        (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
    }

    fn days_in_year(y: i64) -> i64 {
        if is_leap(y) { 366 } else { 365 }
    }

    fn days_in_month(y: i64, m: i64) -> i64 {
        match m {
            1 => 31, 2 => if is_leap(y) { 29 } else { 28 },
            3 => 31, 4 => 30, 5 => 31, 6 => 30,
            7 => 31, 8 => 31, 9 => 30, 10 => 31, 11 => 30, 12 => 31,
            _ => 30,
        }
    }

    let mut year: i64 = EPOCH_YEAR as i64;
    let mut remaining_days = days_total;

    // Fast-path: jump by estimated years to avoid iterating one year at a time
    if remaining_days >= 0 {
        let approx_years = remaining_days / 366; // conservative (leap years)
        if approx_years > 0 {
            year += approx_years;
            remaining_days -= approx_years * 365 + approx_years / 4 - approx_years / 100 + approx_years / 400;
        }
        while remaining_days >= days_in_year(year) {
            remaining_days -= days_in_year(year);
            year += 1;
        }
        // Overshoot correction from the fast-path approximation
        while remaining_days < 0 {
            year -= 1;
            remaining_days += days_in_year(year);
        }
    } else {
        let approx_years = (-remaining_days) / 365;
        if approx_years > 0 {
            year -= approx_years;
            remaining_days += approx_years * 365 + approx_years / 4 - approx_years / 100 + approx_years / 400;
        }
        while remaining_days < 0 {
            year -= 1;
            remaining_days += days_in_year(year);
        }
    }

    let mut month: i64 = 1;
    while month < 12 && remaining_days >= days_in_month(year, month) {
        remaining_days -= days_in_month(year, month);
        month += 1;
    }
    let day = remaining_days + 1; // 1-indexed

    const MONTH_NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    format!(
        "{} {}, {} {:02}:{:02}",
        MONTH_NAMES[(month - 1) as usize], day, year, hours, minutes
    )
}
