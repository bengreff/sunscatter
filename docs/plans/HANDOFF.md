# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-09-26, after the fix round, the owner's second review, a full code review and the owner interview for the next chunk.*

## Your first job: write the feature plan from the owner interview

The owner was interviewed at the end of the last session; do **not** re-run the interview. Everything is in `docs/plans/interview-2026-09-26.md` (bugs in the owner's words, theme, test craft, scope, order) and decisions **D058–D063**.

1. **Read** the docs below, the interview record, D058–D063, and `docs/reviews/2026-09-26-code-review.md`.
2. **Write the feature plan** `docs/features/realism-1.md` (or split into a few docs if it gets long), in the style of `map-view-lighting-controls.md`: scope, the owner's decisions folded in, per-item design with **the data model first, then the UI** (D060), pure functions and tests, performance budgets, build order, open questions marked **Q**. It covers, in this order (D060):
   1. Visual fixes under D058 (simplify what does not work): haze tuning + setting, procedural sub-sample terrain in `sim` (D059), the flashing when zooming (whitish shapes), the dark horizon at certain angles, the navball rim and Time to Ap flicker when landed.
   2. Foundation pass: the high-severity review items.
   3. The test craft's data model (one part: one temperature, one state, fuel/thrust/Isp, gear geometry; crewed, minimal resources).
   4. Relativity and light (proper time; light delay for telemetry and ground commands, D063).
   5. Aero, heating and hitbox on cells of the ship's 3D model (D061), and rails warp disabled below a per-body altitude (D062).
   6. UI: burn planner (maneuver nodes, burns under warp), powered Moon landing, rendezvous tools.
   7. The MCP server on a shared command API (D063).
   Research first where the problem is hard (process.md: KSP mods such as FAR and Deadly Reentry for aero/heating, Principia for flight plans, Persistent Thrust for burns under warp; papers for entry heating).
3. **Ask the owner only what the interview left open** (use `AskUserQuestion`), fold the answers in, get the plan approved, commit it, then build it.

## Read, in this order

1. `CLAUDE.md`: rules and commands.
2. `docs/vision.md`: pillars and non-goals.
3. `docs/process.md`: how we work (docs, tests, agents, shared machine, context resets); `docs/windows.md` for the Windows runs.
4. `docs/architecture.md`: who owns what. Check it before writing any new function.
5. `docs/decisions.md`: skim all of it; read D045–D057 closely.
6. `docs/features/map-view-lighting-controls.md`: the round just finished; its **Review** section has the status per item.
7. `docs/reviews/2026-09-26-code-review.md`: 30 verified findings (sim and game) with status.
8. `docs/plans/interview-2026-09-26.md`: **the owner's answers for the next chunk.**
9. Only when touching motion or physics: `docs/design/motion-model.md`.

## State

- `main` is green in CI (macOS and Windows). The game also builds, passes the sim tests (bit-exact goldens) and runs the full demo on the owner's Windows PC (`docs/windows.md`).
- Built in the fix round: per-object map view (D054), physical lighting per body with eclipses and planetshine (D055, **fixed exposure**: eye adaptation was built and removed on review), orbit lines of one revolution about the dominant body (D056), the Esc pause menu and tabbed settings, movable saved panels, navball, full-screen tracking station, camera collision, vessels drawn at the clock, geomorphing, water mask.
- After the owner's second review (this session's end):
  - Night sides are dark: Bevy's derived sky light leaked after leaving the atmosphere, and the terrain now fades sky light through twilight.
  - Terrain overhaul: CC0 ground textures (grass, soil, sand, rock, snow) chosen per fragment, patchy cover, beach band, animated water waves. The pad moved ~6 km west onto land (LC-39A's barrier island is below the maps' resolution).
  - Navball: no apsis times or velocity markers while landed (the flicker). Saves get a default name. The double-click body menu works in the station. Flight keys are off in the station; F/`/Tab off while typing. Settings tabs no longer mark settings changed every frame.
- Demo views added: `pad_top` (straight down from 300 m), `earth_night`, `moon_night`, the 10 km coast view.

## Known issues

- All open review findings: `docs/reviews/2026-09-26-code-review.md` (notably: tracked vessels' orbit lines are stubs; `nearest_body` vs `Dominance`; per-frame recomputation of line ends and Dominance; UI mutating the sim after the scene is placed; fleet-index vessel identity; sim NaN hang and no ephemeris-end guard; attitude control ignored while coasting at 60 fps).
- Haze at 10–20 km looks strong (the ground fully blue). Not the aerial-perspective range; atmosphere data matches Hillaire. Suspect the colour map's darkness as albedo.
- Terrain close up is still limited by the ~5 km colour map; a launch-site patch is the real fix.
- Shader compile errors are only logged (a reserved word silently removed all terrain once): the demo should fail on pipeline errors.

## How to work

- Refactor as you touch: move each rule into a tested pure function first, then change the behaviour.
- Verify with the demo, offscreen (`SUNSCATTER_DEMO_OFFSCREEN=1` hides the window). One game window at a time; subagents never run the demo; `cargo -j 4` (process.md). **No branches, no worktrees**: agents work on `main` in the main checkout on non-overlapping files.
- At the end of a big session: fill in the plan or feature review, update this file, run the Windows check (`docs/windows.md`).
