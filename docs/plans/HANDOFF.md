# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-09-26 evening, during the realism-1 build (autonomous, with the owner checking in).*

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

- `main` is green in CI. realism-1 progress (details and checkboxes in `docs/features/realism-1.md`, **Build order**):
  1. **Visual fixes: done.** Navball rim/horizon flicker; dark horizon (Bevy's aerial LUT clamp: we install our own `render_sky.wgsl`, `game::sky::haze`); haze setting (1 = physical; measured roughly physical, owner to judge the default); physical ocean albedo; atmosphere radius follows the ellipsoid; zoom "whitish shapes" = one-frame terrain holes on LOD splits + sag rule skipped past the horizon + flare streak + glint on coarse triangles + orbit lines through the camera plane (all fixed; `SUNSCATTER_DEMO_ZOOM=<tier>` sweep); sub-sample terrain detail in `sim` (D059), rendered.
  2. **Foundation: done** (review sim 1–4, 11; game 1, 2, 5 partly, 7, 8). `GameCommand`/`InputContext` (`game::commands`), `VesselId` everywhere, one `Dominance` in `SimState`, trajectories of coast/burn segments with planned burns under warp.
  3. **Test craft: done in sim and game** (`sim::craft`, `sim::rigid`, `sim::contact`): files, adaptive cells, mass properties, engine with propellant, rigid body, debug mode (pause menu), rigid-body ground contact (tips at ~32–35°, impact per contact point, rests tilted), 3D model, propellant/Δv/TWR/mass readouts.
  4. **Relativity and light: done** except uncrewed probes. Proper time per vessel (GPS check +38.54 µs/day; ship clock in the flight panel). `sim::comms` (sites: Houston, DSN, Merritt Island, the pad's umbilical; link budget; occlusion; light time; relays) and `game::comms` (control locations: station = mission control; signals, retarded positions, last heard; commands in flight with light delay, `commands::InFlight`).
  5. **Aero/heating: design doc done** (`docs/design/aero-thermal.md`); rails floor done (D062). `sim::aero` and `sim::thermal` are being written by an agent as pure modules; integration into the vessel is next.
  6. **Flight UI: first versions done.** Burn planner (N): burns at Ap/Pe/+10 min, prograde/normal/radial Δv, sent with light delay; the line is drawn through every burn. Landing panel below 20 km and impact marker. Rendezvous: closest approach to the navball target (map marker, HUD line). Not done: node handles on the line, navball maneuver marker, intercept helper (Lambert + correction).
  7. **MCP: done for current tools.** `crates/mcp` (local HTTP JSON-RPC) + `game::agent` (get_state, list_bodies, get_vessel, get_trajectory, set_warp, switch_vessel, go_to_mission_control, set_controls, set_plan); `docs/mcp.md`; `SUNSCATTER_AGENT=<port>:<token>`.
- Perf (demo, 11 vessels): 333 fps at 1x, 131 fps at 1,000,000x.

## Known issues

- Open review findings: `docs/reviews/2026-09-26-code-review.md` (remaining: the per-frame line-end rescans, review game 4/6; saves format and validation, sim 7/8; Kepler edge cases, sim 9; small ones).
- Haze: physical by measurement (see realism-1 §1a); the owner decides the default of the new setting.
- The Moon's sub-sample detail is noise-like (no craters yet; data-only later).
- Terrain close up is still limited by the ~5 km colour map; a launch-site patch is the real fix.
- The test craft's chute (600 m²) lands at 23 m/s full / 10 m/s empty, above its 8 m/s impact limit: without debug mode a parachute landing is fatal (plan Q2 numbers).
- Aero is still an isotropic drag area until `sim::aero` is integrated; no heating yet.

## How to work

- Refactor as you touch: move each rule into a tested pure function first, then change the behaviour.
- Verify with the demo, offscreen (`SUNSCATTER_DEMO_OFFSCREEN=1` hides the window). One game window at a time; subagents never run the demo; `cargo -j 4` (process.md). **No branches, no worktrees**: agents work on `main` in the main checkout on non-overlapping files.
- At the end of a big session: fill in the plan or feature review, update this file, run the Windows check (`docs/windows.md`).
