# Warp invariance: the same course of events at every warp (D129)

**Status: audit** (AUDIT, 2026-10-30, at 14c2a4d). This doc maps how every subsystem advances, whether frame-sized chunks give the same bits, where events are found, and where anything is decided from the warp. It proves what it can with `crates/sim/tests/warp_course.rs`. Nothing here is fixed yet: ADVANCE fixes the sim's side and LAYERS the warp stop ([brief-advance.md](../plans/brief-advance.md), [brief-layers.md](../plans/brief-layers.md)). The system: [motion](../systems/motion.md).

D129: "EXACTLY the same behavior at 100,000 warp as you would get at 1x warp." Only the player's input changes the course of events, and that input lands at the epoch of the frame it arrives in (the one accepted break, coarser at high warp). A craft's model is chosen by its state, never by the warp. Where a model cannot give the same events at high warp, the warp is restricted, and the model is not changed.

"The same" here means the same bits: every vessel of the fleet equal (`Vessel: PartialEq`) at the same epoch, for the same inputs at the same epochs.

## 1. How a frame advances

The game, each frame (`crates/game/src/state.rs` `advance` and `SimState::step`; `crates/game/src/fleet.rs`):

1. **The span:** `dt × warp`, at most twice what the clock moved last frame (`fleet::frame_span`); the warp is capped at 4× while `rails_block` finds a reason (any vessel live, the active one thrusting or below its rails floor, a vessel held on the pad).
2. **The controls:** each vessel's acting controls from the clock (`vessel::fleet_controls`; an uncrewed vessel's through its helm's light delay). At rails warp the rotation input is zeroed (`state.rs:362`).
3. **The bubble first** (`vessel::bubble::advance`): craft near a live controllable craft fly its tick lattice together.
4. **The fleet:** every vessel, independently and in parallel, to the frame's target by `Vessel::advance_frame` in chunks of 2,000 budget units. A vessel that goes live inside the frame with more than `LIVE_STOP` (1 s) left stops there (`coast.rs:190`).
5. **The clock:** the target, held back at the slowest vessel that trails it by more than a tick (`fleet::clock_after`).
6. **After the frame:** breakups are built on a task and split in a later frame (the vessel waits at its breakup epoch; `game/src/debris.rs`), dead debris 10 km from any craft is deleted (`debris.rs:153`), and the look-ahead extends the drawn line.

Inside `Vessel::advance_budgeted` (`vessel/coast.rs`): the orders due now are applied, the advance stops at the next order's epoch while the vessel advances continuously, then the phase advances (landed, crashed, live ticks or the coast with its lattices). It stops at the target, out of budget, at the live boundary, or where the vessel woke.

**The sim never reads the warp.** Outside tests, `crates/sim` has no mention of it. Every model is chosen by state: thrust, air (the live boundary of `vessel::limbo`), a surface, a live craft within 10 km, the pad's clamps.

### The lattices

Everything that steps, steps on a lattice fixed by the vessel's state and history, never by the frames.

| Lattice | Spacing | Anchored at | Owner |
|---|---|---|---|
| Live tick | 20 ms (`vessel::TICK`) | the epoch the vessel went live | `vessel::live` |
| Contact substeps | `contact::SUBSTEPS` per tick | the tick | `vessel::live`, `bubble` |
| Systems lattice | 60 s (`systems::STEP`) | the vessel's first advance (`Systems::start_at`); pieces keep their parent's | `systems`, `vessel::systems` |
| Coast thermal lattice | adaptive, twice the skin's shortest radiative time, 1–600 s; points skipped while the sunlight holds | its last point | `vessel::aerothermal` |
| Attitude control ticks | `attitude::FOLLOW_STEP` | the hold's base | `vessel::attitude`, `vessel::coast` |
| Coast law spans (tanks, D119) | ≥ 1 h | one of the segment's integration samples | `systems::coast_law` |
| Landed proper time | `LANDED_STEP` | the clock's base | `vessel::clock` |
| Coast integration | DOP853's own steps, chunked = single pass | the segment's start | `vessel::segment`, `integrate` |

## 2. Subsystem by subsystem

"Chunks" says whether the same span in frame-sized chunks gives the same bits today, and on what evidence. "Events" says where an event of that subsystem is found.

| Subsystem | Live (ticks) | 60 s lattice / coast | Chunks | Events found at | From the warp? |
|---|---|---|---|---|---|
| **Thermal** | network stepped each tick (`aerothermal`) | coasting: the adaptive coast lattice with orbit-averaged sunlight and thin air; **landed: never stepped** (`advance_grounded`, `vessel/mod.rs:544`; `thermal.restart` resets without stepping) | same bits (warp_course; limbo, aerothermal tests) | overheat only after a live tick (`breakage::check_overheat`); never on a coast or landed | no |
| **Plumbing, boil-off** | `plumbing::fast` and `slow` each tick; `slow` covers the time since it last stepped (`systems/ship.rs:68`) | coasting: the coast law (D119), closed form in spans, applied at lattice points (`coast_lattice`); landed: `plumbing::slow` on the lattice (`lattice`) | same bits (warp_course; `a_day_on_rails_…`) | inside the law's spans in closed form; the ship state shows it at lattice points | no |
| **Relief, venting** | `relieve` each tick | coast law: the relief epoch in closed form (warming to the MEOP, then venting at the heat leak) | same bits | closed form (coast); tick (live) | no |
| **Power, batteries, shedding** | `power::step_ship` each tick with the tick's `dt` (`ship.rs:79`) | `step_ship` on the 60 s lattice with the lattice's `dt`; shedding judged on what the battery can give over the step (`power.rs:192`) | same bits (warp_course: the S2 sheds its loads by priority from +45,660.000 s, its computer last at +45,720.000 s, in every run) | a lattice epoch (coast), a tick (live) | no |
| **Life support** | `crew::step` each tick | `crew::step` on the lattice | same bits (warp_course: Crew Dragon's warp stop as its LiOH runs low at +8.959 days, the CO₂'s at +9.991, in one pass, at 10⁵× and at 1× across the stop) | a lattice epoch (coast), a tick (live) | no |
| **Failures, fates** | `failure::step` each tick: hazards, time at temperature, vibration, bursts (`ship.rs:78`) | **none**: `lattice` and `coast_lattice` run no failure step, so nothing accumulates or bursts off live ticks (failure-modes.md: "hazards on the rails lattice") | same bits (warp_course: an uncontained MVac restart fails at +3,000.123 s and breaks the stack up in every run; `a_failure_fires_the_same_…`) | a tick | no |
| **Orders (ship commands)** | the first tick at or after the epoch | exactly at the epoch: the advance stops there (`order_stop`) | same bits (warp_course; `orders_at_epochs_…`) | their own epoch | no |
| **Comms, light delay** | ship orders are queued at `clock + delay` (`game/src/commands.rs`) and are exact; **controls through the helm and the game's other delayed commands act at the first frame start after they arrive** (`helm.rs:87`, `commands.rs:162`) | the same | **differ** (D2) | frame starts | the delay is sampled per frame |
| **Attitude, holds** | the controller each tick | coasting: the pointing law where known, control ticks where not; regime changes at control ticks, a turn's start, a burn's end, the integrator step where the automatic reference changes | same bits (`limbo_regime.rs`, `hold_modes.rs`, `attitude_after_frames_and_jump`), **except the TARGET hold on a vessel** (D1) | their own epochs | rotation input zeroed at rails warp (D6) |
| **The bubble (D082)** | members on the reference's tick lattice | — | same bits once members are on its lattice (`a_bubble_in_frames_equals_one_call`); **a coasting neighbour the fleet moved past the epoch where a craft went live joins late** (D3) | the lattice epoch, or later | no, but decided at frame starts |
| **Atmosphere entry** | — | the live boundary found by bisection on the segment's dense output (`Segment::entry`); the advance stops there | same bits (`frame_live_stop.rs`, `a_live_descent_…`) | its own epoch | warp capped after |
| **Surface contact, landing** | contact substeps; rest after `REST_TICKS` ticks | a coast ends `live_height` above a surface (an integrator event) | same bits (`landed_rest.rs`, `contact.rs`) | its own epoch, then ticks | warp capped after |
| **Planned burns coming due** | flown by ticks (`flown`) | ignition and the turn's start are regime stops (`regime_stops`); low thrust is a perturbed segment | same bits on the made-up path (`planned_burns.rs`, `limbo.rs`); **no craft of parts can plan one** (`PlanError::PartsUnsupported`; PLANNER) | their own epochs | live burns cap the warp |
| **Separations, breakups** | at a tick; splits keep fates and the parent's lattices | an order's epoch | same bits (warp_course, `a_coast_split_…`, `parts.rs`) | their own epoch; the game applies the split in a later frame, the vessel waiting at the epoch | no |
| **Debris** | — | the game deletes a dead, uncontrollable piece 10 km from any craft at the frame's clock | not tested; frame epochs (D4) | frame clock | no |
| **Proper time** | per tick | the segment's integral; landed on `LANDED_STEP` | same bits (`proper_time.rs`) | — | no |

## 3. What the test proves

`crates/sim/tests/warp_course.rs` emulates the game's frame loop (§1: 60 fps, the span ramp, the 4× cap while anything is live, the bubble, `advance_frame` chunks, the clock held at the slowest, splits between frames, the look-ahead) and compares the whole fleet against one pass, bit for bit, with an FNV hash of each vessel's saved form. On a difference it names the first field that differs (the path through the pretty RON).

- **`the_upper_stage_and_dragon_fly_the_same_course_at_every_warp`:** the Falcon 9's second stage with Crew Dragon in a 400 km orbit, fates seeded, over two days. The MVac restarts at +3,000.123 s, shuts down 20 s later, and Dragon separates at +20,000.5 s. Then both pieces coast through about 30 eclipses on their lattices: the S2's battery runs out and it sheds its loads by priority from +45,660 s, its computer last at +45,720 s (the warp stop: no flight computer working), while Dragon's cabin holds near 0.2 kPa of CO₂ (step 6's η·Q·c). One pass, 100× (103,973 frames), 10⁴× (1,350) and 10⁵× (419) end on the same hashes (Dragon `438b3034828be2c2`, the stage `9384ec562dd63aa9`). 1× (186,000 frames, the first 3,100 s through the burn) matches its own one pass.
- **`an_uncontained_restart_fails_the_same_at_every_warp`:** a seed whose fate is below one start's hazard (D105 §6.3). The MVac fails uncontained at its ignition tick (+3,000.123 s), is destroyed and breaks the stack into three vessels; a day on, the same three vessels at 1× (the window), 10⁴× and 10⁵×.
- **The bare minimum** (D128, software-layers.md §3: layer 1 alone, no SAS, no program, torque-free): `an_uncrewed_dragon_coasts_three_months_on_layer_1_with_nothing_stopping_warp` (90 days, 10⁵× in 4,666 frames, and 600 s at 1×: no stop, its batteries full) and `crew_dragon_on_layer_1_stops_warp_as_its_consumables_run_low` (10.5 days: the stops at +8.959 days, a day of LiOH left, and +9.991, the CO₂ over its limit; at 1× over ten minutes across the first, at its epoch).

Together with the existing invariance tests (listed in the table's "Chunks" column), this covers coasts, the lattices, orders, a live burn with its settling and tail-off, a failure and its breakup, and separations. It does not cover:
- a craft of parts' planned burn (none exists);
- an atmosphere entry or a landing of a craft of parts in rails frames (`frame_live_stop.rs` covers the made-up path);
- the bubble with contact at rails warp;
- the game itself: the emulation is a copy of its rules and drifts if they change.

Two findings about the model, not about warp:
- The S2 runs out of power after 12.7 h.
- Dragon's cabin CO₂ crosses 1 kPa after about a day: its life support removes 4.0 kg a day (`data/parts/dragon/dragon-eclss.ron`), and four crew make 4.16 kg (BVAD).

Both are the kind of event D128's warp stop has to catch (§7, point 3).

## 4. Divergences found

Each has a reproduction in `warp_course.rs`, `#[ignore]`d. Each asserts the D129 property and fails today; run them with `cargo test -p sim --test warp_course -- --ignored`. The fix un-ignores its test.

- **D1, the TARGET hold on a vessel** (`d1_…`): the hold aims at a `TargetState` that the game snapshots at each frame's clock and carries at constant velocity (`vessel/hold.rs:93`, `game/src/navball/mod.rs:156` `feed_hold`). At 1× the snapshot is at most 16.7 ms old. At 10⁵× it is a whole frame old (here 300 s: carried at constant velocity, the target is about 390 km off its curved path in LEO). Two small rockets 4.7 km apart: after 300 s the holder's attitude is **102.2° apart** at 1× and at 10⁵×. A body target is exact (its own anchor).
- **D2, settings and commands that arrive with light delay** (`d2_…`): an uncrewed vessel's helm queues each new setting for `sent + delay`, but `Helm::acting` runs once per frame (`fleet_controls`), so the vessel acts on it from the first frame start after it arrives (`helm.rs:87`). The game's other delayed commands (`commands::InFlight`: plans, holds, the sequence's edits, debug) are applied the same way (`commands.rs:162`). Prograde instead of the attitude hold, sent at t₀ with 1.3 s delay, acts at 1.32 s at 1× and after the 300 s frame at 10⁵×: **109.5° apart**. Ship orders are exact (`order_at(clock + delay)`).
- **D3, a bubble's neighbour ahead of the lattice** (`d3_…`): craft A lights its engines by an order at +100.37 s, with craft B 2 km behind. At 1× both are in a bubble from +100.390 s, and B flies live ticks with A. At 10⁵× the fleet had already moved B to the end of the frame where A went live. `bubble::epoch` judges a craft past the lattice epoch only later (`bubble.rs:324`: "Craft past the epoch … are judged later"), so B never joins (A is gone by then). B ends **0.152 m apart** after 300 s, its model chosen by when the frames fell.
- **D4, debris deletion at the frame's clock** (game, no reproduction in sim): `debris::debris(world, clock, …)` at each frame (`game/src/debris.rs:153`). A dead piece is deleted at the first frame after it is 10 km from every craft. Nothing else changes unless the piece would have come back within 10 km between that epoch and the frame (a collision then happens at one warp and not at another).
- **D5, the agent's `run_until` conditions** (game, layer 3): an epoch stop is exact (`clamp_to_stop`), but a condition (an altitude, an event) is checked at frame ends, so the stop overshoots by up to a frame, and the agent's next input lands later at high warp. It is input, but it is the tool the mission tests stand on.
- **D6, rotation input zeroed at rails warp** (`state.rs:362`): the one place the warp decides something. It is input (and rotation input on a coast only sets the pointing to `Mean`); with D1's fix and D129's input rule it can go: a held rotation key at rails warp should act as at 1×, or the warp should be refused while it is held.

## 5. Model gaps that are the same at every warp

These do not break D129 (the lattice is fixed), but each is a place where the coarse model and live flight differ, or where one advance law (D121) does not hold yet. ADVANCE's list:

- **Power and life support lose time on going live.** A live tick steps them by the tick only (`systems/ship.rs:79, 81`), while the plumbing covers the time since the last lattice point (`:68`). Each coast-to-live change loses up to 60 s of their consumption.
- **Power's step depends on its length:** shedding judges what a battery can give over the step (`power.rs:192`), so 20 ms ticks and 60 s steps shed at different charges. One law over any interval would solve the battery's empty epoch in closed form instead.
- **No failure off live ticks:** no hazard accumulates on the lattice (a powered computer on a coast never ages), no burst (a tank with a stuck relief valve never bursts on a coast), no time at temperature, no overheat.
- **A landed vessel's temperatures never step** (its tanks do, on the lattice).
- **Events on a lattice land on its epochs:** a battery's shedding, the CO₂ alarm, a tank's state at lattice points. D128's warp stop needs them at their own epochs (§6, point 3).

## 6. Checking the lead's first answer

The lead's six points ([interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md)) against the code:

1. *Cost, not determinism:* confirmed. The sim never reads the warp, every model is chosen by state, and two days at 100×–10⁵× (and the first 3,100 s at 1×) give the same bits as one pass. A live craft caps everyone's warp (`state.rs:298`).
2. *The last bits across frames:* holds today for everything tested (§3), on lattices fixed by state. Its weak points are §5's.
3. *Events inside a frame:*
   - at their own epoch: orders, the live boundary, surface ends, ignitions, a turn's start, separations;
   - on the lattice: shedding, the CO₂ alarm, the tank state;
   - at frame starts: light-delayed settings (D2) and debris deletion (D4).
4. *Stopping warp:* built at lattice epochs (LAYERS-3, `ship_os::warp_stop`, software-layers.md §4): a frame's advance stops at the risk's lattice epoch as at its budget, and the game drops the warp there; same stops at every warp (`warp_course.rs`). Was: not built. Nothing stops the warp on an event today; `rails_block` only reacts to a vessel that has gone live. It needs each event's epoch from its law, a stop there, and the clock held there (§7, point 3).
5. *Ship programs under warp:* PLANNER's maneuver program (`systems::maneuver::step`) waits for a wake epoch. That epoch must stop the advance as an order does, or the program's first step lands at a frame start (D2's mechanism).
6. *The player's input:* accepted. Add D6.

The audit adds one point the lead's list lacks: **rules between two craft** (D1, D3) are decided at frame starts, because the fleet advances each vessel independently to the frame's target (D127).

## 7. Where it is difficult (for the owner), ranked

Each with its evidence, what restricting the warp would cost, and what changing the model would cost.

1. **Anything live holds every craft at physics warp.** One craft burning, entering or landing caps the whole fleet at 4× (`game/src/state.rs:298`, `RailsBlock::OtherLive`). With dozens of craft on trade routes, somebody is often live.
   - *Restricting* (today): correct, but the fleet's throughput falls with its size. It is the only option for entries and landings: 20 ms ticks at 10⁵× are 5 million ticks per real second per craft.
   - *Changing the model:* D029's warp while thrusting for burns only. A burn becomes a coast segment with the engines' steady state as continuous laws, its transients (start, settling, tail-off) in closed form or refused, and the layer-2 burn program run on a fixed schedule inside the segment (each guidance update a segment restart at a fixed epoch). Large: the maneuver program, the coast law and the integrator all change, and every guidance step costs a restart.
2. **Rules between two craft are decided at frame starts** (tests `d1_…`: 102°, `d3_…`: 0.15 m and a bubble that never forms; `bubble.rs:324`, `hold.rs:93`). The fleet moves each vessel independently to the frame's target, so a neighbour can run ahead of the epoch where another goes live, and a hold on another craft sees it only as of the last frame.
   - *Restricting:* does not help. 1× and 4× differ too, only by less.
   - *Changing the model:* moderate.
     - Each vessel knows in advance the epochs where it can go live: its next order, its live boundary or surface end, its next ignition, all already in its stored trajectory and runtime. The fleet caps each frame's target, for the craft within reach, at the earliest of those epochs, so the bubble is formed at the right epoch.
     - The TARGET hold names the target vessel and reads its stored trajectory at each control tick, from a snapshot taken at the frame's start (the look-ahead keeps it computed ahead). This needs care where the target's own coast restarts inside the frame.
3. **Events on coarse steps, and a warp stop at the event's own epoch** (warp_course: shedding at +45,660.000 s and the CO₂ alarm at +85,680.000 s land on lattice epochs; no failure step in `systems/ship.rs` `lattice` or `coast_lattice`; D128's stop not built). D129 holds, since the lattice is fixed, but "warp stops at the critical event's epoch, with the craft state at that epoch" needs more.
   - *Restricting:* to stop within a minute of any event, high warp would have to go. No.
   - *Changing the model:* moderate per law, a pattern for all. Each coarse law gives its next event's epoch in closed form, or by bisection inside its step: a battery empty at t = E/P_net, a CO₂ threshold, a hazard reaching its fate at t = (E − H)/λ for a constant λ, a relief or burst pressure from the coast law. The vessel's advance stops there, as at the live boundary. The game drops the warp and holds the clock. The other craft, already ahead in that frame, are independent and are only drawn ahead.
4. **Light-delayed settings act at frame starts** (test `d2_…`: 109°; `helm.rs:87`; `game/src/commands.rs:162`).
   - *Restricting:* no.
   - *Changing the model:* small. Every arrival becomes an order at its epoch: the helm's queue and the game's `InFlight` commands become the runtime's orders, and the advance stops at the next arrival as `order_stop` does. The same mechanism serves a layer-2 program's wake epochs.
5. **Keeping every law exact under any chunking as D121's ADVANCE rewrites them.** Today the bits hold (§3) because every law steps on a lattice fixed by state. A law stepped by "the time since it last ran", or whose result depends on its step's length, breaks it silently: power and crew stepping by the tick and losing the span since the lattice (`systems/ship.rs:79, 81` against `:68`), power's shedding over the step (`power.rs:192`), the landed thermal network that never steps (`vessel/mod.rs:544`).
   - *Restricting:* no.
   - *Changing the model:* this is ADVANCE's work. Each subsystem gets one law over any interval, tested in `warp_course.rs`'s harness at 1× against 10⁵× and against one pass.

Less difficult: debris deletion at the frame's clock (D4: evaluate the 10 km rule at the epoch the piece crosses it, from the stored trajectories); `run_until`'s conditions at frame ends (D5: stop at the condition's epoch where its law gives one, like point 3); rotation input at rails warp (D6).
