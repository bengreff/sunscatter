//! Ships in the scene: each vessel a root entity placed camera-relative
//! from its state at the clock, its parts drawn as children
//! (`super::parts`: a craft of catalog parts by part, static parts merged
//! by colour, moving parts turned from the ship state; the test craft as
//! one mesh in the ship colour), a flame per engine sized by its thrust,
//! and the ship's light (`ship_light`: every star with its eclipses,
//! near-field planetshine, D055) in one buffer its materials share.
//!
//! Drawings are built once per craft id (a piece after a split gets its own
//! from the meshes of its parts, kept per base craft and part id) and a
//! vessel's is rebuilt when its fleet slot holds another craft.

use super::parts::{self, Group, Nozzle};
use crate::camera::CameraRig;
use crate::ship_light::{ShipExt, ShipLight, ShipMaterial};
use crate::state::SimState;
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use glam::DVec3;
use sim::craft::{Craft, RenderMesh};
use std::collections::HashMap;
use std::sync::Arc;

/// Ship base colours (sRGB) of the test craft: other vessels and the active one.
const SHIP_COLOR: [f32; 3] = [0.8, 0.8, 0.82];
const ACTIVE_COLOR: [f32; 3] = [0.95, 0.85, 0.6];

/// The test craft's flame mark: it starts this far behind the engine's
/// mount point (the bell's throat) and is this wide (m).
const TEST_FLAME_START: f64 = 1.1;
const TEST_FLAME_RADIUS: f64 = 0.7;
/// A component engine's (a thruster pod's) exit below its gimbal point (m).
const POD_EXIT: f64 = 0.3;

/// A vessel's drawing: the fleet slot it draws and the craft it was built
/// for.
#[derive(Component)]
pub struct ShipVisual {
    pub index: usize,
    pub craft: String,
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
    flames: Vec<(Option<usize>, Nozzle)>,
}

/// The drawings built so far.
#[derive(Resource, Default)]
pub struct ShipModels {
    /// Each part's mesh by (base craft, part id).
    parts: HashMap<(String, u32), Arc<RenderMesh>>,
    crafts: HashMap<String, Arc<Model>>,
    /// The unit flame cone and its material.
    flame: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
}

/// A Bevy mesh from a render mesh (body axes, +Z the nose).
pub fn bevy_mesh(m: &RenderMesh) -> Mesh {
    Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone())
        .with_inserted_indices(bevy::mesh::Indices::U32(m.indices.clone()))
}

impl ShipModels {
    /// Part `k`'s mesh of `craft` (kept per base craft and part id).
    pub fn part_mesh(&mut self, craft: &Craft, k: usize) -> Arc<RenderMesh> {
        let key = (craft.id.base().to_string(), craft.parts[k].id);
        self.parts.entry(key).or_insert_with(|| Arc::new(parts::part_mesh(craft, k))).clone()
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
            let e = &craft.spec.engine;
            let exhaust = -e.mount.dir.normalize_or(DVec3::Z);
            let nozzle = Nozzle { exit: e.mount.pos + exhaust * TEST_FLAME_START, exhaust, radius: TEST_FLAME_RADIUS };
            let plain = Drawn { mesh: meshes.add(bevy_mesh(&craft.mesh)), colour: None, mechanism: None };
            return Model { groups: vec![plain], flames: vec![(None, nozzle)] };
        };
        let m = sim::materials::materials();
        let ids: Vec<u32> = craft.parts.iter().map(|p| p.id).collect();
        let colours: Vec<[f32; 3]> = (ids.iter())
            .map(|&id| match def.parts.iter().find(|p| p.part == id) {
                Some(info) => parts::colour(&info.material, info.finish.as_deref(), m),
                None => parts::UNKNOWN,
            })
            .collect();
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
                (Some(i), nozzle)
            })
            .collect();
        Model { groups, flames }
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
    buffers: &mut Assets<ShaderBuffer>,
) {
    let model = models.model(craft, meshes);
    let (cone, glow) = flame_assets(models, meshes, flame_materials);
    // One light buffer per ship (its light depends on where it is), shared
    // by its materials.
    let light = buffers.add(ShaderBuffer::from(ShipLight::default()));
    let root = commands
        .spawn((ShipVisual { index, craft: craft.id.0.clone() }, Transform::IDENTITY, Visibility::Visible))
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
            Transform::IDENTITY,
            ChildOf(root),
        ));
    }
    for (engine, nozzle) in &model.flames {
        commands.spawn((
            Flame { engine: *engine, nozzle: *nozzle },
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
    look: Res<crate::look::Parts>,
    mut models: ResMut<ShipModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ship_materials: ResMut<Assets<ShipMaterial>>,
    mut flame_materials: ResMut<Assets<StandardMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut ships: Query<(Entity, &ShipVisual, &mut Transform, &Children)>,
    groups: Query<(&ShipGroup, &MeshMaterial3d<ShipMaterial>)>,
) {
    let snap = sim.world.snapshot(sim.clock);
    let bodies = scene.spheres();
    let star_dirs: Vec<DVec3> = scene.slots.iter().map(|&k| scene.stars[k].centre.normalize_or(DVec3::Z)).collect();
    let mut seen = vec![false; sim.fleet.len()];
    for (entity, ship, mut t, children) in &mut ships {
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
        let mut light = crate::lighting::object_light(pos, &scene.stars, &scene.slots, &bodies);
        if !look.planetshine {
            light.shines.clear();
        }
        let data = crate::ship_light::ship_light(&light, &star_dirs);
        let want = Color::srgb_from_array(if ship.index == sim.active { ACTIVE_COLOR } else { SHIP_COLOR });
        let mut light_done = false;
        for child in children.iter() {
            let Ok((group, mat)) = groups.get(child) else { continue };
            let Some(m) = ship_materials.get(&mat.0) else { continue };
            if !light_done {
                if let Some(mut b) = buffers.get_mut(&m.extension.light) {
                    b.set_data(data);
                }
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
    for (i, _) in seen.iter().enumerate().filter(|(_, s)| !**s) {
        let craft = sim::craft::craft_of(&sim.fleet[i].craft.id);
        let assets = (&mut *meshes, &mut *ship_materials, &mut *flame_materials, &mut *buffers);
        spawn(&mut commands, i, &craft, &mut models, assets.0, assets.1, assets.2, assets.3);
    }
}

/// Turns the parts on mechanisms to the ship state's angles and sizes each
/// engine's flame by its share of full thrust (hidden when it gives none).
pub fn update_parts(
    sim: Res<SimState>,
    ships: Query<&ShipVisual>,
    mut groups: Query<(&ShipGroup, &ChildOf, &mut Transform), Without<Flame>>,
    mut flames: Query<(&Flame, &ChildOf, &mut Transform, &mut Visibility), Without<ShipGroup>>,
) {
    let vessel = |parent: Entity| ships.get(parent).ok().and_then(|s| sim.fleet.get(s.index));
    for (group, child_of, mut t) in &mut groups {
        let (Some(k), Some(v)) = (group.mechanism, vessel(child_of.parent())) else { continue };
        let (Some(def), Some(state)) = (v.ship_def(), v.ship()) else { continue };
        let (Some(m), Some(s)) = (def.mechanisms.get(k), state.mechanisms.get(k)) else { continue };
        let want = parts::hinge(m.item.pos, m.item.axis, m.item.stowed_deg, s.angle);
        if *t != want {
            *t = want;
        }
    }
    let mut fractions: HashMap<Entity, Vec<f64>> = HashMap::new();
    let snap = sim.world.snapshot(sim.clock);
    for (flame, child_of, mut t, mut vis) in &mut flames {
        let parent = child_of.parent();
        let Some(v) = vessel(parent) else { continue };
        let i = ships.get(parent).map_or(usize::MAX, |s| s.index);
        let fraction = match flame.engine {
            None => plain_fraction(&sim, i),
            Some(e) => {
                let all = fractions.entry(parent).or_insert_with(|| {
                    let (anchor, r, _) = v.state_at(&sim.world, sim.clock);
                    let p = sim::forces::ambient_pressure(&sim.world, &snap, anchor, r);
                    match (v.ship_def(), v.engine_thrusts(throttle_of(&sim, i), p)) {
                        (Some(def), Some(thrusts)) => parts::engine_fractions(&def, &thrusts),
                        _ => Vec::new(),
                    }
                });
                all.get(e).copied().unwrap_or(0.0)
            }
        };
        let shown = fraction > 0.0;
        vis.set_if_neq(if shown { Visibility::Visible } else { Visibility::Hidden });
        if shown {
            *t = parts::flame(&flame.nozzle, fraction);
        }
    }
}

/// The test craft's engine's share of full thrust: a planned burn's full
/// thrust, else its setting at the throttle while it runs.
fn plain_fraction(sim: &SimState, i: usize) -> f64 {
    let Some(v) = sim.fleet.get(i) else { return 0.0 };
    if !crate::scene::engine_on(sim, i) {
        return 0.0;
    }
    let burning = v.trajectory().and_then(|tr| tr.segment_at(sim.clock)).is_some();
    if burning {
        1.0
    } else {
        v.craft.engine.setting(throttle_of(sim, i))
    }
}

/// The throttle vessel `i` flies with: the pilot's for the active vessel,
/// none for the others (they fly with passive controls).
fn throttle_of(sim: &SimState, i: usize) -> f64 {
    if i == sim.active {
        sim.controls.throttle
    } else {
        0.0
    }
}
