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

**Done** (2026-10-29/30), all five items, each with its tests:

1. 9886435: "Launch abort" shuts every Merlin and the MVac down (`Shutdown("s1")`, `Shutdown("s2")`) before Dragon separates and lights its SuperDracos; `systems::command` test on the pad, booster and second stage, and the vessel-level pad test checks the pieces.
2. 7424645: Dragon's tanks at **16.5 MPa** (2,400 psi, E: SpaceX's 15 July 2019 call, as reported by Teslarati [S29] in falcon9.md §3.2; the only published figure). The craft check `craft::build::underfed` / `fed` (table test): every pressure-fed engine's feed above its chamber plus a 20 % injector drop (E, Sutton & Biblarz), reported as a build contradiction; the Falcon 9 test asserts none. Engines carry a published `chamber_pressure` (the SuperDraco's 6.9 MPa). The tank walls (titanium, sized at the new MEOP) add 333 kg, carried by the capsule's unmodelled gap (2,356 → 2,023 kg; Dragon's total unchanged); the tank transducer moved to a 5,000 psi class.
3. e48a0fc: `game::pad`: once the engines of the active, whole vessel are off after a "Pad abort", `GameCommand::Reset` puts a fresh vessel of its kind on the pad (new launch save, toast); the clock runs on. Rule `replace_after_abort` with a table test, and the Falcon 9's pad abort in a game test.
4. 3a4f502: the stage editor shows the stages already run locked, tagged RUN (`systems_screen::edit::allowed`, table test); the save's refusal stays.
5. cd74409: parts.md's two "Not done" lists checked against the code, the Falcon 9's status *playable*.

Also 9aca5bc: `SUNSCATTER_DEMO_STOP_AFTER=PadAbort` (with `SUNSCATTER_CRAFT=falcon9-crew`) shows items 3 and 4 on screen: `parts_stages_locked` (Ignition and Liftoff locked, MECO's "up" off) and `parts_pad_abort` (the fresh vessel clamped, Ignition next, full tanks, the toast; a second launch save written). Seen at 1600×900.

**Open for the owner** (logical defaults taken):
- A pad abort replaces only the **whole** vessel: after a launch abort on the pad the stack (its capsule gone) stays on the pad, since that launch flies on with the crew in the capsule and a fresh Falcon 9 would bring a second crew. Revert or New flight clears it.
- It fires only for the **active** vessel (a new flight replaces the active vessel and writes its launch save); a vessel switched away from during its tail-off waits until it is active again.
- The launch abort's thrust termination is a commanded shutdown with the normal tail-off, not an instant cut.
- Dragon's tanks are loaded at 16.5 MPa from the pad; the real system pressurises to it ~100 ms before the SuperDracos light. The two helium bottles (0.1 m³ at 31 MPa, U) give ~3 kg above 16.5 MPa, against ~26 kg to hold it through a 5 s abort burn: the tanks' pressure sags late in the burn. Resize them (U: ~1.6 m³ at 41 MPa to empty the tanks) when thrust follows feed pressure (the review's recommendation 5).
- The Dracos (thrusters) are not in the feed check: their chamber pressure is U (falcon9.md proposes 1.0 MPa).

**Seen in passing, not mine:** the staging bar on the pad shows "Second stage ignition … Δv 0.0 m/s" with full tanks and a 9 m 32 s burnout (`parts_pad_abort`).
