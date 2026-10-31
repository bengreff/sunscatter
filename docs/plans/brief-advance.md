# Brief: ADVANCE, one advance law per subsystem and the same course at every warp (draft, 2026-10-30)

**Drafted by AUDIT; reviewed by the lead 2026-10-31.** You run after LAYERS, alone in the sim.

Two owner decisions:
- **D121:** "one advance law per subsystem". Each subsystem advances over any interval, exact or coarse-grained, chunked = single pass. Live ticks, the systems lattice and coasts only choose the interval.
- **D129:** "EXACTLY the same behavior at 100,000 warp as you would get at 1x warp … Deterministic course of events is only broken when the player inputs something." A craft's model is chosen by its state, never by the warp. Where a model cannot give the same events at high warp, the warp is restricted, and the model is not changed.

AUDIT mapped the code and proved what holds in [warp-invariance.md](../technical/warp-invariance.md). Two days of the Falcon 9's second stage with Dragon (a restart, a separation, a failure, the lattices) give the same bits as one pass at 100×, 10⁴× and 10⁵×, and at 1× over the first 3,100 s (`crates/sim/tests/warp_course.rs`). It found six divergences (D1–D6) and the model gaps that ADVANCE closes.

Follow [agent-rules.md](agent-rules.md). Your name is ADVANCE and your target dir is `target/agents/advance`: start warm from the newest sim target the lead names. You run after LAYERS ([brief-layers.md](brief-layers.md)); its `ship_os::warp_stop` is what step 4 makes exact.

## Read first

- D121, D127, D128, D129, D119, D081, D082, D034, D112 in `docs/decisions.md`.
- [warp-invariance.md](../technical/warp-invariance.md) (all of it), [software-layers.md](../technical/software-layers.md) §4, [limbo.md](../technical/limbo.md) §3, [ship-model.md](../technical/ship-model.md) §5.10, [scale-performance.md](../technical/scale-performance.md) §2–4.
- Code:
  - `sim::vessel::{coast, live, systems, ship_tick, helm, bubble, hold}`, `sim::systems::{mod, ship, power, crew, coast_law, failure}`;
  - `game::state` (`step`, `advance`, `rails_block`), `game::fleet`, `game::commands` (`InFlight`), `game::navball::feed_hold`, `game::debris`, `game::agent::session` (`run_until`).
- `crates/sim/tests/warp_course.rs`: its game emulation is your harness. Keep it in step with any change you make to the game's frame.

## The work, in order

1. **Arrivals at their epochs** (D2). Every setting and command that arrives with light delay becomes an order at its arrival epoch, and the advance stops there as `order_stop` does for ship orders:
   - the helm's queue (`vessel::helm`);
   - the game's `InFlight` commands (plans, holds, the sequence's edits, debug).
   
   The same stop serves the wake epochs of LAYERS' programs. Un-ignore `d2_…`.
2. **Rules between craft at their epochs** (D3, D1).
   - Each vessel gives the epochs where it can go live inside a frame: its next order or arrival, its live boundary or surface end, its next ignition. All are already in its trajectory and runtime. The fleet caps the frame's target, for the craft within the bubble's reach, at the earliest of them, so the bubble forms at the right epoch (`bubble::epoch`'s "judged later" goes).
   - The TARGET hold on a vessel names the vessel and reads its stored trajectory at each control tick, from a snapshot taken at the frame's start, instead of the game's per-frame `TargetState`.
   - Un-ignore `d3_…` and `d1_…`. D127's parallelism stays: the results must not depend on threads (the fleet's tests).
3. **One law per subsystem** (D121, warp-invariance.md §5), each with a test at 1× against 10⁵× and against one pass in `warp_course.rs`'s harness:
   - power and life support step over the span since they last stepped, so going live loses nothing (`systems/ship.rs` `tick`);
   - power's shedding no longer depends on the step's length: the battery's empty epoch in closed form;
   - the landed vessel's temperatures step (today never);
   - failures on the lattice: hazards accumulate on coasts and landed, a fate's crossing epoch is t = (E − H)/λ for a constant λ, bursts come from the coast law's pressures, time at temperature runs on the coast lattice.
4. **Events at their own epochs** (warp-invariance.md §7, point 3; with LAYERS' `warp_stop`). Each coarse law gives its next event's epoch in closed form or by bisection inside its step: a battery empty, a CO₂ or O₂ limit, a hazard reaching its fate, a burst. The vessel's advance stops there, as at the live boundary. The game drops the warp and holds the clock at that epoch, for the events `warp_stop` stops on.
5. **The rest:**
   - D4: debris deleted at the epoch it crosses 10 km, from the stored trajectories.
   - D5: `run_until`'s conditions stop at the condition's epoch where a law gives one.
   - D6: rotation input held at rails warp refuses rails warp, as thrust does, instead of being zeroed (AUDIT's default under D129: restrict the warp, don't change the model; the owner may choose that it acts as at 1×).
6. **Warp restrictions where a model cannot be made exact:** a rule in `game::state::rails_block` with a reason the player sees, never a changed model (D129). List each one.

Small commits, one purpose each. `cargo test -p sim` and `-p game`'s touched tests pass at every commit. Every divergence you find goes into warp-invariance.md with a reproduction. Docs in the same round:
- warp-invariance.md's tables and status;
- `docs/systems/motion.md` (D121's "not built");
- the owners table (`docs/owners.md`): the arrival order, the frame's live epochs.

## State

(Update as you land: commits with hashes, what is left in order, the warp restrictions added, Open for the owner.)
