# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-10-30 (night). Running: ONEKIND-7, INTERFACE-3, DOCS-3 (all three slots). PLANNER finished. The owner asked for next steps 1–3 done; answers in [interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md) (D120, D126 revised; D128, D129 new; D063, D121 extended).*

## Your first job

The owner, 2026-10-31: "Work all night on this please", with at least one agent on the interface (INTERFACE). The lead's context may go as high as needed tonight. The cap stays at 3 agents plus the lead.

1. **Running:**
   - **ONEKIND-7** ([brief-onekind.md](brief-onekind.md), ONEKIND-6's State): the deletion's steps (b) (a patch ready), (c) and (d), the seven decisions, the Dragon playtest save, the docs' test-craft mentions. **The wave waits on it.** ONEKIND-6 landed step (a), 2a5420e.
   - **INTERFACE-3** ([brief-interface.md](brief-interface.md), INTERFACE-2's Left): shots at HEAD, finding 13 (D106 in the Landing menu and `hottest`), MCP reach (target, staging bar, notifications), the navball's holds during a program and `get_vessel`'s apsides while thrusting (from PLANNER-4), then more of the owner's cleanup.
   - **DOCS-3** ([brief-docs.md](brief-docs.md), its section): the archived specs into technical docs, stale status lines, the README's images, Windows on backhouse, then the finished briefs archived.
2. **Next:** once PLANNER and ONEKIND have landed, the wave one agent at a time, each brief written and reviewed: STATE ([brief-state.md](brief-state.md)), LAYERS ([brief-layers.md](brief-layers.md)), FIXES3 ([brief-fixes3.md](brief-fixes3.md)), ADVANCE ([brief-advance.md](brief-advance.md)), PERF ([brief-perf.md](brief-perf.md); first the coast's tank law at 5,000 parts), DEBRIS ([brief-debris.md](brief-debris.md)). INTERFACE can run beside them (game only). Successor agents continue from a brief's State when one wraps up at ~300k.
3. **Done tonight:** PLANNER, finished (PLANNER-4: 178e814 turns limited by the gas left, a2a9d55, 1eb6715, 431155d, 2dfd940, 1422b99 `technical/planner.md`; the Falcon 9 mission test raised the orbit with two nodes from warp 6: 412.1 × 343.2 km against 413.2 × 343.5 predicted, shots checked by the lead; its Left: Dracos' burns go with the Dragon flight, the pilot's throttle to FIXES3, two game items to INTERFACE-3); ONEKIND-5 (051f52b, b055104, 84e9b41); PLANNER-3 (8b5e8e9: the game's planner, MCP `set_plan` and the landing prediction on nodes); ONEKIND-4 (716e2fd, 4ee9212: sim unit tests and examples on the small rocket); INTERFACE-1 (def10d2, 1183eb1, 723ceec, 85bb56e, f7aff55); RESEARCH (26f4639, 4dd65f9); AUDIT (14c2a4d, 17ec7aa, 08a34c8); PLANNER-1 (bce4379, c7c2de4, 0a382b6); PLANNER-2 (bb43d53, 8de24aa, 835f839); ONEKIND-3 (1721504, be83b64; the stress craft's numbers in `scale-performance.md`). ONEKIND-2, MISSION-2 and DOCS-2 before them (`brief-mission.md` and `brief-audit.md` are archived in `archive/briefs/`, with PLANNER's and RESEARCH's; `brief-docs.md` goes when DOCS-3's Left is done).

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
4. **The two flights, in parallel** (D117, D123, D125, D126), each checked by an MCP mission test (D122). RESEARCH's designs are ready: `technical/landing-guidance.md`, `entry-ablation.md`, `water-landing.md`, and 16 tagged rows in `falcon9.md` (26f4639; its open points in `archive/briefs/brief-research.md`):
   - **Crew Dragon:** planned burns on the Dracos (thruster blocks; PLANNER-4's Left), the trunk joint, entry with 1-D ablation by material, the chutes' real logic as the automatic option, a calm sea, D116's doses, a rigid placeholder station.
   - **The booster:** relight, boostback and entry by predictor–corrector, convex landing guidance written as WASM-ready flight-layer code (D043), a landing zone first.
5. **Small items:**
   - in `bench_limbo` the small rocket held prograde at 900 km loses 6.6 t of its 12.2 t of propellant in 30 days (ONEKIND-4). The lead's hand check: 220 kg/day is ~0.5 kW into the LOX at 213 kJ/kg, and a bare 2 m tank in sunlight at 900 km takes ~0.5–1.5 kW orbit-averaged while a 90 K wall radiates almost nothing: plausible, not a bug. Insulation (MLI) on cryogenic tanks is a part parameter for later;
   - resolved: INTERFACE-1's ascent fell back because it flew with debug on (propellant does not drain); with debug off, INTERFACE-2 matched testing.md's worked example to the digit (max-Q T+66 s 34.6 kPa, MECO T+138 s 65.3 km). MCP `run_until {max_q}` not firing is still to check;
   - the MVac: the apoapsis rose ~80 km in the 2 s after `throttle: 0` (its floor is 64 %): sent to PLANNER-4 (D114's closed-throttle shutdown against the program's cutoff);
   - Crew Dragon has no cabin sensors in its data (INTERFACE-2's finding 19): for LAYERS' bare-minimum test and FIXES3;
   - a part's peak interior temperature may not update during a warp-6 coast (finding 18): for ADVANCE (D129);
   - the 11-vessel performance check: the tool exists now (`spawn_vessels`, developer MCP, b055104); measure headless with `spawn_vessels` then `profile` at warp levels 0, 6 and 9;
   - a tumbling craft of parts with liquid aboard keeps angular momentum at a split only to ~2e-7 (1e-9 empty): the gap is in the pieces' mass properties with their liquids (ONEKIND-5; `coast-split` test allows 1e-6). For STATE or DEBRIS;
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
- **Pushes queue:** the pre-push hook takes `target/prepush.lock` (its pid inside; a dead owner's lock is cleared), because two checks in the one mirror overwrote each other (2026-10-31). A push may print "waiting for another push's check"; run pushes with `run_in_background`.
- **A linker error saying `Undefined symbols … _anon.….llvm`** is a corrupted incremental cache, not code: `cargo clean -p sim` (23.7 GB freed on 2026-10-30).
- **The machine:** three agents building at once turn an hour into ~40 minutes of work. One game at a time under `target/demo.lock` (the lock kept its name).
- **Mission tests:** `./run headless <port>` plus MCP ([mcp.md](../technical/mcp.md)). The booster's plumes sit behind the navball cluster: `shoot` with `ui_scale: 0.2`, `set_camera` farther out, and crop with `sips`.
- egui's proportional font lacks ▲ ▼ ▸ →: use ⏶ ⏷. egui areas default to 400 px tall: set the max height before a scroll area.
- **Disk:** keep ≥ 30 GB free (`docs/process.md`). Saves are not compatible across versions (D063); the playtest save (`./run play`, then Cmd+L) still carries the test craft.
