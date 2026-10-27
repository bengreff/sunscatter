//! Simulation state owned by the app: the world, the fleet, the clock and
//! time warp, the player's controls, and trajectory predictions.

use crate::commands::{GameCommand, InputContext, Keys};
use crate::relations::Dominance;
use crate::trajectory::{self, settings::OrbitSettings};
use bevy::prelude::*;
use bevy::tasks::{futures::check_ready, AsyncComputeTaskPool, Task};
use glam::DVec3;
use sim::craft::{craft_of, test_craft, Craft, CraftId};
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{Controls, HoldMode, Phase, Segment, SiteClock, Vessel, VesselId, VesselIds};
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

/// The craft a new flight can start with (D100: the Falcon 9 Crew; D064:
/// the test craft as the debug craft): a directory under `data/craft` and
/// its name in the new-flight menu.
pub const CRAFT_CHOICES: [(&str, &str); 2] =
    [("falcon9-crew", "Falcon 9 Block 5 · Crew Dragon"), (sim::craft::TEST_CRAFT, "Test craft (debug)")];

/// The choices whose craft files exist (`exists`: a directory under
/// `data/craft` holds a `craft.ron`), in menu order.
pub fn craft_choices(exists: impl Fn(&str) -> bool) -> Vec<(&'static str, &'static str)> {
    CRAFT_CHOICES.iter().copied().filter(|(id, _)| exists(id)).collect()
}

/// Whether `data/craft/<id>/craft.ron` exists.
pub fn craft_exists(id: &str) -> bool {
    sim::craft::default_craft_dir().join(id).join(sim::craft::CRAFT_FILE).exists()
}

/// The craft a game starts with: `wanted` (`SUNSCATTER_CRAFT`) when it is
/// one that exists; else the test craft in the demo (its script flies the
/// test craft) and the first available choice in play (the Falcon 9 once
/// its files exist), else the test craft.
pub fn startup_craft(wanted: Option<&str>, available: &[&str], demo: bool) -> String {
    match wanted.filter(|w| available.contains(w)) {
        Some(w) => w.to_string(),
        None if demo => sim::craft::TEST_CRAFT.to_string(),
        None => available.first().map_or(sim::craft::TEST_CRAFT, |c| c).to_string(),
    }
}

/// The craft `id` names, or the test craft (with the reason logged) when it
/// does not build: a broken craft file never stops the game.
pub fn craft_or_test(id: &str) -> Arc<Craft> {
    let id = CraftId(id.to_string());
    if id.0 != sim::craft::TEST_CRAFT {
        if let Err(e) = sim::craft::registry::definition(&id) {
            error!("craft {:?} does not build, flying the test craft: {e}", id.0);
            return craft_of(&CraftId(sim::craft::TEST_CRAFT.into()));
        }
    }
    craft_of(&id)
}

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
    /// The origin's clock (D088): the launch site's proper time, the time
    /// the game shows (it reads TT); ship clocks are shown against it. The
    /// physics runs on TDB (`clock`).
    pub site: SiteClock,
    /// The origin's name.
    pub origin: String,
    /// The game's seed, made once per new game and saved with it: the
    /// failure fates of the vessels it creates are drawn from it (D105).
    pub seed: u64,
}

/// A new game's seed: the wall clock and the process, mixed (tests: 0, so
/// their fates are the same every run).
fn new_game_seed() -> u64 {
    if cfg!(test) {
        return 0;
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    let z = (nanos ^ (u64::from(std::process::id()) << 32)).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    z ^ (z >> 31)
}

/// How far the clock moves before the dominance radii are rebuilt (s).
const DOMINANCE_REFRESH: f64 = 86_400.0;

/// The origin (D088, the owner's "origin location (launch site)"): the
/// first launch site in the comms data, its name and its clock, set to TT
/// at the ephemeris' first epoch.
fn origin_clock(world: &World) -> (String, SiteClock) {
    let data = sim::comms::CommsData::load(&sim::comms::CommsData::default_path()).unwrap_or_else(|e| panic!("{e}"));
    let site =
        data.sites.iter().find(|s| s.kind == sim::comms::SiteKind::LaunchSite).expect("a launch site in comms.ron");
    let body = world.find(&site.body).expect("the launch site's body");
    let fixed = site.fixed(body.physical.as_ref().expect("a solid body"));
    (site.name.clone(), SiteClock::reading_tt(body.node, fixed, sol::sol_epoch()))
}

fn load_world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    World::sol(Arc::new(Ephemeris::from_bytes(&bytes).expect("valid ephemeris")))
}

impl SimState {
    /// A new game with the test craft on the pad (tests).
    #[cfg(test)]
    pub fn new() -> Self {
        Self::with_craft(test_craft())
    }

    /// A new game with `craft` on the pad, in daylight over Florida:
    /// 2030-01-01 17:00 TDB.
    pub fn with_craft(craft: &Craft) -> Self {
        Self::starting_with(sol::sol_epoch().add_seconds(17.0 * 3600.0), craft)
    }

    /// A new game whose clock starts at `clock` (the body tour's date),
    /// with the test craft on the pad.
    pub fn starting_at(clock: Epoch) -> Self {
        Self::starting_with(clock, test_craft())
    }

    fn starting_with(clock: Epoch, craft: &Craft) -> Self {
        let world = load_world();
        let (origin, mut site) = origin_clock(&world);
        let mut vessel_ids = VesselIds::default();
        let mut ship = Vessel::landed_at(&world, vessel_ids.allocate(), "Earth", PAD_LAT, PAD_LON, clock, craft);
        ship.set_clock_offset(&world, site.offset_at(&world, clock));
        let seed = new_game_seed();
        ship.seed_fates(seed);
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
            site,
            origin,
            seed,
        }
    }

    /// The origin's clock at the game clock (D088): what the game shows.
    pub fn site_time(&self) -> Epoch {
        self.clock.add_seconds(self.site.offset())
    }

    /// Brings the origin's clock to the game clock (after it moves).
    pub fn advance_site_clock(&mut self) {
        let (world, clock) = (&self.world, self.clock);
        self.site.offset_at(world, clock);
    }

    /// Vessel `i`'s ship clock against the origin's (s, D088).
    pub fn ship_clock_offset(&self, i: usize) -> f64 {
        self.fleet[i].proper_time_offset() - self.site.offset()
    }

    /// A new vessel's clock, set from the origin's (D088).
    fn synchronised(&mut self, mut vessel: Vessel) -> Vessel {
        let offset = self.site.offset_at(&self.world, vessel.time);
        vessel.set_clock_offset(&self.world, offset);
        vessel
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
    /// with the throttle closed (and not held live on the pad); otherwise
    /// capped at physics warp.
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
        // Held on the pad with its systems live (engines lit, even at their
        // minimum with the throttle closed; the release armed): ticked in
        // place every 20 ms, so never at rails speed.
        if let Some(v) = self.fleet.iter().find(|v| held_live(v)) {
            return Some(if v.id() == ship.id() { RailsBlock::Held } else { RailsBlock::OtherHeld(v.id()) });
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

    /// Puts the active vessel back on the pad as a whole new craft of its
    /// own kind (a piece's base craft).
    pub fn reset(&mut self) {
        let base = self.ship().craft.id.base().to_string();
        self.new_flight(&craft_or_test(&base));
    }

    /// Replaces the active vessel with a new one of `craft` on the pad
    /// (fuelled at its loading, crew aboard; D104).
    pub fn new_flight(&mut self, craft: &Craft) {
        let id = self.vessel_ids.allocate();
        let mut ship = Vessel::landed_at(&self.world, id, "Earth", PAD_LAT, PAD_LON, self.clock, craft);
        ship.seed_fates(self.seed);
        self.fleet[self.active] = self.synchronised(ship);
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
            let vessel = Vessel::coasting(&self.world, id, self.clock, earth.node, r, v, test_craft());
            let mut vessel = self.synchronised(vessel);
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
    sim.advance_site_clock();
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
    /// Held on the pad's clamps with its systems live ([`held_live`]).
    Held,
    /// Another vessel is held on the pad with its systems live.
    OtherHeld(VesselId),
}

/// Whether vessel `v` is held on the pad's clamps with its systems needing
/// live ticks (an engine starting, running or in its tail-off, the release
/// armed, a mechanism moving: `sim::systems::ship::busy`): the sim then
/// ticks it in place every 20 ms (`Vessel::advance_held`), whatever the
/// throttle.
pub fn held_live(v: &Vessel) -> bool {
    v.clamped() && v.ship().is_some_and(sim::systems::ship::busy)
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

/// The active vessel's predicted path where its stored trajectory does not
/// give it (`trajectory::predict`, D089): all of it while the vessel flies
/// live; from the entry on while it coasts into air that needs live flight
/// (its stored segment is cut there). Computed on a background task and
/// swapped in atomically when complete, so the drawn line never flickers.
#[derive(Resource, Default)]
pub struct Prediction {
    /// The predicted segments, in time order.
    pub segments: Vec<Segment>,
    /// While coasting: the vessel and the start of the stored segment the
    /// prediction takes over from (at its entry).
    pub after: Option<(VesselId, Epoch)>,
    task: Option<Task<trajectory::predict::Path>>,
    since_last: f64,
}

pub fn update_prediction(
    time: Res<Time>,
    sim: Res<SimState>,
    orbits: Res<OrbitSettings>,
    mut pred: ResMut<Prediction>,
) {
    if let Some(task) = pred.task.as_mut() {
        if let Some(path) = check_ready(task) {
            (pred.segments, pred.after) = (path.segments, path.after);
            pred.task = None;
        }
    }
    let ship = sim.ship();
    if !trajectory::predict::needed(ship) {
        (pred.segments, pred.after) = (Vec::new(), None);
        return;
    }
    pred.since_last += time.delta_secs_f64();
    if pred.task.is_none() && pred.since_last > 0.25 {
        pred.since_last = 0.0;
        let (world, vessel, controls, orbits) = (sim.world.clone(), ship.clone(), sim.controls, orbits.clone());
        pred.task = Some(
            AsyncComputeTaskPool::get()
                .spawn(async move { trajectory::predict::path(&world, &vessel, &controls, &orbits) }),
        );
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
