//! Staging (D080): a craft's stages as the staging bar and the menus show
//! them, as pure functions. Its sequence is the craft's own program
//! ([`parts::stack`], `sim::craft::program`): Space runs the next stage
//! (`GameCommand::Stage`), and [`no_stage_left`] says when none is left.
//! Its parachutes read in their D079 sequence ([`canopies`]); P is not
//! locked: nothing refuses the parachute command (D079).

pub mod parts;

use sim::chute::{CanopyState, ChuteSpec, ChuteState};
use sim::systems::ship_os::chutes::Chutes;

/// Where a stage is in the sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageState {
    /// Space activates it.
    Next,
    /// Activated.
    Active,
    /// After the next one.
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
    /// Its name in the craft's sequence.
    pub name: Option<String>,
    pub parts: Vec<Part>,
}

/// What Space after the sequence's last stage tells the player.
pub const NO_STAGE_LEFT: &str = "No stage left to activate";

/// A craft's canopies in the D079 sequence: the drogues (if it has any),
/// then the mains, each with its state (the Landing menu's PARACHUTE rows);
/// the mains' height the deploy's (`deploy`: the pilot's or the craft's,
/// `ship_os::chutes::main_height`).
pub fn canopies(spec: &ChuteSpec, deploy: &Chutes, chute: &ChuteState) -> Vec<Part> {
    let mut out = Vec::new();
    if let Some(d) = &spec.drogue {
        out.push(canopy_part("DROGUES", d, chute.drogue, chute.armed, None));
    }
    // Behind a drogue the main waits for its height; alone it opens on
    // the command (sim::chute).
    let waiting =
        (chute.armed && spec.drogue.is_some()).then(|| sim::systems::ship_os::chutes::main_height(spec, deploy));
    out.push(canopy_part("MAINS", &spec.main, chute.main, chute.armed, waiting));
    out
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

/// Whether orders arm the parachutes (a craft of parts, `def`): a command
/// that does, the next stage of the vessel's sequence in its view (or one
/// run again) or a saved function with one (`sim::craft::program`). Arming
/// them is the pilot's parachute command (D079).
pub fn arms_chutes(
    def: &sim::craft::ship::ShipDef,
    state: &sim::craft::state::ShipState,
    kinds: &[sim::systems::OrderKind],
) -> bool {
    use sim::craft::command::ShipCommand::ArmChutes;
    use sim::craft::program;
    use sim::systems::OrderKind;
    let has = |k: usize| state.stages.get(k).is_some_and(|s| s.actions.contains(&ArmChutes));
    kinds.iter().any(|k| match k {
        OrderKind::Command(c) => *c == ArmChutes,
        OrderKind::Stage => program::next(def, &state.stages, state.sequence).is_some_and(has),
        OrderKind::Again(k) => program::has_run(def, &state.stages, state.sequence, *k) && has(*k as usize),
        OrderKind::Function(name) => {
            program::function(def, &state.functions, name).is_some_and(|f| f.actions.contains(&ArmChutes))
        }
    })
}

/// Whether Space finds no stage left in a craft of parts' sequence (`def`'s
/// view, `sim::craft::program`): every stage run, or already ordered (an
/// order waits for its tick, or for the signal's light delay, in
/// `ShipState::orders`).
pub fn no_stage_left(def: &sim::craft::ship::ShipDef, state: &sim::craft::state::ShipState) -> bool {
    let ordered = state.orders.iter().filter(|o| matches!(o.kind, sim::systems::OrderKind::Stage)).count();
    sim::craft::program::pending(def, &state.stages, state.sequence).len() <= ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canopies_follow_the_parachute_sequence() {
        let spec = sim::craft::small::rocket_ref().spec.chute.clone();
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
            let c = canopies(&spec, &Chutes::default(), &chute);
            let names: Vec<&str> = c.iter().map(|p| p.name.as_str()).collect();
            assert_eq!(names, ["DROGUES", "MAINS"]);
            let states: Vec<&str> = c.iter().map(|p| p.state.as_str()).collect();
            assert_eq!(states, [want_d, want_m], "{drogue:?} {main:?}");
        }
        // The deploy's own height, where the pilot set one (D126's config).
        let pilots = Chutes { main_height: Some(2_000.0), ..Chutes::default() };
        let waiting = ChuteState { armed: true, drogue: open(30.0), main: CanopyState::Stowed };
        assert_eq!(canopies(&spec, &pilots, &waiting)[1].state, "armed · opens below 2000 m");
        let torn = ChuteState { armed: true, drogue: CanopyState::Failed, main: CanopyState::Stowed };
        assert_eq!(canopies(&spec, &Chutes::default(), &torn)[0].tone, PartTone::Failed);
        // Mains alone open on the command.
        let mains = ChuteSpec { drogue: None, ..spec.clone() };
        let armed = ChuteState { armed: true, ..ChuteState::default() };
        assert_eq!(
            canopies(&mains, &Chutes::default(), &armed).iter().map(|p| p.state.as_str()).collect::<Vec<_>>(),
            ["armed"]
        );
    }

    /// Space after the sequence's last stage is told so (review: it gave
    /// no feedback); stages already ordered count as run.
    #[test]
    fn space_after_the_last_stage_finds_none_left() {
        use sim::systems::OrderKind;
        let craft = crate::test_rocket::rocket();
        let sim = crate::state::SimState::with_craft(&craft);
        let mut state = sim.ship().ship().unwrap().clone();
        let def = sim.ship().ship_def().unwrap();
        let n = state.stages.len();
        assert!(n >= 1);
        // (run, ordered, none left)
        let table = [(0, 0, false), (n - 1, 0, false), (n, 0, true), (n - 1, 1, true), (0, n, true)];
        for (run, ordered, want) in table {
            state.sequence = run as u32;
            state.orders.clear();
            for _ in 0..ordered {
                state.order(sim.clock, OrderKind::Stage);
            }
            state.order(sim.clock, OrderKind::Function("x".into()));
            assert_eq!(no_stage_left(&def, &state), want, "{run} {ordered}");
        }
    }
}
