//! The centre of mass aboard on the Falcon 9 (`estimate::centre`): the
//! pad's mount left out, exact in debug mode, and to the precision the
//! tank sensors give.

use super::*;
use crate::systems::data;

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs())
}

/// The centre of mass aboard (interface-map finding 41's last row): the
/// pad's mount left out (its 50 t and its foot), exact in debug mode, and
/// to the precision the sensors give, which bounds where Dragon's
/// unmeasured tanks could put it.
#[test]
fn the_centre_of_mass_leaves_the_mount_out() {
    let craft = crate::craft::craft_of(&crate::craft::CraftId("falcon9-crew".into()));
    let def = craft.ship.as_deref().unwrap();
    let state = ShipState::initial(def);
    let (params, design) = (craft.params(), craft.design());
    let at = |s: &ShipState, k: &Knowledge| centre(def, s, design, &params.contacts, k);
    let exact = at(&state, &Knowledge::perfect());
    assert_eq!(exact.precision, 0.0);
    // By hand: the loaded craft's mass properties less the mount's part,
    // over the lowest contact point not on it.
    let mount: Vec<u32> = def.parts.iter().filter(|p| p.ground).map(|p| p.part).collect();
    let (mm, mmz) = (design.parts.iter().zip(&design.part_mass))
        .filter(|(p, _)| mount.contains(&p.id))
        .fold((0.0, 0.0), |(m, mz), (_, p)| (m + p.mass, mz + p.mass * p.com.z));
    assert!(mm > 1e4, "the mount weighs {mm} kg");
    let all = craft.loaded_props();
    let z = (all.mass * all.com.z - mmz) / (all.mass - mm);
    let on = |c: &&crate::craft::cells::ContactPoint| !mount.contains(&design.cell_part[c.cell as usize]);
    let base = params.contacts.iter().filter(on).map(|c| c.pos.z).fold(f64::INFINITY, f64::min);
    assert!(close(exact.value, z - base, 1e-9), "{exact:?} vs {}", z - base);
    assert!(params.bottom_z < base - 1.0, "the mount's foot {} under the craft's {base}", params.bottom_z);
    // As the sensors give it: Dragon's tanks unmeasured widen it, and
    // every way they could be (each empty or full) lies within it.
    let k = data::knowledge(def, &state, false);
    let known = at(&state, &k);
    assert_eq!(known.value, exact.value);
    assert!(known.precision > 0.0 && known.precision < 3.0, "{known:?}");
    let blind: Vec<usize> = (0..def.tanks.len()).filter(|&t| !k.knows(Quantity::TankLevel(t as u32))).collect();
    assert!(!blind.is_empty());
    for mask in 0..1u32 << blind.len() {
        let mut s = state.clone();
        for (n, &t) in blind.iter().enumerate() {
            s.tanks[t].liquid = if mask >> n & 1 == 1 { def.tanks[t].capacity } else { 0.0 };
        }
        let v = at(&s, &Knowledge::perfect()).value;
        assert!((v - exact.value).abs() <= known.precision + 1e-9, "{v} vs {known:?}");
    }
}
