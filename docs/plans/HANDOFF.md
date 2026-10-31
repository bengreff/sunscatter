# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-30 (night). Running: PLANNER-3, INTERFACE, ONEKIND-4 (all three slots). The owner asked for next steps 1–3 done; answers in [interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md) (D120, D126 revised; D128, D129 new; D063, D121 extended).*

## Your first job

The owner, 2026-10-31: "Work all night on this please", with at least one agent on the interface (INTERFACE). The lead's context may go as high as needed tonight. The cap stays at 3 agents plus the lead.

1. **Running:**
   - **PLANNER-3** ([brief-planner.md](brief-planner.md), "For PLANNER-3"): the game's planner on nodes, then `predict_tests`, `landing::model`, the second stage's slew cap, MCP `set_plan` and the Falcon 9 mission test.
   - **INTERFACE** ([brief-interface.md](brief-interface.md)): the map of the interface and its cleanup; the systems screen's thermal mesh and part isolation (D123's freeze lifted for the interface).
   - **ONEKIND-4** ([brief-onekind.md](brief-onekind.md), its section): the rest of the tests, then the made-up path's deletion once PLANNER-3 says so.
2. **Next:** once PLANNER and ONEKIND have landed, the wave one agent at a time: STATE, LAYERS, FIXES3, ADVANCE (from `brief-advance.md`), PERF (first: the coast's tank law at 5,000 parts), DEBRIS. Successor agents continue from a brief's State when one wraps up at ~300k.
3. **Done tonight:** RESEARCH (26f4639, 4dd65f9); AUDIT (14c2a4d, 17ec7aa, 08a34c8); PLANNER-1 (bce4379, c7c2de4, 0a382b6); PLANNER-2 (bb43d53, 8de24aa, 835f839); ONEKIND-3 (1721504, be83b64; the stress craft's numbers in `scale-performance.md`). ONEKIND-2, MISSION-2 and DOCS-2 before them (`brief-mission.md`, `brief-docs.md` and `brief-audit.md` to archive when convenient; DOCS-2's Left is still open).

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
- **Decisions recorded:** D117–D129, with D027, D034, D043, D044, D063, D081, D114, D115, D120, D121, D123 and D126 revised. The interviews are in `docs/archive/interviews/` (2026-10-29-next, 2026-10-30). v0.2 is complete with both flights plus D121's scale target (`vision.md`).
- **Half done:**
  - ONEKIND: `craft::small` (`small-rocket`) exists and about a dozen suites have moved. The test craft and the made-up path are still in the code. The owner confirmed a hard impact breaking a craft of parts apart is intended. The small rocket's numbers are estimates (E) awaiting the owner: 400 N cold-gas blocks, 3e5 N/m and 1.8e4 N·s/m feet with a 0.35 m stroke, and the 1.7e-4 nozzle heat share.
  - MISSION: `step`, `run_until`, `new_flight`, saves, `bench` and `profile` have not been run in the game yet.
- **CI:** green through 8ece779; the agents' last pushes were queued at handoff (`gh run list --limit 3`).

## Next steps

1. **PLANNER** (running): see its brief.
2. **ONEKIND-3** (running): Left 1–2 now; Left 3–4 after PLANNER (delete the made-up path and the test craft; D074, D064, D060, D078, D079, D100, D104 and their citations; the playtest save regenerated as Dragon alone in orbit).
3. **The wave, one sim-wide agent at a time** (D121, D128, D129), in this order:
   - **STATE:** merge `ShipState` and `Runtime`; typed handles; the engine rule in one place. Saves hold no static data (D063, extended).
   - **LAYERS** (D128, from AUDIT's `software-layers.md` and `brief-layers.md`): the parts' own logic (layer 1) out of `sim::systems::flight` into the parts; the ship computer (layer 2) as functions; the bare minimum defined; the warp-stop line (crew or vessel at risk) with layer 2 able to replace it; notifications off per craft.
   - **FIXES3** (on LAYERS' split):
     - the function runner through `program::offered`;
     - the load's vapour in `ShipState::initial`;
     - `estimate::ready` against `program::can_start`;
     - fail-safe shutdown for a lost controller, as layer-1 engine logic (D123);
     - SAS and the holds on coasts through the flight layer;
     - D034's routing (a crewed vessel must not command probes);
     - D123's switch to turn random failures off;
     - the automatic abort's decision as layer 2 on a thrust-to-weight margin (E), the manual abort firing whenever an escape system can (D120, simplified).
   - **ADVANCE** (from AUDIT's `warp-invariance.md` and `brief-advance.md`): one advance law per subsystem; the same bits at 1× and 10⁵× for every subsystem and event (D129); events at their own epoch; warp restricted, not models changed.
   - **PERF:** aero once per tick; thermal at its own rate; analytic coast heat; a spatial index for the geometry scans; per-vessel threads with bit-identical cross-machine tests (D127). The game's side by judgement, not arbitrary cubes (D121, extended).
   - **DEBRIS:** the controllability check; ballistic lumps; designs precomputed at build.
4. **The two flights, in parallel** (D117, D123, D125, D126), each checked by an MCP mission test (D122). RESEARCH's designs are ready: `technical/landing-guidance.md`, `entry-ablation.md`, `water-landing.md`, and 16 tagged rows in `falcon9.md` (26f4639; its open points in `brief-research.md`):
   - **Crew Dragon:** the trunk joint, entry with 1-D ablation by material, the chutes' real logic as the automatic option, a calm sea, D116's doses, a rigid placeholder station.
   - **The booster:** relight, boostback and entry by predictor–corrector, convex landing guidance written as WASM-ready flight-layer code (D043), a landing zone first.
5. **Small items:**
   - the 11-vessel performance check needs a spawn tool once the test craft is gone (ONEKIND-3 has it);
   - the render scale flips between 0.8 and 1.0 every half second in a headless game;
   - stage 2's RP-1 reads 29.70 t against a 29.66 t capacity on the pad (densified load against nominal density?);
   - `tools/gpu_trace.py` still launches the deleted plunge demo.
6. **Queued:** Dragon's helium bottles; nozzle flow separation; the Falcon 9's U/E data. After the flights: D115's fidelity. **Frozen:** visuals. **The career:** after v0.3.

## Open for the owner

These are agents' defaults, in each brief's "Open for the owner":
- Dragon's 19.0 MPa MEOP (U);
- the coast law's hourly refresh;
- the thermosphere film (140–150 km);
- in roll, the liquid turns with its tank;
- the staging defaults (ROUNDTRIP);
- the pad-abort cases (QUICKFIX);
- the takeover rule's reading (D126: a sequence switches to manual only for conflicting input; PLANNER defines "conflicting").

Older: `falcon9.md`'s U/E data, Dragon's CO₂ removal, the Moon's field degree, the 1,000-year ephemeris, backhouse's certificate (Windows not checked since 2026-10-26), CRUCIBLE's exit states.

## Gotchas

- **Agents follow [agent-rules.md](agent-rules.md).**
  - Their private-index commits leave the shared index stale, so run `git reset -q` before every lead commit, or commit through a private index.
  - The stop hook tests the shared tree, so an agent's unfinished work fails it: check `git diff HEAD --stat` before chasing a failure.
  - Agents stall at the stream watchdog when one call shows no output for 600 s: the whole sim suite under load, or a long MCP `run_until`. Brief them to keep blocking calls under ~5 minutes (`run_in_background`, narrower test targets, shorter spans). `SendMessage` resumes them.
- **A linker error saying `Undefined symbols … _anon.….llvm`** is a corrupted incremental cache, not code: `cargo clean -p sim` (23.7 GB freed on 2026-10-30).
- **The machine:** three agents building at once turn an hour into ~40 minutes of work. One game at a time under `target/demo.lock` (the lock kept its name).
- **Mission tests:** `./run headless <port>` plus MCP ([mcp.md](../technical/mcp.md)). The booster's plumes sit behind the navball cluster: `shoot` with `ui_scale: 0.2`, `set_camera` farther out, and crop with `sips`.
- egui's proportional font lacks ▲ ▼ ▸ →: use ⏶ ⏷. egui areas default to 400 px tall: set the max height before a scroll area.
- **Disk:** keep ≥ 30 GB free (`docs/process.md`). Saves are not compatible across versions (D063); the playtest save (`./run play`, then Cmd+L) still carries the test craft.
