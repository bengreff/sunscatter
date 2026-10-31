//! Planned burns of a craft of parts (D126, D129): a node flown live by
//! the maneuver program (`sim::systems::maneuver`) closed-loop to the orbit
//! the node gives, on the small rocket of parts (`craft::small`; the same
//! bits whatever the frames) and on the Falcon 9's second stage (its turn on
//! the gas it can afford, its settling on its nitrogen, its MVac's start).

use glam::DVec3;
use sim::craft::small;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::systems::maneuver::{Node, Step};
use sim::time::Epoch;
use sim::vessel::{Controls, Phase, Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

fn leo(w: &World) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, &small::rocket())
}

fn earth_elements(w: &World, ship: &Vessel) -> Elements {
    let earth = w.find("Earth").unwrap();
    let (anchor, r, v) = ship.state(w);
    let k = w.eph.relative(anchor, earth.node, ship.time);
    Elements::from_state(r + k.r, v + k.v, earth.gm)
}

#[test]
fn a_node_is_flown_to_its_orbit_the_same_in_frames_as_in_one_jump() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let mut frames = leo(&w);
    let node = Node { t: t0().add_seconds(900.0), dv: DVec3::new(40.0, 0.0, 0.0), reference: earth.node };
    frames.set_nodes(vec![node]).unwrap();
    let program = frames.program().unwrap().clone();
    assert_eq!(program.step, Step::Wait);
    assert!(program.wake < program.ignite && program.ignite < node.t, "{program:?}");
    // The orbit the node gives (the prediction), from the stored coast.
    frames.extend_coast(&w, node.t.add_seconds(60.0), usize::MAX);
    let after = frames.node_prediction(&w, 0).unwrap();
    // The small rocket's thruster blocks only turn it: it cannot settle,
    // and the planner says so (its start does what the physics says).
    println!("planner: {:?}", after.warnings);
    assert!(after.warnings.iter().any(|w| w.contains("cannot be settled")), "{:?}", after.warnings);
    let mut jump = frames.clone();
    let controls = Controls::default();
    // Just past the burn (≈1.4 s about the node) and its tail-off.
    let end = node.t.add_seconds(20.0);
    // The flown state carried to the node along its conic (the program's
    // model; over the tenths of a second since the cutoff J2 is nothing).
    let at_node = |s: &Vessel| {
        let (anchor, r, v) = s.state(&w);
        let k = w.eph.relative(anchor, earth.node, s.time);
        sim::kepler::propagate(r + k.r, v + k.v, earth.gm, node.t.seconds_since(s.time)).unwrap()
    };
    // Frames at 60 fps (the player at 1×) against one advance (10⁵×).
    let mut k = 0u64;
    let mut cut = None;
    while frames.time < end {
        k += 1;
        let t = t0().add_seconds(k as f64 / 60.0);
        frames.advance(&w, if t > end { end } else { t }, &controls, usize::MAX);
        if cut.is_none() && frames.program().is_some_and(|p| p.step == Step::Done) {
            cut = Some(at_node(&frames));
        }
    }
    while jump.time < end {
        jump.advance(&w, end, &controls, usize::MAX);
    }
    let p = frames.program().unwrap();
    println!(
        "{:?}, {:?} m/s left; events {:?}",
        p.step,
        p.left,
        frames.ship_events().iter().map(|e| &e.text).collect::<Vec<_>>()
    );
    assert_eq!(p.step, Step::Done);
    assert!(p.left.unwrap() < 0.2, "{:?}", p.left);
    assert!(matches!(frames.phase, Phase::Coasting { .. }), "coasting after the burn");
    let flown = earth_elements(&w, &frames);
    let target = Elements::from_state(after.after.1, after.after.2, earth.gm);
    println!("apoapsis {:.0} m (prediction {:.0} m)", flown.apoapsis(), target.apoapsis());
    // On the orbit the program closed the loop on, just after the cutoff.
    let (rn, vn) = cut.unwrap();
    let aim = p.target.unwrap();
    let (dr, dv) = ((rn - aim.0).length(), (vn - aim.1).length());
    let off = (vn - after.after.2).length();
    println!("at the node: {dr:.1} m, {dv:.3} m/s from the target ({off:.2} m/s from the prediction)");
    // The last ticks throttled to end on it (`maneuver::last_ticks`), the
    // steering held for the last seconds (`maneuver::STEER_HOLD`): 0.08 m/s
    // of 40 (2026-10-31; 0.19 while it steered to the end).
    assert!(dv < 0.2 && dr < 50.0, "{dr} m, {dv} m/s");
    // The prediction (the stored coast plus the Δv at once) against the
    // target (two-body over the settling and half the burn, ~25 s of J2).
    assert!(off < 0.6, "{off} m/s from the prediction");
    // D129: the same bits whatever the frames.
    assert_eq!(frames.time, jump.time);
    assert_eq!(frames.state(&w), jump.state(&w));
    assert_eq!(frames.mass(), jump.mass());
    assert_eq!(frames.program(), jump.program());
}

/// The Falcon 9's second stage with Crew Dragon (`falcon9-crew#0-11`) in
/// the same orbit: its MVac, its nitrogen pods (which push along it) and
/// its tanks' level sensors.
fn upper_stage(w: &World) -> Vessel {
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    let craft = sim::craft::craft_of(&sim::craft::CraftId("falcon9-crew#0-11".into()));
    Vessel::coasting(w, VesselId(1), t0(), earth.node, r, v, &craft)
}

/// The moles in the second stage's nitrogen bottle.
fn nitrogen(s: &Vessel) -> f64 {
    let def = s.ship_def().unwrap();
    let b = def.bottles.iter().position(|b| b.id == "s2.nitrogen").unwrap();
    s.ship().unwrap().bottles[b].moles
}

#[test]
fn the_second_stage_turns_on_the_gas_it_can_afford_settles_and_flies_the_node() {
    let w = world();
    let earth = w.find("Earth").unwrap();
    let mut s = upper_stage(&w);
    let node = Node { t: t0().add_seconds(900.0), dv: DVec3::new(100.0, 0.0, 0.0), reference: earth.node };
    s.set_nodes(vec![node]).unwrap();
    s.extend_coast(&w, node.t.add_seconds(60.0), usize::MAX);
    let after = s.node_prediction(&w, 0).unwrap();
    assert!(after.warnings.is_empty(), "{:?}", after.warnings);
    // The slew the gas affords (`vessel::ship::gas_cap`): a fraction of a
    // degree per second for this 120 t stack, where the turn cap allows 10.
    let cap = s.gas_turn_cap();
    println!("gas cap {:.3} deg/s", cap.to_degrees());
    assert!(cap < 1.0_f64.to_radians(), "{cap} rad/s");
    let full = nitrogen(&s);
    let controls = Controls::default();
    let at_node = |s: &Vessel| {
        let (anchor, r, v) = s.state(&w);
        let k = w.eph.relative(anchor, earth.node, s.time);
        sim::kepler::propagate(r + k.r, v + k.v, earth.gm, node.t.seconds_since(s.time)).unwrap()
    };
    let (mut fastest, mut turned, mut cut) = (0.0_f64, None, None);
    let end = node.t.add_seconds(30.0);
    while s.time < end {
        let t = s.time.add_seconds(0.5);
        s.advance(&w, if t > end { end } else { t }, &controls, usize::MAX);
        let p = s.program().unwrap();
        if p.step == Step::Turn {
            fastest = fastest.max(s.attitude.omega.length());
        }
        if turned.is_none() && p.step == Step::Settle {
            turned = Some(nitrogen(&s));
        }
        if cut.is_none() && p.step == Step::Done {
            cut = Some((at_node(&s), s.attitude.omega.length()));
        }
    }
    let p = s.program().unwrap();
    let events: Vec<_> = s.ship_events().iter().map(|e| e.text.clone()).collect();
    println!("{:?} {:?} m/s left; events {events:?}", p.step, p.left);
    assert_eq!(p.step, Step::Done, "{events:?}");
    // The turn kept to the cap and spent little of the bottle (it emptied
    // it on the turn cap alone: PLANNER-2, 2026-10-30).
    let turned = turned.unwrap();
    println!(
        "fastest turn {:.3} deg/s; nitrogen {full:.0} mol, {turned:.0} after the turn, {:.0} after the burn",
        fastest.to_degrees(),
        nitrogen(&s)
    );
    assert!(fastest < 1.05 * cap, "{fastest} rad/s against the cap {cap}");
    assert!(full - turned < 0.2 * full, "{full} → {turned} mol");
    // It settled on its nitrogen and started its MVac on settled propellant.
    assert!(nitrogen(&s) < turned, "the settling push spends nitrogen");
    assert!(p.lit_seen);
    // On the orbit the program closed the loop on, just after the cutoff,
    // not spinning: the steering held its direction for the last seconds.
    let ((rn, vn), spin) = cut.unwrap();
    let aim = p.target.unwrap();
    let (dr, dv) = ((rn - aim.0).length(), (vn - aim.1).length());
    let off = (vn - after.after.2).length();
    println!("at the node: {dr:.1} m, {dv:.3} m/s from the target ({off:.2} m/s from the prediction), {spin:.4} rad/s");
    assert!(spin < 0.002, "{spin} rad/s at the cutoff");
    assert!(dv < 0.5 && dr < 100.0, "{dr} m, {dv} m/s");
    assert!(off < 1.0, "{off} m/s from the prediction");
}
