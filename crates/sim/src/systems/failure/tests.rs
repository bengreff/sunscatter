//! Failures on the Falcon 9's definition (ship-model.md §6, §11): fates
//! the same for the same launch, each outcome, time at temperature, a
//! bottle's burst cascading into its tank, debug mode.

use super::*;
use crate::craft::state::EngineStatus;
use crate::craft::{default_craft_dir, load_assembly};
use crate::systems::structure::Bodies;
use std::sync::Arc;

fn falcon9() -> Arc<ShipDef> {
    load_assembly(&default_craft_dir().join("falcon9-crew")).unwrap().def.ship.unwrap()
}

fn t0() -> Epoch {
    Epoch::new(845_000_000, 0.5)
}

fn env(dt: f64, debug: bool) -> TickEnv<'static> {
    use glam::DVec3;
    TickEnv {
        at: t0(),
        dt,
        p: 101_325.0,
        throttle: 1.0,
        accel: DVec3::Z * 9.8,
        omega: DVec3::ZERO,
        alpha: DVec3::ZERO,
        com: DVec3::ZERO,
        sun: DVec3::ZERO,
        air_k: None,
        debug,
        bodies: None,
    }
}

fn engine(def: &ShipDef, id: &str) -> usize {
    def.engines.iter().position(|e| e.id == id).unwrap()
}

fn fate(state: &ShipState, owner: &str, mode: &str) -> usize {
    state.fates.iter().position(|f| f.owner == owner && f.mode == mode).unwrap()
}

fn launched(def: &ShipDef) -> ShipState {
    let mut s = ShipState::initial(def);
    s.launch = Some(t0());
    draw_fates(def, &mut s, 0, t0());
    s
}

/// One fate per catalog mode (nine Merlins' two, the MVac's restart, four
/// grid fins' pumps); the same launch draws the same bits, another launch
/// or seed other thresholds.
#[test]
fn fates_are_drawn_once_from_the_launch() {
    let def = falcon9();
    let a = launched(&def);
    assert_eq!(a.fates.len(), 9 * 2 + 1 + 4);
    assert_eq!(a, launched(&def));
    let mut later = ShipState::initial(&def);
    draw_fates(&def, &mut later, 0, t0().add_seconds(1.0));
    let mut seeded = ShipState::initial(&def);
    draw_fates(&def, &mut seeded, 1, t0());
    for (x, y) in a.fates.iter().zip(&later.fates).chain(a.fates.iter().zip(&seeded.fates)) {
        assert_eq!((&x.owner, &x.mode), (&y.owner, &y.mode));
        assert_ne!(x.threshold, y.threshold);
    }
    // Drawing again changes nothing (a split keeps its fates).
    let mut again = a.clone();
    draw_fates(&def, &mut again, 0, t0());
    assert_eq!(again, a);
}

/// The save's seed redraws the thresholds (the same seed the same bits),
/// keeps the hazards, and stays in the ship state for a split's pieces.
#[test]
fn a_seed_redraws_the_fates() {
    let def = falcon9();
    let mut a = launched(&def);
    a.fates[0].hazard = 0.25;
    let mut b = a.clone();
    reseed(&mut a, 42);
    reseed(&mut b, 42);
    assert_eq!(a, b);
    assert_eq!((a.seed, a.fates[0].hazard), (42, 0.25));
    let mut fresh = ShipState::initial(&def);
    draw_fates(&def, &mut fresh, 42, t0());
    assert!(a.fates.iter().zip(&fresh.fates).all(|(x, y)| x.threshold == y.threshold));
    assert!(a.fates.iter().zip(&launched(&def).fates).all(|(x, y)| x.threshold != y.threshold));
    let kept: Vec<u32> = def.parts.iter().filter(|p| p.id.starts_with("s1.")).map(|p| p.part).collect();
    assert_eq!(a.partition(&kept, &def, &def.subset(&kept), true).seed, 42);
}

/// The same running from the same launch: the same hazards to the bit; a
/// part beyond its rated burn time accumulates faster.
#[test]
fn the_same_inputs_give_the_same_hazards() {
    let def = falcon9();
    let run = |worn: bool| {
        let mut s = launched(&def);
        let e = engine(&def, "s1.merlin.3");
        s.engines[e].status = EngineStatus::Running;
        if worn {
            let part = def.engines[e].part;
            s.parts.iter_mut().find(|p| p.part == part).unwrap().history.run_time = 3600.0;
        }
        for _ in 0..500 {
            step(&def, &mut s, &env(0.02, false));
        }
        s.fates[fate(&s, "s1.merlin.3", "in-flight shutdown")].hazard
    };
    let h = run(false);
    assert_eq!(h.to_bits(), run(false).to_bits());
    assert!((h - 2.2e-3 * 10.0 / 3600.0).abs() < 1e-15, "{h}");
    assert!((run(true) / h - 4.0).abs() < 1e-9, "twice its rated burn time: ×4");
}

/// Each outcome: a benign shutdown, an ignition failure (the start
/// aborted), an uncontained engine (destroyed), a pump lost (its fin frozen).
#[test]
fn each_outcome() {
    let def = falcon9();
    let mut s = launched(&def);
    let e3 = engine(&def, "s1.merlin.3");
    s.engines[e3].status = EngineStatus::Running;
    let k = fate(&s, "s1.merlin.3", "in-flight shutdown");
    s.fates[k].threshold = 1e-12;
    assert!(step(&def, &mut s, &env(0.02, false)).is_empty());
    assert_eq!(s.engines[e3].status, EngineStatus::Failed);
    assert!(s.events.iter().any(|e| e.alarm && e.text.contains("in-flight shutdown")));
    // An ignition: the start counted, the fate reached: aborted.
    let e5 = engine(&def, "s1.merlin.5");
    s.engines[e5].status = EngineStatus::Starting;
    let part = def.engines[e5].part;
    s.parts.iter_mut().find(|p| p.part == part).unwrap().history.starts += 1;
    let k = fate(&s, "s1.merlin.5", "ignition");
    s.fates[k].threshold = 1e-12;
    step(&def, &mut s, &env(0.02, false));
    assert_eq!(s.engines[e5].status, EngineStatus::Off);
    assert!(s.engines[e5].reason.as_deref().unwrap_or("").contains("ignition"));
    // The MVac's restart: uncontained, the engine destroyed.
    let mvac = engine(&def, "s2.mvac");
    s.engines[mvac].status = EngineStatus::Starting;
    let part = def.engines[mvac].part;
    s.parts.iter_mut().find(|p| p.part == part).unwrap().history.starts += 1;
    let k = fate(&s, "s2.mvac", "restart");
    s.fates[k].threshold = 1e-12;
    let effects = step(&def, &mut s, &env(0.02, false));
    assert_eq!(effects, vec![Effect::Destroy(part, PartFailure::Uncontained)]);
    assert_eq!(s.engines[mvac].status, EngineStatus::Failed);
    assert_eq!(s.parts.iter().find(|p| p.part == part).unwrap().integrity, 0.0);
    // A grid fin's pump: lost at its cycle, the fin frozen where it is.
    let fin = def.parts.iter().find(|p| p.id == "s1.gridfin#2").unwrap().part;
    let m = def.mechanisms.iter().position(|m| m.part == fin).unwrap();
    s.parts.iter_mut().find(|p| p.part == fin).unwrap().history.cycles += 1;
    let k = fate(&s, "s1.gridfin#2", "hydraulic pump stall");
    s.fates[k].threshold = 1e-12;
    s.mechanisms[m].target = def.mechanisms[m].item.deployed_deg;
    step(&def, &mut s, &env(0.02, false));
    assert!(s.lost.iter().any(|l| l.owner == "s1.gridfin#2"));
    assert_eq!(s.mechanisms[m].target, s.mechanisms[m].angle, "frozen");
    // The others' fates are untouched.
    assert!(s.fates.iter().filter(|f| f.hazard >= f.threshold).count() == 4);
}

/// Debug mode: nothing fails, nothing accumulates.
#[test]
fn debug_mode_fails_nothing() {
    let def = falcon9();
    let mut s = launched(&def);
    let e3 = engine(&def, "s1.merlin.3");
    s.engines[e3].status = EngineStatus::Running;
    s.fates.iter_mut().for_each(|f| f.threshold = 0.0);
    let before = s.clone();
    assert!(step(&def, &mut s, &env(0.02, true)).is_empty());
    assert_eq!(s, before);
}

/// Time at temperature: an interior between its rated and destruction
/// temperatures takes dt/life of damage; at its destruction temperature it
/// is destroyed at once.
#[test]
fn time_at_temperature_destroys() {
    let def = falcon9();
    let mut s = launched(&def);
    let ids: Vec<u32> = def.parts.iter().map(|p| p.part).collect();
    let parents: Vec<Option<u32>> = vec![None; ids.len()];
    let tank = def.parts.iter().find(|p| p.id == "s2.tank").unwrap().part as usize;
    let r = physical::thermal_rating(&def, tank as u32).unwrap();
    let room = vec![295.0; ids.len()];
    let mut warm = room.clone();
    warm[tank] = 0.5 * (r.rated_k + r.destruction_k);
    let mut melting = room.clone();
    melting[tank] = r.destruction_k;
    let b = Bodies {
        ids: &ids,
        parents: &parents,
        dry: &[],
        kelvin: &room,
        pushes: &[],
        ambient: 0.0,
        hottest: &warm,
        q: 0.0,
        mach: 0.0,
    };
    let e = TickEnv { bodies: Some(b), ..env(1.0, false) };
    assert!(step(&def, &mut s, &e).is_empty());
    let d = s.parts[tank].thermal_damage;
    assert!((d - 1.0 / r.life(warm[tank])).abs() < 1e-12, "{d}");
    let b = Bodies { hottest: &melting, ..b };
    let effects = step(&def, &mut s, &TickEnv { bodies: Some(b), ..env(0.02, false) });
    assert!(matches!(effects[..], [Effect::Destroy(p, PartFailure::Melted { .. })] if p == tank as u32), "{effects:?}");
}

/// A helium bottle over its burst pressure in the second stage's LOX: its
/// blast (through the liquid) ruptures the tank: the stage is destroyed,
/// the bottle's gas gone; the other bottles hold (AMOS-6's path).
#[test]
fn a_bottle_burst_cascades_into_its_tank() {
    let def = falcon9();
    let mut s = launched(&def);
    let b = def.bottles.iter().position(|b| b.id == "s2.helium#1").unwrap();
    s.bottles[b].moles *= 1.6;
    let effects = step(&def, &mut s, &env(0.02, false));
    let tank = def.bottles[b].part;
    assert!(matches!(effects[..], [Effect::Destroy(p, PartFailure::Blast { .. })] if p == tank), "{effects:?}");
    assert_eq!(s.bottles[b].moles, 0.0);
    let others = def.bottles.iter().enumerate().filter(|(i, x)| *i != b && x.id.starts_with("s2.helium"));
    assert!(others.clone().count() == 2 && others.into_iter().all(|(i, _)| s.bottles[i].moles > 0.0));
    let texts: Vec<&str> = s.events.iter().map(|e| e.text.as_str()).collect();
    assert!(texts.iter().any(|t| t.contains("s2.helium#1: burst")), "{texts:?}");
}
