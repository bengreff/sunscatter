# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-30 (night). Three agents are running: ONEKIND-2, MISSION-2 and DOCS-2.*

## Your first job

1. Take the three agents' reports as they arrive. Each reports through a message with its commits, what is left and its questions. Their briefs hold their State.
   - **ONEKIND-2** ([brief-onekind.md](brief-onekind.md)) moves the ~55 remaining test files off the test craft onto `craft::small` or the Falcon 9, and rebuilds `craft::stress` on parts. It does **not** delete the made-up path. The lead decided that comes only after PLANNER, so the game never loses planned burns.
   - **MISSION-2** ([brief-mission.md](brief-mission.md)) flies the Falcon 9 through stage separation in a headless game via MCP, then writes the mission-test guide in `docs/technical/testing.md`.
   - **DOCS-2** ([brief-docs.md](brief-docs.md)) brings `docs/systems/` up to D121–D127 and applies MISSION's listed doc changes.
2. When a slot frees (cap: 3 agents plus the lead), launch **PLANNER**: next steps, item 1.

## State

- **Landed 29–30 Oct.** Each brief is in `docs/archive/briefs/`:
  - F1, split and merge by construction (`craft::partition`, `Vessel::piece`, `craft::program`);
  - F2, one mass truth (`vessel::mass`);
  - F3, the flight-software layer (`sim::systems::flight`);
  - D119's coast law (`systems::coast_law`);
  - D120: Δv in vacuum, an abort only where it saves;
  - QUICKFIX's five fixes;
  - DOCS: the docs reorganised by system (D124), `owners.md` replacing `architecture.md`, no file cap;
  - MISSION: `./run headless`, the testing tools, the demo and tour deleted.
- **Decisions recorded:** D117–D127, with D027, D034, D043, D044, D081, D114, D115 and D123 revised. The interviews are in `docs/archive/interviews/` (2026-10-29-next, 2026-10-30). v0.2 is complete with both flights plus D121's scale target (`vision.md`).
- **Half done:**
  - ONEKIND: `craft::small` (`small-rocket`) exists and six suites have moved. The test craft and the made-up path are still in the code. The owner confirmed a hard impact breaking a craft of parts apart is intended.
  - MISSION: `step`, `run_until`, `new_flight`, saves, `bench` and `profile` have not been run in the game yet.
- **CI:** green through 8ece779; the agents' last pushes were queued at handoff (`gh run list --limit 3`).

## Next steps

1. **PLANNER** (D126): planned burns for craft of parts (`PlanError::PartsUnsupported`). Closed-loop to the target state, as a flight-layer program, with real starts, ullage settling and tail-off. Move the planned-burn tests (`planned_burns`, `hold_modes`' maneuver holds, the game's planner and predictions, `landing::model`) onto parts.
2. **ONEKIND-3**, after PLANNER: delete the made-up path and the test craft. Fix D074, D064, D060, D078, D079, D100 and D104 and their citations. Regenerate the playtest save from a Falcon 9 flight (`save_game` in a headless run) or drop it.
3. **The next wave, one sim-wide agent at a time** (D121):
   - **STATE:** merge `ShipState` and `Runtime`; typed handles; the engine rule in one place.
   - **FIXES3:**
     - the function runner through `program::offered`;
     - the load's vapour in `ShipState::initial`;
     - `estimate::ready` against `program::can_start`;
     - an abort needs enough escape thrust;
     - fail-safe shutdown for a lost controller (D123);
     - SAS and the holds on coasts through the flight layer.
   - **ADVANCE:** one advance law per subsystem.
   - **PERF:** aero once per tick; thermal at its own rate; analytic coast heat; a spatial index for the geometry scans; per-vessel threads with bit-identical cross-machine tests (D127).
   - **DEBRIS:** the controllability check; ballistic lumps; designs precomputed at build.
4. **The two flights, in parallel** (D117, D123, D125, D126), each checked by an MCP mission test (D122):
   - **Crew Dragon:** the trunk joint, entry with 1-D ablation by material, the chutes' real logic as the automatic option, a calm sea, D116's doses, a rigid placeholder station.
   - **The booster:** relight, boostback and entry by predictor–corrector, convex landing guidance written as WASM-ready flight-layer code (D043), a landing zone first.
5. **Queued:** Dragon's helium bottles; nozzle flow separation; the Falcon 9's U/E data. After the flights: D115's fidelity. **Frozen:** visuals. **The career:** after v0.3.

## Open for the owner

These are agents' defaults, in each brief's "Open for the owner":
- Dragon's 19.0 MPa MEOP (U);
- the coast law's hourly refresh;
- the thermosphere film (140–150 km);
- in roll, the liquid turns with its tank;
- the staging defaults (ROUNDTRIP);
- the pad-abort cases (QUICKFIX).

Older: `falcon9.md`'s U/E data, Dragon's CO₂ removal, the Moon's field degree, the 1,000-year ephemeris, backhouse's certificate (Windows not checked since 2026-10-26), CRUCIBLE's exit states.

## Gotchas

- **Agents follow [agent-rules.md](agent-rules.md).**
  - Their private-index commits leave the shared index stale, so run `git reset -q` before every lead commit, or commit through a private index.
  - The stop hook tests the shared tree, so an agent's unfinished work fails it: check `git diff HEAD --stat` before chasing a failure.
  - Agents often stall at the stream watchdog, sometimes all at once; `SendMessage` resumes them.
- **A linker error saying `Undefined symbols … _anon.….llvm`** is a corrupted incremental cache, not code: `cargo clean -p sim` (23.7 GB freed on 2026-10-30).
- **The machine:** three agents building at once turn an hour into ~40 minutes of work. One game at a time under `target/demo.lock` (the lock kept its name).
- **Mission tests:** `./run headless <port>` plus MCP ([mcp.md](../technical/mcp.md)). The booster's plumes sit behind the navball cluster: `shoot` with `ui_scale: 0.2`, `set_camera` farther out, and crop with `sips`.
- egui's proportional font lacks ▲ ▼ ▸ →: use ⏶ ⏷. egui areas default to 400 px tall: set the max height before a scroll area.
- **Disk:** keep ≥ 30 GB free (`docs/process.md`). Saves are not compatible across versions (D063); the playtest save (`./run play`, then Cmd+L) still carries the test craft.
