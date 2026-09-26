//! Simulation state owned by the app: the world, the fleet, the clock and
//! time warp, the player's controls, and trajectory predictions.

use crate::trajectory::{self, settings::OrbitSettings};
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, ComputeTaskPool, Task};
use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, Phase, Segment, Vessel, VesselIds, VesselParams, TICK};
use sim::world::World;
use std::sync::Arc;

/// Warp levels. Indices 0..=3 are physics warp (thrust and control allowed);
/// the rest are rails warp (coasting only). Maximum 1,000,000x (D022).
pub const WARP_LEVELS: [f64; 10] = [1.0, 2.0, 3.0, 4.0, 10.0, 100.0, 1e3, 1e4, 1e5, 1e6];
pub const MAX_PHYSICS_WARP: usize = 3;

/// Launch site: LC-39A, Kennedy Space Center.
/// The launch pad: Kennedy Space Center, on Merritt Island about 6 km west
/// of LC-39A. At the committed maps' resolution (~2 km water mask, ~5 km
/// colour map) LC-39A's barrier island is not resolved and the pad would sit
/// in the surf; a proper launch-site data patch is future work.
pub const PAD_LAT: f64 = 28.6082;
pub const PAD_LON: f64 = -80.66;

/// Per-vessel, per-frame integration budgets (steps, ~7 µs each) so a frame
/// never stalls. When a coast can't keep up, the clock is held back: warp is
/// then limited by compute, never the physics.
const COAST_STEPS_PER_FRAME: usize = 1_200;
const LOOKAHEAD_STEPS_PER_FRAME: usize = 1_500;

#[derive(Resource)]
pub struct SimState {
    pub world: World,
    pub fleet: Vec<Vessel>,
    /// Hands out vessel ids (saved, so ids are never reused).
    pub vessel_ids: VesselIds,
    pub active: usize,
    pub clock: Epoch,
    pub warp: usize,
    pub controls: Controls,
    /// Frames where the clock was held back by integration budget.
    pub compute_limited: bool,
}

fn load_world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    World::sol(Arc::new(Ephemeris::from_bytes(&bytes).expect("valid ephemeris")))
}

impl SimState {
    pub fn new() -> Self {
        let world = load_world();
        // Start in daylight over Florida: 2030-01-01 17:00 TDB.
        let clock = sol::sol_epoch().add_seconds(17.0 * 3600.0);
        let mut vessel_ids = VesselIds::default();
        let ship =
            Vessel::landed_at(&world, vessel_ids.allocate(), "Earth", PAD_LAT, PAD_LON, clock, VesselParams::block());
        SimState {
            world,
            fleet: vec![ship],
            vessel_ids,
            active: 0,
            clock,
            warp: 0,
            controls: Controls { sas: true, ..Default::default() },
            compute_limited: false,
        }
    }

    pub fn ship(&self) -> &Vessel {
        &self.fleet[self.active]
    }

    /// The warp actually applied: rails warp only while coasting or landed
    /// with the throttle closed; otherwise capped at physics warp.
    pub fn effective_warp(&self) -> usize {
        if self.rails_allowed() {
            self.warp
        } else {
            self.warp.min(MAX_PHYSICS_WARP)
        }
    }

    /// Whether rails warp (beyond 4x) is allowed right now.
    pub fn rails_allowed(&self) -> bool {
        self.controls.throttle == 0.0 && !matches!(self.ship().phase, Phase::Powered { .. })
    }

    pub fn reset(&mut self) {
        let id = self.vessel_ids.allocate();
        let ship = Vessel::landed_at(&self.world, id, "Earth", PAD_LAT, PAD_LON, self.clock, VesselParams::block());
        self.fleet[self.active] = ship;
        self.controls = Controls { sas: true, ..Default::default() };
        self.warp = 0;
    }

    /// Debug: spawn `n` coasting test ships in low Earth orbit (performance).
    pub fn spawn_test_ships(&mut self, n: usize) {
        let earth = self.world.find("Earth").expect("Earth").clone();
        for k in 0..n {
            let el = Elements {
                a: 6_778_137.0 + 20_000.0 * k as f64,
                e: 0.001,
                i: 0.5 + 0.05 * k as f64,
                raan: 0.6 * k as f64,
                argp: 0.0,
                mean_anomaly: 0.7 * k as f64,
            };
            let (r, v) = el.to_state(earth.gm);
            let id = self.vessel_ids.allocate();
            self.fleet.push(Vessel::coasting(&self.world, id, self.clock, earth.node, r, v, VesselParams::block()));
        }
    }
}

/// Keyboard → controls, warp, and meta actions.
pub fn read_controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    egui: Res<bevy_egui::input::EguiWantsInput>,
    menu: Res<crate::interface::pause::PauseMenu>,
    station: Res<crate::tracking::TrackingStation>,
    mut sim: ResMut<SimState>,
) {
    let dt = time.delta_secs_f64();
    let c = &mut sim.controls;
    // Typing in a text field (e.g. a save name), the pause menu and the
    // tracking station (R would reset the ship unseen) do not fly the ship.
    if egui.wants_any_keyboard_input() || menu.open || station.open {
        c.rotate = DVec3::ZERO;
        return;
    }
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        c.throttle = (c.throttle + 0.6 * dt).min(1.0);
    }
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        c.throttle = (c.throttle - 0.6 * dt).max(0.0);
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        c.throttle = 1.0;
    }
    if keys.just_pressed(KeyCode::KeyX) {
        c.throttle = 0.0;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        c.sas = !c.sas;
    }
    if keys.just_pressed(KeyCode::KeyP) {
        c.chute = true;
    }
    let axis =
        |pos: KeyCode, neg: KeyCode| f64::from(u8::from(keys.pressed(pos))) - f64::from(u8::from(keys.pressed(neg)));
    // Body axes: +Z nose. Pitch about X (W/S), yaw about Y (A/D), roll about Z (Q/E).
    c.rotate = DVec3::new(
        axis(KeyCode::KeyS, KeyCode::KeyW),
        axis(KeyCode::KeyD, KeyCode::KeyA),
        axis(KeyCode::KeyE, KeyCode::KeyQ),
    );

    if keys.just_pressed(KeyCode::Period) {
        sim.warp = (sim.effective_warp() + 1).min(WARP_LEVELS.len() - 1);
    }
    if keys.just_pressed(KeyCode::Comma) {
        sim.warp = sim.effective_warp().saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::Slash) {
        sim.warp = 0;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        sim.reset();
    }
    if keys.just_pressed(KeyCode::F2) {
        sim.spawn_test_ships(10);
    }
}

/// Freezes the clock (used by the demo while it captures every graphics
/// tier; the pause menu freezes it too).
#[derive(Resource, Default)]
pub struct SimPause(pub bool);

/// Advances every vessel to the new clock. Time warp only changes how far the
/// clock moves per frame; each vessel's physics is independent of it.
pub fn advance(
    time: Res<Time>,
    pause: Res<SimPause>,
    menu: Res<crate::interface::pause::PauseMenu>,
    orbits: Res<OrbitSettings>,
    mut sim: ResMut<SimState>,
) {
    if pause.0 || menu.open {
        return;
    }
    let level = sim.effective_warp();
    let warp = WARP_LEVELS[level];
    // Rotation input is only honoured at physics warp.
    let mut active_controls = sim.controls;
    if level > MAX_PHYSICS_WARP {
        active_controls.rotate = DVec3::ZERO;
    }
    let dt = time.delta_secs_f64().min(0.1);
    // Nothing is simulated past the ephemeris window: the clock stops there.
    let end = sim.world.end();
    let target = sim.clock.add_seconds(dt * warp);
    let target = if target > end { end } else { target };
    let sim = &mut *sim;
    let passive = Controls { sas: true, ..Default::default() };
    // Vessels are independent, so advancing them in parallel is deterministic.
    let world = &sim.world;
    let active = sim.active;
    let reached: Vec<Epoch> = ComputeTaskPool::get().scope(|scope| {
        for (i, vessel) in sim.fleet.iter_mut().enumerate() {
            let controls = if i == active { active_controls } else { passive };
            scope.spawn(async move { vessel.advance(world, target, &controls, COAST_STEPS_PER_FRAME) });
        }
    });
    // Powered vessels trail by less than a tick; a coast that could not be
    // integrated far enough holds the clock back.
    let mut clock = target;
    for r in reached {
        if r.seconds_since(target) < -TICK && r < clock {
            clock = r;
        }
    }
    sim.compute_limited = clock < target;
    sim.clock = clock;
    // Look ahead as far as the drawn line needs (D056), a bounded number of
    // steps per frame.
    let ship = &mut sim.fleet[sim.active];
    if let Some(seg) = ship.segment() {
        let until = trajectory::lookahead_until(&sim.world, seg, sim.clock, &orbits);
        ship.extend_coast(&sim.world, until, LOOKAHEAD_STEPS_PER_FRAME);
    }
}

/// The predicted trajectory while powered (the stored segment is used
/// directly while coasting). Computed on a background task and swapped in
/// atomically when complete, so the drawn line never flickers.
#[derive(Resource, Default)]
pub struct Prediction {
    pub segment: Option<Segment>,
    task: Option<Task<Segment>>,
    since_last: f64,
}

pub fn update_prediction(
    time: Res<Time>,
    sim: Res<SimState>,
    orbits: Res<OrbitSettings>,
    mut pred: ResMut<Prediction>,
) {
    if let Some(task) = pred.task.as_mut() {
        if let Some(seg) = check_ready(task) {
            pred.segment = Some(seg);
            pred.task = None;
        }
    }
    let ship = sim.ship();
    if !matches!(ship.phase, Phase::Powered { .. }) {
        pred.segment = None;
        return;
    }
    pred.since_last += time.delta_secs_f64();
    if pred.task.is_none() && pred.since_last > 0.25 {
        pred.since_last = 0.0;
        let world = sim.world.clone();
        let mut seg = ship.coast_from_now(&world);
        let orbits = orbits.clone();
        pred.task = Some(AsyncComputeTaskPool::get().spawn(async move {
            trajectory::extend_to_line_end(&world, &mut seg, &orbits, 40_000);
            seg
        }));
    }
}
