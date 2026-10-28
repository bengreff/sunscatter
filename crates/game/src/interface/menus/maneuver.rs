//! The Maneuver menu (key 3): the planned burns and the way into the burn
//! planner (D111: from the Orbital menu; N is translate-back now).

use super::data::{Burn, FlightData};
use super::{MenuAction, Section, Tone};
use crate::format::duration as fmt_duration;

pub fn sections(d: &FlightData) -> Vec<Section> {
    vec![summary(&d.burns)]
}

fn summary(burns: &[Burn]) -> Section {
    let mut s = Section::summary();
    if burns.is_empty() {
        s.note("no burns planned", Tone::Dim);
    }
    for (k, b) in burns.iter().enumerate() {
        let dv = b.dv.map_or("fixed time".to_string(), |dv| format!("Δv {dv:.1} m/s"));
        s.row(format!("BURN {}", k + 1), dv);
        s.toned("  IGNITION IN", fmt_duration(b.in_s), if b.in_s < 60.0 { Tone::Warn } else { Tone::Normal });
        s.row("  DURATION", fmt_duration(b.duration));
    }
    s.button("Open the burn planner", MenuAction::OpenPlanner);
    s
}

#[cfg(test)]
mod tests {
    use super::super::Item;
    use super::*;

    /// The burns in order, ignition soon in amber, and the planner's button.
    #[test]
    fn the_burns_and_the_planner() {
        let burns =
            [Burn { dv: Some(12.34), in_s: 30.0, duration: 4.0 }, Burn { dv: None, in_s: 600.0, duration: 20.0 }];
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
}
