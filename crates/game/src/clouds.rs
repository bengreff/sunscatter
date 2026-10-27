//! Opaque cloud decks seen from above (Venus; docs/features/solar-visuals.md#venus):
//! a Lambert sphere at the deck's top (`CloudTopDef`), lit by the star's
//! flux at the body. From above it hides the ground, and the sky is drawn
//! with only the gas above it (`atmosphere::DeckMedia`): raymarching 30
//! optical depths of cloud from outside in 24 even steps drew concentric
//! ripples at the limb, where each step crossed the cloud top. Below the
//! top the shell is hidden and the full atmosphere is drawn.

use crate::body_visual::{AtmosphereDef, CloudTopDef};
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
    /// rgb: the deck's Lambert albedo.
    pub albedo: Vec4,
    /// The body's light (`lighting::LightUniform`): every drawn star, the
    /// bodies that can eclipse them, planetshine.
    pub starlight: crate::lighting::LightUniform,
    /// Seen from below the deck's top: rgb the sky's radiance there
    /// (lux/sr, `lighting::haze::under_deck_radiance`), w 1.
    pub inside: Vec4,
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

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Both faces: from below the deck the shell's inside is the sky.
        descriptor.primitive.cull_mode = None;
        Ok(())
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

/// A cloud deck's shell: its body, the deck's top, and the atmosphere
/// (the gas below the deck fogs the ground).
#[derive(Component)]
pub struct Deck(pub NodeId, pub CloudTopDef, pub AtmosphereDef);

/// Spawns a shell for every body whose atmosphere has a cloud top.
pub fn setup(
    mut commands: Commands,
    defs: Res<BodyDefs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<DeckMaterial>>,
) {
    for (node, def) in &defs.defs {
        let Some(atmosphere) = def.atmosphere.as_ref() else { continue };
        let Some(top) = atmosphere.cloud_top else { continue };
        let albedo = Vec3::from_array(top.albedo).extend(0.0);
        let material = materials.add(DeckMaterial { params: DeckParams { albedo, ..default() } });
        // 512 x 256 facets: the silhouette is within half a pixel from low orbit.
        let mesh = meshes.add(Sphere::new(1.0).mesh().uv(512, 256));
        commands.spawn((
            Deck(*node, top, atmosphere.clone()),
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

/// Places each shell camera-relative and lights it. From above its top
/// the shell is the deck (a Lambert sphere); from below, it is the sky:
/// the diffuse light at the camera's height, which also fogs the body's
/// ground through the gas below the deck (Bevy's atmosphere, without
/// multiple scattering, drew a black sky there: `atmosphere::update`
/// leaves it out).
#[allow(clippy::too_many_arguments)]
pub fn update(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    scene: Res<crate::lighting::LightScene>,
    parts: Res<crate::look::Parts>,
    terrain: Res<crate::terrain::Terrain>,
    mut decks: Query<(&Deck, &MeshMaterial3d<DeckMaterial>, &mut Transform, &mut Visibility)>,
    mut materials: ResMut<Assets<DeckMaterial>>,
    mut terrain_materials: ResMut<Assets<crate::terrain::TerrainMaterial>>,
) {
    let snap = sim.world.snapshot(sim.clock);
    for (deck, mat, mut t, mut vis) in &mut decks {
        let Some(p) = sim.world.source(deck.0).and_then(|s| s.physical.as_ref()) else { continue };
        let centre: DVec3 = snap.relative_r(deck.0, rig.anchor) - rig.cam_pos;
        let altitude = centre.length() - p.radius_eq;
        let below = !shell_drawn(altitude, &deck.1);
        vis.set_if_neq(Visibility::Visible);
        *t = Transform {
            translation: centre.as_vec3(),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat((p.radius_eq + f64::from(deck.1.altitude_m)) as f32),
        };
        let sky = below.then(|| {
            let up = (-centre).normalize_or(DVec3::Z);
            let haze = crate::lighting::haze_above(&deck.2, p.radius_eq, altitude.max(0.0));
            let lights: Vec<([f64; 3], f64)> = scene
                .stars
                .iter()
                .map(|s| (s.tint.map(|c| c * s.lux_at(centre)), up.dot((s.centre - centre).normalize_or_zero())))
                .collect();
            crate::lighting::haze::under_deck_radiance(haze.diffuse, &lights)
        });
        let inside = sky.map_or(Vec4::ZERO, |l| Vec4::new(l[0] as f32, l[1] as f32, l[2] as f32, 1.0));
        let Some(mut m) = materials.get_mut(&mat.0) else { continue };
        m.params.center = centre.as_vec3().extend(0.0);
        m.params.inside = inside;
        if let Some(light) = scene.body(deck.0) {
            m.params.starlight = crate::lighting::LightUniform::new(&scene, &light, &[], parts.planetshine);
        }
        let ground = terrain.materials().find(|(n, _)| *n == deck.0).and_then(|(_, h)| terrain_materials.get_mut(h));
        if let Some(mut g) = ground {
            let a = &deck.2;
            let sigma = a.rayleigh_scattering;
            g.extension.params.fog = Vec4::new(sigma[0], sigma[1], sigma[2], a.rayleigh_scale_height_m);
            let j = sky.map_or([0.0; 3], |l| crate::lighting::haze::fog_source(l, a.ground_albedo));
            g.extension.params.fog_light = Vec4::new(j[0] as f32, j[1] as f32, j[2] as f32, inside.w);
        }
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
