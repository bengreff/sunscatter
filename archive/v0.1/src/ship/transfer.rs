//! Transfer planner: Hohmann, Lambert, and interplanetary transfer calculations

use crate::bodies::{CelestialBody, G, Orbit};
use std::f64::consts::{PI, TAU};

/// Result of a Hohmann transfer calculation
pub struct HohmannResult {
    pub departure_delta_v: f64,       // m/s total magnitude
    pub departure_prograde: f64,      // m/s prograde component
    pub departure_radial: f64,        // m/s radial component (cancel radial velocity)
    pub arrival_delta_v: f64,         // m/s (info only)
    pub transfer_time: f64,           // seconds
    pub required_phase_angle: f64,    // radians
    pub current_phase_angle: f64,     // radians
    pub time_to_window: f64,          // seconds until phase angle matches
    pub departure_position_angle: f64, // inertial angle where burn should happen (radians)
}

/// Result of a Lambert solver
pub struct LambertResult {
    pub v1: [f64; 2], // departure velocity vector
    pub v2: [f64; 2], // arrival velocity vector
}

/// Result of an interplanetary transfer calculation
pub struct InterplanetaryResult {
    pub ejection_delta_v: f64,        // total burn magnitude at parking orbit
    pub ejection_prograde: f64,       // prograde component in ship frame
    pub ejection_radial: f64,         // radial component in ship frame
    pub v_infinity: f64,              // excess velocity at SOI edge
    pub transfer_time: f64,           // seconds
    pub departure_position_angle: f64, // inertial angle where ejection burn should happen (radians)
    pub current_phase_angle: f64,     // radians (between planets)
    pub required_phase_angle: f64,    // radians
    pub time_to_window: f64,          // seconds
    pub arrival_v_infinity: f64,      // m/s at target
}

/// Pre-formatted transfer data for the UI (avoids complex structs in RenderState)
pub struct TransferDisplay {
    pub mode: u8,                      // 0 = Hohmann, 1 = Lambert
    pub target_name: String,
    pub departure_dv: f64,             // m/s
    pub arrival_dv: f64,               // m/s (Hohmann) or arrival v_infinity (Lambert)
    pub transfer_time: f64,            // seconds
    pub current_phase_angle: f64,      // degrees
    pub required_phase_angle: f64,     // degrees
    pub time_to_window: f64,           // seconds
    pub departure_position_angle: f64, // inertial position angle for node placement (radians)
    pub prograde_dv: f64,              // prograde component for the node
    pub radial_dv: f64,                // radial component for the node
    pub valid: bool,
}

/// Get valid Hohmann transfer targets (bodies within the same SOI, i.e. children of ship_soi)
/// Example: ship orbits Earth → targets are Moon (orbits Earth)
pub fn hohmann_targets(ship_soi: usize, bodies: &[CelestialBody]) -> Vec<(usize, String)> {
    bodies.iter().enumerate()
        .filter(|(_, b)| b.parent == Some(ship_soi) && b.orbit.is_some())
        .map(|(i, b)| (i, b.name.clone()))
        .collect()
}

/// Get valid Lambert transfer targets (siblings of ship_soi orbiting the same parent)
/// Example: ship orbits Earth → targets are Mars, Venus, etc. (also orbit Sun)
pub fn lambert_targets(ship_soi: usize, bodies: &[CelestialBody]) -> Vec<(usize, String)> {
    let parent = match bodies[ship_soi].parent {
        Some(p) => p,
        None => return Vec::new(),
    };
    bodies.iter().enumerate()
        .filter(|(i, b)| *i != ship_soi && b.parent == Some(parent) && b.orbit.is_some())
        .map(|(i, b)| (i, b.name.clone()))
        .collect()
}

/// Compute a Hohmann-like transfer to a sibling body using the Lambert solver.
///
/// Uses phase angle timing to find the departure window, then solves the Lambert
/// problem for the exact transfer trajectory. This handles arbitrary eccentricities
/// naturally — the delta-v is computed from the actual velocity difference at the
/// burn point, with proper prograde/radial decomposition.
pub fn compute_hohmann(
    ship_orbit: &Orbit,
    ship_retrograde: bool,
    ship_mean_anomaly: f64,
    target_orbit: &Orbit,
    parent_mass: f64,
    sim_time: f64,
) -> Option<HohmannResult> {
    let mu = G * parent_mass;
    let e = ship_orbit.eccentricity;
    let a = ship_orbit.semi_major_axis;
    let r2 = target_orbit.semi_major_axis;

    if a <= 0.0 || r2 <= 0.0 || mu <= 0.0 {
        return None;
    }

    // Reject near-hyperbolic/hyperbolic orbits
    if e >= 0.95 {
        return None;
    }

    // --- Phase angle timing (approximate, using Hohmann transfer time) ---
    let r1_approx = a; // Use SMA as approximate departure radius for timing
    let a_t_approx = (r1_approx + r2) / 2.0;
    let transfer_time_approx = PI * (a_t_approx.powi(3) / mu).sqrt();

    // Current phase angle: target position - ship position
    let ship_ea = ship_orbit.solve_kepler(ship_mean_anomaly);
    let ship_ta = ship_orbit.true_anomaly(ship_ea);
    let ship_angle = ship_ta + ship_orbit.argument_of_periapsis;

    let target_ma = target_orbit.mean_anomaly_at(sim_time, parent_mass);
    let target_ea = target_orbit.solve_kepler(target_ma);
    let target_ta = target_orbit.true_anomaly(target_ea);
    let target_angle = target_ta + target_orbit.argument_of_periapsis;

    let current_phase = normalize_angle(target_angle - ship_angle);

    let omega_target = target_orbit.mean_motion(parent_mass);
    let omega_ship = ship_orbit.mean_motion(parent_mass);
    let omega_ship_signed = if ship_retrograde { -omega_ship } else { omega_ship };

    let required_phase_approx = if r2 > r1_approx {
        PI - omega_target * transfer_time_approx
    } else {
        -(PI - omega_target * transfer_time_approx)
    };
    let required_phase_approx = normalize_angle(required_phase_approx);

    // Time to window from synodic period
    let synodic_rate_signed = omega_target - omega_ship_signed;
    let time_to_window = if synodic_rate_signed.abs() > 1e-15 {
        let phase_diff = normalize_angle(required_phase_approx - current_phase);
        let t = phase_diff / synodic_rate_signed;
        let synodic_period = TAU / synodic_rate_signed.abs();
        t.rem_euclid(synodic_period)
    } else {
        0.0
    };

    // --- Compute departure state ---
    let departure_ma = ship_mean_anomaly + omega_ship_signed * time_to_window;
    let departure_ea = ship_orbit.solve_kepler(departure_ma);
    let departure_ta = ship_orbit.true_anomaly(departure_ea);
    let departure_position = departure_ta + ship_orbit.argument_of_periapsis;

    // Ship position at departure (relative to parent)
    let r1 = if e < 1.0 {
        a * (1.0 - e * departure_ea.cos())
    } else {
        a.abs() * (e * departure_ea.cosh() - 1.0)
    };
    let ship_pos = [
        r1 * departure_position.cos(),
        r1 * departure_position.sin(),
    ];

    // Ship velocity at departure
    let ship_vel = ship_orbit.velocity_from_mean_anomaly_with_direction(
        departure_ma, parent_mass, ship_retrograde,
    );

    // --- Compute target position at arrival ---
    // Arrival is at departure_time + transfer_time
    // Use actual departure radius for more accurate transfer time
    let a_t = (r1 + r2) / 2.0;
    let transfer_time = PI * (a_t.powi(3) / mu).sqrt();

    let arrival_time = sim_time + time_to_window + transfer_time;
    let target_pos = target_orbit.position_at(arrival_time, parent_mass);

    // --- Solve Lambert for the exact transfer ---
    let lambert = solve_lambert_2d(
        ship_pos, target_pos, transfer_time, mu, !ship_retrograde,
    )?;

    // --- Delta-v decomposition into prograde/radial ---
    // Delta-v vector in inertial frame
    let dv_x = lambert.v1[0] - ship_vel[0];
    let dv_y = lambert.v1[1] - ship_vel[1];
    let total_dv = (dv_x * dv_x + dv_y * dv_y).sqrt();

    // Prograde direction = velocity direction (tangent to orbit)
    let v_mag = (ship_vel[0] * ship_vel[0] + ship_vel[1] * ship_vel[1]).sqrt();
    let (prograde_dv, radial_dv) = if v_mag > 1e-10 {
        let pro_hat = [ship_vel[0] / v_mag, ship_vel[1] / v_mag];
        // Radial outward = perpendicular to prograde, pointing away from parent
        // For prograde (CCW): radial_out = rotate pro_hat by -90° = [pro_hat[1], -pro_hat[0]]
        // For retrograde (CW): radial_out = rotate pro_hat by +90° = [-pro_hat[1], pro_hat[0]]
        let rad_hat = if ship_retrograde {
            [-pro_hat[1], pro_hat[0]]
        } else {
            [pro_hat[1], -pro_hat[0]]
        };
        let pro = dv_x * pro_hat[0] + dv_y * pro_hat[1];
        let rad = dv_x * rad_hat[0] + dv_y * rad_hat[1];
        (pro, rad)
    } else {
        (total_dv, 0.0)
    };

    // Arrival delta-v (Lambert arrival velocity vs target circular velocity)
    let target_vel = target_orbit.velocity_from_mean_anomaly(
        target_orbit.mean_anomaly_at(arrival_time, parent_mass), parent_mass,
    );
    let arr_dv_x = lambert.v2[0] - target_vel[0];
    let arr_dv_y = lambert.v2[1] - target_vel[1];
    let arrival_dv = (arr_dv_x * arr_dv_x + arr_dv_y * arr_dv_y).sqrt();

    // Phase angle at the actual departure time
    let target_ma_at_dep = target_orbit.mean_anomaly_at(sim_time + time_to_window, parent_mass);
    let target_ea_at_dep = target_orbit.solve_kepler(target_ma_at_dep);
    let target_ta_at_dep = target_orbit.true_anomaly(target_ea_at_dep);
    let target_angle_at_dep = target_ta_at_dep + target_orbit.argument_of_periapsis;
    let required_phase = normalize_angle(target_angle_at_dep - departure_position);

    Some(HohmannResult {
        departure_delta_v: total_dv,
        departure_prograde: prograde_dv,
        departure_radial: radial_dv,
        arrival_delta_v: arrival_dv,
        transfer_time,
        required_phase_angle: required_phase,
        current_phase_angle: current_phase,
        time_to_window,
        departure_position_angle: departure_position,
    })
}

// --- Stumpff functions for universal variable Lambert solver ---

fn stumpff_c2(psi: f64) -> f64 {
    if psi.abs() < 1e-6 {
        // Taylor series: 1/2 - psi/24 + psi^2/720 - ...
        1.0 / 2.0 - psi / 24.0 + psi * psi / 720.0
    } else if psi > 0.0 {
        (1.0 - psi.sqrt().cos()) / psi
    } else {
        ((-psi).sqrt().cosh() - 1.0) / (-psi)
    }
}

fn stumpff_c3(psi: f64) -> f64 {
    if psi.abs() < 1e-6 {
        // Taylor series: 1/6 - psi/120 + psi^2/5040 - ...
        1.0 / 6.0 - psi / 120.0 + psi * psi / 5040.0
    } else if psi > 0.0 {
        let sqrt_psi = psi.sqrt();
        (sqrt_psi - sqrt_psi.sin()) / (psi * sqrt_psi)
    } else {
        let sqrt_neg = (-psi).sqrt();
        (sqrt_neg.sinh() - sqrt_neg) / ((-psi) * sqrt_neg)
    }
}

/// Solve the Lambert problem in 2D using universal variable formulation
pub fn solve_lambert_2d(
    r1_vec: [f64; 2],
    r2_vec: [f64; 2],
    tof: f64,
    mu: f64,
    prograde: bool,
) -> Option<LambertResult> {
    let r1_mag = (r1_vec[0].powi(2) + r1_vec[1].powi(2)).sqrt();
    let r2_mag = (r2_vec[0].powi(2) + r2_vec[1].powi(2)).sqrt();

    if r1_mag < 1e-6 || r2_mag < 1e-6 || tof <= 0.0 {
        return None;
    }

    // Cross product (z-component in 2D) to determine transfer angle
    let cross = r1_vec[0] * r2_vec[1] - r1_vec[1] * r2_vec[0];
    let cos_dnu = (r1_vec[0] * r2_vec[0] + r1_vec[1] * r2_vec[1]) / (r1_mag * r2_mag);
    let cos_dnu = cos_dnu.clamp(-1.0, 1.0);

    let sin_dnu = if prograde {
        if cross >= 0.0 { (1.0 - cos_dnu * cos_dnu).sqrt() } else { -(1.0 - cos_dnu * cos_dnu).sqrt() }
    } else if cross >= 0.0 {
        -(1.0 - cos_dnu * cos_dnu).sqrt()
    } else {
        (1.0 - cos_dnu * cos_dnu).sqrt()
    };

    // Check for degenerate 180-degree transfer
    if (1.0 - cos_dnu).abs() < 1e-12 {
        return None;
    }
    let big_a = sin_dnu * (r1_mag * r2_mag / (1.0 - cos_dnu)).sqrt();
    if big_a.abs() < 1e-12 {
        return None;
    }

    // Newton-Raphson with bisection fallback on universal variable psi
    let mut psi_low = -4.0 * PI * PI;
    let mut psi_high = 4.0 * PI * PI;
    let mut psi = 0.0;

    for _ in 0..60 {
        let c2 = stumpff_c2(psi);
        let c3 = stumpff_c3(psi);

        // y = r1 + r2 + A * (psi * c3 - 1) / sqrt(c2)
        if c2.abs() < 1e-20 {
            psi = (psi_low + psi_high) / 2.0;
            continue;
        }
        let y = r1_mag + r2_mag + big_a * (psi * c3 - 1.0) / c2.sqrt();

        if y < 0.0 {
            // Adjust bounds
            psi_low = psi;
            psi = (psi_low + psi_high) / 2.0;
            continue;
        }

        let chi = (y / c2).sqrt();
        let f_psi = chi.powi(3) * c3 + big_a * y.sqrt() - tof * mu.sqrt();

        // Derivative
        let d_psi = if psi.abs() > 1e-6 {
            let term1 = chi.powi(3) * (stumpff_c3(psi) - 3.0 * c3 * stumpff_c2(psi) / (2.0 * c2)) / (2.0 * psi);
            let term2 = 3.0 * c3 * chi / (2.0 * c2);
            term1 + term2 + big_a / (2.0 * y.sqrt()) * (c2.sqrt() - psi * stumpff_c3(psi) / c2.sqrt()) * chi
        } else {
            // Near zero: simplified
            let term = (2.0 / 5.0_f64.sqrt()) * y.powf(1.5) / 3.0 + big_a * y.sqrt() / 2.0;
            if term.abs() < 1e-20 { 1.0 } else { term }
        };

        // Newton step with bounds checking
        if d_psi.abs() < 1e-20 {
            psi = (psi_low + psi_high) / 2.0;
            continue;
        }

        let psi_new = psi - f_psi / d_psi;
        if f_psi < 0.0 {
            psi_low = psi;
        } else {
            psi_high = psi;
        }

        // Use Newton step if in bounds, bisection otherwise
        psi = if psi_new > psi_low && psi_new < psi_high {
            psi_new
        } else {
            (psi_low + psi_high) / 2.0
        };

        if (f_psi / mu.sqrt()).abs() < 1e-10 {
            break;
        }
    }

    let c2 = stumpff_c2(psi);
    let c3 = stumpff_c3(psi);
    let y = r1_mag + r2_mag + big_a * (psi * c3 - 1.0) / c2.sqrt();

    if y < 0.0 {
        return None;
    }

    // Lagrange coefficients
    let f = 1.0 - y / r1_mag;
    let g_dot = 1.0 - y / r2_mag;
    let g = big_a * (y / mu).sqrt();

    if g.abs() < 1e-20 {
        return None;
    }

    let v1 = [
        (r2_vec[0] - f * r1_vec[0]) / g,
        (r2_vec[1] - f * r1_vec[1]) / g,
    ];
    let v2 = [
        (g_dot * r2_vec[0] - r1_vec[0]) / g,
        (g_dot * r2_vec[1] - r1_vec[1]) / g,
    ];

    // Validate result
    let v1_mag = (v1[0].powi(2) + v1[1].powi(2)).sqrt();
    let v2_mag = (v2[0].powi(2) + v2[1].powi(2)).sqrt();
    if !v1_mag.is_finite() || !v2_mag.is_finite() || v1_mag > 1e8 || v2_mag > 1e8 {
        return None;
    }

    Some(LambertResult { v1, v2 })
}

/// Compute time to traverse a hyperbolic escape trajectory from periapsis to SOI boundary.
/// Used to adjust the Lambert departure time so the v_inf direction is correct at SOI exit
/// rather than at the LEO burn point.
fn compute_hyperbolic_escape_time(
    v_inf_mag: f64,
    r_periapsis: f64,
    soi_radius: f64,
    mu: f64,
) -> f64 {
    if v_inf_mag <= 0.0 || r_periapsis <= 0.0 || soi_radius <= r_periapsis {
        return 0.0;
    }

    // Hyperbolic orbit parameters
    let a_hyp = mu / v_inf_mag.powi(2); // |a| (positive value; real a is negative for hyperbola)
    let e_hyp = 1.0 + r_periapsis / a_hyp; // = 1 + r_p * v_inf^2 / mu

    // Semi-latus rectum: p = |a| * (e^2 - 1)
    let p = a_hyp * (e_hyp.powi(2) - 1.0);

    // True anomaly at SOI radius: r = p / (1 + e*cos(ta))
    let cos_ta = ((p / soi_radius) - 1.0) / e_hyp;
    if cos_ta.abs() > 1.0 {
        return 0.0; // SOI beyond hyperbola's reach
    }
    let ta_exit = cos_ta.acos();

    // Convert true anomaly to hyperbolic anomaly H
    // tanh(H/2) = sqrt((e-1)/(e+1)) * tan(ta/2)
    let tan_half_ta = (ta_exit / 2.0).tan();
    let factor = ((e_hyp - 1.0) / (e_hyp + 1.0)).sqrt();
    let tanh_half_h = factor * tan_half_ta;
    if tanh_half_h.abs() >= 1.0 {
        return 0.0;
    }
    let h_exit = 2.0 * tanh_half_h.atanh();

    // Hyperbolic mean anomaly: M = e * sinh(H) - H
    let m_hyp = e_hyp * h_exit.sinh() - h_exit;

    // Mean motion: n = sqrt(mu / |a|^3)
    let n = (mu / a_hyp.powi(3)).sqrt();

    if n > 0.0 { m_hyp / n } else { 0.0 }
}

/// Compute an interplanetary transfer using Lambert solver
pub fn compute_interplanetary(
    ship_orbit: &Orbit,
    ship_retrograde: bool,
    _ship_mean_anomaly: f64,
    ship_soi_body: usize,        // e.g. Earth (index 4)
    target_body: usize,          // e.g. Mars (index 6)
    departure_time: f64,         // absolute sim time of departure
    arrival_time: f64,           // absolute sim time of arrival
    sim_time: f64,               // current simulation time
    bodies: &[CelestialBody],
) -> Option<InterplanetaryResult> {
    let parent = bodies[ship_soi_body].parent?; // e.g. Sun
    let grandparent_mass = bodies[parent].mass;
    let mu_sun = G * grandparent_mass;

    let departure_orbit = bodies[ship_soi_body].orbit.as_ref()?;
    let target_orbit = bodies[target_body].orbit.as_ref()?;

    let mu_planet = G * bodies[ship_soi_body].mass;
    let a_parking = ship_orbit.semi_major_axis;
    let e_parking = ship_orbit.eccentricity;
    if a_parking <= 0.0 {
        return None;
    }
    // Reject near-hyperbolic/hyperbolic parking orbits
    if e_parking >= 0.95 {
        return None;
    }

    // --- Step 1: Initial Lambert solve to get approximate v_inf ---
    let r1_vec = departure_orbit.position_at(departure_time, grandparent_mass);
    let r2_vec = target_orbit.position_at(arrival_time, grandparent_mass);
    let tof = arrival_time - departure_time;
    if tof <= 0.0 {
        return None;
    }

    let lambert_initial = solve_lambert_2d(r1_vec, r2_vec, tof, mu_sun, true)?;
    let dep_ma_initial = departure_orbit.mean_anomaly_at(departure_time, grandparent_mass);
    let planet_vel_initial = departure_orbit.velocity_from_mean_anomaly(dep_ma_initial, grandparent_mass);
    let v_inf_initial_mag = {
        let vx = lambert_initial.v1[0] - planet_vel_initial[0];
        let vy = lambert_initial.v1[1] - planet_vel_initial[1];
        (vx.powi(2) + vy.powi(2)).sqrt()
    };

    // --- Step 2: Compute escape time (parking orbit burn to SOI exit on hyperbolic trajectory) ---
    // The ship takes hours/days to escape from parking orbit to the SOI boundary.
    // During this time the departure planet moves in its orbit, changing the required
    // v_inf direction. We correct for this by solving Lambert for the SOI exit time.
    // Use periapsis as approximate escape radius (conservative; actual radius computed later).
    let r_periapsis = a_parking * (1.0 - e_parking);
    let soi_radius = bodies[ship_soi_body].soi_radius;
    let escape_time = compute_hyperbolic_escape_time(
        v_inf_initial_mag, r_periapsis, soi_radius, mu_planet,
    );

    // --- Step 3: Re-solve Lambert for effective departure (SOI exit time) ---
    let effective_departure = departure_time + escape_time;
    let r1_corrected = departure_orbit.position_at(effective_departure, grandparent_mass);
    let tof_corrected = arrival_time - effective_departure;
    if tof_corrected <= 0.0 {
        return None;
    }

    let lambert = solve_lambert_2d(r1_corrected, r2_vec, tof_corrected, mu_sun, true)?;

    // Corrected v_inf: what the ship needs at SOI exit
    let dep_ma = departure_orbit.mean_anomaly_at(effective_departure, grandparent_mass);
    let planet_vel = departure_orbit.velocity_from_mean_anomaly(dep_ma, grandparent_mass);
    let v_inf = [
        lambert.v1[0] - planet_vel[0],
        lambert.v1[1] - planet_vel[1],
    ];
    let v_inf_mag = (v_inf[0].powi(2) + v_inf[1].powi(2)).sqrt();

    // Arrival v_inf (arrival time unchanged, but Lambert solution differs slightly)
    let arr_ma = target_orbit.mean_anomaly_at(arrival_time, grandparent_mass);
    let target_vel = target_orbit.velocity_from_mean_anomaly(arr_ma, grandparent_mass);
    let arr_v_inf = [
        lambert.v2[0] - target_vel[0],
        lambert.v2[1] - target_vel[1],
    ];
    let arrival_v_infinity = (arr_v_inf[0].powi(2) + arr_v_inf[1].powi(2)).sqrt();

    // Ejection angle calculation: find the position on the parking orbit where a
    // burn produces an escape trajectory with the desired v_infinity direction.
    //
    // For the turn angle geometry, use periapsis radius (the hyperbolic escape
    // trajectory's periapsis is approximately at the burn point's radius).
    let r_burn_approx = r_periapsis; // conservative estimate for geometry

    // Hyperbolic eccentricity of the ejection orbit (using periapsis as burn point)
    let e_hyp = 1.0 + r_burn_approx * v_inf_mag.powi(2) / mu_planet;

    // True anomaly at SOI exit on the escape hyperbola
    let a_hyp = mu_planet / v_inf_mag.powi(2);
    let p_hyp = a_hyp * (e_hyp.powi(2) - 1.0);
    let cos_ta_exit = ((p_hyp / soi_radius) - 1.0) / e_hyp;
    let ta_exit = cos_ta_exit.clamp(-1.0, 1.0).acos();

    // Turn angle from periapsis to the velocity direction at SOI exit
    let turn_angle = (e_hyp + ta_exit.cos()).atan2(-ta_exit.sin());

    // V-infinity direction in planet-centered frame (velocity at SOI exit from Lambert)
    let v_inf_angle = v_inf[1].atan2(v_inf[0]);

    // Ejection position angle (where on the orbit to burn)
    let ejection_position_angle = if ship_retrograde {
        v_inf_angle + turn_angle
    } else {
        v_inf_angle - turn_angle
    };

    // --- Actual radius and velocity at ejection point on parking orbit ---
    // Convert ejection inertial angle to true anomaly on the parking orbit
    let ejection_ta = ejection_position_angle - ship_orbit.argument_of_periapsis;
    let r_at_ejection = a_parking * (1.0 - e_parking * e_parking)
        / (1.0 + e_parking * ejection_ta.cos());

    // Ship's actual speed at ejection point (vis-viva)
    let v_at_ejection = (mu_planet * (2.0 / r_at_ejection - 1.0 / a_parking)).sqrt();

    // Escape speed needed (energy conservation — exact regardless of orbit shape)
    let v_escape = (v_inf_mag.powi(2) + 2.0 * mu_planet / r_at_ejection).sqrt();
    let ejection_dv = (v_escape - v_at_ejection).abs();

    // Decompose into prograde/radial: compute tangential and radial velocity
    // components at ejection point, then delta-v is approximately prograde
    // (escape burn is mostly tangential) with a radial correction
    let h_parking = (mu_planet * a_parking * (1.0 - e_parking * e_parking)).sqrt();
    let v_tangential_at_ejection = h_parking / r_at_ejection;
    let v_radial_at_ejection = {
        let vr_sq = v_at_ejection * v_at_ejection - v_tangential_at_ejection * v_tangential_at_ejection;
        if vr_sq > 0.0 {
            let sign = ejection_ta.sin();
            vr_sq.sqrt() * sign.signum()
        } else {
            0.0
        }
    };

    // Escape velocity is approximately tangential at the burn point
    let ejection_prograde = v_escape - v_tangential_at_ejection;
    let ejection_radial = -v_radial_at_ejection;

    // Phase angles between planets at current time (not departure time)
    let dep_ma_now = departure_orbit.mean_anomaly_at(sim_time, grandparent_mass);
    let dep_ea_now = departure_orbit.solve_kepler(dep_ma_now);
    let dep_ta_now = departure_orbit.true_anomaly(dep_ea_now);
    let dep_angle_now = dep_ta_now + departure_orbit.argument_of_periapsis;

    let target_ma_now = target_orbit.mean_anomaly_at(sim_time, grandparent_mass);
    let target_ea_now = target_orbit.solve_kepler(target_ma_now);
    let target_ta_now = target_orbit.true_anomaly(target_ea_now);
    let target_angle_now = target_ta_now + target_orbit.argument_of_periapsis;

    let current_phase = normalize_angle(target_angle_now - dep_angle_now);

    // Required phase angle (from Hohmann approximation for reference)
    let a_t_ip = (departure_orbit.semi_major_axis + target_orbit.semi_major_axis) / 2.0;
    let hohmann_tof = PI * (a_t_ip.powi(3) / mu_sun).sqrt();
    let omega_target = target_orbit.mean_motion(grandparent_mass);
    let required_phase = normalize_angle(PI - omega_target * hohmann_tof);

    // Time to departure — if departure_time is in the past, advance by
    // synodic period(s) to the next window with the same geometry.
    let raw_dt = departure_time - sim_time;
    let time_to_window = if raw_dt >= 0.0 {
        raw_dt
    } else {
        let omega_dep = departure_orbit.mean_motion(grandparent_mass);
        let omega_tgt = target_orbit.mean_motion(grandparent_mass);
        let synodic_rate = (omega_dep - omega_tgt).abs();
        if synodic_rate > 1e-15 {
            let synodic_period = TAU / synodic_rate;
            let periods_behind = (-raw_dt / synodic_period).ceil();
            raw_dt + periods_behind * synodic_period
        } else {
            0.0
        }
    };

    Some(InterplanetaryResult {
        ejection_delta_v: ejection_dv,
        ejection_prograde,
        ejection_radial,
        v_infinity: v_inf_mag,
        transfer_time: tof,
        departure_position_angle: ejection_position_angle,
        current_phase_angle: current_phase,
        required_phase_angle: required_phase,
        time_to_window,
        arrival_v_infinity,
    })
}

/// Calculate Hohmann-optimal departure and arrival times for Lambert mode defaults
pub fn hohmann_optimal_times(
    ship_soi_body: usize,
    target_body: usize,
    sim_time: f64,
    bodies: &[CelestialBody],
) -> Option<(f64, f64)> {
    let parent = bodies[ship_soi_body].parent?;
    let grandparent_mass = bodies[parent].mass;
    let mu_sun = G * grandparent_mass;

    let dep_orbit = bodies[ship_soi_body].orbit.as_ref()?;
    let target_orbit = bodies[target_body].orbit.as_ref()?;

    // Hohmann transfer time
    let a_t = (dep_orbit.semi_major_axis + target_orbit.semi_major_axis) / 2.0;
    let transfer_time = PI * (a_t.powi(3) / mu_sun).sqrt();

    // Find when phase angle matches
    let omega_dep = dep_orbit.mean_motion(grandparent_mass);
    let omega_target = target_orbit.mean_motion(grandparent_mass);
    let required_phase = normalize_angle(PI - omega_target * transfer_time);

    let dep_ma = dep_orbit.mean_anomaly_at(sim_time, grandparent_mass);
    let dep_ea = dep_orbit.solve_kepler(dep_ma);
    let dep_ta = dep_orbit.true_anomaly(dep_ea);
    let dep_angle = dep_ta + dep_orbit.argument_of_periapsis;

    let target_ma = target_orbit.mean_anomaly_at(sim_time, grandparent_mass);
    let target_ea = target_orbit.solve_kepler(target_ma);
    let target_ta = target_orbit.true_anomaly(target_ea);
    let target_angle = target_ta + target_orbit.argument_of_periapsis;

    let current_phase = normalize_angle(target_angle - dep_angle);

    // Signed rate: phase = (target_angle - dep_angle) changes at (omega_target - omega_dep)
    let synodic_rate_signed = omega_target - omega_dep;
    let time_to_window = if synodic_rate_signed.abs() > 1e-15 {
        let phase_diff = normalize_angle(required_phase - current_phase);
        let t = phase_diff / synodic_rate_signed;
        let synodic_period = TAU / synodic_rate_signed.abs();
        t.rem_euclid(synodic_period)
    } else {
        0.0
    };

    let departure_time = sim_time + time_to_window;
    let arrival_time = departure_time + transfer_time;

    Some((departure_time, arrival_time))
}

/// Incremental porkchop computation. Holds precomputed per-column state and
/// processes Lambert solves a chunk of rows at a time so the main thread (the
/// only thread on wasm) stays responsive. On both targets we now drive this
/// from the frame loop instead of spawning a worker.
pub struct PorkchopJob {
    target_idx: usize,
    target_orbit: Orbit,
    grandparent_mass: f64,
    planet_mass: f64,

    cols: usize,
    rows: usize,
    dep_start: f64,
    dep_end: f64,
    tof_min: f64,
    tof_max: f64,
    log_ratio: f64,

    dep_positions: Vec<[f64; 2]>,
    dep_velocities: Vec<[f64; 2]>,
    dep_times: Vec<f64>,
    ship_radii: Vec<f64>,
    ship_speeds: Vec<f64>,

    next_row: usize,
    points: Vec<Option<crate::render::PorkchopPoint>>,
    min_dv: f64,
    max_dv: f64,
    best_idx: Option<usize>,
}

impl PorkchopJob {
    /// Set up precomputes; returns None if inputs are degenerate.
    pub fn new(
        departure_orbit: Orbit,
        target_orbit: Orbit,
        grandparent_mass: f64,
        planet_mass: f64,
        sim_time: f64,
        ship_orbit: Orbit,
        ship_mean_anomaly: f64,
        target_idx: usize,
    ) -> Option<Self> {
        let mu_sun = G * grandparent_mass;
        let mu_planet = G * planet_mass;

        // Departure axis: one full synodic period (cycle of transfer windows).
        let omega_dep = departure_orbit.mean_motion(grandparent_mass);
        let omega_tgt = target_orbit.mean_motion(grandparent_mass);
        let synodic_rate = (omega_dep - omega_tgt).abs();
        let synodic_period = if synodic_rate > 1e-15 {
            TAU / synodic_rate
        } else {
            TAU / omega_dep
        };
        let dep_start = sim_time;
        let dep_end = sim_time + synodic_period;

        let a_t = (departure_orbit.semi_major_axis + target_orbit.semi_major_axis) / 2.0;
        let hohmann_tof = PI * (a_t.powi(3) / mu_sun).sqrt();

        let tof_min = hohmann_tof / 100.0;
        let tof_max = hohmann_tof * 2.0;

        let cols: usize = 60;
        let rows: usize = 50;

        let mut dep_positions: Vec<[f64; 2]> = Vec::with_capacity(cols);
        let mut dep_velocities: Vec<[f64; 2]> = Vec::with_capacity(cols);
        let mut dep_times: Vec<f64> = Vec::with_capacity(cols);
        let mut ship_radii: Vec<f64> = Vec::with_capacity(cols);
        let mut ship_speeds: Vec<f64> = Vec::with_capacity(cols);

        let a_ship = ship_orbit.semi_major_axis;
        let e_ship = ship_orbit.eccentricity;
        let omega_ship = ship_orbit.mean_motion(planet_mass);

        for col in 0..cols {
            let dep_time = dep_start + (col as f64 / cols as f64) * (dep_end - dep_start);
            dep_times.push(dep_time);
            dep_positions.push(departure_orbit.position_at(dep_time, grandparent_mass));
            let dep_ma = departure_orbit.mean_anomaly_at(dep_time, grandparent_mass);
            dep_velocities.push(departure_orbit.velocity_from_mean_anomaly(dep_ma, grandparent_mass));

            let dt = dep_time - sim_time;
            let ma_at_dep = ship_mean_anomaly + omega_ship * dt;
            let ea_at_dep = ship_orbit.solve_kepler(ma_at_dep);
            let r_at_dep = a_ship * (1.0 - e_ship * ea_at_dep.cos());
            let v_at_dep = (mu_planet * (2.0 / r_at_dep - 1.0 / a_ship)).sqrt();
            ship_radii.push(r_at_dep);
            ship_speeds.push(v_at_dep);
        }

        Some(Self {
            target_idx,
            target_orbit,
            grandparent_mass,
            planet_mass,
            cols,
            rows,
            dep_start,
            dep_end,
            tof_min,
            tof_max,
            log_ratio: (tof_max / tof_min).ln(),
            dep_positions,
            dep_velocities,
            dep_times,
            ship_radii,
            ship_speeds,
            next_row: 0,
            points: Vec::with_capacity(cols * rows),
            min_dv: f64::MAX,
            max_dv: 0.0_f64,
            best_idx: None,
        })
    }

    pub fn target_idx(&self) -> usize { self.target_idx }
    pub fn done(&self) -> bool { self.next_row >= self.rows }

    /// Compute up to `rows_to_run` more rows of Lambert solves.
    pub fn run_chunk(&mut self, rows_to_run: usize) {
        let mu_sun = G * self.grandparent_mass;
        let mu_planet = G * self.planet_mass;
        let end_row = (self.next_row + rows_to_run).min(self.rows);

        for row in self.next_row..end_row {
            let t = row as f64 / self.rows as f64;
            let tof = self.tof_min * (t * self.log_ratio).exp();

            for col in 0..self.cols {
                let idx = row * self.cols + col;
                let r1 = self.dep_positions[col];
                let r2 = self.target_orbit.position_at(self.dep_times[col] + tof, self.grandparent_mass);

                let point = solve_lambert_2d(r1, r2, tof, mu_sun, true).and_then(|lambert| {
                    let planet_vel = self.dep_velocities[col];
                    let v_inf_x = lambert.v1[0] - planet_vel[0];
                    let v_inf_y = lambert.v1[1] - planet_vel[1];
                    let v_inf_sq = v_inf_x * v_inf_x + v_inf_y * v_inf_y;

                    let r_col = self.ship_radii[col];
                    let v_col = self.ship_speeds[col];
                    let v_ejection = (v_inf_sq + 2.0 * mu_planet / r_col).sqrt();
                    let ejection_dv = v_ejection - v_col;

                    if !ejection_dv.is_finite() || ejection_dv < 0.0 {
                        return None;
                    }

                    Some(crate::render::PorkchopPoint {
                        ejection_dv,
                        dep_time: self.dep_times[col],
                        tof,
                    })
                });

                if let Some(ref p) = point {
                    if p.ejection_dv < self.min_dv {
                        self.min_dv = p.ejection_dv;
                        self.best_idx = Some(idx);
                    }
                    if p.ejection_dv > self.max_dv {
                        self.max_dv = p.ejection_dv;
                    }
                }
                self.points.push(point);
            }
        }

        self.next_row = end_row;
    }

    /// Consume the job and return the completed grid.
    pub fn take_grid(self) -> crate::render::PorkchopGrid {
        crate::render::PorkchopGrid {
            points: self.points,
            cols: self.cols,
            rows: self.rows,
            dep_start: self.dep_start,
            dep_end: self.dep_end,
            tof_min: self.tof_min,
            tof_max: self.tof_max,
            min_dv: self.min_dv,
            max_dv: self.max_dv,
            best_idx: self.best_idx,
            target_idx: self.target_idx,
        }
    }
}

/// Normalize angle to [-PI, PI]
pub fn normalize_angle(mut angle: f64) -> f64 {
    while angle > PI {
        angle -= TAU;
    }
    while angle < -PI {
        angle += TAU;
    }
    angle
}
