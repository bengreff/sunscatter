//! Scripted demo flight (set `SUNSCATTER_DEMO=<screenshot dir>`): flies the
//! same pad → orbit → de-orbit → parachute profile as the sim scenario test,
//! through the real app loop, saving screenshots at each stage. Used to verify
//! the prototype end to end and as a reproducible showcase.
//!
//! The script drives the same controls a player would, except that it holds
//! attitude directly (magic attitude hold) like the scenario test.

use crate::camera::{self, CameraRig, Focus};
use crate::hud::{PlotFrame, UiState};
use crate::state::SimState;
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
    Moon,
    Perf(usize),
    Deorbit,
    Fall,
    Done,
}

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
    perf: PerfSample,
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
            perf: PerfSample::default(),
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
        self.step = step;
        self.timer = 0.0;
        self.shot_taken = false;
    }
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

/// Runs before the simulation each frame.
#[allow(clippy::too_many_lines)]
pub fn run(
    mut commands: Commands,
    time: Res<Time>,
    demo: Option<ResMut<Demo>>,
    mut sim: ResMut<SimState>,
    mut rig: ResMut<CameraRig>,
    mut ui: ResMut<UiState>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(mut demo) = demo else { return };
    demo.timer += time.delta_secs_f64();
    let earth = sim.world.find("Earth").expect("Earth").clone();
    let re = sim::body::earth().radius_eq;
    let (_, r, v) = sim.ship().state(&sim.world);
    let el = Elements::from_state(r, v, earth.gm);
    let alt = r.length() - re;
    let wait_shot = |demo: &mut Demo, commands: &mut Commands, name: &str, after: f64| -> bool {
        if !demo.shot_taken && demo.timer > after {
            demo.shot(commands, name);
            demo.shot_taken = true;
        }
        demo.shot_taken && demo.timer > after + 0.5
    };
    let point = |sim: &mut SimState, dir: DVec3| {
        let a = sim.active;
        sim.fleet[a].attitude.q = quat_z_to(dir.normalize());
        sim.fleet[a].attitude.omega = DVec3::ZERO;
    };
    match demo.step {
        Step::Pad => {
            rig.distance = 40.0;
            if wait_shot(&mut demo, &mut commands, "pad", 1.5) {
                sim.controls.throttle = 1.0;
                sim.warp = 3;
                demo.next(Step::Ascent);
            }
        }
        Step::Ascent => {
            let up = r.normalize();
            let pitch = (1.4 * (alt / 50_000.0).clamp(0.0, 1.0).sqrt()).min(1.35);
            point(&mut sim, up * pitch.cos() + horizontal(r, v) * pitch.sin());
            if alt > 20_000.0 && !demo.shot_taken {
                rig.distance = 80.0;
                demo.shot(&mut commands, "ascent");
                demo.shot_taken = true;
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
                rig.distance = 2.5e6;
                rig.pitch = 0.6;
                demo.next(Step::Orbit);
            }
        }
        Step::Orbit => {
            if wait_shot(&mut demo, &mut commands, "orbit_with_prediction", 1.0) {
                ui.plot_frame = PlotFrame::EarthInertial;
                if let Some(moon) = sim.world.find("Moon").map(|s| s.node) {
                    camera::focus_body(&mut rig, &sim, moon);
                }
                demo.next(Step::Moon);
            }
        }
        Step::Moon => {
            if wait_shot(&mut demo, &mut commands, "moon_focus", 1.0) {
                rig.focus = Focus::Ship;
                rig.distance = 3.0e6;
                sim.spawn_test_ships(10);
                demo.next(Step::Perf(0));
            }
        }
        Step::Perf(level) => {
            // 11 vessels in flight; measure frame times at 1x, 1000x, 1e6x.
            // (At 1e6x this stage spans ~46 days: the demo orbit must be high
            // enough that drag does not decay it meanwhile.)
            const LEVELS: [usize; 3] = [0, 6, 9];
            sim.warp = LEVELS[level];
            let dt = time.delta_secs_f64();
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
                    demo.next(Step::Done);
                }
                Phase::Crashed { speed, .. } => {
                    error!("demo: crashed at {speed:.1} m/s");
                    demo.next(Step::Done);
                }
                _ => {}
            }
        }
        Step::Done => {
            if wait_shot(&mut demo, &mut commands, "landed", 1.0) {
                exit.write(AppExit::Success);
            }
        }
    }
}
