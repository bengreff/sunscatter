use crate::colony::ResourceType;

/// Events emitted by the render layer (egui UI, flight HUD, etc.) to be applied
/// to game state by `main.rs` after each frame.
///
/// Replaces a collection of per-event `Option<T>` "request" fields on `RenderState`
/// that were read/cleared each frame. All UI interactions now push variants onto
/// `RenderState::render_requests`; `main.rs` drains the queue in FIFO order.
#[derive(Debug, Clone)]
pub enum RenderRequest {
    /// Toggle a specific engine on/off.
    EngineToggle { part_index: usize, enabled: bool },
    /// Toggle crossfeed on a decoupler.
    CrossfeedToggle { part_index: usize, enabled: bool },
    /// Manually decouple a specific decoupler part.
    Decouple { part_index: usize },
    /// Deploy a fairing base, jettisoning its shroud.
    FairingDeploy { part_index: usize },
    /// Deploy or retract a solar panel.
    SolarDeploy { part_index: usize, deploy: bool },
    /// Deploy or retract a radiator wing.
    RadiatorDeploy { part_index: usize, deploy: bool },
    /// Toggle an active shield on/off.
    ShieldToggle { part_index: usize, active: bool },
    /// Manually re-ignite tripped reactors after a thermal cascade.
    ReactorRestart,
    /// Deploy a parachute.
    ParachuteDeploy { part_index: usize },
    /// Cut a deployed parachute.
    ParachuteCut { part_index: usize },
    /// Create a maneuver node from the transfer planner.
    TransferNode {
        position_angle: f64,
        prograde_dv: f64,
        radial_dv: f64,
        time_to_window: f64,
    },
    /// Establish a new colony on the given body.
    EstablishColony { body_index: usize },
    /// Transfer vessel cargo to a colony on the given body.
    TransferCargo { body_index: usize },
    /// Open the colony screen for the given body.
    OpenColony { body_index: usize },
    /// Debug: teleport ship to low Earth orbit.
    DebugTeleportLeo,
    /// Debug: teleport ship to landed on the given body.
    DebugTeleportBody { body_index: usize },
}

/// Margin from hyperbolic asymptote for trajectory endpoint rendering
pub const HYPERBOLIC_RENDER_MARGIN: f64 = 0.01;

/// Maximum number of line segments for any orbit ellipse. This is the single
/// source of truth — every orbit in the game caps at this value. Actual segment
/// count is adaptive based on screen-space circumference (see `orbit_segments`).
pub const ORBIT_SEGMENTS: u32 = 5120;

/// Compute adaptive orbit segment count from screen-space size.
/// Allocates ~1 segment per 3 pixels of circumference, clamped to [64, ORBIT_SEGMENTS].
pub fn orbit_segments(semi_major_axis_world: f64, zoom: f32, screen_height: f32) -> u32 {
    let circumference_px = std::f64::consts::TAU * semi_major_axis_world
        * zoom as f64 * screen_height as f64 * 0.5;
    (circumference_px / 3.0).clamp(64.0, ORBIT_SEGMENTS as f64) as u32
}

/// Vertex for 2D rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
    pub uv: [f32; 2],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x4,  // color
        2 => Float32x2,  // uv
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }

    /// Create a solid-color vertex (no texture)
    pub fn new(position: [f32; 2], color: [f32; 4]) -> Self {
        Self { position, color, uv: [0.0, 0.0] }
    }

    /// Create a textured vertex. layer_index is stored in color.a.
    pub fn textured(position: [f32; 2], uv: [f32; 2], layer_index: u32) -> Self {
        Self {
            position,
            color: [0.0, 0.0, 0.0, layer_index as f32],
            uv,
        }
    }

    /// Create a sprite vertex. Atlas UV is offset by +2.0 on X to flag the sprite path in the shader.
    /// Tint color multiplies the sampled texture color (use [1,1,1,1] for no tinting).
    pub fn sprite(position: [f32; 2], atlas_uv: [f32; 2], tint: [f32; 4]) -> Self {
        Self {
            position,
            color: tint,
            uv: [atlas_uv[0] + 2.0, atlas_uv[1]],
        }
    }
}

/// Stored body data for hit testing (in world units)
#[derive(Clone)]
pub struct BodyData {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub indicator_radius: f64,
}

/// Orbit data for rendering orbit lines
#[derive(Clone)]
pub struct OrbitRenderData {
    pub parent_x: f64,
    pub parent_y: f64,
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub argument_of_periapsis: f64,
    pub color: [f32; 4],
}

/// Per-nozzle activation state for RCS thrusters
#[derive(Clone, Debug)]
pub struct RcsNozzleState {
    pub lateral: bool,           // Side nozzle (left for right-mount, right for left-mount)
    pub lateral_mirrored: bool,  // Opposite side (right for right-mount); used by bilateral pod RCS
    pub up: bool,                // Top nozzle
    pub down: bool,              // Bottom nozzle
}

/// Render data for a single part in a vessel
#[derive(Clone)]
pub struct ShipPartRenderData {
    pub definition_id: String,
    pub local_x: f64,  // meters, relative to vessel COM
    pub local_y: f64,
    pub rotation: f64,  // part rotation in radians (0 = default orientation)
    pub engine_active: bool,  // true if this engine has fuel and can fire
    // Extended info for part popup
    pub part_index: usize,        // index in FlightVessel.parts
    pub name: String,             // display name from PartDefinition
    pub dry_mass: f64,            // tonnes
    pub hitbox_half_w: f64,       // meters, for click detection
    pub hitbox_half_h: f64,       // meters, for click detection
    pub click_local_y: f64,       // meters, click center (differs from local_y for retracted panels)
    pub click_hitbox_half_h: f64, // meters, click half-height (differs for retracted panels)
    // Engine info
    pub engine_thrust_vac: Option<f64>,  // kN
    pub engine_thrust_asl: Option<f64>,  // kN
    pub engine_isp_vac: Option<f64>,     // s
    pub engine_isp_asl: Option<f64>,     // s
    pub engine_enabled: bool,
    pub engine_no_power: bool,
    pub propellant_name: Option<String>,
    // Tank info
    pub fuel_type_name: Option<String>,
    pub fuel_current: Option<f64>,  // kg (fuel only, not oxidizer)
    pub fuel_max: Option<f64>,      // kg
    pub ox_current: Option<f64>,    // kg (oxidizer)
    pub ox_max: Option<f64>,        // kg
    // Pod info
    pub crew_capacity: Option<u32>,
    pub monoprop_current: Option<f64>,  // kg (monopropellant in this pod)
    pub monoprop_max: Option<f64>,      // kg
    // Battery info
    pub battery_current: Option<f64>,   // Wh stored
    pub battery_max: Option<f64>,       // Wh capacity
    // Solar panel info
    pub solar_output: Option<f64>,      // Current watts (distance-adjusted in flight)
    // RTG info
    pub rtg_output: Option<f64>,        // Constant watts
    // Reactor info
    pub reactor_output: Option<f64>,    // Constant watts
    pub reactor_waste_heat: Option<f64>, // Watts
    // Shield info
    pub shield_type: Option<String>,    // "Whipple", "FRES", "Geodesic"
    pub shield_max_c: Option<f64>,      // Max velocity as fraction of c
    pub shield_power: Option<f64>,      // Base power draw in watts
    pub shield_active: bool,
    // Decoupler info
    pub is_decoupler: bool,
    pub crossfeed_enabled: bool,
    // Gimbal info
    pub gimbal_angle: f64,  // Current gimbal deflection (radians)
    // RCS info
    pub rcs_thrust: Option<f64>,  // kN (Some if this is an RCS part)
    pub rcs_nozzle_state: Option<RcsNozzleState>,  // Per-nozzle activation (None = no plumes)
    // Thermal state
    pub heat_fraction: f32,  // 0.0-1.0 per-part heat for visual tinting (proximity to destruction)
    pub temperature: f64,    // Kelvin, for blackbody glow color
    // Fairing info
    pub is_fairing: bool,
    pub fairing_shape: Option<crate::parts::FairingShape>,
    pub fairing_half: Option<crate::parts::FairingHalf>,
    // Solar panel / radiator deployment
    pub deploy_fraction: f64,
    pub is_solar_panel: bool,
    pub is_radiator: bool,
    pub radiator_tier: Option<crate::parts::RadiatorTier>,
    pub radiator_rejection_watts: Option<f64>,
    // Engine waste heat
    pub engine_waste_heat: Option<f64>,  // Watts
    // Parachute info
    pub is_parachute: bool,
    pub parachute_deployed: bool,
    pub parachute_spent: bool,
    pub parachute_deploy_fraction: f64,
    pub parachute_deployed_width_m: f64,
    pub parachute_fully_deployed: bool,
    pub sprite_half_h: f64,  // Visual sprite half-height in meters (for cable anchoring)
}

/// Ship render data
#[derive(Clone)]
pub struct ShipRenderData {
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
    pub size: f64,
    pub color: [f32; 4],
    pub orbit: Option<ShipOrbitData>,
    pub patched_trajectory: Vec<OrbitSegmentData>,
    pub velocity: f64,
    pub altitude: f64,
    pub soi_body_name: String,
    pub throttle: f64,
    pub time_to_intercept: Option<f64>,
    pub acceleration: f64,  // m/s^2 - ship's current thrust / mass
    pub current_true_anomaly: f64, // Ship's current true anomaly in its orbit
    // Vessel-specific data (None when no vessel loaded)
    pub parts: Option<Vec<ShipPartRenderData>>,
    pub total_mass: Option<f64>,     // tonnes
    pub fuel_fraction: Option<f64>,  // 0.0-1.0
    pub monoprop_fraction: Option<f64>,  // 0.0-1.0
    pub thrust_kn: Option<f64>,
    pub drag_kn: f64,                // kN, aerodynamic drag force
    pub delta_v: Option<f64>,        // m/s, Tsiolkovsky
    pub soi_surface_gravity: f64,    // m/s², for TWR calculation
    pub g_force: f64,                // Felt acceleration in g's (thrust + drag, not gravity)
    // Electricity
    pub power_generation: Option<f64>,       // Watts
    pub power_consumption: Option<f64>,      // Watts
    pub electricity_fraction: Option<f64>,   // 0.0-1.0
    pub electricity_stored: Option<f64>,     // Wh currently stored
    pub electricity_max: Option<f64>,        // Wh max capacity
    // Staging
    pub current_stage: Option<usize>,  // Stages activated so far
    pub total_stages: Option<usize>,   // Total number of stages
    pub stages: Option<Vec<Vec<StagedPartInfo>>>,  // Full stage data for UI
    pub stage_delta_vs: Option<Vec<f64>>,  // Per-stage delta-v (m/s, vacuum)
    pub stage_burn_times: Option<Vec<f64>>,  // Per-stage burn time at 100% thrust (seconds)
    // Thermal state (per-part aero heating, hottest part)
    pub temperature: f64,       // Kelvin
    pub heat_fraction: f32,     // 0.0-1.0 normalized for visual effects
    pub heat_flux: f64,         // W/m² for HUD display
    // Ship-wide waste-heat pool (engines + reactors vs radiators)
    pub thermal_pool_temp: f64,       // Kelvin
    pub thermal_pool_gen_w: f64,      // Waste heat generated this tick (W)
    pub thermal_pool_reject_w: f64,   // Radiator rejection capacity active this tick (W)
    pub reactors_tripped: bool,       // True when thermal cascade has shut down reactors
    // Life support
    pub food_days: Option<f64>,        // Days of food remaining (None if no crew)
    pub total_crew: Option<u32>,       // Living crew count
    pub is_starving: bool,             // True when food == 0 and crew > 0
    // Landing zone state
    pub below_landing_altitude: bool,
    // RCS state
    pub rcs_direction: f64,  // -1.0/0.0/1.0 rotation direction for nozzle activation
    pub rcs_translate: [f64; 2],  // [forward, right] translation for nozzle activation
    // Velocity direction for prograde arrow
    pub velocity_direction: [f64; 2],  // Normalized velocity unit vector (or [0,0] if nearly stationary)
    // Relativistic state
    pub speed_fraction_c: f64,
    pub lorentz_gamma: f64,
    pub proper_time: f64,
    pub mission_time: f64,
    pub is_relativistic: bool,
    pub grav_time_factor: f64,
    // Galaxy view
    pub orbits_root: bool,  // Ship is directly orbiting the root body (Sgr A*)
    pub has_control: bool,  // Whether the vessel has a functioning command pod
}

/// Info about a part in a stage, for the staging UI
#[derive(Clone)]
pub struct StagedPartInfo {
    pub part_index: usize,
    pub name: String,
}

/// Ship orbit data for rendering
#[derive(Clone)]
pub struct ShipOrbitData {
    pub parent_x: f64,
    pub parent_y: f64,
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub argument_of_periapsis: f64,
    pub apoapsis: f64,
    pub periapsis: f64,
    pub orbital_period: f64,
    pub time_to_apoapsis: f64,
    pub time_to_periapsis: f64,
    pub parent_body_radius: f64,
    pub parent_name: String,
    pub retrograde: bool,
}

/// A single segment of a patched conics trajectory for rendering
#[derive(Clone)]
pub struct OrbitSegmentData {
    pub parent_x: f64,
    pub parent_y: f64,
    pub semi_major_axis: f64,
    pub eccentricity: f64,
    pub argument_of_periapsis: f64,
    pub start_true_anomaly: f64,
    pub end_true_anomaly: Option<f64>,
    pub color: [f32; 4],
    pub is_first_segment: bool,
    pub retrograde: bool,
    pub soi_radius: f64,
    pub parent_body_radius: f64,
    pub parent_mass: f64,       // kg - for velocity calculations
    pub parent_idx: usize,      // Index of parent body
    pub render_scale: f64,
    pub start_time: f64,        // Relative seconds from trajectory start to this segment
    pub base_epoch: f64,        // Absolute sim time at trajectory start (start_time is relative to this)
}

/// Delta-V components for a maneuver node
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ManeuverDeltaV {
    pub prograde: f64,    // m/s (positive = prograde, negative = retrograde)
    pub radial_out: f64,  // m/s (positive = radial out, negative = radial in)
}

/// A maneuver node - fixed on the orbit it was created on
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ManeuverNode {
    pub id: u64,
    // Orbit parameters (fixed at creation time)
    pub semi_major_axis: f64,      // Scaled for rendering
    pub eccentricity: f64,
    pub argument_of_periapsis: f64,
    pub parent_x: f64,             // Parent position at creation (scaled)
    pub parent_y: f64,
    pub retrograde: bool,
    // Position on the orbit
    pub true_anomaly: f64,         // Position on the stored orbit
    // Parent body info
    pub parent_idx: usize,
    pub parent_mass: f64,
    pub render_scale: f64,
    // Absolute simulation time at which the node occurs
    pub epoch: f64,
    // Delta-v (original - used for trajectory prediction)
    pub delta_v: ManeuverDeltaV,
    // Remaining delta-v (counts down during burns, displayed in UI)
    pub remaining_delta_v: ManeuverDeltaV,
}

impl Default for ManeuverNode {
    fn default() -> Self {
        Self {
            id: 0,
            semi_major_axis: 0.0,
            eccentricity: 0.0,
            argument_of_periapsis: 0.0,
            parent_x: 0.0,
            parent_y: 0.0,
            retrograde: false,
            true_anomaly: 0.0,
            parent_idx: 0,
            parent_mass: 0.0,
            render_scale: 0.0,
            epoch: 0.0,
            delta_v: ManeuverDeltaV::default(),
            remaining_delta_v: ManeuverDeltaV::default(),
        }
    }
}

impl ManeuverNode {
    /// Calculate total delta-v magnitude (original, for trajectory)
    pub fn total_delta_v(&self) -> f64 {
        (self.delta_v.prograde.powi(2) + self.delta_v.radial_out.powi(2)).sqrt()
    }

    /// Calculate remaining delta-v magnitude (for display)
    pub fn total_remaining_delta_v(&self) -> f64 {
        (self.remaining_delta_v.prograde.powi(2) + self.remaining_delta_v.radial_out.powi(2)).sqrt()
    }

    /// Orbit-relative offset (without parent position) for two-step precision rendering
    pub fn orbit_offset(&self) -> [f64; 2] {
        let e = self.eccentricity;
        let ta = self.true_anomaly;
        let arg_peri = self.argument_of_periapsis;

        let r = if e >= 1.0 {
            let a_abs = self.semi_major_axis.abs();
            let p = a_abs * (e * e - 1.0);
            let denom = 1.0 + e * ta.cos();
            if denom <= 0.001 { self.semi_major_axis.abs() } else { p / denom }
        } else {
            let p = self.semi_major_axis * (1.0 - e * e);
            p / (1.0 + e * ta.cos())
        };

        let angle = ta + arg_peri;
        [r * angle.cos(), r * angle.sin()]
    }

    /// Calculate world position from stored orbit parameters and current parent position
    pub fn world_pos(&self, current_parent_x: f64, current_parent_y: f64) -> [f64; 2] {
        let off = self.orbit_offset();
        [current_parent_x + off[0], current_parent_y + off[1]]
    }

    /// Calculate velocity at this point on the orbit (unscaled, m/s)
    pub fn velocity(&self) -> [f64; 2] {
        let e = self.eccentricity;
        let ta = self.true_anomaly;
        let arg_peri = self.argument_of_periapsis;
        let a_unscaled = self.semi_major_axis / self.render_scale;

        let r_unscaled = if e >= 1.0 {
            let a_abs = a_unscaled.abs();
            let p = a_abs * (e * e - 1.0);
            let denom = 1.0 + e * ta.cos();
            if denom <= 0.001 { a_abs } else { p / denom }
        } else {
            let p = a_unscaled * (1.0 - e * e);
            p / (1.0 + e * ta.cos())
        };

        let mu = 6.67430e-11 * self.parent_mass;
        let v_squared = mu * (2.0 / r_unscaled - 1.0 / a_unscaled);
        let v_mag = if v_squared > 0.0 { v_squared.sqrt() } else { 0.0 };

        let flight_path_angle = (e * ta.sin()).atan2(1.0 + e * ta.cos());
        let direction_sign = if self.retrograde { -1.0 } else { 1.0 };
        let angle = ta + arg_peri;
        let velocity_angle = angle + direction_sign * std::f64::consts::FRAC_PI_2 - flight_path_angle;

        [v_mag * velocity_angle.cos(), v_mag * velocity_angle.sin()]
    }

    /// Get prograde unit vector at this node
    pub fn prograde_unit(&self) -> [f64; 2] {
        let vel = self.velocity();
        let vel_mag = (vel[0].powi(2) + vel[1].powi(2)).sqrt();
        if vel_mag > 0.0 {
            [vel[0] / vel_mag, vel[1] / vel_mag]
        } else {
            [1.0, 0.0]
        }
    }

    /// Get radial out unit vector at this node
    pub fn radial_unit(&self) -> [f64; 2] {
        let prograde = self.prograde_unit();
        [prograde[1], -prograde[0]]
    }
}

/// Render data for a vessel in the tracking station or as a background vessel in flight
#[derive(Clone)]
pub struct TrackingVesselData {
    pub id: u64,
    pub name: String,
    pub color: [f32; 4],
    pub x: f64,       // Absolute position (scaled for rendering)
    pub y: f64,
    pub body_center: [f64; 2],  // SOI body position in render units (large, galaxy-scale)
    pub rel_offset: [f64; 2],   // Vessel offset from SOI body in render units (small, local)
    pub soi_body: usize,
    pub orbit: Option<OrbitRenderData>,
    pub parts: Option<Vec<ShipPartRenderData>>,
    pub rotation: f64,
    pub is_debris: bool,
}

/// Action returned from the tracking station UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingStationAction {
    None,
    FlyVessel(u64), // VesselId
    FocusVessel(u64), // VesselId - focus camera on this vessel
    DeleteVessel(u64), // VesselId - remove vessel from tracking
    FocusBody(usize), // body_index - focus camera on a body
    OpenColony(usize), // body_index - open colony management screen
}

/// Action returned from the main menu UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuAction {
    None,
    Editor,
    TrackingStation,
    Colonies,
    Management,
    Quit,
}

/// Action returned from the trade route UI.
#[derive(Debug, Clone, PartialEq)]
pub enum TradeAction {
    None,
    CreateRoute {
        route: crate::colony::TradeRoute,
    },
    PauseRoute(crate::colony::TradeRouteId),
    ResumeRoute(crate::colony::TradeRouteId),
    DeleteRoute(crate::colony::TradeRouteId),
    DeleteShip(crate::colony::TradeShipId),
    /// Update an existing route (preserves id, assigned_ship_id, last_launch_time).
    EditRoute {
        route_id: crate::colony::TradeRouteId,
        route: crate::colony::TradeRoute,
    },
    /// Request to open the route editor for an existing route.
    OpenEditor(crate::colony::TradeRouteId),
}

/// Action returned from the colony overview screen
#[derive(Debug, Clone, PartialEq)]
pub enum ColonyOverviewAction {
    None,
    OpenColony(usize),
    GoToMainMenu,
    ChangeWarp(usize),
    Trade(TradeAction),
}

/// Action returned from the management screen
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ManagementAction {
    None,
    OpenTechTree,
    GoToMainMenu,
    ChangeWarp(usize),
    AcceptContract(u64),
    CancelContract(u64),
    SetRdBudget(f64),
}

/// Action returned from the full-screen tech tree
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TechTreeScreenAction {
    None,
    Back,
    ChangeWarp(usize),
}

/// A selected navigation target (body or vessel)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectedTarget {
    Body(usize),
    Vessel(u64),
}

/// Compact star info for multi-star system barycenter info panel
pub struct CatalogStarInfo {
    pub name: String,
    pub spectral_type: String,
    pub mass_solar: f64,
    pub radius_solar: f64,
    pub luminosity_solar: f64,
}

/// Compact planet/moon info for the catalog star info panel
pub struct CatalogPlanetInfo {
    pub name: String,
    pub designation: String,
    pub temperature_k: f64,
    pub gravity_g: f64,
    pub habitability: u32,
    pub has_atmosphere: bool,
    pub has_life: bool,
    pub is_moon: bool,
    pub is_gas_giant: bool,
}

/// Static body info for the tracking station info panel
pub struct BodyInfoData {
    pub name: String,
    pub description: String,
    pub radius_m: f64,
    pub surface_gravity_ms2: f64,
    pub mass_kg: f64,
    pub atmosphere_pressure_pa: Option<f64>,
    pub atmosphere_height_m: Option<f64>,
    pub orbit_semi_major_axis_m: Option<f64>,
    pub orbit_eccentricity: Option<f64>,
    pub orbit_period_s: Option<f64>,
    pub mineable_resources: Vec<ResourceType>,
    pub atmospheric_resources: Vec<ResourceType>,
    pub habitability_score: u32,
    pub luminosity_solar: Option<f64>,  // solar luminosities (for stars)
    pub star_type: Option<String>,      // e.g. "G-type Main Sequence" (for stars)
    pub temperature_k: Option<f64>,     // surface temperature in Kelvin (stars only)
    pub soi_radius_m: Option<f64>,      // SOI radius in meters
    pub is_galactic_orbit: bool,        // true → orbit section uses pc/kpc formatting
    // Catalog star extended info
    pub catalog_stars: Vec<CatalogStarInfo>,
    pub catalog_planets: Vec<CatalogPlanetInfo>,
    pub catalog_zone: Option<u8>,
    pub catalog_distance_ly: Option<f32>,
    pub catalog_spectral: Option<String>,  // multi-star: "G2V / K1V / M5.5Ve"
}

/// A single point in the porkchop plot grid
pub struct PorkchopPoint {
    pub ejection_dv: f64,     // total departure delta-v (m/s)
    pub dep_time: f64,        // absolute departure time
    pub tof: f64,             // time of flight (seconds)
}

/// 2D grid of Lambert transfer delta-v values for the porkchop plot
pub struct PorkchopGrid {
    pub points: Vec<Option<PorkchopPoint>>,  // cols * rows, None = invalid
    pub cols: usize,
    pub rows: usize,
    pub dep_start: f64,       // departure range start (sim_time when computed)
    pub dep_end: f64,         // departure range end
    pub tof_min: f64,         // shortest transfer time
    pub tof_max: f64,         // longest transfer time
    pub min_dv: f64,          // for color scaling
    pub max_dv: f64,          // for color scaling
    pub best_idx: Option<usize>,  // index of lowest-dv valid point
    pub target_idx: usize,    // which target this was computed for
}

/// Popup shown when single-clicking a body or vessel
pub struct TargetPopup {
    pub target: SelectedTarget,
    pub name: String,
}

/// Action returned from the pause overlay UI
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseAction {
    None,
    Resume,
    MainMenu,
    RecoverVessel,
    Quicksave,
    LoadQuicksave(String),
    RevertToLaunch,
    RevertToEditor,
}

/// Action returned from the title screen UI
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitleScreenAction {
    None,
    NewGame(String),
    LoadGame(String),
    DeleteGame(String),
    /// Open a file picker to import a `.ron` save into IndexedDB (wasm only).
    ImportSave,
    /// Trigger a browser download of the save with this id (wasm only).
    ExportSave(String),
    QuitGame,
}
