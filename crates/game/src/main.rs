//! Sunscatter v0.2 prototype: Earth–Moon flight on the sim crate.
//!
//! Frame order (explicit, lesson from v0.1's one-frame-stale bugs):
//! input → simulation → camera → scene transforms → trajectory → UI.
//! Everything drawn is derived from the simulation at the *current* clock.

mod agent;
mod atmosphere;
mod bench;
mod body_visual;
mod camera;
mod clouds;
mod commands;
mod comms;
mod debris;
mod fleet;
mod format;
mod hud;
mod interface;
mod landing;
mod lighting;
mod look;
mod map;
mod map_view;
mod navball;
mod offscreen;
mod pad;
mod persist;
mod planner;
mod profile;
mod relations;
mod render_scale;
mod rendezvous;
mod rings;
mod saves;
mod scene;
mod settings_ui;
#[cfg(test)]
mod shader_tests;
mod ship_light;
mod sky;
mod staging;
mod state;
mod systems_screen;
mod terrain;
#[cfg(test)]
mod test_rocket;
mod tracking;
mod trajectory;

use bevy::prelude::*;
use bevy::window::{ExitCondition, PresentMode};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum Stage {
    Input,
    Simulate,
    Camera,
    Scene,
}

/// Bevy's default plugins with the window and the thread pools
/// ([`fleet::task_pools`]).
fn default_plugins(measuring: bool, offscreen: bool) -> bevy::app::PluginGroupBuilder {
    DefaultPlugins
        .set(WindowPlugin {
            primary_window: Some(Window {
                title: "Sunscatter v0.2 — Earth–Moon prototype".into(),
                // A headless or developer's game measures frame times:
                // don't wait for the display.
                present_mode: if measuring { PresentMode::AutoNoVsync } else { PresentMode::AutoVsync },
                visible: !offscreen,
                ..default()
            }),
            // A headless game lives without its hidden window: closing the
            // Mac's lid removed the monitor, the window went with it and the
            // game exited mid-flight (2026-10-30). It ends when killed.
            exit_condition: if offscreen { ExitCondition::DontExit } else { ExitCondition::OnAllClosed },
            close_when_requested: !offscreen,
            ..default()
        })
        .set(TaskPoolPlugin { task_pool_options: fleet::task_pools() })
        .set(bevy::log::LogPlugin { filter: profile::log_filter(), custom_layer: profile::layer, ..default() })
}

fn main() {
    let mut app = App::new();
    // A headless game renders into an image: don't show an empty window.
    let offscreen = offscreen::enabled();
    let measuring = offscreen || agent::session::developer();
    // The interface belongs to the main camera (`camera::setup` gives it the
    // primary egui context). Left automatic, bevy_egui took the first camera
    // spawned, and the systems screen's own camera sometimes came first,
    // which hid every menu, the navball and the HUD (2026-10-27).
    app.insert_resource(bevy_egui::EguiGlobalSettings { auto_create_primary_context: false, ..default() });
    app.insert_resource(ClearColor(Color::BLACK))
        .add_plugins(default_plugins(measuring, offscreen))
        .add_plugins(EguiPlugin::default())
        .add_plugins(offscreen::OffscreenPlugin)
        .add_plugins(body_plugins())
        .add_plugins(GameState { craft: startup_craft() })
        // After the settings defaults: replaces them with the saved settings.
        .add_plugins(persist::PersistPlugin)
        .add_plugins(interface_plugins())
        .configure_sets(Update, (Stage::Input, Stage::Simulate, Stage::Camera, Stage::Scene).chain())
        .add_systems(Startup, (scene::setup, terrain::setup, sky::setup_stars, camera::setup).chain())
        .add_systems(
            Update,
            (
                commands::update_context,
                interface::pause::keys,
                interface::help::keys,
                commands::keys::read_controls,
                hud::measure_fps,
                settings_ui::record_frames,
                bench::run,
                look::apply,
                atmosphere::configure_camera,
                sky::install_haze,
                camera::read_input,
                hud::pick_bodies,
            )
                .chain()
                .in_set(Stage::Input),
        )
        .add_systems(
            Update,
            (saves::keys, tracking::update, agent::serve, commands::apply, saves::record_launch)
                .chain()
                .after(hud::pick_bodies)
                .in_set(Stage::Input),
        )
        .add_systems(
            Update,
            (
                state::advance,
                debris::update,
                pad::update,
                agent::session::run,
                state::update_prediction,
                landing::update,
            )
                .chain()
                .in_set(Stage::Simulate),
        )
        .add_systems(
            Update,
            (camera::update, comms::update, rendezvous::update, map::update).chain().in_set(Stage::Camera),
        )
        .add_systems(
            Update,
            (lighting::update_scene, lighting::update_lights).chain().after(Stage::Camera).before(Stage::Scene),
        )
        .add_systems(
            Update,
            (
                scene::update_bodies,
                atmosphere::update,
                atmosphere::update_mode,
                scene::ships::update_ships,
                (scene::ships::update_parts, scene::update_canopies).after(scene::ships::update_ships),
                (terrain::update, lighting::update_terrain, terrain::upload_frames).chain(),
                lighting::update_exposure,
                terrain::update_textures,
                sky::update_stars,
                sky::update_ambient,
                trajectory::draw,
                map::draw_body_orbits,
            )
                .in_set(Stage::Scene),
        )
        .add_systems(Update, trajectory::apsides::update.after(state::update_prediction).in_set(Stage::Simulate))
        .add_systems(
            EguiPrimaryContextPass,
            (
                interface::apply_style,
                sky::draw_flare,
                map::draw_overlay,
                rendezvous::draw,
                landing::draw,
                hud::draw,
                settings_ui::draw,
            )
                .chain(),
        )
        .add_systems(
            EguiPrimaryContextPass,
            (
                tracking::draw,
                hud::draw_body_menu,
                saves::draw,
                planner::draw,
                planner::pick_on_line,
                interface::pause::draw,
                interface::help::draw,
                interface::toasts::draw,
            )
                .chain()
                .after(hud::draw),
        );
    app.run();
}

/// The bodies' and ships' rendering: lighting, terrain, sky, atmospheres,
/// rings, cloud decks, ships' light.
fn body_plugins() -> (
    lighting::LightingPlugin,
    terrain::TerrainPlugin,
    sky::SkyPlugin,
    atmosphere::AtmosphereFixPlugin,
    rings::RingsPlugin,
    clouds::CloudsPlugin,
    ship_light::ShipLightPlugin,
    render_scale::RenderScalePlugin,
    scene::exhaust::ExhaustPlugin,
) {
    (
        lighting::LightingPlugin,
        terrain::TerrainPlugin,
        sky::SkyPlugin,
        atmosphere::AtmosphereFixPlugin,
        rings::RingsPlugin,
        clouds::CloudsPlugin,
        ship_light::ShipLightPlugin,
        render_scale::RenderScalePlugin,
        scene::exhaust::ExhaustPlugin,
    )
}

/// The flight view's instruments and screens: the navball cluster, the
/// task menus, the profiler, the systems screen, the action-group bar,
/// the staging bar and the part cards.
fn interface_plugins() -> (
    navball::NavballPlugin,
    interface::menus::view::MenusPlugin,
    profile::ProfilePlugin,
    systems_screen::SystemsPlugin,
    interface::action_bar::ActionBarPlugin,
    interface::stage_bar::StageBarPlugin,
    interface::part_card::PartCardPlugin,
) {
    (
        navball::NavballPlugin,
        interface::menus::view::MenusPlugin,
        profile::ProfilePlugin,
        systems_screen::SystemsPlugin,
        interface::action_bar::ActionBarPlugin,
        interface::stage_bar::StageBarPlugin,
        interface::part_card::PartCardPlugin,
    )
}

/// The craft the game starts with: `SUNSCATTER_CRAFT`, else the first
/// choice whose files exist.
fn startup_craft() -> String {
    let available: Vec<&str> = state::craft_choices(state::craft_exists).iter().map(|c| c.0).collect();
    state::startup_craft(std::env::var("SUNSCATTER_CRAFT").ok().as_deref(), &available)
}

/// The game's resources and messages (before the saved settings replace
/// the settings defaults), with `craft` on the pad.
struct GameState {
    craft: String,
}

impl Plugin for GameState {
    fn build(&self, app: &mut App) {
        app.insert_resource(state::SimState::with_craft(&state::craft_or_fallback(&self.craft)))
            .init_resource::<state::Prediction>()
            .init_resource::<landing::LandingPrediction>()
            .init_resource::<landing::ImpactMarker>()
            .init_resource::<state::SimPause>()
            .init_resource::<camera::CameraRig>()
            .init_resource::<hud::UiState>()
            .init_resource::<hud::FpsMeter>()
            .init_resource::<interface::pause::PauseMenu>()
            .init_resource::<interface::help::Help>()
            .init_resource::<interface::toasts::Toasts>()
            .init_resource::<debris::Breakups>()
            .init_resource::<scene::ShipModels>()
            .init_resource::<look::Parts>()
            .init_resource::<settings_ui::SettingsUi>()
            .init_resource::<bench::Bench>()
            .init_resource::<map::MapView>()
            .init_resource::<trajectory::apsides::Apsides>()
            .init_resource::<trajectory::lines::Lines>()
            .init_resource::<saves::SaveUi>()
            .init_resource::<tracking::Tracked>()
            .init_resource::<tracking::TrackingStation>()
            .init_resource::<commands::InputContext>()
            .insert_resource(comms::Comms::load())
            .add_message::<commands::GameCommand>()
            .init_resource::<commands::InFlight>()
            .init_resource::<planner::Planner>()
            .init_resource::<rendezvous::Rendezvous>()
            .init_resource::<agent::session::Session>()
            .init_resource::<agent::session::StopAt>()
            .init_resource::<agent::HeldRotation>();
    }
}
