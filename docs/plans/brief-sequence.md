# Brief: SEQUENCE, the sequence bug sweep (2026-10-29)

The owner's queue, item 2 ([interview-2026-10-29.md](interview-2026-10-29.md); the bugs are listed in [open-issues.md](open-issues.md), "From the night's review"). Follow [agent-rules.md](agent-rules.md). Your name is SEQUENCE; your target dir is `target/agents/sequence`. The lead stays out of `crates/sim` while you run.

## The work, in order

Each bug gets a failing test first (in the owning module's tests), then the fix, then one commit.

1. **Pad abort past Liftoff.** After a pad abort the sequence index is past Liftoff, so Space would then run MECO and stage separation on the pad. Find the path: the Falcon 9's "Launch abort" function, and whatever moves `ShipState::sequence` (`sim::systems::command`). A function must never move the sequence. If an abort on the pad does need to change it, record the rule you chose under "Open for the owner" in [fixes-2026-10-28.md](fixes-2026-10-28.md).
2. **`Vessel::set_stages` keeps the index.** Editing the stages in flight only shortens the index, so stages can be skipped or repeated. The stages already run must stay as they were (D111: `edit_sequence` edits only stages not yet run), and the index must still count exactly those.
3. **A release armed with no engine lit stays armed.** The vessel is then held in live ticks forever, and rails warp is refused. The real pad's rule: the clamps let go only at full thrust, and when the engines stop, the release is disarmed (with an event in the log). Keep D114: AGAIN on Ignition relights the engines on the pad, and running Liftoff again re-arms the release.
4. **Grid-fin deflections after a split.** `Runtime::partition` copies them whole, so they can misalign. Partition them by each piece's fins.
5. **An aborted bubble tick** does not restore the `ShipState` changes made in `begin_tick`. Restore them, or make `begin_tick` not mutate before the tick commits.

The rules that matter here (CLAUDE.md): determinism (`sim::math`, no `mul_add`), one owner per piece of math, and the stored trajectory is the truth. Run `cargo test -p sim` after each fix, and `cargo clippy -p sim --all-targets -- -D warnings`.

## Files

Yours: `crates/sim/src/vessel/*`, `crates/sim/src/systems/{command,runtime,propulsion}.rs`, `crates/sim/src/craft/state*.rs`, and this brief. Docs: remove each fixed bug from `open-issues.md`; update `docs/architecture.md` rows where an owner changes.

## State

(Update as you land: commits with hashes, what is left.)

- **1, pad abort past Liftoff: done** (1f9f5b3; reverted by the lead's b48bc21, a commit from the stale shared index, restored in 0a5fc91). The path was the pad's own abort (`propulsion::release`: an engine stopped before full thrust), not a function: Liftoff had run, so the index stayed past it. Also a split while the release was armed (the "Launch abort" function on the pad) disarmed it silently. Now `propulsion::disarm` (both paths, with an event) puts the sequence back to the last stage run that releases the clamps (`command::release_undone`); the capsule leaving the pad is no longer marked clamped (`ShipState::partition`). Functions never moved the sequence (tested). Rule chosen, for the owner: see below.
- **2, `set_stages` keeps the index: done** (133079f). A whole list (the systems screen's) whose first stages are not exactly those already run is refused with a reason; the index stays and counts them. The systems screen's editor still lets the player edit stages already run in its draft (it then gets the refusal toast): the lead may want it to lock them.
- **3, a release armed with no engine lit: done** (08b2fc1). Armed with none lit at the end of its tick (and none settling to start), it is disarmed with an alarm event, and Liftoff is next again (rule 1's `disarm`). Engines stopping while it waits (the throttle closed on the pad) were already a pad abort; tested with D114's recovery (AGAIN on Ignition, Liftoff again).
- **4, grid-fin deflections after a split: done** (f41f229). `Runtime::partition` takes the whole ship's definition and keeps each piece's own surfaces' deflections, aligned with its definition; `ShipState::partition(kept, whole, piece)`.
- **5, an aborted bubble tick: done** (this commit). `begin_tick` no longer changes the vessel but its attitude control (which an aborted tick already restored): the parachutes step on a copy, the grid fins' deflections and the gimbal's direction travel in `TickStart` and `finish_tick` commits them (`Vessel::hold_effectors`). The same for a solo tick that fails. `check_overheat` moved to `vessel::breakage` (live.rs at the size limit).
- Nothing left of the brief.

### Open for the owner (to copy into fixes-2026-10-28.md once the lead has committed it)

- **After a pad abort, Liftoff is next again** (SEQUENCE, logical default): a release that did not let go (a pad abort, the vessel split while it was armed) did not happen, so the sequence goes back to the stage that armed it, and stages run after it (MECO pressed during the spool-up) are to run again. The Ignition stage counts as run: AGAIN on it relights the Merlins from the pad's supply (D114), then Space runs Liftoff. The alternative was to recycle to the Ignition stage (Space, Space as at first).
