//! Body tour (`SUNSCATTER_TOUR=<body,body,…>` with `SUNSCATTER_DEMO=<dir>`,
//! usually `SUNSCATTER_DEMO_OFFSCREEN=1`): screenshots of each listed body
//! in the one look (D073), with the clock frozen. Per body: the whole disc
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
use crate::state::{SimPause, SimState};
use crate::terrain::Terrain;
use bevy::app::AppExit;
use bevy::prelude::*;
use glam::DVec3;
use sim::frame::NodeId;
use sim::time::Epoch;

/// One of the tour's views of a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Disc,
    Limb,
    Ground,
    /// A moon half into its primary's shadow (the eclipse's ingress), seen
    /// as the disc view (moons only).
    Eclipse,
    /// A moon's shadow on its primary: the primary's disc view when the
    /// shadow's axis falls near the middle of it (moons only).
    Transit,
}

impl View {
    /// The views shot unless `SUNSCATTER_TOUR_VIEWS` lists others.
    const DEFAULT: [View; 3] = [View::Disc, View::Limb, View::Ground];
    const ALL: [View; 5] = [View::Disc, View::Limb, View::Ground, View::Eclipse, View::Transit];

    fn name(self) -> &'static str {
        match self {
            View::Disc => "disc",
            View::Limb => "limb",
            View::Ground => "ground",
            View::Eclipse => "eclipse",
            View::Transit => "transit",
        }
    }

    /// The views named in `SUNSCATTER_TOUR_VIEWS` (comma-separated), in
    /// that order; unknown names are skipped.
    pub fn parse_list(s: &str) -> Vec<View> {
        parse_list(s).iter().filter_map(|n| View::ALL.into_iter().find(|v| v.name().eq_ignore_ascii_case(n))).collect()
    }

    /// The pose the view uses (the event views are disc views).
    fn pose_view(self) -> View {
        match self {
            View::Eclipse | View::Transit => View::Disc,
            v => v,
        }
    }
}

/// Whether two body names match, ignoring case and punctuation ("hiiaka"
/// is Hi'iaka).
pub fn same_name(a: &str, b: &str) -> bool {
    let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    key(a) == key(b)
}

/// `SUNSCATTER_TOUR_DATE`: `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM` (TDB).
pub fn parse_date(s: &str) -> Option<Epoch> {
    let (date, time) = s.trim().split_once('T').unwrap_or((s.trim(), "00:00"));
    let d: Vec<&str> = date.split('-').collect();
    let t: Vec<&str> = time.split(':').collect();
    let [y, mo, da] = d[..] else { return None };
    let [h, mi] = t[..] else { return None };
    let (y, mo, da, h, mi) = (y.parse().ok()?, mo.parse().ok()?, da.parse().ok()?, h.parse().ok()?, mi.parse().ok()?);
    ((1..=12).contains(&mo) && (1..=31).contains(&da) && h < 24 && mi < 60)
        .then(|| Epoch::from_calendar(y, mo, da, h, mi, 0.0))
}

/// The first time in `(0, span]` (s after the start, sampled every `step`)
/// at which `f` falls from at least `level` to below it, refined to 1 s by
/// bisection; `None` if it does not within the span. A start below the
/// level waits for `f` to rise first (an eclipse already under way).
pub fn first_fall(span: f64, step: f64, level: f64, f: impl Fn(f64) -> f64) -> Option<f64> {
    let mut armed = f(0.0) >= level;
    let mut prev = 0.0;
    let mut t = step;
    while t <= span {
        let above = f(t) >= level;
        if armed && !above {
            let (mut lo, mut hi) = (prev, t);
            while hi - lo > 1.0 {
                let mid = 0.5 * (lo + hi);
                if f(mid) >= level {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some(hi);
        }
        armed |= above;
        prev = t;
        t += step;
    }
    None
}

/// How far from the middle of its primary a moon's shadow falls, in the
/// primary's radii: the distance from the primary's centre (the origin) to
/// the line from the star through the moon (`moon`, `star` relative to the
/// primary), beyond the moon; infinite when the primary is not behind the
/// moon.
pub fn shadow_offset(moon: DVec3, star: DVec3, primary_r: f64) -> f64 {
    let axis = (moon - star).normalize();
    let along = -moon.dot(axis);
    if along <= 0.0 {
        return f64::INFINITY;
    }
    (moon + axis * along).length() / primary_r
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
/// The ground view's height above the terrain (m), at most.
pub const GROUND_ALTITUDE_M: f64 = 2000.0;
/// The ground view stands within this vertical optical depth (green) of
/// the ground, so the ground shows through thick air.
const GROUND_TAU: f64 = 0.05;

/// The ground view's height above the terrain: [`GROUND_ALTITUDE_M`], or
/// lower where the air at the ground (extinction `extinction_m`, m⁻¹ in
/// green) would veil the ground from there: Venus's 90 bar of CO₂ (one
/// optical depth per ~1 km) puts the camera ~50 m up, Titan's haze ~500 m,
/// Earth stays at 2 km.
pub fn ground_altitude(extinction_m: f64) -> f64 {
    if extinction_m > 0.0 {
        GROUND_ALTITUDE_M.min(GROUND_TAU / extinction_m)
    } else {
        GROUND_ALTITUDE_M
    }
}
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
/// none) with extinction `extinction_m` at the ground (m⁻¹, green).
pub fn altitude(view: View, radius: f64, air_top: f64, extinction_m: f64) -> f64 {
    match view {
        View::Disc | View::Eclipse | View::Transit => (DISC_RADII - 1.0) * radius,
        View::Limb => (LIMB_FRACTION * radius).max(LIMB_ABOVE_AIR * air_top),
        View::Ground => ground_altitude(extinction_m),
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
        View::Disc | View::Eclipse | View::Transit => {
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
    views: Vec<View>,
    /// `SUNSCATTER_TOUR_DATE`, applied when the tour starts.
    date: Option<Epoch>,
    /// The clock the views start from (the event views search from it).
    base: Option<Epoch>,
    body: usize,
    view: usize,
    timer: f64,
    started: bool,
    /// The body framed and the view being shot, for [`aim`]; kept through
    /// the frame that takes the shot (it renders after [`run`] advances).
    current: Option<(NodeId, View)>,
    /// Whether the current view's clock and framing are set.
    prepared: bool,
}

impl Tour {
    /// From `SUNSCATTER_TOUR` (and `SUNSCATTER_TOUR_VIEWS`,
    /// `SUNSCATTER_TOUR_DATE`); only with a demo directory to write to.
    pub fn from_env(demo: bool) -> Option<Self> {
        let list = parse_list(&std::env::var("SUNSCATTER_TOUR").ok()?);
        let views = std::env::var("SUNSCATTER_TOUR_VIEWS").map_or(View::DEFAULT.to_vec(), |v| View::parse_list(&v));
        let date = std::env::var("SUNSCATTER_TOUR_DATE").ok().and_then(|d| {
            let e = parse_date(&d);
            if e.is_none() {
                error!("tour: SUNSCATTER_TOUR_DATE={d} is not YYYY-MM-DD[THH:MM]");
            }
            e
        });
        (demo && !list.is_empty() && !views.is_empty()).then_some(Tour {
            bodies: list,
            views,
            date,
            base: None,
            body: 0,
            view: 0,
            timer: 0.0,
            started: false,
            current: None,
            prepared: false,
        })
    }

    fn advance(&mut self) {
        self.timer = 0.0;
        self.prepared = false;
        self.view += 1;
        if self.view == self.views.len() {
            self.view = 0;
            self.body += 1;
        }
    }
}

/// How long the event views search ahead (s), and their step: a few orbits
/// of the slowest moon with eclipses every orbit (Callisto's 16.7 days
/// seldom eclipse; Iapetus's never mostly), sampled finely enough for
/// Io's 3-hour transit.
const EVENT_SPAN_S: f64 = 120.0 * 86_400.0;
const EVENT_STEP_S: f64 = 600.0;

/// The transit view: the shadow's axis within this many radii of the
/// primary's centre (Io's passes 0.33 radii off in 2030: the Sun is 3° off
/// Jupiter's equator).
const TRANSIT_OFFSET: f64 = 0.6;

/// The clock and the framed body for an event view of `moon`, searched
/// from `t0`: the eclipse's ingress (half the star's disc hidden by the
/// primary, seen at the moon's centre) or the shadow transit (the shadow's
/// axis within [`TRANSIT_OFFSET`] of the primary's centre).
fn event(sim: &SimState, star: NodeId, moon: NodeId, view: View, t0: Epoch) -> Option<(Epoch, NodeId)> {
    let primary = crate::relations::primary(&sim.world.eph, moon)?;
    let r_of = |n: NodeId| sim.world.source(n).and_then(|s| s.physical.as_ref()).map(|p| p.radius_eq);
    let (r_p, r_star) = (r_of(primary)?, r_of(star)?);
    let at = |dt: f64| {
        let snap = sim.world.snapshot(t0.add_seconds(dt));
        (snap.relative_r(moon, primary), snap.relative_r(star, primary))
    };
    let dt = match view {
        View::Eclipse => first_fall(EVENT_SPAN_S, EVENT_STEP_S, 0.5, |dt| {
            let (m, s) = at(dt);
            crate::lighting::eclipse_factor(m, s, r_star, &[(DVec3::ZERO, r_p)])
        })?,
        View::Transit => first_fall(EVENT_SPAN_S, EVENT_STEP_S, TRANSIT_OFFSET, |dt| {
            let (m, s) = at(dt);
            shadow_offset(m, s, r_p)
        })?,
        _ => return None,
    };
    Some((t0.add_seconds(dt), if view == View::Transit { primary } else { moon }))
}

/// Sets the tour up if `SUNSCATTER_TOUR` asks for one (with a demo
/// directory); returns whether it did. The shots show the bodies without
/// the interface (no egui context).
///
/// Either way bevy_egui's automatic choice of camera is off: the interface
/// belongs to the main camera (`camera::setup` gives it the primary egui
/// context, except on a tour). Left automatic, bevy_egui took the first
/// camera spawned, and the systems screen's own camera sometimes came first,
/// which hid every menu, the navball and the HUD (2026-10-27).
pub fn install(app: &mut App, demo: bool) -> bool {
    app.insert_resource(bevy_egui::EguiGlobalSettings { auto_create_primary_context: false, ..default() });
    let Some(tour) = Tour::from_env(demo) else { return false };
    app.insert_resource(tour);
    true
}

/// Runs the tour (instead of the demo script) before the simulation.
#[allow(clippy::too_many_arguments)]
pub fn run(
    mut commands: Commands,
    time: Res<Time>,
    tour: Option<ResMut<Tour>>,
    mut demo: ResMut<Demo>,
    mut sim: ResMut<SimState>,
    defs: Res<BodyDefs>,
    mut rig: ResMut<CameraRig>,
    mut pause: ResMut<SimPause>,
    terrain: Res<Terrain>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut tour) = tour else { return };
    if !tour.started {
        tour.started = true;
        // Not a demo step: the demo's per-step checks stay quiet.
        demo.step = Step::Done;
        if let Some(date) = tour.date {
            *sim = SimState::starting_at(date);
        }
        tour.base = Some(sim.clock);
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
    let view = tour.views[tour.view];
    let found = sim.world.sources.iter().find(|s| same_name(&s.name, &name) && s.physical.is_some());
    let Some((node, solid)) = found.map(|s| (s.node, s.physical.as_ref().is_some_and(|p| p.solid))) else {
        error!("tour: no body {name}");
        tour.view = tour.views.len() - 1;
        tour.advance();
        return;
    };
    if view == View::Ground && !solid {
        tour.advance();
        return;
    }
    if !tour.prepared {
        // A new view: its clock (the event views search for theirs).
        let base = tour.base.unwrap_or(sim.clock);
        let star = crate::lighting::brightest_star(&defs, &sim, node, DVec3::ZERO);
        let (clock, framed) = match view {
            View::Eclipse | View::Transit => match star.and_then(|s| event(&sim, s, node, view, base)) {
                Some(e) => e,
                None => {
                    warn!("tour: no {} of {name} within {} days", view.name(), EVENT_SPAN_S / 86_400.0);
                    tour.advance();
                    return;
                }
            },
            _ => (base, node),
        };
        if clock != sim.clock {
            sim.clock = clock;
        }
        tour.current = Some((framed, view));
        tour.prepared = true;
    }
    let framed = tour.current.map_or(node, |c| c.0);
    rig.focus = Focus::Body(framed);
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
    let Some(sun) = crate::lighting::brightest_star(&defs, &sim, node, DVec3::ZERO) else { return };
    let snap = sim.world.snapshot(sim.clock);
    let to_sun = snap.relative_r(sun, node).normalize();
    let air = defs.get(node).and_then(|d| d.atmosphere.as_ref());
    let air_top = air.map_or(0.0, |a| f64::from(a.thickness_m));
    let extinction =
        air.map_or(0.0, |a| f64::from(a.rayleigh_scattering[1] + a.mie_scattering[1] + a.mie_absorption[1]));
    // A ringed body's disc view frames its rings (3 ring radii).
    let rings = defs.get(node).and_then(|d| d.rings.as_ref()).map_or(0.0, |r| f64::from(r.outer_m));
    let view = view.pose_view();
    let framed = if view == View::Disc { p.radius_eq.max(rings) } else { p.radius_eq };
    let alt = altitude(view, framed, air_top, extinction) + framed - p.radius_eq;
    let pose = pose(view, to_sun, p.rotation.pole(sim.clock), p.radius_eq, alt);
    let centre = snap.relative_r(node, rig.anchor);
    let mut cam_pos = centre + pose.offset;
    if view == View::Ground {
        // Over the terrain, not the sphere.
        let above = camera::clearance(&sim.world, &snap, rig.anchor, cam_pos);
        cam_pos += pose.up * (ground_altitude(extinction) - above);
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
        // (view, atmosphere top, extinction at the ground (m-1, green),
        // expected height above the radius)
        let cases = [
            (View::Disc, 0.0, 0.0, 2.0 * R),
            (View::Limb, 0.0, 0.0, 0.015 * R),
            (View::Limb, 1.0e5, 1.3e-5, 1.05e5),    // Earth-like: just above the air
            (View::Limb, 6.0e5, 5.2e-5, 6.3e5),     // Titan-like: well above 1.5 %
            (View::Ground, 1.0e5, 1.3e-5, 2_000.0), // over the sphere; `aim` lifts it over terrain
            (View::Ground, 6.0e5, 1.07e-4, 0.05 / 1.07e-4), // Titan's N2 and haze: ~470 m
            (View::Ground, 3.5e5, 1.06e-3, 0.05 / 1.06e-3), // Venus's CO2: ~47 m up, tau 0.05
        ];
        for (view, air, ext, want) in cases {
            let alt = altitude(view, R, air, ext);
            assert!((alt - want).abs() < 1e-6, "{view:?} {air} {ext}: {alt}");
            let p = pose(view, sun, pole, R, alt);
            assert!((p.offset.length() - (R + alt)).abs() < 1e-3, "{view:?}");
            assert!((p.look.length() - 1.0).abs() < 1e-12 && (p.up.length() - 1.0).abs() < 1e-12, "{view:?}");
            let nadir = -p.offset.normalize();
            match view {
                View::Disc | View::Eclipse | View::Transit => {
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
    fn dates_and_views_parse() {
        // (text, expected seconds after 2030-01-01T00:00 TDB, or none)
        let start = Epoch::from_calendar(2030, 1, 1, 0, 0, 0.0);
        let cases = [
            ("2030-01-01", Some(0.0)),
            ("2030-01-02T06:30", Some(86_400.0 + 6.5 * 3600.0)),
            ("2030-13-01", None),
            ("2030-01-01T24:00", None),
            ("tomorrow", None),
        ];
        for (text, want) in cases {
            assert_eq!(parse_date(text).map(|e| e.seconds_since(start)), want, "{text}");
        }
        assert_eq!(View::parse_list("disc, Eclipse,nope,transit"), [View::Disc, View::Eclipse, View::Transit]);
    }

    #[test]
    fn the_first_fall_below_a_level_is_found_to_a_second() {
        // (case, function of t, expected crossing)
        type Case = (&'static str, fn(f64) -> f64, Option<f64>);
        let cases: [Case; 4] = [
            ("a falling line crosses at 1234.5", |t| 1234.5 - t, Some(1234.5)),
            ("never below", |_| 1.0, None),
            (
                "below from the start, then rising and falling again",
                |t| if (500.0..3000.0).contains(&t) { 1.0 } else { -1.0 },
                Some(3000.0),
            ),
            ("a dip shorter than a step is missed", |t| if (1000.0..1100.0).contains(&t) { -1.0 } else { 1.0 }, None),
        ];
        for (case, f, want) in cases {
            let got = first_fall(10_000.0, 600.0, 0.0, f);
            match (got, want) {
                (Some(g), Some(w)) => assert!((g - w).abs() <= 1.0, "{case}: {g}"),
                (g, w) => assert_eq!(g, w, "{case}"),
            }
        }
    }

    #[test]
    fn a_shadow_falls_on_the_primary_only_behind_the_moon() {
        let star = DVec3::new(-1.0e12, 0.0, 0.0);
        // (case, moon, expected offset in primary radii)
        let cases = [
            ("on the axis", DVec3::new(-4.0e8, 0.0, 0.0), 0.0),
            ("half a radius off", DVec3::new(-4.0e8, 3.5e7, 0.0), 0.5),
            ("behind the primary", DVec3::new(4.0e8, 0.0, 0.0), f64::INFINITY),
        ];
        for (case, moon, want) in cases {
            let got = shadow_offset(moon, star, 7.0e7);
            assert!(got == want || (got - want).abs() < 2e-3, "{case}: {got}");
        }
    }

    #[test]
    fn the_list_is_comma_separated() {
        assert_eq!(parse_list("Mars, venus,,Titan "), ["Mars", "venus", "Titan"]);
        assert!(parse_list(" ").is_empty());
        // (typed, body name, matches)
        for (typed, name, want) in [("hiiaka", "Hi'iaka", true), ("TITAN", "Titan", true), ("io", "Ion", false)] {
            assert_eq!(same_name(typed, name), want, "{typed}");
        }
    }
}
