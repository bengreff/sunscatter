# Sunscatter v0.2

Real-scale 3D spaceflight, mission-design and logistics game in Rust. Being rebuilt from scratch; v0.1 (2D) is archived in `archive/v0.1/` — do not edit it.

## Read first
- `docs/vision.md` — pillars and non-goals. `docs/decisions.md` — what is decided (check before proposing).
- `docs/design/motion-model.md` — how bodies and ships move (no SOIs, frame tree, precision, determinism).
- `docs/plans/` — the current build plan; check items off as you go.
- `docs/process.md` — doc types, research strategy (KSP mods, Principia, …), engineering rules.

## Layout
- `crates/sim` — simulation core. **No engine dependencies.** f64, frame-typed, deterministic.
- `crates/game` — Bevy app (rendering, input, UI). Converts sim state to camera-relative f32 each frame.
- `crates/ephem-tool` — offline ephemeris generation (DE440 → our tables in `data/ephemeris/`).

## Commands
- `cargo test -p sim` — fast sim tests (run constantly).
- `cargo run -p game --release` — the prototype. `--features dev` (Bevy dynamic linking) for faster incremental builds while iterating; never for releases.
- `SUNSCATTER_DEMO=<dir> [SUNSCATTER_DEMO_OFFSCREEN=1] cargo run -p game --release` — scripted flight (pad → orbit → parachute), performance numbers with 11 vessels, screenshots. Offscreen works with the screen locked. Use it to verify visual changes.
- `cargo run -p sim --release --example bench_coast` — coast integration cost per step.
- `cargo run -p ephem-tool --release -- sol` — regenerate the Solar System ephemeris (needs `data/external/de440s.bsp`; see the tool's docs). The golden tests fail if the shipped file and code disagree.
- `cargo clippy -p sim -p ephem-tool -p asset-tool --all-targets -- -D warnings`, `cargo fmt --all`, `tools/check_file_sizes.sh`.
- CI: a fast `sim` job (no Bevy) and a separate `game` job, on macOS and Windows. `Demo (Windows, software GPU)` tries the demo on WARP (manual; not yet working, see plan B5).
- Hooks (`.claude/settings.json`, scripts in `tools/hooks/`): `cargo check` of the owning crate after editing a `.rs` file, `cargo test -p sim` on stop. Failures are fed back (exit 2).

## Non-negotiable rules
1. **Physics has no reference bodies.** Anchors/frames exist only for precision or display; changing them must not change trajectories (invariance tests enforce this). Never add SOI logic.
2. **Determinism.** In `sim`, all trig/exp/log goes through `sim::math` (libm). No `mul_add`, no glam trig helpers (`from_axis_angle`, `slerp`), no order-dependent parallel reductions.
3. **Precision.** Never subtract two large absolute coordinates to get a precise quantity; go through the lowest common ancestor in the frame tree.
4. **The stored trajectory is the truth.** Time warp only changes how fast stored segments are sampled; the renderer never integrates. Chunked integration must equal single-pass (tests enforce).
5. **One owner per piece of math** (one Kepler solver: `sim::kepler`).
6. Tests come with the code. Small commits, one purpose each, pushed to `main`. Every commit passes fmt, clippy, tests.
7. Describe features honestly: *design / skeleton / playable / polished*.
