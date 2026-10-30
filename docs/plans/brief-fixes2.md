# Brief: FIXES2, Δv in vacuum and the abort that exists (2026-10-30)

D120 (owner, 2026-10-30). Follow [agent-rules.md](agent-rules.md). Your name is FIXES2 and your target dir is `target/agents/fixes2`: start warm with `mv target/agents/quickfix target/agents/fixes2` (a release build of the game is in it). Two other agents run at the same time. **FLIGHTSW** ([brief-flightsw.md](brief-flightsw.md)) is on SAS, the holds, `systems/data.rs`, `systems/estimate.rs`, `landing/` and the pad's release. **COAST** ([brief-coast.md](brief-coast.md)) is on `vessel/coast.rs`, `vessel/mass.rs`, `systems/plumbing.rs` and the lattice. Stay out of files with their uncommitted changes in them. "NO patchwork fixes": fix the default or the model, never a case.

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

(Update as you land: commits with hashes, what is left.)
