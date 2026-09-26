use serde::{Serialize, Deserialize};
use crate::bodies::{SolarSystem, G, Orbit};
use crate::render::ManeuverNode;

mod orbit;
mod patched_conics;
mod soi;
pub mod transfer;

/// Autopilot target direction for ship rotation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutopilotTarget {
    #[default]
    Off,
    Prograde,
    Retrograde,
    RadialIn,
    RadialOut,
    ManeuverNode,
    Target,
}

/// Ship size in meters (physics space) - fallback when no vessel loaded
pub const SHIP_SIZE: f64 = 10.0;

/// Maximum thrust acceleration in m/s² - fallback when no vessel loaded
pub const MAX_THRUST_ACCELERATION: f64 = 20.0;

/// Rotation acceleration in radians/second² (30 degrees/s/s) - fallback
pub const ROTATION_ACCEL: f64 = 30.0 * std::f64::consts::PI / 180.0;

/// Physics data derived from a FlightVessel, passed into Ship::update()
#[derive(Clone, Debug)]
pub struct VesselPhysicsData {
    pub total_mass: f64,       // tonnes
    pub max_thrust_vac: f64,   // kN
    pub max_thrust_asl: f64,   // kN
    pub vessel_height: f64,    // meters (half-height for collision)
    pub bottom_extent: f64,    // meters (COM to bottom, for surface placement)
    pub moment_of_inertia: f64,
    pub rcs_torque: f64,       // kN·m from RCS thrusters
    pub gimbal_torque: f64,    // kN·m from gimbaled engines (signed: + = CCW)
    pub max_gimbal_torque: f64, // kN·m max possible at full deflection (always positive)
    pub vessel_half_width: f64, // meters (half-width for cross-section)
    pub rcs_translation_force: f64, // kN total from all RCS thrusters for translation
    pub parachute_drag_width: f64, // meters, total deployed parachute width
    pub parachute_drag_multiplier: f64, // 50x when fully deployed (<=2000m), 1x partial
}

/// Rotation drag (natural deceleration) in radians/second² (9 degrees/s/s)
pub const ROTATION_DRAG: f64 = 9.0 * std::f64::consts::PI / 180.0;

/// Maximum rotation speed in radians/second (for reference, kept for compatibility)
pub const ROTATION_SPEED: f64 = 2.0;

/// Throttle change rate per second (0-1 scale)
pub const THROTTLE_RATE: f64 = 0.25;

/// Time warp threshold for on-rails mode (max physics warp)
pub const RAILS_WARP_THRESHOLD: f64 = 10.0;

/// Maximum physics timestep for accurate integration (seconds)
const MAX_PHYSICS_DT: f64 = 0.01;

/// Speed of light in m/s
pub const SPEED_OF_LIGHT: f64 = 2.998e8;
/// Speed of light squared
pub const C_SQUARED: f64 = SPEED_OF_LIGHT * SPEED_OF_LIGHT;
/// Below this speed, relativistic effects are skipped (~2,998 km/s = 0.01c)
pub const RELATIVISTIC_SPEED_THRESHOLD: f64 = 0.01 * SPEED_OF_LIGHT;
/// GM/rc² threshold below which gravitational time dilation is skipped
const GRAV_DILATION_THRESHOLD: f64 = 0.001;

/// Lorentz factor. Returns 1.0 below threshold.
pub fn lorentz_gamma(speed: f64) -> f64 {
    if speed < RELATIVISTIC_SPEED_THRESHOLD { return 1.0; }
    let beta_sq = (speed / SPEED_OF_LIGHT).powi(2);
    1.0 / (1.0 - beta_sq).max(1e-12).sqrt()
}

/// Gravitational time dilation factor √(1 - 2GM/rc²). Returns 1.0 unless compact + above threshold.
pub fn gravitational_time_factor(gm: f64, r: f64, body_is_compact: bool) -> f64 {
    if !body_is_compact || r <= 0.0 { return 1.0; }
    let potential = gm / (r * C_SQUARED);
    if potential < GRAV_DILATION_THRESHOLD { return 1.0; }
    (1.0 - 2.0 * potential).max(1e-12).sqrt()
}

/// Convert Newtonian Δv to relativistic cruise velocity: v = c·tanh(Δv/c)
pub fn relativistic_cruise_velocity(newtonian_dv: f64) -> f64 {
    SPEED_OF_LIGHT * (newtonian_dv / SPEED_OF_LIGHT).tanh()
}

/// Aerodynamic drag coefficient (blunt body)
const DRAG_COEFFICIENT: f64 = 0.4;

/// Specific heat capacity of vessel structure (J/kg/K, aluminum)
const VESSEL_SPECIFIC_HEAT: f64 = 900.0;

/// Stefan-Boltzmann constant (W/m²/K⁴)
const STEFAN_BOLTZMANN: f64 = 5.670374419e-8;

/// Emissivity for radiative cooling
const VESSEL_EMISSIVITY: f64 = 0.8;

/// Ambient / starting temperature (K)
pub const AMBIENT_TEMPERATURE: f64 = 300.0;

/// Sutton-Graves convective heating constant for N₂/O₂ atmosphere
const SUTTON_GRAVES_K: f64 = 1.7415e-4;

/// Maximum number of SOI changes to predict in patched conics
pub const MAX_PATCHED_CONICS: usize = 3;

/// Number of samples for SOI intersection detection
pub(crate) const SOI_INTERSECTION_SAMPLES: usize = 200;

/// SOI exit detection threshold (fraction of SOI radius)
pub(crate) const SOI_EXIT_THRESHOLD: f64 = 0.99;

/// SOI entry detection threshold (fraction of SOI radius)
pub(crate) const SOI_ENTRY_THRESHOLD: f64 = 1.01;

/// Minimum time threshold for SOI intersection (seconds)
pub(crate) const MIN_INTERSECTION_TIME: f64 = 0.01;

/// Binary search iterations for precise SOI crossing
pub(crate) const BINARY_SEARCH_ITERATIONS: usize = 50;

/// Binary search iterations for SOI intersection refinement
pub(crate) const SOI_REFINE_ITERATIONS: usize = 10;

/// Small angle margin for hyperbolic orbit calculations
pub(crate) const HYPERBOLIC_ANGLE_MARGIN: f64 = 0.01;

/// Ship state - either flying through space or landed on a body
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ShipState {
    Flying,
    Landed { body_index: usize, surface_angle: f64 },
}

/// Keyboard input state for ship controls
#[derive(Clone, Debug, Default)]
pub struct ShipInput {
    pub throttle_up: bool,
    pub throttle_down: bool,
    pub throttle_full: bool,
    pub throttle_zero: bool,
    pub rotate_left: bool,
    pub rotate_right: bool,
    pub translate_forward: bool,
    pub translate_backward: bool,
    pub translate_left: bool,
    pub translate_right: bool,
}

/// Cached orbit with mean anomaly for on-rails propagation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShipOrbit {
    pub orbit: Orbit,
    pub mean_anomaly: f64,
    pub parent_idx: usize,
    pub retrograde: bool,
}

/// Full orbital information for UI display
#[derive(Clone, Debug)]
pub struct OrbitalInfo {
    pub orbit: Orbit,
    pub parent_idx: usize,
    pub apoapsis: f64,
    pub periapsis: f64,
    pub orbital_period: f64,
    pub time_to_apoapsis: f64,
    pub time_to_periapsis: f64,
    pub retrograde: bool,
}

/// A single segment of a patched conics trajectory
#[derive(Clone, Debug)]
pub struct PatchedConicSegment {
    pub orbit: Orbit,
    pub parent_idx: usize,
    pub retrograde: bool,
    pub start_true_anomaly: f64,
    pub end_true_anomaly: Option<f64>,
    pub start_time: f64,
    pub end_time: Option<f64>,
}

/// Full patched conics trajectory
#[derive(Clone, Debug)]
pub struct PatchedTrajectory {
    pub segments: Vec<PatchedConicSegment>,
}

/// A flyable spaceship
/// Position and velocity are stored RELATIVE to the current SOI body.
/// Uses struct-level `#[serde(default)]` so old saves missing new fields still load.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Ship {
    pub rel_position: [f64; 2],
    pub rel_velocity: [f64; 2],
    pub rotation: f64,
    pub rotational_velocity: f64,  // rad/s
    pub throttle: f64,
    pub state: ShipState,
    pub color: [f32; 4],
    pub soi_body: usize,
    pub on_rails: bool,
    #[serde(skip)]
    pub(crate) cached_orbit: Option<ShipOrbit>,
    /// Cached patched trajectory to avoid recalculating every frame
    #[serde(skip)]
    pub(crate) cached_trajectory: Option<PatchedTrajectory>,
    /// Frame counter when trajectory was last calculated
    #[serde(skip)]
    pub(crate) trajectory_calc_frame: u64,
    /// SOI body when trajectory was calculated (for cache invalidation)
    #[serde(skip)]
    pub(crate) trajectory_soi_body: usize,
    /// Current frame counter (incremented each update)
    #[serde(skip)]
    pub(crate) frame_counter: u64,
    /// Current vessel temperature (Kelvin)
    pub temperature: f64,
    /// Current aerodynamic heat flux (W/m²) for HUD display
    #[serde(skip)]
    pub heat_flux: f64,
    /// RCS translation input: [forward, right] in vessel-local frame, -1..1 each
    #[serde(skip)]
    pub rcs_translate: [f64; 2],
    /// Ship clock (seconds) — ticks slower at high v or near compact objects
    #[serde(default)]
    pub proper_time: f64,
    /// Coordinate/Earth time elapsed (seconds)
    #[serde(default)]
    pub mission_time: f64,
}

impl Default for Ship {
    fn default() -> Self {
        Self {
            rel_position: [0.0, 0.0],
            rel_velocity: [0.0, 0.0],
            rotation: 0.0,
            rotational_velocity: 0.0,
            throttle: 0.0,
            state: ShipState::Flying,
            color: [1.0, 0.2, 0.2, 1.0],
            soi_body: 0,
            on_rails: false,
            cached_orbit: None,
            cached_trajectory: None,
            trajectory_calc_frame: 0,
            trajectory_soi_body: 0,
            frame_counter: 0,
            temperature: AMBIENT_TEMPERATURE,
            heat_flux: 0.0,
            rcs_translate: [0.0, 0.0],
            proper_time: 0.0,
            mission_time: 0.0,
        }
    }
}

impl Ship {
    /// Create a ship spawned on Earth's surface (Earth is at index 4)
    pub fn spawn_on_earth(solar_system: &SolarSystem) -> Self {
        let earth_index = solar_system.earth_index;
        let earth = &solar_system.bodies[earth_index];
        let earth_radius = earth.radius;
        let earth_mass = earth.mass;

        // Spawn in Low Earth Orbit (LEO) - 400 km altitude (ISS orbit)
        let leo_altitude = 4.0e5;
        let orbital_radius = earth_radius + leo_altitude;

        // Position at top of orbit (angle = PI/2)
        let orbit_angle = std::f64::consts::FRAC_PI_2;
        let rel_position = [
            orbital_radius * orbit_angle.cos(),
            orbital_radius * orbit_angle.sin(),
        ];

        // Circular orbital velocity: v = sqrt(GM/r)
        let orbital_velocity = (G * earth_mass / orbital_radius).sqrt();

        // Velocity perpendicular to position (prograde direction)
        let vel_angle = orbit_angle + std::f64::consts::FRAC_PI_2;
        let rel_velocity = [
            orbital_velocity * vel_angle.cos(),
            orbital_velocity * vel_angle.sin(),
        ];

        let rotation = vel_angle;

        Self {
            rel_position,
            rel_velocity,
            rotation,
            rotational_velocity: 0.0,
            throttle: 0.0,
            state: ShipState::Flying,
            color: [1.0, 0.2, 0.2, 1.0],
            soi_body: earth_index,
            on_rails: false,
            cached_orbit: None,
            cached_trajectory: None,
            trajectory_calc_frame: 0,
            trajectory_soi_body: earth_index,
            frame_counter: 0,
            temperature: AMBIENT_TEMPERATURE,
            heat_flux: 0.0,
            rcs_translate: [0.0, 0.0],
            proper_time: 0.0,
            mission_time: 0.0,
        }
    }

    /// Get absolute position (for rendering)
    pub fn absolute_position(&self, solar_system: &SolarSystem) -> [f64; 2] {
        let soi_pos = solar_system.body_position(self.soi_body);
        [
            soi_pos[0] + self.rel_position[0],
            soi_pos[1] + self.rel_position[1],
        ]
    }

    /// Get absolute velocity (for rendering/SOI transitions)
    pub fn absolute_velocity(&self, solar_system: &SolarSystem) -> [f64; 2] {
        let soi_vel = self.get_body_velocity(self.soi_body, solar_system);
        [
            soi_vel[0] + self.rel_velocity[0],
            soi_vel[1] + self.rel_velocity[1],
        ]
    }

    /// Time in seconds until the ship reaches periapsis.
    /// Returns None if no cached orbit or orbit is hyperbolic.
    pub fn time_to_periapsis(&self, solar_system: &SolarSystem) -> Option<f64> {
        let ship_orbit = self.cached_orbit.as_ref()?;
        let a = ship_orbit.orbit.semi_major_axis;
        if a <= 0.0 { return None; } // hyperbolic
        let parent = &solar_system.bodies[ship_orbit.parent_idx];
        let parent_mass = parent.effective_mass_at(a);
        let n = ship_orbit.orbit.mean_motion(parent_mass);
        if n <= 0.0 { return None; }
        // Periapsis is at mean anomaly = 0
        // For prograde: time = (TAU - M) / n (going forward to M=0)
        // For retrograde: time = M / n (going backward to M=0)
        let m = ship_orbit.mean_anomaly;
        let time = if ship_orbit.retrograde { m / n } else { (std::f64::consts::TAU - m) % std::f64::consts::TAU / n };
        Some(time)
    }

    /// Time in seconds until the ship's orbit reaches a given distance from the SOI body center.
    /// Returns None if the orbit never reaches that distance (periapsis >= target_distance),
    /// if the ship is landed, or if no orbit can be computed.
    /// Falls back to computing the orbit from state vectors if no cached orbit exists.
    pub fn time_to_distance(&self, solar_system: &SolarSystem, target_distance: f64) -> Option<f64> {
        use std::f64::consts::TAU;

        // Get cached orbit, or compute one from state vectors (fixes first-frame-of-warp issue)
        let ship_orbit = match self.cached_orbit.as_ref() {
            Some(orbit) => orbit.clone(),
            None => self.calculate_orbit_with_anomaly(solar_system)?,
        };

        let a = ship_orbit.orbit.semi_major_axis;
        let e = ship_orbit.orbit.eccentricity;
        if e >= 1.0 || a <= 0.0 {
            return None; // only elliptical orbits
        }

        let periapsis = a * (1.0 - e);
        if periapsis >= target_distance {
            return None; // orbit never descends to target
        }

        // Semi-latus rectum
        let p = a * (1.0 - e * e);

        // True anomaly where r = target_distance
        // r = p / (1 + e*cos(v))  =>  cos(v) = (p/r - 1) / e
        let cos_v = (p / target_distance - 1.0) / e;
        if cos_v.abs() > 1.0 {
            return None;
        }
        let v_cross = cos_v.acos();

        let parent = &solar_system.bodies[ship_orbit.parent_idx];
        let parent_mass = parent.effective_mass_at(a);
        let n = ship_orbit.orbit.mean_motion(parent_mass);
        if n <= 0.0 {
            return None;
        }

        let current_m = ship_orbit.mean_anomaly;

        // Two crossing points: v_cross (ascending) and TAU - v_cross (descending)
        let m_ascending = self.true_to_mean_anomaly(&ship_orbit.orbit, v_cross);
        let m_descending = self.true_to_mean_anomaly(&ship_orbit.orbit, TAU - v_cross);

        // Time to each crossing (same pattern as get_orbital_info)
        let time_to_m = |target_m: f64| -> f64 {
            let mut delta_m = target_m - current_m;
            if ship_orbit.retrograde {
                delta_m = -delta_m;
            }
            while delta_m < 0.0 {
                delta_m += TAU;
            }
            while delta_m >= TAU {
                delta_m -= TAU;
            }
            delta_m / n
        };

        let t1 = time_to_m(m_ascending);
        let t2 = time_to_m(m_descending);

        Some(t1.min(t2))
    }

    /// Check if the ship is currently inside an atmosphere
    pub fn in_atmosphere(&self, solar_system: &SolarSystem) -> bool {
        let soi_body = &solar_system.bodies[self.soi_body];
        if let Some(ref atmo) = soi_body.atmosphere {
            let dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
            let altitude = dist - soi_body.radius;
            altitude >= 0.0 && altitude < atmo.visible_height()
        } else {
            false
        }
    }

    /// Check if the ship is below the landing altitude of the current SOI body.
    /// Works for both atmospheric and airless bodies.
    /// Returns true for negative altitudes (inside the body surface), which can
    /// happen for on-rails vessels whose Keplerian orbits pass through the body.
    pub fn below_landing_altitude(&self, solar_system: &SolarSystem) -> bool {
        let soi_body = &solar_system.bodies[self.soi_body];
        let dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
        let altitude = dist - soi_body.radius;
        altitude < soi_body.landing_altitude()
    }

    /// Check if the ship's orbit periapsis dips below the planet's surface.
    /// Used together with position checks to delete vessels that will
    /// inevitably hit the surface.
    pub fn periapsis_below_surface(&self, solar_system: &SolarSystem) -> bool {
        if matches!(self.state, ShipState::Landed { .. }) {
            return false;
        }

        let soi_body = &solar_system.bodies[self.soi_body];
        let threshold = soi_body.radius;

        // Use cached orbit if available
        if let Some(ref ship_orbit) = self.cached_orbit {
            let a = ship_orbit.orbit.semi_major_axis;
            let e = ship_orbit.orbit.eccentricity;
            let periapsis = if e < 1.0 {
                a * (1.0 - e)
            } else {
                a.abs() * (e - 1.0)
            };
            return periapsis < threshold;
        }

        // Fallback: compute from state vectors using vis-viva and angular momentum
        let r = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
        let v2 = self.rel_velocity[0].powi(2) + self.rel_velocity[1].powi(2);
        let mu = G * soi_body.effective_mass_at(r);

        // Semi-major axis from vis-viva: 1/a = 2/r - v²/μ
        let inv_a = 2.0 / r - v2 / mu;
        if inv_a.abs() < 1e-10 {
            return true; // parabolic - will hit surface
        }
        let a = 1.0 / inv_a;

        // Angular momentum magnitude: h = |r × v| (2D cross product)
        let h = (self.rel_position[0] * self.rel_velocity[1] - self.rel_position[1] * self.rel_velocity[0]).abs();

        // Semi-latus rectum: p = h²/μ
        let p = h * h / mu;

        // Eccentricity from p = a(1-e²) for elliptical, p = |a|(e²-1) for hyperbolic
        let e = if a > 0.0 {
            (1.0 - p / a).max(0.0).sqrt()
        } else {
            (1.0 + p / a.abs()).max(0.0).sqrt()
        };

        let periapsis = if a > 0.0 {
            a * (1.0 - e)
        } else {
            a.abs() * (e - 1.0)
        };

        periapsis < threshold
    }

    /// Check if the ship's orbit is suborbital (periapsis below body surface).
    /// Landed state is always considered suborbital.
    pub fn is_suborbital(&self, solar_system: &SolarSystem) -> bool {
        if matches!(self.state, ShipState::Landed { .. }) {
            return true;
        }

        let soi_body = &solar_system.bodies[self.soi_body];

        // Use cached orbit if available
        if let Some(ref ship_orbit) = self.cached_orbit {
            let a = ship_orbit.orbit.semi_major_axis;
            let e = ship_orbit.orbit.eccentricity;
            let periapsis = if e < 1.0 {
                a * (1.0 - e) // elliptical
            } else {
                a.abs() * (e - 1.0) // hyperbolic
            };
            return periapsis < soi_body.radius;
        }

        // Fallback: compute from state vectors using vis-viva and angular momentum
        let r = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
        let v2 = self.rel_velocity[0].powi(2) + self.rel_velocity[1].powi(2);
        let mu = G * soi_body.effective_mass_at(r);

        // Semi-major axis from vis-viva: 1/a = 2/r - v²/μ
        let inv_a = 2.0 / r - v2 / mu;
        if inv_a.abs() < 1e-10 {
            return true; // parabolic - will hit surface
        }
        let a = 1.0 / inv_a;

        // Angular momentum magnitude: h = |r × v| (2D cross product)
        let h = (self.rel_position[0] * self.rel_velocity[1] - self.rel_position[1] * self.rel_velocity[0]).abs();

        // Semi-latus rectum: p = h²/μ
        let p = h * h / mu;

        // Eccentricity from p = a(1-e²) for elliptical, p = |a|(e²-1) for hyperbolic
        let e = if a > 0.0 {
            (1.0 - p / a).max(0.0).sqrt()
        } else {
            (1.0 + p / a.abs()).max(0.0).sqrt()
        };

        let periapsis = if a > 0.0 {
            a * (1.0 - e)
        } else {
            a.abs() * (e - 1.0)
        };

        periapsis < soi_body.radius
    }

    /// Update ship physics for one frame
    pub fn update(&mut self, dt: f64, time_warp: f64, input: &ShipInput, solar_system: &SolarSystem, vessel: Option<&VesselPhysicsData>, autopilot_active: bool, has_flight_vessel: bool) {
        let in_landing_zone = self.below_landing_altitude(solar_system);
        let actually_thrusting = self.throttle > 0.0
            && vessel.map(|v| v.max_thrust_vac > 0.0).unwrap_or(false);
        let should_be_on_rails = time_warp > RAILS_WARP_THRESHOLD
            && matches!(self.state, ShipState::Flying)
            && !actually_thrusting
            && !in_landing_zone;

        if should_be_on_rails && !self.on_rails {
            self.enter_rails_mode(solar_system);
        } else if !should_be_on_rails && self.on_rails {
            self.exit_rails_mode(solar_system);
        }

        if self.on_rails {
            self.throttle = 0.0;
        } else {
            self.update_throttle(dt, input);
        }

        match &self.state {
            ShipState::Flying => {
                if self.on_rails {
                    self.update_on_rails(dt, solar_system);
                } else {
                    let effective_dt = if self.throttle > 0.0 && time_warp > RAILS_WARP_THRESHOLD {
                        dt / time_warp * RAILS_WARP_THRESHOLD
                    } else {
                        dt
                    };
                    self.update_flying(effective_dt, input, solar_system, vessel, autopilot_active, has_flight_vessel);
                }
            }
            ShipState::Landed { body_index, surface_angle } => {
                let body_index = *body_index;
                let surface_angle = *surface_angle;
                self.update_landed(dt, input, solar_system, body_index, surface_angle, vessel);
            }
        }
    }

    /// Enter on-rails mode - calculate and cache the current orbit
    pub fn enter_rails_mode(&mut self, solar_system: &SolarSystem) {
        if let Some(ship_orbit) = self.calculate_orbit_with_anomaly(solar_system) {
            self.cached_orbit = Some(ship_orbit);
            self.on_rails = true;
            self.rotational_velocity = 0.0;
        }
    }

    /// Ensure this ship is on-rails with a valid cached orbit.
    /// Handles the case where on_rails is true but cached_orbit is None
    /// (after deserialization, since cached_orbit is serde-skipped).
    pub fn ensure_on_rails(&mut self, solar_system: &SolarSystem) {
        if !self.on_rails || self.cached_orbit.is_none() {
            self.enter_rails_mode(solar_system);
        }
    }

    /// Exit on-rails mode - restore position and velocity from orbit
    pub fn exit_rails_mode(&mut self, solar_system: &SolarSystem) {
        if let Some(ref ship_orbit) = self.cached_orbit {
            let parent = &solar_system.bodies[ship_orbit.parent_idx];
            let parent_mass = parent.effective_mass_at(ship_orbit.orbit.semi_major_axis);
            self.rel_position = ship_orbit.orbit.position_from_mean_anomaly(
                ship_orbit.mean_anomaly,
                parent_mass,
            );
            self.rel_velocity = ship_orbit.orbit.velocity_from_mean_anomaly_with_direction(
                ship_orbit.mean_anomaly,
                parent_mass,
                ship_orbit.retrograde,
            );
        }
        self.on_rails = false;
    }

    /// Update throttle based on input
    fn update_throttle(&mut self, dt: f64, input: &ShipInput) {
        if input.throttle_full {
            self.throttle = 1.0;
        } else if input.throttle_zero {
            self.throttle = 0.0;
        } else {
            if input.throttle_up {
                self.throttle += THROTTLE_RATE * dt;
            }
            if input.throttle_down {
                self.throttle -= THROTTLE_RATE * dt;
            }
        }
        self.throttle = self.throttle.clamp(0.0, 1.0);
    }

    /// Update while on rails - follow the orbit exactly
    pub fn update_on_rails(&mut self, dt: f64, solar_system: &SolarSystem) {
        if let Some(ref mut ship_orbit) = self.cached_orbit {
            let parent = &solar_system.bodies[ship_orbit.parent_idx];
            let parent_mass = parent.effective_mass_at(ship_orbit.orbit.semi_major_axis);

            let mean_motion = ship_orbit.orbit.mean_motion(parent_mass);
            let direction = if ship_orbit.retrograde { -1.0 } else { 1.0 };
            ship_orbit.mean_anomaly += direction * mean_motion * dt;

            ship_orbit.mean_anomaly = ship_orbit.mean_anomaly.rem_euclid(std::f64::consts::TAU);

            self.rel_position = ship_orbit.orbit.position_from_mean_anomaly(
                ship_orbit.mean_anomaly,
                parent_mass,
            );
            self.rel_velocity = ship_orbit.orbit.velocity_from_mean_anomaly_with_direction(
                ship_orbit.mean_anomaly,
                parent_mass,
                ship_orbit.retrograde,
            );

            {
                let speed = (self.rel_velocity[0].powi(2) + self.rel_velocity[1].powi(2)).sqrt();
                let gamma = lorentz_gamma(speed);
                let dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
                let grav_factor = gravitational_time_factor(
                    G * parent.effective_mass_at(dist), dist, parent.is_compact(),
                );
                self.proper_time += dt * grav_factor / gamma;
                self.mission_time += dt;
            }

            self.check_soi_transition_on_rails(solar_system, dt);
        }

        // Radiative cooling while on rails (exponential decay toward ambient)
        if self.temperature > AMBIENT_TEMPERATURE {
            self.temperature += (AMBIENT_TEMPERATURE - self.temperature) * (1.0 - (-0.01 * dt).exp());
            self.temperature = self.temperature.max(AMBIENT_TEMPERATURE);
        }
        self.heat_flux = 0.0;
    }

    /// Update while flying (physics simulation with sub-stepping)
    fn update_flying(&mut self, dt: f64, input: &ShipInput, solar_system: &SolarSystem, vessel: Option<&VesselPhysicsData>, autopilot_active: bool, has_flight_vessel: bool) {
        // Gimbal torque always applies (set by manual input or autopilot)
        let gimbal_accel = vessel
            .map(|v| if v.moment_of_inertia > 0.0 { v.gimbal_torque / v.moment_of_inertia } else { 0.0 })
            .unwrap_or(0.0);
        self.rotational_velocity += gimbal_accel * 0.5 * dt;

        // Manual rotation: reaction wheels + drag (skip when autopilot controls rotation)
        if !autopilot_active {
            let rw_accel = vessel
                .map(|v| if v.moment_of_inertia > 0.0 { v.rcs_torque / v.moment_of_inertia } else { ROTATION_ACCEL })
                .unwrap_or(ROTATION_ACCEL);

            if input.rotate_left {
                self.rotational_velocity += rw_accel * dt;
            } else if input.rotate_right {
                self.rotational_velocity -= rw_accel * dt;
            } else {
                // Apply drag when no rotation input
                if self.rotational_velocity.abs() > 0.0 {
                    let drag = ROTATION_DRAG * dt;
                    if self.rotational_velocity > drag {
                        self.rotational_velocity -= drag;
                    } else if self.rotational_velocity < -drag {
                        self.rotational_velocity += drag;
                    } else {
                        self.rotational_velocity = 0.0;
                    }
                }
            }
            self.rotation += self.rotational_velocity * dt;
        }

        // Cap physics substeps to prevent lag at high time warp when not on rails
        const MAX_SUBSTEPS: usize = 1000;
        let num_steps = ((dt / MAX_PHYSICS_DT).ceil() as usize).clamp(1, MAX_SUBSTEPS);
        let sub_dt = dt / num_steps as f64;

        for _ in 0..num_steps {
            self.physics_substep(sub_dt, input, solar_system, vessel);
        }

        // Proper time accumulation (once per frame, after all substeps)
        {
            let soi_body = &solar_system.bodies[self.soi_body];
            let speed = (self.rel_velocity[0].powi(2) + self.rel_velocity[1].powi(2)).sqrt();
            let gamma = lorentz_gamma(speed);
            let dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
            let grav_factor = gravitational_time_factor(
                G * soi_body.effective_mass_at(dist), dist, soi_body.is_compact(),
            );
            self.proper_time += dt * grav_factor / gamma;
            self.mission_time += dt;
        }

        // Update temperature from aerodynamic heating (once per frame)
        // Per-part system handles this when a flight vessel exists
        self.update_temperature(dt, solar_system, vessel, has_flight_vessel);

        self.check_and_handle_collisions(solar_system, vessel);
    }

    /// Compute aerodynamic environment (density, airspeed, airspeed direction in world coords).
    /// Returns None if not in atmosphere or density too low.
    pub fn compute_aero_environment(&self, solar_system: &SolarSystem) -> Option<(f64, f64, [f64; 2])> {
        let soi_body = &solar_system.bodies[self.soi_body];
        let atmo = soi_body.atmosphere.as_ref()?;

        let dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
        let altitude = dist - soi_body.radius;
        if altitude < 0.0 || altitude > atmo.visible_height() {
            return None;
        }

        let density = atmo.density_at_altitude(altitude);
        if density < 1e-15 {
            return None;
        }

        // Surface-relative airspeed
        let surface_vel = soi_body.surface_velocity_at(dist);
        let radial_x = self.rel_position[0] / dist;
        let radial_y = self.rel_position[1] / dist;
        let tangent_x = -radial_y;
        let tangent_y = radial_x;
        let airspeed_x = self.rel_velocity[0] - surface_vel * tangent_x;
        let airspeed_y = self.rel_velocity[1] - surface_vel * tangent_y;
        let airspeed = (airspeed_x.powi(2) + airspeed_y.powi(2)).sqrt();

        if airspeed < 1.0 {
            return None;
        }

        let dir = [airspeed_x / airspeed, airspeed_y / airspeed];
        Some((density, airspeed, dir))
    }

    /// Update vessel temperature from aerodynamic heating and radiative cooling.
    /// Only used when no FlightVessel is present (per-part system handles that case).
    fn update_temperature(&mut self, dt: f64, solar_system: &SolarSystem, vessel: Option<&VesselPhysicsData>, has_flight_vessel: bool) {
        // When a flight vessel exists, per-part heating handles everything
        if has_flight_vessel {
            return;
        }

        let aero = self.compute_aero_environment(solar_system);

        let (density, airspeed, airspeed_dir) = match aero {
            Some(a) => a,
            None => {
                // No atmosphere or too thin: radiative cooling toward ambient
                self.heat_flux = 0.0;
                if self.temperature > AMBIENT_TEMPERATURE {
                    let (half_width, half_height) = vessel
                        .map(|v| (v.vessel_half_width, v.vessel_height))
                        .unwrap_or((SHIP_SIZE / 4.0, SHIP_SIZE / 2.0));
                    let perimeter = 2.0 * (half_width * 2.0 + half_height * 2.0);
                    let total_mass_kg = vessel
                        .map(|v| v.total_mass * 1000.0)
                        .unwrap_or(1000.0);
                    let q_out = VESSEL_EMISSIVITY * STEFAN_BOLTZMANN * (self.temperature.powi(4) - AMBIENT_TEMPERATURE.powi(4)) * perimeter;
                    let d_temp = q_out / (total_mass_kg * VESSEL_SPECIFIC_HEAT) * dt;
                    self.temperature = (self.temperature - d_temp).max(AMBIENT_TEMPERATURE);
                }
                return;
            }
        };

        let (half_width, half_height) = vessel
            .map(|v| (v.vessel_half_width, v.vessel_height))
            .unwrap_or((SHIP_SIZE / 4.0, SHIP_SIZE / 2.0));

        let velocity_angle = airspeed_dir[1].atan2(airspeed_dir[0]);
        let aoa = (self.rotation - velocity_angle).sin().abs();
        let frontal_area = half_width * 2.0 * (1.0 - aoa) + half_height * 2.0 * aoa;

        // Sutton-Graves heat input: q_in = K * sqrt(density) * airspeed^3 * frontal_area
        let q_in = SUTTON_GRAVES_K * density.sqrt() * airspeed.powi(3) * frontal_area;

        // Radiative cooling (net radiation): q_out = emissivity * sigma * (T^4 - T_ambient^4) * surface_area
        let perimeter = 2.0 * (half_width * 2.0 + half_height * 2.0);
        let q_out = VESSEL_EMISSIVITY * STEFAN_BOLTZMANN * (self.temperature.powi(4) - AMBIENT_TEMPERATURE.powi(4)) * perimeter;

        let total_mass_kg = vessel
            .map(|v| v.total_mass * 1000.0)
            .unwrap_or(1000.0);

        let d_temp = (q_in - q_out) / (total_mass_kg * VESSEL_SPECIFIC_HEAT) * dt;
        self.temperature += d_temp;
        self.temperature = self.temperature.max(AMBIENT_TEMPERATURE);
        self.heat_flux = q_in / frontal_area.max(0.01);
    }

    /// Single physics sub-step using velocity Verlet integration
    fn physics_substep(&mut self, dt: f64, _input: &ShipInput, solar_system: &SolarSystem, vessel: Option<&VesselPhysicsData>) {
        let soi_body = &solar_system.bodies[self.soi_body];

        let calc_gravity_accel = |pos: [f64; 2]| -> [f64; 2] {
            let dist_sq = pos[0] * pos[0] + pos[1] * pos[1];
            let dist = dist_sq.sqrt();
            if dist > soi_body.radius {
                let accel_mag = G * soi_body.effective_mass_at(dist) / dist_sq;
                [-pos[0] / dist * accel_mag, -pos[1] / dist * accel_mag]
            } else {
                [0.0, 0.0]
            }
        };

        // Compute atmospheric pressure fraction (0.0 = vacuum, 1.0 = sea level)
        let atmo_pressure = soi_body.atmosphere.as_ref().map_or(0.0, |atmo| {
            let dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
            let alt = dist - soi_body.radius;
            if alt >= 0.0 {
                (atmo.pressure_at_altitude(alt) / 101_325.0).clamp(0.0, 1.0)
            } else {
                1.0
            }
        });

        // Use vessel-derived thrust acceleration if available, else fallback
        // Interpolate between vacuum and sea-level thrust based on atmospheric pressure
        let max_thrust_accel = vessel
            .map(|v| if v.total_mass > 0.0 {
                let thrust = v.max_thrust_vac * (1.0 - atmo_pressure) + v.max_thrust_asl * atmo_pressure;
                thrust / v.total_mass
            } else { 0.0 })
            .unwrap_or(MAX_THRUST_ACCELERATION);

        let thrust_accel = if self.throttle > 0.0 {
            let mag = self.throttle * max_thrust_accel;
            let raw = [self.rotation.cos() * mag, self.rotation.sin() * mag];
            let speed_sq = self.rel_velocity[0].powi(2) + self.rel_velocity[1].powi(2);
            if speed_sq > RELATIVISTIC_SPEED_THRESHOLD * RELATIVISTIC_SPEED_THRESHOLD {
                let speed = speed_sq.sqrt();
                let gamma = lorentz_gamma(speed);
                let gamma_cubed = gamma * gamma * gamma;
                let v_hat = [self.rel_velocity[0] / speed, self.rel_velocity[1] / speed];
                let a_dot_v = raw[0] * v_hat[0] + raw[1] * v_hat[1];
                let a_long = [a_dot_v * v_hat[0], a_dot_v * v_hat[1]];
                let a_trans = [raw[0] - a_long[0], raw[1] - a_long[1]];
                [a_long[0] / gamma_cubed + a_trans[0] / gamma,
                 a_long[1] / gamma_cubed + a_trans[1] / gamma]
            } else { raw }
        } else {
            [0.0, 0.0]
        };

        // RCS translation acceleration (vessel-local forward/right mapped to world)
        let rcs_accel = vessel
            .filter(|v| v.rcs_translation_force > 0.0 && v.total_mass > 0.0)
            .map(|v| {
                let translate_mag = (self.rcs_translate[0].powi(2) + self.rcs_translate[1].powi(2)).sqrt();
                if translate_mag < 0.001 {
                    return [0.0, 0.0];
                }
                let accel_mag = v.rcs_translation_force / v.total_mass;
                let fwd = [self.rotation.cos(), self.rotation.sin()];
                let right = [self.rotation.sin(), -self.rotation.cos()];
                [
                    (fwd[0] * self.rcs_translate[0] + right[0] * self.rcs_translate[1]) * accel_mag,
                    (fwd[1] * self.rcs_translate[0] + right[1] * self.rcs_translate[1]) * accel_mag,
                ]
            })
            .unwrap_or([0.0, 0.0]);

        // Compute aerodynamic drag acceleration
        let drag_accel = self.compute_drag_accel(soi_body, vessel);

        let grav_accel_1 = calc_gravity_accel(self.rel_position);
        let accel_1 = [
            grav_accel_1[0] + thrust_accel[0] + rcs_accel[0] + drag_accel[0],
            grav_accel_1[1] + thrust_accel[1] + rcs_accel[1] + drag_accel[1],
        ];

        let prev_rel_pos = self.rel_position;
        let prev_rel_vel = self.rel_velocity;

        self.rel_position[0] += self.rel_velocity[0] * dt + 0.5 * accel_1[0] * dt * dt;
        self.rel_position[1] += self.rel_velocity[1] * dt + 0.5 * accel_1[1] * dt * dt;

        let grav_accel_2 = calc_gravity_accel(self.rel_position);
        let accel_2 = [
            grav_accel_2[0] + thrust_accel[0] + rcs_accel[0] + drag_accel[0],
            grav_accel_2[1] + thrust_accel[1] + rcs_accel[1] + drag_accel[1],
        ];

        self.rel_velocity[0] += 0.5 * (accel_1[0] + accel_2[0]) * dt;
        self.rel_velocity[1] += 0.5 * (accel_1[1] + accel_2[1]) * dt;

        self.check_soi_transition_precise(solar_system, prev_rel_pos, prev_rel_vel, dt);
    }

    /// Compute aerodynamic drag acceleration (m/s²) opposing airspeed
    pub fn compute_drag_accel(&self, soi_body: &crate::bodies::CelestialBody, vessel: Option<&VesselPhysicsData>) -> [f64; 2] {
        let atmo = match &soi_body.atmosphere {
            Some(a) => a,
            None => return [0.0, 0.0],
        };

        let dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
        let altitude = dist - soi_body.radius;
        if altitude < 0.0 || altitude > atmo.visible_height() {
            return [0.0, 0.0];
        }

        let density = atmo.density_at_altitude(altitude);
        if density < 1e-12 {
            return [0.0, 0.0];
        }

        // Surface-relative airspeed: subtract body rotation velocity
        let surface_vel = soi_body.surface_velocity_at(dist);
        // Rotation velocity is perpendicular to radial direction (tangential, CCW)
        let radial_x = self.rel_position[0] / dist;
        let radial_y = self.rel_position[1] / dist;
        let tangent_x = -radial_y;
        let tangent_y = radial_x;

        let airspeed_x = self.rel_velocity[0] - surface_vel * tangent_x;
        let airspeed_y = self.rel_velocity[1] - surface_vel * tangent_y;
        let airspeed = (airspeed_x.powi(2) + airspeed_y.powi(2)).sqrt();
        if airspeed < 0.01 {
            return [0.0, 0.0];
        }

        // Orientation-dependent cross-section
        let (half_width, half_height) = vessel
            .map(|v| (v.vessel_half_width, v.vessel_height))
            .unwrap_or((SHIP_SIZE / 4.0, SHIP_SIZE / 2.0));

        let velocity_angle = airspeed_y.atan2(airspeed_x);
        let aoa = (self.rotation - velocity_angle).sin().abs(); // 0 = nose-on, 1 = broadside
        let body_cross_section = half_width * 2.0 * (1.0 - aoa) + half_height * 2.0 * aoa;
        // Parachute drag is orientation-independent (multiplier depends on full vs partial deployment)
        let parachute_multiplier = vessel.map(|v| v.parachute_drag_multiplier).unwrap_or(1.0);
        let parachute_area = vessel.map(|v| v.parachute_drag_width).unwrap_or(0.0) * parachute_multiplier;
        let cross_section = body_cross_section + parachute_area;

        // F_drag = 0.5 * rho * v^2 * Cd * A
        let total_mass_kg = vessel
            .map(|v| v.total_mass * 1000.0) // tonnes -> kg
            .unwrap_or(1000.0);
        let drag_force = 0.5 * density * airspeed * airspeed * DRAG_COEFFICIENT * cross_section;
        let drag_accel_mag = drag_force / total_mass_kg;

        // Apply opposite to airspeed direction
        [
            -airspeed_x / airspeed * drag_accel_mag,
            -airspeed_y / airspeed * drag_accel_mag,
        ]
    }

    /// Update while landed on a surface
    fn update_landed(
        &mut self,
        dt: f64,
        _input: &ShipInput,
        solar_system: &SolarSystem,
        body_index: usize,
        surface_angle: f64,
        vessel: Option<&VesselPhysicsData>,
    ) {
        use crate::game::{LAUNCHPAD_SURFACE_ANGLE, LAUNCHPAD_HEIGHT, LAUNCHPAD_BOTTOM_WIDTH};

        let body = &solar_system.bodies[body_index];
        let body_radius = body.radius;
        let surface_gravity = G * body.mass / (body_radius * body_radius);

        // Landed = on surface, use full surface pressure for thrust interpolation
        let atmo_pressure = if body.atmosphere.is_some() { 1.0 } else { 0.0 };
        let max_thrust_accel = vessel
            .map(|v| if v.total_mass > 0.0 {
                let thrust = v.max_thrust_vac * (1.0 - atmo_pressure) + v.max_thrust_asl * atmo_pressure;
                thrust / v.total_mass
            } else { 0.0 })
            .unwrap_or(MAX_THRUST_ACCELERATION);
        let thrust_accel = self.throttle * max_thrust_accel;

        let bottom = vessel
            .map(|v| v.bottom_extent)
            .unwrap_or(SHIP_SIZE / 2.0);

        // Account for launchpad height
        let launchpad_offset = if body_index == solar_system.earth_index {
            let angle_diff = surface_angle - LAUNCHPAD_SURFACE_ANGLE;
            let angle_diff = angle_diff - (angle_diff / std::f64::consts::TAU).round() * std::f64::consts::TAU;
            let half_angle = (LAUNCHPAD_BOTTOM_WIDTH * 0.5) / body_radius;
            if angle_diff.abs() < half_angle {
                LAUNCHPAD_HEIGHT
            } else {
                0.0
            }
        } else {
            0.0
        };

        self.rotation = surface_angle;
        self.rotational_velocity = 0.0;
        let surface_distance = body_radius + launchpad_offset + bottom;

        if thrust_accel > surface_gravity {
            let net_accel = thrust_accel - surface_gravity;
            let up_dir = [surface_angle.cos(), surface_angle.sin()];

            self.rel_velocity = [
                up_dir[0] * net_accel * dt,
                up_dir[1] * net_accel * dt,
            ];

            // Move position above the surface so the terrain collision check
            // doesn't immediately catch us and zero throttle
            let displacement = 0.5 * net_accel * dt * dt;
            self.rel_position = [
                (surface_distance + displacement) * up_dir[0],
                (surface_distance + displacement) * up_dir[1],
            ];

            self.state = ShipState::Flying;
            self.cached_orbit = None;
        } else {
            self.rel_position = [
                surface_distance * surface_angle.cos(),
                surface_distance * surface_angle.sin(),
            ];
            self.rel_velocity = [0.0, 0.0];
        }

        {
            let dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
            let grav_factor = gravitational_time_factor(
                G * body.effective_mass_at(dist), dist, body.is_compact(),
            );
            self.proper_time += dt * grav_factor;
            self.mission_time += dt;
        }
    }

    /// Check for collisions with bodies
    fn check_and_handle_collisions(&mut self, solar_system: &SolarSystem, vessel: Option<&VesselPhysicsData>) {
        use crate::game::{LAUNCHPAD_SURFACE_ANGLE,
                          LAUNCHPAD_HEIGHT, LAUNCHPAD_BOTTOM_WIDTH};

        let ship_radius = vessel
            .map(|v| v.bottom_extent)
            .unwrap_or(SHIP_SIZE / 2.0);

        // Compute ship position relative to each body.
        // For the SOI body, use rel_position directly to avoid floating-point
        // precision loss from round-tripping through galactic-scale absolute coords.
        let soi_pos = solar_system.body_position(self.soi_body);

        for (i, body) in solar_system.bodies.iter().enumerate() {
            let (dx, dy) = if i == self.soi_body {
                (self.rel_position[0], self.rel_position[1])
            } else {
                let body_pos = solar_system.body_position(i);
                (
                    soi_pos[0] + self.rel_position[0] - body_pos[0],
                    soi_pos[1] + self.rel_position[1] - body_pos[1],
                )
            };
            let dist = (dx * dx + dy * dy).sqrt();

            // Check launchpad collision (raised surface)
            if i == solar_system.earth_index {
                let lp_collision_dist = body.radius + LAUNCHPAD_HEIGHT + ship_radius;
                if dist < lp_collision_dist {
                    let surface_angle = dy.atan2(dx);
                    let angle_diff = surface_angle - LAUNCHPAD_SURFACE_ANGLE;
                    let angle_diff = angle_diff - (angle_diff / std::f64::consts::TAU).round() * std::f64::consts::TAU;
                    let half_angle = (LAUNCHPAD_BOTTOM_WIDTH * 0.5) / body.radius;
                    if angle_diff.abs() < half_angle {
                        if i != self.soi_body {
                            self.soi_body = i;
                        }
                        let surface_distance = body.radius + LAUNCHPAD_HEIGHT + ship_radius;
                        self.rel_position = [
                            surface_distance * surface_angle.cos(),
                            surface_distance * surface_angle.sin(),
                        ];
                        self.rel_velocity = [0.0, 0.0];
                        self.throttle = 0.0;
                        self.rotation = surface_angle;
                        self.state = ShipState::Landed {
                            body_index: i,
                            surface_angle,
                        };
                        self.on_rails = false;
                        self.cached_orbit = None;
                        return;
                    }
                }
            }

            let collision_dist = body.radius + ship_radius;

            if dist < collision_dist {
                let surface_angle = dy.atan2(dx);

                if i != self.soi_body {
                    self.soi_body = i;
                }

                let surface_distance = body.radius + ship_radius;
                self.rel_position = [
                    surface_distance * surface_angle.cos(),
                    surface_distance * surface_angle.sin(),
                ];

                self.rel_velocity = [0.0, 0.0];
                self.throttle = 0.0;
                self.rotation = surface_angle;

                self.state = ShipState::Landed {
                    body_index: i,
                    surface_angle,
                };
                self.on_rails = false;
                self.cached_orbit = None;

                return;
            }
        }
    }

    /// Get the cached orbit data (for transfer planner etc.)
    pub fn get_cached_orbit(&self) -> Option<&ShipOrbit> {
        self.cached_orbit.as_ref()
    }

    /// Get the current orbit for rendering (uses cached orbit if on rails)
    pub fn get_render_orbit(&self) -> Option<(Orbit, usize)> {
        if matches!(self.state, ShipState::Landed { .. }) {
            return None;
        }
        self.cached_orbit.as_ref().map(|o| (o.orbit, o.parent_idx))
    }

    /// Calculate orbital elements (for when not on rails)
    pub fn calculate_orbit(&mut self, solar_system: &SolarSystem) -> Option<(Orbit, usize)> {
        if self.on_rails {
            return self.get_render_orbit();
        }

        if let Some(ship_orbit) = self.calculate_orbit_with_anomaly(solar_system) {
            let result = (ship_orbit.orbit, ship_orbit.parent_idx);
            self.cached_orbit = Some(ship_orbit);
            return Some(result);
        }

        None
    }

    /// Calculate a predicted trajectory from a given state (for maneuver node predictions)
    /// Returns segments similar to patched conics trajectory.
    /// `epoch` is the absolute simulation time at which this state occurs (for correct body positions).
    pub fn calculate_predicted_trajectory(
        &self,
        pos: [f64; 2],
        vel: [f64; 2],
        parent_idx: usize,
        solar_system: &SolarSystem,
        epoch: f64,
    ) -> Option<PatchedTrajectory> {
        let parent = &solar_system.bodies[parent_idx];
        let r = (pos[0].powi(2) + pos[1].powi(2)).sqrt();
        let (orbit, true_anomaly, retrograde) = self.calculate_orbit_from_state(pos, vel, parent.effective_mass_at(r))?;

        let segments = self.compute_patched_segments(
            orbit, true_anomaly, retrograde, parent_idx,
            epoch, solar_system,
        );

        if segments.is_empty() { None } else { Some(PatchedTrajectory { segments }) }
    }

    /// Calculate target angle for autopilot based on current mode
    /// Returns the target rotation angle, or None if no valid target
    pub fn autopilot_target_angle(
        &self,
        target: AutopilotTarget,
        maneuver_node: Option<&ManeuverNode>,
    ) -> Option<f64> {
        match target {
            AutopilotTarget::Off => None,
            AutopilotTarget::Prograde | AutopilotTarget::Retrograde |
            AutopilotTarget::RadialIn | AutopilotTarget::RadialOut => {
                let vel = self.rel_velocity;
                let vel_mag = (vel[0] * vel[0] + vel[1] * vel[1]).sqrt();
                if vel_mag > 0.1 {
                    let prograde_angle = vel[1].atan2(vel[0]);
                    match target {
                        AutopilotTarget::Prograde => Some(prograde_angle),
                        AutopilotTarget::Retrograde => Some(prograde_angle + std::f64::consts::PI),
                        AutopilotTarget::RadialOut => Some(prograde_angle + std::f64::consts::FRAC_PI_2),
                        AutopilotTarget::RadialIn => Some(prograde_angle - std::f64::consts::FRAC_PI_2),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            AutopilotTarget::ManeuverNode => {
                if let Some(node) = maneuver_node {
                    let prograde = node.prograde_unit();
                    let radial = node.radial_unit();
                    let dv_x = prograde[0] * node.delta_v.prograde + radial[0] * node.delta_v.radial_out;
                    let dv_y = prograde[1] * node.delta_v.prograde + radial[1] * node.delta_v.radial_out;
                    let dv_mag = (dv_x * dv_x + dv_y * dv_y).sqrt();
                    if dv_mag > 0.001 {
                        Some(dv_y.atan2(dv_x))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            // Target angle is computed externally by main.rs
            AutopilotTarget::Target => None,
        }
    }

    /// Compute the autopilot rotation command in [-1, 1].
    /// Used to command engine gimbals and for RCS fuel consumption direction.
    /// Uses combined RCS + gimbal torque for stopping distance, and returns
    /// a proportional value so gimbals smoothly reduce near the target.
    pub fn autopilot_desired_direction(&self, target_angle: f64, vessel: Option<&VesselPhysicsData>) -> f64 {
        let mut angle_diff = target_angle - self.rotation;
        while angle_diff > std::f64::consts::PI {
            angle_diff -= std::f64::consts::TAU;
        }
        while angle_diff < -std::f64::consts::PI {
            angle_diff += std::f64::consts::TAU;
        }

        let vel = self.rotational_velocity;
        let rw_accel = vessel
            .map(|v| if v.moment_of_inertia > 0.0 { v.rcs_torque / v.moment_of_inertia } else { ROTATION_ACCEL })
            .unwrap_or(ROTATION_ACCEL);
        // Use max gimbal torque (full deflection) so autopilot can bootstrap from neutral gimbal.
        // Applied at 0.5x in update_flying, so match that here.
        let gimbal_accel = vessel
            .map(|v| if v.moment_of_inertia > 0.0 { v.max_gimbal_torque * 0.5 / v.moment_of_inertia } else { 0.0 })
            .unwrap_or(0.0);
        let total_accel = rw_accel + gimbal_accel;
        let threshold = 0.002;

        if angle_diff.abs() < threshold && vel.abs() < 0.01 {
            return 0.0;
        }

        if total_accel < 1e-12 {
            return 0.0; // No torque available — cannot rotate
        }

        let stopping_dist = vel.powi(2) / (2.0 * total_accel);
        let going_right_way = (angle_diff > 0.0 && vel >= 0.0) || (angle_diff < 0.0 && vel <= 0.0);
        let should_brake = going_right_way && stopping_dist >= angle_diff.abs() * 0.5;

        if should_brake {
            // Full brake
            if vel > 0.0 { -1.0 } else { 1.0 }
        } else {
            // Accelerate toward target, scaled down when close
            let fraction = (angle_diff.abs() / 0.15).clamp(0.0, 1.0);
            if angle_diff > 0.0 { fraction } else { -fraction }
        }
    }

    /// Update ship rotation toward target angle using autopilot
    /// Uses acceleration-based control with stopping distance braking.
    /// Applies only reaction wheel torque here; gimbal torque is applied
    /// separately in update_flying (always active).
    pub fn autopilot_rotate(&mut self, target_angle: f64, dt: f64, vessel: Option<&VesselPhysicsData>) {
        // Normalize angle difference to [-PI, PI]
        let mut angle_diff = target_angle - self.rotation;
        while angle_diff > std::f64::consts::PI {
            angle_diff -= std::f64::consts::TAU;
        }
        while angle_diff < -std::f64::consts::PI {
            angle_diff += std::f64::consts::TAU;
        }

        let vel = self.rotational_velocity;
        // Use reaction wheel torque for direct control
        let rw_accel = vessel
            .map(|v| if v.moment_of_inertia > 0.0 { v.rcs_torque / v.moment_of_inertia } else { ROTATION_ACCEL })
            .unwrap_or(ROTATION_ACCEL);
        // Use max gimbal torque for braking calculations (matches autopilot_desired_direction)
        let gimbal_accel = vessel
            .map(|v| if v.moment_of_inertia > 0.0 { v.max_gimbal_torque * 0.5 / v.moment_of_inertia } else { 0.0 })
            .unwrap_or(0.0);
        let total_accel = rw_accel + gimbal_accel;
        let threshold = 0.002; // ~0.1 degrees

        if angle_diff.abs() < threshold && vel.abs() < 0.01 {
            // Close enough and nearly stopped - snap to target
            self.rotational_velocity = 0.0;
            self.rotation = target_angle;
        } else if total_accel < 1e-12 {
            // No torque available — cannot rotate, do nothing
        } else {
            // Calculate stopping distance at current velocity: s = v²/(2a)
            let stopping_dist = vel.powi(2) / (2.0 * total_accel);

            // Are we going the right direction?
            let going_right_way = (angle_diff > 0.0 && vel >= 0.0) || (angle_diff < 0.0 && vel <= 0.0);

            // Should we brake? Start braking when stopping distance reaches 50% of remaining
            let should_brake = going_right_way && stopping_dist >= angle_diff.abs() * 0.5;

            if should_brake {
                // Brake: accelerate opposite to velocity (RW only; gimbal assists via update_flying)
                if vel > 0.0 {
                    self.rotational_velocity -= rw_accel * dt;
                } else {
                    self.rotational_velocity += rw_accel * dt;
                }
            } else {
                // Accelerate toward target (RW only; gimbal assists via update_flying)
                if angle_diff > 0.0 {
                    self.rotational_velocity += rw_accel * dt;
                } else {
                    self.rotational_velocity -= rw_accel * dt;
                }
            }

            // Apply rotational velocity
            self.rotation += self.rotational_velocity * dt;
        }
    }
}
