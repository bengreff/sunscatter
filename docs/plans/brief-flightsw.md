# Brief: FLIGHTSW, the flight software reads what the sensors know (2026-10-30)

D117's third foundation: F3, the review's recommendation 4 ([fidelity-review-2026-10-29.md](fidelity-review-2026-10-29.md) §3 F3 and §4.4). Follow [agent-rules.md](agent-rules.md). Your name is FLIGHTSW and your target dir is `target/agents/flightsw`. Two other agents run at the same time. **COAST** ([brief-coast.md](brief-coast.md)) is on `vessel/coast.rs`, `vessel/mass.rs`, `systems/plumbing.rs` and the systems lattice. **FIXES2** ([brief-fixes2.md](brief-fixes2.md)) is on the game's staging bar and action bar, `craft/program.rs` and the craft's functions. Stay out of files with their uncommitted changes in them.

## What is wrong

- SAS (`vessel/attitude.rs`), the holds (`vessel/hold.rs::hold_inputs`), the pad's health check (`systems/propulsion.rs::release`), the estimates (`systems/estimate.rs`) and the landing prediction (`landing/`) all read the **true** state. Even with every IMU dead, SAS holds attitude perfectly.
- `Knowledge` (`systems/data.rs`: `knowledge(def, state, debug)`; `Vessel::knowledge`) already gives what working, powered, linked sensors measure, but only the display reads it.
- D115's adaptive functions (engine-out re-trim, launch-commit, propellant utilisation, the navigation filter) and D117's guidance (the booster flying itself home, Dragon's autonomous docking) all need a flight-software layer. Its inputs are `Knowledge` and the computers' health, and its outputs are `ShipCommand`s. Built on the truth, they would have to be rewritten.

## The work, in order

1. **Inventory** every control-side reader of the true state (sim and game), in this brief's State.
2. **The flight-software layer.** It is one owner (rule 5): each vessel's computers (`*.computer` in `data/craft/falcon9-crew/craft.ron`, linked to their sensors and effectors by the data links) run functions whose only inputs are `Knowledge` and the computers' health, and whose outputs are commands. Design it so D117's guidance and D115's adaptive functions plug in as functions of this layer, and so that later a ship-OS program (pillar 4) can be one. Its inputs and outputs are pure, with table tests. Don't build guidance or adaptive functions now.
3. **Move the readers onto it:** SAS and the holds, the pad's release check (it waits on the engine controllers' thrust readings), the estimates the player is shown, and the landing prediction. Debug mode keeps its truth view and changes nothing else.
4. **Acceptance (table tests):**
   - With every IMU lost, SAS will not hold, and says "no attitude reference".
   - With the stage computer unpowered, its running engines are safed by their own controllers. Find the real behaviour, or take the class default and mark it E.
   - The pad's release waits on the engine controllers' `EngineThrust` readings, so a lost controller means a pad abort.
   - Debug mode is unchanged.
   - Determinism and the warp tests are unchanged.
5. **Docs:** the `architecture.md` rows (a new owner for the flight software), `design/ship-model.md` or `features/ship-systems.md` where control is described, the review's §3 F3 marked with the commits, and this brief's State.

Respect D115: only the real vehicle's sensors, and readings to their sensor's significant figures. Noise, drift and outages only where they matter, and not in this brief.

## State

*FLIGHTSW, 2026-10-30.*

### Landed

- **994fad7** The flight-software layer, `sim::systems::flight`, and the readers moved onto it: SAS and the holds in live ticks, the pad's launch commit, the controllers' safing, the estimates' engine thrust, the landing prediction's gate; the acceptance tests (`systems/flight_tests.rs`, `vessel/software_tests.rs`). The test rocket (`craft/build_tests.rs`) got a battery: its computer was unpowered and so, with safing, commanded nothing.
- **ccf251d** The navball's SAS button says what the flight software says (`navball::rules::sas_button`: SAS NO REF, NO NAV, NO CPU).

### Inventory: control-side readers of the true state

| Reader | Read | Now |
|---|---|---|
| SAS in live ticks (`vessel::attitude::command` from `live::begin_tick`) | the true attitude and rates | **moved**: `Vessel::steer` gives it the controls `flight::Sas::acting` lets through and the attitude `flight::sense` gives |
| The holds in live ticks (`hold::hold_inputs` via `Vessel::aim`) | the true position and velocity, the target's, the plan's burn direction | **moved**: from `View::nav`; without a navigation solution the attitude is held (`Sas::NoNavigation`) |
| SAS and the holds in coasts and on rails (`vessel::coast::attitude`, `aim_at`, `attitude::advance_coast`) | the true attitude, the stored trajectory's state, the craft's ideal torque | **left**: `coast.rs` is COAST's; after D119 lands, gate the coast's controls through `Sas::acting` at its start and wherever the systems lattice changes what is known, so 60 fps and one jump still agree |
| Planned burns flown by ticks (`vessel::flown`) | the true attitude | test craft only (planned burns are refused for craft of parts); through `steer` when they are not |
| SAS's slew cap ∝ 1/q (`attitude::limits`) | the true air at the true position | left: equal to what the software derives from its navigation and air model while it has them; without navigation only small attitude errors are held |
| The pad's release (`propulsion::release`) | each engine's true status and level | **moved**: `flight::commit` on the controllers' readouts (`engine_readings`, `Quantity::EngineThrust` known); a missing readout aborts |
| A computer lost or unpowered with its engines running | nothing (they ran on) | **new**: `flight::safing` (E) |
| Which actuators a command reaches (`command::reachable`) | the computers' `Runtime::out` | **moved**: `flight::commanded` (one rule with `Knowledge`'s `read_by_computer`) |
| The stage estimates (`systems::estimate`) | `Knowledge` for tank levels and thrust; the true statuses and contents as the readings' centres | **moved** for thrust (an engine whose readout reaches no computer is a dash); the centres stay (D106: the truth at its precision) |
| The staging bar's inputs (`game::interface::stage_bar::refresh`) | g at the true position (gated on Position known), the air there for CURRENT AIR | left (FIXES2's file; D120 made vacuum the default) |
| The landing prediction (`landing::LandingStart::of_vessel`, `game::landing`) | the true state, attitude and propellant | **moved** gate: `flight::can_predict` (navigation; the attitude for the inertial assumption); its values the truth (known), its propellant the consumables (left) |
| The engine start's wait for settled propellant (`command::apply`, `propulsion::settle_or_start`, `feeds_settled`) | the true liquid position | left: the real sequence is a timed ullage burn (D115's adaptive functions) |
| The parachutes' deploy sequence (`chute::ChuteState::step` in `live::begin_tick`) | the true height and dynamic pressure | left: Dragon's computer deploys on barometric altitude; with recommendation 3 (the entry) |
| The g-load limit (`systems::g_limit`, navball and systems screen) | the true joint loads | left: the real vehicle has no strain gauges in flight; its limits are its design's |
| Gas ingestion and cavitation cut by the controller (`propulsion::inlet_trouble`) | the inlet's true state | stays: the engine's physics and its controller's own sensors |
| Power shedding (`power::step_ship`), structure, failures, crew | the true state | stays: physics and the buses' own electronics, not flight software |
| The pad's fresh vessel after an abort (`game::pad`), the state line (`hud::state_line`) | true engine statuses | stays (world management, a display); the state line could read the readouts |
| The MCP agent's tools (`game::agent`) | the true state, TWR | left: the player's AI agent should see what the player sees (`Knowledge`) |
| The demo's scripted flights (`game::demo*`) | the truth | stays: a test harness |

### Decisions taken (unattended)

- A reading's value is the truth at its sensor's precision (D106); the software acts on it, so with every sensor working nothing changes bit for bit. Noise, drift and outages later, only where they matter (D115).
- An attitude reference is attitude **and** rates: Dragon's star trackers without its IMUs give none ("no attitude reference", as the acceptance asks).
- Without GNSS the direction holds hold the attitude: the navigation filter on the IMU is D115's adaptive function.
- Without a working computer neither SAS nor the pilot's rotation reaches the actuators; translation is left as it was.
- Debug mode: the truth view only; the computers' health still counts (a computer switched off is off in debug too).
- A craft with no computers and no data links (hand-built fixtures) is ideal, as `Knowledge` already treats an unlinked craft.

### Left, in order

1. SAS and the holds in coasts and on rails through the layer: a small hunk in `vessel/coast.rs`, now free (COAST committed it: dbe3f3b, 861157e). Gate the coast's controls through `Sas::acting` at the systems lattice's epochs, not at frame starts, so 60 fps and one jump agree; test it with every IMU lost on a coast.
2. The parachutes' deploy on the computer's barometric altitude; the engine start's ullage as a timed burn (adaptive functions, D115).
3. The MCP agent's tools through `Knowledge`.

### Open for the owner

- **Safing (E):** a Merlin's controller that loses every flight computer shuts its engine down. No source for the Merlin; the Shuttle's SSME held its last command until the crew acted. Which should the Falcon 9 do?
- **A lost controller** (its own function lost) leaves its engine running at its last setting, unseen: the launch commit aborts on the pad; in flight it runs until the pilot or the sequence stops it. Should a lost controller shut its engine (fail-safe valves)?
- **Debug mode and the computers:** debug keeps the truth view; should it also ignore dead computers (SAS holding with every computer off)?
