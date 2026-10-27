//! The Staging menu (key G, right dock): the stage stack from
//! [`crate::staging`], stage 0 at the top, each stage's parts and their
//! state, and the button for the next stage (Space). A skeleton until the
//! parts model brings real stages (D060).

use super::data::FlightData;
use super::{MenuAction, Section, Tone};
use crate::staging::{PartTone, StageState};

pub fn sections(d: &FlightData) -> Vec<Section> {
    let mut s = Section::summary();
    for stage in &d.stages {
        let (state, tone) = match stage.state {
            StageState::Next => (" · NEXT", Tone::Accent),
            StageState::Active => (" · ACTIVE", Tone::Dim),
            StageState::Later => ("", Tone::Normal),
        };
        let name = stage.name.as_ref().map(|n| format!(" · {}", n.to_uppercase())).unwrap_or_default();
        s.heading(format!("STAGE {}{name}{state}", stage.number), tone);
        for p in &stage.parts {
            s.toned(format!("  {}", p.name), &p.state, part_tone(p.tone));
        }
    }
    let next = crate::staging::next(&d.stages);
    // A craft of parts launches by its sequence: only the test craft's
    // staging is locked on the ground.
    let parts = d.stages.iter().any(|s| s.name.is_some());
    if next.is_some() && d.landed && !parts {
        s.note("on the ground: staging locked (P still deploys the parachutes)", Tone::Dim);
    } else if let Some(n) = next {
        s.button(format!("Activate stage {n}  (Space)"), MenuAction::NextStage);
    } else if !d.stages.is_empty() {
        s.note("every stage is active", Tone::Dim);
    }
    vec![s]
}

/// How a part's state reads in a menu.
pub(super) fn part_tone(t: PartTone) -> Tone {
    match t {
        PartTone::Idle => Tone::Dim,
        PartTone::Working => Tone::Good,
        PartTone::Failed => Tone::Alert,
    }
}

#[cfg(test)]
mod tests {
    use super::super::Item;
    use super::*;
    use crate::staging::{stack, StageInputs};
    use sim::chute::{CanopyState, ChuteState};
    use sim::vessel::Controls;

    fn data(controls: &Controls, chute: &ChuteState) -> FlightData {
        let spec = sim::craft::test_craft().params().chute;
        let inputs = StageInputs { controls, chute, spec: &spec, engine_running: true, fuelled: true };
        FlightData { stages: stack(&inputs), ..FlightData::default() }
    }

    #[test]
    fn the_stack_lists_stage_zero_first_with_its_parts_then_the_button() {
        let c = Controls { throttle: 1.0, ..Controls::default() };
        let s = sections(&data(&c, &ChuteState::default()));
        let items = &s[0].items;
        assert_eq!(items[0], Item::Heading("STAGE 0 · NEXT".into(), Tone::Accent));
        assert_eq!(s[0].rows(), [("  DROGUES", "stowed"), ("  MAINS", "stowed"), ("  ENGINE", "running · 100 %")]);
        assert!(items.contains(&Item::Heading("STAGE 1 · ACTIVE".into(), Tone::Dim)));
        assert_eq!(
            items.last(),
            Some(&Item::Button { label: "Activate stage 0  (Space)".into(), action: MenuAction::NextStage })
        );
    }

    #[test]
    fn on_the_ground_the_button_gives_way_to_a_note() {
        let mut d = data(&Controls::default(), &ChuteState::default());
        d.landed = true;
        let s = sections(&d);
        assert!(!s[0].items.iter().any(|i| matches!(i, Item::Button { .. })));
        assert!(matches!(s[0].items.last(), Some(Item::Note(n, Tone::Dim)) if n.contains("staging locked")));
    }

    #[test]
    fn once_armed_there_is_no_next_stage_and_failures_alert() {
        let c = Controls { chute: true, ..Controls::default() };
        let torn = ChuteState { armed: true, drogue: CanopyState::Released, main: CanopyState::Failed };
        let s = sections(&data(&c, &torn));
        assert!(s[0].items.contains(&Item::Heading("STAGE 0 · ACTIVE".into(), Tone::Dim)));
        assert_eq!(s[0].tone("  MAINS"), Some(Tone::Alert));
        assert!(!s[0].items.iter().any(|i| matches!(i, Item::Button { .. })));
        assert_eq!(s[0].items.last(), Some(&Item::Note("every stage is active".into(), Tone::Dim)));
    }
}
