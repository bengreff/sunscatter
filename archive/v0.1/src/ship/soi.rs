use crate::bodies::{SolarSystem, G};
use super::{Ship, ShipOrbit, BINARY_SEARCH_ITERATIONS};

impl Ship {
    /// Get velocity of a body in absolute coordinates
    pub(crate) fn get_body_velocity(&self, body_index: usize, solar_system: &SolarSystem) -> [f64; 2] {
        let body = &solar_system.bodies[body_index];

        match (body.parent, &body.orbit) {
            (Some(parent_idx), Some(orbit)) => {
                let parent_mass = solar_system.bodies[parent_idx].effective_mass_at(orbit.semi_major_axis);

                let body_pos = solar_system.body_position(body_index);
                let parent_pos = solar_system.body_position(parent_idx);

                let dx = body_pos[0] - parent_pos[0];
                let dy = body_pos[1] - parent_pos[1];
                let r = (dx * dx + dy * dy).sqrt();

                let mu = G * parent_mass;
                let v_mag = (mu * (2.0 / r - 1.0 / orbit.semi_major_axis)).sqrt();

                let angle = dy.atan2(dx);
                let v_angle = angle + std::f64::consts::FRAC_PI_2;

                let parent_velocity = self.get_body_velocity(parent_idx, solar_system);

                [
                    parent_velocity[0] + v_mag * v_angle.cos(),
                    parent_velocity[1] + v_mag * v_angle.sin(),
                ]
            }
            _ => [0.0, 0.0],
        }
    }

    /// Check for SOI transitions when on rails (with precise timing)
    ///
    /// Uses binary search to find the exact SOI boundary crossing time within
    /// the current timestep, then performs frame conversion at that precise moment.
    /// This eliminates velocity kicks at high time warp caused by using body
    /// positions/velocities at the overshot time instead of the crossing time.
    pub(crate) fn check_soi_transition_on_rails(&mut self, solar_system: &SolarSystem, dt: f64) {
        // Extract orbit data into locals to avoid borrow issues when mutating self
        let (orbit, mean_anomaly, parent_idx, retrograde) = match self.cached_orbit {
            Some(ref so) => (so.orbit, so.mean_anomaly, so.parent_idx, so.retrograde),
            None => return,
        };

        let parent = &solar_system.bodies[parent_idx];
        let parent_mass = parent.effective_mass_at(orbit.semi_major_axis);
        let mean_motion = orbit.mean_motion(parent_mass);
        let direction = if retrograde { -1.0 } else { 1.0 };
        let delta_m = direction * mean_motion * dt;
        let prev_mean_anomaly = mean_anomaly - delta_m;

        let current_body = &solar_system.bodies[self.soi_body];
        let dist_from_current = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();

        // Check if we've exited current SOI
        if dist_from_current > current_body.soi_radius {
            if let Some(new_parent_idx) = current_body.parent {
                // Binary search to find exact crossing fraction
                let mut lo = 0.0_f64;
                let mut hi = 1.0_f64;
                for _ in 0..BINARY_SEARCH_ITERATIONS {
                    let mid = (lo + hi) / 2.0;
                    let test_m = prev_mean_anomaly + mid * delta_m;
                    let test_pos = orbit.position_from_mean_anomaly(test_m, parent_mass);
                    let test_dist = (test_pos[0].powi(2) + test_pos[1].powi(2)).sqrt();
                    if test_dist < current_body.soi_radius {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let crossing_fraction = (lo + hi) / 2.0;
                let crossing_time = solar_system.time - dt * (1.0 - crossing_fraction);
                let crossing_m = prev_mean_anomaly + crossing_fraction * delta_m;

                // Ship state at crossing
                let cross_pos = orbit.position_from_mean_anomaly(crossing_m, parent_mass);
                let cross_vel = orbit.velocity_from_mean_anomaly_with_direction(crossing_m, parent_mass, retrograde);

                // Body state at crossing time
                let body_pos = self.get_body_position_at_time(self.soi_body, crossing_time, solar_system);
                let body_vel = self.get_body_velocity_at_time(self.soi_body, crossing_time, solar_system);

                // Convert to parent frame at crossing time
                self.rel_position = [cross_pos[0] + body_pos[0], cross_pos[1] + body_pos[1]];
                self.rel_velocity = [cross_vel[0] + body_vel[0], cross_vel[1] + body_vel[1]];
                self.soi_body = new_parent_idx;

                // Compute new orbit and propagate remaining time
                let remaining_dt = dt * (1.0 - crossing_fraction);
                if let Some(new_orbit) = self.calculate_orbit_with_anomaly(solar_system) {
                    let new_parent = &solar_system.bodies[new_orbit.parent_idx];
                    let new_parent_mass = new_parent.effective_mass_at(new_orbit.orbit.semi_major_axis);
                    let new_mean_motion = new_orbit.orbit.mean_motion(new_parent_mass);
                    let new_direction = if new_orbit.retrograde { -1.0 } else { 1.0 };

                    let mut advanced_m = new_orbit.mean_anomaly + new_direction * new_mean_motion * remaining_dt;
                    advanced_m = advanced_m.rem_euclid(std::f64::consts::TAU);

                    self.rel_position = new_orbit.orbit.position_from_mean_anomaly(advanced_m, new_parent_mass);
                    self.rel_velocity = new_orbit.orbit.velocity_from_mean_anomaly_with_direction(advanced_m, new_parent_mass, new_orbit.retrograde);

                    self.cached_orbit = Some(ShipOrbit {
                        mean_anomaly: advanced_m,
                        ..new_orbit
                    });
                } else {
                    // Hyperbolic or invalid orbit in new frame — go off rails
                    self.rel_position = [
                        self.rel_position[0] + self.rel_velocity[0] * remaining_dt,
                        self.rel_position[1] + self.rel_velocity[1] * remaining_dt,
                    ];
                    self.on_rails = false;
                }

                println!("SOI: Entered {} SOI", solar_system.bodies[new_parent_idx].name);
                return;
            }
        }

        // Check if we've entered a child body's SOI
        for (child_idx, child) in solar_system.bodies.iter().enumerate() {
            if child.parent != Some(self.soi_body) {
                continue;
            }

            let child_pos_now = self.get_body_position_at_time(child_idx, solar_system.time, solar_system);
            let dx = self.rel_position[0] - child_pos_now[0];
            let dy = self.rel_position[1] - child_pos_now[1];
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < child.soi_radius {
                // Binary search to find exact crossing fraction
                let mut lo = 0.0_f64;
                let mut hi = 1.0_f64;
                for _ in 0..BINARY_SEARCH_ITERATIONS {
                    let mid = (lo + hi) / 2.0;
                    let test_m = prev_mean_anomaly + mid * delta_m;
                    let test_time = solar_system.time - dt + mid * dt;
                    let ship_pos = orbit.position_from_mean_anomaly(test_m, parent_mass);
                    let child_pos = self.get_body_position_at_time(child_idx, test_time, solar_system);
                    let cdx = ship_pos[0] - child_pos[0];
                    let cdy = ship_pos[1] - child_pos[1];
                    let test_dist = (cdx * cdx + cdy * cdy).sqrt();
                    if test_dist >= child.soi_radius {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let crossing_fraction = (lo + hi) / 2.0;
                let crossing_time = solar_system.time - dt + crossing_fraction * dt;
                let crossing_m = prev_mean_anomaly + crossing_fraction * delta_m;

                // Ship state at crossing
                let cross_pos = orbit.position_from_mean_anomaly(crossing_m, parent_mass);
                let cross_vel = orbit.velocity_from_mean_anomaly_with_direction(crossing_m, parent_mass, retrograde);

                // Child body state at crossing time
                let child_pos_cross = self.get_body_position_at_time(child_idx, crossing_time, solar_system);
                let child_vel_cross = self.get_body_velocity_at_time(child_idx, crossing_time, solar_system);

                // Convert to child frame at crossing time
                self.rel_position = [cross_pos[0] - child_pos_cross[0], cross_pos[1] - child_pos_cross[1]];
                self.rel_velocity = [cross_vel[0] - child_vel_cross[0], cross_vel[1] - child_vel_cross[1]];
                self.soi_body = child_idx;

                // Compute new orbit and propagate remaining time
                let remaining_dt = dt * (1.0 - crossing_fraction);
                if let Some(new_orbit) = self.calculate_orbit_with_anomaly(solar_system) {
                    let new_parent = &solar_system.bodies[new_orbit.parent_idx];
                    let new_parent_mass = new_parent.effective_mass_at(new_orbit.orbit.semi_major_axis);
                    let new_mean_motion = new_orbit.orbit.mean_motion(new_parent_mass);
                    let new_direction = if new_orbit.retrograde { -1.0 } else { 1.0 };

                    let mut advanced_m = new_orbit.mean_anomaly + new_direction * new_mean_motion * remaining_dt;
                    advanced_m = advanced_m.rem_euclid(std::f64::consts::TAU);

                    self.rel_position = new_orbit.orbit.position_from_mean_anomaly(advanced_m, new_parent_mass);
                    self.rel_velocity = new_orbit.orbit.velocity_from_mean_anomaly_with_direction(advanced_m, new_parent_mass, new_orbit.retrograde);

                    self.cached_orbit = Some(ShipOrbit {
                        mean_anomaly: advanced_m,
                        ..new_orbit
                    });
                } else {
                    // Hyperbolic or invalid orbit in new frame — go off rails
                    self.rel_position = [
                        self.rel_position[0] + self.rel_velocity[0] * remaining_dt,
                        self.rel_position[1] + self.rel_velocity[1] * remaining_dt,
                    ];
                    self.on_rails = false;
                }

                println!("SOI: Entered {} SOI", solar_system.bodies[child_idx].name);
                return;
            }
        }
    }

    /// Check for SOI transitions with precise interpolation (physics mode)
    pub(crate) fn check_soi_transition_precise(
        &mut self,
        solar_system: &SolarSystem,
        prev_rel_pos: [f64; 2],
        prev_rel_vel: [f64; 2],
        dt: f64,
    ) {
        let current_body = &solar_system.bodies[self.soi_body];
        let prev_dist = (prev_rel_pos[0].powi(2) + prev_rel_pos[1].powi(2)).sqrt();
        let curr_dist = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();

        // Check for leaving current SOI
        if curr_dist > current_body.soi_radius && prev_dist <= current_body.soi_radius {
            if let Some(parent_idx) = current_body.parent {
                let crossing_fraction = self.find_soi_exit_fraction(
                    prev_rel_pos,
                    self.rel_position,
                    current_body.soi_radius,
                );

                let crossing_time = solar_system.time + dt * crossing_fraction;

                let cross_rel_pos = [
                    prev_rel_pos[0] + crossing_fraction * (self.rel_position[0] - prev_rel_pos[0]),
                    prev_rel_pos[1] + crossing_fraction * (self.rel_position[1] - prev_rel_pos[1]),
                ];
                let cross_rel_vel = [
                    prev_rel_vel[0] + crossing_fraction * (self.rel_velocity[0] - prev_rel_vel[0]),
                    prev_rel_vel[1] + crossing_fraction * (self.rel_velocity[1] - prev_rel_vel[1]),
                ];

                let soi_body_pos_at_cross = self.get_body_position_at_time(self.soi_body, crossing_time, solar_system);
                let soi_body_vel_at_cross = self.get_body_velocity_at_time(self.soi_body, crossing_time, solar_system);

                let new_rel_pos = [
                    cross_rel_pos[0] + soi_body_pos_at_cross[0],
                    cross_rel_pos[1] + soi_body_pos_at_cross[1],
                ];
                let new_rel_vel = [
                    cross_rel_vel[0] + soi_body_vel_at_cross[0],
                    cross_rel_vel[1] + soi_body_vel_at_cross[1],
                ];

                let remaining_dt = dt * (1.0 - crossing_fraction);
                self.rel_position = [
                    new_rel_pos[0] + new_rel_vel[0] * remaining_dt,
                    new_rel_pos[1] + new_rel_vel[1] * remaining_dt,
                ];
                self.rel_velocity = new_rel_vel;
                self.soi_body = parent_idx;

                println!("SOI: Entered {} SOI", solar_system.bodies[parent_idx].name);
                return;
            }
        }

        // Check for entering child SOI
        for (i, body) in solar_system.bodies.iter().enumerate() {
            if body.parent != Some(self.soi_body) {
                continue;
            }

            // child_pos is relative to its parent (= our SOI body), matching prev_rel_pos frame
            let child_pos_start = self.get_body_position_at_time(i, solar_system.time, solar_system);
            let child_pos_end = self.get_body_position_at_time(i, solar_system.time + dt, solar_system);

            let prev_dx = prev_rel_pos[0] - child_pos_start[0];
            let prev_dy = prev_rel_pos[1] - child_pos_start[1];
            let prev_dist_to_child = (prev_dx * prev_dx + prev_dy * prev_dy).sqrt();

            let curr_dx = self.rel_position[0] - child_pos_end[0];
            let curr_dy = self.rel_position[1] - child_pos_end[1];
            let curr_dist_to_child = (curr_dx * curr_dx + curr_dy * curr_dy).sqrt();

            if curr_dist_to_child < body.soi_radius && prev_dist_to_child >= body.soi_radius {
                let crossing_fraction = self.find_soi_entry_fraction_moving(
                    prev_rel_pos,
                    self.rel_position,
                    child_pos_start,
                    child_pos_end,
                    body.soi_radius,
                );

                let crossing_time = solar_system.time + dt * crossing_fraction;

                let child_pos_cross = self.get_body_position_at_time(i, crossing_time, solar_system);
                let child_vel_cross = self.get_body_velocity_at_time(i, crossing_time, solar_system);

                let cross_rel_pos = [
                    prev_rel_pos[0] + crossing_fraction * (self.rel_position[0] - prev_rel_pos[0]),
                    prev_rel_pos[1] + crossing_fraction * (self.rel_position[1] - prev_rel_pos[1]),
                ];
                let cross_rel_vel = [
                    prev_rel_vel[0] + crossing_fraction * (self.rel_velocity[0] - prev_rel_vel[0]),
                    prev_rel_vel[1] + crossing_fraction * (self.rel_velocity[1] - prev_rel_vel[1]),
                ];

                let new_rel_pos = [
                    cross_rel_pos[0] - child_pos_cross[0],
                    cross_rel_pos[1] - child_pos_cross[1],
                ];
                let new_rel_vel = [
                    cross_rel_vel[0] - child_vel_cross[0],
                    cross_rel_vel[1] - child_vel_cross[1],
                ];

                let remaining_dt = dt * (1.0 - crossing_fraction);
                self.rel_position = [
                    new_rel_pos[0] + new_rel_vel[0] * remaining_dt,
                    new_rel_pos[1] + new_rel_vel[1] * remaining_dt,
                ];
                self.rel_velocity = new_rel_vel;
                self.soi_body = i;

                println!("SOI: Entered {} SOI", body.name);
                return;
            }
        }
    }

    /// Find the fraction of the timestep where ship exits current SOI
    fn find_soi_exit_fraction(&self, prev_pos: [f64; 2], curr_pos: [f64; 2], soi_radius: f64) -> f64 {
        let mut lo = 0.0;
        let mut hi = 1.0;

        for _ in 0..BINARY_SEARCH_ITERATIONS {
            let mid = (lo + hi) / 2.0;
            let pos = [
                prev_pos[0] + mid * (curr_pos[0] - prev_pos[0]),
                prev_pos[1] + mid * (curr_pos[1] - prev_pos[1]),
            ];
            let dist = (pos[0] * pos[0] + pos[1] * pos[1]).sqrt();

            if dist < soi_radius {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        (lo + hi) / 2.0
    }

    /// Find the fraction of the timestep where ship enters child SOI
    fn find_soi_entry_fraction_moving(
        &self,
        ship_pos_start: [f64; 2],
        ship_pos_end: [f64; 2],
        body_pos_start: [f64; 2],
        body_pos_end: [f64; 2],
        soi_radius: f64,
    ) -> f64 {
        let mut lo = 0.0;
        let mut hi = 1.0;

        for _ in 0..BINARY_SEARCH_ITERATIONS {
            let mid = (lo + hi) / 2.0;

            let ship_pos = [
                ship_pos_start[0] + mid * (ship_pos_end[0] - ship_pos_start[0]),
                ship_pos_start[1] + mid * (ship_pos_end[1] - ship_pos_start[1]),
            ];

            let body_pos = [
                body_pos_start[0] + mid * (body_pos_end[0] - body_pos_start[0]),
                body_pos_start[1] + mid * (body_pos_end[1] - body_pos_start[1]),
            ];

            let dx = ship_pos[0] - body_pos[0];
            let dy = ship_pos[1] - body_pos[1];
            let dist = (dx * dx + dy * dy).sqrt();

            if dist >= soi_radius {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        (lo + hi) / 2.0
    }

    /// Get position of a body relative to another body
    pub(crate) fn get_body_position_relative(&self, body_idx: usize, relative_to: usize, solar_system: &SolarSystem) -> [f64; 2] {
        let body_pos = solar_system.body_position(body_idx);
        let relative_pos = solar_system.body_position(relative_to);
        [body_pos[0] - relative_pos[0], body_pos[1] - relative_pos[1]]
    }

    /// Get body position at a future time, relative to its parent
    pub(crate) fn get_body_position_at_time(&self, body_idx: usize, time: f64, solar_system: &SolarSystem) -> [f64; 2] {
        let body = &solar_system.bodies[body_idx];
        if let (Some(parent_idx), Some(ref orbit)) = (body.parent, &body.orbit) {
            let parent = &solar_system.bodies[parent_idx];
            orbit.position_at(time, parent.effective_mass_at(orbit.semi_major_axis))
        } else {
            solar_system.body_position(body_idx)
        }
    }

    /// Get body velocity at a future time, relative to its parent
    pub(crate) fn get_body_velocity_at_time(&self, body_idx: usize, time: f64, solar_system: &SolarSystem) -> [f64; 2] {
        let body = &solar_system.bodies[body_idx];
        if let (Some(parent_idx), Some(ref orbit)) = (body.parent, &body.orbit) {
            let parent = &solar_system.bodies[parent_idx];
            let parent_mass = parent.effective_mass_at(orbit.semi_major_axis);
            let mean_anomaly = orbit.mean_anomaly_at(time, parent_mass);
            orbit.velocity_from_mean_anomaly(mean_anomaly, parent_mass)
        } else {
            self.get_body_velocity(body_idx, solar_system)
        }
    }

    /// Convert state vectors when entering a child body's SOI
    pub(crate) fn convert_to_child_frame(
        &self,
        pos: [f64; 2],
        vel: [f64; 2],
        _old_parent_idx: usize,
        new_parent_idx: usize,
        time: f64,
        solar_system: &SolarSystem,
    ) -> ([f64; 2], [f64; 2], bool) {
        let child_pos = self.get_body_position_at_time(new_parent_idx, time, solar_system);
        let child_vel = self.get_body_velocity_at_time(new_parent_idx, time, solar_system);

        let new_pos = [pos[0] - child_pos[0], pos[1] - child_pos[1]];
        let new_vel = [vel[0] - child_vel[0], vel[1] - child_vel[1]];

        let h = new_pos[0] * new_vel[1] - new_pos[1] * new_vel[0];
        let retrograde = h < 0.0;

        (new_pos, new_vel, retrograde)
    }

    /// Convert state vectors when exiting to parent body's SOI
    pub(crate) fn convert_to_parent_frame(
        &self,
        pos: [f64; 2],
        vel: [f64; 2],
        old_parent_idx: usize,
        _new_parent_idx: usize,
        time: f64,
        solar_system: &SolarSystem,
    ) -> ([f64; 2], [f64; 2], bool) {
        let old_parent_pos = self.get_body_position_at_time(old_parent_idx, time, solar_system);
        let old_parent_vel = self.get_body_velocity_at_time(old_parent_idx, time, solar_system);

        let new_pos = [pos[0] + old_parent_pos[0], pos[1] + old_parent_pos[1]];
        let new_vel = [vel[0] + old_parent_vel[0], vel[1] + old_parent_vel[1]];

        let h = new_pos[0] * new_vel[1] - new_pos[1] * new_vel[0];
        let retrograde = h < 0.0;

        (new_pos, new_vel, retrograde)
    }
}
