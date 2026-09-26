//! Maneuver node management for RenderState

use super::state::RenderState;
use super::types::{ManeuverDeltaV, ManeuverNode, HYPERBOLIC_RENDER_MARGIN};
use crate::bodies::G;
use crate::ship::AutopilotTarget;

impl RenderState {
    /// Check if a screen click is near a maneuver node, returns node ID if found
    pub fn maneuver_node_at_screen_pos(&self, screen_x: f32, screen_y: f32) -> Option<u64> {
        let click_threshold = 20.0f32; // pixels
        let scale_factor = self.window.scale_factor() as f32;

        for (node_id, screen_pos) in &self.maneuver_node_screen_positions {
            let dx = screen_x - screen_pos[0] * scale_factor;
            let dy = screen_y - screen_pos[1] * scale_factor;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < click_threshold {
                return Some(*node_id);
            }
        }
        None
    }

    /// Check if a screen click is near any visible orbit line, returns (true_anomaly, segment_data) if found.
    /// Searches current trajectory and all predicted trajectories (from maneuver nodes).
    pub fn orbit_click_position(&self, screen_x: f32, screen_y: f32) -> Option<(f64, super::types::OrbitSegmentData)> {
        if self.current_trajectory.is_empty() && self.predicted_trajectories.is_empty() {
            return None;
        }

        let click_threshold = 15.0f32; // pixels
        let scale_factor = self.window.scale_factor() as f32;

        // Convert screen position to egui points
        let click_x = screen_x / scale_factor;
        let click_y = screen_y / scale_factor;

        let cam_x = self.camera.position[0];
        let cam_y = self.camera.position[1];
        let aspect_ratio = self.camera.aspect_ratio;
        let size = self.size;

        let mut best_match: Option<(f64, super::types::OrbitSegmentData, f32)> = None; // (true_anomaly, segment_data, distance)

        // Helper: hit-test a single segment against the click position
        let test_segment = |segment: &super::types::OrbitSegmentData, best: &mut Option<(f64, super::types::OrbitSegmentData, f32)>| {
            let e = segment.eccentricity;
            let arg_peri = segment.argument_of_periapsis;
            let num_samples = 256;
            // Subtract camera from parent first for precision
            let pcam_x = segment.parent_x - cam_x;
            let pcam_y = segment.parent_y - cam_y;

            if e >= 1.0 {
                // Hyperbolic - sample along the visible arc
                let a_abs = segment.semi_major_axis.abs();
                let p = a_abs * (e * e - 1.0);
                let max_ta = (-1.0 / e).acos();

                let start_ta = segment.start_true_anomaly;
                let end_ta = segment.end_true_anomaly.unwrap_or(
                    if segment.retrograde { -(max_ta - HYPERBOLIC_RENDER_MARGIN) } else { max_ta - HYPERBOLIC_RENDER_MARGIN }
                );

                for i in 0..num_samples {
                    let t = i as f64 / (num_samples - 1) as f64;
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

                    let view_x = (px as f32) * self.camera.zoom;
                    let view_y = (py as f32) * self.camera.zoom;
                    let ndc_x = view_x / aspect_ratio;
                    let ndc_y = view_y;
                    let scr_x = (ndc_x + 1.0) * 0.5 * size.width as f32 / scale_factor;
                    let scr_y = (1.0 - ndc_y) * 0.5 * size.height as f32 / scale_factor;

                    let dx = click_x - scr_x;
                    let dy = click_y - scr_y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    if dist < click_threshold {
                        match best {
                            None => *best = Some((ta, segment.clone(), dist)),
                            Some((_, _, prev_dist)) if dist < *prev_dist => *best = Some((ta, segment.clone(), dist)),
                            _ => {}
                        }
                    }
                }
            } else {
                // Elliptical orbit
                let a = segment.semi_major_axis;
                let b = a * (1.0 - e * e).sqrt();
                let c = a * e;
                let center_x = pcam_x - c * arg_peri.cos();
                let center_y = pcam_y - c * arg_peri.sin();

                let start_ta = segment.start_true_anomaly;
                let start_ea = (start_ta.sin() * (1.0 - e * e).sqrt()).atan2(e + start_ta.cos());

                let (start_angle, angle_span) = match segment.end_true_anomaly {
                    Some(end_ta) => {
                        let end_ea = (end_ta.sin() * (1.0 - e * e).sqrt()).atan2(e + end_ta.cos());
                        let span = if segment.retrograde {
                            let mut s = start_ea - end_ea;
                            if s < 0.0 { s += std::f64::consts::TAU; }
                            -s
                        } else {
                            let mut s = end_ea - start_ea;
                            if s < 0.0 { s += std::f64::consts::TAU; }
                            s
                        };
                        (start_ea, span)
                    }
                    None => (start_ea, std::f64::consts::TAU),
                };

                for i in 0..num_samples {
                    let t = i as f64 / num_samples as f64;
                    let ea = start_angle + t * angle_span;

                    let ex = a * ea.cos();
                    let ey = b * ea.sin();
                    let rx = ex * arg_peri.cos() - ey * arg_peri.sin();
                    let ry = ex * arg_peri.sin() + ey * arg_peri.cos();
                    let px = center_x + rx;
                    let py = center_y + ry;

                    let ta = 2.0 * ((1.0 + e).sqrt() * (ea / 2.0).sin())
                        .atan2((1.0 - e).sqrt() * (ea / 2.0).cos());

                    let view_x = (px as f32) * self.camera.zoom;
                    let view_y = (py as f32) * self.camera.zoom;
                    let ndc_x = view_x / aspect_ratio;
                    let ndc_y = view_y;
                    let scr_x = (ndc_x + 1.0) * 0.5 * size.width as f32 / scale_factor;
                    let scr_y = (1.0 - ndc_y) * 0.5 * size.height as f32 / scale_factor;

                    let dx = click_x - scr_x;
                    let dy = click_y - scr_y;
                    let dist = (dx * dx + dy * dy).sqrt();

                    if dist < click_threshold {
                        match best {
                            None => *best = Some((ta, segment.clone(), dist)),
                            Some((_, _, prev_dist)) if dist < *prev_dist => *best = Some((ta, segment.clone(), dist)),
                            _ => {}
                        }
                    }
                }
            }
        };

        // Search current trajectory segments
        for segment in &self.current_trajectory {
            test_segment(segment, &mut best_match);
        }

        // Search predicted trajectories (from maneuver node burns)
        for trajectory in &self.predicted_trajectories {
            for segment in trajectory {
                test_segment(segment, &mut best_match);
            }
        }

        best_match.map(|(ta, seg, _)| (ta, seg))
    }

    /// Convert true anomaly to mean anomaly for an elliptical orbit.
    fn ta_to_ma(ta: f64, e: f64) -> f64 {
        let cos_nu = ta.cos();
        let sin_nu = ta.sin();
        let ea = (sin_nu * (1.0 - e * e).sqrt()).atan2(e + cos_nu);
        (ea - e * ea.sin()).rem_euclid(std::f64::consts::TAU)
    }

    /// Compute the epoch (absolute simulation time) for a node on a given orbit segment.
    /// Uses the segment's own start_true_anomaly and start_time to compute the
    /// time delta from segment start to node position. Both TAs are in the same
    /// orbit frame, so no arg_peri mismatch is possible.
    fn compute_node_epoch(
        &self,
        node_ta: f64,
        segment: &super::types::OrbitSegmentData,
    ) -> f64 {
        let e = segment.eccentricity;
        if e >= 1.0 {
            // Hyperbolic — no good time estimate
            return segment.base_epoch;
        }

        // Convert segment start TA and node TA to mean anomalies (same orbit frame)
        let start_ma = Self::ta_to_ma(segment.start_true_anomaly, e);
        let node_ma = Self::ta_to_ma(node_ta, e);

        // Delta mean anomaly from segment start to node, accounting for orbit direction
        let delta_ma = if segment.retrograde {
            (start_ma - node_ma).rem_euclid(std::f64::consts::TAU)
        } else {
            (node_ma - start_ma).rem_euclid(std::f64::consts::TAU)
        };

        // Unscale semi_major_axis back to meters for mean_motion
        let a_meters = segment.semi_major_axis / segment.render_scale;
        let mu = G * segment.parent_mass;
        let mean_motion = (mu / a_meters.powi(3)).sqrt();

        if mean_motion > 0.0 {
            segment.base_epoch + segment.start_time + delta_ma / mean_motion
        } else {
            segment.base_epoch
        }
    }

    /// Create a new maneuver node at the specified orbit position
    pub fn create_maneuver_node(&mut self, true_anomaly: f64, segment: &super::types::OrbitSegmentData) -> u64 {
        let id = self.next_node_id;
        self.next_node_id += 1;

        let epoch = self.compute_node_epoch(true_anomaly, segment);
        self.maneuver_nodes.push(ManeuverNode {
            id,
            semi_major_axis: segment.semi_major_axis,
            eccentricity: segment.eccentricity,
            argument_of_periapsis: segment.argument_of_periapsis,
            parent_x: segment.parent_x,
            parent_y: segment.parent_y,
            retrograde: segment.retrograde,
            true_anomaly,
            parent_idx: segment.parent_idx,
            parent_mass: segment.parent_mass,
            render_scale: segment.render_scale,
            epoch,
            delta_v: ManeuverDeltaV::default(),
            remaining_delta_v: ManeuverDeltaV::default(),
        });

        self.pending_orbit_click = None;
        self.selected_maneuver_node = Some(id);
        id
    }

    /// Create a maneuver node with pre-filled delta-v values
    pub fn create_maneuver_node_with_dv(&mut self, true_anomaly: f64, segment: &super::types::OrbitSegmentData, dv: ManeuverDeltaV) -> u64 {
        let id = self.next_node_id;
        self.next_node_id += 1;

        let epoch = self.compute_node_epoch(true_anomaly, segment);
        self.maneuver_nodes.push(ManeuverNode {
            id,
            semi_major_axis: segment.semi_major_axis,
            eccentricity: segment.eccentricity,
            argument_of_periapsis: segment.argument_of_periapsis,
            parent_x: segment.parent_x,
            parent_y: segment.parent_y,
            retrograde: segment.retrograde,
            true_anomaly,
            parent_idx: segment.parent_idx,
            parent_mass: segment.parent_mass,
            render_scale: segment.render_scale,
            epoch,
            delta_v: dv.clone(),
            remaining_delta_v: dv,
        });

        self.pending_orbit_click = None;
        self.selected_maneuver_node = Some(id);
        id
    }

    /// Create a maneuver node with pre-filled delta-v and an explicit epoch (absolute simulation time)
    pub fn create_maneuver_node_with_epoch(&mut self, true_anomaly: f64, segment: &super::types::OrbitSegmentData, dv: ManeuverDeltaV, epoch: f64) -> u64 {
        let id = self.next_node_id;
        self.next_node_id += 1;

        self.maneuver_nodes.push(ManeuverNode {
            id,
            semi_major_axis: segment.semi_major_axis,
            eccentricity: segment.eccentricity,
            argument_of_periapsis: segment.argument_of_periapsis,
            parent_x: segment.parent_x,
            parent_y: segment.parent_y,
            retrograde: segment.retrograde,
            true_anomaly,
            parent_idx: segment.parent_idx,
            parent_mass: segment.parent_mass,
            render_scale: segment.render_scale,
            epoch,
            delta_v: dv.clone(),
            remaining_delta_v: dv,
        });

        self.pending_orbit_click = None;
        self.selected_maneuver_node = Some(id);
        id
    }

    /// Delete a maneuver node by ID
    pub fn delete_maneuver_node(&mut self, id: u64) {
        self.maneuver_nodes.retain(|n| n.id != id);
        if self.selected_maneuver_node == Some(id) {
            self.selected_maneuver_node = None;
        }
    }

    /// Get mutable reference to a maneuver node by ID
    pub fn get_maneuver_node_mut(&mut self, id: u64) -> Option<&mut ManeuverNode> {
        self.maneuver_nodes.iter_mut().find(|n| n.id == id)
    }

    /// Set predicted trajectories from external calculation (main.rs)
    pub fn set_predicted_trajectories(&mut self, trajectories: Vec<Vec<super::types::OrbitSegmentData>>) {
        self.predicted_trajectories = trajectories;
    }

    /// Start dragging a maneuver node
    pub fn start_dragging_node(&mut self, node_id: u64) {
        self.dragging_maneuver_node = Some(node_id);
    }

    /// Stop dragging maneuver node
    pub fn stop_dragging_node(&mut self) {
        self.dragging_maneuver_node = None;
    }

    /// Update dragged node position based on mouse position (constrained to node's stored orbit)
    pub fn update_dragged_node(&mut self, screen_x: f32, screen_y: f32) {
        let node_id = match self.dragging_maneuver_node {
            Some(id) => id,
            None => return,
        };

        // Get the node's stored orbit parameters and current parent position
        let node_orbit = match self.maneuver_nodes.iter().find(|n| n.id == node_id) {
            Some(n) => {
                // Use current parent position from bodies
                let (parent_x, parent_y) = self.bodies.get(n.parent_idx)
                    .map(|b| (b.x, b.y))
                    .unwrap_or((n.parent_x, n.parent_y));
                (n.semi_major_axis, n.eccentricity, n.argument_of_periapsis,
                 parent_x, parent_y, n.retrograde)
            }
            None => return,
        };

        // Capture old TA, epoch, and orbit params before mutable borrow
        let (old_ta, old_epoch, e, a, parent_mass, render_scale, retrograde) =
            match self.maneuver_nodes.iter().find(|n| n.id == node_id) {
                Some(n) => (n.true_anomaly, n.epoch, n.eccentricity,
                            n.semi_major_axis, n.parent_mass, n.render_scale, n.retrograde),
                None => return,
            };

        // Find closest true_anomaly on the node's stored orbit
        if let Some(new_ta) = self.find_closest_ta_on_orbit(screen_x, screen_y, node_orbit) {
            // Compute epoch delta from old_ta to new_ta on the node's own orbit
            let new_epoch = if e < 1.0 {
                let old_ma = Self::ta_to_ma(old_ta, e);
                let new_ma = Self::ta_to_ma(new_ta, e);

                let a_meters = a / render_scale;
                let mu = G * parent_mass;
                let mean_motion = (mu / a_meters.powi(3)).sqrt();

                if mean_motion > 0.0 {
                    // Signed delta MA accounting for orbit direction
                    let raw_delta = if retrograde {
                        old_ma - new_ma
                    } else {
                        new_ma - old_ma
                    };
                    // Wrap to [-PI, PI] for forward/backward disambiguation
                    let delta_ma = ((raw_delta + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU))
                        - std::f64::consts::PI;
                    let delta_time = delta_ma / mean_motion;
                    old_epoch + delta_time
                } else {
                    old_epoch
                }
            } else {
                // Hyperbolic — keep existing epoch
                old_epoch
            };

            if let Some(node) = self.maneuver_nodes.iter_mut().find(|n| n.id == node_id) {
                node.true_anomaly = new_ta;
                node.epoch = new_epoch;
            }
        }
    }

    /// Find the closest true anomaly on a given orbit to the screen position
    fn find_closest_ta_on_orbit(
        &self,
        screen_x: f32,
        screen_y: f32,
        orbit: (f64, f64, f64, f64, f64, bool), // (a, e, arg_peri, parent_x, parent_y, retrograde)
    ) -> Option<f64> {
        let (a, e, arg_peri, parent_x, parent_y, _retrograde) = orbit;

        let scale_factor = self.window.scale_factor() as f32;
        let click_x = screen_x / scale_factor;
        let click_y = screen_y / scale_factor;

        let cam_x = self.camera.position[0];
        let cam_y = self.camera.position[1];
        let aspect_ratio = self.camera.aspect_ratio;
        let size = self.size;

        // Subtract camera from parent first for precision
        let pcam_x = parent_x - cam_x;
        let pcam_y = parent_y - cam_y;

        let mut best_match: Option<(f64, f32)> = None; // (true_anomaly, distance)
        let num_samples = 360;

        if e >= 1.0 {
            // Hyperbolic
            let a_abs = a.abs();
            let p = a_abs * (e * e - 1.0);
            let max_ta = (-1.0 / e).acos();

            for i in 0..num_samples {
                let t = i as f64 / (num_samples - 1) as f64;
                let ta = -max_ta + HYPERBOLIC_RENDER_MARGIN + t * 2.0 * (max_ta - HYPERBOLIC_RENDER_MARGIN);

                let denom = 1.0 + e * ta.cos();
                if denom <= 0.001 { continue; }
                let r = p / denom;
                if r <= 0.0 || !r.is_finite() { continue; }

                let angle = ta + arg_peri;
                let px = pcam_x + r * angle.cos();
                let py = pcam_y + r * angle.sin();

                let view_x = (px as f32) * self.camera.zoom;
                let view_y = (py as f32) * self.camera.zoom;
                let ndc_x = view_x / aspect_ratio;
                let ndc_y = view_y;
                let scr_x = (ndc_x + 1.0) * 0.5 * size.width as f32 / scale_factor;
                let scr_y = (1.0 - ndc_y) * 0.5 * size.height as f32 / scale_factor;

                let dx = click_x - scr_x;
                let dy = click_y - scr_y;
                let dist = (dx * dx + dy * dy).sqrt();

                match best_match {
                    None => best_match = Some((ta, dist)),
                    Some((_, prev_dist)) if dist < prev_dist => best_match = Some((ta, dist)),
                    _ => {}
                }
            }
        } else {
            // Elliptical
            let b = a * (1.0 - e * e).sqrt();
            let c = a * e;
            let center_x = pcam_x - c * arg_peri.cos();
            let center_y = pcam_y - c * arg_peri.sin();

            for i in 0..num_samples {
                let t = i as f64 / num_samples as f64;
                let ea = t * std::f64::consts::TAU;

                let ex = a * ea.cos();
                let ey = b * ea.sin();
                let rx = ex * arg_peri.cos() - ey * arg_peri.sin();
                let ry = ex * arg_peri.sin() + ey * arg_peri.cos();
                let px = center_x + rx;
                let py = center_y + ry;

                // Convert eccentric anomaly to true anomaly
                let ta = 2.0 * ((1.0 + e).sqrt() * (ea / 2.0).sin())
                    .atan2((1.0 - e).sqrt() * (ea / 2.0).cos());

                let view_x = (px as f32) * self.camera.zoom;
                let view_y = (py as f32) * self.camera.zoom;
                let ndc_x = view_x / aspect_ratio;
                let ndc_y = view_y;
                let scr_x = (ndc_x + 1.0) * 0.5 * size.width as f32 / scale_factor;
                let scr_y = (1.0 - ndc_y) * 0.5 * size.height as f32 / scale_factor;

                let dx = click_x - scr_x;
                let dy = click_y - scr_y;
                let dist = (dx * dx + dy * dy).sqrt();

                match best_match {
                    None => best_match = Some((ta, dist)),
                    Some((_, prev_dist)) if dist < prev_dist => best_match = Some((ta, dist)),
                    _ => {}
                }
            }
        }

        best_match.map(|(ta, _)| ta)
    }

    /// Get maneuver nodes for external processing
    pub fn get_maneuver_nodes(&self) -> &[ManeuverNode] {
        &self.maneuver_nodes
    }

    /// Get current trajectory for external processing
    pub fn get_current_trajectory(&self) -> &[super::types::OrbitSegmentData] {
        &self.current_trajectory
    }

    /// Get the current autopilot target
    pub fn get_autopilot_target(&self) -> AutopilotTarget {
        self.autopilot_target
    }

    /// Get the selected maneuver node (if any)
    pub fn get_selected_maneuver_node(&self) -> Option<&ManeuverNode> {
        self.selected_maneuver_node
            .and_then(|id| self.maneuver_nodes.iter().find(|n| n.id == id))
    }

    /// Check if a screen click hits a flight part, returns index into flight_parts_cache
    pub fn flight_part_at_screen_pos(&self, screen_x: f32, screen_y: f32) -> Option<usize> {
        if self.flight_parts_cache.is_empty() {
            return None;
        }

        // Compute click offset from camera center directly in f32, avoiding the precision
        // loss from adding a tiny screen offset to galaxy-scale camera.position in f64.
        // camera.position ≈ ship_render_x at ~1e10, so a ~1e-8 screen offset would vanish.
        let ndc_x = (screen_x / self.size.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_y / self.size.height as f32) * 2.0;
        let view_x = ndc_x * self.camera.aspect_ratio / self.camera.zoom;
        let view_y = ndc_y / self.camera.zoom;
        let cos_cam = self.camera.rotation.cos();
        let sin_cam = self.camera.rotation.sin();
        let offset_x = (view_x * cos_cam + view_y * sin_cam) as f64;
        let offset_y = (-view_x * sin_cam + view_y * cos_cam) as f64;

        // offset is click relative to camera center. Add any camera-vs-ship drift
        // (zero when tracking, non-zero after panning).
        let rel_x = offset_x + (self.camera.position[0] - self.ship_render_x);
        let rel_y = offset_y + (self.camera.position[1] - self.ship_render_y);

        // Un-rotate by visual rotation (rotation - PI/2 maps vessel "up" to the heading)
        let visual_rot = self.ship_render_rotation - std::f64::consts::FRAC_PI_2;
        let cos_r = visual_rot.cos();
        let sin_r = visual_rot.sin();
        let local_x = rel_x * cos_r + rel_y * sin_r;
        let local_y = -rel_x * sin_r + rel_y * cos_r;

        // Convert from render units back to meters
        let scale = self.ship_render_scale;
        let local_x_m = local_x / scale;
        let local_y_m = local_y / scale;

        let mut closest: Option<(usize, f64)> = None;

        for (i, part) in self.flight_parts_cache.iter().enumerate() {
            let dx = (local_x_m - part.local_x).abs();
            let dy = (local_y_m - part.click_local_y).abs();

            if dx <= part.hitbox_half_w && dy <= part.click_hitbox_half_h {
                let dist = dx * dx + dy * dy;
                match closest {
                    None => closest = Some((i, dist)),
                    Some((_, prev_dist)) if dist < prev_dist => closest = Some((i, dist)),
                    _ => {}
                }
            }
        }

        closest.map(|(i, _)| i)
    }

    /// Apply a burn to the selected maneuver node, reducing its remaining delta-v
    /// burn_direction: unit vector of the ship's thrust direction
    /// delta_v_magnitude: how much delta-v was applied this frame (m/s)
    /// Note: Only affects remaining_delta_v, not the original delta_v (which defines the trajectory)
    pub fn apply_burn_to_maneuver(&mut self, burn_direction: [f64; 2], delta_v_magnitude: f64) {
        if let Some(node_id) = self.selected_maneuver_node {
            if let Some(node) = self.maneuver_nodes.iter_mut().find(|n| n.id == node_id) {
                // Get the maneuver's prograde and radial unit vectors
                let prograde = node.prograde_unit();
                let radial = node.radial_unit();

                // Project the burn onto the maneuver's coordinate system
                let burn_prograde = burn_direction[0] * prograde[0] + burn_direction[1] * prograde[1];
                let burn_radial = burn_direction[0] * radial[0] + burn_direction[1] * radial[1];

                // Project burn onto maneuver coordinate system
                let prograde_contribution = burn_prograde * delta_v_magnitude;
                let radial_contribution = burn_radial * delta_v_magnitude;

                // Apply burn to remaining delta-v. Correct-direction burns reduce it
                // toward zero; wrong-direction burns increase it (moving further from target).
                node.remaining_delta_v.prograde -= prograde_contribution;
                node.remaining_delta_v.radial_out -= radial_contribution;
            }
        }
    }
}
