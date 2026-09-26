//! Camera: a KSP-style orbit view around a focus (the ship, or a body).
//!
//! The camera pose is computed every frame in f64 in the active ship's anchor
//! frame; the Bevy camera sits at the render origin and everything else is
//! placed relative to it (our floating origin: the engine only ever sees small
//! f32 camera-relative coordinates).

use crate::relations;
use crate::state::SimState;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use glam::DVec3;
use sim::ephem::Snapshot;
use sim::forces::altitude_above;
use sim::frame::NodeId;
use sim::world::World;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Focus {
    /// The active vessel.
    Ship,
    /// Another vessel by fleet index (the tracking station).
    Vessel(usize),
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
#[allow(clippy::too_many_arguments)]
pub fn read_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    egui: Res<EguiWantsInput>,
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
            rig.distance = zoom(rig.distance, lines, controls.wheel_zoom).max(min_distance(rig.focus));
        }
        // A rotation or zoom into a surface is refused: the camera stops.
        let new = (rig.yaw, rig.pitch, rig.distance);
        if new != old {
            let (anchor, target, up) = focus_frame(&sim, &rig);
            let snap = sim.world.snapshot(sim.clock);
            let clear = |(yaw, pitch, distance): (f64, f64, f64)| {
                let p = target + view_dir(up, yaw, pitch) * distance;
                clearance(&sim.world, &snap, anchor, p) - surface_margin(distance)
            };
            if !accept(clear(old), clear(new)) {
                (rig.yaw, rig.pitch, rig.distance) = old;
            }
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

/// Closest and farthest camera distances from the focus (m).
pub const MIN_DISTANCE: f64 = 5.0;
pub const MAX_DISTANCE: f64 = 5.0e12;
/// Closest camera distance to a vessel: outside its bounding sphere.
pub const VESSEL_MIN_DISTANCE: f64 = 1.2 * crate::map::VESSEL_RADIUS;

/// Closest allowed distance to a focus (a body's surface is handled by
/// [`clearance`] instead).
pub fn min_distance(focus: Focus) -> f64 {
    match focus {
        Focus::Ship | Focus::Vessel(_) => VESSEL_MIN_DISTANCE,
        Focus::Body(_) => MIN_DISTANCE,
    }
}

/// Height the camera keeps above any surface (m) at a focus distance.
pub fn surface_margin(distance: f64) -> f64 {
    (0.002 * distance).max(2.0)
}

/// Whether a camera move is allowed, from the clearance (m, negative when
/// too close) before and after: it stays clear, or it moves out of a
/// surface it was already too close to (the surface can move under it).
pub fn accept(old: f64, new: f64) -> bool {
    new >= 0.0 || new >= old
}

/// The distance along the view ray to place the camera: `distance` if clear
/// there, else the farthest clear point between `min` and `distance`, so a
/// surface rising behind the camera pushes it towards the focus. `clear(s)`
/// is the clearance at distance `s`. If the focus side is not clear either,
/// `distance` is kept (nothing better exists on this ray).
pub fn pull_in(distance: f64, min: f64, clear: impl Fn(f64) -> f64) -> f64 {
    if clear(distance) >= 0.0 || clear(min) < 0.0 {
        return distance;
    }
    let (mut lo, mut hi) = (min, distance);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if clear(mid) >= 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
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

/// Unit vector from the focus to the camera.
pub fn view_dir(up: DVec3, yaw: f64, pitch: f64) -> DVec3 {
    let (e1, e2) = basis(up);
    (e1 * yaw.cos() + e2 * yaw.sin()) * pitch.cos() + up * pitch.sin()
}

/// The focus distance after zooming in by `lines` wheel lines, each a factor
/// of `per_line`.
pub fn zoom(distance: f64, lines: f64, per_line: f64) -> f64 {
    (distance * per_line.powf(-lines)).clamp(MIN_DISTANCE, MAX_DISTANCE)
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
pub fn basis(up: DVec3) -> (DVec3, DVec3) {
    let seed = if up.cross(DVec3::Z).length() > 1e-6 { DVec3::Z } else { DVec3::X };
    let e1 = up.cross(seed).normalize();
    (e1, up.cross(e1))
}

/// The camera frame's anchor, the focus position in it and the camera's up.
fn focus_frame(sim: &SimState, rig: &CameraRig) -> (NodeId, DVec3, DVec3) {
    let (anchor, ship_r, _) = sim.ship().state_at(&sim.world, sim.clock);
    let snap = sim.world.snapshot(sim.clock);
    let (target, up) = match rig.focus {
        Focus::Ship => {
            let near = nearest_body(sim).map_or(DVec3::ZERO, |b| snap.relative(b, anchor).r);
            (ship_r, (ship_r - near).normalize())
        }
        Focus::Vessel(i) => {
            let (va, vr, _) = sim.fleet[i].state_at(&sim.world, sim.clock);
            let r = snap.relative_r(va, anchor) + vr;
            let near = nearest_body_to(sim, i).map_or(DVec3::ZERO, |b| snap.relative(b, anchor).r);
            (r, (r - near).normalize())
        }
        Focus::Body(b) => (snap.relative(b, anchor).r, body_up(sim, b)),
    };
    (anchor, target, up)
}

pub fn update(sim: Res<SimState>, mut rig: ResMut<CameraRig>, mut cam: Query<&mut Transform, With<MainCamera>>) {
    let (anchor, target, up) = focus_frame(&sim, &rig);
    let snap = sim.world.snapshot(sim.clock);
    let d = view_dir(up, rig.yaw, rig.pitch);
    // The focus moved a surface into the view ray: slide in, keeping the
    // chosen distance for when the way is clear again.
    let margin = surface_margin(rig.distance);
    let clear = |s: f64| clearance(&sim.world, &snap, anchor, target + d * s) - margin;
    let distance = pull_in(rig.distance, min_distance(rig.focus), clear);
    rig.anchor = anchor;
    rig.cam_pos = target + d * distance;
    rig.up = up;
    if let Ok(mut t) = cam.single_mut() {
        *t = Transform::IDENTITY.looking_to((-d).as_vec3(), up.as_vec3());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_table() {
        // (distance, lines, per line, expected)
        let cases = [
            (1000.0, 1.0, 1.072, 1000.0 / 1.072),
            (1000.0, -1.0, 1.072, 1072.0),
            (1000.0, 0.0, 1.072, 1000.0),
            (6.0, 10.0, 1.072, MIN_DISTANCE),
            (4.0e12, -10.0, 1.072, MAX_DISTANCE),
        ];
        for (d, lines, per_line, expected) in cases {
            assert!((zoom(d, lines, per_line) - expected).abs() < 1e-9 * expected, "{d} {lines}");
        }
        // Two lines at the new speed equal one at the old (1.15 per line).
        assert!((zoom(1000.0, 2.0, 1.072) / (1000.0 / 1.15) - 1.0).abs() < 1e-3);
    }

    /// Clearance above a 1,000 m sphere at the origin with a 200 m mountain
    /// around +x, minus the margin.
    fn world_clear(p: DVec3, distance: f64) -> f64 {
        let mountain = 200.0 * (p.normalize().dot(DVec3::X) - 0.9).max(0.0) * 10.0;
        p.length() - 1000.0 - mountain - surface_margin(distance)
    }

    #[test]
    fn zooming_or_rotating_into_terrain_is_refused() {
        // (case, clearance before, after, accepted)
        let cases = [
            ("clear to clear", 50.0, 20.0, true),
            ("zoom into the ground", 5.0, -1.0, false),
            ("already too close, moving out", -3.0, -1.0, true),
            ("already too close, moving in", -3.0, -4.0, false),
            ("exactly at the margin", 1.0, 0.0, true),
        ];
        for (case, old, new, expected) in cases {
            assert_eq!(accept(old, new), expected, "{case}");
        }
    }

    #[test]
    fn orbiting_below_a_mountain_stops_at_its_slope() {
        // Ship 20 m above the plain at +z; the camera swings down towards the
        // mountain at +x, 300 m away.
        let target = DVec3::new(0.0, 0.0, 1020.0);
        let up = DVec3::Z;
        let d = 300.0;
        let clear = |pitch: f64| world_clear(target + view_dir(up, 0.0, pitch) * d, d);
        let mut pitch: f64 = 0.8;
        let mut stopped = false;
        while pitch > -1.5 {
            let next = pitch - 0.01;
            if !accept(clear(pitch), clear(next)) {
                stopped = true;
                break;
            }
            pitch = next;
        }
        assert!(stopped, "the camera went into the ground");
        assert!(clear(pitch) >= 0.0 && pitch < 0.2, "stopped at pitch {pitch}");
    }

    #[test]
    fn a_rising_surface_pulls_the_camera_in() {
        let target = DVec3::new(0.0, 0.0, 1050.0);
        let dir = DVec3::new(0.6, 0.0, -0.8);
        let clear = |s: f64| world_clear(target + dir * s, 100.0);
        // 100 m out along a descending ray is underground; 10 m is not.
        assert!(clear(100.0) < 0.0 && clear(10.0) > 0.0);
        let s = pull_in(100.0, 10.0, clear);
        assert!(clear(s).abs() < 1e-6 && s < 100.0 && s > 10.0, "{s}");
        assert_eq!(pull_in(30.0, 10.0, |_| 1.0), 30.0, "clear: unchanged");
        assert_eq!(pull_in(30.0, 10.0, |_| -1.0), 30.0, "nothing clear on the ray");
    }

    #[test]
    fn the_camera_stays_outside_the_ship() {
        let d = zoom(20.0, 30.0, 1.072).max(min_distance(Focus::Ship));
        assert_eq!(d, 1.2 * crate::map::VESSEL_RADIUS);
        assert_eq!(min_distance(Focus::Body(NodeId(3))), MIN_DISTANCE);
    }

    #[test]
    fn margin_grows_with_distance() {
        assert_eq!(surface_margin(60.0), 2.0);
        assert_eq!(surface_margin(1.0e6), 2000.0);
    }
}
