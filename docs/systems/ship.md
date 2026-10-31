# The ship: parts, components, layers, state, mass

What a craft is made of and how its state is kept. A craft is **parts** (physical objects with a shape and mounting points), **components** inside them (bottles, batteries, computers, sensors, valves, seats), and **layers** across them (structure, plumbing, electrical, data, thermal, mechanisms). Everything comes from a generator, and real hardware is a catalog entry. The first vehicle is the Falcon 9 Block 5 with Crew Dragon (D100). The systems that run on this model have their own docs: [propulsion](propulsion.md), [structure](structure.md), [aero and thermal](aero-thermal.md), [avionics](avionics.md), [crew](crew.md), [failures](failures.md).

**Status:** the Falcon 9 Crew, built from catalog parts, flies in the game from the pad through stage separation: *playable*. The foundations F1 (split and merge by construction) and F2 (one mass truth) are done. D121's simplification is under way:
- one kind of craft: the tests fly small craft of parts (`craft::small`); the test craft, its file format and its made-up path are deleted (ONEKIND, D121);
- one state, one advance law per subsystem, and debris lumps: *design* (STATE, ADVANCE and DEBRIS in the [hand-off](../plans/HANDOFF.md)).

D125's universal physics holds on the parts path, with the gaps listed under Not built. The visual editor and the glTF importer are not built (D102).

## Laws

- **Parts, components, layers** (D100, D018, D033): systems are layered on the part model, abstracted but not too far (D104): they run as behaviour, not as tables of numbers.
- **Every part comes from a generator** (D101): properties are derived from the generator and its materials; published numbers override them, each with a source. A gap between a derived and a published total is reported as unmodelled mass, never spread. Data carries its tags (P published, E estimate, D derived, U unknown: [falcon9.md](../vehicles/falcon9.md)); missing data takes the analogue's value, marked E.
- **Follow the real vehicle** (D115): its sensors only, published figures over derived ones, guidance only where the real vehicle flies itself (D117).
- **Physics is universal** (D125): no ship-specific models. A behaviour is modelled once, for every craft that has the parameters for it, or it is not modelled. Every part is fully described by its parameters.
- **Rigid until it breaks** (D103, D032): mechanisms move; nothing wobbles.
- **A vessel is what is joined by structure or by data**, and keeps its own controls (D112). Splits and merges come from the construction (F1): every piece keeps the whole program, seen through its own view.
- **One mass truth** (F2): a craft of parts' propellant and mass are its ship state's; each liquid sits where its slug is. On a coast the tanks follow one law of time that the trajectory reads too (D119).
- **One kind of craft, one state, one advance law** (D121). Every craft is built from parts (D100); the test craft and the made-up path (one placeholder part with a fuel number) are deleted. Each vessel has one state, grouped by subsystem and split and merged by construction; ids are strings only at the edges (files, saves, the interface, MCP) and typed handles inside. Each subsystem advances over any interval, exact or coarse-grained, chunked = single pass.
- **Scale** (D121): a 5,000-part craft flown live at 1× and 60 fps on the M2 Pro, and 50 craft of 300 parts at every warp up to 10⁶×. Live physics has no cap on the number of craft.
- **Separations** (D121): each new piece is checked for control (radio, crew or an onboard computer, and power). A controllable piece becomes a full craft, simulated while inactive. Any other piece becomes a ballistic lump: its mass, inertia, a drag area and heating from its bounding shape, and one heat node. The pieces the stage sequence can make get their designs when the craft is built; an unplanned controllable piece gets a design composed from its parts' bakes.

## What exists

- **Data:** materials (D101, `data/materials.ron`) and fluids (D104, `data/fluids.ron`) with properties against temperature; the part catalog of generators (`data/parts/<family>/<id>.ron`: tanks, shells, thrust structures, engines, fins, grid fins, legs, surfaces of revolution, components). *Playable.*
- **Craft files** (D102, `data/craft/<craft>/craft.ron`): catalog parts attached through compatible mounting points, patterns, layer connections, the stage sequence and saved functions. Configurations (legs out, grid fins in the flow) are craft of their own. *Playable.*
- **The build** (`sim::craft::build`, `layers`): the ship definition (tanks, bottles, engines and feeds, thruster blocks, valves, batteries, mechanisms, sensors), the per-design bake for aero and the volume grid. *Playable*; slow at scale (a 1,000-part design takes ~13.5 s: the D121 work).
- **The ship state** (`sim::craft::state`): everything that changes in flight, saved with the vessel, in one state per vessel (D121): grouped by subsystem (plumbing, propulsion, attitude control and the aero surfaces, valves, batteries, mechanisms, structure, power, the cabin and crew, failures, the program, the motion the systems felt), each item of a part holding all of its state, split and merged field by field (`craft::partition`). *Playable*. Inside, a function's owner is a typed handle (`craft::handle::Owner`), its lost, unpowered and fate lists hold handles, and a part's state is found by a binary search; ids are strings at the edges (files, the interface, MCP, events). Left: plumbing's heated-tank check and the estimate's joints by name, and the engine indices (`release_engines`) as typed ids (STATE).
- **The program** (D107, D111, D114, `sim::craft::program`): the stages and functions, each piece's view of them, what can start and what is offered (D120: an abort only when it would save the crew or the vehicle). *Playable.*
- **Splits** (`sim::vessel::split`): momentum and angular momentum exact, every field of the vessel by its own rule. *Playable.* See [structure](structure.md).
- **Small craft of parts** (`sim::craft::small`, D121): rockets of catalog test parts built in code for tests and benchmarks, never offered in the game (`small-rocket`: a capsule with a computer, a seat, cold-gas thrusters, Apollo parachutes, an IMU and a GPS receiver, on an RP-1/LOX tank and four 100 kN engines with legs). They replace the test craft in the tests (ONEKIND).
- **The stress craft** (`sim::craft::stress`, `stress-<n>`): exactly N catalog parts for benchmarks and tests (D081, D121): a capsule on a core stack with rings of radial boosters. The placeholder layout is deleted.
- **The made-up path is deleted** (D121): the test craft and its single-part file format, the scalar propellant, the boil-off placeholder, D078's ideal couples and the lumped engine. A craft that does not build is replaced by the small rocket of parts (`game::state::craft_or_fallback`). Left: the game's branches for a craft without a ship definition, the placeholder fields of `parts::Part`, and the playtest save (`./run play`), which still holds the test craft and no longer loads.

## Not built

- **D121:** one state per vessel (`ShipState` and `Runtime` merged; typed handles inside: STATE); one advance law per subsystem (ADVANCE); the controllability check at separation, ballistic lumps and the pieces' designs at build (DEBRIS).
- **D125's gaps:** every vessel carries one antenna constant, not its parts' (`game::comms::VESSEL_ANTENNA`); the SuperDraco's thrust ignores its feed pressure; the made-up path until ONEKIND deletes it.
- The visual editor and the glTF importer (D102); a player's own parts as catalog entries (D101).
- Building from abstract components, materials and factories (D118): *direction*, later.
- Docking ports, and a cabin list joined by hatches (F1's left item).
- The career's knowledge of a design's reliability, refurbishment and costs (D105, open question 1).

## Owners and decisions

- Owners: [owners.md, The ship](../owners.md#the-ship-parts-craft-definition-state).
- Decisions: D018, D032, D033, D036, D081, D100–D104, D107, D112, D114, D115, D117–D121, D125.
- Technical: [ship-model.md](../technical/ship-model.md) (the design: concepts, data, build, runtime, failures, information, budgets), [parts.md](../technical/parts.md) (the part layer, splits, breakage, debris), each subsystem as built: [ship-commands.md](../technical/ship-commands.md), [plumbing.md](../technical/plumbing.md), [engines.md](../technical/engines.md), [mechanisms.md](../technical/mechanisms.md), [knowledge.md](../technical/knowledge.md), [flight-software.md](../technical/flight-software.md), [joint-loads.md](../technical/joint-loads.md), [failure-modes.md](../technical/failure-modes.md), [power.md](../technical/power.md), [life-support.md](../technical/life-support.md). Vehicles: [falcon9.md](../vehicles/falcon9.md).
