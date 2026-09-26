//! Sunscatter v0.2 prototype: Earth–Moon flight on the sim crate.
//!
//! Frame order (explicit, lesson from v0.1's one-frame-stale bugs):
//! input → simulation → camera → scene transforms → trajectory → UI.
//! Everything drawn is derived from the simulation at the *current* clock.

mod atmosphere;
mod bench;
mod body_visual;
mod camera;
mod demo;
mod hud;
mod map;
mod scene;
mod settings;
mod settings_ui;
mod state;
mod terrain;
mod trajectory;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum Stage {
    Input,
    Simulate,
    Camera,
    Scene,
}

fn main() {
    let mut app = App::new();
    if let Some(demo) = demo::Demo::from_env() {
        app.insert_resource(demo);
    }
    app.insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Sunscatter v0.2 — Earth–Moon prototype".into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .add_plugins(terrain::TerrainPlugin)
        .insert_resource(state::SimState::new())
        .init_resource::<state::Prediction>()
        .init_resource::<camera::CameraRig>()
        .init_resource::<hud::UiState>()
        .init_resource::<settings::GraphicsSettings>()
        .init_resource::<settings_ui::SettingsUi>()
        .init_resource::<bench::Bench>()
        .init_resource::<map::MapMode>()
        .configure_sets(Update, (Stage::Input, Stage::Simulate, Stage::Camera, Stage::Scene).chain())
        .add_systems(Startup, (scene::setup, terrain::setup, camera::setup).chain())
        .add_systems(
            Update,
            (
                state::read_controls,
                settings_ui::toggle,
                demo::run,
                bench::run,
                settings::apply,
                atmosphere::apply_settings,
                camera::read_input,
                hud::pick_bodies,
            )
                .chain()
                .in_set(Stage::Input),
        )
        .add_systems(Update, (state::advance, state::update_prediction).chain().in_set(Stage::Simulate))
        .add_systems(Update, (camera::update, map::update_mode).chain().in_set(Stage::Camera))
        .add_systems(
            Update,
            (
                scene::update_bodies,
                atmosphere::update,
                scene::update_ships,
                terrain::update,
                terrain::update_textures,
                trajectory::draw,
                map::draw_body_orbits,
            )
                .in_set(Stage::Scene),
        )
        .add_systems(EguiPrimaryContextPass, (map::draw_overlay, hud::draw, settings_ui::draw).chain())
        .run();
}
