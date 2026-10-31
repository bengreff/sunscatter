# Brief: LAYERS, the three layers of ship software (draft, 2026-10-30)

**Drafted by AUDIT; reviewed by the lead 2026-10-31.** You run after STATE ([brief-state.md](brief-state.md)), alone in the sim; INTERFACE may still be at work in the game's interface.

D128 (owner, [interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md)):
- **Layer 1:** "Each part should have built in, fundamental logic … synonomous with how the part actually works". With it alone a ship "should be enough to coast for months if everything is set up".
- **Layer 2:** the ship computer "makes pretty much the same inputs the player makes, but with simple hard coded logic, plus some elements that are more automated".
- **Layer 3:** the agent outside the game.
- "I don't want "ship computers" mingling until assembling a ship means fighting an integration of procedures."
- A failure that puts the crew or the vessel at risk stops warp at its own epoch, unless layer 2 sets its own line or the craft's notifications are off.

AUDIT classified every piece of deciding logic and designed the layout in [software-layers.md](../technical/software-layers.md). LAYERS builds it. FIXES3 follows on the split (the [hand-off](HANDOFF.md)).

Follow [agent-rules.md](agent-rules.md). Your name is LAYERS and your target dir is `target/agents/layers`: start warm from the newest sim target the lead names. PLANNER's maneuver program (`systems::maneuver`, D126) is the first layer-2 program; move it, don't rewrite it.

## Read first

- D043, D106, D115, D117, D120, D123, D125, D126, D128, D129 in `docs/decisions.md`.
- [software-layers.md](../technical/software-layers.md) (all of it), [warp-invariance.md](../technical/warp-invariance.md) §7, [flight-software.md](../technical/flight-software.md), [knowledge.md](../technical/knowledge.md), [ship-commands.md](../technical/ship-commands.md), [power.md](../technical/power.md), [life-support.md](../technical/life-support.md), and the [avionics](../systems/avionics.md) system doc.
- Code:
  - `sim::systems::{flight, propulsion, command, power, crew, plumbing, rcs, fins, maneuver}`, `sim::craft::program`, `sim::chute`;
  - `sim::vessel::{ship, helm, attitude, hold}`.

## The work, in order (software-layers.md §7)

1. **`systems::flight` becomes `systems::ship_os`** (`mod`, `sas`, `commit`). The engine's fail-safe shutdown (`flight::safing`) moves into `propulsion`'s engine controller, reading its own links. D123's lost-controller case comes with it (FIXES3's item; take it if it is one line).
2. **The ullage push** out of `propulsion` (`ullage` driving `rcs`) into `ship_os::ullage`. The engine keeps its own start on command. Decide the settling wait by D128's rule (software-layers.md §1, open point 1) and record it.
3. **The sequence, functions and interlocks** out of `systems::command` into `ship_os::sequence`; `command::apply` stays the native boundary. **The chutes:** arming and the main's opening height into `ship_os::chutes`, from the barometric altitude in `Knowledge`. The reefing timers stay in `chute`.
4. **The program API** (software-layers.md §6): `View` in, `Commands` out, saved program state.
   - The schedule: each program runs at fixed epochs, its period or its wake epochs, and the advance stops at each due epoch as at an order. This shares ADVANCE's arrival order: build the stop here if ADVANCE has not.
   - D126's switch per program: by itself, on a press, or off.
   - `maneuver` moves in as the first program.
5. **The warp stop.**
   - `ship_os::warp_stop::bare_minimum`: the built-in rule of software-layers.md §4, a pure function with a table test.
   - A program's replacement for it, with the built-in rule back when that program's computer is lost.
   - `notify` per craft with the vessel's controls (saved; MCP sets it).
   - The game drops the warp and holds the clock at the stop's epoch: a lattice epoch until ADVANCE makes the laws give exact ones.
6. **The bare minimum, proven.** The priorities of power shedding become parameters (D125); the life support gets stocks (O₂, N₂, CO₂ removal) and rates for its crew. Then two tests, at 1× (a window) and 10⁵× (`warp_course.rs`'s harness):
   - an uncrewed craft set up for it (power positive, tanks sealed) coasts months on layer 1 alone, with nothing stopping warp;
   - Crew Dragon coasts on layer 1 alone until its consumables run low, and the warp stops at that epoch: crew at risk, the built-in line.

   Use RESEARCH's rows in `falcon9.md` §5.5: CO₂ removal by LiOH at a rate η·Q·c in place of the fixed 4.0 kg/day cap, so four crew settle near 0.2 kPa; about 60 kg of LiOH for four crew over 10 days; free-flight endurance about 10 days (E; Polaris Dawn flew about 5). Today the cap fails within a day. Mark anything unsourced E.

Small commits, one purpose each; `cargo test -p sim` and the game's touched tests at every commit; `warp_course` stays green (the moves change no bits unless a step says so). Docs in the same round:
- software-layers.md's status words, item by item;
- flight-software.md and `docs/systems/avionics.md` ("D128's three layers" under Not built);
- the owners table (`docs/owners.md`): `ship_os` and each program, the fail-safe in `propulsion`.

## State

### LAYERS-1 (2026-10-31)

**Landed** (each passed fmt, clippy, the whole sim suite with `warp_course`, and the game's check and touched tests; no bits changed except D123's case):
- f57d46b **step 1**: `systems::flight` is `systems::ship_os` (`mod`: `View`, `sense`, `Health`, `engine_readings`, `feeds_read`, `can_predict`; `sas`; `commit`: the rule and the pad's armed release, `commit::step` after the engines step, `disarm`, `await_lit`). The data links both layers read (`works`, `read_by_computer`, `commanded`, `controller`) are `systems::links`, so neither layer calls the other. The fail-safe is the engine controller's own: `propulsion::safes` (table test `the_fail_safe_table`), each controller reading its own links, **D123's lost controller included** (it safes its engine, in debug mode too; `a_lost_controller_on_the_pad_aborts_the_launch` updated). The game's navball names `ship_os::Sas`.
- 7ea2ab2 **step 2**: the settling wait and push are `ship_os::ullage` (`hold`, `push`, `step`, `SETTLE_LIMIT`); the engine keeps only its start on command (`propulsion::light`; floating propellant: gas ingestion). `propulsion::protect` (the closed throttle, the fail-safe) runs before the ullage step, so the order is today's. Open point 1 decided (below).
- cf87fb7 **step 3a**: `next_stage`, `again`, `function`, `edit_sequence`, `release_undone` and the legs' `interlock` are `ship_os::sequence` (tests in `ship_os/sequence_tests.rs`); `command::apply` stays the native boundary, `carry_out` dispatches orders.
- Docs with each: software-layers.md's status item by item (§2's rows, §7's items 1–3 *built*), flight-software.md, avionics.md, engines.md, systems/propulsion.md, planner.md, staging-bar.md, owners.md, D043's module path.

**Left, in order:**
1. **Step 3b, the chutes** (`ship_os::chutes`). Today `chute::ChuteState::step` arms on `Controls::chute` and opens the main at the true height (`vessel/live.rs:435`, `Flow::height`; `predicted_cd_area` the same). Move the main's decision out: `chute::step` takes "open the main now" from the ship computer, which decides it from `Knowledge`'s `Quantity::Altitude` (GNSS's `position` gives it at 1.5× its accuracy, an `altitude` sensor directly; Dragon's barometric altimeter is not in its data: add it, sourced or E, with the cabin sensors below) and the drogue's reefing state. Then Dragon's automatic arming on barometric altitude, with D126's switch (step 4's). Without an altitude reading the main does not open by height: the pilot's "deploy" command opens it (D126: the manual deploy runs the same function). The landing prediction (`predicted_cd_area`) reads the same rule. Expect bits to change only where the altitude is unknown.
2. **Step 4, the program API** (software-layers.md §6): `View` in, `Commands` out, saved program state; the schedule as orders (each due epoch stops the advance as `order_stop` does; ADVANCE's arrivals have not been built: build the stop here); D126's switch per program; `systems::maneuver` moved in as `ship_os::maneuver` (its `Commands` is the model; `vessel::maneuver` is its vessel side).
3. **Step 5, the warp stop**, then **step 6, the bare minimum proven**, as the brief says. For step 6: INTERFACE-2's finding 19 (Dragon's cabin sensors) first.
4. Small: `systems::flight`'s old tests file and module are gone; `Vessel::flight_health` kept its name (it is the computers' health). STATE's note: `Systems` (the lattice clock) and `ship_tick::ship_lattice` could be one call.

**Open for the owner (LAYERS-1):**
- **The settling wait is layer 2** (software-layers.md §8, open point 1, by D128's rule): the wait reads the feed tanks and fires other parts' thrusters, so it coordinates parts, and the real sequence's ullage burn is the stage computer's. With layer 1 alone an ignition on floating propellant ingests gas; a pilot settles by hand (translation, then ignite), as D126 allows. The wait's state stays each engine's `settling` (saved, and the game reads it), and it still reads the true liquid position (FIXES3: the level sensors, a timed burn where they do not read the tanks).
- **D123 in debug mode:** a lost controller safes its engine in debug mode too (D123: "a dead or unpowered computer is still dead"); before, debug's launch went ahead with a lost controller.

### From INTERFACE-10 (2026-10-31)

- **Older saves crash the game** since f2806ca added Dragon's cabin air monitor and barometric altimeter as components: loading a `falcon9-crew` save from before it (the playtest save `docs/plans/playtest-2026-10-01-orbit.ron`, `./run play`, included) panics at the first power step, `crates/sim/src/systems/power.rs:110` index out of bounds (the save's 39 power entries, 41 components). A save whose ship state does not fit its craft should be refused at load (or SAVE_VERSION bumped, D063), and the playtest save flown again (testing.md).

### LAYERS-2 (2026-10-31), wrapped up at its context cap

**Landed** (each passed fmt, clippy, the whole sim suite with `warp_course`, and the game's check):
- f2806ca: Crew Dragon's **cabin air monitor and barometric altimeter** (finding 19; E parts `cabin-air-monitor`, `barometric-altimeter`): the `cabin` class (total, O₂ to ±100 Pa, CO₂ ±10 Pa) and the `barometric` class (the altitude while the air outside is above 1 kPa, `data::BAROMETRIC_FLOOR`, from `ShipState::ambient`: the last live tick's static pressure, 0 on a coast).
- c369b60 **step 3b, the chutes**: `ship_os::chutes` (`arm_now`, `main_now`, `press`, `predicted_cd_area`); `chute::step` opens the main on the computer's word (`drogue_done` stays layer 1's). Without an altitude reading the main waits for a second press. `ShipCommand::ArmChutes` is a press in `after_command`, so an order arms too (before, only the game's `controls.chute` did). D126's `Switch` (Auto arms below `ChuteSpec::drogue_height` while the navigation solution descends; Dragon 5.5 km, P) is saved in `ship_os::Software`; the default is Press (no bits changed where the altitude is known).
- 215f0ea **the save fix** (the coordinator's urgent item): f2806ca added two components, so older falcon9-crew saves panicked at the first power step: `ShipState::fits` was never called at load. `SaveGame::from_ron` now refuses such a save as a format error (`tests/save_load.rs`), `fits` checks the fins too, **SAVE_VERSION 17**. Rule from the coordinator: any change to a craft's data that changes what its state holds bumps SAVE_VERSION in the same commit.
- 5187deb **step 4, the program API** (`ship_os::program`: `Context` with `read` through `data::truth`, `Commands` moved from `maneuver`, `Schedule`, `Slot`, `Installed`, `Software::{maneuver, programs}`), the vessel side `vessel::programs` (`programs_stop` in `order_stop`, `run_programs` inside `apply_orders`, `install`, `set_switch`, `press`; a switch or press restarts a period from now, `Installed::resume`), the first scheduled program `ship_os::power_guard`. `tests/ship_programs.rs`: the same switch epochs and battery bits at 1× and 10⁵×, the switch, the maneuver's switch.

**Left, in order:**
0. **REVIEW's #1 for the load refusal** (docs/technical/review-2026-10-31.md): the load checks only `ShipState`'s list lengths (`save.rs`'s `fits` call). Make the fit check cover the saved thermal state against the rebuilt design's cell and node counts (a cell-count change panics at the first thermal step, `thermal/mod.rs:301`), and match items by their owner handles, not only by count (items reordered within a kind load misaligned without an error); a test for each. (REVIEW's #2 for the chutes, `press("chutes")` refused while `set_switch` accepted it, is fixed: one rule, `Vessel::press` runs the deploy press.)
1. **Re-fly the playtest save** (the coordinator's item 3): SAVE_VERSION 17 refuses `docs/plans/playtest-2026-10-01-orbit.ron`. Fly it again by testing.md's procedure (STATE-4's flight: Dragon alone in orbit), write it over that file, and check `./run play` loads it and flies on past a power step. HANDOFF's "SAVE_VERSION 16" line then says 17.
2. **The maneuver move:** `git mv` `systems::maneuver{,_tests}` into `ship_os/` and update the paths (sim, tests, `game::{commands, planner, fleet_tests, interface::menus::{data, maneuver}, trajectory::{apsides, predict_tests}}`, docs). It waited because INTERFACE-10 had `game/commands.rs` open.
3. **Step 5, the warp stop**, then **step 6** as the brief says. The cabin's sensors exist now (finding 19 done). For step 5 the program API is ready: a `warp_stop` program is a `Builtin` variant reading the step's events; `notify` goes with the helm's controls.
4. Not built in the chutes (entry-ablation.md §3.2): the drogue's wait above its design q, the mains' cut on water, the chute part's own baroswitch. The game and the agent cannot set a switch or press yet (the sim's `Vessel::set_switch`, `press` are ready); `game::interface::part_card::content` still says "a cabin air sensor (none modelled yet)".

**Open for the owner (LAYERS-2):**
- **The chutes' switch defaults to Press** (today's behaviour: the player arms). Real Crew Dragon arms itself; setting Dragon to Auto changes every entry flown without arming. Proposed: Auto for Dragon when the entry flight comes.
- **A second press calls the main** (D126: the manual deploy runs the same function), at any height once the drogue lets it.
- **A due program without a working computer misses its epoch** rather than waiting.

### From the lead (2026-10-31)

- INTERFACE-2's finding 19: Crew Dragon has no cabin sensors in its data (`data/parts/dragon*`). The bare minimum's crew-at-risk line reads the cabin through `Knowledge` (D106), so give the cabin its real sensors (CO₂, O₂, pressure, temperature; sourced or E) before the warp-stop test.
- STATE is finished (2026-10-31): `Runtime` is merged into `ShipState` by subsystem, owners are typed handles (`craft::handle`), the engine rule lives only in `systems::command::apply` (F8), and `Vessel::powered()` comes from `systems::power::powered` (the ship's batteries and arrays). Read `brief-state.md`'s States for where things moved; the code paths named above may have changed.
- Your target dir: start warm with `mv target/agents/state target/agents/layers`. INTERFACE-9 runs beside you in the game crate and `game::agent`.
