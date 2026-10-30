# Avionics: power, data, sensors, flight software, comms

What a craft knows, what its own computers decide, how it is powered, and how it talks to the ground. The player and every agent see a craft only through what its sensors measure and its links carry; the vehicle's computers act only on what they know.

**Status:** the information model (`Knowledge`) is *playable* and the interface reads it. Power is *playable*. The comm network (sites, link budgets, line of sight, light time, relays) and light-delayed control are *playable*. The flight software is a *skeleton*: one layer that SAS, the holds, the launch commit and the engine controllers' safing go through, with no guidance and no adaptive functions yet. Ship scripting is *design*.

## Laws

- **Information comes from sensors** (D106): a value is shown only when a working, powered, linked sensor measures it, at the digits its precision gives; otherwise a dash. The interface never shows something wrong. Debug mode shows the truth (D064), but a dead or unpowered computer is still dead (D123).
- **The vehicle's computers read what the sensors know** (F3; D115, D117): the true state enters the flight software in one place (`sense`). What flies itself in reality flies itself in the game, and guidance exists only where the real vehicle has it.
- **Control depends on the speed of light** (D034, D067, D068): the player and their agent act from control locations; commands and telemetry travel the relay network with its light delay. A probe acts on the controls that have reached it.
- **A vessel keeps its own controls** (D112): what is set, what has reached it, and an uncrewed vessel's settings in flight.
- **Fail-safe engines** (D123): an engine whose flight computers are all lost, or whose own controller is lost, shuts down (E until sourced).
- **Flown by hand until scripting** (D108): scripting will read the same information model; ship scripts are WASM modules (D035, D043).

## What exists

- **Knowledge** (`sim::systems::data`): position, velocity, attitude, rates, acceleration; tank levels, pressures and temperatures; bottle pressures; engine thrust; battery charge; joint loads; part temperatures; the cabin's air. From IMUs, star trackers, GNSS, level sensors, pressure transducers and thermocouples. *Playable.*
- **The flight software** (`sim::systems::flight`): SAS and the holds need an attitude reference and a working computer, a direction hold a navigation solution too; the pad's launch commit; the engine controllers' safing when every linked computer is lost. *Skeleton.*
- **Power** (`sim::systems::power`): each bus's loads against the pad's umbilical, solar cells and batteries (I²R losses), shedding the least essential load. *Playable.*
- **Comms** (D068, `sim::comms`, `game::comms`): ground sites, link budgets, line of sight against the bodies' ellipsoids, light time, relay paths; delayed telemetry and commands. *Playable.*
- **Controls** (D112, `sim::vessel::helm`). *Playable.*

## Not built

- **Guidance** (D117, D123): convex powered-descent guidance for the booster's landing burn (lossless convexification, re-solved several times a second with a fixed iteration count); a predictor–corrector for its boostback and entry burns; Dragon's docking at three autonomy levels.
- **Adaptive functions** (D115): engine-out re-trim, propellant utilisation, the navigation filter carrying the state on the IMU when GNSS is lost, the timed ullage burn, the chutes' deploy altitude from the computers.
- SAS and the holds on coasts still hold on the truth (FIXES3 in the [hand-off](../plans/HANDOFF.md)).
- Sensor noise where needed (D115), GNSS outages, dead reckoning.
- Radiation's single-event upsets (open question 1).
- The ship's operating system and scripting (D035, D043; pillar 4 of the [vision](../vision.md)).

## Owners and decisions

- Owners: [owners.md, Avionics](../owners.md#avionics-data-flight-software-controls-power-comms).
- Decisions: D034, D035, D043, D064, D067, D068, D104, D106, D108, D112, D115, D117, D123.
- Technical: [ship-systems.md](../technical/ship-systems.md) (information, flight software, power, commands and the pad), [ship-model.md](../technical/ship-model.md) §5.5, §7, §8. The comm network's spec is in the archived [realism-1](../archive/plans/realism-1.md) §4.
