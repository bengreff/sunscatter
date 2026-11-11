# Handoff: read this first in a fresh session

A SessionStart hook loads this file automatically after `/clear` or at startup. It is kept current by `/handoff`.

*Updated 2026-11-10 by sun (the Director's worker). **From 2026-11-02 sunscatter is managed by the Director** (owner). Design session 1 (the craft data model) is consolidated; design session 2 (time and warp) runs with the Director (`docs/design/foundation/session-2-time-warp.md`, the Director's, untracked). The tree is clean apart from untracked files that are not the worker's (`chatGPT/`, `codex-research/`, three in `archive/v0.1/`, the session-2 file).*

## State

- **CI:** green through 14bdd92 (`gh run list --limit 3`, 2026-11-10); the worker's commits a569acd to the handoff are pushed after it.
- **Design session 1 done** (2026-11-04 to 11-10): decisions D130-D136 (a569acd; D134 reverses D115's data rule, D132 refines D103) and the spec [craft-data-model.md](../technical/craft-data-model.md) (cacf05f): every item cites its ruling, 24 items open for the next session (its section 16), the Electron gaps (16 closed, 6 partly, 1 open), and the conflicts with canon listed, not resolved (its section 15: D100-D105, D027, D063; decisions.md open question 6).
- **Done overnight** (briefs in `docs/archive/briefs/`; the owner's answers in `archive/interviews/interview-2026-10-30-roadmap.md`):
  - decisions D128 (three software layers) and D129 (the same course of events at every warp) new; D120 and D126 revised; D063, D121, D123 extended;
  - PLANNER: closed-loop planned burns for craft of parts (nodes, `sim::systems::maneuver`, `kepler::propagate`); the Falcon 9 mission test raised its orbit with two nodes from warp 6;
  - ONEKIND: the test craft and the made-up path deleted; every craft is built from parts;
  - STATE: one state per vessel (`Runtime` merged into `ShipState`), typed owner handles, the engine rule in one place (F8), saves without static data, `powered()` from the ship's batteries and arrays;
  - AUDIT (`technical/warp-invariance.md`, `software-layers.md`), RESEARCH (`technical/landing-guidance.md`, `entry-ablation.md`, `water-landing.md`), DOCS-3, REVIEW (`technical/review-2026-10-31.md`, 13 findings routed into the briefs).
- **Half done:**
  - **LAYERS** ([brief-layers.md](brief-layers.md), LAYERS-4's State): `ship_os`, the fail-safe per engine, settling and the sequence as layer 2, the chutes from `Knowledge`, the program API with D126's switch, the warp stop with `notify` per craft, and the bare minimum proven (4d4478d: an uncrewed Dragon coasts 90 days, Crew Dragon's warp stops at +8.96 days, the same bits at 1× and 10⁵×). Finding 71 and MCP `run_until` ending on a warp stop **landed from `git stash@{0}`** on 2026-11-10 (5d16bc5, 2c7e7dc, 2c8090e, a6ca66e; brief-layers.md's last State entry), with a sim fix the stash's test found: a frame holds at a warp stop raised at its last lattice point (before, it ran up to 60 s past it). Left: `maneuver` into `ship_os`; bubble ticks and landed frames holding at a stop's epoch.
  - **INTERFACE** ([brief-interface.md](brief-interface.md), INTERFACE-12's State): the owner's request is done (the thermal mesh and part isolation, Esc one level, generated help, one rule per shown quantity, MCP parity with the player, the PROGRAMS tab with editable config). Left: a walk from a fresh ascent, the first minutes for a new player, a warp-stop program's `Line` editable.
- **Playtest save:** SAVE_VERSION 21, Dragon alone at 199 × 639 km; `./run play`, then Cmd+L.
- **Usage:** the night's 34 agents ran into the weekly limit (reset 2026-11-02 01:00 CT), past CLAUDE.md's 20 %-a-night cap: agents running to 300k+ cost far more than the earlier "5 agents ≈ 3 %".

## Next steps (owner + Director, 2026-11-03: scope reassessment)

**Read [scope-2026-11.md](scope-2026-11.md) first.** The owner's 68-answer interview ([interview-2026-11-02-scope.md](../archive/interviews/interview-2026-11-02-scope.md)) redefined v0.2 around a general foundation (generic part types, calibration on Electron/Falcon 1, then F9 + Dragon, a probe, warp and scale benchmarks). Until the design sessions settle it:

1. ~~Land stash@{0}~~ **done 2026-11-10**: every piece landed, none left. Why: MCP is the mission-test tool, not the frozen interface; the life-support change is a correctness fix; finding 71 is small; no change to what a save holds (SAVE_VERSION stays 21, the playtest save not re-flown). The stash held no INTERFACE-13 walk (its only action_bar change is finding 71's test fix). **stash@{0} is left in place, not dropped**: its content equals main's but for the two fixes; dropping it is the owner's call.
2. **Then only research spikes**, no feature work: warp invariance (scope 3.3's four cases at 1x / 10^3x / 10^5x), CoolProp over FFI, Rapier determinism Mac vs Windows, one ExoPlaSim climatology.
3. **Design sessions with the owner**, craft data model first (scope section 8).
4. **Frozen:** the interface (scope section 4 replaces it later); the FIXES3 / ADVANCE / PERF / DEBRIS wave and the two flights wait for the new design (their briefs stay as reference).

## Open for the owner

DROPPED (owner, 2026-11-03): the ~50 defaults in [owner-questions.md](owner-questions.md) stay frozen in code; only those the new design still needs get re-asked (the stress craft's shape is still open: scope section 9).

## Gotchas

- **Agents follow [agent-rules.md](agent-rules.md):** private-index commits (run `git reset -q` before a commit in the shared index); an old patch goes in with `git apply --3way`; a change to what a save holds bumps `SAVE_VERSION` and the playtest save is flown again (`tools/playtest_flight.py <port> all`, ~10 min headless, under the game lock).
- **The stop hook tests the shared tree,** so a running agent's unfinished work fails it: `git diff HEAD --stat` before chasing a failure.
- **Agents stall at the stream watchdog** when a call shows no output for 600 s: keep blocking calls under ~5 minutes (`run_in_background`, narrower tests, shorter MCP spans). Pushes queue on `target/prepush.lock`: run them in the background.
- **Mission tests:** `./run headless <port>` plus MCP ([mcp.md](../technical/mcp.md)); fly with debug off (in debug, propellant does not drain). MCP `read` gives any screen as data. One game at a time under `target/demo.lock`.
- **A vessel's first warp-stop check (+60 s, `systems::STEP`) only records the risks it starts with** (`Watch::primed`): a test that breaks something before then gets no stop.
- **A linker error `Undefined symbols … _anon.….llvm`** is a corrupt incremental cache: `cargo clean -p sim`.
- **Disk:** keep ≥ 30 GB free; the pre-push mirror's `target/prepush/target` grew to ~45 GB in a night (delete it under the pre-push lock when no push runs; the next push rebuilds it).
- egui's proportional font lacks ▲ ▼ ▸ →: use ⏶ ⏷. egui areas default to 400 px tall: set the max height before a scroll area.
