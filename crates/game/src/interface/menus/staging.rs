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
            StageState::Next => ("NEXT", Tone::Accent),
            StageState::Active => ("ACTIVE", Tone::Dim),
        };
        s.heading(format!("STAGE {} · {state}", stage.number), tone);
        for p in &stage.parts {
            s.toned(format!("  {}", p.name), &p.state, part_tone(p.tone));
        }
    }
    if let Some(n) = crate::staging::next(&d.stages) {
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
