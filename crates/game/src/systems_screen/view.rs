//! The systems screen's main view: the craft alone, no planets (D107). A
//! camera of its own renders render layer [`LAYER`] into an image the
//! screen shows (`super::ui`); the craft's parts are drawn there one by one
//! (each its own colour in the layer shown, parts on mechanisms turned from
//! the ship state), lit by a light of the layer. The camera orbits what it
//! frames ([`rules::orbit_pose`], [`rules::frame`]): the craft, or an
//! isolated part with the others ghosted. A click picks the part under it
//! ([`rules::pick_isolating`]) and isolates it.
//!
//! Each part is an entity (its mechanism's turn) with its drawings as
//! children: the plain mesh (the layers' colours, or the ghost) and, for
//! the thermal layer, the thermal network's skin cells, their boundaries
//! and the interior nodes ([`super::thermal`]), reading one texture of a
//! texel per cell and node that is rewritten as the temperatures change.

use super::rules::{self, Temps};
use super::thermal::{self, ThermalMesh};
use super::{inspect::Selection, SystemsScreen};
use crate::scene::{parts, ShipModels};
use crate::state::SimState;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use sim::craft::{Craft, RenderMesh};
use std::sync::Arc;

/// The render layer of the systems screen's view.
pub const LAYER: usize = 1;
/// The view's background (sRGB).
const BACKGROUND: Color = Color::srgb(0.035, 0.045, 0.06);
/// The camera's vertical field of view (rad; Bevy's default).
pub const FOV: f32 = std::f32::consts::FRAC_PI_4;
/// A ghosted part: what is not isolated (linear, with its opacity).
const GHOST: Color = Color::linear_rgba(0.45, 0.5, 0.58, 0.07);

/// The view's camera, image and what it shows.
#[derive(Resource)]
pub struct SystemsView {
    pub image: Handle<Image>,
    camera: Entity,
    root: Option<Entity>,
    /// The craft shown (its id) and its parts' entities, drawings, meshes
    /// and facts.
    craft: Option<String>,
    parts: Vec<PartView>,
    meshes: Vec<Arc<RenderMesh>>,
    pub facts: Vec<rules::PartFacts>,
    /// The craft's centre (craft axes) and its size (m), framing the view.
    centre: Vec3,
    pub radius: f32,
    /// Each part's centre (craft axes) and radius: its network lines' ends
    /// and its framing when isolated.
    centres: Vec<Vec3>,
    radii: Vec<f32>,
    /// What the camera looks at now (view axes) and from how far, eased
    /// toward the framing; the radius of what it frames.
    look: Option<(Vec3, f32)>,
    pub framed_radius: f32,
    pub distance_now: f32,
    /// The thermal layer: its mesh, texture and materials.
    thermal: ThermalMesh,
    texture: Handle<Image>,
    texels: Vec<u8>,
    ghost: Handle<StandardMaterial>,
}

/// A part's drawings: its plain mesh and own material, and the thermal
/// layer's skin, boundaries and nodes.
struct PartView {
    plain: Entity,
    material: Handle<StandardMaterial>,
    thermal: [Entity; 3],
}

/// A drawn part of the view (an index into the craft's parts).
#[derive(Component)]
pub struct ViewPart(usize);

/// The view's network lines (plumbing, buses, data links), drawn over the
/// parts on the view's render layer.
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct NetworkGizmos;

/// The network lines' layer, width and place over the parts.
pub fn setup_gizmos(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<NetworkGizmos>();
    config.render_layers = RenderLayers::layer(LAYER);
    config.depth_bias = -1.0;
    config.line.width = 2.5;
}

/// The camera, its image and its light (inactive until the screen opens).
pub fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let image = images.add(Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None));
    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera { is_active: false, order: -1, clear_color: ClearColorConfig::Custom(BACKGROUND), ..default() },
            Projection::Perspective(PerspectiveProjection { fov: FOV, ..default() }),
            RenderTarget::from(image.clone()),
            Tonemapping::None,
            AmbientLight { color: Color::WHITE, brightness: 350.0, affects_lightmapped_meshes: false },
            RenderLayers::layer(LAYER),
            Transform::from_xyz(0.0, 0.0, 100.0),
        ))
        .id();
    commands.spawn((
        DirectionalLight { illuminance: 900.0, shadow_maps_enabled: false, ..default() },
        RenderLayers::layer(LAYER),
        Transform::from_xyz(-3.0, 4.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let texture = images.add(texture_image(1, 1, vec![0; 4]));
    let ghost =
        materials.add(StandardMaterial { base_color: GHOST, alpha_mode: AlphaMode::Blend, unlit: true, ..default() });
    commands.insert_resource(SystemsView {
        image,
        camera,
        root: None,
        craft: None,
        parts: Vec::new(),
        meshes: Vec::new(),
        facts: Vec::new(),
        centre: Vec3::ZERO,
        radius: 10.0,
        centres: Vec::new(),
        radii: Vec::new(),
        look: None,
        framed_radius: 10.0,
        distance_now: 24.0,
        thermal: ThermalMesh::default(),
        texture,
        texels: Vec::new(),
        ghost,
    });
}

/// The thermal texture: a texel per cell and node, sampled exactly.
fn texture_image(width: u32, height: u32, data: Vec<u8>) -> Image {
    let size = Extent3d { width, height, depth_or_array_layers: 1 };
    let mut image =
        Image::new(size, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    image.sampler = ImageSampler::nearest();
    image
}

/// The centre of the parts' meshes' bounds and the radius around it.
fn bounds(meshes: &[Arc<RenderMesh>]) -> (Vec3, f32) {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in meshes.iter().flat_map(|m| m.positions.iter()) {
        lo = lo.min(Vec3::from_array(*p));
        hi = hi.max(Vec3::from_array(*p));
    }
    if lo.x > hi.x {
        return (Vec3::ZERO, 10.0);
    }
    ((lo + hi) / 2.0, ((hi - lo).length() / 2.0).max(0.25))
}

/// A Bevy mesh of triangles reading the thermal texture.
fn uv_mesh(m: &thermal::UvMesh) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, m.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, m.uvs.clone())
        .with_inserted_indices(Indices::U32(m.indices.clone()))
}

/// A Bevy mesh of line segments reading the thermal texture.
fn line_mesh(l: &thermal::Lines) -> Mesh {
    Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, l.positions.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, l.uvs.clone())
}

/// Draws `craft`'s parts under a new root, replacing the old.
#[allow(clippy::too_many_arguments)]
fn rebuild(
    commands: &mut Commands,
    view: &mut SystemsView,
    craft: &Craft,
    design: &sim::craft::design::CraftDesign,
    models: &mut ShipModels,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) {
    if let Some(root) = view.root.take() {
        commands.entity(root).despawn();
    }
    view.meshes = (0..craft.parts.len()).map(|k| models.part_mesh(craft, k)).collect();
    let (centre, radius) = bounds(&view.meshes);
    (view.centre, view.radius) = (centre, radius);
    (view.centres, view.radii) = view.meshes.iter().map(|m| bounds(std::slice::from_ref(m))).unzip();
    view.look = None;
    // The thermal network as drawn: the simulation's own cells and nodes.
    let centroids: Vec<Vec3> = craft.cells.cells.iter().map(|c| c.centroid.as_vec3()).collect();
    view.thermal = thermal::build(
        &craft.surface,
        &craft.cells.tri_cell,
        &centroids,
        &design.cell_part,
        &design.volume,
        &design.node_part,
        craft.parts.len(),
    );
    let tex = view.thermal.texture.unwrap_or(thermal::Texture::new(0, 0));
    view.texels = vec![0; tex.texels() * 4];
    view.texture = images.add(texture_image(tex.width, tex.height, view.texels.clone()));
    let surface = materials.add(StandardMaterial {
        base_color_texture: Some(view.texture.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        double_sided: true,
        cull_mode: None,
        perceptual_roughness: 0.9,
        ..default()
    });
    let lines = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(0.18, 0.18, 0.18),
        base_color_texture: Some(view.texture.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        unlit: true,
        ..default()
    });
    // Craft axes into the view's (nose up), its centre at the origin.
    let turn = rules::craft_to_view();
    let root =
        commands.spawn((Transform::from_rotation(turn).with_translation(-(turn * centre)), Visibility::Visible)).id();
    let layer = RenderLayers::layer(LAYER);
    view.parts = (0..craft.parts.len())
        .map(|k| {
            let material = materials.add(StandardMaterial { base_color: Color::WHITE, ..default() });
            let part = commands.spawn((ViewPart(k), Transform::IDENTITY, Visibility::Visible, ChildOf(root))).id();
            let child = |commands: &mut Commands, mesh: Handle<Mesh>, m: Handle<StandardMaterial>| {
                commands
                    .spawn((Mesh3d(mesh), MeshMaterial3d(m), layer.clone(), Transform::IDENTITY, ChildOf(part)))
                    .id()
            };
            let plain = child(commands, models.part_handle(craft, k, meshes), material.clone());
            let t = &view.thermal.parts[k];
            let thermal = [
                child(commands, meshes.add(uv_mesh(&t.skin)), surface.clone()),
                child(commands, meshes.add(line_mesh(&t.lines)), lines.clone()),
                child(commands, meshes.add(uv_mesh(&t.nodes)), surface.clone()),
            ];
            PartView { plain, material, thermal }
        })
        .collect();
    view.root = Some(root);
    view.craft = Some(craft.id.0.clone());
}

/// Each part's layer facts on `vessel` now ([`rules::facts`]: its colour,
/// joint, temperatures and what it measures), as the view keeps them while
/// the screen is open: one builder for the screen and the agent's
/// `read {layer}` (`agent::read`). None for a craft without a ship
/// definition.
pub fn facts_of(vessel: &sim::vessel::Vessel) -> Option<Vec<rules::PartFacts>> {
    let (def, state) = vessel.ship_def().zip(vessel.ship())?;
    let craft = sim::craft::craft_of(&vessel.craft.id);
    Some(facts_with(&craft, &def, state, &Temps::of(vessel), &vessel.knowledge()))
}

/// [`facts_of`] from its pieces (the view keeps the temperatures and the
/// knowledge for the thermal texture).
fn facts_with(
    craft: &Craft,
    def: &sim::craft::ship::ShipDef,
    state: &sim::craft::state::ShipState,
    temps: &Temps,
    known: &sim::systems::Knowledge,
) -> Vec<rules::PartFacts> {
    let colours = parts::part_colours(craft, def, sim::materials::materials());
    rules::facts(&craft.parts, &colours, def, state, Some(temps), known)
}

/// Keeps the view on the active vessel while the screen is open: its parts
/// (rebuilt when its craft changes), their layer colours, ghosts and
/// mechanisms, the thermal texture, the camera's framing and image size,
/// and a click's pick.
#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    sim: Res<SimState>,
    time: Res<Time<Real>>,
    mut screen: ResMut<SystemsScreen>,
    mut view: ResMut<SystemsView>,
    mut models: ResMut<ShipModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<(&mut Camera, &mut Transform, &GlobalTransform), Without<ViewPart>>,
    mut parts: Query<(&ViewPart, &mut Transform, &GlobalTransform), Without<Camera>>,
    mut drawn: Query<(&mut Visibility, &mut MeshMaterial3d<StandardMaterial>), Without<ViewPart>>,
    mut gizmos: Gizmos<NetworkGizmos>,
) {
    let Ok((mut camera, mut cam_t, cam_g)) = cameras.get_mut(view.camera) else { return };
    let vessel = sim.ship();
    let ship = vessel.ship_def().zip(vessel.ship());
    camera.is_active = screen.open && ship.is_some();
    let (true, Some((def, state))) = (screen.open, ship) else { return };
    let craft = sim::craft::craft_of(&vessel.craft.id);
    let design = vessel.craft.design();
    if view.craft.as_deref() != Some(craft.id.0.as_str()) {
        rebuild(&mut commands, &mut view, &craft, design, &mut models, &mut meshes, &mut materials, &mut images);
        // A part keeps its selection through a new configuration (its id
        // stays); a component's index may name another after a split.
        let kept = |s: &Selection| matches!(s, Selection::Part(id) if craft.parts.iter().any(|p| p.id == *id));
        if screen.selected.is_some_and(|s| !kept(&s)) {
            screen.back();
        }
        return;
    }
    // What each part is now, and its colour in the layer shown.
    let temps = Temps::of(vessel);
    let known = vessel.knowledge();
    view.facts = facts_with(&craft, &def, state, &temps, &known);
    // The isolated part (the selection's), if any.
    let isolated_id = screen.back.and(screen.selected).and_then(|s| match s {
        Selection::Part(id) => Some(id),
        Selection::Component(i) => def.components.get(i).map(|c| c.part),
    });
    let isolated = isolated_id.and_then(|id| craft.parts.iter().position(|p| p.id == id));
    let thermal_layer = screen.layer == rules::Layer::Thermal;
    for (k, pv) in view.parts.iter().enumerate() {
        let ghost = isolated.is_some_and(|i| i != k);
        let [r, g, b] = view.facts.get(k).map_or(rules::GHOST, |f| rules::colour(screen.layer, f));
        let want = Color::linear_rgb(r, g, b);
        if materials.get(&pv.material).is_some_and(|m| m.base_color != want) {
            if let Some(mut m) = materials.get_mut(&pv.material) {
                m.base_color = want;
            }
        }
        if let Ok((mut vis, mut mat)) = drawn.get_mut(pv.plain) {
            vis.set_if_neq(shown(ghost || !thermal_layer));
            let m = if ghost { view.ghost.clone() } else { pv.material.clone() };
            if mat.0 != m {
                mat.0 = m;
            }
        }
        for e in pv.thermal {
            if let Ok((mut vis, _)) = drawn.get_mut(e) {
                vis.set_if_neq(shown(thermal_layer && !ghost));
            }
        }
    }
    let deployed = craft.id.deployed();
    for (part, mut t, _) in &mut parts {
        let id = craft.parts.get(part.0).map(|p| p.id);
        let found = def.mechanisms.iter().enumerate().find(|(_, m)| Some(m.part) == id);
        let want = match found.and_then(|(i, m)| state.mechanisms.get(i).map(|s| (m, s))) {
            Some((m, s)) => {
                parts::hinge(m.item.pos, m.item.axis, parts::built_angle(&m.item, deployed.contains(&m.part)), s.angle)
            }
            None => Transform::IDENTITY,
        };
        if *t != want {
            *t = want;
        }
    }
    // The layer's networks, from item to item (a component's place, else its
    // part's centre), into the view's axes as the parts' root places them.
    let turn = rules::craft_to_view();
    let at = |id: &str, part: u32| -> Vec3 {
        let own = def.components.iter().find(|c| c.id == id).map(|c| c.pose.pos.as_vec3());
        let k = craft.parts.iter().position(|p| p.id == part);
        own.or_else(|| k.and_then(|k| view.centres.get(k).copied())).unwrap_or(view.centre)
    };
    let [r, g, b] = rules::network_colour(screen.layer);
    for (a, b2) in rules::networks(screen.layer, &def, &at) {
        gizmos.line(turn * (a - view.centre), turn * (b2 - view.centre), Color::linear_rgb(r, g, b));
    }
    // What the camera frames: the isolated part where its drawing is now
    // (its mechanism turned), else the craft; eased there.
    let aspect = screen.view_px.map_or(16.0 / 9.0, |p| p.x as f32 / p.y.max(1) as f32);
    let (target, radius) = match isolated {
        Some(k) => {
            let at = parts.iter().find(|(p, _, _)| p.0 == k).map(|(_, _, g)| g.transform_point(view.centres[k]));
            (at.unwrap_or(Vec3::ZERO), view.radii[k])
        }
        None => (Vec3::ZERO, view.radius),
    };
    let distance = screen.distance.unwrap_or_else(|| rules::frame(radius, FOV / 2.0, aspect));
    let look = rules::ease(view.look.unwrap_or((target, distance)), (target, distance), time.delta_secs());
    view.look = Some(look);
    (view.framed_radius, view.distance_now) = (radius, look.1);
    *cam_t = rules::orbit_pose(look.0, screen.yaw, screen.pitch, look.1);
    // The thermal texture: each cell and node by its own temperature where
    // measured, the cut-away facing the camera.
    if thermal_layer {
        let measured: Vec<bool> =
            craft.parts.iter().map(|p| known.knows(sim::systems::Quantity::PartTemperature(p.id))).collect();
        let reading = thermal::Reading { temps: &temps, measured: &measured };
        paint_thermal(&mut view, &mut images, &screen, isolated, design, &reading);
    }
    // The image follows the view's size on screen (physical pixels).
    if let Some(size) = screen.view_px {
        if let Some(image) = images.get(&view.image) {
            if image.size() != size && size.x >= 16 && size.y >= 16 {
                if let Some(mut image) = images.get_mut(&view.image) {
                    image.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
                }
            }
        }
    }
    // A click in the view isolates the part under it.
    if let Some(at) = screen.click.take() {
        if let Ok(ray) = camera.viewport_to_world(cam_g, at) {
            let placed: Vec<(usize, &RenderMesh, Transform)> = (parts.iter())
                .filter_map(|(p, _, g)| view.meshes.get(p.0).map(|m| (p.0, m.as_ref(), g.compute_transform())))
                .collect();
            click(&mut screen, ray, &placed, isolated, &craft);
        }
    }
}

/// The thermal texture: each cell and node by its own temperature where
/// measured, the cut-away facing the camera across what is framed (the
/// isolated part, else the craft); written only when it changes.
fn paint_thermal(
    view: &mut SystemsView,
    images: &mut Assets<Image>,
    screen: &SystemsScreen,
    isolated: Option<usize>,
    design: &sim::craft::design::CraftDesign,
    reading: &thermal::Reading,
) {
    // A longitudinal cut: the plane holds the craft's long axis (the view's
    // vertical) and faces the camera's side.
    let toward = rules::craft_to_view().inverse() * Vec3::new(screen.yaw.sin(), 0.0, screen.yaw.cos());
    let framed = |owner: &[u32], i: usize| isolated.is_none_or(|k| owner.get(i).is_some_and(|&p| p as usize == k));
    let th = &view.thermal;
    let cells = (0..th.cell_centres.len()).filter(|&i| framed(&design.cell_part, i)).map(|i| th.cell_centres[i]);
    let nodes = (0..th.node_centres.len()).filter(|&i| framed(&design.node_part, i)).map(|i| th.node_centres[i]);
    let cut = thermal::Cut::across(cells.chain(nodes), toward, screen.cut);
    let mut texels = std::mem::take(&mut view.texels);
    thermal::texels(&view.thermal, reading, Some(&cut), &mut texels);
    if images.get(&view.texture).is_some_and(|i| i.data.as_deref() != Some(texels.as_slice())) {
        if let Some(mut image) = images.get_mut(&view.texture) {
            image.data = Some(texels.clone());
        }
    }
    view.texels = texels;
}

/// A click's ray picks a part (`placed`: each part's index, mesh and
/// transform): the isolated part first (a click on it keeps it, putting
/// a component's selection back to its part), else the nearest, which is
/// isolated; a click on nothing changes nothing.
fn click(
    screen: &mut SystemsScreen,
    ray: Ray3d,
    placed: &[(usize, &RenderMesh, Transform)],
    isolated: Option<usize>,
    craft: &Craft,
) {
    let meshes: Vec<(&RenderMesh, Transform)> = placed.iter().map(|(_, m, t)| (*m, *t)).collect();
    let first = isolated.and_then(|k| placed.iter().position(|(o, _, _)| *o == k));
    let hit = rules::pick_isolating(ray.origin, ray.direction.as_vec3(), &meshes, first).map(|i| placed[i].0);
    match hit.and_then(|k| craft.parts.get(k)) {
        Some(p) if isolated == hit && matches!(screen.selected, Some(Selection::Component(_))) => {
            screen.select(Some(Selection::Part(p.id)));
        }
        Some(p) if isolated != hit => screen.select(Some(Selection::Part(p.id))),
        _ => {}
    }
}

/// Visible or hidden.
fn shown(on: bool) -> Visibility {
    if on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    }
}
