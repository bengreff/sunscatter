//! Non-flight render frame functions, extracted from main.rs.
//!
//! These are submodule functions of the binary crate so they can access
//! main.rs's private helpers (super::compute_scaled_positions, super::build_body_data,
//! super::build_orbit_data, super::SCALE, super::WARP_LEVELS, etc.) via `super::`.
//!
//! `render_flight_frame` stays in main.rs — it's the hot path and tightly
//! coupled to input dispatch.

use crate::bodies::G;
use crate::editor::{
    render_editor_ui, EditorAction, generate_grid_vertices, generate_part_vertices,
    generate_ghost_vertices, BodyInfo,
};
use crate::game::{Game, GameMode};
use crate::render::{BodyInfoData, MainMenuAction, PauseAction, RenderState, TitleScreenAction, Vertex};
use crate::save::SaveGame;

/// Render the all-colonies summary screen.
pub fn render_colony_overview_frame(
    game: &mut Game,
    render_state: &mut RenderState,
) {
    if !game.paused {
        let dt = 1.0 / 60.0;
        let time_warp = super::WARP_LEVELS[game.warp_index];
        game.solar_system.update(dt * time_warp);

        let dt_sim = dt * time_warp;
        for vessel in &mut game.flight.inactive_vessels {
            vessel.ship.ensure_on_rails(&game.solar_system);
            vessel.ship.update_on_rails(dt_sim, &game.solar_system);
        }

        game.check_government_milestones();
        game.check_contracts();
        game.update_rd_science(dt_sim);
        game.update_colonies(dt_sim);
    }

    // Camera on Sun, zoomed out
    render_state.tracked_body = Some(game.solar_system.sun_index);
    let scaled_positions = super::compute_scaled_positions(game);
    let in_galaxy_view = super::is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center);
    render_state.update_tracking(&scaled_positions, super::SCALE);

    let mut bodies = super::build_body_data(game, &scaled_positions, in_galaxy_view);
    let mut orbits = super::build_orbit_data(game, &scaled_positions, render_state);
    let accretion_discs = super::build_accretion_disc_data(game);
    let procedural_stars = super::build_procedural_star_data(game, render_state);

    let num_real_bodies = game.solar_system.bodies.len();
    let mut body_names: Vec<String> = game.solar_system.bodies.iter().map(|b| b.name.clone()).collect();
    let focused_star = render_state.focused_star_id.and_then(|(sx, sy, si)| {
        procedural_stars.iter().find(|s| s.sector_x == sx && s.sector_y == sy && s.sector_index == si)
    });
    let ppwu = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    super::inject_catalog_planets(focused_star, &mut bodies, &mut orbits, &mut body_names, game.time(), num_real_bodies, ppwu, &mut render_state.body_texture_map, &mut render_state.catalog_body_info);
    render_state.body_names = body_names.clone();
    render_state.num_real_bodies = num_real_bodies;
    render_state.track_catalog_body(&bodies, super::SCALE);

    render_state.update_bodies_orbits_ship_and_vessels(&bodies, &orbits, None, super::SCALE, Some(&game.part_definitions), &[], &accretion_discs, in_galaxy_view, &procedural_stars);

    let date_str = crate::game::format_date(game.time());

    let sim_time = game.time();
    match render_state.render_colony_overview(
        &game.colony_manager,
        &body_names,
        super::WARP_LEVELS,
        game.warp_index,
        game.paused,
        &date_str,
        game.company.money,
        game.science.available,
        &game.fleet,
        game.solar_system.earth_index,
        &game.blueprints,
        &game.part_definitions,
        &game.solar_system,
        sim_time,
        &game.dyson_swarms,
        &game.tech_tree,
    ) {
        Ok((new_warp_index, action)) => {
            game.warp_index = new_warp_index;
            match action {
                crate::render::ColonyOverviewAction::OpenColony(bi) => {
                    game.enter_colony(bi, GameMode::ColonyOverview);
                }
                crate::render::ColonyOverviewAction::GoToMainMenu => {
                    game.enter_main_menu();
                }
                crate::render::ColonyOverviewAction::ChangeWarp(idx) => {
                    game.warp_index = idx;
                }
                crate::render::ColonyOverviewAction::Trade(trade_action) => {
                    // Intercept OpenEditor — populate route_creation state instead of forwarding
                    if let crate::render::TradeAction::OpenEditor(route_id) = &trade_action {
                        if let Some(route) = game.fleet.get_route(*route_id) {
                            render_state.route_creation =
                                crate::render::RouteCreationState::start_from_route(
                                    route,
                                    &game.fleet,
                                );
                        }
                    } else {
                        super::handle_trade_action(trade_action, game);
                    }
                }
                crate::render::ColonyOverviewAction::None => {}
            }
        }
        Err(wgpu::SurfaceError::Lost) => render_state.resize(render_state.size),
        Err(wgpu::SurfaceError::OutOfMemory) => panic!("OutOfMemory"),
        Err(e) => eprintln!("Colony overview render error: {:?}", e),
    }

    // Process notifications
    for notif in &mut game.notifications {
        if !notif.read && notif.kind.stops_warp() {
            game.warp_index = 0;
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        } else if !notif.read {
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        }
    }
    render_state.active_toasts.retain(|(_, t)| t.elapsed().as_secs_f32() < 5.0);
}

/// Render the management (company finances / contracts) screen.
pub fn render_management_frame(
    game: &mut Game,
    render_state: &mut RenderState,
) {
    if !game.paused {
        let dt = 1.0 / 60.0;
        let time_warp = super::WARP_LEVELS[game.warp_index];
        game.solar_system.update(dt * time_warp);

        let dt_sim = dt * time_warp;
        for vessel in &mut game.flight.inactive_vessels {
            vessel.ship.ensure_on_rails(&game.solar_system);
            vessel.ship.update_on_rails(dt_sim, &game.solar_system);
        }

        game.check_government_milestones();
        game.check_contracts();
        game.update_rd_science(dt_sim);
        game.update_colonies(dt_sim);
    }

    // Camera on Sun, zoomed out
    render_state.tracked_body = Some(game.solar_system.sun_index);
    let scaled_positions = super::compute_scaled_positions(game);
    let in_galaxy_view = super::is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center);
    render_state.update_tracking(&scaled_positions, super::SCALE);

    let mut bodies = super::build_body_data(game, &scaled_positions, in_galaxy_view);
    let mut orbits = super::build_orbit_data(game, &scaled_positions, render_state);
    let accretion_discs = super::build_accretion_disc_data(game);
    let procedural_stars = super::build_procedural_star_data(game, render_state);

    let num_real_bodies = game.solar_system.bodies.len();
    let mut body_names: Vec<String> = game.solar_system.bodies.iter().map(|b| b.name.clone()).collect();
    let focused_star = render_state.focused_star_id.and_then(|(sx, sy, si)| {
        procedural_stars.iter().find(|s| s.sector_x == sx && s.sector_y == sy && s.sector_index == si)
    });
    let ppwu = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    super::inject_catalog_planets(focused_star, &mut bodies, &mut orbits, &mut body_names, game.time(), num_real_bodies, ppwu, &mut render_state.body_texture_map, &mut render_state.catalog_body_info);
    render_state.body_names = body_names.clone();
    render_state.num_real_bodies = num_real_bodies;
    render_state.track_catalog_body(&bodies, super::SCALE);

    render_state.update_bodies_orbits_ship_and_vessels(&bodies, &orbits, None, super::SCALE, Some(&game.part_definitions), &[], &accretion_discs, in_galaxy_view, &procedural_stars);

    let date_str = crate::game::format_date(game.time());

    match render_state.render_management(
        &game.company,
        &game.science,
        &game.contracts,
        game.company.rd_budget,
        super::WARP_LEVELS,
        game.warp_index,
        game.paused,
        &date_str,
    ) {
        Ok((new_warp_index, action, new_budget)) => {
            game.warp_index = new_warp_index;
            game.company.rd_budget = new_budget;
            match action {
                crate::render::ManagementAction::OpenTechTree => {
                    game.enter_tech_tree(GameMode::Management);
                }
                crate::render::ManagementAction::GoToMainMenu => {
                    game.enter_main_menu();
                }
                crate::render::ManagementAction::ChangeWarp(idx) => {
                    game.warp_index = idx;
                }
                crate::render::ManagementAction::AcceptContract(id) => {
                    game.contracts.accept(id);
                }
                crate::render::ManagementAction::CancelContract(id) => {
                    game.contracts.cancel(id);
                    game.contracts.refill_one(
                        &game.science.discoveries,
                        &game.solar_system,
                        game.solar_system.time,
                    );
                }
                crate::render::ManagementAction::SetRdBudget(budget) => {
                    game.company.rd_budget = budget;
                }
                crate::render::ManagementAction::None => {}
            }
        }
        Err(wgpu::SurfaceError::Lost) => render_state.resize(render_state.size),
        Err(wgpu::SurfaceError::OutOfMemory) => panic!("OutOfMemory"),
        Err(e) => eprintln!("Management render error: {:?}", e),
    }

    // Process notifications
    for notif in &mut game.notifications {
        if !notif.read && notif.kind.stops_warp() {
            game.warp_index = 0;
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        } else if !notif.read {
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        }
    }
    render_state.active_toasts.retain(|(_, t)| t.elapsed().as_secs_f32() < 5.0);
}

/// Render the title screen (static Sun background, new/load/quit dialogs).
pub fn render_title_screen_frame(
    game: &mut Game,
    render_state: &mut RenderState,
    elwt: &winit::event_loop::EventLoopWindowTarget<()>,
) {
    // Static camera on Sun — no time advancement
    let sun_pos = game.solar_system.body_position(game.solar_system.sun_index);
    render_state.camera.position[0] = sun_pos[0] * super::SCALE * super::BODY_SCALE;
    render_state.camera.position[1] = sun_pos[1] * super::SCALE * super::BODY_SCALE;
    render_state.camera.body_center = render_state.camera.position;
    render_state.camera.ship_offset = [0.0, 0.0];
    render_state.camera.zoom = 0.002;

    // Generate geometry for visual background only (no simulation update)
    let scaled_positions = super::compute_scaled_positions(game);
    let bodies = super::build_body_data(game, &scaled_positions, false);
    let orbits = super::build_orbit_data(game, &scaled_positions, render_state);
    render_state.update_tracking(&scaled_positions, super::SCALE);
    render_state.update_bodies_orbits_and_ship(&bodies, &orbits, None, super::SCALE, None);

    let paused = game.paused;
    let mut show_new_game = game.title_screen.show_new_game;
    let mut show_load_game = game.title_screen.show_load_game;
    let mut new_game_name = game.title_screen.new_game_name.clone();
    let save_list = &game.title_screen.save_list;
    let mut confirm_delete = game.title_screen.confirm_delete.clone();

    match render_state.render_title_screen(|ctx| {
        let mut action = TitleScreenAction::None;

        if paused {
            // Quit confirmation overlay
            egui::Area::new(egui::Id::new("quit_overlay"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                        .inner_margin(egui::Margin::same(40.0))
                        .rounding(egui::Rounding::same(8.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.heading(egui::RichText::new("Quit Game?").size(32.0).color(egui::Color32::WHITE));
                                ui.add_space(20.0);
                                if ui.button(egui::RichText::new("Quit").size(18.0)).clicked() {
                                    action = TitleScreenAction::QuitGame;
                                }
                            });
                        });
                });
        } else if show_new_game {
            // New game dialog
            egui::Area::new(egui::Id::new("new_game_dialog"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
                        .inner_margin(egui::Margin::same(40.0))
                        .rounding(egui::Rounding::same(8.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.heading(egui::RichText::new("New Game").size(28.0).color(egui::Color32::WHITE));
                                ui.add_space(20.0);
                                ui.label(egui::RichText::new("Save name:").color(egui::Color32::LIGHT_GRAY));
                                ui.add_space(5.0);
                                let response = ui.text_edit_singleline(&mut new_game_name);
                                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !new_game_name.trim().is_empty() {
                                    action = TitleScreenAction::NewGame(new_game_name.trim().to_string());
                                }
                                ui.add_space(15.0);
                                ui.horizontal(|ui| {
                                    if ui.button(egui::RichText::new("Start").size(18.0)).clicked() && !new_game_name.trim().is_empty() {
                                        action = TitleScreenAction::NewGame(new_game_name.trim().to_string());
                                    }
                                    ui.add_space(10.0);
                                    if ui.button(egui::RichText::new("Back").size(18.0)).clicked() {
                                        show_new_game = false;
                                    }
                                });
                            });
                        });
                });
        } else if show_load_game {
            // Load game dialog
            egui::Area::new(egui::Id::new("load_game_dialog"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
                        .inner_margin(egui::Margin::same(40.0))
                        .rounding(egui::Rounding::same(8.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.heading(egui::RichText::new("Load Game").size(28.0).color(egui::Color32::WHITE));
                                ui.add_space(20.0);

                                let import_btn = ui.button(egui::RichText::new("Import Save File\u{2026}").size(15.0));
                                if import_btn.clicked() {
                                    action = TitleScreenAction::ImportSave;
                                }
                                import_btn.on_hover_text(
                                    "Pick a .ron save file from disk",
                                );
                                ui.add_space(10.0);

                                if save_list.is_empty() {
                                    ui.label(egui::RichText::new("No saves yet — start a New Game or Import\u{2026}").color(egui::Color32::GRAY));
                                } else {
                                    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                                        for save in save_list {
                                            let is_fixture = save.save_id == crate::save::test_fixture::FIXTURE_ID;
                                            let label = if is_fixture {
                                                format!("{} \u{2014} dev fixture", save.name)
                                            } else {
                                                format!(
                                                    "{} \u{2014} {} vessels \u{2014} {}",
                                                    save.name,
                                                    save.vessel_count,
                                                    crate::game::format_date(save.simulation_time),
                                                )
                                            };
                                            ui.horizontal(|ui| {
                                                let total_width = ui.available_width();
                                                // No export/delete buttons on the fixture row, so it
                                                // gets the full-width load button.
                                                let row_button_width = if is_fixture {
                                                    total_width
                                                } else {
                                                    total_width - 56.0
                                                };
                                                let button_resp = ui.add_sized(
                                                    [row_button_width, 0.0],
                                                    egui::Button::new(egui::RichText::new(&label).size(16.0)),
                                                );
                                                if button_resp.clicked() {
                                                    action = TitleScreenAction::LoadGame(save.save_id.clone());
                                                }
                                                if !is_fixture {
                                                    let export_btn = ui.small_button(
                                                        egui::RichText::new("\u{2913}")
                                                            .size(16.0)
                                                            .color(egui::Color32::from_rgb(120, 180, 220)),
                                                    );
                                                    if export_btn.clicked() {
                                                        action = TitleScreenAction::ExportSave(save.save_id.clone());
                                                    }
                                                    export_btn.on_hover_text("Export save as .ron");
                                                    let delete_btn = ui.small_button(
                                                        egui::RichText::new("\u{00d7}")
                                                            .size(16.0)
                                                            .color(egui::Color32::from_rgb(180, 80, 80)),
                                                    );
                                                    if delete_btn.clicked() {
                                                        confirm_delete = Some(save.save_id.clone());
                                                    }
                                                    delete_btn.on_hover_text("Delete save");
                                                }
                                            });
                                        }
                                    });
                                }
                                ui.add_space(15.0);
                                if ui.button(egui::RichText::new("Back").size(18.0)).clicked() {
                                    show_load_game = false;
                                    confirm_delete = None;
                                }
                            });
                        });
                });

            // Delete confirmation overlay (shown on top of load dialog)
            if let Some(delete_id) = confirm_delete.clone() {
                let delete_name = save_list.iter()
                    .find(|s| s.save_id == delete_id)
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| delete_id.clone());
                let confirm_label = format!("Delete \"{}\"?", delete_name);
                egui::Area::new(egui::Id::new("delete_confirm_overlay"))
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220))
                            .inner_margin(egui::Margin::same(30.0))
                            .rounding(egui::Rounding::same(8.0))
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    ui.label(egui::RichText::new(&confirm_label).size(20.0).color(egui::Color32::WHITE));
                                    ui.add_space(5.0);
                                    ui.label(egui::RichText::new("This cannot be undone.").size(14.0).color(egui::Color32::from_rgb(200, 150, 150)));
                                    ui.add_space(15.0);
                                    ui.horizontal(|ui| {
                                        if ui.button(egui::RichText::new("Delete").size(16.0).color(egui::Color32::from_rgb(220, 80, 80))).clicked() {
                                            action = TitleScreenAction::DeleteGame(delete_id.clone());
                                            confirm_delete = None;
                                        }
                                        ui.add_space(10.0);
                                        if ui.button(egui::RichText::new("Cancel").size(16.0)).clicked() {
                                            confirm_delete = None;
                                        }
                                    });
                                });
                            });
                    });
            }
        } else {
            // Main title screen
            egui::Area::new(egui::Id::new("title_screen"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                        .inner_margin(egui::Margin::same(40.0))
                        .rounding(egui::Rounding::same(8.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.heading(egui::RichText::new("Sunscatter").size(48.0).color(egui::Color32::WHITE));
                                ui.add_space(30.0);
                                if ui.button(egui::RichText::new("New Game").size(20.0)).clicked() {
                                    new_game_name = "default".to_string();
                                    show_new_game = true;
                                }
                                ui.add_space(10.0);
                                if ui.button(egui::RichText::new("Load Game").size(20.0)).clicked() {
                                    show_load_game = true;
                                }
                            });
                        });
                });
        }

        action
    }) {
        Ok(title_action) => {
            match &title_action {
                TitleScreenAction::NewGame(name) => {
                    game.reset_for_new_game(name.clone());
                    return;
                }
                TitleScreenAction::LoadGame(save_id) => {
                    match SaveGame::load_from_file(save_id) {
                        Ok(save) => {
                            let save_name = save.name.clone();
                            save.restore_to_game(game);
                            game.save_name = Some(save_name);
                            game.mode = GameMode::MainMenu;
                            game.paused = false;
                        }
                        Err(e) => {
                            log::error!("Failed to load save: {}", e);
                        }
                    }
                    return;
                }
                TitleScreenAction::DeleteGame(save_id) => {
                    match SaveGame::delete_save(save_id) {
                        Ok(()) => {
                            log::info!("Deleted save: {}", save_id);
                            game.title_screen.save_list = SaveGame::list_saves();
                        }
                        Err(e) => {
                            log::error!("Failed to delete save: {}", e);
                        }
                    }
                    game.title_screen.confirm_delete = None;
                    return;
                }
                TitleScreenAction::ImportSave => {
                    crate::save::io::import_save(&render_state.window);
                }
                TitleScreenAction::ExportSave(save_id) => {
                    crate::save::io::export_save(&render_state.window, save_id);
                }
                TitleScreenAction::QuitGame => {
                    elwt.exit();
                }
                TitleScreenAction::None => {}
            }

            // Sync title_screen UI state back
            game.title_screen.show_new_game = show_new_game;
            // Refresh save list every frame while the load dialog is open
            // so async imports become visible without the user reopening it.
            // Reads from an in-memory HashMap, trivially cheap.
            if show_load_game {
                game.title_screen.save_list = SaveGame::list_saves();
            } else {
                game.title_screen.save_list.clear();
            }
            game.title_screen.show_load_game = show_load_game;
            game.title_screen.new_game_name = new_game_name;
            game.title_screen.confirm_delete = confirm_delete;
        }
        Err(wgpu::SurfaceError::Lost) => render_state.resize(render_state.size),
        Err(wgpu::SurfaceError::OutOfMemory) => panic!("OutOfMemory"),
        Err(e) => eprintln!("Title screen render error: {:?}", e),
    }
}

/// Render the main menu (in-game top-level menu).
pub fn render_main_menu_frame(
    game: &mut Game,
    render_state: &mut RenderState,
    _elwt: &winit::event_loop::EventLoopWindowTarget<()>,
) {
    if !game.paused {
        let dt = 1.0 / 60.0;
        let time_warp = super::WARP_LEVELS[game.warp_index];
        game.solar_system.update(dt * time_warp);

        // Propagate all vessels on rails (no active vessel while not in flight)
        let dt_sim = dt * time_warp;
        for vessel in &mut game.flight.inactive_vessels {
            vessel.ship.ensure_on_rails(&game.solar_system);
            vessel.ship.update_on_rails(dt_sim, &game.solar_system);
        }
        game.flight.inactive_vessels.retain(|v| {
            let in_landing_zone = v.ship.in_atmosphere(&game.solar_system)
                || v.ship.below_landing_altitude(&game.solar_system);
            !(v.ship.periapsis_below_surface(&game.solar_system) && in_landing_zone)
        });

        // Update colony simulation
        game.update_colonies(dt_sim);
    }

    // Camera follows the Sun AFTER the simulation update so positions match
    let sun_pos = game.solar_system.body_position(game.solar_system.sun_index);
    render_state.camera.position[0] = sun_pos[0] * super::SCALE * super::BODY_SCALE;
    render_state.camera.position[1] = sun_pos[1] * super::SCALE * super::BODY_SCALE;
    render_state.camera.body_center = render_state.camera.position;
    render_state.camera.ship_offset = [0.0, 0.0];
    render_state.camera.zoom = 0.002;

    let scaled_positions = super::compute_scaled_positions(game);
    let bodies = super::build_body_data(game, &scaled_positions, false);
    let orbits = super::build_orbit_data(game, &scaled_positions, render_state);

    // Update camera tracking (body focus)
    render_state.update_tracking(&scaled_positions, super::SCALE);

    // Update body/orbit geometry (no ship)
    render_state.update_bodies_orbits_and_ship(&bodies, &orbits, None, super::SCALE, None);

    let paused = game.paused;
    let date_str = crate::game::format_date(game.time());

    match render_state.render_main_menu(super::WARP_LEVELS, game.warp_index, &date_str, |ctx| {
        let mut action = MainMenuAction::None;

        if paused {
            // Pause overlay
            egui::Area::new(egui::Id::new("pause_overlay"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                        .inner_margin(egui::Margin::same(40.0))
                        .rounding(egui::Rounding::same(8.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.heading(egui::RichText::new("Paused").size(32.0).color(egui::Color32::WHITE));
                                ui.add_space(20.0);
                                if ui.button(egui::RichText::new("Title Screen").size(18.0)).clicked() {
                                    action = MainMenuAction::Quit;
                                }
                            });
                        });
                });
        } else {
            // Main menu centered
            egui::Area::new(egui::Id::new("main_menu"))
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                        .inner_margin(egui::Margin::same(40.0))
                        .rounding(egui::Rounding::same(8.0))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.heading(egui::RichText::new("Sunscatter").size(36.0).color(egui::Color32::WHITE));
                                ui.add_space(30.0);
                                if ui.button(egui::RichText::new("Editor").size(20.0)).clicked() {
                                    action = MainMenuAction::Editor;
                                }
                                ui.add_space(10.0);
                                if ui.button(egui::RichText::new("Tracking Station").size(20.0)).clicked() {
                                    action = MainMenuAction::TrackingStation;
                                }
                                ui.add_space(10.0);
                                if ui.button(egui::RichText::new("Colonies").size(20.0)).clicked() {
                                    action = MainMenuAction::Colonies;
                                }
                                ui.add_space(10.0);
                                if ui.button(egui::RichText::new("Management").size(20.0)).clicked() {
                                    action = MainMenuAction::Management;
                                }
                            });
                        });
                });
        }

        action
    }) {
        Ok((new_warp_index, menu_action)) => {
            game.warp_index = new_warp_index;
            match menu_action {
                MainMenuAction::Editor => game.enter_editor(),
                MainMenuAction::TrackingStation => {
                    game.enter_tracking_station();
                    render_state.focus_on_body(game.solar_system.earth_index);
                    // Zoom so Earth fills ~half the screen
                    let earth_radius_world = game.solar_system.bodies[game.solar_system.earth_index].radius * super::SCALE * super::BODY_SCALE;
                    render_state.camera.zoom = (0.25 / earth_radius_world) as f32;
                },
                MainMenuAction::Colonies => {
                    game.enter_colony_overview();
                    render_state.focus_on_body(game.solar_system.sun_index);
                    render_state.camera.zoom = 0.002;
                },
                MainMenuAction::Management => {
                    game.enter_management();
                    render_state.focus_on_body(game.solar_system.sun_index);
                    render_state.camera.zoom = 0.002;
                },
                MainMenuAction::Quit => {
                    super::save_and_quit_to_title(game, render_state);
                },
                MainMenuAction::None => {}
            }
        }
        Err(wgpu::SurfaceError::Lost) => render_state.resize(render_state.size),
        Err(wgpu::SurfaceError::OutOfMemory) => panic!("OutOfMemory"),
        Err(e) => eprintln!("Main menu render error: {:?}", e),
    }

    // Process colony notifications
    for notif in &mut game.notifications {
        if !notif.read && notif.kind.stops_warp() {
            game.warp_index = 0;
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        } else if !notif.read {
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        }
    }
    render_state.active_toasts.retain(|(_, t)| t.elapsed().as_secs_f32() < 5.0);
}

/// Render full-screen tech tree.
pub fn render_tech_tree_frame(
    game: &mut Game,
    render_state: &mut RenderState,
) {
    if !game.paused {
        let dt = 1.0 / 60.0;
        let time_warp = super::WARP_LEVELS[game.warp_index];
        game.solar_system.update(dt * time_warp);

        let dt_sim = dt * time_warp;
        for vessel in &mut game.flight.inactive_vessels {
            vessel.ship.ensure_on_rails(&game.solar_system);
            vessel.ship.update_on_rails(dt_sim, &game.solar_system);
        }

        game.check_government_milestones();
        game.check_contracts();
        game.update_rd_science(dt_sim);
        game.update_colonies(dt_sim);
    }

    // Camera on Sun, zoomed out
    render_state.tracked_body = Some(game.solar_system.sun_index);
    let scaled_positions = super::compute_scaled_positions(game);
    let in_galaxy_view = super::is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center);
    render_state.update_tracking(&scaled_positions, super::SCALE);

    let mut bodies = super::build_body_data(game, &scaled_positions, in_galaxy_view);
    let mut orbits = super::build_orbit_data(game, &scaled_positions, render_state);
    let accretion_discs = super::build_accretion_disc_data(game);
    let procedural_stars = super::build_procedural_star_data(game, render_state);

    let num_real_bodies = game.solar_system.bodies.len();
    let mut body_names: Vec<String> = game.solar_system.bodies.iter().map(|b| b.name.clone()).collect();
    let focused_star = render_state.focused_star_id.and_then(|(sx, sy, si)| {
        procedural_stars.iter().find(|s| s.sector_x == sx && s.sector_y == sy && s.sector_index == si)
    });
    let ppwu = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    super::inject_catalog_planets(focused_star, &mut bodies, &mut orbits, &mut body_names, game.time(), num_real_bodies, ppwu, &mut render_state.body_texture_map, &mut render_state.catalog_body_info);
    render_state.body_names = body_names.clone();
    render_state.num_real_bodies = num_real_bodies;
    render_state.track_catalog_body(&bodies, super::SCALE);

    render_state.update_bodies_orbits_ship_and_vessels(&bodies, &orbits, None, super::SCALE, Some(&game.part_definitions), &[], &accretion_discs, in_galaxy_view, &procedural_stars);

    let date_str = crate::game::format_date(game.time());

    match render_state.render_tech_tree_screen(
        &mut game.tech_tree,
        &mut game.science,
        super::WARP_LEVELS,
        game.warp_index,
        game.paused,
        &date_str,
        &game.part_definitions,
    ) {
        Ok((new_warp_index, action)) => {
            game.warp_index = new_warp_index;
            match action {
                crate::render::TechTreeScreenAction::Back => {
                    game.leave_tech_tree();
                }
                crate::render::TechTreeScreenAction::ChangeWarp(idx) => {
                    game.warp_index = idx;
                }
                crate::render::TechTreeScreenAction::None => {}
            }
        }
        Err(wgpu::SurfaceError::Lost) => render_state.resize(render_state.size),
        Err(wgpu::SurfaceError::OutOfMemory) => panic!("OutOfMemory"),
        Err(e) => eprintln!("Tech tree render error: {:?}", e),
    }

    // Process notifications
    for notif in &mut game.notifications {
        if !notif.read && notif.kind.stops_warp() {
            game.warp_index = 0;
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        } else if !notif.read {
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        }
    }
    render_state.active_toasts.retain(|(_, t)| t.elapsed().as_secs_f32() < 5.0);
}

/// Render the per-colony management screen.
pub fn render_colony_frame(
    game: &mut Game,
    render_state: &mut RenderState,
) {
    if !game.paused {
        let dt = 1.0 / 60.0;
        let time_warp = super::WARP_LEVELS[game.warp_index];
        game.solar_system.update(dt * time_warp);

        // Propagate all vessels on rails
        let dt_sim = dt * time_warp;
        for vessel in &mut game.flight.inactive_vessels {
            vessel.ship.ensure_on_rails(&game.solar_system);
            vessel.ship.update_on_rails(dt_sim, &game.solar_system);
        }

        // Check milestones, contracts, R&D science, and update colony simulation
        game.check_government_milestones();
        game.check_contracts();
        game.update_rd_science(dt_sim);
        game.update_colonies(dt_sim);
    }

    // Camera: focus on colony body
    let body_index = game.colony_view_body_index.unwrap_or(game.solar_system.earth_index);
    let scaled_positions = super::compute_scaled_positions(game);
    let in_galaxy_view = super::is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center);

    // Track the colony body
    render_state.tracked_body = Some(body_index);
    render_state.update_tracking(&scaled_positions, super::SCALE);

    let mut bodies = super::build_body_data(game, &scaled_positions, in_galaxy_view);
    let mut orbits = super::build_orbit_data(game, &scaled_positions, render_state);
    let accretion_discs = super::build_accretion_disc_data(game);
    let procedural_stars = super::build_procedural_star_data(game, render_state);

    let num_real_bodies = game.solar_system.bodies.len();
    let mut body_names: Vec<String> = game.solar_system.bodies.iter().map(|b| b.name.clone()).collect();
    let focused_star = render_state.focused_star_id.and_then(|(sx, sy, si)| {
        procedural_stars.iter().find(|s| s.sector_x == sx && s.sector_y == sy && s.sector_index == si)
    });
    let ppwu = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    super::inject_catalog_planets(focused_star, &mut bodies, &mut orbits, &mut body_names, game.time(), num_real_bodies, ppwu, &mut render_state.body_texture_map, &mut render_state.catalog_body_info);
    render_state.body_names = body_names.clone();
    render_state.num_real_bodies = num_real_bodies;
    render_state.track_catalog_body(&bodies, super::SCALE);

    render_state.update_bodies_orbits_ship_and_vessels(&bodies, &orbits, None, super::SCALE, Some(&game.part_definitions), &[], &accretion_discs, in_galaxy_view, &procedural_stars);

    let date_str = crate::game::format_date(game.time());

    // Build colony data
    let colony_body_hab: Vec<u32> = game.solar_system.bodies.iter().map(|b| b.habitability_score).collect();
    let colony_body_radii: Vec<f64> = game.solar_system.bodies.iter().map(|b| b.radius).collect();
    let colony_body_mineable: Vec<Vec<crate::colony::ResourceType>> = game.solar_system.bodies.iter().map(|b| b.mineable_resources.clone()).collect();
    let colony_body_atmospheric: Vec<Vec<crate::colony::ResourceType>> = game.solar_system.bodies.iter().map(|b| b.atmospheric_resources.clone()).collect();

    // Check if we can return to flight (only if we came from flight)
    let can_return_to_flight = game.colony_return_mode == Some(GameMode::Flight);

    // Compute solar power factor for this body: (AU / sun_distance)^2
    let au: f64 = 1.496e11;
    let sun_dist = crate::colony::simulation::sun_distance(body_index, &game.solar_system);
    let solar_power_factor = (au / sun_dist).powi(2);

    match render_state.render_colony(
        &body_names,
        super::WARP_LEVELS,
        game.warp_index,
        game.paused,
        &date_str,
        body_index,
        &game.colony_manager,
        &colony_body_hab,
        &colony_body_radii,
        &colony_body_mineable,
        &colony_body_atmospheric,
        can_return_to_flight,
        solar_power_factor,
        &game.tech_tree,
        &game.fleet,
        game.solar_system.earth_index,
    ) {
        Ok((new_warp_index, action)) => {
            game.warp_index = new_warp_index;
            match action {
                crate::render::ColonyScreenAction::QueueBuilding(bi, bt, count) => {
                    let hab_score = game.solar_system.bodies[bi].habitability_score;
                    let body_radius_m = game.solar_system.bodies[bi].radius;
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        if let Err(e) = colony.queue_building(bt, hab_score, body_radius_m, count) {
                            log::error!("Failed to queue building: {}", e);
                        }
                    }
                }
                crate::render::ColonyScreenAction::AddMineAssignment(bi, resource, count) => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        for _ in 0..count {
                            if let Some(building) = colony.buildings.iter_mut().find(|b| {
                                b.building_type == crate::colony::BuildingType::Mine
                                    && b.assigned_resource.is_none()
                            }) {
                                building.assigned_resource = Some(resource);
                            } else {
                                break;
                            }
                        }
                    }
                }
                crate::render::ColonyScreenAction::RemoveMineAssignment(bi, resource, count) => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        for _ in 0..count {
                            if let Some(building) = colony.buildings.iter_mut().find(|b| {
                                b.building_type == crate::colony::BuildingType::Mine
                                    && b.assigned_resource == Some(resource)
                            }) {
                                building.assigned_resource = None;
                            } else {
                                break;
                            }
                        }
                    }
                }
                crate::render::ColonyScreenAction::AddCollectorAssignment(bi, resource, count) => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        for _ in 0..count {
                            if let Some(building) = colony.buildings.iter_mut().find(|b| {
                                b.building_type == crate::colony::BuildingType::AtmosphericCollector
                                    && b.assigned_resource.is_none()
                            }) {
                                building.assigned_resource = Some(resource);
                            } else {
                                break;
                            }
                        }
                    }
                }
                crate::render::ColonyScreenAction::RemoveCollectorAssignment(bi, resource, count) => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        for _ in 0..count {
                            if let Some(building) = colony.buildings.iter_mut().find(|b| {
                                b.building_type == crate::colony::BuildingType::AtmosphericCollector
                                    && b.assigned_resource == Some(resource)
                            }) {
                                building.assigned_resource = None;
                            } else {
                                break;
                            }
                        }
                    }
                }
                crate::render::ColonyScreenAction::AddFactoryAssignment(bi, recipe, count) => {
                    if !game.tech_tree.is_recipe_available(recipe.recipe_id()) {
                        log::warn!("[colony] Blocked factory assignment: recipe {:?} not unlocked", recipe);
                    } else if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        for _ in 0..count {
                            if let Some(building) = colony.buildings.iter_mut().find(|b| {
                                b.building_type == crate::colony::BuildingType::Factory
                                    && b.assigned_recipe.is_none()
                            }) {
                                building.assigned_recipe = Some(recipe);
                            } else {
                                break;
                            }
                        }
                    }
                }
                crate::render::ColonyScreenAction::RemoveFactoryAssignment(bi, recipe, count) => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        for _ in 0..count {
                            if let Some(building) = colony.buildings.iter_mut().find(|b| {
                                b.building_type == crate::colony::BuildingType::Factory
                                    && b.assigned_recipe == Some(recipe)
                            }) {
                                building.assigned_recipe = None;
                            } else {
                                break;
                            }
                        }
                    }
                }
                crate::render::ColonyScreenAction::ReturnToFlight => {
                    game.leave_colony();
                }
                crate::render::ColonyScreenAction::GoToTrackingStation => {
                    game.colony_view_body_index = None;
                    game.colony_return_mode = None;
                    game.enter_tracking_station();
                }
                crate::render::ColonyScreenAction::GoToColonyOverview => {
                    game.colony_view_body_index = None;
                    game.colony_return_mode = None;
                    game.enter_colony_overview();
                }
                crate::render::ColonyScreenAction::GoToMainMenu => {
                    game.colony_view_body_index = None;
                    game.colony_return_mode = None;
                    game.enter_main_menu();
                }
                crate::render::ColonyScreenAction::ChangeWarp(idx) => {
                    game.warp_index = idx;
                }
                crate::render::ColonyScreenAction::SwitchColony(bi) => {
                    game.colony_view_body_index = Some(bi);
                    render_state.tracked_body = Some(bi);
                }
                crate::render::ColonyScreenAction::DebugAddResource(bi, res, amount) => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        // Route Food to food_stored instead of resource inventory
                        if res == crate::colony::ResourceType::Food {
                            colony.food_stored += amount;
                        } else {
                            colony.resources.add(res, amount);
                        }
                    }
                }
                crate::render::ColonyScreenAction::DebugAddBuilding(bi, bt) => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        colony.buildings.push(crate::colony::BuildingInstance::new(bt));
                    }
                }
                crate::render::ColonyScreenAction::DebugAddCrew(bi, count) => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        let cap = colony.crew_capacity();
                        if cap > 0 {
                            colony.crew = (colony.crew + count).min(cap);
                        } else {
                            colony.crew += count;
                        }
                    }
                }
                crate::render::ColonyScreenAction::ScrapShip(bi, ship_id) => {
                    let body_name = game.solar_system.bodies.get(bi)
                        .map(|b| b.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        let ship_name = colony.stored_ships.iter()
                            .find(|s| s.id == ship_id)
                            .map(|s| s.name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());
                        if colony.scrap_ship(ship_id, &game.part_definitions).is_some() {
                            log::info!("Scrapped ship '{}' at {}", ship_name, body_name);
                            game.notifications.push(crate::colony::Notification {
                                kind: crate::colony::NotificationKind::ShipScrapped {
                                    ship_name,
                                    location: body_name,
                                },
                                time: game.solar_system.time,
                                read: false,
                            });
                        }
                    }
                }
                crate::render::ColonyScreenAction::Trade(trade_action) => {
                    super::handle_trade_action(trade_action, game);
                }
                crate::render::ColonyScreenAction::SetStorageAllocation { body_index: bi, resource, percent } => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        colony.storage_allocation.set_pinned(resource, percent);
                    }
                }
                crate::render::ColonyScreenAction::UnpinStorageAllocation { body_index: bi, resource } => {
                    if let Some(colony) = game.colony_manager.get_by_body_mut(bi) {
                        colony.storage_allocation.unpin(resource);
                    }
                }
                crate::render::ColonyScreenAction::None => {}
            }
        }
        Err(wgpu::SurfaceError::Lost) => render_state.resize(render_state.size),
        Err(wgpu::SurfaceError::OutOfMemory) => panic!("OutOfMemory"),
        Err(e) => eprintln!("Colony render error: {:?}", e),
    }

    // Process notifications
    for notif in &mut game.notifications {
        if !notif.read && notif.kind.stops_warp() {
            game.warp_index = 0;
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        } else if !notif.read {
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        }
    }
    render_state.active_toasts.retain(|(_, t)| t.elapsed().as_secs_f32() < 5.0);
}

/// Render the tracking station (vessel selection / body catalog).
pub fn render_tracking_station_frame(
    game: &mut Game,
    render_state: &mut RenderState,
) {
    if !game.paused {
        let dt = 1.0 / 60.0;
        let time_warp = super::WARP_LEVELS[game.warp_index];
        game.solar_system.update(dt * time_warp);

        // Propagate all vessels on rails (no active vessel while not in flight)
        let dt_sim = dt * time_warp;
        for vessel in &mut game.flight.inactive_vessels {
            vessel.ship.ensure_on_rails(&game.solar_system);
            vessel.ship.update_on_rails(dt_sim, &game.solar_system);
        }
        game.flight.inactive_vessels.retain(|v| {
            let in_landing_zone = v.ship.in_atmosphere(&game.solar_system)
                || v.ship.below_landing_altitude(&game.solar_system);
            !(v.ship.periapsis_below_surface(&game.solar_system) && in_landing_zone)
        });

        // Check milestones, contracts, R&D science, and update colony simulation
        game.check_government_milestones();
        game.check_contracts();
        game.update_rd_science(dt_sim);
        game.update_colonies(dt_sim);
    }

    // Set colony state for tracking station UI
    render_state.has_colonies = !game.colony_manager.colonies.is_empty();

    let scaled_positions = super::compute_scaled_positions(game);
    let in_galaxy_view = super::is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center);

    let mut bodies = super::build_body_data(game, &scaled_positions, in_galaxy_view);
    let mut orbits = super::build_orbit_data(game, &scaled_positions, render_state);

    // Update camera tracking (body or vessel focus)
    render_state.update_tracking(&scaled_positions, super::SCALE);

    // Build vessel tracking data
    let tracking_vessels = super::build_tracking_vessel_data(game, &scaled_positions);

    // Update camera tracking for focused vessel
    if let Some(vessel_id) = render_state.tracked_vessel {
        if let Some(vessel_data) = tracking_vessels.iter().find(|v| v.id == vessel_id) {
            // Use SOI body center + vessel offset for precision
            let soi_pos = scaled_positions[vessel_data.soi_body];
            render_state.camera.body_center = [soi_pos[0] * super::SCALE, soi_pos[1] * super::SCALE];
            // vessel_data.x/y already include the offset; recover it by subtraction
            render_state.camera.ship_offset = [
                vessel_data.x - render_state.camera.body_center[0],
                vessel_data.y - render_state.camera.body_center[1],
            ];
            render_state.camera.position[0] = vessel_data.x;
            render_state.camera.position[1] = vessel_data.y;
        } else {
            // Tracked vessel was destroyed, focus on Earth
            render_state.tracked_vessel = None;
            render_state.focus_on_body(game.solar_system.earth_index);
        }
    }

    let accretion_discs = super::build_accretion_disc_data(game);
    let procedural_stars = super::build_procedural_star_data(game, render_state);
    let body_is_star = super::build_body_is_star(game);

    // Inject catalog planets as synthetic bodies when a catalog star is focused
    let num_real_bodies = game.solar_system.bodies.len();
    let mut body_names: Vec<String> = game.solar_system.bodies.iter().map(|b| b.name.clone()).collect();
    let focused_star = render_state.focused_star_id.and_then(|(sx, sy, si)| {
        procedural_stars.iter().find(|s| s.sector_x == sx && s.sector_y == sy && s.sector_index == si)
    });
    let ppwu = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    super::inject_catalog_planets(focused_star, &mut bodies, &mut orbits, &mut body_names, game.time(), num_real_bodies, ppwu, &mut render_state.body_texture_map, &mut render_state.catalog_body_info);
    render_state.body_names = body_names.clone();
    render_state.num_real_bodies = num_real_bodies;
    render_state.track_catalog_body(&bodies, super::SCALE);

    render_state.update_bodies_orbits_ship_and_vessels(&bodies, &orbits, None, super::SCALE, Some(&game.part_definitions), &tracking_vessels, &accretion_discs, in_galaxy_view, &procedural_stars);
    let date_str = crate::game::format_date(game.time());

    // Build body info data for the info panel
    let body_info: Vec<BodyInfoData> = game.solar_system.bodies.iter().enumerate().map(|(i, body)| {
        let orbit_period_s = body.orbit.as_ref().and_then(|orbit| {
            body.parent.map(|pi| {
                let parent_mass = game.solar_system.bodies[pi].effective_mass_at(orbit.semi_major_axis);
                let mu = G * parent_mass;
                std::f64::consts::TAU * (orbit.semi_major_axis.powi(3) / mu).sqrt()
            })
        });
        // Detect stars and black holes: root body or bodies orbiting root
        let is_star = body.parent.is_none() || body_is_star[i];
        // Collect children (moons/planets whose parent is this body) for the info panel.
        // For non-star bodies (planets), children are moons. For stars, children are planets.
        // Exclude child stars (e.g. the Sun) from the planetary system — only show planets.
        let children: Vec<crate::render::CatalogPlanetInfo> = game.solar_system.bodies.iter().enumerate().filter(|(j, b)| {
            b.parent == Some(i) && !body_is_star[*j]
        }).map(|(_, b)| crate::render::CatalogPlanetInfo {
            name: b.name.clone(),
            designation: String::new(),
            temperature_k: 0.0,
            gravity_g: b.surface_gravity() / 9.81,
            habitability: b.habitability_score,
            has_atmosphere: b.atmosphere.is_some(),
            has_life: false,
            is_moon: !is_star,
            is_gas_giant: b.is_gas_giant,
        }).collect();
        let (luminosity_solar, star_type_str, temperature_k) = if is_star {
            if body.parent.is_none() && body.name != "Sun" {
                // Root body that isn't the Sun (e.g. Sgr A*) — supermassive black hole
                (None, Some("Supermassive Black Hole".to_string()), None)
            } else if body.name == "Sun" {
                (Some(1.0), Some("G-type Main Sequence".to_string()), Some(5778.0))
            } else {
                // Other solar system stars: L ∝ M^3.5, T from Stefan-Boltzmann
                let mass_solar = body.mass / 1.989e30;
                let lum = mass_solar.powf(3.5);
                // T = T_sun * (L/R²)^0.25 where R in solar radii
                let r_solar = body.radius / 6.957e8;
                let temp = 5778.0 * (lum / (r_solar * r_solar)).powf(0.25);
                (Some(lum), Some("Star".to_string()), Some(temp))
            }
        } else {
            (None, None, None)
        };
        // is_galactic_orbit: body orbits the root (Sgr A*), i.e. parent == Some(0)
        let is_galactic_orbit = body.parent == Some(0);
        BodyInfoData {
            name: body.name.clone(),
            description: body.description.clone(),
            radius_m: body.radius,
            surface_gravity_ms2: body.surface_gravity(),
            mass_kg: body.mass,
            atmosphere_pressure_pa: body.atmosphere.as_ref().map(|a| a.surface_pressure),
            atmosphere_height_m: body.atmosphere.as_ref().map(|a| a.visible_height()),
            orbit_semi_major_axis_m: body.orbit.as_ref().map(|o| o.semi_major_axis),
            orbit_eccentricity: body.orbit.as_ref().map(|o| o.eccentricity),
            orbit_period_s,
            mineable_resources: body.mineable_resources.clone(),
            atmospheric_resources: body.atmospheric_resources.clone(),
            habitability_score: body.habitability_score,
            luminosity_solar,
            star_type: star_type_str,
            temperature_k,
            soi_radius_m: Some(body.soi_radius).filter(|r| r.is_finite()),
            is_galactic_orbit,
            catalog_stars: vec![],
            catalog_planets: children,
            catalog_zone: None,
            catalog_distance_ly: None,
            catalog_spectral: None,
        }
    }).collect();

    // Build focused_star_info from procedural star data (for unified info panel)
    render_state.focused_star_info = render_state.focused_star.and_then(|idx| {
        render_state.current_procedural_stars.get(idx).map(|s| {
            use crate::bodies::{galactic_enclosed_mass, calculate_soi};
            use crate::render::{CatalogPlanetInfo, CatalogStarInfo};
            let mass_kg = s.mass_solar * 1.989e30;
            let soi = calculate_soi(s.semi_major_axis_m, mass_kg, galactic_enclosed_mass(s.semi_major_axis_m)) / 20.0;
            let surface_gravity = G * mass_kg / (s.radius_m * s.radius_m);

            // Look up catalog data if this is a named star
            let cat = crate::galaxy::catalog::lookup_system(s.catalog_index);
            let is_multi_star = cat.map(|sys| sys.stars.len() > 1).unwrap_or(false);

            if is_multi_star {
                // Multi-star system: barycenter view
                let sys = cat.unwrap();
                let desc = if sys.description.is_empty() {
                    match sys.stars.len() {
                        2 => "Binary star system",
                        3 => "Triple star system",
                        4 => "Quadruple star system",
                        _ => "Multiple star system",
                    }
                } else {
                    sys.description
                };
                let catalog_stars: Vec<CatalogStarInfo> = sys.stars.iter().map(|st| CatalogStarInfo {
                    name: st.name.to_string(),
                    spectral_type: st.spectral_type.to_string(),
                    mass_solar: st.mass_solar,
                    radius_solar: st.radius_solar,
                    luminosity_solar: st.luminosity_solar,
                }).collect();
                let catalog_zone = Some(sys.zone);
                let catalog_distance_ly = Some(sys.distance_ly);
                let catalog_spectral = Some(
                    sys.stars.iter().map(|st| st.spectral_type).collect::<Vec<_>>().join(" / ")
                );

                BodyInfoData {
                    name: s.format_name(),
                    description: desc.to_string(),
                    radius_m: 0.0,
                    surface_gravity_ms2: 0.0,
                    mass_kg: 0.0,
                    atmosphere_pressure_pa: None,
                    atmosphere_height_m: None,
                    orbit_semi_major_axis_m: Some(s.semi_major_axis_m),
                    orbit_eccentricity: Some(s.eccentricity as f64),
                    orbit_period_s: Some(s.orbital_period_s),
                    mineable_resources: vec![],
                    atmospheric_resources: vec![],
                    habitability_score: 0,
                    luminosity_solar: None,
                    star_type: None,
                    temperature_k: None,
                    soi_radius_m: Some(soi),
                    is_galactic_orbit: true,
                    catalog_stars,
                    catalog_planets: vec![],
                    catalog_zone,
                    catalog_distance_ly,
                    catalog_spectral,
                }
            } else {
                // Single star or procedural star
                let catalog_planets: Vec<CatalogPlanetInfo> = cat.map(|sys| {
                    sys.bodies.iter().map(|b| CatalogPlanetInfo {
                        name: b.name.to_string(),
                        designation: b.designation.to_string(),
                        temperature_k: b.temperature_k,
                        gravity_g: b.gravity_g,
                        habitability: b.habitability,
                        has_atmosphere: b.atmosphere.is_some(),
                        has_life: b.has_life,
                        is_moon: b.is_moon,
                        is_gas_giant: b.is_gas_giant,
                    }).collect()
                }).unwrap_or_default();
                let catalog_zone = cat.map(|sys| sys.zone);
                let catalog_distance_ly = cat.map(|sys| sys.distance_ly);
                let catalog_spectral = cat.map(|sys| {
                    sys.stars.iter().map(|st| st.spectral_type).collect::<Vec<_>>().join(" / ")
                });

                BodyInfoData {
                    name: s.format_name(),
                    description: cat.map(|sys| sys.description.to_string()).unwrap_or_default(),
                    radius_m: s.radius_m,
                    surface_gravity_ms2: surface_gravity,
                    mass_kg,
                    atmosphere_pressure_pa: None,
                    atmosphere_height_m: None,
                    orbit_semi_major_axis_m: Some(s.semi_major_axis_m),
                    orbit_eccentricity: Some(s.eccentricity as f64),
                    orbit_period_s: Some(s.orbital_period_s),
                    mineable_resources: vec![],
                    atmospheric_resources: vec![],
                    habitability_score: 0,
                    luminosity_solar: Some(s.luminosity as f64),
                    star_type: Some(s.star_type.to_string()),
                    temperature_k: Some(s.temperature as f64),
                    soi_radius_m: Some(soi),
                    is_galactic_orbit: true,
                    catalog_stars: vec![],
                    catalog_planets,
                    catalog_zone,
                    catalog_distance_ly,
                    catalog_spectral,
                }
            }
        })
    });

    match render_state.render_tracking_station(&body_names, super::WARP_LEVELS, game.warp_index, game.paused, &date_str, &tracking_vessels, &body_info, &game.colony_manager) {
        Ok((new_warp_index, pause_action, ts_action)) => {
            game.warp_index = new_warp_index;
            match pause_action {
                PauseAction::MainMenu => {
                    game.enter_main_menu();
                }
                PauseAction::Resume | PauseAction::None | PauseAction::RecoverVessel
                | PauseAction::Quicksave | PauseAction::LoadQuicksave(_)
                | PauseAction::RevertToLaunch | PauseAction::RevertToEditor => {}
            }
            // Handle tracking station actions
            match ts_action {
                crate::render::TrackingStationAction::FlyVessel(id) => {
                    // Pull vessel from inactive list and enter flight
                    match game.flight.activate_vessel(id, &game.solar_system) {
                        Ok(()) => {
                            render_state.maneuver_nodes = game.flight.active_maneuver_nodes.clone();
                            game.warp_index = 0;
                        }
                        Err(e) => log::error!("Failed to activate vessel: {}", e),
                    }
                    game.enter_flight();
                }
                crate::render::TrackingStationAction::FocusVessel(id) => {
                    // Focus camera on vessel and track it continuously
                    if let Some(vessel_data) = tracking_vessels.iter().find(|v| v.id == id) {
                        let soi_pos = scaled_positions[vessel_data.soi_body];
                        render_state.camera.body_center = [soi_pos[0] * super::SCALE, soi_pos[1] * super::SCALE];
                        render_state.camera.ship_offset = [
                            vessel_data.x - render_state.camera.body_center[0],
                            vessel_data.y - render_state.camera.body_center[1],
                        ];
                        render_state.camera.position[0] = vessel_data.x;
                        render_state.camera.position[1] = vessel_data.y;
                        render_state.tracked_body = None;
                        render_state.tracked_vessel = Some(id);
                    }
                }
                crate::render::TrackingStationAction::DeleteVessel(id) => {
                    game.flight.inactive_vessels.retain(|v| v.id != id);
                    // If we were tracking the deleted vessel, stop tracking
                    if render_state.tracked_vessel == Some(id) {
                        render_state.tracked_vessel = None;
                        render_state.focus_on_body(game.solar_system.earth_index);
                    }
                }
                crate::render::TrackingStationAction::FocusBody(bi) => {
                    render_state.focus_on_body(bi);
                }
                crate::render::TrackingStationAction::OpenColony(bi) => {
                    game.enter_colony(bi, GameMode::TrackingStation);
                }
                crate::render::TrackingStationAction::None => {}
            }
        }
        Err(wgpu::SurfaceError::Lost) => render_state.resize(render_state.size),
        Err(wgpu::SurfaceError::OutOfMemory) => panic!("OutOfMemory"),
        Err(e) => eprintln!("Tracking station render error: {:?}", e),
    }

    // Process notifications
    for notif in &mut game.notifications {
        if !notif.read && notif.kind.stops_warp() {
            game.warp_index = 0;
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        } else if !notif.read {
            render_state.active_toasts.push((notif.kind.message(), web_time::Instant::now()));
            notif.read = true;
        }
    }
    render_state.active_toasts.retain(|(_, t)| t.elapsed().as_secs_f32() < 5.0);
}

/// Render the vehicle editor.
pub fn render_editor_frame(
    game: &mut Game,
    render_state: &mut RenderState,
    dt: f32,
) {
    // Update camera position based on held keys
    game.editor.update_camera(dt);

    // Tick alert timer
    if game.editor.alert_timer > 0.0 {
        game.editor.alert_timer -= dt as f64;
        if game.editor.alert_timer <= 0.0 {
            game.editor.alert_message = None;
        }
    }

    let screen_width = render_state.size.width as f32;
    let screen_height = render_state.size.height as f32;

    // Set editor camera
    render_state.set_editor_camera(game.editor.camera_offset, game.editor.camera_zoom);

    // Generate editor geometry
    let mut vertices: Vec<Vertex> = Vec::new();

    // Grid
    vertices.extend(generate_grid_vertices(&game.editor, screen_width, screen_height));

    // Placed parts
    vertices.extend(generate_part_vertices(&game.editor, &game.part_definitions, Some(&render_state.sprite_atlas)));

    // Ghost preview
    vertices.extend(generate_ghost_vertices(&game.editor, &game.part_definitions, Some(&render_state.sprite_atlas)));

    // Get blueprint names for load dialog
    let blueprint_names: Vec<&str> = game.blueprints.names();

    // Clone what we need for the closure
    let part_defs = game.part_definitions.clone();

    // Calculate ship stats
    let stats = game.editor.calculate_stats(&part_defs);

    // Build body info list for TWR dropdown
    let bodies: Vec<BodyInfo> = game.solar_system.bodies.iter()
        .map(|b| BodyInfo {
            name: b.name.clone(),
            surface_gravity: b.surface_gravity(),
        })
        .collect();

    // Render with editor UI
    let mut action = EditorAction::None;
    let mut editor_pause_action = PauseAction::None;
    let paused = game.paused;

    let stage_dv_burns = game.editor.calculate_stage_delta_v(&part_defs);
    let stage_delta_vs: Vec<f64> = stage_dv_burns.iter().map(|(dv, _)| *dv).collect();
    let stage_burn_times: Vec<f64> = stage_dv_burns.iter().map(|(_, bt)| *bt).collect();
    let vessel_cost = game.editor.calculate_vessel_cost(&part_defs);
    let company_money = game.company.money;
    let mut show_contracts = render_state.show_contracts;

    // Precompute which blueprints have locked parts
    let locked_blueprints: std::collections::HashSet<String> = blueprint_names.iter()
        .filter(|name| {
            game.blueprints.get(name)
                .map_or(false, |bp| {
                    bp.parts.iter().any(|p| {
                        game.part_definitions.get(&p.definition_id)
                            .map_or(false, |def| !game.tech_tree.is_part_available(&def.name))
                    })
                })
        })
        .map(|name| name.to_string())
        .collect();

    // Build launch sites: Earth + colonies with launchpads
    let mut launch_sites: Vec<(usize, String)> = vec![
        (game.solar_system.earth_index, "Earth".to_string()),
    ];
    for colony in &game.colony_manager.colonies {
        if colony.body_index != game.solar_system.earth_index
            && colony.buildings.iter().any(|b| b.building_type == crate::colony::BuildingType::Launchpad && b.operational)
        {
            let name = game.solar_system.bodies.get(colony.body_index)
                .map(|b| b.name.clone())
                .unwrap_or_else(|| format!("Body {}", colony.body_index));
            launch_sites.push((colony.body_index, name));
        }
    }

    let result = render_state.render_editor(&vertices, |ctx| {
        action = render_editor_ui(
            ctx,
            &mut game.editor,
            &part_defs,
            &blueprint_names,
            &locked_blueprints,
            &stats,
            &bodies,
            &stage_delta_vs,
            &stage_burn_times,
            company_money,
            vessel_cost,
            &game.contracts,
            &game.tech_tree,
            &launch_sites,
        );

        // Contract board window
        if show_contracts {
            let mut editor_contract_action = crate::render::ManagementAction::None;
            egui::Window::new("Contracts")
                .open(&mut show_contracts)
                .default_width(400.0)
                .resizable(true)
                .show(ctx, |ui| {
                    crate::render::management_ui::render_contracts_section(
                        ui,
                        &game.contracts,
                        &mut editor_contract_action,
                    );
                });
            match editor_contract_action {
                crate::render::ManagementAction::AcceptContract(id) => {
                    game.contracts.accept(id);
                }
                crate::render::ManagementAction::CancelContract(id) => {
                    let cancelled_ids = game.contracts.cancel(id);
                    // Remove payloads from editor parts for cancelled contracts
                    for part in game.editor.parts.values_mut() {
                        part.cargo_payloads.retain(|p| !cancelled_ids.contains(&p.contract_id));
                    }
                    game.contracts.refill_one(
                        &game.science.discoveries,
                        &game.solar_system,
                        game.solar_system.time,
                    );
                }
                _ => {}
            }
        }

        // Pause overlay on top of editor
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
                                    editor_pause_action = PauseAction::MainMenu;
                                }
                            });
                        });
                });
        }
    });
    render_state.show_contracts = show_contracts;

    // Handle editor actions
    match action {
        EditorAction::Launch => {
            let launch_body = game.editor.launch_body;
            game.flight.recover_vessels_on_launchpad(&game.solar_system, launch_body);
            match game.launch_from_editor(launch_body) {
                Ok(()) => {
                    // Zoom camera to see the vessel on the surface
                    if let Some(ref vessel) = game.flight.vessel {
                        let vessel_world_size = vessel.bounding_half_height() * 2.0 * super::SCALE * super::BODY_SCALE;
                        // We want the vessel to take up ~1/4 of the screen height
                        // pixels = vessel_world_size * zoom * screen_height / 2
                        // We want pixels ≈ screen_height / 4
                        // So zoom = screen_height/4 / (vessel_world_size * screen_height/2)
                        //         = 1 / (2 * vessel_world_size)
                        let target_fraction = 0.25;
                        let zoom = target_fraction / vessel_world_size as f32;
                        render_state.camera.zoom = zoom;
                    }
                    // Create launch save for "Revert to Launch"
                    if let Some(ref name) = game.save_name {
                        let save = SaveGame::from_game(game, name);
                        match save.write_launch_save() {
                            Ok(()) => {
                                game.has_launch_save = true;
                                log::info!("Created launch save");
                            }
                            Err(e) => log::error!("Failed to create launch save: {}", e),
                        }
                    }
                    log::info!("Launched vessel");
                }
                Err(e) => {
                    log::error!("Failed to launch: {}", e);
                    game.editor.alert_message = Some(e);
                    game.editor.alert_timer = 3.0;
                }
            }
        }
        EditorAction::SaveBlueprint(name) => {
            match game.save_blueprint(name) {
                Ok(()) => log::info!("Blueprint saved"),
                Err(e) => log::error!("Failed to save: {}", e),
            }
        }
        EditorAction::LoadBlueprint(name) => {
            match game.load_blueprint(&name) {
                Ok(()) => log::info!("Blueprint loaded"),
                Err(e) => {
                    log::error!("Failed to load: {}", e);
                    game.editor.alert_message = Some(e);
                    game.editor.alert_timer = 4.0;
                }
            }
        }
        EditorAction::DeleteBlueprint(name) => {
            match game.blueprints.delete(&name) {
                Ok(()) => log::info!("Blueprint deleted: {}", name),
                Err(e) => log::error!("Failed to delete blueprint: {}", e),
            }
        }
        EditorAction::ImportBlueprint => {
            crate::save::io::import_blueprint(&render_state.window);
            // Refresh the in-memory registry so the imported blueprint appears
            // in the palette. Native: blocks on rfd, registry is up-to-date.
            // Wasm: spawn_local writes to IDB async; load_all reads from the
            // synchronous IDB cache that write_blueprint updates immediately.
            if let Err(e) = game.blueprints.load_all() {
                log::warn!("Refresh after import: {}", e);
            }
        }
        EditorAction::ExportBlueprint(name) => {
            crate::save::io::export_blueprint(&render_state.window, &name);
        }
        EditorAction::NewVessel => {
            game.new_vessel();
        }
        EditorAction::OpenContracts => {
            render_state.show_contracts = true;
        }
        EditorAction::None => {}
    }

    // Handle pause action
    match editor_pause_action {
        PauseAction::MainMenu => {
            game.enter_main_menu();
        }
        PauseAction::Resume | PauseAction::None | PauseAction::RecoverVessel
        | PauseAction::Quicksave | PauseAction::LoadQuicksave(_)
        | PauseAction::RevertToLaunch | PauseAction::RevertToEditor => {}
    }

    // Process any pending part deletions
    game.editor.process_pending_delete();

    if let Err(e) = result {
        match e {
            wgpu::SurfaceError::Lost => render_state.resize(render_state.size),
            wgpu::SurfaceError::OutOfMemory => panic!("OutOfMemory"),
            e => eprintln!("Editor render error: {:?}", e),
        }
    }
}
