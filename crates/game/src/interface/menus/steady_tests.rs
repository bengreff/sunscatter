//! Nothing in the menus flickers on the pad (the owner, 2026-10-28: Comms'
//! VIA and S2.BATTERY#1 came and went): the Falcon 9 stands for a few
//! hundred frames, the snapshot refreshed as the game does, and every
//! menu's sections and lines stay the same.

use super::data::{gather, steady, Sources};
use super::{Item, MenuId, Steady};
use crate::comms::Comms;
use crate::state::{craft_exists, craft_or_fallback, SimState};
use std::collections::HashMap;

/// Every section's id and its lines' labels (a note as "note").
fn shape(sections: &[super::Section]) -> Vec<(String, Vec<String>)> {
    let label = |i: &Item| match i {
        Item::Row(r) => r.label.clone(),
        Item::Toggle { label, .. } | Item::Choice { label, .. } | Item::Button { label, .. } => label.clone(),
        Item::Note(..) => "note".into(),
    };
    sections.iter().map(|s| (s.id.to_string(), s.items.iter().map(label).collect())).collect()
}

#[test]
fn on_the_pad_no_menu_line_comes_and_goes() {
    if !craft_exists("falcon9-crew") {
        return;
    }
    bevy::tasks::ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
    let mut sim = SimState::with_craft(&craft_or_fallback("falcon9-crew"));
    let mut comms = Comms::load();
    let (pred, aps, rv, tracked) = Default::default();
    let mut data = super::FlightData::default();
    let mut held: HashMap<MenuId, Steady> = HashMap::new();
    let mut first: Option<Vec<_>> = None;
    // 300 frames at 60 Hz; the snapshot every 0.1 s, as the game does.
    for frame in 0..300u32 {
        let t = sim.clock.add_seconds(1.0 / 60.0);
        sim.step(t, |_| Some(0.0), None);
        crate::comms::refresh(&sim, false, &mut comms);
        if frame % 6 != 0 {
            continue;
        }
        let now = f64::from(frame) / 60.0;
        let sources = Sources {
            sim: &sim,
            pred: &pred,
            nav: None,
            target: None,
            aps: &aps,
            landing: None,
            rv: &rv,
            comms: &comms,
            tracked: &tracked,
            fps: "60".into(),
        };
        let mut d = gather(&sources);
        steady(&mut d, &data, &sim);
        data = d;
        // The snapshot itself (the causes), then what is drawn.
        let raw: Vec<_> = MenuId::ALL.iter().map(|m| (*m, shape(&m.sections(&data)))).collect();
        let drawn: Vec<_> = (MenuId::ALL.iter())
            .map(|m| (*m, shape(held.entry(*m).or_default().update(m.sections(&data), now))))
            .collect();
        match &first {
            None => first = Some(raw.clone()),
            Some(f) => {
                assert_eq!(&raw, f, "frame {frame}: the snapshot's lines changed");
                assert_eq!(&drawn, f, "frame {frame}: the drawn lines changed");
            }
        }
    }
    let f = first.expect("refreshed");
    let comms_page = &f.iter().find(|m| m.0 == MenuId::Comms).expect("comms").1;
    assert!(!comms_page.is_empty(), "the link home is shown");
}
