//! Camera: a KSP-style orbit view around a focus (the ship, or a body).
//!
//! The camera pose is computed every frame in f64 in the active ship's anchor
//! frame; the Bevy camera sits at the render origin and everything else is
//! placed relative to it (our floating origin: the engine only ever sees small
//! f32 camera-relative coordinates).

use crate::commands::{InputContext, Keys};
use crate::relations;
use crate::state::SimState;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use glam::DVec3;
use sim::ephem::Snapshot;
use sim::forces::altitude_above;
use sim::frame::NodeId;
use sim::time::Epoch;
use sim::vessel::Vessel;
use sim::world::World;

mod rules;
pub use rules::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Focus {
    /// The active vessel.
    Ship,
    /// Another vessel by fleet index (the tracking station).
    Vessel(sim::vessel::VesselId),
    Body(NodeId),
}

#[derive(Resource)]
pub struct CameraRig {
    pub focus: Focus,
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
    /// Computed each frame: the frame's anchor, camera position within it,
    /// and the camera's up vector.
    pub anchor: NodeId,
    pub cam_pos: DVec3,
    pub up: DVec3,
    /// The horizontal direction yaw is measured from, perpendicular to `up`:
    /// carried from frame to frame ([`carry_reference`]), reset to
    /// [`basis`] when the focus changes.
    pub reference: DVec3,
    /// Where `up` comes from, and where it is turning from while the
    /// reference body changes (`up_blend` of the turn done, 0..1).
    pub up_ref: UpRef,
    pub up_from: UpRef,
    pub up_blend: f64,
    /// The focus and sim time of the last frame.
    pub last: Option<(Focus, Epoch)>,
}

/// What the camera's up is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpRef {
    /// Away from a body's centre (a vessel focus: the local vertical).
    Away(NodeId),
    /// A body's pole (a body focus).
    Pole(NodeId),
    /// Inertial +Z (no body).
    Fixed,
}

impl Default for CameraRig {
    fn default() -> Self {
        CameraRig {
            focus: Focus::Ship,
            yaw: 0.6,
            pitch: 0.25,
            distance: 60.0,
            anchor: NodeId(0),
            cam_pos: DVec3::ZERO,
            up: DVec3::Z,
            reference: DVec3::X,
            up_ref: UpRef::Fixed,
            up_from: UpRef::Fixed,
            up_blend: 1.0,
            last: None,
        }
    }
}

#[derive(Component)]
pub struct MainCamera;

/// Fixed exposure: sunlit surfaces read well; night sides are dark.
pub const EV100: f32 = 14.5;

pub fn setup(mut commands: Commands, demo: Option<ResMut<crate::demo::Demo>>, mut images: ResMut<Assets<Image>>) {
    let mut cam = commands.spawn((
        MainCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { near: 0.1, far: 1.0e13, ..default() }),
        // Physical light units: sunlight ~128,000 lux, sunny-16 exposure.
        bevy::camera::Hdr,
        bevy::camera::Exposure { ev100: EV100 },
        Transform::IDENTITY,
    ));
    if let Some(mut demo) = demo {
        if std::env::var_os("SUNSCATTER_DEMO_OFFSCREEN").is_some() {
            use bevy::render::render_resource::TextureFormat;
            let image = Image::new_target_texture(1600, 900, TextureFormat::Rgba8UnormSrgb, None);
            let handle = images.add(image);
            cam.insert(bevy::camera::RenderTarget::from(handle.clone()));
            demo.offscreen = Some(handle);
        }
    }
}

/// Mouse drag orbits, scroll zooms; F focuses the nearest body, backtick the
/// ship (in flight only, `InputContext`).
#[allow(clippy::too_many_arguments)]
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    egui: Res<EguiWantsInput>,
    ctx: Res<InputContext>,
    sim: Res<SimState>,
    controls: Res<crate::persist::ControlsSettings>,
    mut rig: ResMut<CameraRig>,
) {
    if !egui.wants_any_pointer_input() {
        let old = (rig.yaw, rig.pitch, rig.distance);
        if buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right) {
            let k = controls.mouse_sensitivity;
            let dy = if controls.invert_y { -motion.delta.y } else { motion.delta.y };
            rig.yaw -= f64::from(motion.delta.x) * k;
            rig.pitch = (rig.pitch + f64::from(dy) * k).clamp(-1.55, 1.55);
        }
        if scroll.delta.y != 0.0 {
            // Trackpads report pixels, many per gesture: scale them to a
            // fraction of a wheel line each so zooming is controllable.
            let lines = match scroll.unit {
                MouseScrollUnit::Line => f64::from(scroll.delta.y),
                MouseScrollUnit::Pixel => f64::from(scroll.delta.y) * controls.trackpad_lines_per_px,
            };
            let min = min_distance(rig.focus, focus_radius(&sim, rig.focus));
            rig.distance = zoom(rig.distance, lines, controls.wheel_zoom).max(min);
        }
        // A rotation or zoom into a surface is refused: the camera stops.
        let new = (rig.yaw, rig.pitch, rig.distance);
        if new != old {
            let (anchor, target, up) = focus_frame(&sim, &rig);
            let snap = sim.world.snapshot(sim.clock);
            let e1 = carry_reference(rig.reference, up, None);
            let clear = |(yaw, pitch, distance): (f64, f64, f64)| {
                let p = target + view_dir(e1, up, yaw, pitch) * distance;
                clearance(&sim.world, &snap, anchor, p) - surface_margin(distance)
            };
            if !accept(clear(old), clear(new)) {
                (rig.yaw, rig.pitch, rig.distance) = old;
            }
        }
    }
    if !ctx.allows(Keys::Flight) {
        return;
    }
    if keys.just_pressed(KeyCode::Backquote) {
        rig.focus = Focus::Ship;
        // The per-frame placement keeps it clear of the ground.
        rig.distance = default_distance(focus_radius(&sim, Focus::Ship));
    }
    if keys.just_pressed(KeyCode::KeyF) {
        if let Some(body) = nearest_body(&sim) {
            focus_body_near_ship(&mut rig, &sim, body);
        }
    }
}

/// Bounding radius of a craft about its centre of mass (m): its farthest
/// contact point (feet and hull points).
pub fn craft_radius(v: &Vessel) -> f64 {
    let com = v.mass_props().com;
    let r = v.craft.contacts.iter().map(|c| (c.pos - com).length()).fold(0.0, f64::max);
    if r > 0.0 {
        r
    } else {
        crate::map::VESSEL_RADIUS
    }
}

/// The bounding radius of the focused craft (0 for a body).
fn focus_radius(sim: &SimState, focus: Focus) -> f64 {
    match focus {
        Focus::Ship => craft_radius(sim.ship()),
        Focus::Vessel(id) => sim.index_of(id).map_or(crate::map::VESSEL_RADIUS, |i| craft_radius(&sim.fleet[i])),
        Focus::Body(_) => 0.0,
    }
}

/// Height of point `p` (relative to `anchor`) above the nearest surface:
/// the terrain of solid bodies (sim's surface, the one physics and rendering
/// use) and the sphere of the others.
pub fn clearance(world: &World, snap: &Snapshot, anchor: NodeId, p: DVec3) -> f64 {
    world
        .sources
        .iter()
        .filter_map(|s| {
            let body = s.physical.as_ref()?;
            let d = (p - snap.relative_r(s.node, anchor)).length() - body.radius_eq;
            // Terrain only matters near the surface (it is under 20 km high).
            Some(if body.solid && d < 50_000.0 { altitude_above(world, snap, anchor, s.node, p).0 } else { d })
        })
        .fold(f64::INFINITY, f64::min)
}

/// The body nearest the active ship (see [`relations::nearest_body`]).
pub fn nearest_body(sim: &SimState) -> Option<NodeId> {
    nearest_body_to(sim, sim.active)
}

/// The body nearest vessel `i`.
pub fn nearest_body_to(sim: &SimState, i: usize) -> Option<NodeId> {
    let (anchor, r, _) = sim.fleet[i].state_at(&sim.world, sim.clock);
    relations::nearest_body(&sim.world, sim.clock, anchor, r)
}

/// Focus a body with the camera placed just beyond the ship, looking at the
/// body along the body→ship line.
pub fn focus_body_near_ship(rig: &mut CameraRig, sim: &SimState, body: NodeId) {
    let (anchor, r, _) = sim.ship().state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let rel = r - snap.relative(body, anchor).r;
    let radius = sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(1.0e6, |p| p.radius_eq);
    let up = body_up(sim, body);
    // The reference the next frame starts from (a new focus: `basis`).
    let (e1, e2) = basis(up);
    let d = safe_dir(rel, e1);
    rig.pitch = d.dot(up).clamp(-1.0, 1.0).asin();
    rig.yaw = d.dot(e2).atan2(d.dot(e1));
    rig.distance = rel.length() + 0.15 * radius;
    rig.focus = Focus::Body(body);
}

/// Focus a body from a moderate distance (from the body menu).
pub fn focus_body(rig: &mut CameraRig, sim: &SimState, body: NodeId) {
    focus_body_near_ship(rig, sim, body);
    let radius = sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(1.0e6, |p| p.radius_eq);
    rig.distance = rig.distance.min(4.0 * radius).max(1.5 * radius);
}

fn body_up(sim: &SimState, body: NodeId) -> DVec3 {
    sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(DVec3::Z, |p| p.rotation.pole(sim.clock))
}

/// The camera frame's anchor, the focus position in it and where its up
/// comes from.
fn focus_target(sim: &SimState, focus: Focus) -> (NodeId, DVec3, UpRef) {
    let (anchor, ship_r, _) = sim.ship().state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let away = |b: Option<NodeId>| b.map_or(UpRef::Fixed, UpRef::Away);
    match focus {
        Focus::Ship => (anchor, ship_r, away(nearest_body(sim))),
        Focus::Vessel(id) => {
            let i = sim.index_of(id).unwrap_or(sim.active);
            let (va, vr, _) = sim.fleet[i].state_at(&sim.world, sim.clock);
            (anchor, snap.relative_r(va, anchor) + vr, away(nearest_body_to(sim, i)))
        }
        Focus::Body(b) => (anchor, snap.relative(b, anchor).r, UpRef::Pole(b)),
    }
}

/// The up an [`UpRef`] gives for a focus at `target` (relative to `anchor`).
fn up_of(sim: &SimState, snap: &Snapshot, anchor: NodeId, target: DVec3, up: UpRef) -> DVec3 {
    match up {
        UpRef::Away(b) => safe_dir(target - snap.relative_r(b, anchor), body_up(sim, b)),
        UpRef::Pole(b) => body_up(sim, b),
        UpRef::Fixed => DVec3::Z,
    }
}

/// The camera frame's anchor, the focus position in it and the camera's up
/// (turning between reference bodies as the rig says).
fn focus_frame(sim: &SimState, rig: &CameraRig) -> (NodeId, DVec3, DVec3) {
    let (anchor, target, _) = focus_target(sim, rig.focus);
    let snap = sim.world.snapshot(sim.clock);
    let to = up_of(sim, &snap, anchor, target, rig.up_ref);
    let up = if rig.up_blend >= 1.0 {
        to
    } else {
        blended_up(up_of(sim, &snap, anchor, target, rig.up_from), to, rig.up_blend)
    };
    (anchor, target, up)
}

/// The rotation of the ground under the focus since `since` (axis, angle),
/// when the focus is low over the body its up comes from.
fn ground_spin(sim: &SimState, rig: &CameraRig, target: DVec3, anchor: NodeId, since: Epoch) -> Option<(DVec3, f64)> {
    let UpRef::Away(b) = rig.up_ref else { return None };
    let p = sim.world.source(b)?.physical.as_ref()?;
    let altitude = (target - sim.world.snapshot(sim.clock).relative_r(b, anchor)).length() - p.radius_eq;
    co_rotates(altitude, p.radius_eq)
        .then(|| (p.rotation.pole(sim.clock), p.rotation.w_rate * sim.clock.seconds_since(since)))
}

pub fn update(
    sim: Res<SimState>,
    time: Res<Time>,
    mut rig: ResMut<CameraRig>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let exists = match rig.focus {
        Focus::Vessel(id) => sim.index_of(id).is_some(),
        _ => true,
    };
    rig.focus = resolve_focus(rig.focus, exists);
    let (anchor, target, up_ref) = focus_target(&sim, rig.focus);
    let last = rig.last;
    let new_focus = last.is_none_or(|(f, _)| f != rig.focus);
    if new_focus {
        (rig.up_ref, rig.up_from, rig.up_blend) = (up_ref, up_ref, 1.0);
    } else if up_ref != rig.up_ref {
        // A new nearest body: turn to it rather than snap.
        (rig.up_from, rig.up_ref, rig.up_blend) = (rig.up_ref, up_ref, 0.0);
    } else {
        rig.up_blend = (rig.up_blend + time.delta_secs_f64() / UP_BLEND_SECONDS).min(1.0);
    }
    let (_, _, up) = focus_frame(&sim, &rig);
    rig.reference = match last {
        Some((_, since)) if !new_focus => {
            let spin = ground_spin(&sim, &rig, target, anchor, since);
            carry_reference(rig.reference, up, spin)
        }
        _ => basis(up).0,
    };
    rig.last = Some((rig.focus, sim.clock));
    let snap = sim.world.snapshot(sim.clock);
    let d = view_dir(rig.reference, up, rig.yaw, rig.pitch);
    // The focus moved a surface into the view ray: slide in, keeping the
    // chosen distance for when the way is clear again; with nothing clear
    // on the ray, rise above the ground.
    let margin = surface_margin(rig.distance);
    let clear = |p: DVec3| clearance(&sim.world, &snap, anchor, p) - margin;
    let min = min_distance(rig.focus, focus_radius(&sim, rig.focus));
    let cam_pos = place(target, d, rig.distance, min, up, clear);
    rig.anchor = anchor;
    rig.cam_pos = cam_pos;
    rig.up = up;
    if let Ok(mut t) = cam.single_mut() {
        let look = safe_dir(target - cam_pos, -d);
        *t = Transform::IDENTITY.looking_to(look.as_vec3(), up.as_vec3());
    }
}
