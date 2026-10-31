//! Ships in the scene: each vessel a root entity placed camera-relative
//! from its state at the clock, its parts drawn as children
//! (`super::parts`: a craft of catalog parts by part, static parts merged
//! by colour, moving parts turned from the ship state; the test craft as
//! one mesh in the ship colour), a flame per engine sized by its thrust,
//! and the ship's light (`ship_light`: every star with its eclipses and
//! the air on the way, planetshine, its own sky and shadow cascade, D110)
//! in one buffer its materials share.
//!
//! Drawings are built once per craft id (a piece after a split gets its own
//! from the meshes of its parts, kept per base craft and part id) and a
//! vessel's is rebuilt when its fleet slot holds another craft.

use super::exhaust::{self, PlumeMaterial, PlumeParams};
use super::parts::{self, Group, Nozzle};
use crate::camera::CameraRig;
use crate::ship_light::{ShipExt, ShipLight, ShipMaterial};
use crate::state::SimState;
use bevy::camera::visibility::RenderLayers;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use glam::DVec3;
use sim::craft::{Craft, RenderMesh};
use sim::systems::exhaust::{NozzleSpec, Products};
use std::collections::HashMap;
use std::sync::Arc;

/// Ship base colours (sRGB) of the test craft: other vessels and the active one.
const SHIP_COLOR: [f32; 3] = [0.8, 0.8, 0.82];
const ACTIVE_COLOR: [f32; 3] = [0.95, 0.85, 0.6];

/// A component engine's (a thruster pod's) exit below its gimbal point (m).
const POD_EXIT: f64 = 0.3;

/// A vessel's drawing: the fleet slot it draws and the craft it was built
/// for.
#[derive(Component)]
pub struct ShipVisual {
    pub index: usize,
    pub craft: String,
    /// The drawing's bounding sphere (centre in body axes, radius; m).
    bound: (Vec3, f32),
    /// The brightest star's light at the craft (rgb lux), what its plumes
    /// scatter.
    starlight: [f32; 3],
}

impl ShipVisual {
    /// The drawing's bounding sphere (centre in body axes, radius; m).
    pub fn bound(&self) -> (Vec3, f32) {
        self.bound
    }
}

/// A drawn group of a vessel's parts; a mechanism (an index into the ship
/// definition's) turns it.
#[derive(Component)]
pub struct ShipGroup {
    mechanism: Option<usize>,
    /// The test craft: coloured as the active ship or another.
    plain: bool,
}

/// An engine's flame (an index into the ship definition's engines; `None`:
/// the test craft's one engine).
#[derive(Component)]
pub struct Flame {
    engine: Option<usize>,
    nozzle: Nozzle,
    /// A main engine: its flame follows the gimbal.
    main: bool,
    /// Its plume (`exhaust`), when its propellants make one.
    plume: Option<Entity>,
}

/// What an engine's plume is drawn from: its nozzle, the products of its
/// propellant pair and its mixture ratio.
#[derive(Clone, Debug)]
pub struct PlumeSource {
    spec: NozzleSpec,
    products: Products,
    mixture_ratio: f64,
}

/// An engine's plume, drawn instead of its flame cone when exhaust is on.
#[derive(Component)]
pub struct Plume {
    source: PlumeSource,
}

/// A drawn group: its mesh, colour (linear; `None`: the ship colour) and
/// mechanism.
struct Drawn {
    mesh: Handle<Mesh>,
    colour: Option<[f32; 3]>,
    mechanism: Option<usize>,
}

/// A craft's drawing.
pub struct Model {
    groups: Vec<Drawn>,
    flames: Vec<(Option<usize>, Nozzle, bool, Option<PlumeSource>)>,
    /// The bounding sphere (centre in body axes, radius; m), with room for
    /// the mechanisms (legs, fins) to swing out.
    bound: (Vec3, f32),
}

/// Room for parts on mechanisms around the drawing's bounding sphere.
const BOUND_MARGIN: f32 = 1.25;

/// The bounding sphere of meshes (centre of their box, farthest vertex)
/// grown by [`BOUND_MARGIN`].
fn bound<'a>(meshes: impl Iterator<Item = &'a RenderMesh> + Clone) -> (Vec3, f32) {
    let points = || meshes.clone().flat_map(|m| m.positions.iter().map(|p| Vec3::from_array(*p)));
    let (lo, hi) = points().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(p), hi.max(p)));
    if lo.x > hi.x {
        return (Vec3::ZERO, 1.0);
    }
    let centre = 0.5 * (lo + hi);
    let radius = points().map(|p| (p - centre).length()).fold(0.0, f32::max);
    (centre, radius * BOUND_MARGIN)
}

/// The drawings built so far.
#[derive(Resource, Default)]
pub struct ShipModels {
    /// Each part's mesh by (base craft, part id, deployed).
    parts: HashMap<(String, u32, bool), Arc<RenderMesh>>,
    crafts: HashMap<String, Arc<Model>>,
    /// Each part's mesh uploaded alone (the systems screen draws parts one
    /// by one), keyed as `parts`.
    handles: HashMap<(String, u32, bool), Handle<Mesh>>,
    /// The unit flame cone and its material.
    flame: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    /// The plumes' bound: a unit cylinder.
    plume: Option<Handle<Mesh>>,
}

/// A Bevy mesh from a render mesh (body axes, +Z the nose).
pub fn bevy_mesh(m: &RenderMesh) -> Mesh {
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone())
        .with_inserted_indices(bevy::mesh::Indices::U32(m.indices.clone()))
}

impl ShipModels {
    /// Part `k`'s mesh of `craft` (kept per base craft, part id and
    /// whether the configuration deploys it: `base[#parts]@deployed`).
    pub fn part_mesh(&mut self, craft: &Craft, k: usize) -> Arc<RenderMesh> {
        let id = craft.parts[k].id;
        let key = (craft.id.base().to_string(), id, craft.id.deployed().contains(&id));
        self.parts.entry(key).or_insert_with(|| Arc::new(parts::part_mesh(craft, k))).clone()
    }

    /// Every part's mesh of `craft` in body axes (the test craft's one
    /// mesh), kept as [`Self::part_mesh`].
    pub fn meshes_of(&mut self, craft: &Craft) -> Vec<Arc<RenderMesh>> {
        if craft.ship.is_none() {
            let key = (craft.id.base().to_string(), u32::MAX, false);
            return vec![self.parts.entry(key).or_insert_with(|| Arc::new(craft.mesh.clone())).clone()];
        }
        (0..craft.parts.len()).map(|k| self.part_mesh(craft, k)).collect()
    }

    /// Part `k`'s mesh of `craft` as a Bevy mesh (kept as [`Self::part_mesh`]).
    pub fn part_handle(&mut self, craft: &Craft, k: usize, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        let id = craft.parts[k].id;
        let key = (craft.id.base().to_string(), id, craft.id.deployed().contains(&id));
        if let Some(h) = self.handles.get(&key) {
            return h.clone();
        }
        let handle = meshes.add(bevy_mesh(&self.part_mesh(craft, k)));
        self.handles.insert(key, handle.clone());
        handle
    }

    /// The drawing of `craft`, built on first use.
    fn model(&mut self, craft: &Craft, meshes: &mut Assets<Mesh>) -> Arc<Model> {
        if let Some(m) = self.crafts.get(&craft.id.0) {
            return m.clone();
        }
        let model = Arc::new(self.build(craft, meshes));
        self.crafts.insert(craft.id.0.clone(), model.clone());
        model
    }

    fn build(&mut self, craft: &Craft, meshes: &mut Assets<Mesh>) -> Model {
        let Some(def) = craft.ship.as_deref() else {
            // No parts' systems: no engines, nothing burns.
            let plain = Drawn { mesh: meshes.add(bevy_mesh(&craft.mesh)), colour: None, mechanism: None };
            let bound = bound(std::iter::once(&craft.mesh));
            return Model { groups: vec![plain], flames: Vec::new(), bound };
        };
        let ids: Vec<u32> = craft.parts.iter().map(|p| p.id).collect();
        let colours = parts::part_colours(craft, def, sim::materials::materials());
        let part_meshes: Vec<Arc<RenderMesh>> = (0..craft.parts.len()).map(|k| self.part_mesh(craft, k)).collect();
        let groups = (parts::groups(&colours, &parts::mechanisms(def, &ids)).into_iter())
            .map(|g| match g {
                Group::Static { colour, parts: list } => {
                    let merged = parts::merge(list.iter().map(|&k| part_meshes[k].as_ref()));
                    Drawn { mesh: meshes.add(bevy_mesh(&merged)), colour: Some(colour), mechanism: None }
                }
                Group::Moving { colour, part, mechanism } => {
                    let mesh = meshes.add(bevy_mesh(&part_meshes[part]));
                    Drawn { mesh, colour: Some(colour), mechanism: Some(mechanism) }
                }
            })
            .collect();
        let flames = (def.engines.iter().enumerate())
            .map(|(i, e)| {
                let k = ids.iter().position(|&id| id == e.part);
                let nozzle = match k.filter(|_| e.main) {
                    Some(k) => parts::nozzle(e.data.pos, e.data.dir, e.data.exit_area, &part_meshes[k]),
                    None => {
                        let exhaust = -e.data.dir.normalize_or(DVec3::Z);
                        let radius = (e.data.exit_area.max(0.0) / std::f64::consts::PI).sqrt();
                        Nozzle { exit: e.data.pos + exhaust * POD_EXIT, exhaust, radius }
                    }
                };
                let source =
                    sim::systems::exhaust::propellants().pair(&e.data.oxidizer, &e.data.fuel).map(|p| PlumeSource {
                        spec: NozzleSpec::of(&e.data),
                        products: p.clone(),
                        mixture_ratio: e.data.mixture_ratio,
                    });
                (Some(i), nozzle, e.main, source)
            })
            .collect();
        let bound = bound(part_meshes.iter().map(|m| m.as_ref()));
        Model { groups, flames, bound }
    }
}

/// The flame mark's look: a unit cone, glowing.
fn flame_assets(
    models: &mut ShipModels,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> (Handle<Mesh>, Handle<StandardMaterial>) {
    models
        .flame
        .get_or_insert_with(|| {
            let mesh = meshes.add(Cone { radius: 1.0, height: 1.0 });
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.6, 0.2),
                emissive: LinearRgba::rgb(40.0, 14.0, 3.0),
                unlit: true,
                ..default()
            });
            (mesh, material)
        })
        .clone()
}

/// Spawns vessel `index`'s drawing of `craft`.
#[allow(clippy::too_many_arguments)]
fn spawn(
    commands: &mut Commands,
    index: usize,
    craft: &Craft,
    models: &mut ShipModels,
    meshes: &mut Assets<Mesh>,
    ship_materials: &mut Assets<ShipMaterial>,
    flame_materials: &mut Assets<StandardMaterial>,
    plume_materials: &mut Assets<PlumeMaterial>,
    buffers: &mut Assets<ShaderBuffer>,
) {
    let model = models.model(craft, meshes);
    let (cone, glow) = flame_assets(models, meshes, flame_materials);
    let bound = models.plume.get_or_insert_with(|| meshes.add(Cylinder::new(1.0, 1.0))).clone();
    // One light buffer per ship (its light depends on where it is), shared
    // by its materials.
    let light = buffers.add(ShaderBuffer::from(ShipLight::default()));
    let root = commands
        .spawn((
            ShipVisual { index, craft: craft.id.0.clone(), bound: model.bound, starlight: [0.0; 3] },
            Transform::IDENTITY,
            Visibility::Visible,
        ))
        .id();
    for Drawn { mesh, colour, mechanism } in &model.groups {
        let base_color = match colour {
            Some([r, g, b]) => Color::linear_rgb(*r, *g, *b),
            None => Color::srgb_from_array(SHIP_COLOR),
        };
        let material = ship_materials.add(ShipMaterial {
            base: StandardMaterial { base_color, ..default() },
            extension: ShipExt { light: light.clone() },
        });
        commands.spawn((
            ShipGroup { mechanism: *mechanism, plain: colour.is_none() },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            // Drawn by the camera, and cast into the craft's shadow
            // cascades (`lighting::shadows`).
            RenderLayers::from_layers(&[0, crate::lighting::shadows::CRAFT_LAYER]),
            Transform::IDENTITY,
            ChildOf(root),
        ));
    }
    for (engine, nozzle, main, source) in &model.flames {
        let plume = source.as_ref().map(|source| {
            commands
                .spawn((
                    Plume { source: source.clone() },
                    Mesh3d(bound.clone()),
                    MeshMaterial3d(plume_materials.add(PlumeMaterial::default())),
                    NotShadowCaster,
                    Transform::IDENTITY,
                    Visibility::Hidden,
                    ChildOf(root),
                ))
                .id()
        });
        commands.spawn((
            Flame { engine: *engine, nozzle: *nozzle, main: *main, plume },
            Mesh3d(cone.clone()),
            MeshMaterial3d(glow.clone()),
            NotShadowCaster,
            Transform::IDENTITY,
            Visibility::Hidden,
            ChildOf(root),
        ));
    }
}

/// Places every ship and fills its light; (re)builds drawings for new
/// vessels and fleet slots that now hold another craft.
#[allow(clippy::too_many_arguments)]
pub fn update_ships(
    mut commands: Commands,
    sim: Res<SimState>,
    rig: Res<CameraRig>,
    scene: Res<crate::lighting::LightScene>,
    defs: Res<crate::scene::BodyDefs>,
    look: Res<crate::look::Parts>,
    mut models: ResMut<ShipModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ship_materials: ResMut<Assets<ShipMaterial>>,
    mut flame_materials: ResMut<Assets<StandardMaterial>>,
    mut plume_materials: ResMut<Assets<PlumeMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut craft_groups: ResMut<crate::lighting::shadows::CraftGroups>,
    mut ships: Query<(Entity, &mut ShipVisual, &mut Transform, &Children)>,
    groups: Query<(&ShipGroup, &MeshMaterial3d<ShipMaterial>)>,
) {
    let snap = sim.world.snapshot(sim.clock);
    let bodies = scene.spheres();
    let airs = scene.airs(&defs);
    let star_dirs: Vec<DVec3> = scene.slots.iter().map(|&k| scene.stars[k].centre.normalize_or(DVec3::Z)).collect();
    let mut seen = vec![false; sim.fleet.len()];
    // Each ship's light buffer, light and bounding sphere: the light is
    // written once the craft are grouped into shadow cascades.
    let mut lit: Vec<(Handle<ShaderBuffer>, ShipLight, (DVec3, f64))> = Vec::new();
    for (entity, mut ship, mut t, children) in &mut ships {
        let vessel = sim.fleet.get(ship.index).filter(|v| v.craft.id.0 == ship.craft);
        let Some(vessel) = vessel.filter(|_| !seen[ship.index]) else {
            commands.entity(entity).despawn();
            continue;
        };
        seen[ship.index] = true;
        let (anchor, r, _) = vessel.state_at(&sim.world, sim.clock);
        let pos = snap.relative(anchor, rig.anchor).r + r - rig.cam_pos;
        // The vessel's position is its centre of mass; the parts are in the
        // craft's body axes.
        let q = vessel.attitude.q;
        let origin = pos - q * vessel.mass_props().com;
        *t = Transform { translation: origin.as_vec3(), rotation: q.as_quat(), scale: Vec3::ONE };
        let mut light = crate::lighting::object_light(pos, &scene.stars, &scene.slots, &bodies, &airs);
        if !look.planetshine {
            light.shines.clear();
        }
        ship.starlight = brightest_starlight(&light, &scene, pos);
        let sky = crate::sky::sky_radiance_at(pos, &scene, &defs);
        let data = crate::ship_light::ship_light(&light, &star_dirs, sky);
        let centre = origin + q * ship.bound.0.as_dvec3();
        let want = Color::srgb_from_array(if ship.index == sim.active { ACTIVE_COLOR } else { SHIP_COLOR });
        let mut light_done = false;
        for child in children.iter() {
            let Ok((group, mat)) = groups.get(child) else { continue };
            let Some(m) = ship_materials.get(&mat.0) else { continue };
            if !light_done {
                lit.push((m.extension.light.clone(), data, (centre, f64::from(ship.bound.1))));
                light_done = true;
            }
            // The test craft's colour changes only when the active ship does.
            if group.plain && m.base.base_color != want {
                if let Some(mut m) = ship_materials.get_mut(&mat.0) {
                    m.base.base_color = want;
                }
            }
        }
    }
    let spheres: Vec<(DVec3, f64)> = lit.iter().map(|l| l.2).collect();
    let (shadow_groups, of) = crate::lighting::shadows::group(&spheres);
    craft_groups.0 = shadow_groups;
    for ((buffer, mut data, _), group) in lit.into_iter().zip(of) {
        data.ambient.z = group.map_or(-1.0, |g| g as f32);
        if let Some(mut b) = buffers.get_mut(&buffer) {
            b.set_data(data);
        }
    }
    for (i, _) in seen.iter().enumerate().filter(|(_, s)| !**s) {
        let craft = sim::craft::craft_of(&sim.fleet[i].craft.id);
        let assets = (&mut *meshes, &mut *ship_materials, &mut *flame_materials, &mut *plume_materials, &mut *buffers);
        spawn(&mut commands, i, &craft, &mut models, assets.0, assets.1, assets.2, assets.3, assets.4);
    }
}

/// The brightest drawn star's light at a craft at `pos` (rgb lux, its
/// eclipses and the air on the way included), what its plumes scatter.
fn brightest_starlight(
    light: &crate::lighting::rules::ObjectLight,
    scene: &crate::lighting::LightScene,
    pos: DVec3,
) -> [f32; 3] {
    let (Some(&k), Some(&share), Some(air)) = (scene.slots.first(), light.scale.first(), light.air.first()) else {
        return [0.0; 3];
    };
    let star = &scene.stars[k];
    let lux = star.lux_at(pos) * share;
    [0, 1, 2].map(|c| (lux * air[c] * star.tint[c]) as f32)
}

/// A vessel's engines now: the air pressure at it (Pa), each engine's
/// thrust (N) and share of its full thrust (the test craft's one engine:
/// its share only).
struct Firing {
    pressure: f64,
    thrusts: Vec<f64>,
    fractions: Vec<f64>,
}

/// Turns the parts on mechanisms to the ship state's angles and draws each
/// engine's exhaust by its thrust (hidden when it gives none): its plume
/// when exhaust is on and its propellants make one (`exhaust`), else its
/// flame cone.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn update_parts(
    sim: Res<SimState>,
    visuals: Res<crate::persist::VisualSettings>,
    look: Res<crate::look::Parts>,
    bench: Res<crate::bench::Bench>,
    ships: Query<(&ShipVisual, &Transform), Without<ShipGroup>>,
    mut groups: Query<(&ShipGroup, &ChildOf, &mut Transform), (Without<Flame>, Without<ShipVisual>, Without<Plume>)>,
    mut flames: Query<
        (&Flame, &ChildOf, &mut Transform, &mut Visibility),
        (Without<ShipGroup>, Without<ShipVisual>, Without<Plume>),
    >,
    mut plumes: Query<
        (&Plume, &mut Transform, &mut Visibility, &MeshMaterial3d<PlumeMaterial>),
        (Without<ShipGroup>, Without<ShipVisual>, Without<Flame>),
    >,
    mut plume_materials: ResMut<Assets<PlumeMaterial>>,
) {
    let vessel = |parent: Entity| ships.get(parent).ok().and_then(|(s, _)| sim.fleet.get(s.index));
    for (group, child_of, mut t) in &mut groups {
        let (Some(k), Some(v)) = (group.mechanism, vessel(child_of.parent())) else { continue };
        let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else { continue };
        let (Some(m), Some(s)) = (def.mechanisms.get(k), state.mechanisms.get(k)) else { continue };
        let built = parts::built_angle(&m.item, v.craft.id.deployed().contains(&m.part));
        let want = parts::hinge(m.item.pos, m.item.axis, built, s.angle);
        if *t != want {
            *t = want;
        }
    }
    let plumes_on = exhaust::drawn(visuals.exhaust, bench.running(), look.exhaust);
    let mut firing: HashMap<Entity, Firing> = HashMap::new();
    let snap = sim.world.snapshot(sim.clock);
    for (flame, child_of, mut t, mut vis) in &mut flames {
        let parent = child_of.parent();
        let Some(v) = vessel(parent) else { continue };
        let i = ships.get(parent).map_or(usize::MAX, |(s, _)| s.index);
        let now = firing.entry(parent).or_insert_with(|| {
            let (anchor, r, _) = v.state_at(&sim.world, sim.clock);
            let pressure = sim::forces::ambient_pressure(&sim.world, &snap, anchor, r);
            match (v.ship_def(), v.engine_thrusts(throttle_of(&sim, i), pressure)) {
                (Some(def), Some(thrusts)) => {
                    Firing { pressure, fractions: parts::engine_fractions(&def, &thrusts), thrusts }
                }
                _ => Firing { pressure, thrusts: Vec::new(), fractions: Vec::new() },
            }
        });
        let k = flame.engine.unwrap_or(0);
        let fraction = now.fractions.get(k).copied().unwrap_or(0.0);
        let thrust = flame.engine.map(|e| now.thrusts.get(e).copied().unwrap_or(0.0));
        let steered = v.ship().and_then(|s| s.thrust_dir).filter(|_| flame.main);
        let nozzle = parts::gimballed(&flame.nozzle, steered);
        if let Some(Ok((source, mut pt, mut pvis, mat))) = flame.plume.map(|e| plumes.get_mut(e)) {
            let shown = plumes_on && fraction > 0.0;
            if shown {
                let (ship, root) =
                    ships.get(parent).map(|(s, t)| (s.starlight, *t)).unwrap_or(([0.0; 3], Transform::IDENTITY));
                let src = &source.source;
                let level = match thrust {
                    Some(f) => src.spec.level(f, now.pressure),
                    None => fraction,
                };
                let seed = sim::systems::exhaust::exit_state(&src.spec, &src.products, src.mixture_ratio, level);
                let p = exhaust::plume(&seed, &sim::systems::exhaust::jet(&seed, now.pressure), now.pressure);
                // A plume without soot is not drawn (nearly invisible).
                pvis.set_if_neq(if p.length > 0.0 { Visibility::Visible } else { Visibility::Hidden });
                *pt = exhaust::bound(&nozzle, &p);
                let origin = root.translation + root.rotation * nozzle.exit.as_vec3();
                let axis = root.rotation * nozzle.exhaust.as_vec3();
                if let Some(mut m) = plume_materials.get_mut(&mat.0) {
                    m.params = PlumeParams::new(&p, origin, axis, ship);
                }
            } else {
                pvis.set_if_neq(Visibility::Hidden);
            }
        }
        // The cone when exhaust is off, or for an engine with no plume.
        let cone = fraction > 0.0 && !(plumes_on && flame.plume.is_some());
        vis.set_if_neq(if cone { Visibility::Visible } else { Visibility::Hidden });
        if cone {
            *t = parts::flame(&nozzle, fraction);
        }
    }
}

/// The throttle vessel `i` flies with: the pilot's for the active vessel,
/// none for the others (they fly with passive controls).
fn throttle_of(sim: &SimState, i: usize) -> f64 {
    if i == sim.active {
        sim.controls().throttle
    } else {
        0.0
    }
}
