# Brief: DOCS, the docs by system (2026-10-30)

D124. The layout:
- a reading guide in the README;
- `docs/vision.md` and `docs/decisions.md`;
- an owners table, and `process.md`;
- `docs/systems/`, one general doc per main system, left open to later ones (colonies, economics, construction, fusion, interstellar tech);
- `docs/technical/` for specific details;
- `docs/vehicles/` for research dossiers;
- `docs/archive/` for finished work;
- `docs/plans/` for live work and `HANDOFF.md`.

Also remove the 700-line file cap. Follow [agent-rules.md](agent-rules.md). Your name is DOCS. You need no target dir: run `cargo` only for the checks CI runs (`tools/check_decision_refs.sh`, and clippy only if you touch code). Two other agents run at the same time: **ONEKIND** ([brief-onekind.md](brief-onekind.md)) on `sim` and `game`, and **MISSION** ([brief-mission.md](../archive/briefs/brief-mission.md)) on the game's MCP tools, the demo and `CLAUDE.md`'s Commands. They will list the doc changes their work needs in their briefs' State: apply them when you reach those docs.

## The work, in order

1. **Moves first**, in one early commit so the others see the new paths:
   - Finished briefs, interviews, overnight logs, playtests and research notes go from `docs/plans/` to `docs/archive/`. A brief whose agent is still running stays (onekind, mission, docs).
   - Research dossiers (`features/falcon9.md`) go to `docs/vehicles/`.
   - Fix every link to a moved file, including `CLAUDE.md`, `HANDOFF.md`, `agent-rules.md` and code comments that cite doc paths.
2. **The file cap goes:** `tools/check_file_sizes.sh`, its CI step, the pre-push hook's call, and `CLAUDE.md`'s mentions.
3. **The owners table:** `architecture.md`'s who-owns-what table becomes `docs/owners.md`. Frame order and test layout go to their technical docs.
4. **`docs/systems/`:** one general doc per main system: what it does, its laws, its status words (rule 7), its owners and decisions. Write them from the existing `design/` and `features/` docs and the code, without inventing anything. A first list to adjust:
   - motion: frames, trajectories, warp, precision;
   - the world: bodies, ephemeris, air, weather, terrain;
   - the ship: parts, components, layers, state, mass;
   - propulsion: engines, plumbing, pressurant, RCS;
   - structure: joints, loads, separation, contact;
   - aero and thermal;
   - avionics: power, data, sensors, flight software, comms;
   - crew;
   - failures;
   - the game: rendering, interface, camera, MCP.

   Mark the open ones without writing them. Detailed material moves to `docs/technical/`, one topic per file. A `features/` or `design/` doc whose content has moved is deleted.
5. **The README's reading guide:** what to read first, for a person and for an agent, in order. Then update `CLAUDE.md`'s Read first section and `process.md` to the new layout.
6. Keep it honest: a system doc says what is built, using the status words, and points at the decisions. Keep `HANDOFF.md`'s SessionStart hook path working.

## State

(Update as you land: commits with hashes, what moved where, what is left in order.)

### Landed

1. **The moves** (ad727c3): `docs/archive/{briefs,interviews,plans,research}/`; `docs/vehicles/falcon9.md`; `docs/technical/` (the design docs, the feature specs, `mcp.md`, `windows.md`); `architecture.md` → `owners.md`; `docs/VISION.md` tracked as `docs/vision.md`. Every link and code-comment path fixed (script: relink by resolved target; a checker confirms every relative link in `docs/`, `README.md`, `CLAUDE.md` resolves). `tools/check_decision_refs.sh` skips `docs/archive/` as it skips the plans. Feature plans that were built (realism-1, playtest-1, map-view-lighting-controls) went to `archive/plans/`.
2. **The file cap** (87aa10f): the script, its CI step, the pre-push call, CLAUDE.md's mentions.
3. **owners.md** (54be1f7): the rows verbatim, grouped by system; the `mcp` crate and the agent's row added; `technical/frame-order.md`, `technical/testing.md`.
4. **docs/systems/** (8520a0a): a README index (status words as used, the written systems, the open ones: the career, economics and logistics, construction, colonies, advanced propulsion, interstellar, ship scripting, radiation) and ten docs: motion, world, ship, propulsion, structure, aero-thermal, avionics, crew, failures, game. Each: what it does, laws, what exists with its status word, not built, owners (its `owners.md` section), decisions, technical docs. Written from the technical docs, HANDOFF and decisions.md only.
5. **The reading guide** (b1c64d9): README (the v0.2 row from what is built, the keys from `commands/keys.rs`, the guide for a person and for an agent, the layout); CLAUDE.md's Read first; process.md's document kinds (D124), D122's mission tests, no file cap, agents' commits and the demo lock as agent-rules says; agent-rules' "docs in the same round" and `/handoff`'s homes.
6. **DOCS-2: the systems docs to today's decisions** (67bf95d): D121 (one kind of craft under way, one state, one advance law and lumps not built), D122 (the testing tools built, the demo and tour gone), D125 (the laws, and its gaps: one antenna constant, SuperDraco thrust, ablation not built, joints without capacity data), D126 (automation switch, closed-loop burns, warp to a point: not built; no craft of parts can plan a burn), D127 (one worker per vessel and per bubble, 20 ms ticks; cross-machine hashes cover coasts, not live flight), D043 (the layer built, native; WASM and the budget design), D034 (one mission control; commands still leave from the player's location). `systems/README.md` gained "Across every system". D123's random-failure switch is noted as not built (fates are still seeded).
7. **DOCS-2: MISSION's and ONEKIND's doc changes** (5a930dc): mcp.md (session tools, developer flag, Knowledge, the headless game); owners rows for `game::offscreen`, `game::agent::session`, `craft::small`, `set_fill`, the saved landed-pose check, the tour and demo mentions gone; agent-rules' game line; HANDOFF's Gotchas; process.md (checks, Windows, D127's hand-measured performance); windows.md's headless procedure (not yet run on backhouse).
8. **DOCS-2: ship-systems.md split** (ae6bf93) into `technical/{ship-commands, plumbing, engines, mechanisms, knowledge, flight-software, joint-loads, failure-modes, power, life-support}.md`, each pointing at its system; stale lines fixed (the agent reads Knowledge, SAS on coasts is FIXES3, D043).
9. **DOCS-2: specs out of the archive, first two** (a068d59): `technical/comms.md` (realism-1 §4b–§4c, checked against the code) and `technical/proper-time.md` (§4a, with TDB's L_B; motion-model.md's formula corrected).

10. **DOCS-3: the specs left in archived plans, as technical docs** (5e1bdd6, 75be157), each checked against the code: `attitude-holds.md` (playtest-1 B: the modes, SAS's controller, the slew caps, the turn lead), `landing-prediction.md` (playtest-1 C, realism-1 §6b), `rendezvous.md` (realism-1 §6c; the intercept helper a *skeleton*: no N-body correction), `map-view.md` (map-view plan §2, §5), `camera.md` (§1, §7), `lighting.md` (§3 as D055, D087, D110 revised it). The pause menu and settings (§6) are mapped in `interface-map.md` (INTERFACE's), so no new doc. `game::map_view`'s comment, D054's details, `systems/game.md`, `motion.md` (also: the Lambert intercept helper exists), `avionics.md` and `landing-guidance.md` point at them; the navball's display of a planned burn's controls (b246f9d) in `attitude-holds.md` and `planner.md`.
11. **DOCS-3: stale status lines** (c31ecaf, 0aa89de): `solar-visuals.md` (*playable*, frozen by D123; the tour's section says it is deleted and what replaced it), `parts.md` (*playable*), `scale-performance.md`'s status and anchor, `reentry.md` item 3, `crisp-surfaces.md` and `staging-bar.md` (seen on screen in the README's shots). `exhaust.md`'s and `systems-screen.md`'s "seen in the demo" kept as history.
12. **DOCS-3: the README's images** (d651286): two shots from the headless game through MCP, built from an export of HEAD 8cc38b4 in the scratchpad (not a worktree), so other agents' unfinished files were not compiled: the Falcon 9 on the pad in the flight view, and the second stage's 412 × 343 km orbit after PLANNER-4's two planned burns, in the map view with the Orbital menu (PLANNER-4's save). In `docs/images/v0.2/`; the old two stay in `v0.2-visual/` (an archived plan cites them). The v0.2 row reaches orbit.
13. **DOCS-3: the finished briefs archived** (9815184): `brief-planner.md`, `brief-audit.md`, `brief-research.md`, `brief-mission.md` to `archive/briefs/`, every link relinked by its resolved target (the running agents' briefs, the workstream table, HANDOFF, `game.md`, `planner.md`); `game.md`'s mission-test lines say flown.

### DOCS-3 (2026-10-31)

You continue DOCS-2's Left below. Your name is DOCS-3; you build nothing and need no target dir. Running alongside: ONEKIND-7 (deleting the test craft and the made-up path; **the test-craft mentions in the docs are its, so skip Left 3**) and INTERFACE-3 (the game's interface; `interface-map.md`, `systems-screen.md` and `flight-menus.md` are its). Since DOCS-2: PLANNER wrote `technical/planner.md` (the burn planner's spec is done: skip it in Left 1); MISSION's guide is in `testing.md` (Left 4 is done). Work only in docs, commit through a private index, and push.

Order: Left 1 (except the planner), Left 2, Left 5 (the README's images: shots from a headless game through MCP, `testing.md`; one game at a time under the lock), Left 6 (try `ssh backhouse`; if it does not connect in a minute, skip it and say so). Then archive the finished briefs: `brief-planner.md`, `brief-audit.md`, `brief-research.md` and `brief-mission.md` go to `docs/archive/briefs/`, with every link to them updated (`grep -rn` the docs). Do that last: the running agents' briefs link to them, so update those links in the same commit.

### Left, in order (for the lead or a fresh agent)

Done by DOCS-3: Left 1, 2, 4, 5 and the archive. Left:

1. **ONEKIND's doc changes** (its State lists them; the test-craft mentions are ONEKIND-7's own). Its step (c) edits `landing-prediction.md` (the made-up engine's fallback, `LandingStart.mass`), which it held uncommitted when DOCS-3 stopped.
2. **`windows.md`'s headless procedure** on backhouse, and `run_demo.cmd` there: backhouse did not connect (2026-10-30, 23:3x: port 22 timed out).
3. `tools/gpu_trace.py` still launches the plunge demo (MISSION's Left 4; a tool, not a doc: it needs `./run headless` and a `profile` call, or deleting).
4. Archive `brief-docs.md` itself when the lead is done with it.

### Open for the owner

- The status words had no written definitions; `systems/README.md` gives the readings these docs use (design: written, not built; skeleton: built and tested, not doing the whole job; playable: does its job under tests; polished: unused).
- `docs/archive/` is split into `briefs/`, `interviews/`, `plans/` and `research/` (D124 names only `archive/`).
- `docs/plans/playtest-2026-10-01-orbit.ron` stays in `plans/` (`./run play` and `sim/tests/save_load.rs` read it); ONEKIND regenerates or drops it (it carries the test craft).
- `plans/flight-ui-2026-10-27.md` (the workstream table agents update) and `plans/open-issues.md` (the backlog) stay in `plans/` as live work, under their old names, so the running agents' references hold.
- The frozen GLOW and CLOUDS briefs were archived with the finished ones (D123: visuals frozen); a fresh brief starts from them when the freeze lifts.
- **D034's routing** (DOCS-2): the docs say commands from a crewed vessel to a probe still leave from the crewed vessel, against D034's revision ("a crewed ship nearby does not command probes"). Recorded as *not built*; no code changed.
- **D123's random failures off** (DOCS-2): no switch exists and the game still seeds fates for every vessel. Recorded as *not built* in `systems/failures.md`.
- **The game lock** keeps its name `target/demo.lock` though the demo is gone (agents and CLAUDE.md use it); renaming it is the lead's call.
- **The pause menu and settings** (map-view plan §6) have no technical doc of their own: `interface-map.md` maps every screen and way in, and a settings spec would repeat it (DOCS-3).
- **A red translucent shape** at the second stage's forward end in orbit after Dragon's separation (PLANNER-4's save loaded headless; shot `scratchpad/docs3/shots/orbit.png`): for the lead to look at; not in the README.
