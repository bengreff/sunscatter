# Systems

One general doc per main system (D124): what it does, the laws it keeps, what is built (with honest status words), what is not, its owners and its decisions. Detail lives in [technical/](../technical/), one topic per file; who owns each piece of code is in [owners.md](../owners.md).

Status words (rule 7, [process.md](../process.md)): *design*, *skeleton*, *playable*, *polished*. As these docs use them: *design* is written down and not built; *skeleton* is built and tested but not yet doing the whole job; *playable* does its job in the sim or the game, under tests; *polished* is not used yet. Each doc says where a part is in the sim only, or not yet seen on screen.

## Written

| System | What it covers |
|---|---|
| [Motion](motion.md) | Frames, ephemerides, trajectories, coasts and limbo, warp, precision, determinism, the 10 km bubble, navigation tools |
| [The world](world.md) | The Solar System's bodies, gravity fields, atmospheres, terrain and surfaces, light, weather, water |
| [The ship](ship.md) | Parts, components and layers, the catalog, craft files and the build, the ship state and program, mass |
| [Propulsion](propulsion.md) | Tanks and pressurant, plumbing, engines, RCS, the gimbal, stage estimates, the exhaust seed |
| [Structure](structure.md) | Joint loads, mechanisms, separation and breakage, debris, ground and craft-to-craft contact, docking, water impact |
| [Aero and thermal](aero-thermal.md) | Aerodynamics on cells, heating, the thermal network, the shock layer and blackout, parachutes |
| [Avionics](avionics.md) | Sensors and what a craft knows, the flight software, power, comms and light delay, controls, guidance, scripting |
| [Crew](crew.md) | The cabin's air, acceleration limits, D116's doses |
| [Failures](failures.md) | Physical triggers, statistical fates, outcomes, ratings |
| [The game](game.md) | Rendering, the flight interface, the systems screen, camera and map, saves, the MCP server and the agent's tools |

## Open (not written yet)

Left open on purpose (D124: "colonies, economics, construction, fusion, various interstellar techs, etc, just leave that open"). Each gets its doc when its design starts; until then its decisions are the record.

- **The career:** an aerospace startup from 2030 (D027).
- **Economics and logistics:** trade routes as tools for managing missions, not abstract cargo (D017).
- **Construction and industry:** building from a few thousand abstract components, materials and factories, the same way on Earth and in colonies (D118).
- **Colonies** (D019, D118; decisions.md's open question 3).
- **Advanced propulsion and power:** fusion and beyond; an engine and propulsion simulator (D020).
- **Interstellar:** the 100-light-year sphere of stars, procedural bodies and the techs to reach them (D004, D007, D010, D013, D014).
- **The ship's operating system and scripting** (D035, D043, D108), now in [avionics](avionics.md) until it grows.
- **Radiation:** dose and single-event upsets (open question 1), now in [crew](crew.md) and [avionics](avionics.md).

## Writing a systems doc

Keep it general: the laws, the parts with their status words, what is not built, and links to the owners, the decisions and the technical docs. Describe only what the code and the tests show. A detail that needs more than a paragraph gets its own file in `technical/`.
