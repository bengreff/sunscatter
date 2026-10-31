//! The ship state's split and merge (`craft::partition`, F1 of the
//! fidelity review): a state with every field set, split at the Falcon 9's
//! stage separation and at Dragon's, puts each item in exactly one piece
//! (the program, the vessel's as a whole, a copy in each), and merging the
//! pieces gives it back bit for bit.

use super::ship::ShipDef;
use super::state::{Event, Fate, JointState, Lost, Order, OrderKind, ShipState, EVENTS_KEPT};
use crate::systems::crew::{CrewRun, Injury};
use crate::systems::power::BusRun;
use crate::time::Epoch;
use glam::DVec3;

fn falcon9() -> ShipDef {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    (*super::load_definition(&path).unwrap().ship.unwrap()).clone()
}

/// A distinct value for item `i` of a list.
fn v(i: usize, base: f64) -> f64 {
    base + 0.125 * (i + 1) as f64
}

/// The ship state with every field off its value at load.
fn every_field_set(def: &ShipDef) -> ShipState {
    let t0 = Epoch::new(1_000_000_000, 0.25);
    let mut s = ShipState::initial(def);
    for (i, x) in s.tanks.iter_mut().enumerate() {
        (x.liquid, x.liquid_k, x.ullage_moles, x.ullage_k) = (v(i, 10.0), v(i, 90.0), v(i, 5.0), v(i, 100.0));
        (x.drift, x.drift_speed, x.pressurant_in, x.vented) = (v(i, 0.1), v(i, 0.2), v(i, 3.0), v(i, 1.0));
    }
    for (i, x) in s.bottles.iter_mut().enumerate() {
        (x.moles, x.k) = (v(i, 7.0), v(i, 80.0));
    }
    for (i, x) in s.engines.iter_mut().enumerate() {
        x.status = super::state::EngineStatus::Running;
        (x.timer, x.ignitions, x.throttle, x.gimbal) = (v(i, 1.0), i as u32, v(i, 0.1), [v(i, 0.01), -v(i, 0.02)]);
        x.selected = !x.selected;
    }
    for x in &mut s.thrusters {
        x.enabled = !x.enabled;
    }
    for (i, x) in s.valves.iter_mut().enumerate() {
        (x.open, x.stuck, x.cycles) = (!x.open, true, i as u32 + 1);
    }
    for (i, x) in s.batteries.iter_mut().enumerate() {
        (x.charge, x.k) = (v(i, 1e5), v(i, 300.0));
    }
    for (i, x) in s.mechanisms.iter_mut().enumerate() {
        (x.angle, x.target) = (v(i, 3.0), v(i, 4.0));
    }
    for (i, x) in s.joints.iter_mut().enumerate() {
        x.intact = i % 2 == 0;
    }
    let child = def.parts.iter().find(|p| p.id.starts_with("dragon.")).unwrap().part;
    // Cut at an earlier split from a part this craft no longer has.
    s.severed = vec![JointState { parent: 9_999, child, intact: false, ratio: 0.5, ..Default::default() }];
    for (i, x) in s.parts.iter_mut().enumerate() {
        (x.integrity, x.thermal_damage, x.vibration_damage) = (v(i, 0.5), v(i, 0.01), v(i, 0.02));
        x.history.run_time = v(i, 60.0);
        (x.history.starts, x.history.flights) = (i as u32, 2);
    }
    for x in &mut s.power {
        x.on = !x.on;
    }
    let cabin = s.cabin.as_mut().expect("Dragon's cabin");
    (cabin.o2, cabin.co2, cabin.k) = (20_000.0, 400.0, 297.0);
    // A keyed list in part order.
    s.fates = (def.parts.iter().step_by(7))
        .map(|p| Fate { part: p.part, owner: p.id.clone(), mode: "wear".into(), threshold: 1.5, hazard: 0.25, seen: 3 })
        .collect();
    s.clamped = true;
    s.sequence = 1;
    s.stages[0].pinned = !s.stages[0].pinned;
    s.functions[0].pinned = !s.functions[0].pinned;
    for (i, x) in s.engines.iter_mut().enumerate() {
        (x.level, x.cutoff, x.reason, x.settling) = (v(i, 0.5), v(i, 0.6), Some(format!("{i}")), Some(v(i, 0.3)));
    }
    for (i, x) in s.joints.iter_mut().enumerate() {
        (x.force, x.moment, x.ratio, x.to_ultimate) =
            (DVec3::splat(v(i, 1e3)), DVec3::X * v(i, 9.0), v(i, 0.3), Some(v(i, 2.0)));
    }
    s.orders = vec![Order { at: t0.add_seconds(5.0), kind: OrderKind::Stage }];
    s.release_armed = true;
    s.release_engines =
        (def.engines.iter().enumerate()).filter(|(_, e)| e.id.starts_with("s1.")).map(|(i, _)| i as u32).collect();
    s.throttle_was = 0.75;
    (s.accel, s.omega, s.alpha) = (DVec3::new(1.0, 2.0, 30.0), DVec3::new(0.01, 0.0, 0.02), DVec3::Y * 0.003);
    s.thrust_dir = Some(DVec3::new(0.01, 0.0, 1.0).normalize());
    s.fins = (0..def.surfaces.len()).map(|i| v(i, 0.01)).collect();
    s.seed = 42;
    (s.launch, s.stepped) = (Some(t0), Some(t0.add_seconds(60.0)));
    s.events = (0..3).map(|k| Event { at: t0.add_seconds(k as f64), text: format!("{k}"), alarm: k == 1 }).collect();
    s.pressure_regulated = true;
    let lost = |p: &super::ship::PartInfo| Lost { part: p.part, owner: p.id.clone(), why: "test".into() };
    s.lost = def.parts.iter().step_by(11).map(lost).collect();
    s.unpowered = def.parts.iter().skip(3).step_by(13).map(lost).collect();
    s.buses = (def.buses.iter().enumerate())
        .map(|(i, b)| BusRun {
            name: b.name.clone(),
            volts: v(i, 28.0),
            load: v(i, 100.0),
            solar: v(i, 1.0),
            ground: true,
        })
        .collect();
    let injury = Injury { at: t0, axis: "+Ax".into(), level: 60.0, seconds: 2.0, limit: 50.0 };
    s.crew = CrewRun { held: vec![vec![(1.0, 20.0)], vec![], vec![(0.5, 45.0)]], injuries: vec![injury] };
    s
}

/// Every field of `s` differs from the state at load (a field added
/// without a value here, and so without a tested rule, fails to compile
/// or fails here).
fn assert_every_field_set(s: &ShipState, at_load: &ShipState) {
    let ShipState {
        tanks,
        bottles,
        pressure_regulated,
        engines,
        release_armed,
        release_engines,
        throttle_was,
        thrust_dir,
        thrusters,
        fins,
        valves,
        batteries,
        mechanisms,
        joints,
        severed,
        parts,
        power,
        unpowered,
        buses,
        cabin,
        crew,
        fates,
        lost,
        seed,
        launch,
        clamped,
        sequence,
        stages,
        functions,
        orders,
        accel,
        omega,
        alpha,
        stepped,
        events,
    } = s;
    let l = at_load;
    let ship = [
        (tanks != &l.tanks, "tanks"),
        (bottles != &l.bottles, "bottles"),
        (engines != &l.engines, "engines"),
        (thrusters != &l.thrusters, "thrusters"),
        (valves != &l.valves, "valves"),
        (batteries != &l.batteries, "batteries"),
        (mechanisms != &l.mechanisms, "mechanisms"),
        (joints != &l.joints, "joints"),
        (severed != &l.severed, "severed"),
        (parts != &l.parts, "parts"),
        (power != &l.power, "power"),
        (cabin != &l.cabin, "cabin"),
        (fates != &l.fates, "fates"),
        (clamped != &false, "clamped"),
        (sequence != &l.sequence, "sequence"),
        (stages != &l.stages, "stages"),
        (functions != &l.functions, "functions"),
    ];
    let r = l;
    let run = [
        (orders != &r.orders, "orders"),
        (release_armed != &r.release_armed, "release_armed"),
        (release_engines != &r.release_engines, "release_engines"),
        (throttle_was != &r.throttle_was, "throttle_was"),
        (accel != &r.accel && omega != &r.omega && alpha != &r.alpha, "motion"),
        (thrust_dir != &r.thrust_dir, "thrust_dir"),
        (fins != &r.fins, "fins"),
        (seed != &r.seed, "seed"),
        (launch != &r.launch && stepped != &r.stepped, "launch, stepped"),
        (events != &r.events, "events"),
        (pressure_regulated != &r.pressure_regulated, "pressure_regulated"),
        (lost != &r.lost && unpowered != &r.unpowered, "lost, unpowered"),
        (buses != &r.buses, "buses"),
        (crew != &r.crew, "crew"),
    ];
    for (set, name) in ship.into_iter().chain(run) {
        assert!(set, "{name} is as loaded: give it a value here and a rule in craft::partition");
    }
}

/// The two splits: the booster (with the pad's mount) from the upper
/// stack, and Dragon from the rest. The vessel's piece first.
fn splits(def: &ShipDef) -> Vec<(&'static str, Vec<u32>, Vec<u32>)> {
    let ids = |f: &dyn Fn(&str) -> bool| def.parts.iter().filter(|p| f(&p.id)).map(|p| p.part).collect::<Vec<_>>();
    let upper = |id: &str| id.starts_with("s2.") || id.starts_with("dragon.");
    let dragon = |id: &str| id.starts_with("dragon.");
    vec![
        ("stage separation", ids(&upper), ids(&|id| !upper(id))),
        ("Dragon's separation", ids(&dragon), ids(&|id| !dragon(id))),
    ]
}

#[test]
fn a_split_puts_each_item_in_exactly_one_piece() {
    let def = falcon9();
    let s = every_field_set(&def);
    assert_every_field_set(&s, &ShipState::initial(&def));
    for (name, a, b) in splits(&def) {
        let (da, db) = (def.subset(&a), def.subset(&b));
        let (pa, pb) = (s.partition(&a, &def, &da, true), s.partition(&b, &def, &db, false));
        let n = |x: &ShipState| {
            [
                x.tanks.len(),
                x.bottles.len(),
                x.engines.len(),
                x.thrusters.len(),
                x.valves.len(),
                x.batteries.len(),
                x.mechanisms.len(),
                x.joints.len() + x.severed.len(),
                x.parts.len(),
                x.power.len(),
                x.cabin.iter().count(),
                x.fates.len(),
                x.fins.len(),
                x.lost.len(),
                x.unpowered.len(),
                x.orders.len(),
                x.release_engines.len(),
            ]
        };
        let (na, nb, ns) = (n(&pa), n(&pb), n(&s));
        for k in 0..ns.len() {
            assert_eq!(na[k] + nb[k], ns[k], "{name}: list {k} of the pieces does not add up");
        }
        // Each piece's lists align with its own definition.
        for (p, d) in [(&pa, &da), (&pb, &db)] {
            assert!(p.fits(d), "{name}");
            assert_eq!(
                (p.tanks.len(), p.engines.len(), p.power.len()),
                (d.tanks.len(), d.engines.len(), d.components.len())
            );
            assert_eq!((p.joints.len(), p.fins.len()), (d.joints.len(), d.surfaces.len()), "{name}");
        }
        // The cabin and the crew's exposure go with Dragon; the clamps and
        // the armed release with the pad's piece; the orders and the
        // pilot's last throttle with the vessel's.
        assert!(pa.cabin.is_some() && pa.crew == s.crew && pb.crew == CrewRun::default());
        assert!(!pa.clamped && !pa.release_armed && pb.clamped && pb.release_armed, "{name}");
        assert_eq!((pa.throttle_was, pb.throttle_was), (0.75, 0.0));
        assert!(pb.orders.is_empty());
        // The program is the vessel's as a whole: a copy in each piece.
        for p in [&pa, &pb] {
            assert!((&p.stages, &p.functions, p.sequence) == (&s.stages, &s.functions, s.sequence), "{name}");
        }
        // The cut joints wait with their child's piece.
        let cut = (def.joints.iter()).filter(|j| a.contains(&j.parent) != a.contains(&j.child)).count();
        assert_eq!(pa.severed.len() + pb.severed.len(), 1 + cut, "{name}");
    }
}

#[test]
fn merging_the_pieces_of_a_split_gives_the_state_back_bit_for_bit() {
    let def = falcon9();
    let s = every_field_set(&def);
    for (name, a, b) in splits(&def) {
        let (da, db) = (def.subset(&a), def.subset(&b));
        let (pa, pb) = (s.partition(&a, &def, &da, true), s.partition(&b, &def, &db, false));
        // Every field, the program too (each piece keeps it whole and
        // reads it through its view, `craft::program`).
        let back = ShipState::merge(&pa, &da, &pb, &db, &def).unwrap();
        assert!(back == s, "{name}: the merge differs from the state split");
        // Bit for bit (PartialEq on f64 would pass -0.0 for 0.0).
        assert_eq!(ron::to_string(&back).unwrap(), ron::to_string(&s).unwrap(), "{name}");
    }
}

#[test]
fn a_merge_refuses_two_cabins_and_starts_a_new_joint_unloaded() {
    let def = falcon9();
    let s = ShipState::initial(&def);
    let dragon = splits(&def).swap_remove(1).1;
    let dd = def.subset(&dragon);
    let pd = s.partition(&dragon, &def, &dd, true);
    assert!(ShipState::merge(&pd, &dd, &pd, &dd, &def).is_err(), "two cabins");
    // A whole whose joint neither piece has (a docking joint): intact, unloaded.
    let (_, x, y) = splits(&def).swap_remove(0);
    let (dx, dy) = (def.subset(&x), def.subset(&y));
    let (mut px, py) = (s.partition(&x, &def, &dx, true), s.partition(&y, &def, &dy, false));
    px.severed.clear();
    let m = ShipState::merge(&px, &dx, &py, &dy, &def).unwrap();
    assert!(m.joints.iter().all(|j| j.intact));
    assert!(m.joints.iter().all(|j| j.force == DVec3::ZERO && j.ratio == 0.0));
}

#[test]
fn orders_come_due_in_epoch_order_ties_as_given() {
    let t0 = Epoch::new(1_000_000_000, 0.0);
    let mut r = ShipState::default();
    let stage = |n: &str| OrderKind::Function(n.into());
    r.order(t0.add_seconds(2.0), stage("c"));
    r.order(t0.add_seconds(1.0), stage("a"));
    r.order(t0.add_seconds(2.0), stage("d"));
    r.order(t0.add_seconds(1.0), stage("b"));
    assert_eq!(r.next_order(), Some(t0.add_seconds(1.0)));
    assert!(r.due(t0.add_seconds(0.5)).is_empty());
    let names = |o: Vec<Order>| {
        o.into_iter()
            .map(|o| match o.kind {
                OrderKind::Function(n) => n,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(names(r.due(t0.add_seconds(1.0))), ["a", "b"]);
    assert_eq!(names(r.due(t0.add_seconds(9.0))), ["c", "d"]);
    assert_eq!(r.next_order(), None);
}

/// Each piece of a split keeps its own aero surfaces' deflections,
/// aligned with its own definition's surfaces (review, 2026-10-27: they
/// were copied whole, so a piece's fin read another's); never deflected,
/// none.
#[test]
fn a_split_partitions_the_fin_deflections() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/craft/falcon9-crew");
    let def = crate::craft::load_definition(&path).unwrap().ship.unwrap();
    let mut whole = ShipState::default();
    assert!(whole.partition(&[], &def, &def.subset(&[]), true).fins.is_empty(), "never deflected");
    whole.fins = (0..def.surfaces.len()).map(|i| 0.01 * (i + 1) as f64).collect();
    let booster: Vec<u32> = (def.surfaces.iter()).filter(|s| s.id.starts_with("s1.")).map(|s| s.part).collect();
    let rest: Vec<u32> = (def.surfaces.iter()).filter(|s| !s.id.starts_with("s1.")).map(|s| s.part).collect();
    assert!(!booster.is_empty() && !rest.is_empty(), "the grid fins and Dragon's");
    for kept in [booster, rest] {
        let piece = def.subset(&kept);
        let fins = whole.partition(&kept, &def, &piece, true).fins;
        assert_eq!(fins.len(), piece.surfaces.len());
        for (s, d) in piece.surfaces.iter().zip(&fins) {
            let i = def.surfaces.iter().position(|w| w.id == s.id).unwrap();
            assert_eq!(*d, whole.fins[i], "{}", s.id);
        }
    }
}

#[test]
fn events_keep_the_newest() {
    let t0 = Epoch::new(1_000_000_000, 0.0);
    let mut r = ShipState::default();
    for k in 0..100 {
        r.event(t0.add_seconds(k as f64), format!("{k}"), false);
    }
    assert_eq!(r.events.len(), EVENTS_KEPT);
    assert_eq!(r.events.last().unwrap().text, "99");
    assert_eq!(r.events[0].text, format!("{}", 100 - EVENTS_KEPT));
}
