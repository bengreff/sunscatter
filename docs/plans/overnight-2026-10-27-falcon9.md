# Overnight plan, 2026-10-26/27: the part system and a flyable Falcon 9

From the owner's interview ([record](interview-2026-10-26-parts.md)); decisions D100–D108; the design is [ship-model.md](../design/ship-model.md). The owner: "Start with just building the complete falcon 9 model within the part system foundation, controls and systems taken to the proper level of realism, allowing me to fly it. … I would like to see a falcon 9 at the proper level of simulation when I wake up." Up to 3 agents at a time (owner). Status words: *design / skeleton / playable / polished*.

The same night, already running: **GFX** (one look, flare, lighting, the 27 fps), **NAV** (origin clock, orbit lines, zoom, picking), **GRAV** (gravity fields, D090). The workstreams below start as they finish.

## Workstreams

| Name | What | Files | Needs |
|---|---|---|---|
| **DATA** | The sourced data sheet [falcon9.md](../features/falcon9.md): every part and component of Falcon 9 Block 5 and Crew Dragon (dimensions, masses, materials, propellant loads and temperatures, tank pressures, COPVs, engines' published performance, throttle ranges, ignitions, start and shutdown transients, gimbal, cold-gas and Draco/SuperDraco thrusters, grid fins and legs, separation systems, avionics, batteries, sensors, parachutes, ECLSS), the materials and fluids tables' values, ratings and failure data (class-level rates with their low bounds), vibration qualification levels, propellant explosion yields, crew limits, the flight sequence (Crew missions' timeline). Each number cited and marked *published*, *estimate* or *derived*. | `docs/features/falcon9.md`, `data/materials.ron`, `data/fluids.ron` | — (research only, no builds) |
| **CORE** | The foundation in `sim`: `sim::materials`, `sim::fluids`, `sim::catalog` (generator framework, entries with published overrides and ratings), generators (tank with domes and common bulkhead, shell/interstage, thrust structure, engine, nozzle, fin, grid fin, leg, capsule as a surface of revolution, nosecone, trunk, heat shield, bottle, box components), craft file v2 (mounting points, patterns, components, joints, layer declarations, sequence, functions), the build into the existing pipeline, `ShipState`, mass properties from every tank's fill, each engine at its own mount and gimbal in the live tick, splits that partition the ship state by part, saves, the registry. The test craft keeps flying unchanged. Then the Falcon 9 Crew craft file and catalog entries from DATA. | `sim::craft` (file, parts, mass, mod, registry), new `sim::{materials, fluids, catalog}`, `data/parts/`, `data/craft/falcon9-crew/` | DATA for the real numbers (placeholders marked until then) |
| **RUNTIME** | `sim::systems::{plumbing, propulsion, rcs, mechanisms, power, structure, vibration, failure, data, crew}`: the behaviour of §5–§7 of the design, `ShipCommand`, sequences and functions, clamps, grid-fin aero, legs, knowledge from sensors, debug mode's perfect information. | `sim::systems::*`, `sim::vessel::live` (wiring), `sim::aero` (grid fins) | CORE's types |
| **GAME** | The flight with a Falcon 9: a new-flight choice of craft; the Falcon 9 on the pad at LC-39A; every part drawn with its own mesh and colour (pieces too); legs, fins and nosecone moving; a flame per running engine; the systems screen (layers, inspector, commands, stages, functions, pins) and the mini systems panel; Space for the next stage; readouts through knowledge (dashes, precision); switching to the booster after separation. | `game::systems_screen` (new), `game::scene`, `game::navball`, `game::interface`, `game::commands` | CORE's types, RUNTIME's commands and knowledge; after GFX and NAV (shared files) |

## Order (at most 3 agents at once)

1. First free slot: **CORE** (the critical path). Second: **DATA** (research only, light on the machine). Third: **GAME** once GFX and NAV are both done (it shares `scene`, `navball`, `interface` with them).
2. When DATA finishes: **RUNTIME** (systems need CORE's types; it starts with plumbing and propulsion).
3. The lead: reviews each push, runs the offscreen demos (one at a time), writes decisions and docs, keeps the test craft and every existing test green, and a final independent review agent if a slot is free.

If the night runs short, cut in this order (owner's default): Dragon's insides (ECLSS, Draco plumbing) first, then the non-physical failures (vibration, statistical); the two Falcon stages keep full depth.

## Acceptance for the morning (honest status in the table below)

- The Falcon 9 Crew stands on the pad at LC-39A, fuelled at its loading temperatures, clamps holding, four crew aboard.
- Space starts the nine Merlins (each with its own state, ignitions and transient); the clamps release when all run; the owner flies the ascent by hand (gimbal SAS, throttle in each engine's range).
- MECO, stage separation (pneumatic pushers), the booster a controllable vessel the owner can switch to (grid fins, cold gas, legs, 3- and 1-engine burns); MVac ignition only with settled propellant; the second stage to orbit; Dragon separates.
- The systems screen shows the craft by layer, with commands, stages, functions and pins; the mini panel works in flight; readouts come from the sensors.
- Physical failures fire from the simulation (overload snap, overheating, overpressure, gas ingestion, cavitation); statistical ones only through fates; debug mode turns them all off and gives perfect information.
- Every result the same bits at 1× and in one jump; the test craft and every existing test unchanged.

## Working rules

- No branches or worktrees. Commit only your own paths (`git commit -m … -- <paths>`), push after each commit; the pre-push hook (now enabled) runs CI's checks. Keep the crate compiling: write new modules completely before wiring them in.
- Each agent builds in `CARGO_TARGET_DIR=target/agents/<name>`, `-j 4`, one cargo command at a time; delete the directory when finished (the disk is ~85 % full).
- Agents never run the game or the demo; the lead runs offscreen demos, one at a time.
- Decision numbers: D100–D108 are the parts decisions (lead); the evening's agents use D088–D099.

## Status at the end of the night

(Filled in by the lead.)
