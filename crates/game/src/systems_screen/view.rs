//! The systems screen's main view: the craft alone, no planets (D107). A
//! camera of its own renders render layer [`LAYER`] into an image the
//! screen shows (`super::ui`); the craft's parts are drawn there one by one
//! (each its own colour in the layer shown, the selection tinted, parts on
//! mechanisms turned from the ship state), lit by a light of the layer. The
//! camera orbits the craft's centre ([`rules::orbit_pose`]); a click picks
//! the part under it ([`rules::pick`]).

use super::rules::{self, Temps};
use super::{inspect::Selection, SystemsScreen};
use crate::scene::{parts, ShipModels};
use crate::state::SimState;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use sim::craft::{Craft, RenderMesh};
use std::sync::Arc;

/// The render layer of the systems screen's view.
pub const LAYER: usize = 1;
/// The view's background (sRGB).
const BACKGROUND: Color = Color::srgb(0.035, 0.045, 0.06);

/// The view's camera, image and what it shows.
#[derive(Resource)]
pub struct SystemsView {
    pub image: Handle<Image>,
    camera: Entity,
    root: Option<Entity>,
    /// The craft shown (its id) and its parts' entities, meshes and facts.
    craft: Option<String>,
    parts: Vec<(Entity, Handle<StandardMaterial>)>,
    meshes: Vec<Arc<RenderMesh>>,
    pub facts: Vec<rules::PartFacts>,
    /// The craft's centre (craft axes) and its size (m), framing the view.
    centre: Vec3,
    pub radius: f32,
    /// Each part's centre (craft axes), where its network lines end.
    centres: Vec<Vec3>,
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
pub fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None));
    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera { is_active: false, order: -1, clear_color: ClearColorConfig::Custom(BACKGROUND), ..default() },
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
    });
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
    ((lo + hi) / 2.0, ((hi - lo).length() / 2.0).max(1.0))
}

/// Draws `craft`'s parts under a new root, replacing the old.
fn rebuild(
    commands: &mut Commands,
    view: &mut SystemsView,
    craft: &Craft,
    models: &mut ShipModels,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if let Some(root) = view.root.take() {
        commands.entity(root).despawn();
    }
    view.meshes = (0..craft.parts.len()).map(|k| models.part_mesh(craft, k)).collect();
    let (centre, radius) = bounds(&view.meshes);
    (view.centre, view.radius) = (centre, radius);
    view.centres = view.meshes.iter().map(|m| bounds(std::slice::from_ref(m)).0).collect();
    // Craft axes into the view's (nose up), its centre at the origin.
    let turn = rules::craft_to_view();
    let root =
        commands.spawn((Transform::from_rotation(turn).with_translation(-(turn * centre)), Visibility::Visible)).id();
    view.parts = (0..craft.parts.len())
        .map(|k| {
            let material = materials.add(StandardMaterial { base_color: Color::WHITE, ..default() });
            let e = commands
                .spawn((
                    ViewPart(k),
                    Mesh3d(models.part_handle(craft, k, meshes)),
                    MeshMaterial3d(material.clone()),
                    RenderLayers::layer(LAYER),
                    Transform::IDENTITY,
                    ChildOf(root),
                ))
                .id();
            (e, material)
        })
        .collect();
    view.root = Some(root);
    view.craft = Some(craft.id.0.clone());
}

/// Keeps the view on the active vessel while the screen is open: its parts
/// (rebuilt when its craft changes), their layer colours and mechanisms,
/// the camera's pose and image size, and a click's pick.
#[allow(clippy::too_many_arguments)]
pub fn update(
    mut commands: Commands,
    sim: Res<SimState>,
    mut screen: ResMut<SystemsScreen>,
    mut view: ResMut<SystemsView>,
    mut models: ResMut<ShipModels>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<(&mut Camera, &mut Transform, &GlobalTransform), Without<ViewPart>>,
    mut parts: Query<(&ViewPart, &mut Transform, &GlobalTransform), Without<Camera>>,
    mut gizmos: Gizmos<NetworkGizmos>,
) {
    let Ok((mut camera, mut cam_t, cam_g)) = cameras.get_mut(view.camera) else { return };
    let vessel = sim.ship();
    let ship = vessel.ship_def().zip(vessel.ship());
    camera.is_active = screen.open && ship.is_some();
    let (true, Some((def, state))) = (screen.open, ship) else { return };
    let craft = sim::craft::craft_of(&vessel.craft.id);
    if view.craft.as_deref() != Some(craft.id.0.as_str()) {
        rebuild(&mut commands, &mut view, &craft, &mut models, &mut meshes, &mut materials);
        // A part keeps its selection through a new configuration (its id
        // stays); a component's index may name another after a split.
        let kept = |s: &Selection| matches!(s, Selection::Part(id) if craft.parts.iter().any(|p| p.id == *id));
        screen.selected = screen.selected.filter(kept);
    }
    // What each part is now, and its colour in the layer shown.
    let colours = parts::part_colours(&craft, &def, sim::materials::materials());
    let design = vessel.craft.design();
    let t = &vessel.thermal().state;
    let temps = Temps {
        skin: &t.skin,
        nodes: &t.nodes,
        cell_part: &design.cell_part,
        node_part: &design.node_part,
        skin_max: &design.skin_max,
        node_max: &design.node_max,
    };
    view.facts = rules::facts(&craft.parts, &colours, &def, state, Some(&temps), &vessel.knowledge());
    for (k, (_, material)) in view.parts.iter().enumerate() {
        let selected = screen.selected == Some(Selection::Part(craft.parts[k].id));
        let [r, g, b] = view.facts.get(k).map_or(rules::GHOST, |f| rules::colour(screen.layer, f, selected));
        let want = Color::linear_rgb(r, g, b);
        if materials.get(material).is_some_and(|m| m.base_color != want) {
            if let Some(mut m) = materials.get_mut(material) {
                m.base_color = want;
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
    let distance = screen.distance.unwrap_or(view.radius * 2.4);
    *cam_t = rules::orbit_pose(Vec3::ZERO, screen.yaw, screen.pitch, distance);
    // The image follows the view's size on screen (physical pixels).
    if let Some(size) = screen.view_px {
        if let Some(image) = images.get(&view.image) {
            if image.size() != size && size.x >= 16 && size.y >= 16 {
                if let Some(mut image) = images.get_mut(&view.image) {
                    image.resize(bevy::render::render_resource::Extent3d {
                        width: size.x,
                        height: size.y,
                        depth_or_array_layers: 1,
                    });
                }
            }
        }
    }
    // A click in the view: the part under it (or nothing: no selection).
    if let Some(at) = screen.click.take() {
        let Ok(ray) = camera.viewport_to_world(cam_g, at) else { return };
        let placed: Vec<(&RenderMesh, Transform)> = (parts.iter())
            .filter_map(|(p, _, g)| view.meshes.get(p.0).map(|m| (m.as_ref(), g.compute_transform())))
            .collect();
        let order: Vec<usize> = parts.iter().map(|(p, _, _)| p.0).collect();
        let hit = rules::pick(ray.origin, ray.direction.as_vec3(), &placed).map(|i| order[i]);
        screen.select(hit.and_then(|k| craft.parts.get(k)).map(|p| Selection::Part(p.id)));
    }
}
