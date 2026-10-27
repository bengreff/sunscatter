//! The Falcon 9 Block 5 with Crew Dragon (`data/craft/falcon9-crew`) built
//! from the catalog (`data/parts`): it builds without errors, its totals are
//! those of the real vehicle within the stated tolerances (provisional data:
//! docs/features/falcon9.md replaces them), its unmodelled masses are
//! reported with their reasons, and it builds to the same bits twice.

use super::{default_craft_dir, load_assembly};

/// The craft's report, printed for the record (`--nocapture`).
#[test]
fn the_falcon_9_builds_with_its_totals() {
    let a = load_assembly(&default_craft_dir().join("falcon9-crew")).unwrap_or_else(|e| panic!("{e}"));
    let r = &a.report;
    let ship = a.def.ship.clone().unwrap();
    let height = r.z_range.1 - r.z_range.0;
    println!("height {height:.2} m, radius {:.2} m, dry {:.0} kg, loaded {:.0} kg", r.radius, r.dry_mass, r.loaded);
    for u in &r.unmodelled {
        println!("unmodelled {}: {:+.0} kg ({})", u.0, u.1, u.2);
    }
    for c in &r.contradictions {
        println!("contradiction: {c}");
    }
    let stage = |prefix: &str| -> (f64, f64) {
        let parts: Vec<u32> = ship.parts.iter().filter(|p| p.id.starts_with(prefix)).map(|p| p.part).collect();
        let dry: f64 = a.def.parts.iter().filter(|p| parts.contains(&p.id)).map(|p| p.dry_mass).sum();
        let wet: f64 = ship.tanks.iter().filter(|t| parts.contains(&t.part)).map(|t| t.capacity * t.load.fill).sum();
        let gas: f64 = ship.bottles.iter().filter(|b| parts.contains(&b.part)).map(|b| ship.bottle_gas(b)).sum();
        (dry, wet + gas)
    };
    let (s1, s2, dragon, pad) = (stage("s1."), stage("s2."), stage("dragon."), stage("pad."));
    println!("first stage dry {:.0} kg, propellant and gas {:.0} kg", s1.0, s1.1);
    println!("second stage dry {:.0} kg, propellant and gas {:.0} kg", s2.0, s2.1);
    println!("dragon dry (crew and cargo) {:.0} kg, propellant and gas {:.0} kg", dragon.0, dragon.1);
    let liftoff = r.dry_mass + r.loaded - pad.0;
    println!("liftoff {liftoff:.0} kg (the pad's mount {:.0} kg apart)", pad.0);
    // falcon9.md §1.1: 65.4 m from the centre nozzle to the nosecone, 3.66 m
    // stages, the capsule 3.96 m; the pad's posts 3 m below the octaweb.
    assert!((r.z_range.1 - r.z_range.0 - 67.4).abs() < 0.05, "{}", r.z_range.1 - r.z_range.0);
    assert!((r.z_range.1 - 4.05).abs() < 0.01, "the nosecone tip at falcon9.md 65.40: {}", r.z_range.1);
    assert!((r.radius - 2.3).abs() < 0.1, "{}", r.radius);
    // §1.7: dry masses by stage (the checks carry the gaps); loads.
    assert!((s1.0 - 25_600.0).abs() < 1.0 && (s2.0 - 4_000.0).abs() < 1.0, "{s1:?} {s2:?}");
    assert!((s1.1 / (287_400.0 + 123_500.0 + 300.0) - 1.0).abs() < 0.01, "{}", s1.1);
    assert!((s2.1 / (76_200.0 + 29_500.0 + 100.0) - 1.0).abs() < 0.01, "{}", s2.1);
    assert!((dragon.0 + dragon.1 - 13_563.0).abs() < 100.0, "{dragon:?}");
    // Liftoff, Crew configuration ≈ 560,200 kg (D).
    assert!((liftoff / 560_200.0 - 1.0).abs() < 0.01, "{liftoff}");
    // Every gap is reported with a reason.
    assert!(r.unmodelled.iter().any(|u| u.0 == "s1.tank" && u.2.contains("ring frames")));
    assert!(r.unmodelled.iter().all(|u| !u.2.is_empty()));
    // Engines, feeds, thrusters.
    assert_eq!(ship.engines.iter().filter(|e| e.main).count(), 10);
    assert_eq!(ship.engines.len(), 18, "nine Merlins, the MVac, eight SuperDracos");
    assert!(ship.engines.iter().all(|e| !e.feeds[0].is_empty() && !e.feeds[1].is_empty()));
    assert_eq!(ship.thrusters.iter().map(|t| t.nozzles.len()).sum::<usize>(), 8 + 8 + 16, "nitrogen pods, Dracos");
    assert_eq!(ship.sequence.len(), 7);
    assert_eq!(a.def.parts[0].crew, 4);
}

/// The legs deployed (the booster's landing configuration): four feet,
/// 30° below horizontal, below the engines and ~18 m across (P).
#[test]
fn the_falcon_9_legs_deploy_to_their_feet() {
    use crate::craft::{CellOptions, ContactKind, Craft, CraftId, Resolution};
    let a = load_assembly(&default_craft_dir().join("falcon9-crew")).unwrap();
    let ship = a.def.ship.clone().unwrap();
    let legs: Vec<u32> = ship.parts.iter().filter(|p| p.id.starts_with("s1.leg")).map(|p| p.part).collect();
    let def = a.def.deploy(&legs).unwrap();
    let opts = CellOptions::for_parts(def.parts.len());
    let craft = Craft::from_def(CraftId("f9-legs".into()), def, &Resolution::default(), &opts);
    let feet: Vec<glam::DVec3> =
        craft.cells.contacts.iter().filter(|c| c.kind == ContactKind::Foot).map(|c| c.pos).collect();
    assert_eq!(feet.len(), 4);
    let centre_exit = -61.35 - 0.25;
    for f in &feet {
        let span = 2.0 * f.truncate().length();
        println!("foot at {f:.2}: span {span:.1} m, {:.2} m below the centre engine's exit", centre_exit - f.z);
        assert!(f.z < centre_exit - 1.0, "{f}");
        assert!((span - 18.0).abs() < 1.0, "{span}");
    }
}

/// Built twice from its files, the same bits: the definition, the craft
/// (surface, cells, mass model) and its design.
#[test]
fn the_falcon_9_builds_to_the_same_bits() {
    use crate::craft::{CellOptions, Craft, CraftId, Resolution};
    let dir = default_craft_dir().join("falcon9-crew");
    let (a, b) = (load_assembly(&dir).unwrap(), load_assembly(&dir).unwrap());
    assert_eq!(a.def, b.def);
    assert_eq!(a.report, b.report);
    let opts = CellOptions::for_parts(a.def.parts.len());
    let id = CraftId("falcon9-crew".into());
    let (ca, cb) = (
        Craft::from_def(id.clone(), a.def, &Resolution::default(), &opts),
        Craft::from_def(id.clone(), b.def, &Resolution::default(), &opts),
    );
    assert_eq!(ca, cb);
    assert_eq!(**ca.design(), **cb.design());
    assert_eq!(*crate::craft::craft_of(&id), ca, "the registry builds the same craft");
}
