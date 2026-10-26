//! Body tour (`SUNSCATTER_TOUR=<body,body,…>` with `SUNSCATTER_DEMO=<dir>`,
//! usually `SUNSCATTER_DEMO_OFFSCREEN=1`): screenshots of each listed body
//! in the Minimal tier, with the clock frozen. Per body: the whole disc
//! sunlit from 3 radii (`disc`), the limb from low orbit (`limb`: 1.5 % of
//! the radius up, or just above the atmosphere's top), and the ground from
//! 2 km at a low Sun (`ground`, solid bodies only). Each shot waits for the
//! terrain and its maps to settle (`Terrain::busy`).
//!
//! The demo's capture machinery writes the files (`<n>_<body>_<view>.png`);
//! the camera pose is a pure function of the body's Sun direction and pole
//! ([`pose`]), applied after the camera rig each frame ([`aim`]).

use crate::camera::{self, CameraRig, Focus, MainCamera};
use crate::demo::{Demo, Step};
use crate::scene::BodyDefs;
use crate::settings::{GraphicsSettings, Tier};
use crate::state::{SimPause, SimState};
use crate::terrain::Terrain;
use bevy::app::AppExit;
use bevy::prelude::*;
use glam::DVec3;
use sim::frame::NodeId;

/// One of the tour's views of a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Disc,
    Limb,
    Ground,
}

impl View {
    const ALL: [View; 3] = [View::Disc, View::Limb, View::Ground];

    fn name(self) -> &'static str {
        match self {
            View::Disc => "disc",
            View::Limb => "limb",
            View::Ground => "ground",
        }
    }
}

/// The disc view's phase angle (Sun–body–camera, deg): mostly lit, with the
/// terminator in view.
pub const DISC_PHASE_DEG: f64 = 30.0;
/// The disc view's distance from the centre, in equatorial radii.
pub const DISC_RADII: f64 = 3.0;
/// The Sun's elevation at the camera's nadir (deg).
pub const LIMB_SUN_DEG: f64 = 30.0;
pub const GROUND_SUN_DEG: f64 = 10.0;
/// The limb view's height as a fraction of the radius (at least), and the
/// margin above the atmosphere's top (fraction of its thickness).
pub const LIMB_FRACTION: f64 = 0.015;
const LIMB_ABOVE_AIR: f64 = 1.05;
/// The ground view's height above the terrain (m).
pub const GROUND_ALTITUDE_M: f64 = 2000.0;
/// How far below the horizon the view's centre points (deg): the horizon
/// sits in the upper part of the frame (the vertical half field is 22.5°).
pub const HORIZON_MARGIN_DEG: f64 = 8.0;

/// Waits before a shot (s): at least the minimum, then until the terrain
/// is not busy, at most the maximum (a new body's maps load meanwhile).
const SETTLE_MIN: f64 = 1.5;
const SETTLE_MAX: f64 = 45.0;

/// A camera pose relative to a body's centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub offset: DVec3,
    pub look: DVec3,
    pub up: DVec3,
}

/// The camera's height above the reference radius for `view` of a body of
/// equatorial radius `radius` whose atmosphere reaches `air_top` (m, 0 for
/// none).
pub fn altitude(view: View, radius: f64, air_top: f64) -> f64 {
    match view {
        View::Disc => (DISC_RADII - 1.0) * radius,
        View::Limb => (LIMB_FRACTION * radius).max(LIMB_ABOVE_AIR * air_top),
        View::Ground => GROUND_ALTITUDE_M,
    }
}

/// The camera pose for `view` at height `alt` above a sphere of `radius`,
/// with the Sun in direction `sun` (unit, from the body) and the body's
/// pole `pole`. The disc view looks at the centre from [`DISC_PHASE_DEG`];
/// the low views stand where the Sun is [`LIMB_SUN_DEG`] or
/// [`GROUND_SUN_DEG`] up and look along the terminator (the light from the
/// side), the horizon [`HORIZON_MARGIN_DEG`] above the view's centre.
pub fn pose(view: View, sun: DVec3, pole: DVec3, radius: f64, alt: f64) -> Pose {
    let side = sun.cross(pole).try_normalize().unwrap_or_else(|| sun.any_orthonormal_vector());
    let around = |deg: f64| {
        let a = deg.to_radians();
        (sun * a.cos() + side * a.sin(), -sun * a.sin() + side * a.cos())
    };
    let r = radius + alt;
    match view {
        View::Disc => {
            let (d, _) = around(DISC_PHASE_DEG);
            let up = (pole - d * pole.dot(d)).try_normalize().unwrap_or(side);
            Pose { offset: d * r, look: -d, up }
        }
        View::Limb | View::Ground => {
            let sun_deg = if view == View::Limb { LIMB_SUN_DEG } else { GROUND_SUN_DEG };
            let (n, towards_night) = around(90.0 - sun_deg);
            let along = n.cross(towards_night);
            let dip = (radius / r).clamp(-1.0, 1.0).acos();
            let down = dip + HORIZON_MARGIN_DEG.to_radians();
            Pose { offset: n * r, look: along * down.cos() - n * down.sin(), up: n }
        }
    }
}

/// The bodies named in `SUNSCATTER_TOUR` (comma-separated).
pub fn parse_list(s: &str) -> Vec<String> {
    s.split(',').map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).collect()
}

#[derive(Resource)]
pub struct Tour {
    bodies: Vec<String>,
    body: usize,
    view: usize,
    timer: f64,
    started: bool,
    /// The body and view being shot, for [`aim`].
    current: Option<(NodeId, View)>,
}

impl Tour {
    /// From `SUNSCATTER_TOUR`; only with a demo directory to write to.
    pub fn from_env(demo: bool) -> Option<Self> {
        let list = parse_list(&std::env::var("SUNSCATTER_TOUR").ok()?);
        (demo && !list.is_empty()).then_some(Tour {
            bodies: list,
            body: 0,
            view: 0,
            timer: 0.0,
            started: false,
            current: None,
        })
    }

    fn advance(&mut self) {
        self.timer = 0.0;
        self.view += 1;
        if self.view == View::ALL.len() {
            self.view = 0;
            self.body += 1;
        }
    }
}

/// Sets the tour up if `SUNSCATTER_TOUR` asks for one (with a demo
/// directory); returns whether it did. The shots show the bodies without
/// the interface (no egui context).
pub fn install(app: &mut App, demo: bool) -> bool {
    let Some(tour) = Tour::from_env(demo) else { return false };
    app.insert_resource(tour);
    app.insert_resource(bevy_egui::EguiGlobalSettings { auto_create_primary_context: false, ..default() });
    true
}

/// Runs the tour (instead of the demo script) before the simulation.
#[allow(clippy::too_many_arguments)]
pub fn run(
    mut commands: Commands,
    time: Res<Time>,
    tour: Option<ResMut<Tour>>,
    mut demo: ResMut<Demo>,
    sim: Res<SimState>,
    mut rig: ResMut<CameraRig>,
    mut settings: ResMut<GraphicsSettings>,
    mut pause: ResMut<SimPause>,
    terrain: Res<Terrain>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut tour) = tour else { return };
    if !tour.started {
        tour.started = true;
        *settings = settings.with_preset(Tier::Minimal);
        // Not a demo step: the demo's per-step checks stay quiet.
        demo.step = Step::Done;
    }
    pause.0 = true;
    tour.timer += time.delta_secs_f64();
    let Some(name) = tour.bodies.get(tour.body).cloned() else {
        tour.current = None;
        // Screenshots are written asynchronously; give the last one time.
        if tour.timer > 3.0 {
            exit.write(AppExit::Success);
        }
        return;
    };
    let view = View::ALL[tour.view];
    let found = sim.world.sources.iter().find(|s| s.name.eq_ignore_ascii_case(&name) && s.physical.is_some());
    let Some(src) = found else {
        error!("tour: no body {name}");
        tour.view = View::ALL.len() - 1;
        tour.advance();
        return;
    };
    if view == View::Ground && !src.physical.as_ref().is_some_and(|p| p.solid) {
        tour.advance();
        return;
    }
    tour.current = Some((src.node, view));
    rig.focus = Focus::Body(src.node);
    let settled = tour.timer > SETTLE_MIN && !terrain.busy();
    if settled || tour.timer > SETTLE_MAX {
        if !settled {
            warn!("tour: {name} {} shot before the terrain settled", view.name());
        }
        demo.shot(&mut commands, &format!("{}_{}", name.to_lowercase(), view.name()));
        tour.advance();
    }
}

/// Places the camera for the current tour view, after the camera rig: the
/// rig's position and the camera's orientation are overridden.
pub fn aim(
    tour: Option<Res<Tour>>,
    sim: Res<SimState>,
    defs: Res<BodyDefs>,
    mut rig: ResMut<CameraRig>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
) {
    let Some((node, view)) = tour.and_then(|t| t.current) else { return };
    let Some(p) = sim.world.source(node).and_then(|s| s.physical.as_ref()) else { return };
    let Some(sun) = defs.light_source.map(|l| l.0) else { return };
    let snap = sim.world.snapshot(sim.clock);
    let to_sun = snap.relative_r(sun, node).normalize();
    let air_top = defs.get(node).and_then(|d| d.atmosphere.as_ref()).map_or(0.0, |a| f64::from(a.thickness_m));
    // A ringed body's disc view frames its rings (3 ring radii).
    let rings = defs.get(node).and_then(|d| d.rings.as_ref()).map_or(0.0, |r| f64::from(r.outer_m));
    let framed = if view == View::Disc { p.radius_eq.max(rings) } else { p.radius_eq };
    let alt = altitude(view, framed, air_top) + framed - p.radius_eq;
    let pose = pose(view, to_sun, p.rotation.pole(sim.clock), p.radius_eq, alt);
    let centre = snap.relative_r(node, rig.anchor);
    let mut cam_pos = centre + pose.offset;
    if view == View::Ground {
        // Over the terrain, not the sphere.
        let above = camera::clearance(&sim.world, &snap, rig.anchor, cam_pos);
        cam_pos += pose.up * (GROUND_ALTITUDE_M - above);
    }
    rig.cam_pos = cam_pos;
    rig.distance = (cam_pos - centre).length();
    if let Ok(mut t) = cam.single_mut() {
        *t = Transform::IDENTITY.looking_to(pose.look.as_vec3(), pose.up.as_vec3());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6.0e6;

    fn deg(a: DVec3, b: DVec3) -> f64 {
        a.dot(b).clamp(-1.0, 1.0).acos().to_degrees()
    }

    #[test]
    fn views_are_placed_and_lit_as_specified() {
        let sun = DVec3::new(0.3, -0.8, 0.2).normalize();
        let pole = DVec3::new(0.1, 0.2, 1.0).normalize();
        // (view, atmosphere top, expected height above the radius)
        let cases = [
            (View::Disc, 0.0, 2.0 * R),
            (View::Limb, 0.0, 0.015 * R),
            (View::Limb, 1.0e5, 1.05e5),    // Earth-like: just above the air
            (View::Limb, 6.0e5, 6.3e5),     // Titan-like: well above 1.5 %
            (View::Ground, 1.0e5, 2_000.0), // over the sphere; `aim` lifts it over terrain
        ];
        for (view, air, want) in cases {
            let alt = altitude(view, R, air);
            assert!((alt - want).abs() < 1e-6, "{view:?} {air}: {alt}");
            let p = pose(view, sun, pole, R, alt);
            assert!((p.offset.length() - (R + alt)).abs() < 1e-3, "{view:?}");
            assert!((p.look.length() - 1.0).abs() < 1e-12 && (p.up.length() - 1.0).abs() < 1e-12, "{view:?}");
            let nadir = -p.offset.normalize();
            match view {
                View::Disc => {
                    assert!(deg(p.look, nadir) < 1e-9, "the disc is centred");
                    assert!((deg(-nadir, sun) - DISC_PHASE_DEG).abs() < 1e-9, "phase angle");
                }
                View::Limb | View::Ground => {
                    let sun_up = 90.0 - deg(-nadir, sun);
                    let want_sun = if view == View::Limb { LIMB_SUN_DEG } else { GROUND_SUN_DEG };
                    assert!((sun_up - want_sun).abs() < 1e-9, "{view:?}: Sun {sun_up}° up");
                    // The horizon is in the upper part of the frame: the
                    // view's centre is the margin below it.
                    let dip = (R / (R + alt)).acos().to_degrees();
                    let below_level = 90.0 - deg(p.look, nadir);
                    assert!((below_level - dip - HORIZON_MARGIN_DEG).abs() < 1e-9, "{view:?}");
                    // The light comes from the side: the view runs along the terminator.
                    let level = (p.look - p.up * p.look.dot(p.up)).normalize();
                    assert!(level.dot(sun).abs() < 1e-9, "{view:?}");
                }
            }
        }
    }

    #[test]
    fn a_sun_over_the_pole_still_gives_a_pose() {
        let p = pose(View::Limb, DVec3::Z, DVec3::Z, R, 1.0e5);
        assert!(p.offset.is_finite() && p.look.is_finite() && p.up.is_finite());
    }

    #[test]
    fn the_list_is_comma_separated() {
        assert_eq!(parse_list("Mars, venus,,Titan "), ["Mars", "venus", "Titan"]);
        assert!(parse_list(" ").is_empty());
    }
}
