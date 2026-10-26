//! Opaque cloud decks seen from above (Venus; docs/features/solar-visuals.md#venus):
//! a Lambert sphere at the deck's top (`CloudTopDef`), lit by the star's
//! flux at the body. From above it hides the ground, and the sky is drawn
//! with only the gas above it (`atmosphere::DeckMedia`): raymarching 30
//! optical depths of cloud from outside in 24 even steps drew concentric
//! ripples at the limb, where each step crossed the cloud top. Below the
//! top the shell is hidden and the full atmosphere is drawn.

use crate::body_visual::CloudTopDef;
use crate::camera::CameraRig;
use crate::scene::BodyDefs;
use crate::state::SimState;
use bevy::asset::embedded_asset;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use glam::DVec3;
use sim::frame::NodeId;

const SHADER: &str = "embedded://game/clouds.wgsl";

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct DeckParams {
    /// xyz: the body's centre relative to the camera.
    pub center: Vec4,
    /// xyz: the star relative to the camera; w: its illuminance at the body (lux).
    pub star: Vec4,
    /// rgb: the deck's Lambert albedo.
    pub albedo: Vec4,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct DeckMaterial {
    #[uniform(0)]
    pub params: DeckParams,
}

impl Material for DeckMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub struct CloudsPlugin;

impl Plugin for CloudsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "clouds.wgsl");
        app.add_plugins(MaterialPlugin::<DeckMaterial>::default())
            .add_systems(Startup, setup.after(crate::terrain::setup))
            .add_systems(Update, update.after(crate::scene::update_bodies));
    }
}

/// A cloud deck's shell: its body and the deck's top.
#[derive(Component)]
pub struct Deck(pub NodeId, pub CloudTopDef);

/// Spawns a shell for every body whose atmosphere has a cloud top.
pub fn setup(
    mut commands: Commands,
    defs: Res<BodyDefs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DeckMaterial>>,
) {
    for (node, def) in &defs.defs {
        let Some(top) = def.atmosphere.as_ref().and_then(|a| a.cloud_top) else { continue };
        let albedo = Vec3::from_array(top.albedo).extend(0.0);
        let material = materials.add(DeckMaterial { params: DeckParams { albedo, ..default() } });
        // 512 x 256 facets: the silhouette is within half a pixel from low orbit.
        let mesh = meshes.add(Sphere::new(1.0).mesh().uv(512, 256));
        commands.spawn((
            Deck(*node, top),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Visibility::Hidden,
            NotShadowCaster,
            NoFrustumCulling,
        ));
    }
}

/// Whether a deck's shell is drawn: only from above its top (`altitude`,
/// the camera's height above the mean radius).
pub fn shell_drawn(altitude: f64, top: &CloudTopDef) -> bool {
    altitude > f64::from(top.altitude_m)
}

/// Places each shell camera-relative and lights it.
pub fn update(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    mut decks: Query<(&Deck, &MeshMaterial3d<DeckMaterial>, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<DeckMaterial>>,
) {
    let Some((star_node, lm)) = defs.light_source else { return };
    let snap = sim.world.snapshot(sim.clock);
    let star = snap.relative_r(star_node, rig.anchor) - rig.cam_pos;
    for (deck, mat, mut t, mut vis) in &mut decks {
        let Some(p) = sim.world.source(deck.0).and_then(|s| s.physical.as_ref()) else { continue };
        let centre: DVec3 = snap.relative_r(deck.0, rig.anchor) - rig.cam_pos;
        let drawn = shell_drawn(centre.length() - p.radius_eq, &deck.1);
        vis.set_if_neq(if drawn { Visibility::Visible } else { Visibility::Hidden });
        if !drawn {
            continue;
        }
        *t = Transform {
            translation: centre.as_vec3(),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat((p.radius_eq + f64::from(deck.1.altitude_m)) as f32),
        };
        let Some(mut m) = materials.get_mut(&mat.0) else { continue };
        let e = crate::lighting::flux(lm, (star - centre).length().max(1.0));
        m.params.center = centre.as_vec3().extend(0.0);
        m.params.star = star.as_vec3().extend(e as f32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shell_is_drawn_only_from_above_the_deck() {
        let top = CloudTopDef { altitude_m: 74_000.0, albedo: [1.0; 3] };
        // (case, camera height above the mean radius, drawn)
        let cases = [
            ("from orbit", 4.0e5, true),
            ("just above the top", 74_001.0, true),
            ("in the deck", 60_000.0, false),
            ("on the ground", 2_000.0, false),
        ];
        for (case, h, want) in cases {
            assert_eq!(shell_drawn(h, &top), want, "{case}");
        }
    }

    #[test]
    fn venus_has_a_sourced_deck() {
        let a = crate::body_visual::load("Venus").and_then(|d| d.atmosphere).expect("Venus's atmosphere");
        let top = a.cloud_top.expect("Venus's cloud top");
        assert!((70_000.0..80_000.0).contains(&top.altitude_m));
        // Pale yellowish white: red brightest, blue a few per cent darker.
        let [r, g, b] = top.albedo;
        assert!(r <= 1.0 && r > g && g > b && b > 0.85 * r, "{:?}", top.albedo);
    }
}
