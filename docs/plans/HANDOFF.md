# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-30 (night). Running: PLANNER-2, ONEKIND-3, RESEARCH (all three slots). PLANNER-1 and AUDIT done. The owner asked for next steps 1–3 done; answers in [interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md) (D120, D126 revised; D128, D129 new; D063, D121 extended).*

## Your first job

1. Take the three agents' reports as they arrive (each through a message: commits, what is left, questions; their briefs hold their State):
   - **PLANNER-2** ([brief-planner.md](brief-planner.md), its State): PLANNER-1 landed `kepler::propagate`, `sim::systems::maneuver` (the layer-2 burn program) and `Vessel::set_nodes` (bce4379, c7c2de4, 0a382b6). PLANNER-2 does the rest in order: accuracy, vessel tests, moving the planned-burn tests onto nodes (this unblocks ONEKIND-3), MCP `set_plan` and the Falcon 9 mission test, takeover hand-over, docs.
   - **RESEARCH** ([brief-research.md](brief-research.md)): docs only. Designs for phase 4: `technical/landing-guidance.md` (convex guidance and a deterministic fixed-iteration solver), `entry-ablation.md`, `water-landing.md`, and the data gaps in `falcon9.md` (Dragon's CO₂ removal, helium bottles, the booster's landing-zone return).
   - **ONEKIND-3** ([brief-onekind.md](brief-onekind.md), its ONEKIND-3 section): the unblocked tests and the stress craft first; then it waits for PLANNER before deleting the made-up path.
   - **AUDIT: done** (14c2a4d, 17ec7aa, 08a34c8): `tests/warp_course.rs` (two days of S2 + Dragon give the same bits at 1×–10⁵×; D1–D3 reproduced as `#[ignore]`d tests that ADVANCE un-ignores), `technical/warp-invariance.md`, `technical/software-layers.md` (design), drafts `brief-advance.md` and `brief-layers.md`. Its findings went to the owner; its open points are in its brief (to archive).
2. When PLANNER lands, tell ONEKIND-3 to go on (Left 3, 4). When AUDIT reports, give the owner its ranked difficulties (they asked), then start the wave (next steps, item 3) one agent at a time as slots free.
3. ONEKIND-2, MISSION-2 and DOCS-2 are done (their briefs: ONEKIND's State; `brief-mission.md` and `brief-docs.md` to archive when convenient; DOCS-2's Left is still open).

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
4. **The two flights, in parallel** (D117, D123, D125, D126), each checked by an MCP mission test (D122):
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
