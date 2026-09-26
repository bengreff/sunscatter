use crate::bodies::{SolarSystem, G, Orbit};
use super::{
    Ship, ShipState, PatchedConicSegment, PatchedTrajectory, MAX_PATCHED_CONICS,
    SOI_INTERSECTION_SAMPLES, SOI_EXIT_THRESHOLD, SOI_ENTRY_THRESHOLD,
    MIN_INTERSECTION_TIME, SOI_REFINE_ITERATIONS, HYPERBOLIC_ANGLE_MARGIN,
};

/// How often to recalculate trajectory (in frames, ~0.5 seconds at 60fps)
const TRAJECTORY_CACHE_FRAMES: u64 = 30;

impl Ship {
    /// Get patched conics trajectory, using cache when possible
    pub fn get_patched_trajectory(&mut self, solar_system: &SolarSystem) -> Option<PatchedTrajectory> {
        // Increment frame counter
        self.frame_counter += 1;

        // Invalidate cache if:
        // 1. Ship is thrusting (orbit changing)
        // 2. SOI body changed
        // 3. Enough frames have passed
        let frames_since_calc = self.frame_counter.saturating_sub(self.trajectory_calc_frame);
        let cache_valid = self.throttle == 0.0
            && !self.on_rails
            && self.soi_body == self.trajectory_soi_body
            && frames_since_calc < TRAJECTORY_CACHE_FRAMES
            && self.cached_trajectory.is_some();

        if cache_valid {
            return self.cached_trajectory.clone();
        }

        // Recalculate and cache
        let trajectory = self.calculate_patched_trajectory_internal(solar_system);
        self.cached_trajectory = trajectory.clone();
        self.trajectory_calc_frame = self.frame_counter;
        self.trajectory_soi_body = self.soi_body;
        trajectory
    }

    /// Calculate patched conics trajectory predicting future SOI changes (internal, uncached)
    fn calculate_patched_trajectory_internal(&self, solar_system: &SolarSystem) -> Option<PatchedTrajectory> {
        if matches!(self.state, ShipState::Landed { .. }) {
            return None;
        }

        let parent_idx = self.soi_body;
        let parent = &solar_system.bodies[parent_idx];

        let r = (self.rel_position[0].powi(2) + self.rel_position[1].powi(2)).sqrt();
        let (orbit, true_anomaly, retrograde) = self.calculate_orbit_from_state(
            self.rel_position,
            self.rel_velocity,
            parent.effective_mass_at(r),
        )?;

        let segments = self.compute_patched_segments(
            orbit, true_anomaly, retrograde, parent_idx,
            solar_system.time, solar_system,
        );

        if segments.is_empty() { None } else { Some(PatchedTrajectory { segments }) }
    }

    /// Compute patched conic segments from given orbital state.
    /// Handles both elliptical and hyperbolic orbits, continuing across SOI boundaries
    /// up to MAX_PATCHED_CONICS transitions.
    pub(crate) fn compute_patched_segments(
        &self,
        initial_orbit: Orbit,
        initial_ta: f64,
        initial_retrograde: bool,
        initial_parent_idx: usize,
        base_time: f64,
        solar_system: &SolarSystem,
    ) -> Vec<PatchedConicSegment> {
        let mut segments = Vec::new();
        let mut current_orbit = initial_orbit;
        let mut current_parent_idx = initial_parent_idx;
        let mut current_ta = initial_ta;
        let mut current_retrograde = initial_retrograde;
        let mut cumulative_time = 0.0;
        let mut soi_crossings = 0;

        loop {
            let parent = &solar_system.bodies[current_parent_idx];

            if current_orbit.eccentricity >= 1.0 {
                // Hyperbolic orbit - calculate SOI exit analytically
                let e = current_orbit.eccentricity;
                let a_abs = current_orbit.semi_major_axis.abs();
                let p = a_abs * (e * e - 1.0);
                let soi_radius = parent.soi_radius;
                let cos_nu_exit = (p / soi_radius - 1.0) / e;

                let exit_true_anomaly = if cos_nu_exit.abs() <= 1.0 {
                    let ta = cos_nu_exit.acos();
                    if current_retrograde { -ta } else { ta }
                } else {
                    let ta = (-1.0 / e).acos() - HYPERBOLIC_ANGLE_MARGIN;
                    if current_retrograde { -ta } else { ta }
                };

                // Compute exit state (position/velocity at SOI boundary) and push segments
                let exit_pos: [f64; 2];
                let exit_vel: [f64; 2];
                let time_to_exit: f64;

                if parent.galactic_mass_profile {
                    // For galactic mass profile bodies, the enclosed mass varies dramatically
                    // with distance (4-component Milky Way model). Subdivide the trajectory
                    // into sub-arcs, recomputing orbital elements at each boundary using
                    // effective_mass_at(r) for accurate periapsis prediction.
                    let num_subs: usize = 32;
                    let mut sub_orbit = current_orbit;
                    let mut sub_ta = current_ta;
                    let mut sub_retro = current_retrograde;
                    let mut sub_time = 0.0_f64;

                    // Compute initial mass from actual distance, not semi-major axis
                    let init_ma = self.true_to_mean_anomaly(&current_orbit, current_ta);
                    let init_pos = current_orbit.position_from_mean_anomaly(init_ma, 0.0);
                    let r_init = (init_pos[0].powi(2) + init_pos[1].powi(2)).sqrt();
                    let mut sub_mass = parent.effective_mass_at(r_init);
                    let mut became_elliptical = false;

                    for i in 0..num_subs {
                        let remaining = num_subs - i;
                        let e = sub_orbit.eccentricity;

                        if e < 1.0 {
                            // Orbit became elliptical during subdivision — no SOI exit.
                            // Push a full-orbit segment and stop.
                            segments.push(PatchedConicSegment {
                                orbit: sub_orbit,
                                parent_idx: current_parent_idx,
                                retrograde: sub_retro,
                                start_true_anomaly: sub_ta,
                                end_true_anomaly: None,
                                start_time: cumulative_time + sub_time,
                                end_time: None,
                            });
                            became_elliptical = true;
                            break;
                        }

                        // Compute SOI exit TA on the current sub-orbit
                        let sub_a_abs = sub_orbit.semi_major_axis.abs();
                        let p = sub_a_abs * (e * e - 1.0);
                        let cos_exit = (p / soi_radius - 1.0) / e;
                        let sub_exit_ta = if cos_exit.abs() <= 1.0 {
                            let ta = cos_exit.acos();
                            if sub_retro { -ta } else { ta }
                        } else {
                            let ta = (-1.0 / e).acos() - HYPERBOLIC_ANGLE_MARGIN;
                            if sub_retro { -ta } else { ta }
                        };

                        // Advance by 1/remaining of the range to SOI exit
                        let ta_range = sub_exit_ta - sub_ta;
                        let end_sub_ta = if remaining == 1 {
                            sub_exit_ta
                        } else {
                            sub_ta + ta_range / remaining as f64
                        };

                        // Time for this sub-arc
                        let mu = G * sub_mass;
                        let n = (mu / sub_a_abs.powi(3)).sqrt();
                        let start_ma = self.true_to_mean_anomaly(&sub_orbit, sub_ta);
                        let end_ma = self.true_to_mean_anomaly(&sub_orbit, end_sub_ta);
                        let dt_sub = if n > 0.0 { (end_ma - start_ma).abs() / n } else { 0.0 };

                        segments.push(PatchedConicSegment {
                            orbit: sub_orbit,
                            parent_idx: current_parent_idx,
                            retrograde: sub_retro,
                            start_true_anomaly: sub_ta,
                            end_true_anomaly: Some(end_sub_ta),
                            start_time: cumulative_time + sub_time,
                            end_time: Some(cumulative_time + sub_time + dt_sub),
                        });

                        sub_time += dt_sub;

                        if remaining > 1 {
                            // Extract state vectors at end of this sub-arc
                            let pos = sub_orbit.position_from_mean_anomaly(end_ma, sub_mass);
                            let vel = sub_orbit.velocity_from_mean_anomaly_with_direction(
                                end_ma, sub_mass, sub_retro,
                            );

                            // Recompute orbit with local enclosed mass
                            let r_new = (pos[0].powi(2) + pos[1].powi(2)).sqrt();
                            let new_mass = parent.effective_mass_at(r_new);
                            if let Some((new_orb, new_ta, new_ret)) =
                                self.calculate_orbit_from_state(pos, vel, new_mass)
                            {
                                sub_orbit = new_orb;
                                sub_ta = new_ta;
                                sub_retro = new_ret;
                                sub_mass = new_mass;
                            } else {
                                log::warn!("Galactic subdivision: orbit recalculation failed at r={:.3e}", r_new);
                                break;
                            }
                        }
                    }

                    if became_elliptical {
                        break; // No SOI exit — trajectory ends here
                    }

                    // Exit state from the last sub-orbit's endpoint
                    let last_end_ta = segments.last()
                        .and_then(|s| s.end_true_anomaly)
                        .unwrap_or(sub_ta);
                    let last_exit_ma = self.true_to_mean_anomaly(&sub_orbit, last_end_ta);
                    exit_pos = sub_orbit.position_from_mean_anomaly(last_exit_ma, sub_mass);
                    exit_vel = sub_orbit.velocity_from_mean_anomaly_with_direction(
                        last_exit_ma, sub_mass, sub_retro,
                    );
                    time_to_exit = sub_time;
                } else {
                    // Standard single-segment trajectory for non-galactic bodies
                    let exit_ma = self.true_to_mean_anomaly(&current_orbit, exit_true_anomaly);
                    let start_ma = self.true_to_mean_anomaly(&current_orbit, current_ta);
                    let parent_mass_eff = parent.effective_mass_at(a_abs);
                    let mu = G * parent_mass_eff;
                    let n = (mu / a_abs.powi(3)).sqrt();
                    time_to_exit = if n > 0.0 {
                        (exit_ma - start_ma).abs() / n
                    } else {
                        0.0
                    };

                    segments.push(PatchedConicSegment {
                        orbit: current_orbit,
                        parent_idx: current_parent_idx,
                        retrograde: current_retrograde,
                        start_true_anomaly: current_ta,
                        end_true_anomaly: Some(exit_true_anomaly),
                        start_time: cumulative_time,
                        end_time: Some(cumulative_time + time_to_exit),
                    });

                    exit_pos = current_orbit.position_from_mean_anomaly(exit_ma, parent_mass_eff);
                    exit_vel = current_orbit.velocity_from_mean_anomaly_with_direction(
                        exit_ma, parent_mass_eff, current_retrograde,
                    );
                }

                // Try to continue to parent body after SOI exit
                if soi_crossings >= MAX_PATCHED_CONICS {
                    break;
                }

                if let Some(grandparent_idx) = parent.parent {
                    let exit_absolute_time = base_time + cumulative_time + time_to_exit;

                    let (new_pos, new_vel, _) = self.convert_to_parent_frame(
                        exit_pos, exit_vel,
                        current_parent_idx,
                        grandparent_idx,
                        exit_absolute_time,
                        solar_system,
                    );

                    let grandparent = &solar_system.bodies[grandparent_idx];
                    let r_new = (new_pos[0].powi(2) + new_pos[1].powi(2)).sqrt();
                    if let Some((new_orbit, new_ta, new_retro)) = self.calculate_orbit_from_state(
                        new_pos, new_vel, grandparent.effective_mass_at(r_new),
                    ) {
                        cumulative_time += time_to_exit;
                        current_orbit = new_orbit;
                        current_parent_idx = grandparent_idx;
                        current_ta = new_ta;
                        current_retrograde = new_retro;
                        soi_crossings += 1;
                        continue;
                    }
                }
                break;
            }

            // Elliptical orbit - if max crossings reached, push full orbit as final segment
            if soi_crossings >= MAX_PATCHED_CONICS {
                segments.push(PatchedConicSegment {
                    orbit: current_orbit,
                    parent_idx: current_parent_idx,
                    retrograde: current_retrograde,
                    start_true_anomaly: current_ta,
                    end_true_anomaly: None,
                    start_time: cumulative_time,
                    end_time: None,
                });
                break;
            }

            let current_ma = self.true_to_mean_anomaly(&current_orbit, current_ta);

            let intersection = self.find_soi_intersection(
                &current_orbit,
                current_parent_idx,
                current_ma,
                current_retrograde,
                solar_system,
                base_time + cumulative_time,
            );

            match intersection {
                Some((intersect_ta, intersect_time, new_parent_idx, entry)) => {
                    let intersect_pos: [f64; 2];
                    let intersect_vel: [f64; 2];
                    let actual_intersect_time: f64;

                    if parent.galactic_mass_profile {
                        // Subdivide elliptical arc to intersection for varying enclosed mass
                        let num_subs: usize = 32;
                        let mut sub_orbit = current_orbit;
                        let mut sub_ta = current_ta;
                        let mut sub_retro = current_retrograde;
                        let mut sub_time = 0.0_f64;

                        let init_ma = self.true_to_mean_anomaly(&current_orbit, current_ta);
                        let init_pos = current_orbit.position_from_mean_anomaly(init_ma, 0.0);
                        let r_init = (init_pos[0].powi(2) + init_pos[1].powi(2)).sqrt();
                        let mut sub_mass = parent.effective_mass_at(r_init);

                        let dt_step = intersect_time / num_subs as f64;

                        for i in 0..num_subs {
                            let mu = G * sub_mass;
                            let a = sub_orbit.semi_major_axis;
                            let n = (mu / a.powi(3)).sqrt();
                            let start_ma = self.true_to_mean_anomaly(&sub_orbit, sub_ta);

                            let delta_ma = n * dt_step;
                            let end_ma = if sub_retro {
                                (start_ma - delta_ma).rem_euclid(std::f64::consts::TAU)
                            } else {
                                (start_ma + delta_ma).rem_euclid(std::f64::consts::TAU)
                            };
                            let end_sub_ta = self.mean_to_true_anomaly(&sub_orbit, end_ma);

                            segments.push(PatchedConicSegment {
                                orbit: sub_orbit,
                                parent_idx: current_parent_idx,
                                retrograde: sub_retro,
                                start_true_anomaly: sub_ta,
                                end_true_anomaly: Some(end_sub_ta),
                                start_time: cumulative_time + sub_time,
                                end_time: Some(cumulative_time + sub_time + dt_step),
                            });

                            sub_time += dt_step;

                            if i < num_subs - 1 {
                                let pos = sub_orbit.position_from_mean_anomaly(end_ma, sub_mass);
                                let vel = sub_orbit.velocity_from_mean_anomaly_with_direction(
                                    end_ma, sub_mass, sub_retro,
                                );

                                let r_new = (pos[0].powi(2) + pos[1].powi(2)).sqrt();
                                let new_mass = parent.effective_mass_at(r_new);
                                if let Some((new_orb, new_ta, new_ret)) =
                                    self.calculate_orbit_from_state(pos, vel, new_mass)
                                {
                                    sub_orbit = new_orb;
                                    sub_ta = new_ta;
                                    sub_retro = new_ret;
                                    sub_mass = new_mass;
                                } else {
                                    break;
                                }
                            }
                        }

                        let last_end_ta = segments.last()
                            .and_then(|s| s.end_true_anomaly)
                            .unwrap_or(sub_ta);
                        let last_ma = self.true_to_mean_anomaly(&sub_orbit, last_end_ta);
                        intersect_pos = sub_orbit.position_from_mean_anomaly(last_ma, sub_mass);
                        intersect_vel = sub_orbit.velocity_from_mean_anomaly_with_direction(
                            last_ma, sub_mass, sub_retro,
                        );
                        actual_intersect_time = sub_time;
                    } else {
                        segments.push(PatchedConicSegment {
                            orbit: current_orbit,
                            parent_idx: current_parent_idx,
                            retrograde: current_retrograde,
                            start_true_anomaly: current_ta,
                            end_true_anomaly: Some(intersect_ta),
                            start_time: cumulative_time,
                            end_time: Some(cumulative_time + intersect_time),
                        });

                        let intersect_mean_anomaly = self.true_to_mean_anomaly(&current_orbit, intersect_ta);
                        let eff_mass = parent.effective_mass_at(current_orbit.semi_major_axis);
                        intersect_pos = current_orbit.position_from_mean_anomaly(intersect_mean_anomaly, eff_mass);
                        intersect_vel = current_orbit.velocity_from_mean_anomaly_with_direction(
                            intersect_mean_anomaly,
                            eff_mass,
                            current_retrograde,
                        );
                        actual_intersect_time = intersect_time;
                    }

                    let absolute_intersect_time = base_time + cumulative_time + actual_intersect_time;
                    let (new_pos, new_vel, _) = if entry {
                        self.convert_to_child_frame(
                            intersect_pos, intersect_vel,
                            current_parent_idx,
                            new_parent_idx,
                            absolute_intersect_time,
                            solar_system,
                        )
                    } else {
                        self.convert_to_parent_frame(
                            intersect_pos, intersect_vel,
                            current_parent_idx,
                            new_parent_idx,
                            absolute_intersect_time,
                            solar_system,
                        )
                    };

                    let new_parent = &solar_system.bodies[new_parent_idx];
                    let r_new = (new_pos[0].powi(2) + new_pos[1].powi(2)).sqrt();
                    if let Some((new_orbit, new_ta, new_retro)) = self.calculate_orbit_from_state(
                        new_pos, new_vel, new_parent.effective_mass_at(r_new),
                    ) {
                        cumulative_time += actual_intersect_time;
                        current_orbit = new_orbit;
                        current_parent_idx = new_parent_idx;
                        current_ta = new_ta;
                        current_retrograde = new_retro;
                        soi_crossings += 1;
                        continue;
                    }
                    break;
                }
                None => {
                    if parent.galactic_mass_profile {
                        // Subdivide one full orbit for accurate display with varying enclosed mass.
                        // The orbit forms a rosette pattern, not a closed ellipse.
                        let num_subs: usize = 32;
                        let mut sub_orbit = current_orbit;
                        let mut sub_ta = current_ta;
                        let mut sub_retro = current_retrograde;
                        let mut sub_time = 0.0_f64;

                        let init_ma = self.true_to_mean_anomaly(&current_orbit, current_ta);
                        let init_pos = current_orbit.position_from_mean_anomaly(init_ma, 0.0);
                        let r_init = (init_pos[0].powi(2) + init_pos[1].powi(2)).sqrt();
                        let mut sub_mass = parent.effective_mass_at(r_init);

                        // Estimate one orbital period from current parameters
                        let mu_init = G * sub_mass;
                        let period_est = std::f64::consts::TAU
                            * (current_orbit.semi_major_axis.powi(3) / mu_init).sqrt();
                        let dt_step = period_est / num_subs as f64;

                        for i in 0..num_subs {
                            let mu = G * sub_mass;
                            let a = sub_orbit.semi_major_axis;
                            let n = (mu / a.powi(3)).sqrt();
                            let start_ma = self.true_to_mean_anomaly(&sub_orbit, sub_ta);

                            let delta_ma = n * dt_step;
                            let end_ma = if sub_retro {
                                (start_ma - delta_ma).rem_euclid(std::f64::consts::TAU)
                            } else {
                                (start_ma + delta_ma).rem_euclid(std::f64::consts::TAU)
                            };
                            let end_sub_ta = self.mean_to_true_anomaly(&sub_orbit, end_ma);

                            segments.push(PatchedConicSegment {
                                orbit: sub_orbit,
                                parent_idx: current_parent_idx,
                                retrograde: sub_retro,
                                start_true_anomaly: sub_ta,
                                end_true_anomaly: Some(end_sub_ta),
                                start_time: cumulative_time + sub_time,
                                end_time: Some(cumulative_time + sub_time + dt_step),
                            });

                            sub_time += dt_step;

                            if i < num_subs - 1 {
                                let pos = sub_orbit.position_from_mean_anomaly(end_ma, sub_mass);
                                let vel = sub_orbit.velocity_from_mean_anomaly_with_direction(
                                    end_ma, sub_mass, sub_retro,
                                );

                                let r_new = (pos[0].powi(2) + pos[1].powi(2)).sqrt();
                                let new_mass = parent.effective_mass_at(r_new);
                                if let Some((new_orb, new_ta, new_ret)) =
                                    self.calculate_orbit_from_state(pos, vel, new_mass)
                                {
                                    sub_orbit = new_orb;
                                    sub_ta = new_ta;
                                    sub_retro = new_ret;
                                    sub_mass = new_mass;
                                } else {
                                    break;
                                }
                            }
                        }
                    } else {
                        segments.push(PatchedConicSegment {
                            orbit: current_orbit,
                            parent_idx: current_parent_idx,
                            retrograde: current_retrograde,
                            start_true_anomaly: current_ta,
                            end_true_anomaly: None,
                            start_time: cumulative_time,
                            end_time: None,
                        });
                    }
                    break;
                }
            }
        }

        segments
    }

    /// Find the next SOI intersection along the orbit
    /// Returns: (true_anomaly, time_to_reach, new_parent_idx, is_entry)
    pub(crate) fn find_soi_intersection(
        &self,
        orbit: &Orbit,
        parent_idx: usize,
        start_mean_anomaly: f64,
        retrograde: bool,
        solar_system: &SolarSystem,
        base_time: f64,
    ) -> Option<(f64, f64, usize, bool)> {
        let parent = &solar_system.bodies[parent_idx];
        let parent_mass_eff = if parent.galactic_mass_profile {
            // Use mass at current orbital distance for better period estimate
            let start_pos = orbit.position_from_mean_anomaly(start_mean_anomaly, 0.0);
            let r = (start_pos[0].powi(2) + start_pos[1].powi(2)).sqrt();
            parent.effective_mass_at(r)
        } else {
            parent.effective_mass_at(orbit.semi_major_axis)
        };
        let mu = G * parent_mass_eff;
        let period = std::f64::consts::TAU * (orbit.semi_major_axis.powi(3) / mu).sqrt();
        let mean_motion = std::f64::consts::TAU / period;

        let apoapsis = orbit.semi_major_axis * (1.0 + orbit.eccentricity);
        let orbit_escapes = parent.parent.is_some() && apoapsis > parent.soi_radius;

        // Ship's orbital range (periapsis to apoapsis)
        let ship_periapsis = orbit.semi_major_axis * (1.0 - orbit.eccentricity);
        let ship_apoapsis = orbit.semi_major_axis * (1.0 + orbit.eccentricity);

        // Only check child bodies whose orbits could potentially intersect with ship's orbit
        let child_bodies: Vec<usize> = solar_system.bodies.iter()
            .enumerate()
            .filter(|(_, b)| b.parent == Some(parent_idx))
            .filter(|(_, b)| {
                // Check if child's orbital range overlaps with ship's orbital range
                if let Some(ref child_orbit) = b.orbit {
                    let child_periapsis = child_orbit.semi_major_axis * (1.0 - child_orbit.eccentricity);
                    let child_apoapsis = child_orbit.semi_major_axis * (1.0 + child_orbit.eccentricity);
                    // Add SOI radius as margin for intersection check
                    let child_inner = child_periapsis - b.soi_radius;
                    let child_outer = child_apoapsis + b.soi_radius;
                    // Orbits can intersect if ranges overlap
                    ship_apoapsis >= child_inner && ship_periapsis <= child_outer
                } else {
                    true // No orbit data, include to be safe
                }
            })
            .map(|(i, _)| i)
            .collect();
        let has_children = !child_bodies.is_empty();

        // Early exit: if orbit doesn't escape and there are no children, no SOI transitions possible
        if !orbit_escapes && !has_children {
            return None;
        }

        // Fast path: if orbit escapes but there are no children, use analytical SOI exit calculation
        if orbit_escapes && !has_children {
            if let Some(grandparent_idx) = parent.parent {
                let e = orbit.eccentricity;
                let a = orbit.semi_major_axis;
                let p = a * (1.0 - e * e); // semi-latus rectum
                let cos_nu = (p / parent.soi_radius - 1.0) / e;

                if cos_nu.abs() <= 1.0 {
                    let nu_exit = cos_nu.acos();
                    let exit_candidates = [nu_exit, std::f64::consts::TAU - nu_exit];

                    for exit_nu in exit_candidates {
                        let exit_ma = self.true_to_mean_anomaly(orbit, exit_nu);

                        let delta_ma = if retrograde {
                            let mut d = start_mean_anomaly - exit_ma;
                            if d < 0.0 { d += std::f64::consts::TAU; }
                            d
                        } else {
                            let mut d = exit_ma - start_mean_anomaly;
                            if d < 0.0 { d += std::f64::consts::TAU; }
                            d
                        };

                        let time_to_exit = delta_ma / mean_motion;

                        if time_to_exit > MIN_INTERSECTION_TIME {
                            return Some((exit_nu, time_to_exit, grandparent_idx, false));
                        }
                    }
                }
            }
            return None;
        }

        let num_samples = SOI_INTERSECTION_SAMPLES;
        let mut best_intersection: Option<(f64, f64, usize, bool)> = None;
        let mut min_time = f64::MAX;

        let get_sample_m = |delta_m: f64| -> f64 {
            if retrograde {
                (start_mean_anomaly - delta_m).rem_euclid(std::f64::consts::TAU)
            } else {
                (start_mean_anomaly + delta_m).rem_euclid(std::f64::consts::TAU)
            }
        };

        // Coarse sampling to find approximate intersections (only when there are children to check)
        for i in 0..num_samples {
            let fraction = i as f64 / num_samples as f64;
            let delta_m = fraction * std::f64::consts::TAU;
            let sample_m = get_sample_m(delta_m);
            let time_to_sample = delta_m / mean_motion;

            if time_to_sample >= min_time {
                continue;
            }

            let pos = orbit.position_from_mean_anomaly(sample_m, parent_mass_eff);
            let dist_from_parent = (pos[0].powi(2) + pos[1].powi(2)).sqrt();

            // Check if exiting current SOI
            if dist_from_parent > parent.soi_radius * SOI_EXIT_THRESHOLD {
                if let Some(grandparent_idx) = parent.parent {
                    let true_anomaly = self.mean_to_true_anomaly(orbit, sample_m);
                    if time_to_sample < min_time && time_to_sample > MIN_INTERSECTION_TIME {
                        min_time = time_to_sample;
                        best_intersection = Some((true_anomaly, time_to_sample, grandparent_idx, false));
                    }
                }
            }

            // Check if entering a child body's SOI (only if there are children)
            if has_children {
                for &child_idx in &child_bodies {
                    let child = &solar_system.bodies[child_idx];

                    let child_pos = if let Some(ref child_orbit) = child.orbit {
                        child_orbit.position_at(base_time + time_to_sample, parent.effective_mass_at(child_orbit.semi_major_axis))
                    } else {
                        self.get_body_position_relative(child_idx, parent_idx, solar_system)
                    };

                    let dx = pos[0] - child_pos[0];
                    let dy = pos[1] - child_pos[1];
                    let dist_from_child = (dx.powi(2) + dy.powi(2)).sqrt();

                    if dist_from_child < child.soi_radius * SOI_ENTRY_THRESHOLD {
                        let true_anomaly = self.mean_to_true_anomaly(orbit, sample_m);
                        if time_to_sample < min_time && time_to_sample > MIN_INTERSECTION_TIME {
                            min_time = time_to_sample;
                            best_intersection = Some((true_anomaly, time_to_sample, child_idx, true));
                        }
                    }
                }
            }
        }

        // Refine child body intersections with binary search
        if let Some((_, approx_time, child_idx, true)) = best_intersection {
            let child = &solar_system.bodies[child_idx];
            if let Some(ref child_orbit) = child.orbit {
                let search_window = std::f64::consts::TAU / num_samples as f64 * 2.0;
                let approx_delta_m = approx_time * mean_motion;

                let mut low_delta_m = (approx_delta_m - search_window).max(0.0);
                let mut high_delta_m = approx_delta_m + search_window;

                for _ in 0..SOI_REFINE_ITERATIONS {
                    let mid_delta_m = (low_delta_m + high_delta_m) / 2.0;
                    let mid_sample_m = get_sample_m(mid_delta_m);
                    let mid_time = mid_delta_m / mean_motion;

                    let ship_pos = orbit.position_from_mean_anomaly(mid_sample_m, parent_mass_eff);
                    let child_pos = child_orbit.position_at(base_time + mid_time, parent.effective_mass_at(child_orbit.semi_major_axis));

                    let dx = ship_pos[0] - child_pos[0];
                    let dy = ship_pos[1] - child_pos[1];
                    let dist = (dx.powi(2) + dy.powi(2)).sqrt();

                    if dist < child.soi_radius {
                        high_delta_m = mid_delta_m;
                    } else {
                        low_delta_m = mid_delta_m;
                    }
                }

                let refined_delta_m = (low_delta_m + high_delta_m) / 2.0;
                let refined_sample_m = get_sample_m(refined_delta_m);
                let refined_time = refined_delta_m / mean_motion;
                let refined_ta = self.mean_to_true_anomaly(orbit, refined_sample_m);

                best_intersection = Some((refined_ta, refined_time, child_idx, true));
            }
        }

        // Calculate analytically if orbit escapes but sampling didn't find intersection
        if best_intersection.is_none() && orbit_escapes {
            if let Some(grandparent_idx) = parent.parent {
                let e = orbit.eccentricity;
                let a = orbit.semi_major_axis;
                let p = a * (1.0 - e * e);
                let cos_nu = (p / parent.soi_radius - 1.0) / e;

                if cos_nu.abs() <= 1.0 {
                    let nu_exit = cos_nu.acos();
                    let exit_candidates = [nu_exit, std::f64::consts::TAU - nu_exit];

                    for exit_nu in exit_candidates {
                        let exit_ma = self.true_to_mean_anomaly(orbit, exit_nu);

                        let delta_ma = if retrograde {
                            let mut d = start_mean_anomaly - exit_ma;
                            if d < 0.0 { d += std::f64::consts::TAU; }
                            d
                        } else {
                            let mut d = exit_ma - start_mean_anomaly;
                            if d < 0.0 { d += std::f64::consts::TAU; }
                            d
                        };

                        let time_to_exit = delta_ma / mean_motion;

                        if time_to_exit > MIN_INTERSECTION_TIME && time_to_exit < min_time {
                            min_time = time_to_exit;
                            best_intersection = Some((exit_nu, time_to_exit, grandparent_idx, false));
                        }
                    }
                }
            }
        }

        best_intersection
    }
}
