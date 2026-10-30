# Brief: QUICKFIX, the owner's quick fixes (2026-10-29)

The quick fixes from the hand-off and the owner's answers ([interview-2026-10-29-next.md](interview-2026-10-29-next.md); D114, D115). Follow [agent-rules.md](agent-rules.md). Your name is QUICKFIX and your target dir is `target/agents/quickfix`: start warm with `mv target/agents/plume target/agents/quickfix`, since PLUME-ALTITUDE has finished. **FOUNDATION** ([brief-foundation.md](brief-foundation.md)) works at the same time on the vessel's state: `crates/sim/src/craft/state.rs`, `systems/runtime.rs`, `vessel/split.rs` and whatever holds `ShipState` / `Runtime` fields. Stay out of those files. If you need one, do your other items first and wait until FOUNDATION has committed it.

## The work, in order

Each item: a failing test first where it is a rule, then the fix, then one commit.

1. **Launch abort shuts the stack's thrust down.** The Falcon 9's "Launch abort" function (`data/craft/falcon9-crew/craft.ron`, line ~225) separates Dragon and lights the SuperDracos, but the Merlins keep running. The real vehicle terminates thrust (the AFSS and the stage 2 pyrovalve, `features/falcon9.md` §5.1). Add the shutdown of every running Merlin and the MVac, using the function actions that already exist if one fits. Test both cases, from the pad and in flight: after the abort no stack engine is starting or running, and Dragon's SuperDracos are.
2. **Dragon's tanks against the SuperDraco's chamber.** Dragon's propellant tanks are loaded at about 1.7 MPa, while the SuperDraco's chamber is at 6.9 MPa, so a pressure-fed engine is fed below its own chamber pressure. D115: published wins. Find the real regulated tank pressure (sources into `falcon9.md`; mark it E if nothing is published) and fix the craft's data. Add a craft check as a pure function with a table test: every pressure-fed engine's feed pressure exceeds its chamber pressure plus an injector drop (E, cite the drop you use). Thrust following feed pressure is the review's recommendation 5 and is not in scope.
3. **A pad abort puts a fresh vessel on the pad** (D114's new bullet). Once the engines are shut down after a "Pad abort" event, the game puts a fresh vessel of the same kind on the pad as loaded (full tanks, new engines, nothing staged), writes a new launch save, and shows a toast. The clock runs on. Reuse the path that already puts a new vessel of its kind on the pad (`crates/game/src/interface/pause.rs`, `GameCommand::NewFlight`). The trigger is a pure rule with a table test: when it fires and when it waits. SEQUENCE's rollback in `sim` can stay.
4. **The stage editor locks the stages already run** (D114, D111). In the systems screen's stage editor (find it from `edit_sequence` / `set_stages` in `crates/game/src`), rows for stages already run are shown but cannot be edited. The refusal on save stays as a backstop.
5. **`docs/features/parts.md`'s "Not done" list** is out of date. It still says engines start and stop at once and that nothing checks loads; RUNTIME and SYSTEMS2 built both (the review's §1, last paragraph). Check the rest of the list against the code too.

**On screen:** check items 3 and 4 in the game or a demo (take the lock), and put the shots in your scratchpad for the lead.

## State

(Update as you land: commits with hashes, what is left.)
