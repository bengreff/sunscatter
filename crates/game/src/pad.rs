//! The pad after a pad abort (D114): "Reset the ship fully, after all it
//! needs to refuel. New launch." Once the engines of a vessel whose clamps
//! never let go are shut down, a fresh vessel of its kind goes on the pad
//! as loaded (full tanks, new engines, nothing staged) with a new launch
//! save: the path of a new flight (`SimState::reset`, sent as
//! `GameCommand::Reset`), and a toast says so. The clock runs on: this is
//! not a revert. The sim's rollback of the sequence
//! (`sim::systems::command::release_undone`) stays for a vessel this does
//! not replace.

use crate::commands::GameCommand;
use crate::interface::toasts::Toasts;
use crate::state::SimState;
use bevy::prelude::*;
use sim::craft::state::EngineStatus;
use sim::vessel::{Vessel, VesselId};

/// What the rule reads of a vessel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PadState {
    /// A pad abort is in its log: the clamps never let go.
    pub aborted: bool,
    /// The pad's clamps still hold it.
    pub clamped: bool,
    /// The craft as loaded, not a piece: after a launch abort the capsule
    /// has left with the crew, and that launch flies on.
    pub whole: bool,
    /// Its engines starting, running or tailing off.
    pub lit: usize,
    /// The active vessel: a new flight replaces the active vessel and
    /// writes the active vessel's launch save.
    pub active: bool,
}

/// Whether a fresh vessel replaces this one on the pad now: after a pad
/// abort, still clamped, whole and active, once every engine is shut
/// down. It waits while an engine tails off (or was lit again) and while
/// the vessel is not the active one.
pub fn replace_after_abort(s: PadState) -> bool {
    s.aborted && s.clamped && s.whole && s.lit == 0 && s.active
}

/// The rule's view of `v` (`None` for a craft without systems).
pub fn pad_state(v: &Vessel, active: bool) -> Option<PadState> {
    let ship = v.ship()?;
    let lit =
        |s: &EngineStatus| matches!(s, EngineStatus::Starting | EngineStatus::Running | EngineStatus::ShuttingDown);
    Some(PadState {
        aborted: v.ship_events().iter().any(|e| e.text.starts_with("Pad abort")),
        clamped: v.clamped(),
        whole: v.craft.id.parts().is_none(),
        lit: ship.engines.iter().map(|e| e.status).filter(lit).count(),
        active,
    })
}

/// After the step: the active vessel replaced once its pad abort is over
/// (each vessel once; the command runs at the next frame's start).
pub fn update(
    sim: Res<SimState>,
    time: Res<Time>,
    mut commands: MessageWriter<GameCommand>,
    mut toasts: ResMut<Toasts>,
    mut sent: Local<Option<VesselId>>,
) {
    let ship = sim.ship();
    if *sent == Some(ship.id()) || !pad_state(ship, true).is_some_and(replace_after_abort) {
        return;
    }
    *sent = Some(ship.id());
    commands.write(GameCommand::Reset);
    let name = ship.ship_def().map_or_else(|| ship.craft.id.base().to_string(), |d| d.name.clone());
    let text = format!("Pad abort: a fresh {name} on the pad, fuelled (a new launch)");
    toasts.push(time.elapsed_secs_f64(), text, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pad abort ends the launch once its engines are off: it fires for
    /// the active, whole vessel still on the clamps, and waits otherwise.
    #[test]
    fn a_pad_abort_puts_a_fresh_vessel_on_the_pad() {
        let over = PadState { aborted: true, clamped: true, whole: true, lit: 0, active: true };
        // (case, state, fires)
        let cases = [
            ("engines off after the abort", over, true),
            ("engines tailing off", PadState { lit: 9, ..over }, false),
            ("no pad abort", PadState { aborted: false, ..over }, false),
            ("the clamps let go", PadState { clamped: false, ..over }, false),
            ("a piece: the capsule left", PadState { whole: false, ..over }, false),
            ("not the active vessel", PadState { active: false, ..over }, false),
        ];
        for (case, state, want) in cases {
            assert_eq!(replace_after_abort(state), want, "{case}");
        }
    }

    /// The Falcon 9 on the pad: the throttle closed while the release is
    /// armed shuts the Merlins down (D114) and the clamps hold (the pad
    /// abort); the rule waits through the tail-off, then fires, and the
    /// fresh vessel is on the pad as loaded with its launch save pending.
    #[test]
    fn the_falcon_9_after_a_pad_abort() {
        let mut sim = SimState::new();
        sim.new_flight(&crate::state::craft_or_fallback("falcon9-crew"));
        let (start, w) = (sim.clock, sim.world.clone());
        let full = sim::vessel::Controls { throttle: 1.0, sas: true, ..Default::default() };
        let closed = sim::vessel::Controls { throttle: 0.0, ..full };
        let v = &mut sim.fleet[sim.active];
        assert_eq!(pad_state(v, true).map(replace_after_abort), Some(false), "on the pad, before");
        v.next_stage(&w).unwrap();
        v.next_stage(&w).unwrap();
        v.advance(&w, start.add_seconds(1.0), &full, usize::MAX);
        v.advance(&w, start.add_seconds(1.1), &closed, usize::MAX);
        let s = pad_state(v, true).unwrap();
        assert!(s.aborted && s.clamped && s.lit > 0, "tailing off: {s:?} {:?}", v.ship_events());
        assert!(!replace_after_abort(s));
        v.advance(&w, start.add_seconds(8.0), &closed, usize::MAX);
        let s = pad_state(v, true).unwrap();
        assert!(replace_after_abort(s), "{s:?}");
        let t = v.time;
        sim.clock = t;
        sim.launch_pending = false;
        sim.reset();
        let fresh = sim.ship();
        assert!(!pad_state(fresh, true).is_some_and(replace_after_abort), "the fresh vessel");
        assert!(sim.launch_pending && fresh.launch == Some(fresh.id()) && fresh.clamped());
        assert_eq!(fresh.ship().unwrap().sequence, 0, "nothing staged");
        assert!(fresh.ship().unwrap().engines.iter().all(|e| e.status == EngineStatus::Off));
        assert_eq!(fresh.time, t, "the clock runs on");
    }
}
