//! Planetary rings (Saturn's; docs/features/solar-visuals.md#rings): a disc
//! in the body's equatorial plane, drawn from the radial profile of normal
//! optical depth in `visual.ron` (`RingsDef`). Each fragment is a slab of
//! that optical depth: its coverage is 1 − exp(−τ/μ) for the view's μ, and
//! its brightness the single-scattering slab law (isotropic particles of
//! the region's albedo), on the lit or the unlit face; the planet's shadow
//! falls on it. The terrain shader dims sunlight through the rings onto
//! the planet with the same profile (`ring_transmission`).

use crate::body_visual::RingsDef;
use crate::camera::CameraRig;
use crate::scene::BodyDefs;
use crate::state::SimState;
use bevy::asset::{embedded_asset, RenderAssetUsages};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat};
use bevy::shader::ShaderRef;
use glam::DVec3;
use sim::frame::NodeId;

const SHADER: &str = "embedded://game/rings.wgsl";

/// Stored units per unit of optical depth (`asset-tool rings`).
pub const TAU_SCALE: f32 = 10_000.0;

#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct RingParams {
    /// xyz: the body's centre relative to the camera; w: its equatorial radius.
    pub center: Vec4,
    /// xyz: the ring plane's normal (the pole).
    pub normal: Vec4,
    /// x inner, y outer radius (m), z the profile's bins.
    pub radii: Vec4,
    /// The planet's light (`lighting::LightUniform`), its own body first
    /// among the occluders: every drawn star, the planet's and its moons'
    /// shadows.
    pub starlight: crate::lighting::LightUniform,
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct RingMaterial {
    #[uniform(0)]
    pub params: RingParams,
    /// rgb: albedo, a: normal optical depth, per bin, with its mip chain
    /// ([`mip_chain`]): sampled over each pixel's radial footprint.
    #[texture(1)]
    #[sampler(2)]
    pub profile: Handle<Image>,
}

impl Material for RingMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Both faces: the unlit face shows the light the rings let through.
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub struct RingsPlugin;

impl Plugin for RingsPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "rings.wgsl");
        app.add_plugins(MaterialPlugin::<RingMaterial>::default())
            .add_systems(Startup, setup.after(crate::terrain::setup))
            .add_systems(Update, update.after(crate::scene::update_bodies));
    }
}

/// A ring entity's body.
#[derive(Component)]
pub struct Rings(pub NodeId);

/// Per-bin albedo and optical depth from the stored profile: bin `i` of
/// `n` is centred at inner + (i + ½)(outer − inner)/n; the albedo is the
/// first region whose outer edge lies beyond it.
pub fn profile_bins(def: &RingsDef, stored: &[u16]) -> Vec<[f32; 4]> {
    let n = stored.len().max(1) as f32;
    stored
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let r = def.inner_m + (i as f32 + 0.5) * (def.outer_m - def.inner_m) / n;
            let a = def.albedo.iter().find(|(edge, _)| r <= *edge).or(def.albedo.last()).map_or([0.5; 3], |x| x.1);
            [a[0], a[1], a[2], f32::from(v) / TAU_SCALE]
        })
        .collect()
}

/// IEEE half-precision bits of a value in 0..65504 (round to nearest;
/// below the normal range, 0).
pub fn f16_bits(v: f32) -> u16 {
    if v.is_nan() || v < 6.1e-5 {
        return 0;
    }
    let bits = v.min(65504.0).to_bits();
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mant = bits & 0x7f_ffff;
    let half = ((exp as u32) << 10) | (mant >> 13);
    // Round to nearest on the dropped bits (a carry into the exponent is right).
    (half + ((mant >> 12) & 1)) as u16
}

/// The profile's mip levels after the first: each halves the bins (width
/// ⌊w/2⌋, an odd last bin joining the last pair, as the GPU sizes mips),
/// averaging the albedo and the transmission at normal incidence (a coarse
/// bin's τ is −ln of its bins' mean e^−τ, so it lets as much light through
/// as they do). Seen from afar, the rings are then the average of the
/// ringlets a pixel covers, not one of them picked at random (aliasing:
/// speckles where the occultation's τ is noisy).
pub fn mip_chain(bins: &[[f32; 4]]) -> Vec<Vec<[f32; 4]>> {
    let mut levels: Vec<Vec<[f32; 4]>> = Vec::new();
    let mut cur = bins.to_vec();
    while cur.len() > 1 {
        let n = cur.len() / 2;
        let next: Vec<[f32; 4]> = (0..n)
            .map(|i| {
                let group = if i + 1 == n { &cur[2 * i..] } else { &cur[2 * i..2 * i + 2] };
                let k = group.len() as f32;
                let mut out = [0.0; 4];
                for b in group {
                    for c in 0..3 {
                        out[c] += b[c] / k;
                    }
                    out[3] += (-b[3]).exp() / k;
                }
                out[3] = -out[3].max(1e-30).ln();
                out
            })
            .collect();
        levels.push(next.clone());
        cur = next;
    }
    levels
}

/// Loads a ring profile as an RGBA16F texture one pixel high, with its
/// mip chain and a linear sampler.
pub fn load_profile(path: &std::path::Path, def: &RingsDef) -> Option<Image> {
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    let img = image::open(path).map_err(|e| error!("ring profile {}: {e}", path.display())).ok()?.to_luma16();
    let bins = profile_bins(def, img.as_raw());
    let mips = mip_chain(&bins);
    let data: Vec<u8> = std::iter::once(&bins)
        .chain(&mips)
        .flat_map(|level| level.iter().flat_map(|b| b.iter().flat_map(|v| f16_bits(*v).to_le_bytes())))
        .collect();
    let mut image = Image::new(
        Extent3d { width: bins.len() as u32, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = 1 + mips.len() as u32;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    Some(image)
}

/// Spawns a ring disc for every body with rings, and gives the body's
/// terrain the profile for the rings' shadows.
#[allow(clippy::too_many_arguments)]
pub fn setup(
    mut commands: Commands,
    defs: Res<BodyDefs>,
    sim: Res<SimState>,
    terrain: Res<crate::terrain::Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RingMaterial>>,
    mut terrain_materials: ResMut<Assets<crate::terrain::TerrainMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    for (node, def) in &defs.defs {
        let Some(r) = &def.rings else { continue };
        let dir = crate::body_visual::body_dir(&sim.world.eph.node(*node).name);
        let Some(img) = load_profile(&dir.join(&r.profile), r) else { continue };
        let bins = img.width();
        let handle = images.add(img);
        let radii = Vec4::new(r.inner_m, r.outer_m, bins as f32, 0.0);
        let planet = terrain.materials().find(|(n, _)| n == node).and_then(|(_, h)| terrain_materials.get_mut(h));
        if let Some(mut m) = planet {
            m.extension.ring_profile = Some(handle.clone());
            m.extension.params.rings = radii;
        }
        let mesh = meshes.add(Annulus::new(r.inner_m / r.outer_m, 1.0).mesh().resolution(512));
        let material = materials.add(RingMaterial { params: RingParams { radii, ..default() }, profile: handle });
        commands.spawn((
            Rings(*node),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NotShadowCaster,
            NoFrustumCulling,
        ));
    }
}

/// Places each ring disc camera-relative in its body's equatorial plane.
pub fn update(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    scene: Res<crate::lighting::LightScene>,
    mut rings: Query<(&Rings, &MeshMaterial3d<RingMaterial>, &mut Transform)>,
    mut materials: ResMut<Assets<RingMaterial>>,
) {
    let snap = sim.world.snapshot(sim.clock);
    for (r, mat, mut t) in &mut rings {
        let Some(p) = sim.world.source(r.0).and_then(|s| s.physical.as_ref()) else { continue };
        let centre = snap.relative_r(r.0, rig.anchor) - rig.cam_pos;
        let pole = p.rotation.pole(sim.clock);
        let Some(mut m) = materials.get_mut(&mat.0) else { continue };
        let outer = f64::from(m.params.radii.y);
        *t = Transform {
            translation: centre.as_vec3(),
            rotation: Quat::from_rotation_arc(Vec3::Z, pole.as_vec3()),
            scale: Vec3::splat(outer as f32),
        };
        m.params.center = centre.as_vec3().extend(p.radius_eq as f32);
        m.params.normal = pole.as_vec3().extend(0.0);
        if let (Some(i), Some(light)) = (scene.index(r.0), scene.body(r.0)) {
            m.params.starlight = crate::lighting::LightUniform::new(&scene, &light, &[i], false);
        }
    }
}

/// The fraction of sunlight the rings let through to a point `p` (both
/// relative to the planet's centre, `n` its pole, `to_star` unit), with
/// `tau_at(r)` the normal optical depth at radius r: exp(−τ/|μ|) where the
/// ray to the star crosses the ring plane, 1 elsewhere. The terrain shader
/// mirrors it (this CPU copy pins the rule in tests).
#[cfg_attr(not(test), allow(dead_code))]
pub fn ring_transmission(p: DVec3, to_star: DVec3, n: DVec3, tau_at: impl Fn(f64) -> f64) -> f64 {
    let mu = to_star.dot(n);
    if mu.abs() < 1e-9 {
        return 1.0;
    }
    let t = -p.dot(n) / mu;
    if t <= 0.0 {
        return 1.0;
    }
    let r = (p + to_star * t).length();
    (-tau_at(r) / mu.abs()).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def() -> RingsDef {
        RingsDef {
            profile: String::new(),
            inner_m: 70.0,
            outer_m: 140.0,
            albedo: vec![(92.0, [0.2; 3]), (140.0, [0.5; 3])],
        }
    }

    #[test]
    fn mips_average_albedo_and_transmission() {
        let ln2 = 2f32.ln();
        // Five bins: the level below has two (the odd last bin joins the
        // last pair), then one.
        let bins = [[0.2, 0.2, 0.2, 0.0], [0.4, 0.4, 0.4, 100.0], [0.5; 4], [0.5; 4], [0.5; 4]];
        let mips = mip_chain(&bins);
        assert_eq!(mips.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
        // (case, texel, expected albedo, expected optical depth)
        let cases = [
            ("clear and opaque: half the light, tau ln 2", mips[0][0], 0.3, ln2),
            ("three equal bins keep their values", mips[0][1], 0.5, 0.5),
        ];
        for (case, t, albedo, tau) in cases {
            assert!((t[0] - albedo).abs() < 1e-6 && (t[3] - tau).abs() < 1e-5, "{case}: {t:?}");
        }
        let mean_t = 0.5 * (0.5 + (-0.5f32).exp());
        assert!((mips[1][0][3] + mean_t.ln()).abs() < 1e-5, "the top level: {:?}", mips[1][0]);
    }

    #[test]
    fn half_floats() {
        // (value, bits)
        let cases = [(0.0, 0), (1.0, 0x3c00), (2.5, 0x4100), (0.5, 0x3800), (6.5535, 0x468e), (1e-6, 0), (1e9, 0x7bff)];
        for (v, bits) in cases {
            assert_eq!(f16_bits(v), bits, "{v}");
        }
    }

    #[test]
    fn bins_take_their_region_albedo_and_depth() {
        let bins = profile_bins(&def(), &[0, 10_000, 25_000, 65_535]);
        // Centres at 78.75, 96.25, 113.75, 131.25.
        assert_eq!(bins[0], [0.2, 0.2, 0.2, 0.0]);
        assert_eq!(bins[1][..3], [0.5; 3]);
        assert!((bins[1][3] - 1.0).abs() < 1e-6 && (bins[2][3] - 2.5).abs() < 1e-6);
    }

    #[test]
    fn rings_shade_only_where_the_ray_crosses_them() {
        let n = DVec3::Z;
        let tau = |r: f64| if (100.0..120.0).contains(&r) { 1.0 } else { 0.0 };
        let sun = DVec3::new(1.0, 0.0, 1.0).normalize();
        // (case, point, expected transmission)
        let cases = [
            ("below the plane, the ray crosses at r = 110", DVec3::new(50.0, 0.0, -60.0), (-2f64.sqrt()).exp()),
            ("the ray crosses inside the gap", DVec3::new(-20.0, 0.0, -60.0), 1.0),
            ("above the plane: the ray never crosses", DVec3::new(50.0, 0.0, 10.0), 1.0),
        ];
        for (case, p, want) in cases {
            let t = ring_transmission(p, sun, n, tau);
            assert!((t - want).abs() < 1e-9, "{case}: {t}");
        }
        assert_eq!(ring_transmission(DVec3::new(50.0, 0.0, -1.0), DVec3::X, n, tau), 1.0, "Sun in the plane");
    }
}
