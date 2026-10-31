//! The Maneuver menu (key 3): the planned burns and the way into the burn
//! planner (D111: from the Orbital menu; N is translate-back now). A craft
//! of parts lists its nodes (D126) with the sim's prediction of each, and
//! what the maneuver program is doing while it flies one; then the
//! program's switch (D126: by itself, on a press, off), what it is doing
//! and its press (`systems_screen::programs`, the PROGRAMS tab's rows).

use super::data::{Burn, FlightData};
use super::{MenuAction, Section, Tone};
use crate::format::duration as fmt_duration;
use crate::systems_screen::programs::step_label;

pub fn sections(d: &FlightData) -> Vec<Section> {
    let program = d.programs.iter().find(|r| r.name == "maneuver");
    // With the program's row, its "no burn planned" says it once.
    let mut s = summary(&d.burns, program.is_none());
    if let Some(r) = program {
        s.program("PROGRAM", r);
    }
    vec![s]
}

/// The burns, and "no burns planned" when there are none and `say_none`.
fn summary(burns: &[Burn], say_none: bool) -> Section {
    let mut s = Section::summary();
    if burns.is_empty() && say_none {
        s.note("no burns planned", Tone::Dim);
    }
    for b in burns {
        let dv = b.dv.map_or("fixed time".to_string(), |dv| format!("Δv {dv:.1} m/s"));
        s.row(format!("BURN {}", b.number), dv);
        if let Some(step) = b.step {
            s.toned("  NOW", step_label(step), Tone::Good);
        }
        if b.in_s >= 0.0 {
            s.toned("  IGNITION IN", fmt_duration(b.in_s), if b.in_s < 60.0 { Tone::Warn } else { Tone::Normal });
        }
        s.row("  DURATION", b.duration.map_or_else(|| "—".to_string(), fmt_duration));
        for w in &b.warnings {
            s.note(w, Tone::Warn);
        }
    }
    s.button("Open the burn planner", MenuAction::OpenPlanner);
    s
}

#[cfg(test)]
mod tests {
    use super::super::Item;
    use super::*;
    use crate::systems_screen::programs::Row;
    use sim::systems::maneuver::Step;
    use sim::systems::ship_os::Switch;

    /// The burns in order, ignition soon in amber, and the planner's button.
    #[test]
    fn the_burns_and_the_planner() {
        let burns = [
            Burn { dv: Some(12.34), in_s: 30.0, duration: Some(4.0), number: 1, ..Burn::default() },
            Burn { dv: None, in_s: 600.0, duration: Some(20.0), number: 2, ..Burn::default() },
        ];
        let s = summary(&burns, true);
        assert_eq!(s.value("BURN 1"), Some("Δv 12.3 m/s"));
        assert_eq!(s.rows().iter().filter(|(l, _)| *l == "  IGNITION IN").count(), 2);
        assert_eq!(s.tone("  IGNITION IN"), Some(Tone::Warn));
        assert_eq!(s.value("BURN 2"), Some("fixed time"));
        assert!(s
            .items
            .contains(&Item::Button { label: "Open the burn planner".into(), action: MenuAction::OpenPlanner }));
        assert!(summary(&[], true).items.contains(&Item::Note("no burns planned".into(), Tone::Dim)));
        // A node the craft cannot fly: its duration unknown, the planner's
        // warning beside it (it read "DURATION 0s" with no warning).
        let warning = "no main engine to burn it: the node cannot be flown".to_string();
        let unflown =
            [Burn { dv: Some(5.0), in_s: 1_800.0, warnings: vec![warning.clone()], number: 1, ..Burn::default() }];
        let s = summary(&unflown, true);
        assert_eq!(s.value("  DURATION"), Some("—"));
        assert!(s.items.contains(&Item::Note(warning, Tone::Warn)));
    }

    /// A node being flown: numbered as the log counts, what the program is
    /// doing, and no ignition countdown once it has ignited.
    #[test]
    fn a_node_being_flown() {
        let burns = [
            Burn {
                dv: Some(100.0),
                in_s: -3.0,
                duration: Some(12.0),
                number: 2,
                step: Some(Step::Burn),
                ..Burn::default()
            },
            Burn { dv: Some(5.0), in_s: 3_000.0, duration: Some(0.6), number: 3, ..Burn::default() },
        ];
        let s = summary(&burns, true);
        assert_eq!(s.value("BURN 2"), Some("Δv 100.0 m/s"));
        assert_eq!(s.value("  NOW"), Some("burning"));
        assert_eq!(s.tone("  NOW"), Some(Tone::Good));
        assert_eq!(s.rows().iter().filter(|(l, _)| *l == "  IGNITION IN").count(), 1);
        assert_eq!(s.value("BURN 3"), Some("Δv 5.0 m/s"));
        let turning = [Burn { step: Some(Step::Turn), in_s: 40.0, number: 1, ..Burn::default() }];
        assert_eq!(summary(&turning, true).value("  NOW"), Some("turning to the burn"));
    }

    /// The maneuver program's switch under the burns: AUTO, ON PRESS, OFF
    /// with the one set, what it is doing, and its press while it does
    /// something.
    #[test]
    fn the_programs_switch_and_press() {
        let row = Row {
            name: "maneuver".into(),
            title: "Planned burns".into(),
            does: String::new(),
            switch: Switch::Press,
            now: "burn 1 wakes in 30s: waits for a press".into(),
            tone: Tone::Warn,
            press: Ok("fly burn 1".into()),
            settings: Vec::new(),
        };
        let d = FlightData { programs: vec![row.clone()], ..FlightData::default() };
        let s = &sections(&d)[0];
        let set = |sw| MenuAction::SetSwitch("maneuver".into(), sw);
        let options = vec![
            ("AUTO".to_string(), false, set(Switch::Auto)),
            ("ON PRESS".to_string(), true, set(Switch::Press)),
            ("OFF".to_string(), false, set(Switch::Off)),
        ];
        assert!(s.items.contains(&Item::Choice { label: "PROGRAM".into(), current: "ON PRESS".into(), options }));
        assert!(s.items.contains(&Item::Note(row.now.clone(), Tone::Warn)));
        let press = Item::Button { label: "Press: fly burn 1".into(), action: MenuAction::Press("maneuver".into()) };
        assert!(s.items.contains(&press));
        assert_eq!(s.items.iter().filter(|i| matches!(i, Item::Note(..))).count(), 1, "said once");
        // Nothing to press: no button.
        let d = FlightData { programs: vec![Row { press: Err("it runs by itself".into()), ..row }], ..d };
        assert!(!sections(&d)[0].items.iter().any(|i| matches!(i, Item::Button { action: MenuAction::Press(_), .. })));
    }
}
