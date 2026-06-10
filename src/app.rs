use std::sync::Arc;
use web_time::{Duration, Instant};
use winit::{
    event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::Key,
};

#[path = "frames.rs"]
mod frames;

use crate::editor::{screen_to_world, part_at_screen_pos};
use crate::bodies::G;
use crate::game::{Game, GameMode};
use crate::render::{RenderState, PauseAction, OrbitRenderData, ShipRenderData, ShipOrbitData, ShipPartRenderData, OrbitSegmentData, SelectedTarget, StagedPartInfo, TargetPopup, BodyInfoData};
use crate::save::SaveGame;
use crate::ship::{AutopilotTarget, ShipState, VesselPhysicsData, SHIP_SIZE, MAX_THRUST_ACCELERATION, AMBIENT_TEMPERATURE, RAILS_WARP_THRESHOLD};
use crate::parts::{default_heat_tolerance, GRID_SQUARE_SIZE};

// 1:1 Real-Scale Solar System Simulation
// All physics use real-world values: masses, radii, distances, orbital velocities
// Rendering scale: 1 world unit = 1 billion meters (1e9 m)
const SCALE: f64 = 1e-9;

// Time warp levels (simulation seconds per real second)
const WARP_LEVELS: &[f64] = &[1.0, 2.0, 3.0, 5.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 1000000.0, 10000000.0, 100000000.0, 1000000000.0, 10000000000.0, 100000000000.0, 1000000000000.0];

// Visual scale factor for bodies (1.0 = real proportions, no artificial enlargement)
const BODY_SCALE: f64 = 1.0;

/// Galaxy view threshold scales with distance from galactic center.
/// Near Sgr A* (dense region): 144 ly screen span triggers galaxy view.
/// At Sun's distance (26,000 ly) and beyond: 640 ly screen span.
/// Linear interpolation between the two.
fn galaxy_view_threshold_m(camera_center: [f64; 2]) -> f64 {
    const LY: f64 = 9.461e15;
    const MIN_THRESHOLD: f64 = 144.0 * LY;
    const MAX_THRESHOLD: f64 = 640.0 * LY;
    const SUN_DISTANCE_M: f64 = 26_000.0 * LY;

    let dist_m = (camera_center[0] * camera_center[0]
                + camera_center[1] * camera_center[1]).sqrt();
    let t = (dist_m / SUN_DISTANCE_M).clamp(0.0, 1.0);
    MIN_THRESHOLD + t * (MAX_THRESHOLD - MIN_THRESHOLD)
}

fn is_galaxy_view(camera_zoom: f32, camera_body_center: [f64; 2]) -> bool {
    let screen_span_m = 2.0 / (camera_zoom as f64 * SCALE);
    // camera_body_center is in scaled coordinates, convert to meters
    let center_m = [
        camera_body_center[0] / SCALE,
        camera_body_center[1] / SCALE,
    ];
    screen_span_m >= galaxy_view_threshold_m(center_m)
}

/// Returns true if `body_idx` is equal to or nested inside `soi_body`'s SOI.
fn is_in_soi_of(body_idx: usize, soi_body: usize, bodies: &[crate::bodies::CelestialBody]) -> bool {
    let mut idx = body_idx;
    loop {
        if idx == soi_body { return true; }
        match bodies[idx].parent {
            Some(parent) => idx = parent,
            None => return false,
        }
    }
}

/// Check if a trajectory segment around `seg_parent` should be hidden because
/// the body between `from_body` and `seg_parent` in the SOI chain is big enough
/// on screen that its orbit line would be hidden by the pixel threshold.
fn segment_hidden_by_ancestor_threshold(
    from_body: usize,
    seg_parent: usize,
    bodies: &[crate::bodies::CelestialBody],
    pixels_per_world_unit: f32,
) -> bool {
    // Walk from from_body up the parent chain to find the direct child of seg_parent
    let mut idx = from_body;
    loop {
        match bodies[idx].parent {
            Some(p) if p == seg_parent => {
                // idx orbits seg_parent — check if idx is big enough to hide its orbit
                let body_world_radius = (bodies[idx].radius * BODY_SCALE * SCALE) as f32;
                let body_pixels = body_world_radius * pixels_per_world_unit * 2.0;
                let is_moon = bodies[seg_parent].parent
                    .map_or(false, |gp| bodies[gp].parent.is_some());
                let pixel_threshold = if is_moon { 100.0 } else { 5.0 };
                return body_pixels >= pixel_threshold;
            }
            Some(p) => idx = p,
            None => return false, // from_body is not in seg_parent's SOI chain
        }
    }
}

/// Compute the innermost body whose circle dominates the screen and whose SOI
/// contains the camera. Returns `None` in galaxy view (no filtering desired).
fn compute_view_soi_body(game: &Game, render_state: &RenderState) -> Option<usize> {
    if is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center) {
        return None;
    }
    let pixels_per_world_unit = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    let cam_x = render_state.camera.position[0] as f64;
    let cam_y = render_state.camera.position[1] as f64;
    let screen_threshold = render_state.size.height as f32 * 0.5;

    let mut best: Option<(usize, f64)> = None; // (body_index, radius)
    for (i, body) in game.solar_system.bodies.iter().enumerate() {
        let body_world_radius = body.radius * BODY_SCALE * SCALE;
        let body_pixels = (body_world_radius as f32) * pixels_per_world_unit * 2.0;
        if body_pixels <= screen_threshold {
            continue; // Body isn't prominent enough on screen
        }
        // Camera must be within the body's SOI
        let pos = game.solar_system.body_position(i);
        let bx = pos[0] * SCALE * BODY_SCALE;
        let by = pos[1] * SCALE * BODY_SCALE;
        let dist = ((cam_x - bx).powi(2) + (cam_y - by).powi(2)).sqrt();
        let soi_world = body.soi_radius * SCALE * BODY_SCALE;
        if dist > soi_world {
            continue;
        }
        // Pick the smallest qualifying body (innermost SOI)
        if best.map_or(true, |(_, r)| body.radius < r) {
            best = Some((i, body.radius));
        }
    }
    best.map(|(idx, _)| idx)
}

/// Save the current game and return to the title screen
fn save_and_quit_to_title(game: &mut Game, render_state: &mut RenderState) {
    if let Some(ref name) = game.save_name {
        game.flight.active_maneuver_nodes = render_state.maneuver_nodes.clone();
        let save = SaveGame::from_game(game, name);
        if let Err(e) = save.write_to_file() {
            log::error!("Failed to save game: {}", e);
        }
    }
    game.enter_title_screen();
}

pub async fn run() -> Result<(), String> {
    log::info!("Sunscatter starting...");
    log::info!("Flight Controls:");
    log::info!("  Escape: Pause / Menu");
    log::info!("  Left Shift / Left Ctrl: Throttle up/down");
    log::info!("  Z/X: Full/cut throttle");
    log::info!("  Q/E: Rotate ship");
    log::info!("  WASD: RCS translation (when RCS enabled)");
    log::info!("  Space: Stage");
    log::info!("  R: Toggle RCS");
    log::info!("  [ / ]: Switch vessel");
    log::info!("  ` (backtick): Focus on ship");
    log::info!("  Left mouse drag: Pan camera");
    log::info!("  Scroll wheel: Zoom in/out");
    log::info!("  Double-click: Focus on body / switch to vessel");
    log::info!("Editor Controls:");
    log::info!("  Left click: Place / select part");
    log::info!("  R: Rotate part");
    log::info!("  Delete / Backspace: Delete part");
    log::info!("  Escape: Deselect");
    log::info!("  Arrow keys or drag: Pan camera");
    log::info!("  Scroll wheel: Zoom");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let window = Arc::new(
        crate::platform::make_window_builder("Sunscatter", (1280, 720))
            .build(&event_loop)
            .unwrap(),
    );

    crate::platform::install_resize_handler(window.clone());

    // Hydrate the IndexedDB-backed save cache before constructing Game so that
    // the title-screen "Load Game" list and editor blueprint palette already
    // know about saves persisted in prior sessions.
    #[cfg(target_arch = "wasm32")]
    {
        if let Err(e) = crate::save::wasm_storage::init_storage().await {
            log::error!("IndexedDB init failed (saves won't persist this session): {e}");
        }
    }

    let t0 = Instant::now();
    let mut game = Game::new();
    log::info!("  Game::new(): {:.0?}", t0.elapsed());
    log::info!(
        "  Catalog: {} parts, {} blueprints, {} tech nodes",
        game.part_definitions.len(),
        game.blueprints.len(),
        game.tech_tree.nodes.len(),
    );

    let body_names: Vec<String> = game.solar_system.bodies.iter().map(|b| b.name.clone()).collect();

    let t1 = Instant::now();
    let mut render_state = RenderState::new(window.clone(), &body_names).await;
    log::info!("  RenderState::new(): {:.0?}", t1.elapsed());
    log::info!("Startup total: {:.0?}", t0.elapsed());
    let mut last_frame = Instant::now();

    // Double-click detection
    let mut last_click_time: Option<Instant> = None;
    let mut last_click_pos: [f32; 2] = [0.0, 0.0];

    // Auto-save timer (every 5 minutes)
    let mut last_autosave = Instant::now();
    const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(300);

    // Cached quicksave list for pause overlay
    let mut cached_quicksaves: Vec<crate::save::QuicksaveInfo> = Vec::new();
    let mut quicksaves_dirty = true;

    // Initial camera: focus on Sun, zoomed out to see all planets
    {
        let sun_pos = game.solar_system.body_position(game.solar_system.sun_index);
        render_state.camera.position[0] = sun_pos[0] * SCALE * BODY_SCALE;
        render_state.camera.position[1] = sun_pos[1] * SCALE * BODY_SCALE;
        render_state.camera.body_center = render_state.camera.position;
        render_state.camera.ship_offset = [0.0, 0.0];
        render_state.camera.zoom = 0.002; // Zoomed out to see full solar system
    }

    crate::platform::spawn_event_loop(event_loop, move |event, elwt| {
            match event {
                Event::WindowEvent { ref event, .. } => {
                    // Pass event to egui first
                    let egui_consumed = render_state.handle_event(event);

                    match event {
                        WindowEvent::CloseRequested => {
                            // Save before closing
                            if let Some(ref name) = game.save_name {
                                game.flight.active_maneuver_nodes = render_state.maneuver_nodes.clone();
                                let save = SaveGame::from_game(&game, name);
                                if let Err(e) = save.write_to_file() {
                                    log::error!("Failed to save on close: {}", e);
                                } else {
                                    log::info!("Saved on close");
                                }
                            }
                            elwt.exit();
                        }
                        WindowEvent::Resized(physical_size) => {
                            render_state.resize(*physical_size);
                        }
                        WindowEvent::RedrawRequested => {
                            // Calculate delta time
                            let now = Instant::now();
                            let dt = now.duration_since(last_frame).as_secs_f32();
                            last_frame = now;

                            // Update FPS (exponential moving average)
                            if dt > 0.0 {
                                let instant_fps = 1.0 / dt;
                                render_state.fps = render_state.fps * 0.95 + instant_fps * 0.05;
                            }

                            // Drain any completed body-texture fetches and
                            // refresh body_idx → layer mappings so menus,
                            // tracking station, colony, etc. pick up newly
                            // arrived textures (not just flight mode).
                            // No-op on native. Catalog planets are still
                            // registered inside render_flight_frame.
                            let real_body_names: Vec<String> = game
                                .solar_system
                                .bodies
                                .iter()
                                .map(|b| b.name.clone())
                                .collect();
                            render_state.tick_body_textures(&real_body_names);

                            match game.mode {
                                GameMode::TitleScreen => {
                                    frames::render_title_screen_frame(
                                        &mut game,
                                        &mut render_state,
                                        elwt,
                                    );
                                }
                                GameMode::MainMenu => {
                                    frames::render_main_menu_frame(
                                        &mut game,
                                        &mut render_state,
                                        elwt,
                                    );
                                }
                                GameMode::Flight => {
                                    // Refresh quicksave cache when paused and dirty
                                    if game.paused && quicksaves_dirty {
                                        if let Some(ref name) = game.save_name {
                                            cached_quicksaves = SaveGame::list_quicksaves(name);
                                        }
                                        quicksaves_dirty = false;
                                    }
                                    render_flight_frame(
                                        &mut game,
                                        &mut render_state,
                                        &mut cached_quicksaves,
                                        &mut quicksaves_dirty,
                                        dt,
                                    );
                                }
                                GameMode::Editor => {
                                    frames::render_editor_frame(
                                        &mut game,
                                        &mut render_state,
                                        dt,
                                    );
                                }
                                GameMode::TrackingStation => {
                                    frames::render_tracking_station_frame(
                                        &mut game,
                                        &mut render_state,
                                    );
                                }
                                GameMode::Colony => {
                                    frames::render_colony_frame(
                                        &mut game,
                                        &mut render_state,
                                    );
                                }
                                GameMode::ColonyOverview => {
                                    frames::render_colony_overview_frame(
                                        &mut game,
                                        &mut render_state,
                                    );
                                }
                                GameMode::Management => {
                                    frames::render_management_frame(
                                        &mut game,
                                        &mut render_state,
                                    );
                                }
                                GameMode::TechTree => {
                                    frames::render_tech_tree_frame(
                                        &mut game,
                                        &mut render_state,
                                    );
                                }
                            }

                            // Auto-save check
                            if game.save_name.is_some() && last_autosave.elapsed() >= AUTOSAVE_INTERVAL {
                                game.flight.active_maneuver_nodes = render_state.maneuver_nodes.clone();
                                let save = SaveGame::from_game(&game, game.save_name.as_ref().unwrap());
                                if let Err(e) = save.write_to_file() {
                                    log::error!("Auto-save failed: {}", e);
                                } else {
                                    log::info!("Auto-saved");
                                }
                                last_autosave = Instant::now();
                            }
                        }

                        WindowEvent::MouseInput { state, button, .. } => {
                            match game.mode {
                                GameMode::TitleScreen | GameMode::MainMenu | GameMode::Colony
                                | GameMode::ColonyOverview | GameMode::Management | GameMode::TechTree => {
                                    // egui-only handling (buttons in menu)
                                }
                                GameMode::Flight => {
                                    handle_flight_mouse_input(
                                        &mut game,
                                        &mut render_state,
                                        *state,
                                        *button,
                                        egui_consumed,
                                        &mut last_click_time,
                                        &mut last_click_pos,
                                    );
                                }
                                GameMode::Editor => {
                                    handle_editor_mouse_input(
                                        &mut game,
                                        &mut render_state,
                                        *state,
                                        *button,
                                        egui_consumed,
                                    );
                                }
                                GameMode::TrackingStation => {
                                    handle_tracking_station_mouse_input(
                                        &mut game,
                                        &mut render_state,
                                        *state,
                                        *button,
                                        egui_consumed,
                                        &mut last_click_time,
                                        &mut last_click_pos,
                                    );
                                }
                            }
                        }

                        WindowEvent::CursorMoved { position, .. } => {
                            let x = position.x as f32;
                            let y = position.y as f32;

                            match game.mode {
                                GameMode::TitleScreen | GameMode::MainMenu | GameMode::Colony
                                | GameMode::ColonyOverview | GameMode::Management | GameMode::TechTree => {
                                    render_state.camera.last_mouse_pos = [x, y];
                                }
                                GameMode::Flight => {
                                    handle_flight_cursor_moved(
                                        &mut render_state,
                                        x, y,
                                        egui_consumed,
                                    );
                                }
                                GameMode::Editor => {
                                    handle_editor_cursor_moved(
                                        &mut game,
                                        &mut render_state,
                                        x, y,
                                        egui_consumed,
                                    );
                                }
                                GameMode::TrackingStation => {
                                    handle_tracking_station_cursor_moved(
                                        &mut render_state,
                                        x, y,
                                        egui_consumed,
                                    );
                                }
                            }
                        }

                        WindowEvent::MouseWheel { delta, .. } => {
                            if !egui_consumed {
                                let scroll_amount = match delta {
                                    MouseScrollDelta::LineDelta(_, y) => *y,
                                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                                };
                                let zoom_factor = (1.1_f32).powf(scroll_amount);

                                match game.mode {
                                    GameMode::Flight | GameMode::TrackingStation | GameMode::Colony
                                    | GameMode::ColonyOverview | GameMode::Management | GameMode::TechTree => {
                                        render_state.camera.zoom_by(zoom_factor);
                                    }
                                    GameMode::TitleScreen | GameMode::MainMenu => {
                                        // Camera is locked at fixed zoom
                                    }
                                    GameMode::Editor => {
                                        game.editor.zoom_camera(zoom_factor);
                                    }
                                }
                            }
                        }

                        WindowEvent::KeyboardInput {
                            event: KeyEvent {
                                logical_key,
                                state,
                                ..
                            },
                            ..
                        } => {
                            let pressed = *state == ElementState::Pressed;

                            // Universal Escape: toggle pause in all modes
                            // In editor: deselect first, then pause on next press
                            let escape_pressed = pressed && matches!(logical_key, Key::Named(winit::keyboard::NamedKey::Escape));
                            if escape_pressed && game.mode == GameMode::Editor
                                && game.editor.fairing_build_mode.is_some()
                            {
                                game.editor.exit_fairing_build_mode();
                            } else if escape_pressed && game.mode == GameMode::Editor
                                && (game.editor.selected_part_def.is_some() || game.editor.selected_placed_part.is_some())
                            {
                                game.editor.deselect();
                            } else if escape_pressed && render_state.show_quicksave_list {
                                render_state.show_quicksave_list = false;
                            } else if escape_pressed {
                                game.toggle_pause();
                                if !game.paused {
                                    render_state.show_quicksave_list = false;
                                } else {
                                    quicksaves_dirty = true;
                                }
                            } else if !game.paused {
                                match game.mode {
                                    GameMode::TitleScreen | GameMode::MainMenu | GameMode::TrackingStation | GameMode::Colony
                                    | GameMode::ColonyOverview | GameMode::Management | GameMode::TechTree => {
                                        // No keyboard shortcuts in these modes
                                    }
                                    GameMode::Flight => {
                                        handle_flight_keyboard(
                                            &mut game,
                                            &mut render_state,
                                            logical_key,
                                            pressed,
                                        );
                                    }
                                    GameMode::Editor => {
                                        handle_editor_keyboard(
                                            &mut game,
                                            logical_key,
                                            pressed,
                                            egui_consumed,
                                        );
                                    }
                                }
                            }
                        }

                        _ => {}
                    }
                }
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        });

    Ok(())
}

/// Render a flight mode frame
fn render_flight_frame(
    game: &mut Game,
    render_state: &mut RenderState,
    cached_quicksaves: &mut Vec<crate::save::QuicksaveInfo>,
    quicksaves_dirty: &mut bool,
    frame_dt: f32,
) {
    // Use actual frame time, clamped to avoid spiral-of-death on hitches
    let dt = (frame_dt as f64).clamp(0.0001, 0.1);

    // Sync earth_index for render state
    render_state.earth_index = game.solar_system.earth_index;

    // Check if the vessel has a functioning command pod
    let has_control = game.flight.vessel.as_ref()
        .map_or(true, |v| v.has_control(&game.part_definitions));

    // Power system state (updated each frame, read when building render data)
    let mut power_generation = 0.0_f64;
    let mut power_consumption = 0.0_f64;

    // --- Simulation (skipped when paused) ---
    let vessel_physics = if !game.paused {
        // Auto-drop on-rails warp when in atmosphere/surface or approaching it.
        // Uses time_to_distance(landing_r) to find when the orbit will cross
        // the landing altitude (atmosphere top or 1% radius for airless bodies).
        // Only when flying — landed ships can warp at any speed (update_landed is analytical).
        // Physics warp (10x and below) is unaffected — numerical integration handles it.
        if matches!(game.flight.ship.state, ShipState::Flying)
            && WARP_LEVELS[game.warp_index] > RAILS_WARP_THRESHOLD
        {
            let soi_body = &game.solar_system.bodies[game.flight.ship.soi_body];
            let dist = (game.flight.ship.rel_position[0].powi(2)
                      + game.flight.ship.rel_position[1].powi(2)).sqrt();
            let altitude = dist - soi_body.radius;
            let danger_altitude = soi_body.landing_altitude();

            if altitude < danger_altitude {
                // Already in landing zone — drop to 1x
                game.warp_index = 0;
            } else {
                // Step down smoothly based on time to landing altitude
                let landing_r = soi_body.radius + danger_altitude;
                if let Some(ttd) = game.flight.ship.time_to_distance(&game.solar_system, landing_r) {
                    const SAFE_FRAMES: f64 = 10.0;
                    let max_safe_warp = ttd / (dt as f64 * SAFE_FRAMES);
                    let safe_index = WARP_LEVELS.iter()
                        .rposition(|&w| w <= max_safe_warp)
                        .unwrap_or(0);
                    if safe_index < game.warp_index {
                        game.warp_index = safe_index;
                    }
                }
            }
        }

        // Update simulation with current time warp
        let time_warp = WARP_LEVELS[game.warp_index];

        // Auto-disable RCS when entering on-rails warp, re-enable when returning to physics warp
        let on_rails_warp = time_warp > RAILS_WARP_THRESHOLD;
        if on_rails_warp && render_state.rcs_enabled {
            render_state.rcs_enabled = false;
            render_state.rcs_disabled_by_rails = true;
        } else if !on_rails_warp && render_state.rcs_disabled_by_rails {
            render_state.rcs_enabled = true;
            render_state.rcs_disabled_by_rails = false;
        }

        game.solar_system.update(dt * time_warp);

        // Determine autopilot state and desired direction (before gimbal update)
        // No control → force autopilot off
        let autopilot_target = if has_control {
            render_state.get_autopilot_target()
        } else {
            render_state.autopilot_target = AutopilotTarget::Off;
            AutopilotTarget::Off
        };
        let autopilot_active = autopilot_target != AutopilotTarget::Off && !game.flight.ship.on_rails;
        let autopilot_target_angle = if autopilot_active {
            if autopilot_target == AutopilotTarget::Target {
                // Target angle is precomputed by the render loop
                render_state.selected_target_angle
            } else {
                let maneuver_node = render_state.get_selected_maneuver_node();
                game.flight.ship.autopilot_target_angle(autopilot_target, maneuver_node)
            }
        } else {
            None
        };

        // Build VesselPhysicsData from flight vessel (before gimbal update for stopping distance calc)
        let rcs_enabled = render_state.rcs_enabled;
        let mut vessel_physics = game.flight.vessel.as_ref().map(|v| VesselPhysicsData {
            total_mass: v.total_mass,
            max_thrust_vac: v.active_thrust_vac(),
            max_thrust_asl: v.active_thrust_asl(),
            vessel_height: v.bounding_half_height(),
            bottom_extent: v.bottom_extent(),
            moment_of_inertia: v.moment_of_inertia,
            rcs_torque: if rcs_enabled { v.compute_rcs_torque(&game.part_definitions) } else { 0.0 },
            gimbal_torque: v.compute_gimbal_torque(),
            max_gimbal_torque: v.compute_max_gimbal_torque(),
            vessel_half_width: v.bounding_half_width(),
            rcs_translation_force: if rcs_enabled { v.compute_rcs_translation_force(&game.part_definitions) } else { 0.0 },
            parachute_drag_width: v.parachute_drag_width(),
            parachute_drag_multiplier: v.parachute_drag_multiplier(),
        });

        // Update engine gimbal angles: driven by autopilot when SAS active, else by A/D input
        if let Some(ref mut vessel) = game.flight.vessel {
            let gimbal_command = if let Some(target_angle) = autopilot_target_angle {
                game.flight.ship.autopilot_desired_direction(target_angle, vessel_physics.as_ref())
            } else if game.flight.ship_input.rotate_left {
                1.0
            } else if game.flight.ship_input.rotate_right {
                -1.0
            } else {
                0.0
            };
            vessel.update_gimbal(gimbal_command);
            // Refresh gimbal torque to reflect new gimbal angles
            if let Some(ref mut vp) = vessel_physics {
                vp.gimbal_torque = vessel.compute_gimbal_torque();
            }
        }

        // Compute RCS translation from input (only when RCS enabled)
        game.flight.ship.rcs_translate = if rcs_enabled {
            let fwd = if game.flight.ship_input.translate_forward { 1.0 } else { 0.0 }
                    - if game.flight.ship_input.translate_backward { 1.0 } else { 0.0 };
            let right = if game.flight.ship_input.translate_right { 1.0 } else { 0.0 }
                      - if game.flight.ship_input.translate_left { 1.0 } else { 0.0 };
            [fwd, right]
        } else {
            [0.0, 0.0]
        };

        // No control → zero all inputs and throttle
        if !has_control {
            game.flight.ship_input = crate::ship::ShipInput::default();
            game.flight.ship.throttle = 0.0;
        }

        // Update ship physics (gimbal torque always applied in update_flying)
        let has_flight_vessel = game.flight.vessel.is_some();
        let prev_soi = game.flight.ship.soi_body;
        game.flight.ship.update(dt * time_warp, time_warp, &game.flight.ship_input, &game.solar_system, vessel_physics.as_ref(), autopilot_active, has_flight_vessel);
        if let Some(target_angle) = autopilot_target_angle {
            game.flight.ship.autopilot_rotate(target_angle, dt * time_warp, vessel_physics.as_ref());
        }

        // Detect SOI transition → notify and drop out of warp.
        if game.flight.ship.soi_body != prev_soi {
            let body_name = game.solar_system.bodies[game.flight.ship.soi_body].name.clone();
            game.notifications.push(crate::colony::Notification {
                kind: crate::colony::NotificationKind::SoiTransition { body_name },
                time: game.solar_system.time,
                read: false,
            });
            game.warp_index = 0;
        }

        // Fuel consumption and vessel sync
        if let Some(ref mut vessel) = game.flight.vessel {
            vessel.throttle = game.flight.ship.throttle;

            // Compute atmospheric pressure fraction for engine ISP interpolation
            let atmo_pressure = {
                let soi = &game.solar_system.bodies[game.flight.ship.soi_body];
                if let Some(ref atmo) = soi.atmosphere {
                    let dist = (game.flight.ship.rel_position[0].powi(2) + game.flight.ship.rel_position[1].powi(2)).sqrt();
                    let alt = dist - soi.radius;
                    if alt >= 0.0 && alt < atmo.visible_height() {
                        (atmo.pressure_at_altitude(alt) / 101_325.0).clamp(0.0, 1.0) // normalized to 1 atm
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            };

            // Always update engine states and consume fuel (updates active flags even at 0 throttle)
            let effective_dt = dt * time_warp;
            if !render_state.debug_infinite_fuel {
                vessel.consume_fuel(effective_dt, atmo_pressure, &game.part_definitions);
            } else {
                // Still update engine states without draining fuel
                vessel.update_engine_states(&game.part_definitions);
            }

            // Consume RCS fuel when rotating (manual or autopilot), only if RCS enabled
            let rcs_direction = if rcs_enabled {
                if autopilot_active {
                    if let Some(target_angle) = autopilot_target_angle {
                        game.flight.ship.autopilot_desired_direction(target_angle, vessel_physics.as_ref())
                    } else {
                        0.0
                    }
                } else if game.flight.ship_input.rotate_left {
                    1.0
                } else if game.flight.ship_input.rotate_right {
                    -1.0
                } else {
                    0.0
                }
            } else {
                0.0
            };
            vessel.consume_rcs_fuel(effective_dt, rcs_direction, &game.part_definitions);

            // Consume RCS fuel for translation
            vessel.consume_rcs_translation_fuel(effective_dt, game.flight.ship.rcs_translate, &game.part_definitions);

            vessel.recalculate_mass(&game.part_definitions);

            // Power update: compute sun distance and update electricity
            {
                let ship_abs = game.flight.ship.absolute_position(&game.solar_system);
                let sun_pos = game.solar_system.body_position(game.solar_system.sun_index);
                let dx = ship_abs[0] - sun_pos[0];
                let dy = ship_abs[1] - sun_pos[1];
                let sun_distance_m = (dx * dx + dy * dy).sqrt();
                let (gen, cons) = vessel.update_power(effective_dt, sun_distance_m, &game.part_definitions);
                power_generation = gen;
                power_consumption = cons;
            }

            // Ship-wide waste-heat pool: engines + reactors generate, deployed radiators reject,
            // pool temp can trip reactors and spill into per-part temperatures past damage threshold.
            if !game.flight.ship.on_rails {
                vessel.update_thermal_pool(effective_dt, &game.part_definitions);
            } else {
                // On-rails: no reactors running, so pool decays toward ambient via rejection capacity.
                // Snap to ambient when very close to avoid tiny drift over long warps.
                let drift = vessel.update_thermal_pool(effective_dt, &game.part_definitions);
                if (drift - 300.0).abs() < 0.1 {
                    vessel.thermal_pool_temp = 300.0;
                }
            }

            // Life support: greenhouse food production, crew consumption, starvation
            vessel.update_life_support(effective_dt, &game.part_definitions);

            // Animate solar panel + radiator deployment
            vessel.update_solar_deploy(effective_dt);

            // Animate parachute deployment and auto-retract
            let chute_altitude = {
                let dist = (game.flight.ship.rel_position[0].powi(2) + game.flight.ship.rel_position[1].powi(2)).sqrt();
                dist - game.solar_system.bodies[game.flight.ship.soi_body].radius
            };
            vessel.update_parachute_deploy(effective_dt, chute_altitude);
            let in_atmo = game.flight.ship.in_atmosphere(&game.solar_system);
            let is_landed = matches!(game.flight.ship.state, ShipState::Landed { .. });
            vessel.auto_retract_parachutes(in_atmo, is_landed);

            // Sync vessel state from ship
            vessel.rel_position = game.flight.ship.rel_position;
            vessel.rel_velocity = game.flight.ship.rel_velocity;
            vessel.rotation = game.flight.ship.rotation;

            // Per-part heat update
            if !game.flight.ship.on_rails {
                if let Some((density, airspeed, airspeed_dir_world)) = game.flight.ship.compute_aero_environment(&game.solar_system) {
                    // Convert airspeed direction from world to part-local coordinates.
                    // Ship physics uses X=forward (rotation=0 → nose along +X), but parts
                    // use Y=forward (editor convention: nose at +Y). We apply the inverse
                    // rotation with a -PI/2 offset to account for this.
                    let rot = game.flight.ship.rotation;
                    let cos_r = rot.cos();
                    let sin_r = rot.sin();
                    // First: physics-local (X=forward)
                    let phys_x = airspeed_dir_world[0] * cos_r + airspeed_dir_world[1] * sin_r;
                    let phys_y = -airspeed_dir_world[0] * sin_r + airspeed_dir_world[1] * cos_r;
                    // Rotate +90° to part-local (Y=forward): part_x = -phys_y, part_y = phys_x
                    let airspeed_dir_local = [-phys_y, phys_x];
                    vessel.update_part_temperatures(effective_dt, density, airspeed, airspeed_dir_local, &game.part_definitions);
                } else {
                    // No atmosphere — radiative cooling only
                    vessel.update_part_temperatures(effective_dt, 0.0, 0.0, [1.0, 0.0], &game.part_definitions);
                }
            } else {
                // On-rails: cool all parts toward ambient with exponential decay
                for part in &mut vessel.parts {
                    if part.destroyed || part.decoupled { continue; }
                    if part.temperature > 300.0 {
                        part.temperature += (300.0 - part.temperature) * (1.0 - (-0.01 * effective_dt).exp());
                        part.temperature = part.temperature.max(300.0);
                    }
                }
            }
            // Sync hottest part temperature back to ship for HUD
            let hottest = vessel.parts.iter()
                .filter(|p| !p.destroyed && !p.decoupled)
                .map(|p| p.temperature)
                .fold(300.0_f64, f64::max);
            game.flight.ship.temperature = hottest;

            // Per-part terrain collision check
            if matches!(game.flight.ship.state, ShipState::Flying) {
                let soi_body = &game.solar_system.bodies[game.flight.ship.soi_body];
                if let Some(surface_angle) = vessel.check_terrain_collision(
                    game.flight.ship.rel_position,
                    game.flight.ship.rotation,
                    soi_body.radius,
                    game.flight.ship.soi_body,
                    game.solar_system.earth_index,
                ) {
                    // If landing on launchpad, account for its height
                    let launchpad_offset = if game.flight.ship.soi_body == game.solar_system.earth_index {
                        let angle_diff = surface_angle - crate::game::LAUNCHPAD_SURFACE_ANGLE;
                        let angle_diff = angle_diff - (angle_diff / std::f64::consts::TAU).round() * std::f64::consts::TAU;
                        let half_angle = (crate::game::LAUNCHPAD_BOTTOM_WIDTH * 0.5)
                            / game.solar_system.bodies[game.flight.ship.soi_body].radius;
                        if angle_diff.abs() < half_angle {
                            crate::game::LAUNCHPAD_HEIGHT
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };
                    let surface_distance = soi_body.radius + launchpad_offset + vessel.bottom_extent();
                    game.flight.ship.rel_position = [
                        surface_distance * surface_angle.cos(),
                        surface_distance * surface_angle.sin(),
                    ];
                    game.flight.ship.rel_velocity = [0.0, 0.0];
                    game.flight.ship.throttle = 0.0;
                    game.flight.ship.rotation = surface_angle;
                    game.flight.ship.state = ShipState::Landed {
                        body_index: game.flight.ship.soi_body,
                        surface_angle,
                    };
                    game.flight.ship.on_rails = false;
                }
            }
        }

        // Apply burns to maneuver node delta-v (atmosphere-adjusted thrust)
        let current_accel = vessel_physics.as_ref()
            .map(|v| if v.total_mass > 0.0 {
                let soi = &game.solar_system.bodies[game.flight.ship.soi_body];
                let p = if let Some(ref atmo) = soi.atmosphere {
                    let dist = (game.flight.ship.rel_position[0].powi(2) + game.flight.ship.rel_position[1].powi(2)).sqrt();
                    let alt = dist - soi.radius;
                    if alt >= 0.0 && alt < atmo.visible_height() {
                        (atmo.pressure_at_altitude(alt) / 101_325.0).clamp(0.0, 1.0)
                    } else { 0.0 }
                } else { 0.0 };
                let thrust = v.max_thrust_vac * (1.0 - p) + v.max_thrust_asl * p;
                thrust / v.total_mass
            } else { 0.0 })
            .unwrap_or(MAX_THRUST_ACCELERATION);
        if game.flight.ship.throttle > 0.0 && render_state.get_selected_maneuver_node().is_some() {
            let delta_v_this_frame = game.flight.ship.throttle * current_accel * dt * time_warp;
            let burn_direction = [game.flight.ship.rotation.cos(), game.flight.ship.rotation.sin()];
            render_state.apply_burn_to_maneuver(burn_direction, delta_v_this_frame);
        }

        // Propagate inactive vessels on rails
        let dt_sim = dt * time_warp;
        for vessel in &mut game.flight.inactive_vessels {
            vessel.ship.ensure_on_rails(&game.solar_system);
            vessel.ship.update_on_rails(dt_sim, &game.solar_system);
        }
        // Delete vessels that entered atmosphere, hit surface, or whose orbit dips into atmosphere
        // Keep vessels within 3km of the active vessel even in atmosphere
        let active_pos = game.flight.ship.rel_position;
        let active_soi = game.flight.ship.soi_body;
        game.flight.inactive_vessels.retain(|v| {
            let in_landing_zone = v.ship.in_atmosphere(&game.solar_system)
                || v.ship.below_landing_altitude(&game.solar_system);
            if v.ship.periapsis_below_surface(&game.solar_system) && in_landing_zone {
                // Check if within 3km of active vessel
                if v.ship.soi_body == active_soi {
                    let dx = v.ship.rel_position[0] - active_pos[0];
                    let dy = v.ship.rel_position[1] - active_pos[1];
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < 3000.0 {
                        return true; // Keep — close to active vessel
                    }
                }
                false // Delete — in atmosphere, far from active vessel
            } else {
                true // Keep — not in danger zone
            }
        });

        // Delete debris vessels that are far from all controllable vessels
        game.flight.cleanup_distant_debris();

        // Check discovery milestones, government milestones, contracts, R&D science, and colony simulation
        game.check_discovery_milestones();
        game.check_government_milestones();
        game.check_contracts();
        game.update_rd_science(dt_sim);
        game.update_colonies(dt_sim);

        // Collision detection: active vs inactive vessels during physics warp (1x-10x)
        // Uses oriented bounding box (OBB) collision via Separating Axis Theorem
        if time_warp <= RAILS_WARP_THRESHOLD {
            if let Some(ref vessel) = game.flight.vessel {
                let active_hw = vessel.bounding_half_width();
                let active_hh = vessel.bounding_half_height();
                let active_rot = game.flight.ship.rotation;

                for inactive in &mut game.flight.inactive_vessels {
                    if inactive.ship.soi_body != active_soi { continue; }

                    let (inactive_hw, inactive_hh) = inactive.vessel.as_ref()
                        .map(|v| (v.bounding_half_width(), v.bounding_half_height()))
                        .unwrap_or((0.5, 1.0));
                    let inactive_rot = inactive.ship.rotation;

                    // Quick broad-phase: circle check with circumscribed radius
                    let dx = active_pos[0] - inactive.ship.rel_position[0];
                    let dy = active_pos[1] - inactive.ship.rel_position[1];
                    let dist_sq = dx * dx + dy * dy;
                    let max_r = (active_hw * active_hw + active_hh * active_hh).sqrt()
                        + (inactive_hw * inactive_hw + inactive_hh * inactive_hh).sqrt();
                    if dist_sq > max_r * max_r { continue; }

                    // Narrow-phase: SAT test on two oriented bounding boxes
                    if obb_overlap(
                        active_pos, active_rot, active_hw, active_hh,
                        inactive.ship.rel_position, inactive_rot, inactive_hw, inactive_hh,
                    ) {
                        let dist = dist_sq.sqrt().max(0.001);
                        let nx = dx / dist;
                        let ny = dy / dist;

                        // Separate: push active vessel out along collision normal
                        // Use a conservative separation distance
                        let overlap = (active_hw + inactive_hw).min(active_hh + inactive_hh);
                        game.flight.ship.rel_position[0] += nx * overlap * 0.5;
                        game.flight.ship.rel_position[1] += ny * overlap * 0.5;

                        // Bounce: reflect relative velocity along collision normal
                        let rel_vx = game.flight.ship.rel_velocity[0] - inactive.ship.rel_velocity[0];
                        let rel_vy = game.flight.ship.rel_velocity[1] - inactive.ship.rel_velocity[1];
                        let rel_dot_n = rel_vx * nx + rel_vy * ny;

                        // Only bounce if moving toward the inactive vessel
                        if rel_dot_n < 0.0 {
                            let restitution = 0.3;
                            let impulse = -(1.0 + restitution) * rel_dot_n;
                            game.flight.ship.rel_velocity[0] += impulse * nx;
                            game.flight.ship.rel_velocity[1] += impulse * ny;
                        }
                        break;
                    }
                }
            }
        }

        vessel_physics
    } else {
        // Paused: still need vessel_physics for HUD display
        let rcs_enabled = render_state.rcs_enabled;
        game.flight.vessel.as_ref().map(|v| VesselPhysicsData {
            total_mass: v.total_mass,
            max_thrust_vac: v.active_thrust_vac(),
            max_thrust_asl: v.active_thrust_asl(),
            vessel_height: v.bounding_half_height(),
            bottom_extent: v.bottom_extent(),
            moment_of_inertia: v.moment_of_inertia,
            rcs_torque: if rcs_enabled { v.compute_rcs_torque(&game.part_definitions) } else { 0.0 },
            gimbal_torque: v.compute_gimbal_torque(),
            max_gimbal_torque: v.compute_max_gimbal_torque(),
            vessel_half_width: v.bounding_half_width(),
            rcs_translation_force: if rcs_enabled { v.compute_rcs_translation_force(&game.part_definitions) } else { 0.0 },
            parachute_drag_width: v.parachute_drag_width(),
            parachute_drag_multiplier: v.parachute_drag_multiplier(),
        })
    };

    // Compute RCS rotation direction for plume rendering (0 if RCS disabled)
    let rcs_direction_for_render = if render_state.rcs_enabled {
        let autopilot_target = render_state.get_autopilot_target();
        let autopilot_active = autopilot_target != AutopilotTarget::Off && !game.flight.ship.on_rails;
        if autopilot_active {
            let autopilot_target_angle = if autopilot_target == AutopilotTarget::Target {
                render_state.selected_target_angle
            } else {
                let maneuver_node = render_state.get_selected_maneuver_node();
                game.flight.ship.autopilot_target_angle(autopilot_target, maneuver_node)
            };
            if let Some(target_angle) = autopilot_target_angle {
                let vessel_physics = game.flight.vessel.as_ref().map(|v| VesselPhysicsData {
                    total_mass: v.total_mass,
                    max_thrust_vac: v.active_thrust_vac(),
                    max_thrust_asl: v.active_thrust_asl(),
                    vessel_height: v.bounding_half_height(),
                    bottom_extent: v.bottom_extent(),
                    moment_of_inertia: v.moment_of_inertia,
                    rcs_torque: v.compute_rcs_torque(&game.part_definitions),
                    gimbal_torque: v.compute_gimbal_torque(),
            max_gimbal_torque: v.compute_max_gimbal_torque(),
                    vessel_half_width: v.bounding_half_width(),
                    rcs_translation_force: v.compute_rcs_translation_force(&game.part_definitions),
                    parachute_drag_width: v.parachute_drag_width(),
                    parachute_drag_multiplier: v.parachute_drag_multiplier(),
                });
                game.flight.ship.autopilot_desired_direction(target_angle, vessel_physics.as_ref())
            } else {
                0.0
            }
        } else if game.flight.ship_input.rotate_left {
            1.0
        } else if game.flight.ship_input.rotate_right {
            -1.0
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Compute RCS translation for plume rendering (only when RCS enabled)
    let rcs_translate_for_render = if render_state.rcs_enabled {
        game.flight.ship.rcs_translate
    } else {
        [0.0, 0.0]
    };

    // Suppress RCS plumes when vessel has no monopropellant
    let (rcs_direction_for_render, rcs_translate_for_render) = if let Some(ref vessel) = game.flight.vessel {
        let (mono_current, _) = vessel.total_monopropellant();
        if mono_current < 0.001 {
            (0.0, [0.0, 0.0])
        } else {
            (rcs_direction_for_render, rcs_translate_for_render)
        }
    } else {
        (rcs_direction_for_render, rcs_translate_for_render)
    };

    // --- Rendering (always runs) ---

    // Compute atmospheric pressure fraction for HUD thrust display
    let hud_atmo_pressure = {
        let soi = &game.solar_system.bodies[game.flight.ship.soi_body];
        if let Some(ref atmo) = soi.atmosphere {
            let dist = (game.flight.ship.rel_position[0].powi(2) + game.flight.ship.rel_position[1].powi(2)).sqrt();
            let alt = dist - soi.radius;
            if alt >= 0.0 && alt < atmo.visible_height() {
                (atmo.pressure_at_altitude(alt) / 101_325.0).clamp(0.0, 1.0)
            } else {
                0.0
            }
        } else {
            0.0
        }
    };

    // Compute current acceleration for HUD (atmosphere-adjusted)
    let current_accel = vessel_physics.as_ref()
        .map(|v| if v.total_mass > 0.0 {
            let thrust = v.max_thrust_vac * (1.0 - hud_atmo_pressure) + v.max_thrust_asl * hud_atmo_pressure;
            thrust / v.total_mass
        } else { 0.0 })
        .unwrap_or(MAX_THRUST_ACCELERATION);

    // Collect body data for rendering
    let mut scaled_positions: Vec<[f64; 2]> = Vec::with_capacity(game.solar_system.bodies.len());

    for i in 0..game.solar_system.bodies.len() {
        let pos = game.solar_system.body_position(i);
        let body = &game.solar_system.bodies[i];

        let scaled_pos = if let Some(parent_idx) = body.parent {
            let parent_scaled = scaled_positions[parent_idx];
            let parent_unscaled = game.solar_system.body_position(parent_idx);
            let rel_x = pos[0] - parent_unscaled[0];
            let rel_y = pos[1] - parent_unscaled[1];
            [
                parent_scaled[0] + rel_x * BODY_SCALE,
                parent_scaled[1] + rel_y * BODY_SCALE,
            ]
        } else {
            pos
        };
        scaled_positions.push(scaled_pos);
    }

    let in_galaxy_view = is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center);

    // Update camera tracking
    if game.flight.tracking_ship {
        // Decompose camera into body_center + ship_offset for precision at galaxy-scale distances.
        // body_center is subtracted from body positions in f64 (precise galaxy-scale subtraction).
        // ship_offset is subtracted in the GPU shader in f32 (precise because it's a small value).
        // camera.position is the full position (quantized) for backward compat with UI/hit-testing.
        let soi_pos = scaled_positions[game.flight.ship.soi_body];
        let rel = game.flight.ship.rel_position;
        let body_center = [soi_pos[0] * SCALE, soi_pos[1] * SCALE];
        let ship_offset = [rel[0] * SCALE * BODY_SCALE, rel[1] * SCALE * BODY_SCALE];
        render_state.camera.body_center = body_center;
        render_state.camera.ship_offset = ship_offset;
        render_state.camera.position[0] = body_center[0] + ship_offset[0];
        render_state.camera.position[1] = body_center[1] + ship_offset[1];
    } else {
        render_state.update_tracking(&scaled_positions, SCALE);
    }

    // Camera rotation: surface-down when below landing altitude and suborbital
    {
        let below_landing = game.flight.ship.below_landing_altitude(&game.solar_system);
        let is_suborbital = game.flight.ship.is_suborbital(&game.solar_system);

        let target_rotation = if below_landing && is_suborbital {
            // Rotate so surface is "down": ship's radial-in direction points screen-down
            let rx = game.flight.ship.rel_position[0];
            let ry = game.flight.ship.rel_position[1];
            // atan2(ry, rx) = angle of ship from body center
            // We want "up" on screen (positive Y) to align with radial-out
            // So rotation = PI/2 - angle_from_body
            (std::f64::consts::FRAC_PI_2 - ry.atan2(rx)) as f32
        } else {
            0.0
        };

        // Smooth interpolation with angle wrapping
        let mut diff = target_rotation - render_state.camera.rotation;
        if !diff.is_finite() { diff = 0.0; }
        while diff > std::f32::consts::PI {
            diff -= std::f32::consts::TAU;
        }
        while diff < -std::f32::consts::PI {
            diff += std::f32::consts::TAU;
        }

        let rate = 5.0 * dt as f32; // ~5 rad/s
        if diff.abs() < rate {
            render_state.camera.rotation = target_rotation;
        } else {
            render_state.camera.rotation += diff.signum() * rate;
        }

        // Normalize to [-PI, PI]
        if !render_state.camera.rotation.is_finite() { render_state.camera.rotation = 0.0; }
        while render_state.camera.rotation > std::f32::consts::PI {
            render_state.camera.rotation -= std::f32::consts::TAU;
        }
        while render_state.camera.rotation < -std::f32::consts::PI {
            render_state.camera.rotation += std::f32::consts::TAU;
        }
    }

    let mut bodies: Vec<_> = (0..game.solar_system.bodies.len())
        .map(|i| {
            let body = &game.solar_system.bodies[i];
            let pos = scaled_positions[i];
            // In galaxy view, only show root + stars (direct children of root)
            let visible = if in_galaxy_view {
                body.parent.is_none() || body.parent.map_or(false, |p|
                    game.solar_system.bodies[p].parent.is_none())
            } else {
                true
            };
            let radius = if visible { body.radius * BODY_SCALE } else { 0.0 };
            let atmo_height = if visible { body.atmosphere.map(|a| a.visible_height()).unwrap_or(0.0) } else { 0.0 };
            let atmo_color = body.atmosphere.map(|a| a.color).unwrap_or([0.0; 3]);
            (pos[0], pos[1], radius, body.color, atmo_height, atmo_color, i)
        })
        .collect();

    let pixels_per_world_unit = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    let view_soi_body = compute_view_soi_body(game, render_state);

    let mut orbits: Vec<Option<OrbitRenderData>> = (0..game.solar_system.bodies.len())
        .map(|i| {
            let body = &game.solar_system.bodies[i];
            match (body.parent, &body.orbit) {
                (Some(parent_idx), Some(orbit)) => {
                    let parent_body = &game.solar_system.bodies[parent_idx];
                    if parent_body.parent.is_none() {
                        // Star orbit: only show in galaxy view when this star is tracked
                        if in_galaxy_view && render_state.tracked_body == Some(i) {
                            let parent_pos = scaled_positions[parent_idx];
                            let orbit_color = [
                                body.color[0] * 0.4,
                                body.color[1] * 0.4,
                                body.color[2] * 0.4,
                                0.5,
                            ];
                            return Some(OrbitRenderData {
                                parent_x: parent_pos[0] * SCALE,
                                parent_y: parent_pos[1] * SCALE,
                                semi_major_axis: orbit.semi_major_axis * SCALE * BODY_SCALE,
                                eccentricity: orbit.eccentricity,
                                argument_of_periapsis: orbit.argument_of_periapsis,
                                color: orbit_color,
                            });
                        }
                        return None;
                    }

                    // In galaxy view, skip all non-star orbits
                    if in_galaxy_view {
                        return None;
                    }

                    // SOI filter: only show orbits within the focused body's SOI
                    if let Some(soi) = view_soi_body {
                        if !is_in_soi_of(parent_idx, soi, &game.solar_system.bodies) {
                            return None;
                        }
                    }

                    let body_world_radius = (body.radius * BODY_SCALE * SCALE) as f32;
                    let body_pixels = body_world_radius * pixels_per_world_unit * 2.0;
                    let is_moon = parent_body.parent
                        .map_or(false, |gp| game.solar_system.bodies[gp].parent.is_some());
                    let pixel_threshold = if is_moon { 100.0 } else { 5.0 };

                    if body_pixels >= pixel_threshold {
                        return None;
                    }

                    let parent_pos = scaled_positions[parent_idx];
                    let orbit_color = [
                        body.color[0] * 0.4,
                        body.color[1] * 0.4,
                        body.color[2] * 0.4,
                        0.5,
                    ];
                    Some(OrbitRenderData {
                        parent_x: parent_pos[0] * SCALE,
                        parent_y: parent_pos[1] * SCALE,
                        semi_major_axis: orbit.semi_major_axis * SCALE * BODY_SCALE,
                        eccentricity: orbit.eccentricity,
                        argument_of_periapsis: orbit.argument_of_periapsis,
                        color: orbit_color,
                    })
                }
                _ => None,
            }
        })
        .collect();

    // Prepare ship render data
    let _ = game.flight.ship.calculate_orbit(&game.solar_system);
    let ship_orbit = game.flight.ship.get_orbital_info(&game.solar_system).map(|info| {
        let parent_pos = scaled_positions[info.parent_idx];
        let parent_body = &game.solar_system.bodies[info.parent_idx];
        ShipOrbitData {
            parent_x: parent_pos[0] * SCALE,
            parent_y: parent_pos[1] * SCALE,
            semi_major_axis: info.orbit.semi_major_axis * SCALE * BODY_SCALE,
            eccentricity: info.orbit.eccentricity,
            argument_of_periapsis: info.orbit.argument_of_periapsis,
            apoapsis: info.apoapsis,
            periapsis: info.periapsis,
            orbital_period: info.orbital_period,
            time_to_apoapsis: info.time_to_apoapsis,
            time_to_periapsis: info.time_to_periapsis,
            parent_body_radius: parent_body.radius,
            parent_name: parent_body.name.clone(),
            retrograde: info.retrograde,
        }
    });

    let velocity = (game.flight.ship.rel_velocity[0].powi(2) + game.flight.ship.rel_velocity[1].powi(2)).sqrt();
    let distance_from_soi = (game.flight.ship.rel_position[0].powi(2) + game.flight.ship.rel_position[1].powi(2)).sqrt();
    let soi_body = &game.solar_system.bodies[game.flight.ship.soi_body];
    let altitude = distance_from_soi - soi_body.radius;

    let patched_traj_raw = game.flight.ship.get_patched_trajectory(&game.solar_system);
    let time_to_intercept = patched_traj_raw.as_ref()
        .and_then(|traj| traj.segments.first())
        .and_then(|seg| seg.end_time);

    let patched_trajectory = patched_traj_raw.as_ref()
        .map(|traj| {
            traj.segments.iter().enumerate()
                .filter(|(_, seg)| {
                    // SOI-based filter
                    let soi_ok = view_soi_body.map_or(true, |soi|
                        is_in_soi_of(seg.parent_idx, soi, &game.solar_system.bodies));
                    // Pixel-threshold filter: hide higher-level segments when the
                    // body in the ship's SOI chain that orbits the segment's parent
                    // is big enough on screen. Skip for the ship's own SOI body —
                    // that orbit hides via ship_pixels < 5.0 in render.
                    let pixel_ok = seg.parent_idx == game.flight.ship.soi_body
                        || !segment_hidden_by_ancestor_threshold(
                            game.flight.ship.soi_body, seg.parent_idx,
                            &game.solar_system.bodies, pixels_per_world_unit);
                    soi_ok && pixel_ok
                })
                .enumerate()
                .map(|(filtered_i, (orig_i, seg))| {
                let parent_pos = scaled_positions[seg.parent_idx];
                let parent_soi = game.solar_system.bodies[seg.parent_idx].soi_radius;
                let parent_mass = game.solar_system.bodies[seg.parent_idx].effective_mass_at(seg.orbit.semi_major_axis);
                let is_first = filtered_i == 0;
                let alpha = if is_first { 0.7 } else { 0.4 };
                // For the first segment, recompute true anomaly from the ship's
                // current position each frame so the orbit line trims smoothly
                // instead of jumping when the cached trajectory refreshes.
                let start_true_anomaly = if orig_i == 0 {
                    let rx = game.flight.ship.rel_position[0];
                    let ry = game.flight.ship.rel_position[1];
                    let pos_angle = ry.atan2(rx);
                    let mut ta = pos_angle - seg.orbit.argument_of_periapsis;
                    if seg.orbit.eccentricity < 1.0 {
                        ta = ta.rem_euclid(std::f64::consts::TAU);
                    } else {
                        while ta > std::f64::consts::PI { ta -= std::f64::consts::TAU; }
                        while ta < -std::f64::consts::PI { ta += std::f64::consts::TAU; }
                    }
                    ta
                } else {
                    seg.start_true_anomaly
                };
                OrbitSegmentData {
                    parent_x: parent_pos[0] * SCALE,
                    parent_y: parent_pos[1] * SCALE,
                    semi_major_axis: seg.orbit.semi_major_axis * SCALE * BODY_SCALE,
                    eccentricity: seg.orbit.eccentricity,
                    argument_of_periapsis: seg.orbit.argument_of_periapsis,
                    start_true_anomaly,
                    end_true_anomaly: seg.end_true_anomaly,
                    color: [0.9, 0.2, 0.2, alpha],
                    is_first_segment: is_first,
                    retrograde: seg.retrograde,
                    soi_radius: parent_soi * SCALE * BODY_SCALE,
                    parent_body_radius: game.solar_system.bodies[seg.parent_idx].radius,
                    parent_mass,
                    parent_idx: seg.parent_idx,
                    render_scale: SCALE * BODY_SCALE,
                    start_time: seg.start_time,
                    base_epoch: game.time(),
                }
            }).collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let current_true_anomaly = patched_trajectory.first()
        .map(|seg| seg.start_true_anomaly)
        .unwrap_or(0.0);

    // Build part render data and vessel stats from flight vessel

    // Sun distance for solar panel output display
    let sun_distance_m = {
        let ship_abs = game.flight.ship.absolute_position(&game.solar_system);
        let sun_pos = game.solar_system.body_position(game.solar_system.sun_index);
        let dx = ship_abs[0] - sun_pos[0];
        let dy = ship_abs[1] - sun_pos[1];
        (dx * dx + dy * dy).sqrt()
    };

    let (part_render_data, vessel_mass, vessel_fuel_frac, vessel_thrust, vessel_delta_v, vessel_stage_delta_vs, vessel_stage_burn_times, vessel_size) =
        if let Some(ref vessel) = game.flight.vessel {
            let parts: Vec<ShipPartRenderData> = vessel.parts.iter()
                .enumerate()
                .filter(|(_, p)| !p.destroyed && !p.decoupled)
                .map(|(i, p)| {
                    let def = game.part_definitions.get(&p.definition_id);
                    let name = def.map(|d| d.name.clone()).unwrap_or_else(|| p.definition_id.clone());
                    let dry_mass = def.map(|d| d.mass).unwrap_or(0.0);

                    // Engine info
                    let is_engine = p.propellant_type.is_some();
                    let engine_thrust_vac = if is_engine { Some(p.engine_thrust_vac) } else { None };
                    let engine_thrust_asl = if is_engine { Some(p.engine_thrust_asl) } else { None };
                    let engine_isp_vac = if is_engine { Some(p.engine_isp_vac) } else { None };
                    let engine_isp_asl = if is_engine { Some(p.engine_isp_asl) } else { None };
                    let propellant_name = p.propellant_type.map(|pt| pt.display_name().to_string());

                    // Tank info: separate oxidizer and fuel
                    let has_tank = def.map(|d| d.tank.is_some()).unwrap_or(false);
                    let (fuel_type_name, fuel_current, fuel_max, ox_current, ox_max) = if has_tank {
                        let fuel_names = ["rp1", "methane", "hydrogen", "monopropellant", "xenon"];
                        let f_current: f64 = fuel_names.iter()
                            .filter_map(|n| p.resources.get(*n))
                            .sum();
                        let f_max: f64 = fuel_names.iter()
                            .filter_map(|n| p.max_resources.get(*n))
                            .sum();
                        let o_current = p.resources.get("oxygen").copied().unwrap_or(0.0);
                        let o_max = p.max_resources.get("oxygen").copied().unwrap_or(0.0);
                        // Determine fuel type name from what's loaded
                        let ft_name = if p.max_resources.contains_key("rp1") {
                            Some("LOX/RP-1".to_string())
                        } else if p.max_resources.contains_key("methane") {
                            Some("LOX/CH4".to_string())
                        } else if p.max_resources.contains_key("hydrogen") {
                            if o_max > 0.0 { Some("LOX/LH2".to_string()) } else { Some("LH2".to_string()) }
                        } else if p.max_resources.contains_key("monopropellant") {
                            Some("Monopropellant".to_string())
                        } else if p.max_resources.contains_key("xenon") {
                            Some("Xenon".to_string())
                        } else if o_max > 0.0 {
                            Some("LOX".to_string())
                        } else {
                            Some("Empty".to_string())
                        };
                        (ft_name, Some(f_current), Some(f_max), Some(o_current), Some(o_max))
                    } else {
                        (None, None, None, None, None)
                    };

                    // Pod info
                    let crew_capacity = def.and_then(|d| d.pod.as_ref().map(|pod| pod.crew_capacity));
                    let (monoprop_current, monoprop_max) = if crew_capacity.is_some() {
                        let cur = p.resources.get("monopropellant").copied();
                        let max = p.max_resources.get("monopropellant").copied();
                        (cur, max)
                    } else {
                        (None, None)
                    };

                    // Compute RCS nozzle activation state (combined rotation + translation)
                    let has_rcs_input = rcs_direction_for_render.abs() > 0.001
                        || rcs_translate_for_render[0].abs() > 0.001
                        || rcs_translate_for_render[1].abs() > 0.001;
                    let rcs_nozzle_state = if has_rcs_input && p.rcs_thrust > 0.0 {
                        if let Some(rcs_def) = def.and_then(|d| d.rcs.as_ref()) {
                            let dir = rcs_direction_for_render;
                            let rx = p.local_position[0]; // part x relative to COM
                            let ry = p.local_position[1]; // part y relative to COM
                            let is_mirrored = rcs_def.is_mirrored;
                            let trans_fwd = rcs_translate_for_render[0];   // +forward (vessel +Y)
                            let trans_right = rcs_translate_for_render[1]; // +right (vessel +X for non-mirrored)

                            // --- Rotation-driven nozzle activation ---
                            // Lateral nozzle exhausts away from mount side: for non-mirrored,
                            // it exhausts left (-X), producing force to the right at this part.
                            // Torque = r × F: with part above COM (ry > 0), rightward force
                            // gives CW (negative) torque, so torque_sign = -ry.
                            let lateral_torque_sign = if is_mirrored { ry } else { -ry };
                            let rot_lateral = dir.abs() > 0.001 && ((dir > 0.0 && lateral_torque_sign > 0.0) || (dir < 0.0 && lateral_torque_sign < 0.0));
                            // Mirrored lateral: opposite side nozzle (for bilateral pod RCS)
                            let lateral_torque_sign_m = if is_mirrored { -ry } else { ry };
                            let rot_lateral_m = dir.abs() > 0.001 && ((dir > 0.0 && lateral_torque_sign_m > 0.0) || (dir < 0.0 && lateral_torque_sign_m < 0.0));
                            let up_torque_sign = -rx;
                            let rot_up = dir.abs() > 0.001 && ((dir > 0.0 && up_torque_sign > 0.0) || (dir < 0.0 && up_torque_sign < 0.0));
                            let down_torque_sign = rx;
                            let rot_down = dir.abs() > 0.001 && ((dir > 0.0 && down_torque_sign > 0.0) || (dir < 0.0 && down_torque_sign < 0.0));

                            // --- Translation-driven nozzle activation ---
                            // Forward (vessel +Y): down nozzles fire (push ship forward=up)
                            let trans_down = trans_fwd > 0.001;
                            // Backward (vessel -Y): up nozzles fire (push ship backward=down)
                            let trans_up = trans_fwd < -0.001;
                            // Going right (+X): fire right-mount laterals (exhaust left, push right)
                            //   Non-mirrored = right-mount, mirrored = left-mount
                            // Going left (-X): fire left-mount laterals (exhaust right, push left)
                            //   Mirrored = left-mount (lateral nozzle exhausts right)
                            let trans_lateral = (trans_right > 0.001 && !is_mirrored) || (trans_right < -0.001 && is_mirrored);
                            let lateral = rot_lateral || trans_lateral;

                            // Pod bilateral nozzles (opposite side):
                            // Going left: fire right nozzle on non-mirrored pod (exhaust right, push left)
                            // Going right: fire left nozzle on mirrored pod (exhaust left, push right)
                            let trans_lateral_m = (trans_right < -0.001 && !is_mirrored) || (trans_right > 0.001 && is_mirrored);
                            let lateral_mirrored = rot_lateral_m || trans_lateral_m;
                            let up = rot_up || trans_up;
                            let down = rot_down || trans_down;
                            if lateral || lateral_mirrored || up || down {
                                Some(crate::render::RcsNozzleState { lateral, lateral_mirrored, up, down })
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    ShipPartRenderData {
                        definition_id: p.definition_id.clone(),
                        local_x: p.local_position[0],
                        local_y: p.local_position[1],
                        rotation: p.rotation,
                        engine_active: p.engine_active,
                        part_index: i,
                        name,
                        dry_mass,
                        hitbox_half_w: if is_part_rotation_swapped(p.rotation) { p.hitbox_half_extents[1] } else { p.hitbox_half_extents[0] },
                        hitbox_half_h: if is_part_rotation_swapped(p.rotation) { p.hitbox_half_extents[0] } else { p.hitbox_half_extents[1] },
                        click_local_y: {
                            let is_solar = def.map(|d| d.solar_panel.is_some()).unwrap_or(false);
                            if is_solar && p.deploy_fraction < 1.0 {
                                // Retracted hitbox: 1 square for small panels, 2 for wide (>=2 grid)
                                let base_squares = if def.map(|d| d.grid_width >= 2.0).unwrap_or(false) { 2.0 } else { 1.0 };
                                let base_half_h = base_squares * GRID_SQUARE_SIZE * 0.5;
                                let half_h = if is_part_rotation_swapped(p.rotation) { p.hitbox_half_extents[0] } else { p.hitbox_half_extents[1] };
                                p.local_position[1] - half_h + base_half_h
                            } else if def.map(|d| d.is_heat_shield).unwrap_or(false) {
                                // Heat shields are top-aligned: shift click center to visual center
                                let editor_half_h = def.map(|d| d.hitbox_height() / 2.0).unwrap_or(0.0);
                                let flight_half_h = if is_part_rotation_swapped(p.rotation) { p.hitbox_half_extents[0] } else { p.hitbox_half_extents[1] };
                                p.local_position[1] + (editor_half_h - flight_half_h)
                            } else {
                                p.local_position[1] + p.hitbox_y_offset
                            }
                        },
                        click_hitbox_half_h: {
                            let is_solar = def.map(|d| d.solar_panel.is_some()).unwrap_or(false);
                            if is_solar && p.deploy_fraction < 1.0 {
                                let base_squares = if def.map(|d| d.grid_width >= 2.0).unwrap_or(false) { 2.0 } else { 1.0 };
                                base_squares * GRID_SQUARE_SIZE * 0.5
                            } else if is_part_rotation_swapped(p.rotation) {
                                p.hitbox_half_extents[0]
                            } else {
                                p.hitbox_half_extents[1]
                            }
                        },
                        engine_thrust_vac,
                        engine_thrust_asl,
                        engine_isp_vac,
                        engine_isp_asl,
                        engine_enabled: p.engine_enabled,
                        engine_no_power: p.engine_no_power,
                        propellant_name,
                        fuel_type_name,
                        fuel_current,
                        fuel_max,
                        ox_current,
                        ox_max,
                        crew_capacity,
                        monoprop_current,
                        monoprop_max,
                        battery_current: if p.max_electricity > 0.0 { Some(p.electricity) } else { None },
                        battery_max: if p.max_electricity > 0.0 { Some(p.max_electricity) } else { None },
                        solar_output: def.and_then(|d| d.solar_panel.as_ref().map(|sp| {
                            let au_m = 1.496e11_f64;
                            let ratio = au_m / sun_distance_m.max(1.0);
                            sp.output_1au * ratio * ratio * p.deploy_fraction
                        })),
                        rtg_output: def.and_then(|d| d.rtg.as_ref().map(|r| r.output_watts)),
                        reactor_output: def.and_then(|d| d.reactor.as_ref().map(|r| r.output_watts)),
                        reactor_waste_heat: def.and_then(|d| d.reactor.as_ref().filter(|r| r.waste_heat_watts > 0.0).map(|r| r.waste_heat_watts)),
                        shield_type: def.and_then(|d| d.shield.as_ref().map(|s| format!("{:?}", s.shield_type))),
                        shield_max_c: def.and_then(|d| d.shield.as_ref().map(|s| s.max_velocity_c)),
                        shield_power: def.and_then(|d| d.shield.as_ref().map(|s| s.power_base_watts)),
                        shield_active: p.shield_active,
                        is_decoupler: def.map(|d| d.decoupler.is_some()).unwrap_or(false),
                        crossfeed_enabled: p.crossfeed_enabled,
                        gimbal_angle: p.gimbal_angle,
                        rcs_thrust: if p.rcs_thrust > 0.0 { Some(p.rcs_thrust) } else { None },
                        rcs_nozzle_state,
                        heat_fraction: ((p.temperature - 300.0) / (p.max_heat_tolerance - 300.0)).clamp(0.0, 1.0) as f32,
                        temperature: p.temperature,
                        is_fairing: def.map(|d| d.fairing.is_some()).unwrap_or(false),
                        fairing_shape: p.fairing_shape.clone(),
                        fairing_half: p.fairing_half,
                        deploy_fraction: p.deploy_fraction,
                        is_solar_panel: def.map(|d| d.solar_panel.is_some()).unwrap_or(false),
                        is_radiator: p.is_radiator,
                        radiator_tier: def.and_then(|d| d.radiator.as_ref().map(|r| r.tier)),
                        radiator_rejection_watts: def.and_then(|d| d.radiator.as_ref().map(|r| r.rejection_watts)),
                        engine_waste_heat: def.and_then(|d| d.engine.as_ref().filter(|e| e.waste_heat_watts > 0.0).map(|e| e.waste_heat_watts)),
                        is_parachute: p.is_parachute,
                        parachute_deployed: p.parachute_deployed,
                        parachute_spent: p.parachute_spent,
                        parachute_deploy_fraction: p.parachute_deploy_fraction,
                        parachute_deployed_width_m: p.parachute_deployed_width_m,
                        parachute_fully_deployed: p.parachute_fully_deployed,
                        sprite_half_h: def.map(|d| d.height() / 2.0).unwrap_or(0.0),
                    }
                })
                .collect();

            let (stage_fuel, stage_fuel_max) = vessel.get_stage_fuel(&game.part_definitions);
            let fuel_frac = if stage_fuel_max > 0.0 {
                stage_fuel / stage_fuel_max
            } else {
                0.0
            };

            let size = vessel.bounding_half_height() * 2.0;
            let stage_dv_burns = vessel.calculate_stage_delta_v(&game.part_definitions);
            let stage_dvs: Vec<f64> = stage_dv_burns.iter().map(|(dv, _)| *dv).collect();
            let stage_burn_times: Vec<f64> = stage_dv_burns.iter().map(|(_, bt)| *bt).collect();
            let dv: f64 = stage_dvs.iter().sum();

            (Some(parts), Some(vessel.total_mass), Some(fuel_frac), Some(vessel.active_thrust_at_pressure(hud_atmo_pressure)), Some(dv), stage_dvs, stage_burn_times, size)
        } else {
            (None, None, None, None, None, Vec::new(), Vec::new(), SHIP_SIZE)
        };

    // Use hottest part temperature when vessel exists, otherwise ship temperature
    let (effective_temp, effective_heat_fraction) = if let Some(ref vessel) = game.flight.vessel {
        let hottest = vessel.parts.iter()
            .filter(|p| !p.destroyed && !p.decoupled)
            .map(|p| p.temperature)
            .fold(300.0_f64, f64::max);
        let max_tol = vessel.parts.iter()
            .filter(|p| !p.destroyed && !p.decoupled)
            .filter(|p| p.temperature == hottest)
            .map(|p| p.max_heat_tolerance)
            .next()
            .unwrap_or(default_heat_tolerance());
        let frac = ((hottest - 300.0) / (max_tol - 300.0)).clamp(0.0, 1.0) as f32;
        (hottest, frac)
    } else {
        let frac = ((game.flight.ship.temperature - AMBIENT_TEMPERATURE)
            / (default_heat_tolerance() - AMBIENT_TEMPERATURE))
            .clamp(0.0, 1.0) as f32;
        (game.flight.ship.temperature, frac)
    };
    let heat_fraction = effective_heat_fraction;

    // Compute felt g-force (what crew experiences)
    const G0: f64 = 9.80665;
    let g_force = match &game.flight.ship.state {
        ShipState::Landed { .. } => {
            // On the ground, crew feels the normal force = surface gravity
            soi_body.surface_gravity() / G0
        }
        ShipState::Flying => {
            // In flight, crew feels thrust + drag as a vector sum (gravity is freefall, not felt)
            let rot = game.flight.ship.rotation;
            let thrust_mag = vessel_physics.as_ref()
                .map(|v| if v.total_mass > 0.0 {
                    let thrust = v.max_thrust_vac * (1.0 - hud_atmo_pressure) + v.max_thrust_asl * hud_atmo_pressure;
                    game.flight.ship.throttle * thrust / v.total_mass
                } else { 0.0 })
                .unwrap_or(0.0);
            let thrust = [rot.cos() * thrust_mag, rot.sin() * thrust_mag];

            let drag = {
                let soi = &game.solar_system.bodies[game.flight.ship.soi_body];
                game.flight.ship.compute_drag_accel(soi, vessel_physics.as_ref())
            };

            let net_x = thrust[0] + drag[0];
            let net_y = thrust[1] + drag[1];
            (net_x.powi(2) + net_y.powi(2)).sqrt() / G0
        }
    };

    // Compute drag force in kN for HUD display
    let drag_kn = {
        let soi = &game.solar_system.bodies[game.flight.ship.soi_body];
        let drag_accel = game.flight.ship.compute_drag_accel(soi, vessel_physics.as_ref());
        let drag_mag = (drag_accel[0].powi(2) + drag_accel[1].powi(2)).sqrt();
        let mass_kg = vessel_physics.as_ref().map(|v| v.total_mass * 1000.0).unwrap_or(1000.0);
        drag_mag * mass_kg / 1000.0 // N -> kN
    };

    // Set atmosphere/landed state for parachute UI
    render_state.ship_in_atmosphere = game.flight.ship.in_atmosphere(&game.solar_system);
    render_state.ship_is_landed = matches!(game.flight.ship.state, ShipState::Landed { .. });

    // Use scaled_positions + rel_position to match body rendering precision
    let soi_pos_render = scaled_positions[game.flight.ship.soi_body];
    let rel_render = game.flight.ship.rel_position;
    let ship_render = ShipRenderData {
        x: soi_pos_render[0] * SCALE + rel_render[0] * SCALE * BODY_SCALE,
        y: soi_pos_render[1] * SCALE + rel_render[1] * SCALE * BODY_SCALE,
        rotation: game.flight.ship.rotation,
        size: vessel_size * SCALE * BODY_SCALE,
        color: game.flight.ship.color,
        orbit: ship_orbit,
        patched_trajectory,
        velocity,
        altitude,
        soi_body_name: soi_body.name.clone(),
        throttle: game.flight.ship.throttle,
        time_to_intercept,
        acceleration: current_accel,
        current_true_anomaly,
        parts: part_render_data,
        total_mass: vessel_mass,
        fuel_fraction: vessel_fuel_frac,
        monoprop_fraction: game.flight.vessel.as_ref().and_then(|v| {
            let (current, max) = v.total_monopropellant();
            if max > 0.0 { Some(current / max) } else { None }
        }),
        power_generation: if game.flight.vessel.is_some() { Some(power_generation) } else { None },
        power_consumption: if game.flight.vessel.is_some() { Some(power_consumption) } else { None },
        electricity_fraction: game.flight.vessel.as_ref().and_then(|v| v.electricity_fraction()),
        electricity_stored: game.flight.vessel.as_ref().map(|v| v.total_electricity()),
        electricity_max: game.flight.vessel.as_ref().map(|v| v.max_electricity()),
        thrust_kn: vessel_thrust,
        drag_kn,
        delta_v: vessel_delta_v,
        soi_surface_gravity: game.solar_system.bodies[game.flight.ship.soi_body].surface_gravity(),
        g_force,
        current_stage: game.flight.vessel.as_ref().map(|v| v.current_stage),
        total_stages: game.flight.vessel.as_ref().map(|v| v.stages.len()),
        temperature: effective_temp,
        heat_fraction,
        heat_flux: game.flight.ship.heat_flux,
        thermal_pool_temp: game.flight.vessel.as_ref().map(|v| v.thermal_pool_temp).unwrap_or(300.0),
        thermal_pool_gen_w: game.flight.vessel.as_ref().map(|v| {
            // Engines firing + reactors running (mirror update_thermal_pool's logic for display).
            let mut g = 0.0_f64;
            for p in &v.parts {
                if p.destroyed || p.decoupled { continue; }
                let Some(def) = game.part_definitions.get(&p.definition_id) else { continue; };
                if let Some(ref e) = def.engine {
                    if p.engine_active { g += e.waste_heat_watts * v.throttle.clamp(0.0, 1.0); }
                }
                if let Some(ref r) = def.reactor {
                    if !v.reactors_tripped { g += r.waste_heat_watts; }
                }
            }
            g
        }).unwrap_or(0.0),
        thermal_pool_reject_w: game.flight.vessel.as_ref().map(|v| {
            v.parts.iter().filter(|p| !p.destroyed && !p.decoupled && p.is_radiator)
                .map(|p| p.radiator_rejection_watts * p.deploy_fraction.clamp(0.0, 1.0))
                .sum()
        }).unwrap_or(0.0),
        reactors_tripped: game.flight.vessel.as_ref().map(|v| v.reactors_tripped).unwrap_or(false),
        food_days: game.flight.vessel.as_ref().and_then(|v| {
            if v.total_crew > 0 {
                let consumption_per_day = v.total_crew as f64 * 1.5;
                Some(v.food_stored / consumption_per_day)
            } else { None }
        }),
        total_crew: game.flight.vessel.as_ref().map(|v| v.total_crew),
        is_starving: game.flight.vessel.as_ref().map(|v| v.food_stored <= 0.0 && v.total_crew > 0).unwrap_or(false),
        rcs_direction: rcs_direction_for_render,
        rcs_translate: rcs_translate_for_render,
        below_landing_altitude: game.flight.ship.below_landing_altitude(&game.solar_system)
            && matches!(game.flight.ship.state, ShipState::Flying),
        velocity_direction: {
            let vx = game.flight.ship.rel_velocity[0];
            let vy = game.flight.ship.rel_velocity[1];
            let speed = (vx * vx + vy * vy).sqrt();
            if speed > 0.1 { [vx / speed, vy / speed] } else { [0.0, 0.0] }
        },
        speed_fraction_c: velocity / crate::ship::SPEED_OF_LIGHT,
        lorentz_gamma: crate::ship::lorentz_gamma(velocity),
        proper_time: game.flight.ship.proper_time,
        mission_time: game.flight.ship.mission_time,
        is_relativistic: velocity > crate::ship::RELATIVISTIC_SPEED_THRESHOLD,
        orbits_root: game.solar_system.bodies[game.flight.ship.soi_body].parent.is_none(),
        has_control: game.flight.vessel.as_ref()
            .map_or(true, |v| v.has_control(&game.part_definitions)),
        grav_time_factor: {
            let dist = distance_from_soi;
            let soi = &game.solar_system.bodies[game.flight.ship.soi_body];
            crate::ship::gravitational_time_factor(
                crate::bodies::G * soi.effective_mass_at(dist), dist, soi.is_compact(),
            )
        },
        stage_delta_vs: if vessel_stage_delta_vs.is_empty() { None } else { Some(vessel_stage_delta_vs) },
        stage_burn_times: if vessel_stage_burn_times.is_empty() { None } else { Some(vessel_stage_burn_times) },
        stages: game.flight.vessel.as_ref().map(|v| {
            v.stages.iter().map(|stage| {
                stage.iter().map(|&part_idx| {
                    let name = if part_idx < v.parts.len() {
                        game.part_definitions.get(&v.parts[part_idx].definition_id)
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| format!("Part {}", part_idx))
                    } else {
                        format!("Part {}", part_idx)
                    };
                    StagedPartInfo { part_index: part_idx, name }
                }).collect()
            }).collect()
        }),
    };

    // Build background vessel data for inactive vessels
    let background_vessels: Vec<crate::render::TrackingVesselData> = game.flight.inactive_vessels.iter()
        .map(|v| {
            // Use scaled_positions + rel_position for precision (same as active ship)
            let soi_pos = scaled_positions[v.ship.soi_body];
            let rel = v.ship.rel_position;
            let orbit_data = v.ship.get_render_orbit().and_then(|(orbit, parent_idx)| {
                // SOI filter: hide orbits outside the focused body's SOI
                if let Some(soi) = view_soi_body {
                    if !is_in_soi_of(parent_idx, soi, &game.solar_system.bodies) {
                        return None;
                    }
                }
                // Pixel-threshold filter: hide higher-level orbits when the
                // body in the vessel's SOI chain is big on screen.
                if parent_idx != v.ship.soi_body
                    && segment_hidden_by_ancestor_threshold(
                        v.ship.soi_body, parent_idx,
                        &game.solar_system.bodies, pixels_per_world_unit)
                {
                    return None;
                }
                let parent_pos = scaled_positions[parent_idx];
                Some(OrbitRenderData {
                    parent_x: parent_pos[0] * SCALE,
                    parent_y: parent_pos[1] * SCALE,
                    semi_major_axis: orbit.semi_major_axis * SCALE * BODY_SCALE,
                    eccentricity: orbit.eccentricity,
                    argument_of_periapsis: orbit.argument_of_periapsis,
                    color: [0.5, 0.5, 0.5, 0.3], // Dimmed grey
                })
            });
            let parts = v.vessel.as_ref().map(|fv| {
                build_vessel_part_render_data(fv, &game.part_definitions)
            });
            crate::render::TrackingVesselData {
                id: v.id,
                name: v.name.clone(),
                color: v.ship.color,
                x: soi_pos[0] * SCALE + rel[0] * SCALE * BODY_SCALE,
                y: soi_pos[1] * SCALE + rel[1] * SCALE * BODY_SCALE,
                body_center: [soi_pos[0] * SCALE, soi_pos[1] * SCALE],
                rel_offset: [rel[0] * SCALE * BODY_SCALE, rel[1] * SCALE * BODY_SCALE],
                soi_body: v.ship.soi_body,
                orbit: orbit_data,
                parts,
                rotation: v.ship.rotation,
                is_debris: v.is_debris,
            }
        })
        .collect();

    // Store decomposed ship position for precision-safe camera-relative rendering
    render_state.ship_body_center = [soi_pos_render[0] * SCALE, soi_pos_render[1] * SCALE];
    render_state.ship_rel_offset = [rel_render[0] * SCALE * BODY_SCALE, rel_render[1] * SCALE * BODY_SCALE];

    // Calculate predicted trajectories for maneuver nodes (before vertex generation so current-frame data is used)
    let mut predicted_trajectories: Vec<Vec<OrbitSegmentData>> = Vec::new();
    for node in render_state.get_maneuver_nodes() {
        if node.total_delta_v() < 0.001 {
            continue;
        }

        let scale = node.render_scale;
        let parent_idx = node.parent_idx;
        let current_parent_pos = scaled_positions[parent_idx];
        let current_parent_x = current_parent_pos[0] * SCALE;
        let current_parent_y = current_parent_pos[1] * SCALE;

        let world_pos = node.world_pos(current_parent_x, current_parent_y);
        let velocity = node.velocity();

        let rel_x = (world_pos[0] - current_parent_x) / scale;
        let rel_y = (world_pos[1] - current_parent_y) / scale;
        let pos = [rel_x, rel_y];

        let prograde = node.prograde_unit();
        let radial = node.radial_unit();

        let new_vel = [
            velocity[0] + node.delta_v.prograde * prograde[0] + node.delta_v.radial_out * radial[0],
            velocity[1] + node.delta_v.prograde * prograde[1] + node.delta_v.radial_out * radial[1],
        ];

        if let Some(pred_traj) = game.flight.ship.calculate_predicted_trajectory(
            pos, new_vel, parent_idx, &game.solar_system, node.epoch
        ) {
            let segments: Vec<OrbitSegmentData> = pred_traj.segments.iter()
                .filter(|seg| {
                    let soi_ok = view_soi_body.map_or(true, |soi|
                        is_in_soi_of(seg.parent_idx, soi, &game.solar_system.bodies));
                    let pixel_ok = seg.parent_idx == game.flight.ship.soi_body
                        || !segment_hidden_by_ancestor_threshold(
                            game.flight.ship.soi_body, seg.parent_idx,
                            &game.solar_system.bodies, pixels_per_world_unit);
                    soi_ok && pixel_ok
                })
                .enumerate()
                .map(|(filtered_i, seg)| {
                let parent_pos = scaled_positions[seg.parent_idx];
                let parent_soi = game.solar_system.bodies[seg.parent_idx].soi_radius;
                let parent_mass = game.solar_system.bodies[seg.parent_idx].effective_mass_at(seg.orbit.semi_major_axis);
                let alpha = if filtered_i == 0 { 0.7 } else { 0.5 };
                OrbitSegmentData {
                    parent_x: parent_pos[0] * SCALE,
                    parent_y: parent_pos[1] * SCALE,
                    semi_major_axis: seg.orbit.semi_major_axis * SCALE * BODY_SCALE,
                    eccentricity: seg.orbit.eccentricity,
                    argument_of_periapsis: seg.orbit.argument_of_periapsis,
                    start_true_anomaly: seg.start_true_anomaly,
                    end_true_anomaly: seg.end_true_anomaly,
                    color: [0.2, 0.8, 0.2, alpha],
                    is_first_segment: filtered_i == 0,
                    retrograde: seg.retrograde,
                    soi_radius: parent_soi * SCALE * BODY_SCALE,
                    parent_body_radius: game.solar_system.bodies[seg.parent_idx].radius,
                    parent_mass,
                    parent_idx: seg.parent_idx,
                    render_scale: SCALE * BODY_SCALE,
                    start_time: seg.start_time,
                    base_epoch: node.epoch,
                }
            }).collect();
            predicted_trajectories.push(segments);
        }
    }
    render_state.set_predicted_trajectories(predicted_trajectories);

    // Compute closest approach marker to navigation target BEFORE render call
    // so that CA world positions use the same frame's scaled_positions as the camera.
    render_state.closest_approach_world_pos = None;
    render_state.closest_approach_marker = None;
    render_state.target_closest_approach_world_pos = None;
    render_state.target_closest_approach_marker = None;
    if let (Some(target), Some(ref traj)) = (render_state.selected_target, &patched_traj_raw) {
        // Skip CA if ship is already orbiting the target body
        let skip_ca = match target {
            SelectedTarget::Body(idx) => {
                // Ship is in the target's SOI — CA is meaningless
                game.flight.ship.soi_body == idx
                // Or trajectory enters the target's SOI — proper encounter exists
                || traj.segments.iter().any(|s| s.parent_idx == idx)
            }
            SelectedTarget::Vessel(_) => false,
        };

        if skip_ca {
            // Leave CA markers as None (already cleared above)
        } else {

        // Determine which SOI the target is in
        let target_soi_parent = match target {
            SelectedTarget::Body(idx) => game.solar_system.bodies[idx].parent,
            SelectedTarget::Vessel(id) => {
                game.flight.inactive_vessels.iter()
                    .find(|v| v.id == id)
                    .map(|v| v.ship.soi_body)
            }
        };

        if let Some(target_parent) = target_soi_parent {
            // Find first trajectory segment in the same SOI as the target
            if let Some(seg) = traj.segments.iter()
                .find(|s| s.parent_idx == target_parent)
                .filter(|s| s.orbit.eccentricity < 1.0) // Skip hyperbolic — ship is escaping
            {
                let e = seg.orbit.eccentricity;
                let a = seg.orbit.semi_major_axis;
                let arg_peri = seg.orbit.argument_of_periapsis;
                let parent_mass = game.solar_system.bodies[seg.parent_idx].effective_mass_at(seg.orbit.semi_major_axis);
                let mu = G * parent_mass;

                // Semi-latus rectum
                let p = if e < 1.0 { a * (1.0 - e * e) } else { a.abs() * (e * e - 1.0) };

                // End true anomaly: use segment end or full orbit
                let end_ta = seg.end_true_anomaly.unwrap_or_else(|| {
                    if seg.retrograde {
                        seg.start_true_anomaly - std::f64::consts::TAU
                    } else {
                        seg.start_true_anomaly + std::f64::consts::TAU
                    }
                });

                let start_ma = game.flight.ship.true_to_mean_anomaly(&seg.orbit, seg.start_true_anomaly);
                let n = if e < 1.0 {
                    (mu / a.powi(3)).sqrt()
                } else {
                    (mu / a.abs().powi(3)).sqrt()
                };

                // Closure to compute ship pos and target pos at parameter t in [0,1]
                let compute_positions = |t: f64| -> Option<([f64; 2], [f64; 2])> {
                    let sample_ta = seg.start_true_anomaly + t * (end_ta - seg.start_true_anomaly);
                    let denom = 1.0 + e * sample_ta.cos();
                    if denom <= 0.001 { return None; }
                    let r = p / denom;
                    if r <= 0.0 || !r.is_finite() { return None; }

                    let angle = sample_ta + arg_peri;
                    let ship_pos = [r * angle.cos(), r * angle.sin()];

                    // Travel time from segment start
                    let sample_ma = game.flight.ship.true_to_mean_anomaly(&seg.orbit, sample_ta);
                    let delta_ma = if e < 1.0 {
                        if seg.retrograde {
                            let mut d = start_ma - sample_ma;
                            if d < 0.0 { d += std::f64::consts::TAU; }
                            d
                        } else {
                            let mut d = sample_ma - start_ma;
                            if d < 0.0 { d += std::f64::consts::TAU; }
                            d
                        }
                    } else {
                        (sample_ma - start_ma).abs()
                    };
                    let travel_time = if n > 0.0 { delta_ma / n } else { 0.0 };
                    let abs_time = game.time() + seg.start_time + travel_time;

                    // Target position relative to shared parent at abs_time
                    let target_pos = match target {
                        SelectedTarget::Body(idx) => {
                            game.solar_system.bodies[idx].orbit.as_ref()
                                .map(|orb| orb.position_at(abs_time, parent_mass))
                                .unwrap_or([0.0, 0.0])
                        }
                        SelectedTarget::Vessel(id) => {
                            // Use vessel's current position (no orbit propagation)
                            game.flight.inactive_vessels.iter()
                                .find(|v| v.id == id)
                                .map(|v| v.ship.rel_position)
                                .unwrap_or([0.0, 0.0])
                        }
                    };

                    Some((ship_pos, target_pos))
                };

                let compute_dist = |t: f64| -> f64 {
                    compute_positions(t).map_or(f64::MAX, |(sp, tp)| {
                        let dx = sp[0] - tp[0];
                        let dy = sp[1] - tp[1];
                        (dx * dx + dy * dy).sqrt()
                    })
                };

                // Coarse sampling (64 points)
                let num_samples = 64usize;
                let mut best_t = 0.0f64;
                let mut best_dist = f64::MAX;
                for i in 0..num_samples {
                    let t = i as f64 / (num_samples - 1) as f64;
                    let dist = compute_dist(t);
                    if dist < best_dist {
                        best_dist = dist;
                        best_t = t;
                    }
                }

                // Golden-section refinement (12 iterations)
                if best_dist < f64::MAX {
                    let phi = (5.0_f64.sqrt() + 1.0) / 2.0;
                    let step = 1.0 / (num_samples - 1) as f64;
                    let mut lo = (best_t - step).max(0.0);
                    let mut hi = (best_t + step).min(1.0);
                    for _ in 0..12 {
                        let c = hi - (hi - lo) / phi;
                        let d = lo + (hi - lo) / phi;
                        if compute_dist(c) < compute_dist(d) {
                            hi = d;
                        } else {
                            lo = c;
                        }
                    }
                    best_t = (lo + hi) / 2.0;
                    best_dist = compute_dist(best_t);
                }

                // Convert best point to render coordinates (split parent + offset for two-step precision)
                if best_dist < f64::MAX {
                    if let Some((ship_pos, target_pos)) = compute_positions(best_t) {
                        let parent_scaled = scaled_positions[seg.parent_idx];
                        let parent_render = [parent_scaled[0] * SCALE, parent_scaled[1] * SCALE];
                        // Ship marker: parent (galaxy-scale) + orbit offset (solar-system-scale)
                        let ship_offset = [ship_pos[0] * SCALE * BODY_SCALE, ship_pos[1] * SCALE * BODY_SCALE];
                        render_state.closest_approach_world_pos = Some((parent_render, ship_offset, best_dist));
                        // Target marker
                        let tgt_offset = [target_pos[0] * SCALE * BODY_SCALE, target_pos[1] * SCALE * BODY_SCALE];
                        render_state.target_closest_approach_world_pos = Some((parent_render, tgt_offset, best_dist));
                    }
                }
            }
        }

        } // else !skip_ca
    }

    let accretion_discs = build_accretion_disc_data(game);
    let procedural_stars = build_procedural_star_data(game, render_state);

    // Inject catalog planets as synthetic bodies when a catalog star is focused
    let num_real_bodies = game.solar_system.bodies.len();
    let mut body_names: Vec<String> = game.solar_system.bodies.iter().map(|b| b.name.clone()).collect();
    let focused_star = render_state.focused_star_id.and_then(|(sx, sy, si)| {
        procedural_stars.iter().find(|s| s.sector_x == sx && s.sector_y == sy && s.sector_index == si)
    });
    let ppwu = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    inject_catalog_planets(focused_star, &mut bodies, &mut orbits, &mut body_names, game.time(), num_real_bodies, ppwu, &mut render_state.body_texture_map, &mut render_state.catalog_body_info);
    render_state.body_names = body_names.clone();
    render_state.num_real_bodies = num_real_bodies;
    render_state.track_catalog_body(&bodies, SCALE);

    // Phase 6: on wasm, lazy-fetch body PNGs as the user zooms in. Fetches
    // are deduped inside the fetcher; uploads land in free layers of the
    // pre-allocated array and immediately update `body_texture_map`.
    #[cfg(target_arch = "wasm32")]
    {
        const FETCH_THRESHOLD_PX: f32 = 16.0;
        let pixels_per_world_unit = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
        for (_array_idx, &(_, _, radius_m, _, _, _, body_idx)) in bodies.iter().enumerate() {
            if radius_m <= 0.0 { continue; }
            let pixel_radius = (radius_m * SCALE * BODY_SCALE) as f32 * pixels_per_world_unit;
            if pixel_radius < FETCH_THRESHOLD_PX { continue; }
            let Some(name) = body_names.get(body_idx) else { continue };
            if name.is_empty() { continue; }
            let tex_name = name.to_lowercase().replace(' ', "_");
            render_state.body_texture_fetcher.request_fetch(&tex_name);
        }

        // Apply any decoded textures to GPU layers, populating name_layers.
        // Borrow disjoint fields directly so the borrow checker can split them.
        let RenderState {
            body_texture_fetcher,
            device,
            queue,
            body_texture,
            body_texture_map,
            ..
        } = &mut *render_state;
        body_texture_fetcher.drain_uploads(device, queue, body_texture, body_texture_map);

        // Backfill body_idx → layer mappings for any bodies whose texture has
        // arrived since last frame. Idempotent; cheap (few hundred map ops).
        for (body_idx, name) in body_names.iter().enumerate() {
            if !name.is_empty() {
                render_state.body_texture_map.register_body_index(body_idx, name);
            }
        }
    }

    render_state.update_bodies_orbits_ship_and_vessels(&bodies, &orbits, Some(&ship_render), SCALE, Some(&game.part_definitions), &background_vessels, &accretion_discs, in_galaxy_view, &procedural_stars);

    // Compute target angle for navigation target
    if let Some(target) = render_state.selected_target {
        let ship_abs = game.flight.ship.absolute_position(&game.solar_system);
        let target_abs = match target {
            SelectedTarget::Body(idx) => {
                game.solar_system.body_position(idx)
            }
            SelectedTarget::Vessel(id) => {
                if let Some(v) = game.flight.inactive_vessels.iter().find(|v| v.id == id) {
                    v.ship.absolute_position(&game.solar_system)
                } else {
                    // Target vessel no longer exists; clear target
                    render_state.selected_target = None;
                    render_state.selected_target_name.clear();
                    render_state.selected_target_angle = None;
                    [0.0, 0.0]
                }
            }
        };
        if render_state.selected_target.is_some() {
            let dx = target_abs[0] - ship_abs[0];
            let dy = target_abs[1] - ship_abs[1];
            render_state.selected_target_angle = Some(dy.atan2(dx));
        }
    } else {
        render_state.selected_target_angle = None;
    }

    // --- Transfer planner computation ---
    if render_state.transfer_planner_open {
        use crate::ship::transfer;

        // Update target lists
        let soi = game.flight.ship.soi_body;
        render_state.transfer_hohmann_targets = transfer::hohmann_targets(soi, &game.solar_system.bodies);
        render_state.transfer_interplanetary_targets = transfer::lambert_targets(soi, &game.solar_system.bodies);

        // Auto-select navigation target in planner if it's valid for the current mode.
        // Don't force mode changes — respect the user's mode choice.
        if let Some(SelectedTarget::Body(idx)) = render_state.selected_target {
            if render_state.transfer_selected_target != Some(idx) {
                let current_targets = if render_state.transfer_planner_mode == 0 {
                    &render_state.transfer_hohmann_targets
                } else {
                    &render_state.transfer_interplanetary_targets
                };
                if current_targets.iter().any(|(i, _)| *i == idx) {
                    render_state.transfer_selected_target = Some(idx);
                }
            }
        }

        // Validate selected target
        let targets = if render_state.transfer_planner_mode == 0 {
            &render_state.transfer_hohmann_targets
        } else {
            &render_state.transfer_interplanetary_targets
        };
        if let Some(sel) = render_state.transfer_selected_target {
            if !targets.iter().any(|(i, _)| *i == sel) {
                render_state.transfer_selected_target = None;
            }
        }

        // Advance the in-progress porkchop computation by a few rows per
        // frame. ~5 rows × 60 cols ≈ 300 Lambert solves per frame keeps the
        // total grid (50 rows) under 10 frames at the cost of a few ms of
        // extra work each frame while a transfer is being planned. Single
        // code path for desktop + wasm — no threads.
        if let Some(job) = render_state.porkchop_job.as_mut() {
            const ROWS_PER_FRAME: usize = 5;
            job.run_chunk(ROWS_PER_FRAME);
            if job.done() {
                let job = render_state.porkchop_job.take().expect("just checked");
                if render_state.porkchop_last_target == Some(job.target_idx()) {
                    let grid = job.take_grid();
                    render_state.porkchop_selected = grid.best_idx;
                    render_state.porkchop_hovered = None;
                    render_state.porkchop_grid = Some(grid);
                }
                render_state.porkchop_computing = false;
            }
        }

        // Compute transfer if target selected and ship has an orbit
        render_state.transfer_display = if let (Some(target_idx), Some(ship_orbit)) =
            (render_state.transfer_selected_target, game.flight.ship.get_cached_orbit())
        {
            let parent_mass = game.solar_system.bodies[ship_orbit.parent_idx].effective_mass_at(ship_orbit.orbit.semi_major_axis);
            if render_state.transfer_planner_mode == 0 {
                // Hohmann mode
                if let Some(target_orbit) = game.solar_system.bodies[target_idx].orbit.as_ref() {
                    transfer::compute_hohmann(
                        &ship_orbit.orbit,
                        ship_orbit.retrograde,
                        ship_orbit.mean_anomaly,
                        target_orbit,
                        parent_mass,
                        game.solar_system.time,
                    ).map(|h| {
                        let target_name = game.solar_system.bodies[target_idx].name.clone();
                        transfer::TransferDisplay {
                            mode: 0,
                            target_name,
                            departure_dv: h.departure_delta_v.abs(),
                            arrival_dv: h.arrival_delta_v,
                            transfer_time: h.transfer_time,
                            current_phase_angle: h.current_phase_angle.to_degrees(),
                            required_phase_angle: h.required_phase_angle.to_degrees(),
                            time_to_window: h.time_to_window,
                            departure_position_angle: h.departure_position_angle,
                            prograde_dv: h.departure_prograde,
                            radial_dv: h.departure_radial,
                            valid: true,
                        }
                    })
                } else {
                    None
                }
            } else {
                // Lambert mode — porkchop plot
                // Spawn background grid computation when target changes or grid is missing
                let need_grid = render_state.porkchop_grid.is_none()
                    || render_state.porkchop_last_target != Some(target_idx)
                    || render_state.porkchop_grid.as_ref()
                        .map_or(false, |g| {
                            // Regenerate when >10% of the departure window has elapsed,
                            // keeping the x-axis columns in the near future.
                            let threshold = g.dep_start + (g.dep_end - g.dep_start) * 0.1;
                            game.solar_system.time > threshold
                        });
                if need_grid && !render_state.porkchop_computing {
                    let bodies = &game.solar_system.bodies;
                    if let (Some(dep_orbit), Some(parent_idx)) =
                        (bodies[soi].orbit, bodies[soi].parent)
                    {
                        let tgt_orbit = bodies[target_idx].orbit.unwrap();
                        let grandparent_mass = bodies[parent_idx].mass;
                        let planet_mass = bodies[soi].mass;
                        let ship_orbit_copy = ship_orbit.orbit;
                        let ship_ma = ship_orbit.mean_anomaly;
                        let sim_time_now = game.solar_system.time;
                        let target = target_idx;

                        render_state.porkchop_job = transfer::PorkchopJob::new(
                            dep_orbit, tgt_orbit, grandparent_mass, planet_mass,
                            sim_time_now, ship_orbit_copy, ship_ma, target,
                        );
                        render_state.porkchop_computing = render_state.porkchop_job.is_some();
                        render_state.porkchop_last_target = Some(target_idx);
                        render_state.porkchop_grid = None;
                        render_state.porkchop_selected = None;
                        render_state.porkchop_hovered = None;
                    }
                }

                // Use selected/hovered/best point for full interplanetary computation
                if let Some(ref grid) = render_state.porkchop_grid {
                    let active_idx = render_state.porkchop_hovered
                        .or(render_state.porkchop_selected)
                        .or(grid.best_idx);
                    if let Some(idx) = active_idx {
                        if let Some(ref pt) = grid.points[idx] {
                            let arr_time = pt.dep_time + pt.tof;
                            transfer::compute_interplanetary(
                                &ship_orbit.orbit,
                                ship_orbit.retrograde,
                                ship_orbit.mean_anomaly,
                                soi,
                                target_idx,
                                pt.dep_time,
                                arr_time,
                                game.solar_system.time,
                                &game.solar_system.bodies,
                            ).map(|ip| {
                                let target_name = game.solar_system.bodies[target_idx].name.clone();
                                transfer::TransferDisplay {
                                    mode: 1,
                                    target_name,
                                    departure_dv: ip.ejection_delta_v,
                                    arrival_dv: ip.arrival_v_infinity,
                                    transfer_time: ip.transfer_time,
                                    current_phase_angle: ip.current_phase_angle.to_degrees(),
                                    required_phase_angle: ip.required_phase_angle.to_degrees(),
                                    time_to_window: ip.time_to_window,
                                    departure_position_angle: ip.departure_position_angle,
                                    prograde_dv: ip.ejection_delta_v,
                                    radial_dv: 0.0,
                                    valid: true,
                                }
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        } else {
            None
        };
    }

    // Compute time-to-node and burn time for the first maneuver node.
    // Computed live each frame from the ship's current orbit to avoid drift
    // over many orbits (numerical integration period != Keplerian period).
    render_state.time_to_node = None;
    render_state.burn_time = None;
    if let Some(first_node) = render_state.maneuver_nodes.first() {
        if let Some(ship_orbit) = game.flight.ship.get_cached_orbit() {
            if first_node.parent_idx == ship_orbit.parent_idx && ship_orbit.orbit.eccentricity < 1.0 {
                // Node's fixed inertial position angle
                let node_inertial = first_node.true_anomaly + first_node.argument_of_periapsis;
                // Project node onto ship's current orbit (same arg_peri for both → errors cancel)
                let node_ta = (node_inertial - ship_orbit.orbit.argument_of_periapsis)
                    .rem_euclid(std::f64::consts::TAU);
                let node_ma = game.flight.ship.true_to_mean_anomaly(&ship_orbit.orbit, node_ta);
                let ship_ma = ship_orbit.mean_anomaly;

                let delta_ma = if ship_orbit.retrograde {
                    (ship_ma - node_ma).rem_euclid(std::f64::consts::TAU)
                } else {
                    (node_ma - ship_ma).rem_euclid(std::f64::consts::TAU)
                };

                let parent_mass = game.solar_system.bodies[ship_orbit.parent_idx].effective_mass_at(ship_orbit.orbit.semi_major_axis);
                let mean_motion = ship_orbit.orbit.mean_motion(parent_mass);
                if mean_motion > 0.0 {
                    // Time to reach node on current orbit (0 to 1 period)
                    let time_one_pass = delta_ma / mean_motion;
                    let orbital_period = std::f64::consts::TAU / mean_motion;

                    // Use epoch to determine how many full orbits remain.
                    // The epoch gives the intended arrival time; we count how many
                    // complete orbits fit between now and the epoch, then add
                    // the within-orbit time for a drift-free countdown.
                    let epoch_remaining = first_node.epoch - game.time();
                    let full_orbits = if epoch_remaining > time_one_pass + orbital_period * 0.5 {
                        ((epoch_remaining - time_one_pass) / orbital_period).round() as u64
                    } else {
                        0
                    };

                    let ttn = time_one_pass + full_orbits as f64 * orbital_period;
                    if ttn > 0.0 {
                        render_state.time_to_node = Some(ttn);
                    }
                }
            }
        }

        // Compute burn time: dv / acceleration
        if let (Some(thrust_kn), Some(mass_tonnes)) = (render_state.vessel_thrust_kn, render_state.vessel_total_mass) {
            if thrust_kn > 0.0 && mass_tonnes > 0.0 {
                let accel = (thrust_kn * 1000.0) / (mass_tonnes * 1000.0); // m/s^2
                let remaining_dv = first_node.total_remaining_delta_v();
                render_state.burn_time = Some(remaining_dv / accel);
            }
        }
    }
    // Cancel warp-to-node if there are no nodes or no time computed
    if render_state.time_to_node.is_none() && render_state.warp_to_node {
        render_state.warp_to_node = false;
        game.warp_index = 0;
    }

    // Auto-warp-to-node logic
    if render_state.warp_to_node {
        if let Some(ttn) = render_state.time_to_node {
            let burn_half = render_state.burn_time.unwrap_or(0.0) / 2.0;
            let effective_time = ttn - burn_half;

            if effective_time <= 0.0 {
                // Past the burn start point — stop warping
                render_state.warp_to_node = false;
                game.warp_index = 0;
            } else if effective_time / WARP_LEVELS[5] < 0.25 {
                // Even minimum on-rails warp (100x) would arrive in < 0.25 real seconds
                // Drop to 1x and stop auto-warp
                render_state.warp_to_node = false;
                game.warp_index = 0;
            } else {
                // Find the highest warp level where we won't overshoot
                // (effective_time / warp_level >= 0.25 real seconds remaining)
                // Minimum auto-warp: index 5 (100x)
                let mut best_index = 5; // 100x minimum
                for i in (5..WARP_LEVELS.len()).rev() {
                    if effective_time / WARP_LEVELS[i] >= 0.25 {
                        best_index = i;
                        break;
                    }
                }
                game.warp_index = best_index;
            }
        }
    }

    let date_str = crate::game::format_date(game.time());

    // Determine if the ship can safely exit flight (go to main menu)
    // Cannot exit if in atmosphere or in landing zone while suborbital (and not landed)
    let is_landed = matches!(game.flight.ship.state, ShipState::Landed { .. });
    let can_exit_flight = is_landed || (
        !game.flight.ship.in_atmosphere(&game.solar_system)
        && !(game.flight.ship.below_landing_altitude(&game.solar_system)
             && game.flight.ship.is_suborbital(&game.solar_system))
    );
    let can_recover = match game.flight.ship.state {
        ShipState::Landed { body_index, .. } => crate::game::is_recoverable_body(body_index, &game.solar_system, &game.colony_manager),
        _ => false,
    };

    // Set economy/science HUD state
    render_state.company_money = game.company.money;
    render_state.science_available = game.science.available;

    // Set colony-related render state
    render_state.can_establish_colony = match game.flight.ship.state {
        ShipState::Landed { body_index, .. } => {
            render_state.landed_body_index = Some(body_index);
            !game.colony_manager.has_colony(body_index)
                && body_index != game.solar_system.earth_index
                && !game.solar_system.bodies[body_index].is_gas_giant
                && game.flight.vessel.as_ref().map_or(false, |v| v.has_colony_buildings())
        }
        _ => {
            render_state.landed_body_index = None;
            false
        }
    };
    render_state.has_colonies = !game.colony_manager.colonies.is_empty();
    render_state.vessel_has_cargo = game.flight.vessel.as_ref()
        .map_or(false, |v| v.has_cargo(&game.part_definitions));
    render_state.landed_body_has_colony = render_state.landed_body_index
        .map_or(false, |bi| game.colony_manager.has_colony(bi));

    let pre_render_warp_index = game.warp_index;
    match render_state.render(&body_names, WARP_LEVELS, game.warp_index, game.paused, &date_str, can_exit_flight, can_recover, game.has_launch_save, cached_quicksaves) {
        Ok((new_warp_index, pause_action)) => {
            game.warp_index = new_warp_index;
            // If user manually changed warp (clicked a button), cancel auto-warp
            if render_state.warp_to_node && new_warp_index != pre_render_warp_index {
                render_state.warp_to_node = false;
            }
            match pause_action {
                PauseAction::MainMenu => {
                    // Save active vessel to inactive list before leaving flight
                    game.flight.active_maneuver_nodes = std::mem::take(&mut render_state.maneuver_nodes);
                    game.flight.shelve_active_vessel(&game.solar_system);
                    game.enter_main_menu();
                }
                PauseAction::RecoverVessel => {
                    // Complete tourism contracts on recovery
                    let tourism_completions = game.contracts.check_tourism_recovery();
                    for (name, payout) in &tourism_completions {
                        game.company.money += payout;
                        log::info!("Tourism contract completed on recovery: {} — {}", name, crate::colony::format_money(*payout));
                        game.notifications.push(crate::colony::Notification {
                            kind: crate::colony::NotificationKind::ContractCompleted {
                                name: name.clone(),
                                payout: *payout,
                            },
                            time: game.solar_system.time,
                            read: false,
                        });
                    }
                    for _ in &tourism_completions {
                        game.contracts.refill_one(
                            &game.science.discoveries,
                            &game.solar_system,
                            game.solar_system.time,
                        );
                    }

                    // Determine landing body
                    let landed_body = match game.flight.ship.state {
                        ShipState::Landed { body_index, .. } => Some(body_index),
                        _ => None,
                    };
                    let is_earth = landed_body.map_or(false, |bi| bi == game.solar_system.earth_index);
                    let vessel_name = game.flight.active_vessel_name.clone();

                    if is_earth {
                        // === Earth Recovery: convert everything to cash at 100% ===
                        let mut total_value = 0.0_f64;

                        if let Some(ref vessel) = game.flight.vessel {
                            use crate::colony::economy::{material_breakdown, fuel_price_per_kg, LOX_PRICE_PER_KG};

                            // Part material value (non-decoupled parts only)
                            for part in &vessel.parts {
                                if part.decoupled || part.destroyed { continue; }
                                if let Some(def) = game.part_definitions.get(&part.definition_id) {
                                    let dry_mass_kg = def.mass * 1000.0;
                                    let breakdown = material_breakdown(def);
                                    let masses = breakdown.to_masses(dry_mass_kg);
                                    total_value += masses.earth_cost();
                                }
                            }

                            // Fuel value (remaining propellant in tanks)
                            for part in &vessel.parts {
                                if part.decoupled || part.destroyed { continue; }
                                for (res_name, &amount) in &part.resources {
                                    if amount <= 0.0 { continue; }
                                    // Map ship resource names to ResourceType for earth_price
                                    let price = match res_name.as_str() {
                                        "lox" => Some(LOX_PRICE_PER_KG),
                                        "rp1" => Some(fuel_price_per_kg(crate::parts::FuelType::Rp1)),
                                        "methane" => Some(fuel_price_per_kg(crate::parts::FuelType::Methane)),
                                        "hydrogen" => Some(fuel_price_per_kg(crate::parts::FuelType::Hydrogen)),
                                        "xenon" => Some(fuel_price_per_kg(crate::parts::FuelType::Xenon)),
                                        "fusion_fuel" | "deuterium" => Some(fuel_price_per_kg(crate::parts::FuelType::FusionFuel)),
                                        "antimatter" => Some(0.0), // Not purchasable
                                        "nuclear_pulse" => Some(fuel_price_per_kg(crate::parts::FuelType::NuclearPulse)),
                                        _ => None,
                                    };
                                    if let Some(p) = price {
                                        total_value += p * amount;
                                    }
                                }
                            }

                            // Cargo resource value
                            for part in &vessel.parts {
                                if part.decoupled || part.destroyed { continue; }
                                let is_cargo = game.part_definitions.get(&part.definition_id)
                                    .and_then(|d| d.cargo.as_ref()).is_some();
                                if !is_cargo { continue; }
                                for (res_name, &amount) in &part.resources {
                                    if amount <= 0.0 { continue; }
                                    if let Some(rt) = crate::colony::ResourceType::from_display_name(res_name) {
                                        if let Some(price) = rt.earth_price() {
                                            total_value += price * amount;
                                        }
                                    }
                                }
                            }
                        }

                        game.company.money += total_value;
                        let value_str = crate::colony::format_money(total_value);
                        log::info!("Earth recovery: {} — {}", vessel_name, value_str);

                        game.notifications.push(crate::colony::Notification {
                            kind: crate::colony::NotificationKind::VesselRecovered {
                                vessel_name: vessel_name.clone(),
                                location: "Earth".to_string(),
                                value_description: value_str,
                            },
                            time: game.solar_system.time,
                            read: false,
                        });
                    } else if let Some(body_index) = landed_body {
                        // === Colony Recovery ===
                        let colony_name = game.colony_manager.get_by_body(body_index)
                            .map(|c| c.name.clone()).unwrap_or_default();

                        if let Some(ref mut vessel) = game.flight.vessel {
                            // Extract crew to colony
                            let crew_count = vessel.total_crew_capacity(&game.part_definitions);
                            if let Some(colony) = game.colony_manager.get_by_body_mut(body_index) {
                                colony.crew += crew_count;
                            }

                            // Extract cargo (food, resources, buildings)
                            let (buildings, cargo_resources, food_kg) = vessel.extract_all_cargo(&game.part_definitions);
                            if let Some(colony) = game.colony_manager.get_by_body_mut(body_index) {
                                colony.food_stored += food_kg;
                                for (name, amount) in &cargo_resources {
                                    if let Some(rt) = crate::colony::ResourceType::from_display_name(name) {
                                        colony.resources.add(rt, *amount);
                                    }
                                }
                                // Buildings are added to colony buildings as instances
                                for name in &buildings {
                                    if let Some(bt) = crate::colony::BuildingType::from_display_name(name) {
                                        colony.buildings.push(crate::colony::BuildingInstance::new(bt));
                                    }
                                }
                            }

                            // Extract fuel from tanks to colony resources
                            for part in &vessel.parts {
                                if part.decoupled || part.destroyed { continue; }
                                for (res_name, &amount) in &part.resources {
                                    if amount <= 0.0 { continue; }
                                    let rt = match res_name.as_str() {
                                        "lox" => Some(crate::colony::ResourceType::Lox),
                                        "rp1" => Some(crate::colony::ResourceType::Rp1),
                                        "methane" => Some(crate::colony::ResourceType::Methane),
                                        "hydrogen" => Some(crate::colony::ResourceType::LiquidHydrogen),
                                        "xenon" => Some(crate::colony::ResourceType::Xenon),
                                        "fusion_fuel" | "deuterium" => Some(crate::colony::ResourceType::Deuterium),
                                        "antimatter" => Some(crate::colony::ResourceType::Antimatter),
                                        "nuclear_pulse" => Some(crate::colony::ResourceType::NuclearPulseUnits),
                                        _ => None,
                                    };
                                    if let Some(rt) = rt {
                                        if let Some(colony) = game.colony_manager.get_by_body_mut(body_index) {
                                            colony.resources.add(rt, amount);
                                        }
                                    }
                                }
                            }

                            // Check if stages were jettisoned (any decoupled parts)
                            let stages_jettisoned = vessel.parts.iter().any(|p| p.decoupled);

                            if stages_jettisoned {
                                // Convert remaining dry mass to resources via material breakdown
                                use crate::colony::economy::material_breakdown;
                                for part in &vessel.parts {
                                    if part.decoupled || part.destroyed { continue; }
                                    if let Some(def) = game.part_definitions.get(&part.definition_id) {
                                        let dry_mass_kg = def.mass * 1000.0;
                                        let breakdown = material_breakdown(def);
                                        let masses = breakdown.to_masses(dry_mass_kg);
                                        if let Some(colony) = game.colony_manager.get_by_body_mut(body_index) {
                                            colony.resources.add(crate::colony::ResourceType::StructuralMetal, masses.metal_kg);
                                            colony.resources.add(crate::colony::ResourceType::HighTempAlloys, masses.hta_kg);
                                            colony.resources.add(crate::colony::ResourceType::Electronics, masses.elec_kg);
                                            if masses.super_kg > 0.0 {
                                                colony.resources.add(crate::colony::ResourceType::Superconductors, masses.super_kg);
                                            }
                                            if masses.pi_kg > 0.0 {
                                                colony.resources.add(crate::colony::ResourceType::PrecisionInstruments, masses.pi_kg);
                                            }
                                        }
                                    }
                                }
                                log::info!("Colony recovery (staged): {} → resources at {}", vessel_name, colony_name);
                                game.notifications.push(crate::colony::Notification {
                                    kind: crate::colony::NotificationKind::VesselRecovered {
                                        vessel_name: vessel_name.clone(),
                                        location: colony_name.clone(),
                                        value_description: "converted to resources (staged)".to_string(),
                                    },
                                    time: game.solar_system.time,
                                    read: false,
                                });
                            } else {
                                // Build a StoredShip from the vessel's non-decoupled parts
                                let stored = crate::parts::FlightVessel::to_stored_ship(
                                    vessel,
                                    &game.part_definitions,
                                    vessel_name.clone(),
                                );
                                if let Some(colony) = game.colony_manager.get_by_body_mut(body_index) {
                                    if colony.has_hangar() && colony.hangar_used() + stored.dry_mass_kg <= colony.hangar_capacity() + 1e-3 {
                                        let ship_name = stored.name.clone();
                                        let _ = colony.store_ship(stored);
                                        log::info!("Colony recovery: {} stored in {} hangar", ship_name, colony_name);
                                        game.notifications.push(crate::colony::Notification {
                                            kind: crate::colony::NotificationKind::ShipStoredInHangar {
                                                ship_name,
                                                colony_name: colony_name.clone(),
                                            },
                                            time: game.solar_system.time,
                                            read: false,
                                        });
                                    } else {
                                        // No hangar or full — convert to resources
                                        use crate::colony::economy::blueprint_material_costs;
                                        let costs = blueprint_material_costs(&stored.blueprint, &game.part_definitions);
                                        for (res, amount) in &costs {
                                            colony.resources.add(*res, *amount);
                                        }
                                        log::info!("Colony recovery: {} → resources at {} (no hangar space)", vessel_name, colony_name);
                                        game.notifications.push(crate::colony::Notification {
                                            kind: crate::colony::NotificationKind::VesselRecovered {
                                                vessel_name: vessel_name.clone(),
                                                location: colony_name.clone(),
                                                value_description: "converted to resources (no hangar)".to_string(),
                                            },
                                            time: game.solar_system.time,
                                            read: false,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Discard maneuver nodes (vessel is recovered, not shelved)
                    render_state.maneuver_nodes.clear();
                    game.flight.active_maneuver_nodes.clear();
                    game.flight.vessel = None;
                    log::info!("Recovered vessel: {} (id={})", game.flight.active_vessel_name, game.flight.active_vessel_id);
                    game.enter_main_menu();
                }
                PauseAction::Quicksave => {
                    if let Some(ref name) = game.save_name {
                        game.flight.active_maneuver_nodes = render_state.maneuver_nodes.clone();
                        let save = SaveGame::from_game(game, name);
                        match save.write_quicksave() {
                            Ok(index) => log::info!("Quicksaved #{}", index),
                            Err(e) => log::error!("Quicksave failed: {}", e),
                        }
                        *quicksaves_dirty = true;
                    }
                }
                PauseAction::LoadQuicksave(filename) => {
                    if let Some(ref name) = game.save_name {
                        match SaveGame::load_quicksave(name, &filename) {
                            Ok(save) => {
                                save.restore_to_game(game);
                                render_state.maneuver_nodes = game.flight.active_maneuver_nodes.clone();
                                game.paused = false;
                                render_state.show_quicksave_list = false;
                            }
                            Err(e) => log::error!("Failed to load quicksave: {}", e),
                        }
                    }
                }
                PauseAction::RevertToLaunch => {
                    if let Some(ref name) = game.save_name {
                        match SaveGame::load_launch_save(name) {
                            Ok(save) => {
                                save.restore_to_game(game);
                                render_state.maneuver_nodes = game.flight.active_maneuver_nodes.clone();
                                game.has_launch_save = true;
                                game.paused = false;
                                log::info!("Reverted to launch");
                            }
                            Err(e) => log::error!("Failed to revert to launch: {}", e),
                        }
                    }
                }
                PauseAction::RevertToEditor => {
                    if let Some(ref name) = game.save_name {
                        match SaveGame::load_launch_save(name) {
                            Ok(save) => {
                                let blueprint = save.editor_blueprint.clone();
                                save.restore_to_game(game);
                                render_state.maneuver_nodes.clear();
                                if let Some(bp) = blueprint {
                                    game.editor.load_blueprint(&bp, &game.part_definitions);
                                }
                                game.enter_editor();
                                log::info!("Reverted to editor");
                            }
                            Err(e) => log::error!("Failed to revert to editor: {}", e),
                        }
                    }
                }
                PauseAction::Resume | PauseAction::None => {}
            }

            // F5 quicksave (keyboard shortcut, works while unpaused)
            if render_state.quicksave_requested {
                render_state.quicksave_requested = false;
                if let Some(ref name) = game.save_name {
                    game.flight.active_maneuver_nodes = render_state.maneuver_nodes.clone();
                    let save = SaveGame::from_game(game, name);
                    match save.write_quicksave() {
                        Ok(index) => log::info!("Quicksaved #{} (F5)", index),
                        Err(e) => log::error!("Quicksave failed: {}", e),
                    }
                    *quicksaves_dirty = true;
                }
            }

            // F9 quickload (keyboard shortcut — loads most recent quicksave)
            if render_state.quickload_requested {
                render_state.quickload_requested = false;
                if let Some(ref name) = game.save_name {
                    let quicksaves = SaveGame::list_quicksaves(name);
                    if let Some(latest) = quicksaves.first() {
                        match SaveGame::load_quicksave(name, &latest.filename) {
                            Ok(save) => {
                                save.restore_to_game(game);
                                render_state.maneuver_nodes = game.flight.active_maneuver_nodes.clone();
                                log::info!("Quickloaded (F9): {}", latest.filename);
                            }
                            Err(e) => log::error!("Quickload failed: {}", e),
                        }
                    }
                }
            }
        }
        Err(wgpu::SurfaceError::Lost) => {
            println!("Surface lost, resizing...");
            render_state.resize(render_state.size);
        }
        Err(wgpu::SurfaceError::OutOfMemory) => {
            eprintln!("Out of memory!");
            panic!("OutOfMemory");
        }
        Err(e) => eprintln!("Render error: {:?}", e),
    }

    // Drain the UI request queue. Requests are processed in FIFO order so that
    // multiple requests emitted on the same frame (e.g. two part pop-ups) apply
    // in the order the user clicked them.
    let requests: Vec<crate::render::RenderRequest> =
        render_state.render_requests.drain(..).collect();
    for req in requests {
        use crate::render::RenderRequest;
        match req {
            RenderRequest::TransferNode { position_angle, prograde_dv, radial_dv, time_to_window } => {
                // The position_angle is an inertial angle; convert to true anomaly using
                // the trajectory segment's arg_peri (not the ship orbit's, which is ill-defined
                // for near-circular parking orbits).
                if let Some(segment) = render_state.current_trajectory.first() {
                    let ta = crate::ship::transfer::normalize_angle(position_angle - segment.argument_of_periapsis);
                    let dv = crate::render::ManeuverDeltaV { prograde: prograde_dv, radial_out: radial_dv };
                    let epoch = game.time() + time_to_window;
                    let seg = segment.clone();
                    render_state.create_maneuver_node_with_epoch(ta, &seg, dv, epoch);
                }
                render_state.transfer_planner_open = false;
            }
            RenderRequest::EngineToggle { part_index, enabled } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len() {
                        vessel.parts[part_index].engine_enabled = enabled;
                    }
                }
            }
            RenderRequest::SolarDeploy { part_index, deploy } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len() {
                        vessel.parts[part_index].deploy_target = deploy;
                        // Sync mirror partner
                        if let Some(mirror_idx) = vessel.parts[part_index].mirror_partner {
                            if mirror_idx < vessel.parts.len() {
                                vessel.parts[mirror_idx].deploy_target = deploy;
                            }
                        }
                    }
                }
            }
            RenderRequest::RadiatorDeploy { part_index, deploy } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len() && vessel.parts[part_index].is_radiator {
                        vessel.parts[part_index].deploy_target = deploy;
                        if let Some(mirror_idx) = vessel.parts[part_index].mirror_partner {
                            if mirror_idx < vessel.parts.len() && vessel.parts[mirror_idx].is_radiator {
                                vessel.parts[mirror_idx].deploy_target = deploy;
                            }
                        }
                    }
                }
            }
            RenderRequest::ShieldToggle { part_index, active } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len() {
                        vessel.parts[part_index].shield_active = active;
                    }
                }
            }
            RenderRequest::ReactorRestart => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    // 100,000 Wh restart charge from batteries — matches design-doc
                    // reactor cold-start values (Fusion Small).
                    vessel.restart_reactors(100_000.0);
                }
            }
            RenderRequest::ParachuteDeploy { part_index } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len()
                        && vessel.parts[part_index].is_parachute
                        && !vessel.parts[part_index].parachute_spent
                        && !vessel.parts[part_index].parachute_deployed
                    {
                        vessel.parts[part_index].parachute_deployed = true;
                    }
                }
            }
            RenderRequest::ParachuteCut { part_index } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len()
                        && vessel.parts[part_index].is_parachute
                        && vessel.parts[part_index].parachute_deployed
                        && !vessel.parts[part_index].parachute_spent
                    {
                        vessel.parts[part_index].parachute_deployed = false;
                        vessel.parts[part_index].parachute_spent = true;
                        vessel.parts[part_index].parachute_deploy_fraction = 0.0;
                        vessel.parts[part_index].parachute_fully_deployed = false;
                    }
                }
            }
            RenderRequest::CrossfeedToggle { part_index, enabled } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len() {
                        vessel.parts[part_index].crossfeed_enabled = enabled;
                    }
                }
            }
            RenderRequest::Decouple { part_index } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len() && !vessel.parts[part_index].decoupled {
                        let def = game.part_definitions.get(&vessel.parts[part_index].definition_id);
                        if let Some(def) = def {
                            if let Some(ref dec_data) = def.decoupler {
                                // Store ejection force for handle_post_decouple
                                vessel.last_decouple_force = dec_data.ejection_force;

                                let decoupler_bottom = vessel.parts[part_index].local_position[1]
                                    - def.hitbox_height() / 2.0;

                                // Mark the decoupler itself as decoupled
                                vessel.parts[part_index].decoupled = true;

                                // Mark all parts whose top edge is at or below the decoupler bottom
                                for i in 0..vessel.parts.len() {
                                    if i == part_index || vessel.parts[i].decoupled {
                                        continue;
                                    }
                                    let other_def = game.part_definitions.get(&vessel.parts[i].definition_id);
                                    let other_top = if let Some(od) = other_def {
                                        vessel.parts[i].local_position[1] + od.hitbox_height() / 2.0
                                    } else {
                                        vessel.parts[i].local_position[1] + vessel.parts[i].hitbox_half_extents[1]
                                    };
                                    if other_top <= decoupler_bottom + 0.01 {
                                        vessel.parts[i].decoupled = true;
                                    }
                                }
                            }
                        }
                    }
                }
                handle_post_decouple(game);
                render_state.selected_flight_part = None;
            }
            RenderRequest::FairingDeploy { part_index } => {
                if let Some(ref mut vessel) = game.flight.vessel {
                    if part_index < vessel.parts.len() && !vessel.parts[part_index].decoupled {
                        let def = game.part_definitions.get(&vessel.parts[part_index].definition_id);
                        if let Some(def) = def {
                            if let Some(ref fairing_data) = def.fairing {
                                vessel.last_decouple_force = fairing_data.ejection_force;
                                vessel.parts[part_index].decoupled = true;
                            }
                        }
                    }
                }
                handle_post_decouple(game);
                render_state.selected_flight_part = None;
            }
            RenderRequest::DebugTeleportLeo => {
                let earth_idx = game.solar_system.earth_index;
                let earth = &game.solar_system.bodies[earth_idx];
                let leo_alt = 4.0e5; // 400 km
                let r = earth.radius + leo_alt;
                let mu = crate::bodies::G * earth.mass;
                let v_orb = (mu / r).sqrt();
                // Place at +Y, moving -X (counterclockwise, matching solar system convention)
                game.flight.ship.rel_position = [0.0, r];
                game.flight.ship.rel_velocity = [-v_orb, 0.0];
                game.flight.ship.soi_body = earth_idx;
                game.flight.ship.state = ShipState::Flying;
                game.flight.ship.on_rails = false;
                if let Some(ref mut vessel) = game.flight.vessel {
                    vessel.rel_position = game.flight.ship.rel_position;
                    vessel.rel_velocity = game.flight.ship.rel_velocity;
                }
            }
            RenderRequest::DebugTeleportBody { body_index } => {
                let body = &game.solar_system.bodies[body_index];
                let surface_angle = std::f64::consts::FRAC_PI_2; // Top of body
                let surface_distance = body.radius + 10.0; // 10m above surface
                let spawn_position = [
                    surface_distance * surface_angle.cos(),
                    surface_distance * surface_angle.sin(),
                ];

                // Move ship into the target body's SOI
                game.flight.ship.soi_body = body_index;
                game.flight.ship.rel_position = spawn_position;
                game.flight.ship.rel_velocity = [0.0, 0.0];
                game.flight.ship.rotation = surface_angle;
                game.flight.ship.rotational_velocity = 0.0;
                game.flight.ship.on_rails = false;
                game.flight.ship.state = ShipState::Landed {
                    body_index,
                    surface_angle,
                };

                if let Some(ref mut vessel) = game.flight.vessel {
                    vessel.rel_position = spawn_position;
                    vessel.rel_velocity = [0.0, 0.0];
                    vessel.rotation = surface_angle;
                }
                log::info!("Debug: teleported to landed on {}", body.name);
            }
            RenderRequest::EstablishColony { body_index } => {
                match game.establish_colony(body_index) {
                    Ok(()) => {
                        render_state.has_colonies = true;
                        render_state.can_establish_colony = false;
                    }
                    Err(e) => log::error!("Failed to establish colony: {}", e),
                }
            }
            RenderRequest::TransferCargo { body_index } => {
                if let Some(vessel) = game.flight.vessel.as_mut() {
                    let (building_names, resources, food_kg) = vessel.extract_all_cargo(&game.part_definitions);
                    if let Some(colony) = game.colony_manager.get_by_body_mut(body_index) {
                        // Add buildings
                        for name in &building_names {
                            if let Some(bt) = crate::colony::BuildingType::from_display_name(name) {
                                colony.buildings.push(crate::colony::BuildingInstance::new(bt));
                            }
                        }
                        // Add resources
                        for (name, amount) in &resources {
                            if let Some(rt) = crate::colony::ResourceType::from_display_name(name) {
                                colony.resources.add(rt, *amount);
                            }
                        }
                        // Add food
                        colony.food_stored += food_kg;
                        log::info!("Transferred cargo to colony: {} buildings, {} resource types, {:.0} kg food",
                            building_names.len(), resources.len(), food_kg);
                    } else {
                        log::error!("No colony on body {} to transfer cargo to", body_index);
                    }
                }
            }
            RenderRequest::OpenColony { body_index } => {
                game.enter_colony(body_index, GameMode::Flight);
            }
        }
    }

    // Process staging reorder request from flight staging panel
    if let Some(new_stages) = render_state.staging_reorder.take() {
        if let Some(ref mut vessel) = game.flight.vessel {
            vessel.stages = new_stages;
        }
    }

    // Process notifications — warp-stopping notifications and toasts
    for notif in &mut game.notifications {
        if !notif.read && notif.kind.stops_warp() {
            game.warp_index = 0;
            render_state.active_toasts.push((notif.kind.message(), Instant::now()));
            notif.read = true;
        } else if !notif.read {
            render_state.active_toasts.push((notif.kind.message(), Instant::now()));
            notif.read = true;
        }
    }

    // Expire old toasts (>5 seconds)
    render_state.active_toasts.retain(|(_, t)| t.elapsed().as_secs_f32() < 5.0);

    // Per-part thermal destruction and vessel splitting
    let debris_list = if let Some(ref mut vessel) = game.flight.vessel {
        let destroyed_parts = vessel.destroy_overheated_parts();
        if !destroyed_parts.is_empty() {
            // Clean up staging lists — remove destroyed part indices
            for stage in &mut vessel.stages {
                stage.retain(|idx| !destroyed_parts.contains(idx));
            }

            // Check for vessel split — collect debris before creating ships
            let debris = vessel.check_and_split(&game.part_definitions);

            // Recenter on new COM
            let com_offset = vessel.recenter_on_com(&game.part_definitions);
            let rot = game.flight.ship.rotation;
            game.flight.ship.rel_position[0] += com_offset[0] * rot.cos() - com_offset[1] * rot.sin();
            game.flight.ship.rel_position[1] += com_offset[0] * rot.sin() + com_offset[1] * rot.cos();

            debris
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Create debris vessels outside the vessel borrow (thermal breakup — no ejection force)
    for (debris_vessel, com_offset) in debris_list {
        game.flight.create_debris_vessel(debris_vessel, com_offset, 0.0, &game.solar_system, &game.part_definitions);
    }

    // Remove vessel if all parts destroyed
    if game.flight.vessel.as_ref().map(|v| !v.parts.iter().any(|p| !p.destroyed && !p.decoupled)).unwrap_or(false) {
        game.flight.vessel = None;
        game.flight.ship.temperature = AMBIENT_TEMPERATURE;
        game.flight.ship.heat_flux = 0.0;
        log::info!("Vessel completely destroyed by aerodynamic heating!");
    }
}

/// Compute scaled body positions for rendering (shared across flight, main menu, tracking station)
fn compute_scaled_positions(game: &Game) -> Vec<[f64; 2]> {
    let mut scaled: Vec<[f64; 2]> = Vec::with_capacity(game.solar_system.bodies.len());
    for i in 0..game.solar_system.bodies.len() {
        let pos = game.solar_system.body_position(i);
        let body = &game.solar_system.bodies[i];
        let scaled_pos = if let Some(parent_idx) = body.parent {
            let parent_scaled = scaled[parent_idx];
            let parent_unscaled = game.solar_system.body_position(parent_idx);
            let rel_x = pos[0] - parent_unscaled[0];
            let rel_y = pos[1] - parent_unscaled[1];
            [
                parent_scaled[0] + rel_x * BODY_SCALE,
                parent_scaled[1] + rel_y * BODY_SCALE,
            ]
        } else {
            pos
        };
        scaled.push(scaled_pos);
    }
    scaled
}

/// Build body render tuples from scaled positions
fn build_body_data(game: &Game, scaled_positions: &[[f64; 2]], in_galaxy_view: bool) -> Vec<(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)> {
    (0..game.solar_system.bodies.len())
        .map(|i| {
            let body = &game.solar_system.bodies[i];
            let pos = scaled_positions[i];
            // In galaxy view, only show root + its direct children (stars)
            let visible = if in_galaxy_view {
                body.parent.is_none() || body.parent.map_or(false, |p|
                    game.solar_system.bodies[p].parent.is_none())
            } else {
                true
            };
            let radius = if visible { body.radius * BODY_SCALE } else { 0.0 };
            let atmo_height = if visible { body.atmosphere.map(|a| a.visible_height()).unwrap_or(0.0) } else { 0.0 };
            let atmo_color = body.atmosphere.map(|a| a.color).unwrap_or([0.0; 3]);
            (pos[0], pos[1], radius, body.color, atmo_height, atmo_color, i)
        })
        .collect()
}

/// Build accretion disc render data from game bodies
fn build_accretion_disc_data(game: &Game) -> Vec<Option<crate::bodies::AccretionDisc>> {
    game.solar_system.bodies.iter()
        .map(|b| b.accretion_disc)
        .collect()
}

/// Inject catalog planets (and companion stars) as synthetic bodies when a catalog star is focused.
/// This gives them indicators, hover names, and orbit lines through the existing body pipeline.
/// Also builds BodyInfoData for each catalog planet for the tracking station info panel.
fn inject_catalog_planets(
    focused_star: Option<&crate::render::StarRenderData>,
    bodies: &mut Vec<(f64, f64, f64, [f32; 4], f64, [f32; 3], usize)>,
    orbits: &mut Vec<Option<OrbitRenderData>>,
    body_names: &mut Vec<String>,
    game_time: f64,
    num_real_bodies: usize,
    pixels_per_world_unit: f32,
    body_texture_map: &mut crate::render::textures::BodyTextureMap,
    catalog_body_info: &mut std::collections::HashMap<usize, BodyInfoData>,
) {
    use crate::galaxy::catalog::host_star_index;

    // Clear any previous dynamic texture mappings for catalog planets
    body_texture_map.clear_dynamic_entries(num_real_bodies);
    catalog_body_info.clear();

    let star = match focused_star {
        Some(s) if s.catalog_index > 0 => s,
        _ => return,
    };
    let sys = match crate::galaxy::catalog::lookup_system(star.catalog_index) {
        Some(s) => s,
        None => return,
    };

    let star_x = star.x; // meters (system barycenter / primary position)
    let star_y = star.y;
    let num_stars = sys.stars.len();

    // ── Compute per-star positions for multi-star systems ───────────────────
    // Flat single-barycenter model: every star in the system orbits the one
    // common system barycenter at (star_x, star_y). Each star's individual
    // orbit is derived from the FIRST binary_orbits entry in which it appears
    // (the tightest pair containing that star). No hierarchical group merging
    // and no nested barycenters — there is exactly one barycenter per system.
    let mut star_positions: Vec<[f64; 2]> = vec![[star_x, star_y]; num_stars];

    #[derive(Clone)]
    struct StarOrbitInfo {
        star_sma_m: f64,
        eccentricity: f64,
        period_s: f64,
        arg_peri: f64,            // 0 for star B side, PI for star A side
    }
    let mut star_orbit_info: Vec<Option<StarOrbitInfo>> = vec![None; num_stars];

    if num_stars > 1 && !sys.binary_orbits.is_empty() {
        for si in 0..num_stars {
            // Find the first binary_orbits entry this star participates in.
            let mut found: Option<(usize, &crate::galaxy::catalog::StarOrbitData, bool)> = None;
            for (pair_idx, pair) in sys.binary_orbits.iter().enumerate() {
                if pair.star_a == si {
                    found = Some((pair_idx, pair, true));
                    break;
                }
                if pair.star_b == si {
                    found = Some((pair_idx, pair, false));
                    break;
                }
            }
            let (pair_idx, pair, is_a) = match found {
                Some(x) => x,
                None => continue, // star isn't in any pair — leave at barycenter
            };

            let partner_idx = if is_a { pair.star_b } else { pair.star_a };
            if partner_idx >= num_stars { continue; }

            let m_star = sys.stars[si].mass_solar;
            let m_partner = sys.stars[partner_idx].mass_solar;
            let total_mass = m_star + m_partner;
            if total_mass <= 0.0 { continue; }

            // Star's individual SMA around the single system barycenter:
            // the pair's full separation scaled by the partner's mass fraction
            // (so this star's position plus the partner's symmetric position
            // would sum to the pair barycenter — which we declare to BE the
            // system barycenter).
            let pair_sma_m = pair.sma_au * 1.496e11;
            let star_sma_m = pair_sma_m * m_partner / total_mass;
            let period_s = pair.period_years * 365.25 * 86400.0;
            let mean_motion = std::f64::consts::TAU / period_s;

            // Deterministic phase offset per (catalog_index, pair_idx) so stars
            // in different pairs aren't all at periapsis. Stars sharing the
            // same pair get the same phase, keeping them in opposition.
            let seed = (star.catalog_index as u64)
                .wrapping_mul(2654435761)
                .wrapping_add((pair_idx as u64).wrapping_mul(11400714819323198549u64));
            let phase_frac = (seed as f64 / u64::MAX as f64).fract();
            let m0_pair = phase_frac * std::f64::consts::TAU;
            let mean_anomaly = m0_pair + mean_motion * game_time;

            // arg_peri = PI for the A side (flipped 180°), 0 for B side.
            // Combined with the shared mean_anomaly this keeps the two stars
            // of a pair diametrically opposite at all times.
            let arg_peri = if is_a { std::f64::consts::PI } else { 0.0 };

            let [ox, oy] = crate::galaxy::kepler_position(
                star_sma_m, pair.eccentricity, arg_peri, mean_anomaly,
            );
            star_positions[si] = [star_x + ox, star_y + oy];

            star_orbit_info[si] = Some(StarOrbitInfo {
                star_sma_m,
                eccentricity: pair.eccentricity,
                period_s,
                arg_peri,
            });
        }
    }

    let mut j = 0usize;

    // ── Push companion stars as synthetic bodies ────────────────────────────
    if num_stars > 1 {
        for (si, cat_star) in sys.stars.iter().enumerate() {
            let [sx, sy] = star_positions[si];
            let synthetic_idx = num_real_bodies + j;

            // Star color from spectral type temperature
            let temp = spectral_temperature(cat_star.spectral_type);
            let rgb = crate::galaxy::star_color::stellar_color(temp);
            let star_color_arr = [rgb[0], rgb[1], rgb[2], 1.0];
            let star_radius = (cat_star.radius_solar * 6.957e8).max(12_000.0); // meters, min 12km (neutron star) for visibility

            bodies.push((sx, sy, star_radius, star_color_arr, 0.0, [0.0, 0.0, 0.0], synthetic_idx));

            // Star orbit rendering: every star's ellipse is centered on the
            // single system barycenter at (star_x, star_y).
            if let Some(ref orbit_info) = star_orbit_info[si] {
                let orbit_color = [rgb[0] * 0.4, rgb[1] * 0.4, rgb[2] * 0.4, 0.5];
                orbits.push(Some(OrbitRenderData {
                    parent_x: star_x * SCALE,
                    parent_y: star_y * SCALE,
                    semi_major_axis: orbit_info.star_sma_m * SCALE,
                    eccentricity: orbit_info.eccentricity,
                    argument_of_periapsis: orbit_info.arg_peri,
                    color: orbit_color,
                }));
            } else {
                orbits.push(None);
            }

            body_names.push(cat_star.name.to_string());

            // Build BodyInfoData for this companion star
            let star_type_enum = crate::galaxy::catalog::spectral_to_star_type(cat_star.spectral_type);
            let star_mass_kg = cat_star.mass_solar * 1.989e30;
            let star_surface_gravity = G * star_mass_kg / (star_radius * star_radius);
            let star_temp = spectral_temperature(cat_star.spectral_type) as f64;

            // Filter planets belonging to this star
            let star_planets: Vec<crate::render::CatalogPlanetInfo> = sys.bodies.iter().filter(|b| {
                host_star_index(b.designation, sys.stars) == si
            }).map(|b| crate::render::CatalogPlanetInfo {
                name: b.name.to_string(),
                designation: b.designation.to_string(),
                temperature_k: b.temperature_k,
                gravity_g: b.gravity_g,
                habitability: b.habitability,
                has_atmosphere: b.atmosphere.is_some(),
                has_life: b.has_life,
                is_moon: b.is_moon,
                is_gas_giant: b.is_gas_giant,
            }).collect();

            // Binary orbit parameters for the orbit section
            let (orbit_sma, orbit_ecc, orbit_period) = if let Some(ref oi) = star_orbit_info[si] {
                (Some(oi.star_sma_m), Some(oi.eccentricity), Some(oi.period_s))
            } else {
                (None, None, None)
            };

            catalog_body_info.insert(synthetic_idx, BodyInfoData {
                name: cat_star.name.to_string(),
                description: if cat_star.description.is_empty() {
                    sys.description.to_string()
                } else {
                    cat_star.description.to_string()
                },
                radius_m: star_radius,
                surface_gravity_ms2: star_surface_gravity,
                mass_kg: star_mass_kg,
                atmosphere_pressure_pa: None,
                atmosphere_height_m: None,
                orbit_semi_major_axis_m: orbit_sma,
                orbit_eccentricity: orbit_ecc,
                orbit_period_s: orbit_period,
                mineable_resources: vec![],
                atmospheric_resources: vec![],
                habitability_score: 0,
                luminosity_solar: Some(cat_star.luminosity_solar),
                star_type: Some(star_type_enum.display_name().to_string()),
                temperature_k: Some(star_temp),
                soi_radius_m: None,
                is_galactic_orbit: false,
                catalog_stars: vec![],
                catalog_planets: star_planets,
                catalog_zone: Some(sys.zone),
                catalog_distance_ly: Some(sys.distance_ly),
                catalog_spectral: Some(cat_star.spectral_type.to_string()),
            });

            j += 1;
        }
    }

    // ── Push planets and moons as synthetic bodies ───────────���──────────────
    // Track world positions per catalog-body index so moons can orbit their parents.
    let mut body_positions: Vec<Option<[f64; 2]>> = vec![None; sys.bodies.len()];
    for (body_idx, body) in sys.bodies.iter().enumerate() {
        // Determine orbit parent: host star for planets, parent planet for moons.
        let (center_x, center_y, sma) = if body.is_moon {
            let parent_idx = match body.parent_body_idx {
                Some(p) => p,
                None => continue,
            };
            let Some([px, py]) = body_positions.get(parent_idx).copied().flatten() else {
                continue;
            };
            (px, py, body.orbit_sma_km * 1000.0)
        } else {
            let host_idx = host_star_index(body.designation, sys.stars);
            let [hx, hy] = if num_stars > 0 {
                star_positions[host_idx.min(num_stars - 1)]
            } else {
                // Starless system (e.g. Sgr A*): planets orbit the system center directly
                [star_x, star_y]
            };
            (hx, hy, body.orbit_sma_au * 1.496e11)
        };

        let ecc = body.orbit_ecc;
        let period_s = body.orbit_period_days * 86400.0;
        let mean_motion = std::f64::consts::TAU / period_s;
        let mean_anomaly = (body_idx as f64) * 2.399 + mean_motion * game_time;

        // Compute body position relative to its orbital center
        let [rel_x, rel_y] = crate::galaxy::kepler_position(sma, ecc, 0.0, mean_anomaly);
        let planet_x = center_x + rel_x;
        let planet_y = center_y + rel_y;
        body_positions[body_idx] = Some([planet_x, planet_y]);

        let synthetic_idx = num_real_bodies + j;
        let tex_name = body.name.to_lowercase().replace(' ', "_");

        // Use texture-derived color if available, else fall back to habitability coloring
        let color: [f32; 4] = if let Some(tex_color) = body_texture_map.color_for_name(&tex_name) {
            tex_color
        } else if body.has_life {
            [0.9, 0.75, 0.2, 1.0]
        } else if body.habitability > 30 {
            [0.3, 0.85, 0.4, 1.0]
        } else {
            [0.6, 0.6, 0.6, 0.8]
        };

        body_texture_map.register_body_index(synthetic_idx, body.name);

        let radius = body.radius_earth * 6.371e6; // meters
        bodies.push((planet_x, planet_y, radius, color, 0.0, [0.0, 0.0, 0.0], synthetic_idx));

        // Orbit line centered on parent (host star for planets, parent planet for moons)
        let body_world_radius = (radius * SCALE) as f32;
        let body_pixels = body_world_radius * pixels_per_world_unit * 2.0;
        if body_pixels >= 5.0 {
            orbits.push(None);
        } else {
            let orbit_color = [color[0] * 0.4, color[1] * 0.4, color[2] * 0.4, 0.5];
            orbits.push(Some(OrbitRenderData {
                parent_x: center_x * SCALE,
                parent_y: center_y * SCALE,
                semi_major_axis: sma * SCALE,
                eccentricity: ecc,
                argument_of_periapsis: 0.0,
                color: orbit_color,
            }));
        }

        body_names.push(body.name.to_string());

        // For planets, collect child moons into catalog_planets so they show in the info panel.
        // Moons have no sub-children (empty list).
        let planet_children: Vec<crate::render::CatalogPlanetInfo> = if body.is_moon {
            vec![]
        } else {
            sys.bodies.iter().filter(|b| {
                b.is_moon && b.parent_body_idx == Some(body_idx)
            }).map(|b| crate::render::CatalogPlanetInfo {
                name: b.name.to_string(),
                designation: b.designation.to_string(),
                temperature_k: b.temperature_k,
                gravity_g: b.gravity_g,
                habitability: b.habitability,
                has_atmosphere: b.atmosphere.is_some(),
                has_life: b.has_life,
                is_moon: true,
                is_gas_giant: b.is_gas_giant,
            }).collect()
        };

        // Build BodyInfoData for this catalog planet/moon
        catalog_body_info.insert(synthetic_idx, BodyInfoData {
            name: body.name.to_string(),
            description: body.description.to_string(),
            radius_m: body.radius_earth * 6.371e6,
            surface_gravity_ms2: body.gravity_g * 9.81,
            mass_kg: body.mass_earth * 5.972e24,
            atmosphere_pressure_pa: body.atmosphere.as_ref().map(|a| a.pressure_atm * 101325.0),
            atmosphere_height_m: body.atmosphere.as_ref().map(|a| a.scale_height_km * 5000.0),
            orbit_semi_major_axis_m: Some(sma),
            orbit_eccentricity: Some(body.orbit_ecc),
            orbit_period_s: Some(body.orbit_period_days.abs() * 86400.0),
            mineable_resources: body.resources.to_vec(),
            atmospheric_resources: body.atmosphere.as_ref().map(|_| {
                body.resources.iter().filter(|r| matches!(r,
                    crate::colony::ResourceType::AtmosphericCo2 |
                    crate::colony::ResourceType::GasGiantAtmosphere
                )).copied().collect::<Vec<_>>()
            }).unwrap_or_default(),
            habitability_score: body.habitability,
            luminosity_solar: None,
            star_type: None,
            temperature_k: Some(body.temperature_k),
            soi_radius_m: None,
            is_galactic_orbit: false,
            catalog_stars: vec![],
            catalog_planets: planet_children,
            catalog_zone: None,
            catalog_distance_ly: None,
            catalog_spectral: None,
        });

        j += 1;
    }
}

/// Estimate temperature from spectral type (for star coloring in inject_catalog_planets).
fn spectral_temperature(spec: &str) -> f32 {
    let bytes = spec.as_bytes();
    if bytes.is_empty() { return 5800.0; }
    let class = bytes[0] as char;
    let subtype: f32 = if bytes.len() > 1 && bytes[1].is_ascii_digit() {
        (bytes[1] - b'0') as f32
    } else {
        5.0
    };
    match class {
        'O' => 50000.0 - subtype * 2000.0,
        'B' => 30000.0 - subtype * 2000.0,
        'A' => 10000.0 - subtype * 250.0,
        'F' => 7500.0 - subtype * 150.0,
        'G' => 6000.0 - subtype * 80.0,
        'K' => 5200.0 - subtype * 150.0,
        'M' => 3700.0 - subtype * 185.0,
        'D' => 25000.0,
        _ => 5800.0,
    }
}

/// Build procedural star render data for stars near the Sun.
/// Renders stars within screen-width distance of the Sun, capped at 1000 ly.
/// Build the list of procedural stars visible near the camera.
fn build_procedural_star_data(
    game: &mut Game,
    render_state: &mut RenderState,
) -> Vec<crate::render::StarRenderData> {
    use crate::bodies::{G, LIGHT_YEAR, SECTOR_SIDE_METERS, SECTOR_GRID_HALF_METERS, SECTOR_GRID_SIZE, galactic_enclosed_mass};

    let aspect = render_state.camera.aspect_ratio as f64;
    let inv_zoom_scale = 1.0 / (render_state.camera.zoom as f64 * SCALE);

    // Screen half-extents in meters
    let half_w = aspect * inv_zoom_scale;
    let half_h = inv_zoom_scale;

    // Screen half-diagonal in meters
    let radius = (half_w * half_w + half_h * half_h).sqrt();

    // Stars become visible once zoomed out past solar-system scale (~0.1 ly).
    // At galaxy view scale, skip procedural stars (galaxy texture handles those)
    // but still show catalog stars so named systems remain visible and clickable.
    let galaxy_view = is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center);

    // Helper: if `star_data` is the focused star, sync camera position to its CURRENT-frame
    // position. Without this, update_tracking reads focused_star_world_pos from the PREVIOUS
    // frame (written during rendering), creating a 1-frame lag between the camera and the
    // companion bodies/planets that inject_catalog_planets positions at the current frame.
    // At high time warp (e.g. 1e12x), galactic orbital motion per frame is ~3.7e15 m for
    // nearby stars, causing severe on-screen drift of all orbiting bodies.
    //
    // The Sun doesn't have this problem because it's a real body tracked via `tracked_body`
    // against scaled_positions, which are computed fresh each frame.
    fn sync_focused(render_state: &mut RenderState, star: &crate::render::StarRenderData) {
        if let Some((sx, sy, si)) = render_state.focused_star_id {
            if star.sector_x == sx && star.sector_y == sy && star.sector_index == si {
                render_state.focused_star_world_pos = Some([star.x, star.y]);
                if render_state.tracked_body.is_none() && render_state.tracked_vessel.is_none() {
                    render_state.camera.position[0] = star.x * SCALE;
                    render_state.camera.position[1] = star.y * SCALE;
                    render_state.camera.body_center = render_state.camera.position;
                    render_state.camera.ship_offset = [0.0, 0.0];
                }
            }
        }
    }

    if radius < 0.1 * LIGHT_YEAR {
        // Zoomed in too close for any stars — except a focused one
        let mut out = Vec::new();
        if let Some((sx, sy, si)) = render_state.focused_star_id {
            if let Some(star_data) = lookup_focused_star(game, sx, sy, si) {
                out.push(star_data);
                sync_focused(render_state, &out[0]);
            }
        }
        ensure_sgr_a_catalog_stars(game, &mut out, game.solar_system.time);
        game.galaxy.tick();
        return out;
    }

    if galaxy_view {
        // Galaxy view: collect only catalog stars (they're few enough to always render).
        // Iterate catalog_by_sector directly — no sector cache generation overhead.
        let game_time = game.solar_system.time;
        let center = [
            (render_state.camera.body_center[0] + render_state.camera.ship_offset[0]) / SCALE,
            (render_state.camera.body_center[1] + render_state.camera.ship_offset[1]) / SCALE,
        ];
        let cull_half_w = half_w;
        let cull_half_h = half_h;

        let mut out: Vec<crate::render::StarRenderData> = Vec::new();

        // Always include focused star first
        if let Some((sx, sy, si)) = render_state.focused_star_id {
            if let Some(star_data) = lookup_focused_star(game, sx, sy, si) {
                out.push(star_data);
                sync_focused(render_state, out.last().unwrap());
            }
        }

        for (_coord, cat_stars) in &game.galaxy.catalog_by_sector {
            for star in cat_stars {
                // Propagate position via Kepler
                let m = star.mean_anomaly_0 + star.mean_motion * game_time;
                let [current_x, current_y] = crate::galaxy::kepler_position(
                    star.semi_major_axis,
                    star.eccentricity as f64,
                    star.arg_periapsis as f64,
                    m,
                );
                // Screen-bounds culling
                if (current_x - center[0]).abs() > cull_half_w
                    || (current_y - center[1]).abs() > cull_half_h
                {
                    continue;
                }

                let mass_solar = star.mass / 1.989e30;
                let orbital_period_s = if star.mean_motion > 0.0 {
                    std::f64::consts::TAU / star.mean_motion
                } else {
                    0.0
                };
                let lum_clamped = star.luminosity.max(0.001);
                let alpha = 0.5 + 0.5 * (lum_clamped.log10() / 4.0).clamp(0.0, 1.0);
                let lum_factor = (1.0 + (lum_clamped.ln() * 0.06) as f64).clamp(1.0, 2.5) as f32;
                let catalog_name = if star.catalog_index > 0 {
                    crate::galaxy::catalog::lookup_system(star.catalog_index).map(|s| s.name)
                } else {
                    None
                };

                let num_catalog_stars = if star.catalog_index > 0 {
                    crate::galaxy::catalog::lookup_system(star.catalog_index)
                        .map(|s| s.stars.len()).unwrap_or(0) as u8
                } else { 0 };
                out.push(crate::render::StarRenderData {
                    x: current_x,
                    y: current_y,
                    color: star.color,
                    luminosity: star.luminosity,
                    radius_m: star.radius_m,
                    temperature: star.temperature,
                    mass_solar,
                    star_type: star.star_type.display_name(),
                    catalog_prefix: star.star_type.catalog_prefix(),
                    sector_x: _coord.x,
                    sector_y: _coord.y,
                    sector_index: star.sector_index,
                    alpha,
                    lum_factor,
                    semi_major_axis_m: star.semi_major_axis,
                    eccentricity: star.eccentricity,
                    arg_periapsis: star.arg_periapsis,
                    orbital_period_s,
                    catalog_name,
                    catalog_index: star.catalog_index,
                    num_catalog_stars,
                });
            }
        }

        ensure_sgr_a_catalog_stars(game, &mut out, game_time);
        game.galaxy.tick();
        return out;
    }

    // Camera center in meters (current position at game time)
    let center = [
        (render_state.camera.body_center[0] + render_state.camera.ship_offset[0]) / SCALE,
        (render_state.camera.body_center[1] + render_state.camera.ship_offset[1]) / SCALE,
    ];

    let game_time = game.solar_system.time;

    // Screen half-extents for rectangular on-screen culling (capped at render limit).
    let cull_half_w = half_w.min(1000.0 * LIGHT_YEAR);
    let cull_half_h = half_h.min(1000.0 * LIGHT_YEAR);

    // Sector lookup radius: screen half-diagonal (covers full rectangle).
    let star_radius = radius.min(1000.0 * LIGHT_YEAR);

    // === Backward rotation for sector lookup ===
    // Stars are cached at their t=0 positions. To find which sectors contain stars
    // that are currently near the camera, rotate the camera position backward by
    // its own angular displacement to estimate where nearby stars were at t=0.
    let r_cam = (center[0] * center[0] + center[1] * center[1]).sqrt();
    let lookup_center = if r_cam > 0.0 && game_time != 0.0 {
        let omega_cam = (G * galactic_enclosed_mass(r_cam) / r_cam).sqrt() / r_cam;
        let theta_cam = center[1].atan2(center[0]);
        let theta_0_cam = theta_cam - omega_cam * game_time;
        [r_cam * theta_0_cam.cos(), r_cam * theta_0_cam.sin()]
    } else {
        center
    };

    const MAX_STARS: usize = 50_000;

    // Find sectors that overlap the render area around the lookup center (t=0 space).
    // Margin includes one full sector side so that edge sectors whose near-side stars
    // could be on screen are never missed, plus 30% of screen radius for rotation drift.
    let margin = star_radius * 0.3 + SECTOR_SIDE_METERS;
    let first_x = ((lookup_center[0] - star_radius - margin + SECTOR_GRID_HALF_METERS) / SECTOR_SIDE_METERS).floor() as i64;
    let last_x = ((lookup_center[0] + star_radius + margin + SECTOR_GRID_HALF_METERS) / SECTOR_SIDE_METERS).ceil() as i64;
    let first_y = ((lookup_center[1] - star_radius - margin + SECTOR_GRID_HALF_METERS) / SECTOR_SIDE_METERS).floor() as i64;
    let last_y = ((lookup_center[1] + star_radius + margin + SECTOR_GRID_HALF_METERS) / SECTOR_SIDE_METERS).ceil() as i64;

    let first_x = first_x.max(0).min(SECTOR_GRID_SIZE as i64) as u16;
    let last_x = last_x.max(0).min(SECTOR_GRID_SIZE as i64) as u16;
    let first_y = first_y.max(0).min(SECTOR_GRID_SIZE as i64) as u16;
    let last_y = last_y.max(0).min(SECTOR_GRID_SIZE as i64) as u16;

    // Collect sector coords and sort by distance from lookup center (closest first).
    // This ensures nearby stars are always collected before distant ones, so the
    // MAX_STARS cap preferentially keeps the closest stars.
    let mut sectors: Vec<(u16, u16, f64)> = Vec::new();
    for sy in first_y..last_y {
        for sx in first_x..last_x {
            let origin = crate::galaxy::density::sector_origin_meters(sx, sy);
            let sector_cx = origin[0] + 0.5 * SECTOR_SIDE_METERS;
            let sector_cy = origin[1] + 0.5 * SECTOR_SIDE_METERS;
            let dx = sector_cx - lookup_center[0];
            let dy = sector_cy - lookup_center[1];
            sectors.push((sx, sy, dx * dx + dy * dy));
        }
    }
    sectors.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    // Collect on-screen stars from distance-ordered sectors, then trim to MAX_STARS
    // by distance — produces an even circular cutoff instead of sector-shaped edges.
    // Collection cap (3× MAX_STARS) limits Kepler solves while giving the circular
    // trim enough data from all directions.
    const COLLECT_CAP: usize = MAX_STARS * 3;
    let mut out: Vec<crate::render::StarRenderData> = Vec::new();

    'sectors: for &(sx, sy, _) in &sectors {
        let coord = crate::bodies::SectorCoord { x: sx, y: sy };
        let stars = game.galaxy.get_sector(coord);
        for star in stars {
            // Propagate star on elliptical orbit via Kepler's equation
            let m = star.mean_anomaly_0 + star.mean_motion * game_time;
            let [current_x, current_y] = crate::galaxy::kepler_position(
                star.semi_major_axis,
                star.eccentricity as f64,
                star.arg_periapsis as f64,
                m,
            );
            // Rectangular screen-bounds check against current position
            if (current_x - center[0]).abs() > cull_half_w
                || (current_y - center[1]).abs() > cull_half_h
            {
                continue;
            }
            let mass_solar = star.mass / 1.989e30;
            let orbital_period_s = if star.mean_motion > 0.0 {
                std::f64::consts::TAU / star.mean_motion
            } else {
                0.0
            };
            // Pre-compute rendering values (avoids log/ln per star per frame in scene.rs)
            let lum_clamped = star.luminosity.max(0.001);
            let alpha = 0.5 + 0.5 * (lum_clamped.log10() / 4.0).clamp(0.0, 1.0);
            let lum_factor = (1.0 + (lum_clamped.ln() * 0.06) as f64).clamp(1.0, 2.5) as f32;
            let catalog_name = if star.catalog_index > 0 {
                crate::galaxy::catalog::lookup_system(star.catalog_index).map(|s| s.name)
            } else {
                None
            };
            let num_catalog_stars = if star.catalog_index > 0 {
                crate::galaxy::catalog::lookup_system(star.catalog_index)
                    .map(|s| s.stars.len()).unwrap_or(0) as u8
            } else { 0 };
            out.push(crate::render::StarRenderData {
                x: current_x,
                y: current_y,
                color: star.color,
                luminosity: star.luminosity,
                radius_m: star.radius_m,
                temperature: star.temperature,
                mass_solar,
                star_type: star.star_type.display_name(),
                catalog_prefix: star.star_type.catalog_prefix(),
                sector_x: sx,
                sector_y: sy,
                sector_index: star.sector_index,
                alpha,
                lum_factor,
                semi_major_axis_m: star.semi_major_axis,
                eccentricity: star.eccentricity,
                arg_periapsis: star.arg_periapsis,
                orbital_period_s,
                catalog_name,
                catalog_index: star.catalog_index,
                num_catalog_stars,
            });
            if out.len() >= COLLECT_CAP {
                break 'sectors;
            }
        }
    }

    // If over MAX_STARS, keep the closest by distance (even circular cutoff).
    if out.len() > MAX_STARS {
        let cx = center[0];
        let cy = center[1];
        out.select_nth_unstable_by(MAX_STARS, |a, b| {
            let da = (a.x - cx) * (a.x - cx) + (a.y - cy) * (a.y - cy);
            let db = (b.x - cx) * (b.x - cx) + (b.y - cy) * (b.y - cy);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(MAX_STARS);
    }

    // Ensure Sgr A* catalog stars are always present for orbit rendering
    ensure_sgr_a_catalog_stars(game, &mut out, game.solar_system.time);

    // Tick galaxy cache (LRU eviction)
    game.galaxy.tick();

    // Sync camera to focused star's current-frame position (fix 1-frame-lag drift).
    // In the sectored path, the focused star isn't pre-inserted — search the output.
    // If the focused star fell outside the visible set, fall back to lookup_focused_star
    // AND push it into `out` so that inject_catalog_planets can still find it. Without
    // this, focusing on a companion star in a multi-star system (e.g. Fomalhaut B at
    // ~0.9 ly from A) and zooming in below the sector scan radius for A would cause
    // inject_catalog_planets to early-return, dropping companion-star bodies, which
    // would in turn cause track_catalog_body to clear tracked_body and snap the camera
    // back to the barycenter on the next frame.
    if let Some((sx, sy, si)) = render_state.focused_star_id {
        let found = out.iter()
            .find(|s| s.sector_x == sx && s.sector_y == sy && s.sector_index == si)
            .map(|s| (s.x, s.y));
        if let Some((fx, fy)) = found {
            render_state.focused_star_world_pos = Some([fx, fy]);
            if render_state.tracked_body.is_none() && render_state.tracked_vessel.is_none() {
                render_state.camera.position[0] = fx * SCALE;
                render_state.camera.position[1] = fy * SCALE;
                render_state.camera.body_center = render_state.camera.position;
                render_state.camera.ship_offset = [0.0, 0.0];
            }
        } else if let Some(star_data) = lookup_focused_star(game, sx, sy, si) {
            sync_focused(render_state, &star_data);
            out.push(star_data);
        }
    }

    out
}

/// Look up the focused star from the galaxy cache by its sector coordinates.
/// Returns a StarRenderData if found, propagated to current game time.
fn lookup_focused_star(
    game: &mut Game,
    sector_x: u16,
    sector_y: u16,
    sector_index: u32,
) -> Option<crate::render::StarRenderData> {
    let coord = crate::bodies::SectorCoord { x: sector_x, y: sector_y };
    let stars = game.galaxy.get_sector(coord);
    let star = stars.iter().find(|s| s.sector_index == sector_index)?;

    // Propagate to current time
    let game_time = game.solar_system.time;
    let m = star.mean_anomaly_0 + star.mean_motion * game_time;
    let [current_x, current_y] = crate::galaxy::kepler_position(
        star.semi_major_axis,
        star.eccentricity as f64,
        star.arg_periapsis as f64,
        m,
    );
    let mass_solar = star.mass / 1.989e30;
    let orbital_period_s = if star.mean_motion > 0.0 {
        std::f64::consts::TAU / star.mean_motion
    } else {
        0.0
    };
    let lum_clamped = star.luminosity.max(0.001);
    let alpha = 0.5 + 0.5 * (lum_clamped.log10() / 4.0).clamp(0.0, 1.0);
    let lum_factor = (1.0 + (lum_clamped.ln() * 0.06) as f64).clamp(1.0, 2.5) as f32;
    let catalog_name = if star.catalog_index > 0 {
        crate::galaxy::catalog::lookup_system(star.catalog_index).map(|s| s.name)
    } else {
        None
    };
    let num_catalog_stars = if star.catalog_index > 0 {
        crate::galaxy::catalog::lookup_system(star.catalog_index)
            .map(|s| s.stars.len()).unwrap_or(0) as u8
    } else { 0 };
    Some(crate::render::StarRenderData {
        x: current_x,
        y: current_y,
        color: star.color,
        luminosity: star.luminosity,
        radius_m: star.radius_m,
        temperature: star.temperature,
        mass_solar,
        star_type: star.star_type.display_name(),
        catalog_prefix: star.star_type.catalog_prefix(),
        sector_x,
        sector_y,
        sector_index,
        alpha,
        lum_factor,
        semi_major_axis_m: star.semi_major_axis,
        eccentricity: star.eccentricity,
        arg_periapsis: star.arg_periapsis,
        orbital_period_s,
        catalog_name,
        catalog_index: star.catalog_index,
        num_catalog_stars,
    })
}

/// Ensure all Sgr A* catalog stars are present in the star list for orbit rendering.
/// Without this, non-focused Sgr A* catalog stars can drop out of the list due to viewport
/// culling or the 0.1 ly early-return, causing their orbits to disappear at different zoom
/// levels than when the star is focused.
fn ensure_sgr_a_catalog_stars(
    game: &Game,
    out: &mut Vec<crate::render::StarRenderData>,
    game_time: f64,
) {
    for (_coord, cat_stars) in &game.galaxy.catalog_by_sector {
        for star in cat_stars {
            if star.catalog_index == 0 { continue; }
            // Check if this is a Sgr A* orbit star
            let sys = match crate::galaxy::catalog::lookup_system(star.catalog_index) {
                Some(s) => s,
                None => continue,
            };
            if !sys.is_sgr_a_orbit || sys.stars.is_empty() { continue; }
            // Skip if already in the list
            if out.iter().any(|s| s.catalog_index == star.catalog_index) { continue; }
            // Propagate to current position and add
            let m = star.mean_anomaly_0 + star.mean_motion * game_time;
            let [current_x, current_y] = crate::galaxy::kepler_position(
                star.semi_major_axis,
                star.eccentricity as f64,
                star.arg_periapsis as f64,
                m,
            );
            let mass_solar = star.mass / 1.989e30;
            let orbital_period_s = if star.mean_motion > 0.0 {
                std::f64::consts::TAU / star.mean_motion
            } else {
                0.0
            };
            let lum_clamped = star.luminosity.max(0.001);
            let alpha = 0.5 + 0.5 * (lum_clamped.log10() / 4.0).clamp(0.0, 1.0);
            let lum_factor = (1.0 + (lum_clamped.ln() * 0.06) as f64).clamp(1.0, 2.5) as f32;
            let catalog_name = crate::galaxy::catalog::lookup_system(star.catalog_index).map(|s| s.name);
            let num_catalog_stars = sys.stars.len() as u8;
            out.push(crate::render::StarRenderData {
                x: current_x,
                y: current_y,
                color: star.color,
                luminosity: star.luminosity,
                radius_m: star.radius_m,
                temperature: star.temperature,
                mass_solar,
                star_type: star.star_type.display_name(),
                catalog_prefix: star.star_type.catalog_prefix(),
                sector_x: _coord.x,
                sector_y: _coord.y,
                sector_index: star.sector_index,
                alpha,
                lum_factor,
                semi_major_axis_m: star.semi_major_axis,
                eccentricity: star.eccentricity,
                arg_periapsis: star.arg_periapsis,
                orbital_period_s,
                catalog_name,
                catalog_index: star.catalog_index,
                num_catalog_stars,
            });
        }
    }
}

/// Build per-body star flag: true for stars (parent is root, no accretion disc, stellar mass).
/// Planets orbiting the root (e.g. Crucible orbiting Sgr A*) are excluded by the mass check.
fn build_body_is_star(game: &Game) -> Vec<bool> {
    const MIN_STAR_MASS_KG: f64 = 1e28; // ~5 Jupiter masses — well below stellar limit
    game.solar_system.bodies.iter().map(|body| {
        match body.parent {
            Some(parent_idx) => {
                game.solar_system.bodies[parent_idx].parent.is_none()
                    && body.accretion_disc.is_none()
                    && body.mass > MIN_STAR_MASS_KG
            }
            None => false,
        }
    }).collect()
}

/// Build orbit render data from scaled positions
fn build_orbit_data(game: &Game, scaled_positions: &[[f64; 2]], render_state: &RenderState) -> Vec<Option<OrbitRenderData>> {
    let pixels_per_world_unit = render_state.camera.zoom * render_state.size.height as f32 / 2.0;
    let in_galaxy_view = is_galaxy_view(render_state.camera.zoom, render_state.camera.body_center);
    (0..game.solar_system.bodies.len())
        .map(|i| {
            let body = &game.solar_system.bodies[i];
            match (body.parent, &body.orbit) {
                (Some(parent_idx), Some(orbit)) => {
                    let parent_body = &game.solar_system.bodies[parent_idx];
                    if parent_body.parent.is_none() {
                        // Body orbiting root (Sgr A*): STARS are handled by the
                        // catalog star orbit pipeline (star field view, focused-only
                        // visibility). Non-stars (e.g. Crucible, a planet at 13 AU)
                        // fall through to the normal body-orbit pipeline below.
                        // Matches the classification in build_body_is_star().
                        const MIN_STAR_MASS_KG: f64 = 1e28;
                        let is_star = body.accretion_disc.is_none()
                            && body.mass > MIN_STAR_MASS_KG;
                        if is_star {
                            return None;
                        }
                    }

                    // In galaxy view, skip all non-star orbits
                    if in_galaxy_view {
                        return None;
                    }

                    let body_world_radius = (body.radius * BODY_SCALE * SCALE) as f32;
                    let body_pixels = body_world_radius * pixels_per_world_unit * 2.0;
                    let is_moon = parent_body.parent
                        .map_or(false, |gp| game.solar_system.bodies[gp].parent.is_some());
                    let pixel_threshold = if is_moon { 100.0 } else { 5.0 };
                    if body_pixels >= pixel_threshold {
                        return None;
                    }
                    let parent_pos = scaled_positions[parent_idx];
                    let orbit_color = [
                        body.color[0] * 0.4,
                        body.color[1] * 0.4,
                        body.color[2] * 0.4,
                        0.5,
                    ];
                    Some(OrbitRenderData {
                        parent_x: parent_pos[0] * SCALE,
                        parent_y: parent_pos[1] * SCALE,
                        semi_major_axis: orbit.semi_major_axis * SCALE * BODY_SCALE,
                        eccentricity: orbit.eccentricity,
                        argument_of_periapsis: orbit.argument_of_periapsis,
                        color: orbit_color,
                    })
                }
                _ => None,
            }
        })
        .collect()
}

/// Build part render data from a FlightVessel for rendering inactive vessels
fn build_vessel_part_render_data(
    vessel: &crate::parts::FlightVessel,
    part_defs: &crate::parts::PartDefinitions,
) -> Vec<ShipPartRenderData> {
    vessel.parts.iter()
        .enumerate()
        .filter(|(_, p)| !p.destroyed && !p.decoupled)
        .map(|(i, p)| {
            let def = part_defs.get(&p.definition_id);
            let name = def.map(|d| d.name.clone()).unwrap_or_else(|| p.definition_id.clone());
            let dry_mass = def.map(|d| d.mass).unwrap_or(0.0);
            let is_engine = p.propellant_type.is_some();
            ShipPartRenderData {
                definition_id: p.definition_id.clone(),
                local_x: p.local_position[0],
                local_y: p.local_position[1],
                rotation: p.rotation,
                engine_active: false, // Inactive vessels don't fire engines
                part_index: i,
                name,
                dry_mass,
                hitbox_half_w: if is_part_rotation_swapped(p.rotation) { p.hitbox_half_extents[1] } else { p.hitbox_half_extents[0] },
                hitbox_half_h: if is_part_rotation_swapped(p.rotation) { p.hitbox_half_extents[0] } else { p.hitbox_half_extents[1] },
                click_local_y: p.local_position[1],
                click_hitbox_half_h: if is_part_rotation_swapped(p.rotation) { p.hitbox_half_extents[0] } else { p.hitbox_half_extents[1] },
                engine_thrust_vac: if is_engine { Some(p.engine_thrust_vac) } else { None },
                engine_thrust_asl: if is_engine { Some(p.engine_thrust_asl) } else { None },
                engine_isp_vac: if is_engine { Some(p.engine_isp_vac) } else { None },
                engine_isp_asl: if is_engine { Some(p.engine_isp_asl) } else { None },
                engine_enabled: p.engine_enabled,
                engine_no_power: p.engine_no_power,
                propellant_name: p.propellant_type.map(|pt| pt.display_name().to_string()),
                fuel_type_name: None,
                fuel_current: None,
                fuel_max: None,
                ox_current: None,
                ox_max: None,
                crew_capacity: def.and_then(|d| d.pod.as_ref().map(|pod| pod.crew_capacity)),
                monoprop_current: None,
                monoprop_max: None,
                battery_current: if p.max_electricity > 0.0 { Some(p.electricity) } else { None },
                battery_max: if p.max_electricity > 0.0 { Some(p.max_electricity) } else { None },
                solar_output: None, // Inactive vessels don't compute solar output
                rtg_output: def.and_then(|d| d.rtg.as_ref().map(|r| r.output_watts)),
                reactor_output: def.and_then(|d| d.reactor.as_ref().map(|r| r.output_watts)),
                reactor_waste_heat: def.and_then(|d| d.reactor.as_ref().filter(|r| r.waste_heat_watts > 0.0).map(|r| r.waste_heat_watts)),
                shield_type: def.and_then(|d| d.shield.as_ref().map(|s| format!("{:?}", s.shield_type))),
                shield_max_c: def.and_then(|d| d.shield.as_ref().map(|s| s.max_velocity_c)),
                shield_power: def.and_then(|d| d.shield.as_ref().map(|s| s.power_base_watts)),
                shield_active: p.shield_active,
                is_decoupler: def.map(|d| d.decoupler.is_some()).unwrap_or(false),
                crossfeed_enabled: p.crossfeed_enabled,
                gimbal_angle: 0.0,
                rcs_thrust: if p.rcs_thrust > 0.0 { Some(p.rcs_thrust) } else { None },
                rcs_nozzle_state: None,
                heat_fraction: ((p.temperature - 300.0) / (p.max_heat_tolerance - 300.0)).clamp(0.0, 1.0) as f32,
                temperature: p.temperature,
                is_fairing: def.map(|d| d.fairing.is_some()).unwrap_or(false),
                fairing_shape: p.fairing_shape.clone(),
                fairing_half: p.fairing_half,
                deploy_fraction: p.deploy_fraction,
                is_solar_panel: def.map(|d| d.solar_panel.is_some()).unwrap_or(false),
                is_radiator: p.is_radiator,
                radiator_tier: def.and_then(|d| d.radiator.as_ref().map(|r| r.tier)),
                radiator_rejection_watts: def.and_then(|d| d.radiator.as_ref().map(|r| r.rejection_watts)),
                engine_waste_heat: def.and_then(|d| d.engine.as_ref().filter(|e| e.waste_heat_watts > 0.0).map(|e| e.waste_heat_watts)),
                is_parachute: p.is_parachute,
                parachute_deployed: p.parachute_deployed,
                parachute_spent: p.parachute_spent,
                parachute_deploy_fraction: p.parachute_deploy_fraction,
                parachute_deployed_width_m: p.parachute_deployed_width_m,
                parachute_fully_deployed: p.parachute_fully_deployed,
                sprite_half_h: def.map(|d| d.height() / 2.0).unwrap_or(0.0),
            }
        })
        .collect()
}

/// Build tracking vessel data for all vessels (active + inactive)
fn build_tracking_vessel_data(
    game: &Game,
    scaled_positions: &[[f64; 2]],
) -> Vec<crate::render::TrackingVesselData> {
    use crate::render::TrackingVesselData;

    let mut vessels = Vec::new();

    // All vessels are in inactive_vessels when not in flight
    for v in &game.flight.inactive_vessels {
        // Use scaled_positions + rel_position for precision at galaxy-scale distances
        let soi_pos = scaled_positions[v.ship.soi_body];
        let rel = v.ship.rel_position;
        let orbit_data = v.ship.get_render_orbit().map(|(orbit, parent_idx)| {
            let parent_pos = scaled_positions[parent_idx];
            crate::render::OrbitRenderData {
                parent_x: parent_pos[0] * SCALE,
                parent_y: parent_pos[1] * SCALE,
                semi_major_axis: orbit.semi_major_axis * SCALE * BODY_SCALE,
                eccentricity: orbit.eccentricity,
                argument_of_periapsis: orbit.argument_of_periapsis,
                color: [0.6, 0.6, 0.6, 0.4], // Grey for all vessels in tracking station
            }
        });
        let parts = v.vessel.as_ref().map(|fv| {
            build_vessel_part_render_data(fv, &game.part_definitions)
        });
        vessels.push(TrackingVesselData {
            id: v.id,
            name: v.name.clone(),
            color: v.ship.color,
            x: soi_pos[0] * SCALE + rel[0] * SCALE * BODY_SCALE,
            y: soi_pos[1] * SCALE + rel[1] * SCALE * BODY_SCALE,
            body_center: [soi_pos[0] * SCALE, soi_pos[1] * SCALE],
            rel_offset: [rel[0] * SCALE * BODY_SCALE, rel[1] * SCALE * BODY_SCALE],
            soi_body: v.ship.soi_body,
            orbit: orbit_data,
            parts,
            rotation: v.ship.rotation,
            is_debris: v.is_debris,
        });
    }

    vessels
}


/// Handle a TradeAction returned by the trade route UI.
fn handle_trade_action(action: crate::render::TradeAction, game: &mut Game) {
    use crate::render::TradeAction;

    match action {
        TradeAction::None => {}
        TradeAction::CreateRoute { route, .. } => {
            // Route is created; automation will build and launch ships on schedule
            game.fleet.create_route(route);
        }
        TradeAction::PauseRoute(route_id) => {
            if let Some(route) = game.fleet.get_route_mut(route_id) {
                route.paused = true;
            }
        }
        TradeAction::ResumeRoute(route_id) => {
            if let Some(route) = game.fleet.get_route_mut(route_id) {
                route.paused = false;
                route.alert_reason = None; // Clear alert so automation retries
            }
        }
        TradeAction::DeleteRoute(route_id) => {
            game.fleet.delete_route(route_id);
        }
        TradeAction::DeleteShip(ship_id) => {
            game.fleet.delete_ship(ship_id);
        }
        TradeAction::EditRoute { route_id, route } => {
            if let Some(existing) = game.fleet.get_route_mut(route_id) {
                // Update route fields, preserving id, assigned_ship_id, last_launch_time
                existing.name = route.name;
                existing.blueprint_name = route.blueprint_name;
                existing.legs = route.legs;
                existing.outbound_cargo = route.outbound_cargo;
                existing.crew = route.crew;
                existing.total_delta_v = route.total_delta_v;
                existing.route_category = route.route_category;
                existing.interval_days = route.interval_days;
                existing.ships_per_window = route.ships_per_window;
                existing.alert_reason = None; // Clear alert so automation retries with new settings
            }
        }
        TradeAction::OpenEditor(_) => {
            // Handled at call site (colony overview intercepts this before calling handle_trade_action)
        }
    }
}


/// Handle flight mode mouse input
fn handle_flight_mouse_input(
    game: &mut Game,
    render_state: &mut RenderState,
    state: ElementState,
    button: MouseButton,
    egui_consumed: bool,
    last_click_time: &mut Option<Instant>,
    last_click_pos: &mut [f32; 2],
) {
    const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(300);
    const DOUBLE_CLICK_DIST: f32 = 10.0;

    // Part selection runs unconditionally so clicking on a part always works,
    // even when the part info window overlaps the ship and egui consumes the click.
    let part_clicked = if button == MouseButton::Left && state == ElementState::Pressed {
        let mouse_pos = render_state.camera.last_mouse_pos;
        if let Some(cache_idx) = render_state.flight_part_at_screen_pos(mouse_pos[0], mouse_pos[1]) {
            render_state.selected_flight_part = Some(cache_idx);
            true
        } else {
            false
        }
    } else {
        false
    };

    if !egui_consumed && button == MouseButton::Left {
        if state == ElementState::Pressed {
            let now = Instant::now();
            let mouse_pos = render_state.camera.last_mouse_pos;
            let dx = mouse_pos[0] - last_click_pos[0];
            let dy = mouse_pos[1] - last_click_pos[1];
            let dist = (dx * dx + dy * dy).sqrt();

            if !part_clicked {
                // Check if clicking on a maneuver node
                if let Some(node_id) = render_state.maneuver_node_at_screen_pos(mouse_pos[0], mouse_pos[1]) {
                    render_state.start_dragging_node(node_id);
                    render_state.selected_maneuver_node = Some(node_id);
                    render_state.pending_orbit_click = None;
                    render_state.selected_flight_part = None;
                } else {
                    render_state.selected_flight_part = None;
                }
            }

            // Helper: single-click target/orbit detection
            let single_click = |game: &Game, render_state: &mut RenderState, mouse_pos: [f32; 2]| {
                // Check body click → show target popup (works for real + catalog planets)
                if let Some(body_idx) = render_state.body_at_screen_pos(mouse_pos[0], mouse_pos[1]) {
                    let name = render_state.body_names.get(body_idx).cloned().unwrap_or_default();
                    render_state.target_popup = Some(TargetPopup {
                        target: SelectedTarget::Body(body_idx),
                        name,
                    });
                    render_state.pending_orbit_click = None;
                }
                // Check background vessel click → show target popup
                else if let Some(vessel_id) = render_state.background_vessel_at_screen_pos(mouse_pos[0], mouse_pos[1]) {
                    // Find vessel name from tracking data
                    let name = game.flight.inactive_vessels.iter()
                        .find(|v| v.id == vessel_id)
                        .map(|v| v.name.clone())
                        .unwrap_or_else(|| format!("Vessel {}", vessel_id));
                    render_state.target_popup = Some(TargetPopup {
                        target: SelectedTarget::Vessel(vessel_id),
                        name,
                    });
                    render_state.pending_orbit_click = None;
                }
                // Check orbit click → pending maneuver node
                else if render_state.dragging_maneuver_node.is_none() {
                    render_state.target_popup = None;
                    if let Some(orbit_pos) = render_state.orbit_click_position(mouse_pos[0], mouse_pos[1]) {
                        render_state.pending_orbit_click = Some(orbit_pos);
                        render_state.selected_maneuver_node = None;
                    } else {
                        render_state.pending_orbit_click = None;
                    }
                }
            };

            // Check for double-click on body or vessel
            if let Some(last_time) = *last_click_time {
                if now.duration_since(last_time) < DOUBLE_CLICK_TIME && dist < DOUBLE_CLICK_DIST {
                    // Double-click: focus camera on body, vessel, or procedural star
                    render_state.target_popup = None;
                    if let Some(vessel_id) = render_state.background_vessel_at_screen_pos(mouse_pos[0], mouse_pos[1]) {
                        switch_to_next_vessel_by_id(game, render_state, vessel_id);
                        render_state.focused_star = None;
                        render_state.focused_star_world_pos = None;
                        render_state.focused_star_id = None;
                    } else if let Some(body_idx) = render_state.body_at_screen_pos_tight(mouse_pos[0], mouse_pos[1]) {
                        let name = render_state.body_names.get(body_idx).cloned().unwrap_or_default();
                        render_state.focus_on_body(body_idx); // Preserves focused_star for catalog planets
                        game.flight.tracking_ship = false;
                        println!("Focused on: {}", name);
                    } else if let Some(body_idx) = render_state.body_at_screen_pos_loose(mouse_pos[0], mouse_pos[1]) {
                        // Check loose (indicator radius) before star dot — ensures companion
                        // star bodies are found before the ProceduralStar dot re-centers on A.
                        let name = render_state.body_names.get(body_idx).cloned().unwrap_or_default();
                        render_state.focus_on_body(body_idx); // Preserves focused_star for catalog planets
                        game.flight.tracking_ship = false;
                        println!("Focused on: {}", name);
                    } else if let Some(star_idx) = render_state.star_at_screen_pos(mouse_pos[0], mouse_pos[1]) {
                        if let Some(star) = render_state.current_procedural_stars.get(star_idx) {
                            let world_x = star.x * SCALE;
                            let world_y = star.y * SCALE;
                            render_state.camera.focus_on([world_x, world_y]);
                            render_state.tracked_body = None;
                            render_state.tracked_vessel = None;
                            render_state.focused_star = Some(star_idx);
                            render_state.focused_star_world_pos = Some([star.x, star.y]);
                            render_state.focused_star_id = Some((star.sector_x, star.sector_y, star.sector_index));
                            game.flight.tracking_ship = false;
                            println!("Focused on star: {}", star.format_name());
                        }
                    }
                    *last_click_time = None;
                } else {
                    single_click(game, render_state, mouse_pos);
                    *last_click_time = Some(now);
                    *last_click_pos = mouse_pos;
                }
            } else {
                single_click(game, render_state, mouse_pos);
                *last_click_time = Some(now);
                *last_click_pos = mouse_pos;
            }
        } else {
            render_state.stop_dragging_node();
        }
    }
}

/// Handle editor mode mouse input
fn handle_editor_mouse_input(
    game: &mut Game,
    render_state: &mut RenderState,
    state: ElementState,
    button: MouseButton,
    egui_consumed: bool,
) {
    if egui_consumed {
        return;
    }

    let mouse_pos = render_state.camera.last_mouse_pos;
    let screen_width = render_state.size.width as f32;
    let screen_height = render_state.size.height as f32;

    // Fairing build mode intercepts all mouse input
    if game.editor.fairing_build_mode.is_some() {
        if button == MouseButton::Left && state == ElementState::Pressed {
            game.editor.add_fairing_vertex(&game.part_definitions);
        } else if button == MouseButton::Right && state == ElementState::Pressed {
            game.editor.undo_fairing_vertex();
        }
        return;
    }

    if button == MouseButton::Left {
        if state == ElementState::Pressed {
            // Check if clicking on a placed part - start dragging it
            if let Some(part_id) = part_at_screen_pos(
                mouse_pos[0], mouse_pos[1],
                screen_width, screen_height,
                &game.editor, &game.part_definitions
            ) {
                let mouse_world = screen_to_world(mouse_pos[0], mouse_pos[1], screen_width, screen_height, &game.editor);
                game.editor.start_drag(part_id, mouse_world);
            } else if game.editor.selected_part_def.is_some() {
                // Try to place a part
                game.editor.place_part(&game.part_definitions);
            }
        } else {
            // Mouse released - finish dragging if active
            if game.editor.is_dragging() {
                game.editor.finish_drag(&game.part_definitions);
            }
        }
    } else if button == MouseButton::Right && state == ElementState::Pressed {
        // Right-click to cancel drag, deselect, or delete
        if game.editor.is_dragging() {
            game.editor.cancel_drag();
        } else if game.editor.selected_part_def.is_some() {
            game.editor.deselect();
        } else if let Some(part_id) = game.editor.selected_placed_part {
            game.editor.delete_part(part_id);
            game.editor.selected_placed_part = None;
        }
    } else if button == MouseButton::Middle {
        // Middle mouse for camera drag
        if state == ElementState::Pressed {
            render_state.camera.is_dragging = true;
        } else {
            render_state.camera.is_dragging = false;
        }
    }
}

/// Handle flight mode cursor movement
fn handle_flight_cursor_moved(
    render_state: &mut RenderState,
    x: f32,
    y: f32,
    egui_consumed: bool,
) {
    if render_state.dragging_maneuver_node.is_some() {
        render_state.update_dragged_node(x, y);
    }

    render_state.camera.last_mouse_pos = [x, y];

    if !egui_consumed {
        render_state.update_hover(x, y);
    } else {
        // Clear hover when cursor is over an egui panel
        render_state.hovered_star = None;
    }
}

/// Handle editor mode cursor movement
fn handle_editor_cursor_moved(
    game: &mut Game,
    render_state: &mut RenderState,
    x: f32,
    y: f32,
    egui_consumed: bool,
) {
    let screen_width = render_state.size.width as f32;
    let screen_height = render_state.size.height as f32;

    if render_state.camera.is_dragging && !egui_consumed {
        let dx = x - render_state.camera.last_mouse_pos[0];
        let dy = y - render_state.camera.last_mouse_pos[1];
        game.editor.pan_camera(-dx as f64, dy as f64);
    }

    render_state.camera.last_mouse_pos = [x, y];

    if !egui_consumed {
        let [world_x, world_y] = screen_to_world(x, y, screen_width, screen_height, &game.editor);

        // Fairing build mode: update ghost point
        if game.editor.fairing_build_mode.is_some() {
            game.editor.update_fairing_ghost(world_x, world_y);
            return;
        }

        // Update drag position if dragging a part
        if game.editor.is_dragging() {
            game.editor.update_drag(world_x, world_y, &game.part_definitions);
        } else {
            // Update ghost position (only when not dragging)
            game.editor.update_ghost(world_x, world_y, &game.part_definitions);

            // Update hovered part
            game.editor.hovered_part = part_at_screen_pos(
                x, y, screen_width, screen_height,
                &game.editor, &game.part_definitions
            );
        }
    }
}

/// After decoupling, extract decoupled parts into a debris vessel and recenter the active vessel.
fn handle_post_decouple(game: &mut Game) {
    // Read ejection force before extracting (set by activate_next_stage or manual decouple)
    let ejection_force = game.flight.vessel.as_ref()
        .map(|v| v.last_decouple_force).unwrap_or(0.0);

    // Extract fairing halves BEFORE extracting decoupled parts
    let fairing_halves = game.flight.vessel.as_mut()
        .map(|v| v.extract_fairing_halves(&game.part_definitions))
        .unwrap_or_default();

    for (debris_vessel, com_offset, half) in fairing_halves {
        let sign = match half {
            crate::parts::FairingHalf::Left => -1.0,
            crate::parts::FairingHalf::Right => 1.0,
        };
        game.flight.create_fairing_debris(debris_vessel, com_offset, 5.0, sign, &game.solar_system);
    }

    // Extract decoupled parts into debris (separate step to avoid borrow conflict)
    let extracted = game.flight.vessel.as_mut()
        .and_then(|v| {
            let result = v.extract_decoupled_parts(&game.part_definitions);
            v.last_decouple_force = 0.0; // Reset after reading
            result
        });

    if let Some((debris_vessel, com_offset)) = extracted {
        game.flight.create_debris_vessel(debris_vessel, com_offset, ejection_force, &game.solar_system, &game.part_definitions);
    }

    // Recenter parts on new COM and shift ship position to match
    if let Some(ref mut vessel) = game.flight.vessel {
        let com_offset = vessel.recenter_on_com(&game.part_definitions);
        let rot = game.flight.ship.rotation - std::f64::consts::FRAC_PI_2;
        game.flight.ship.rel_position[0] += com_offset[0] * rot.cos() - com_offset[1] * rot.sin();
        game.flight.ship.rel_position[1] += com_offset[0] * rot.sin() + com_offset[1] * rot.cos();
    }
}

/// Handle flight mode keyboard input
fn handle_flight_keyboard(
    game: &mut Game,
    render_state: &mut RenderState,
    logical_key: &Key,
    pressed: bool,
) {
    if let Key::Named(named_key) = logical_key {
        match named_key {
            winit::keyboard::NamedKey::Space => {
                if pressed {
                    let can_stage = game.flight.vessel.as_ref()
                        .map_or(true, |v| v.has_control(&game.part_definitions));
                    if can_stage {
                        let in_atmo = game.flight.ship.in_atmosphere(&game.solar_system);
                        let is_landed = matches!(game.flight.ship.state, ShipState::Landed { .. });
                        if let Some(ref mut vessel) = game.flight.vessel {
                            vessel.activate_next_stage(&game.part_definitions, in_atmo, is_landed);
                        }
                        handle_post_decouple(game);
                    }
                }
            }
            winit::keyboard::NamedKey::Shift => game.flight.ship_input.throttle_up = pressed,
            winit::keyboard::NamedKey::Control => game.flight.ship_input.throttle_down = pressed,
            winit::keyboard::NamedKey::F5 => {
                if pressed { render_state.quicksave_requested = true; }
            }
            winit::keyboard::NamedKey::F9 => {
                if pressed { render_state.quickload_requested = true; }
            }
            _ => {}
        }
    }

    if let Key::Character(c) = logical_key {
        match c.as_str() {
            "w" | "W" => game.flight.ship_input.translate_forward = pressed,
            "s" | "S" => game.flight.ship_input.translate_backward = pressed,
            "a" | "A" => game.flight.ship_input.translate_left = pressed,
            "d" | "D" => game.flight.ship_input.translate_right = pressed,
            "q" | "Q" => game.flight.ship_input.rotate_left = pressed,
            "e" | "E" => game.flight.ship_input.rotate_right = pressed,
            "z" | "Z" => game.flight.ship_input.throttle_full = pressed,
            "x" | "X" => game.flight.ship_input.throttle_zero = pressed,
            "r" | "R" => {
                if pressed {
                    render_state.rcs_enabled = !render_state.rcs_enabled;
                }
            }
            "`" => {
                if pressed {
                    game.flight.tracking_ship = true;
                    render_state.tracked_body = None;
                    println!("Focused on: Ship");
                }
            }
            "[" | "]" => {
                if pressed {
                    switch_to_next_vessel(game, render_state, c.as_str() == "]");
                }
            }
            _ => {}
        }
    }
}

/// Switch to the next/previous vessel in the sorted vessel list.
fn switch_to_next_vessel(game: &mut Game, render_state: &mut RenderState, forward: bool) {
    if game.flight.inactive_vessels.is_empty() {
        return;
    }

    let ids = game.flight.all_vessel_ids();
    let current_pos = ids.iter().position(|&id| id == game.flight.active_vessel_id).unwrap_or(0);
    let next_pos = if forward {
        (current_pos + 1) % ids.len()
    } else {
        (current_pos + ids.len() - 1) % ids.len()
    };

    let target_id = ids[next_pos];
    if target_id == game.flight.active_vessel_id {
        return;
    }

    switch_to_next_vessel_by_id(game, render_state, target_id);
}

/// Switch directly to a specific vessel by ID
fn switch_to_next_vessel_by_id(game: &mut Game, render_state: &mut RenderState, target_id: u64) {
    if target_id == game.flight.active_vessel_id {
        return;
    }

    game.flight.active_maneuver_nodes = std::mem::take(&mut render_state.maneuver_nodes);
    match game.flight.switch_to_vessel(target_id, &game.solar_system, &game.part_definitions) {
        Ok(()) => {
            render_state.maneuver_nodes = game.flight.active_maneuver_nodes.clone();
            render_state.tracked_body = None;
            game.warp_index = 0;
            log::info!("Switched to vessel: {} (id={})", game.flight.active_vessel_name, target_id);
        }
        Err(e) => log::error!("Failed to switch vessel: {}", e),
    }
}

/// Returns true if rotation is approximately 90° or 270° (dimensions should be swapped)
fn is_part_rotation_swapped(rotation: f64) -> bool {
    let norm = rotation.rem_euclid(std::f64::consts::TAU);
    let quarter = std::f64::consts::FRAC_PI_2;
    (norm - quarter).abs() < 0.01 || (norm - 3.0 * quarter).abs() < 0.01
}

/// Handle editor mode keyboard input
fn handle_editor_keyboard(
    game: &mut Game,
    logical_key: &Key,
    pressed: bool,
    egui_consumed: bool,
) {
    // Arrow keys track held state for smooth camera movement
    if let Key::Named(key) = logical_key {
        match key {
            winit::keyboard::NamedKey::ArrowUp => {
                game.editor.keys_held.up = pressed;
            }
            winit::keyboard::NamedKey::ArrowDown => {
                game.editor.keys_held.down = pressed;
            }
            winit::keyboard::NamedKey::ArrowLeft => {
                game.editor.keys_held.left = pressed;
            }
            winit::keyboard::NamedKey::ArrowRight => {
                game.editor.keys_held.right = pressed;
            }
            winit::keyboard::NamedKey::Delete | winit::keyboard::NamedKey::Backspace if pressed => {
                if let Some(part_id) = game.editor.selected_placed_part {
                    game.editor.delete_part(part_id);
                    game.editor.selected_placed_part = None;
                }
            }
            _ => {}
        }
    }

    // Character keys only on press, and only when egui doesn't have focus
    if pressed && !egui_consumed {
        if let Key::Character(c) = logical_key {
            match c.as_str() {
                "r" | "R" => {
                    // Rotate ghost or placed part by 90° clockwise
                    if game.editor.selected_part_def.is_some() {
                        // Ghost mode: rotate ghost preview
                        game.editor.ghost_rotation = (game.editor.ghost_rotation - std::f64::consts::FRAC_PI_2)
                            .rem_euclid(std::f64::consts::TAU);
                    } else if let Some(part_id) = game.editor.selected_placed_part {
                        // Placed part selected: rotate it in place (with overlap check)
                        if let Some(part) = game.editor.parts.get(&part_id) {
                            let new_rot = (part.rotation - std::f64::consts::FRAC_PI_2)
                                .rem_euclid(std::f64::consts::TAU);
                            let def_id = part.definition_id.clone();
                            let pos = part.position;
                            let mirror_id = part.mirror_partner;
                            if let Some(def) = game.part_definitions.get(&def_id) {
                                // Check if rotated hitbox would overlap
                                let new_bounds = crate::editor::EditorState::calc_bounds_pub(
                                    pos,
                                    def.rotated_hitbox_width(new_rot),
                                    def.rotated_hitbox_height(new_rot),
                                );
                                let mut overlaps = false;
                                for (&other_id, other_part) in &game.editor.parts {
                                    if other_id == part_id || Some(other_id) == mirror_id {
                                        continue;
                                    }
                                    if let Some(other_def) = game.part_definitions.get(&other_part.definition_id) {
                                        let other_bounds = crate::editor::EditorState::calc_bounds_pub(
                                            other_part.position,
                                            other_def.rotated_hitbox_width(other_part.rotation),
                                            other_def.rotated_hitbox_height(other_part.rotation),
                                        );
                                        if crate::editor::EditorState::bounds_overlap_pub(&new_bounds, &other_bounds) {
                                            overlaps = true;
                                            break;
                                        }
                                    }
                                }
                                if !overlaps {
                                    if let Some(part) = game.editor.parts.get_mut(&part_id) {
                                        part.rotation = new_rot;
                                    }
                                    // Also rotate mirror partner (opposite direction)
                                    if let Some(mid) = mirror_id {
                                        if let Some(mirror_part) = game.editor.parts.get_mut(&mid) {
                                            mirror_part.rotation = (-new_rot).rem_euclid(std::f64::consts::TAU);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Handle tracking station mouse input (body double-click focus, camera drag)
fn handle_tracking_station_mouse_input(
    _game: &mut Game,
    render_state: &mut RenderState,
    state: ElementState,
    button: MouseButton,
    egui_consumed: bool,
    last_click_time: &mut Option<Instant>,
    last_click_pos: &mut [f32; 2],
) {
    const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(300);
    const DOUBLE_CLICK_DIST: f32 = 10.0;

    if !egui_consumed && button == MouseButton::Left {
        if state == ElementState::Pressed {
            let now = Instant::now();
            let mouse_pos = render_state.camera.last_mouse_pos;
            let dx = mouse_pos[0] - last_click_pos[0];
            let dy = mouse_pos[1] - last_click_pos[1];
            let dist = (dx * dx + dy * dy).sqrt();

            // Double-click on body or procedural star to focus
            let mut was_double_click = false;
            if let Some(last_time) = *last_click_time {
                if now.duration_since(last_time) < DOUBLE_CLICK_TIME && dist < DOUBLE_CLICK_DIST {
                    if let Some(body_idx) = render_state.body_at_screen_pos_tight(mouse_pos[0], mouse_pos[1]) {
                        let name = render_state.body_names.get(body_idx).cloned().unwrap_or_default();
                        render_state.focus_on_body(body_idx); // Preserves focused_star for catalog planets
                        println!("Focused on: {}", name);
                        was_double_click = true;
                    } else if let Some(body_idx) = render_state.body_at_screen_pos_loose(mouse_pos[0], mouse_pos[1]) {
                        // Check loose (indicator radius) before star dot — ensures companion
                        // star bodies are found before the ProceduralStar dot re-centers on A.
                        let name = render_state.body_names.get(body_idx).cloned().unwrap_or_default();
                        render_state.focus_on_body(body_idx); // Preserves focused_star for catalog planets
                        println!("Focused on: {}", name);
                        was_double_click = true;
                    } else if let Some(star_idx) = render_state.star_at_screen_pos(mouse_pos[0], mouse_pos[1]) {
                        if let Some(star) = render_state.current_procedural_stars.get(star_idx) {
                            let world_x = star.x * SCALE;
                            let world_y = star.y * SCALE;
                            render_state.camera.focus_on([world_x, world_y]);
                            render_state.tracked_body = None;
                            render_state.tracked_vessel = None;
                            render_state.focused_star = Some(star_idx);
                            render_state.focused_star_world_pos = Some([star.x, star.y]);
                            render_state.focused_star_id = Some((star.sector_x, star.sector_y, star.sector_index));
                            println!("Focused on star: {}", star.format_name());
                            was_double_click = true;
                        }
                    }
                    *last_click_time = None;
                } else {
                    *last_click_time = Some(now);
                    *last_click_pos = mouse_pos;
                }
            } else {
                *last_click_time = Some(now);
                *last_click_pos = mouse_pos;
            }

            // Only start drag if this wasn't a double-click focus
            if !was_double_click {
                render_state.camera.is_dragging = true;
            }
        } else {
            render_state.camera.is_dragging = false;
        }
    }
}

/// Handle tracking station cursor movement (camera drag, hover)
fn handle_tracking_station_cursor_moved(
    render_state: &mut RenderState,
    x: f32,
    y: f32,
    egui_consumed: bool,
) {
    if render_state.camera.is_dragging && !egui_consumed {
        // Only pan if not tracking anything; tracking holds camera on the object
        if render_state.tracked_body.is_none() && render_state.tracked_vessel.is_none() {
            let dx = x - render_state.camera.last_mouse_pos[0];
            let dy = y - render_state.camera.last_mouse_pos[1];
            let scale = 2.0 / render_state.size.height as f32;
            render_state.camera.pan(dx * scale, dy * scale);
        }
    }

    render_state.camera.last_mouse_pos = [x, y];

    if !egui_consumed {
        render_state.update_hover(x, y);
    } else {
        // Clear hover when cursor is over an egui panel
        render_state.hovered_star = None;
    }
}

/// Oriented Bounding Box overlap test using the Separating Axis Theorem.
fn obb_overlap(
    pos_a: [f64; 2], rot_a: f64, hw_a: f64, hh_a: f64,
    pos_b: [f64; 2], rot_b: f64, hw_b: f64, hh_b: f64,
) -> bool {
    let (sin_a, cos_a) = rot_a.sin_cos();
    let (sin_b, cos_b) = rot_b.sin_cos();

    // Two axes per OBB: local right and local up
    let axes = [
        [cos_a, sin_a],
        [-sin_a, cos_a],
        [cos_b, sin_b],
        [-sin_b, cos_b],
    ];
    let halves_a = [hw_a, hh_a];
    let halves_b = [hw_b, hh_b];

    let d = [pos_b[0] - pos_a[0], pos_b[1] - pos_a[1]];

    for axis in &axes {
        let dist_proj = (d[0] * axis[0] + d[1] * axis[1]).abs();

        let a_proj = (axes[0][0] * axis[0] + axes[0][1] * axis[1]).abs() * halves_a[0]
            + (axes[1][0] * axis[0] + axes[1][1] * axis[1]).abs() * halves_a[1];

        let b_proj = (axes[2][0] * axis[0] + axes[2][1] * axis[1]).abs() * halves_b[0]
            + (axes[3][0] * axis[0] + axes[3][1] * axis[1]).abs() * halves_b[1];

        if dist_proj > a_proj + b_proj {
            return false;
        }
    }
    true
}
