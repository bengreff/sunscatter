# Brief: AUDIT, warp invariance and the software layers (2026-10-30)

Two owner decisions from tonight ([interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md)) need a map of the code before the wave can build them:
- **D129:** "EXACTLY the same behavior at 100,000 warp as you would get at 1x warp." Only the player's input changes the course of events. The owner asked: "Tell me where that is difficult."
- **D128:** three layers of ship software. Each part's own logic is layer 1; the ship computer's programs are layer 2; the agent outside the game is layer 3.

Your job is to find out and write it down, not to fix. ADVANCE and LAYERS fix it later from your docs. Follow [agent-rules.md](agent-rules.md). Your name is AUDIT and your target dir is `target/agents/audit` (a cold start; sim only, mostly).

Two other agents edit code at the same time:
- **PLANNER** ([brief-planner.md](brief-planner.md)) works on planned burns;
- **ONEKIND-3** ([brief-onekind.md](brief-onekind.md)) works on tests and the stress craft.

Write only **new** files: your docs, and test files of your own under `crates/sim/tests/`. Edit nothing else. List any other change you find needed in your State.

## Read first

- D129, D128, D121, D119, D081, D082, D111, D105, D123, D126, D127 in `docs/decisions.md`; the lead's first answer at the end of the interview.
- `docs/systems/motion.md`, `ship.md`, `avionics.md`, `failures.md`; `docs/technical/motion-model.md`, `limbo.md`, `ship-model.md` §5.10, `flight-software.md`, `failure-modes.md`, `scale-performance.md`.
- Code:
  - how a vessel advances: `sim::vessel::{live, coast, systems, ship_tick, bubble}` and `sim::systems::*` (the 60 s lattice, `coast_law`, `plumbing_coast`);
  - `sim::thermal`, `sim::systems::failure`;
  - `sim::systems::flight` and `sim::craft::program`;
  - the game's warp: `game::state` (`rails_block`, `effective_warp`) and where it steps the sim each frame.

## The work, in order

1. **D129: the warp audit** → `docs/technical/warp-invariance.md`. For every subsystem and every kind of event (thermal, plumbing and boil-off, relief, power and batteries, life support, failures and fates, comms and light delay, attitude and holds, the bubble, atmosphere entry, surface contact, planned burns coming due, separations):
   - how it advances live, on the 60 s lattice and on a coast;
   - whether the same span advanced in frame-sized chunks at 1× (16.7 ms), 100×, 10⁴× and 10⁵× gives the same bits;
   - where an event is found at the end of a frame instead of at its own epoch;
   - where anything is decided from the warp rather than from the craft's state.

   Prove what you can with a test file of your own (e.g. `crates/sim/tests/warp_course.rs`): a craft of parts (the Falcon 9's second stage with Dragon in orbit, or `craft::small`'s) through a long span with events, fates on, advanced in each chunking, compared by a hash of the whole state. Report the first field that differs. Commit only passing tests. A divergence you find goes in the doc with a reproduction, not as a red test.

   End the doc with a ranked list of "where it is difficult" for the owner, each with what restricting warp would cost against what changing the model would cost.
2. **D128: the layers inventory** → `docs/technical/software-layers.md` (design). Classify every piece of logic that decides something, with where it lives today and where it should:
   - layer 1, the part's own: engine controllers, start and shutdown, the fail-safe shutdown, valves, relief, regulators, power shedding, life support's regulation, chute mechanisms;
   - layer 2, the ship computer: SAS, the holds, the launch commit, the stage sequence and functions, aborts, chute deploy logic, the maneuver function PLANNER is writing.

   Then define:
   - the bare minimum: what layer 1 alone must do to "coast for months if everything is set up";
   - the warp-stop line on it (a failure that puts the crew or the vessel at risk), and how layer 2 replaces it;
   - notifications off per craft;
   - the module layout and the two APIs. Layer 1 is part behaviour set by the part's parameters (D125). Layer 2 is `Knowledge` in, commands out (D043).
3. **Draft the next two briefs** from 1 and 2, in the house format: `docs/plans/brief-advance.md` and `docs/plans/brief-layers.md`.

Docs are design until built: say so. End with the report agent-rules asks for, with the top five difficulties for the owner in it.

## State

(Update as you go: findings, tests committed with hashes, what is left in order, Open for the owner.)
