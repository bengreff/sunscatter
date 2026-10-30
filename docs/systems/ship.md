# The ship: parts, components, layers, state, mass

What a craft is made of and how its state is kept. A craft is **parts** (physical objects with a shape and mounting points), **components** inside them (bottles, batteries, computers, sensors, valves, seats), and **layers** across them (structure, plumbing, electrical, data, thermal, mechanisms). Everything comes from a generator, and real hardware is a catalog entry. The first vehicle is the Falcon 9 Block 5 with Crew Dragon (D100). The systems that run on this model have their own docs: [propulsion](propulsion.md), [structure](structure.md), [aero and thermal](aero-thermal.md), [avionics](avionics.md), [crew](crew.md), [failures](failures.md).

**Status:** the Falcon 9 Crew, built from catalog parts, flies in the game from the pad through stage separation: *playable*. The foundations F1 (split and merge by construction) and F2 (one mass truth) are done. D121's simplification is under way: one kind of craft (the test craft and its made-up path are being deleted, ONEKIND), then one state, one advance law per subsystem and debris lumps (the next wave in the [hand-off](../plans/HANDOFF.md)). The visual editor and the glTF importer are not built (D102).

## Laws

- **Parts, components, layers** (D100, D018, D033): systems are layered on the part model, abstracted but not too far (D104): they run as behaviour, not as tables of numbers.
- **Every part comes from a generator** (D101): properties are derived from the generator and its materials; published numbers override them, each with a source. A gap between a derived and a published total is reported as unmodelled mass, never spread. Data carries its tags (P published, E estimate, D derived, U unknown: [falcon9.md](../vehicles/falcon9.md)); missing data takes the analogue's value, marked E.
- **Follow the real vehicle** (D115): its sensors only, published figures over derived ones, guidance only where the real vehicle flies itself (D117).
- **Rigid until it breaks** (D103, D032): mechanisms move; nothing wobbles.
- **A vessel is what is joined by structure or by data**, and keeps its own controls (D112). Splits and merges come from the construction (F1): every piece keeps the whole program, seen through its own view.
- **One mass truth** (F2): a craft of parts' propellant and mass are its ship state's; each liquid sits where its slug is. On a coast the tanks follow one law of time that the trajectory reads too (D119).
- **Scale** (D121): a 5,000-part craft flown live and 50 craft of 300 parts at 10⁶×, through one kind of craft, one state, and one advance law per subsystem over any interval.

## What exists

- **Data:** materials (D101, `data/materials.ron`) and fluids (D104, `data/fluids.ron`) with properties against temperature; the part catalog of generators (`data/parts/<family>/<id>.ron`: tanks, shells, thrust structures, engines, fins, grid fins, legs, surfaces of revolution, components). *Playable.*
- **Craft files** (D102, `data/craft/<craft>/craft.ron`): catalog parts attached through compatible mounting points, patterns, layer connections, the stage sequence and saved functions. Configurations (legs out, grid fins in the flow) are craft of their own. *Playable.*
- **The build** (`sim::craft::build`, `layers`): the ship definition (tanks, bottles, engines and feeds, thruster blocks, valves, batteries, mechanisms, sensors), the per-design bake for aero and the volume grid. *Playable*; slow at scale (a 1,000-part design takes ~13.5 s: the D121 work).
- **The ship state** (`sim::craft::state`, `sim::systems::runtime`): everything that changes in flight, saved with the vessel. Two structs today (`ShipState`, `Runtime`), to be merged (D121, STATE).
- **The program** (D107, D111, D114, `sim::craft::program`): the stages and functions, each piece's view of them, what can start and what is offered (D120: an abort only when it would save the crew or the vehicle). *Playable.*
- **Splits** (`sim::vessel::split`): momentum and angular momentum exact, every field of the vessel by its own rule. *Playable.* See [structure](structure.md).
- **The stress craft** (`sim::craft::stress`): N synthetic parts for benchmarks and tests (D081); being moved onto real catalog types (ONEKIND).

## Not built

- The visual editor and the glTF importer (D102); a player's own parts as catalog entries (D101).
- Building from abstract components, materials and factories (D118): *direction*, later.
- Docking ports, and a cabin list joined by hatches (F1's left item).
- The career's knowledge of a design's reliability, refurbishment and costs (D105, open question 1).

## Owners and decisions

- Owners: [owners.md, The ship](../owners.md#the-ship-parts-craft-definition-state).
- Decisions: D018, D032, D033, D036, D081, D100–D104, D107, D112, D114, D115, D117–D121.
- Technical: [ship-model.md](../technical/ship-model.md) (the design: concepts, data, build, runtime, failures, information, budgets), [parts.md](../technical/parts.md) (the part layer, splits, breakage, debris), [ship-systems.md](../technical/ship-systems.md) (each system as built). Vehicles: [falcon9.md](../vehicles/falcon9.md).
