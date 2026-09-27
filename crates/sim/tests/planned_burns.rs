//! Planned burns (review sim 11, realism-1 §6a): a trajectory of coast and
//! burn segments with mass in the state, flown identically at every warp.

use glam::DVec3;
use sim::craft::test_craft;
use sim::ephem::Ephemeris;
use sim::frame::NodeId;
use sim::kepler::Elements;
use sim::save::SaveGame;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{
    BurnEnd, BurnLimits, CoastStart, Controls, EndKind, FlightPlan, PlanError, PlannedBurn, SegmentKind, Trajectory,
    Vessel, VesselId, VesselIds,
};
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

/// A 400 km orbit about Earth (above the atmosphere's effect on a 3 s burn).
fn leo_ship(w: &World) -> Vessel {
    let (earth, mu) = earth(w);
    let (r, v) = Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(mu);
    Vessel::coasting(w, VesselId(1), t0(), earth, r, v, sim::craft::test_craft())
}

/// A prograde burn (relative to Earth) of `dv` m/s at `t0 + at` seconds.
/// A prograde burn by the test craft's engine.
fn prograde(w: &World, at: f64, dv: f64) -> PlannedBurn {
    let engine = test_craft().params().engine;
    PlannedBurn::delta_v_with(t0().add_seconds(at), DVec3::new(dv, 0.0, 0.0), &engine, Some(earth(w).0))
}

fn plan(burns: &[PlannedBurn]) -> FlightPlan {
    FlightPlan { burns: burns.to_vec() }
}

/// State relative to Earth at `t`.
fn earth_state(w: &World, traj: &Trajectory, t: Epoch) -> (DVec3, DVec3) {
    let (anchor, r, v) = traj.eval(t).expect("stored");
    let k = w.eph.relative(anchor, earth(w).0, t);
    (r + k.r, v + k.v)
}

#[test]
fn a_burn_flown_in_frames_equals_one_jump_bit_for_bit() {
    let w = world();
    let mut frames = leo_ship(&w);
    frames.set_plan(&w, plan(&[prograde(&w, 300.0, 100.0), prograde(&w, 500.0, 50.0)])).unwrap();
    let mut jump = frames.clone();
    let controls = Controls { sas: true, ..Controls::default() };
    let end = t0().add_seconds(700.0);
    let mut t = t0();
    let mut k = 0u64;
    while t < end {
        k += 1;
        t = t0().add_seconds(k as f64 / 60.0).min_epoch(end);
        frames.advance(&w, t, &controls, usize::MAX);
        if k.is_multiple_of(7) {
            frames.extend_coast(&w, t.add_seconds(900.0), 37); // look-ahead in odd chunks
        }
    }
    jump.advance(&w, end, &controls, usize::MAX);
    assert_eq!(frames.time, jump.time);
    assert_eq!(frames.state(&w), jump.state(&w));
    assert_eq!(frames.mass(), jump.mass());
    // The look-ahead ran further; compare the stored part both have.
    jump.extend_coast(&w, frames.trajectory().unwrap().computed_until(), usize::MAX);
    assert_eq!(frames.trajectory(), jump.trajectory());
    assert!(frames.mass() < 20_000.0, "the burns used propellant");
}

trait MinEpoch {
    fn min_epoch(self, other: Epoch) -> Epoch;
}
impl MinEpoch for Epoch {
    fn min_epoch(self, other: Epoch) -> Epoch {
        if self.seconds_since(other) > 0.0 {
            other
        } else {
            self
        }
    }
}

#[test]
fn chunked_chain_equals_single_pass() {
    let w = world();
    let ship = leo_ship(&w);
    let p = plan(&[prograde(&w, 200.0, 80.0), prograde(&w, 400.0, 40.0)]);
    let (anchor, r, v) = ship.state(&w);
    let start = CoastStart {
        anchor,
        r,
        v,
        drag: Some(ship.drag()),
        contact_height: 5.0,
        horizon: 1e9,
        fixed_anchor: false,
        proper_time: 0.0,
    };
    let until = t0().add_seconds(3_000.0);
    let mut one = Trajectory::new(&w, t0(), start, ship.mass(), &p, BurnLimits::default());
    one.extend(&w, &p, until, usize::MAX);
    let mut chunked = Trajectory::new(&w, t0(), start, ship.mass(), &p, BurnLimits::default());
    for (i, n) in [1usize, 7, 3, 50, 1, 2, 11].iter().cycle().enumerate() {
        if chunked.computed_until().seconds_since(until) >= 0.0 {
            break;
        }
        // Alternate step budgets and intermediate targets.
        let target = t0().add_seconds(40.0 * i as f64);
        chunked.extend(&w, &p, if i % 2 == 0 { until } else { target }, *n);
    }
    assert_eq!(chunked, one);
    let kinds: Vec<_> = one.segments.iter().map(|s| (s.kind, s.end.map(|e| e.kind))).collect();
    assert!(matches!(kinds[0], (SegmentKind::Coast, Some(EndKind::BurnStart))));
    assert!(matches!(kinds[1], (SegmentKind::Burn(_), Some(EndKind::BurnEnd))));
    assert!(matches!(kinds[2], (SegmentKind::Coast, Some(EndKind::BurnStart))));
    assert!(matches!(kinds[3], (SegmentKind::Burn(_), Some(EndKind::BurnEnd))));
    assert!(matches!(kinds[4], (SegmentKind::Coast, None)));
    // Segments land exactly on the planned ignitions.
    assert_eq!(one.segments[1].t0, p.burns[0].t_start);
    assert_eq!(one.segments[3].t0, p.burns[1].t_start);
}

#[test]
fn a_prograde_burn_raises_apoapsis_as_the_rocket_equation_predicts() {
    let w = world();
    let (_, mu) = earth(&w);
    let mut ship = leo_ship(&w);
    let dv = 100.0;
    let burn = prograde(&w, 600.0, dv);
    ship.set_plan(&w, plan(&[burn])).unwrap();
    let m0 = ship.mass();
    let duration = burn.end.duration(&burn.law, m0).unwrap();
    let t_end = burn.t_start.add_seconds(duration);
    ship.extend_coast(&w, t_end.add_seconds(10.0), usize::MAX);
    let traj = ship.trajectory().unwrap();

    // Impulsive prediction: add Δv along the velocity at ignition.
    let (r0, v0) = earth_state(&w, traj, burn.t_start);
    let predicted = Elements::from_state(r0, v0 + v0.normalize() * dv, mu).apoapsis();
    let before = Elements::from_state(r0, v0, mu).apoapsis();
    let (r1, v1) = earth_state(&w, traj, t_end);
    let flown = Elements::from_state(r1, v1, mu).apoapsis();
    println!("apoapsis {before:.0} → {flown:.1} m (impulsive {predicted:.1} m), burn {duration:.3} s");
    assert!(flown - before > 300_000.0, "raised by {} m", flown - before);
    // Finite-burn loss over a 3 s arc is tiny (≪ 0.1%).
    assert!((flown - predicted).abs() < 200.0, "flown {flown}, predicted {predicted}");

    // Mass after the burn = m0 − ṁ·t, and stays constant in the next coast.
    let burn_seg = traj.segments.iter().find(|s| matches!(s.kind, SegmentKind::Burn(_))).unwrap();
    assert_eq!(burn_seg.mass_at(duration), m0 - burn.law.mass_flow * duration);
    ship.advance(&w, t_end.add_seconds(5.0), &Controls::default(), usize::MAX);
    assert_eq!(ship.mass(), m0 - burn.law.mass_flow * duration);
    let m1 = ship.mass();
    let delivered = burn.law.thrust / burn.law.mass_flow * sim::math::ln(m0 / m1);
    assert!((delivered - dv).abs() < 1e-9, "rocket equation Δv {delivered}");
}

#[test]
fn editing_the_second_burn_keeps_the_first_burns_samples() {
    let w = world();
    let b1 = prograde(&w, 300.0, 60.0);
    let b2 = prograde(&w, 2_000.0, 30.0);
    let b2_new = PlannedBurn::delta_v_with(
        t0().add_seconds(2_500.0),
        DVec3::new(0.0, 20.0, -5.0),
        &test_craft().params().engine,
        Some(earth(&w).0),
    );

    let mut ship = leo_ship(&w);
    ship.set_plan(&w, plan(&[b1, b2])).unwrap();
    ship.advance(&w, t0().add_seconds(100.0), &Controls::default(), usize::MAX);
    ship.extend_coast(&w, t0().add_seconds(4_000.0), usize::MAX);
    let before = ship.trajectory().unwrap().clone();

    ship.set_plan(&w, plan(&[b1, b2_new])).unwrap();
    let after = ship.trajectory().unwrap();
    // The coast to the first burn and the burn itself are untouched.
    assert_eq!(after.segments.len(), 2);
    assert_eq!(after.segments[..2], before.segments[..2]);
    assert!(matches!(after.segments[1].kind, SegmentKind::Burn(_)));

    // The edited plan equals a vessel planned that way from the start.
    let mut fresh = leo_ship(&w);
    fresh.set_plan(&w, plan(&[b1, b2_new])).unwrap();
    fresh.advance(&w, t0().add_seconds(100.0), &Controls::default(), usize::MAX);
    let until = t0().add_seconds(4_000.0);
    ship.extend_coast(&w, until, usize::MAX);
    fresh.extend_coast(&w, until, usize::MAX);
    assert_eq!(ship.trajectory(), fresh.trajectory());
    assert_eq!(ship.trajectory().unwrap().segments[3].t0, b2_new.t_start);

    // Editing the next burn while coasting towards it rebuilds the current
    // coast from its start: again the same as planning it from the start.
    let b1_new = prograde(&w, 350.0, 70.0);
    ship.set_plan(&w, plan(&[b1_new, b2_new])).unwrap();
    let mut fresh = leo_ship(&w);
    fresh.set_plan(&w, plan(&[b1_new, b2_new])).unwrap();
    fresh.advance(&w, t0().add_seconds(100.0), &Controls::default(), usize::MAX);
    ship.extend_coast(&w, until, usize::MAX);
    fresh.extend_coast(&w, until, usize::MAX);
    assert_eq!(ship.trajectory(), fresh.trajectory());
    assert_eq!(ship.state(&w), fresh.state(&w));
}

#[test]
fn plan_edits_that_rewrite_the_past_are_rejected() {
    let w = world();
    let mut ship = leo_ship(&w);
    let b1 = prograde(&w, 300.0, 100.0);
    assert_eq!(ship.set_plan(&w, plan(&[prograde(&w, 400.0, 1.0), b1])), Err(PlanError::NotSorted));
    ship.set_plan(&w, plan(&[b1])).unwrap();
    // Inside the burn (it lasts ~3 s).
    ship.advance(&w, t0().add_seconds(301.0), &Controls::default(), usize::MAX);
    assert!(ship.burn().is_some());
    assert_eq!(ship.set_plan(&w, plan(&[prograde(&w, 300.0, 50.0)])), Err(PlanError::Started));
    assert_eq!(ship.set_plan(&w, plan(&[b1, prograde(&w, 200.0, 5.0)])), Err(PlanError::NotSorted));
    assert_eq!(ship.set_plan(&w, plan(&[b1, prograde(&w, 300.5, 5.0)])), Err(PlanError::InThePast));
    ship.advance(&w, t0().add_seconds(310.0), &Controls::default(), usize::MAX);
    assert!(ship.burn().is_none());
    // A later burn can still be added.
    ship.set_plan(&w, plan(&[b1, prograde(&w, 900.0, 5.0)])).unwrap();
}

#[test]
fn a_burn_saved_mid_way_continues_bit_identically() {
    let w = world();
    let mut ship = leo_ship(&w);
    ship.set_plan(&w, plan(&[prograde(&w, 300.0, 100.0)])).unwrap();
    ship.advance(&w, t0().add_seconds(301.5), &Controls::default(), usize::MAX);
    assert!(ship.burn().is_some());
    let mut ids = VesselIds::default();
    ids.allocate();
    let text = SaveGame::capture(&w, ship.time, std::slice::from_ref(&ship), ids, 0, Controls::default()).to_ron();
    let mut loaded = SaveGame::from_ron(&text).unwrap().vessels.remove(0);
    assert_eq!(loaded, ship);
    let end = t0().add_seconds(1_000.0);
    ship.advance(&w, end, &Controls::default(), usize::MAX);
    loaded.advance(&w, end, &Controls::default(), usize::MAX);
    assert_eq!(loaded, ship);
}

/// Rule 1: the anchor the burn is integrated in does not change it (the
/// tracking reference, Earth, is reached through the frame tree).
#[test]
fn a_burn_does_not_depend_on_the_anchor() {
    let w = world();
    let (earth, _) = earth(&w);
    let moon = w.find("Moon").unwrap().node;
    let ship = leo_ship(&w);
    let (_, r, v) = ship.state(&w);
    let p = plan(&[prograde(&w, 100.0, 100.0)]);
    let run = |anchor: NodeId| {
        let k = w.eph.relative(earth, anchor, t0());
        let start = CoastStart {
            anchor,
            r: r + k.r,
            v: v + k.v,
            drag: None,
            contact_height: 0.0,
            horizon: 1e9,
            fixed_anchor: true,
            proper_time: 0.0,
        };
        let mut traj = Trajectory::new(&w, t0(), start, ship.mass(), &p, BurnLimits::default());
        let t = t0().add_seconds(400.0);
        traj.extend(&w, &p, t, usize::MAX);
        earth_state(&w, &traj, t)
    };
    let (a, b) = (run(earth), run(moon));
    let (dr, dv) = ((a.0 - b.0).length(), (a.1 - b.1).length());
    println!("Earth- vs Moon-anchored burn: {dr:.4} m, {dv:.2e} m/s");
    assert!(dr < 0.5 && dv < 1e-3, "{dr} m, {dv} m/s");
}

#[test]
fn a_burn_that_would_use_all_the_mass_fails_instead_of_dividing_by_zero() {
    // Without a dry mass (a bare trajectory), a burn longer than the mass
    // lasts cannot be flown.
    let w = world();
    let ship = leo_ship(&w);
    let mut burn = prograde(&w, 100.0, 10.0);
    burn.end = BurnEnd::Duration(1e6);
    let (anchor, r, v) = ship.state(&w);
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
    let p = plan(&[burn]);
    let mut traj = Trajectory::new(&w, t0(), start, ship.mass(), &p, BurnLimits::default());
    traj.extend(&w, &p, t0().add_seconds(500.0), usize::MAX);
    assert!(traj.finished());
    assert_eq!(traj.last().end.map(|e| e.kind), Some(EndKind::Failed));
    assert_eq!(traj.computed_until(), burn.t_start, "the trajectory stops at the failed ignition");
}

#[test]
fn a_vessel_burn_stops_at_burnout() {
    let w = world();
    let mut ship = leo_ship(&w);
    let mut burn = prograde(&w, 100.0, 10.0);
    burn.end = BurnEnd::Duration(1e6);
    ship.set_plan(&w, plan(&[burn])).unwrap();
    let burnout = 16_000.0 / burn.law.mass_flow;
    let after = t0().add_seconds(100.0 + burnout + 60.0);
    assert_eq!(ship.advance(&w, after, &Controls::default(), usize::MAX), after);
    assert_eq!(ship.propellant(), 0.0);
    assert_eq!(ship.mass(), 4_000.0);
    let traj = ship.trajectory().unwrap();
    assert!(matches!(traj.current().kind, SegmentKind::Coast), "coasting after burnout");
}

#[test]
fn debug_mode_burns_take_no_propellant() {
    let w = world();
    let mut ship = leo_ship(&w);
    ship.set_debug(&w, true);
    let burn = prograde(&w, 100.0, 200.0);
    ship.set_plan(&w, plan(&[burn])).unwrap();
    // 200 m/s at constant mass: t = Δv·m / F.
    let duration = 200.0 * 20_000.0 / burn.law.thrust;
    ship.extend_coast(&w, t0().add_seconds(100.0 + duration + 10.0), usize::MAX);
    let seg = ship.trajectory().unwrap().segments.iter().find(|s| matches!(s.kind, SegmentKind::Burn(_))).unwrap();
    assert!((seg.horizon() - duration).abs() < 1e-9, "{} vs {duration}", seg.horizon());
    assert_eq!(seg.mass_at(seg.horizon()), 20_000.0);
    ship.advance(&w, t0().add_seconds(100.0 + duration + 10.0), &Controls::default(), usize::MAX);
    assert_eq!(ship.propellant(), 16_000.0);
    // Turning debug mode off keeps the propellant and restarts the coast.
    ship.set_debug(&w, false);
    assert!(!ship.debug() && ship.propellant() == 16_000.0);
}
