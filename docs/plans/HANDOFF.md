# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-29 (evening): the fix list done, the SEQUENCE and PLUME-ALTITUDE agents landed, the FIDELITY review written. Nothing running.*

## Your first job

1. Get the owner's answers. These were asked at the end of the session; the answers may be in the first message:
   - Should the fidelity review's three foundations ([fidelity-review-2026-10-29.md](fidelity-review-2026-10-29.md) §4) come before D115's fidelity work? The lead recommends yes.
   - Plume: soot 0.5 % for the main nozzle (Ross's 2 % includes the gas generator's exhaust, which the Merlin vents through its own pipe)? The exit temperature from the agent's CEA-method runs until CRUCIBLE gives each engine's figure?
   - SEQUENCE's defaults ([fixes-2026-10-28.md](fixes-2026-10-28.md), Open for the owner): after a pad abort Liftoff is next again; an edit to a stage already run is refused.
   - The parachute's q / DESIGN q flicker on the pad: is it still seen? It has not been reproduced since f57da59.
2. Then work through "What is next" in the order the answers give.

## State

- **The fixes of 2026-10-28: done, seen on screen.** AGAIN on every stage already run (`stage_bar::rows::Place::Done`); a part's card sums its components by kind (`part_card::inside`); the plume leaves the lip at the bell's width (`exhaust::profile`); the action bar at the top right; the systems screen's two columns, no pop-ups. Not done, with reasons in the plan's "lead's findings": the hold for cards, bar and screen.
- **Sequence ([brief-sequence.md](brief-sequence.md)): all five bugs fixed and tested.** The systems screen's stage editor still lets a draft edit stages already run; saving it gets a refusal toast. Lock those stages in the editor.
- **Plume ([brief-plume-altitude.md](brief-plume-altitude.md)):** the exit temperature in shifting equilibrium (Merlin 1D 1,770 K, 1077a54) and a mixing layer that merges the nine Merlins ~2 m behind the bells (62e660e). Left: the grey cloud of cold soot at MECO (reheating where the fans meet), the shock beads in the merged max-Q plume.
- **D116, crew doses: decided, not built.** `systems::crew` still records injuries.
- **Fidelity review:** the parts/components/layers split is detailed enough; the weakness is coupling (much state drives nothing). Worst gaps: "Launch abort" leaves the Merlins running; SuperDraco thrust ignores feed pressure (Dragon's tanks at 1.7 MPa against a 6.9 MPa chamber); no capsule–trunk joint; a solid ocean; no docking; the flight software reads the true state; failures are only flags; each tank's liquid stays at its base.
- **New this session:** the lead hands off at ~400k (CLAUDE.md); a pre-commit hook refuses a commit from a stale index; `SUNSCATTER_DEMO_UI_SCALE` shrinks the interface for shots.
- CI: check `gh run list --limit 4` (the last pushes were still running at handoff).

## What is next

1. The owner's answers (above).
2. Quick fixes:
   - Launch abort also shuts the Merlins down.
   - Dragon's tank loading against the SuperDraco's chamber pressure (D115: published wins).
   - Lock the stages already run in the stage editor.
   - `docs/features/parts.md`'s "Not done" list is out of date.
3. If approved, the foundations (review §4):
   - split and merge vessel state by construction (fields are still copied whole: crew, buses, gimbal direction, pressure flag);
   - one mass truth (each tank's liquid position feeds the mass model);
   - the flight software reads what the sensors know.
4. The interview's queue: planned burns for craft of parts (`PlanError::PartsUnsupported`); the booster after MECO (no vacuum relight; separation inside 1 s of MECO); Falcon 9 inconsistencies (the legs' 173 kg against ~500 kg published; the U/E rows of `falcon9.md`).
5. D115's fidelity: sensors, adaptive functions, powered components' heat (revises D077: update it and its citations in the same commit), per-engine gimbals, O₂ / N₂ tanks, noise only where needed.
6. The plume's rest; D116's doses; GLOW, CLOUDS; the Crew mission (docking first).

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
