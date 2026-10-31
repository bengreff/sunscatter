//! D129: the course of events is the same at every warp
//! (`docs/technical/warp-invariance.md`). A craft of parts with events (an
//! engine restart with its settling, a burn, a shutdown, a separation, the
//! lattices' eclipses, boil-off and venting, failure fates drawn from a
//! save's seed) is flown over a long span in one pass and in the game's
//! frames at 1×, 100×, 10⁴× and 10⁵×, and the whole fleet is compared bit
//! for bit (`Vessel: PartialEq`), with a hash of each vessel's saved form
//! and, on a difference, the first field that differs.
//!
//! The frames are the game's (`crates/game/src/state.rs` `advance`, `step`,
//! `rails_block`; `crates/game/src/fleet.rs`): 60 fps, the span asked for
//! ramped at most ×2 a frame, rails warp refused while any vessel is live
//! (capped at 4×), the 10 km bubble first, then each vessel in chunks of
//! 2,000 budget units by `advance_frame` (which stops where a vessel goes
//! live inside a frame), the clock held back at the slowest vessel, a
//! breakup split between frames, and the look-ahead extending the line.
//! The emulation is a copy of those rules, kept short; if the game's change,
//! change it here too.
//!
//! The divergences this audit found are reproduced by the `#[ignore]`d
//! tests at the end: each asserts the D129 property and fails today. Run
//! them with `cargo test -p sim --test warp_course -- --ignored`; the fix
//! un-ignores its test.

use sim::craft::command::ShipCommand;
use sim::craft::{craft_of, CraftId};
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::systems::OrderKind;
use sim::time::Epoch;
use sim::vessel::{bubble, Controls, HoldMode, Phase, TargetState, Vessel, VesselId, VesselIds, TICK};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

/// The game's frame: 60 fps.
const DT: f64 = 1.0 / 60.0;
/// Physics warp's top level (`game::state::WARP_LEVELS[MAX_PHYSICS_WARP]`).
const PHYSICS_WARP: f64 = 4.0;
/// `game::fleet::{CHUNK_UNITS, MAX_CHUNKS, GROWTH}`.
const CHUNK_UNITS: usize = 2_000;
const MAX_CHUNKS: usize = 256;
const GROWTH: f64 = 2.0;
/// `game::state::LOOKAHEAD_STEPS_PER_FRAME`, and about a low orbit ahead.
const LOOKAHEAD_STEPS: usize = 1_500;
const LOOKAHEAD: f64 = 6_000.0;

/// The game's fleet, each vessel's controls as set, its clock and the span
/// it moved last frame.
#[derive(Clone)]
struct Game {
    fleet: Vec<Vessel>,
    set: Vec<Controls>,
    /// The light delay of every vessel's settings (`Some`: through its helm,
    /// `Vessel::acting_controls`, as the game's `fleet_controls`; `None`:
    /// acted on as set, as aboard).
    delay: Option<f64>,
    /// `(a, b)`: vessel `a` holds on vessel `b`, its target state fed at the
    /// clock each frame (the game's `navball::feed_hold`).
    target_of: Option<(usize, usize)>,
    ids: VesselIds,
    clock: Epoch,
    last: f64,
    frames: usize,
    /// The vessels a bubble ever held (D082), and from when.
    members: Vec<(VesselId, Epoch)>,
    /// The warp stops taken (D128), in the order taken: as the game, each
    /// frame takes them after the advance (the vessel held the clock there).
    stops: Vec<(VesselId, Epoch, String)>,
}

impl Game {
    fn new(fleet: Vec<Vessel>, set: Vec<Controls>, ids: VesselIds) -> Self {
        let clock = fleet[0].time;
        Game {
            fleet,
            set,
            delay: None,
            target_of: None,
            ids,
            clock,
            last: 0.0,
            frames: 0,
            members: Vec::new(),
            stops: Vec::new(),
        }
    }

    /// The controls each vessel acts on in a frame from the clock.
    fn acting(&mut self, w: &World) -> Vec<Controls> {
        if let Some((a, b)) = self.target_of {
            let (anchor, r, v) = self.fleet[b].state_at(w, self.clock);
            self.set[a].target = Some(TargetState { anchor, epoch: self.clock, r, v });
        }
        let (clock, delay) = (self.clock, self.delay);
        (self.fleet.iter_mut().zip(&self.set))
            .map(|(v, c)| match delay {
                Some(d) => {
                    *v.controls_mut() = *c;
                    v.acting_controls(clock, Some(d))
                }
                None => *c,
            })
            .collect()
    }

    /// `game::state::rails_block` for this fleet: refused while any vessel
    /// is live or produces thrust (no pad, no rails floor in orbit).
    fn rails_allowed(&self) -> bool {
        !self.fleet.iter().any(|v| matches!(v.phase, Phase::Powered { .. }) || v.engine_running())
    }

    /// One frame at `warp` towards at most `end` (`game::state::advance`).
    fn frame(&mut self, w: &World, warp: f64, end: Epoch) {
        let acting = self.acting(w);
        let warp = if self.rails_allowed() { warp } else { warp.min(PHYSICS_WARP) };
        let wanted = DT * warp;
        let span = if self.last > 0.0 { wanted.min(GROWTH * self.last) } else { wanted };
        let target = self.clock.add_seconds(span);
        let target = if target > end { end } else { target };
        let before = self.clock;
        let of = |i: usize| acting[i];
        bubble::advance(w, &mut self.fleet, &of, target);
        for v in &self.fleet {
            if v.in_bubble() && !self.members.iter().any(|m| m.0 == v.id()) {
                self.members.push((v.id(), v.time));
            }
        }
        let mut clock = target;
        for (v, c) in self.fleet.iter_mut().zip(&acting) {
            for _ in 0..MAX_CHUNKS {
                let was = v.time;
                let (reached, spent) = v.advance_frame(w, target, c, CHUNK_UNITS);
                let stopped = matches!(&v.phase, Phase::Coasting { trajectory } if trajectory.finished());
                let more =
                    target.seconds_since(reached) > TICK && matches!(v.phase, Phase::Coasting { .. }) && !stopped;
                if !(more && (reached > was || spent > 0)) {
                    break;
                }
            }
            let stopped = matches!(&v.phase, Phase::Coasting { trajectory } if trajectory.finished());
            if !stopped && v.time.seconds_since(target) < -TICK && v.time < clock {
                clock = v.time;
            }
        }
        self.clock = clock;
        self.last = clock.seconds_since(before);
        self.take_stops();
        self.split_all(w);
        for v in &mut self.fleet {
            v.extend_coast(w, clock.add_seconds(LOOKAHEAD), LOOKAHEAD_STEPS);
        }
        self.frames += 1;
    }

    /// Takes every vessel's warp stop (`game::state::advance` drops the warp
    /// there; the vessel advances on).
    fn take_stops(&mut self) {
        for v in &mut self.fleet {
            if let Some(s) = v.take_warp_stop() {
                self.stops.push((v.id(), s.at, s.why));
            }
        }
    }

    /// Applies every breakup waiting (the game builds it on a task and
    /// applies it in a later frame; the vessel waits at its epoch).
    fn split_all(&mut self, w: &World) {
        let mut k = 0;
        while k < self.fleet.len() {
            if let Some(plan) = self.fleet[k].plan_breakup() {
                let crafts = plan.crafts();
                let pieces = self.fleet[k].split(w, &plan, &crafts, &mut self.ids);
                let piece = self.set[k].for_piece();
                self.set.extend(pieces.iter().map(|_| piece));
                self.fleet.extend(pieces);
            } else {
                k += 1;
            }
        }
    }

    /// Frames at `warp` until the clock reaches `end`.
    fn fly(&mut self, w: &World, warp: f64, end: Epoch) {
        let mut still = 0;
        while self.clock < end {
            let before = self.clock;
            self.frame(w, warp, end);
            still = if self.clock == before { still + 1 } else { 0 };
            assert!(still < 1_000, "the clock stalled at {:?}", self.clock);
        }
    }

    /// The whole fleet in one call each (every split applied at once), to
    /// `end`: the reference.
    fn one_pass(&mut self, w: &World, end: Epoch) {
        let mut k = 0;
        while k < self.fleet.len() {
            let c = self.set[k];
            self.fleet[k].advance(w, end, &c, usize::MAX);
            self.take_stops();
            if self.fleet[k].plan_breakup().is_some() {
                self.split_all(w);
                continue;
            }
            k += 1;
        }
        self.clock = end;
    }
}

/// FNV-1a over a vessel's saved form: RON prints each f64 in its shortest
/// form that reads back to the same bits, so equal bits give equal text.
fn hash(v: &Vessel) -> u64 {
    ron::ser::to_string(v)
        .expect("a vessel serialises")
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3))
}

/// The first field where `a` and `b` differ: the path of field names to
/// the first differing line of their pretty saved forms, and both lines.
fn first_difference(a: &Vessel, b: &Vessel) -> String {
    let pretty = |v: &Vessel| ron::ser::to_string_pretty(v, ron::ser::PrettyConfig::default()).expect("serialises");
    let (sa, sb) = (pretty(a), pretty(b));
    let mut path: Vec<(usize, String)> = Vec::new();
    for (la, lb) in sa.lines().zip(sb.lines()) {
        let indent = la.len() - la.trim_start().len();
        while path.last().is_some_and(|(i, _)| *i >= indent) {
            path.pop();
        }
        if la != lb {
            let names: Vec<&str> = path.iter().map(|(_, n)| n.as_str()).collect();
            return format!("{} :: {} | {}", names.join("."), la.trim(), lb.trim());
        }
        if let Some((name, _)) = la.trim().split_once(':') {
            if !name.contains(' ') && !name.contains('"') {
                path.push((indent, name.to_string()));
            }
        }
    }
    "the saved forms agree (a difference outside them)".into()
}

/// Asserts `framed` ends as `reference`: the same vessels, bit for bit
/// (each line extended alike first, as the stored line is the truth only
/// up to where it was computed).
fn assert_same_course(w: &World, framed: &mut Game, reference: &mut Game, what: &str) {
    let ids = |g: &Game| g.fleet.iter().map(|v| v.id()).collect::<Vec<_>>();
    assert_eq!(ids(framed), ids(reference), "{what}: the fleet");
    for (a, b) in framed.fleet.iter_mut().zip(reference.fleet.iter_mut()) {
        if let (Some(ta), Some(tb)) = (a.trajectory(), b.trajectory()) {
            let until =
                if ta.computed_until() > tb.computed_until() { ta.computed_until() } else { tb.computed_until() };
            a.extend_coast(w, until, usize::MAX);
            b.extend_coast(w, until, usize::MAX);
        }
        assert_eq!(a.time, b.time, "{what}: vessel {:?}'s time", a.id());
        if a != b {
            panic!("{what}: vessel {:?} differs; first: {}", a.id(), first_difference(a, b));
        }
        println!("{what}: vessel {:?} {:016x} ({} frames)", a.id(), hash(a), framed.frames);
    }
    // The warp stops (D128) at the same epochs: the reference's first one
    // per vessel among the frames' (a frame takes each, so later ones come
    // too).
    for stop in &reference.stops {
        assert!(framed.stops.contains(stop), "{what}: the warp stop {stop:?} not in {:?}", framed.stops);
    }
}

/// The Falcon 9's second stage with Crew Dragon (`falcon9-crew#0-11`) in a
/// 400 km orbit, its fates drawn from the save's `seed`.
fn upper_stage(w: &World, seed: u64) -> (Vessel, VesselIds) {
    let earth = w.find("Earth").unwrap();
    let el = Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 };
    let (r, v) = el.to_state(earth.gm);
    let craft = craft_of(&CraftId("falcon9-crew#0-11".into()));
    let mut s = Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, &craft);
    s.seed_fates(seed);
    assert!(matches!(s.phase, Phase::Coasting { .. }));
    let mut ids = VesselIds::default();
    ids.allocate();
    (s, ids)
}

/// The orders of the span (epochs off every lattice and frame): the MVac's
/// restart (its start sequence settles the propellant with the nitrogen
/// pods first), its shutdown 20 s later, Dragon's separation hours on.
fn orders(v: &mut Vessel) -> [Epoch; 3] {
    let ignite = t0().add_seconds(3_000.123);
    let cut = ignite.add_seconds(20.0);
    let separate = t0().add_seconds(20_000.5);
    v.order_at(ignite, OrderKind::Command(ShipCommand::Ignite("s2".into()))).unwrap();
    v.order_at(cut, OrderKind::Command(ShipCommand::Shutdown("s2".into()))).unwrap();
    v.order_at(separate, OrderKind::Command(ShipCommand::Separate("dragon separation".into()))).unwrap();
    [ignite, cut, separate]
}

fn controls() -> Controls {
    Controls { throttle: 1.0, sas: true, hold: HoldMode::Prograde, ..Controls::default() }
}

/// A save's seed whose fate for the MVac's restart is below (`fails`) or
/// far above the hazard of one start, `per_start` (D105 §6.3).
fn seed_for(launch: Epoch, owner: &str, mode: &str, per_start: f64, fails: bool) -> u64 {
    let hazard = sim::systems::failure::fates::per_event(per_start);
    (0u64..)
        .find(|&s| {
            let e = sim::systems::failure::fates::threshold(s, launch, owner, mode);
            if fails {
                e < 0.5 * hazard
            } else {
                e > 1.0
            }
        })
        .expect("a seed")
}

/// The owner and per-start rate of the MVac's restart mode.
fn restart_mode(v: &Vessel) -> (String, f64) {
    let def = v.ship_def().expect("a craft of parts");
    let modes = sim::systems::failure::modes(&def);
    let (owner, _, _, m) = modes.iter().find(|(_, _, _, m)| m.mode == "restart").expect("the MVac's restart mode");
    let sim::catalog::Hazard::PerStart(p) = m.hazard else { panic!("per start") };
    (def.owner_id(*owner).to_string(), p)
}

/// The MVac's ignitions left, if the vessel has it.
fn mvac_ignitions(v: &Vessel) -> Option<u32> {
    let def = v.ship_def()?;
    let i = def.engines.iter().position(|e| e.id == "s2.mvac")?;
    Some(v.ship()?.engines[i].ignitions)
}

fn events(g: &Game) -> Vec<String> {
    g.fleet
        .iter()
        .flat_map(|v| {
            v.ship_events().iter().map(move |e| format!("{:?} +{:.3} s {}", v.id(), e.at.seconds_since(t0()), e.text))
        })
        .collect()
}

/// Flies the upper stage with `seed` over `span` in one pass and at each
/// warp, and checks every run ends the same; at 1× over `span_1x` only.
fn same_course(seed: u64, span: f64, span_1x: f64, warps: &[f64]) -> Game {
    let w = world();
    let (mut v, ids) = upper_stage(&w, seed);
    orders(&mut v);
    let start = Game::new(vec![v], vec![controls()], ids);
    let mut reference = start.clone();
    let end = t0().add_seconds(span);
    reference.one_pass(&w, end);
    let mut short = start.clone();
    let end_1x = t0().add_seconds(span_1x);
    short.one_pass(&w, end_1x);
    let mut at_1x = start.clone();
    at_1x.fly(&w, 1.0, end_1x);
    assert_same_course(&w, &mut at_1x, &mut short, "1×");
    let mut stops = None;
    for &warp in warps {
        let mut framed = start.clone();
        framed.fly(&w, warp, end);
        assert_same_course(&w, &mut framed, &mut reference.clone(), &format!("{warp}×"));
        let at = stops.get_or_insert_with(|| framed.stops.clone());
        assert_eq!(&framed.stops, at, "{warp}×: the warp stops");
    }
    for s in stops.unwrap_or_default() {
        println!("warp stop: {s:?}");
    }
    for e in events(&reference) {
        println!("{e}");
    }
    reference
}

/// Two days of the upper stage with Dragon (its MVac restarted and shut
/// down, Dragon separated hours later, then both pieces coasting through
/// 30 eclipses with their lattices), fates on and no failure drawn: the
/// same fleet, bit for bit, in one pass and in frames at every warp.
#[test]
fn the_upper_stage_and_dragon_fly_the_same_course_at_every_warp() {
    let w = world();
    let (v, _) = upper_stage(&w, 0);
    let (owner, per_start) = restart_mode(&v);
    let seed = seed_for(v.time, &owner, "restart", per_start, false);
    let before = mvac_ignitions(&v);
    let g = same_course(seed, 2.0 * 86_400.0, 3_100.0, &[100.0, 1e4, 1e5]);
    assert_eq!(g.fleet.len(), 2, "Dragon separated from the stage");
    let after = g.fleet.iter().find_map(mvac_ignitions).expect("the stage");
    assert_eq!(Some(after), before.map(|n| n - 1), "the MVac restarted once");
}

/// Notifications off (D128, software-layers.md §5): the course's two
/// risks (the stage's computer shed at +12.7 h, Dragon's cabin CO₂ after a
/// day) stop nothing, on the vessel or the piece it split off, and the
/// flight is the same bits as one pass.
#[test]
fn with_notifications_off_nothing_stops_warp() {
    let w = world();
    let (mut v, ids) = upper_stage(&w, 0);
    orders(&mut v);
    v.set_notify(false);
    let start = Game::new(vec![v], vec![controls()], ids);
    let end = t0().add_seconds(1.5 * 86_400.0);
    let mut reference = start.clone();
    reference.one_pass(&w, end);
    let mut framed = start.clone();
    framed.fly(&w, 1e5, end);
    assert_same_course(&w, &mut framed, &mut reference, "10⁵×, notifications off");
    assert!(framed.stops.is_empty() && reference.stops.is_empty(), "{:?}", framed.stops);
    assert!(framed.fleet.iter().all(|v| !v.notify()), "the piece keeps the craft's setting");
}

/// The same span with a seed whose fate makes the MVac's restart fail
/// uncontained (D105): the failure, its explosion and the breakup it causes
/// happen at the same tick, and the pieces fly on the same, at every warp.
#[test]
fn an_uncontained_restart_fails_the_same_at_every_warp() {
    let w = world();
    let (v, _) = upper_stage(&w, 0);
    let (owner, per_start) = restart_mode(&v);
    let seed = seed_for(v.time, &owner, "restart", per_start, true);
    let g = same_course(seed, 86_400.0, 3_100.0, &[1e4, 1e5]);
    let failed = events(&g).iter().any(|e| e.contains("uncontained failure"));
    assert!(failed, "the restart failed: {:#?}", events(&g));
}

// ---------------------------------------------------------------------
// Divergences (warp-invariance.md, "Divergences found"): each test asserts
// D129 and fails today. Un-ignore with the fix.
// ---------------------------------------------------------------------

/// `craft::small`'s rocket coasting in a 400 km orbit at mean anomaly `m`.
fn small_at(w: &World, id: u64, m: f64) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let el = Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: m };
    let (r, v) = el.to_state(earth.gm);
    let s = Vessel::coasting(w, VesselId(id), t0(), earth.node, r, v, &sim::craft::small::rocket());
    assert!(matches!(s.phase, Phase::Coasting { .. }));
    s
}

/// The angle (degrees) between two attitudes.
fn angle(a: &Vessel, b: &Vessel) -> f64 {
    let d = (a.attitude.q.inverse() * b.attitude.q).normalize();
    2.0 * d.w.abs().min(1.0).acos().to_degrees()
}

fn two_runs(start: &Game, w: &World, span: f64) -> (Game, Game) {
    let end = t0().add_seconds(span);
    let mut at_1x = start.clone();
    at_1x.fly(w, 1.0, end);
    let mut fast = start.clone();
    fast.fly(w, 1e5, end);
    (at_1x, fast)
}

/// D1: the TARGET hold on a vessel aims at a target state the game feeds
/// at each frame's clock and carries at constant velocity
/// (`vessel::hold::TargetState`, `game::navball::feed_hold`): at 1× it is
/// 16.7 ms old at most, at 10⁵× a whole 300 s frame, so the craft turns to
/// a different direction.
#[test]
#[ignore = "D129 divergence D1: TARGET hold fed per frame (warp-invariance.md)"]
fn d1_a_target_hold_on_a_vessel_is_the_same_at_every_warp() {
    let w = world();
    let (a, b) = (small_at(&w, 1, 0.0), small_at(&w, 2, 0.000_7));
    let hold = Controls { sas: true, hold: HoldMode::Target, ..Controls::default() };
    let mut start =
        Game::new(vec![a, b], vec![hold, Controls { sas: true, ..Controls::default() }], VesselIds::default());
    start.target_of = Some((0, 1));
    let (x, y) = two_runs(&start, &w, 300.0);
    let deg = angle(&x.fleet[0], &y.fleet[0]);
    println!("TARGET hold: 1× and 10⁵× attitudes {deg:.3}° apart after 300 s");
    assert_eq!(x.fleet[0].attitude, y.fleet[0].attitude, "{deg:.3}° apart");
}

/// D2: an uncrewed vessel's new setting (prograde instead of holding its
/// attitude), sent at t₀ with a 1.3 s light delay, reaches it at t₀ + 1.3 s,
/// but the vessel acts on it from the first frame start after that
/// (`Helm::acting`, called once per frame by `fleet_controls`): 1.32 s at
/// 1×, a whole 300 s frame at 10⁵×. The game's other delayed commands
/// (`game::commands::InFlight`: plans, holds, the sequence's edits) are
/// applied the same way; ship orders are not (`Vessel::order_at`, exact).
#[test]
#[ignore = "D129 divergence D2: delayed settings act at frame starts (warp-invariance.md)"]
fn d2_an_uncrewed_setting_acts_at_its_arrival_at_every_warp() {
    let w = world();
    let mut v = small_at(&w, 1, 0.0);
    v.set_crew(0);
    let prograde = Controls { sas: true, hold: HoldMode::Prograde, ..Controls::default() };
    let mut start = Game::new(vec![v], vec![prograde], VesselIds::default());
    start.delay = Some(1.3);
    let (x, y) = two_runs(&start, &w, 300.0);
    let deg = angle(&x.fleet[0], &y.fleet[0]);
    println!("delayed setting: 1× and 10⁵× attitudes {deg:.3}° apart after 300 s");
    assert_eq!(x.fleet[0].attitude, y.fleet[0].attitude, "{deg:.3}° apart");
}

/// D3: a craft 2 km behind another that lights its engines at an order's
/// epoch joins its 10 km bubble (D082) at the next tick at 1×; at 10⁵× it
/// was already advanced to the end of the frame where the other went live,
/// and `bubble::epoch` judges a craft past the lattice epoch only later,
/// so it joins late (or, as here, never: the leader is gone by then). It
/// flies live ticks at 1× and a coast at 10⁵×.
#[test]
#[ignore = "D129 divergence D3: a bubble's neighbour ahead of the lattice joins late (warp-invariance.md)"]
fn d3_a_neighbour_joins_the_bubble_at_the_same_epoch_at_every_warp() {
    let w = world();
    let (mut a, b) = (small_at(&w, 1, 0.000_3), small_at(&w, 2, 0.0));
    a.order_at(t0().add_seconds(100.37), OrderKind::Stage).unwrap();
    let sas = Controls { sas: true, throttle: 1.0, ..Controls::default() };
    let mut ids = VesselIds::default();
    ids.allocate();
    ids.allocate();
    let start = Game::new(vec![a, b], vec![sas, sas], ids);
    let (x, y) = two_runs(&start, &w, 300.0);
    let (bx, by) = (&x.fleet[1], &y.fleet[1]);
    let d = (bx.state(&w).1 - by.state(&w).1).length();
    let joined = |g: &Game| {
        g.members.iter().map(|(id, t)| format!("{id:?} at +{:.3} s", t.seconds_since(t0()))).collect::<Vec<_>>()
    };
    println!(
        "bubble: the neighbour {d:.6} m apart at 1× and 10⁵× after 300 s; held at 1×: {:?}; at 10⁵×: {:?}",
        joined(&x),
        joined(&y)
    );
    assert_eq!(bx.state(&w), by.state(&w), "{d} m apart");
}
