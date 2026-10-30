# Brief: FOUNDATION, the vessel's state split and merged by construction (2026-10-29)

D117 puts the fidelity review's foundations first, and this is the first of them: F1, the review's recommendation 1 ([fidelity-review-2026-10-29.md](fidelity-review-2026-10-29.md) §3 F1 and §4.1). Follow [agent-rules.md](agent-rules.md). Your name is FOUNDATION and your target dir is `target/agents/foundation`. QUICKFIX ([brief-quickfix.md](brief-quickfix.md)) works at the same time in `data/craft`, `crates/game/src/interface` and the systems screen, and in `systems/functions` or `command` for the launch abort. The lead stays out of `crates/sim` while you run.

## What is wrong

- `ShipState` (`crates/sim/src/craft/state.rs`) and `Runtime` (`crates/sim/src/systems/runtime.rs`) are lists aligned by index with `ShipDef`.
- At a split (`vessel/split.rs`; `ShipState::partition`, `Runtime::partition`) they are filtered by part, and **every field not named is copied whole** (`..self.clone()`): the crew, the buses, the gimbal's direction, the pressure flag. Both pieces then own the same crew.
- `Runtime::refit` silently rebuilds the lists as loaded when their lengths disagree, so state can be lost with no sign.
- There is no merge, and docking (D117's Crew mission) needs one.
- The grid fins were fixed by hand (f41f229), and `TickStart` carries some fields across an aborted bubble tick (ed5ab9f). Both should become cases of one rule.

## The work, in order

1. **Inventory.** List every field of `ShipState`, `Runtime` and any vessel-level state a split copies, in this brief's State: the field, its owner (a part, component, bus, cabin, engine, or the vessel as a whole), and how it splits today. Look for other copies the same way (`clone()` of a whole state in `vessel/`).
2. **Design, then build.** Every item belongs to an owner, and every field has an explicit split and merge, with the vessel-wide fields named one by one. **No default copy:** the compiler must refuse a new field that has no rule. For example, destructure exhaustively (`let ShipState { a, b, .. }` is not allowed; list them all). Keying by the owner's id rather than by index is the review's recommendation. If you keep the index alignment, record why in the brief.
   - The crew goes with its cabin, the fins' deflections with their parts, and the buses with theirs.
   - A field that belongs to the whole vessel needs a stated rule for each piece (for example, the attitude target goes to both pieces, each a copy, by name).
3. **`refit` never loses state silently.** A length mismatch is a bug: a debug assertion and an alarm event, or remove the path.
4. **Merge,** the inverse, for docking: two states and their definitions give one state.
5. **Tests** (in the owning modules' tests):
   - A state with every field set to a non-default value, split at stage separation and at Dragon's separation, puts each item in exactly one piece (a coverage test that fails when a field is added without a rule).
   - `merge(partition(s, k), partition(s, ¬k)) == s`, bit for bit.
   - The existing split, sequence and grid-fin tests still pass, and the warp and chunked = single-pass tests are unchanged.
6. **Docs:** the `architecture.md` row for the vessel's state, `design/ship-model.md` where it describes splits, this brief's State, and the review's §3 F1 marked with the commits.

Not in scope: F2 (one mass truth: `Vessel::propellant` against the tanks) and F3 (flight software reading `Knowledge`). Each gets a fresh agent after you. Touch `crates/game` only as far as needed to compile.

## State

- **Landed** (FOUNDATION, 2026-10-29): `craft::partition` (new) holds `ShipState::partition` and `ShipState::merge`; `Runtime::partition` and `Runtime::merge`; both destructure every field (no `..self.clone()`), so a new field fails to compile until it has a rule. Index alignment with `ShipDef` is kept (reason: the systems step state and definition in lockstep in ~every module; `ShipDef::subset` keeps the order, so `zip_merge` puts each entry back by its part). Cut joints go with their child's piece in `ShipState::severed` / `Runtime::severed` until a merge rejoins their parts. `refit` rebuilds silently only a state that never ran; any other mismatch is a `debug_assert` and an alarm event. Tests: `craft/state_tests.rs` (every field set, both splits; exactly-one-piece counts; merge round trip bit for bit by RON text; two cabins refused; a new joint unloaded).
- **Inventory, rules as built** (`partition.rs` module doc and `Runtime::partition`'s doc list them all): by part: tanks, bottles, engines, thrusters, valves, batteries, mechanisms, parts, power switches, fates, runtime tanks/engines, lost, unpowered, fins; joints within, cut joints to `severed`; cabin by its part and the crew's exposure (`CrewRun`) with it; clamps and the armed release (engines renumbered by id) with the hold-down piece; orders and `throttle_was` with the vessel's piece (others 0, matching `Controls::for_piece`); `thrust_dir` with a piece that has engines; `pressure_regulated` with a piece that has tanks; bus readouts with each piece keeping the bus; copies in every piece: accel/omega/alpha, seed, launch, stepped, events (merge: union in epoch order).
- **Left, in order:** (1) the sequence (stages, functions, position) is the one lossy field: the partition drops commands that name nothing in a piece (a cut joint's `Separate`), so the round-trip test compares it separately; making it invertible needs each stage's origin or the whole program kept with a per-piece view in the stage bar (game, QUICKFIX's area). (2) `Vessel::split` still starts each piece from `self.clone()`: make it an exhaustive destructure with a rule per `Vessel` field (crew count, chute, bubble, limbo, clock, launch …). (3) `TickStart`'s carry (ed5ab9f) is a tick rollback, not a split; leave it. (4) Docs: `design/ship-model.md` §4/§6.3 on splits, the review's §3 F1 marked.
- **Open for the owner:** keyed lists (fates, lost, unpowered) come back from a merge in part order (their order carries no state); a cut bus's readout is copied to both pieces until their next power step; a merge refuses two cabins (one cabin per ship state).
