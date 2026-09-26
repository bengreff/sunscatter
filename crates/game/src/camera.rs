//! Camera: a KSP-style orbit view around a focus (the ship, or a body).
//!
//! The camera pose is computed every frame in f64 in the active ship's anchor
//! frame; the Bevy camera sits at the render origin and everything else is
//! placed relative to it (our floating origin: the engine only ever sees small
//! f32 camera-relative coordinates).

use crate::state::SimState;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use glam::DVec3;
use sim::frame::NodeId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Focus {
    Ship,
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
        }
    }
}

/// Zoom per trackpad pixel, in wheel lines (the prototype used 1.0, far too fast).
const TRACKPAD_LINES_PER_PIXEL: f64 = 0.25;

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

/// Mouse drag orbits, scroll zooms; F focuses the nearest body, backtick the ship.
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    egui: Res<EguiWantsInput>,
    sim: Res<SimState>,
    mut rig: ResMut<CameraRig>,
) {
    if !egui.wants_any_pointer_input() {
        if buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right) {
            rig.yaw -= f64::from(motion.delta.x) * 0.005;
            rig.pitch = (rig.pitch + f64::from(motion.delta.y) * 0.005).clamp(-1.55, 1.55);
        }
        if scroll.delta.y != 0.0 {
            // Trackpads report pixels, many per gesture: scale them to about a
            // quarter of a wheel line each so zooming is controllable.
            let lines = match scroll.unit {
                MouseScrollUnit::Line => f64::from(scroll.delta.y),
                MouseScrollUnit::Pixel => f64::from(scroll.delta.y) * TRACKPAD_LINES_PER_PIXEL,
            };
            rig.distance = (rig.distance * 1.15_f64.powf(-lines)).clamp(5.0, 5.0e12);
        }
    }
    if keys.just_pressed(KeyCode::Backquote) {
        rig.focus = Focus::Ship;
        rig.distance = 60.0;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        if let Some(body) = nearest_body(&sim) {
            focus_body_near_ship(&mut rig, &sim, body);
        }
    }
}

/// The surface body closest to the active ship (by altitude).
pub fn nearest_body(sim: &SimState) -> Option<NodeId> {
    let (anchor, r, _) = sim.ship().state(&sim.world);
    let snap = sim.world.snapshot(sim.clock);
    sim.world
        .surfaces()
        .map(|s| {
            let radius = s.physical.as_ref().map_or(0.0, |p| p.radius_eq);
            (s.node, (r - snap.relative(s.node, anchor).r).length() - radius)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(n, _)| n)
}

/// Focus a body with the camera placed just beyond the ship, looking at the
/// body along the body→ship line.
pub fn focus_body_near_ship(rig: &mut CameraRig, sim: &SimState, body: NodeId) {
    let (anchor, r, _) = sim.ship().state(&sim.world);
    let snap = sim.world.snapshot(sim.clock);
    let rel = r - snap.relative(body, anchor).r;
    let radius = sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(1.0e6, |p| p.radius_eq);
    let up = body_up(sim, body);
    let (e1, e2) = basis(up);
    let d = rel.normalize();
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

/// Two unit vectors perpendicular to `up` (and to each other).
fn basis(up: DVec3) -> (DVec3, DVec3) {
    let seed = if up.cross(DVec3::Z).length() > 1e-6 { DVec3::Z } else { DVec3::X };
    let e1 = up.cross(seed).normalize();
    (e1, up.cross(e1))
}

pub fn update(sim: Res<SimState>, mut rig: ResMut<CameraRig>, mut cam: Query<&mut Transform, With<MainCamera>>) {
    let (anchor, ship_r, _) = sim.ship().state(&sim.world);
    let snap = sim.world.snapshot(sim.clock);
    let (target, up) = match rig.focus {
        Focus::Ship => {
            let near = nearest_body(&sim).map_or(DVec3::ZERO, |b| snap.relative(b, anchor).r);
            (ship_r, (ship_r - near).normalize())
        }
        Focus::Body(b) => (snap.relative(b, anchor).r, body_up(&sim, b)),
    };
    let (e1, e2) = basis(up);
    let (cp, sp) = (rig.pitch.cos(), rig.pitch.sin());
    let d = (e1 * rig.yaw.cos() + e2 * rig.yaw.sin()) * cp + up * sp;
    rig.anchor = anchor;
    rig.cam_pos = target + d * rig.distance;
    rig.up = up;
    if let Ok(mut t) = cam.single_mut() {
        *t = Transform::IDENTITY.looking_to((-d).as_vec3(), up.as_vec3());
    }
}
