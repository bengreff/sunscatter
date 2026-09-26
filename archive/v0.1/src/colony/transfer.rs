//! Transfer delta-v computation for trade routes.
//!
//! Wraps the existing Lambert solver and Hohmann transfer code from `ship::transfer`
//! with colony-specific logic: atmosphere classification, gravity losses, surface
//! launch/landing delta-v, and cargo capacity computation.

use serde::{Deserialize, Serialize};

use crate::bodies::{CelestialBody, SolarSystem, G};
use crate::parts::{PartDefinitions, VesselBlueprint, FlightVessel};
use crate::ship::transfer::{compute_interplanetary, hohmann_optimal_times};
use std::f64::consts::{PI, TAU};

/// Route category for scheduling and transfer computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteCategory {
    /// Both bodies share the same SOI parent (e.g. Earth-Moon, two moons).
    SameSOI,
    /// Different planets around the Sun.
    Interplanetary,
    /// Different star systems (future-proofed).
    Interstellar,
}

impl Default for RouteCategory {
    fn default() -> Self {
        Self::SameSOI
    }
}

/// Atmosphere classification for landing/launch delta-v.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AtmosphereClass {
    /// No significant atmosphere (<1000 Pa). Full propulsive landing required.
    Airless,
    /// Thin atmosphere (1000-10000 Pa, e.g. Mars). Partial aerobraking.
    Thin,
    /// Thick atmosphere (>10000 Pa, e.g. Earth, Venus, Titan). Free aerobraking.
    Thick,
}

/// Classify a body's atmosphere for landing delta-v purposes.
pub fn classify_atmosphere(body: &CelestialBody) -> AtmosphereClass {
    match body.atmosphere {
        None => AtmosphereClass::Airless,
        Some(atm) => {
            if atm.surface_pressure < 1000.0 {
                AtmosphereClass::Airless
            } else if atm.surface_pressure < 10000.0 {
                AtmosphereClass::Thin
            } else {
                AtmosphereClass::Thick
            }
        }
    }
}

/// Gravity loss factor multiplied against launch delta-v.
/// Accounts for the additional delta-v needed due to gravity drag during ascent.
pub fn gravity_loss_factor(class: AtmosphereClass) -> f64 {
    match class {
        AtmosphereClass::Airless => 1.1,
        AtmosphereClass::Thin => 1.15,
        AtmosphereClass::Thick => 1.3,
    }
}

/// Fraction of orbital velocity needed for propulsive landing.
/// 0.0 = free aerobraking (thick atmosphere with parachutes), 1.0 = full propulsive.
pub fn landing_dv_fraction(class: AtmosphereClass) -> f64 {
    match class {
        AtmosphereClass::Airless => 1.0,
        AtmosphereClass::Thin => 0.3,  // Partial aerobraking (Mars-like)
        AtmosphereClass::Thick => 0.0, // Full aerobraking
    }
}

/// Orbital velocity at the surface of a body (m/s).
pub fn surface_orbital_velocity(body: &CelestialBody) -> f64 {
    let mu = G * body.mass;
    if body.radius > 0.0 && mu > 0.0 {
        (mu / body.radius).sqrt()
    } else {
        0.0
    }
}

/// Result of computing a single route leg's delta-v.
#[derive(Debug, Clone)]
pub struct LegResult {
    /// Total delta-v for this leg (m/s).
    pub total_dv: f64,
    /// Delta-v for surface-to-orbit launch (m/s).
    pub launch_dv: f64,
    /// Delta-v for the orbital transfer itself (m/s).
    pub transfer_dv: f64,
    /// Delta-v for orbit-to-surface landing (m/s).
    pub landing_dv: f64,
    /// Departure burn component of transfer_dv (m/s).
    pub departure_burn: f64,
    /// Arrival burn component of transfer_dv (m/s).
    pub arrival_burn: f64,
    /// Flight time (seconds).
    pub flight_time: f64,
}

/// Determine the relationship between two bodies for transfer computation.
fn find_common_parent(
    from_idx: usize,
    to_idx: usize,
    bodies: &[CelestialBody],
) -> TransferType {
    let from_parent = bodies[from_idx].parent;
    let to_parent = bodies[to_idx].parent;

    // Same parent (e.g. Earth-Moon both orbit a common parent, or two moons of same planet)
    if from_parent == to_parent && from_parent.is_some() {
        return TransferType::SameParent(from_parent.unwrap());
    }

    // from orbits to (e.g. Moon -> Earth, where Moon.parent = Earth)
    if from_parent == Some(to_idx) {
        return TransferType::ChildToParent;
    }

    // to orbits from (e.g. Earth -> Moon)
    if to_parent == Some(from_idx) {
        return TransferType::ParentToChild;
    }

    // Cross-parent (interplanetary): both orbit the Sun but are different planets
    // or one is a moon of a planet and the other is a different planet
    let from_sun_parent = sun_parent(from_idx, bodies);
    let to_sun_parent = sun_parent(to_idx, bodies);

    if from_sun_parent.is_some() && to_sun_parent.is_some() {
        // Use the sun-orbiting parent bodies for the interplanetary transfer
        return TransferType::Interplanetary {
            from_planet: from_sun_parent.unwrap(),
            to_planet: to_sun_parent.unwrap(),
        };
    }

    TransferType::Unknown
}

/// Walk up the parent chain to find the body that directly orbits the Sun.
fn sun_parent(body_idx: usize, bodies: &[CelestialBody]) -> Option<usize> {
    let mut idx = body_idx;
    loop {
        match bodies[idx].parent {
            Some(parent) => {
                // If parent is the Sun (no grandparent or grandparent is root)
                if bodies[parent].parent.is_none()
                    || bodies[parent].name == "Sun"
                    || bodies[parent].name == "Sagittarius A*"
                {
                    return Some(idx);
                }
                // Check if parent's parent is the Sun
                if let Some(grandparent) = bodies[parent].parent {
                    if bodies[grandparent].name == "Sun" {
                        return Some(parent);
                    }
                }
                idx = parent;
            }
            None => return None,
        }
    }
}

#[derive(Debug)]
enum TransferType {
    /// Both bodies share the same orbital parent.
    SameParent(usize),
    /// Flying from a child to its parent (e.g. Moon -> Earth).
    ChildToParent,
    /// Flying from a parent to its child (e.g. Earth -> Moon).
    ParentToChild,
    /// Interplanetary: different planets around the Sun.
    Interplanetary {
        from_planet: usize,
        to_planet: usize,
    },
    Unknown,
}

/// Compute the delta-v for a single trade route leg.
///
/// `speed_factor` ranges from 0.0 (Hohmann/minimum energy) to 1.0 (express/fast).
/// Higher speed_factor reduces flight time but increases delta-v.
pub fn compute_leg_delta_v(
    from_body: Option<usize>,
    to_body: Option<usize>,
    sim_time: f64,
    speed_factor: f64,
    solar_system: &SolarSystem,
) -> Option<LegResult> {
    let earth_idx = solar_system.earth_index;
    let bodies = &solar_system.bodies;

    // Resolve None (Earth) to earth_index
    let from_idx = from_body.unwrap_or(earth_idx);
    let to_idx = to_body.unwrap_or(earth_idx);

    if from_idx == to_idx {
        return None; // Same body, no transfer needed
    }

    // Compute surface launch/landing delta-v
    let from_body_data = &bodies[from_idx];
    let to_body_data = &bodies[to_idx];

    let from_atm = classify_atmosphere(from_body_data);
    let to_atm = classify_atmosphere(to_body_data);

    let launch_dv = surface_orbital_velocity(from_body_data) * gravity_loss_factor(from_atm);
    let landing_dv = surface_orbital_velocity(to_body_data) * landing_dv_fraction(to_atm);

    // Compute transfer delta-v based on relationship
    let transfer_type = find_common_parent(from_idx, to_idx, bodies);

    let (transfer_dv, departure_burn, arrival_burn, flight_time) = match transfer_type {
        TransferType::SameParent(parent) => {
            // E.g. Earth-Moon: Hohmann between sibling orbits
            let from_orbit = bodies[from_idx].orbit.as_ref()?;
            let to_orbit = bodies[to_idx].orbit.as_ref()?;
            let parent_mass = bodies[parent].mass;

            let (dep_dv, arr_dv, base_time) = compute_hohmann_simple(from_orbit, to_orbit, parent_mass)?;
            let total_hohmann_dv = dep_dv + arr_dv;
            // Speed factor: reduce time by up to 50%, increases dv by ~4x
            let time_factor = 1.0 - speed_factor * 0.5;
            let adjusted_time = base_time * time_factor;
            // Approximate: dv scales roughly inversely with time for Lambert
            let dv_factor = if time_factor > 0.1 { 1.0 / time_factor } else { 10.0 };
            let scale = dv_factor.min(4.0);
            (total_hohmann_dv * scale, dep_dv * scale, arr_dv * scale, adjusted_time)
        }
        TransferType::ChildToParent | TransferType::ParentToChild => {
            // E.g. Moon -> Earth or Earth -> Moon
            // Create a synthetic parking orbit around the parent and compute Hohmann to child's orbit
            let child_idx = match transfer_type {
                TransferType::ChildToParent => from_idx,
                TransferType::ParentToChild => to_idx,
                _ => unreachable!(),
            };
            let parent_idx = bodies[child_idx].parent?;
            let parent_mass = bodies[parent_idx].mass;
            let child_orbit = bodies[child_idx].orbit.as_ref()?;

            // Synthetic parking orbit at 200 km above parent surface
            let parking_orbit = crate::bodies::Orbit {
                semi_major_axis: bodies[parent_idx].radius + 200_000.0,
                eccentricity: 0.0,
                argument_of_periapsis: 0.0,
                mean_anomaly_at_epoch: 0.0,
            };

            let (dep_dv, arr_dv, base_time) = compute_hohmann_simple(&parking_orbit, child_orbit, parent_mass)?;
            let transfer_dv = dep_dv + arr_dv;

            let time_factor = 1.0 - speed_factor * 0.5;
            (transfer_dv, dep_dv, arr_dv, base_time * time_factor)
        }
        TransferType::Interplanetary {
            from_planet,
            to_planet,
        } => {
            // Interplanetary: use hohmann_optimal_times + compute_interplanetary
            let (dep_time, arr_time) =
                hohmann_optimal_times(from_planet, to_planet, sim_time, bodies)?;

            let base_tof = arr_time - dep_time;
            // Speed factor adjusts TOF
            let time_factor = 1.0 - speed_factor * 0.5;
            let adjusted_tof = base_tof * time_factor;
            let adjusted_arr = dep_time + adjusted_tof;

            // Create a dummy parking orbit for the computation
            let dummy_orbit = crate::bodies::Orbit {
                semi_major_axis: bodies[from_planet].radius + 200_000.0, // 200km parking
                eccentricity: 0.0,
                argument_of_periapsis: 0.0,
                mean_anomaly_at_epoch: 0.0,
            };

            let result = compute_interplanetary(
                &dummy_orbit,
                false,
                0.0,
                from_planet,
                to_planet,
                dep_time,
                adjusted_arr,
                sim_time,
                bodies,
            )?;

            // For interplanetary, ejection_delta_v is primarily a departure burn
            (result.ejection_delta_v, result.ejection_delta_v, 0.0, adjusted_tof)
        }
        TransferType::Unknown => return None,
    };

    let total_dv = launch_dv + transfer_dv + landing_dv;

    Some(LegResult {
        total_dv,
        launch_dv,
        transfer_dv,
        landing_dv,
        departure_burn,
        arrival_burn,
        flight_time,
    })
}

/// Compute delta-v for a trade route leg with mass driver assist at the origin.
///
/// `mass_driver_velocity_ms` is the velocity imparted by the mass driver (m/s).
/// When nonzero:
/// - `launch_dv` is eliminated (the mass driver handles surface-to-orbit)
/// - If the mass driver velocity exceeds escape velocity, the excess hyperbolic
///   velocity (`v_∞ = sqrt(v_launch² - v_esc²)`) reduces the `transfer_dv`.
///
/// Returns a modified LegResult with reduced delta-v requirements.
pub fn compute_leg_delta_v_with_mass_driver(
    from_body: Option<usize>,
    to_body: Option<usize>,
    sim_time: f64,
    speed_factor: f64,
    solar_system: &SolarSystem,
    mass_driver_velocity_ms: f64,
) -> Option<LegResult> {
    let mut leg = compute_leg_delta_v(from_body, to_body, sim_time, speed_factor, solar_system)?;

    if mass_driver_velocity_ms <= 0.0 {
        return Some(leg);
    }

    let earth_idx = solar_system.earth_index;
    let from_idx = from_body.unwrap_or(earth_idx);
    let from_body_data = &solar_system.bodies[from_idx];

    // Escape velocity from the body's surface
    let mu = G * from_body_data.mass;
    let v_esc = if from_body_data.radius > 0.0 && mu > 0.0 {
        (2.0 * mu / from_body_data.radius).sqrt()
    } else {
        0.0
    };

    // Mass driver eliminates launch_dv entirely
    leg.launch_dv = 0.0;

    // If mass driver provides more than escape velocity, excess reduces departure burn
    if mass_driver_velocity_ms > v_esc {
        let v_infinity = (mass_driver_velocity_ms * mass_driver_velocity_ms - v_esc * v_esc).sqrt();
        let reduced_departure = (leg.departure_burn - v_infinity).max(0.0);
        leg.transfer_dv = reduced_departure + leg.arrival_burn;
        leg.departure_burn = reduced_departure;
    }

    leg.total_dv = leg.launch_dv + leg.transfer_dv + leg.landing_dv;
    Some(leg)
}

/// Simple Hohmann transfer delta-v between two circular-ish orbits.
/// Returns (departure_dv, arrival_dv, transfer_time).
fn compute_hohmann_simple(
    from_orbit: &crate::bodies::Orbit,
    to_orbit: &crate::bodies::Orbit,
    parent_mass: f64,
) -> Option<(f64, f64, f64)> {
    let mu = G * parent_mass;
    let r1 = from_orbit.semi_major_axis;
    let r2 = to_orbit.semi_major_axis;

    if r1 <= 0.0 || r2 <= 0.0 || mu <= 0.0 {
        return None;
    }

    let a_t = (r1 + r2) / 2.0;
    let transfer_time = PI * (a_t.powi(3) / mu).sqrt();

    // Departure delta-v (vis-viva at r1)
    let v_circular_r1 = (mu / r1).sqrt();
    let v_transfer_r1 = (mu * (2.0 / r1 - 1.0 / a_t)).sqrt();
    let departure_dv = (v_transfer_r1 - v_circular_r1).abs();

    // Arrival delta-v (vis-viva at r2)
    let v_circular_r2 = (mu / r2).sqrt();
    let v_transfer_r2 = (mu * (2.0 / r2 - 1.0 / a_t)).sqrt();
    let arrival_dv = (v_circular_r2 - v_transfer_r2).abs();

    Some((departure_dv, arrival_dv, transfer_time))
}

/// Compute total delta-v of a blueprint (sum of all stage delta-v's).
pub fn blueprint_total_delta_v(
    blueprint: &VesselBlueprint,
    part_defs: &PartDefinitions,
) -> f64 {
    let vessel = FlightVessel::from_blueprint(
        blueprint,
        part_defs,
        [0.0, 0.0],
        [0.0, 0.0],
        0, // dummy SOI body
    );

    match vessel {
        Ok(v) => {
            let stage_dvs = v.calculate_stage_delta_v(part_defs);
            stage_dvs.iter().map(|(dv, _)| dv).sum()
        }
        Err(_) => 0.0,
    }
}

/// Compute maximum cargo capacity (kg) for a blueprint on a route with given delta-v.
///
/// Uses binary search: adds payload mass and checks if the vessel still has enough dv.
/// For multi-leg routes, use the minimum capacity across all legs.
pub fn compute_cargo_capacity(
    blueprint: &VesselBlueprint,
    part_defs: &PartDefinitions,
    required_dv: f64,
) -> f64 {
    if required_dv <= 0.0 {
        return 0.0;
    }

    // First check if the ship can even make the trip empty
    let base_dv = blueprint_total_delta_v(blueprint, part_defs);
    if base_dv < required_dv {
        return 0.0; // Can't make the trip at all
    }

    // Binary search for max payload
    // The payload is added as extra mass to the vessel
    let mut low = 0.0_f64;
    let mut high = 1_000_000.0; // 1000 tonnes max payload

    for _ in 0..25 {
        let mid = (low + high) / 2.0;
        let dv_with_payload = blueprint_dv_with_cargo(blueprint, part_defs, mid);

        if dv_with_payload >= required_dv {
            low = mid;
        } else {
            high = mid;
        }
    }

    // Return conservative lower bound
    low.max(0.0)
}

/// Compute delta-v of a blueprint with extra payload mass (cargo) added (kg).
pub fn blueprint_dv_with_cargo(
    blueprint: &VesselBlueprint,
    part_defs: &PartDefinitions,
    extra_mass_kg: f64,
) -> f64 {
    let vessel = FlightVessel::from_blueprint(
        blueprint,
        part_defs,
        [0.0, 0.0],
        [0.0, 0.0],
        0,
    );

    match vessel {
        Ok(mut v) => {
            // Add extra mass as cargo payload — included in wet_mass by calculate_stage_delta_v
            v.extra_dry_mass_tonnes = extra_mass_kg / 1000.0;

            let stage_dvs = v.calculate_stage_delta_v(part_defs);
            stage_dvs.iter().map(|(dv, _)| dv).sum()
        }
        Err(_) => 0.0,
    }
}

/// Sum the cargo container capacity (kg) across all parts in a blueprint.
pub fn blueprint_cargo_container_capacity(
    blueprint: &VesselBlueprint,
    part_defs: &PartDefinitions,
) -> f64 {
    blueprint.parts.iter()
        .filter_map(|p| part_defs.get(&p.definition_id))
        .filter_map(|d| d.cargo.as_ref())
        .map(|c| c.capacity_kg)
        .sum()
}

/// Check if a blueprint has a parachute part.
pub fn blueprint_has_parachute(
    blueprint: &VesselBlueprint,
    part_defs: &PartDefinitions,
) -> bool {
    blueprint.parts.iter().any(|p| {
        part_defs
            .get(&p.definition_id)
            .map_or(false, |d| d.parachute.is_some())
    })
}

/// Check if a blueprint has a command pod and its crew capacity.
pub fn blueprint_is_crewed(
    blueprint: &VesselBlueprint,
    part_defs: &PartDefinitions,
) -> (bool, u32) {
    let mut has_pod = false;
    let mut total_capacity = 0u32;

    for placed in &blueprint.parts {
        if let Some(def) = part_defs.get(&placed.definition_id) {
            if let Some(ref pod) = def.pod {
                if pod.can_control {
                    has_pod = true;
                }
                total_capacity += pod.crew_capacity;
            }
        }
    }

    (has_pod, total_capacity)
}

/// Find the next launch window from one body to another.
/// Returns (departure_time, baseline_delta_v) or None.
pub fn next_launch_window(
    from_body: Option<usize>,
    to_body: Option<usize>,
    sim_time: f64,
    solar_system: &SolarSystem,
) -> Option<(f64, f64)> {
    let earth_idx = solar_system.earth_index;
    let bodies = &solar_system.bodies;

    let from_idx = from_body.unwrap_or(earth_idx);
    let to_idx = to_body.unwrap_or(earth_idx);

    if from_idx == to_idx {
        return None;
    }

    let transfer_type = find_common_parent(from_idx, to_idx, bodies);

    match transfer_type {
        TransferType::SameParent(parent) => {
            // Same parent: use synodic period
            let from_orbit = bodies[from_idx].orbit.as_ref()?;
            let to_orbit = bodies[to_idx].orbit.as_ref()?;
            let parent_mass = bodies[parent].mass;

            let omega_from = from_orbit.mean_motion(parent_mass);
            let omega_to = to_orbit.mean_motion(parent_mass);
            let synodic_rate = (omega_from - omega_to).abs();

            if synodic_rate < 1e-15 {
                return Some((sim_time, 0.0)); // Always aligned
            }

            let synodic_period = TAU / synodic_rate;
            let (dep_dv, arr_dv, _time) = compute_hohmann_simple(from_orbit, to_orbit, parent_mass)?;
            let dv = dep_dv + arr_dv;

            // Next window is within one synodic period
            Some((sim_time + synodic_period * 0.1, dv)) // Approximate
        }
        TransferType::Interplanetary {
            from_planet,
            to_planet,
        } => {
            let (dep_time, _arr_time) =
                hohmann_optimal_times(from_planet, to_planet, sim_time, bodies)?;

            // Compute baseline dv
            let leg = compute_leg_delta_v(from_body, to_body, sim_time, 0.0, solar_system)?;
            Some((dep_time, leg.total_dv))
        }
        TransferType::ChildToParent | TransferType::ParentToChild => {
            // These transfers are always available (no phase angle dependency)
            let leg = compute_leg_delta_v(from_body, to_body, sim_time, 0.0, solar_system)?;
            Some((sim_time, leg.total_dv))
        }
        TransferType::Unknown => None,
    }
}

// ============================================================
// Route classification and scheduling helpers
// ============================================================

/// Classify a route as SameSOI, Interplanetary, or Interstellar.
pub fn classify_route(
    from_body: Option<usize>,
    to_body: Option<usize>,
    solar_system: &SolarSystem,
) -> RouteCategory {
    let earth_idx = solar_system.earth_index;
    let from_idx = from_body.unwrap_or(earth_idx);
    let to_idx = to_body.unwrap_or(earth_idx);

    if from_idx == to_idx {
        return RouteCategory::SameSOI;
    }

    let transfer_type = find_common_parent(from_idx, to_idx, &solar_system.bodies);
    match transfer_type {
        TransferType::SameParent(_)
        | TransferType::ChildToParent
        | TransferType::ParentToChild => RouteCategory::SameSOI,
        TransferType::Interplanetary { .. } => RouteCategory::Interplanetary,
        TransferType::Unknown => RouteCategory::Interstellar,
    }
}

/// Compute the synodic period between two bodies (seconds).
/// For Interplanetary: uses planet-level orbits around the Sun.
/// For SameSOI siblings: uses their orbits around the common parent.
/// Returns None if not applicable.
pub fn compute_synodic_period(
    from_body: Option<usize>,
    to_body: Option<usize>,
    solar_system: &SolarSystem,
) -> Option<f64> {
    let earth_idx = solar_system.earth_index;
    let bodies = &solar_system.bodies;
    let from_idx = from_body.unwrap_or(earth_idx);
    let to_idx = to_body.unwrap_or(earth_idx);

    if from_idx == to_idx {
        return None;
    }

    let transfer_type = find_common_parent(from_idx, to_idx, bodies);

    match transfer_type {
        TransferType::SameParent(parent) => {
            let from_orbit = bodies[from_idx].orbit.as_ref()?;
            let to_orbit = bodies[to_idx].orbit.as_ref()?;
            let parent_mass = bodies[parent].mass;
            let omega_from = from_orbit.mean_motion(parent_mass);
            let omega_to = to_orbit.mean_motion(parent_mass);
            let synodic_rate = (omega_from - omega_to).abs();
            if synodic_rate < 1e-15 {
                return None; // Nearly co-orbital
            }
            Some(TAU / synodic_rate)
        }
        TransferType::Interplanetary { from_planet, to_planet } => {
            let from_orbit = bodies[from_planet].orbit.as_ref()?;
            let to_orbit = bodies[to_planet].orbit.as_ref()?;
            // Find the Sun (parent of the planets)
            let sun_idx = bodies[from_planet].parent?;
            let sun_mass = bodies[sun_idx].mass;
            let omega_from = from_orbit.mean_motion(sun_mass);
            let omega_to = to_orbit.mean_motion(sun_mass);
            let synodic_rate = (omega_from - omega_to).abs();
            if synodic_rate < 1e-15 {
                return None;
            }
            Some(TAU / synodic_rate)
        }
        _ => None,
    }
}

/// Result of an interstellar transfer computation.
#[derive(Debug, Clone)]
pub struct InterstellarResult {
    /// Distance between bodies (meters).
    pub distance: f64,
    /// Cruise velocity (m/s) = dv_budget / 2.
    pub v_cruise: f64,
    /// Coordinate (observer-frame) travel time (seconds).
    pub coordinate_time: f64,
    /// Proper (ship-frame) travel time (seconds).
    pub proper_time: f64,
    /// Lorentz factor at cruise speed.
    pub gamma: f64,
}

/// Compute interstellar transfer parameters.
/// Uses simple accel/decel model: v_cruise = dv_budget / 2.
pub fn compute_interstellar_transfer(
    from_body: Option<usize>,
    to_body: Option<usize>,
    dv_budget: f64,
    solar_system: &SolarSystem,
) -> Option<InterstellarResult> {
    let c: f64 = 299_792_458.0; // Speed of light (m/s)
    let earth_idx = solar_system.earth_index;
    let from_idx = from_body.unwrap_or(earth_idx);
    let to_idx = to_body.unwrap_or(earth_idx);

    if from_idx == to_idx || dv_budget <= 0.0 {
        return None;
    }

    // Straight-line distance between body positions
    let from_pos = solar_system.body_position(from_idx);
    let to_pos = solar_system.body_position(to_idx);
    let dx = to_pos[0] - from_pos[0];
    let dy = to_pos[1] - from_pos[1];
    let distance = (dx * dx + dy * dy).sqrt();

    if distance <= 0.0 {
        return None;
    }

    let v_cruise = dv_budget / 2.0;

    // Relativistic check: v >= 0.01c
    let is_relativistic = v_cruise >= 0.01 * c;
    // Cap cruise speed at 0.999c
    let v_cruise = v_cruise.min(0.999 * c);

    // Simple model: assume instantaneous accel/decel (for high-level estimate)
    let coordinate_time = distance / v_cruise;

    let (gamma, proper_time) = if is_relativistic {
        let beta = v_cruise / c;
        let g = 1.0 / (1.0 - beta * beta).sqrt();
        (g, coordinate_time / g)
    } else {
        (1.0, coordinate_time)
    };

    Some(InterstellarResult {
        distance,
        v_cruise,
        coordinate_time,
        proper_time,
        gamma,
    })
}

/// Estimate flight time for a route leg given a delta-v budget.
/// Uses Hohmann baseline and scales inversely with excess dv.
/// For interstellar routes, delegates to compute_interstellar_transfer.
pub fn estimate_flight_time(
    from_body: Option<usize>,
    to_body: Option<usize>,
    dv_budget: f64,
    sim_time: f64,
    solar_system: &SolarSystem,
) -> Option<f64> {
    let category = classify_route(from_body, to_body, solar_system);

    if category == RouteCategory::Interstellar {
        let result = compute_interstellar_transfer(from_body, to_body, dv_budget, solar_system)?;
        return Some(result.coordinate_time);
    }

    // Compute Hohmann baseline (speed_factor=0)
    let baseline = compute_leg_delta_v(from_body, to_body, sim_time, 0.0, solar_system)?;
    let min_dv = baseline.total_dv;
    let base_time = baseline.flight_time;

    if dv_budget <= 0.0 || min_dv <= 0.0 {
        return Some(base_time);
    }

    if dv_budget <= min_dv {
        return Some(base_time);
    }

    // Scale inversely: more dv = shorter time, clamped to 25% of base
    let ratio = min_dv / dv_budget;
    let scaled_time = base_time * ratio;
    Some(scaled_time.max(base_time * 0.25))
}
