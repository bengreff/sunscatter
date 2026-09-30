//! Attitude hold modes and planned burns flown by the attitude (D075).

use glam::DVec3;
use sim::craft::test_craft;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{
    prograde_normal_radial, Attitude, BurnLimits, CoastStart, Controls, FlightPlan, HoldMode, PlannedBurn, SegmentKind,
    SpeedReference, TargetState, Trajectory, Vessel, VesselId, VesselIds,
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
    Vessel::coasting(w, VesselId(1), t0(), earth(w), r, v, test_craft())
}

fn burn(w: &World, at: f64, dv: DVec3) -> PlannedBurn {
    PlannedBurn::delta_v_with(t0().add_seconds(at), dv, &test_craft().params().engine, Some(earth(w)))
}

fn plan(burns: &[PlannedBurn]) -> FlightPlan {
    FlightPlan { burns: burns.to_vec() }
}

fn sas(hold: HoldMode) -> Controls {
    Controls { sas: true, hold, reference: None, ..Controls::default() }
}

/// State relative to Earth at `t`.
fn earth_state(w: &World, ship: &Vessel, t: Epoch) -> (DVec3, DVec3) {
    let (anchor, r, v) = ship.state_at(w, t);
    let k = w.eph.relative(anchor, earth(w), t);
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

#[test]
fn the_maneuver_hold_turns_the_craft_before_ignition() {
    let w = world();
    let mut ship = leo_ship(&w);
    let b = burn(&w, 400.0, DVec3::new(50.0, 0.0, 0.0));
    ship.set_plan(&w, plan(&[b])).unwrap();
    let controls = sas(HoldMode::Stability);
    let lead = ship.trajectory().unwrap().craft().unwrap().turn_lead(ship.mass());
    // Before the turn: the stability hold keeps the attitude.
    let held = ship.attitude;
    ship.advance(&w, b.t_start.add_seconds(-lead - 1.0), &controls, usize::MAX);
    assert_eq!(ship.attitude, held);
    let (r, v) = earth_state(&w, &ship, ship.time);
    let (prograde, _, _) = prograde_normal_radial(r, v);
    // At ignition the engine points along the burn.
    ship.advance(&w, b.t_start, &controls, usize::MAX);
    let (r, v) = earth_state(&w, &ship, ship.time);
    let dir = prograde_normal_radial(r, v).0;
    let axis = ship.attitude.q * ship.craft.engine.mount_dir;
    let off = axis.cross(dir).length();
    println!("lead {lead} s; at ignition the engine is {off:.2e} rad off");
    assert!(off < 1e-5);
    assert!(prograde.dot(dir) > 0.99);
    // During the burn the attitude is the stored flight's, still on it.
    ship.advance(&w, b.t_start.add_seconds(1.0), &controls, usize::MAX);
    assert!(ship.burn().is_some());
    let (r, v) = earth_state(&w, &ship, ship.time);
    let axis = ship.attitude.q * ship.craft.engine.mount_dir;
    assert!(axis.cross(prograde_normal_radial(r, v).0).length() < 1e-4);
}

#[test]
fn a_burn_thrusts_along_the_attitude_not_the_law() {
    // A burn at the very start of a trajectory, the craft pointing across
    // it: the thrust follows the attitude while it turns, so it ends
    // elsewhere than a burn along the law.
    let w = world();
    let ship = leo_ship(&w);
    let (anchor, r, v) = ship.state(&w);
    let b = PlannedBurn::delta_v_with(t0(), DVec3::new(100.0, 0.0, 0.0), &ship.craft.engine, Some(earth(&w)));
    let p = plan(&[b]);
    let start = CoastStart {
        anchor,
        r,
        v,
        drag: None,
        contact_height: 0.0,
        horizon: 1e9,
        fixed_anchor: false,
        proper_time: 0.0,
    };
    let across = Attitude { q: sim::vessel::quat_z_to(r.normalize()), omega: DVec3::ZERO };
    let craft = sim::vessel::BurnCraft::of(&ship.craft);
    let limits = BurnLimits::default();
    let seed = (across, Default::default());
    let mut flown = Trajectory::flown(&w, t0(), start, ship.mass(), &p, limits, craft, seed);
    let mut ideal = Trajectory::new(&w, t0(), start, ship.mass(), &p, limits);
    let end = t0().add_seconds(120.0);
    flown.extend(&w, &p, end, usize::MAX);
    ideal.extend(&w, &p, end, usize::MAX);
    let ((_, r1, v1), (_, r2, v2)) = (flown.eval(end).unwrap(), ideal.eval(end).unwrap());
    // The same thrust magnitude integral (rocket equation) either way.
    assert_eq!(flown.mass_at(end), ideal.mass_at(end));
    let dv = (v1 - v2).length();
    println!("turning while thrusting: {dv:.2} m/s and {:.0} m from a burn along the law", (r1 - r2).length());
    assert!(dv > 1.0, "{dv}");
    // The attitude is stored with the burn.
    let seg = flown.segments.iter().find(|s| matches!(s.kind, SegmentKind::Burn(_))).unwrap();
    assert_eq!(seg.attitude_at(0.0), Some(across));
    assert_ne!(seg.attitude_at(1.0), Some(across));
}

#[test]
fn a_predicted_burn_is_what_is_flown() {
    // The burn drawn before ignition (predicted from the craft settled on
    // it) and the burn flown (from the attitude the turn ended in).
    let w = world();
    let mut ship = leo_ship(&w);
    let b = burn(&w, 300.0, DVec3::new(80.0, 10.0, -5.0));
    ship.set_plan(&w, plan(&[b])).unwrap();
    let after = t0().add_seconds(600.0);
    ship.extend_coast(&w, after, usize::MAX);
    let predicted = ship.trajectory().unwrap().eval(after).unwrap();
    ship.advance(&w, after, &sas(HoldMode::Stability), usize::MAX);
    let flown = ship.state(&w);
    let (dr, dv) = ((predicted.1 - flown.1).length(), (predicted.2 - flown.2).length());
    println!("predicted vs flown 300 s after the burn: {dr:.2e} m, {dv:.2e} m/s");
    assert_eq!(predicted.0, flown.0);
    assert!(dr < 0.01 && dv < 1e-5, "{dr} m, {dv} m/s");
}

#[test]
fn burns_and_holds_are_the_same_at_60_fps_and_in_one_jump() {
    let w = world();
    for hold in [HoldMode::Stability, HoldMode::Prograde, HoldMode::Retrograde, HoldMode::Maneuver] {
        let mut frames = leo_ship(&w);
        frames
            .set_plan(&w, plan(&[burn(&w, 300.0, DVec3::new(60.0, 0.0, 0.0)), burn(&w, 500.0, DVec3::X * -20.0)]))
            .unwrap();
        let mut jump = frames.clone();
        let controls = Controls { reference: Some(earth(&w)), ..sas(hold) };
        let end = t0().add_seconds(700.0);
        fly_frames(&w, &mut frames, end, &controls);
        jump.advance(&w, end, &controls, usize::MAX);
        assert_eq!(frames.time, jump.time);
        assert_eq!(frames.state(&w), jump.state(&w), "{hold:?}");
        assert_eq!(frames.attitude, jump.attitude, "{hold:?}");
        assert_eq!(frames.mass(), jump.mass());
        jump.extend_coast(&w, frames.trajectory().unwrap().computed_until(), usize::MAX);
        assert_eq!(frames.trajectory(), jump.trajectory(), "{hold:?}");
    }
}

#[test]
fn a_coast_does_not_depend_on_the_attitude() {
    // Torque-free rotation, holds and input never move the centre of mass.
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
    assert_eq!(states[0], states[1]);
    assert_eq!(states[0], states[2]);
}

#[test]
fn holds_point_where_their_rule_says_on_rails() {
    // A day at rails warp in each mode (reference Earth; the target a
    // point 1000 km ahead): settled, then followed without control ticks.
    let w = world();
    let e = earth(&w);
    let ship0 = leo_ship(&w);
    let (_, r, v) = ship0.state(&w);
    // A fixed point 10,000 km out (a body-like target, exact at every epoch).
    let _ = v;
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
        let snap = w.snapshot(ship.time);
        let (anchor, r, v) = ship.state(&w);
        let x = sim::vessel::HoldInputs {
            v_orbit: v,
            v_surface: v - w.find("Earth").unwrap().physical.as_ref().unwrap().rotation.omega(ship.time).raw().cross(r),
            target: Some((target.r + target.v * ship.time.seconds_since(t0()) - r, target.v - v)),
            maneuver: None,
        };
        assert_eq!(anchor, e);
        let _ = snap;
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
    let b = burn(&w, 300.0, DVec3::new(0.0, 30.0, 0.0));
    ship.set_plan(&w, plan(&[b])).unwrap();
    let controls = Controls { reference: Some(earth(&w)), ..sas(HoldMode::Prograde) };
    ship.advance(&w, b.t_start.add_seconds(-5.0), &controls, usize::MAX);
    let mut ids = VesselIds::default();
    ids.allocate();
    let text = SaveGame::capture(&w, ship.time, std::slice::from_ref(&ship), ids, 0, controls).to_ron();
    let save = SaveGame::from_ron(&text).unwrap();
    assert_eq!(save.controls, controls);
    let mut loaded = save.vessels[0].clone();
    assert_eq!(loaded, ship);
    let end = t0().add_seconds(900.0);
    ship.advance(&w, end, &controls, usize::MAX);
    loaded.advance(&w, end, &controls, usize::MAX);
    assert_eq!(loaded, ship);
}

#[test]
fn a_long_burn_on_rails_is_cheap() {
    // Per-burn cost at any warp: the burn is integrated once, tick by tick.
    let w = world();
    let mut ship = leo_ship(&w);
    ship.set_debug(&w, true);
    let b = burn(&w, 100.0, DVec3::new(1_000.0, 0.0, 0.0));
    ship.set_plan(&w, plan(&[b])).unwrap();
    let duration =
        b.end.duration(&sim::vessel::BurnLimits { dry_mass: 0.0, infinite: true }.effective(b.law), ship.mass());
    let started = Instant::now();
    ship.advance(&w, t0().add_seconds(16_667.0), &sas(HoldMode::Stability), usize::MAX);
    let cost = started.elapsed().as_secs_f64();
    println!("a {:.0} s burn and a 16,667 s frame: {:.1} ms", duration.unwrap(), cost * 1e3);
    assert!(ship.burn().is_none());
}

#[test]
fn turning_attitudes_are_the_same_at_60_fps_and_in_one_jump() {
    // Mid-turn, before any snap makes them equal again: a tick's lattice
    // point can fall just past a frame's end, so the next frame's tick aims
    // from samples the first frame must not prune.
    let w = world();
    for hold in [HoldMode::Stability, HoldMode::Prograde, HoldMode::Retrograde, HoldMode::Maneuver] {
        let mut base = leo_ship(&w);
        base.set_plan(&w, plan(&[burn(&w, 300.0, DVec3::new(60.0, 0.0, 0.0))])).unwrap();
        let controls = Controls { reference: Some(earth(&w)), ..sas(hold) };
        for seconds in [1.0, 5.0, 20.0] {
            let end = t0().add_seconds(seconds);
            let (mut frames, mut jump) = (base.clone(), base.clone());
            fly_frames(&w, &mut frames, end, &controls);
            jump.advance(&w, end, &controls, usize::MAX);
            assert_eq!(frames.attitude, jump.attitude, "{hold:?} at {seconds} s");
        }
    }
}
