use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::ship::AutopilotTarget;
use super::camera::Camera;
use super::textures::BodyTextureMap;
use super::types::{
    BodyData, ManeuverNode, ShipOrbitData, Vertex,
};

/// Render FPS counter in the top-right corner of the screen.
/// Call inside an egui run closure.
pub fn fps_overlay(ctx: &egui::Context, fps: f32) {
    egui::Area::new(egui::Id::new("fps_overlay"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 4.0))
        .interactable(false)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(format!("{:.0} fps", fps))
                .size(12.0)
                .color(egui::Color32::from_rgba_premultiplied(120, 120, 120, 180)));
        });
}

/// Main render state holding all wgpu resources
pub struct RenderState {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub render_pipeline: wgpu::RenderPipeline,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_indices: u32,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub camera: Camera,
    pub window: Arc<Window>,
    pub msaa_texture: wgpu::Texture,
    pub msaa_view: wgpu::TextureView,
    pub bodies: Vec<BodyData>,
    pub earth_index: usize,
    pub hovered_body: Option<usize>,
    pub tracked_body: Option<usize>, // Body the camera is following
    pub ship_orbit_info: Option<ShipOrbitData>, // Stored for UI display
    pub fps: f32,              // Frames per second (smoothed)
    pub ship_velocity: f64,    // Current velocity (m/s)
    pub ship_altitude: f64,    // Current altitude (m)
    pub ship_vertical_speed: f64, // Radial velocity (m/s, positive = ascending)
    pub ship_throttle: f64,    // Current throttle (0.0 to 1.0)
    pub ship_soi_name: String, // Current SOI body name
    pub ship_time_to_intercept: Option<f64>, // Time to next SOI transition (seconds)
    pub ship_acceleration: f64,            // Ship's max thrust acceleration (m/s^2)
    pub ship_current_true_anomaly: f64,    // Ship's current position in its orbit (radians)
    // Vessel stats for HUD
    pub vessel_total_mass: Option<f64>,    // tonnes
    pub vessel_fuel_fraction: Option<f64>, // 0.0-1.0
    pub vessel_monoprop_fraction: Option<f64>, // 0.0-1.0
    pub vessel_power_generation: Option<f64>,  // Watts
    pub vessel_power_consumption: Option<f64>, // Watts
    pub vessel_electricity_fraction: Option<f64>, // 0.0-1.0
    pub vessel_electricity_stored: Option<f64>,   // Wh
    pub vessel_electricity_max: Option<f64>,     // Wh max capacity
    pub vessel_thrust_kn: Option<f64>,     // kN
    pub vessel_drag_kn: f64,               // kN, aerodynamic drag
    pub vessel_delta_v: Option<f64>,       // m/s
    pub vessel_current_stage: Option<usize>,  // Stages activated so far
    pub vessel_total_stages: Option<usize>,   // Total stages
    pub vessel_stages: Vec<Vec<super::types::StagedPartInfo>>,  // Full stage data for UI
    pub vessel_stage_delta_vs: Vec<f64>,  // Per-stage delta-v (m/s, vacuum)
    pub vessel_stage_burn_times: Vec<f64>,  // Per-stage burn time at 100% thrust (seconds)
    pub staging_reorder: Option<Vec<Vec<usize>>>,  // Request to reorder stages (part indices)
    pub ship_soi_surface_gravity: f64,     // m/s², for TWR
    pub ship_g_force: f64,                 // Felt acceleration in g's (thrust + drag, not gravity)
    // Thermal state
    pub ship_temperature: f64,            // Kelvin (hottest part — aero/spillover heat)
    pub ship_heat_fraction: f32,          // 0.0-1.0, for visual effects
    pub ship_heat_flux: f64,              // W/m², for HUD display
    // Ship-wide waste-heat pool (separate from per-part aero temperature)
    pub thermal_pool_temp: f64,           // Kelvin
    pub thermal_pool_gen_w: f64,          // Waste heat generated this tick (W)
    pub thermal_pool_reject_w: f64,       // Radiator rejection this tick (W)
    pub reactors_tripped: bool,           // True when thermal cascade has shut down reactors
    // Life support
    pub vessel_food_days: Option<f64>,     // Days of food remaining
    pub vessel_total_crew: Option<u32>,    // Living crew count
    pub vessel_is_starving: bool,          // True when food == 0 and crew > 0
    pub reactor_restart_request: bool,    // UI -> game request to attempt restart
    pub quicksave_requested: bool,        // F5 keyboard shortcut
    pub quickload_requested: bool,        // F9 keyboard shortcut
    pub ship_below_landing_altitude: bool, // Whether warp > 10x should be blocked
    pub ship_velocity_direction: [f64; 2], // Normalized velocity unit vector for prograde arrow
    // Relativistic state
    pub ship_speed_fraction_c: f64,
    pub ship_lorentz_gamma: f64,
    pub ship_proper_time: f64,
    pub ship_mission_time: f64,
    pub ship_is_relativistic: bool,
    pub ship_grav_time_factor: f64,
    // Part click state
    pub selected_flight_part: Option<usize>,  // index into flight_parts_cache
    pub flight_parts_cache: Vec<super::types::ShipPartRenderData>,
    pub ship_render_x: f64,
    pub ship_render_y: f64,
    pub ship_orbits_root: bool,     // Ship is directly orbiting the root body (Sgr A*)
    pub ship_body_center: [f64; 2],  // SOI body position in render units (large, galaxy-scale)
    pub ship_rel_offset: [f64; 2],   // Ship offset from SOI body in render units (small, local)
    pub ship_render_rotation: f64,
    pub ship_render_scale: f64,     // SCALE * BODY_SCALE used for rendering
    /// Queue of UI-emitted events to be applied to game state after each frame.
    /// Writers push via `self.render_requests.push(...)`; `main.rs` drains in FIFO order.
    pub render_requests: Vec<super::types::RenderRequest>,
    pub ship_has_control: bool,    // Whether the vessel has a functioning command pod
    pub ship_in_atmosphere: bool,  // Whether the active vessel is in atmosphere
    pub ship_is_landed: bool,      // Whether the active vessel is landed
    pub ap_markers: Vec<([f64; 2], f64)>, // Apoapsis markers: (world pos relative to camera, altitude)
    pub pe_markers: Vec<([f64; 2], f64)>, // Periapsis markers: (world pos relative to camera, altitude)
    pub closest_approach_world_pos: Option<([f64; 2], [f64; 2], f64)>, // (parent render pos, orbit offset, distance meters)
    pub closest_approach_marker: Option<([f64; 2], f64)>, // (camera-relative pos, distance meters) - for egui hover
    pub target_closest_approach_world_pos: Option<([f64; 2], [f64; 2], f64)>, // (parent render pos, orbit offset, distance meters)
    pub target_closest_approach_marker: Option<([f64; 2], f64)>, // Camera-relative for egui hover
    // Maneuver node state
    pub pending_orbit_click: Option<(f64, super::types::OrbitSegmentData)>,  // (true_anomaly, segment_data) - awaiting node creation
    pub selected_maneuver_node: Option<u64>,        // ID of selected node
    pub maneuver_nodes: Vec<ManeuverNode>,
    pub time_to_node: Option<f64>,      // seconds until first maneuver node
    pub burn_time: Option<f64>,         // estimated burn duration (seconds)
    pub warp_to_node: bool,             // auto-warp to node active
    pub next_node_id: u64,
    pub maneuver_node_screen_positions: Vec<(u64, [f32; 2])>, // (node_id, screen_pos) for click detection
    pub current_trajectory: Vec<super::types::OrbitSegmentData>, // Stored for click detection
    pub predicted_trajectories: Vec<Vec<super::types::OrbitSegmentData>>, // Predicted trajectories after maneuver burns (one per node)
    pub dragging_maneuver_node: Option<u64>, // ID of node being dragged
    // Background vessel screen positions for click detection
    pub background_vessel_screen_positions: Vec<(u64, [f32; 2])>,
    // Vessel the camera is following (tracking station)
    pub tracked_vessel: Option<u64>,
    // Autopilot state
    pub autopilot_target: AutopilotTarget,
    // Navigation target state
    pub selected_target: Option<super::types::SelectedTarget>,
    pub selected_target_name: String,
    pub selected_target_angle: Option<f64>,
    pub target_popup: Option<super::types::TargetPopup>,
    // RCS toggle (off by default)
    pub rcs_enabled: bool,
    // RCS was auto-disabled when entering on-rails warp; re-enable when returning to physics warp
    pub rcs_disabled_by_rails: bool,
    // Transfer planner state
    pub transfer_planner_open: bool,
    pub transfer_planner_mode: u8,              // 0 = Hohmann, 1 = Lambert
    pub transfer_selected_target: Option<usize>,
    pub porkchop_grid: Option<super::types::PorkchopGrid>,
    pub porkchop_selected: Option<usize>,   // locked selection (click)
    pub porkchop_hovered: Option<usize>,    // transient hover
    pub porkchop_last_target: Option<usize>, // to detect target changes
    /// In-flight chunked Lambert sweep for the porkchop plot. Driven a few
    /// rows per frame from the run loop.
    pub porkchop_job: Option<crate::ship::transfer::PorkchopJob>,
    pub porkchop_computing: bool,
    pub transfer_display: Option<crate::ship::transfer::TransferDisplay>,
    pub transfer_hohmann_targets: Vec<(usize, String)>,
    pub transfer_interplanetary_targets: Vec<(usize, String)>,
    // Quicksave UI state
    pub show_quicksave_list: bool,
    // Debug menu
    pub debug_menu_open: bool,
    pub debug_infinite_fuel: bool,
    // Body textures
    pub body_texture: wgpu::Texture,
    pub body_texture_bind_group: wgpu::BindGroup,
    pub body_texture_map: BodyTextureMap,
    /// Lazy fetcher used on wasm to populate `body_texture` layers from
    /// `/textures/bodies/<name>.png` as the user zooms in. None on native.
    #[cfg(target_arch = "wasm32")]
    pub body_texture_fetcher: super::body_fetcher::BodyTextureFetcher,
    // Sprite atlas
    pub sprite_atlas: super::sprites::SpriteAtlas,
    pub plume_start_time: web_time::Instant,
    // Economy/science HUD state
    pub company_money: f64,
    pub science_available: f64,
    pub show_contracts: bool,
    // Colony UI state
    pub can_establish_colony: bool,
    pub has_colonies: bool,
    pub landed_body_index: Option<usize>,
    pub vessel_has_cargo: bool,  // Whether the vessel has non-empty cargo containers
    pub landed_body_has_colony: bool,  // Whether the body we're landed on has a colony
    // Trade route creation wizard state
    pub route_creation: super::trade_ui::RouteCreationState,
    // Procedural star interaction state
    pub procedural_star_screen_positions: Vec<(usize, [f32; 2])>, // (index, screen pos in pixels)
    pub hovered_star: Option<usize>,       // index into current_procedural_stars
    pub focused_star: Option<usize>,       // index into current_procedural_stars (camera tracking)
    pub focused_star_world_pos: Option<[f64; 2]>, // world pos in meters for camera tracking
    pub focused_star_id: Option<(u16, u16, u32)>, // (sector_x, sector_y, sector_index) of focused star
    pub current_procedural_stars: Vec<super::scene::StarRenderData>, // cached for info panel
    pub focused_star_info: Option<super::types::BodyInfoData>, // unified info panel data for focused procedural star
    // Body names for all bodies (real + catalog), set each frame after inject_catalog_planets
    pub body_names: Vec<String>,
    pub num_real_bodies: usize,
    // Info data for catalog planets (keyed by synthetic body index)
    pub catalog_body_info: std::collections::HashMap<usize, super::types::BodyInfoData>,
    // Toast notifications
    pub active_toasts: Vec<(String, web_time::Instant)>,
    // Egui state
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub egui_renderer: egui_wgpu::Renderer,
}

impl RenderState {
    /// Create a new render state from a window
    pub async fn new(window: Arc<Window>, body_names: &[String]) -> Self {
        let size = window.inner_size();

        // Create wgpu instance.
        //
        // On wasm we restrict to the GL (WebGL2) backend. Letting wgpu auto-pick
        // WebGPU on Chrome causes a `requestDevice` rejection because wgpu 0.19's
        // WebGPU limits descriptor includes `maxInterStageShaderComponents`,
        // which current Chromium WebGPU doesn't recognize. Forcing WebGL avoids
        // this version-drift; Phase 7 can re-add WebGPU once we upgrade wgpu.
        #[cfg(target_arch = "wasm32")]
        let backends = wgpu::Backends::GL;
        #[cfg(not(target_arch = "wasm32"))]
        let backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(window.clone()).unwrap();

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        // Request device. On native we use the adapter's actual limits to
        // unlock e.g. >256 texture array layers. On wasm we use WebGL2 down-
        // level defaults — anything more aggressive trips on missing WebGL
        // features and request_device rejects.
        #[cfg(target_arch = "wasm32")]
        let required_limits = wgpu::Limits::downlevel_webgl2_defaults()
            .using_resolution(adapter.limits());
        #[cfg(not(target_arch = "wasm32"))]
        let required_limits = adapter.limits();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Guard against 0-sized initial inner_size on web (canvas not laid out
        // yet). Configuring a surface at 0×0 panics in wgpu's WebGL backend.
        let initial_w = size.width.max(1);
        let initial_h = size.height.max(1);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: initial_w,
            height: initial_h,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        log::info!("Surface configured: {}x{}, format={:?}", initial_w, initial_h, surface_format);

        // Create camera
        let aspect_ratio = size.width as f32 / size.height as f32;
        let camera = Camera::new(aspect_ratio);
        let camera_uniform = camera.to_uniform();

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        // Load body textures
        let t_tex = web_time::Instant::now();
        let (body_texture, body_texture_view, body_sampler, body_texture_map) =
            super::textures::load_body_textures(&device, &queue, body_names);
        log::info!("    Body textures: {:.0?}", t_tex.elapsed());

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let body_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&body_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&body_sampler),
                },
            ],
            label: Some("body_texture_bind_group"),
        });

        // Load sprite atlas
        let t_spr = web_time::Instant::now();
        let sprite_atlas = super::sprites::load_sprite_atlas(&device, &queue);
        println!("    Sprite atlas: {:.0?}", t_spr.elapsed());
        let sprite_bind_group_layout = super::sprites::create_sprite_bind_group_layout(&device);
        let plume_start_time = web_time::Instant::now();

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Create render pipeline
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout, &sprite_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 4, // 4x MSAA
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        // Create MSAA texture
        let msaa_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MSAA Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let msaa_view = msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create buffers for dynamic geometry (bodies + visible star hexagons).
        let max_vertices = 2_000_000;
        let max_indices = 6_000_000;

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: (max_vertices * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            size: (max_indices * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let num_indices = 0;

        // Initialize egui
        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, config.format, None, 1);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            camera_buffer,
            camera_bind_group,
            camera,
            window,
            msaa_texture,
            msaa_view,
            bodies: Vec::new(),
            earth_index: 4, // Updated from game on first frame
            hovered_body: None,
            tracked_body: None,
            ship_orbit_info: None,
            fps: 0.0,
            ship_velocity: 0.0,
            ship_altitude: 0.0,
            ship_vertical_speed: 0.0,
            ship_throttle: 0.0,
            ship_soi_name: String::new(),
            ship_time_to_intercept: None,
            ship_acceleration: 20.0,  // Default max thrust acceleration
            ship_current_true_anomaly: 0.0,
            vessel_total_mass: None,
            vessel_fuel_fraction: None,
            vessel_monoprop_fraction: None,
            vessel_power_generation: None,
            vessel_power_consumption: None,
            vessel_electricity_fraction: None,
            vessel_electricity_stored: None,
            vessel_electricity_max: None,
            vessel_thrust_kn: None,
            vessel_drag_kn: 0.0,
            vessel_delta_v: None,
            vessel_current_stage: None,
            vessel_total_stages: None,
            vessel_stages: Vec::new(),
            vessel_stage_delta_vs: Vec::new(),
            vessel_stage_burn_times: Vec::new(),
            staging_reorder: None,
            ship_soi_surface_gravity: 9.81,
            ship_g_force: 0.0,
            ship_temperature: 300.0,
            ship_heat_fraction: 0.0,
            ship_heat_flux: 0.0,
            thermal_pool_temp: 300.0,
            thermal_pool_gen_w: 0.0,
            thermal_pool_reject_w: 0.0,
            reactors_tripped: false,
            vessel_food_days: None,
            vessel_total_crew: None,
            vessel_is_starving: false,
            reactor_restart_request: false,
            quicksave_requested: false,
            quickload_requested: false,
            ship_below_landing_altitude: false,
            ship_velocity_direction: [0.0, 0.0],
            ship_speed_fraction_c: 0.0,
            ship_lorentz_gamma: 1.0,
            ship_proper_time: 0.0,
            ship_mission_time: 0.0,
            ship_is_relativistic: false,
            ship_grav_time_factor: 1.0,
            selected_flight_part: None,
            flight_parts_cache: Vec::new(),
            ship_render_x: 0.0,
            ship_render_y: 0.0,
            ship_orbits_root: false,
            ship_body_center: [0.0, 0.0],
            ship_rel_offset: [0.0, 0.0],
            ship_render_rotation: 0.0,
            ship_render_scale: 1.0,
            render_requests: Vec::new(),
            ship_has_control: true,
            ship_in_atmosphere: false,
            ship_is_landed: false,
            ap_markers: Vec::new(),
            pe_markers: Vec::new(),
            closest_approach_world_pos: None,
            closest_approach_marker: None,
            target_closest_approach_world_pos: None,
            target_closest_approach_marker: None,
            pending_orbit_click: None,
            selected_maneuver_node: None,
            maneuver_nodes: Vec::new(),
            time_to_node: None,
            burn_time: None,
            warp_to_node: false,
            next_node_id: 1,
            maneuver_node_screen_positions: Vec::new(),
            current_trajectory: Vec::new(),
            predicted_trajectories: Vec::new(),
            dragging_maneuver_node: None,
            background_vessel_screen_positions: Vec::new(),
            tracked_vessel: None,
            autopilot_target: AutopilotTarget::Off,
            selected_target: None,
            selected_target_name: String::new(),
            selected_target_angle: None,
            target_popup: None,
            rcs_enabled: false,
            rcs_disabled_by_rails: false,
            transfer_planner_open: false,
            transfer_planner_mode: 0,
            transfer_selected_target: None,
            porkchop_grid: None,
            porkchop_selected: None,
            porkchop_hovered: None,
            porkchop_last_target: None,
            porkchop_job: None,
            porkchop_computing: false,
            transfer_display: None,
            transfer_hohmann_targets: Vec::new(),
            transfer_interplanetary_targets: Vec::new(),
            show_quicksave_list: false,
            debug_menu_open: false,
            debug_infinite_fuel: false,
            company_money: 0.0,
            science_available: 0.0,
            show_contracts: false,
            can_establish_colony: false,
            has_colonies: false,
            landed_body_index: None,
            vessel_has_cargo: false,
            landed_body_has_colony: false,
            route_creation: Default::default(),
            procedural_star_screen_positions: Vec::new(),
            hovered_star: None,
            focused_star: None,
            focused_star_world_pos: None,
            focused_star_id: None,
            current_procedural_stars: Vec::new(),
            focused_star_info: None,
            body_names: Vec::new(),
            num_real_bodies: 0,
            catalog_body_info: std::collections::HashMap::new(),
            active_toasts: Vec::new(),
            body_texture,
            body_texture_bind_group,
            body_texture_map,
            #[cfg(target_arch = "wasm32")]
            body_texture_fetcher: {
                let mut f = super::body_fetcher::BodyTextureFetcher::new(
                    super::textures::WASM_BODY_TEXTURE_CAPACITY,
                );
                // Eagerly fetch the real solar-system bodies + galaxy backdrop
                // so they appear textured in every game mode at any zoom level.
                // Catalog/procedural planets continue to be lazy-fetched per
                // zoom threshold in render_flight_frame.
                f.prefetch(body_names);
                f.request_fetch("milky_way");
                f
            },
            sprite_atlas,
            plume_start_time,
            egui_ctx,
            egui_state,
            egui_renderer,
        }
    }

    // Note: create_circle and create_ship_triangle are in geometry.rs

    /// Once-per-frame body-texture maintenance for the wasm build: upload any
    /// fetched PNGs that have finished decoding to free GPU layers, then
    /// refresh body_idx → layer mappings so newly arrived textures are picked
    /// up by the renderer regardless of game mode. No-op on native.
    #[cfg(target_arch = "wasm32")]
    pub fn tick_body_textures(&mut self, body_names: &[String]) {
        let Self {
            body_texture_fetcher,
            device,
            queue,
            body_texture,
            body_texture_map,
            ..
        } = self;
        body_texture_fetcher.drain_uploads(device, queue, body_texture, body_texture_map);
        for (body_idx, name) in body_names.iter().enumerate() {
            if !name.is_empty() {
                body_texture_map.register_body_index(body_idx, name);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn tick_body_textures(&mut self, _body_names: &[String]) {}

    /// Handle window resize
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);

            // Recreate MSAA texture with new size
            self.msaa_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("MSAA Texture"),
                size: wgpu::Extent3d {
                    width: new_size.width,
                    height: new_size.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 4,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.msaa_view = self.msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());

            // Update camera aspect ratio
            self.camera.aspect_ratio = new_size.width as f32 / new_size.height as f32;
            self.update_camera_buffer();
        }
    }

    /// Update camera uniform buffer with current camera state
    pub fn update_camera_buffer(&self) {
        let camera_uniform = self.camera.to_uniform();
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );
    }

    /// Handle a winit window event for egui
    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        self.egui_state.on_window_event(&self.window, event).consumed
    }

    /// Convert world position to screen position
    pub fn world_to_screen(&self, world_x: f64, world_y: f64) -> (f32, f32) {
        // Subtract in f64 to preserve precision at large distances, then cast to f32
        let rel_x = (world_x - self.camera.position[0]) as f32;
        let rel_y = (world_y - self.camera.position[1]) as f32;

        // Apply rotation
        let cos_r = self.camera.rotation.cos();
        let sin_r = self.camera.rotation.sin();
        let rotated_x = rel_x * cos_r - rel_y * sin_r;
        let rotated_y = rel_x * sin_r + rel_y * cos_r;

        // Apply zoom and aspect ratio correction
        let view_x = rotated_x * self.camera.zoom;
        let view_y = rotated_y * self.camera.zoom;
        let ndc_x = view_x / self.camera.aspect_ratio;
        let ndc_y = view_y;

        // Convert NDC to screen coordinates
        let screen_x = (ndc_x + 1.0) * 0.5 * self.size.width as f32;
        let screen_y = (1.0 - ndc_y) * 0.5 * self.size.height as f32;

        (screen_x, screen_y)
    }

    /// Get window reference
    pub fn window(&self) -> &Window {
        &self.window
    }

}
