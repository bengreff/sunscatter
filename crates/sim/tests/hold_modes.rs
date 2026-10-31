//! Attitude hold modes, and planned burns flown by the maneuver hold
//! (D075, D126) on the small rocket of parts (`craft::small`).

use glam::DVec3;
use sim::craft::small;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::systems::maneuver::{self, Node, Step};
use sim::time::Epoch;
use sim::vessel::{
    prograde_normal_radial, Controls, HoldMode, Phase, SpeedReference, TargetState, Vessel, VesselId, VesselIds,
};
use sim::world::World;
use std::sync::Arc;
use std::time::Instant;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

fn earth(w: &World) -> NodeId {
    w.find("Earth").unwrap().node
}

fn leo_ship(w: &World) -> Vessel {
    let mu = w.find("Earth").unwrap().gm;
    let (r, v) = Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(mu);
    Vessel::coasting(w, VesselId(1), t0(), earth(w), r, v, &small::rocket())
}

/// A node of `dv` (prograde, normal, radial-out) at `t0 + at` s.
fn node(w: &World, at: f64, dv: DVec3) -> Node {
    Node { t: t0().add_seconds(at), dv, reference: earth(w) }
}

fn sas(hold: HoldMode) -> Controls {
    Controls { sas: true, hold, reference: None, ..Controls::default() }
}

/// State relative to Earth now.
fn earth_state(w: &World, ship: &Vessel) -> (DVec3, DVec3) {
    let (anchor, r, v) = ship.state(w);
    let k = w.eph.relative(anchor, earth(w), ship.time);
    (r + k.r, v + k.v)
}

/// The thrust axis of the main engines, inertial.
fn thrust_axis(ship: &Vessel) -> DVec3 {
    let def = ship.ship_def().unwrap();
    let body: DVec3 = def.engines.iter().filter(|e| e.main).map(|e| e.data.dir.normalize()).sum();
    (ship.attitude.q * body).normalize()
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

#[test]
fn the_maneuver_hold_turns_the_craft_before_ignition() {
    let w = world();
    let mut ship = leo_ship(&w);
    let n = node(&w, 400.0, DVec3::new(50.0, 0.0, 0.0));
    ship.set_nodes(vec![n]).unwrap();
    let controls = sas(HoldMode::Stability);
    let (wake, ignite) = ship.program().map(|p| (p.wake, p.ignite)).unwrap();
    // Before the wake: the pilot's stability hold keeps the attitude.
    let held = ship.attitude;
    ship.advance(&w, wake.add_seconds(-1.0), &controls, usize::MAX);
    assert_eq!(ship.attitude.q, held.q);
    // At ignition the engines point along the burn (the program's
    // alignment, then held through the settling).
    ship.advance(&w, ignite, &controls, usize::MAX);
    let p = ship.program().unwrap();
    let dir = p.dir.unwrap();
    let off = thrust_axis(&ship).dot(dir).clamp(-1.0, 1.0).acos();
    println!(
        "turn from {:.0} s before ignition; at ignition the engines are {off:.2e} rad off",
        ignite.seconds_since(wake)
    );
    assert!(off < maneuver::ALIGN, "{off} rad");
    let (prograde, _, _) = prograde_normal_radial(earth_state(&w, &ship).0, earth_state(&w, &ship).1);
    assert!(prograde.dot(dir) > 0.99);
    // During the burn the thrust stays on v_go.
    ship.advance(&w, n.t, &controls, usize::MAX);
    let p = ship.program().unwrap();
    assert_eq!(p.step, Step::Burn);
    let off = thrust_axis(&ship).dot(p.dir.unwrap()).clamp(-1.0, 1.0).acos();
    assert!(off < maneuver::ALIGN, "{off} rad in the burn");
    // After it the pilot's hold again.
    ship.advance(&w, n.t.add_seconds(30.0), &controls, usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Done);
    assert_eq!(ship.controls().hold, HoldMode::Stability);
}

#[test]
fn the_stored_coast_after_a_burn_is_what_is_flown() {
    // Rule 4 after a planned burn: the coast stored from the state the burn
    // ended in (held still: the coast knows its attitude, a perturbed coast,
    // `vessel::pointing`) is what the vessel flies.
    let w = world();
    let mut ship = leo_ship(&w);
    ship.advance(&w, t0(), &sas(HoldMode::Stability), usize::MAX);
    let n = node(&w, 300.0, DVec3::new(80.0, 10.0, -5.0));
    ship.set_nodes(vec![n]).unwrap();
    ship.advance(&w, n.t.add_seconds(60.0), &sas(HoldMode::Stability), usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Done);
    assert!(matches!(ship.phase, Phase::Coasting { .. }));
    let after = t0().add_seconds(900.0);
    ship.extend_coast(&w, after, usize::MAX);
    let predicted = ship.trajectory().unwrap().eval(after).unwrap();
    ship.advance(&w, after, &sas(HoldMode::Stability), usize::MAX);
    let flown = ship.state(&w);
    let (dr, dv) = ((predicted.1 - flown.1).length(), (predicted.2 - flown.2).length());
    println!("stored vs flown 540 s after the burn: {dr:.2e} m, {dv:.2e} m/s");
    assert_eq!(predicted.0, flown.0);
    assert!(dr < 0.05 && dv < 2e-4, "{dr} m, {dv} m/s");
}

#[test]
fn burns_and_holds_are_the_same_at_60_fps_and_in_one_jump() {
    let w = world();
    for hold in [HoldMode::Stability, HoldMode::Prograde, HoldMode::Retrograde, HoldMode::Maneuver] {
        let mut frames = leo_ship(&w);
        frames.set_nodes(vec![node(&w, 300.0, DVec3::new(60.0, 0.0, 0.0)), node(&w, 900.0, DVec3::X * -20.0)]).unwrap();
        let mut jump = frames.clone();
        let controls = Controls { reference: Some(earth(&w)), ..sas(hold) };
        let end = t0().add_seconds(1_100.0);
        fly_frames(&w, &mut frames, end, &controls);
        jump.advance(&w, end, &controls, usize::MAX);
        assert_eq!(frames.time, jump.time);
        assert_eq!(frames.state(&w), jump.state(&w), "{hold:?}");
        assert_eq!(frames.attitude, jump.attitude, "{hold:?}");
        assert_eq!(frames.mass(), jump.mass());
        assert_eq!(frames.program(), jump.program(), "{hold:?}");
        assert_eq!(frames.program().map(|p| (p.node, p.step)), Some((1, Step::Done)), "{hold:?}");
        jump.extend_coast(&w, frames.trajectory().unwrap().computed_until(), usize::MAX);
        assert_eq!(frames.trajectory(), jump.trajectory(), "{hold:?}");
    }
}

#[test]
fn a_coast_depends_on_the_attitude_only_through_drag_and_light() {
    // Torque-free rotation, a prograde hold and rotation input present
    // different areas to the thin air and to sunlight at 400 km
    // (`vessel::limbo`): the centre of mass moves by metres in 50 minutes,
    // not more.
    let w = world();
    let end = t0().add_seconds(3_000.0);
    let mut states = Vec::new();
    for controls in [
        Controls::default(),
        sas(HoldMode::Prograde),
        Controls { rotate: DVec3::new(0.5, 0.0, 1.0), ..Controls::default() },
    ] {
        let mut ship = leo_ship(&w);
        ship.advance(&w, end, &controls, usize::MAX);
        states.push(ship.state(&w));
    }
    for s in &states[1..] {
        let d = (s.1 - states[0].1).length();
        println!("{d:.3} m");
        assert!(s.0 == states[0].0 && d > 1e-3 && d < 20.0, "{d} m");
    }
}

#[test]
fn holds_point_where_their_rule_says_on_rails() {
    // A day at rails warp in each mode (reference Earth; the target a
    // fixed point 10,000 km out): settled, then followed without control
    // ticks.
    let w = world();
    let e = earth(&w);
    let (_, r, _) = leo_ship(&w).state(&w);
    let target = TargetState { anchor: e, epoch: t0(), r: r.normalize() * 1e7, v: DVec3::ZERO };
    let cases = [
        (HoldMode::Prograde, SpeedReference::Orbit),
        (HoldMode::Retrograde, SpeedReference::Orbit),
        (HoldMode::Prograde, SpeedReference::Surface),
        (HoldMode::Target, SpeedReference::Orbit),
        (HoldMode::AntiTarget, SpeedReference::Orbit),
        (HoldMode::Prograde, SpeedReference::Target),
    ];
    for (hold, speed) in cases {
        let controls = Controls { reference: Some(e), speed, target: Some(target), ..sas(hold) };
        let mut ship = leo_ship(&w);
        let mut jump = ship.clone();
        let started = Instant::now();
        let mut t = t0();
        for _ in 0..6 {
            // Frames of 16,667 s: a million times faster than real time.
            t = t.add_seconds(16_667.0);
            ship.advance(&w, t, &controls, usize::MAX);
        }
        let per_frame = started.elapsed().as_secs_f64() / 6.0;
        jump.advance(&w, t, &controls, usize::MAX);
        assert_eq!(ship.attitude, jump.attitude, "{hold:?} {speed:?}");
        let (anchor, r, v) = ship.state(&w);
        let x = sim::vessel::HoldInputs {
            v_orbit: v,
            v_surface: v - w.find("Earth").unwrap().physical.as_ref().unwrap().rotation.omega(ship.time).raw().cross(r),
            target: Some((target.r + target.v * ship.time.seconds_since(t0()) - r, target.v - v)),
            maneuver: None,
        };
        assert_eq!(anchor, e);
        let dir = sim::vessel::hold_direction(hold, speed, &x).unwrap();
        let err = ship.attitude.nose().cross(dir).length();
        println!("{hold:?} {speed:?}: {err:.1e} rad after a day; {:.2} ms per 1,000,000x frame", per_frame * 1e3);
        assert!(err < 1e-9, "{hold:?} {speed:?}: {err}");
    }
}

#[test]
fn a_vessel_saved_mid_turn_continues_bit_identically() {
    let w = world();
    let mut ship = leo_ship(&w);
    let n = node(&w, 300.0, DVec3::new(0.0, 30.0, 0.0));
    ship.set_nodes(vec![n]).unwrap();
    let controls = Controls { reference: Some(earth(&w)), ..sas(HoldMode::Prograde) };
    let wake = ship.program().unwrap().wake;
    ship.advance(&w, wake.add_seconds(5.0), &controls, usize::MAX);
    assert_eq!(ship.program().unwrap().step, Step::Turn);
    let mut ids = VesselIds::default();
    ids.allocate();
    *ship.controls_mut() = controls;
    let text = SaveGame::capture(&w, ship.time, std::slice::from_ref(&ship), ids, 0).to_ron();
    let save = SaveGame::from_ron(&text).unwrap();
    assert_eq!(save.vessels[0].controls(), &controls);
    let mut loaded = save.vessels[0].clone();
    assert_eq!(loaded, ship);
    let end = t0().add_seconds(900.0);
    ship.advance(&w, end, &controls, usize::MAX);
    loaded.advance(&w, end, &controls, usize::MAX);
    assert_eq!(loaded, ship);
    assert_eq!(ship.program().unwrap().step, Step::Done);
}

#[test]
fn a_long_burn_inside_a_rails_warp_frame_flies_its_live_ticks() {
    // A 1 km/s node (debug mode: the propellant kept) inside one frame of
    // 16,667 s: the vessel goes live at the wake, flies the burn tick by
    // tick (D129: burns fly live for now) and returns to its coast.
    let w = world();
    let mut ship = leo_ship(&w);
    ship.set_debug(&w, true);
    let n = node(&w, 600.0, DVec3::new(1_000.0, 0.0, 0.0));
    ship.set_nodes(vec![n]).unwrap();
    let duration = ship.node_prediction(&w, 0).unwrap().duration.unwrap();
    let started = Instant::now();
    ship.advance(&w, t0().add_seconds(16_667.0), &sas(HoldMode::Stability), usize::MAX);
    let cost = started.elapsed().as_secs_f64();
    println!("a {duration:.0} s burn and a 16,667 s frame: {:.1} ms", cost * 1e3);
    assert_eq!(ship.program().unwrap().step, Step::Done);
    assert!(matches!(ship.phase, Phase::Coasting { .. }));
}

#[test]
fn turning_attitudes_are_the_same_at_60_fps_and_in_one_jump() {
    // Mid-turn, before any snap makes them equal again: a tick's lattice
    // point can fall just past a frame's end, so the next frame's tick aims
    // from samples the first frame must not prune. The pilot's holds turn
    // from the start; the program's maneuver hold from its wake.
    let w = world();
    for hold in [HoldMode::Stability, HoldMode::Prograde, HoldMode::Retrograde, HoldMode::Maneuver] {
        let mut base = leo_ship(&w);
        base.set_nodes(vec![node(&w, 300.0, DVec3::new(60.0, 0.0, 0.0))]).unwrap();
        let wake = base.program().unwrap().wake.seconds_since(t0());
        let controls = Controls { reference: Some(earth(&w)), ..sas(hold) };
        for seconds in [1.0, 5.0, 20.0, wake + 1.0, wake + 5.0, wake + 20.0] {
            let end = t0().add_seconds(seconds);
            let (mut frames, mut jump) = (base.clone(), base.clone());
            fly_frames(&w, &mut frames, end, &controls);
            jump.advance(&w, end, &controls, usize::MAX);
            assert_eq!(frames.attitude, jump.attitude, "{hold:?} at {seconds} s");
        }
    }
}
