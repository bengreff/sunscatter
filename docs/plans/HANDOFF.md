# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-30 (evening): F1–F3 landed; DOCS and MISSION done; two interviews recorded (D121–D127, D043/D034 revised); ONEKIND, MISSION-2 and DOCS-2 running.*

## Your first job

1. Check on the three agents:
   - ONEKIND (D121: delete the test craft, [brief-onekind.md](brief-onekind.md));
   - MISSION-2 (D122: fly the Falcon 9 through MCP and write the guide, [brief-mission.md](brief-mission.md));
   - DOCS-2 (the system docs to today's decisions, [brief-docs.md](brief-docs.md)).

   Agents wrap up at ~300k context.
2. Then launch the next wave from "What is next". The docs moved (D124): read the README's reading guide; `owners.md` replaced `architecture.md`.

## State

- **F3, the flight software: done** ([brief-flightsw.md](../archive/briefs/brief-flightsw.md); 994fad7, ccf251d). One layer, `sim::systems::flight`, whose only inputs are `Knowledge` and the computers' health. SAS, the holds, the launch commit, the estimates and the landing gate go through it, and the navball shows SAS NO REF / NO NAV / NO CPU. Left: SAS and the holds on coasts, the chutes' deploy altitude and the ullage burn (adaptive functions), the MCP tools (MISSION).
- **COAST (D119): done** ([brief-coast.md](../archive/briefs/brief-coast.md); dbe3f3b, 861157e). `sim::systems::coast_law`: on a coast the tanks follow closed-form phases, and the segment's mass equals the ship's at every lattice point.
- **FIXES2 (D120): done** ([brief-fixes2.md](../archive/briefs/brief-fixes2.md); f806401, 537e456 + 02dfb33, 57950ca). Δv in vacuum by default; an abort offered from what the piece holds (`program::offered`); Dragon's tanks no longer vent (MEOP 19.0 MPa, U).
- **QUICKFIX ([brief-quickfix.md](../archive/briefs/brief-quickfix.md)): done.** Launch abort shuts the Merlins and the MVac down (9886435). Dragon's tanks are at the published 2,400 psi, with a feed check (7424645). A pad abort puts a fresh vessel on the pad (e48a0fc). The stage editor locks the stages already run (3a4f502). parts.md is current (cd74409). The demo has `SUNSCATTER_DEMO_STOP_AFTER=PadAbort`. Both checked on screen.
- **F1, split and merge by construction: done** ([brief-foundation.md](../archive/briefs/brief-foundation.md)): `craft::partition` (4ed2658), `Vessel::piece` (2d9de38), and every piece keeps the whole program with a per-piece view, `craft::program` (e4718a0, 7a0f7f6). Seen on screen after the separation (the lead's demo, 2026-10-30). The upper stack's bar shows Dragon separation, SECO, Second stage ignition (6,600 m/s at 79.5 km) and Liftoff. The booster's shows "no stage left", Stage separation active, and MECO, Liftoff and Ignition with AGAIN. The booster's action bar still offers "Launch abort": D120 (FIXES2). Left for docking: a list of cabins joined by hatches (the brief's Left 2).
- **F2, one mass truth: done** ([brief-mass.md](../archive/briefs/brief-mass.md), 751d0da). A craft of parts' propellant is its ship state's, each liquid sits where its slug is, and the thermal nodes take each tank's contents. A coast keeps its starting mass for drag and radiation pressure (open below). The booster's flip (the centre of mass moving as the LOX is thrown up) is not in the demo, so it has not been seen on screen.
- **The fixes of 2026-10-28: done, seen on screen.** AGAIN on every stage already run (`stage_bar::rows::Place::Done`); a part's card sums its components by kind (`part_card::inside`); the plume leaves the lip at the bell's width (`exhaust::profile`); the action bar at the top right; the systems screen's two columns, no pop-ups. Not done, with reasons in the plan's "lead's findings": the hold for cards, bar and screen.
- **Sequence ([brief-sequence.md](../archive/briefs/brief-sequence.md)): all five bugs fixed and tested.**
- **Plume ([brief-plume-altitude.md](../archive/briefs/brief-plume-altitude.md)):** the exit temperature in shifting equilibrium (Merlin 1D 1,770 K, 1077a54) and a mixing layer that merges the nine Merlins ~2 m behind the bells (62e660e). Left: the grey cloud of cold soot at MECO (reheating where the fans meet), the shock beads in the merged max-Q plume.
- **D116, crew doses: decided, not built.** `systems::crew` still records injuries.
- **Fidelity review:** the parts/components/layers split is detailed enough; the weakness is coupling (much state drives nothing). Gaps left: SuperDraco thrust ignores feed pressure; no capsule–trunk joint; a solid ocean; no docking; the flight software reads the true state; failures are only flags.
- **New this session:** the lead hands off at ~400k (CLAUDE.md); a pre-commit hook refuses a commit from a stale index; `SUNSCATTER_DEMO_UI_SCALE` shrinks the interface for shots.
- CI: check `gh run list --limit 4` (the last pushes were still running at handoff).

## What is next

1. **Running:** ONEKIND, MISSION, DOCS (above).
2. **PLANNER, right after ONEKIND** (D126). Planned burns for craft of parts (`PlanError::PartsUnsupported`), run closed-loop to their target state as a flight-layer program: real starts, ullage settling, tail-off. Once the test craft goes, nothing else can plan a burn.
3. **Next wave, after ONEKIND** (each touches the whole of `sim`, so one at a time or split by subsystem):
   - **STATE** (D121): merge `ShipState` and `Runtime` into one state grouped by subsystem; typed handles inside, strings only at the edges (the 46 linear id searches); the engine rule in one place (review F8).
   - **FIXES3:**
     - the function runner through `program::offered`;
     - the load's vapour in `ShipState::initial`;
     - `estimate`'s `ready` against `program::can_start`;
     - an abort needs enough escape thrust (D123);
     - fail-safe shutdown when an engine's controller is lost (D123);
     - SAS and the holds on coasts through the flight layer.
   - **ADVANCE** (D121): one advance law per subsystem over any interval; the modes only choose the interval (closes F6).
   - **PERF** (D121):
     - live aero once per tick;
     - the live thermal network at its own rate;
     - analytic node relaxation on coasts;
     - geometry scans with a spatial index;
     - against the scale target: 5,000 parts live, 50 × 300 at 10⁶×.
   - **DEBRIS** (D121): the controllability check at separation; ballistic lumps; the planned pieces' designs at craft build; composed designs for unplanned pieces.
4. **Then the two flights end to end, in parallel** (D117, D123):
   - **Crew Dragon:** the capsule–trunk joint and the entry; a calm sea with water impact; D116's doses; docking with a rigid placeholder station, at three autonomy levels.
   - **The booster:** the vacuum relight; boostback and entry by a predictor–corrector; the landing burn by convex guidance; a landing zone first.
   - Every part is fully parameterised, and there is no ship-specific physics (D125). Ablation is one 1-D mechanism by material. Supersonic retropropulsion is thrust plus clean drag.
   - Automation is a switch: chutes, burns and guidance run automatic, on a button, or off, and manual control is always possible (D126).
   - Random failures stay off until both flights are clean (D123).
   - Each flight is checked by a mission test flown through MCP (D122).
   - v0.2 is complete with both flights plus D121's scale target (vision.md).
5. **Queued:**
   - Dragon's helium bottles (recommendation 5, with thrust following feed pressure);
   - flow separation in vacuum nozzles at sea level;
   - planned burns for craft of parts;
   - the Falcon 9 inconsistencies (the legs' mass, falcon9.md's U/E rows).
6. D115's fidelity after the flights:
   - sensors' noise only where needed;
   - the adaptive functions;
   - powered components' heat (revise D077 with it);
   - per-engine gimbals;
   - O₂ / N₂ tanks.
7. **Frozen** (D123): visuals (the plume's rest, GLOW, CLOUDS).
8. **Later roadmap:** the career as a startup (D027) and D118's components and factories.

## Open for the owner, new (defaults in place)

- Dragon's 19.0 MPa MEOP and relief (U, proposed: 115 % of the feed; FIXES2).
- On a coast, the tanks get orbit-averaged sunlight and their conditions are refreshed hourly (COAST). A direct write to a coasting vessel's tanks must go through a restart.
- Tanks between the live boundary and 150 km use the pad's air film against ~700 K thermosphere air (an existing problem, COAST).
- In roll, the liquid turns with its tank until the slosh model (MASS).
- The staging defaults: Liftoff on both pieces' bars, edits per piece, stage numbers per view (ROUNDTRIP).
- After a launch abort on the pad, the stack without its capsule stays; a pad abort of an inactive vessel waits until it is active (QUICKFIX).

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
