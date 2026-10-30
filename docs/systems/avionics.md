# Avionics: power, data, sensors, flight software, comms

What a craft knows, what its own computers decide, how it is powered, and how it talks to the ground. The player and every agent see a craft only through what its sensors measure and its links carry; the vehicle's computers act only on what they know.

**Status:**
- The information model (`Knowledge`) is *playable*; the interface and the agent's tools read it.
- Power is *playable*.
- The comm network (sites, link budgets, line of sight, light time, relays) and light-delayed control are *playable*. D034's revision is not built: today a command to another vessel leaves from wherever the player is, so from a crewed vessel a probe is commanded with the delay between the two, not from mission control.
- The flight software is a *skeleton*. D043's one flight-computer layer per vessel exists (`sim::systems::flight`): SAS, the holds, the launch commit and the engine controllers' safing go through it. Its functions are native Rust, with no compute budget, no guidance and no adaptive functions yet. D043's WASM programs and ship scripting are *design*.
- D126's automation switch is *design*.

## Laws

- **Information comes from sensors** (D106): a value is shown only when a working, powered, linked sensor measures it, at the digits its precision gives; otherwise a dash. The interface never shows something wrong. Debug mode shows the truth (D064), but a dead or unpowered computer is still dead (D123).
- **The vehicle's computers read what the sensors know** (F3; D115, D117): the true state enters the flight software in one place (`sense`). What flies itself in reality flies itself in the game, and guidance exists only where the real vehicle has it.
- **Control depends on the speed of light** (D034, D067, D068): the player and their agent act from control locations; commands and telemetry travel the relay network with its light delay. A probe acts on the controls that have reached it.
- **A vessel keeps its own controls** (D112): what is set, what has reached it, and an uncrewed vessel's settings in flight.
- **Fail-safe engines** (D123): an engine whose flight computers are all lost, or whose own controller is lost, shuts down (E until sourced).
- **One flight-computer layer per vessel is the ship OS** (D043). Every computer on the ship is summed into one abstracted operating system. Its compute budget is the sum of the computers' real processors (or their class, marked E), and a lost computer takes its share. Every program on it is a WASM module with one API (`Knowledge` in, commands out, the budget), sandboxed and deterministic within its budget per tick. That covers the vehicle's own flight software, which ships with the craft (readable, editable, shareable), and the player's programs alike. Native code is only the layer itself (sensors to `Knowledge`, commands to actuators). Scripts are plain files on disk, driven by a command-line tool, in any language that compiles to WASM (D035).
- **Commands come from the nearest mission control** (D034). An uncrewed ship is commanded with the real light delay from the nearest mission control (on Earth at first, later on other planets). New commands and programs travel over the relay network, and a crewed ship nearby does not command probes. A crewed ship is controlled at once, "as if the player is right there on the ship".
- **Automation is a switch; manual control is always possible** (D126). Every automatic sequence (the parachutes' deploy on barometric altitude, a planned burn, the booster's guidance) can run by itself, when the player presses for it, or not at all. A manual "deploy" runs the same function as the automatic sequence. The autopilot is a switch with a config menu. Manual control, with today's aids (automatic engine vectoring), is always there, "even if it leads to catastrophic failure".
- **Flown by hand until scripting** (D108): scripting reads the same information model.

## What exists

- **Knowledge** (`sim::systems::data`): position, velocity, attitude, rates, acceleration; tank levels, pressures and temperatures; bottle pressures; engine thrust; battery charge; joint loads; part temperatures; the cabin's air. From IMUs, star trackers, GNSS, level sensors, pressure transducers and thermocouples. *Playable.*
- **The flight software** (`sim::systems::flight`, D043's layer): SAS and the holds need an attitude reference and a working computer, a direction hold a navigation solution too; the pad's launch commit; the engine controllers' safing when every linked computer is lost. Each is a native function of what the sensors know, the computers' health and the craft's definition, the shape a WASM program will take. *Skeleton.*
- **Power** (`sim::systems::power`): each bus's loads against the pad's umbilical, solar cells and batteries (I²R losses), shedding the least essential load. *Playable.*
- **Comms** (D068, `sim::comms`, `game::comms`): ground sites, link budgets, line of sight against the bodies' ellipsoids, light time, relay paths; delayed telemetry and commands. *Playable.* The control location (D067, `game::comms::location_rule`): aboard a crewed active vessel the player commands it at once; flying an uncrewed vessel, or at the tracking station, the player is at mission control (one site, on Earth, in `data/comms.ron`). An uncrewed vessel's helm carries the light delay itself (`game::commands::controls_route`).
- **Controls** (D112, `sim::vessel::helm`): manual control with SAS and the gimbal's automatic vectoring, always available (D126). *Playable.*
- **The parachutes' sequence** (D079, `sim::chute`): the command arms it, then the drogue and the main open by themselves. There is no automatic arming (Dragon's computer deploys on its barometric altitude) and no off switch yet (D126).

## Not built

- **Guidance** (D117, D123): convex powered-descent guidance for the booster's landing burn (lossless convexification, re-solved several times a second with a fixed iteration count); a predictor–corrector for its boostback and entry burns; Dragon's docking at three autonomy levels.
- **Adaptive functions** (D115): engine-out re-trim, propellant utilisation, the navigation filter carrying the state on the IMU when GNSS is lost, the timed ullage burn, the chutes' deploy altitude from the computers.
- SAS and the holds on coasts still hold on the truth (FIXES3 in the [hand-off](../plans/HANDOFF.md)).
- Sensor noise where needed (D115), GNSS outages, dead reckoning.
- Radiation's single-event upsets (open question 1).
- **The ship OS** (D043, D035; pillar 4 of the [vision](../vision.md)): the WASM runtime and its one API, the compute budget summed from the computers, the vehicle's flight software as WASM modules, the player's scripts and their command-line tool.
- **D034's routing:** commands to a vessel leave only from the nearest mission control, and programs upload over the relay network; with one mission control there is no choice of the nearest yet.
- **D126's switch:** each automatic sequence by itself, on a press or off; the autopilot's config menu; planned burns as flight-layer programs (PLANNER, [motion](motion.md)).
- **Antennas from the parts** (D125): every vessel carries the same antenna constant (`game::comms::VESSEL_ANTENNA`, the test craft's).

## Owners and decisions

- Owners: [owners.md, Avionics](../owners.md#avionics-data-flight-software-controls-power-comms).
- Decisions: D034, D035, D043, D064, D067, D068, D104, D106, D108, D112, D115, D117, D123, D125, D126.
- Technical: [knowledge.md](../technical/knowledge.md), [flight-software.md](../technical/flight-software.md), [power.md](../technical/power.md), [ship-commands.md](../technical/ship-commands.md) (commands, the pad, light-delayed orders), [ship-model.md](../technical/ship-model.md) §5.5, §7, §8. The comm network's spec is in the archived [realism-1](../archive/plans/realism-1.md) §4.
