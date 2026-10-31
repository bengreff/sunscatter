# Brief: STATE, one state per vessel (2026-10-31)

D121: "`ShipState` and `Runtime` merge into one, grouped by subsystem, split and merged by construction (F1). Ids are strings only at the edges (files, saves, the interface, MCP) and typed handles inside." It is the first agent of the sim-wide wave. LAYERS ([brief-layers.md](brief-layers.md)) runs after you and moves the software into D128's layers on top of your state. Follow [agent-rules.md](agent-rules.md). Your name is STATE and your target dir is `target/agents/state`: start warm with `mv target/agents/onekind4 target/agents/state` if it exists, else cold.

You run after ONEKIND has deleted the made-up path and the test craft, so there is one kind of craft: every vessel has a ship state. You are the only sim-wide agent running; INTERFACE may still be at work in `game::systems_screen` and `game::interface` (check its brief's State and `git status` before editing a file there).

## Read first

- D121, D127, D128, D129, D063 (extended: a save holds no static data), D104, D119 in `docs/decisions.md`.
- The fidelity review's F1, F4 and F8 (`docs/archive/research/fidelity-review-2026-10-29.md` §3) and FOUNDATION's brief (`docs/archive/briefs/brief-foundation.md`: why the lists stayed aligned by index with `ShipDef`).
- `docs/systems/ship.md`, `docs/technical/ship-model.md` §5, `docs/technical/software-layers.md` (where LAYERS will put the software, so your groups fit it), `docs/technical/warp-invariance.md` §5 (the laws ADVANCE will rewrite).
- Code: `sim::craft::{state, ship, partition, command}`, `sim::systems::{runtime, ship, command, propulsion, power, structure, crew, flight, estimate}`, `sim::vessel::{ship, ship_tick, split}`, `sim::save`, and how the game and MCP read them (`game::systems_screen`, `game::interface::part_card`, `game::agent`).

## The work, in order

1. **The plan, in this brief's State, before code:** the merged state's groups by subsystem (tanks, bottles, engines, thrusters, valves, batteries, mechanisms, joints, parts, cabin and crew, structure, failures, the program), what moves out of `Runtime` into each, and what is derived each tick and is not state. Then the handle types (`TankId`, `EngineId`, `PartId`, …), their one place of resolution from a string at the edges, and the linear searches each removes (count them before and after). Then where the engine rule lives once (F8: `ShipState::apply`'s instant ignite and deploy, `craft::engine::ship_thrust` against `systems::propulsion`).
2. **Merge `ShipState` and `Runtime`** into one state grouped by subsystem, in steps that each pass every test. Keep the partition and merge exhaustive (no default copy) and bit-exact; keep the warp, chunked = single-pass and determinism tests green, `tests/warp_course.rs` included.
3. **Typed handles inside.** Strings only in files, saves, the interface and MCP. `Runtime::lost` and `unpowered` become typed sets. Measure the per-tick cost on the stress craft at 300 and 1,000 parts before and after (`bench_scale`), and record it in `scale-performance.md`.
4. **The engine rule in one place** (F8), with its table tests.
5. **Saves:** the save holds the vessel's state, trajectory and thermal state, and no static data (body data, part behaviour, designs it can rebuild). A load then run equals a run (a test). Keep it simple (owner, 2026-10-31: "Don't make things harder than you need to").
6. **Docs:** `owners.md` rows, `systems/ship.md`, `ship-model.md` §5, the save's doc, and every mention of `Runtime`.

Small commits, each passing fmt, clippy, `cargo test -p sim` and the game's tests it touches. End with the report agent-rules asks for.

## State

(Update as you land: the plan, commits with hashes, the cost before and after, what is left in order, Open for the owner.)
