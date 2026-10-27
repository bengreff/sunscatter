//! The Falcon 9's joints in flight (ship-model.md §5.6, §11): held on the
//! pad at full thrust the stack above the octaweb only feels its own weight
//! (the clamps carry thrust less weight); after lift-off each stack joint
//! carries what is above it at the proper acceleration the tick flew, none
//! near its ultimate; the loads are the same in frames and in one jump.

use super::*;
use crate::craft::{craft_of, CraftId};
use crate::ephem::Ephemeris;
use crate::sol;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn t0() -> Epoch {
    sol::sol_epoch().add_seconds(2.0 * 86_400.0)
}

fn full() -> Controls {
    Controls { throttle: 1.0, sas: true, ..Default::default() }
}

/// The run of the joint named `name` and its definition.
fn joint(v: &Vessel, name: &str) -> (crate::craft::ship::JointDef, crate::systems::runtime::JointRun) {
    let def = v.ship_def().unwrap();
    let j = def.joints.iter().position(|j| j.name == name).unwrap_or_else(|| panic!("{name}"));
    (def.joints[j].clone(), v.ship().unwrap().runtime.joints[j].clone())
}

/// The mass (kg) of the parts not hanging from `child` (the capsule's side).
fn above(v: &Vessel, child: u32) -> f64 {
    let design = v.craft.design();
    let parts = &design.parts;
    let mut inside: Vec<bool> = parts.iter().map(|p| p.id == child).collect();
    for _ in 0..parts.len() {
        for (k, p) in parts.iter().enumerate() {
            if p.parent.is_some_and(|q| inside[q as usize]) {
                inside[k] = true;
            }
        }
    }
    let (def, state) = v.ship_parts().unwrap();
    let ids: Vec<u32> = parts.iter().map(|p| p.id).collect();
    let b = crate::systems::structure::Bodies {
        ids: &ids,
        parents: &[],
        dry: &design.part_mass,
        kelvin: &[],
        pushes: &[],
        ambient: 0.0,
        hottest: &[],
        q: 0.0,
        mach: 0.0,
    };
    let mass = crate::systems::structure::part_masses(def, state, &b);
    (0..parts.len()).filter(|&k| !inside[k]).map(|k| mass[k].mass).sum()
}

#[test]
fn the_falcon_9_joints_on_the_pad_and_after_lift_off() {
    let w = world();
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    let mut v = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft);
    v.next_stage(&w).unwrap();
    v.advance(&w, t0().add_seconds(1.98), &full(), usize::MAX);
    assert!(v.clamped());
    // Held at nearly full thrust: the tank on the octaweb carries the weight
    // above it (the clamps take thrust less weight).
    let (d, run) = joint(&v, "s1.tank|s1.octaweb");
    let g = v.ship().unwrap().runtime.accel.length();
    let weight = above(&v, d.child) * g;
    let along = run.force.dot(d.normal);
    println!("held: {:.0} kN on the octaweb's joint against {:.0} kN of weight above", along / 1e3, weight / 1e3);
    // (The stack stands on the ground's normal, a little off the plumb line.)
    assert!((along / weight - 1.0).abs() < 1e-3, "{along} against {weight}");
    // Lift-off, the mount left as ground, then 3 s of flight.
    v.next_stage(&w).unwrap();
    v.advance(&w, v.time.add_seconds(0.1), &full(), usize::MAX);
    let plan = v.plan_breakup().expect("the mount separates");
    let mut ids = VesselIds::default();
    ids.allocate();
    v.split(&w, &plan, &plan.crafts(), &mut ids);
    let mut frames = v.clone();
    let end = v.time.add_seconds(3.0);
    v.advance(&w, end, &full(), usize::MAX);
    assert!(v.plan_breakup().is_none(), "nothing broke");
    assert!(v.ship_events().iter().all(|e| !e.text.contains("snapped") && !e.text.contains("buckled")));
    let a = v.ship().unwrap().runtime.accel.length();
    for name in
        ["dragon separation", "s2.ring|s2.tank", "stage separation", "s1.interstage|s1.tank", "s1.tank|s1.octaweb"]
    {
        let (d, run) = joint(&v, name);
        let expect = above(&v, d.child) * a;
        let along = run.force.dot(d.normal);
        println!("{name:24} {:6.0} kN ({:.0} kN by hand), ratio {:.3}", along / 1e3, expect / 1e3, run.ratio);
        // The air (at ~30 m/s) and the gimbal shift it by a little.
        assert!((along / expect - 1.0).abs() < 0.01, "{name}: {along} against {expect}");
        assert!(run.ratio > 0.0 && run.ratio < 0.5, "{name}: {}", run.ratio);
    }
    // The same bits in 60 fps frames.
    let mut t = frames.time;
    while t < end {
        let next = t.add_seconds(1.0 / 60.0);
        t = if next < end { next } else { end };
        frames.advance(&w, t, &full(), usize::MAX);
    }
    assert_eq!(frames.ship().unwrap().runtime.joints, v.ship().unwrap().runtime.joints);
}

/// The ascent at full throttle (straight up, SAS holding): lift-off, MECO
/// at the booster's last ~7 %, stage separation and the MVac's burn. Every
/// part's normal operating temperatures and loads sit inside its limits:
/// nothing melts, bursts, snaps or buckles (the radiatively cooled MVac
/// bell runs ~1,400 K on its niobium, judged by its own material).
#[test]
fn the_falcon_9_ascent_breaks_nothing() {
    let w = world();
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    let mut v = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft);
    let mut ids = VesselIds::default();
    ids.allocate();
    v.next_stage(&w).unwrap();
    v.next_stage(&w).unwrap();
    v.advance(&w, t0().add_seconds(3.0), &full(), usize::MAX);
    let plan = v.plan_breakup().expect("the mount separates");
    v.split(&w, &plan, &plan.crafts(), &mut ids);
    let bad = |v: &Vessel| -> Vec<String> {
        let words = ["destroyed", "burst", "ruptured", "snapped", "buckled", "melted"];
        (v.ship_events().iter()).filter(|e| words.iter().any(|w| e.text.contains(w))).map(|e| e.text.clone()).collect()
    };
    // The booster to ~7 % of its propellant (MECO).
    let def = v.ship_def().unwrap();
    let s1: Vec<usize> = (0..def.tanks.len()).filter(|&i| def.tanks[i].id.starts_with("s1.")).collect();
    let load: f64 = s1.iter().map(|&i| v.ship().unwrap().tanks[i].liquid).sum();
    let mut t = 0.0;
    while s1.iter().map(|&i| v.ship().unwrap().tanks[i].liquid).sum::<f64>() > 0.07 * load && t < 200.0 {
        t += 1.0;
        v.advance(&w, v.time.add_seconds(1.0), &full(), usize::MAX);
        assert!(v.plan_breakup().is_none(), "at T+{t}: {:?}", bad(&v));
    }
    let (anchor, r, vel) = v.state(&w);
    let up = crate::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0;
    println!("MECO at T+{t:.0} s: {:.1} km up, {:.0} m/s", up / 1e3, vel.length());
    let peak = |v: &Vessel| {
        let s = v.ship().unwrap();
        s.runtime.joints.iter().map(|j| j.ratio).fold(0.0, f64::max)
    };
    println!("peak joint ratio now {:.2}", peak(&v));
    assert!(bad(&v).is_empty(), "{:?}", bad(&v));
    v.next_stage(&w).unwrap();
    v.next_stage(&w).unwrap();
    let plan = v.plan_breakup().expect("the stages separate");
    let mut booster = v.split(&w, &plan, &plan.crafts(), &mut ids).remove(0);
    v.next_stage(&w).unwrap();
    for _ in 0..30 {
        let next = v.time.add_seconds(1.0);
        v.advance(&w, next, &full(), usize::MAX);
        booster.advance(&w, next, &Controls::default(), usize::MAX);
        assert!(v.plan_breakup().is_none() && booster.plan_breakup().is_none(), "{:?} {:?}", bad(&v), bad(&booster));
    }
    let mvac = v.ship().unwrap().runtime.engines.iter().map(|e| e.level).fold(0.0, f64::max);
    println!("MVac level {mvac:.2}; events: {:?}", v.ship_events().iter().map(|e| &e.text).collect::<Vec<_>>());
    assert!(bad(&v).is_empty() && bad(&booster).is_empty(), "{:?} {:?}", bad(&v), bad(&booster));
}

/// The demo's gravity turn: the attitude held directly along a pitch
/// program (pitch from vertical 1.2·√(h / 70 km) from 500 m), so the stack
/// flies through max-Q (~36 kPa) at ~4° angle of attack. The air's drag is
/// not all on the nose (the forebody's share, the rest along the body) and
/// its normal force is spread along the stack: the Dragon ring peaks well
/// under its ultimate and nothing breaks.
#[test]
fn the_demo_gravity_turn_breaks_nothing() {
    let w = world();
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    let mut v = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6084, -80.6043, t0(), &craft);
    let mut ids = VesselIds::default();
    ids.allocate();
    v.next_stage(&w).unwrap();
    v.next_stage(&w).unwrap();
    v.advance(&w, t0().add_seconds(3.0), &full(), usize::MAX);
    let plan = v.plan_breakup().expect("the mount separates");
    v.split(&w, &plan, &plan.crafts(), &mut ids);
    let liftoff = v.time;
    let def = v.ship_def().unwrap();
    let ring = def.joints.iter().position(|j| j.name == "s2.ring|s2.tank").unwrap();
    let (mut peak, mut q_max) = (0.0f64, 0.0f64);
    while v.time.seconds_since(liftoff) < 75.0 {
        let (anchor, r, vel) = v.state(&w);
        let alt = crate::forces::altitude_above(&w, &w.snapshot(v.time), anchor, anchor, r).0;
        let up = r.normalize();
        let h = vel - up * vel.dot(up);
        let h = if h.length() > 1.0 { h.normalize() } else { DVec3::Z.cross(up).normalize() };
        let pitch = if alt < 500.0 { 0.0 } else { (1.2 * (alt / 70_000.0).sqrt()).min(1.2) };
        let dir = up * pitch.cos() + h * pitch.sin();
        v.set_attitude(Attitude { q: crate::vessel::quat_z_to(dir.normalize()), omega: DVec3::ZERO });
        v.advance(&w, v.time.add_seconds(1.0 / 30.0), &full(), usize::MAX);
        assert!(v.plan_breakup().is_none(), "at T+{:.1}: {:?}", v.time.seconds_since(liftoff), v.ship_events());
        let snap = w.snapshot(v.time);
        let (anchor, r, vel) = v.state(&w);
        let q = crate::vessel::air_at(&w, &snap, anchor, r, vel).map_or(0.0, |a| 0.5 * a.rho * a.wind.length_squared());
        q_max = q_max.max(q);
        peak = peak.max(v.ship().unwrap().runtime.joints[ring].ratio);
    }
    println!("max-Q {:.1} kPa; the Dragon ring's peak ratio {peak:.2}", q_max / 1e3);
    assert!(q_max > 30e3, "through max-Q: {q_max}");
    assert!(peak < 0.7, "{peak}");
}
