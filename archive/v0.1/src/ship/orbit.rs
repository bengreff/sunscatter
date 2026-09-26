use crate::bodies::{SolarSystem, G, Orbit};
use super::{Ship, ShipOrbit, ShipState, OrbitalInfo, HYPERBOLIC_ANGLE_MARGIN};

impl Ship {
    /// Calculate orbit with current mean anomaly
    pub(crate) fn calculate_orbit_with_anomaly(&self, solar_system: &SolarSystem) -> Option<ShipOrbit> {
        if matches!(self.state, ShipState::Landed { .. }) {
            return None;
        }

        let parent_idx = self.soi_body;
        let parent = &solar_system.bodies[parent_idx];

        let rx = self.rel_position[0];
        let ry = self.rel_position[1];
        let r = (rx * rx + ry * ry).sqrt();

        let vx = self.rel_velocity[0];
        let vy = self.rel_velocity[1];
        let v = (vx * vx + vy * vy).sqrt();

        let mu = G * parent.effective_mass_at(r);

        // Specific orbital energy
        let energy = v * v / 2.0 - mu / r;

        // Semi-major axis (negative for hyperbolic orbits)
        let a = -mu / (2.0 * energy);

        // Specific angular momentum (negative = clockwise/retrograde)
        let h = rx * vy - ry * vx;
        let retrograde = h < 0.0;

        // Eccentricity vector
        let ex = (vy * h) / mu - rx / r;
        let ey = -(vx * h) / mu - ry / r;
        let e = (ex * ex + ey * ey).sqrt();

        // Argument of periapsis
        let arg_peri = ey.atan2(ex);

        // True anomaly (angle from periapsis to current position)
        let pos_angle = ry.atan2(rx);
        let true_anomaly = (pos_angle - arg_peri).rem_euclid(std::f64::consts::TAU);

        // Only cache elliptical orbits (hyperbolic orbits can't go on-rails)
        if e >= 1.0 {
            return None;
        }

        // Eccentric anomaly from true anomaly
        let cos_nu = true_anomaly.cos();
        let sin_nu = true_anomaly.sin();
        let eccentric_anomaly = (sin_nu * (1.0 - e * e).sqrt()).atan2(e + cos_nu);

        // Mean anomaly from eccentric anomaly (Kepler's equation)
        let mean_anomaly = (eccentric_anomaly - e * eccentric_anomaly.sin()).rem_euclid(std::f64::consts::TAU);

        let orbit = Orbit {
            semi_major_axis: a,
            eccentricity: e,
            argument_of_periapsis: arg_peri,
            mean_anomaly_at_epoch: 0.0,
        };

        Some(ShipOrbit {
            orbit,
            mean_anomaly,
            parent_idx,
            retrograde,
        })
    }

    /// Get full orbital information for UI display
    pub fn get_orbital_info(&self, solar_system: &SolarSystem) -> Option<OrbitalInfo> {
        let ship_orbit = self.cached_orbit.as_ref()?;
        let parent = &solar_system.bodies[ship_orbit.parent_idx];
        let mu = G * parent.effective_mass_at(ship_orbit.orbit.semi_major_axis);

        let a = ship_orbit.orbit.semi_major_axis;
        let e = ship_orbit.orbit.eccentricity;

        let apoapsis = a * (1.0 + e);
        let periapsis = a * (1.0 - e);

        // Orbital period: T = 2π * sqrt(a³/μ)
        let orbital_period = std::f64::consts::TAU * (a.powi(3) / mu).sqrt();
        let mean_motion = std::f64::consts::TAU / orbital_period;
        let current_m = ship_orbit.mean_anomaly;

        let time_to_anomaly = |target_m: f64| -> f64 {
            let mut delta_m = target_m - current_m;
            if ship_orbit.retrograde {
                delta_m = -delta_m;
            }
            while delta_m < 0.0 {
                delta_m += std::f64::consts::TAU;
            }
            while delta_m >= std::f64::consts::TAU {
                delta_m -= std::f64::consts::TAU;
            }
            delta_m / mean_motion
        };

        let time_to_periapsis = time_to_anomaly(0.0);
        let time_to_apoapsis = time_to_anomaly(std::f64::consts::PI);

        Some(OrbitalInfo {
            orbit: ship_orbit.orbit,
            parent_idx: ship_orbit.parent_idx,
            apoapsis,
            periapsis,
            orbital_period,
            time_to_apoapsis,
            time_to_periapsis,
            retrograde: ship_orbit.retrograde,
        })
    }

    /// Calculate orbital elements from state vectors
    /// Returns (Orbit, true_anomaly, retrograde)
    pub fn calculate_orbit_from_state(&self, pos: [f64; 2], vel: [f64; 2], parent_mass: f64) -> Option<(Orbit, f64, bool)> {
        let rx = pos[0];
        let ry = pos[1];
        let r = (rx * rx + ry * ry).sqrt();

        if r < 1.0 {
            return None;
        }

        let vx = vel[0];
        let vy = vel[1];
        let v = (vx * vx + vy * vy).sqrt();

        let mu = G * parent_mass;

        // Specific orbital energy
        let energy = v * v / 2.0 - mu / r;

        // Semi-major axis (negative for hyperbolic)
        let a = -mu / (2.0 * energy);

        // Angular momentum
        let h = rx * vy - ry * vx;
        let retrograde = h < 0.0;

        // Eccentricity vector
        let ex = (vy * h) / mu - rx / r;
        let ey = -(vx * h) / mu - ry / r;
        let e = (ex * ex + ey * ey).sqrt();

        if !a.is_finite() || !e.is_finite() || e < 0.0 {
            return None;
        }

        // Argument of periapsis
        let arg_peri = ey.atan2(ex);

        // True anomaly (angle from periapsis to current position)
        let pos_angle = ry.atan2(rx);
        let mut true_anomaly = pos_angle - arg_peri;

        // Normalize true anomaly based on orbit type
        if e < 1.0 {
            true_anomaly = true_anomaly.rem_euclid(std::f64::consts::TAU);
        } else {
            // Hyperbolic: keep in valid range (-max_ta, max_ta)
            let max_ta = (-1.0 / e).acos();
            if !max_ta.is_finite() {
                return None;
            }
            while true_anomaly > std::f64::consts::PI {
                true_anomaly -= std::f64::consts::TAU;
            }
            while true_anomaly < -std::f64::consts::PI {
                true_anomaly += std::f64::consts::TAU;
            }

            // For hyperbolic orbits, we need to determine if ship is on incoming or outgoing leg
            // by checking radial velocity (positive = moving away from parent)
            let radial_velocity = (rx * vx + ry * vy) / r;

            // For prograde hyperbolic: incoming leg has ta < 0, outgoing has ta > 0
            // For retrograde hyperbolic: incoming leg has ta > 0, outgoing has ta < 0
            // The geometric ta calculation gives the "unsigned" angle, so we need to fix the sign
            if radial_velocity.abs() < 1e-10 {
                // At periapsis: true anomaly should be near 0
                if true_anomaly.abs() > std::f64::consts::FRAC_PI_2 {
                    true_anomaly = -true_anomaly;
                }
            } else if retrograde {
                // Retrograde: outgoing (receding) should have negative ta
                if radial_velocity > 0.0 && true_anomaly > 0.0 {
                    true_anomaly = -true_anomaly;
                } else if radial_velocity < 0.0 && true_anomaly < 0.0 {
                    true_anomaly = -true_anomaly;
                }
            } else {
                // Prograde: outgoing (receding) should have positive ta
                if radial_velocity > 0.0 && true_anomaly < 0.0 {
                    true_anomaly = -true_anomaly;
                } else if radial_velocity < 0.0 && true_anomaly > 0.0 {
                    true_anomaly = -true_anomaly;
                }
            }

            true_anomaly = true_anomaly.clamp(-max_ta + HYPERBOLIC_ANGLE_MARGIN, max_ta - HYPERBOLIC_ANGLE_MARGIN);
        }

        let orbit = Orbit {
            semi_major_axis: a,
            eccentricity: e,
            argument_of_periapsis: arg_peri,
            mean_anomaly_at_epoch: 0.0,
        };

        Some((orbit, true_anomaly, retrograde))
    }

    /// Convert mean anomaly to true anomaly
    pub fn mean_to_true_anomaly(&self, orbit: &Orbit, mean_anomaly: f64) -> f64 {
        let eccentric_anomaly = orbit.solve_kepler(mean_anomaly);
        orbit.true_anomaly(eccentric_anomaly)
    }

    /// Convert true anomaly to mean anomaly
    pub fn true_to_mean_anomaly(&self, orbit: &Orbit, true_anomaly: f64) -> f64 {
        let e = orbit.eccentricity;

        if e < 1.0 {
            // Elliptical orbit: use eccentric anomaly
            let cos_nu = true_anomaly.cos();
            let sin_nu = true_anomaly.sin();
            let eccentric_anomaly = (sin_nu * (1.0 - e * e).sqrt()).atan2(e + cos_nu);
            (eccentric_anomaly - e * eccentric_anomaly.sin()).rem_euclid(std::f64::consts::TAU)
        } else {
            // Hyperbolic orbit: use hyperbolic anomaly
            let half_nu = true_anomaly / 2.0;
            let tan_half_nu = half_nu.tan();
            let tanh_half_h = tan_half_nu * ((e - 1.0) / (e + 1.0)).sqrt();
            // Clamp to valid range for atanh to avoid NaN/infinity
            let tanh_half_h = tanh_half_h.clamp(-0.99999, 0.99999);
            let h = 2.0 * tanh_half_h.atanh();
            e * h.sinh() - h
        }
    }
}
