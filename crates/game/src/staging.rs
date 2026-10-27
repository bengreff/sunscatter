//! Staging (D080): the craft's stage stack and what Space activates, as
//! pure functions. A *skeleton* until the parts model brings real stages
//! (D060): the test craft has one stage, its engine (lit by the throttle
//! since launch), and the parachute event above it (D079).
//!
//! Stages are numbered as KSP does: the highest number fires first, stage
//! 0 last, and the stack is listed with stage 0 at the top. A stage is
//! *next* until it is activated, then *active*. Staging is locked on the
//! ground (Space is the launch key in KSP habits; here the throttle
//! launches, and Space on the pad must not arm the parachutes). P is not
//! locked: nothing refuses the parachute command (D079).

pub mod parts;

use sim::chute::{CanopyState, ChuteSpec, ChuteState};
use sim::vessel::Controls;

/// Where a stage is in the sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageState {
    /// Space activates it.
    Next,
    /// Activated.
    Active,
    /// After the next one (a craft of parts' sequence).
    Later,
}

/// How a part's state reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartTone {
    Idle,
    Working,
    Failed,
}

/// One part of a stage and its state, for display.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub state: String,
    pub tone: PartTone,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stage {
    pub number: u32,
    pub state: StageState,
    /// Its name in a craft of parts' sequence.
    pub name: Option<String>,
    pub parts: Vec<Part>,
}

/// What activating a stage does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activation {
    /// The parachute command (the same as P, D079).
    ArmParachutes,
}

/// Why nothing was activated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Landed or wrecked: staging is locked on the ground.
    Grounded,
    NoStageLeft,
}

impl Refusal {
    /// What the player is told.
    pub fn message(self) -> &'static str {
        match self {
            Refusal::Grounded => "On the ground: staging locked (P still deploys the parachutes)",
            Refusal::NoStageLeft => "No stage left to activate",
        }
    }
}

/// What a vessel's stages are made of now.
#[derive(Clone, Copy, Debug)]
pub struct StageInputs<'a> {
    pub controls: &'a Controls,
    pub chute: &'a ChuteState,
    pub spec: &'a ChuteSpec,
    pub engine_running: bool,
    /// Propellant left, or debug mode (D064).
    pub fuelled: bool,
}

impl<'a> StageInputs<'a> {
    /// A vessel's stages under `controls`.
    pub fn of(vessel: &'a sim::vessel::Vessel, controls: &'a Controls) -> Self {
        StageInputs {
            controls,
            chute: &vessel.chute,
            spec: &vessel.craft.chute,
            engine_running: vessel.engine_running(controls),
            fuelled: vessel.debug() || vessel.propellant() > 0.0,
        }
    }
}

/// The stack, stage 0 (the parachutes) first.
pub fn stack(s: &StageInputs) -> Vec<Stage> {
    let chute_stage = if s.controls.chute { StageState::Active } else { StageState::Next };
    let mut chutes = Vec::new();
    if let Some(d) = &s.spec.drogue {
        chutes.push(canopy_part("DROGUES", d, s.chute.drogue, s.chute.armed, None));
    }
    // Behind a drogue the main waits for its height; alone it opens on
    // the command (sim::chute).
    let waiting = (s.chute.armed && s.spec.drogue.is_some()).then_some(s.spec.main_height);
    chutes.push(canopy_part("MAINS", &s.spec.main, s.chute.main, s.chute.armed, waiting));
    let engine = if s.engine_running {
        Part {
            name: "ENGINE".into(),
            state: format!("running · {:.0} %", s.controls.throttle * 100.0),
            tone: PartTone::Working,
        }
    } else if !s.fuelled {
        Part { name: "ENGINE".into(), state: "off · no propellant".into(), tone: PartTone::Failed }
    } else {
        Part { name: "ENGINE".into(), state: "off".into(), tone: PartTone::Idle }
    };
    vec![
        Stage { number: 0, state: chute_stage, name: None, parts: chutes },
        Stage { number: 1, state: StageState::Active, name: None, parts: vec![engine] },
    ]
}

/// A vessel's stack under `controls`: a craft of parts' sequence
/// ([`parts::stack`]), else the test craft's stages.
pub fn stack_of(vessel: &sim::vessel::Vessel, controls: &Controls) -> Vec<Stage> {
    match (vessel.ship_def(), vessel.ship()) {
        (Some(def), Some(state)) => parts::stack(&def, state, controls.chute),
        _ => stack(&StageInputs::of(vessel, controls)),
    }
}

/// One canopy's state in the D079 sequence. `waiting`: the height the
/// main opens below, once armed.
fn canopy_part(
    name: &'static str,
    c: &sim::chute::Canopy,
    state: CanopyState,
    armed: bool,
    waiting: Option<f64>,
) -> Part {
    let (state, tone) = match state {
        CanopyState::Stowed => match waiting {
            Some(h) => (format!("armed · opens below {}", crate::format::distance(h)), PartTone::Working),
            None if armed => ("armed".into(), PartTone::Working),
            None => ("stowed".into(), PartTone::Idle),
        },
        CanopyState::Open { age, .. } if c.reefed(age) => ("open · reefed".into(), PartTone::Working),
        CanopyState::Open { .. } => ("open".into(), PartTone::Working),
        CanopyState::Released => ("cut away".into(), PartTone::Idle),
        CanopyState::Failed => ("TORN".into(), PartTone::Failed),
    };
    Part { name: name.into(), state, tone }
}

/// The stage Space activates now, if any.
pub fn next(stack: &[Stage]) -> Option<u32> {
    stack.iter().filter(|s| s.state == StageState::Next).map(|s| s.number).max()
}

/// What activating stage `number` does.
pub fn activation(number: u32) -> Option<Activation> {
    (number == 0).then_some(Activation::ArmParachutes)
}

/// Activates the next stage on `controls`, unless the craft is on the
/// ground (`grounded`: landed or wrecked); returns what it did.
pub fn activate_next(s: &StageInputs, grounded: bool, controls: &mut Controls) -> Result<Activation, Refusal> {
    if grounded {
        return Err(Refusal::Grounded);
    }
    let a = next(&stack(s)).and_then(activation).ok_or(Refusal::NoStageLeft)?;
    match a {
        Activation::ArmParachutes => controls.chute = true,
    }
    Ok(a)
}

/// Whether orders arm the parachutes (a craft of parts): a command that
/// does, the next stage of the vessel's sequence or a saved function with
/// one. Arming them is the pilot's parachute command (D079).
pub fn arms_chutes(state: &sim::craft::state::ShipState, kinds: &[sim::systems::OrderKind]) -> bool {
    use sim::craft::command::ShipCommand::ArmChutes;
    use sim::systems::OrderKind;
    let has = |list: &[sim::craft::command::ShipCommand]| list.contains(&ArmChutes);
    kinds.iter().any(|k| match k {
        OrderKind::Command(c) => *c == ArmChutes,
        OrderKind::Stage => state.stages.get(state.sequence as usize).is_some_and(|s| has(&s.actions)),
        OrderKind::Function(name) => state.functions.iter().any(|f| f.name == *name && has(&f.actions)),
    })
}

/// Whether Space finds no stage left in a craft of parts' sequence: every
/// stage run, or already ordered (an order waits for its tick, or for the
/// signal's light delay, in `Runtime::orders`).
pub fn no_stage_left(state: &sim::craft::state::ShipState) -> bool {
    let ordered = state.runtime.orders.iter().filter(|o| matches!(o.kind, sim::systems::OrderKind::Stage)).count();
    state.sequence as usize + ordered >= state.stages.len()
}

/// Whether a vessel counts as on the ground for staging.
pub fn grounded(vessel: &sim::vessel::Vessel) -> bool {
    matches!(vessel.phase, sim::vessel::Phase::Landed { .. } | sim::vessel::Phase::Crashed { .. })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs<'a>(c: &'a Controls, chute: &'a ChuteState, spec: &'a ChuteSpec, running: bool) -> StageInputs<'a> {
        StageInputs { controls: c, chute, spec, engine_running: running, fuelled: true }
    }

    #[test]
    fn the_test_craft_stacks_parachutes_over_its_engine() {
        let spec = sim::craft::test_craft().params().chute;
        let c = Controls { throttle: 0.5, ..Controls::default() };
        let chute = ChuteState::default();
        let s = stack(&inputs(&c, &chute, &spec, true));
        assert_eq!(
            s.iter().map(|s| (s.number, s.state)).collect::<Vec<_>>(),
            [(0, StageState::Next), (1, StageState::Active)]
        );
        let parts: Vec<_> =
            s.iter().flat_map(|s| s.parts.iter().map(|p| (p.name.as_str(), p.state.as_str()))).collect();
        assert_eq!(parts, [("DROGUES", "stowed"), ("MAINS", "stowed"), ("ENGINE", "running · 50 %")]);
        assert_eq!(next(&s), Some(0));
    }

    #[test]
    fn part_states_follow_the_parachute_sequence() {
        let spec = sim::craft::test_craft().params().chute;
        let c = Controls { chute: true, ..Controls::default() };
        let open = |age| CanopyState::Open { age, diameter: 1.0 };
        // (drogue, main, armed, expected drogue, expected main)
        let table = [
            (CanopyState::Stowed, CanopyState::Stowed, false, "stowed", "stowed"),
            (open(1.0), CanopyState::Stowed, true, "open · reefed", "armed · opens below 3350 m"),
            (open(30.0), CanopyState::Stowed, true, "open", "armed · opens below 3350 m"),
            (CanopyState::Released, open(2.0), true, "cut away", "open · reefed"),
            (CanopyState::Released, open(60.0), true, "cut away", "open"),
            (CanopyState::Failed, CanopyState::Failed, true, "TORN", "TORN"),
        ];
        for (drogue, main, armed, want_d, want_m) in table {
            let chute = ChuteState { armed, drogue, main };
            let s = stack(&inputs(&c, &chute, &spec, false));
            let states: Vec<&str> = s[0].parts.iter().map(|p| p.state.as_str()).collect();
            assert_eq!(states, [want_d, want_m], "{drogue:?} {main:?}");
        }
        let torn = ChuteState { armed: true, drogue: CanopyState::Failed, main: CanopyState::Stowed };
        assert_eq!(stack(&inputs(&c, &torn, &spec, false))[0].parts[0].tone, PartTone::Failed);
    }

    #[test]
    fn space_arms_the_parachutes_once_then_there_is_no_next_stage() {
        let spec = sim::craft::test_craft().params().chute;
        let chute = ChuteState::default();
        let mut c = Controls::default();
        let before = c;
        assert_eq!(activate_next(&inputs(&before, &chute, &spec, false), false, &mut c), Ok(Activation::ArmParachutes));
        assert!(c.chute);
        let after = c;
        let s = stack(&inputs(&after, &chute, &spec, false));
        assert_eq!(s[0].state, StageState::Active);
        assert_eq!(next(&s), None);
        assert_eq!(activate_next(&inputs(&after, &chute, &spec, false), false, &mut c), Err(Refusal::NoStageLeft));
    }

    /// Space after the sequence's last stage is told so (review: it gave
    /// no feedback); stages already ordered count as run.
    #[test]
    fn space_after_the_last_stage_finds_none_left() {
        use sim::systems::OrderKind;
        let craft = crate::test_rocket::rocket();
        let sim = crate::state::SimState::with_craft(&craft);
        let mut state = sim.ship().ship().unwrap().clone();
        let n = state.stages.len();
        assert!(n >= 1);
        // (run, ordered, none left)
        let table = [(0, 0, false), (n - 1, 0, false), (n, 0, true), (n - 1, 1, true), (0, n, true)];
        for (run, ordered, want) in table {
            state.sequence = run as u32;
            state.runtime.orders.clear();
            for _ in 0..ordered {
                state.runtime.order(sim.clock, OrderKind::Stage);
            }
            state.runtime.order(sim.clock, OrderKind::Function("x".into()));
            assert_eq!(no_stage_left(&state), want, "{run} {ordered}");
        }
    }

    #[test]
    fn staging_is_locked_on_the_ground_and_changes_nothing() {
        let spec = sim::craft::test_craft().params().chute;
        let chute = ChuteState::default();
        // (grounded, result, armed after)
        let table = [(true, Err(Refusal::Grounded), false), (false, Ok(Activation::ArmParachutes), true)];
        for (grounded, want, armed) in table {
            let mut c = Controls::default();
            let before = c;
            assert_eq!(activate_next(&inputs(&before, &chute, &spec, false), grounded, &mut c), want, "{grounded}");
            assert_eq!(c.chute, armed, "{grounded}");
        }
        assert!(Refusal::Grounded.message().contains("P still deploys"));
        let sim = crate::state::SimState::new();
        assert!(grounded(sim.ship()), "on the pad");
    }

    #[test]
    fn an_engine_without_propellant_says_so() {
        let spec = sim::craft::test_craft().params().chute;
        let (c, chute) = (Controls::default(), ChuteState::default());
        let dry = StageInputs { fuelled: false, ..inputs(&c, &chute, &spec, false) };
        let engine = &stack(&dry)[1].parts[0];
        assert_eq!((engine.state.as_str(), engine.tone), ("off · no propellant", PartTone::Failed));
    }
}
