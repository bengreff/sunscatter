# Sunscatter v0.2

Real-scale 3D spaceflight, mission-design and logistics game in Rust. Being rebuilt from scratch; v0.1 (2D) is archived in `archive/v0.1/` — do not edit it.

## Read first
The reading guide is in `README.md`; the docs are organised by system (D124).
- `docs/plans/HANDOFF.md` — **start here in a fresh session** (auto-loaded by a SessionStart hook): current state, next steps, gotchas. `/handoff` updates it before a `/clear`.
- `docs/vision.md` — pillars and non-goals. `docs/decisions.md` — what is decided (check before proposing).
- `docs/owners.md` — **who owns what**, by system (search it before writing a new function about bodies, orbits, visibility, lighting or time).
- `docs/systems/` — one general doc per system: its laws, what is built (status words), owners, decisions. Read the one you work on.
- `docs/technical/` — specific details, one topic per file: `motion-model.md` (no SOIs, frame tree, precision, determinism), `ship-model.md`, the frame order, testing, the spec of what you are building. `docs/vehicles/` — research dossiers (`falcon9.md`).
- `docs/plans/` — live work only: the hand-off, `agent-rules.md`, running briefs, the workstream plan; check items off as you go. Finished work goes to `docs/archive/`.
- `docs/process.md` — doc types, research strategy (KSP mods, Principia, …), engineering rules.

## Layout
- `crates/sim` — simulation core. **No engine dependencies.** f64, frame-typed, deterministic.
- `crates/game` — Bevy app (rendering, input, UI). Converts sim state to camera-relative f32 each frame.
- `crates/ephem-tool`, `crates/asset-tool` — offline generation of committed data (`data/`).
- `crates/mcp` — the MCP server (local HTTP, JSON-RPC); knows nothing of the game, which answers tool calls each frame.

## Commands
- `cargo test -p sim` — fast sim tests (run constantly).
- `./run` — the game (`run.cmd` on Windows; `./run dev` for faster rebuilds, `./run play` for the low-orbit playtest save). It is `cargo run -p game --release`: `--features dev` (Bevy dynamic linking) for faster incremental builds while iterating; never for releases.
- `./run headless [port]` — **the mission test** (D122, D044): the game with no window (`SUNSCATTER_OFFSCREEN=1`, works with the screen locked), its MCP server at `http://127.0.0.1:<port>/mcp` (default 7777, `Authorization: Bearer dev`), the developer tools on (`SUNSCATTER_DEV=1`: `bench`, `profile`), a fresh home dir for saves and shots (`SUNSCATTER_HOME=<dir>` to choose it). An agent flies it through the tools (`tools/list`; `shoot`, `set_camera`, `pause`, `step`, `run_until`, `new_flight`, `save_game`, `load_game`, `set_debug`, the flight's) and reads its shots with the Read tool. With your own `CARGO_TARGET_DIR`; one game at a time (`target/demo.lock`).
- `SUNSCATTER_PROFILE=1` (with the game, or `./run headless` and the `profile` tool) — the profiler (`game::profile`): every 5 s, CPU per system and thread, GPU per pass group (not on Metal: use `xcrun xctrace record --template 'Metal System Trace' … --launch -- <game binary>` and `tools/gpu_trace.py`), the counts that drive the cost, to the terminal, `profile.log` and the performance overlay (Esc menu).
- `SUNSCATTER_CRAFT=falcon9-crew` — the craft of a new game (the default; every craft is built from parts, D121, and one that does not build is replaced by the small rocket of parts). `SUNSCATTER_RENDER_SCALE=<s>` pins the 3D view's resolution (D072). `SUNSCATTER_HOME=<dir>` moves saves and settings.
- Body data lives in `data/bodies/<body>/` (`body.ron` for sim, `visual.ron` for game, maps baked by `cargo run -p asset-tool --release -- all`).
- `cargo run -p sim --release --example bench_coast` — coast integration cost per step.
- `cargo run -p ephem-tool --release -- sol` — regenerate the Solar System ephemeris (needs `data/external/de440s.bsp`; see the tool's docs). The golden tests fail if the shipped file and code disagree.
- `cargo clippy -p sim -p ephem-tool -p asset-tool -p mcp --all-targets -- -D warnings`, `cargo fmt --all`.
- CI: a fast `sim` job (no Bevy; fmt, decision references, clippy, tests) and a separate `game` job, on macOS and Windows. `tools/check_decision_refs.sh` fails on citations of deleted (superseded) decisions.
- Windows: at the end of a big session, try `ssh backhouse` (the owner's PC) and run the game there (procedure: `docs/technical/windows.md`); if it doesn't connect, skip it.
- Hooks (`.claude/settings.json`, scripts in `tools/hooks/`): the handoff is loaded at session start, `cargo check` of the owning crate runs after editing a `.rs` file, and `cargo test -p sim` runs on stop. Failures are fed back (exit 2).
- Git pre-push hook (`tools/git-hooks/pre-push`; enable per checkout with `git config core.hooksPath tools/git-hooks`): runs CI's checks on the exact commit being pushed, in a private mirror, and refuses a push that would fail CI (a night of red pushes flooded the owner's inbox, 2026-10-26). Its pre-commit hook refuses a commit from a stale index, one that would revert HEAD's version of a file the work tree still has (after an agent's private-index commit; `git reset -q -- <file>`).

## Non-negotiable rules
1. **Physics has no reference bodies.** Anchors/frames exist only for precision or display; changing them must not change trajectories (invariance tests enforce this). Never add SOI logic.
2. **Determinism.** In `sim`, all trig/exp/log goes through `sim::math` (libm). No `mul_add`, no glam trig helpers (`from_axis_angle`, `slerp`), no order-dependent parallel reductions.
3. **Precision.** Never subtract two large absolute coordinates to get a precise quantity; go through the lowest common ancestor in the frame tree.
4. **The stored trajectory is the truth.** Time warp only changes how fast stored segments are sampled; the renderer never integrates. Chunked integration must equal single-pass (tests enforce).
5. **One owner per piece of math** (one Kepler solver: `sim::kepler`).
6. Tests come with the code. Small commits, one purpose each, pushed to `main`. Every commit passes fmt, clippy, tests.
7. Describe features honestly: *design / skeleton / playable / polished*.
8. **Game rules are pure functions with table tests** (visibility, hover, camera limits, lighting, orbit-line length); Bevy systems only wire them up. One owner per rule (`docs/owners.md`).
9. **Superseded decisions are deleted**, and everything citing them is updated in the same commit (CI checks the citations).

## Usage budget (owner's rule, 2026-10-27)
Two overnight swarms (26-27 Oct) used about half of a week's limit in two days: about 35 agents, many running at 500k-975k context, re-reading about 6 billion tokens to write about 2 million. Every call re-reads the whole context, so cost scales with context size times the number of calls.
1. **Cap context.** The lead session (the orchestrator) hands off at about 400k tokens (`/handoff` then `/clear`; owner, 2026-10-29). Agents wrap up at about 300k (owner, 2026-10-30): they commit, write their state into their brief, and a fresh agent continues from it.
2. **Cap agents.** One lead plus at most 3 parallel agents, non-branching (no sub-agents of agents). Reviewers count toward the 3. Prefer one strong agent per workstream over many small ones; swarms leave patchwork.
3. **Cap a night.** An unattended run spends at most 20% of the weekly limit. Check `/usage` before launching and stop launching new agents at the cap.
4. **Agents read files, not transcripts.** Give each agent a short brief that points at docs and files, not a pasted history. Large logs go through `head`/`grep`, never whole into context.
