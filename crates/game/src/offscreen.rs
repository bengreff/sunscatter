//! The headless game (D122): `SUNSCATTER_OFFSCREEN=1` renders the main
//! camera, the interface with it, into an image instead of the window, and
//! the window is never shown. Screenshots work with the screen locked; an
//! agent drives the game through the MCP server (`agent`, D044) and takes
//! its shots of the image (`agent::session`).
//!
//! In a headless run a shader that fails to compile ends the game (exit 3:
//! a reserved word once silently removed all terrain), and the interface
//! must sit on the main camera (bevy_egui once took the systems screen's
//! camera and every menu vanished, 2026-10-27).

use bevy::prelude::*;

/// The variable that makes the game headless.
pub const VAR: &str = "SUNSCATTER_OFFSCREEN";

/// The image's size until a shot asks for another (px).
pub const DEFAULT_SIZE: (u32, u32) = (1600, 900);

/// Whether the game runs headless (`SUNSCATTER_OFFSCREEN` set, not `0`).
pub fn enabled() -> bool {
    std::env::var(VAR).is_ok_and(|v| !v.is_empty() && v != "0")
}

/// The view's render target and the interface scale of a shot.
#[derive(Resource, Default)]
pub struct Offscreen {
    /// The image the main camera draws into (headless only).
    pub image: Option<Handle<Image>>,
    /// Its size (px).
    pub size: (u32, u32),
    /// The interface scale a shot asked for, over the player's setting.
    pub ui_scale: Option<f32>,
}

impl Offscreen {
    /// The interface's scale for the image's pixels: the reference Mac's
    /// (2) for a Retina-sized image, else 1 ([`native_scale`]); 1 in a
    /// window.
    pub fn native_scale(&self) -> f32 {
        if self.image.is_some() {
            native_scale(self.size)
        } else {
            1.0
        }
    }
}

/// The interface's scale in an image of `size`: the reference Mac's (2)
/// for a Retina-sized image (2400 px wide or more, its full screen
/// 3024×1964), else 1 (the image's pixels are the interface's).
pub fn native_scale(size: (u32, u32)) -> f32 {
    if size.0 >= 2400 {
        2.0
    } else {
        1.0
    }
}

/// The main camera's target in a headless run: a new image of
/// [`DEFAULT_SIZE`] (called by `camera::setup`).
pub fn target(offscreen: &mut Offscreen, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    if !enabled() {
        return None;
    }
    use bevy::render::render_resource::TextureFormat;
    let (w, h) = DEFAULT_SIZE;
    let handle = images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None));
    (offscreen.image, offscreen.size) = (Some(handle.clone()), DEFAULT_SIZE);
    Some(handle)
}

/// Resizes the headless image (a shot's size); false in a window.
pub fn resize(offscreen: &mut Offscreen, images: &mut Assets<Image>, (w, h): (u32, u32)) -> bool {
    let Some(handle) = offscreen.image.clone() else { return false };
    if let Some(mut image) = images.get_mut(&handle) {
        if (image.width(), image.height()) != (w, h) {
            image.resize(bevy::render::render_resource::Extent3d { width: w, height: h, depth_or_array_layers: 1 });
        }
        offscreen.size = (w, h);
    }
    true
}

/// The headless run's checks.
pub struct OffscreenPlugin;

impl Plugin for OffscreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Offscreen>();
        if !enabled() {
            return;
        }
        app.add_systems(Update, check_interface_camera);
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app.add_systems(bevy::render::Render, fail_on_shader_errors);
        }
    }
}

/// The interface is drawn over the main camera's view: the one primary
/// egui context sits on it.
fn check_interface_camera(primary: Query<Has<crate::camera::MainCamera>, With<bevy_egui::PrimaryEguiContext>>) {
    let on: Vec<bool> = primary.iter().collect();
    assert!(on == [true], "the interface must be drawn over the main camera (primary egui contexts: {on:?})");
}

fn fail_on_shader_errors(cache: Res<bevy::render::render_resource::PipelineCache>) {
    use bevy::render::render_resource::CachedPipelineState;
    use bevy::shader::ShaderCacheError;
    for p in cache.pipelines() {
        if let CachedPipelineState::Err(
            e @ (ShaderCacheError::ProcessShaderError(_) | ShaderCacheError::CreateShaderModule(_)),
        ) = &p.state
        {
            error!("headless: a shader failed to compile: {e}");
            std::process::exit(3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_retina_sized_image_draws_the_interface_at_scale_2() {
        // (size, scale)
        let table = [((1600, 900), 1.0), ((1920, 1080), 1.0), ((3024, 1964), 2.0), ((2560, 1600), 2.0)];
        for (size, want) in table {
            assert_eq!(native_scale(size), want, "{size:?}");
        }
        assert_eq!(Offscreen { size: (3024, 1964), ..default() }.native_scale(), 1.0, "a window: its own scale");
    }
}
