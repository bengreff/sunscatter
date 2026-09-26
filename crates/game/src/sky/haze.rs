//! Haze: installs our copy of Bevy's sky compositing shader
//! (`render_sky.wgsl`, see its header) over the embedded original, with the
//! haze strength setting baked in as a constant. Re-installed when the
//! setting changes or if Bevy (re)loads the original.

use crate::settings::GraphicsSettings;
use bevy::prelude::*;
use bevy::shader::{Shader, Source};

const PATH: &str = "embedded://bevy_pbr/atmosphere/render_sky.wgsl";
const SOURCE: &str = include_str!("render_sky.wgsl");
const MARKER: &str = "sunscatter haze override";

/// The shader source for a haze strength (1 = physical).
pub fn source(haze: f32) -> String {
    SOURCE.replace("{{HAZE}}", &format!("{:.4}", haze.clamp(0.0, 4.0)))
}

/// Keeps our sky shader installed with the current haze strength.
pub fn install(
    settings: Res<GraphicsSettings>,
    assets: Res<AssetServer>,
    mut shaders: ResMut<Assets<Shader>>,
    mut state: Local<Option<(Handle<Shader>, f32)>>,
) {
    let (handle, applied) = state.get_or_insert_with(|| (assets.load(PATH), f32::NAN));
    let Some(current) = shaders.get(&*handle) else { return };
    let ours = matches!(&current.source, Source::Wgsl(s) if s.contains(MARKER));
    if ours && *applied == settings.haze {
        return;
    }
    let mut shader = Shader::from_wgsl(source(settings.haze), PATH);
    shader.shader_defs.clone_from(&current.shader_defs);
    let _ = shaders.insert(&*handle, shader);
    *applied = settings.haze;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_bakes_the_strength_and_keeps_the_marker() {
        let s = source(0.5);
        assert!(s.contains("const HAZE: f32 = 0.5000;"));
        assert!(s.contains(MARKER));
        assert!(!s.contains("{{"));
        assert!(source(9.0).contains("const HAZE: f32 = 4.0000;"));
    }
}
