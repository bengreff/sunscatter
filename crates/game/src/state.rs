//! Simulation state owned by the app: the world, the fleet, the clock and
//! time warp, the player's controls, and trajectory predictions.

use crate::commands::{GameCommand, InputContext, Keys};
use crate::relations::Dominance;
use crate::trajectory::{self, settings::OrbitSettings};
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};
use glam::DVec3;
use sim::craft::test_craft;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, HoldMode, Phase, Segment, Vessel, VesselId, VesselIds};
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

/// Per-frame look-ahead budgets (integration steps, ~10 µs each). The
/// vessels themselves advance within the frame's time ([`crate::fleet`]):
/// when they can't keep up, the clock is held back and the warp is limited
/// by compute, never the physics.
pub(crate) const LOOKAHEAD_STEPS_PER_FRAME: usize = 1_500;
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
    /// The warp actually achieved (simulated over real seconds, smoothed
    /// over about half a second): shown when compute limits it.
    pub achieved_warp: f64,
    /// Wall time of the vessels' advance per frame (ms, smoothed like the
    /// warp): shown with the frame rate.
    pub sim_ms: f64,
    /// How far the clock moved last frame (s): a compute-limited frame asks
    /// only a little more of the next ([`crate::fleet::frame_span`]).
    pub last_advance: f64,
    /// Controls on their way to an uncrewed active vessel (D034).
    pub remote: RemoteControls,
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
        // Start in daylight over Florida: 2030-01-01 17:00 TDB.
        Self::starting_at(sol::sol_epoch().add_seconds(17.0 * 3600.0))
    }

    /// A new game whose clock starts at `clock` (the body tour's date),
    /// with the ship on the pad.
    pub fn starting_at(clock: Epoch) -> Self {
        let world = load_world();
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
            achieved_warp: 1.0,
            sim_ms: 0.0,
            last_advance: 0.0,
            remote: RemoteControls::default(),
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
        if self.controls.throttle != 0.0 {
            return Some(RailsBlock::Thrust);
        }
        if matches!(ship.phase, Phase::Powered { .. }) {
            // Held live near a live controllable craft (D082), else flown
            // live: thrusting, in the atmosphere or in contact.
            if ship.in_bubble() && !ship.leads(&self.world, &self.controls) {
                if let Some(id) = self.bubble_leader() {
                    return Some(RailsBlock::Near(id));
                }
            }
            return Some(RailsBlock::Live);
        }
        // Every vessel moves at the same warp: one flying live (in an
        // atmosphere, say) would need 20 ms ticks at rails speed.
        if let Some(v) = self.fleet.iter().find(|v| matches!(v.phase, Phase::Powered { .. })) {
            return Some(RailsBlock::OtherLive(v.id()));
        }
        if matches!(ship.phase, Phase::Landed { .. } | Phase::Crashed { .. }) {
            return None;
        }
        let (anchor, r, _) = ship.state_at(&self.world, self.clock);
        below_rails_floor(&self.world, self.clock, anchor, r).map(|(body, floor)| RailsBlock::Floor { body, floor })
    }

    /// The nearest other live controllable craft: what holds the active
    /// vessel in its bubble (D082).
    fn bubble_leader(&self) -> Option<VesselId> {
        let ship = self.ship();
        let live = |v: &&Vessel| v.id() != ship.id() && matches!(v.phase, Phase::Powered { .. }) && v.controllable();
        let distance = |v: &Vessel| sim::vessel::debris::separation(&self.world, self.clock, ship, v);
        let nearest = self.fleet.iter().filter(live).map(|v| (distance(v), v.id()));
        nearest.min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, id)| id)
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
            let mut vessel = Vessel::coasting(&self.world, id, self.clock, earth.node, r, v, test_craft());
            // Every other one is an uncrewed probe (flown with light delay).
            if k % 2 == 1 {
                vessel.set_crew(0);
            }
            self.fleet.push(vessel);
        }
    }
}

/// The keys that pick an attitude hold mode.
pub const HOLD_KEYS: [(KeyCode, HoldMode); 6] = [
    (KeyCode::Digit1, HoldMode::Stability),
    (KeyCode::Digit2, HoldMode::Prograde),
    (KeyCode::Digit3, HoldMode::Retrograde),
    (KeyCode::Digit4, HoldMode::Target),
    (KeyCode::Digit5, HoldMode::AntiTarget),
    (KeyCode::Digit6, HoldMode::Maneuver),
];

/// Keyboard → controls, warp, and meta actions.
pub fn read_controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    ctx: Res<InputContext>,
    mut sim: ResMut<SimState>,
    mut commands: MessageWriter<GameCommand>,
    mut toasts: ResMut<crate::interface::toasts::Toasts>,
    mut reset_pressed: Local<Option<f64>>,
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
    let held = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    c.throttle = throttle_after(
        c.throttle,
        held(KeyCode::ShiftLeft, KeyCode::ShiftRight),
        held(KeyCode::ControlLeft, KeyCode::ControlRight),
        held(KeyCode::AltLeft, KeyCode::AltRight),
        dt,
    );
    if keys.just_pressed(KeyCode::KeyZ) {
        c.throttle = 1.0;
    }
    if keys.just_pressed(KeyCode::KeyX) {
        c.throttle = 0.0;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        c.sas = !c.sas;
    }
    // Hold modes (D075): 1 stability, 2 prograde, 3 retrograde, 4 target,
    // 5 anti-target, 6 maneuver.
    for (key, mode) in HOLD_KEYS {
        if keys.just_pressed(key) {
            commands.write(GameCommand::SetHold(mode));
        }
    }
    if keys.just_pressed(KeyCode::KeyP) {
        c.chute = true;
    }
    if keys.just_pressed(KeyCode::Space) {
        commands.write(GameCommand::NextStage);
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
        let now = time.elapsed_secs_f64();
        if reset_confirmed(*reset_pressed, now) {
            commands.write(GameCommand::Reset);
            *reset_pressed = None;
        } else {
            toasts.push(now, "Press R again to reset to the pad", false);
            *reset_pressed = Some(now);
        }
    }
    if keys.just_pressed(KeyCode::F2) && ctx.allows(Keys::Debug) {
        commands.write(GameCommand::SpawnTestShips(10));
    }
}

/// Throttle ramp rates (per second): normal, and fine while Alt is held.
pub const THROTTLE_RATE: f64 = 0.6;
pub const THROTTLE_RATE_FINE: f64 = 0.05;

/// The throttle after `dt` seconds with the up/down keys held; `fine`
/// slows the ramp for small adjustments (hovering, docking).
pub fn throttle_after(throttle: f64, up: bool, down: bool, fine: bool, dt: f64) -> f64 {
    let rate = if fine { THROTTLE_RATE_FINE } else { THROTTLE_RATE };
    let step = rate * dt * (f64::from(u8::from(up)) - f64::from(u8::from(down)));
    (throttle + step).clamp(0.0, 1.0)
}

/// A second press of R within this many seconds resets the ship.
pub const RESET_CONFIRM_S: f64 = 2.0;

/// Whether a press of R at `now` confirms one at `previous`.
pub fn reset_confirmed(previous: Option<f64>, now: f64) -> bool {
    previous.is_some_and(|p| now - p <= RESET_CONFIRM_S)
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
    comms: Res<crate::comms::Comms>,
    mut sim: ResMut<SimState>,
    mut turn: Local<usize>,
) {
    if pause.0 || menu.open {
        return;
    }
    let level = sim.effective_warp();
    let warp = WARP_LEVELS[level];
    // A probe acts on the controls that have reached it (D034).
    let mut active_controls = if sim.ship().crew() > 0 {
        sim.controls
    } else {
        let delay = comms.signal(sim.ship().id()).map(|s| s.delay);
        let (clock, controls) = (sim.clock, sim.controls);
        sim.remote.step(clock, controls, delay)
    };
    // Rotation input is only honoured at physics warp.
    if level > MAX_PHYSICS_WARP {
        active_controls.rotate = DVec3::ZERO;
    }
    let dt = time.delta_secs_f64().min(0.1);
    // Nothing is simulated past the ephemeris window: the clock stops there.
    let end = sim.world.end();
    let span = crate::fleet::frame_span(dt * warp, sim.compute_limited, sim.last_advance);
    let target = sim.clock.add_seconds(span);
    let target = if target > end { end } else { target };
    let sim = &mut *sim;
    let passive = Controls { sas: true, ..Default::default() };
    let active = sim.active;
    let controls = move |i: usize| if i == active { active_controls } else { passive };
    // Vessels are independent: advanced in parallel within the frame's
    // time for the simulation, the slowest first (`crate::fleet`).
    let start = std::time::Instant::now();
    let deadline = start + std::time::Duration::from_secs_f64(crate::fleet::SIM_FRAME_TIME);
    let before = sim.clock;
    // The 10 km bubble (D082): craft near a live controllable craft fly
    // live with it, tick for tick; the fleet then moves the rest.
    sim::vessel::bubble::advance(&sim.world, &mut sim.fleet, &controls, target);
    let adv = crate::fleet::advance_fleet(&sim.world, &mut sim.fleet, &controls, target, Some(deadline));
    let clock = crate::fleet::clock_after(&adv, target);
    sim.compute_limited = clock < target;
    sim.clock = clock;
    sim.achieved_warp = crate::fleet::achieved_warp(sim.achieved_warp, clock.seconds_since(before), dt);
    sim.last_advance = clock.seconds_since(before);
    let ms = start.elapsed().as_secs_f64() * 1e3;
    sim.sim_ms += (dt / 0.5).min(1.0) * (ms - sim.sim_ms);
    sim.refresh_dominance();
    // Look ahead as far as the drawn line needs (D056), a bounded number of
    // steps per frame.
    // Through the planned burns: the line ends on the last segment.
    let ship = &mut sim.fleet[sim.active];
    if let Some(seg) = ship.trajectory().map(|t| t.last()) {
        let until = trajectory::lookahead_until(&sim.world, seg, sim.clock, &orbits);
        ship.extend_coast(&sim.world, until, LOOKAHEAD_STEPS_PER_FRAME);
    }
    // The other vessels' lines (review game 1): one vessel per frame, in
    // turn, from a smaller shared budget.
    if let Some(i) = lookahead_turn(*turn, sim.fleet.len(), sim.active) {
        *turn = i + 1;
        let vessel = &mut sim.fleet[i];
        if let Some(seg) = vessel.trajectory().map(|t| t.last()) {
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

/// The controls an uncrewed vessel acts on: the operator's inputs, each
/// arriving after the signal's light delay (or never, without a signal).
#[derive(Clone, Debug, Default)]
pub struct RemoteControls {
    queue: std::collections::VecDeque<(Epoch, Controls)>,
    /// What the vessel last received.
    pub received: Controls,
}

impl RemoteControls {
    /// Sends the operator's `controls` at `now` with one-way `delay` (s),
    /// or nothing without a signal; returns what the vessel has at `now`.
    pub fn step(&mut self, now: Epoch, controls: Controls, delay: Option<f64>) -> Controls {
        if let Some(d) = delay {
            self.queue.push_back((now.add_seconds(d), controls));
        }
        while self.queue.front().is_some_and(|(t, _)| now.seconds_since(*t) >= 0.0) {
            self.received = self.queue.pop_front().expect("front").1;
        }
        self.received
    }

    /// Forgets everything in flight (switching vessels, loading).
    pub fn reset(&mut self) {
        *self = RemoteControls::default();
    }
}

/// Why rails warp is not allowed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RailsBlock {
    /// The throttle is open.
    Thrust,
    /// Flown in live ticks (near the ground, or thrust just ended).
    Live,
    /// Another vessel is flying live (in an atmosphere, or landing).
    OtherLive(VesselId),
    /// Held live within 10 km of this live controllable craft (D082).
    Near(VesselId),
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
    fn throttle_keys_ramp_and_clamp() {
        // (start, up, down, fine, dt, expected)
        let cases = [
            (0.0, true, false, false, 0.5, 0.3),
            (0.0, true, false, true, 1.0, 0.05),
            (0.5, false, true, false, 0.5, 0.2),
            (0.5, true, true, false, 1.0, 0.5),
            (0.95, true, false, false, 1.0, 1.0),
            (0.02, false, true, true, 1.0, 0.0),
        ];
        for (t, up, down, fine, dt, want) in cases {
            let got = throttle_after(t, up, down, fine, dt);
            assert!((got - want).abs() < 1e-12, "{t} {up} {down} {fine} {dt}: {got}");
        }
    }

    #[test]
    fn reset_needs_a_second_press() {
        assert!(!reset_confirmed(None, 10.0));
        assert!(reset_confirmed(Some(9.0), 10.0));
        assert!(!reset_confirmed(Some(7.0), 10.0));
    }

    #[test]
    fn remote_controls_arrive_after_the_delay_and_not_without_signal() {
        let t = sol::sol_epoch();
        let mut rc = RemoteControls::default();
        let full = Controls { throttle: 1.0, ..Default::default() };
        assert_eq!(rc.step(t, full, Some(1.3)).throttle, 0.0);
        assert_eq!(rc.step(t.add_seconds(1.0), full, Some(1.3)).throttle, 0.0);
        assert_eq!(rc.step(t.add_seconds(1.3), Controls::default(), Some(1.3)).throttle, 1.0);
        // No signal: the vessel keeps what it last received.
        assert_eq!(rc.step(t.add_seconds(10.0), Controls::default(), None).throttle, 0.0);
        let mut rc = RemoteControls::default();
        rc.step(t, full, None);
        assert_eq!(rc.step(t.add_seconds(100.0), full, None).throttle, 0.0, "nothing sent without signal");
    }

    #[test]
    fn lookahead_takes_turns_and_skips_the_active_vessel() {
        assert_eq!(lookahead_turn(0, 1, 0), None);
        assert_eq!(lookahead_turn(0, 3, 0), Some(1));
        assert_eq!(lookahead_turn(2, 3, 0), Some(2));
        assert_eq!(lookahead_turn(3, 3, 0), Some(1), "wraps past the active vessel");
        assert_eq!(lookahead_turn(5, 3, 2), Some(0));
    }

    /// D081: rails warp is refused while any craft is live, with the
    /// reason; craft in limbo (perturbed coasts: thin-air drag, light, a
    /// low-thrust burn under way) never refuse it.
    #[test]
    fn only_live_craft_refuse_rails_warp_and_limbo_craft_do_not() {
        use sim::vessel::{BurnEnd, BurnLaw, DirectionLaw, FlightPlan, PlannedBurn, SegmentKind};
        let mut sim = SimState::new();
        let w = sim.world.clone();
        let earth = w.find("Earth").unwrap();
        let re = earth.physical.as_ref().unwrap().radius_eq;
        let circular = |h: f64| {
            let r = re + h;
            (DVec3::new(r, 0.0, 0.0), DVec3::new(0.0, (earth.gm / r).sqrt(), 0.0))
        };
        let craft = |sim: &mut SimState, h: f64| {
            let (r, v) = circular(h);
            let id = sim.vessel_ids.allocate();
            Vessel::coasting(&w, id, sim.clock, earth.node, r, v, test_craft())
        };
        // On the pad, nothing else flying: allowed.
        assert_eq!(sim.rails_block(), None);
        // Limbo: drag decay at 300 km, and a 0.5 N burn spiralling up.
        let decaying = craft(&mut sim, 300_000.0);
        let mut spiral = craft(&mut sim, 700_000.0);
        let law = BurnLaw::from_isp(0.5, 3_000.0, DirectionLaw::Tracking { reference: earth.node, axes: DVec3::X });
        let burn = PlannedBurn { t_start: sim.clock, law, end: BurnEnd::Duration(30.0 * 86_400.0) };
        spiral.set_plan(&w, FlightPlan { burns: vec![burn] }).unwrap();
        spiral.advance(&w, sim.clock.add_seconds(3_600.0), &Controls { sas: true, ..Default::default() }, 1_200);
        assert!(matches!(spiral.segment().unwrap().kind, SegmentKind::Burn(_)), "burning on rails");
        sim.fleet.extend([decaying, spiral]);
        assert_eq!(sim.rails_block(), None, "limbo craft do not refuse rails warp");
        // A craft in the air (live below the boundary, 100 km): refused, named.
        let entering = craft(&mut sim, 100_000.0);
        let id = entering.id();
        assert!(matches!(entering.phase, Phase::Powered { .. }));
        sim.fleet.push(entering);
        assert_eq!(sim.rails_block(), Some(RailsBlock::OtherLive(id)));
        // The active vessel's own reasons come first.
        sim.controls.throttle = 0.5;
        assert_eq!(sim.rails_block(), Some(RailsBlock::Thrust));
        sim.controls.throttle = 0.0;
        sim.active = sim.fleet.len() - 1;
        assert_eq!(sim.rails_block(), Some(RailsBlock::Live));
    }

    /// D082: a craft held live in the bubble of a live controllable craft
    /// names it as the reason rails warp is refused.
    #[test]
    fn a_craft_held_in_a_bubble_names_its_leader() {
        let mut sim = SimState::new();
        let w = sim.world.clone();
        let earth = w.find("Earth").unwrap();
        let r = earth.physical.as_ref().unwrap().radius_eq + 400_000.0;
        let (pos, vel) = (DVec3::new(r, 0.0, 0.0), DVec3::new(0.0, (earth.gm / r).sqrt(), 0.0));
        let craft = |sim: &mut SimState, along: f64| {
            let id = sim.vessel_ids.allocate();
            Vessel::coasting(&w, id, sim.clock, earth.node, pos + DVec3::Y * along, vel, test_craft())
        };
        let (ship, other) = (craft(&mut sim, 0.0), craft(&mut sim, 2_000.0));
        let other_id = other.id();
        sim.fleet = vec![ship, other];
        sim.active = 0;
        assert_eq!(sim.rails_block(), None, "both coasting");
        // The other craft burns: a bubble forms around it and holds the
        // active one live.
        let burn = |i: usize| Controls { sas: true, throttle: if i == 1 { 0.1 } else { 0.0 }, ..Default::default() };
        sim::vessel::bubble::advance(&w, &mut sim.fleet, &burn, sim.clock.add_seconds(0.1));
        assert!(sim.fleet[0].in_bubble() && matches!(sim.fleet[0].phase, Phase::Powered { .. }));
        assert_eq!(sim.rails_block(), Some(RailsBlock::Near(other_id)));
    }
}
