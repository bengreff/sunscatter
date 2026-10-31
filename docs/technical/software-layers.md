# Ship software layers (D128): the inventory and the design

**Status: being built** (designed by AUDIT, 2026-10-30, at 14c2a4d; LAYERS builds it, [brief-layers.md](../plans/brief-layers.md)): §7's moves say item by item what is *built*; the rest is *design*. This doc classifies every piece of logic in the code that decides something, says where it lives today and where it should, and defines the bare minimum, the warp-stop line, notifications per craft, and the module layout with its two APIs. The system: [avionics](../systems/avionics.md); the flight software as built: [flight-software.md](flight-software.md).

D128:
- **Layer 1** is each part's own logic, "synonymous with how the part actually works": an engine controller's start sequence and fail-safe shutdown, a valve's relief.
- **Layer 2** is the ship computer (D043's ship OS). It makes the inputs a player makes, with simple hard-coded logic plus some automation, and runs functions and whole missions.
- **Layer 3** is the agent outside the game, through MCP.
- "I don't want "ship computers" mingling until assembling a ship means fighting an integration of procedures."

## 1. The rule that sorts them

A piece of logic is **layer 1** when it belongs to one part and reads only that part's state, its parameters (D125) and what reaches it through its declared connections (its bus's voltage, its feed's pressure, a command addressed to it). It runs whatever the rest of the ship is. Assembling a ship never needs it configured.

It is **layer 2** when it coordinates parts, or reads the craft through its sensors to choose commands: what a player could do at the controls or in the systems screen. It reads `Knowledge`, never the truth (D106), and its outputs are the commands a player issues (D043).

Neither: the physics (what happens), the native boundary (sensors to `Knowledge`, commands to actuators: `systems::data`, `systems::links`, `ship_os::sense`, `ship_os::health`, `systems::command::apply`, `ShipState::apply`), and the interface's readers (estimates, the g-limit, the abort's offer).

## 2. Inventory

### Layer 1: the parts' own logic

| Logic | What it decides | Lives today | Should live | Notes |
|---|---|---|---|---|
| Engine start sequence | an ignition (a charge, or the ground's supply on the pad), the spin-up ramp over its start time, *starting* → *running* | `systems::propulsion` (`light`, the state machine) | stays: the engine controller in `propulsion` | |
| Shutdown and tail-off | *shutting down* with the published impulse, then *off* | `propulsion::stop` | stays | |
| Throttle within range | a running engine cannot go below its minimum | `propulsion` | stays | |
| Flame-out when dry | an engine whose feed runs dry stops (not a failure) | `propulsion` | stays (physics) | |
| **The fail-safe shutdown** (D123) | a lit engine whose linked computers are all lost or unpowered shuts down; D123 adds its own controller lost | was `systems::flight::safing`, called from `propulsion` | **built:** `propulsion::safes` (a table test), each controller reading its own links (`systems::links::controller`), D123's lost controller included | |
| Benign shutdown on a failure | the controller stops an engine whose mode is benign | `failure::outcome` → `propulsion::stop` | stays | |
| **The wait for settled propellant** | an engine commanded on floating propellant waits up to `SETTLE_LIMIT`, then starts; commanded again it starts at once | was `propulsion::settle_or_start` | **built: layer 2**, `ship_os::ullage::{hold, step}` (open point 1, decided below) | PLANNER's `systems::maneuver` called the settling wait layer 1 |
| **The push that settles it** | the thrusters along the thrust fire during the wait (the S2's nitrogen pods) | was `propulsion::ullage` → `systems::rcs` | **built: layer 2**, `ship_os::ullage::push` | an engine commanding another part's thrusters is the "mingling" D128 forbids. The real sequence's timed ullage burn is the flight computer's (flight-software.md: "an adaptive function"). Open point 1 |
| Relief valves | a tank above its MEOP vents; at saturation it boils off | `plumbing::relieve`; the coast law in closed form | stays; MEOP a parameter | |
| Regulators | the ullage held at its setpoint while the bottle can | `plumbing::fast` (`pressure_regulated`); the coast law's top-up | stays; setpoint a parameter | |
| Valves | open, closed, stuck | `ShipState::apply` (`Open`, `Close`); failure outcomes | stays (a mechanism commanded from outside) | |
| **Power shedding** | a short bus drops its least essential loads, then the largest | `power::step_ship`; `essential()` by component kind (`power.rs:118`) | stays in `power` as the bus's protection; each load's priority a parameter of the bus or the component (D125), not a list of kinds | layer 1 needs it to coast |
| Battery charging | the umbilical or the solar cells charge the batteries | `power::step_ship` | stays | |
| **Life support's regulation** | CO₂ and water removed up to their rates; O₂ fed to 21.3 kPa; N₂ to 101.3 kPa total; 295 K | `crew::air` | stays as the unit's loops; setpoints parameters of the life-support entry | |
| Actuators | a mechanism moves to its target at its rate | `mechanisms::step` | stays | |
| Parachute reefing | reefing-line cutters on timers from line stretch; filling; strength | `chute::step` after arming | stays | the main's opening height is layer 2 (below) |
| Hold-down clamps | let go when commanded; torn out at their capacity | `ship_tick::advance_held` | stays (the pad's own, and physics) | |

### Layer 2: the ship computer

| Logic | What it decides | Lives today | Should live | Notes |
|---|---|---|---|---|
| **SAS and the holds** | whether SAS can hold (an attitude reference, navigation, a computer), what it aims at, the torque it asks | `flight::sas` (gating), `vessel::hold` (aims), `vessel::attitude::command` (the controller), `Vessel::steer` | a program: `ship_os::sas` | on coasts and rails it still holds on the truth (flight-software.md) |
| Torque and force allocation | which thrusters fire, the gimbals' automatic vectoring (D126's "simplified aids"), the grid fins' share | `systems::rcs` (allocation), `fins::allocate`, `vessel::attitude` | part of `ship_os::sas` | coordinates parts. With layer 1 alone a pilot fires thrusters one by one (D128's "nearly impossible") |
| The pilot's RCS modes (D111) | what the translation and rotation keys fire | `rcs::pilot` | the pilot's input handling in layer 2 | |
| Throttle to zero shuts down (D114) | closing the throttle stops the lit engines | `propulsion` (the level's rule) | layer 2's input handling | an engine controller holds its minimum; the lever's meaning is the ship's |
| **The launch commit** | the clamps let go when every engine reports rated thrust; an engine failing first aborts the launch | was `flight::commit`, called from `propulsion` | **built:** `ship_os::commit` (the rule and the armed release, `commit::step`, run after the engines step) | propulsion keeps only the engines' readouts |
| **The stage sequence and functions** | Space runs the next stage; AGAIN; saved functions; the sequence's position | was `systems::command` (`next_stage`, `again`, `function`, `run`), `craft::program` | **built:** `ship_os::sequence` (`next_stage`, `again`, `function`, `edit_sequence`, `release_undone`); `command::carry_out` dispatches the orders to it | the edits are the systems screen's (`edit_sequence`); the piece's view stays `craft::program` |
| Interlocks | the legs refused while the clamps hold; a stage with nothing left refused | was `systems::command`, `game::staging` | **built:** `ship_os::sequence::interlock` (the legs; `command::apply` asks it) | the stage with nothing left is the interface's (`game::staging`) |
| **Aborts** (D120) | the procedure's commands; the automatic decision on a thrust-to-weight margin | the procedure: a function (`program::offered` for the offer); the decision: not built | `ship_os::abort` | the offer stays an interface rule |
| **Parachute deploy** (D126) | arming on command; Dragon's automatic arming on barometric altitude; the main's opening at `main_height` | was `Controls::chute` and `chute::step` on the true height | **built:** `ship_os::chutes` (`arm_now`, `main_now`, `press`, the prediction's `predicted_cd_area`; D126's `Switch` in `ship_os::Software`), from `Knowledge`'s altitude (GNSS, or the barometric altimeter in the air) | without an altitude the main waits for a second press; `ShipCommand::ArmChutes` is a press (it now arms through an order too) |
| The ullage push | see layer 1's settling rows | **built:** `ship_os::ullage` | | open point 1, decided |
| **The maneuver program** (D126) | a planned burn closed-loop to its node's orbit | `systems::maneuver` (PLANNER, c7c2de4: `step(&Inputs, &mut Program) -> Commands`) | `ship_os::maneuver` | the first program on the layer-2 API |
| Landing guidance, docking (D117, D123) | the booster's burns; Dragon's approach | not built | `ship_os` programs | |
| The warp-stop line, replaced | §4 | **built:** `ship_os::warp_stop::Line` in `Software::warp_stop` (D126's switch) | a program's `warp_stop` | the built-in line returns when it is off or its computer is lost |

### Layer 3, and the game's own rules

- **Layer 3** is the MCP tools (`game::agent`). Through the same command API as the player (D063), with the light delay (D069), the agent calls functions, sends commands and controls, takes shots and runs time. Its `run_until` conditions are checked at frame ends ([warp-invariance.md](warp-invariance.md) D5).
- **Not ship software:** the warp cap (`game::state::rails_block`), the bubble (`vessel::bubble`), debris deletion (`game::debris`), the helm's light delay (`vessel::helm`), the interface's readers (`systems::estimate`, `systems::g_limit`, `program::offered`).

## 3. The bare minimum: layer 1 alone

D128: with layer 1 alone a ship "should be enough to coast for months if everything is set up". It can in theory be flown by hand, but hardly.

**"Set up"** means:
- every load switched as wanted;
- the valves set and the engines off;
- nothing moving, the clamps released;
- the life support stocked.

**What layer 1 must then do, with nothing else running:**
- **Power:** the sources charge the batteries, and a short bus sheds by its loads' priorities and restores them when it can.
- **Life support:** the unit holds the cabin's air and temperature while it has power and stock.
- **Plumbing:** relief valves hold their MEOP, regulators their setpoints while their bottles last, and boil-off vents.
- **Propulsion:** an engine that is off stays off; a lit engine without its computers safes itself.
- **Mechanisms** hold where they are.
- **Attitude:** none. Without layer 2 the craft rotates freely (torque-free, `vessel::pointing::Free`), and its sunlight is what that rotation gives.
- **Comms:** the receiver takes commands as they arrive (the player or the agent, with light delay).

**Today it does not hold for a month.** In `warp_course.rs`'s two days:
- The S2 alone sheds every load at +12.7 h: no solar cells, and its batteries drained.
- Dragon's cabin CO₂ passes 1 kPa after about a day: its life support's 4.0 kg/day of removal (`dragon-eclss.ron`) is below four crew's 4.16 kg/day (BVAD). O₂ has a rate but no tank to run out of.
- Thermal control (heaters, radiators, the cabin's loop) is not modelled, so a cold soak is not either.

So the bare minimum needs, in data and model:
- each consumable a stock (O₂, N₂, LiOH or its regenerable equivalent);
- removal rates matching the crew;
- the loads' priorities as parameters.

## 4. The warp-stop line

*Built* (LAYERS-3): `ship_os::warp_stop` (`Facts`, `risks` with its table test `the_risks_table`, `bare_minimum`, `Line`, `decide`), the vessel's check at each lattice step of its systems (`vessel::warp_stop::watch`: in the coast's lattice loop, on the ground, and at a live tick that crosses a lattice epoch), a frame's advance held at the stop's epoch until the game takes it (`Vessel::take_warp_stop`; the coast stops as at its budget, so the bits are the same: `warp_course.rs` gives the same stops at 100×, 10⁴× and 10⁵× and the same fleet as one pass), and the game dropping the warp there with a toast (`game::fleet::take_warp_stops`, `SimState::step`, `interface::events::toast_warp_stops`). The risks are read from the parts' own state and the cabin's air through `Knowledge`; not yet read: the cabin's temperature (no sensor quantity), the acceleration limits (the crew's injuries are logged), a consumable's time to empty (step 6: stocks), the receiver (no receiver part in the data).

**Built in, on layer 1** (D128): a failure that puts the crew or the vessel at risk stops warp at its own epoch, with the craft's state at that epoch. Anything else is logged (`ShipState::event`, its `alarm` flag as today).

**At risk** (a pure rule with a table test, one owner: `ship_os::warp_stop::bare_minimum`):

| Risk | Trigger (from the event or the state at its epoch) |
|---|---|
| Crew: a limit will be breached | ppCO₂ above its limit, ppO₂ below its floor, cabin pressure below its floor, the cabin's temperature outside its range (NASA-STD-3001, E until D116's thresholds); acceleration limits (live only); or a consumable's time to empty below a margin (E) |
| Crew: their support lost | the life support lost (failed or unpowered) with crew aboard |
| Vessel lost | a part destroyed, a breakup or burst, an uncontained failure (`Effect::Destroy`, `PartFailure`) |
| Vessel uncontrollable | the last working computer lost, or the receiver unpowered (D121's controllability check) |
| Power | an essential load shed (a computer, the life support, the receiver) |

Not a stop: a benign engine shutdown on a coast, a stuck valve, a cell lost, a sensor lost, venting, a non-essential load shed.

**At its own epoch:** a live event is at its tick. A lattice event today is at its lattice epoch (the shedding at +45,660.000 s). For "the craft state at that epoch" each coarse law gives its next event's epoch, and the vessel's advance stops there ([warp-invariance.md](warp-invariance.md) §7, point 3). Until then the stop is at the lattice epoch: the same at every warp, coarser than live.

**How layer 2 replaces it:** a craft's programs may include one `warp_stop` program. It reads the events of the step and the `View`, and answers stop or continue. When it is present and running it replaces the built-in rule entirely; the events are still logged. When its computer is lost or the program is switched off (D126), the built-in rule returns, so the fail-safe stays layer 1's.

## 5. Notifications off per craft

*Built* (LAYERS-3): `Helm::notify`, `Vessel::{notify, set_notify}`, the game's `GameCommand::SetNotify`, MCP `set_notify`; a piece of a split keeps the craft's setting. A per-vessel setting kept with the vessel's own controls (D112, `vessel::helm`), saved: `notify` (on by default). Off: nothing on that craft stops warp, neither the built-in rule nor its program's. Events are still logged and shown in the tracking station, without toasts. "If something fails, it fails, the autopilot scripts do the best they can." The game's warp stop reads it; MCP sets it like any setting.

## 6. The module layout and the two APIs

```
sim::systems::
  propulsion  (layer 1: each engine's controller: start, shutdown, tail-off, range, the fail-safe)
  plumbing    (layer 1: relief, regulators, valves)
  power       (layer 1: each bus's protection by its loads' priorities; charging)
  crew        (layer 1: the life support's loops; the crew's limits are physics)
  mechanisms  (layer 1: actuators), chute (layer 1: reefing timers)
  ship_os::   (layer 2, today's systems::flight renamed)
    mod       the layer: sense (truth → View), health, the schedule, the budget, commands → actuators
    sas       the holds, the attitude controller, allocation to thrusters, gimbals and fins
    commit    the pad's launch commit
    sequence  the stage sequence, functions, interlocks
    abort     the procedure and, automated, its decision (D120)
    chutes    arming and the main's height from Knowledge (D126; built)
    ullage    the settling push before a start
    maneuver  PLANNER's program (its Commands are the API's; its switch built)
    program   the API: Context, Commands, Schedule, Slot, Installed (built)
    power_guard the load manager, the first scheduled program (built)
    warp_stop the built-in bare-minimum rule (§4) and a program's replacement (built)
```

**Layer 1's API: part behaviour set by parameters (D125).** Each part kind's logic is one function of its own definition (catalog parameters), its own state, its inputs through its declared connections and the step:

`fn step(def: &PartDef, state: &mut PartState, inputs: &Inputs, dt: f64) -> Vec<Effect>`

- Its inputs are the bus's voltage, the feed's pressure and the commands addressed to it.
- No part reads another part's state except through those connections, and nothing in layer 1 calls layer 2.
- It advances over any interval with one law (D121, ADVANCE): exact or coarse-grained, chunked = single pass.

**Layer 2's API: `Knowledge` in, commands out (D043).** A program is:

`fn step(view: &View, health: &Health, def: &ShipDef, events: &[Event], state: &mut ProgramState, budget: &mut Budget) -> Commands`

- `View` is `Knowledge` through `sense`, and `Commands` are the commands and controls a player issues (PLANNER's `maneuver::Commands` is the model).
- **The schedule** (D129): a program runs at fixed epochs: its period from its own epoch, or wake epochs it asks for. Each due epoch stops the vessel's advance as an order does, so it acts at the same epoch at every warp. Its state is saved with the vessel.
- **The switch** (D126): each program runs by itself, when the player presses for it, or not at all, with its configuration. Input that conflicts with what a running program commands switches it to manual; input that agrees leaves it running.
- Native Rust now, WASM later with the same signature (D043).

## 7. The moves for LAYERS, in order

1. *Built.* `systems::flight` → `systems::ship_os::{mod, sas, commit}`; `safing` → `propulsion`'s engine controller (D123's lost controller with it); the data links both layers read → `systems::links`.
2. *Built.* The ullage push out of `propulsion` into `ship_os::ullage`, with the engine's own start rule left in `propulsion` (open point 1: the wait goes with the push). The wait's state stays each engine's `settling` (saved; the game reads it), and it still reads the true liquid position (FIXES3: the level sensors, a timed burn without them).
3. *Built.* The sequence, functions and interlocks out of `systems::command` into `ship_os::sequence`; `command::apply` stays the native boundary.
4. *Built.* The chutes' arming and main height into `ship_os::chutes`, from the altitude in `Knowledge` (Dragon's barometric altimeter and cabin air monitor added, E).
5. *Built (LAYERS-2).* The program API, `ship_os::program`: `Context` in (the `View`, `Health`, the definition, and `Context::read` for the ship-state quantities `Knowledge` knows, `systems::data::truth`), `Commands` out (the maneuver program's, moved here, plus ship commands and a wake epoch), saved state (`Installed` in `ship_os::Software`, saved with the vessel). The schedule (`Schedule`: a period from its own epoch, or a wake epoch): `Vessel::programs_stop` joins `order_stop`, so each due epoch stops the continuous advance as an order does; due programs run where orders are applied (`vessel::programs`; live ticks at the first tick at or after). D126's switch per program (`Slot`: by itself, one run per press, off; a switch or press restarts a period from now, `Installed::resume`), and the maneuver program's own (`Software::maneuver`). The first scheduled program: the load manager `ship_os::power_guard`. Test: `tests/ship_programs.rs` (the same switch epochs and battery bits at 1× and 10⁵×). Left: `systems::maneuver` still lives outside `ship_os` (the move waits for `game::commands` to be free); the agent's and the systems screen's switches (game).
6. *Built (LAYERS-3).* `ship_os::warp_stop`: the built-in rule (§4) as a pure function with a table test, the program's replacement, and `notify` per craft; the game's warp stop at the stop's epoch (a lattice epoch; SAVE_VERSION 18). Left: `run_until` ending on a stop (MCP), the bubble's joint tick (`vessel::bubble`) does not check.
7. The power priorities as parameters; the life support's stocks; the bare-minimum test: Dragon coasting a month on layer 1 alone, set up, with nothing stopping warp. Today it fails on CO₂ within a day.

## 8. Open

1. **The settling wait** (decided by LAYERS by D128's rule, §1; for the owner to confirm). The wait goes with the push into layer 2 (`ship_os::ullage`): it reads the feed tanks and fires other parts' thrusters, so it coordinates parts, and the real sequence's ullage burn is the stage computer's. The engine keeps only its start on command (gas ingestion if its propellant floats), so with layer 1 alone a pilot settles by hand (translation, then ignite), as D126 allows.
2. **The warp-stop thresholds:** E until D116's crew thresholds and NASA-STD-3001's limits are sourced.
3. **Whether the attitude controller is one program or the layer itself.** SAS runs on every live tick today. As a program it would run on its schedule, the tick.
