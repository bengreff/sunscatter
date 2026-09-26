use egui_wgpu::ScreenDescriptor;

use super::colony_ui::ColonyScreenAction;
use super::formatting::{format_duration, format_distance, format_mass, format_pressure};
use super::types::{
    BodyInfoData, ColonyOverviewAction, MainMenuAction, ManagementAction, PauseAction,
    TechTreeScreenAction, TitleScreenAction, TrackingStationAction, TrackingVesselData,
};
use super::state::RenderState;

impl RenderState {
    /// Render the main menu: planets + centered menu buttons + time warp panel.
    /// Returns a MainMenuAction if the user clicked a button.
    pub fn render_main_menu(
        &mut self,
        warp_levels: &[f64],
        current_warp_index: usize,
        date_str: &str,
        egui_callback: impl FnOnce(&egui::Context) -> MainMenuAction,
    ) -> Result<(usize, MainMenuAction), wgpu::SurfaceError> {
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut menu_action = MainMenuAction::None;
        let mut new_warp_index = current_warp_index;

        let fps = self.fps;
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            // Time warp panel at top
            egui::TopBottomPanel::top("time_warp_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Time Warp:");
                    for (i, &warp) in warp_levels.iter().enumerate() {
                        let label = if warp >= 1_000_000_000.0 {
                            format!("{}B", (warp / 1_000_000_000.0) as i32)
                        } else if warp >= 1_000_000.0 {
                            format!("{}M", (warp / 1_000_000.0) as i32)
                        } else if warp >= 1000.0 {
                            format!("{}K", (warp / 1000.0) as i32)
                        } else {
                            format!("{}x", warp as i32)
                        };
                        let is_selected = i == current_warp_index;
                        if ui.selectable_label(is_selected, &label).clicked() {
                            new_warp_index = i;
                        }
                    }
                    ui.separator();
                    let current_warp = warp_levels[current_warp_index];
                    ui.label(format!("Current: {}x", current_warp as i64));

                    ui.separator();
                    ui.label(date_str);
                });
            });

            menu_action = egui_callback(ctx);
            super::state::fps_overlay(ctx, fps);
        });

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);
        let tris = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Main Menu Render Encoder"),
        });
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen_descriptor);

        // Geometry pass (planets/orbits already in self.vertex_buffer)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Menu Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
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

        // Egui pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Menu Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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

        Ok((new_warp_index, menu_action))
    }

    /// Render the title screen: planets + egui overlay (no time warp bar).
    pub fn render_title_screen(
        &mut self,
        egui_callback: impl FnOnce(&egui::Context) -> TitleScreenAction,
    ) -> Result<TitleScreenAction, wgpu::SurfaceError> {
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut action = TitleScreenAction::None;

        let fps = self.fps;
        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            // No time warp panel — just the callback
            action = egui_callback(ctx);
            super::state::fps_overlay(ctx, fps);
        });

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);
        let tris = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Title Screen Render Encoder"),
        });
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen_descriptor);

        // Geometry pass (planets/orbits already in self.vertex_buffer)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Title Screen Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
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

        // Egui pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Title Screen Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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

        Ok(action)
    }

    /// Render tracking station: planets + time warp panel + vessel list.
    /// Returns the new warp index, pause action, and tracking station action.
    pub fn render_tracking_station(
        &mut self,
        body_names: &[String],
        warp_levels: &[f64],
        current_warp_index: usize,
        paused: bool,
        date_str: &str,
        vessels: &[TrackingVesselData],
        body_info: &[BodyInfoData],
        colony_manager: &crate::colony::ColonyManager,
    ) -> Result<(usize, PauseAction, TrackingStationAction), wgpu::SurfaceError> {
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut new_warp_index = current_warp_index;
        let mut pause_action = PauseAction::None;
        let mut ts_action = TrackingStationAction::None;
        let hovered = self.hovered_body;
        let bodies_copy = self.bodies.clone();
        let size = self.size;
        let camera_pos = self.camera.position;
        let camera_zoom = self.camera.zoom;
        let camera_rotation = self.camera.rotation;
        let aspect_ratio = self.camera.aspect_ratio;
        let scale_factor = self.window.scale_factor() as f32;

        // Procedural star hover state: capture name and screen pos for label
        let hovered_star_label: Option<(String, [f32; 2])> = self.hovered_star.and_then(|idx| {
            let name = self.current_procedural_stars.get(idx).map(|s| s.format_name())?;
            let screen_pos = self.procedural_star_screen_positions.iter()
                .find(|&&(i, _)| i == idx)
                .map(|&(_, pos)| pos)?;
            Some((name, screen_pos))
        });

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            // Time warp panel at top
            egui::TopBottomPanel::top("time_warp_panel").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Time Warp:");
                    for (i, &warp) in warp_levels.iter().enumerate() {
                        let label = if warp >= 1_000_000_000.0 {
                            format!("{}B", (warp / 1_000_000_000.0) as i32)
                        } else if warp >= 1_000_000.0 {
                            format!("{}M", (warp / 1_000_000.0) as i32)
                        } else if warp >= 1000.0 {
                            format!("{}K", (warp / 1000.0) as i32)
                        } else {
                            format!("{}x", warp as i32)
                        };
                        let is_selected = i == current_warp_index;
                        if ui.selectable_label(is_selected, &label).clicked() {
                            new_warp_index = i;
                        }
                    }
                    ui.separator();
                    let current_warp = warp_levels[current_warp_index];
                    ui.label(format!("Current: {}x", current_warp as i64));

                    ui.separator();
                    ui.label(date_str);
                });
            });

            // Vessels & Colonies sidebar
            let has_sidebar_content = !vessels.is_empty() || !colony_manager.colonies.is_empty();
            if has_sidebar_content {
                egui::SidePanel::left("vessels_panel")
                    .default_width(180.0)
                    .resizable(false)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            // Vessels section
                            if !vessels.is_empty() {
                                ui.heading(egui::RichText::new("Vessels").size(16.0).color(egui::Color32::WHITE));
                                ui.separator();

                                for vessel in vessels {
                                    let body_name = body_names.get(vessel.soi_body)
                                        .cloned()
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            let name_color = if vessel.is_debris {
                                                egui::Color32::from_rgb(140, 140, 140)
                                            } else {
                                                egui::Color32::WHITE
                                            };
                                            let name_text = egui::RichText::new(&vessel.name)
                                                .color(name_color)
                                                .size(13.0);
                                            if ui.add(egui::Label::new(name_text).sense(egui::Sense::click())).clicked() {
                                                ts_action = TrackingStationAction::FocusVessel(vessel.id);
                                            }
                                            let mut info = format!("SOI: {}", body_name);
                                            if let Some(ref orbit) = vessel.orbit {
                                                if orbit.eccentricity < 1.0 && orbit.semi_major_axis > 0.0 {
                                                    let body_radius = body_info.get(vessel.soi_body)
                                                        .map(|b| b.radius_m).unwrap_or(0.0);
                                                    let ap = orbit.semi_major_axis * (1.0 + orbit.eccentricity) - body_radius;
                                                    let pe = orbit.semi_major_axis * (1.0 - orbit.eccentricity) - body_radius;
                                                    if ap > 0.0 && pe > -body_radius * 0.5 {
                                                        let fmt = |v: f64| -> String {
                                                            if v >= 1e6 { format!("{:.0}Mm", v/1e6) }
                                                            else { format!("{:.0}km", v/1000.0) }
                                                        };
                                                        info = format!("{} • {} / {}", body_name, fmt(ap), fmt(pe));
                                                    }
                                                }
                                            }
                                            ui.label(egui::RichText::new(info)
                                                .size(10.0)
                                                .color(egui::Color32::GRAY));
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if !vessel.is_debris {
                                                if ui.small_button("Fly").clicked() {
                                                    ts_action = TrackingStationAction::FlyVessel(vessel.id);
                                                }
                                            }
                                            let delete_btn = egui::Button::new(
                                                egui::RichText::new("X").color(egui::Color32::from_rgb(200, 80, 80))
                                            ).small();
                                            if ui.add(delete_btn).on_hover_text("Delete vessel").clicked() {
                                                ts_action = TrackingStationAction::DeleteVessel(vessel.id);
                                            }
                                        });
                                    });
                                    ui.add_space(4.0);
                                }
                            }

                            // Colonies section
                            if !colony_manager.colonies.is_empty() {
                                ui.add_space(8.0);
                                ui.heading(egui::RichText::new("Colonies").size(16.0).color(egui::Color32::WHITE));
                                ui.separator();

                                for colony in &colony_manager.colonies {
                                    let body_name = body_names.get(colony.body_index)
                                        .cloned()
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            let name_text = egui::RichText::new(&colony.name)
                                                .color(egui::Color32::WHITE)
                                                .size(13.0);
                                            if ui.add(egui::Label::new(name_text).sense(egui::Sense::click())).clicked() {
                                                ts_action = TrackingStationAction::FocusBody(colony.body_index);
                                            }
                                            ui.label(egui::RichText::new(format!("{}, {} crew", body_name, colony.crew))
                                                .size(11.0)
                                                .color(egui::Color32::GRAY));
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.small_button("Open").clicked() {
                                                ts_action = TrackingStationAction::OpenColony(colony.body_index);
                                            }
                                        });
                                    });
                                    ui.add_space(4.0);
                                }
                            }
                        });
                    });
            }

            // Unified info panel (body or focused procedural star)
            let panel_info: Option<&BodyInfoData> = if let Some(idx) = self.tracked_body {
                body_info.get(idx)
                    .or_else(|| self.catalog_body_info.get(&idx))
            } else {
                self.focused_star_info.as_ref()
            };
            if let Some(info) = panel_info {
                egui::SidePanel::right("body_info_panel")
                    .default_width(220.0)
                    .resizable(false)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let is_star = info.star_type.is_some();
                            // Name — light blue for stars, white otherwise
                            let name_color = if is_star {
                                egui::Color32::from_rgb(200, 200, 255)
                            } else {
                                egui::Color32::WHITE
                            };
                            ui.heading(egui::RichText::new(&info.name).size(18.0).color(name_color));

                            // Star type label (italic, light blue)
                            if let Some(ref st) = info.star_type {
                                ui.label(egui::RichText::new(st)
                                    .size(12.0)
                                    .italics()
                                    .color(egui::Color32::from_rgb(160, 160, 200)));
                            }
                            // Description (italic, gray)
                            if !info.description.is_empty() {
                                ui.label(egui::RichText::new(&info.description)
                                    .size(12.0)
                                    .italics()
                                    .color(egui::Color32::from_rgb(160, 160, 160)));
                            }
                            ui.separator();

                            // Physical properties (skip for barycenters where radius_m == 0)
                            if info.radius_m > 0.0 {
                                ui.label(egui::RichText::new("Physical Properties").size(13.0).color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.add_space(2.0);
                                const R_SUN: f64 = 6.957e8;
                                const M_SUN: f64 = 1.989e30;
                                if is_star {
                                    // Stars: radius in solar radii + metric
                                    let radius_solar = info.radius_m / R_SUN;
                                    if radius_solar >= 1.0 {
                                        ui.label(format!("Radius: {:.2} R\u{2609}", radius_solar));
                                    } else {
                                        ui.label(format!("Radius: {:.4} R\u{2609}", radius_solar));
                                    }
                                    ui.label(format!("  ({})", format_distance(info.radius_m)));
                                } else {
                                    ui.label(format!("Radius: {}", format_distance(info.radius_m)));
                                }
                                ui.label(format!("Surface gravity: {:.2} m/s\u{b2}", info.surface_gravity_ms2));
                                if is_star {
                                    let mass_solar = info.mass_kg / M_SUN;
                                    ui.label(format!("Mass: {:.2} M\u{2609}", mass_solar));
                                } else {
                                    ui.label(format!("Mass: {}", format_mass(info.mass_kg)));
                                }

                                // Stellar properties (for stars)
                                if is_star {
                                    ui.add_space(4.0);
                                    ui.separator();
                                    ui.label(egui::RichText::new("Stellar Properties").size(13.0).color(egui::Color32::from_rgb(200, 200, 200)));
                                    ui.add_space(2.0);
                                    if let Some(lum) = info.luminosity_solar {
                                        if lum >= 1.0 {
                                            ui.label(format!("Luminosity: {:.1} L\u{2609}", lum));
                                        } else {
                                            ui.label(format!("Luminosity: {:.4} L\u{2609}", lum));
                                        }
                                    }
                                    if let Some(temp) = info.temperature_k {
                                        ui.label(format!("Temperature: {:.0} K", temp));
                                    }
                                    if let Some(soi) = info.soi_radius_m {
                                        ui.label(format!("SOI: {}", format_distance(soi)));
                                    }
                                }
                            }

                            // Component Stars section (for multi-star system barycenters)
                            if !info.catalog_stars.is_empty() {
                                ui.add_space(4.0);
                                ui.separator();
                                ui.label(egui::RichText::new("Component Stars").size(13.0).color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.add_space(2.0);
                                for cs in &info.catalog_stars {
                                    ui.label(egui::RichText::new(&cs.name)
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(180, 200, 255)));
                                    ui.label(egui::RichText::new(format!(
                                        "  {} | {:.2} M\u{2609} | {:.4} L\u{2609}",
                                        cs.spectral_type, cs.mass_solar, cs.luminosity_solar
                                    ))
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(140, 140, 140)));
                                }
                            }

                            // Moons section for non-star bodies with children
                            if !is_star && !info.catalog_planets.is_empty() {
                                ui.add_space(4.0);
                                ui.separator();
                                let moon_count = info.catalog_planets.len();
                                ui.label(egui::RichText::new(format!("Moons ({})", moon_count))
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.add_space(2.0);

                                for body in &info.catalog_planets {
                                    let name_color = if body.has_life {
                                        egui::Color32::from_rgb(255, 215, 80) // gold
                                    } else if body.habitability > 30 {
                                        egui::Color32::from_rgb(140, 220, 140) // green
                                    } else {
                                        egui::Color32::from_rgb(180, 180, 180) // gray
                                    };

                                    let name_str = if body.designation.is_empty() {
                                        body.name.clone()
                                    } else if body.name.is_empty() {
                                        body.designation.clone()
                                    } else {
                                        format!("{} \"{}\"", body.designation, body.name)
                                    };

                                    ui.label(egui::RichText::new(name_str)
                                        .size(12.0)
                                        .color(name_color));

                                    let temp_str = if body.temperature_k > 0.0 {
                                        format!(" | {} K", body.temperature_k as i64)
                                    } else {
                                        String::new()
                                    };
                                    if body.is_gas_giant {
                                        ui.label(egui::RichText::new(format!("  Gas giant{}", temp_str))
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(140, 140, 140)));
                                    } else {
                                        let atm_str = if body.has_atmosphere { "atm" } else { "no atm" };
                                        let life_str = if body.has_life { " | LIFE" } else { "" };
                                        ui.label(egui::RichText::new(format!(
                                            "  {:.2}g{} | {} | hab {}/100{}",
                                            body.gravity_g, temp_str,
                                            atm_str, body.habitability, life_str
                                        ))
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(140, 140, 140)));
                                    }
                                }
                            }

                            // Catalog star system info
                            if is_star && (info.catalog_zone.is_some() || !info.catalog_planets.is_empty()) {
                                ui.add_space(4.0);
                                ui.separator();
                                ui.label(egui::RichText::new("System Info").size(13.0).color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.add_space(2.0);
                                if let Some(zone) = info.catalog_zone {
                                    ui.label(format!("Zone: {}", zone));
                                }
                                if let Some(dist) = info.catalog_distance_ly {
                                    ui.label(format!("Distance from Sol: {:.2} ly", dist));
                                }
                                if let Some(ref spec) = info.catalog_spectral {
                                    ui.label(format!("Spectral: {}", spec));
                                }

                                // Planetary system listing
                                if !info.catalog_planets.is_empty() {
                                    ui.add_space(4.0);
                                    ui.separator();
                                    let planet_count = info.catalog_planets.iter().filter(|p| !p.is_moon).count();
                                    let moon_count = info.catalog_planets.iter().filter(|p| p.is_moon).count();
                                    let label = if moon_count > 0 {
                                        format!("Planetary System ({} planets, {} moons)", planet_count, moon_count)
                                    } else {
                                        format!("Planetary System ({} planets)", planet_count)
                                    };
                                    ui.label(egui::RichText::new(label).size(13.0).color(egui::Color32::from_rgb(200, 200, 200)));
                                    ui.add_space(2.0);

                                    for body in &info.catalog_planets {
                                        let indent = if body.is_moon { "    " } else { "" };
                                        // Color coding: green for habitable, gold for life
                                        let name_color = if body.has_life {
                                            egui::Color32::from_rgb(255, 215, 80) // gold
                                        } else if body.habitability > 30 {
                                            egui::Color32::from_rgb(140, 220, 140) // green
                                        } else {
                                            egui::Color32::from_rgb(180, 180, 180) // gray
                                        };

                                        let name_str = if body.designation.is_empty() {
                                            body.name.clone()
                                        } else if body.name.is_empty() {
                                            body.designation.clone()
                                        } else {
                                            format!("{} \"{}\"", body.designation, body.name)
                                        };

                                        ui.label(egui::RichText::new(format!("{}{}", indent, name_str))
                                            .size(12.0)
                                            .color(name_color));

                                        let temp_str = if body.temperature_k > 0.0 {
                                            format!(" | {} K", body.temperature_k as i64)
                                        } else {
                                            String::new()
                                        };
                                        if body.is_gas_giant {
                                            ui.label(egui::RichText::new(format!("{}  Gas giant{}", indent, temp_str))
                                                .size(11.0)
                                                .color(egui::Color32::from_rgb(140, 140, 140)));
                                        } else {
                                            let atm_str = if body.has_atmosphere { "atm" } else { "no atm" };
                                            let life_str = if body.has_life { " | LIFE" } else { "" };
                                            ui.label(egui::RichText::new(format!(
                                                "{}  {:.2}g{} | {} | hab {}/100{}",
                                                indent, body.gravity_g, temp_str,
                                                atm_str, body.habitability, life_str
                                            ))
                                                .size(11.0)
                                                .color(egui::Color32::from_rgb(140, 140, 140)));
                                        }
                                    }
                                }
                            }

                            // Atmosphere (non-star bodies only)
                            if !is_star {
                                ui.add_space(4.0);
                                if let Some(pressure) = info.atmosphere_pressure_pa {
                                    ui.label(egui::RichText::new("Atmosphere").size(13.0).color(egui::Color32::from_rgb(200, 200, 200)));
                                    ui.add_space(2.0);
                                    ui.label(format!("Pressure: {}", format_pressure(pressure)));
                                    if let Some(height) = info.atmosphere_height_m {
                                        ui.label(format!("Height: {}", format_distance(height)));
                                    }
                                } else {
                                    ui.label(egui::RichText::new("No atmosphere")
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(120, 120, 120)));
                                }
                            }

                            // Orbit
                            if let Some(sma) = info.orbit_semi_major_axis_m {
                                ui.add_space(4.0);
                                ui.separator();
                                let orbit_label = if info.is_galactic_orbit { "Galactic Orbit" } else { "Orbit" };
                                ui.label(egui::RichText::new(orbit_label).size(13.0).color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.add_space(2.0);
                                if info.is_galactic_orbit {
                                    const KPC: f64 = 3.086e19; // 1 kpc in meters
                                    const AU: f64 = 1.496e11;
                                    let sma_kpc = sma / KPC;
                                    if sma_kpc >= 1.0 {
                                        ui.label(format!("Semi-major axis: {:.2} kpc", sma_kpc));
                                    } else if sma_kpc * 1000.0 >= 0.5 {
                                        ui.label(format!("Semi-major axis: {:.0} pc", sma_kpc * 1000.0));
                                    } else {
                                        ui.label(format!("Semi-major axis: {:.0} AU", sma / AU));
                                    }
                                } else {
                                    ui.label(format!("Semi-major axis: {}", format_distance(sma)));
                                }
                                if let Some(ecc) = info.orbit_eccentricity {
                                    if info.is_galactic_orbit {
                                        ui.label(format!("Eccentricity: {:.3}", ecc));
                                    } else {
                                        ui.label(format!("Eccentricity: {:.4}", ecc));
                                    }
                                }
                                if let Some(period) = info.orbit_period_s {
                                    ui.label(format!("Period: {}", format_duration(period)));
                                }
                            }

                            // SOI (non-star bodies)
                            if !is_star {
                                if let Some(soi) = info.soi_radius_m {
                                    ui.add_space(4.0);
                                    ui.label(format!("SOI: {}", format_distance(soi)));
                                }
                            }

                            // Colony prospects (non-star bodies)
                            if !is_star && (!info.mineable_resources.is_empty() || !info.atmospheric_resources.is_empty() || info.habitability_score > 0) {
                                ui.add_space(4.0);
                                ui.separator();
                                ui.label(egui::RichText::new("Colony Prospects").size(13.0).color(egui::Color32::from_rgb(200, 200, 200)));
                                ui.add_space(2.0);

                                ui.label(format!("Habitability: {}/100", info.habitability_score));

                                if !info.mineable_resources.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new("Mineable Resources")
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)));
                                    for res in &info.mineable_resources {
                                        ui.label(format!("  {}", res.display_name()));
                                    }
                                }

                                if !info.atmospheric_resources.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new("Atmospheric Resources")
                                        .size(12.0)
                                        .color(egui::Color32::from_rgb(180, 180, 180)));
                                    for res in &info.atmospheric_resources {
                                        ui.label(format!("  {}", res.display_name()));
                                    }
                                }
                            }
                        });
                    });
            }

            // Bottom panel: Tracking Station label
            egui::TopBottomPanel::bottom("tracking_station_label").show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(egui::RichText::new("Tracking Station").size(14.0).color(egui::Color32::from_rgb(180, 180, 180)));
                });
            });

            // Hovered body labels (same logic as flight)
            if let Some(idx) = hovered {
                if let Some(body) = bodies_copy.get(idx) {
                    if let Some(name) = body_names.get(idx) {
                        let rel_x = (body.x - camera_pos[0]) as f32;
                        let rel_y = (body.y - camera_pos[1]) as f32;
                        let cos_r = camera_rotation.cos();
                        let sin_r = camera_rotation.sin();
                        let rot_x = rel_x * cos_r - rel_y * sin_r;
                        let rot_y = rel_x * sin_r + rel_y * cos_r;
                        let ndc_x = rot_x * camera_zoom / aspect_ratio;
                        let ndc_y = rot_y * camera_zoom;
                        let screen_x = (ndc_x + 1.0) * 0.5 * size.width as f32 / scale_factor;
                        let screen_y = (1.0 - ndc_y) * 0.5 * size.height as f32 / scale_factor;
                        let label_y = screen_y - 20.0;

                        egui::Area::new(egui::Id::new("body_label"))
                            .fixed_pos(egui::pos2(screen_x, label_y))
                            .pivot(egui::Align2::CENTER_BOTTOM)
                            .show(ctx, |ui| {
                                ui.label(egui::RichText::new(name).color(egui::Color32::WHITE).size(14.0));
                            });
                    }
                }
            }

            // Hovered procedural star label
            if let Some((ref star_name, star_pos)) = hovered_star_label {
                let sx = star_pos[0] / scale_factor;
                let sy = star_pos[1] / scale_factor - 20.0;
                egui::Area::new(egui::Id::new("star_label"))
                    .fixed_pos(egui::pos2(sx, sy))
                    .pivot(egui::Align2::CENTER_BOTTOM)
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new(star_name).color(egui::Color32::from_rgb(200, 200, 255)).size(14.0));
                    });
            }

            // Pause overlay
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
                                    ui.heading(egui::RichText::new("Paused").size(32.0).color(egui::Color32::WHITE));
                                    ui.add_space(20.0);
                                    if ui.button(egui::RichText::new("Main Menu").size(18.0)).clicked() {
                                        pause_action = PauseAction::MainMenu;
                                    }
                                });
                            });
                    });
            }

            // Toast notifications
            super::flight::render_toasts(ctx, &self.active_toasts);

            super::state::fps_overlay(ctx, self.fps);
        });

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);
        let tris = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Tracking Station Render Encoder"),
        });
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen_descriptor);

        // Geometry pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Tracking Station Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
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

        // Egui pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Tracking Station Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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

        Ok((new_warp_index, pause_action, ts_action))
    }

    /// Render colony management screen: planets in background + full-screen colony UI.
    /// Returns the new warp index and colony screen action.
    pub fn render_colony(
        &mut self,
        body_names: &[String],
        warp_levels: &[f64],
        current_warp_index: usize,
        paused: bool,
        date_str: &str,
        body_index: usize,
        colony_manager: &crate::colony::ColonyManager,
        body_habitability: &[u32],
        body_radii: &[f64],
        body_mineable: &[Vec<crate::colony::ResourceType>],
        body_atmospheric: &[Vec<crate::colony::ResourceType>],
        can_return_to_flight: bool,
        solar_power_factor: f64,
        tech_tree: &crate::colony::TechTree,
        fleet: &crate::colony::FleetManager,
        earth_index: usize,
    ) -> Result<(usize, ColonyScreenAction), wgpu::SurfaceError> {
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut new_warp_index = current_warp_index;
        let mut colony_action = ColonyScreenAction::None;

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            let result = super::colony_ui::render_colony_screen(
                ctx,
                body_index,
                colony_manager,
                body_names,
                body_habitability,
                body_radii,
                body_mineable,
                body_atmospheric,
                warp_levels,
                current_warp_index,
                date_str,
                paused,
                can_return_to_flight,
                &self.active_toasts,
                solar_power_factor,
                tech_tree,
                fleet,
                earth_index,
            );
            // Extract warp change if that was the action
            if let ColonyScreenAction::ChangeWarp(idx) = &result {
                new_warp_index = *idx;
            }
            colony_action = result;
            super::state::fps_overlay(ctx, self.fps);
        });

        self.egui_state.handle_platform_output(&self.window, full_output.platform_output);
        let tris = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer.update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Colony Render Encoder"),
        });
        self.egui_renderer.update_buffers(&self.device, &self.queue, &mut encoder, &tris, &screen_descriptor);

        // Geometry pass (planets/orbits in background)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Colony Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
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

        // Egui pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Colony Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
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

        Ok((new_warp_index, colony_action))
    }

    /// Render colony overview screen: planets + full-screen colony list.
    pub fn render_colony_overview(
        &mut self,
        colony_manager: &crate::colony::ColonyManager,
        body_names: &[String],
        warp_levels: &[f64],
        current_warp_index: usize,
        paused: bool,
        date_str: &str,
        company_money: f64,
        science_available: f64,
        fleet: &crate::colony::FleetManager,
        earth_index: usize,
        blueprints: &crate::parts::BlueprintRegistry,
        part_defs: &crate::parts::PartDefinitions,
        solar_system: &crate::bodies::SolarSystem,
        sim_time: f64,
        dyson_swarms: &std::collections::HashMap<usize, crate::colony::DysonSwarm>,
        tech_tree: &crate::colony::TechTree,
    ) -> Result<(usize, ColonyOverviewAction), wgpu::SurfaceError> {
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut new_warp_index = current_warp_index;
        let mut overview_action = ColonyOverviewAction::None;

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            let result = super::colony_overview_ui::render_colony_overview_screen(
                ctx,
                colony_manager,
                body_names,
                warp_levels,
                current_warp_index,
                date_str,
                paused,
                company_money,
                science_available,
                &self.active_toasts,
                fleet,
                earth_index,
                &mut self.route_creation,
                blueprints,
                part_defs,
                solar_system,
                sim_time,
                dyson_swarms,
                tech_tree,
            );
            if let ColonyOverviewAction::ChangeWarp(idx) = &result {
                new_warp_index = *idx;
            }
            overview_action = result;
            super::state::fps_overlay(ctx, self.fps);
        });

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);
        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Colony Overview Render Encoder"),
            });
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &tris,
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Colony Overview Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&view),
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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Colony Overview Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.egui_renderer
                .render(&mut render_pass, &tris, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok((new_warp_index, overview_action))
    }

    /// Render management screen: planets + full-screen management UI.
    pub fn render_management(
        &mut self,
        company: &crate::colony::Company,
        science: &crate::colony::ScienceState,
        contracts: &crate::colony::ContractManager,
        rd_budget: f64,
        warp_levels: &[f64],
        current_warp_index: usize,
        paused: bool,
        date_str: &str,
    ) -> Result<(usize, ManagementAction, f64), wgpu::SurfaceError> {
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut new_warp_index = current_warp_index;
        let mut mgmt_action = ManagementAction::None;
        let mut new_budget = rd_budget;

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            let (act, bud) = super::management_ui::render_management_screen(
                ctx,
                company,
                science,
                contracts,
                rd_budget,
                warp_levels,
                current_warp_index,
                date_str,
                paused,
                &self.active_toasts,
            );
            mgmt_action = act;
            new_budget = bud;
            if let ManagementAction::ChangeWarp(idx) = act {
                new_warp_index = idx;
            }
            super::state::fps_overlay(ctx, self.fps);
        });

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);
        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Management Render Encoder"),
            });
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &tris,
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Management Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&view),
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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Management Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.egui_renderer
                .render(&mut render_pass, &tris, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok((new_warp_index, mgmt_action, new_budget))
    }

    /// Render full-screen tech tree: planets + tech tree graph.
    pub fn render_tech_tree_screen(
        &mut self,
        tech_tree: &mut crate::colony::TechTree,
        science: &mut crate::colony::ScienceState,
        warp_levels: &[f64],
        current_warp_index: usize,
        paused: bool,
        date_str: &str,
        part_defs: &crate::parts::PartDefinitions,
    ) -> Result<(usize, TechTreeScreenAction), wgpu::SurfaceError> {
        self.update_camera_buffer();

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut new_warp_index = current_warp_index;
        let mut tt_action = TechTreeScreenAction::None;

        let raw_input = self.egui_state.take_egui_input(&self.window);
        let full_output = self.egui_ctx.run(raw_input, |ctx| {
            let result = super::tech_tree_ui::render_tech_tree_screen(
                ctx,
                tech_tree,
                science,
                warp_levels,
                current_warp_index,
                date_str,
                paused,
                &self.active_toasts,
                part_defs,
            );
            tt_action = result;
            if let TechTreeScreenAction::ChangeWarp(idx) = result {
                new_warp_index = idx;
            }
            super::state::fps_overlay(ctx, self.fps);
        });

        self.egui_state
            .handle_platform_output(&self.window, full_output.platform_output);
        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.egui_renderer
                .update_texture(&self.device, &self.queue, *id, image_delta);
        }

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: self.window.scale_factor() as f32,
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Tech Tree Render Encoder"),
            });
        self.egui_renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &tris,
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Tech Tree Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&view),
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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Tech Tree Egui Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.egui_renderer
                .render(&mut render_pass, &tris, &screen_descriptor);
        }

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok((new_warp_index, tt_action))
    }
}
