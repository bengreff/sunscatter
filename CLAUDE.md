# Sunscatter v0.2

Real-scale 3D spaceflight, mission-design and logistics game in Rust. Being rebuilt from scratch; v0.1 (2D) is archived in `archive/v0.1/` — do not edit it.

## Read first
- `docs/plans/HANDOFF.md` — **start here in a fresh session** (auto-loaded by a SessionStart hook): reading order, current state, next steps. `/handoff` updates it before a `/clear`.
- `docs/vision.md` — pillars and non-goals. `docs/decisions.md` — what is decided (check before proposing).
- `docs/design/motion-model.md` — how bodies and ships move (no SOIs, frame tree, precision, determinism).
- `docs/plans/` — the current build plan; check items off as you go.
- `docs/process.md` — doc types, research strategy (KSP mods, Principia, …), engineering rules.
- `docs/architecture.md` — **who owns what** (search it before writing a new function about bodies, orbits, visibility, lighting or time), frame order, where tests live.
- `docs/features/` — the spec for the feature you are building, if one exists.

## Layout
- `crates/sim` — simulation core. **No engine dependencies.** f64, frame-typed, deterministic.
- `crates/game` — Bevy app (rendering, input, UI). Converts sim state to camera-relative f32 each frame.
- `crates/ephem-tool`, `crates/asset-tool` — offline generation of committed data (`data/`).
- `crates/mcp` — the MCP server (local HTTP, JSON-RPC); knows nothing of the game, which answers tool calls each frame.

## Commands
- `cargo test -p sim` — fast sim tests (run constantly).
- `cargo run -p game --release` — the prototype. `--features dev` (Bevy dynamic linking) for faster incremental builds while iterating; never for releases.
- `SUNSCATTER_DEMO=<dir> [SUNSCATTER_DEMO_OFFSCREEN=1] cargo run -p game --release` — scripted flight (pad → orbit → parachute), screenshots of every graphics tier at each key view, graphics benchmark tables (`bench_*.md`), performance numbers with 11 vessels. Offscreen works with the screen locked. Use it to verify visual changes. Quick checks: `SUNSCATTER_DEMO_STOP_AFTER=<step>` (e.g. `Pad`, `MoonClose`), `SUNSCATTER_DEMO_NO_BENCH=1`. `SUNSCATTER_TIER=<tier>` sets the starting tier; `SUNSCATTER_HOME=<dir>` moves saves and settings.
- Body data lives in `data/bodies/<body>/` (`body.ron` for sim, `visual.ron` for game, maps baked by `cargo run -p asset-tool --release -- all`).
- `cargo run -p sim --release --example bench_coast` — coast integration cost per step.
- `cargo run -p ephem-tool --release -- sol` — regenerate the Solar System ephemeris (needs `data/external/de440s.bsp`; see the tool's docs). The golden tests fail if the shipped file and code disagree.
- `cargo clippy -p sim -p ephem-tool -p asset-tool -p mcp --all-targets -- -D warnings`, `cargo fmt --all`, `tools/check_file_sizes.sh`.
- CI: a fast `sim` job (no Bevy; fmt, file sizes, decision references, clippy, tests) and a separate `game` job, on macOS and Windows. `tools/check_decision_refs.sh` fails on citations of deleted (superseded) decisions.
- Windows: at the end of a big session, try `ssh backhouse` (the owner's PC) and run the demo there (procedure: `docs/windows.md`); if it doesn't connect, skip it.
- Hooks (`.claude/settings.json`, scripts in `tools/hooks/`): the handoff is loaded at session start, `cargo check` of the owning crate runs after editing a `.rs` file, and `cargo test -p sim` runs on stop. Failures are fed back (exit 2).

## Non-negotiable rules
1. **Physics has no reference bodies.** Anchors/frames exist only for precision or display; changing them must not change trajectories (invariance tests enforce this). Never add SOI logic.
2. **Determinism.** In `sim`, all trig/exp/log goes through `sim::math` (libm). No `mul_add`, no glam trig helpers (`from_axis_angle`, `slerp`), no order-dependent parallel reductions.
3. **Precision.** Never subtract two large absolute coordinates to get a precise quantity; go through the lowest common ancestor in the frame tree.
4. **The stored trajectory is the truth.** Time warp only changes how fast stored segments are sampled; the renderer never integrates. Chunked integration must equal single-pass (tests enforce).
5. **One owner per piece of math** (one Kepler solver: `sim::kepler`).
6. Tests come with the code. Small commits, one purpose each, pushed to `main`. Every commit passes fmt, clippy, tests.
7. Describe features honestly: *design / skeleton / playable / polished*.
8. **Game rules are pure functions with table tests** (visibility, hover, camera limits, lighting, orbit-line length); Bevy systems only wire them up. One owner per rule (`docs/architecture.md`).
9. **Superseded decisions are deleted**, and everything citing them is updated in the same commit (CI checks the citations).
