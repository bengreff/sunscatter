//! Saves: save → load → propagate gives bit-identical results to propagating
//! without the save, for coasting, landed and powered vessels.

use glam::DVec3;
use sim::ephem::Ephemeris;
use sim::kepler::Elements;
use sim::save::{SaveError, SaveGame};
use sim::sol;
use sim::time::Epoch;
use sim::vessel::{CoastStart, Controls, Phase, Segment, Vessel, VesselId, VesselIds, VesselParams};
use sim::world::World;
use std::sync::Arc;

fn world() -> World {
    let path = format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(3.0 * 86_400.0)
}

fn leo(w: &World) -> (sim::frame::NodeId, DVec3, DVec3) {
    let earth = w.find("Earth").unwrap();
    let (r, v) =
        Elements { a: 6_778_137.0, e: 0.0005, i: 0.9, raan: 1.0, argp: 0.5, mean_anomaly: 0.0 }.to_state(earth.gm);
    (earth.node, r, v)
}

/// A counter that has issued ids 1..=n.
fn ids_up_to(n: u64) -> VesselIds {
    let mut ids = VesselIds::default();
    for _ in 0..n {
        ids.allocate();
    }
    ids
}

/// Saves the fleet to text and loads it back.
fn through_save(w: &World, clock: Epoch, fleet: &[Vessel], controls: Controls) -> Vec<Vessel> {
    let text = SaveGame::capture(w, clock, fleet, ids_up_to(2), 0, controls).to_ron();
    let save = SaveGame::from_ron(&text).unwrap();
    save.check_world(w).unwrap();
    assert_eq!(save.clock, clock);
    assert_eq!(save.controls, controls);
    // Saving the loaded state gives the same text.
    assert_eq!(SaveGame::capture(w, clock, &save.vessels, save.vessel_ids, 0, controls).to_ron(), text);
    save.vessels
}

/// Advances in game-like frames of `dt` until `end`.
fn fly(w: &World, v: &mut Vessel, from: Epoch, end: Epoch, dt: f64, controls: &Controls) {
    let mut t = from;
    while t < end {
        t = t.add_seconds(dt);
        if t > end {
            t = end;
        }
        v.advance(w, t, controls, usize::MAX);
        v.extend_coast(w, t.add_seconds(3_000.0), 500); // look-ahead, as the game does
    }
}

#[test]
fn coasting_vessel_continues_bit_identically_after_load() {
    let w = world();
    let (earth, r, v) = leo(&w);
    let mut ship = Vessel::coasting(&w, VesselId(1), t0(), earth, r, v, VesselParams::block());
    let controls = Controls { sas: true, ..Default::default() };
    let mid = t0().add_seconds(1_234.567);
    fly(&w, &mut ship, t0(), mid, 0.25, &controls);
    assert!(ship.segment().unwrap().samples.len() > 2, "save holds future samples");
    let mut loaded = through_save(&w, mid, std::slice::from_ref(&ship), controls).remove(0);
    assert_eq!(loaded, ship);
    let end = t0().add_seconds(30_000.0);
    fly(&w, &mut ship, mid, end, 7.0, &controls);
    fly(&w, &mut loaded, mid, end, 7.0, &controls);
    assert_eq!(loaded, ship);
    assert_eq!(loaded.state(&w), ship.state(&w));
}

#[test]
fn chunked_extension_across_a_save_equals_single_pass() {
    let w = world();
    let (anchor, r, v) = leo(&w);
    let start = CoastStart { anchor, r, v, drag: None, contact_height: 0.0, horizon: 1e9, fixed_anchor: false };
    let mut one = Segment::new(&w, t0(), start);
    one.extend(&w, 2000);
    let mut part = Segment::new(&w, t0(), start);
    part.extend(&w, 700);
    let mut resumed: Segment = ron::from_str(&ron::to_string(&part).unwrap()).unwrap();
    resumed.extend(&w, 1300);
    assert_eq!(one, resumed);
}

#[test]
fn landed_vessel_lifts_off_identically_after_load() {
    let w = world();
    let pad = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6082, -80.6041, t0(), VesselParams::block());
    let idle = Controls { sas: true, ..Default::default() };
    let mut a = pad.clone();
    let wait = t0().add_seconds(100.0);
    fly(&w, &mut a, t0(), wait, 1.0, &idle);
    let mut b = through_save(&w, wait, std::slice::from_ref(&a), idle).remove(0);
    let burn = Controls { throttle: 1.0, sas: true, ..Default::default() };
    let end = wait.add_seconds(30.0);
    fly(&w, &mut a, wait, end, 1.0 / 60.0, &burn);
    fly(&w, &mut b, wait, end, 1.0 / 60.0, &burn);
    assert!(matches!(a.phase, Phase::Powered { .. }));
    assert_eq!(a, b);
}

#[test]
fn powered_flight_continues_identically_after_load() {
    let w = world();
    let mut a = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6082, -80.6041, t0(), VesselParams::block());
    let burn = Controls { throttle: 1.0, sas: true, rotate: DVec3::new(0.3, 0.0, 0.0), ..Default::default() };
    // A clock that is not a whole number of ticks: the vessel trails it.
    let mid = t0().add_seconds(12.345);
    fly(&w, &mut a, t0(), mid, 1.0 / 60.0, &burn);
    assert!(matches!(a.phase, Phase::Powered { .. }));
    assert!(a.time < mid);
    let mut b = through_save(&w, mid, std::slice::from_ref(&a), burn).remove(0);
    let end = mid.add_seconds(40.0);
    fly(&w, &mut a, mid, end, 1.0 / 60.0, &burn);
    fly(&w, &mut b, mid, end, 1.0 / 60.0, &burn);
    assert_eq!(a, b);
    // Then cut the engine and coast: the new segment matches too.
    let coast = Controls::default();
    let later = end.add_seconds(500.0);
    fly(&w, &mut a, end, later, 1.0, &coast);
    fly(&w, &mut b, end, later, 1.0, &coast);
    assert_eq!(a, b);
}

#[test]
fn save_files_round_trip_and_check_the_ephemeris() {
    let w = world();
    let (earth, r, v) = leo(&w);
    let mut ids = VesselIds::default();
    let ship = Vessel::coasting(&w, ids.allocate(), t0(), earth, r, v, VesselParams::block());
    let pad = Vessel::landed_at(&w, ids.allocate(), "Moon", 0.67, 23.47, t0(), VesselParams::block());
    let fleet = [ship, pad];
    let save = SaveGame::capture(&w, t0(), &fleet, ids, 1, Controls::default());
    let dir = std::env::temp_dir().join(format!("sunscatter-save-{}", std::process::id()));
    let path = dir.join("quicksave.ron");
    save.write(&path).unwrap();
    let back = SaveGame::read(&path, &w).unwrap();
    assert_eq!(back, save);
    assert_eq!(back.active, 1);
    assert_eq!(back.vessels[1].id(), VesselId(2));
    // The counter is saved: a vessel created after loading gets a new id.
    let mut loaded_ids = back.vessel_ids;
    assert_eq!(loaded_ids.allocate(), VesselId(3));

    let mut other = save.clone();
    other.ephemeris.hash ^= 1;
    assert!(matches!(other.check_world(&w), Err(SaveError::Ephemeris { .. })));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn saves_with_duplicate_or_unissued_vessel_ids_are_rejected() {
    let w = world();
    let pad = |id| Vessel::landed_at(&w, id, "Moon", 0.67, 23.47, t0(), VesselParams::block());
    let load = |fleet: &[Vessel], ids| {
        SaveGame::from_ron(&SaveGame::capture(&w, t0(), fleet, ids, 0, Controls::default()).to_ron())
    };
    assert!(load(&[pad(VesselId(1)), pad(VesselId(2))], ids_up_to(2)).is_ok());
    let dup = load(&[pad(VesselId(1)), pad(VesselId(1))], ids_up_to(2));
    assert!(matches!(dup, Err(SaveError::Format(_))), "duplicate id accepted");
    let unissued = load(&[pad(VesselId(3))], ids_up_to(2));
    assert!(matches!(unissued, Err(SaveError::Format(_))), "unissued id accepted");
}
