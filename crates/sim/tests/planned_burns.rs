//! Planned burns of a craft of parts (D126, D129; realism-1 §6a): nodes
//! flown live by the maneuver program (`sim::systems::maneuver`) on the
//! small rocket of parts (`craft::small`). Nodes leave the stored trajectory
//! as it is; the vessel goes live at the program's wake epoch whatever the
//! warp, and the course is the same bits at every warp.

use glam::DVec3;
use sim::craft::small;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::systems::maneuver::{Node, Step};
use sim::time::Epoch;
use sim::vessel::{Controls, Phase, PlanError, Vessel, VesselId, VesselIds};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

fn earth(w: &World) -> (NodeId, f64) {
    let e = w.find("Earth").unwrap();
    (e.node, e.gm)
}

/// The small rocket in a 400 km orbit about Earth, anchored at `anchor`.
fn leo_at(w: &World, anchor: NodeId) -> Vessel {
    let (earth, mu) = earth(w);
    let (r, v) = Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(mu);
    let k = w.eph.relative(earth, anchor, t0());
    Vessel::coasting(w, VesselId(1), t0(), anchor, r + k.r, v + k.v, &small::rocket())
}

fn leo_ship(w: &World) -> Vessel {
    leo_at(w, earth(w).0)
}

/// A node of `dv` (prograde, normal, radial-out; m/s) at `t0 + at` s,
/// relative to Earth.
fn node(w: &World, at: f64, dv: DVec3) -> Node {
    Node { t: t0().add_seconds(at), dv, reference: earth(w).0 }
}

fn prograde(w: &World, at: f64, dv: f64) -> Node {
    node(w, at, DVec3::new(dv, 0.0, 0.0))
}

/// The vessel's state relative to Earth now.
fn earth_state(w: &World, ship: &Vessel) -> (DVec3, DVec3) {
    let (anchor, r, v) = ship.state(w);
    let k = w.eph.relative(anchor, earth(w).0, ship.time);
    (r + k.r, v + k.v)
}

/// Advances at 60 fps from the ship's time to `end`, with look-ahead in odd
/// chunks.
fn fly_frames(w: &World, ship: &mut Vessel, end: Epoch, controls: &Controls) {
    let start = ship.time;
    let mut k = 0u64;
    loop {
        k += 1;
        let t = start.add_seconds(k as f64 / 60.0);
        let t = if t > end { end } else { t };
        ship.advance(w, t, controls, usize::MAX);
        if k.is_multiple_of(7) {
            ship.extend_coast(w, t.add_seconds(900.0), 37);
        }
        if t == end {
            return;
        }
    }
}

fn events(ship: &Vessel) -> Vec<String> {
    ship.ship_events().iter().map(|e| e.text.clone()).collect()
}

#[test]
fn nodes_flown_in_frames_equal_one_jump_bit_for_bit() {
    let w = world();
    let mut frames = leo_ship(&w);
    frames.set_nodes(vec![prograde(&w, 300.0, 60.0), prograde(&w, 900.0, -20.0)]).unwrap();
    let mut jump = frames.clone();
    let controls = Controls { sas: true, ..Controls::default() };
    let end = t0().add_seconds(1_100.0);
    fly_frames(&w, &mut frames, end, &controls);
    jump.advance(&w, end, &controls, usize::MAX);
    assert_eq!(frames.time, jump.time);
    assert_eq!(frames.state(&w), jump.state(&w));
    assert_eq!(frames.attitude, jump.attitude);
    assert_eq!(frames.mass(), jump.mass());
    assert_eq!(frames.program(), jump.program());
    // The look-ahead ran further; compare the stored part both have.
    jump.extend_coast(&w, frames.trajectory().unwrap().computed_until(), usize::MAX);
    assert_eq!(frames.trajectory(), jump.trajectory());
    let flown: Vec<_> = events(&frames).into_iter().filter(|e| e.contains("on target")).collect();
    assert_eq!(flown.len(), 2, "{:?}", events(&frames));
}

#[test]
fn nodes_leave_the_coast_as_it_is_and_chunked_equals_single_pass() {
    // The stored trajectory is the coast without the burns (rule 4: what
    // will be flown until the program wakes); chunked = single pass.
    let w = world();
    let plain = leo_ship(&w);
    let mut planned = plain.clone();
    planned.set_nodes(vec![prograde(&w, 200.0, 80.0), prograde(&w, 400.0, 40.0)]).unwrap();
    assert_eq!(planned.trajectory(), plain.trajectory(), "setting nodes leaves the coast");
    let until = t0().add_seconds(3_000.0);
    let (mut one, mut chunked) = (plain.clone(), planned.clone());
    one.extend_coast(&w, until, usize::MAX);
    for (i, n) in [1usize, 7, 3, 50, 1, 2, 11].iter().cycle().enumerate() {
        if chunked.trajectory().unwrap().computed_until() >= until {
            break;
        }
        // Alternate step budgets and intermediate targets.
        let target = t0().add_seconds(40.0 * i as f64);
        chunked.extend_coast(&w, if i % 2 == 0 { until } else { target }, *n);
    }
    let (a, b) = (one.trajectory().unwrap(), chunked.trajectory().unwrap());
    let upto = a.computed_until().min_epoch(b.computed_until());
    assert!(upto >= until);
    assert_eq!(a.eval(upto), b.eval(upto));
    assert_eq!(a.segments, b.segments);
    // The vessel stops at the wake, like an order, and flies live from there.
    let wake = planned.program().unwrap().wake;
    planned.advance(&w, wake, &Controls::default(), usize::MAX);
    assert_eq!(planned.time, wake);
    let mut past = planned.clone();
    past.advance(&w, wake.add_seconds(1.0), &Controls::default(), usize::MAX);
    assert!(matches!(past.phase, Phase::Powered { .. }), "live from the wake");
    assert_eq!(past.program().unwrap().step, Step::Turn);
}

trait MinEpoch {
    fn min_epoch(self, other: Epoch) -> Epoch;
}
impl MinEpoch for Epoch {
    fn min_epoch(self, other: Epoch) -> Epoch {
        if self > other {
            other
        } else {
            self
        }
    }
}

#[test]
fn a_prograde_node_raises_apoapsis_as_the_rocket_equation_predicts() {
    let w = world();
    let (_, mu) = earth(&w);
    let mut ship = leo_ship(&w);
    let dv = 100.0;
    let n = prograde(&w, 600.0, dv);
    ship.set_nodes(vec![n]).unwrap();
    ship.extend_coast(&w, n.t.add_seconds(60.0), usize::MAX);
    let predicted = ship.node_prediction(&w, 0).unwrap();
    let before = {
        let (anchor, r, v) = ship.trajectory().unwrap().eval(n.t).unwrap();
        let k = w.eph.relative(anchor, earth(&w).0, n.t);
        Elements::from_state(r + k.r, v + k.v, mu).apoapsis()
    };
    let impulsive = Elements::from_state(predicted.after.1, predicted.after.2, mu).apoapsis();
    // The flown state carried back to the node along its conic (seconds:
    // J2 moves it by centimetres per second), against the prediction.
    let at_node = |s: &Vessel| {
        let (r, v) = earth_state(&w, s);
        sim::kepler::propagate(r, v, mu, n.t.seconds_since(s.time)).unwrap()
    };
    // Just after the burn and its tail-off.
    let duration = predicted.duration.unwrap();
    ship.advance(&w, predicted.ignite, &Controls::default(), usize::MAX);
    let m0 = ship.mass();
    let end = n.t.add_seconds(duration + 5.0);
    ship.advance(&w, end, &Controls::default(), usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Done, "{:?}", events(&ship));
    let (r1, v1) = at_node(&ship);
    let flown = Elements::from_state(r1, v1, mu).apoapsis();
    let off = (v1 - predicted.after.2).length();
    println!(
        "apoapsis {before:.0} → {flown:.1} m (impulsive {impulsive:.1} m), burn {duration:.2} s, {off:.3} m/s off"
    );
    assert!(flown - before > 300_000.0, "raised by {} m", flown - before);
    // Finite-burn and steering losses over a ~3 s arc, and the cutoff:
    // tenths of a metre per second (~3.6 km of apoapsis per m/s).
    assert!(off < 0.5, "{off} m/s from the prediction");
    assert!((flown - impulsive).abs() < 2_000.0, "flown {flown}, predicted {impulsive}");
    // The rocket equation: the Δv given is the exhaust speed times the log
    // of the mass ratio (the thrusters' nitrogen held the attitude: a few
    // kilograms of the ~300 burnt).
    let m1 = ship.mass();
    let isp = 320.0 * sim::vessel::G0;
    let given = isp * sim::math::ln(m0 / m1);
    println!("{:.1} kg burnt: rocket equation {given:.2} m/s", m0 - m1);
    assert!((given - dv).abs() < 0.02 * dv, "rocket equation Δv {given}");
    // The mass stays the same in the coast after.
    ship.advance(&w, end.add_seconds(300.0), &Controls::default(), usize::MAX);
    assert_eq!(ship.mass(), m1);
    assert!(matches!(ship.phase, Phase::Coasting { .. }));
}

#[test]
fn editing_a_later_node_keeps_the_next_program_and_equals_planning_it_so() {
    let w = world();
    let n1 = prograde(&w, 300.0, 60.0);
    let n2 = prograde(&w, 2_000.0, 30.0);
    let n2_new = node(&w, 2_500.0, DVec3::new(0.0, 20.0, -5.0));
    let mut ship = leo_ship(&w);
    ship.set_nodes(vec![n1, n2]).unwrap();
    ship.advance(&w, t0().add_seconds(100.0), &Controls::default(), usize::MAX);
    let program = ship.program().unwrap().clone();
    let coast = ship.trajectory().unwrap().clone();
    ship.set_nodes(vec![n1, n2_new]).unwrap();
    // The next node's program and the coast are untouched.
    assert_eq!(ship.program(), Some(&program));
    assert_eq!(ship.trajectory(), Some(&coast));
    // The edited plan equals a vessel planned that way from the start.
    let mut fresh = leo_ship(&w);
    fresh.set_nodes(vec![n1, n2_new]).unwrap();
    fresh.advance(&w, t0().add_seconds(100.0), &Controls::default(), usize::MAX);
    assert_eq!(ship, fresh);
    // Editing the next node while coasting towards it schedules it anew,
    // from the stage estimate now (the LOX boiling off since moves it by
    // microseconds): the same as a vessel given those nodes now, after
    // both burns.
    let n1_new = prograde(&w, 350.0, 70.0);
    ship.set_nodes(vec![n1_new, n2_new]).unwrap();
    assert_ne!(ship.program(), Some(&program));
    let mut fresh = leo_ship(&w);
    fresh.advance(&w, t0().add_seconds(100.0), &Controls::default(), usize::MAX);
    fresh.set_nodes(vec![n1_new, n2_new]).unwrap();
    assert_eq!(ship, fresh);
    // Without nodes no program waits: the vessel coasts past the old wake.
    let mut none = ship.clone();
    none.set_nodes(vec![]).unwrap();
    assert!(none.program().is_none());
    let past = t0().add_seconds(600.0);
    assert_eq!(none.advance_frame(&w, past, &Controls::default(), usize::MAX).0, past);
    assert!(matches!(none.phase, Phase::Coasting { .. }));
    let end = t0().add_seconds(2_700.0);
    ship.advance(&w, end, &Controls::default(), usize::MAX);
    fresh.advance(&w, end, &Controls::default(), usize::MAX);
    assert_eq!(ship.state(&w), fresh.state(&w));
    assert_eq!(ship, fresh);
    assert_eq!(ship.program().map(|p| (p.node, p.step)), Some((1, Step::Done)), "{:?}", events(&ship));
}

#[test]
fn node_edits_that_rewrite_the_past_are_rejected() {
    let w = world();
    let mut ship = leo_ship(&w);
    let n1 = prograde(&w, 300.0, 100.0);
    assert_eq!(ship.set_nodes(vec![prograde(&w, 400.0, 1.0), n1]), Err(PlanError::NotSorted));
    ship.set_nodes(vec![n1]).unwrap();
    // Inside the burn (~4 s about the node).
    ship.advance(&w, n1.t, &Controls::default(), usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Burn);
    assert_eq!(ship.set_nodes(vec![prograde(&w, 300.0, 50.0)]), Err(PlanError::Started));
    assert_eq!(ship.set_nodes(vec![]), Err(PlanError::Started));
    assert_eq!(ship.set_nodes(vec![n1, prograde(&w, 200.0, 5.0)]), Err(PlanError::NotSorted));
    assert_eq!(ship.set_nodes(vec![n1, prograde(&w, 299.5, 5.0)]), Err(PlanError::NotSorted));
    // A later node can be added while it burns, but not one in the past.
    ship.advance(&w, n1.t.add_seconds(1.0), &Controls::default(), usize::MAX);
    assert_eq!(ship.set_nodes(vec![n1, prograde(&w, 300.5, 5.0)]), Err(PlanError::InThePast));
    ship.set_nodes(vec![n1, prograde(&w, 900.0, 5.0)]).unwrap();
    assert_eq!(ship.program().unwrap().node, 0, "the burn flies on");
    ship.advance(&w, n1.t.add_seconds(30.0), &Controls::default(), usize::MAX);
    assert_eq!(ship.program().map(|p| (p.node, p.step)), Some((1, Step::Wait)));
}

#[test]
fn a_node_saved_mid_burn_continues_bit_identically() {
    let w = world();
    let mut ship = leo_ship(&w);
    let n = prograde(&w, 300.0, 100.0);
    ship.set_nodes(vec![n, prograde(&w, 1_500.0, 10.0)]).unwrap();
    ship.advance(&w, n.t.add_seconds(-0.5), &Controls::default(), usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Burn);
    let mut ids = VesselIds::default();
    ids.allocate();
    let text = SaveGame::capture(&w, ship.time, std::slice::from_ref(&ship), ids, 0).to_ron();
    let mut loaded = SaveGame::from_ron(&text).unwrap().vessels.remove(0);
    assert_eq!(loaded, ship);
    let end = t0().add_seconds(1_000.0);
    ship.advance(&w, end, &Controls::default(), usize::MAX);
    loaded.advance(&w, end, &Controls::default(), usize::MAX);
    assert_eq!(loaded, ship);
    assert_eq!(ship.nodes().len(), 2);
}

/// Rule 1: the anchor the vessel is integrated in does not change the burn
/// (the node's reference, Earth, is reached through the frame tree).
#[test]
fn a_node_does_not_depend_on_the_anchor() {
    let w = world();
    let (earth, _) = earth(&w);
    let moon = w.find("Moon").unwrap().node;
    let n = prograde(&w, 300.0, 100.0);
    // The same craft pointing the same way (a new vessel points its nose
    // away from its anchor: from the Moon, elsewhere).
    let attitude = leo_at(&w, earth).attitude;
    let run = |anchor: NodeId| {
        let mut ship = leo_at(&w, anchor);
        ship.attitude = attitude;
        ship.set_nodes(vec![n]).unwrap();
        ship.advance(&w, t0().add_seconds(400.0), &Controls::default(), usize::MAX);
        assert_eq!(ship.program().unwrap().step, Step::Done);
        let at: Vec<_> = ship.ship_events().iter().map(|e| (e.at.seconds_since(t0()), e.text.clone())).collect();
        println!("{anchor:?}: {at:?}");
        earth_state(&w, &ship)
    };
    let (a, b) = (run(earth), run(moon));
    let (dr, dv) = ((a.0 - b.0).length(), (a.1 - b.1).length());
    println!("Earth- vs Moon-anchored node: {dr:.4} m, {dv:.2e} m/s");
    assert!(dr < 0.5 && dv < 1e-3, "{dr} m, {dv} m/s");
}

#[test]
fn a_node_beyond_the_propellant_burns_out_ends_short_and_was_warned_of() {
    // More Δv than the stage has: the planner says so; the engines burn
    // to depletion (no division by an empty mass), the program ends short
    // and the craft coasts.
    let w = world();
    let mut ship = leo_ship(&w);
    let n = prograde(&w, 600.0, 20_000.0);
    ship.set_nodes(vec![n]).unwrap();
    let p = ship.node_prediction(&w, 0).unwrap();
    // The small rocket reads no tank levels: its Δv is not known, nor
    // warned of (a stage that reads them is, below).
    assert_eq!(p.available, None);
    assert!(p.duration.is_some_and(f64::is_finite));
    let end = n.t.add_seconds(p.duration.unwrap() + 600.0);
    let m0 = ship.mass();
    ship.advance(&w, end, &Controls::default(), usize::MAX);
    println!("{:?}", events(&ship));
    let prog = ship.program().unwrap();
    assert_eq!(prog.step, Step::Short, "{:?}", events(&ship));
    assert!(events(&ship).iter().any(|e| e.contains("ended short")));
    assert!(matches!(ship.phase, Phase::Coasting { .. }), "coasting after burnout");
    assert!(ship.mass() < m0);
    assert!(ship.mass() > 0.0 && ship.mass().is_finite());
    // The Falcon 9's second stage reads its tanks' levels (falcon9.md §5.2):
    // the planner says how much of the node its engine has.
    let craft = sim::craft::craft_of(&sim::craft::CraftId("falcon9-crew#0-11".into()));
    let (r, v) = earth_state(&w, &leo_ship(&w));
    let mut s2 = Vessel::coasting(&w, VesselId(2), t0(), earth(&w).0, r, v, &craft);
    s2.set_nodes(vec![n]).unwrap();
    let p = s2.node_prediction(&w, 0).unwrap();
    println!("second stage: {:?} m/s, {:?}", p.available, p.warnings);
    assert!(p.available.is_some_and(|a| a > 1_000.0 && a < 20_000.0));
    assert!(p.warnings.iter().any(|w| w.contains("the engines have")), "{:?}", p.warnings);
}

#[test]
fn debug_mode_nodes_take_no_propellant() {
    let w = world();
    let mut ship = leo_ship(&w);
    ship.set_debug(&w, true);
    let n = prograde(&w, 300.0, 200.0);
    ship.set_nodes(vec![n]).unwrap();
    let ignite = ship.program().unwrap().ignite;
    ship.advance(&w, ignite.add_seconds(-1.0), &Controls::default(), usize::MAX);
    let before = ship.propellant();
    let end = n.t.add_seconds(60.0);
    ship.advance(&w, end, &Controls::default(), usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Done, "{:?}", events(&ship));
    // The engines took nothing (the LOX's boil-off goes on: grams).
    println!("{} kg less", before - ship.propellant());
    assert!((ship.propellant() - before).abs() < 0.01, "{} kg", before - ship.propellant());
    // Turning debug mode off keeps the propellant.
    let kept = ship.propellant();
    ship.set_debug(&w, false);
    assert!(!ship.debug() && ship.propellant() == kept);
}

/// A frame target earlier than the vessel's own time (the game's next
/// frame can ask for less than a vessel that finished first reached) does
/// nothing: no rewind, and no panic, before the wake, in the turn, mid-burn
/// and after (review of 2026-10-26, PERF 1).
#[test]
fn a_target_before_the_vessel_does_nothing() {
    let w = world();
    let controls = Controls { sas: true, ..Controls::default() };
    let mut ship = leo_ship(&w);
    let n = prograde(&w, 2_000.0, 100.0);
    ship.set_nodes(vec![n]).unwrap();
    let wake = ship.program().unwrap().wake.seconds_since(t0());
    for (at, back) in [
        (1_000.0, 500.0),
        (wake + 10.0, 1_000.0),
        (1_999.0, wake + 5.0),
        (2_000.5, 1_999.0),
        (3_000.0, 1_000.0),
        (3_000.0, 2_005.0),
    ] {
        ship.advance(&w, t0().add_seconds(at), &controls, usize::MAX);
        let before = ship.clone();
        let (reached, spent) = ship.advance_frame(&w, t0().add_seconds(back), &controls, usize::MAX);
        assert_eq!((reached, spent), (before.time, 0), "at {at} s, back to {back} s");
        assert_eq!(ship, before);
        assert_eq!(ship.advance(&w, t0().add_seconds(back), &controls, usize::MAX), before.time);
    }
}

#[test]
fn a_rails_warp_frame_stops_at_the_wake_and_goes_live_there() {
    // D129: a frame of 16,667 s (10⁶× at 60 fps) stops where the program
    // wakes, like an order, and the turn begins at its own epoch.
    let w = world();
    let mut ship = leo_ship(&w);
    let n = prograde(&w, 900.0, 40.0);
    ship.set_nodes(vec![n]).unwrap();
    let wake = ship.program().unwrap().wake;
    let mut one = ship.clone();
    let (reached, _) = ship.advance_frame(&w, t0().add_seconds(16_667.0), &Controls::default(), usize::MAX);
    assert_eq!(reached, wake, "the frame stopped at the wake");
    assert_eq!(ship.program().unwrap().step, Step::Turn, "woken at its epoch");
    // On from there: the turn started at the wake, and the course is the
    // vessel's that flew there at 1×.
    let end = n.t.add_seconds(30.0);
    ship.advance(&w, end, &Controls::default(), usize::MAX);
    fly_frames(&w, &mut one, end, &Controls::default());
    let turn = ship.ship_events().iter().find(|e| e.text.contains("turning to the burn")).unwrap();
    assert_eq!(turn.at, wake);
    assert_eq!(ship.program().unwrap().step, Step::Done);
    assert_eq!(ship.state(&w), one.state(&w));
    assert_eq!(ship.program(), one.program());
}

#[test]
fn a_throttle_change_mid_burn_hands_the_burn_to_the_pilot_and_keeps_the_next_node() {
    // D126's takeover: the pilot's throttle differs from the program's.
    let w = world();
    let mut ship = leo_ship(&w);
    let n = prograde(&w, 300.0, 100.0);
    ship.set_nodes(vec![n, prograde(&w, 1_500.0, 10.0)]).unwrap();
    ship.advance(&w, n.t.add_seconds(-1.0), &Controls::default(), usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Burn);
    // Input that touches what the program does not command (the chutes'
    // arming is left out: it would open them) leaves it flying.
    let other = Controls { speed: sim::vessel::SpeedReference::Surface, ..Controls::default() };
    ship.advance(&w, n.t.add_seconds(-0.5), &other, usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Burn);
    // A different throttle: the burn is the pilot's from this tick on.
    let half = Controls { throttle: 0.5, ..other };
    ship.advance(&w, n.t, &half, usize::MAX);
    assert!(events(&ship).iter().any(|e| e.contains("taken over by the pilot")), "{:?}", events(&ship));
    // The next node's program waits for its own wake.
    let p = ship.program().unwrap();
    assert_eq!((p.node, p.step), (1, Step::Wait));
    // The pilot's throttle flies: the engines run on at half.
    ship.advance(&w, n.t.add_seconds(2.0), &half, usize::MAX);
    assert!(matches!(ship.phase, Phase::Powered { .. }));
}

#[test]
fn the_planner_warns_of_a_burn_too_soon_for_the_turn() {
    let w = world();
    let mut ship = leo_ship(&w);
    let soon = prograde(&w, 5.0, 10.0);
    ship.set_nodes(vec![soon]).unwrap();
    let p = ship.node_prediction(&w, 0).unwrap();
    assert!(p.warnings.iter().any(|w| w.contains("too soon")), "{:?}", p.warnings);
    // A node far enough ahead: only the small rocket's settling warning.
    let mut ship = leo_ship(&w);
    ship.set_nodes(vec![prograde(&w, 900.0, 10.0)]).unwrap();
    let p = ship.node_prediction(&w, 0).unwrap();
    assert_eq!(p.warnings.len(), 1, "{:?}", p.warnings);
    assert!(p.warnings[0].contains("cannot be settled"));
}
