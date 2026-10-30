# Brief: FIXES2, Δv in vacuum and the abort that exists (2026-10-30)

D120 (owner, 2026-10-30). Follow [agent-rules.md](../../plans/agent-rules.md). Your name is FIXES2 and your target dir is `target/agents/fixes2`: start warm with `mv target/agents/quickfix target/agents/fixes2` (a release build of the game is in it). Two other agents run at the same time. **FLIGHTSW** ([brief-flightsw.md](brief-flightsw.md)) is on SAS, the holds, `systems/data.rs`, `systems/estimate.rs`, `landing/` and the pad's release. **COAST** ([brief-coast.md](brief-coast.md)) is on `vessel/coast.rs`, `vessel/mass.rs`, `systems/plumbing.rs` and the lattice. Stay out of files with their uncommitted changes in them. "NO patchwork fixes": fix the default or the model, never a case.

## The work, in order

1. **The staging bar's Δv is in vacuum by default.** The CURRENT AIR / VACUUM switch stays, with VACUUM as the default. Don't add per-stage air rules (ROUNDTRIP's question in [brief-foundation.md](brief-foundation.md) is answered by D120). Test the default.
2. **"Launch abort" is offered only when it would save the crew or the vehicle.** The rule is derived from what the piece holds, as a pure function with a table test:
   - a crewed capsule;
   - escape engines on it that can start (ignitions, propellant, power);
   - the joint that separates it from a stack still in the piece.

   No names in the rule. The craft file may mark a function as an abort, but its availability comes from the state. Cases:
   - the Falcon 9 stack on the pad and in ascent: offered;
   - the booster after the stage separation: not offered;
   - Dragon after its own separation: not offered;
   - a craft with no escape system: not offered (Cargo Dragon has no SuperDracos).

   The action bar and the systems screen go through the rule. Today the booster's bar shows "Launch abort" after separation: see `/private/tmp/claude-501/-Users-ben-sunscatter/1dd0ee7c-0f1f-4edf-9dcf-b65170a182f9/scratchpad/demo-f1f2/17_parts_booster.png`.
3. **Dragon's NTO and MMH tanks vent at relief in orbit** (11 g and 0.5 g a day, MASS's boil-off test). Since 7424645 they are loaded at 16.5 MPa. Check the loading pressure against each tank's MEOP and relief setting from the sources (`falcon9.md` §3.2), and fix whichever is wrong: the data, or the relief rule if it is the model.

**On screen:** after the stage separation, the booster's action bar has no "Launch abort", and the upper stack's does (the demo with `SUNSCATTER_CRAFT=falcon9-crew`; take the lock).

## State

**Landed (2026-10-30):**

1. f806401: the staging bar's figures in vacuum by default (`stage_bar::Air`, `#[default] Vacuum`; `ambient(air, p)` the one rule for the estimates' pressure), CURRENT AIR the switch's other side, one air for every stage; table test `the_figures_are_in_vacuum_by_default`. *Playable.*
2. 537e456 + 02dfb33 (clippy's type complexity in the test, which the pre-push caught): `Stage::abort` (the craft file marks a function as an abort: the Falcon 9's "Launch abort"); `sim::craft::program::offered(def, state, f)`: an abort is offered only while the piece holds a crew cabin (seats), an intact joint the abort separates holding the cabin to a stack, and an engine it lights on the cabin's side that can start (`program::can_start`: not failed, `can_ignite`, `reachable`, `propulsion::fed`); else `NoEscape` says why. 13-row table test (pad, ascent, upper stack: offered; booster, Dragon alone, Cargo Dragon, SuperDracos failed, dry or unpowered, Dragon's separation fired, a procedure that separates nothing or lights what stays behind: not; a plain function: offered). The action bar shows only offered functions; the systems screen tags an abort ABORT and turns its run button off where it would save no one, with the reason. *Playable.*
3. 57950ca: Dragon's NTO and MMH tanks' MEOP and relief 19.0 MPa (U: 115 % of the 16.5 MPa regulated feed, the main tanks' rule; falcon9.md §3.2). The cause: the loading put the helium at 16.5 MPa with the propellant's vapour on top (NTO ~0.1 MPa, MMH ~5 kPa at 293 K), over a relief equal to the feed pressure, so each tank vented its vapour's share once (11 g, 0.5 g). The data was wrong (no margin between operating pressure and MEOP); the relief rule (vent above MEOP) stands. The boil-off tests now check that Dragon's tanks vent nothing and that the LOX vents what boils (`at_saturation_the_heat_boils_off` had passed only on Dragon's 11 g).

**On screen** (the parts demo, `SUNSCATTER_CRAFT=falcon9-crew`, built from 57950ca's tree; shots in FIXES2's scratchpad, `fixes2/demo/`): after the stage separation the booster's action bar has no "Launch abort" (`17_parts_booster.png`) and the upper stack's has it (`15_parts_ui_separation.png`); VACUUM is selected on every bar, and on the pad the second stage reads Δv 6600 m/s, no longer 0.0 (`02_parts_ui_pad.png`).

**Left, in order:**

1. The sim's function runner (`systems::command::function`, the MCP's and a script's route by name) still runs an abort the piece does not offer: route it through `program::offered` (refuse with `NoEscape::describe` in the event log). Waits for FLIGHTSW's uncommitted `systems/command.rs`.
2. The load's vapour: `ShipState::initial` puts the pressurant at the loading pressure and the liquid's vapour on top, while the controller (`plumbing::pressurise`: the gas to the loading pressure less the vapour) and the relief read the total, so a loaded tank reads its vapour pressure high (Dragon's NTO 16.6 MPa, LOX +3.4 kPa). Load the gas at the loading pressure less `plumbing::vapour`. Waits for COAST's uncommitted `craft/state.rs`.
3. `systems::estimate`'s ignition check (`ready`) repeats `program::can_start` without the propellant; one owner after FLIGHTSW lands.

**Open for the owner:**

- An abort needs a crew cabin, not people in it: an uncrewed Crew Dragon (Demo-1, the in-flight abort test) is offered it, since it saves the vehicle (D120's "the crew or the vehicle").
- "Can start" is any escape engine able to start: one SuperDraco of eight (T/W under 1 on the pad) still offers the abort. A thrust criterion (the leaving side's T/W, or its acceleration away from the stack) is the refinement if wanted.
- The systems screen keeps an abort it would not offer in its list (the program is the whole vessel's; hiding it would hide it from editing), with the run button off; only the action bar hides it.
- Dragon's 19.0 MPa MEOP and relief are proposed (U, not published); the walls are 15 % heavier and the capsule's dry-mass check carries the difference.
