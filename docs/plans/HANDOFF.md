# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-09-27 morning, after the realism-1 build (autonomous overnight, with the owner checking in).*

## Current job: build `docs/features/realism-1.md` in its build order

The plan was written 2026-09-26 from the owner interview (`docs/plans/interview-2026-09-26.md`) and the owner's answers the same evening (D064–D069). The owner allowed building in plan order before reviewing it. Check items off in the plan's **Build order** as they land; open defaults are marked **Q**.

## Read, in this order

1. `CLAUDE.md`: rules and commands.
2. `docs/vision.md`: pillars and non-goals.
3. `docs/process.md`: how we work (docs, tests, agents, shared machine, context resets); `docs/windows.md` for the Windows runs.
4. `docs/architecture.md`: who owns what. Check it before writing any new function.
5. `docs/decisions.md`: skim all of it; read D045–D069 closely.
6. `docs/features/map-view-lighting-controls.md`: the round just finished; its **Review** section has the status per item.
7. `docs/reviews/2026-09-26-code-review.md`: 30 verified findings (sim and game) with status.
8. `docs/plans/interview-2026-09-26.md`: the owner's answers for this chunk.
9. `docs/features/realism-1.md`: **the current plan.**
10. Only when touching motion or physics: `docs/design/motion-model.md`.

## State

- `main` is green in CI. **realism-1 is built through all seven items** (checkboxes and notes in `docs/features/realism-1.md`, **Build order**; nothing has been played by the owner yet):
  1. **Visual fixes:** navball flicker; dark horizon (our `render_sky.wgsl`, `game::sky::haze`); haze setting (1 = physical; owner to judge); physical ocean albedo; zoom flashing (terrain holes on LOD splits, horizon sag, flare streak, glint on coarse triangles, lines through the camera); sub-sample terrain detail in `sim` (D059). The demo fails on shader compile errors.
  2. **Foundation:** review sim 1–4, 10, 11 and game 1, 2, 7, 8, 9, 12, 16 fixed (5 partly); `GameCommand`/`InputContext`, `VesselId`, one `Dominance` in `SimState`.
  3. **Test craft:** `sim::craft` (files, adaptive cells, mass properties, engine, render mesh), `sim::rigid`, `sim::contact` (rigid-body ground contact, tips at ~32–35°, rests tilted), debug mode (pause menu), 3D model and readouts.
  4. **Relativity and light:** proper time (GPS +38.54 µs/day; ship clock shown); `sim::comms` + `game::comms` (control locations, DSN + Merritt Island + pad umbilical, occlusion, relays, light delay, retarded positions, commands in flight); uncrewed probes (F2), whose controls arrive with the light delay.
  5. **Aero and heat (D061, D065 revised, D070):** `sim::aero` (cell bake, modified Newtonian with real-gas Cp,max, subsonic/transonic/supersonic with wave drag from A(x), slender-body and fin lift, skin friction, free-molecular with the Wilmoth bridge, US 1976 atmosphere), `sim::thermal` (skin cells + interior volume nodes, Sutton–Graves + Tauber–Sutton, sunlight with eclipses, engine heat, implicit solve), integrated in live ticks and coasts; destruction by overheat or impact (not in debug mode). Rails floor (D062). Limitations table in `docs/design/aero-thermal.md`.
  6. **Flight UI:** burn planner (N; Ap/Pe/+10 min/click on the line/intercept via Lambert first guess; sent with light delay; line through the burns; navball maneuver marker), landing panel and impact marker, rendezvous (closest approach), skin and interior temperatures.
  7. **MCP:** `crates/mcp` + `game::agent` (11 tools incl. set_plan, landing prediction, closest approaches); `docs/mcp.md`.
- Perf (demo, 11 vessels, measured by the perf agent 2026-09-27): 294 fps at 1x, 307 at 1000x, 211 at 1,000,000x, 0 compute-limited frames.
- Windows check: `backhouse` unreachable on 2026-09-27; last Windows run was before this chunk (docs/windows.md).

## Known issues

- **Owner decisions pending:** haze default (1 = physical); the test craft's chute (lands at 23 m/s full / 10 m/s empty, over its 8 m/s limit: parachute landings are fatal outside debug mode); the engine's heat fraction (2e-4, regenerative cooling assumed).
- The test craft is aerodynamically unstable nose-first (trims ~130° from nose-first hypersonic); an entry from orbit burns it up at the engine bell without debug mode (no heat shield, D064).
- Coast skin temperatures are orbit averages (the eclipse swing only shows in live flight).
- Some aero constants were written from memory by the agents and are unverified: the Wilmoth bridge form, Tauber–Sutton half-km Earth table points and the Mars table, the real-gas γ_eff curve (±10 %). See the limitations table.
- Open review items: game 4/6 (line-end rescans; the game-side 1x cost is `Dominance::of`, `vessel_line_end`, navball), sim 5–9, 12–13, game 10, 11, 14, 15, 17.
- The Moon's sub-sample detail is noise-like (no craters); terrain close up limited by the ~5 km colour map.
- `SUNSCATTER_DEMO_STOP_AFTER=Perf` does not stop (the step is `Perf(n)`).

## How to work

- Refactor as you touch: move each rule into a tested pure function first, then change the behaviour.
- Verify with the demo, offscreen (`SUNSCATTER_DEMO_OFFSCREEN=1` hides the window). One game window at a time; subagents never run the demo; `cargo -j 4` (process.md). **No branches, no worktrees**: agents work on `main` in the main checkout on non-overlapping files.
- At the end of a big session: fill in the plan or feature review, update this file, run the Windows check (`docs/windows.md`).
