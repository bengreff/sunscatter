//! Installs our copy of Bevy's sky compositing shader (`render_sky.wgsl`,
//! see its header) over the embedded original, and again if Bevy (re)loads
//! the original.

use bevy::prelude::*;
use bevy::shader::{Shader, Source};

const PATH: &str = "embedded://bevy_pbr/atmosphere/render_sky.wgsl";
const SOURCE: &str = include_str!("render_sky.wgsl");
const MARKER: &str = "sunscatter sky override";

/// Keeps our sky shader installed.
pub fn install(
    assets: Res<AssetServer>,
    mut shaders: ResMut<Assets<Shader>>,
    mut handle: Local<Option<Handle<Shader>>>,
) {
    let handle = handle.get_or_insert_with(|| assets.load(PATH));
    let Some(current) = shaders.get(&*handle) else { return };
    if matches!(&current.source, Source::Wgsl(s) if s.contains(MARKER)) {
        return;
    }
    let mut shader = Shader::from_wgsl(SOURCE, PATH);
    shader.shader_defs.clone_from(&current.shader_defs);
    let _ = shaders.insert(&*handle, shader);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_shader_carries_the_marker_and_no_placeholders() {
        assert!(SOURCE.contains(MARKER));
        assert!(!SOURCE.contains("{{"));
    }
}
