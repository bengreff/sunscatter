//! Planned burns of a craft of parts (D126, D129): a node flown live by
//! the maneuver program (`sim::systems::maneuver`) on the small rocket of
//! parts (`craft::small`), closed-loop to the orbit the node gives, the same
//! bits whatever the frames.

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
    // The last ticks throttled to end on it (`maneuver::last_ticks`): 0.19
    // m/s of 40 (2026-10-30), the steering's lag across the burn.
    assert!(dv < 0.3 && dr < 50.0, "{dr} m, {dv} m/s");
    // The prediction (the stored coast plus the Δv at once) against the
    // target (two-body over the settling and half the burn, ~25 s of J2).
    assert!(off < 0.6, "{off} m/s from the prediction");
    // D129: the same bits whatever the frames.
    assert_eq!(frames.time, jump.time);
    assert_eq!(frames.state(&w), jump.state(&w));
    assert_eq!(frames.mass(), jump.mass());
    assert_eq!(frames.program(), jump.program());
}
