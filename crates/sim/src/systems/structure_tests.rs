//! The Falcon 9's joints under its own engines (ship-model.md §5.6;
//! falcon9.md §1, §6.4): standing on the pad's clamps, at lift-off and near
//! MECO every joint carries what hand calculations give, and none is near
//! its ultimate load in a nominal flight.

use super::*;
use crate::craft::state::ShipState;
use crate::craft::{craft_of, Craft, CraftId};
use std::sync::Arc;

fn falcon9() -> Arc<Craft> {
    craft_of(&CraftId("falcon9-crew".into()))
}

/// The parts of `craft` as the joints see them, at `kelvin`.
fn bodies_of(craft: &Craft) -> (Vec<u32>, Vec<Option<u32>>, Vec<f64>) {
    let ids = craft.parts.iter().map(|p| p.id).collect();
    let parents = craft.parts.iter().map(|p| p.parent).collect();
    (ids, parents, vec![295.0; craft.parts.len()])
}

/// The nine Merlins at `thrust` each along +Z at their gimbal points.
fn merlins(craft: &Craft, thrust: f64) -> Vec<Push> {
    let def = craft.ship.as_ref().unwrap();
    let index = |id: u32| craft.parts.iter().position(|p| p.id == id).unwrap();
    (def.engines.iter())
        .filter(|e| e.owner.starts_with("s1.merlin"))
        .map(|e| Push { part: index(e.part), force: e.data.dir * thrust, at: e.data.pos, torque: DVec3::ZERO })
        .collect()
}

fn joint(def: &ShipDef, name: &str) -> usize {
    def.joints.iter().position(|j| j.name == name).unwrap_or_else(|| panic!("{name}"))
}

/// Lift-off: nine Merlins at their sea-level thrust (845 kN), the vehicle
/// off its clamps accelerating at (T − 0)/M proper. Each stack joint
/// carries the inertia of what is above it (the capsule is the root, at the
/// top): F = −m_above·a along the stack, compression; the tank joints are
/// relieved by their ullage and liquid heads.
#[test]
fn lift_off_loads_by_hand() {
    let craft = falcon9();
    let def = craft.ship.as_ref().unwrap();
    let mut state = ShipState::initial(def);
    let (ids, parents, kelvin) = bodies_of(&craft);
    let pushes = merlins(&craft, 845e3);
    let b = Bodies {
        ids: &ids,
        parents: &parents,
        dry: &craft.part_mass,
        kelvin: &kelvin,
        pushes: &pushes,
        ambient: 101_325.0,
    };
    let mass = part_masses(def, &state, &b);
    let m_total: f64 = mass.iter().map(|m| m.mass).sum();
    let a = 9.0 * 845e3 / m_total;
    println!("lift-off: {m_total:.0} kg, proper acceleration {a:.2} m/s²");
    assert!((m_total / 560_200.0 - 1.0).abs() < 0.01, "{m_total}");
    let m = Motion { accel: DVec3::Z * a, ..Motion::default() };
    let t0 = Epoch::new(1_000_000_000, 0.0);
    let effects = step(def, &mut state, &b, &m, t0, false);
    assert!(effects.is_empty(), "{effects:?}");
    // The mass above each stack joint (the capsule's side) by hand.
    let index = |id: u32| craft.parts.iter().position(|p| p.id == id).unwrap();
    let above = |j: &JointDef| -> f64 {
        // Everything not in the child's subtree.
        let mut inside = vec![false; craft.parts.len()];
        inside[index(j.child)] = true;
        for _ in 0..craft.parts.len() {
            for (k, p) in craft.parts.iter().enumerate() {
                if p.parent.is_some_and(|q| inside[q as usize]) {
                    inside[k] = true;
                }
            }
        }
        (0..craft.parts.len()).filter(|&k| !inside[k]).map(|k| mass[k].mass).sum()
    };
    for name in
        ["dragon separation", "s2.ring|s2.tank", "stage separation", "s1.interstage|s1.tank", "s1.tank|s1.octaweb"]
    {
        let j = joint(def, name);
        let d = &def.joints[j];
        let run = &state.runtime.joints[j];
        let expect = above(d) * a;
        // The parent pushes the child's side away from it (down the stack).
        let along = run.force.dot(d.normal);
        assert!((along / expect - 1.0).abs() < 1e-9, "{name}: {along} against {expect}");
        // Only the off-axis equipment's lever (Dragon's centre of mass is
        // ~10 cm off the axis) bends it: a few percent of the line load.
        assert!(run.moment.length() < 0.1 * expect * d.radius, "{name}: {:?}", run.moment);
        let gauge = section_gauge(def, &state, d, m.accel, 101_325.0);
        println!("{name:24} {:7.0} kN compression, gauge {:5.2} bar, ratio {:.3}", along / 1e3, gauge / 1e5, run.ratio);
        assert!(run.ratio < 0.5, "{name}: {}", run.ratio);
    }
    // The tank joints are pressurised (3.5 bar loaded), the skirts and the
    // interstage are not.
    let cases = [
        ("s2.ring|s2.tank", false),
        ("s2.tank|s2.skirt", true),
        ("stage separation", false),
        ("s1.interstage|s1.tank", true),
        ("s1.tank|s1.octaweb", true),
    ];
    for (name, pressurised) in cases {
        let d = &def.joints[joint(def, name)];
        assert_eq!(section_gauge(def, &state, d, m.accel, 101_325.0) > 2e5, pressurised, "{name}");
    }
}

/// Near MECO: the first stage nearly empty, nine Merlins at their vacuum
/// thrust (914 kN), ~4.5 g. The second stage's tank and the stage
/// separation carry the full upper stack; still under their ultimate, the
/// tank joints only thanks to their pressure.
#[test]
fn near_meco_the_upper_stack_is_heaviest() {
    let craft = falcon9();
    let def = craft.ship.as_ref().unwrap();
    let mut state = ShipState::initial(def);
    // 5 % left, the ullage held at its 3.5 bar by the helium (the
    // regulator's work, `systems::plumbing`).
    for (t, s) in def.tanks.iter().zip(state.tanks.iter_mut()) {
        if t.id.starts_with("s1.") {
            s.liquid *= 0.05;
            let rho = crate::fluids::fluids().liquid(&t.fluid).unwrap().density(s.liquid_k);
            s.ullage_moles = crate::fluids::moles(3.5e5, t.volume - s.liquid / rho, s.ullage_k);
        }
    }
    let (ids, parents, kelvin) = bodies_of(&craft);
    let pushes = merlins(&craft, 914e3);
    let b =
        Bodies { ids: &ids, parents: &parents, dry: &craft.part_mass, kelvin: &kelvin, pushes: &pushes, ambient: 0.0 };
    let total: f64 = part_masses(def, &state, &b).iter().map(|m| m.mass).sum();
    let a = 9.0 * 914e3 / total;
    println!("near MECO: {total:.0} kg, {:.2} g", a / 9.80665);
    let m = Motion { accel: DVec3::Z * a, ..Motion::default() };
    let effects = step(def, &mut state, &b, &m, Epoch::new(1_000_000_000, 0.0), false);
    for (j, d) in def.joints.iter().enumerate() {
        let r = &state.runtime.joints[j];
        if r.ratio > 0.0 {
            println!("{:28} {:8.0} kN ratio {:.3}", d.name, r.force.dot(d.normal) / 1e3, r.ratio);
        }
    }
    assert!(effects.is_empty(), "{effects:?}");
    // Without its pressure the second stage's tank would buckle here.
    let d = &def.joints[joint(def, "s2.tank|s2.skirt")];
    let run = &state.runtime.joints[joint(def, "s2.tank|s2.skirt")];
    let bare = section(run.force, run.moment, d.normal, d.radius, 0.0);
    assert!(bare.compression > d.buckling, "{} against {}", bare.compression, d.buckling);
}

/// Over its ultimate a joint snaps (once), logged as an alarm; in debug
/// mode nothing snaps.
#[test]
fn an_overload_snaps_the_joint() {
    let craft = falcon9();
    let def = craft.ship.as_ref().unwrap();
    let (ids, parents, kelvin) = bodies_of(&craft);
    let pushes = merlins(&craft, 845e3);
    let b = Bodies {
        ids: &ids,
        parents: &parents,
        dry: &craft.part_mass,
        kelvin: &kelvin,
        pushes: &pushes,
        ambient: 101_325.0,
    };
    // 30 g: the forward skirt under Dragon goes first.
    let m = Motion { accel: DVec3::Z * 300.0, ..Motion::default() };
    let t0 = Epoch::new(1_000_000_000, 0.0);
    let mut debug = ShipState::initial(def);
    assert!(step(def, &mut debug, &b, &m, t0, true).is_empty());
    assert!(debug.joints.iter().all(|j| j.intact));
    let mut state = ShipState::initial(def);
    let effects = step(def, &mut state, &b, &m, t0, false);
    let ring = def.parts.iter().find(|p| p.id == "s2.tank").unwrap().part;
    assert!(effects.iter().any(|e| matches!(e, Effect::Snap(c, _) if *c == ring)), "{effects:?}");
    assert!(
        state.runtime.events.iter().any(|e| e.alarm && e.text.contains("s2.ring|s2.tank")),
        "{:?}",
        state.runtime.events
    );
    // Snapped joints are not judged again.
    let again = step(def, &mut state, &b, &m, t0, false);
    assert!(again.iter().all(|e| !effects.contains(e)), "{again:?}");
}

#[test]
fn pressure_raises_the_buckling_load() {
    // x = 0.605·γ·p·R/n0: the second stage's 73 kN/m at 3.5 bar is x ≈ 1.7.
    let n0 = 73e3;
    assert_eq!(pressurised_buckling(n0, 1.83, 0.0), n0);
    let n = pressurised_buckling(n0, 1.83, 3.5e5);
    let x = 0.605 * crate::catalog::KNOCKDOWN * 3.5e5 * 1.83 / n0;
    assert!((x - 1.75).abs() < 0.05, "{x}");
    assert!((n / n0 - (0.33 + pressure_knockdown_gain(x)) / 0.33).abs() < 1e-12);
    assert!(n > 2.4 * n0 && n < 2.8 * n0, "{}", n / n0);
    // The curve: 0 at no pressure, rising, under 0.6.
    let mut last = 0.0;
    for x in [0.0, 0.01, 0.1, 0.3, 1.0, 3.0, 10.0, 100.0] {
        let g = pressure_knockdown_gain(x);
        assert!(g >= last && g < 0.6, "{x}: {g}");
        last = g;
    }
}
