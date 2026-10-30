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

Also remove the 700-line file cap. Follow [agent-rules.md](agent-rules.md). Your name is DOCS. You need no target dir: run `cargo` only for the checks CI runs (`tools/check_decision_refs.sh`, and clippy only if you touch code). Two other agents run at the same time: **ONEKIND** ([brief-onekind.md](brief-onekind.md)) on `sim` and `game`, and **MISSION** ([brief-mission.md](brief-mission.md)) on the game's MCP tools, the demo and `CLAUDE.md`'s Commands. They will list the doc changes their work needs in their briefs' State: apply them when you reach those docs.

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

### Left, in order

4. `docs/systems/` (a README index plus one doc per system), from the technical docs and the code.
5. The README's reading guide; CLAUDE.md's Read first; process.md's document types.

### Open for the owner

- `docs/plans/playtest-2026-10-01-orbit.ron` stays in `plans/` (`./run play` and `sim/tests/save_load.rs` read it); ONEKIND regenerates or drops it (it carries the test craft).
- `plans/flight-ui-2026-10-27.md` (the workstream table agents update) and `plans/open-issues.md` (the backlog) stay in `plans/` as live work, under their old names, so the running agents' references hold.
- The frozen GLOW and CLOUDS briefs were archived with the finished ones (D123: visuals frozen); a fresh brief starts from them when the freeze lifts.
