//! The Maneuver menu (key 3): the planned burns and the way into the burn
//! planner (D111: from the Orbital menu; N is translate-back now). A craft
//! of parts lists its nodes (D126) with the sim's prediction of each, and
//! what the maneuver program is doing while it flies one.

use super::data::{Burn, FlightData};
use super::{MenuAction, Section, Tone};
use crate::format::duration as fmt_duration;
use sim::systems::maneuver::Step;

pub fn sections(d: &FlightData) -> Vec<Section> {
    vec![summary(&d.burns)]
}

fn summary(burns: &[Burn]) -> Section {
    let mut s = Section::summary();
    if burns.is_empty() {
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
        s.row("  DURATION", fmt_duration(b.duration));
    }
    s.button("Open the burn planner", MenuAction::OpenPlanner);
    s
}

/// What the maneuver program is doing, in the menu's words (the ship's log
/// tells how a burn ended).
fn step_label(step: Step) -> &'static str {
    match step {
        Step::Wait => "waiting",
        Step::Turn => "turning to the burn",
        Step::Settle => "settling the propellant",
        Step::Burn => "burning",
        Step::TailOff => "cutting off",
        Step::Done => "on target",
        Step::Manual => "taken over",
        Step::Short => "ended short",
    }
}

#[cfg(test)]
mod tests {
    use super::super::Item;
    use super::*;

    /// The burns in order, ignition soon in amber, and the planner's button.
    #[test]
    fn the_burns_and_the_planner() {
        let burns = [
            Burn { dv: Some(12.34), in_s: 30.0, duration: 4.0, number: 1, step: None },
            Burn { dv: None, in_s: 600.0, duration: 20.0, number: 2, step: None },
        ];
        let s = summary(&burns);
        assert_eq!(s.value("BURN 1"), Some("Δv 12.3 m/s"));
        assert_eq!(s.rows().iter().filter(|(l, _)| *l == "  IGNITION IN").count(), 2);
        assert_eq!(s.tone("  IGNITION IN"), Some(Tone::Warn));
        assert_eq!(s.value("BURN 2"), Some("fixed time"));
        assert!(s
            .items
            .contains(&Item::Button { label: "Open the burn planner".into(), action: MenuAction::OpenPlanner }));
        assert!(summary(&[]).items.contains(&Item::Note("no burns planned".into(), Tone::Dim)));
    }

    /// A node being flown: numbered as the log counts, what the program is
    /// doing, and no ignition countdown once it has ignited.
    #[test]
    fn a_node_being_flown() {
        let burns = [
            Burn { dv: Some(100.0), in_s: -3.0, duration: 12.0, number: 2, step: Some(Step::Burn) },
            Burn { dv: Some(5.0), in_s: 3_000.0, duration: 0.6, number: 3, step: None },
        ];
        let s = summary(&burns);
        assert_eq!(s.value("BURN 2"), Some("Δv 100.0 m/s"));
        assert_eq!(s.value("  NOW"), Some("burning"));
        assert_eq!(s.tone("  NOW"), Some(Tone::Good));
        assert_eq!(s.rows().iter().filter(|(l, _)| *l == "  IGNITION IN").count(), 1);
        assert_eq!(s.value("BURN 3"), Some("Δv 5.0 m/s"));
        let turning = [Burn { step: Some(Step::Turn), in_s: 40.0, number: 1, ..Burn::default() }];
        assert_eq!(summary(&turning).value("  NOW"), Some("turning to the burn"));
    }
}
