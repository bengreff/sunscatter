//! Scripted demo flight (set `SUNSCATTER_DEMO=<screenshot dir>`): flies the
//! same pad → orbit → de-orbit → parachute profile as the sim scenario test,
//! through the real app loop. At each key view it freezes the clock and
//! captures every graphics tier; at the pad and in orbit it runs the graphics
//! benchmark (every tier, each feature alone). Used to verify the game end to
//! end, to judge visuals, and as a reproducible showcase.
//!
//! The script drives the same controls a player would, except that it holds
//! attitude directly (magic attitude hold) like the scenario test.

use crate::bench::{self, Bench};
use crate::camera::{self, CameraRig, Focus};
use crate::hud::{PlotFrame, UiState};
use crate::settings::{GraphicsSettings, Tier};
use crate::state::{SimPause, SimState};
use crate::terrain::Terrain;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use glam::DVec3;
use sim::kepler::Elements;
use sim::vessel::{quat_z_to, Phase};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Pad,
    Ascent,
    Coast,
    Circularize,
    Orbit,
    Map,
    /// Earth's night side and terminator from 20,000 km: must be dark.
    EarthNight,
    MoonFar,
    MoonClose,
    /// The Moon's night side from Earth's direction, near new Moon on the
    /// demo date: it must be dark (Earthshine is ~14 stops below sunlight).
    MoonNight,
    Perf(usize),
    Deorbit,
    Fall,
    Landed,
    Sunset,
    Done,
    Tracking,
    /// The pause menu, then the settings screen over it.
    Menu,
}

/// Checks, in code, what the map-view rule (D054) must show at each view,
/// and panics (failing the run) if it doesn't.
pub fn check_map_view(
    demo: Option<Res<Demo>>,
    sim: Res<SimState>,
    map: Res<crate::map::MapView>,
    station: Res<crate::tracking::TrackingStation>,
) {
    use crate::map_view::ObjectId;
    let Some(demo) = demo else { return };
    let ship = map.in_map(ObjectId::Vessel(sim.active));
    let body = |name: &str| sim.world.find(name).map(|s| map.in_map(ObjectId::Body(s.node)));
    match demo.step {
        Step::Pad => assert!(!ship, "demo Pad: the ship must not be in map view"),
        Step::Map if demo.shot_taken => {
            assert!(ship, "demo Map: the ship must be in map view");
            assert_eq!(body("Earth"), Some(false), "demo Map: Earth is drawn at full size");
        }
        Step::Tracking if station.open && demo.timer > 1.0 => {
            assert_eq!(body("Moon"), Some(true), "demo tracking station: the Moon's orbit must show");
            assert!(ship, "demo tracking station: the active vessel is pinned");
        }
        _ => {}
    }
}

/// Capturing one view in every tier.
struct Capture {
    view: &'static str,
    tier: usize,
    timer: f64,
}

/// Longest wait for terrain and textures to settle before a shot (s).
const SETTLE_MAX: f64 = 12.0;
const SETTLE_MIN: f64 = 1.2;

#[derive(Resource)]
pub struct Demo {
    /// Offscreen render target (`SUNSCATTER_DEMO_OFFSCREEN=1`): screenshots
    /// work even when the window is not presented (e.g. screen locked);
    /// egui draws into the same image, so the HUD is included.
    pub offscreen: Option<Handle<Image>>,
    dir: std::path::PathBuf,
    step: Step,
    timer: f64,
    shots: u32,
    shot_taken: bool,
    /// The camera yaw before the low ascent view looked back over land.
    land_view: Option<f64>,
    /// The settings screen's shot was taken (the Menu step).
    menu_shot: bool,
    /// The top-down pad view was captured.
    pad_top_done: bool,
    perf: PerfSample,
    capture: Option<Capture>,
    /// A benchmark was started and its results are pending.
    benching: bool,
    /// Graphics tier used while flying (between captures).
    flying_tier: Tier,
    /// End the demo after this step (`SUNSCATTER_DEMO_STOP_AFTER`, e.g. `Pad`).
    stop_after: Option<String>,
    /// Run the graphics benchmark (off with `SUNSCATTER_DEMO_NO_BENCH`).
    bench: bool,
}

/// Frame statistics for one warp level of the performance stage.
#[derive(Default)]
struct PerfSample {
    frames: u32,
    real: f64,
    worst: f64,
    limited: u32,
    sim_start: Option<sim::time::Epoch>,
}

impl Demo {
    pub fn from_env() -> Option<Self> {
        let dir = std::env::var_os("SUNSCATTER_DEMO")?;
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).ok()?;
        Some(Demo {
            offscreen: None,
            dir,
            step: Step::Pad,
            timer: 0.0,
            shots: 0,
            shot_taken: false,
            land_view: None,
            menu_shot: false,
            pad_top_done: false,
            perf: PerfSample::default(),
            capture: None,
            benching: false,
            flying_tier: Tier::Medium,
            stop_after: std::env::var("SUNSCATTER_DEMO_STOP_AFTER").ok(),
            bench: std::env::var_os("SUNSCATTER_DEMO_NO_BENCH").is_none(),
        })
    }

    fn shot(&mut self, commands: &mut Commands, name: &str) {
        self.shots += 1;
        let path = self.dir.join(format!("{:02}_{name}.png", self.shots));
        info!("demo screenshot {}", path.display());
        let shot = match &self.offscreen {
            Some(image) => Screenshot::image(image.clone()),
            None => Screenshot::primary_window(),
        };
        commands.spawn(shot).observe(save_to_disk(path));
    }

    fn next(&mut self, step: Step) {
        info!("demo step {:?} -> {:?}", self.step, step);
        let stop = self.stop_after.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(&format!("{:?}", self.step)));
        self.step = if stop { Step::Done } else { step };
        self.timer = 0.0;
        self.shot_taken = false;
    }

    /// Starts capturing `view` in every tier (the clock is frozen meanwhile),
    /// unless `SUNSCATTER_DEMO_NO_CAPTURE` is set (benchmark-only runs).
    fn capture(&mut self, view: &'static str) {
        if std::env::var_os("SUNSCATTER_DEMO_NO_CAPTURE").is_none() {
            self.capture = Some(Capture { view, tier: 0, timer: 0.0 });
        }
    }
}

/// Focuses `body` with the camera `distance` from its centre, in the
/// direction `angle` (rad) away from the Sun (0 = over the subsolar point).
fn view_sunlit(rig: &mut CameraRig, sim: &SimState, body: sim::frame::NodeId, angle: f64, distance: f64) {
    let snap = sim.world.snapshot(sim.clock);
    let Some(sun) = sim.world.find("Sun").map(|s| s.node) else { return };
    let to_sun = snap.relative_r(sun, body).normalize();
    camera::focus_body(rig, sim, body);
    let up = sim.world.source(body).and_then(|s| s.physical.as_ref()).map_or(DVec3::Z, |p| p.rotation.pole(sim.clock));
    let side = to_sun.cross(up).try_normalize().unwrap_or(DVec3::X);
    let d = to_sun * angle.cos() + side * angle.sin();
    let (e1, e2) = camera::basis(up);
    rig.pitch = d.dot(up).clamp(-1.0, 1.0).asin();
    rig.yaw = d.dot(e2).atan2(d.dot(e1));
    rig.distance = distance;
}

fn horizontal(r: DVec3, v: DVec3) -> DVec3 {
    let up = r.normalize();
    let h = v - up * v.dot(up);
    if h.length() > 1.0 {
        h.normalize()
    } else {
        DVec3::Z.cross(up).normalize()
    }
}

/// Runs the capture sub-state; returns true while it is busy.
fn run_capture(
    demo: &mut Demo,
    commands: &mut Commands,
    dt: f64,
    settings: &mut GraphicsSettings,
    terrain: &Terrain,
    pause: &mut SimPause,
) -> bool {
    let Some(cap) = demo.capture.as_mut() else { return false };
    pause.0 = true;
    let tier = Tier::ALL[cap.tier];
    if cap.timer == 0.0 {
        *settings = settings.with_preset(tier);
    }
    cap.timer += dt;
    let settled = cap.timer > SETTLE_MIN && !terrain.busy();
    if settled || cap.timer > SETTLE_MAX {
        let name = format!("{}_{}", cap.view, tier.name().to_lowercase());
        cap.tier += 1;
        cap.timer = 0.0;
        if cap.tier == Tier::ALL.len() {
            demo.capture = None;
            *settings = settings.with_preset(demo.flying_tier);
            pause.0 = false;
        }
        demo.shot(commands, &name);
    }
    true
}

/// Runs before the simulation each frame.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn run(
    mut commands: Commands,
    time: Res<Time>,
    demo: Option<ResMut<Demo>>,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut ui: ResMut<UiState>,
    mut settings: ResMut<GraphicsSettings>,
    mut bench: ResMut<Bench>,
    mut pause: ResMut<SimPause>,
    terrain: Res<Terrain>,
    mut station: ResMut<crate::tracking::TrackingStation>,
    mut menu: ResMut<crate::interface::pause::PauseMenu>,
    mut settings_ui: ResMut<crate::settings_ui::SettingsUi>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut demo) = demo else { return };
    let dt = time.delta_secs_f64();
    if run_capture(&mut demo, &mut commands, dt, &mut settings, &terrain, &mut pause) {
        return;
    }
    if demo.benching {
        pause.0 = true;
        if bench.running() {
            return;
        }
        info!("demo benchmark results\n{}", bench::markdown(&bench));
        let file = demo.dir.join(format!("bench_{}.md", bench.view));
        if let Err(e) = std::fs::write(&file, bench::markdown(&bench)) {
            error!("{}: {e}", file.display());
        }
        demo.benching = false;
        pause.0 = false;
    }
    demo.timer += dt;
    let earth = sim.world.find("Earth").expect("Earth").clone();
    let re = earth.physical.as_ref().expect("Earth is physical").radius_eq;
    let (_, r, v) = sim.ship().state(&sim.world);
    let el = Elements::from_state(r, v, earth.gm);
    let alt = r.length() - re;
    let point = |sim: &mut SimState, dir: DVec3| {
        let a = sim.active;
        sim.fleet[a].attitude.q = quat_z_to(dir.normalize());
        sim.fleet[a].attitude.omega = DVec3::ZERO;
    };
    let moon = sim.world.find("Moon").map(|s| s.node);
    match demo.step {
        Step::Pad => {
            if !demo.pad_top_done {
                rig.distance = 40.0;
            }
            if demo.timer > 1.0 && !demo.shot_taken {
                demo.shot_taken = true;
                demo.capture("pad");
            } else if demo.shot_taken && !demo.pad_top_done {
                // Straight down from 300 m: ground texture and contrast.
                demo.pad_top_done = true;
                rig.pitch = 1.5;
                rig.distance = 300.0;
                demo.capture("pad_top");
            } else if demo.shot_taken && demo.bench && !demo.benching && bench.results.is_empty() {
                bench.start("pad", *settings);
                demo.benching = true;
            } else if demo.shot_taken && (!demo.bench || !bench.results.is_empty()) {
                sim.controls.throttle = 1.0;
                sim.warp = 3;
                (rig.pitch, rig.distance) = (0.25, 40.0);
                demo.next(Step::Ascent);
            }
        }
        Step::Ascent => {
            let up = r.normalize();
            let pitch = (1.4 * (alt / 50_000.0).clamp(0.0, 1.0).sqrt()).min(1.35);
            point(&mut sim, up * pitch.cos() + horizontal(r, v) * pitch.sin());
            // At 10 km, look north along the Florida coast from above and
            // behind the ship: the view where the ground showed hard-edged
            // dark patches (land against sky-reflecting ocean).
            if alt > 10_000.0 && demo.land_view.is_none() {
                demo.land_view = Some(rig.yaw);
                let (e1, e2) = camera::basis(up);
                let north = up.cross(DVec3::Z.cross(up)).normalize();
                let back = -north;
                rig.yaw = back.dot(e2).atan2(back.dot(e1));
                rig.pitch = 0.35;
                rig.distance = 80.0;
                demo.capture("ascent_10km_coast");
            }
            if alt > 20_000.0 && !demo.shot_taken {
                if let Some(yaw) = demo.land_view {
                    (rig.yaw, rig.pitch) = (yaw, 0.25);
                }
                rig.distance = 80.0;
                demo.shot_taken = true;
                demo.capture("ascent_20km");
            }
            if el.apoapsis() - re > 300_000.0 {
                sim.controls.throttle = 0.0;
                sim.warp = 5;
                demo.next(Step::Coast);
            }
        }
        Step::Coast => {
            let to_apo = (std::f64::consts::PI - el.mean_anomaly) / el.mean_motion(earth.gm);
            if to_apo < 80.0 {
                sim.warp = 3;
            }
            if to_apo < 70.0 {
                demo.next(Step::Circularize);
            }
        }
        Step::Circularize => {
            let (up, h) = (r.normalize(), horizontal(r, v));
            let v_circ = (earth.gm / r.length()).sqrt();
            sim.controls.throttle = ((v_circ - v.dot(h)) / 60.0).clamp(0.05, 1.0);
            point(&mut sim, h + up * (-v.dot(up) / 100.0).clamp(-0.2, 0.5));
            if el.periapsis() - re > 250_000.0 {
                sim.controls.throttle = 0.0;
                sim.warp = 0;
                rig.focus = Focus::Ship;
                rig.distance = 60.0;
                rig.pitch = 0.12;
                demo.next(Step::Orbit);
            }
        }
        Step::Orbit => {
            if !demo.shot_taken {
                demo.shot_taken = true;
                demo.capture("orbit_day");
            } else if demo.bench && !demo.benching && bench.view != "orbit" {
                bench.start("orbit", *settings);
                demo.benching = true;
            } else {
                ui.plot_frame = PlotFrame::EarthInertial;
                rig.distance = 2.5e7;
                rig.pitch = 0.9;
                demo.next(Step::Map);
            }
        }
        Step::Map => {
            if !demo.shot_taken {
                demo.shot_taken = true;
                demo.capture("map");
            } else {
                view_sunlit(&mut rig, &sim, earth.node, 120f64.to_radians(), 2.6e7);
                demo.next(Step::EarthNight);
            }
        }
        Step::EarthNight => {
            if !demo.shot_taken {
                demo.shot_taken = true;
                demo.capture("earth_night");
            } else {
                if let Some(moon) = moon {
                    view_sunlit(&mut rig, &sim, moon, 40f64.to_radians(), 3.0 * 1_737_400.0);
                }
                demo.next(Step::MoonFar);
            }
        }
        Step::MoonFar => {
            if !demo.shot_taken {
                demo.shot_taken = true;
                demo.capture("moon_orbit");
            } else {
                if let Some(moon) = moon {
                    // A low Sun (about 12° up) brings out the relief.
                    view_sunlit(&mut rig, &sim, moon, 78f64.to_radians(), 1_737_400.0 + 4_000.0);
                }
                demo.next(Step::MoonClose);
            }
        }
        Step::MoonClose => {
            if !demo.shot_taken {
                demo.shot_taken = true;
                demo.capture("moon_close");
            } else {
                if let (Some(moon), Some(earth)) = (moon, sim.world.find("Earth").map(|s| s.node)) {
                    let snap = sim.world.snapshot(sim.clock);
                    let d = snap.relative_r(earth, moon).normalize();
                    camera::focus_body(&mut rig, &sim, moon);
                    let up = sim
                        .world
                        .source(moon)
                        .and_then(|s| s.physical.as_ref())
                        .map_or(DVec3::Z, |p| p.rotation.pole(sim.clock));
                    let (e1, e2) = camera::basis(up);
                    rig.pitch = d.dot(up).clamp(-1.0, 1.0).asin();
                    rig.yaw = d.dot(e2).atan2(d.dot(e1));
                    rig.distance = 3.0 * 1_737_400.0;
                }
                demo.next(Step::MoonNight);
            }
        }
        Step::MoonNight => {
            if demo.timer > 2.0 && !demo.shot_taken {
                demo.shot_taken = true;
                demo.shot(&mut commands, "moon_night");
            } else if demo.timer > 4.0 {
                settings.set_if_neq(GraphicsSettings::preset(Tier::Minimal));
                rig.focus = Focus::Ship;
                rig.distance = 3.0e6;
                sim.spawn_test_ships(10);
                demo.next(Step::Perf(0));
            }
        }
        Step::Perf(level) => {
            // 11 vessels in flight; measure frame times at 1x, 1000x, 1e6x
            // (Minimal graphics, like the prototype's measurements).
            // (At 1e6x this stage spans ~46 days: the demo orbit must be high
            // enough that drag does not decay it meanwhile.)
            const LEVELS: [usize; 3] = [0, 6, 9];
            sim.warp = LEVELS[level];
            let clock = sim.clock;
            let limited = sim.compute_limited;
            let p = &mut demo.perf;
            if p.sim_start.is_none() {
                p.sim_start = Some(clock);
            } else {
                p.frames += 1;
                p.real += dt;
                p.worst = p.worst.max(dt);
                p.limited += u32::from(limited);
            }
            if demo.timer > 4.0 {
                let p = &demo.perf;
                let sim_span = clock.seconds_since(p.sim_start.expect("started"));
                info!(
                    "demo perf: {} vessels, warp {}x: {:.0} fps avg, worst frame {:.1} ms, effective warp {:.0}x, compute-limited {}/{} frames",
                    sim.fleet.len(),
                    crate::state::WARP_LEVELS[LEVELS[level]],
                    f64::from(p.frames) / p.real,
                    p.worst * 1e3,
                    sim_span / p.real,
                    p.limited,
                    p.frames
                );
                demo.perf = PerfSample::default();
                if level + 1 < LEVELS.len() {
                    demo.next(Step::Perf(level + 1));
                } else {
                    sim.fleet.truncate(1);
                    *settings = settings.with_preset(demo.flying_tier);
                    rig.distance = 60.0;
                    sim.warp = 3;
                    sim.controls.throttle = 1.0;
                    demo.next(Step::Deorbit);
                }
            }
        }
        Step::Deorbit => {
            point(&mut sim, -v);
            if el.periapsis() - re < 30_000.0 {
                sim.controls.throttle = 0.0;
                sim.warp = 6;
                demo.next(Step::Fall);
            }
        }
        Step::Fall => {
            // Drag coasts are stored segments too, so rails warp is valid in
            // the atmosphere; slow down only to watch the parachute.
            sim.warp = if alt > 100_000.0 {
                6
            } else if alt > 15_000.0 {
                5
            } else {
                4
            };
            if alt < 12_000.0 {
                sim.controls.chute = true;
                if !demo.shot_taken && alt < 3_000.0 {
                    rig.distance = 150.0;
                    demo.shot(&mut commands, "parachute");
                    demo.shot_taken = true;
                }
            }
            match sim.ship().phase {
                Phase::Landed { .. } => {
                    info!("demo: landed safely");
                    sim.warp = 0;
                    demo.next(Step::Landed);
                }
                Phase::Crashed { speed, .. } => {
                    error!("demo: crashed at {speed:.1} m/s");
                    demo.next(Step::Done);
                }
                _ => {}
            }
        }
        Step::Landed => {
            if !demo.shot_taken && demo.timer > 1.0 {
                demo.shot(&mut commands, "landed");
                demo.shot_taken = true;
            }
            if demo.shot_taken && demo.timer > 1.5 {
                demo.next(Step::Sunset);
            }
        }
        Step::Sunset => {
            // Warp until the Sun is low, then look towards it.
            let snap = sim.world.snapshot(sim.clock);
            let (anchor, r, _) = sim.ship().state(&sim.world);
            let sun = sim.world.find("Sun").map(|s| s.node).expect("Sun");
            let to_sun = (snap.relative_r(sun, anchor) - r).normalize();
            let up = r.normalize();
            let elevation = to_sun.dot(up).asin().to_degrees();
            if demo.shot_taken {
                demo.next(Step::Tracking);
            } else if elevation > 4.0 || elevation < -0.5 {
                sim.warp = if elevation > 12.0 || elevation < -0.5 { 6 } else { 5 };
            } else {
                sim.warp = 0;
                let (e1, e2) = camera::basis(up);
                let away = -(to_sun - up * to_sun.dot(up));
                // Turned ~20° so the Sun (and its flare) is beside the ship.
                rig.yaw = away.dot(e2).atan2(away.dot(e1)) + 0.35;
                rig.pitch = 0.05;
                rig.distance = 40.0;
                demo.shot_taken = true;
                demo.capture("sunset");
            }
        }
        Step::Tracking => {
            // A few vessels in orbit to list in the tracking station.
            if !station.open {
                sim.spawn_test_ships(3);
                station.open = true;
            }
            if !demo.shot_taken && demo.timer > 1.5 {
                demo.shot(&mut commands, "tracking_station");
                demo.shot_taken = true;
                demo.next(Step::Menu);
            }
        }
        Step::Menu => {
            station.open = false;
            menu.open = true;
            if demo.timer > 1.0 && !demo.shot_taken {
                demo.shot(&mut commands, "pause_menu");
                demo.shot_taken = true;
                settings_ui.open = true;
                settings_ui.tab = crate::settings_ui::SettingsTab::Interface;
            } else if demo.timer > 2.0 && demo.shot_taken && !demo.menu_shot {
                demo.shot(&mut commands, "settings_interface");
                // Closed a second later: the shot is of the frame being drawn.
                demo.menu_shot = true;
            } else if demo.timer > 3.0 && demo.menu_shot {
                settings_ui.open = false;
                menu.open = false;
                demo.next(Step::Done);
            }
        }
        Step::Done => {
            // Screenshots are written asynchronously; give the last one time.
            if demo.timer > 3.0 {
                exit.write(AppExit::Success);
            }
        }
    }
}
