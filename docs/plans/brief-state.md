# Brief: STATE, one state per vessel (2026-10-31)

D121: "`ShipState` and `Runtime` merge into one, grouped by subsystem, split and merged by construction (F1). Ids are strings only at the edges (files, saves, the interface, MCP) and typed handles inside." It is the first agent of the sim-wide wave. LAYERS ([brief-layers.md](brief-layers.md)) runs after you and moves the software into D128's layers on top of your state. Follow [agent-rules.md](agent-rules.md). Your name is STATE and your target dir is `target/agents/state`: start warm with `mv target/agents/onekind7 target/agents/state`.

ONEKIND has deleted the test craft and the made-up path (6741e5e, 15085cd, 8ef3c46), so there is one kind of craft: every vessel has a ship state. You are the only sim-wide agent running. INTERFACE-4 works in `game::systems_screen`, `game::interface`, MCP's `game::agent` and the game's dead branches for a ship-less craft (ONEKIND-7's Left 1): check `git status` and its brief's State before editing a file there. Run pushes with `run_in_background` (the pre-push hook queues on a lock).

## Read first

- D121, D127, D128, D129, D063 (extended: a save holds no static data), D104, D119 in `docs/decisions.md`.
- The fidelity review's F1, F4 and F8 (`docs/archive/research/fidelity-review-2026-10-29.md` §3) and FOUNDATION's brief (`docs/archive/briefs/brief-foundation.md`: why the lists stayed aligned by index with `ShipDef`).
- `docs/systems/ship.md`, `docs/technical/ship-model.md` §5, `docs/technical/software-layers.md` (where LAYERS will put the software, so your groups fit it), `docs/technical/warp-invariance.md` §5 (the laws ADVANCE will rewrite).
- Code: `sim::craft::{state, ship, partition, command}`, `sim::systems::{runtime, ship, command, propulsion, power, structure, crew, flight, estimate}`, `sim::vessel::{ship, ship_tick, split}`, `sim::save`, and how the game and MCP read them (`game::systems_screen`, `game::interface::part_card`, `game::agent`).

## The work, in order

0. **ONEKIND's sim leftovers first** (ONEKIND-7's State in `brief-onekind.md`, Left 2): `Part.tank`/`rcs`, `CraftFile`'s `propellant`/`attitude_control`, `breakage`'s one-part rules, and any other placeholder field it lists. They are the made-up path's last traces in the state you are about to merge.
1. **The plan, in this brief's State, before code:** the merged state's groups by subsystem (tanks, bottles, engines, thrusters, valves, batteries, mechanisms, joints, parts, cabin and crew, structure, failures, the program), what moves out of `Runtime` into each, and what is derived each tick and is not state. Then the handle types (`TankId`, `EngineId`, `PartId`, …), their one place of resolution from a string at the edges, and the linear searches each removes (count them before and after). Then where the engine rule lives once (F8: `ShipState::apply`'s instant ignite and deploy, `craft::engine::ship_thrust` against `systems::propulsion`).
2. **Merge `ShipState` and `Runtime`** into one state grouped by subsystem, in steps that each pass every test. Keep the partition and merge exhaustive (no default copy) and bit-exact; keep the warp, chunked = single-pass and determinism tests green, `tests/warp_course.rs` included.
3. **Typed handles inside.** Strings only in files, saves, the interface and MCP. `Runtime::lost` and `unpowered` become typed sets. Measure the per-tick cost on the stress craft at 300 and 1,000 parts before and after (`bench_scale`), and record it in `scale-performance.md`.
4. **The engine rule in one place** (F8), with its table tests.
5. **Saves:** the save holds the vessel's state, trajectory and thermal state, and no static data (body data, part behaviour, designs it can rebuild). A load then run equals a run (a test). Keep it simple (owner, 2026-10-31: "Don't make things harder than you need to").
   Then **the playtest save** (ONEKIND-7's Left 3; `./run play` has not loaded since 15085cd): fly the Falcon 9 to orbit headless through MCP (`docs/technical/testing.md`, debug off; INTERFACE-2's insertion script and PLANNER-4's orbit save are under the session scratchpad `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/` in `interface2/` and `planner4/`), separate Dragon, save in your format, and point `./run play` at it. One game at a time, under the lock in agent-rules.
6. **Docs:** `owners.md` rows, `systems/ship.md`, `ship-model.md` §5, the save's doc, and every mention of `Runtime`.

Small commits, each passing fmt, clippy, `cargo test -p sim` and the game's tests it touches. End with the report agent-rules asks for.

## State

### STATE-1 (2026-10-31)

**The plan (step 1).**

*The merged state, by subsystem.* `ShipState` keeps its per-item lists aligned with `ShipDef`'s (FOUNDATION's reason stands: the systems step definition and state in lockstep), and each item takes its run's fields, so `Runtime` disappears field by field. Groups (what comes in from `Runtime`):
- **tanks** (`Tank`: liquid, liquid_k, ullage_moles, ullage_k + `TankRun`'s drift, drift_speed, pressurant_in, vented) and **bottles** (unchanged); **plumbing** (`pressure_regulated`).
- **engines** (`Engine`: status, timer, ignitions, throttle, gimbal, selected + `EngineRun`'s level, cutoff, reason, settling); **propulsion** (`release_armed`, `release_engines` as `Vec<EngineId>`, `throttle_was`, `thrust_dir`).
- **thrusters**, **valves** (unchanged); **surfaces** (the fins' deflections, `fins`, one per `ShipDef::surfaces` item, never empty).
- **batteries**; **power** (the switches `power`, `unpowered`, `buses`).
- **mechanisms** (unchanged).
- **joints** (`Joint`: parent, child, intact + `JointRun`'s force, moment, ratio, to_ultimate) and `severed` (one list of `Joint`, not two); **parts** (unchanged); `clamped`.
- **cabin** (`CabinState` + `CrewRun`'s held and injuries: the crew's exposure goes with the cabin by construction, which removes the partition's special case).
- **failures** (`fates`, `lost`, `seed`, `launch`).
- **program** (`sequence`, `stages`, `functions`, `orders`).
- **felt** (`accel`, `omega`, `alpha`: the last tick's motion), `stepped`, `events` (the log).

Not state (derived each tick, never saved): the feeds (`plumbing::Feed`), the thrust (`propulsion::output`), the loads' inputs, the bus voltages (`BusRun` stays a readout of the last power step, saved because the screens read it between steps; derivable, left as is).

*Handles.* `TankId`, `BottleId`, `EngineId`, `ThrusterId`, `ValveId`, `BatteryId`, `MechanismId`, `SurfaceId`, `JointId`, `ComponentId` (newtypes over `u32`: the index into `ShipDef`'s list) and `PartId` (`Part::id`, the design index pieces keep). One owner `craft::handle` with the one resolution from a string: `ShipDef::resolve(&str) -> Option<Item>` and the glob `ShipDef::pick(pattern) -> Vec<Item>` (today's `matches` loops). Strings stay in files, saves (ids, so a save names its items), the interface and MCP. Inside: `Lost`/`unpowered` become sets of `Item` (an owner: a part or a component), `Fate.owner` an `Item`, `Effect::{Separate, Snap, Destroy}` take a `PartId`, `release_engines` `EngineId`s. Linear searches by id: **108 in sim** (regex `(position|find|any|filter|find_map)(|x| x.(id|owner|name|part) ==`, tests excluded), **about 70 of them in `systems/` and `vessel/`** (the review's count), run each tick: `failure` 20, `power` 11, `runtime` 6, `structure` 5, `command` 4. Target: none per tick (the build's stay, they run once).

*The engine rule in one place (F8).* `systems::command::execute` owns ignite, shutdown, throttle, select and the mechanisms (start sequence, ignitions, the clamps' interlock, cycles). `ShipState::apply` keeps only the plain edits with no transient (valves, thrusters, power switches, separations, the clamps, the sequence's edits) and refuses an engine or mechanism command; `ShipState::next_stage` (only two tests use it) is deleted for `systems::command::next_stage`. `craft::engine::ship_thrust` (only a test uses it; the vessel flies `propulsion::output`) is deleted and its test moves onto `propulsion::output`.

*Order.* Each a commit passing every test: (2a) tanks, (2b) engines and propulsion, (2c) joints and severed, (2d) surfaces, power, cabin and crew, failures, program, felt: `Runtime` deleted with its partition and merge folded into `craft::partition`; (3) handles, then the sets, then the per-tick searches, with the cost after; (4) F8; (5) the save and the playtest save; (6) docs.

**Cost before** (`bench_scale craft parts=300,1000 runs=1`, release, HEAD bcc3cc7, load 8–12, one run):

| parts | live tick: entry | ascent M 1.2 | vacuum burn | pad | thermal step 20 ms | coast step | lattice 600 s first, tenth | tumbling day, per point | 10⁶ s holding |
|---|---|---|---|---|---|---|---|---|---|
| 300 | 16.6 ms | 12.6 ms | 11.1 ms | 12.9 ms | 13.4 ms | 0.51 ms | 6.6, 4.8 ms | 4.3 ms | 5.5 s |
| 1,000 | 90.4 ms | 75.2 ms | 62.4 ms | 73.7 ms | 84.3 ms | 3.2 ms | 39.9, 31.3 ms | 27.7 ms | 37.3 s |

Read the ratios, not the last digit (other agents were building). The handles' gain shows in the live ticks and the lattice (the systems run in both).

**Landed** (each passed fmt, clippy, the whole sim suite, and the game's check and its touched modules' tests):
- 4360a3d **step 0**, ONEKIND-7's sim leftovers: `Part::{tank, rcs}` (`PartTank`, `RcsFile`), `CraftFile::{propellant, attitude_control}`, `GeometryFile::tank`, `CraftDef::tanks`' ship-less branch, `ship_aggregate`, `MassModel::new`, `VolumeGrid::{new, propellant}`, `CraftDesign::tank_capacity` and `node_capacity`'s fill, the design's one-part branches, the breakage's one-part and ship-less rules (`check_overheat`), `cell_options`' id. Docs: parts.md, proximity.md.
- 4bf532a the plan above.
- 7ee1753 **2a** tanks: `TankRun` into `TankState` (drift, drift speed, the ledgers).
- 2948566 **2b** engines: `EngineRun` into `EngineState` (level, cutoff, reason, settling).
- 2f16f26 **2c** joints: `JointRun` into `JointState` (force, moment, ratio, to_ultimate), one `severed` list; `Runtime::{initial, fits, refit}` gone (`ShipState::fits` checks every list); script `scratchpad/state/step2c.py`.

**Left, in order** (scratchpad `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/state/`):
1. **2d**, the rest of `Runtime` into `ShipState`, grouped as the plan says, and `systems/runtime.rs` deleted: `step2d.py` is written against the tree after 2c and untested. It moves the fields, the types (`Order`, `OrderKind`, `Event`, `Lost`, `EVENTS_KEPT`), the methods (`out`, `event`, `order`, `next_order`, `due`), `renumber` and the partition and merge rules into `craft::state` and `craft::partition`, and renames `.runtime.` everywhere. By hand after it: the three `let r = &mut state.runtime;` / `let run = &state.runtime;` bindings (`systems/ship.rs:61`, `game/src/interface/menus/{power, crew}.rs`), the coverage test's `Runtime` half (`state_tests.rs`: fold its field list into `ShipState`'s), and `systems/mod.rs`' re-export. Run it with `python3 step2d.py` from the repo root, then `cargo check`.
2. Step 3 (handles), 4 (F8), 5 (saves, the playtest save), 6 (docs) as the plan says. The docs naming `Runtime` (owners.md:64, 112, 119; systems/ship.md:29; ship-model.md §5; power.md:5; life-support.md:5; systems-screen.md:21, 51; crew.md:21) wait for 2d.
3. The cost after: rerun `bench_scale craft parts=300,1000 runs=1` after step 3 and add the row to `docs/technical/scale-performance.md`.

**Open for the owner (STATE-1):**
- A craft of one part now takes the parts' path everywhere: its interior is judged by its time at temperature (`systems::failure`), not at once at its service limit, and when its one part is destroyed it flies on as a wreck by the split (it was destroyed whole). The only difference is the interior rule, which now matches every other craft.
- The crew count of a built craft is the file's (`file.crew`), as before; a piece's is its parts' seats.

### STATE-2 (2026-10-31)

**Landed** (each passed fmt, clippy, the whole sim suite and the game's check and touched modules' tests):
- 3fc9a0a **2d**: the rest of `Runtime` into `ShipState`, by subsystem; `systems/runtime.rs` deleted; its partition and merge rules in `craft::partition`'s one destructuring; the coverage test lists every field. STATE-1's `step2d.py` plus `scratchpad/state2/step2d_hand.py` (the by-hand fixes). Docs naming `Runtime` updated (owners.md, ship.md, crew.md, power.md, life-support.md, engines.md, failure-modes.md, ship-commands.md, software-layers.md); `ship-model.md` §5's heading "Runtime" means the run-time models and stays.
- 1689247 **3a**, typed owner handles: `craft::handle` (`Owner::{Part(Part::id), Component(ComponentDef::n)}`, `n` the component's number in the design, kept by a split's pieces; `part_index`, `component_index` binary searches; `owner_id`, `owner_part`, `ratings_of`, `failures_of`, `component_of`; `owner_named` the one resolution from a string). Items carry their owner (`Owned`, `EngineDef`, `ThrusterDef`, buses' and links' members); `Lost` (lost, unpowered) and `Fate` hold handles (a fate: owner plus its mode's index in its owner's modes; the thresholds still hash the id and mode name, so the draws are unchanged); `draw_fates` does nothing once there is a fate per mode. 14 per-tick searches of a part's state are `part_index`. Linear searches by id in sim (STATE-1's pattern, tests excluded): **107 → 81**; per tick, the id strings left are `plumbing::heated_inside` and `estimate`'s joints by name. The game names owners only through `owner_id`.

**Cost.** A profile of the baseline's live tick at 1,000 parts (`bench_scale profile=live,1000`, `sample`, 9,179 samples): `ThermalNetwork::step_to` 62 %, `VolumeGrid::with_parts` 7 %, aero 4 %, the ship systems (`systems::ship::tick`) **about 3 %**. So the handles cannot move the live tick much; the lever at scale is the thermal network (PERF). Before and after: `docs/technical/scale-performance.md` ("One state per vessel").

**Left, in order** (scratchpad `…/scratchpad/state2/`):
1. **3b**, the rest of the handles: `TankDef` and `BottleDef` take their `owner` so `plumbing::heated_inside` compares handles; `estimate`'s joints by name and `release_engines` as typed indices (or a `JointId`/`EngineId` newtype over the index, renumbered at a split as `renumber` does); the command selectors (`matches` loops in `ShipState::apply`, `systems::command::apply`) run once per command, at the edge: leave them.
2. **Step 4 (F8)** as STATE-1's plan says (`ShipState::apply` refuses engine and mechanism commands; `next_stage` and `craft::engine::ship_thrust` deleted; table tests).
3. **Step 5**: saves without static data, a load-then-run test; then the playtest save (Dragon alone in orbit) and `./run play` pointed at it.
4. **Step 6**: the save's doc; `ship-model.md` §5 on the one state.

**Open for the owner (STATE-2):**
- `Part::id` stays a plain `u32`, not a `PartId` newtype: it is already the handle (the design index a split's pieces keep, found by `part_index`); a newtype would touch about 290 `.part` sites in sim and game for type safety alone, in the wave where LAYERS moves the files next. The owners became typed because they were strings.
- The handles are for correctness and D121, not speed: the ship systems are about 3 % of a 1,000-part live tick (above).

### STATE-3 (2026-10-31)

**Landed** (each passed fmt, clippy, the whole sim suite, and the game's touched modules' tests; step 4's game tests in a private mirror, INTERFACE-7's unfinished `menus/rcs_data.rs` stopping the shared tree's game build):
- ef5f344 **3b**: `TankDef::owner`, `BottleDef::owner` (plumbing's `heated_inside` reads the handle); one resolution of a joint's name, `ShipDef::joint_named`, for every lookup (the state's plain effect, the program, the estimate's walk, the build, the staging bar and its icons); `release_engines` is a flag on each engine, `EngineState::awaited`, which splits and merges with the engines by construction, so `partition::renumber` and its search by engine id go (not typed ids: nothing left to renumber). Searches by id in sim (STATE-1's pattern, outside test modules): 67 → 62.
- f2ff50f **4, F8**: the engine rule in one place. `systems::command::apply` owns ignite, shutdown, throttle, select and the mechanisms; `ShipState::apply` refuses them (it keeps the plain edits with no transient); `ShipState::next_stage` and `craft::engine::{ship_thrust, engine_of}` deleted (their tests on `systems::command::next_stage` and `propulsion::output`). Table test `systems::command::tests::the_engine_rule` (17 cases: charges, the pad's supply, hypergolic, lit, tail-off, failed, controller lost, floating propellant and its wait, shutdown, throttle; selection, a deployment at its actuator's rate, the clamps' leg interlock, `apply`'s refusals). The game's tests set the states they read (`test_rocket::{running, deployed, next_stage}`). Docs: owners.md, parts.md, engines.md, open-issues.md.

**Left, in order** (scratchpad `…/scratchpad/state3/`: `sync` lays files over a mirror of HEAD in `mirror/`, `data/` linked):
1. **Step 5, saves without static data.** What a save holds now (PLANNER-4's orbit save, `scratchpad/planner4/home/saves/orbit.ron`, by vessel field): the trajectory 765 KB, thermal 25 KB, ship state 14 KB, `craft` 13–23 KB. The design (ShipDef, geometry, bake, network) is already rebuilt from the craft id (`CraftParams::design` and `Limbo::design` are `#[serde(skip)]`, `registry::craft_of`). The static data left:
   - `Vessel::craft` (`CraftParams`: mass model, drag area, chute spec, thermal limits, impact speed, contact file and points, `bottom_z`) is all `Craft::params()` of its id. Save it as the id: `#[serde(into = "CraftId", try_from = "CraftId")]`, `TryFrom` through a fallible `registry::try_craft_of` (`craft_of` panics), `SAVE_VERSION` 15. A load then builds each vessel's design, as a split does.
   - The deleted test craft's lumped power placeholder runs for every vessel and is saved: `Systems::power` (`systems::power::{Power, PowerSpec}`, D033: a 10 m² tracking panel, 10 kWh, 1 kW of load), `Part::power` (`PartPower`) and the split's `Shares` (`vessel/split.rs`). `Vessel::powered()` (D083's debris deletion, `controllable()`, the bubble) reads it, not the ship's batteries and solar arrays. Fix the model (no patchwork): delete them; `powered()` from the ship state (a battery with charge, or a solar array), one pure rule with a table test in `systems::power`; `Systems` keeps the lattice epoch only (its `Env` then has no user: drop it); `examples/long_run.rs` reads the batteries.
   - A load then run equals a run: `tests/save_load.rs` has it for the small rocket (coasting, landed, powered); add the Falcon 9 on the pad through its ignition, and a check that the saved text holds no contact points.
2. **The playtest save**: `./run play` copies `docs/plans/playtest-2026-10-01-orbit.ron` to its home's quicksave (Cmd+L at the pad; see `run`). Regenerate it in the new format as Dragon alone in orbit: `./run headless <port>` (debug off, `docs/technical/testing.md`), the Falcon 9 to orbit with INTERFACE-2's `scratchpad/interface2/insert.py` (its port at the top; `m` and `v` are its MCP helpers), separate Dragon, `save_game`, copy it over the playtest file, and check `./run play` loads it (headless: `load_game`).
3. **Step 6**: the save's doc (`sim::save`'s module doc, `docs/technical/` on saves) and `ship-model.md` §5 on the one state.

**Open for the owner (STATE-3):**
- The pad release's engines are a flag on each engine, not a list of typed ids: a flag needs no renumbering at a split or merge (D121's "by construction").
- The game's tests of readouts now set the engine and leg states they read directly, instead of through `ShipState::apply`'s instant ignite and deploy (which is gone); after the ignition the staging bar reads "4/4 starting", as the vessel does.

### INTERFACE-6 in sim (2026-10-31)

INTERFACE-6 (game crate, [brief-interface.md](brief-interface.md)) adds one function to `sim::systems::estimate` for interface-map finding 41: `estimate::aboard`, the mass aboard now with the precision the tank sensors give (the estimate's own walk and bounds), so the systems screen and the staging bar read one rule. An added function and its test in `estimate_tests.rs`; nothing else in sim changes. STATE's 3b (`estimate`'s joints and `release_engines`) is untouched by it.
