//! Starfield: the Yale Bright Star Catalogue (J2000 equatorial, which is
//! our inertial frame) drawn as pixel-sized billboards with colours from B−V.

use crate::camera::CameraRig;
use crate::scene::BodyDefs;
use crate::settings::GraphicsSettings;
use crate::state::SimState;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use serde::Deserialize;

const SHADER: &str = "embedded://game/sky/stars.wgsl";

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct StarParams {
    pub intensity: f32,
    pub px_scale: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct StarMaterial {
    #[uniform(0)]
    pub params: StarParams,
}

impl Material for StarMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Add
    }
}

#[derive(Deserialize)]
struct Catalogue {
    stars: Vec<Star>,
}

#[derive(Clone, Copy, Deserialize)]
struct Star {
    ra: f64,
    dec: f64,
    vmag: f32,
    bv: f32,
}

#[derive(Resource)]
pub struct Starfield {
    stars: Vec<Star>,
    entity: Option<Entity>,
    material: Handle<StarMaterial>,
    /// Magnitude limit of the current mesh.
    built_for: f32,
}

/// Approximate RGB of a star from its B−V colour index (Ballesteros 2012
/// temperature, then a blackbody-like colour), normalised to max 1.
pub fn bv_to_rgb(bv: f32) -> [f32; 3] {
    let bv = bv.clamp(-0.4, 2.0);
    let t = 4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62));
    let t = t / 100.0;
    let r = if t <= 66.0 { 1.0 } else { (1.292_936 * (t - 60.0).powf(-0.133_204_76)).clamp(0.0, 1.0) };
    let g = if t <= 66.0 {
        (0.390_081_58 * t.ln() - 0.631_841_4).clamp(0.0, 1.0)
    } else {
        (1.129_890_9 * (t - 60.0).powf(-0.075_514_85)).clamp(0.0, 1.0)
    };
    let b = if t >= 66.0 {
        1.0
    } else if t <= 19.0 {
        0.0
    } else {
        (0.543_206_8 * (t - 10.0).ln() - 1.196_254_1).clamp(0.0, 1.0)
    };
    let m = r.max(g).max(b);
    [r / m, g / m, b / m]
}

pub fn setup(mut commands: Commands, mut materials: ResMut<Assets<StarMaterial>>) {
    let path = format!("{}/../../data/stars/bsc5.ron", env!("CARGO_MANIFEST_DIR"));
    let stars = match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| ron::from_str::<Catalogue>(&t).map_err(|e| e.to_string()))
    {
        Ok(c) => c.stars,
        Err(e) => {
            error!("{path}: {e}");
            Vec::new()
        }
    };
    let material = materials.add(StarMaterial { params: StarParams { intensity: 1.0, px_scale: 1.0, ..default() } });
    commands.insert_resource(Starfield { stars, entity: None, material, built_for: -1.0 });
}

fn build_mesh(stars: &[Star], limit: f32) -> Mesh {
    let chosen: Vec<&Star> = stars.iter().filter(|s| s.vmag <= limit).collect();
    let mut pos = Vec::with_capacity(chosen.len() * 4);
    let mut uv = Vec::with_capacity(chosen.len() * 4);
    let mut col = Vec::with_capacity(chosen.len() * 4);
    let mut idx = Vec::with_capacity(chosen.len() * 6);
    for s in chosen {
        let (cd, sd) = (s.dec.cos(), s.dec.sin());
        let dir = [(cd * s.ra.cos()) as f32, (cd * s.ra.sin()) as f32, sd as f32];
        // Relative flux against a magnitude-1 star, compressed so that the
        // faint end stays visible (the eye and displays are not linear).
        let flux = 10f32.powf(-0.4 * (s.vmag - 1.0));
        let brightness = flux.powf(0.5).min(2.0) * 0.9;
        let radius = 1.1 + 1.4 * flux.min(4.0).sqrt();
        let [r, g, b] = bv_to_rgb(s.bv);
        let base = pos.len() as u32;
        for corner in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
            pos.push(dir);
            uv.push(corner);
            col.push([r * brightness, g * brightness, b * brightness, radius]);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

/// Rebuilds the star mesh when the magnitude limit changes, and fades the
/// stars in daylight (inside an atmosphere with the Sun up).
#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    settings: Res<GraphicsSettings>,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    mut field: ResMut<Starfield>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StarMaterial>>,
) {
    let limit = settings.star_magnitude;
    if limit != field.built_for {
        field.built_for = limit;
        if let Some(e) = field.entity.take() {
            commands.entity(e).despawn();
        }
        if limit > 0.0 {
            let mesh = meshes.add(build_mesh(&field.stars, limit));
            let e = commands
                .spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(field.material.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    NotShadowCaster,
                ))
                .id();
            field.entity = Some(e);
        }
    }
    let day = daylight(&sim, &rig, &defs);
    if let Some(mut m) = materials.get_mut(&field.material) {
        m.params.intensity = 1.0 - 0.98 * day;
    }
}

/// How much daylight sky surrounds the camera (0 = space or night, 1 =
/// full day at the surface).
pub(super) fn daylight(sim: &SimState, rig: &CameraRig, defs: &BodyDefs) -> f32 {
    let Some((sun, _)) = defs.light_source else { return 0.0 };
    let snap = sim.world.snapshot(sim.clock);
    let sun_pos = snap.relative_r(sun, rig.anchor) - rig.cam_pos;
    defs.defs
        .iter()
        .filter_map(|(node, d)| {
            let a = d.atmosphere.as_ref()?;
            let p = sim.world.source(*node)?.physical.as_ref()?;
            let to_cam = rig.cam_pos - snap.relative_r(*node, rig.anchor);
            let alt = to_cam.length() - p.radius_eq;
            let thickness = (a.rayleigh_scale_height_m as f64).max(1.0);
            let air = (-alt.max(0.0) / thickness).exp();
            let sin_elev = to_cam.normalize().dot(sun_pos.normalize());
            let sun_up = ((sin_elev + 0.1) / 0.15).clamp(0.0, 1.0);
            Some((air.min(1.0) * sun_up) as f32)
        })
        .fold(0.0, f32::max)
}
