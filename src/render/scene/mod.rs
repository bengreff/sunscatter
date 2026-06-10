mod bodies;
mod effects;
mod galaxy;

use super::formatting::apply_heat_tint;
use super::types::{
    OrbitRenderData, ShipRenderData, TrackingVesselData, Vertex, HYPERBOLIC_RENDER_MARGIN,
    orbit_segments,
};
use super::state::RenderState;

/// Pre-computed render data for a procedural star dot.
/// Built in main.rs from GalaxyState, consumed by scene.rs.
#[derive(Clone)]
pub struct StarRenderData {
    pub x: f64,   // world position in meters
    pub y: f64,
    pub color: [f32; 3],
    pub luminosity: f32,    // solar luminosities (determines dot size)
    pub radius_m: f64,      // physical radius in meters (for real circle rendering)
    pub temperature: f32,   // Kelvin
    pub mass_solar: f64,    // solar masses
    pub star_type: &'static str,  // display string e.g. "G-type Main Sequence"
    // Decomposed catalog ID (avoids per-frame String allocation)
    pub catalog_prefix: &'static str, // e.g. "G", "WD", "RG"
    pub sector_x: u16,
    pub sector_y: u16,
    pub sector_index: u32,
    // Pre-computed rendering values (avoids per-frame log/ln)
    pub alpha: f32,       // brightness alpha from luminosity
    pub lum_factor: f32,  // hexagon size factor from luminosity
    // Galactic orbital characteristics
    pub semi_major_axis_m: f64,  // galactic orbit semi-major axis (meters)
    pub eccentricity: f32,       // galactic orbit eccentricity
    pub arg_periapsis: f32,      // galactic orbit argument of periapsis (radians)
    pub orbital_period_s: f64,   // galactic orbital period (seconds)
    // Catalog star fields (None/0 for procedural stars)
    pub catalog_name: Option<&'static str>,
    pub catalog_index: u16,
    pub num_catalog_stars: u8,  // number of stars in catalog system (0 for procedural)
}

impl StarRenderData {
    /// Format the display name: real name for catalog stars, procedural ID otherwise.
    /// Multi-star catalog systems get " System" suffix.
    pub fn format_name(&self) -> String {
        if let Some(name) = self.catalog_name {
            if self.num_catalog_stars > 1 {
                format!("{} System", name)
            } else {
                name.to_string()
            }
        } else {
            format!("{}-{:04}-{:04}-{:04}", self.catalog_prefix, self.sector_x, self.sector_y, self.sector_index)
        }
    }

    /// Check if this star matches the given sector identity.
    pub fn matches_id(&self, sector_x: u16, sector_y: u16, sector_index: u32) -> bool {
        self.sector_x == sector_x && self.sector_y == sector_y && self.sector_index == sector_index
    }
}

impl RenderState {
    /// Update geometry with multiple bodies and their orbits
    /// scale: world units per meter (e.g., 1e-9 means 1 billion meters = 1 world unit)
    pub fn update_bodies_with_orbits(
        &mut self,
        bodies: &[(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)],
        orbits: &[Option<OrbitRenderData>],
        scale: f64,
    ) {
        let mut all_vertices = Vec::new();
        let mut all_indices = Vec::new();

        // Get camera position for relative coordinate calculation
        let cam_x = self.camera.body_center[0];
        let cam_y = self.camera.body_center[1];
        let off_x = self.camera.ship_offset[0];
        let off_y = self.camera.ship_offset[1];

        // First, draw all orbit lines (so they appear behind bodies)
        for orbit_opt in orbits {
            if let Some(orbit) = orbit_opt {
                let base_index = all_vertices.len() as u32;

                // Ellipse parameters
                let a = orbit.semi_major_axis; // semi-major axis
                let e = orbit.eccentricity;
                let b = a * (1.0 - e * e).sqrt(); // semi-minor axis
                let c = a * e; // distance from center to focus

                // The parent is at one focus, so ellipse center is offset
                let arg_peri = orbit.argument_of_periapsis;
                let center_x = orbit.parent_x - c * arg_peri.cos();
                let center_y = orbit.parent_y - c * arg_peri.sin();

                let segments = orbit_segments(a, self.camera.zoom, self.size.height as f32);
                let line_width = 0.002 / self.camera.zoom as f64; // Thin line in world units

                // Generate orbit ellipse vertices (inner and outer for line thickness)
                for i in 0..segments {
                    let angle = (i as f64 / segments as f64) * std::f64::consts::TAU;

                    // Point on ellipse (before rotation)
                    let ex = a * angle.cos();
                    let ey = b * angle.sin();

                    // Rotate by argument of periapsis
                    let rx = ex * arg_peri.cos() - ey * arg_peri.sin();
                    let ry = ex * arg_peri.sin() + ey * arg_peri.cos();

                    // Final position
                    let px = center_x + rx;
                    let py = center_y + ry;

                    // Calculate normal for line thickness
                    let next_angle = ((i + 1) as f64 / segments as f64) * std::f64::consts::TAU;
                    let next_ex = a * next_angle.cos();
                    let next_ey = b * next_angle.sin();
                    let next_rx = next_ex * arg_peri.cos() - next_ey * arg_peri.sin();
                    let next_ry = next_ex * arg_peri.sin() + next_ey * arg_peri.cos();

                    let dx = next_rx - rx;
                    let dy = next_ry - ry;
                    let len = (dx * dx + dy * dy).sqrt();
                    let nx = -dy / len * line_width;
                    let ny = dx / len * line_width;

                    // Outer vertex
                    let rel_outer_x = (px + nx - cam_x - off_x) as f32;
                    let rel_outer_y = (py + ny - cam_y - off_y) as f32;
                    all_vertices.push(Vertex::new([rel_outer_x, rel_outer_y], orbit.color));

                    // Inner vertex
                    let rel_inner_x = (px - nx - cam_x - off_x) as f32;
                    let rel_inner_y = (py - ny - cam_y - off_y) as f32;
                    all_vertices.push(Vertex::new([rel_inner_x, rel_inner_y], [orbit.color[0] * 0.5, orbit.color[1] * 0.5, orbit.color[2] * 0.5, orbit.color[3] * 0.7]));
                }

                // Create indices for orbit ring
                for i in 0..segments {
                    let i0 = base_index + i * 2;
                    let i1 = base_index + i * 2 + 1;
                    let i2 = base_index + ((i + 1) % segments) * 2;
                    let i3 = base_index + ((i + 1) % segments) * 2 + 1;

                    all_indices.push(i0);
                    all_indices.push(i2);
                    all_indices.push(i1);

                    all_indices.push(i1);
                    all_indices.push(i2);
                    all_indices.push(i3);
                }
            }
        }

        // Now draw bodies on top of orbit lines
        self.add_body_vertices(&mut all_vertices, &mut all_indices, bodies, scale);

        self.num_indices = all_indices.len() as u32;

        // Update buffers
        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&all_vertices));
        self.queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&all_indices));
    }

    /// Update geometry with bodies, orbits, and optionally a ship
    pub fn update_bodies_orbits_and_ship(
        &mut self,
        bodies: &[(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)],
        orbits: &[Option<OrbitRenderData>],
        ship: Option<&ShipRenderData>,
        scale: f64,
        part_defs: Option<&crate::parts::PartDefinitions>,
    ) {
        self.update_bodies_orbits_ship_and_vessels(bodies, orbits, ship, scale, part_defs, &[], &[], false, &[]);
    }

    /// Update geometry with bodies, orbits, optionally a ship, and background vessels
    pub fn update_bodies_orbits_ship_and_vessels(
        &mut self,
        bodies: &[(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)],
        orbits: &[Option<OrbitRenderData>],
        ship: Option<&ShipRenderData>,
        scale: f64,
        part_defs: Option<&crate::parts::PartDefinitions>,
        background_vessels: &[TrackingVesselData],
        accretion_discs: &[Option<crate::bodies::AccretionDisc>],
        in_galaxy_view: bool,
        procedural_stars: &[StarRenderData],
    ) {
        // Store ship info for UI display
        self.ship_orbit_info = ship.and_then(|s| s.orbit.clone());
        if let Some(s) = ship {
            self.ship_velocity = s.velocity;
            self.ship_altitude = s.altitude;
            self.ship_throttle = s.throttle;
            self.ship_soi_name = s.soi_body_name.clone();
            self.ship_time_to_intercept = s.time_to_intercept;
            self.ship_acceleration = s.acceleration;
            self.ship_current_true_anomaly = s.current_true_anomaly;
            self.vessel_total_mass = s.total_mass;
            self.vessel_fuel_fraction = s.fuel_fraction;
            self.vessel_monoprop_fraction = s.monoprop_fraction;
            self.vessel_power_generation = s.power_generation;
            self.vessel_power_consumption = s.power_consumption;
            self.vessel_electricity_fraction = s.electricity_fraction;
            self.vessel_electricity_stored = s.electricity_stored;
            self.vessel_electricity_max = s.electricity_max;
            self.vessel_thrust_kn = s.thrust_kn;
            self.vessel_drag_kn = s.drag_kn;
            self.vessel_delta_v = s.delta_v;
            self.vessel_current_stage = s.current_stage;
            self.vessel_total_stages = s.total_stages;
            self.vessel_stages = s.stages.clone().unwrap_or_default();
            self.vessel_stage_delta_vs = s.stage_delta_vs.clone().unwrap_or_default();
            self.vessel_stage_burn_times = s.stage_burn_times.clone().unwrap_or_default();
            self.ship_soi_surface_gravity = s.soi_surface_gravity;
            self.ship_g_force = s.g_force;
            self.ship_temperature = s.temperature;
            self.ship_heat_fraction = s.heat_fraction;
            self.ship_heat_flux = s.heat_flux;
            self.thermal_pool_temp = s.thermal_pool_temp;
            self.thermal_pool_gen_w = s.thermal_pool_gen_w;
            self.thermal_pool_reject_w = s.thermal_pool_reject_w;
            self.reactors_tripped = s.reactors_tripped;
            self.vessel_food_days = s.food_days;
            self.vessel_total_crew = s.total_crew;
            self.vessel_is_starving = s.is_starving;
            self.ship_below_landing_altitude = s.below_landing_altitude;
            self.ship_velocity_direction = s.velocity_direction;
            self.ship_speed_fraction_c = s.speed_fraction_c;
            self.ship_lorentz_gamma = s.lorentz_gamma;
            self.ship_proper_time = s.proper_time;
            self.ship_mission_time = s.mission_time;
            self.ship_is_relativistic = s.is_relativistic;
            self.ship_grav_time_factor = s.grav_time_factor;
            self.ship_orbits_root = s.orbits_root;
            self.ship_has_control = s.has_control;
            self.ship_render_x = s.x;
            self.ship_render_y = s.y;
            self.ship_render_rotation = s.rotation;
            self.ship_render_scale = if s.size > 0.0 { scale } else { 1.0 };
            if let Some(ref parts) = s.parts {
                self.flight_parts_cache = parts.clone();
            } else {
                self.flight_parts_cache.clear();
            }
            // Store trajectory for orbit click detection
            self.current_trajectory = s.patched_trajectory.clone();
            // Update predicted orbits with new trajectory data
        } else {
            self.current_trajectory.clear();
            self.predicted_trajectories.clear();
        }

        let mut all_vertices = Vec::new();
        let mut all_indices = Vec::new();

        let cam_x = self.camera.body_center[0];
        let cam_y = self.camera.body_center[1];
        let off_x = self.camera.ship_offset[0];
        let off_y = self.camera.ship_offset[1];

        // Draw atmosphere behind everything
        self.add_atmosphere_vertices(&mut all_vertices, &mut all_indices, bodies, scale);

        // Draw accretion discs (behind orbits and bodies)
        self.add_accretion_disc_vertices(&mut all_vertices, &mut all_indices, bodies, accretion_discs, scale);

        // Draw galaxy star field dots (behind orbits and bodies, on top of accretion disc)
        self.add_galaxy_texture_quad(&mut all_vertices, &mut all_indices, in_galaxy_view, scale);

        // Draw procedural stars (on top of galaxy texture, behind sector grid)
        self.current_procedural_stars = procedural_stars.to_vec();
        // Clear hovered_star since screen positions are rebuilt each frame.
        self.hovered_star = None;
        // Re-resolve focused star by sector ID in the current star list
        if let Some((sx, sy, si)) = self.focused_star_id {
            if let Some(new_idx) = self.current_procedural_stars.iter()
                .position(|s| s.matches_id(sx, sy, si))
            {
                self.focused_star = Some(new_idx);
                let star = &self.current_procedural_stars[new_idx];
                self.focused_star_world_pos = Some([star.x, star.y]);
            } else {
                // Star not in current visible set — clear index but keep world pos for tracking.
                // This prevents the info panel from showing a wrong star.
                self.focused_star = None;
            }
        }
        Self::add_procedural_stars_impl(
            &self.camera, self.size, &mut self.procedural_star_screen_positions,
            &mut all_vertices, &mut all_indices, &self.current_procedural_stars, scale,
            self.focused_star,
        );

        // Near Sgr A* (within 1000 ly), show galactic orbits for all catalog stars
        // and the focused star (even if procedural). Only when not in galaxy view
        // and the star's dot is sub-pixel — orbits disappear as you zoom in.
        if !in_galaxy_view {
            const SGR_A_RADIUS_M: f64 = 1000.0 * 9.461e15; // 1000 ly

            for (i, star) in self.current_procedural_stars.iter().enumerate() {
                let dist_from_sgr_a = (star.x * star.x + star.y * star.y).sqrt();
                let near_sgr_a = dist_from_sgr_a < SGR_A_RADIUS_M;
                let is_focused = Some(i) == self.focused_star;

                // Within 1000 ly: catalog stars + focused star
                // Outside: focused star only
                if near_sgr_a {
                    if !is_focused && star.catalog_index == 0 { continue; }
                } else {
                    if !is_focused { continue; }
                }

                // Sub-pixel check using physical stellar radius (matches the
                // hexagon-vs-circle transition in add_procedural_stars_impl).
                // Orbit disappears when you zoom in close enough that the star's
                // physical disk exceeds 1 pixel.
                let radius_px = star.radius_m * scale
                    * self.camera.zoom as f64 * self.size.height as f64 * 0.5;
                if radius_px >= 1.0 { continue; }

                Self::add_galactic_orbit_line(
                    &self.camera, &mut all_vertices, &mut all_indices,
                    star, scale, self.size.height as f32,
                );
            }
        }

        // Draw all orbit lines (on top of atmosphere, behind bodies)
        for orbit_opt in orbits {
            if let Some(orbit) = orbit_opt {
                let base_index = all_vertices.len() as u32;

                let a = orbit.semi_major_axis;
                let e = orbit.eccentricity;
                let b = a * (1.0 - e * e).sqrt();
                let c = a * e;

                let arg_peri = orbit.argument_of_periapsis;
                // Subtract camera from parent first for precision — both are galaxy-scale,
                // their difference is solar-system-scale, so orbit geometry stays precise.
                let pcam_x = orbit.parent_x - cam_x - off_x;
                let pcam_y = orbit.parent_y - cam_y - off_y;
                let center_x = pcam_x - c * arg_peri.cos();
                let center_y = pcam_y - c * arg_peri.sin();

                let segments = orbit_segments(a, self.camera.zoom, self.size.height as f32);
                let line_width = 0.002 / self.camera.zoom as f64;

                for i in 0..segments {
                    let angle = (i as f64 / segments as f64) * std::f64::consts::TAU;

                    let ex = a * angle.cos();
                    let ey = b * angle.sin();

                    let rx = ex * arg_peri.cos() - ey * arg_peri.sin();
                    let ry = ex * arg_peri.sin() + ey * arg_peri.cos();

                    let px = center_x + rx;
                    let py = center_y + ry;

                    let next_angle = ((i + 1) as f64 / segments as f64) * std::f64::consts::TAU;
                    let next_ex = a * next_angle.cos();
                    let next_ey = b * next_angle.sin();
                    let next_rx = next_ex * arg_peri.cos() - next_ey * arg_peri.sin();
                    let next_ry = next_ex * arg_peri.sin() + next_ey * arg_peri.cos();

                    let dx = next_rx - rx;
                    let dy = next_ry - ry;
                    let len = (dx * dx + dy * dy).sqrt();
                    let nx = -dy / len * line_width;
                    let ny = dx / len * line_width;

                    let rel_outer_x = (px + nx) as f32;
                    let rel_outer_y = (py + ny) as f32;
                    all_vertices.push(Vertex::new([rel_outer_x, rel_outer_y], orbit.color));

                    let rel_inner_x = (px - nx) as f32;
                    let rel_inner_y = (py - ny) as f32;
                    all_vertices.push(Vertex::new([rel_inner_x, rel_inner_y], [orbit.color[0] * 0.5, orbit.color[1] * 0.5, orbit.color[2] * 0.5, orbit.color[3] * 0.7]));
                }

                for i in 0..segments {
                    let i0 = base_index + i * 2;
                    let i1 = base_index + i * 2 + 1;
                    let i2 = base_index + ((i + 1) % segments) * 2;
                    let i3 = base_index + ((i + 1) % segments) * 2 + 1;

                    all_indices.push(i0);
                    all_indices.push(i2);
                    all_indices.push(i1);

                    all_indices.push(i1);
                    all_indices.push(i2);
                    all_indices.push(i3);
                }
            }
        }

        // Draw ship orbit line (patched conics) on top of celestial orbits but BELOW bodies
        // Only show orbit line when ship is small on screen (< 5 pixels)
        self.ap_markers.clear();
        self.pe_markers.clear();
        self.closest_approach_marker = None;

        if let Some(ship_data) = ship {
            let pixels_per_world_unit = self.camera.zoom * self.size.height as f32 / 2.0;
            let ship_pixels = ship_data.size as f32 * pixels_per_world_unit * 2.0;

            if ship_pixels < 5.0 && !ship_data.patched_trajectory.is_empty() {
                let line_width = 0.002 / self.camera.zoom as f64;
                let marker_radius = 0.008 / self.camera.zoom as f64;
                let marker_segments = 16u32;

                // Draw each patched conic segment
                for segment in &ship_data.patched_trajectory {
                    let e = segment.eccentricity;
                    let arg_peri = segment.argument_of_periapsis;

                    if e >= 1.0 {
                        // Hyperbolic trajectory - draw from ship position to SOI exit
                        let a_abs = segment.semi_major_axis.abs();

                        // Subtract camera from parent first for orbit precision
                        let pcam_x = segment.parent_x - cam_x - off_x;
                        let pcam_y = segment.parent_y - cam_y - off_y;

                        // Semi-latus rectum: p = |a| * (e² - 1)
                        let p = a_abs * (e * e - 1.0);

                        // True anomaly is limited: |ν| < arccos(-1/e)
                        let max_true_anomaly = (-1.0 / e).acos();

                        // Start from ship's current true anomaly
                        let start_ta = segment.start_true_anomaly;

                        // Calculate SOI exit true anomaly if not provided
                        let end_ta = segment.end_true_anomaly.unwrap_or_else(|| {
                            // Calculate true anomaly at SOI exit: r = p / (1 + e*cos(ν))
                            // Solving: cos(ν) = (p / soi_radius - 1) / e
                            let soi_radius = segment.soi_radius;
                            if soi_radius > 0.0 && soi_radius.is_finite() {
                                let cos_nu_exit = (p / soi_radius - 1.0) / e;
                                if cos_nu_exit.abs() <= 1.0 {
                                    let nu_exit = cos_nu_exit.acos();
                                    // Choose exit direction based on orbit direction
                                    // Prograde: exit on outgoing leg (positive ta)
                                    // Retrograde: exit on incoming leg (negative ta)
                                    if segment.retrograde { -nu_exit } else { nu_exit }
                                } else {
                                    // Fallback to asymptote limit
                                    if segment.retrograde { -(max_true_anomaly - HYPERBOLIC_RENDER_MARGIN) } else { max_true_anomaly - HYPERBOLIC_RENDER_MARGIN }
                                }
                            } else {
                                // Fallback to asymptote limit
                                if segment.retrograde { -(max_true_anomaly - HYPERBOLIC_RENDER_MARGIN) } else { max_true_anomaly - HYPERBOLIC_RENDER_MARGIN }
                            }
                        });

                        // Generate points along the hyperbola
                        let num_points = 1024usize;
                        let mut points: Vec<(f64, f64)> = Vec::with_capacity(num_points);

                        for i in 0..num_points {
                            let t = i as f64 / (num_points - 1) as f64;
                            let ta = start_ta + t * (end_ta - start_ta);

                            // Calculate radius from orbit equation: r = p / (1 + e*cos(ν))
                            let denom = 1.0 + e * ta.cos();
                            if denom <= 0.001 {
                                continue; // Near asymptote
                            }
                            let r = p / denom;

                            // Skip invalid radii
                            if r <= 0.0 || !r.is_finite() {
                                continue;
                            }

                            // Position relative to camera (focus at parent, camera-relative)
                            let angle = ta + arg_peri;
                            let px = pcam_x + r * angle.cos();
                            let py = pcam_y + r * angle.sin();

                            points.push((px, py));
                        }

                        // Draw line segments between consecutive points
                        if points.len() >= 2 {
                            let base_index = all_vertices.len() as u32;

                            for i in 0..points.len() - 1 {
                                let (px, py) = points[i];
                                let (nx_pt, ny_pt) = points[i + 1];

                                let dx = nx_pt - px;
                                let dy = ny_pt - py;
                                let len = (dx * dx + dy * dy).sqrt();
                                if len < 1e-10 {
                                    continue;
                                }

                                // Perpendicular for line width
                                let nx = -dy / len * line_width;
                                let ny = dx / len * line_width;

                                let rel_outer_x = (px + nx) as f32;
                                let rel_outer_y = (py + ny) as f32;
                                all_vertices.push(Vertex::new([rel_outer_x, rel_outer_y], segment.color));

                                let rel_inner_x = (px - nx) as f32;
                                let rel_inner_y = (py - ny) as f32;
                                all_vertices.push(Vertex::new([rel_inner_x, rel_inner_y], [segment.color[0] * 0.5, segment.color[1] * 0.5, segment.color[2] * 0.5, segment.color[3] * 0.7]));
                            }

                            let num_line_segments = (all_vertices.len() as u32 - base_index) / 2;
                            for i in 0..num_line_segments.saturating_sub(1) {
                                let i0 = base_index + i * 2;
                                let i1 = base_index + i * 2 + 1;
                                let i2 = base_index + (i + 1) * 2;
                                let i3 = base_index + (i + 1) * 2 + 1;

                                all_indices.push(i0);
                                all_indices.push(i2);
                                all_indices.push(i1);

                                all_indices.push(i1);
                                all_indices.push(i2);
                                all_indices.push(i3);
                            }
                        }

                        // Check if periapsis (true anomaly = 0) will be reached
                        // For hyperbolic orbits, the ship travels monotonically from start_ta to end_ta
                        // Prograde: ta increases, so Pe is reached if start_ta <= 0 <= end_ta
                        // Retrograde: ta decreases, so Pe is reached if start_ta >= 0 >= end_ta
                        let pe_will_be_reached = if segment.retrograde {
                            start_ta >= 0.0 && end_ta <= 0.0
                        } else {
                            start_ta <= 0.0 && end_ta >= 0.0
                        };

                        // Only draw periapsis marker if it will be reached
                        if pe_will_be_reached {
                            // Draw periapsis marker for hyperbolic trajectory
                            // Periapsis is at true anomaly = 0
                            let a_abs = segment.semi_major_axis.abs();
                            let p = a_abs * (e * e - 1.0);
                            let pe_r = p / (1.0 + e); // Distance at periapsis
                            let pe_x = pcam_x + pe_r * arg_peri.cos();
                            let pe_y = pcam_y + pe_r * arg_peri.sin();

                            // Use dimmer color for future segments
                            let alpha = if segment.is_first_segment { 1.0 } else { 0.6 };
                            let pe_color = [0.3, 0.8, 1.0, alpha];

                            let pe_base = all_vertices.len() as u32;
                            all_vertices.push(Vertex::new([pe_x as f32, pe_y as f32], pe_color));
                            for i in 0..marker_segments {
                                let angle = (i as f64 / marker_segments as f64) * std::f64::consts::TAU;
                                all_vertices.push(Vertex::new([
                                        (pe_x + marker_radius * angle.cos()) as f32,
                                        (pe_y + marker_radius * angle.sin()) as f32,
                                    ], pe_color));
                            }
                            for i in 0..marker_segments {
                                all_indices.push(pe_base);
                                all_indices.push(pe_base + 1 + i);
                                all_indices.push(pe_base + 1 + (i + 1) % marker_segments);
                            }

                            // Store position and altitude for UI hover
                            // pe_r is in scaled units, convert to meters then subtract body radius
                            let pe_altitude = (pe_r / segment.render_scale) - segment.parent_body_radius;
                            self.pe_markers.push(([pe_x, pe_y], pe_altitude));
                        }

                        continue; // Skip ellipse drawing code
                    }

                    // Elliptical orbit — subtract camera from parent first for precision
                    let a = segment.semi_major_axis;
                    let b = a * (1.0 - e * e).sqrt();
                    let c = a * e;

                    let pcam_x = segment.parent_x - cam_x - off_x;
                    let pcam_y = segment.parent_y - cam_y - off_y;
                    let center_x = pcam_x - c * arg_peri.cos();
                    let center_y = pcam_y - c * arg_peri.sin();

                    // Determine angle range to draw
                    let (start_angle, angle_span) = match segment.end_true_anomaly {
                        Some(end_ta) => {
                            // Partial orbit - convert true anomaly to eccentric anomaly
                            let start_ta = segment.start_true_anomaly;
                            let start_ea = (start_ta.sin() * (1.0 - e * e).sqrt()).atan2(e + start_ta.cos());
                            let end_ea = (end_ta.sin() * (1.0 - e * e).sqrt()).atan2(e + end_ta.cos());

                            // Calculate span based on orbit direction
                            let span = if segment.retrograde {
                                // Retrograde: going from start toward end in decreasing direction
                                let mut s = start_ea - end_ea;
                                if s < 0.0 {
                                    s += std::f64::consts::TAU;
                                }
                                -s // Negative span for retrograde (draw clockwise)
                            } else {
                                // Prograde: going from start toward end in increasing direction
                                let mut s = end_ea - start_ea;
                                if s < 0.0 {
                                    s += std::f64::consts::TAU;
                                }
                                s
                            };
                            (start_ea, span)
                        }
                        None => {
                            // Full orbit - but start from the ship's entry point
                            // Convert start_true_anomaly to eccentric anomaly
                            let start_ta = segment.start_true_anomaly;
                            let start_ea = (start_ta.sin() * (1.0 - e * e).sqrt()).atan2(e + start_ta.cos());
                            (start_ea, std::f64::consts::TAU)
                        }
                    };

                    let is_full_orbit = segment.end_true_anomaly.is_none();
                    let full_segments = orbit_segments(a, self.camera.zoom, self.size.height as f32);
                    let num_segments = ((angle_span.abs() / std::f64::consts::TAU) * full_segments as f64).max(16.0) as u32;
                    let base_index = all_vertices.len() as u32;

                    for i in 0..num_segments {
                        let t = i as f64 / num_segments as f64;
                        let angle = start_angle + t * angle_span;

                        let ex = a * angle.cos();
                        let ey = b * angle.sin();

                        let rx = ex * arg_peri.cos() - ey * arg_peri.sin();
                        let ry = ex * arg_peri.sin() + ey * arg_peri.cos();

                        let px = center_x + rx;
                        let py = center_y + ry;

                        let next_t = (i + 1) as f64 / num_segments as f64;
                        let next_angle = start_angle + next_t * angle_span;
                        let next_ex = a * next_angle.cos();
                        let next_ey = b * next_angle.sin();
                        let next_rx = next_ex * arg_peri.cos() - next_ey * arg_peri.sin();
                        let next_ry = next_ex * arg_peri.sin() + next_ey * arg_peri.cos();

                        let dx = next_rx - rx;
                        let dy = next_ry - ry;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len < 1e-10 {
                            continue;
                        }
                        let nx = -dy / len * line_width;
                        let ny = dx / len * line_width;

                        let rel_outer_x = (px + nx) as f32;
                        let rel_outer_y = (py + ny) as f32;
                        all_vertices.push(Vertex::new([rel_outer_x, rel_outer_y], segment.color));

                        let rel_inner_x = (px - nx) as f32;
                        let rel_inner_y = (py - ny) as f32;
                        all_vertices.push(Vertex::new([rel_inner_x, rel_inner_y], [segment.color[0] * 0.5, segment.color[1] * 0.5, segment.color[2] * 0.5, segment.color[3] * 0.7]));
                    }

                    // For full orbits, wrap around to connect last segment to first
                    // For partial orbits, only connect consecutive segments
                    let index_count = if is_full_orbit { num_segments } else { num_segments.saturating_sub(1) };
                    for i in 0..index_count {
                        let i0 = base_index + i * 2;
                        let i1 = base_index + i * 2 + 1;
                        let next_i = if is_full_orbit { (i + 1) % num_segments } else { i + 1 };
                        let i2 = base_index + next_i * 2;
                        let i3 = base_index + next_i * 2 + 1;

                        all_indices.push(i0);
                        all_indices.push(i2);
                        all_indices.push(i1);

                        all_indices.push(i1);
                        all_indices.push(i2);
                        all_indices.push(i3);
                    }

                    // Draw Ap/Pe markers for all segments (dimmer for future segments)
                    // For first segment with intercept, only show markers if they're in the traversed arc
                    let alpha = if segment.is_first_segment { 1.0 } else { 0.6 };

                    // Helper to check if a true anomaly is in the arc from start to end
                    let is_in_arc = |marker_ta: f64, start_ta: f64, end_ta: f64, retrograde: bool| -> bool {
                        let tau = std::f64::consts::TAU;
                        let normalize = |a: f64| a.rem_euclid(tau);
                        let marker = normalize(marker_ta);
                        let start = normalize(start_ta);
                        let end = normalize(end_ta);

                        if retrograde {
                            // Moving in decreasing direction
                            if start >= end {
                                marker <= start && marker >= end
                            } else {
                                marker >= end || marker <= start
                            }
                        } else {
                            // Moving in increasing direction
                            if start <= end {
                                marker >= start && marker <= end
                            } else {
                                marker >= start || marker <= end
                            }
                        }
                    };

                    // Calculate orbital distances for altitude (convert from scaled to meters)
                    let pe_distance = a * (1.0 - e) / segment.render_scale;
                    let ap_distance = a * (1.0 + e) / segment.render_scale;
                    let pe_altitude = pe_distance - segment.parent_body_radius;
                    let ap_altitude = ap_distance - segment.parent_body_radius;

                    // Check if markers are in traversed arc (for first segment with intercept)
                    let (show_pe, show_ap) = if segment.is_first_segment {
                        if let Some(end_ta) = segment.end_true_anomaly {
                            let start_ta = segment.start_true_anomaly;
                            let pe_in_arc = is_in_arc(0.0, start_ta, end_ta, segment.retrograde);
                            let ap_in_arc = is_in_arc(std::f64::consts::PI, start_ta, end_ta, segment.retrograde);
                            (pe_in_arc, ap_in_arc)
                        } else {
                            (true, true) // Full orbit, show both
                        }
                    } else {
                        (true, true) // Future segments always show markers
                    };

                    // Periapsis marker (at true anomaly 0) - cyan/blue
                    if show_pe {
                        let pe_ex = a;
                        let pe_ey = 0.0;
                        let pe_rx = pe_ex * arg_peri.cos() - pe_ey * arg_peri.sin();
                        let pe_ry = pe_ex * arg_peri.sin() + pe_ey * arg_peri.cos();
                        let pe_x = center_x + pe_rx;
                        let pe_y = center_y + pe_ry;
                        let pe_color = [0.3, 0.8, 1.0, alpha];

                        let pe_base = all_vertices.len() as u32;
                        all_vertices.push(Vertex::new([pe_x as f32, pe_y as f32], pe_color));
                        for i in 0..marker_segments {
                            let angle = (i as f64 / marker_segments as f64) * std::f64::consts::TAU;
                            all_vertices.push(Vertex::new([
                                    (pe_x + marker_radius * angle.cos()) as f32,
                                    (pe_y + marker_radius * angle.sin()) as f32,
                                ], pe_color));
                        }
                        for i in 0..marker_segments {
                            all_indices.push(pe_base);
                            all_indices.push(pe_base + 1 + i);
                            all_indices.push(pe_base + 1 + (i + 1) % marker_segments);
                        }
                        // Store for UI hover
                        self.pe_markers.push(([pe_x, pe_y], pe_altitude));
                    }

                    // Apoapsis marker (at true anomaly π) - orange
                    if show_ap {
                        let ap_ex = -a;
                        let ap_ey = 0.0;
                        let ap_rx = ap_ex * arg_peri.cos() - ap_ey * arg_peri.sin();
                        let ap_ry = ap_ex * arg_peri.sin() + ap_ey * arg_peri.cos();
                        let ap_x = center_x + ap_rx;
                        let ap_y = center_y + ap_ry;
                        let ap_color = [1.0, 0.6, 0.2, alpha];

                        let ap_base = all_vertices.len() as u32;
                        all_vertices.push(Vertex::new([ap_x as f32, ap_y as f32], ap_color));
                        for i in 0..marker_segments {
                            let angle = (i as f64 / marker_segments as f64) * std::f64::consts::TAU;
                            all_vertices.push(Vertex::new([
                                    (ap_x + marker_radius * angle.cos()) as f32,
                                    (ap_y + marker_radius * angle.sin()) as f32,
                                ], ap_color));
                        }
                        for i in 0..marker_segments {
                            all_indices.push(ap_base);
                            all_indices.push(ap_base + 1 + i);
                            all_indices.push(ap_base + 1 + (i + 1) % marker_segments);
                        }
                        // Store for UI hover
                        self.ap_markers.push(([ap_x, ap_y], ap_altitude));
                    }
                }

                // Draw closest approach marker (yellow dot)
                if let Some((parent_pos, orbit_off, dist)) = self.closest_approach_world_pos {
                    // Two-step precision: (parent - body_center) + (orbit_offset - ship_offset)
                    let ca_x = (parent_pos[0] - cam_x) + (orbit_off[0] - off_x);
                    let ca_y = (parent_pos[1] - cam_y) + (orbit_off[1] - off_y);
                    let ca_color = [1.0, 1.0, 0.0, 0.9_f32];

                    let ca_base = all_vertices.len() as u32;
                    all_vertices.push(Vertex::new([ca_x as f32, ca_y as f32], ca_color));
                    for i in 0..marker_segments {
                        let angle = (i as f64 / marker_segments as f64) * std::f64::consts::TAU;
                        all_vertices.push(Vertex::new([
                            (ca_x + marker_radius * angle.cos()) as f32,
                            (ca_y + marker_radius * angle.sin()) as f32,
                        ], ca_color));
                    }
                    for i in 0..marker_segments {
                        all_indices.push(ca_base);
                        all_indices.push(ca_base + 1 + i);
                        all_indices.push(ca_base + 1 + (i + 1) % marker_segments);
                    }
                    self.closest_approach_marker = Some(([ca_x, ca_y], dist));
                }

                // Draw target closest approach marker (yellow dot at target's position)
                if let Some((parent_pos, orbit_off, dist)) = self.target_closest_approach_world_pos {
                    // Two-step precision: (parent - body_center) + (orbit_offset - ship_offset)
                    let tca_x = (parent_pos[0] - cam_x) + (orbit_off[0] - off_x);
                    let tca_y = (parent_pos[1] - cam_y) + (orbit_off[1] - off_y);
                    let tca_color = [1.0, 1.0, 0.0, 0.9_f32];

                    let tca_base = all_vertices.len() as u32;
                    all_vertices.push(Vertex::new([tca_x as f32, tca_y as f32], tca_color));
                    for i in 0..marker_segments {
                        let angle = (i as f64 / marker_segments as f64) * std::f64::consts::TAU;
                        all_vertices.push(Vertex::new([
                            (tca_x + marker_radius * angle.cos()) as f32,
                            (tca_y + marker_radius * angle.sin()) as f32,
                        ], tca_color));
                    }
                    for i in 0..marker_segments {
                        all_indices.push(tca_base);
                        all_indices.push(tca_base + 1 + i);
                        all_indices.push(tca_base + 1 + (i + 1) % marker_segments);
                    }
                    self.target_closest_approach_marker = Some(([tca_x, tca_y], dist));
                }
            }
        }

        // Draw predicted trajectories as solid green lines
        for trajectory in &self.predicted_trajectories {
            for (seg_idx, segment) in trajectory.iter().enumerate() {
                let e = segment.eccentricity;
                let arg_peri = segment.argument_of_periapsis;
                let a = segment.semi_major_axis;

                let line_width = 0.0015 / self.camera.zoom as f64;

                // Green for first segment, dimmer for subsequent segments
                let alpha = if seg_idx == 0 { 0.9 } else { 0.6 };
                let seg_color = [0.0, 1.0, 0.0, alpha];

                // Subtract camera from parent first for precision — both are galaxy-scale,
                // their difference is solar-system-scale, so orbit geometry stays precise.
                let pcam_x = segment.parent_x - cam_x - off_x;
                let pcam_y = segment.parent_y - cam_y - off_y;

                if e >= 1.0 {
                    // Hyperbolic orbit segment
                    let a_abs = a.abs();
                    let p = a_abs * (e * e - 1.0);
                    let max_ta = (-1.0 / e).acos();

                    let start_ta = segment.start_true_anomaly;
                    let end_ta = segment.end_true_anomaly.unwrap_or_else(|| {
                        if segment.retrograde {
                            -(max_ta - HYPERBOLIC_RENDER_MARGIN)
                        } else {
                            max_ta - HYPERBOLIC_RENDER_MARGIN
                        }
                    });

                    let num_points = orbit_segments(a_abs, self.camera.zoom, self.size.height as f32) as usize;
                    let mut points: Vec<(f64, f64)> = Vec::with_capacity(num_points);

                    for i in 0..num_points {
                        let t = i as f64 / (num_points - 1) as f64;
                        let ta = start_ta + t * (end_ta - start_ta);

                        let denom = 1.0 + e * ta.cos();
                        if denom <= 0.001 {
                            continue;
                        }
                        let r = p / denom;
                        if r <= 0.0 || !r.is_finite() {
                            continue;
                        }

                        let angle = ta + arg_peri;
                        let px = pcam_x + r * angle.cos();
                        let py = pcam_y + r * angle.sin();
                        points.push((px, py));
                    }

                    // Draw solid line segments
                    for i in 0..points.len().saturating_sub(1) {
                        let (px, py) = points[i];
                        let (nx, ny) = points[i + 1];

                        let dx = nx - px;
                        let dy = ny - py;
                        let seg_len = (dx * dx + dy * dy).sqrt();

                        if seg_len < 1e-10 {
                            continue;
                        }

                        let base_index = all_vertices.len() as u32;
                        let len = seg_len;
                        let nx_perp = -dy / len * line_width;
                        let ny_perp = dx / len * line_width;

                        all_vertices.push(Vertex::new([(px + nx_perp) as f32, (py + ny_perp) as f32], seg_color));
                        all_vertices.push(Vertex::new([(px - nx_perp) as f32, (py - ny_perp) as f32], seg_color));
                        all_vertices.push(Vertex::new([(nx + nx_perp) as f32, (ny + ny_perp) as f32], seg_color));
                        all_vertices.push(Vertex::new([(nx - nx_perp) as f32, (ny - ny_perp) as f32], seg_color));

                        all_indices.push(base_index);
                        all_indices.push(base_index + 2);
                        all_indices.push(base_index + 1);
                        all_indices.push(base_index + 1);
                        all_indices.push(base_index + 2);
                        all_indices.push(base_index + 3);
                    }

                    // Draw periapsis marker for hyperbolic (if we'll reach it)
                    let start_ta_norm = start_ta;
                    let end_ta_norm = end_ta;
                    let pe_will_be_reached = if segment.retrograde {
                        start_ta_norm >= 0.0 && end_ta_norm <= 0.0
                    } else {
                        start_ta_norm <= 0.0 && end_ta_norm >= 0.0
                    };

                    if pe_will_be_reached {
                        let pe_r = p / (1.0 + e);
                        let pe_x = pcam_x + pe_r * arg_peri.cos();
                        let pe_y = pcam_y + pe_r * arg_peri.sin();
                        let marker_radius = 0.006 / self.camera.zoom as f64;
                        let marker_segments = 12u32;
                        let marker_alpha = if seg_idx == 0 { 0.7f32 } else { 0.5f32 };
                        let pe_color = [0.2, 0.7, 0.9, marker_alpha];

                        let pe_base = all_vertices.len() as u32;
                        all_vertices.push(Vertex::new([pe_x as f32, pe_y as f32], pe_color));
                        for j in 0..marker_segments {
                            let angle = (j as f64 / marker_segments as f64) * std::f64::consts::TAU;
                            all_vertices.push(Vertex::new([
                                    (pe_x + marker_radius * angle.cos()) as f32,
                                    (pe_y + marker_radius * angle.sin()) as f32,
                                ], pe_color));
                        }
                        for j in 0..marker_segments {
                            all_indices.push(pe_base);
                            all_indices.push(pe_base + 1 + j);
                            all_indices.push(pe_base + 1 + (j + 1) % marker_segments);
                        }

                        // Store for UI hover display
                        let pe_distance = pe_r / segment.render_scale;
                        let pe_altitude = pe_distance - segment.parent_body_radius;
                        self.pe_markers.push(([pe_x, pe_y], pe_altitude));
                    }
                } else {
                    // Elliptical orbit segment
                    let b = a * (1.0 - e * e).sqrt();
                    let c = a * e;
                    let center_x = pcam_x - c * arg_peri.cos();
                    let center_y = pcam_y - c * arg_peri.sin();

                    let start_ta = segment.start_true_anomaly;
                    let start_ea = (start_ta.sin() * (1.0 - e * e).sqrt()).atan2(e + start_ta.cos());

                    // Calculate angle span
                    let angle_span = match segment.end_true_anomaly {
                        Some(end_ta) => {
                            let end_ea = (end_ta.sin() * (1.0 - e * e).sqrt()).atan2(e + end_ta.cos());
                            if segment.retrograde {
                                let mut s = start_ea - end_ea;
                                if s < 0.0 { s += std::f64::consts::TAU; }
                                -s
                            } else {
                                let mut s = end_ea - start_ea;
                                if s < 0.0 { s += std::f64::consts::TAU; }
                                s
                            }
                        }
                        None => std::f64::consts::TAU,
                    };

                    let num_segments_draw = orbit_segments(a, self.camera.zoom, self.size.height as f32);
                    let mut prev_point: Option<(f64, f64)> = None;

                    for i in 0..=num_segments_draw {
                        let t = i as f64 / num_segments_draw as f64;
                        let ea = start_ea + t * angle_span;

                        let ex = a * ea.cos();
                        let ey = b * ea.sin();
                        let rx = ex * arg_peri.cos() - ey * arg_peri.sin();
                        let ry = ex * arg_peri.sin() + ey * arg_peri.cos();
                        let px = center_x + rx;
                        let py = center_y + ry;

                        if let Some((prev_x, prev_y)) = prev_point {
                            let dx = px - prev_x;
                            let dy = py - prev_y;
                            let seg_len = (dx * dx + dy * dy).sqrt();

                            if seg_len >= 1e-10 {
                                let base_index = all_vertices.len() as u32;
                                let len = seg_len;
                                let nx_perp = -dy / len * line_width;
                                let ny_perp = dx / len * line_width;

                                all_vertices.push(Vertex::new([(prev_x + nx_perp) as f32, (prev_y + ny_perp) as f32], seg_color));
                                all_vertices.push(Vertex::new([(prev_x - nx_perp) as f32, (prev_y - ny_perp) as f32], seg_color));
                                all_vertices.push(Vertex::new([(px + nx_perp) as f32, (py + ny_perp) as f32], seg_color));
                                all_vertices.push(Vertex::new([(px - nx_perp) as f32, (py - ny_perp) as f32], seg_color));

                                all_indices.push(base_index);
                                all_indices.push(base_index + 2);
                                all_indices.push(base_index + 1);
                                all_indices.push(base_index + 1);
                                all_indices.push(base_index + 2);
                                all_indices.push(base_index + 3);
                            }
                        }

                        prev_point = Some((px, py));
                    }

                    // Draw Ap/Pe markers for all segments of predicted trajectories
                    let marker_radius = 0.006 / self.camera.zoom as f64;
                    let marker_segments = 12u32;
                    let marker_alpha = if seg_idx == 0 { 0.7f32 } else { 0.5f32 };

                    // Helper to check if a true anomaly is in the arc from start to end
                    let is_in_arc = |marker_ta: f64, start_ta: f64, end_ta: f64, retrograde: bool| -> bool {
                        let tau = std::f64::consts::TAU;
                        let normalize = |ang: f64| ang.rem_euclid(tau);
                        let marker = normalize(marker_ta);
                        let start = normalize(start_ta);
                        let end = normalize(end_ta);

                        if retrograde {
                            if start >= end { marker <= start && marker >= end }
                            else { marker >= end || marker <= start }
                        } else {
                            if start <= end { marker >= start && marker <= end }
                            else { marker >= start || marker <= end }
                        }
                    };

                    // Determine which markers to show
                    let (show_pe, show_ap) = if let Some(end_ta) = segment.end_true_anomaly {
                        let start_ta = segment.start_true_anomaly;
                        let pe_in_arc = is_in_arc(0.0, start_ta, end_ta, segment.retrograde);
                        let ap_in_arc = is_in_arc(std::f64::consts::PI, start_ta, end_ta, segment.retrograde);
                        (pe_in_arc, ap_in_arc)
                    } else {
                        (true, true) // Full orbit, show both
                    };

                    // Periapsis (ta = 0) - cyan
                    if show_pe {
                        let pe_r = a * (1.0 - e);
                        let pe_x = pcam_x + pe_r * arg_peri.cos();
                        let pe_y = pcam_y + pe_r * arg_peri.sin();
                        let pe_color = [0.2, 0.7, 0.9, marker_alpha];

                        let pe_base = all_vertices.len() as u32;
                        all_vertices.push(Vertex::new([pe_x as f32, pe_y as f32], pe_color));
                        for j in 0..marker_segments {
                            let angle = (j as f64 / marker_segments as f64) * std::f64::consts::TAU;
                            all_vertices.push(Vertex::new([
                                    (pe_x + marker_radius * angle.cos()) as f32,
                                    (pe_y + marker_radius * angle.sin()) as f32,
                                ], pe_color));
                        }
                        for j in 0..marker_segments {
                            all_indices.push(pe_base);
                            all_indices.push(pe_base + 1 + j);
                            all_indices.push(pe_base + 1 + (j + 1) % marker_segments);
                        }

                        // Store for UI hover display
                        let pe_distance = a * (1.0 - e) / segment.render_scale;
                        let pe_altitude = pe_distance - segment.parent_body_radius;
                        self.pe_markers.push(([pe_x, pe_y], pe_altitude));
                    }

                    // Apoapsis (ta = π) - orange
                    if show_ap {
                        let ap_r = a * (1.0 + e);
                        let ap_angle = arg_peri + std::f64::consts::PI;
                        let ap_x = pcam_x + ap_r * ap_angle.cos();
                        let ap_y = pcam_y + ap_r * ap_angle.sin();
                        let ap_color = [0.9, 0.5, 0.1, marker_alpha];

                        let ap_base = all_vertices.len() as u32;
                        all_vertices.push(Vertex::new([ap_x as f32, ap_y as f32], ap_color));
                        for j in 0..marker_segments {
                            let angle = (j as f64 / marker_segments as f64) * std::f64::consts::TAU;
                            all_vertices.push(Vertex::new([
                                    (ap_x + marker_radius * angle.cos()) as f32,
                                    (ap_y + marker_radius * angle.sin()) as f32,
                                ], ap_color));
                        }
                        for j in 0..marker_segments {
                            all_indices.push(ap_base);
                            all_indices.push(ap_base + 1 + j);
                            all_indices.push(ap_base + 1 + (j + 1) % marker_segments);
                        }

                        // Store for UI hover display
                        let ap_distance = a * (1.0 + e) / segment.render_scale;
                        let ap_altitude = ap_distance - segment.parent_body_radius;
                        self.ap_markers.push(([ap_x, ap_y], ap_altitude));
                    }
                }
            }
        }

        // Draw bodies on top of orbit lines
        self.add_body_vertices(&mut all_vertices, &mut all_indices, bodies, scale);

        // Draw launchpad on body surface (ship view only, not galaxy view)
        if let Some(ship_data) = ship.filter(|_| !in_galaxy_view) {
            self.add_launchpad_vertices(&mut all_vertices, &mut all_indices, bodies, scale, ship_data, self.earth_index);
        }

        // Draw ship on top of everything
        if let Some(ship_data) = ship {
            // Ship position relative to camera, using two-step subtraction for precision.
            // Each subtraction is between values of similar magnitude, preserving f64 precision.
            let rel_x = ((self.ship_body_center[0] - cam_x) + (self.ship_rel_offset[0] - off_x)) as f32;
            let rel_y = ((self.ship_body_center[1] - cam_y) + (self.ship_rel_offset[1] - off_y)) as f32;
            let size = ship_data.size as f32;
            let rotation = ship_data.rotation as f32;

            // Calculate ship size in pixels
            let pixels_per_world_unit = self.camera.zoom * self.size.height as f32 / 2.0;
            let ship_pixels = size * pixels_per_world_unit * 2.0;
            let needs_indicator = ship_pixels < 5.0;

            // Draw the actual ship (parts or triangle) if visible
            if ship_pixels >= 1.0 {
                let has_parts = ship_data.parts.is_some() && part_defs.is_some();

                if has_parts {
                    // Part-based rendering: render each part at its position
                    // Offset by -π/2 because editor parts are Y-up but rotation=0 means +X
                    let visual_rotation = rotation - std::f32::consts::FRAC_PI_2;
                    let parts = ship_data.parts.as_ref().unwrap();
                    let defs = part_defs.unwrap();
                    let cos_r = visual_rotation.cos();
                    let sin_r = visual_rotation.sin();
                    let render_scale = scale as f32;


                    let mut part_verts: Vec<Vertex> = Vec::with_capacity(256);
                    for part_data in parts {
                        if let Some(def) = defs.get(&part_data.definition_id) {
                            // Transform part local position to world-relative position
                            let local_x = part_data.local_x as f32 * render_scale;
                            let local_y = part_data.local_y as f32 * render_scale;

                            // Rotate local position by vessel rotation
                            let rotated_x = local_x * cos_r - local_y * sin_r;
                            let rotated_y = local_x * sin_r + local_y * cos_r;

                            // Generate vertices at origin, then transform
                            // Skip base disc for fairing half debris (shell-only)
                            part_verts.clear();
                            if part_data.fairing_half.is_none() {
                                crate::editor::generate_part_shape_vertices(
                                    &mut part_verts, def, 0.0, 0.0, 1.0,
                                    Some(&self.sprite_atlas),
                                    if part_data.is_solar_panel || part_data.is_radiator { Some(part_data.deploy_fraction) } else { None },
                                );
                            }

                            // Add engine plume if this engine is firing
                            let plume_elapsed = self.plume_start_time.elapsed().as_secs_f64();
                            if part_data.engine_active && ship_data.throttle > 0.0 && def.engine.is_some() {
                                crate::editor::generate_engine_plume_vertices(
                                    &mut part_verts, def, 0.0, 0.0, ship_data.throttle as f32,
                                    Some(&self.sprite_atlas), plume_elapsed,
                                );
                            }

                            // Add RCS plumes if nozzles are active
                            if let Some(ref nozzle_state) = part_data.rcs_nozzle_state {
                                if def.rcs.is_some() {
                                    if def.category == crate::parts::PartCategory::Pods {
                                        // Pods have bilateral nozzles — use pod-specific plume function
                                        crate::editor::generate_pod_rcs_plume_vertices(
                                            &mut part_verts, def, 0.0, 0.0, nozzle_state,
                                        );
                                    } else {
                                        crate::editor::generate_rcs_plume_vertices(
                                            &mut part_verts, def, 0.0, 0.0, nozzle_state,
                                        );
                                    }
                                }
                            }

                            // Apply part rotation and gimbal rotation for engine parts, then scale
                            // and rotate each vertex by vessel rotation.
                            // PRECISION: compute vertex as rel + (local + vert) not (rel + local) + vert.
                            // The inner sum (local + vert) stays near zero with full f32 precision.
                            // Adding rel (~0.006) last ensures adjacent part boundaries that share
                            // the same mathematical position round to the same f32 value.
                            let gimbal = if def.engine.is_some() {
                                part_data.gimbal_angle as f32
                            } else {
                                0.0
                            };
                            let part_rot = part_data.rotation as f32;
                            let base_index = all_vertices.len() as u32;
                            let scale_factor = render_scale;
                            for vert in &part_verts {
                                let mut vx = vert.position[0] * scale_factor;
                                let mut vy = vert.position[1] * scale_factor;
                                // Apply part rotation in part-local space
                                if part_rot.abs() > 1e-6 {
                                    let pc = part_rot.cos();
                                    let ps = part_rot.sin();
                                    let px = vx * pc - vy * ps;
                                    let py = vx * ps + vy * pc;
                                    vx = px;
                                    vy = py;
                                }
                                // Apply gimbal rotation in part-local space
                                if gimbal.abs() > 1e-6 {
                                    let gc = gimbal.cos();
                                    let gs = gimbal.sin();
                                    let gx = vx * gc - vy * gs;
                                    let gy = vx * gs + vy * gc;
                                    vx = gx;
                                    vy = gy;
                                }
                                // Rotate around origin by vessel rotation
                                let rx = vx * cos_r - vy * sin_r;
                                let ry = vx * sin_r + vy * cos_r;
                                // Apply per-part heat tinting (blackbody glow)
                                let color = apply_heat_tint(vert.color, part_data.temperature);
                                all_vertices.push(Vertex {
                                    position: [rel_x + (rotated_x + rx), rel_y + (rotated_y + ry)],
                                    color,
                                    uv: vert.uv,  // preserve sprite UVs
                                });
                            }

                            // Part vertices are triangle lists (every 3 verts = 1 triangle)
                            let num_part_verts = part_verts.len() as u32;
                            for i in (0..num_part_verts).step_by(3) {
                                if i + 2 < num_part_verts {
                                    all_indices.push(base_index + i);
                                    all_indices.push(base_index + i + 1);
                                    all_indices.push(base_index + i + 2);
                                }
                            }
                        }
                    }

                    // Second pass: draw decoupler adapter fairings
                    for part_data in parts {
                        if let Some(decoupler_def) = defs.get(&part_data.definition_id) {
                            if decoupler_def.decoupler.is_none() {
                                continue;
                            }

                            // Generate adapter vertices at origin using the same function
                            // We need to build a temporary parts map for the adapter check
                            let dec_x = part_data.local_x as f32;
                            let dec_y = part_data.local_y as f32;
                            let mut adapter_verts: Vec<Vertex> = Vec::new();
                            crate::editor::generate_flight_decoupler_adapter(
                                &mut adapter_verts, decoupler_def,
                                dec_x, dec_y, parts, defs, 1.0,
                            );

                            if !adapter_verts.is_empty() {
                                let base_index = all_vertices.len() as u32;
                                for vert in &adapter_verts {
                                    let vx = vert.position[0] * render_scale;
                                    let vy = vert.position[1] * render_scale;
                                    let rx = vx * cos_r - vy * sin_r;
                                    let ry = vx * sin_r + vy * cos_r;
                                    all_vertices.push(Vertex::new([rel_x + rx, rel_y + ry], vert.color));
                                }
                                let num_verts = adapter_verts.len() as u32;
                                for i in (0..num_verts).step_by(3) {
                                    if i + 2 < num_verts {
                                        all_indices.push(base_index + i);
                                        all_indices.push(base_index + i + 1);
                                        all_indices.push(base_index + i + 2);
                                    }
                                }
                            }
                        }
                    }

                    // Third pass: draw fairing shells
                    for part_data in parts {
                        let Some(ref shape) = part_data.fairing_shape else { continue };
                        let Some(fairing_def) = defs.get(&part_data.definition_id) else { continue };
                        if fairing_def.fairing.is_none() { continue; }

                        let px = part_data.local_x as f32;
                        let py = part_data.local_y as f32;
                        let hitbox_half_h = part_data.hitbox_half_h as f32;
                        let base_half_w = (fairing_def.width() / 2.0) as f32;
                        let mut shell_verts: Vec<Vertex> = Vec::new();
                        crate::editor::generate_flight_fairing_shell(
                            &mut shell_verts, shape,
                            px, py, hitbox_half_h, base_half_w, 1.0,
                            part_data.fairing_half,
                        );

                        if !shell_verts.is_empty() {
                            let base_index = all_vertices.len() as u32;
                            for vert in &shell_verts {
                                let vx = vert.position[0] * render_scale;
                                let vy = vert.position[1] * render_scale;
                                let rx = vx * cos_r - vy * sin_r;
                                let ry = vx * sin_r + vy * cos_r;
                                all_vertices.push(Vertex::new([rel_x + rx, rel_y + ry], vert.color));
                            }
                            let num_verts = shell_verts.len() as u32;
                            for i in (0..num_verts).step_by(3) {
                                if i + 2 < num_verts {
                                    all_indices.push(base_index + i);
                                    all_indices.push(base_index + i + 1);
                                    all_indices.push(base_index + i + 2);
                                }
                            }
                        }
                    }

                    // Fourth pass: draw deployed parachute canopies
                    {
                        // Retrograde direction in world frame
                        let vdir = ship_data.velocity_direction;
                        let vel_mag = (vdir[0] * vdir[0] + vdir[1] * vdir[1]).sqrt();
                        let (retro_world_x, retro_world_y) = if vel_mag > 0.1 {
                            (-vdir[0] as f32, -vdir[1] as f32)
                        } else {
                            let heading = ship_data.rotation as f32;
                            (-heading.cos(), -heading.sin())
                        };

                        // Convert retrograde from world frame to vessel-local frame
                        // (undo the visual_rotation so canopy directions are in local meter space)
                        let retro_local_x = retro_world_x * cos_r + retro_world_y * sin_r;
                        let retro_local_y = -retro_world_x * sin_r + retro_world_y * cos_r;

                        for part_data in parts {
                            if !part_data.is_parachute || part_data.parachute_deploy_fraction < 1e-6 {
                                continue;
                            }

                            // Anchor cables to dome top (bottom-aligned sprite, lowered 0.25 grid squares)
                            let anchor_local_x = part_data.local_x as f32;
                            let anchor_local_y = part_data.local_y as f32 - part_data.hitbox_half_h as f32 + (part_data.sprite_half_h * 2.0) as f32 - 0.125;

                            // Generate canopy in meter space relative to anchor (0,0)
                            let mut canopy_verts: Vec<Vertex> = Vec::new();
                            let visual_scale = if part_data.parachute_fully_deployed { 1.0 } else { 0.5 };
                            crate::editor::generate_parachute_canopy_vertices(
                                &mut canopy_verts,
                                retro_local_x, retro_local_y,
                                part_data.parachute_deployed_width_m,
                                part_data.parachute_deploy_fraction,
                                visual_scale,
                            );

                            if !canopy_verts.is_empty() {
                                let base_index = all_vertices.len() as u32;
                                // Transform from meter space to screen: offset by anchor, scale, rotate
                                for vert in &canopy_verts {
                                    let mx = (vert.position[0] + anchor_local_x) * render_scale;
                                    let my = (vert.position[1] + anchor_local_y) * render_scale;
                                    let rx = mx * cos_r - my * sin_r;
                                    let ry = mx * sin_r + my * cos_r;
                                    all_vertices.push(Vertex::new([rel_x + rx, rel_y + ry], vert.color));
                                }
                                let num_verts = canopy_verts.len() as u32;
                                for i in (0..num_verts).step_by(3) {
                                    if i + 2 < num_verts {
                                        all_indices.push(base_index + i);
                                        all_indices.push(base_index + i + 1);
                                        all_indices.push(base_index + i + 2);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Fallback: draw triangle when no parts available
                    let base_index = all_vertices.len() as u32;

                    // Apply heat tinting to ship color
                    let tri_color = apply_heat_tint(ship_data.color, ship_data.temperature);

                    let nose_angle = rotation;
                    let back_left_angle = rotation + std::f32::consts::PI * 0.8;
                    let back_right_angle = rotation - std::f32::consts::PI * 0.8;

                    all_vertices.push(Vertex::new([
                            rel_x + size * nose_angle.cos(),
                            rel_y + size * nose_angle.sin(),
                        ], tri_color));
                    all_vertices.push(Vertex::new([
                            rel_x + size * 0.6 * back_left_angle.cos(),
                            rel_y + size * 0.6 * back_left_angle.sin(),
                        ], tri_color));
                    all_vertices.push(Vertex::new([
                            rel_x + size * 0.6 * back_right_angle.cos(),
                            rel_y + size * 0.6 * back_right_angle.sin(),
                        ], tri_color));

                    all_indices.push(base_index);
                    all_indices.push(base_index + 1);
                    all_indices.push(base_index + 2);
                }
            }

            // Draw prograde direction arrow at screen edge in ship view
            if !needs_indicator {
                let vdir = ship_data.velocity_direction;
                let has_velocity = vdir[0] != 0.0 || vdir[1] != 0.0;
                if has_velocity {
                    let arrow_color = [1.0_f32, 1.0, 1.0, 0.85];
                    let vdx = vdir[0] as f32;
                    let vdy = vdir[1] as f32;

                    // Scale velocity direction to rendering coordinates
                    let scale_f = scale as f32;
                    let vdx_s = vdx * scale_f;
                    let vdy_s = vdy * scale_f;
                    let vmag = (vdx_s * vdx_s + vdy_s * vdy_s).sqrt();
                    let (vdx_n, vdy_n) = if vmag > 0.0 { (vdx_s / vmag, vdy_s / vmag) } else { (0.0, 1.0) };

                    // Asymmetric margins to keep arrow inside the flight viewport (outside GUI panels)
                    let margin_left = 60.0_f32;    // status panel (50px) + buffer
                    let margin_right = 220.0_f32;  // staging (150) + throttle (50) + buffer (20)
                    let margin_top = 40.0_f32;     // time warp panel + buffer
                    let margin_bottom = 80.0_f32;  // flight info panel + buffer

                    let screen_w = self.size.width as f32;
                    let screen_h = self.size.height as f32;
                    // Bounds relative to screen center
                    let bound_left = -(screen_w / 2.0 - margin_left);
                    let bound_right = screen_w / 2.0 - margin_right;
                    let bound_bottom = -(screen_h / 2.0 - margin_bottom);
                    let bound_top = screen_h / 2.0 - margin_top;

                    // Ship position in screen pixels relative to screen center
                    let ship_scr_x = rel_x * pixels_per_world_unit;
                    let ship_scr_y = rel_y * pixels_per_world_unit;

                    // Direction in screen pixels
                    let dir_scr_x = vdx_n * pixels_per_world_unit;
                    let dir_scr_y = vdy_n * pixels_per_world_unit;

                    // Ray-cast: find t where ship_scr + t*dir_scr hits the bounded viewport edge
                    let mut t = f32::MAX;
                    if dir_scr_x.abs() > 1e-6 {
                        let tx = if dir_scr_x > 0.0 { (bound_right - ship_scr_x) / dir_scr_x } else { (bound_left - ship_scr_x) / dir_scr_x };
                        if tx > 0.0 { t = t.min(tx); }
                    }
                    if dir_scr_y.abs() > 1e-6 {
                        let ty = if dir_scr_y > 0.0 { (bound_top - ship_scr_y) / dir_scr_y } else { (bound_bottom - ship_scr_y) / dir_scr_y };
                        if ty > 0.0 { t = t.min(ty); }
                    }
                    if t == f32::MAX { t = 1.0; }

                    // Arrow tip in world-rendering coords
                    let tip_x = rel_x + vdx_n * t;
                    let tip_y = rel_y + vdy_n * t;

                    // Fixed screen-size arrow: 80px head, 25px half-width (5x original)
                    let arrow_len = 80.0 / pixels_per_world_unit;
                    let half_width = 25.0 / pixels_per_world_unit;

                    // Stem dimensions: extends from arrow base toward the ship
                    let stem_length = 120.0 / pixels_per_world_unit;
                    let stem_half_width = 6.0 / pixels_per_world_unit;

                    // Perpendicular direction
                    let perp_x = -vdy_n;
                    let perp_y = vdx_n;

                    // Arrow base center (where head meets stem)
                    let base_cx = tip_x - vdx_n * arrow_len;
                    let base_cy = tip_y - vdy_n * arrow_len;

                    // Filled triangle head: tip + two base corners
                    let base_index = all_vertices.len() as u32;
                    all_vertices.push(Vertex::new([tip_x, tip_y], arrow_color));
                    all_vertices.push(Vertex::new([
                        base_cx + perp_x * half_width,
                        base_cy + perp_y * half_width,
                    ], arrow_color));
                    all_vertices.push(Vertex::new([
                        base_cx - perp_x * half_width,
                        base_cy - perp_y * half_width,
                    ], arrow_color));
                    all_indices.push(base_index);
                    all_indices.push(base_index + 1);
                    all_indices.push(base_index + 2);

                    // Stem: rectangle from arrow base toward the ship (two triangles)
                    let stem_end_x = base_cx - vdx_n * stem_length;
                    let stem_end_y = base_cy - vdy_n * stem_length;

                    let si = all_vertices.len() as u32;
                    // Four corners of the stem rectangle
                    all_vertices.push(Vertex::new([
                        base_cx + perp_x * stem_half_width,
                        base_cy + perp_y * stem_half_width,
                    ], arrow_color)); // si+0: base left
                    all_vertices.push(Vertex::new([
                        base_cx - perp_x * stem_half_width,
                        base_cy - perp_y * stem_half_width,
                    ], arrow_color)); // si+1: base right
                    all_vertices.push(Vertex::new([
                        stem_end_x - perp_x * stem_half_width,
                        stem_end_y - perp_y * stem_half_width,
                    ], arrow_color)); // si+2: end right
                    all_vertices.push(Vertex::new([
                        stem_end_x + perp_x * stem_half_width,
                        stem_end_y + perp_y * stem_half_width,
                    ], arrow_color)); // si+3: end left
                    all_indices.extend_from_slice(&[si, si + 1, si + 2, si, si + 2, si + 3]);
                }
            }

            // Draw triangle indicator when ship is too small
            if needs_indicator {
                let base_index = all_vertices.len() as u32;

                // Fixed screen-size indicator (16 pixels)
                let indicator_screen_radius = 16.0f32;
                let indicator_size = (indicator_screen_radius / pixels_per_world_unit) as f32;

                // Triangle indicator pointing in direction of ship rotation
                let nose_angle = rotation;
                let back_left_angle = rotation + std::f32::consts::PI * 0.8;
                let back_right_angle = rotation - std::f32::consts::PI * 0.8;

                // Apply heat tinting to indicator color
                let indicator_color = apply_heat_tint(ship_data.color, ship_data.temperature);

                // Outer triangle (indicator)
                all_vertices.push(Vertex::new([
                        rel_x + indicator_size * nose_angle.cos(),
                        rel_y + indicator_size * nose_angle.sin(),
                    ], indicator_color));

                all_vertices.push(Vertex::new([
                        rel_x + indicator_size * 0.6 * back_left_angle.cos(),
                        rel_y + indicator_size * 0.6 * back_left_angle.sin(),
                    ], indicator_color));

                all_vertices.push(Vertex::new([
                        rel_x + indicator_size * 0.6 * back_right_angle.cos(),
                        rel_y + indicator_size * 0.6 * back_right_angle.sin(),
                    ], indicator_color));

                // Inner triangle (darker, for outline effect)
                let inner_size = indicator_size * 0.6;
                let inner_color = [
                    indicator_color[0] * 0.3,
                    indicator_color[1] * 0.3,
                    indicator_color[2] * 0.3,
                    indicator_color[3],
                ];

                all_vertices.push(Vertex::new([
                        rel_x + inner_size * nose_angle.cos(),
                        rel_y + inner_size * nose_angle.sin(),
                    ], inner_color));

                all_vertices.push(Vertex::new([
                        rel_x + inner_size * 0.6 * back_left_angle.cos(),
                        rel_y + inner_size * 0.6 * back_left_angle.sin(),
                    ], inner_color));

                all_vertices.push(Vertex::new([
                        rel_x + inner_size * 0.6 * back_right_angle.cos(),
                        rel_y + inner_size * 0.6 * back_right_angle.sin(),
                    ], inner_color));

                // Outer triangle
                all_indices.push(base_index);
                all_indices.push(base_index + 1);
                all_indices.push(base_index + 2);

                // Inner triangle
                all_indices.push(base_index + 3);
                all_indices.push(base_index + 4);
                all_indices.push(base_index + 5);
            }
        }

        // Background vessels (tracking station, flight map view)
        self.background_vessel_screen_positions.clear();
        if !background_vessels.is_empty() && !in_galaxy_view {
            let cam_x = self.camera.body_center[0];
            let cam_y = self.camera.body_center[1];
            let off_x = self.camera.ship_offset[0];
            let off_y = self.camera.ship_offset[1];
            let pixels_per_world_unit = self.camera.zoom * self.size.height as f32 / 2.0;

            for vessel in background_vessels {
                let rel_x = ((vessel.body_center[0] - cam_x) + (vessel.rel_offset[0] - off_x)) as f32;
                let rel_y = ((vessel.body_center[1] - cam_y) + (vessel.rel_offset[1] - off_y)) as f32;

                let has_parts = vessel.parts.is_some() && part_defs.is_some();

                // Estimate vessel size in pixels to decide if we need an indicator
                let vessel_size_world = if has_parts {
                    let parts = vessel.parts.as_ref().unwrap();
                    let max_extent = parts.iter()
                        .map(|p| (p.local_x.abs() + p.hitbox_half_h).max(p.local_y.abs() + p.hitbox_half_h))
                        .fold(0.0f64, f64::max);
                    (max_extent * 2.0 * scale) as f32
                } else {
                    0.0
                };
                let vessel_pixels = vessel_size_world * pixels_per_world_unit * 2.0;
                let needs_indicator = !has_parts || vessel_pixels < 5.0;

                if has_parts {
                    // Full part rendering for background vessels
                    let parts = vessel.parts.as_ref().unwrap();
                    let defs = part_defs.unwrap();
                    let render_scale = scale as f32;
                    let visual_rotation = vessel.rotation as f32 - std::f32::consts::FRAC_PI_2;
                    let cos_r = visual_rotation.cos();
                    let sin_r = visual_rotation.sin();

                    // First pass: parts
                    let mut part_verts: Vec<Vertex> = Vec::with_capacity(256);
                    for part_data in parts {
                        if let Some(def) = defs.get(&part_data.definition_id) {
                            let local_x = part_data.local_x as f32 * render_scale;
                            let local_y = part_data.local_y as f32 * render_scale;
                            let rotated_x = local_x * cos_r - local_y * sin_r;
                            let rotated_y = local_x * sin_r + local_y * cos_r;
                            // Skip base disc for fairing half debris (shell-only)
                            part_verts.clear();
                            if part_data.fairing_half.is_none() {
                                crate::editor::generate_part_shape_vertices(
                                    &mut part_verts, def, 0.0, 0.0, 1.0,
                                    Some(&self.sprite_atlas),
                                    if part_data.is_solar_panel || part_data.is_radiator { Some(part_data.deploy_fraction) } else { None },
                                );
                            }

                            let base_index = all_vertices.len() as u32;
                            let scale_factor = render_scale;
                            let bg_part_rot = part_data.rotation as f32;
                            for vert in &part_verts {
                                let mut vx = vert.position[0] * scale_factor;
                                let mut vy = vert.position[1] * scale_factor;
                                // Apply part rotation
                                if bg_part_rot.abs() > 1e-6 {
                                    let pc = bg_part_rot.cos();
                                    let ps = bg_part_rot.sin();
                                    let px = vx * pc - vy * ps;
                                    let py = vx * ps + vy * pc;
                                    vx = px;
                                    vy = py;
                                }
                                let rx = vx * cos_r - vy * sin_r;
                                let ry = vx * sin_r + vy * cos_r;
                                all_vertices.push(Vertex {
                                    position: [rel_x + (rotated_x + rx), rel_y + (rotated_y + ry)],
                                    color: vert.color,
                                    uv: vert.uv,
                                });
                            }
                            let num_part_verts = part_verts.len() as u32;
                            for i in (0..num_part_verts).step_by(3) {
                                if i + 2 < num_part_verts {
                                    all_indices.push(base_index + i);
                                    all_indices.push(base_index + i + 1);
                                    all_indices.push(base_index + i + 2);
                                }
                            }
                        }
                    }

                    // Second pass: decoupler adapter fairings
                    for part_data in parts {
                        if let Some(decoupler_def) = defs.get(&part_data.definition_id) {
                            if decoupler_def.decoupler.is_none() {
                                continue;
                            }
                            let dec_x = part_data.local_x as f32;
                            let dec_y = part_data.local_y as f32;
                            let mut adapter_verts: Vec<Vertex> = Vec::new();
                            crate::editor::generate_flight_decoupler_adapter(
                                &mut adapter_verts, decoupler_def,
                                dec_x, dec_y, parts, defs, 1.0,
                            );
                            if !adapter_verts.is_empty() {
                                let base_index = all_vertices.len() as u32;
                                for vert in &adapter_verts {
                                    let vx = vert.position[0] * render_scale;
                                    let vy = vert.position[1] * render_scale;
                                    let rx = vx * cos_r - vy * sin_r;
                                    let ry = vx * sin_r + vy * cos_r;
                                    all_vertices.push(Vertex::new([rel_x + rx, rel_y + ry], vert.color));
                                }
                                let num_verts = adapter_verts.len() as u32;
                                for i in (0..num_verts).step_by(3) {
                                    if i + 2 < num_verts {
                                        all_indices.push(base_index + i);
                                        all_indices.push(base_index + i + 1);
                                        all_indices.push(base_index + i + 2);
                                    }
                                }
                            }
                        }
                    }

                    // Third pass: fairing shells
                    for part_data in parts {
                        let Some(ref shape) = part_data.fairing_shape else { continue };
                        let Some(fairing_def) = defs.get(&part_data.definition_id) else { continue };
                        if fairing_def.fairing.is_none() { continue; }

                        let px = part_data.local_x as f32;
                        let py = part_data.local_y as f32;
                        let hitbox_half_h = part_data.hitbox_half_h as f32;
                        let base_half_w = (fairing_def.width() / 2.0) as f32;
                        let mut shell_verts: Vec<Vertex> = Vec::new();
                        crate::editor::generate_flight_fairing_shell(
                            &mut shell_verts, shape,
                            px, py, hitbox_half_h, base_half_w, 1.0,
                            part_data.fairing_half,
                        );
                        if !shell_verts.is_empty() {
                            let base_index = all_vertices.len() as u32;
                            for vert in &shell_verts {
                                let vx = vert.position[0] * render_scale;
                                let vy = vert.position[1] * render_scale;
                                let rx = vx * cos_r - vy * sin_r;
                                let ry = vx * sin_r + vy * cos_r;
                                all_vertices.push(Vertex::new([rel_x + rx, rel_y + ry], vert.color));
                            }
                            let num_verts = shell_verts.len() as u32;
                            for i in (0..num_verts).step_by(3) {
                                if i + 2 < num_verts {
                                    all_indices.push(base_index + i);
                                    all_indices.push(base_index + i + 1);
                                    all_indices.push(base_index + i + 2);
                                }
                            }
                        }
                    }
                }

                // Triangle indicator when vessel is too small to see or has no parts
                if needs_indicator {
                    let icon_screen_size = 8.0f32;
                    let icon_world_size = icon_screen_size / pixels_per_world_unit;

                    let base_idx = all_vertices.len() as u32;
                    let (tri_verts, tri_idxs) = super::geometry::create_ship_triangle(
                        rel_x, rel_y,
                        icon_world_size,
                        std::f32::consts::FRAC_PI_2,
                        vessel.color,
                    );
                    for v in tri_verts {
                        all_vertices.push(v);
                    }
                    for idx in tri_idxs {
                        all_indices.push(base_idx + idx);
                    }
                }

                // Store screen position for click detection
                let ndc_x = rel_x * self.camera.zoom / self.camera.aspect_ratio;
                let ndc_y = rel_y * self.camera.zoom;
                let scale_factor = self.window.scale_factor() as f32;
                let screen_x = (ndc_x + 1.0) * 0.5 * self.size.width as f32 / scale_factor;
                let screen_y = (1.0 - ndc_y) * 0.5 * self.size.height as f32 / scale_factor;
                self.background_vessel_screen_positions.push((vessel.id, [screen_x, screen_y]));

                // Draw orbit line for this vessel
                if let Some(ref orbit) = vessel.orbit {
                    let e = orbit.eccentricity;
                    if e < 1.0 && orbit.semi_major_axis > 0.0 {
                        let a = orbit.semi_major_axis;
                        let b = a * (1.0 - e * e).sqrt();
                        let c = a * e;
                        let arg_peri = orbit.argument_of_periapsis;
                        // Subtract camera from parent first for precision — both are galaxy-scale,
                        // their difference is solar-system-scale, so orbit geometry stays precise.
                        let pcam_x = orbit.parent_x - cam_x - off_x;
                        let pcam_y = orbit.parent_y - cam_y - off_y;
                        let center_x = pcam_x - c * arg_peri.cos();
                        let center_y = pcam_y - c * arg_peri.sin();

                        let segments = orbit_segments(a, self.camera.zoom, self.size.height as f32);
                        let line_width = 0.002 / self.camera.zoom as f64;

                        for i in 0..segments {
                            let angle = (i as f64 / segments as f64) * std::f64::consts::TAU;
                            let ex = a * angle.cos();
                            let ey = b * angle.sin();
                            let rx = ex * arg_peri.cos() - ey * arg_peri.sin();
                            let ry = ex * arg_peri.sin() + ey * arg_peri.cos();
                            let px = center_x + rx;
                            let py = center_y + ry;

                            let next_angle = ((i + 1) as f64 / segments as f64) * std::f64::consts::TAU;
                            let next_ex = a * next_angle.cos();
                            let next_ey = b * next_angle.sin();
                            let next_rx = next_ex * arg_peri.cos() - next_ey * arg_peri.sin();
                            let next_ry = next_ex * arg_peri.sin() + next_ey * arg_peri.cos();

                            let dx = next_rx - rx;
                            let dy = next_ry - ry;
                            let len = (dx * dx + dy * dy).sqrt();
                            if len < 1e-20 { continue; }
                            let nx = -dy / len * line_width;
                            let ny = dx / len * line_width;

                            let next_px = center_x + next_rx;
                            let next_py = center_y + next_ry;
                            let base = all_vertices.len() as u32;
                            all_vertices.push(Vertex::new([(px + nx) as f32, (py + ny) as f32], orbit.color));
                            all_vertices.push(Vertex::new([(px - nx) as f32, (py - ny) as f32], orbit.color));
                            all_vertices.push(Vertex::new([(next_px - nx) as f32, (next_py - ny) as f32], orbit.color));
                            all_vertices.push(Vertex::new([(next_px + nx) as f32, (next_py + ny) as f32], orbit.color));
                            all_indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                        }
                    }
                }
            }
        }

        // Truncate to buffer capacity to avoid wgpu overrun
        let max_verts = self.vertex_buffer.size() as usize / std::mem::size_of::<Vertex>();
        let max_idx = self.index_buffer.size() as usize / std::mem::size_of::<u32>();
        all_vertices.truncate(max_verts);
        all_indices.truncate(max_idx);

        self.num_indices = all_indices.len() as u32;

        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&all_vertices));
        self.queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&all_indices));
    }

}
