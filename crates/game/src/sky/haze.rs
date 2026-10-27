//! Installs our copies of Bevy's atmosphere shaders over the embedded
//! originals, and again if Bevy (re)loads an original: the sky compositing
//! (`render_sky.wgsl`) and the aerial-view table (`aerial_view_lut.wgsl`,
//! which from outside the atmosphere holds the table from space the
//! compositing reads; see their headers).

use bevy::prelude::*;
use bevy::shader::{Shader, Source};

/// (Bevy's embedded path, our source, the marker that tells ours apart).
const OVERRIDES: [(&str, &str, &str); 2] = [
    ("embedded://bevy_pbr/atmosphere/render_sky.wgsl", include_str!("render_sky.wgsl"), "sunscatter sky override"),
    (
        "embedded://bevy_pbr/atmosphere/aerial_view_lut.wgsl",
        include_str!("aerial_view_lut.wgsl"),
        "sunscatter aerial view override",
    ),
];

/// Keeps our atmosphere shaders installed.
pub fn install(assets: Res<AssetServer>, mut shaders: ResMut<Assets<Shader>>, mut handles: Local<Vec<Handle<Shader>>>) {
    if handles.is_empty() {
        *handles = OVERRIDES.iter().map(|(path, _, _)| assets.load(*path)).collect();
    }
    for (handle, (path, source, marker)) in handles.iter().zip(OVERRIDES) {
        let Some(current) = shaders.get(handle) else { continue };
        if matches!(&current.source, Source::Wgsl(s) if s.contains(marker)) {
            continue;
        }
        let mut shader = Shader::from_wgsl(source, path);
        shader.shader_defs.clone_from(&current.shader_defs);
        let _ = shaders.insert(handle, shader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_shaders_carry_their_markers_and_no_placeholders() {
        for (path, source, marker) in OVERRIDES {
            assert!(source.contains(marker), "{path}");
            assert!(!source.contains("{{"), "{path}");
        }
    }
}
