//! Simulation state owned by the app: the world, the fleet, the clock and
//! time warp, the player's controls, and trajectory predictions.

use crate::commands::{GameCommand, InputContext, Keys};
use crate::relations::Dominance;
use crate::trajectory::{self, settings::OrbitSettings};
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, ComputeTaskPool, Task};
use glam::DVec3;
use sim::craft::test_craft;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, Phase, Segment, Vessel, VesselId, VesselIds, TICK};
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
const OTHER_LOOKAHEAD_STEPS_PER_FRAME: usize = 300;

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
    /// The display rule for "which body is this about" (D056), rebuilt when
    /// the clock has moved by more than `DOMINANCE_REFRESH` (its radii
    /// change slowly). Display only (rule 1).
    pub dominance: Dominance,
    dominance_at: Epoch,
}

/// How far the clock moves before the dominance radii are rebuilt (s).
const DOMINANCE_REFRESH: f64 = 86_400.0;

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
        let ship = Vessel::landed_at(&world, vessel_ids.allocate(), "Earth", PAD_LAT, PAD_LON, clock, test_craft());
        SimState {
            dominance: Dominance::new(&world.eph, clock),
            dominance_at: clock,
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

    /// Rebuilds the dominance radii if the clock moved far enough.
    pub fn refresh_dominance(&mut self) {
        if self.clock.seconds_since(self.dominance_at).abs() > DOMINANCE_REFRESH {
            self.dominance = Dominance::new(&self.world.eph, self.clock);
            self.dominance_at = self.clock;
        }
    }

    /// The body fleet vessel `i` is about now, for display (its dominant
    /// body, D056).
    pub fn dominant_of(&self, i: usize) -> NodeId {
        let (anchor, r, _) = self.fleet[i].state_at(&self.world, self.clock);
        self.dominance.of(&self.world.eph, self.clock, anchor, r, None, None)
    }

    pub fn ship(&self) -> &Vessel {
        &self.fleet[self.active]
    }

    /// The fleet index of vessel `id`, if it exists.
    pub fn index_of(&self, id: VesselId) -> Option<usize> {
        self.fleet.iter().position(|v| v.id() == id)
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
        self.rails_block().is_none()
    }

    /// Why rails warp is not allowed right now, if it is not.
    pub fn rails_block(&self) -> Option<RailsBlock> {
        let ship = self.ship();
        if self.controls.throttle != 0.0 || matches!(ship.phase, Phase::Powered { .. }) {
            return Some(RailsBlock::Thrust);
        }
        if matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. }) {
            return None;
        }
        let (anchor, r, _) = ship.state_at(&self.world, self.clock);
        below_rails_floor(&self.world, self.clock, anchor, r).map(|(body, floor)| RailsBlock::Floor { body, floor })
    }

    pub fn reset(&mut self) {
        let id = self.vessel_ids.allocate();
        let ship = Vessel::landed_at(&self.world, id, "Earth", PAD_LAT, PAD_LON, self.clock, test_craft());
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
            self.fleet.push(Vessel::coasting(&self.world, id, self.clock, earth.node, r, v, test_craft()));
        }
    }
}

/// Keyboard → controls, warp, and meta actions.
pub fn read_controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    ctx: Res<InputContext>,
    mut sim: ResMut<SimState>,
    mut commands: MessageWriter<GameCommand>,
) {
    let dt = time.delta_secs_f64();
    if ctx.allows(Keys::Warp) {
        let level = sim.effective_warp();
        if keys.just_pressed(KeyCode::Period) {
            commands.write(GameCommand::SetWarp(level + 1));
        }
        if keys.just_pressed(KeyCode::Comma) {
            commands.write(GameCommand::SetWarp(level.saturating_sub(1)));
        }
        if keys.just_pressed(KeyCode::Slash) {
            commands.write(GameCommand::SetWarp(0));
        }
    }
    let c = &mut sim.controls;
    // Typing in a text field, the pause menu and the tracking station (R
    // would reset the ship unseen) do not fly the ship.
    if !ctx.allows(Keys::Flight) {
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
    if keys.just_pressed(KeyCode::KeyR) {
        commands.write(GameCommand::Reset);
    }
    if keys.just_pressed(KeyCode::F2) && ctx.allows(Keys::Debug) {
        commands.write(GameCommand::SpawnTestShips(10));
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
    mut turn: Local<usize>,
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
    let reached: Vec<(Epoch, bool)> = ComputeTaskPool::get().scope(|scope| {
        for (i, vessel) in sim.fleet.iter_mut().enumerate() {
            let controls = if i == active { active_controls } else { passive };
            scope.spawn(async move {
                let r = vessel.advance(world, target, &controls, COAST_STEPS_PER_FRAME);
                (r, stopped(vessel))
            });
        }
    });
    let mut clock = target;
    for (r, stopped) in reached {
        if holds_clock(r, target, stopped) && r < clock {
            clock = r;
        }
    }
    sim.compute_limited = clock < target;
    sim.clock = clock;
    sim.refresh_dominance();
    // Look ahead as far as the drawn line needs (D056), a bounded number of
    // steps per frame.
    let ship = &mut sim.fleet[sim.active];
    if let Some(seg) = ship.segment() {
        let until = trajectory::lookahead_until(&sim.world, seg, sim.clock, &orbits);
        ship.extend_coast(&sim.world, until, LOOKAHEAD_STEPS_PER_FRAME);
    }
    // The other vessels' lines (review game 1): one vessel per frame, in
    // turn, from a smaller shared budget.
    if let Some(i) = lookahead_turn(*turn, sim.fleet.len(), sim.active) {
        *turn = i + 1;
        let vessel = &mut sim.fleet[i];
        if let Some(seg) = vessel.segment() {
            let until = trajectory::lookahead_until(&sim.world, seg, sim.clock, &orbits);
            vessel.extend_coast(&sim.world, until, OTHER_LOOKAHEAD_STEPS_PER_FRAME);
        }
    }
}

/// Which non-active vessel gets this frame's look-ahead, starting the scan
/// at `turn` (wrapping); `None` if the active vessel is alone.
pub fn lookahead_turn(turn: usize, fleet: usize, active: usize) -> Option<usize> {
    (0..fleet).map(|k| (turn + k) % fleet).find(|&i| i != active)
}

/// Why rails warp is not allowed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RailsBlock {
    /// The throttle is open or the ship is under thrust.
    Thrust,
    /// Below a body's rails floor (D062): (body, floor altitude in m).
    Floor { body: NodeId, floor: f64 },
}

/// The first body whose rails floor (D062) a flying object at (`anchor`,
/// `r`) is below, with that floor. Physics, not display: every body counts,
/// not just a dominant one (rule 1).
pub fn below_rails_floor(world: &World, t: Epoch, anchor: NodeId, r: DVec3) -> Option<(NodeId, f64)> {
    let snap = world.snapshot(t);
    world.surfaces().find_map(|s| {
        let p = s.physical.as_ref().filter(|p| p.rails_floor > 0.0)?;
        let rel = r - snap.relative_r(s.node, anchor);
        let alt = p.altitude(p.rotation.to_fixed(sim::frame::Vec3::from_raw(rel), t));
        (alt < p.rails_floor).then_some((s.node, p.rails_floor))
    })
}

/// Whether a vessel's motion has ended for good at a time before the clock
/// (its trajectory failed or reached the ephemeris end): it stays there and
/// must not hold the clock.
fn stopped(vessel: &Vessel) -> bool {
    matches!(&vessel.phase, Phase::Coasting { trajectory } if trajectory.finished())
}

/// Whether a vessel that reached `reached` holds the clock back from
/// `target`: powered vessels trail by less than a tick; a coast that could
/// not be integrated far enough holds it; a stopped vessel never does.
pub fn holds_clock(reached: Epoch, target: Epoch, stopped: bool) -> bool {
    !stopped && reached.seconds_since(target) < -TICK
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rails_floor_blocks_low_orbits_on_earth_and_low_passes_over_the_moon() {
        let sim = SimState::new();
        let w = &sim.world;
        let t = sim.clock;
        let earth = w.find("Earth").unwrap().node;
        let moon = w.find("Moon").unwrap().node;
        let re = w.find("Earth").unwrap().physical.as_ref().unwrap().radius_eq;
        let rm = w.find("Moon").unwrap().physical.as_ref().unwrap().radius_eq;
        // (anchor, distance from its centre along x, blocked by)
        let cases = [
            (earth, re + 120_000.0, Some(earth)),
            (earth, re + 200_000.0, None),
            (moon, rm + 10_000.0, Some(moon)),
            (moon, rm + 15_000.0, None),
        ];
        for (anchor, d, expected) in cases {
            let got = below_rails_floor(w, t, anchor, DVec3::X * d).map(|b| b.0);
            assert_eq!(got, expected, "{d}");
        }
    }

    #[test]
    fn lookahead_takes_turns_and_skips_the_active_vessel() {
        assert_eq!(lookahead_turn(0, 1, 0), None);
        assert_eq!(lookahead_turn(0, 3, 0), Some(1));
        assert_eq!(lookahead_turn(2, 3, 0), Some(2));
        assert_eq!(lookahead_turn(3, 3, 0), Some(1), "wraps past the active vessel");
        assert_eq!(lookahead_turn(5, 3, 2), Some(0));
    }

    #[test]
    fn only_a_lagging_live_vessel_holds_the_clock() {
        let t = sol::sol_epoch();
        // (seconds behind the target, stopped, holds)
        let cases = [(0.0, false, false), (0.5 * TICK, false, false), (10.0, false, true), (10.0, true, false)];
        for (behind, stopped, holds) in cases {
            assert_eq!(holds_clock(t.add_seconds(-behind), t, stopped), holds, "{behind} {stopped}");
        }
    }
}
