//! A save whose state does not fit its craft's current data is refused at
//! load as a format error (REVIEW-2026-10-31's #1): one row per misfit, on
//! a Falcon 9 Crew on its pad saved and edited as a data change would leave
//! it.

use super::*;
use crate::craft::handle::Owner;
use crate::craft::state::Lost;
use crate::craft::{craft_of, CraftId};
use crate::ephem::Ephemeris;
use crate::save::{SaveError, SaveGame};
use crate::sol;
use std::sync::Arc;

fn world() -> World {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(sol::EPHEMERIS_PATH);
    World::sol(Arc::new(Ephemeris::from_bytes(&std::fs::read(path).unwrap()).unwrap()))
}

fn load(w: &World, t: Epoch, v: &Vessel) -> Result<SaveGame, SaveError> {
    let mut ids = VesselIds::default();
    ids.allocate();
    SaveGame::from_ron(&SaveGame::capture(w, t, std::slice::from_ref(v), ids, 0).to_ron())
}

type Edit = fn(&mut Vessel);

fn ship(v: &mut Vessel) -> &mut crate::craft::state::ShipState {
    v.ship.as_mut().unwrap()
}

#[test]
fn every_misfit_of_a_saved_state_is_refused_at_load() {
    let w = world();
    let t = sol::sol_epoch().add_seconds(3.0 * 86_400.0);
    let craft = craft_of(&CraftId("falcon9-crew".into()));
    let pad = Vessel::landed_at(&w, VesselId(1), "Earth", 28.6084, -80.6043, t, &craft);
    assert!(load(&w, t, &pad).is_ok(), "the unedited save loads");
    let rows: [(&str, Edit, &str); 17] = [
        ("a skin cell fewer (a bake change)", |v| _ = v.thermal.state.skin.pop(), "skin cells"),
        ("a thermal node more", |v| v.thermal.state.nodes.push(290.0), "thermal nodes"),
        ("a tank on another part", |v| ship(v).tanks[0].part = 9_999, "tanks 0 on part 9999"),
        ("a bottle fewer", |v| _ = ship(v).bottles.pop(), "bottles"),
        ("the engines reordered", |v| ship(v).engines.reverse(), "engines 0 on part"),
        ("a thruster on another part", |v| ship(v).thrusters[0].part = 9_999, "thrusters 0"),
        ("a valve on another part", |v| ship(v).valves[0].part = 9_999, "valves 0"),
        ("a battery on another part", |v| ship(v).batteries[0].part = 9_999, "batteries 0"),
        ("a mechanism on another part", |v| ship(v).mechanisms[0].part = 9_999, "mechanisms 0"),
        ("the parts reordered", |v| ship(v).parts.swap(0, 1), "parts 0"),
        ("a power switch on another part", |v| ship(v).power[0].part = 9_999, "power switches 0"),
        ("a joint between other parts", |v| ship(v).joints[0].parent = 9_999, "joint 0"),
        ("a fin deflection without its surfaces", |v| ship(v).fins = vec![0.0; 99], "fin deflections"),
        ("the cabin on a part without a seat", |v| ship(v).cabin.as_mut().unwrap().part = 9_999, "the cabin"),
        ("a fate's owner gone", |v| ship(v).fates[0].owner = Owner::Part(9_999), "failure fate"),
        ("a fate's mode beyond its owner's", |v| ship(v).fates[0].mode = 9_999, "failure fate"),
        (
            "a lost function's owner gone",
            |v| ship(v).lost.push(Lost { part: 9_999, owner: Owner::Component(9_999), why: "x".into() }),
            "lost or unpowered",
        ),
    ];
    for (what, edit, says) in rows {
        let mut v = pad.clone();
        edit(&mut v);
        match load(&w, t, &v) {
            Err(SaveError::Format(e)) => assert!(e.contains("does not fit") && e.contains(says), "{what}: {e}"),
            other => panic!("{what}: {:?}", other.map(|_| ())),
        }
    }
}
