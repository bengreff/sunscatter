# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-09-26, after the fix round, the owner's second review (night sides, terrain, navball, saves, station) and a full code review.*

## Your first job: interview the owner, then write a feature plan

Do **not** start coding. The fix round is done; the next chunk of work is not chosen yet.

1. **Read** the docs below (quickly: you need the vocabulary, not every detail), then `docs/reviews/2026-09-26-code-review.md`.
2. **Interview the owner** with `AskUserQuestion` (a few questions per call, options with short descriptions, your recommendation first). Nail down, in this order:
   1. **Feedback on the current build**: night sides, the new terrain textures, the launch site, the GUI, anything broken. Ask for a quicksave (F5) for anything visual that is wrong.
   2. **Open decisions** that block features: the kind of organization the player leads (D027); whether saves must stay compatible from now on (review sim #7); whether a high-resolution launch-site data patch (imagery/heights around KSC) is wanted; the ship model for the next step (blocks vs predefined ships, D036/D040); anything in `docs/decisions.md` "Open questions".
   3. **Which features to build now.** No full aerodynamics model yet (the owner: "that will be VERY complex"). Candidates to offer, with your recommendation:
      - **Burn planner**: maneuver nodes on the N-body trajectory, burns executed under warp (D011, D029; study Principia's flight plans and Persistent Thrust).
      - **Finite burns, propellant and mass**: trajectory as segments (coast / burn with a thrust law and mass flow), sim review #11. Probably a prerequisite for the planner.
      - **Predefined ships / staging** (v0.2's "blocks with made-up thrust first, then predefined ships").
      - **Rendezvous and targeting tools**: relative velocity, closest approach, target markers (the navball already has target mode).
      - **Moon landing under power** (landing legs, suicide-burn readouts), since parachutes only work on Earth.
      - **Launch-site patch**: high-resolution heights/imagery around the pad.
      - **Ship systems view** skeleton (D057).
      - **Scripting (WASM, D043) or the MCP server (D044)**: a first cut.
      - **Foundation work from the review** (recommend bundling the high-severity items with whichever feature needs them): stable `VesselId` (saved), a `GameCommand` message applied in `Stage::Input`, one per-frame `ActiveFlight` / `VesselLines` resource, an `InputContext`, sim robustness (NaN hang, ephemeris-end guard, attitude on the tick lattice, gravity cutoff re-evaluation).
3. **Write a feature plan** for what the owner picks: `docs/features/<name>.md` in the style of `map-view-lighting-controls.md` (scope, decisions with the owner's answers folded in, per-item design, pure functions and tests, performance budget, build order, open questions marked **Q**). Record decisions in `docs/decisions.md` (delete superseded ones, rule 9). Get the owner's approval of the plan, commit it, and only then build.

## Read, in this order

1. `CLAUDE.md`: rules and commands.
2. `docs/vision.md`: pillars and non-goals.
3. `docs/process.md`: how we work (docs, tests, agents, shared machine, context resets); `docs/windows.md` for the Windows runs.
4. `docs/architecture.md`: who owns what. Check it before writing any new function.
5. `docs/decisions.md`: skim all of it; read D045–D057 closely.
6. `docs/features/map-view-lighting-controls.md`: the round just finished; its **Review** section has the status per item.
7. `docs/reviews/2026-09-26-code-review.md`: 30 verified findings (sim and game) with status.
8. Only when touching motion or physics: `docs/design/motion-model.md`.

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
- Verify with the demo, offscreen (`SUNSCATTER_DEMO_OFFSCREEN=1` hides the window). One game window at a time; subagents never run the demo; `cargo -j 4` (process.md). Close agents and delete their worktrees as soon as their work is merged.
- At the end of a big session: fill in the plan or feature review, update this file, run the Windows check (`docs/windows.md`).
