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
4. **docs/systems/** (8520a0a): a README index (status words as used, the written systems, the open ones: the career, economics and logistics, construction, colonies, advanced propulsion, interstellar, ship scripting, radiation) and ten docs: motion, world, ship, propulsion, structure, aero-thermal, avionics, crew, failures, game. Each: what it does, laws, what exists with its status word, not built, owners (its `owners.md` section), decisions, technical docs. Written from the technical docs, HANDOFF and decisions.md only.
5. **The reading guide** (b1c64d9): README (the v0.2 row from what is built, the keys from `commands/keys.rs`, the guide for a person and for an agent, the layout); CLAUDE.md's Read first; process.md's document kinds (D124), D122's mission tests, no file cap, agents' commits and the demo lock as agent-rules says; agent-rules' "docs in the same round" and `/handoff`'s homes.

### Left, in order (for the lead or a fresh agent)

1. **ONEKIND's and MISSION's doc changes** when their States list them (none listed by 2026-10-30 afternoon). Expected: the technical docs' and systems docs' mentions of the test craft (aero-thermal.md's opening, parts.md, ship-systems.md, limbo.md, the systems docs' "being deleted"); MISSION's mission-test guide goes in `technical/testing.md` (its Mission tests row and the End to end row), and CLAUDE.md's Commands are MISSION's.
2. **Split `technical/ship-systems.md`** by subsystem (commands and the pad, plumbing, propulsion, mechanisms, information, flight software, structure, failures, power, crew), one topic per file, and point the systems docs at the new files.
3. **Extract the specs that live only in archived plans** into technical docs: the comm network, proper time and control locations (`archive/plans/realism-1.md` §4), the burn planner (§6), the map view, lighting, orbit lines and settings (`archive/plans/map-view-lighting-controls.md`), attitude holds and landing prediction (`archive/plans/playtest-1.md` B, C).
4. Old status lines in technical docs that may be stale (crisp-surfaces.md "not yet seen on screen", solar-visuals.md "in progress", parts.md "skeleton", staging-bar.md "not yet seen"): check against the code and the shots and update.
5. The README's two images are from before the one look (D073): replace them with shots from MISSION's tools.

### Open for the owner

- The status words had no written definitions; `systems/README.md` gives the readings these docs use (design: written, not built; skeleton: built and tested, not doing the whole job; playable: does its job under tests; polished: unused).
- `docs/archive/` is split into `briefs/`, `interviews/`, `plans/` and `research/` (D124 names only `archive/`).
- `docs/plans/playtest-2026-10-01-orbit.ron` stays in `plans/` (`./run play` and `sim/tests/save_load.rs` read it); ONEKIND regenerates or drops it (it carries the test craft).
- `plans/flight-ui-2026-10-27.md` (the workstream table agents update) and `plans/open-issues.md` (the backlog) stay in `plans/` as live work, under their old names, so the running agents' references hold.
- The frozen GLOW and CLOUDS briefs were archived with the finished ones (D123: visuals frozen); a fresh brief starts from them when the freeze lifts.
