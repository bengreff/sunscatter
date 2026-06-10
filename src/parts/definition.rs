use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Size categories for parts (width in grid squares)
/// Tiny=1, Small=3, Medium=5, Large=9, XL=13
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartSize {
    Tiny,   // 1 grid square wide
    Small,  // 3 grid squares wide
    Medium, // 5 grid squares wide
    Large,  // 9 grid squares wide
    XL,     // 13 grid squares wide
}

impl PartSize {
    /// Width in grid squares for this part size
    pub fn grid_width(&self) -> u32 {
        match self {
            PartSize::Tiny => 1,
            PartSize::Small => 3,
            PartSize::Medium => 5,
            PartSize::Large => 9,
            PartSize::XL => 13,
        }
    }

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            PartSize::Tiny => "Tiny",
            PartSize::Small => "Small",
            PartSize::Medium => "Medium",
            PartSize::Large => "Large",
            PartSize::XL => "XL",
        }
    }

    /// All sizes in order
    pub fn all() -> &'static [PartSize] {
        &[PartSize::Tiny, PartSize::Small, PartSize::Medium, PartSize::Large, PartSize::XL]
    }
}

/// Shape of a part for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartShape {
    Rectangle,     // Simple rectangle (width x height)
    Triangle,      // Triangle with base at bottom
    Trapezoid,     // Trapezoid (top_width, bottom_width, height)
    TriangleRight, // Right triangle: vertical edge on right, hypotenuse on left
    TriangleLeft,  // Right triangle: vertical edge on left, hypotenuse on right
}

/// Part categories for organizing in the editor palette
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PartCategory {
    Pods,
    Propulsion,
    FuelTanks,
    Structural,
    Aerodynamic,
    Utility,
    Electricity,
    Heat,
    Interstellar,
    Cargo,
}

impl PartCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            PartCategory::Pods => "Command",
            PartCategory::Propulsion => "Engines",
            PartCategory::FuelTanks => "Fuel Tanks",
            PartCategory::Structural => "Structural",
            PartCategory::Aerodynamic => "Aerodynamic",
            PartCategory::Utility => "Utility",
            PartCategory::Electricity => "Electricity",
            PartCategory::Heat => "Heat",
            PartCategory::Interstellar => "Interstellar",
            PartCategory::Cargo => "Cargo",
        }
    }

    pub fn all() -> &'static [PartCategory] {
        &[
            PartCategory::Pods,
            PartCategory::Propulsion,
            PartCategory::FuelTanks,
            PartCategory::Structural,
            PartCategory::Aerodynamic,
            PartCategory::Utility,
            PartCategory::Electricity,
            PartCategory::Heat,
            PartCategory::Interstellar,
            PartCategory::Cargo,
        ]
    }
}

/// Grid square size in meters (for converting grid units to world units)
pub const GRID_SQUARE_SIZE: f64 = 0.5;

/// Propellant types for engines
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Propellant {
    #[default]
    Kerolox,      // LOX + RP-1 (kerosene)
    Methalox,     // LOX + Methane
    Hydrolox,     // LOX + Hydrogen
    Hydrogen,     // Pure hydrogen (NTR engines, no oxidizer)
    Xenon,        // Xenon (electric propulsion, no oxidizer)
    FusionFuel,   // D+He3 (fusion engines)
    Antimatter,   // Antimatter (AM engines)
    NuclearPulse, // Nuclear pulse units (Orion-style)
}

impl Propellant {
    pub fn display_name(&self) -> &'static str {
        match self {
            Propellant::Kerolox => "LOX/RP-1",
            Propellant::Methalox => "LOX/CH4",
            Propellant::Hydrolox => "LOX/LH2",
            Propellant::Hydrogen => "LH2",
            Propellant::Xenon => "Xenon",
            Propellant::FusionFuel => "D+He3",
            Propellant::Antimatter => "Antimatter",
            Propellant::NuclearPulse => "Pulse Units",
        }
    }

    /// Get the corresponding tank fuel type
    pub fn fuel_type(&self) -> FuelType {
        match self {
            Propellant::Kerolox => FuelType::Rp1,
            Propellant::Methalox => FuelType::Methane,
            Propellant::Hydrolox => FuelType::Hydrogen,
            Propellant::Hydrogen => FuelType::PureHydrogen,
            Propellant::Xenon => FuelType::Xenon,
            Propellant::FusionFuel => FuelType::FusionFuel,
            Propellant::Antimatter => FuelType::Antimatter,
            Propellant::NuclearPulse => FuelType::NuclearPulse,
        }
    }
}

/// Engine-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineData {
    pub thrust_vac: f64,      // kN vacuum thrust
    pub thrust_asl: f64,      // kN sea-level thrust
    pub isp_vac: f64,         // Specific impulse in vacuum (seconds)
    pub isp_asl: f64,         // Specific impulse at sea level
    pub throttleable: bool,   // Can throttle?
    #[serde(default)]
    pub gimbal_range: f64,    // Gimbal range in degrees (0 = fixed)
    #[serde(default)]
    pub propellant: Propellant,  // Propellant type
    #[serde(default)]
    pub alternator_power: f64,  // Watts generated when running
    #[serde(default)]
    pub power_required: f64,    // Watts required to fire (electric propulsion)
    #[serde(default)]
    pub nozzle_offsets: Option<Vec<f64>>,  // X offsets in grid squares for multi-nozzle engines
    #[serde(default)]
    pub secondary_propellant: Option<Propellant>,  // Secondary fuel (e.g. antimatter catalyst)
    #[serde(default)]
    pub secondary_fuel_fraction: f64,  // Fraction of total mass flow that is secondary fuel (0.0-1.0)
    #[serde(default)]
    pub mass_flow_kg_s: Option<f64>,  // Total mass flow at vacuum full throttle (kg/s), auto-computed if absent
    #[serde(default)]
    pub waste_heat_watts: f64,  // Heat generated when firing at full throttle (Watts). Scales with throttle.
}

impl EngineData {
    /// Total mass flow at vacuum full throttle (kg/s)
    pub fn total_mass_flow_kg_s(&self) -> f64 {
        self.mass_flow_kg_s.unwrap_or_else(|| {
            if self.isp_vac > 0.0 {
                (self.thrust_vac * 1000.0) / (9.80665 * self.isp_vac)
            } else {
                0.0
            }
        })
    }

    /// Per-component mass flow rates for display: Vec of (display_name, kg/s)
    pub fn fuel_flows_display(&self) -> Vec<(&'static str, f64)> {
        let total = self.total_mass_flow_kg_s();
        let primary_fraction = 1.0 - self.secondary_fuel_fraction;
        let primary_flow = total * primary_fraction;

        let fuel_type = self.propellant.fuel_type();
        let (ox_per_sq, fuel_per_sq) = fuel_type.propellant_per_grid_square();
        let sum = ox_per_sq + fuel_per_sq;

        let mut flows = Vec::new();
        if sum > 0.0 {
            if ox_per_sq > 0.0 {
                flows.push(("LOX", primary_flow * ox_per_sq / sum));
            }
            flows.push((fuel_type.fuel_display_name(), primary_flow * fuel_per_sq / sum));
        } else {
            flows.push((self.propellant.display_name(), primary_flow));
        }

        if let Some(secondary) = self.secondary_propellant {
            flows.push((secondary.display_name(), total * self.secondary_fuel_fraction));
        }

        flows
    }
}

/// Fuel types for tanks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FuelType {
    #[default]
    Empty,          // No fuel loaded
    Rp1,            // LOX + RP-1 (kerosene)
    Methane,        // LOX + Methane
    Hydrogen,       // LOX + Hydrogen
    Monopropellant, // Monopropellant (no oxidizer)
    PureHydrogen,   // Pure LH2 (NTR engines, no oxidizer)
    Xenon,          // Xenon (electric propulsion, no oxidizer)
    FusionFuel,     // D+He3 cryogenic (fusion engines)
    Antimatter,     // Antimatter containment (AM engines)
    NuclearPulse,   // Nuclear pulse units (Orion-style)
}

impl FuelType {
    pub fn display_name(&self) -> &'static str {
        match self {
            FuelType::Empty => "Empty",
            FuelType::Rp1 => "LOX/RP-1",
            FuelType::Methane => "LOX/CH4",
            FuelType::Hydrogen => "LOX/LH2",
            FuelType::Monopropellant => "Monopropellant",
            FuelType::PureHydrogen => "LH2",
            FuelType::Xenon => "Xenon",
            FuelType::FusionFuel => "D+He3",
            FuelType::Antimatter => "Antimatter",
            FuelType::NuclearPulse => "Pulse Units",
        }
    }

    pub fn all() -> &'static [FuelType] {
        &[FuelType::Empty, FuelType::Rp1, FuelType::Methane, FuelType::Hydrogen, FuelType::Monopropellant, FuelType::PureHydrogen, FuelType::Xenon, FuelType::FusionFuel, FuelType::Antimatter, FuelType::NuclearPulse]
    }

    /// Whether this fuel may be loaded into a standard (unrestricted) tank.
    /// Specialized fuels (Xenon, Antimatter, NuclearPulse) require dedicated tank
    /// parts that lock their fuel type via `TankData::fixed_fuel_type`.
    pub fn is_standard_tank_compatible(self) -> bool {
        !matches!(self, FuelType::Xenon | FuelType::Antimatter | FuelType::NuclearPulse)
    }

    /// Get propellant masses per grid square (in kg)
    /// Returns (oxygen_kg, fuel_kg)
    pub fn propellant_per_grid_square(&self) -> (f64, f64) {
        match self {
            FuelType::Empty => (0.0, 0.0),
            FuelType::Rp1 => (470.0, 185.0),
            FuelType::Methane => (270.0, 75.0),
            FuelType::Hydrogen => (155.0, 25.0),
            FuelType::Monopropellant => (0.0, 200.0),
            FuelType::PureHydrogen => (0.0, 140.0),  // ~70 kg/m³ LH2, no LOX (FF 80%)
            FuelType::Xenon => (0.0, 315.0),  // supercritical, high-pressure COPV (FF 90%)
            FuelType::FusionFuel => (0.0, 158.0),  // D+He3 mix ~79 kg/m³ (FF 82%)
            FuelType::Antimatter => (0.0, 5.0),   // mostly containment mass
            FuelType::NuclearPulse => (0.0, 500.0), // heavy fissile pulse units
        }
    }

    /// Get the fuel display name (non-oxidizer component)
    pub fn fuel_display_name(&self) -> &'static str {
        match self {
            FuelType::Empty => "None",
            FuelType::Rp1 => "RP-1",
            FuelType::Methane => "CH4",
            FuelType::Hydrogen => "LH2",
            FuelType::Monopropellant => "Monopropellant",
            FuelType::PureHydrogen => "LH2",
            FuelType::Xenon => "Xenon",
            FuelType::FusionFuel => "D+He3",
            FuelType::Antimatter => "Antimatter",
            FuelType::NuclearPulse => "Pulse Units",
        }
    }

    /// Get the fuel resource name
    pub fn fuel_resource_name(&self) -> Option<&'static str> {
        match self {
            FuelType::Empty => None,
            FuelType::Rp1 => Some("rp1"),
            FuelType::Methane => Some("methane"),
            FuelType::Hydrogen => Some("hydrogen"),
            FuelType::Monopropellant => Some("monopropellant"),
            FuelType::PureHydrogen => Some("hydrogen"),
            FuelType::Xenon => Some("xenon"),
            FuelType::FusionFuel => Some("fusion_fuel"),
            FuelType::Antimatter => Some("antimatter"),
            FuelType::NuclearPulse => Some("nuclear_pulse"),
        }
    }
}

/// Tank-specific data (for fuel tanks)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TankData {
    pub grid_area: f64,  // Tank volume in grid-area-equivalent units (for capacity calculation)
    /// If set, the tank can only hold this fuel type — the editor selector is hidden and
    /// the part loads with this fuel by default. Used for specialized containment
    /// (Xenon high-pressure COPVs, Penning-trap antimatter arrays, Orion pulse magazines).
    #[serde(default)]
    pub fixed_fuel_type: Option<FuelType>,
}

impl TankData {
    /// Get propellant masses for a given fuel type (in kg)
    /// Returns (oxygen_kg, fuel_kg)
    pub fn propellant_capacity(&self, fuel_type: FuelType) -> (f64, f64) {
        let (ox_per_sq, fuel_per_sq) = fuel_type.propellant_per_grid_square();
        (ox_per_sq * self.grid_area, fuel_per_sq * self.grid_area)
    }
}

/// Decoupler-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecouplerData {
    pub ejection_force: f64,  // Force in kN when decoupling
    #[serde(default)]
    pub is_radial: bool,      // Radial decoupler (separates sideways, not by Y position)
}

/// Pod-specific data (for command modules)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodData {
    pub crew_capacity: u32,
    #[serde(default)]
    pub power_draw: f64,  // Watts consumed
    #[serde(default)]
    pub can_control: bool, // Whether this part provides vessel control
}

/// Battery-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryData {
    pub capacity_wh: f64,  // Storage capacity in Watt-hours
}

/// Solar panel data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolarPanelData {
    pub output_1au: f64,  // Watts generated at 1 AU from the Sun
}

/// Radioisotope thermoelectric generator data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtgData {
    pub output_watts: f64,  // Constant power output in Watts
}

/// Reactor data (fission/fusion/antimatter power sources)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactorData {
    pub output_watts: f64,  // Power output in Watts
    /// Optional fuel consumption. Reactors without this run for free at constant
    /// output (fission RTG-style abstraction). Antimatter reactors set this so
    /// they only produce power while their fuel reserves are non-zero.
    #[serde(default)]
    pub fuel: Option<ReactorFuelData>,
    /// Waste heat generated when running (Watts). Constant while the reactor is producing power.
    #[serde(default)]
    pub waste_heat_watts: f64,
}

/// Per-reactor fuel-consumption configuration. Used by antimatter reactors,
/// which consume an antiproton/hydrogen mix continuously while running.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactorFuelData {
    /// Primary fuel resource the reactor consumes
    pub primary: FuelType,
    /// Optional secondary fuel (e.g. matter half of a matter+antimatter mix)
    #[serde(default)]
    pub secondary: Option<FuelType>,
    /// Fraction of total mass flow that is the secondary fuel (0.0-1.0)
    #[serde(default)]
    pub secondary_fraction: f64,
    /// Total combined mass flow when running (kg/s)
    pub total_kg_s: f64,
}

/// Shield type for interstellar shielding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShieldType {
    Whipple,   // Passive debris shield, effective up to ~0.1c
    FRES,      // Fluid Recirculating Electromagnetic Shield
    Geodesic,  // Geodesic force field (highest tier)
}

/// Shield data for interstellar debris/radiation protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldData {
    pub shield_type: ShieldType,
    pub max_velocity_c: f64,    // Max speed rating as fraction of c
    pub power_base_watts: f64,  // Base power consumption (0 for Whipple)
}

/// Parachute-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParachuteData {
    pub deployed_width: f64,  // Deployed canopy diameter in grid squares
}

/// Greenhouse data (food production for life support)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenhouseData {
    pub food_production_rate: f64,  // kg of food per day
}

/// Radiator tier for waste-heat rejection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadiatorTier {
    HeatPipe,   // Fission era, ~1200K operating temp
    Droplet,    // Fusion era, ~2500K operating temp
    Phononic,   // Antimatter era, ~6000K operating temp
}

/// Radiator-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadiatorData {
    pub tier: RadiatorTier,
    /// Heat rejection capacity at full deployment (Watts)
    pub rejection_watts: f64,
    /// Time to fully deploy / retract the wing (seconds)
    #[serde(default = "default_radiator_deploy_time")]
    pub deploy_time_sec: f64,
}

fn default_radiator_deploy_time() -> f64 { 5.0 }

/// Fairing-specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FairingData {
    pub ejection_force: f64,  // Force in kN when jettisoning
}

/// RCS thruster data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RcsData {
    pub thrust: f64,  // kN per axis
    pub isp: f64,     // Specific impulse (seconds)
    #[serde(default)]
    pub is_mirrored: bool,  // true = left-mount (nozzles point right/up/down)
    /// Multiplier for rotational torque (thrust stays the same for translation)
    #[serde(default)]
    pub torque_multiplier: Option<f64>,
}

/// Cargo container data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoData {
    pub capacity_kg: f64,
}

/// A part definition loaded from RON files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: PartCategory,
    pub mass: f64,            // Dry mass in tonnes
    pub cost: u32,
    pub size: PartSize,
    pub shape: PartShape,
    // Visual dimensions (what is drawn) - can be decimals for fine-tuning
    pub grid_width: f64,      // Visual width in grid squares (can be decimal)
    pub grid_height: f64,     // Visual height in grid squares (can be decimal)
    #[serde(default)]
    pub top_width: Option<f64>, // For trapezoids: visual width at top in grid squares
    // Hitbox dimensions (for placement/collision) - defaults to ceiling of visual if not specified
    #[serde(default)]
    pub hitbox_width: Option<u32>,  // Hitbox width in grid squares (editor placement)
    #[serde(default)]
    pub hitbox_height: Option<u32>, // Hitbox height in grid squares (editor placement)
    // Flight hitbox: used for flight collision and sprite rendering size.
    // Defaults to editor hitbox when not set. Allows editor hitbox to be wider (odd)
    // for grid alignment while sprites render at their natural size.
    // Values are in grid squares and can be fractional (f64).
    #[serde(default)]
    pub flight_hitbox_width: Option<f64>,
    #[serde(default)]
    pub flight_hitbox_height: Option<f64>,
    #[serde(default)]
    pub tech_required: String,
    #[serde(default)]
    pub engine: Option<EngineData>,
    #[serde(default)]
    pub tank: Option<TankData>,
    #[serde(default)]
    pub pod: Option<PodData>,
    #[serde(default)]
    pub decoupler: Option<DecouplerData>,
    #[serde(default)]
    pub rcs: Option<RcsData>,
    #[serde(default)]
    pub cargo: Option<CargoData>,
    #[serde(default)]
    pub fairing: Option<FairingData>,
    #[serde(default)]
    pub battery: Option<BatteryData>,
    #[serde(default)]
    pub solar_panel: Option<SolarPanelData>,
    #[serde(default)]
    pub rtg: Option<RtgData>,
    #[serde(default)]
    pub reactor: Option<ReactorData>,
    #[serde(default)]
    pub shield: Option<ShieldData>,
    #[serde(default)]
    pub radiator: Option<RadiatorData>,
    #[serde(default)]
    pub parachute: Option<ParachuteData>,
    #[serde(default)]
    pub greenhouse: Option<GreenhouseData>,
    #[serde(default)]
    pub resources: HashMap<String, f64>,
    // Thermal properties
    #[serde(default = "default_heat_tolerance")]
    pub max_heat_tolerance: f64,
    #[serde(default = "default_specific_heat")]
    pub specific_heat: f64,
    #[serde(default = "default_emissivity")]
    pub emissivity: f64,
    #[serde(default)]
    pub is_heat_shield: bool,
    #[serde(default)]
    pub mirror_def_id: Option<String>,
}

pub fn default_heat_tolerance() -> f64 { 1000.0 }
fn default_specific_heat() -> f64 { 900.0 }
fn default_emissivity() -> f64 { 0.8 }

/// Welding hitbox extends 5% past the build/flight hitbox
const WELD_HITBOX_PADDING: f64 = 0.05;

impl PartDefinition {
    // --- Visual dimensions (what is drawn) ---

    /// Visual width in meters
    pub fn width(&self) -> f64 {
        self.grid_width * GRID_SQUARE_SIZE
    }

    /// Visual height in meters
    pub fn height(&self) -> f64 {
        self.grid_height * GRID_SQUARE_SIZE
    }

    /// Visual top width in meters (for trapezoids)
    pub fn top_width(&self) -> f64 {
        self.top_width.unwrap_or(self.grid_width) * GRID_SQUARE_SIZE
    }

    // --- Hitbox dimensions (for placement, collision, centering) ---

    /// Hitbox width in grid squares (defaults to ceiling of visual width)
    pub fn hitbox_grid_width(&self) -> u32 {
        self.hitbox_width.unwrap_or_else(|| self.grid_width.ceil() as u32)
    }

    /// Hitbox height in grid squares (defaults to ceiling of visual height)
    pub fn hitbox_grid_height(&self) -> u32 {
        self.hitbox_height.unwrap_or_else(|| self.grid_height.ceil() as u32)
    }

    /// Hitbox width in meters
    pub fn hitbox_width(&self) -> f64 {
        self.hitbox_grid_width() as f64 * GRID_SQUARE_SIZE
    }

    /// Hitbox height in meters
    pub fn hitbox_height(&self) -> f64 {
        self.hitbox_grid_height() as f64 * GRID_SQUARE_SIZE
    }

    // --- Welding hitbox (5% larger than build/flight hitbox) ---

    /// Welding hitbox width in meters
    pub fn weld_hitbox_width(&self) -> f64 {
        self.hitbox_width() * (1.0 + WELD_HITBOX_PADDING)
    }

    /// Welding hitbox height in meters
    pub fn weld_hitbox_height(&self) -> f64 {
        self.hitbox_height() * (1.0 + WELD_HITBOX_PADDING)
    }

    // --- Flight hitbox (for collision in flight and sprite rendering) ---

    /// Flight hitbox width in grid squares (defaults to editor hitbox, can be fractional)
    pub fn flight_hitbox_grid_width(&self) -> f64 {
        self.flight_hitbox_width.unwrap_or_else(|| self.hitbox_grid_width() as f64)
    }

    /// Flight hitbox height in grid squares (defaults to editor hitbox, can be fractional)
    pub fn flight_hitbox_grid_height(&self) -> f64 {
        self.flight_hitbox_height.unwrap_or_else(|| self.hitbox_grid_height() as f64)
    }

    /// Flight hitbox width in meters
    pub fn flight_hitbox_width_m(&self) -> f64 {
        self.flight_hitbox_grid_width() * GRID_SQUARE_SIZE
    }

    /// Flight hitbox height in meters
    pub fn flight_hitbox_height_m(&self) -> f64 {
        self.flight_hitbox_grid_height() * GRID_SQUARE_SIZE
    }

    // --- Rotated dimensions (for parts at 90°/270° rotation) ---

    /// Returns true if rotation is approximately 90° or 270° (i.e., dims should be swapped)
    fn is_rotation_swapped(rotation: f64) -> bool {
        let norm = rotation.rem_euclid(std::f64::consts::TAU);
        let quarter = std::f64::consts::FRAC_PI_2;
        (norm - quarter).abs() < 0.01 || (norm - 3.0 * quarter).abs() < 0.01
    }

    /// Hitbox grid width accounting for rotation (swaps at 90°/270°)
    pub fn rotated_hitbox_grid_width(&self, rotation: f64) -> u32 {
        if Self::is_rotation_swapped(rotation) { self.hitbox_grid_height() } else { self.hitbox_grid_width() }
    }

    /// Hitbox grid height accounting for rotation (swaps at 90°/270°)
    pub fn rotated_hitbox_grid_height(&self, rotation: f64) -> u32 {
        if Self::is_rotation_swapped(rotation) { self.hitbox_grid_width() } else { self.hitbox_grid_height() }
    }

    /// Hitbox width in meters accounting for rotation
    pub fn rotated_hitbox_width(&self, rotation: f64) -> f64 {
        self.rotated_hitbox_grid_width(rotation) as f64 * GRID_SQUARE_SIZE
    }

    /// Hitbox height in meters accounting for rotation
    pub fn rotated_hitbox_height(&self, rotation: f64) -> f64 {
        self.rotated_hitbox_grid_height(rotation) as f64 * GRID_SQUARE_SIZE
    }

    /// Weld hitbox width in meters accounting for rotation
    pub fn rotated_weld_hitbox_width(&self, rotation: f64) -> f64 {
        self.rotated_hitbox_width(rotation) * (1.0 + WELD_HITBOX_PADDING)
    }

    /// Weld hitbox height in meters accounting for rotation
    pub fn rotated_weld_hitbox_height(&self, rotation: f64) -> f64 {
        self.rotated_hitbox_height(rotation) * (1.0 + WELD_HITBOX_PADDING)
    }

    /// Whether this part can be a root part (command pod)
    pub fn can_be_root(&self) -> bool {
        self.category == PartCategory::Pods
    }

    /// Get total mass including resources
    pub fn wet_mass(&self) -> f64 {
        self.mass + self.resources.values().sum::<f64>() * 0.001 // resources in kg
    }
}

/// Container for part definitions from a RON file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartDefinitionFile {
    pub parts: Vec<PartDefinition>,
}

/// Registry of all loaded part definitions
#[derive(Debug, Clone, Default)]
pub struct PartDefinitions {
    parts: HashMap<String, PartDefinition>,
}

impl PartDefinitions {
    pub fn new() -> Self {
        Self {
            parts: HashMap::new(),
        }
    }

    /// Load all part definition files from a directory
    pub fn load_from_directory<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let mut definitions = Self::new();
        let dir = path.as_ref();

        if !dir.exists() {
            return Err(format!("Parts directory does not exist: {:?}", dir));
        }

        for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "ron") {
                log::info!("Loading parts from: {:?}", path);
                definitions.load_file(&path)?;
            }
        }

        log::info!("Loaded {} part definitions", definitions.parts.len());
        Ok(definitions)
    }

    /// Load a single RON file
    pub fn load_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {:?}: {}", path.as_ref(), e))?;
        self.load_str(&format!("{:?}", path.as_ref()), &content)
    }

    /// Parse a RON document already in memory and merge its parts in.
    pub fn load_str(&mut self, source: &str, content: &str) -> Result<(), String> {
        let file: PartDefinitionFile = ron::from_str(content)
            .map_err(|e| format!("Failed to parse {}: {}", source, e))?;

        for part in file.parts {
            log::debug!("  Loaded part: {} ({})", part.name, part.id);
            self.parts.insert(part.id.clone(), part);
        }

        Ok(())
    }

    /// Load the canonical part catalog. On wasm this uses RON files embedded
    /// at compile time (the filesystem is unavailable in the browser); on
    /// desktop it reads from `data/parts/` for fast iteration.
    pub fn load_default() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            let mut defs = Self::new();
            for (name, content) in super::embedded::PARTS_RON {
                if let Err(e) = defs.load_str(name, content) {
                    log::error!("Failed to parse embedded {}: {}", name, e);
                }
            }
            log::info!("Loaded {} part definitions (embedded)", defs.parts.len());
            defs
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::load_from_directory("data/parts").unwrap_or_else(|e| {
                log::error!("Failed to load parts: {}", e);
                Self::new()
            })
        }
    }

    /// Get a part definition by ID
    pub fn get(&self, id: &str) -> Option<&PartDefinition> {
        self.parts.get(id)
    }

    /// Find a part definition by display name
    pub fn find_by_name(&self, name: &str) -> Option<&PartDefinition> {
        self.parts.values().find(|p| p.name == name)
    }

    /// Get all parts in a category, sorted by width then height
    pub fn by_category(&self, category: PartCategory) -> Vec<&PartDefinition> {
        let mut parts: Vec<_> = self.parts
            .values()
            .filter(|p| p.category == category)
            .collect();

        // Sort by width first, then by height
        parts.sort_by(|a, b| {
            a.grid_width.total_cmp(&b.grid_width)
                .then(a.grid_height.total_cmp(&b.grid_height))
        });

        parts
    }

    /// Get all parts in a category and size, sorted by height
    pub fn by_category_and_size(&self, category: PartCategory, size: PartSize) -> Vec<&PartDefinition> {
        let mut parts: Vec<_> = self.parts
            .values()
            .filter(|p| p.category == category && p.size == size)
            .collect();

        // Sort by height
        parts.sort_by(|a, b| a.grid_height.total_cmp(&b.grid_height));

        parts
    }

    /// Check if any parts exist for a category and size
    pub fn has_parts_for_size(&self, category: PartCategory, size: PartSize) -> bool {
        self.parts.values().any(|p| p.category == category && p.size == size)
    }

    /// Get all part definitions
    pub fn all(&self) -> impl Iterator<Item = &PartDefinition> {
        self.parts.values()
    }

    /// Check if any parts are loaded
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Number of parts loaded.
    pub fn len(&self) -> usize {
        self.parts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_part_size_grid() {
        assert_eq!(PartSize::Tiny.grid_width(), 1);
        assert_eq!(PartSize::Small.grid_width(), 3);
        assert_eq!(PartSize::Medium.grid_width(), 5);
        assert_eq!(PartSize::Large.grid_width(), 9);
        assert_eq!(PartSize::XL.grid_width(), 13);
    }

    #[test]
    fn test_all_part_ron_files_parse() {
        let defs = PartDefinitions::load_from_directory("data/parts")
            .expect("parts directory must load");
        assert!(defs.parts.len() > 100, "expected catalog of at least 100 parts");

        for (id, expected_fuel) in &[
            ("tank_am_tiny",      FuelType::Antimatter),
            ("tank_am_small",     FuelType::Antimatter),
            ("tank_am_medium",    FuelType::Antimatter),
            ("tank_am_large",     FuelType::Antimatter),
            ("tank_am_sphere_s",  FuelType::Antimatter),
            ("tank_am_sphere_m",  FuelType::Antimatter),
            ("tank_am_sphere_l",  FuelType::Antimatter),
            ("tank_pulse_tiny",   FuelType::NuclearPulse),
            ("tank_pulse_small",  FuelType::NuclearPulse),
            ("tank_pulse_medium", FuelType::NuclearPulse),
            ("tank_pulse_large",  FuelType::NuclearPulse),
            ("tank_xe_tiny",      FuelType::Xenon),
            ("tank_xe_small",     FuelType::Xenon),
            ("tank_xe_medium",    FuelType::Xenon),
            ("tank_xe_large",     FuelType::Xenon),
        ] {
            let part = defs.get(id).unwrap_or_else(|| panic!("missing part: {}", id));
            let tank = part.tank.as_ref().unwrap_or_else(|| panic!("{} has no tank data", id));
            assert_eq!(
                tank.fixed_fuel_type,
                Some(*expected_fuel),
                "{} should be locked to {:?}",
                id,
                expected_fuel,
            );
        }

        // LH2-specific tanks were deleted; verify they are gone
        for removed in &["tank_h2_tiny", "tank_h2_small", "tank_h2_medium", "tank_h2_large"] {
            assert!(defs.get(removed).is_none(), "{} should be deleted", removed);
        }

        // Per-part Earth cost is positive and derived from material breakdown.
        // (The static `def.cost` RON field is no longer displayed — both the
        // editor info panel and the vessel total now use part_dry_earth_cost.)
        use crate::colony::economy::{part_dry_earth_cost, part_filled_fuel_cost};
        for (id, part) in &defs.parts {
            let cost = part_dry_earth_cost(part);
            assert!(cost > 0.0,
                "{} should have positive Earth cost (mass {} t)", id, part.mass);
        }

        // Loading antimatter into an AM Sphere L adds an enormous fuel cost
        // (16,193 t × $0/kg AM = $0 since AM is non-purchasable on Earth);
        // loading LH2 into a fusion sphere of equal grid_area would add a
        // measurable cost. Sanity-check the helper handles the AM zero case.
        let am_l = defs.get("tank_am_sphere_l").expect("tank_am_sphere_l");
        let am_fuel_cost = part_filled_fuel_cost(am_l, FuelType::Antimatter, 1.0);
        assert_eq!(am_fuel_cost, 0.0,
            "Antimatter has no Earth purchase price, so a full AM Sphere L \
             should add $0 fuel cost (got {})", am_fuel_cost);

        // Smoke test the full palette filter path: with the dev-fixture's
        // tech state, AM Spheres must appear in (FuelTanks, XL).
        let tree = crate::colony::TechTree::load_default()
            .expect("tech tree loads");
        // Mimic the fixture's all-techs-unlocked state.
        let mut all_unlocked: std::collections::HashSet<String> = tree.nodes.iter()
            .map(|n| n.id.clone())
            .collect();
        all_unlocked.insert("basic_rocketry".to_string());
        let mut tree_unlocked = tree;
        tree_unlocked.apply_save_state(
            all_unlocked,
            std::collections::HashMap::new(),
        );
        let xl_fuel_tanks: Vec<&str> = defs
            .by_category_and_size(PartCategory::FuelTanks, PartSize::XL)
            .into_iter()
            .filter(|p| tree_unlocked.is_part_available(&p.name))
            .map(|p| p.name.as_str())
            .collect();
        for sphere in &["AM Sphere S", "AM Sphere M", "AM Sphere L"] {
            assert!(xl_fuel_tanks.contains(sphere),
                "Expected {} in (FuelTanks, XL) palette list. Got: {:?}",
                sphere, xl_fuel_tanks);
        }

        // FuelType compatibility helper
        assert!(!FuelType::Xenon.is_standard_tank_compatible());
        assert!(!FuelType::Antimatter.is_standard_tank_compatible());
        assert!(!FuelType::NuclearPulse.is_standard_tank_compatible());
        assert!(FuelType::Rp1.is_standard_tank_compatible());
        assert!(FuelType::FusionFuel.is_standard_tank_compatible());
        assert!(FuelType::Monopropellant.is_standard_tank_compatible());
        assert!(FuelType::PureHydrogen.is_standard_tank_compatible());

        // Every engine SHALL set flight_hitbox_width and flight_hitbox_height
        // (visible-content extents), so the flight collision matches the
        // visible engine and the part doesn't appear to "float" above an
        // empty hitbox region. See editor/parts/spec.md.
        for (id, part) in &defs.parts {
            if part.engine.is_none() { continue; }
            assert!(part.flight_hitbox_width.is_some(),
                "engine {} missing flight_hitbox_width", id);
            assert!(part.flight_hitbox_height.is_some(),
                "engine {} missing flight_hitbox_height", id);
        }

        // All interstellar engines have zero gimbal.
        for (id, part) in &defs.parts {
            if part.category != PartCategory::Interstellar { continue; }
            let Some(eng) = part.engine.as_ref() else { continue };
            assert_eq!(eng.gimbal_range, 0.0,
                "interstellar engine {} has non-zero gimbal {}",
                id, eng.gimbal_range);
        }

        // AM Torch and Gamma Converter consume Antimatter + Hydrogen (LH2) 50/50.
        for id in &["engine_am_torch", "engine_gamma_conversion"] {
            let part = defs.get(id).expect(id);
            let eng = part.engine.as_ref().expect("engine data");
            assert_eq!(eng.propellant, Propellant::Antimatter,
                "{} primary propellant", id);
            assert_eq!(eng.secondary_propellant, Some(Propellant::Hydrogen),
                "{} secondary propellant (Propellant::Hydrogen = pure LH2)", id);
            assert!((eng.secondary_fuel_fraction - 0.5).abs() < 1e-9,
                "{} secondary fraction should be 0.5, got {}",
                id, eng.secondary_fuel_fraction);
        }

        // AM reactors have ReactorFuelData configured with 50/50 AM+LH2 split.
        for id in &["reactor_am_small", "reactor_am_large"] {
            let part = defs.get(id).expect(id);
            let reactor = part.reactor.as_ref().expect("reactor data");
            let fuel = reactor.fuel.as_ref()
                .unwrap_or_else(|| panic!("{} should consume fuel", id));
            assert_eq!(fuel.primary, FuelType::Antimatter, "{} primary fuel", id);
            assert_eq!(fuel.secondary, Some(FuelType::PureHydrogen),
                "{} secondary fuel", id);
            assert!((fuel.secondary_fraction - 0.5).abs() < 1e-9,
                "{} should split 50/50, got {}", id, fuel.secondary_fraction);
            assert!(fuel.total_kg_s > 0.0, "{} must have positive fuel flow", id);
        }

        // Endgame AM spheres: AM capacity must equal the matching fusion
        // sphere's LH2 capacity, and dry mass must be 2× the fusion sphere.
        for (am_id, fus_id) in &[
            ("tank_am_sphere_s", "tank_sphere_s"),
            ("tank_am_sphere_m", "tank_sphere_m"),
            ("tank_am_sphere_l", "tank_sphere_l"),
        ] {
            let am = defs.get(am_id).expect(am_id);
            let fus = defs.get(fus_id).expect(fus_id);
            let am_cap = am.tank.as_ref().unwrap()
                .propellant_capacity(FuelType::Antimatter).1;
            let lh2_cap = fus.tank.as_ref().unwrap()
                .propellant_capacity(FuelType::PureHydrogen).1;
            assert!((am_cap - lh2_cap).abs() < 1.0,
                "{} AM capacity {} should match {} LH2 capacity {}",
                am_id, am_cap, fus_id, lh2_cap);
            assert!((am.mass - 2.0 * fus.mass).abs() < 0.01,
                "{} mass {} should be 2× {} mass {}",
                am_id, am.mass, fus_id, fus.mass);
        }
    }
}
