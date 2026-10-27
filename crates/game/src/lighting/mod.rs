//! Physical lighting per object (D055), for any number of stars: the
//! illuminance each lit object receives from every star at *its own*
//! position (not the camera's), the part of each star's disc other bodies
//! hide (eclipses), and planetshine (starlight reflected by nearby bodies,
//! Lambert spheres with phase angle). The rules are pure (`rules`, `haze`,
//! `color`); the systems here gather the scene once a frame
//! ([`LightScene`]), drive Bevy's directional lights (one per drawn star)
//! and fill the shaders' uniforms ([`LightUniform`], `light.wgsl`).
//!
//! Units: luminous power in lumens, illuminance in lux, distances in metres.

pub mod color;
pub mod haze;
pub mod rules;
#[cfg(test)]
mod tests;

pub use haze::{haze_above, haze_exposure_share, haze_light};
pub use rules::{
    body_light, eclipse_factor, exposure_ev100, flux, luminous_power, object_light, reference_lux, star_slots,
    BodyLight, Sphere, Star, MAX_OCCLUDERS, MAX_SHINES, MAX_STARS,
};

use crate::camera::CameraRig;
use crate::look::Parts;
use crate::scene::BodyDefs;
use crate::state::SimState;
use crate::terrain::{Terrain, TerrainMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::ShaderType;
use glam::DVec3;
use sim::frame::NodeId;

/// A star in the scene's data: its node, luminous power (lm) and the
/// colour of its light relative to the Sun's (`color::star_tint`).
#[derive(Clone, Copy, Debug)]
pub struct StarDef {
    pub node: NodeId,
    pub lm: f64,
    pub tint: [f64; 3],
}

/// The scene's light this frame, camera-relative: every star, the stars
/// drawn as lights (brightest at the camera first), and every body that
/// can occlude or reflect (each non-emissive body with a radius).
#[derive(Resource, Default)]
pub struct LightScene {
    pub stars: Vec<Star>,
    pub slots: Vec<usize>,
    pub bodies: Vec<(NodeId, Sphere)>,
}

impl LightScene {
    /// The bodies as spheres, in the same order as `bodies`.
    pub fn spheres(&self) -> Vec<Sphere> {
        self.bodies.iter().map(|b| b.1).collect()
    }

    /// The index of `node` among `bodies`.
    pub fn index(&self, node: NodeId) -> Option<usize> {
        self.bodies.iter().position(|b| b.0 == node)
    }

    /// The lighting of body `node` ([`body_light`]); none when it is not
    /// one of the scene's bodies.
    pub fn body(&self, node: NodeId) -> Option<BodyLight> {
        let i = self.index(node)?;
        Some(body_light(i, &self.spheres(), &self.stars, &self.slots))
    }

    /// The total stellar flux at the camera (lux, no eclipses: D087).
    pub fn lux_at_camera(&self) -> f64 {
        self.stars.iter().map(|s| s.lux_at(DVec3::ZERO)).sum()
    }
}

/// The star that gives the most light at `pos` (relative to `anchor`),
/// ties by order: the reference star for what draws one star only (the
/// tour's framing, the sky's daylight, the lens flare).
pub fn brightest_star(defs: &BodyDefs, sim: &SimState, anchor: NodeId, pos: DVec3) -> Option<NodeId> {
    let snap = sim.world.snapshot(sim.clock);
    let lux = |s: &StarDef| flux(s.lm, (snap.relative_r(s.node, anchor) - pos).length().max(1.0));
    let mut best: Option<(&StarDef, f64)> = None;
    for s in &defs.stars {
        let e = lux(s);
        if best.is_none_or(|(_, b)| e > b) {
            best = Some((s, e));
        }
    }
    best.map(|(s, _)| s.node)
}

/// Gathers [`LightScene`] from the simulation at the current clock.
pub fn update_scene(sim: Res<SimState>, rig: Res<CameraRig>, defs: Res<BodyDefs>, mut scene: ResMut<LightScene>) {
    let snap = sim.world.snapshot(sim.clock);
    let rel = |n: NodeId| snap.relative_r(n, rig.anchor) - rig.cam_pos;
    let radius = |n: NodeId| sim.world.source(n).and_then(|s| s.physical.as_ref()).map_or(0.0, |p| p.radius_eq);
    scene.stars = defs
        .stars
        .iter()
        .map(|s| Star { centre: rel(s.node), radius: radius(s.node), lm: s.lm, tint: s.tint })
        .collect();
    scene.slots = star_slots(&scene.stars, DVec3::ZERO);
    scene.bodies = defs
        .defs
        .iter()
        .filter(|(n, d)| d.emissive.is_none() && radius(*n) > 0.0)
        .map(|(n, d)| {
            let sphere = Sphere {
                centre: rel(*n),
                radius: radius(*n),
                albedo: reflector_albedo(d.albedo, d.geometric_albedo),
                tint: color::reflector_tint(d.base_color),
            };
            (*n, sphere)
        })
        .collect();
}

/// The Lambert albedo a body reflects other bodies' light with
/// ([`planetshine`](rules::planetshine)'s sphere): the one whose geometric
/// albedo (2/3 of it) is the body's measured one, so planetshine at full
/// phase is right (E p (R/d)²); else the Bond albedo.
pub fn reflector_albedo(bond: f32, geometric: Option<f32>) -> f64 {
    geometric.map_or(f64::from(bond), |p| 1.5 * f64::from(p))
}

/// A drawn star's light: Bevy's directional light for light slot `.0`.
#[derive(Component, Clone, Copy, Debug)]
pub struct StarLight(pub usize);

/// Points each star's light from the camera to its star, with the star's
/// flux at the camera (no eclipses: each lit object applies its own),
/// colour, and disc size in the sky (the atmosphere draws it); a light
/// without a star this frame gives none.
pub fn update_lights(
    scene: Res<LightScene>,
    mut lights: Query<(&StarLight, &mut Transform, &mut DirectionalLight, &mut bevy::light::SunDisk)>,
) {
    for (slot, mut t, mut l, mut disc) in &mut lights {
        let Some(s) = scene.slots.get(slot.0).map(|&k| scene.stars[k]) else {
            l.illuminance = 0.0;
            disc.intensity = 0.0;
            continue;
        };
        let dir = -s.centre.normalize_or(DVec3::Z).as_vec3();
        *t = Transform::IDENTITY.looking_to(dir, crate::scene::light_up(dir));
        l.illuminance = s.lux_at(DVec3::ZERO) as f32;
        let [r, g, b] = s.tint.map(|c| c as f32);
        l.color = Color::linear_rgb(r, g, b);
        disc.angular_size = (2.0 * (s.radius / s.centre.length().max(s.radius)).asin()) as f32;
        disc.intensity = 1.0;
    }
}

/// The lighting uniforms every lit material shares (`light.wgsl`'s
/// `Lights`), for one body: the drawn stars, their light there, the
/// bodies that can eclipse them, planetshine and the ambient rest.
#[derive(Clone, Copy, Debug, Default, ShaderType, Reflect)]
pub struct LightUniform {
    /// Per light slot: the star's centre (camera-relative), radius (w = 0:
    /// no star in the slot).
    pub stars: [Vec4; MAX_STARS],
    /// Per slot: rgb the star's light at the body's centre (tint × lux);
    /// w its flux there over its flux at the camera (the slot's light's).
    pub star_light: [Vec4; MAX_STARS],
    /// Bodies that can eclipse the stars here: centre, radius (w = 0: none).
    pub occluders: [Vec4; MAX_OCCLUDERS],
    /// Reflecting bodies: centre, w 1 (0: none).
    pub shines: [Vec4; MAX_SHINES],
    /// rgb: each reflector's planetshine at the body's centre (lux).
    pub shine_light: [Vec4; MAX_SHINES],
    /// x: starlight without a light slot, sphere-averaged (lux).
    pub ambient: Vec4,
}

impl LightUniform {
    /// The uniforms for a body lit as `light` (from [`body_light`]), with
    /// `extra` occluders first (a ring's planet), then the body's own.
    pub fn new(scene: &LightScene, light: &BodyLight, extra: &[usize], shine: bool) -> Self {
        let mut u = LightUniform::default();
        let v4 = |c: [f64; 3], w: f64| Vec4::new(c[0] as f32, c[1] as f32, c[2] as f32, w as f32);
        for (k, (&star, &scale)) in scene.slots.iter().zip(&light.scale).enumerate() {
            let s = scene.stars[star];
            let lux = s.lux_at(DVec3::ZERO) * scale;
            u.stars[k] = s.centre.as_vec3().extend(s.radius as f32);
            u.star_light[k] = v4(s.tint.map(|c| c * lux), scale);
        }
        let occ = extra.iter().chain(light.occluders.iter().filter(|j| !extra.contains(j)));
        for (slot, &j) in u.occluders.iter_mut().zip(occ) {
            let b = scene.bodies[j].1;
            *slot = b.centre.as_vec3().extend(b.radius as f32);
        }
        if shine {
            for (m, &(j, rgb)) in light.shines.iter().enumerate() {
                u.shines[m] = scene.bodies[j].1.centre.as_vec3().extend(1.0);
                u.shine_light[m] = v4(rgb, 0.0);
            }
        }
        u.ambient.x = light.ambient as f32;
        u
    }
}

/// Sets the camera's exposure from the starlight at the camera (D087).
pub fn update_exposure(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    scene: Res<LightScene>,
    mut cams: Query<&mut bevy::camera::Exposure, With<crate::camera::MainCamera>>,
) {
    if scene.stars.is_empty() {
        return;
    }
    let snap = sim.world.snapshot(sim.clock);
    // Under a haze, the light that reaches the camera through it.
    let haze = defs.defs.iter().find_map(|(node, def)| {
        let a = def.atmosphere.as_ref().filter(|a| a.raymarch_inside)?;
        let p = sim.world.source(*node)?.physical.as_ref()?;
        let h = (rig.cam_pos - snap.relative_r(*node, rig.anchor)).length() - p.radius_eq;
        (h < f64::from(a.thickness_m)).then(|| haze_exposure_share(&haze_above(a, p.radius_eq, h)))
    });
    let e_cam = scene.lux_at_camera() * haze.unwrap_or(1.0);
    let ev100 = exposure_ev100(crate::camera::EV100, e_cam, reference_lux());
    for mut e in &mut cams {
        if (e.ev100 - ev100).abs() > 1e-4 {
            e.ev100 = ev100;
        }
    }
}

/// Fills each terrain body's lighting uniforms from [`body_light`].
pub fn update_terrain(
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    defs: Res<BodyDefs>,
    scene: Res<LightScene>,
    parts: Res<Parts>,
    terrain: Res<Terrain>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let sky = crate::sky::sky_ambient(&sim, &rig, &defs);
    let spheres = scene.spheres();
    for (node, handle) in terrain.materials() {
        let Some(i) = scene.index(node) else { continue };
        let l = body_light(i, &spheres, &scene.stars, &scene.slots);
        let Some(mut mat) = materials.get_mut(handle) else { continue };
        let p = &mut mat.extension.params;
        p.starlight = LightUniform::new(&scene, &l, &[], parts.planetshine);
        p.ambient.x = crate::sky::ambient_share(node, sky);
    }
}

/// Loads the shared lighting shader library (`sunscatter::light`) and
/// schedules the scene's light.
pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        bevy::shader::load_shader_library!(app, "light.wgsl");
        app.init_resource::<LightScene>();
    }
}
