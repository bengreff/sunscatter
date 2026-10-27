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
        hottest: &kelvin,
        q: 0.0,
        mach: 0.0,
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
        // (Its own off-axis masses turn it a little: the forces' own motion.)
        assert!((along / expect - 1.0).abs() < 1e-6, "{name}: {along} against {expect}");
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
    let b = Bodies {
        ids: &ids,
        parents: &parents,
        dry: &craft.part_mass,
        kelvin: &kelvin,
        pushes: &pushes,
        ambient: 0.0,
        hottest: &kelvin,
        q: 0.0,
        mach: 0.0,
    };
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

/// Max-Q at 4° of angle of attack (falcon9.md §4.3: ~36 kPa near Mach
/// 1.3), the Merlins throttled to ~75 %: the axial drag on the nose
/// (q·S·0.5), slender-body lift on the nose (2α·q·S) and crossflow drag
/// along the tanks (1.2·sin²α·q·D·L), the gimbal turning the thrust so the
/// moment about the centre of mass vanishes (as the flight computer holds
/// it). Every joint stays under 0.7 of its ultimate (the design factor 1.4).
#[test]
fn max_q_at_a_few_degrees_bends_the_stack_safely() {
    let craft = falcon9();
    let def = craft.ship.as_ref().unwrap();
    let mut state = ShipState::initial(def);
    // ~70 s of burn gone from the first stage (its regulator holds 3.5 bar).
    for (t, s) in def.tanks.iter().zip(state.tanks.iter_mut()) {
        if t.id.starts_with("s1.") {
            s.liquid *= 0.55;
            let rho = crate::fluids::fluids().liquid(&t.fluid).unwrap().density(s.liquid_k);
            s.ullage_moles = crate::fluids::moles(3.5e5, t.volume - s.liquid / rho, s.ullage_k);
        }
    }
    state.runtime.pressure_regulated = true;
    let (ids, parents, kelvin) = bodies_of(&craft);
    let index = |name: &str| craft.parts.iter().position(|p| def.parts[p.id as usize].id == name).unwrap();
    let (q, alpha) = (36e3, 4.0f64.to_radians());
    let area = crate::math::PI * 1.83 * 1.83;
    let mut pushes = merlins(&craft, 0.75 * 914e3 - 20e3 * 0.681);
    let lateral = DVec3::X;
    let nose = Push {
        part: index("dragon.capsule"),
        force: -DVec3::Z * (q * area * 0.5) + lateral * (2.0 * alpha * q * area),
        at: DVec3::Z * 2.0,
        torque: DVec3::ZERO,
    };
    pushes.push(nose);
    let cross = |len: f64| 1.2 * alpha.sin() * alpha.sin() * q * 3.66 * len;
    for (name, z, len) in [("s2.tank", -9.0, 9.0), ("s1.tank", -39.0, 35.5)] {
        pushes.push(Push { part: index(name), force: lateral * cross(len), at: DVec3::Z * z, torque: DVec3::ZERO });
    }
    // The gimbal: a side force at the engines cancelling the moment about
    // the centre of mass.
    let total = {
        let b = Bodies {
            ids: &ids,
            parents: &parents,
            dry: &craft.part_mass,
            kelvin: &kelvin,
            pushes: &[],
            ambient: 20e3,
            hottest: &kelvin,
            q,
            mach: 1.3,
        };
        MassProps::sum(&part_masses(def, &state, &b))
    };
    let tau: DVec3 = pushes.iter().map(|p| (p.at - total.com).cross(p.force)).sum();
    let engines = DVec3::Z * -59.0;
    let lever = engines - total.com;
    let side = lever.cross(tau) / lever.length_squared();
    let side = side - lever * side.dot(lever) / lever.length_squared();
    pushes.push(Push { part: index("s1.octaweb"), force: side, at: engines, torque: DVec3::ZERO });
    let b = Bodies {
        ids: &ids,
        parents: &parents,
        dry: &craft.part_mass,
        kelvin: &kelvin,
        pushes: &pushes,
        ambient: 20e3,
        hottest: &kelvin,
        q,
        mach: 1.3,
    };
    let m = consistent(&total, &pushes, DVec3::ZERO);
    println!(
        "max-Q: {:.2} g, {:.4} rad/s² (balanced), gimbal side force {:.0} kN",
        m.accel.length() / 9.80665,
        m.alpha.length(),
        side.length() / 1e3
    );
    assert!(m.alpha.length() < 1e-3, "{:?}", m.alpha);
    let effects = step(def, &mut state, &b, &Motion::default(), Epoch::new(1_000_000_000, 0.0), false);
    assert!(effects.is_empty());
    for (j, d) in def.joints.iter().enumerate() {
        let r = &state.runtime.joints[j];
        if r.ratio > 0.05 {
            println!(
                "{:28} {:6.0} kN, {:5.0} kN·m, ratio {:.3}",
                d.name,
                r.force.dot(d.normal) / 1e3,
                r.moment.length() / 1e3,
                r.ratio
            );
        }
        assert!(r.ratio <= 0.7, "{}: {}", d.name, r.ratio);
    }
}

/// Until the plumbing regulates the tanks' pressures, a draining first
/// stage keeps its flight pressure's relief: near MECO with its loaded gas
/// only (0.07 bar in its 95 % ullage) its joints stay well under their
/// ultimate; regulated, the real pressure counts (and without its
/// pressurant the aft joint is near buckling).
#[test]
fn an_unregulated_tank_keeps_its_flight_pressure() {
    let craft = falcon9();
    let def = craft.ship.as_ref().unwrap();
    let mut state = ShipState::initial(def);
    for (t, s) in def.tanks.iter().zip(state.tanks.iter_mut()) {
        if t.id.starts_with("s1.") {
            s.liquid *= 0.05;
        }
    }
    let (ids, parents, kelvin) = bodies_of(&craft);
    let pushes = merlins(&craft, 914e3);
    let b = Bodies {
        ids: &ids,
        parents: &parents,
        dry: &craft.part_mass,
        kelvin: &kelvin,
        pushes: &pushes,
        ambient: 0.0,
        hottest: &kelvin,
        q: 0.0,
        mach: 0.0,
    };
    let total: f64 = part_masses(def, &state, &b).iter().map(|m| m.mass).sum();
    let m = Motion { accel: DVec3::Z * (9.0 * 914e3 / total), ..Motion::default() };
    let t0 = Epoch::new(1_000_000_000, 0.0);
    let ratio = |state: &ShipState, name: &str| state.runtime.joints[joint(def, name)].ratio;
    assert!(!state.runtime.pressure_regulated);
    assert!(step(def, &mut state, &b, &m, t0, false).is_empty());
    for name in ["s1.interstage|s1.tank", "s1.tank|s1.octaweb"] {
        assert!(ratio(&state, name) < 0.5, "{name}: {}", ratio(&state, name));
    }
    let mut regulated = ShipState::initial(def);
    regulated.tanks = state.tanks.clone();
    regulated.runtime.pressure_regulated = true;
    step(def, &mut regulated, &b, &m, t0, true);
    assert!(ratio(&regulated, "s1.tank|s1.octaweb") > 0.75, "{}", ratio(&regulated, "s1.tank|s1.octaweb"));
}

/// Over its ultimate a joint snaps (once), logged as an alarm; in debug
/// mode nothing snaps.
#[test]
fn an_overload_snaps_the_joint() {
    let craft = falcon9();
    let def = craft.ship.as_ref().unwrap();
    let (ids, parents, kelvin) = bodies_of(&craft);
    let pushes = merlins(&craft, 845e3 * 22.0);
    let b = Bodies {
        ids: &ids,
        parents: &parents,
        dry: &craft.part_mass,
        kelvin: &kelvin,
        pushes: &pushes,
        ambient: 101_325.0,
        hottest: &kelvin,
        q: 0.0,
        mach: 0.0,
    };
    // ~30 g (22 times the thrust): the forward skirt under Dragon goes first.
    let m = Motion::default();
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
    // A leg hangs by its hinge and strut, not a ring: its load is shown,
    // never judged (its feet's overload is the contact's).
    let leg = &state.runtime.joints[joint(def, "s1.octaweb|s1.leg#1")];
    assert!(leg.force.length() > 1e4 && leg.ratio == 0.0, "{leg:?}");
    // Snapped joints are not judged again.
    let again = step(def, &mut state, &b, &m, t0, false);
    assert!(again.iter().all(|e| !effects.contains(e)), "{again:?}");
}

/// The g-limit's scale (`JointRun::to_ultimate`): at lift-off the weakest
/// joint's, and the forces scaled by it bring that joint to its ultimate
/// load (just under: not yet; just over: there).
#[test]
fn the_scale_to_the_ultimate_is_where_the_weakest_joint_fails() {
    let craft = falcon9();
    let def = craft.ship.as_ref().unwrap();
    let (ids, parents, kelvin) = bodies_of(&craft);
    let run = |scale: f64| {
        let pushes = merlins(&craft, 845e3 * scale);
        let b = Bodies {
            ids: &ids,
            parents: &parents,
            dry: &craft.part_mass,
            kelvin: &kelvin,
            pushes: &pushes,
            ambient: 101_325.0,
            hottest: &kelvin,
            q: 0.0,
            mach: 0.0,
        };
        let mut state = ShipState::initial(def);
        step(def, &mut state, &b, &Motion::default(), Epoch::new(1_000_000_000, 0.0), true);
        state
    };
    let now = run(1.0);
    let (j, k) = (now.runtime.joints.iter().enumerate())
        .filter_map(|(j, r)| r.to_ultimate.map(|k| (j, k)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("a joint with data");
    // The first stage's tank on the octaweb (its compression grows with the
    // thrust, its ullage pressure does not) at ~3.4× the lift-off's 1.5 g.
    println!("weakest at lift-off: {} at {k:.2}× ({:.1} g)", def.joints[j].name, k * 1.5);
    assert_eq!(def.joints[j].name, "s1.tank|s1.octaweb");
    assert!(k > 2.0 && k < 22.0, "{k}");
    assert!(run(k * 0.995).runtime.joints[j].ratio < 1.0);
    assert!(run(k * 1.005).runtime.joints[j].ratio >= 1.0);
    // A leg (no capacity data) never limits.
    assert_eq!(now.runtime.joints[joint(def, "s1.octaweb|s1.leg#1")].to_ultimate, None);
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

/// What the systems' steps of SYSTEMS2 cost per live tick on the Falcon 9
/// (ship-model.md §10's budget: the whole systems step under 0.5 ms).
#[test]
#[ignore]
fn cost_per_tick() {
    let craft = falcon9();
    let def = craft.ship.as_ref().unwrap();
    let mut state = ShipState::initial(def);
    state.clamped = false;
    let (ids, parents, kelvin) = bodies_of(&craft);
    let pushes = merlins(&craft, 845e3);
    let b = Bodies {
        ids: &ids,
        parents: &parents,
        dry: &craft.part_mass,
        kelvin: &kelvin,
        pushes: &pushes,
        ambient: 101_325.0,
        hottest: &kelvin,
        q: 20e3,
        mach: 1.0,
    };
    let m = Motion { accel: DVec3::Z * 14.0, ..Motion::default() };
    let t0 = Epoch::new(1_000_000_000, 0.0);
    let env = crate::systems::ship::TickEnv {
        at: t0,
        dt: 0.02,
        p: 101_325.0,
        throttle: 1.0,
        accel: m.accel,
        omega: DVec3::ZERO,
        alpha: DVec3::ZERO,
        com: DVec3::ZERO,
        sun: DVec3::new(-1361.0, 0.0, 0.0),
        air_k: Some(288.0),
        debug: false,
        bodies: Some(b),
    };
    let n = 2000;
    let t = std::time::Instant::now();
    for _ in 0..n {
        step(def, &mut state, &b, &m, t0, false);
    }
    let s = t.elapsed().as_secs_f64() / f64::from(n) * 1e3;
    let t = std::time::Instant::now();
    for _ in 0..n {
        crate::systems::failure::step(def, &mut state, &env);
    }
    let f = t.elapsed().as_secs_f64() / f64::from(n) * 1e3;
    let t = std::time::Instant::now();
    for _ in 0..n {
        crate::systems::power::step_ship(def, &mut state, env.sun, 0.02, t0);
        crate::systems::crew::step(def, &mut state, Some(m.accel), 293.0, 0.02, t0);
    }
    let p = t.elapsed().as_secs_f64() / f64::from(n) * 1e3;
    println!("per tick: structure {s:.3} ms, failures {f:.3} ms, power and crew {p:.3} ms");
}
