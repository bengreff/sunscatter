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
