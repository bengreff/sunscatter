//! Each vessel flies on its own controls (D112): an inactive, uncrewed
//! vessel acts on the settings that have reached it (D034), holds its
//! attitude and burns its engines while the active vessel coasts, inside
//! the 10 km bubble (D082) and out of it; its motion does not depend on
//! how the frames chunk the time (rule 4).

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{bubble, fleet_controls, Controls, HoldMode, Vessel, VesselId};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

/// The probe's one-way delay from its operator (s).
const DELAY: f64 = 0.3;

/// Vessel 0 (active, crewed) coasts with SAS on; vessel 1, an uncrewed
/// probe `gap` m ahead along track, has burned at its least throttle since
/// 3 s before `t0` (its engines started by its sequence, the throttle
/// reached it) and is set at `t0` to hold prograde (its attitude is
/// radial-out: a quarter turn to make).
fn scene(w: &World, gap: f64) -> Vec<Vessel> {
    let earth = w.find("Earth").unwrap();
    let start = t0().add_seconds(-3.0);
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    let along = (v - r * (v.dot(r) / r.length_squared())).normalize();
    let craft = sim::craft::small::rocket();
    let active = Vessel::coasting(w, VesselId(1), start, earth.node, r, v, &craft);
    let mut probe = Vessel::coasting(w, VesselId(2), start, earth.node, r + along * gap, v, &craft);
    probe.set_crew(0);
    assert_eq!(probe.next_stage(w).unwrap().0, "Ignition");
    *probe.controls_mut() = Controls { throttle: 0.4, ..Controls::idle() };
    let mut fleet = vec![active, probe];
    fly(w, &mut fleet, start, t0(), 0.1, &mut |_| {});
    *fleet[1].controls_mut() = Controls { throttle: 0.4, hold: HoldMode::Prograde, ..Controls::idle() };
    fleet
}

/// Frames of `dt` from `from` to `to` as the game runs them: every
/// vessel's controls, then the bubble, then every vessel.
fn fly(w: &World, fleet: &mut [Vessel], from: Epoch, to: Epoch, dt: f64, each: &mut dyn FnMut(&[Vessel])) {
    let frames = (to.seconds_since(from) / dt).round() as usize;
    for k in 0..frames {
        let now = from.add_seconds(dt * k as f64);
        let target = from.add_seconds(dt * (k + 1) as f64);
        let acting = fleet_controls(fleet, now, |id| (id == VesselId(2)).then_some(DELAY));
        let controls = |i: usize| acting[i];
        bubble::advance(w, fleet, &controls, target);
        for (i, v) in fleet.iter_mut().enumerate() {
            v.advance(w, target, &controls(i), usize::MAX);
        }
        each(fleet);
    }
}

/// The angle between the probe's nose and its velocity about Earth (deg).
fn off_prograde(w: &World, v: &Vessel) -> f64 {
    let (anchor, _, vel) = v.state(w);
    assert_eq!(anchor, w.find("Earth").unwrap().node);
    (v.attitude.q * DVec3::Z).angle_between(vel).to_degrees()
}

fn check(gap: f64, bubbled: bool) {
    let w = world();
    let mut fleet = scene(&w, gap);
    let (fuel, coast) = (fleet[1].propellant(), fleet[0].propellant());
    let turn = off_prograde(&w, &fleet[1]);
    // Until the setting reaches it, the probe holds its attitude.
    let mut held = false;
    fly(&w, &mut fleet, t0(), t0().add_seconds(0.2), 0.1, &mut |f| held |= f[0].in_bubble());
    assert!((off_prograde(&w, &fleet[1]) - turn).abs() < 0.1, "the setting is still on its way");
    assert_eq!(fleet[1].controls_in_flight(), 1);
    let warm = t0().add_seconds(1.0);
    fly(&w, &mut fleet, t0().add_seconds(0.2), warm, 0.1, &mut |f| held |= f[0].in_bubble());
    assert_eq!(fleet[1].controls_in_flight(), 0);
    // Chunked as 0.1 s or 0.5 s frames: the same motion, bit for bit.
    let mut chunked = fleet.clone();
    let end = warm.add_seconds(60.0);
    fly(&w, &mut fleet, warm, end, 0.1, &mut |f| held |= f[0].in_bubble());
    fly(&w, &mut chunked, warm, end, 0.5, &mut |_| {});
    assert_eq!(fleet[1].state(&w), chunked[1].state(&w), "the probe");
    assert_eq!(fleet[0].state(&w), chunked[0].state(&w), "the active vessel");
    assert_eq!(fleet[1].attitude, chunked[1].attitude);
    // The probe burned and turned to prograde; the active vessel did not.
    assert!(fleet[1].propellant() < fuel, "the probe burns");
    // The active vessel only boils off (its LOX on the coast law).
    assert!((coast - fleet[0].propellant()).abs() < 1.0, "the active vessel coasts");
    let off = off_prograde(&w, &fleet[1]);
    println!("gap {gap} m: the probe {off:.2} deg off prograde; the active vessel held live: {held}");
    assert!(off < 2.0, "{off}");
    assert_eq!(held, bubbled, "held live by the probe's bubble");
}

#[test]
fn an_inactive_probe_holds_prograde_and_burns_in_the_bubble() {
    check(1_000.0, true);
}

#[test]
fn an_inactive_probe_holds_prograde_and_burns_out_of_the_bubble() {
    check(300_000.0, false);
}
