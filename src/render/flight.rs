use egui_wgpu::ScreenDescriptor;

use crate::ship::{AutopilotTarget, RAILS_WARP_THRESHOLD};
use super::formatting::{format_duration, format_duration_no_seconds, format_power_si, porkchop_color};
use super::types::PauseAction;
use super::state::RenderState;

impl RenderState {
    pub fn render(
        &mut self,
        body_names: &[String],
        warp_levels: &[f64],
        current_warp_index: usize,
        paused: bool,
        date_str: &str,
        can_exit_flight: bool,
        can_recover: bool,
        has_launch_save: bool,
        quicksaves: &[crate::save::QuicksaveInfo],
    ) -> Result<(usize, PauseAction), wgpu::SurfaceError> {
        // Update camera buffer before rendering
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build egui UI for hovered body label and time warp controls
        let hovered = self.hovered_body;
        let bodies_copy = &self.bodies;
        let size = self.size;
        let camera_pos = self.camera.position;
        let camera_zoom = self.camera.zoom;
        let camera_rotation = self.camera.rotation;
        let camera_body_center = self.camera.body_center;
        let camera_ship_offset = self.camera.ship_offset;
        let aspect_ratio = self.camera.aspect_ratio;
        let scale_factor = self.window.scale_factor() as f32;
        let fps = self.fps;
        let ship_orbit = &self.ship_orbit_info;
        let ship_velocity = self.ship_velocity;
        let ship_altitude = self.ship_altitude;
        let ship_throttle = self.ship_throttle;
        let ship_soi_name = &self.ship_soi_name;
        let ship_time_to_intercept = self.ship_time_to_intercept;
        let vessel_total_mass = self.vessel_total_mass;
        let vessel_fuel_fraction = self.vessel_fuel_fraction;
        let vessel_monoprop_fraction = self.vessel_monoprop_fraction;
        let vessel_electricity_fraction = self.vessel_electricity_fraction;
        let vessel_electricity_stored = self.vessel_electricity_stored;
        let vessel_electricity_max = self.vessel_electricity_max;
        let vessel_power_generation = self.vessel_power_generation;
        let vessel_power_consumption = self.vessel_power_consumption;
        let vessel_food_days = self.vessel_food_days;
        let vessel_total_crew = self.vessel_total_crew;
        let vessel_is_starving = self.vessel_is_starving;
        let vessel_thrust_kn = self.vessel_thrust_kn;
        let vessel_drag_kn = self.vessel_drag_kn;
        let vessel_delta_v = self.vessel_delta_v;
        let vessel_current_stage = self.vessel_current_stage;

        let ship_acceleration = self.ship_acceleration;
        let ship_speed_fraction_c = self.ship_speed_fraction_c;
        let ship_lorentz_gamma = self.ship_lorentz_gamma;
        let ship_proper_time = self.ship_proper_time;
        let ship_mission_time = self.ship_mission_time;
        let ship_is_relativistic = self.ship_is_relativistic;
        let ship_grav_time_factor = self.ship_grav_time_factor;
        let ship_below_landing_altitude = self.ship_below_landing_altitude;
        let ship_soi_surface_gravity = self.ship_soi_surface_gravity;
        let ship_g_force = self.ship_g_force;
        let ship_temperature = self.ship_temperature;
        let ship_heat_fraction = self.ship_heat_fraction;
        let thermal_pool_temp = self.thermal_pool_temp;
        let thermal_pool_gen_w = self.thermal_pool_gen_w;
        let thermal_pool_reject_w = self.thermal_pool_reject_w;
        let reactors_tripped = self.reactors_tripped;
        let selected_flight_part = self.selected_flight_part;
        let flight_parts_cache = &self.flight_parts_cache;
        let ap_markers = &self.ap_markers;
        let pe_markers = &self.pe_markers;
        let pending_orbit_click = &self.pending_orbit_click;
        let selected_maneuver_node = self.selected_maneuver_node;
        let maneuver_nodes = &self.maneuver_nodes;
        let time_to_node = self.time_to_node;
        let burn_time = self.burn_time;
        let warp_to_node_active = self.warp_to_node;
        let current_autopilot = self.autopilot_target;
        let has_control = self.ship_has_control;
        let vessel_stages = &self.vessel_stages;
        let vessel_stage_delta_vs = &self.vessel_stage_delta_vs;
        let vessel_stage_burn_times = &self.vessel_stage_burn_times;

        let mut new_warp_index = current_warp_index;
        let mut pause_action = PauseAction::None;
        let mut create_node_at: Option<(f64, super::types::OrbitSegmentData)> = None;
        let mut delete_node_id: Option<u64> = None;
        let mut close_maneuver_panel = false;
        let mut start_warp_to_node = false;
        let mut cancel_warp_to_node = false;
        let mut prograde_delta: f64 = 0.0;
        let mut radial_delta: f64 = 0.0;
        let mut new_autopilot_target = current_autopilot;
        let mut engine_toggle_req: Option<(usize, bool)> = None;
        let mut crossfeed_toggle_req: Option<(usize, bool)> = None;
        let mut decouple_req: Option<usize> = None;
        let mut fairing_deploy_req: Option<usize> = None;
        let mut solar_deploy_req: Option<(usize, bool)> = None;
        let mut radiator_deploy_req: Option<(usize, bool)> = None;
        let mut shield_toggle_req: Option<(usize, bool)> = None;
        let mut reactor_restart_req: bool = false;
        let mut parachute_deploy_req: Option<usize> = None;
        let mut parachute_cut_req: Option<usize> = None;
        let ship_in_atmosphere = self.ship_in_atmosphere;
        let ship_is_landed = self.ship_is_landed;
        let mut staging_reorder_req: Option<Vec<Vec<usize>>> = None;

        let active_toasts = &self.active_toasts;

        // Procedural star hover state: capture name and screen pos for label
        let hovered_star_label: Option<(String, [f32; 2])> = self.hovered_star.and_then(|idx| {
            let name = self.current_procedural_stars.get(idx).map(|s| s.format_name())?;
            let screen_pos = self.procedural_star_screen_positions.iter()
                .find(|&&(i, _)| i == idx)
                .map(|&(_, pos)| pos)?;
            Some((name, screen_pos))
        });

        let raw_input = self.egui_state.take_egui_input(&self.window);
        // Clone egui::Context (cheap Arc ref-count bump) so the closure
        // can borrow self fields directly without conflicting with self.egui_ctx.
        let egui_ctx = self.egui_ctx.clone();
        let full_output = egui_ctx.run(raw_input, |ctx| {
            // Time warp panel at top of screen
            egui::TopBottomPanel::top("time_warp_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Time Warp:");
                    for (i, &warp) in warp_levels.iter().enumerate() {
                        let label = if warp >= 1000000000.0 {
                            format!("{}B", (warp / 1000000000.0) as i32)
                        } else if warp >= 1000000.0 {
                            format!("{}M", (warp / 1000000.0) as i32)
                        } else if warp >= 1000.0 {
                            format!("{}K", (warp / 1000.0) as i32)
                        } else {
                            format!("{}x", warp as i32)
                        };

                        let is_selected = i == current_warp_index;
                        // Block selecting warp > max physics warp while actually producing thrust
                        let actually_thrusting = ship_throttle > 0.0 && ship_acceleration > 0.0;
                        let blocked_throttle = actually_thrusting && warp > RAILS_WARP_THRESHOLD;
                        // Block on-rails warps that would reach SOI boundary in < 0.1 real seconds
                        // Physics warp (≤10x) handles SOI transitions via substeps, so only block on-rails
                        let blocked_intercept = warp > RAILS_WARP_THRESHOLD && ship_time_to_intercept
                            .map(|t| t / warp < 0.1)
                            .unwrap_or(false);
                        // Block on-rails warp when below landing altitude
                        let blocked_landing = ship_below_landing_altitude && warp > RAILS_WARP_THRESHOLD;
                        let blocked = blocked_throttle || blocked_intercept || blocked_landing;
                        let button = ui.add_enabled(!blocked, egui::SelectableLabel::new(is_selected, &label));
                        if button.clicked() && !blocked {
                            new_warp_index = i;
                        }
                    }

                    // Show current warp value
                    ui.separator();
                    let current_warp = warp_levels[current_warp_index];
                    ui.label(format!("Current: {}x", current_warp as i64));

                    ui.separator();
                    ui.label(date_str);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("{:.0} fps", fps)).size(11.0).color(egui::Color32::GRAY));
                        ui.separator();
                        ui.label(egui::RichText::new(format!("Sci: {:.0}", self.science_available)).size(11.0).color(egui::Color32::from_rgb(100, 180, 255)));
                        ui.label(egui::RichText::new(crate::colony::format_money(self.company_money)).size(11.0).color(egui::Color32::from_rgb(100, 200, 100)));
                    });
                });

                // Orbital info display
                if let Some(ref orbit) = ship_orbit {
                    ui.separator();
                    ui.horizontal(|ui| {
                        // Format distance for display
                        let format_distance = |meters: f64, body_radius: f64| -> String {
                            let altitude = meters - body_radius;
                            if altitude.abs() >= 1e9 {
                                format!("{:.2} Gm", altitude / 1e9)
                            } else if altitude.abs() >= 1e6 {
                                format!("{:.1} Mm", altitude / 1e6)
                            } else if altitude.abs() >= 1e3 {
                                format!("{:.1} km", altitude / 1e3)
                            } else {
                                format!("{:.0} m", altitude)
                            }
                        };

                        // Format time for display
                        let format_time = |seconds: f64| -> String {
                            if seconds >= 86400.0 * 365.0 {
                                format!("{:.1}y", seconds / (86400.0 * 365.0))
                            } else if seconds >= 86400.0 {
                                format!("{:.1}d", seconds / 86400.0)
                            } else if seconds >= 3600.0 {
                                format!("{:.1}h", seconds / 3600.0)
                            } else if seconds >= 60.0 {
                                format!("{:.1}m", seconds / 60.0)
                            } else {
                                format!("{:.0}s", seconds)
                            }
                        };

                        let ap_alt = format_distance(orbit.apoapsis, orbit.parent_body_radius);
                        let pe_alt = format_distance(orbit.periapsis, orbit.parent_body_radius);

                        ui.label(format!("⬆ Ap: {}", ap_alt));
                        ui.label(format!("({})", format_time(orbit.time_to_apoapsis)));
                        ui.separator();
                        ui.label(format!("⬇ Pe: {}", pe_alt));
                        ui.label(format!("({})", format_time(orbit.time_to_periapsis)));
                        ui.separator();
                        ui.label(format!("⏱ T: {}", format_time(orbit.orbital_period)));
                        ui.separator();
                        ui.label(format!("e: {:.3}", orbit.eccentricity));
                        ui.separator();
                        ui.label(format!("◉ {}", ship_soi_name));
                    });
                }
            });

            // Bottom panel for autopilot buttons and velocity/altitude display
            egui::TopBottomPanel::bottom("flight_info_panel")
                .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 200)))
                .show(ctx, |ui| {
                    // Format velocity
                    let vel_str = if ship_speed_fraction_c > 0.01 {
                        format!("{:.2}% c", ship_speed_fraction_c * 100.0)
                    } else if ship_velocity >= 1000.0 {
                        format!("{:.2} km/s", ship_velocity / 1000.0)
                    } else {
                        format!("{:.1} m/s", ship_velocity)
                    };

                    // Format altitude
                    let alt_str = if ship_altitude.abs() >= 1e9 {
                        format!("{:.2} Gm", ship_altitude / 1e9)
                    } else if ship_altitude.abs() >= 1e6 {
                        format!("{:.2} Mm", ship_altitude / 1e6)
                    } else if ship_altitude.abs() >= 1e3 {
                        format!("{:.2} km", ship_altitude / 1e3)
                    } else {
                        format!("{:.1} m", ship_altitude)
                    };

                    // Autopilot buttons row
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);

                        // RCS toggle button
                        {
                            let rcs_btn_color = if !has_control {
                                egui::Color32::from_rgb(40, 40, 45)
                            } else if self.rcs_enabled {
                                egui::Color32::from_rgb(80, 150, 80)
                            } else {
                                egui::Color32::from_rgb(60, 60, 70)
                            };
                            let rcs_text_color = if !has_control {
                                egui::Color32::from_rgb(80, 80, 80)
                            } else if self.rcs_enabled {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::LIGHT_GRAY
                            };
                            let rcs_btn = egui::Button::new(egui::RichText::new("RCS").size(11.0).color(rcs_text_color))
                                .fill(rcs_btn_color)
                                .min_size(egui::vec2(35.0, 20.0));
                            if ui.add(rcs_btn).clicked() && has_control {
                                self.rcs_enabled = !self.rcs_enabled;
                            }
                        }

                        ui.add_space(5.0);
                        ui.label(egui::RichText::new("SAS").size(11.0).color(egui::Color32::GRAY));
                        ui.add_space(5.0);

                        // Helper to create autopilot button
                        let autopilot_btn = |ui: &mut egui::Ui, label: &str, target: AutopilotTarget, current: AutopilotTarget| -> bool {
                            let is_active = current == target;
                            let btn_color = if !has_control {
                                egui::Color32::from_rgb(40, 40, 45)
                            } else if is_active {
                                egui::Color32::from_rgb(80, 150, 80)
                            } else {
                                egui::Color32::from_rgb(60, 60, 70)
                            };
                            let text_color = if !has_control {
                                egui::Color32::from_rgb(80, 80, 80)
                            } else if is_active {
                                egui::Color32::WHITE
                            } else {
                                egui::Color32::LIGHT_GRAY
                            };
                            let btn = egui::Button::new(egui::RichText::new(label).size(11.0).color(text_color))
                                .fill(btn_color)
                                .min_size(egui::vec2(35.0, 20.0));
                            ui.add(btn).clicked() && has_control
                        };

                        if !has_control {
                            ui.colored_label(egui::Color32::RED, "NO CONTROL");
                            ui.add_space(5.0);
                        }

                        // Prograde button
                        if autopilot_btn(ui, "PRO", AutopilotTarget::Prograde, new_autopilot_target) {
                            new_autopilot_target = if new_autopilot_target == AutopilotTarget::Prograde {
                                AutopilotTarget::Off
                            } else {
                                AutopilotTarget::Prograde
                            };
                        }

                        // Retrograde button
                        if autopilot_btn(ui, "RET", AutopilotTarget::Retrograde, new_autopilot_target) {
                            new_autopilot_target = if new_autopilot_target == AutopilotTarget::Retrograde {
                                AutopilotTarget::Off
                            } else {
                                AutopilotTarget::Retrograde
                            };
                        }

                        // Radial In button
                        if autopilot_btn(ui, "R-", AutopilotTarget::RadialIn, new_autopilot_target) {
                            new_autopilot_target = if new_autopilot_target == AutopilotTarget::RadialIn {
                                AutopilotTarget::Off
                            } else {
                                AutopilotTarget::RadialIn
                            };
                        }

                        // Radial Out button
                        if autopilot_btn(ui, "R+", AutopilotTarget::RadialOut, new_autopilot_target) {
                            new_autopilot_target = if new_autopilot_target == AutopilotTarget::RadialOut {
                                AutopilotTarget::Off
                            } else {
                                AutopilotTarget::RadialOut
                            };
                        }

                        // Maneuver node button (only if there's a selected node)
                        if selected_maneuver_node.is_some() {
                            if autopilot_btn(ui, "MAN", AutopilotTarget::ManeuverNode, new_autopilot_target) {
                                new_autopilot_target = if new_autopilot_target == AutopilotTarget::ManeuverNode {
                                    AutopilotTarget::Off
                                } else {
                                    AutopilotTarget::ManeuverNode
                                };
                            }
                        }

                        // Target button (only if a target is selected)
                        if self.selected_target.is_some() {
                            if autopilot_btn(ui, "TGT", AutopilotTarget::Target, new_autopilot_target) {
                                new_autopilot_target = if new_autopilot_target == AutopilotTarget::Target {
                                    AutopilotTarget::Off
                                } else {
                                    AutopilotTarget::Target
                                };
                            }
                        }

                        // Target name display
                        if self.selected_target.is_some() {
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new(format!("→ {}", self.selected_target_name))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(130, 190, 255)));
                            // Clear target button
                            let x_btn = egui::Button::new(egui::RichText::new("x").size(10.0).color(egui::Color32::GRAY))
                                .fill(egui::Color32::TRANSPARENT)
                                .min_size(egui::vec2(16.0, 16.0));
                            if ui.add(x_btn).clicked() {
                                self.selected_target = None;
                                self.selected_target_name.clear();
                                self.selected_target_angle = None;
                                if new_autopilot_target == AutopilotTarget::Target {
                                    new_autopilot_target = AutopilotTarget::Off;
                                }
                            }
                        }

                        // Vessel stats (if vessel loaded)
                        if let Some(mass) = vessel_total_mass {
                            ui.separator();
                            ui.label(egui::RichText::new("M").size(11.0).color(egui::Color32::GRAY));
                            ui.label(egui::RichText::new(format!("{:.2}t", mass)).size(11.0).color(egui::Color32::WHITE));
                        }
                        if let Some(thrust) = vessel_thrust_kn {
                            ui.label(egui::RichText::new("T").size(11.0).color(egui::Color32::GRAY));
                            ui.label(egui::RichText::new(format!("{:.0}kN", thrust)).size(11.0).color(egui::Color32::WHITE));

                            if vessel_drag_kn > 0.01 {
                                ui.label(egui::RichText::new("D").size(11.0).color(egui::Color32::GRAY));
                                ui.label(egui::RichText::new(format!("{:.1}kN", vessel_drag_kn)).size(11.0).color(egui::Color32::WHITE));
                            }

                            // TWR display
                            if let Some(mass) = vessel_total_mass {
                                if mass > 0.0 && ship_soi_surface_gravity > 0.0 {
                                    let twr = thrust / (mass * ship_soi_surface_gravity);
                                    let twr_color = if twr >= 1.0 {
                                        egui::Color32::from_rgb(100, 220, 100)
                                    } else {
                                        egui::Color32::from_rgb(220, 80, 80)
                                    };
                                    ui.label(egui::RichText::new("TWR").size(11.0).color(egui::Color32::GRAY));
                                    ui.label(egui::RichText::new(format!("{:.2}", twr)).size(11.0).color(twr_color));
                                }
                            }
                        }

                        // G-force display
                        {
                            let g_color = if ship_g_force < 3.0 {
                                egui::Color32::WHITE
                            } else if ship_g_force < 6.0 {
                                egui::Color32::from_rgb(220, 200, 80)
                            } else {
                                egui::Color32::from_rgb(220, 80, 80)
                            };
                            ui.label(egui::RichText::new("G").size(11.0).color(egui::Color32::GRAY));
                            ui.label(egui::RichText::new(format!("{:.1}", ship_g_force)).size(11.0).color(g_color));
                        }
                        if let Some(dv) = vessel_delta_v {
                            ui.label(egui::RichText::new("Δv").size(11.0).color(egui::Color32::GRAY));
                            let dv_str = if dv >= 1000.0 {
                                format!("{:.1}km/s", dv / 1000.0)
                            } else {
                                format!("{:.0}m/s", dv)
                            };
                            ui.label(egui::RichText::new(&dv_str).size(11.0).color(egui::Color32::WHITE));
                        }

                        // Velocity and altitude display (right side)
                        let remaining = ui.available_width();
                        ui.add_space((remaining / 2.0 - 100.0).max(10.0));
                        ui.label(egui::RichText::new("VEL").size(11.0).color(egui::Color32::GRAY));
                        ui.label(egui::RichText::new(&vel_str).size(13.0).strong().color(egui::Color32::WHITE));
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("ALT").size(11.0).color(egui::Color32::GRAY));
                        ui.label(egui::RichText::new(&alt_str).size(13.0).strong().color(egui::Color32::WHITE));

                        if ship_is_relativistic || ship_grav_time_factor < 0.999 {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("\u{03B3}").size(11.0).color(egui::Color32::from_rgb(160, 120, 220)));
                            ui.label(egui::RichText::new(format!("{:.4}", ship_lorentz_gamma))
                                .size(13.0).strong().color(egui::Color32::from_rgb(200, 170, 255)));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(format!("Ship T+{}", format_duration_no_seconds(ship_proper_time)))
                                .size(11.0).color(egui::Color32::from_rgb(170, 210, 255)));
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new(format!("Earth T+{}", format_duration_no_seconds(ship_mission_time)))
                                .size(11.0).color(egui::Color32::from_rgb(255, 210, 170)));
                        }

                    });

                    // Colony buttons row (only when landed or colonies exist)
                    let can_transfer = self.ship_is_landed && self.vessel_has_cargo
                        && self.landed_body_has_colony;
                    if self.can_establish_colony || self.landed_body_has_colony || can_transfer {
                        ui.horizontal(|ui| {
                            if self.can_establish_colony {
                                let btn = egui::Button::new(
                                    egui::RichText::new("Establish Colony").size(12.0)
                                ).fill(egui::Color32::from_rgb(60, 130, 60));
                                if ui.add(btn).clicked() {
                                    if let Some(bi) = self.landed_body_index {
                                        self.render_requests.push(
                                            crate::render::types::RenderRequest::EstablishColony { body_index: bi },
                                        );
                                    }
                                }
                            }
                            if can_transfer {
                                let btn = egui::Button::new(
                                    egui::RichText::new("Transfer Cargo").size(12.0)
                                ).fill(egui::Color32::from_rgb(60, 100, 180));
                                if ui.add(btn).clicked() {
                                    if let Some(bi) = self.landed_body_index {
                                        self.render_requests.push(
                                            crate::render::types::RenderRequest::TransferCargo { body_index: bi },
                                        );
                                    }
                                }
                            }
                            if self.landed_body_has_colony {
                                let btn = egui::Button::new(
                                    egui::RichText::new("Open Colony").size(12.0)
                                ).fill(egui::Color32::from_rgb(80, 120, 60));
                                if ui.add(btn).clicked() {
                                    if let Some(bi) = self.landed_body_index {
                                        self.render_requests.push(
                                            crate::render::types::RenderRequest::OpenColony { body_index: bi },
                                        );
                                    }
                                }
                            }
                        });
                    }
                });

            // Only draw label for hovered body
            if let Some(idx) = hovered {
                if let Some(body) = bodies_copy.get(idx) {
                    if let Some(name) = body_names.get(idx) {
                        let painter = ctx.layer_painter(egui::LayerId::new(
                            egui::Order::Foreground,
                            egui::Id::new("body_labels"),
                        ));

                        // Convert world to screen coordinates (in pixels)
                        // Calculate relative position in f64, then convert to f32 for screen
                        let rel_x = (body.x - camera_pos[0]) as f32;
                        let rel_y = (body.y - camera_pos[1]) as f32;
                        let cos_r = camera_rotation.cos();
                        let sin_r = camera_rotation.sin();
                        let rot_x = rel_x * cos_r - rel_y * sin_r;
                        let rot_y = rel_x * sin_r + rel_y * cos_r;
                        let view_x = rot_x * camera_zoom;
                        let view_y = rot_y * camera_zoom;
                        let ndc_x = view_x / aspect_ratio;
                        let ndc_y = view_y;
                        let screen_x_px = (ndc_x + 1.0) * 0.5 * size.width as f32;
                        let screen_y_px = (1.0 - ndc_y) * 0.5 * size.height as f32;

                        // Convert pixels to egui points
                        let screen_x = screen_x_px / scale_factor;
                        let screen_y = screen_y_px / scale_factor;

                        // Position label above the indicator circle
                        let label_y = screen_y - 20.0;

                        painter.text(
                            egui::pos2(screen_x, label_y),
                            egui::Align2::CENTER_BOTTOM,
                            name,
                            egui::FontId::proportional(12.0),
                            egui::Color32::WHITE,
                        );
                    }
                }
            }

            // Draw label for hovered procedural star
            if let Some((ref star_name, star_pos)) = hovered_star_label {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("star_labels"),
                ));
                let sx = star_pos[0] / scale_factor;
                let sy = star_pos[1] / scale_factor - 20.0;
                painter.text(
                    egui::pos2(sx, sy),
                    egui::Align2::CENTER_BOTTOM,
                    star_name,
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_rgb(200, 200, 255),
                );
            }

            // Draw Apoapsis and Periapsis labels
            let marker_painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("orbit_marker_labels"),
            ));

            // Helper to convert world position to screen position
            let world_to_screen = |world_rel: [f64; 2]| -> (f32, f32) {
                let rx = world_rel[0] as f32;
                let ry = world_rel[1] as f32;
                let cos_r = camera_rotation.cos();
                let sin_r = camera_rotation.sin();
                let rot_x = rx * cos_r - ry * sin_r;
                let rot_y = rx * sin_r + ry * cos_r;
                let view_x = rot_x * camera_zoom;
                let view_y = rot_y * camera_zoom;
                let ndc_x = view_x / aspect_ratio;
                let ndc_y = view_y;
                let screen_x_px = (ndc_x + 1.0) * 0.5 * size.width as f32;
                let screen_y_px = (1.0 - ndc_y) * 0.5 * size.height as f32;
                (screen_x_px / scale_factor, screen_y_px / scale_factor)
            };

            // Helper to format altitude
            let format_altitude = |meters: f64| -> String {
                if meters.abs() >= 1e9 {
                    format!("{:.2} Gm", meters / 1e9)
                } else if meters.abs() >= 1e6 {
                    format!("{:.1} Mm", meters / 1e6)
                } else if meters.abs() >= 1e3 {
                    format!("{:.1} km", meters / 1e3)
                } else {
                    format!("{:.0} m", meters)
                }
            };

            // Get mouse position for hover detection
            let mouse_pos = ctx.input(|i| i.pointer.hover_pos());
            let hover_radius = 20.0; // pixels

            // Draw all apoapsis markers
            for (pos, altitude) in ap_markers.iter() {
                let (screen_x, screen_y) = world_to_screen(*pos);
                let marker_screen_pos = egui::pos2(screen_x, screen_y);

                // Check if mouse is hovering over marker
                let is_hovered = mouse_pos.map_or(false, |mp| {
                    (mp - marker_screen_pos).length() < hover_radius
                });

                marker_painter.text(
                    egui::pos2(screen_x, screen_y - 12.0),
                    egui::Align2::CENTER_BOTTOM,
                    "Ap",
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(255, 153, 51), // Orange
                );

                // Show altitude on hover
                if is_hovered {
                    marker_painter.text(
                        egui::pos2(screen_x, screen_y + 14.0),
                        egui::Align2::CENTER_TOP,
                        &format_altitude(*altitude),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgb(255, 153, 51),
                    );
                }
            }

            // Draw all periapsis markers
            for (pos, altitude) in pe_markers.iter() {
                let (screen_x, screen_y) = world_to_screen(*pos);
                let marker_screen_pos = egui::pos2(screen_x, screen_y);

                // Check if mouse is hovering over marker
                let is_hovered = mouse_pos.map_or(false, |mp| {
                    (mp - marker_screen_pos).length() < hover_radius
                });

                marker_painter.text(
                    egui::pos2(screen_x, screen_y - 12.0),
                    egui::Align2::CENTER_BOTTOM,
                    "Pe",
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(77, 204, 255), // Cyan
                );

                // Show altitude on hover
                if is_hovered {
                    marker_painter.text(
                        egui::pos2(screen_x, screen_y + 14.0),
                        egui::Align2::CENTER_TOP,
                        &format_altitude(*altitude),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgb(77, 204, 255),
                    );
                }
            }

            // Draw closest approach marker label
            if let Some((pos, distance)) = &self.closest_approach_marker {
                let (screen_x, screen_y) = world_to_screen(*pos);
                let marker_screen_pos = egui::pos2(screen_x, screen_y);

                let is_hovered = mouse_pos.map_or(false, |mp| {
                    (mp - marker_screen_pos).length() < hover_radius
                });

                marker_painter.text(
                    egui::pos2(screen_x, screen_y - 12.0),
                    egui::Align2::CENTER_BOTTOM,
                    "CA",
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(255, 255, 0), // Yellow
                );

                if is_hovered {
                    marker_painter.text(
                        egui::pos2(screen_x, screen_y + 14.0),
                        egui::Align2::CENTER_TOP,
                        &format_altitude(*distance),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgb(255, 255, 0),
                    );
                }
            }

            // Draw target closest approach marker label
            if let Some((pos, distance)) = &self.target_closest_approach_marker {
                let (screen_x, screen_y) = world_to_screen(*pos);
                let marker_screen_pos = egui::pos2(screen_x, screen_y);

                let is_hovered = mouse_pos.map_or(false, |mp| {
                    (mp - marker_screen_pos).length() < hover_radius
                });

                marker_painter.text(
                    egui::pos2(screen_x, screen_y - 12.0),
                    egui::Align2::CENTER_BOTTOM,
                    "CA",
                    egui::FontId::proportional(11.0),
                    egui::Color32::from_rgb(255, 255, 0),
                );

                if is_hovered {
                    marker_painter.text(
                        egui::pos2(screen_x, screen_y + 14.0),
                        egui::Align2::CENTER_TOP,
                        &format_altitude(*distance),
                        egui::FontId::proportional(10.0),
                        egui::Color32::from_rgb(255, 255, 0),
                    );
                }
            }

            // Draw "Create Maneuver Node" button if pending click
            if let Some((ta, ref segment)) = pending_orbit_click {
                let e = segment.eccentricity;
                let arg_peri = segment.argument_of_periapsis;

                let r = if e >= 1.0 {
                    let a_abs = segment.semi_major_axis.abs();
                    let p = a_abs * (e * e - 1.0);
                    let denom = 1.0 + e * ta.cos();
                    if denom > 0.001 { p / denom } else { 0.0 }
                } else {
                    let a = segment.semi_major_axis;
                    let p = a * (1.0 - e * e);
                    p / (1.0 + e * ta.cos())
                };

                if r > 0.0 && r.is_finite() {
                    let angle = ta + arg_peri;
                    let world_x = segment.parent_x + r * angle.cos();
                    let world_y = segment.parent_y + r * angle.sin();

                    let (scr_x, scr_y) = world_to_screen([world_x - camera_pos[0], world_y - camera_pos[1]]);

                    // Draw a small window with the create button
                    egui::Area::new(egui::Id::new("create_node_popup"))
                        .fixed_pos(egui::pos2(scr_x + 15.0, scr_y - 15.0))
                        .show(ctx, |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                if ui.button("Create Maneuver Node").clicked() {
                                    create_node_at = Some((*ta, segment.clone()));
                                }
                            });
                        });
                }
            }

            // Draw "Select as Target" popup if a body/vessel was single-clicked
            if let Some(ref popup) = self.target_popup {
                let popup_target = popup.target;
                let popup_name = popup.name.clone();

                // Compute screen position dynamically from the target's world position
                // (world_to_screen inlined to avoid borrow conflict with egui closure)
                // Output in egui logical points (divided by scale_factor)
                let cam_x = self.camera.position[0];
                let cam_y = self.camera.position[1];
                let cam_zoom = self.camera.zoom;
                let cam_rot = self.camera.rotation;
                let cam_aspect = self.camera.aspect_ratio;
                let scr_w = self.size.width as f32;
                let scr_h = self.size.height as f32;
                let w2s = |wx: f64, wy: f64| -> (f32, f32) {
                    let rel_x = (wx - cam_x) as f32;
                    let rel_y = (wy - cam_y) as f32;
                    let cos_r = cam_rot.cos();
                    let sin_r = cam_rot.sin();
                    let rx = rel_x * cos_r - rel_y * sin_r;
                    let ry = rel_x * sin_r + rel_y * cos_r;
                    let nx = rx * cam_zoom / cam_aspect;
                    let ny = ry * cam_zoom;
                    (
                        (nx + 1.0) * 0.5 * scr_w / scale_factor,
                        (1.0 - ny) * 0.5 * scr_h / scale_factor,
                    )
                };
                let points_per_world_unit = cam_zoom * scr_h / 2.0 / scale_factor;

                let screen_pos = match popup.target {
                    super::types::SelectedTarget::Body(idx) => {
                        if let Some(body) = self.bodies.get(idx) {
                            let (sx, sy) = w2s(body.x, body.y);
                            let visual_radius = if body.indicator_radius > 0.0 {
                                body.indicator_radius as f32 * points_per_world_unit
                            } else {
                                body.radius as f32 * points_per_world_unit
                            };
                            let offset_below = visual_radius.max(8.0) + 6.0;
                            Some((sx, sy + offset_below))
                        } else {
                            None
                        }
                    }
                    super::types::SelectedTarget::Vessel(id) => {
                        self.background_vessel_screen_positions.iter()
                            .find(|&&(vid, _)| vid == id)
                            .map(|&(_, pos)| (pos[0] / scale_factor, pos[1] / scale_factor + 14.0))
                    }
                };

                if let Some((popup_x, popup_y)) = screen_pos {
                    egui::Area::new(egui::Id::new("target_select_popup"))
                        .fixed_pos(egui::pos2(popup_x, popup_y))
                        .pivot(egui::Align2::CENTER_TOP)
                        .show(ctx, |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.label(egui::RichText::new(&popup_name).strong());
                                let is_current_target = self.selected_target == Some(popup_target);
                                if is_current_target {
                                    if ui.button("Unselect Target").clicked() {
                                        self.selected_target = None;
                                        self.selected_target_name.clear();
                                        self.target_popup = None;
                                    }
                                } else {
                                    if ui.button("Set as Target").clicked() {
                                        self.selected_target = Some(popup_target);
                                        self.selected_target_name = popup_name.clone();
                                        self.target_popup = None;
                                    }
                                }
                            });
                        });
                } else {
                    // Target not visible on screen, clear popup
                    self.target_popup = None;
                }
            }

            // Right panel - Staging (always visible when vessel has stages)
            if !vessel_stages.is_empty() {
                egui::SidePanel::right("flight_staging_panel")
                    .default_width(150.0)
                    .show(ctx, |ui| {
                        ui.heading("Staging");
                        ui.separator();

                        // Total Δv
                        let total_dv: f64 = vessel_stage_delta_vs.iter().sum();
                        if total_dv > 0.0 {
                            let dv_str = if total_dv >= 1000.0 {
                                format!("{:.1} km/s", total_dv / 1000.0)
                            } else {
                                format!("{:.0} m/s", total_dv)
                            };
                            ui.label(egui::RichText::new(format!("Total Δv: {}", dv_str))
                                .size(12.0).strong());
                            let cruise_v = crate::ship::relativistic_cruise_velocity(total_dv);
                            let cruise_beta = cruise_v / crate::ship::SPEED_OF_LIGHT;
                            if cruise_beta > 0.005 {
                                let cruise_str = if cruise_beta > 0.01 {
                                    format!("Cruise: {:.2}% c", cruise_beta * 100.0)
                                } else {
                                    format!("Cruise: {:.0} km/s", cruise_v / 1000.0)
                                };
                                ui.label(egui::RichText::new(cruise_str)
                                    .size(10.0).color(egui::Color32::from_rgb(180, 140, 240)));
                            }
                        }

                        if let Some(current) = vessel_current_stage {
                            ui.label(egui::RichText::new(format!("Active: {}/{}", current, vessel_stages.len()))
                                .size(11.0).color(egui::Color32::GRAY));
                        }
                        ui.separator();

                        // Drag payload: either a part or a whole stage
                        #[derive(Clone, Copy)]
                        enum FlightStageDrag {
                            Part(usize),   // part_index
                            Stage(usize),  // stage_index
                        }

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut insert_stage_at: Option<usize> = None;
                            let mut delete_stage_at: Option<usize> = None;
                            let mut move_stage_to: Option<(usize, usize)> = None;
                            let mut drop_action: Option<(FlightStageDrag, usize)> = None;

                            // Helper: "+" gap that doubles as a drop zone for stage reordering
                            let plus_gap = |ui: &mut egui::Ui, insert_pos: usize,
                                                 insert_out: &mut Option<usize>,
                                                 move_out: &mut Option<(usize, usize)>| {
                                let frame = egui::Frame::none();
                                let (inner, dropped) = ui.dnd_drop_zone::<FlightStageDrag, ()>(frame, |ui| {
                                    if ui.small_button("+").on_hover_text("Insert stage here").clicked() {
                                        *insert_out = Some(insert_pos);
                                    }
                                });
                                if inner.response.hovered() && egui::DragAndDrop::has_any_payload(ui.ctx()) {
                                    inner.response.highlight();
                                }
                                if let Some(payload) = dropped {
                                    if let FlightStageDrag::Stage(from_idx) = *payload {
                                        *move_out = Some((from_idx, insert_pos));
                                    }
                                }
                            };

                            for stage_idx in (0..vessel_stages.len()).rev() {
                                plus_gap(ui, stage_idx + 1, &mut insert_stage_at, &mut move_stage_to);

                                let frame = egui::Frame::group(ui.style());
                                let (_, dropped_payload) = ui.dnd_drop_zone::<FlightStageDrag, ()>(frame, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let activated = vessel_current_stage.map_or(false, |c| stage_idx < c);
                                        let label_color = if activated {
                                            egui::Color32::DARK_GRAY
                                        } else {
                                            egui::Color32::WHITE
                                        };
                                        let stage_drag_id = egui::Id::new(("flight_staging_stage", stage_idx));
                                        ui.dnd_drag_source(stage_drag_id, FlightStageDrag::Stage(stage_idx), |ui| {
                                            ui.label(egui::RichText::new(format!("{}", stage_idx + 1)).color(label_color));
                                        });
                                        // Per-stage Δv
                                        let stage_dv = vessel_stage_delta_vs.get(stage_idx).copied().unwrap_or(0.0);
                                        if stage_dv > 0.0 {
                                            let dv_str = if stage_dv >= 1000.0 {
                                                format!("{:.1}km/s", stage_dv / 1000.0)
                                            } else {
                                                format!("{:.0}m/s", stage_dv)
                                            };
                                            ui.label(egui::RichText::new(dv_str)
                                                .size(10.0).color(egui::Color32::from_rgb(120, 200, 120)));
                                        }
                                        // Per-stage burn time
                                        let burn_time = vessel_stage_burn_times.get(stage_idx).copied().unwrap_or(0.0);
                                        if burn_time > 0.0 {
                                            ui.label(egui::RichText::new(format_duration(burn_time))
                                                .size(10.0).color(egui::Color32::from_rgb(120, 200, 120)));
                                        }
                                        // Delete button for empty stages
                                        if vessel_stages[stage_idx].is_empty() {
                                            if ui.small_button("\u{2715}").on_hover_text("Delete empty stage").clicked() {
                                                delete_stage_at = Some(stage_idx);
                                            }
                                        }
                                    });

                                    if vessel_stages[stage_idx].is_empty() {
                                        ui.weak("(empty)");
                                    }

                                    let selected_vessel_part = selected_flight_part
                                        .and_then(|ci| flight_parts_cache.get(ci))
                                        .map(|p| p.part_index);

                                    for part_info in &vessel_stages[stage_idx] {
                                        let item_id = egui::Id::new(("flight_staging_item", part_info.part_index));
                                        let is_selected = selected_vessel_part == Some(part_info.part_index);
                                        ui.dnd_drag_source(item_id, FlightStageDrag::Part(part_info.part_index), |ui| {
                                            let text = egui::RichText::new(&part_info.name);
                                            let text = if is_selected {
                                                text.color(egui::Color32::from_rgb(128, 179, 255))
                                            } else {
                                                text
                                            };
                                            ui.label(text);
                                        });
                                    }
                                });

                                if let Some(payload) = dropped_payload {
                                    drop_action = Some((*payload, stage_idx));
                                }
                            }

                            plus_gap(ui, 0, &mut insert_stage_at, &mut move_stage_to);

                            // Build new stages if any action occurred
                            if move_stage_to.is_some() || delete_stage_at.is_some() || insert_stage_at.is_some() || drop_action.is_some() {
                                let mut new_stages: Vec<Vec<usize>> = vessel_stages.iter()
                                    .map(|stage| stage.iter().map(|p| p.part_index).collect())
                                    .collect();

                                if let Some((from_idx, to_pos)) = move_stage_to {
                                    if from_idx < new_stages.len() {
                                        let stage = new_stages.remove(from_idx);
                                        let insert_at = if from_idx < to_pos {
                                            (to_pos - 1).min(new_stages.len())
                                        } else {
                                            to_pos.min(new_stages.len())
                                        };
                                        new_stages.insert(insert_at, stage);
                                    }
                                } else if let Some(idx) = delete_stage_at {
                                    if idx < new_stages.len() && new_stages[idx].is_empty() {
                                        new_stages.remove(idx);
                                    }
                                } else if let Some(idx) = insert_stage_at {
                                    new_stages.insert(idx, Vec::new());
                                } else if let Some((drag, target_idx)) = drop_action {
                                    match drag {
                                        FlightStageDrag::Part(part_idx) => {
                                            for stage in &mut new_stages {
                                                stage.retain(|&idx| idx != part_idx);
                                            }
                                            if target_idx < new_stages.len() {
                                                new_stages[target_idx].push(part_idx);
                                            }
                                        }
                                        FlightStageDrag::Stage(from_idx) => {
                                            if from_idx != target_idx && from_idx < new_stages.len() {
                                                let stage = new_stages.remove(from_idx);
                                                let insert_at = if from_idx < target_idx {
                                                    (target_idx - 1).min(new_stages.len())
                                                } else {
                                                    target_idx.min(new_stages.len())
                                                };
                                                new_stages.insert(insert_at, stage);
                                            }
                                        }
                                    }
                                }

                                staging_reorder_req = Some(new_stages);
                            }
                        });
                    });
            }

            // Throttle bar on right side (left of staging panel)
            egui::SidePanel::right("throttle_panel")
                .exact_width(50.0)
                .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 200)))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("THR").size(10.0).color(egui::Color32::GRAY));
                        ui.add_space(5.0);

                        // Throttle percentage text
                        let throttle_pct = (ship_throttle * 100.0) as i32;
                        ui.label(egui::RichText::new(format!("{}%", throttle_pct))
                            .size(12.0)
                            .strong()
                            .color(egui::Color32::WHITE));
                        ui.add_space(5.0);

                        // Vertical throttle bar
                        let bar_height = 150.0;
                        let bar_width = 20.0;
                        let (rect, _response) = ui.allocate_exact_size(
                            egui::vec2(bar_width, bar_height),
                            egui::Sense::hover()
                        );

                        let painter = ui.painter();

                        // Background (empty part)
                        painter.rect_filled(
                            rect,
                            2.0,
                            egui::Color32::from_rgb(40, 40, 50)
                        );

                        // Filled part (from bottom up)
                        let fill_height = bar_height * ship_throttle as f32;
                        let fill_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x, rect.max.y - fill_height),
                            egui::vec2(bar_width, fill_height)
                        );

                        // Color gradient: green at low, yellow at mid, red at high
                        let fill_color = if ship_throttle < 0.5 {
                            egui::Color32::from_rgb(100, 200, 100) // Green
                        } else if ship_throttle < 0.8 {
                            egui::Color32::from_rgb(200, 200, 100) // Yellow
                        } else {
                            egui::Color32::from_rgb(200, 100, 100) // Red
                        };

                        painter.rect_filled(fill_rect, 2.0, fill_color);

                        // Border
                        painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::GRAY));

                        // Heat bar (shown when temperature > 350K)
                        if ship_temperature > 350.0 {
                            ui.add_space(15.0);
                            ui.label(egui::RichText::new("HEAT").size(10.0).color(egui::Color32::GRAY));
                            ui.add_space(3.0);

                            ui.label(egui::RichText::new(format!("{}K", ship_temperature as i32))
                                .size(11.0)
                                .color(egui::Color32::WHITE));
                            ui.add_space(3.0);

                            let heat_bar_height = 80.0;
                            let heat_bar_width = 20.0;
                            let (heat_rect, _) = ui.allocate_exact_size(
                                egui::vec2(heat_bar_width, heat_bar_height),
                                egui::Sense::hover()
                            );

                            let heat_painter = ui.painter();
                            heat_painter.rect_filled(heat_rect, 2.0, egui::Color32::from_rgb(40, 40, 50));

                            let heat_fill = heat_bar_height * ship_heat_fraction;
                            let heat_fill_rect = egui::Rect::from_min_size(
                                egui::pos2(heat_rect.min.x, heat_rect.max.y - heat_fill),
                                egui::vec2(heat_bar_width, heat_fill)
                            );
                            let heat_color = if ship_heat_fraction < 0.33 {
                                egui::Color32::from_rgb(220, 200, 80)  // yellow
                            } else if ship_heat_fraction < 0.66 {
                                egui::Color32::from_rgb(220, 140, 40)  // orange
                            } else {
                                egui::Color32::from_rgb(220, 60, 60)   // red
                            };
                            heat_painter.rect_filled(heat_fill_rect, 2.0, heat_color);
                            heat_painter.rect_stroke(heat_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::GRAY));

                            // Show the critical part (highest heat_fraction)
                            if let Some(hottest) = flight_parts_cache.iter()
                                .max_by(|a, b| a.heat_fraction.partial_cmp(&b.heat_fraction).unwrap_or(std::cmp::Ordering::Equal))
                            {
                                if hottest.heat_fraction > 0.01 {
                                    ui.add_space(5.0);
                                    let crit_color = if hottest.heat_fraction < 0.33 {
                                        egui::Color32::from_rgb(220, 200, 80)
                                    } else if hottest.heat_fraction < 0.66 {
                                        egui::Color32::from_rgb(220, 140, 40)
                                    } else {
                                        egui::Color32::from_rgb(220, 60, 60)
                                    };
                                    // Truncate name to fit the narrow panel
                                    let name: String = hottest.name.chars().take(8).collect();
                                    ui.label(egui::RichText::new(name)
                                        .size(8.0).color(crit_color));
                                }
                            }
                        }

                        // Waste-heat pool bar (shown when pool warm OR reactors tripped)
                        if thermal_pool_temp > 400.0 || reactors_tripped {
                            ui.add_space(12.0);
                            ui.label(egui::RichText::new("WASTE").size(10.0).color(egui::Color32::GRAY));
                            ui.add_space(3.0);
                            ui.label(egui::RichText::new(format!("{}K", thermal_pool_temp as i32))
                                .size(11.0)
                                .color(egui::Color32::WHITE));
                            ui.add_space(3.0);

                            let pool_bar_height = 80.0;
                            let pool_bar_width = 20.0;
                            let (pool_rect, _) = ui.allocate_exact_size(
                                egui::vec2(pool_bar_width, pool_bar_height),
                                egui::Sense::hover()
                            );
                            let p = ui.painter();
                            p.rect_filled(pool_rect, 2.0, egui::Color32::from_rgb(40, 40, 50));
                            // Map 300 K → 0%, 1500 K → 100%
                            let pool_frac = ((thermal_pool_temp - 300.0) / 1200.0).clamp(0.0, 1.0) as f32;
                            let pool_fill = pool_bar_height * pool_frac;
                            let pool_fill_rect = egui::Rect::from_min_size(
                                egui::pos2(pool_rect.min.x, pool_rect.max.y - pool_fill),
                                egui::vec2(pool_bar_width, pool_fill),
                            );
                            // Thresholds: 800 K → 0.417, 1200 K → 0.75, 1500 K → 1.0
                            let pool_color = if pool_frac < 0.417 {
                                egui::Color32::from_rgb(100, 200, 100)
                            } else if pool_frac < 0.75 {
                                egui::Color32::from_rgb(220, 200, 80)
                            } else if pool_frac < 1.0 {
                                egui::Color32::from_rgb(220, 140, 40)
                            } else {
                                egui::Color32::from_rgb(220, 60, 60)
                            };
                            p.rect_filled(pool_fill_rect, 2.0, pool_color);
                            p.rect_stroke(pool_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::GRAY));

                            if reactors_tripped {
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new("TRIP").size(9.0)
                                    .color(egui::Color32::from_rgb(255, 100, 100)).strong());
                                let can_restart = thermal_pool_temp < 800.0;
                                ui.add_enabled_ui(can_restart, |ui| {
                                    if ui.small_button("Restart").clicked() {
                                        reactor_restart_req = true;
                                    }
                                });
                            } else if thermal_pool_gen_w > thermal_pool_reject_w && thermal_pool_gen_w > 0.0 {
                                ui.add_space(4.0);
                                let deficit_w = thermal_pool_gen_w - thermal_pool_reject_w;
                                ui.label(egui::RichText::new(format!("+{}", format_power_si(deficit_w)))
                                    .size(8.0)
                                    .color(egui::Color32::from_rgb(220, 140, 40)))
                                    .on_hover_text("Net heat input (generation exceeds rejection)");
                            }
                        }
                    });
                });

            // Left panel - fuel, electricity, stage, XFER, debug
            egui::SidePanel::left("status_panel")
                .exact_width(50.0)
                .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 200)))
                .show(ctx, |ui| {
                    // Fuel bar (if vessel loaded)
                    if let Some(fuel_frac) = vessel_fuel_fraction {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("FUEL").size(10.0).color(egui::Color32::GRAY));
                        ui.add_space(3.0);

                        let fuel_pct = (fuel_frac * 100.0) as i32;
                        ui.label(egui::RichText::new(format!("{}%", fuel_pct))
                            .size(11.0)
                            .color(egui::Color32::WHITE));
                        ui.add_space(3.0);

                        let fuel_bar_height = 80.0;
                        let bar_width = 20.0;
                        let (fuel_rect, fuel_resp) = ui.allocate_exact_size(
                            egui::vec2(bar_width, fuel_bar_height),
                            egui::Sense::hover()
                        );
                        fuel_resp.on_hover_text(format!("Current stage fuel: {:.0}%", fuel_frac * 100.0));

                        let fuel_painter = ui.painter();
                        fuel_painter.rect_filled(fuel_rect, 2.0, egui::Color32::from_rgb(40, 40, 50));

                        let fuel_fill = fuel_bar_height * fuel_frac as f32;
                        let fuel_fill_rect = egui::Rect::from_min_size(
                            egui::pos2(fuel_rect.min.x, fuel_rect.max.y - fuel_fill),
                            egui::vec2(bar_width, fuel_fill)
                        );
                        let fuel_color = if fuel_frac > 0.3 {
                            egui::Color32::from_rgb(80, 160, 220)
                        } else if fuel_frac > 0.1 {
                            egui::Color32::from_rgb(220, 180, 80)
                        } else {
                            egui::Color32::from_rgb(220, 80, 80)
                        };
                        fuel_painter.rect_filled(fuel_fill_rect, 2.0, fuel_color);
                        fuel_painter.rect_stroke(fuel_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::GRAY));
                    }

                    // Monopropellant bar (if vessel has monoprop capacity)
                    if let Some(mono_frac) = vessel_monoprop_fraction {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("MONO").size(10.0).color(egui::Color32::GRAY));
                        ui.add_space(3.0);

                        let mono_pct = (mono_frac * 100.0) as i32;
                        ui.label(egui::RichText::new(format!("{}%", mono_pct))
                            .size(11.0)
                            .color(egui::Color32::WHITE));
                        ui.add_space(3.0);

                        let mono_bar_height = 80.0;
                        let bar_width = 20.0;
                        let (mono_rect, mono_resp) = ui.allocate_exact_size(
                            egui::vec2(bar_width, mono_bar_height),
                            egui::Sense::hover()
                        );
                        mono_resp.on_hover_text(format!("Monopropellant: {:.0}%", mono_frac * 100.0));

                        let mono_painter = ui.painter();
                        mono_painter.rect_filled(mono_rect, 2.0, egui::Color32::from_rgb(40, 40, 50));

                        let mono_fill = mono_bar_height * mono_frac as f32;
                        let mono_fill_rect = egui::Rect::from_min_size(
                            egui::pos2(mono_rect.min.x, mono_rect.max.y - mono_fill),
                            egui::vec2(bar_width, mono_fill)
                        );
                        let mono_color = if mono_frac > 0.3 {
                            egui::Color32::from_rgb(80, 200, 200)  // cyan
                        } else if mono_frac > 0.1 {
                            egui::Color32::from_rgb(220, 180, 80)  // yellow
                        } else {
                            egui::Color32::from_rgb(220, 80, 80)   // red
                        };
                        mono_painter.rect_filled(mono_fill_rect, 2.0, mono_color);
                        mono_painter.rect_stroke(mono_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::GRAY));
                    }

                    // Electricity bar (if vessel has batteries)
                    if let Some(elec_frac) = vessel_electricity_fraction {
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("ELEC").size(10.0).color(egui::Color32::GRAY));
                        ui.add_space(3.0);

                        let stored = vessel_electricity_stored.unwrap_or(0.0);
                        let max = vessel_electricity_max.unwrap_or(0.0);
                        let fmt_wh = |v: f64| -> String {
                            if v >= 1000.0 { format!("{:.1}k", v / 1000.0) } else { format!("{:.0}", v) }
                        };
                        ui.label(egui::RichText::new(format!("{} / {} Wh", fmt_wh(stored), fmt_wh(max)))
                            .size(11.0)
                            .color(egui::Color32::WHITE));
                        ui.add_space(3.0);

                        let elec_bar_height = 80.0;
                        let bar_width = 20.0;
                        let (elec_rect, elec_resp) = ui.allocate_exact_size(
                            egui::vec2(bar_width, elec_bar_height),
                            egui::Sense::hover()
                        );
                        elec_resp.on_hover_text(format!("{:.0} / {:.0} Wh", stored, max));

                        let elec_painter = ui.painter();
                        elec_painter.rect_filled(elec_rect, 2.0, egui::Color32::from_rgb(40, 40, 50));

                        let elec_fill = elec_bar_height * elec_frac as f32;
                        let elec_fill_rect = egui::Rect::from_min_size(
                            egui::pos2(elec_rect.min.x, elec_rect.max.y - elec_fill),
                            egui::vec2(bar_width, elec_fill)
                        );
                        let elec_color = if elec_frac > 0.3 {
                            egui::Color32::from_rgb(200, 190, 60)  // gold/yellow
                        } else if elec_frac > 0.1 {
                            egui::Color32::from_rgb(220, 140, 40)  // orange
                        } else {
                            egui::Color32::from_rgb(220, 60, 60)   // red
                        };
                        elec_painter.rect_filled(elec_fill_rect, 2.0, elec_color);
                        elec_painter.rect_stroke(elec_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::GRAY));

                        // Power generation/consumption text
                        if let (Some(gen), Some(cons)) = (vessel_power_generation, vessel_power_consumption) {
                            ui.add_space(3.0);
                            let net_text = format!("+{:.0}W\n-{:.0}W", gen, cons);
                            ui.label(egui::RichText::new(net_text).size(9.0).color(egui::Color32::GRAY));

                            // Power duration when draining
                            let net = gen - cons;
                            if net < 0.0 && stored > 0.0 {
                                let seconds = (stored / net.abs()) * 3600.0;
                                ui.label(egui::RichText::new(format_duration(seconds))
                                    .size(9.0)
                                    .color(egui::Color32::from_rgb(220, 180, 40)));
                            }
                        }
                    }


                    // Food bar (if crew aboard)
                    if let Some(food_days) = vessel_food_days {
                        if vessel_total_crew.unwrap_or(0) > 0 {
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("FOOD").size(10.0).color(egui::Color32::GRAY));
                            ui.add_space(3.0);

                            let crew = vessel_total_crew.unwrap_or(0);
                            let days_text = if food_days > 999.0 {
                                format!("{:.0}d", food_days)
                            } else {
                                format!("{:.1}d", food_days)
                            };
                            let label_color = if vessel_is_starving {
                                egui::Color32::from_rgb(255, 60, 60)
                            } else {
                                egui::Color32::WHITE
                            };
                            ui.label(egui::RichText::new(format!("{} ({})", days_text, crew))
                                .size(11.0)
                                .color(label_color));
                            ui.add_space(3.0);

                            let food_bar_height = 80.0;
                            let bar_width = 20.0;
                            let (food_rect, food_resp) = ui.allocate_exact_size(
                                egui::vec2(bar_width, food_bar_height),
                                egui::Sense::hover()
                            );
                            food_resp.on_hover_text(format!("{:.1} days at {:.1} kg/day ({} crew)", food_days, crew as f64 * 1.5, crew));

                            let food_painter = ui.painter();
                            food_painter.rect_filled(food_rect, 2.0, egui::Color32::from_rgb(40, 40, 50));

                            let frac = (food_days / 60.0).clamp(0.0, 1.0) as f32;
                            let food_fill = food_bar_height * frac;
                            let food_fill_rect = egui::Rect::from_min_size(
                                egui::pos2(food_rect.min.x, food_rect.max.y - food_fill),
                                egui::vec2(bar_width, food_fill)
                            );
                            let food_color = if food_days > 30.0 {
                                egui::Color32::from_rgb(80, 200, 100)
                            } else if food_days > 7.0 {
                                egui::Color32::from_rgb(220, 180, 60)
                            } else {
                                egui::Color32::from_rgb(220, 60, 60)
                            };
                            food_painter.rect_filled(food_fill_rect, 2.0, food_color);
                            food_painter.rect_stroke(food_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::GRAY));
                        }
                    }

                    // XFER button anchored at bottom
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        let xfer_active = self.transfer_planner_open;
                        let xfer_btn_color = if xfer_active {
                            egui::Color32::from_rgb(80, 120, 180)
                        } else {
                            egui::Color32::from_rgb(60, 60, 70)
                        };
                        let xfer_text_color = if xfer_active {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::LIGHT_GRAY
                        };

                        // Ellipse icon + XFER label as button
                        let btn_size = egui::vec2(40.0, 32.0);
                        let (rect, response) = ui.allocate_exact_size(btn_size, egui::Sense::click());

                        // Draw button background
                        let painter = ui.painter();
                        painter.rect_filled(rect, 4.0, xfer_btn_color);

                        // Draw ellipse icon (orbit shape) using line segments
                        let cx = rect.center().x;
                        let cy = rect.center().y - 4.0;
                        let rx = 10.0_f32;
                        let ry = 6.0_f32;
                        let n = 24;
                        let ellipse_points: Vec<egui::Pos2> = (0..=n).map(|i| {
                            let angle = std::f32::consts::TAU * i as f32 / n as f32;
                            egui::pos2(cx + rx * angle.cos(), cy + ry * angle.sin())
                        }).collect();
                        painter.add(egui::Shape::line(ellipse_points, egui::Stroke::new(1.5, xfer_text_color)));

                        // Draw "XFER" label below icon
                        painter.text(
                            egui::pos2(rect.center().x, rect.max.y - 6.0),
                            egui::Align2::CENTER_CENTER,
                            "XFER",
                            egui::FontId::proportional(8.0),
                            xfer_text_color,
                        );

                        if response.clicked() {
                            self.transfer_planner_open = !self.transfer_planner_open;
                        }

                        // Debug menu button (above XFER)
                        ui.add_space(5.0);
                        let dbg_color = if self.debug_menu_open {
                            egui::Color32::from_rgb(180, 80, 80)
                        } else {
                            egui::Color32::from_rgb(60, 60, 70)
                        };
                        let dbg_btn = egui::Button::new(
                            egui::RichText::new("DBG").size(9.0).color(egui::Color32::LIGHT_GRAY)
                        ).fill(dbg_color).min_size(egui::vec2(40.0, 20.0));
                        if ui.add(dbg_btn).clicked() {
                            self.debug_menu_open = !self.debug_menu_open;
                        }
                    });
                });

            // Right panel for selected maneuver node
            if let Some(node_id) = selected_maneuver_node {
                if let Some(node) = maneuver_nodes.iter().find(|n| n.id == node_id) {
                    let remaining_dv = node.total_remaining_delta_v();

                    egui::SidePanel::right("maneuver_panel")
                        .exact_width(200.0)
                        .show(ctx, |ui| {
                            ui.heading("Maneuver Node");
                            ui.separator();

                            // Remaining delta-v display
                            ui.label(format!("Remaining Δv: {:.1} m/s", remaining_dv));

                            // Time-to-node and burn time
                            if let Some(ttn) = time_to_node {
                                ui.label(format!("T- {}", format_duration(ttn)));
                                if let Some(bt) = burn_time {
                                    if bt > 0.5 {
                                        ui.label(format!("Burn: {}", format_duration(bt)));
                                    }
                                }
                                ui.add_space(4.0);
                                if warp_to_node_active {
                                    if ui.button("Cancel Warp").clicked() {
                                        cancel_warp_to_node = true;
                                        new_warp_index = 0;
                                    }
                                } else if ttn > 10.0 {
                                    if ui.button("Warp to Node").clicked() {
                                        start_warp_to_node = true;
                                    }
                                }
                            }
                            ui.separator();

                            // Prograde/Retrograde slider (show remaining values)
                            ui.label("Prograde / Retrograde:");
                            ui.horizontal(|ui| {
                                ui.label(format!("{:+.1} m/s", node.remaining_delta_v.prograde));
                            });

                            // Snap-back slider for prograde
                            let prograde_response = ui.add(
                                egui::Slider::new(&mut prograde_delta, -100.0..=100.0)
                                    .show_value(false)
                                    .text("")
                            );
                            if prograde_response.drag_stopped() {
                                prograde_delta = 0.0;
                            }

                            ui.add_space(10.0);

                            // Radial slider (show remaining values)
                            ui.label("Radial Out / In:");
                            ui.horizontal(|ui| {
                                ui.label(format!("{:+.1} m/s", node.remaining_delta_v.radial_out));
                            });

                            // Snap-back slider for radial
                            let radial_response = ui.add(
                                egui::Slider::new(&mut radial_delta, -100.0..=100.0)
                                    .show_value(false)
                                    .text("")
                            );
                            if radial_response.drag_stopped() {
                                radial_delta = 0.0;
                            }

                            ui.add_space(10.0);
                            ui.separator();

                            // Delete and close buttons
                            ui.horizontal(|ui| {
                                if ui.button("Delete").clicked() {
                                    delete_node_id = Some(node_id);
                                }
                                if ui.button("Close").clicked() {
                                    close_maneuver_panel = true;
                                }
                            });
                        });
                }
            }

            // Part info popup
            if let Some(cache_idx) = selected_flight_part {
                if let Some(part) = flight_parts_cache.get(cache_idx) {
                    egui::Window::new(&part.name)
                        .id(egui::Id::new("flight_part_info"))
                        .collapsible(false)
                        .resizable(false)
                        .default_width(200.0)
                        .max_height(ctx.screen_rect().height() * 0.7)
                        .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.label(format!("Mass: {:.3} t", part.dry_mass));

                            // Engine info
                            if let Some(thrust_vac) = part.engine_thrust_vac {
                                ui.separator();
                                ui.label(egui::RichText::new("Engine").strong());
                                ui.label(format!("Thrust (vac): {:.1} kN", thrust_vac));
                                if let Some(thrust_asl) = part.engine_thrust_asl {
                                    ui.label(format!("Thrust (ASL): {:.1} kN", thrust_asl));
                                }
                                if let Some(isp_vac) = part.engine_isp_vac {
                                    ui.label(format!("ISP (vac): {:.0} s", isp_vac));
                                }
                                if let Some(isp_asl) = part.engine_isp_asl {
                                    ui.label(format!("ISP (ASL): {:.0} s", isp_asl));
                                }
                                if let Some(ref prop) = part.propellant_name {
                                    ui.label(format!("Propellant: {}", prop));
                                }
                                let status_text = if part.engine_enabled {
                                    if part.engine_active {
                                        egui::RichText::new("Active").color(egui::Color32::from_rgb(100, 220, 100))
                                    } else if part.engine_no_power {
                                        egui::RichText::new("No Power").color(egui::Color32::from_rgb(200, 140, 220))
                                    } else {
                                        egui::RichText::new("No Fuel").color(egui::Color32::from_rgb(220, 180, 80))
                                    }
                                } else {
                                    egui::RichText::new("Disabled").color(egui::Color32::from_rgb(220, 80, 80))
                                };
                                ui.label(status_text);

                                if let Some(wh) = part.engine_waste_heat {
                                    ui.label(format!("Waste heat: {}", format_power_si(wh)));
                                }

                                let btn_label = if part.engine_enabled { "Deactivate" } else { "Activate" };
                                if ui.button(btn_label).clicked() {
                                    engine_toggle_req = Some((part.part_index, !part.engine_enabled));
                                }
                            }

                            // Tank info
                            if let Some(ref fuel_name) = part.fuel_type_name {
                                ui.separator();
                                ui.label(egui::RichText::new("Fuel Tank").strong());
                                ui.label(format!("Type: {}", fuel_name));
                                // Oxidizer bar
                                if let (Some(current), Some(max)) = (part.ox_current, part.ox_max) {
                                    if max > 0.0 {
                                        let frac = (current / max) as f32;
                                        let bar = egui::ProgressBar::new(frac)
                                            .text(format!("O2: {:.0}/{:.0} kg", current, max))
                                            .fill(egui::Color32::from_rgb(80, 140, 200));
                                        ui.add(bar);
                                    }
                                }
                                // Fuel bar
                                if let (Some(current), Some(max)) = (part.fuel_current, part.fuel_max) {
                                    if max > 0.0 {
                                        let frac = (current / max) as f32;
                                        let fuel_label = fuel_name.split('/').last().unwrap_or("Fuel");
                                        let bar = egui::ProgressBar::new(frac)
                                            .text(format!("{}: {:.0}/{:.0} kg", fuel_label, current, max))
                                            .fill(egui::Color32::from_rgb(200, 160, 60));
                                        ui.add(bar);
                                    }
                                }
                            }

                            // Pod info
                            if let Some(crew) = part.crew_capacity {
                                ui.separator();
                                ui.label(egui::RichText::new("Command Pod").strong());
                                ui.label(format!("Crew capacity: {}", crew));

                                // Monopropellant bar
                                if let (Some(current), Some(max)) = (part.monoprop_current, part.monoprop_max) {
                                    if max > 0.0 {
                                        let frac = (current / max) as f32;
                                        let bar = egui::ProgressBar::new(frac)
                                            .text(format!("Monoprop: {:.1}/{:.0} kg", current, max))
                                            .fill(egui::Color32::from_rgb(180, 200, 80));
                                        ui.add(bar);
                                    }
                                }
                            }

                            // Battery info
                            if let (Some(current), Some(max)) = (part.battery_current, part.battery_max) {
                                ui.separator();
                                ui.label(egui::RichText::new("Battery").strong());
                                ui.label(format!("Capacity: {:.0} Wh", max));
                                if max > 0.0 {
                                    let frac = (current / max) as f32;
                                    let bar = egui::ProgressBar::new(frac)
                                        .text(format!("{:.0} / {:.0} Wh", current, max))
                                        .fill(egui::Color32::from_rgb(200, 190, 60));
                                    ui.add(bar);
                                }
                            }

                            // Solar panel info
                            if let Some(output) = part.solar_output {
                                ui.separator();
                                ui.label(egui::RichText::new("Solar Panel").strong());
                                ui.label(format!("Output: {:.0} W", output));
                                let label = if part.deploy_fraction >= 0.5 { "Retract" } else { "Extend" };
                                if ui.button(label).clicked() {
                                    solar_deploy_req = Some((part.part_index, part.deploy_fraction < 0.5));
                                }
                            }

                            // Radiator info
                            if part.is_radiator {
                                ui.separator();
                                let tier_label = match part.radiator_tier {
                                    Some(crate::parts::RadiatorTier::HeatPipe) => "Heat Pipe Radiator",
                                    Some(crate::parts::RadiatorTier::Droplet)  => "Droplet Radiator",
                                    Some(crate::parts::RadiatorTier::Phononic) => "Phononic Emitter",
                                    None => "Radiator",
                                };
                                ui.label(egui::RichText::new(tier_label).strong());
                                if let Some(rejection) = part.radiator_rejection_watts {
                                    ui.label(format!("Rejection: {}", format_power_si(rejection)));
                                }
                                let deploy_pct = (part.deploy_fraction * 100.0) as i32;
                                ui.label(format!("Deployed: {}%", deploy_pct));
                                let label = if part.deploy_fraction >= 0.5 { "Retract" } else { "Deploy" };
                                if ui.button(label).clicked() {
                                    radiator_deploy_req = Some((part.part_index, part.deploy_fraction < 0.5));
                                }
                            }

                            // RTG info
                            if let Some(output) = part.rtg_output {
                                ui.separator();
                                ui.label(egui::RichText::new("RTG").strong());
                                ui.label(format!("Output: {:.0} W", output));
                            }

                            // Reactor info
                            if let Some(output) = part.reactor_output {
                                ui.separator();
                                ui.label(egui::RichText::new("Reactor").strong());
                                ui.label(format!("Output: {}", format_power_si(output)));
                                if let Some(wh) = part.reactor_waste_heat {
                                    ui.label(format!("Waste heat: {}", format_power_si(wh)));
                                }
                            }

                            // Shield info
                            if let Some(ref shield_type) = part.shield_type {
                                ui.separator();
                                ui.label(egui::RichText::new("Shield").strong());
                                ui.label(format!("Type: {}", shield_type));
                                if let Some(max_c) = part.shield_max_c {
                                    ui.label(format!("Max Velocity: {:.0}% c", max_c * 100.0));
                                }
                                if let Some(power) = part.shield_power {
                                    if power > 0.0 {
                                        ui.label(format!("Power Draw: {}", format_power_si(power)));
                                        if part.shield_active {
                                            ui.label(egui::RichText::new("Active").color(egui::Color32::from_rgb(100, 220, 100)));
                                        } else {
                                            ui.label(egui::RichText::new("Inactive").color(egui::Color32::from_rgb(160, 160, 160)));
                                        }
                                        let btn_label = if part.shield_active { "Deactivate" } else { "Activate" };
                                        if ui.button(btn_label).clicked() {
                                            shield_toggle_req = Some((part.part_index, !part.shield_active));
                                        }
                                    } else {
                                        ui.label("Power Draw: None (passive)");
                                    }
                                }
                            }

                            // Decoupler info
                            if part.is_decoupler {
                                ui.separator();
                                ui.label(egui::RichText::new("Decoupler").strong());

                                let crossfeed_label = if part.crossfeed_enabled {
                                    "Disable Crossfeed"
                                } else {
                                    "Enable Crossfeed"
                                };
                                if ui.button(crossfeed_label).clicked() {
                                    crossfeed_toggle_req = Some((part.part_index, !part.crossfeed_enabled));
                                }

                                if ui.button("Decouple").clicked() {
                                    decouple_req = Some(part.part_index);
                                }
                            }

                            // Fairing info
                            if part.is_fairing {
                                ui.separator();
                                ui.label(egui::RichText::new("Fairing").strong());
                                if ui.button("Deploy").clicked() {
                                    fairing_deploy_req = Some(part.part_index);
                                }
                            }

                            // Parachute info
                            if part.is_parachute {
                                ui.separator();
                                ui.label(egui::RichText::new("Parachute").strong());
                                ui.label(format!("Deployed Width: {:.1} m", part.parachute_deployed_width_m));
                                if part.parachute_spent {
                                    let btn = ui.add_enabled(false, egui::Button::new("Spent"));
                                    btn.on_disabled_hover_text("Parachute already used");
                                } else if part.parachute_deployed {
                                    if ui.button("Cut").clicked() {
                                        parachute_cut_req = Some(part.part_index);
                                    }
                                } else {
                                    let can_deploy = ship_in_atmosphere && !ship_is_landed;
                                    let btn = ui.add_enabled(can_deploy, egui::Button::new("Deploy"));
                                    if btn.clicked() {
                                        parachute_deploy_req = Some(part.part_index);
                                    }
                                    if !can_deploy {
                                        if !ship_in_atmosphere {
                                            btn.on_disabled_hover_text("Cannot deploy in vacuum");
                                        } else {
                                            btn.on_disabled_hover_text("Cannot deploy while landed");
                                        }
                                    }
                                }
                            }
                        }); // ScrollArea
                        });
                }
            }

            // Draw maneuver node markers
            let node_painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("maneuver_node_markers"),
            ));

            for node in maneuver_nodes.iter() {
                // Two-step precision: (parent - body_center) + (orbit_offset - ship_offset)
                if let Some(parent) = bodies_copy.get(node.parent_idx) {
                    let off = node.orbit_offset();
                    let rel_x = (parent.x - camera_body_center[0]) + off[0] - camera_ship_offset[0];
                    let rel_y = (parent.y - camera_body_center[1]) + off[1] - camera_ship_offset[1];
                    let (scr_x, scr_y) = world_to_screen([rel_x, rel_y]);

                    let is_selected = selected_maneuver_node == Some(node.id);
                    let marker_color = if is_selected {
                        egui::Color32::from_rgb(255, 200, 100) // Bright gold when selected
                    } else {
                        egui::Color32::from_rgb(200, 150, 50) // Dimmer gold
                    };

                    // Draw a diamond marker
                    let marker_size = if is_selected { 10.0 } else { 8.0 };
                    let center = egui::pos2(scr_x, scr_y);
                    let points = vec![
                        egui::pos2(center.x, center.y - marker_size),
                        egui::pos2(center.x + marker_size, center.y),
                        egui::pos2(center.x, center.y + marker_size),
                        egui::pos2(center.x - marker_size, center.y),
                    ];
                    node_painter.add(egui::Shape::convex_polygon(
                        points,
                        marker_color,
                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                    ));

                    // Show remaining delta-v on hover or when selected
                    if is_selected || mouse_pos.map_or(false, |mp| (mp - center).length() < 15.0) {
                        let remaining_dv = node.total_remaining_delta_v();
                        if remaining_dv > 0.0 {
                            node_painter.text(
                                egui::pos2(scr_x, scr_y - 18.0),
                                egui::Align2::CENTER_BOTTOM,
                                format!("{:.0} m/s", remaining_dv),
                                egui::FontId::proportional(10.0),
                                egui::Color32::WHITE,
                            );
                        }
                    }
                }
            }

            // Transfer planner window
            if self.transfer_planner_open {
                let mut close_planner = false;
                egui::Area::new(egui::Id::new("transfer_planner"))
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 35, 240))
                            .inner_margin(egui::Margin::same(12.0))
                            .rounding(egui::Rounding::same(6.0))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 80)))
                            .show(ui, |ui| {
                                ui.set_min_width(280.0);
                                ui.set_max_width(320.0);

                                // Title bar with close button
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Transfer Planner").size(14.0).color(egui::Color32::WHITE).strong());
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("X").clicked() {
                                            close_planner = true;
                                        }
                                    });
                                });
                                ui.separator();

                                // Mode selector
                                let prev_mode = self.transfer_planner_mode;
                                ui.horizontal(|ui| {
                                    ui.selectable_value(&mut self.transfer_planner_mode, 0, "Hohmann");
                                    ui.selectable_value(&mut self.transfer_planner_mode, 1, "Lambert");
                                });
                                if self.transfer_planner_mode != prev_mode {
                                    self.transfer_selected_target = None;
                                    self.porkchop_grid = None;
                                    self.porkchop_selected = None;
                                    self.porkchop_hovered = None;
                                }
                                ui.add_space(4.0);

                                // Target dropdown
                                let targets = if self.transfer_planner_mode == 0 {
                                    &self.transfer_hohmann_targets
                                } else {
                                    &self.transfer_interplanetary_targets
                                };

                                if targets.is_empty() {
                                    ui.label(egui::RichText::new("No valid targets").size(11.0).color(egui::Color32::from_rgb(200, 150, 100)));
                                } else {
                                    let current_name = self.transfer_selected_target
                                        .and_then(|idx| targets.iter().find(|(i, _)| *i == idx).map(|(_, n)| n.as_str()))
                                        .unwrap_or("Select target...");
                                    egui::ComboBox::from_label("")
                                        .selected_text(current_name)
                                        .show_ui(ui, |ui| {
                                            for (idx, name) in targets {
                                                let selected = self.transfer_selected_target == Some(*idx);
                                                if ui.selectable_label(selected, name).clicked() {
                                                    self.transfer_selected_target = Some(*idx);
                                                    // Sync nav target to match planner selection
                                                    self.selected_target = Some(super::types::SelectedTarget::Body(*idx));
                                                    self.selected_target_name = name.clone();
                                                    // Reset porkchop selection when changing target
                                                    self.porkchop_grid = None;
                                                    self.porkchop_selected = None;
                                                    self.porkchop_hovered = None;
                                                }
                                            }
                                        });
                                }

                                ui.add_space(6.0);

                                // Porkchop plot (Lambert mode)
                                if self.transfer_planner_mode == 1 {
                                    if let Some(ref grid) = self.porkchop_grid {
                                        let plot_width = 280.0_f32;
                                        let plot_height = 200.0_f32;
                                        let (response, painter) = ui.allocate_painter(
                                            egui::vec2(plot_width, plot_height),
                                            egui::Sense::click_and_drag(),
                                        );
                                        let rect = response.rect;

                                        // Paint background
                                        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(20, 20, 30));

                                        // Cell dimensions
                                        let cell_w = plot_width / grid.cols as f32;
                                        let cell_h = plot_height / grid.rows as f32;
                                        let log_dv_min = grid.min_dv.ln();
                                        let log_dv_max = grid.max_dv.ln();
                                        let log_dv_range = (log_dv_max - log_dv_min).max(0.01);

                                        // Paint grid cells with log-scale multi-stop color gradient
                                        for row in 0..grid.rows {
                                            for col in 0..grid.cols {
                                                let idx = row * grid.cols + col;
                                                if let Some(ref pt) = grid.points[idx] {
                                                    // Log-scale normalization
                                                    let norm = ((pt.ejection_dv.ln() - log_dv_min) / log_dv_range).clamp(0.0, 1.0);
                                                    let color = porkchop_color(norm as f32);
                                                    let x = rect.left() + col as f32 * cell_w;
                                                    let y = rect.top() + row as f32 * cell_h;
                                                    painter.rect_filled(
                                                        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cell_w, cell_h)),
                                                        0.0,
                                                        color,
                                                    );
                                                }
                                            }
                                        }

                                        // Determine hovered cell
                                        let mut new_hovered = None;
                                        if let Some(hover_pos) = response.hover_pos() {
                                            let rel_x = (hover_pos.x - rect.left()) / plot_width;
                                            let rel_y = (hover_pos.y - rect.top()) / plot_height;
                                            if (0.0..1.0).contains(&rel_x) && (0.0..1.0).contains(&rel_y) {
                                                let col = (rel_x * grid.cols as f32) as usize;
                                                let row = (rel_y * grid.rows as f32) as usize;
                                                let col = col.min(grid.cols - 1);
                                                let row = row.min(grid.rows - 1);
                                                let idx = row * grid.cols + col;
                                                if grid.points[idx].is_some() {
                                                    new_hovered = Some(idx);
                                                } else {
                                                    // Snap to nearest valid cell (search radius 3)
                                                    let mut best_idx = None;
                                                    let mut best_dist_sq = u32::MAX;
                                                    for dr in -3i32..=3 {
                                                        for dc in -3i32..=3 {
                                                            let nr = row as i32 + dr;
                                                            let nc = col as i32 + dc;
                                                            if nr >= 0 && nr < grid.rows as i32 && nc >= 0 && nc < grid.cols as i32 {
                                                                let ni = nr as usize * grid.cols + nc as usize;
                                                                if grid.points[ni].is_some() {
                                                                    let dist_sq = (dr * dr + dc * dc) as u32;
                                                                    if dist_sq < best_dist_sq {
                                                                        best_dist_sq = dist_sq;
                                                                        best_idx = Some(ni);
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    new_hovered = best_idx;
                                                }
                                            }
                                        }
                                        self.porkchop_hovered = new_hovered;

                                        // Click to lock selection
                                        if response.clicked() {
                                            if let Some(hovered) = self.porkchop_hovered {
                                                self.porkchop_selected = Some(hovered);
                                            }
                                        }

                                        // Draw highlight on active point (selected or hovered)
                                        let active_idx = self.porkchop_hovered.or(self.porkchop_selected).or(grid.best_idx);
                                        if let Some(idx) = active_idx {
                                            let row = idx / grid.cols;
                                            let col = idx % grid.cols;
                                            let x = rect.left() + col as f32 * cell_w;
                                            let y = rect.top() + row as f32 * cell_h;
                                            let highlight_rect = egui::Rect::from_min_size(
                                                egui::pos2(x, y),
                                                egui::vec2(cell_w, cell_h),
                                            );
                                            painter.rect_stroke(highlight_rect, 0.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                                        }

                                        // Draw best point marker
                                        if let Some(best) = grid.best_idx {
                                            let row = best / grid.cols;
                                            let col = best % grid.cols;
                                            let cx = rect.left() + (col as f32 + 0.5) * cell_w;
                                            let cy = rect.top() + (row as f32 + 0.5) * cell_h;
                                            painter.circle_stroke(egui::pos2(cx, cy), 4.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                                        }

                                        // Axis labels
                                        let dep_range_days = (grid.dep_end - grid.dep_start) / 86400.0;
                                        // Bottom: departure time labels
                                        for i in 0..=4 {
                                            let frac = i as f32 / 4.0;
                                            let day = frac as f64 * dep_range_days;
                                            let x = rect.left() + frac * plot_width;
                                            painter.text(
                                                egui::pos2(x, rect.bottom() + 2.0),
                                                egui::Align2::CENTER_TOP,
                                                format!("{:.0}d", day),
                                                egui::FontId::proportional(8.0),
                                                egui::Color32::from_rgb(150, 150, 150),
                                            );
                                        }
                                        // Left: transfer time labels (log scale)
                                        let log_ratio = (grid.tof_max / grid.tof_min).ln();
                                        for i in 0..=4 {
                                            let frac = i as f32 / 4.0;
                                            let tof = grid.tof_min * (frac as f64 * log_ratio).exp();
                                            let label = if tof >= 86400.0 * 365.25 {
                                                format!("{:.1}y", tof / (86400.0 * 365.25))
                                            } else {
                                                format!("{:.0}d", tof / 86400.0)
                                            };
                                            let y = rect.top() + frac * plot_height;
                                            painter.text(
                                                egui::pos2(rect.left() - 2.0, y),
                                                egui::Align2::RIGHT_CENTER,
                                                label,
                                                egui::FontId::proportional(8.0),
                                                egui::Color32::from_rgb(150, 150, 150),
                                            );
                                        }

                                        ui.add_space(14.0); // Space for bottom axis labels
                                    } else if self.porkchop_computing {
                                        ui.label(egui::RichText::new("Computing...").size(10.0).color(egui::Color32::GRAY));
                                    }
                                }

                                // Display results
                                if let Some(ref display) = self.transfer_display {
                                    if display.valid {
                                        ui.separator();
                                        let fmt_dv = |dv: f64| -> String {
                                            if dv >= 1000.0 { format!("{:.2} km/s", dv / 1000.0) }
                                            else { format!("{:.1} m/s", dv) }
                                        };
                                        let fmt_time = |t: f64| -> String {
                                            if t >= 86400.0 * 365.25 {
                                                format!("{:.1}y", t / (86400.0 * 365.25))
                                            } else if t >= 86400.0 {
                                                format!("{:.1}d", t / 86400.0)
                                            } else if t >= 3600.0 {
                                                format!("{:.1}h", t / 3600.0)
                                            } else {
                                                format!("{:.0}s", t)
                                            }
                                        };

                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("Departure dv:").size(11.0).color(egui::Color32::GRAY));
                                            ui.label(egui::RichText::new(fmt_dv(display.departure_dv)).size(11.0).color(egui::Color32::WHITE));
                                        });
                                        if display.mode == 0 {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("Arrival dv:").size(11.0).color(egui::Color32::GRAY));
                                                ui.label(egui::RichText::new(fmt_dv(display.arrival_dv)).size(11.0).color(egui::Color32::WHITE));
                                            });
                                        } else {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("Arrival v_inf:").size(11.0).color(egui::Color32::GRAY));
                                                ui.label(egui::RichText::new(fmt_dv(display.arrival_dv)).size(11.0).color(egui::Color32::WHITE));
                                            });
                                        }
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("Transfer time:").size(11.0).color(egui::Color32::GRAY));
                                            ui.label(egui::RichText::new(fmt_time(display.transfer_time)).size(11.0).color(egui::Color32::WHITE));
                                        });

                                        // Phase angle display
                                        let phase_diff = (display.current_phase_angle - display.required_phase_angle).abs();
                                        let phase_color = if phase_diff < 5.0 {
                                            egui::Color32::from_rgb(100, 220, 100)
                                        } else if phase_diff < 20.0 {
                                            egui::Color32::from_rgb(220, 220, 100)
                                        } else {
                                            egui::Color32::from_rgb(200, 200, 200)
                                        };
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new("Phase angle:").size(11.0).color(egui::Color32::GRAY));
                                            ui.label(egui::RichText::new(format!("{:.1}", display.current_phase_angle)).size(11.0).color(phase_color));
                                            ui.label(egui::RichText::new(format!("/ {:.1}", display.required_phase_angle)).size(11.0).color(egui::Color32::GRAY));
                                        });
                                        if display.mode == 1 {
                                            // Lambert: always show time to departure node
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("Time to node:").size(11.0).color(egui::Color32::GRAY));
                                                ui.label(egui::RichText::new(fmt_time(display.time_to_window)).size(11.0).color(egui::Color32::WHITE));
                                            });
                                        } else if display.time_to_window > 60.0 {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new("Window in:").size(11.0).color(egui::Color32::GRAY));
                                                ui.label(egui::RichText::new(fmt_time(display.time_to_window)).size(11.0).color(egui::Color32::WHITE));
                                            });
                                        }

                                        ui.add_space(6.0);
                                        let create_btn = egui::Button::new(
                                            egui::RichText::new("Create Node").size(12.0).color(egui::Color32::WHITE)
                                        ).fill(egui::Color32::from_rgb(60, 120, 60));
                                        if ui.add(create_btn).clicked() {
                                            self.render_requests.push(
                                                crate::render::types::RenderRequest::TransferNode {
                                                    position_angle: display.departure_position_angle,
                                                    prograde_dv: display.prograde_dv,
                                                    radial_dv: display.radial_dv,
                                                    time_to_window: display.time_to_window,
                                                },
                                            );
                                            close_planner = true;
                                        }
                                    } else {
                                        ui.label(egui::RichText::new("No valid transfer found").size(11.0).color(egui::Color32::from_rgb(220, 100, 100)));
                                    }
                                }
                            });
                    });
                if close_planner {
                    self.transfer_planner_open = false;
                }
            }

            // Debug menu window
            if self.debug_menu_open {
                egui::Window::new("Debug")
                    .collapsible(false)
                    .resizable(false)
                    .default_pos(egui::pos2(60.0, 200.0))
                    .show(ctx, |ui| {
                        // Infinite fuel toggle
                        let fuel_label = if self.debug_infinite_fuel { "Infinite Fuel: ON" } else { "Infinite Fuel: OFF" };
                        let fuel_color = if self.debug_infinite_fuel {
                            egui::Color32::from_rgb(60, 130, 60)
                        } else {
                            egui::Color32::from_rgb(80, 80, 90)
                        };
                        if ui.add(egui::Button::new(fuel_label).fill(fuel_color).min_size(egui::vec2(160.0, 24.0))).clicked() {
                            self.debug_infinite_fuel = !self.debug_infinite_fuel;
                        }

                        ui.add_space(4.0);

                        // Teleport to LEO
                        if ui.add(egui::Button::new("Set Orbit (LEO)").min_size(egui::vec2(160.0, 24.0))).clicked() {
                            self.render_requests.push(
                                crate::render::types::RenderRequest::DebugTeleportLeo,
                            );
                        }

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Teleport to landed on any body
                        ui.label("Land on body:");
                        egui::ComboBox::from_id_source("debug_land_body")
                            .selected_text("Select...")
                            .width(160.0)
                            .show_ui(ui, |ui| {
                                for (i, name) in body_names.iter().enumerate() {
                                    if ui.selectable_label(false, name).clicked() {
                                        self.render_requests.push(
                                            crate::render::types::RenderRequest::DebugTeleportBody { body_index: i },
                                        );
                                    }
                                }
                            });
                    });
            }

            // Pause overlay (drawn on top of everything)
            if paused {
                egui::Area::new(egui::Id::new("pause_overlay"))
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                            .inner_margin(egui::Margin::same(40.0))
                            .rounding(egui::Rounding::same(8.0))
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    if self.show_quicksave_list {
                                        // Quicksave list view
                                        ui.heading(egui::RichText::new("Load Quicksave").size(28.0).color(egui::Color32::WHITE));
                                        ui.add_space(12.0);
                                        egui::ScrollArea::vertical()
                                            .max_height(300.0)
                                            .show(ui, |ui| {
                                                for qs in quicksaves {
                                                    let label = format!(
                                                        "#{} — {}",
                                                        qs.index,
                                                        crate::game::format_date(qs.simulation_time),
                                                    );
                                                    if ui.button(egui::RichText::new(&label).size(16.0)).clicked() {
                                                        pause_action = PauseAction::LoadQuicksave(qs.filename.clone());
                                                        self.show_quicksave_list = false;
                                                    }
                                                }
                                            });
                                        ui.add_space(12.0);
                                        if ui.button(egui::RichText::new("Back").size(18.0)).clicked() {
                                            self.show_quicksave_list = false;
                                        }
                                    } else {
                                        // Default pause view
                                        ui.heading(egui::RichText::new("Paused").size(32.0).color(egui::Color32::WHITE));
                                        ui.add_space(20.0);
                                        if ui.button(egui::RichText::new("Quicksave").size(18.0)).clicked() {
                                            pause_action = PauseAction::Quicksave;
                                        }
                                        if !quicksaves.is_empty() {
                                            ui.add_space(8.0);
                                            if ui.button(egui::RichText::new("Load Quicksave").size(18.0)).clicked() {
                                                self.show_quicksave_list = true;
                                            }
                                        }
                                        if has_launch_save {
                                            ui.add_space(8.0);
                                            let revert_btn = egui::Button::new(
                                                egui::RichText::new("Revert to Launch").size(18.0)
                                            ).fill(egui::Color32::from_rgb(180, 120, 40));
                                            if ui.add(revert_btn).clicked() {
                                                pause_action = PauseAction::RevertToLaunch;
                                            }
                                            ui.add_space(8.0);
                                            let revert_editor_btn = egui::Button::new(
                                                egui::RichText::new("Revert to Editor").size(18.0)
                                            ).fill(egui::Color32::from_rgb(180, 120, 40));
                                            if ui.add(revert_editor_btn).clicked() {
                                                pause_action = PauseAction::RevertToEditor;
                                            }
                                        }
                                        if can_recover {
                                            ui.add_space(8.0);
                                            let recover_btn = egui::Button::new(
                                                egui::RichText::new("Recover Vessel").size(18.0)
                                            ).fill(egui::Color32::from_rgb(60, 130, 60));
                                            if ui.add(recover_btn).clicked() {
                                                pause_action = PauseAction::RecoverVessel;
                                            }
                                        }
                                        ui.add_space(8.0);
                                        if can_exit_flight {
                                            if ui.button(egui::RichText::new("Main Menu").size(18.0)).clicked() {
                                                pause_action = PauseAction::MainMenu;
                                            }
                                        } else {
                                            ui.add_enabled(false, egui::Button::new(egui::RichText::new("Main Menu").size(18.0)));
                                            ui.label(egui::RichText::new("Cannot exit while in atmosphere or landing zone")
                                                .size(11.0)
                                                .color(egui::Color32::from_rgb(200, 150, 100)));
                                        }
                                    }
                                });
                            });
                    });
            }

            // Toast notifications
            render_toasts(ctx, &active_toasts);

            super::state::fps_overlay(ctx, fps);
        });

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);

        // Handle maneuver node UI actions
        if let Some((ta, ref segment)) = create_node_at {
            self.create_maneuver_node(ta, segment);
        }
        if let Some(node_id) = delete_node_id {
            // Cancel warp-to-node if the first node is being deleted
            if self.maneuver_nodes.first().map(|n| n.id) == Some(node_id) {
                self.warp_to_node = false;
            }
            self.delete_maneuver_node(node_id);
        }
        if close_maneuver_panel {
            self.selected_maneuver_node = None;
        }
        if start_warp_to_node {
            self.warp_to_node = true;
        }
        if cancel_warp_to_node {
            self.warp_to_node = false;
        }
        // Update autopilot target
        self.autopilot_target = new_autopilot_target;
        // Enqueue part-interaction requests for main.rs to process.
        use crate::render::types::RenderRequest;
        if let Some((part_index, enabled)) = engine_toggle_req {
            self.render_requests.push(RenderRequest::EngineToggle { part_index, enabled });
        }
        if let Some((part_index, enabled)) = crossfeed_toggle_req {
            self.render_requests.push(RenderRequest::CrossfeedToggle { part_index, enabled });
        }
        if let Some(part_index) = decouple_req {
            self.render_requests.push(RenderRequest::Decouple { part_index });
        }
        if let Some(part_index) = fairing_deploy_req {
            self.render_requests.push(RenderRequest::FairingDeploy { part_index });
        }
        if let Some((part_index, deploy)) = solar_deploy_req {
            self.render_requests.push(RenderRequest::SolarDeploy { part_index, deploy });
        }
        if let Some((part_index, deploy)) = radiator_deploy_req {
            self.render_requests.push(RenderRequest::RadiatorDeploy { part_index, deploy });
        }
        if let Some((part_index, active)) = shield_toggle_req {
            self.render_requests.push(RenderRequest::ShieldToggle { part_index, active });
        }
        if reactor_restart_req {
            self.render_requests.push(RenderRequest::ReactorRestart);
        }
        if let Some(part_index) = parachute_deploy_req {
            self.render_requests.push(RenderRequest::ParachuteDeploy { part_index });
        }
        if let Some(part_index) = parachute_cut_req {
            self.render_requests.push(RenderRequest::ParachuteCut { part_index });
        }
        // Store staging reorder request for main.rs to process
        if staging_reorder_req.is_some() {
            self.staging_reorder = staging_reorder_req;
        }
        // Apply slider deltas to selected node with non-linear scaling
        // Full deflection = 1000 m/s per second, minimal = ~1 m/s per second
        if let Some(node_id) = self.selected_maneuver_node {
            if prograde_delta.abs() > 0.001 || radial_delta.abs() > 0.001 {
                if let Some(node) = self.get_maneuver_node_mut(node_id) {
                    // Non-linear scaling: use power of 2 for smooth precision control
                    // At 60fps: max deflection (100) = ~16.67 m/s/frame = 1000 m/s/s
                    // Small deflection (~3%) = ~0.017 m/s/frame = ~1 m/s/s
                    let apply_curve = |delta: f64| -> f64 {
                        let normalized = delta / 100.0; // -1 to 1
                        let curved = normalized.signum() * normalized.abs().powf(2.0);
                        curved * 16.67 // Scale to m/s per frame
                    };
                    let prograde_change = apply_curve(prograde_delta);
                    let radial_change = apply_curve(radial_delta);
                    // Update both original (for trajectory) and remaining (for display)
                    node.delta_v.prograde += prograde_change;
                    node.delta_v.radial_out += radial_change;
                    node.remaining_delta_v.prograde += prograde_change;
                    node.remaining_delta_v.radial_out += radial_change;
                }
            }
        }

        // Update maneuver node screen positions for click detection
        self.maneuver_node_screen_positions.clear();
        let scale_factor = self.window.scale_factor() as f32;
        for node in &self.maneuver_nodes {
            // Two-step precision: (parent - body_center) + (orbit_offset - ship_offset)
            if let Some(parent) = self.bodies.get(node.parent_idx) {
                let off = node.orbit_offset();
                let rel_x = ((parent.x - self.camera.body_center[0]) + off[0] - self.camera.ship_offset[0]) as f32;
                let rel_y = ((parent.y - self.camera.body_center[1]) + off[1] - self.camera.ship_offset[1]) as f32;
                let cos_r = self.camera.rotation.cos();
                let sin_r = self.camera.rotation.sin();
                let rot_x = rel_x * cos_r - rel_y * sin_r;
                let rot_y = rel_x * sin_r + rel_y * cos_r;
                let view_x = rot_x * self.camera.zoom;
                let view_y = rot_y * self.camera.zoom;
                let ndc_x = view_x / self.camera.aspect_ratio;
                let ndc_y = view_y;
                let scr_x = (ndc_x + 1.0) * 0.5 * self.size.width as f32 / scale_factor;
                let scr_y = (1.0 - ndc_y) * 0.5 * self.size.height as f32 / scale_factor;
                self.maneuver_node_screen_positions.push((node.id, [scr_x, scr_y]));
            }
        }

        let tris = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen_descriptor);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,        // Render to MSAA texture
                    resolve_target: Some(&view),  // Resolve to swapchain
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.body_texture_bind_group, &[]);
            render_pass.set_bind_group(2, &self.sprite_atlas.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

        // Render egui on top (separate pass without MSAA)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Keep the previous render
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.egui_renderer.render(&mut render_pass, &tris, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok((new_warp_index, pause_action))
    }
}

/// Render toast notifications as fading overlays at the top-center of the screen.
pub(super) fn render_toasts(ctx: &egui::Context, toasts: &[(String, web_time::Instant)]) {
    let now = web_time::Instant::now();
    for (i, (msg, created)) in toasts.iter().enumerate() {
        let elapsed = now.duration_since(*created).as_secs_f32();
        if elapsed > 5.0 {
            continue;
        }
        // Fade out over last second
        let alpha = if elapsed > 4.0 {
            ((5.0 - elapsed) * 255.0) as u8
        } else {
            255
        };
        let y_offset = 40.0 + i as f32 * 30.0;
        egui::Area::new(egui::Id::new("toast").with(i))
            .anchor(egui::Align2::CENTER_TOP, egui::Vec2::new(0.0, y_offset))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 60, alpha))
                    .inner_margin(egui::Margin::symmetric(12.0, 6.0))
                    .rounding(egui::Rounding::same(4.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(msg)
                                .color(egui::Color32::from_rgba_unmultiplied(255, 255, 200, alpha))
                                .size(13.0),
                        );
                    });
            });
    }
}
