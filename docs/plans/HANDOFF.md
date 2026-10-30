# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-30: F1 and F2 landed; the owner answered (D119 coasts, D120 Δv in vacuum and the real abort); FLIGHTSW, COAST and FIXES2 running.*

## Your first job

1. Check on the three agents: FLIGHTSW (F3, [brief-flightsw.md](brief-flightsw.md)), COAST (D119, [brief-coast.md](brief-coast.md)) and FIXES2 (D120, [brief-fixes2.md](brief-fixes2.md)). Agents wrap up at ~300k context (owner, 2026-10-30).
2. Then work through "What is next".

## State

- **QUICKFIX ([brief-quickfix.md](brief-quickfix.md)): done.** Launch abort shuts the Merlins and the MVac down (9886435). Dragon's tanks are at the published 2,400 psi, with a feed check (7424645). A pad abort puts a fresh vessel on the pad (e48a0fc). The stage editor locks the stages already run (3a4f502). parts.md is current (cd74409). The demo has `SUNSCATTER_DEMO_STOP_AFTER=PadAbort`. Both checked on screen.
- **F1, split and merge by construction: done** ([brief-foundation.md](brief-foundation.md)): `craft::partition` (4ed2658), `Vessel::piece` (2d9de38), and every piece keeps the whole program with a per-piece view, `craft::program` (e4718a0, 7a0f7f6). Seen on screen after the separation (the lead's demo, 2026-10-30). The upper stack's bar shows Dragon separation, SECO, Second stage ignition (6,600 m/s at 79.5 km) and Liftoff. The booster's shows "no stage left", Stage separation active, and MECO, Liftoff and Ignition with AGAIN. The booster's action bar still offers "Launch abort": D120 (FIXES2). Left for docking: a list of cabins joined by hatches (the brief's Left 2).
- **F2, one mass truth: done** ([brief-mass.md](brief-mass.md), 751d0da). A craft of parts' propellant is its ship state's, each liquid sits where its slug is, and the thermal nodes take each tank's contents. A coast keeps its starting mass for drag and radiation pressure (open below). The booster's flip (the centre of mass moving as the LOX is thrown up) is not in the demo, so it has not been seen on screen.
- **The fixes of 2026-10-28: done, seen on screen.** AGAIN on every stage already run (`stage_bar::rows::Place::Done`); a part's card sums its components by kind (`part_card::inside`); the plume leaves the lip at the bell's width (`exhaust::profile`); the action bar at the top right; the systems screen's two columns, no pop-ups. Not done, with reasons in the plan's "lead's findings": the hold for cards, bar and screen.
- **Sequence ([brief-sequence.md](brief-sequence.md)): all five bugs fixed and tested.**
- **Plume ([brief-plume-altitude.md](brief-plume-altitude.md)):** the exit temperature in shifting equilibrium (Merlin 1D 1,770 K, 1077a54) and a mixing layer that merges the nine Merlins ~2 m behind the bells (62e660e). Left: the grey cloud of cold soot at MECO (reheating where the fans meet), the shock beads in the merged max-Q plume.
- **D116, crew doses: decided, not built.** `systems::crew` still records injuries.
- **Fidelity review:** the parts/components/layers split is detailed enough; the weakness is coupling (much state drives nothing). Gaps left: SuperDraco thrust ignores feed pressure; no capsule–trunk joint; a solid ocean; no docking; the flight software reads the true state; failures are only flags.
- **New this session:** the lead hands off at ~400k (CLAUDE.md); a pre-commit hook refuses a commit from a stale index; `SUNSCATTER_DEMO_UI_SCALE` shrinks the interface for shots.
- CI: check `gh run list --limit 4` (the last pushes were still running at handoff).

## What is next

1. **F3, the last foundation** (D117; the review's §4.4): the flight software reads `Knowledge`. SAS, the holds, the pad's health check, the estimates and the landing prediction stop reading the truth.
2. **COAST** (D119): a coast's slow changes solved analytically where possible, with one law for the trajectory and the ship state. **FIXES2** (D120): Δv in vacuum by default; "Launch abort" only where a real abort exists; Dragon's tanks venting at relief.
3. Queued: Dragon's helium bottles are too small for 16.5 MPa through an abort burn (with recommendation 5); a vacuum nozzle at sea level has no flow separation, so the MVac's thrust goes negative under the current-air readout.
4. **Then two flights end to end, in parallel** (D117):
   - **Crew Dragon to the ISS and back:** the capsule–trunk joint and the entry; the abort tests; water and splashdown with D116's doses; docking at three autonomy levels.
   - **The booster's return:** the vacuum relight and separation after MECO; the booster flown by its own flight software through the boostback, entry and landing burns; retropropulsion; a landing zone first.
5. The interview's older queue:
   - planned burns for craft of parts (`PlanError::PartsUnsupported`), needed for Dragon's phasing burns;
   - the Falcon 9 inconsistencies (the legs' mass, `falcon9.md`'s U/E rows).
6. D115's fidelity, after the foundations:
   - sensors;
   - adaptive functions;
   - powered components' heat (this revises D077: update it and its citations in the same commit);
   - per-engine gimbals;
   - O₂ / N₂ tanks;
   - noise only where needed.
7. The plume's rest (the MECO cloud, the shock cells), GLOW, CLOUDS.
8. Later roadmap, nothing built now: the career as a startup (D027), with D118's components and factories.

## Open for the owner, new (defaults in place)

- **QUICKFIX** ([brief-quickfix.md](brief-quickfix.md)):
  - after a launch abort on the pad, the stack without its capsule stays (no second crew);
  - a pad abort of a vessel not being flown waits until it is active;
  - thrust termination uses the normal tail-off;
  - the helium bottles get resized with recommendation 5.
- **ROUNDTRIP** ([brief-foundation.md](brief-foundation.md)):
  - CURRENT AIR: answered, D120 (vacuum by default, no per-stage rules);
  - a vacuum nozzle at sea level: queued (a flow-separation model);
  - Liftoff stays on both pieces' bars;
  - edits are per piece, and docking joins the two copies by name;
  - stage numbers count within each piece's view.
- **MASS** ([brief-mass.md](brief-mass.md)):
  - a coast's mass for its perturbations: answered, D119 (COAST);
  - in roll, the liquid turns with its tank until the slosh model;
  - the ullage gas is left out of the thermal capacities.

## Open for the owner (older)

U/E data in `falcon9.md`, Dragon's CO₂ removal, the Moon's field degree, the 1,000-year ephemeris (D005), the rails floor against a q-based boundary, backhouse's certificate (Windows not checked since 2026-10-26). Engine exit states later from the owner's CRUCIBLE (`~/inquiry-project`).

## Gotchas


- **Agents follow [agent-rules.md](agent-rules.md).** Their private-index commits leave the shared index stale. The lead runs `git reset -q` before every commit while agents run; b48bc21 reverted an agent's fix this way, and the new pre-commit hook now refuses such a commit. An agent can stall at the stream watchdog (PLUME-ALTITUDE did); `SendMessage` resumes it.
- **The machine:** builds by two agents plus the lead at once are slow; plan an hour as ~40 minutes of work. `target/agents/plume` (25 GB, game and sim) is the one warm copy for the next agent.
- **Demos:** fewer than 10 windows in a burst, one at a time under `target/demo.lock`. The booster's plumes sit behind the navball cluster: use `SUNSCATTER_DEMO_UI_SCALE=0.2 SUNSCATTER_DEMO_FAR=0.6` and crop with `sips`.
- The egui proportional font lacks ▲ ▼ ▸ →: use ⏶ ⏷ or test the glyph in a demo shot. egui areas default to 400 px tall: set the max height before a scroll area.
- **The disk:** follow `docs/process.md`'s disk hygiene (keep ≥ 30 GB free; the SessionStart hook prints it).
- `cargo test` stops at the first failing binary unless `--no-fail-fast`. The stop hook tests the shared tree, so an agent's unfinished work shows there.
- Saves are not compatible across versions (D063). The playtest save (version 14) embeds `CraftParams` and the ephemeris id; regenerate it from the demo's Orbit step (`orbit_save.ron`) when they change. `./run play`, then Cmd+L.
