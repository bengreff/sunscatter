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

(Update as you land: commits with hashes, the inventory, what is left in order.)
