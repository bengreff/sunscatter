# Feature: parts, splits and debris

**Status:** *playable*. The part system's foundation (D100–D104, `docs/technical/ship-model.md`), built 2026-10-27 by CORE: materials and fluids as data, the catalog of generators, craft files of catalog parts, the build, the ship definition and state, and a vessel of parts flying its engines, separating and saving, all tested. The systems' behaviour (`sim::systems`) and the game's craft drawn part by part were built on it, and the Falcon 9 Crew, from [falcon9.md](../vehicles/falcon9.md)'s numbers, flies from the pad to orbit ([the ship](../systems/ship.md)). The part layer below it (2026-10-26, PARTS): splits, breakage triggers and the debris rule; the stress craft is of catalog parts since ONEKIND-3 (`craft::stress`). Decisions: D032, D033, D036, D060, D064, D065, D066, D081, D083, D100–D108.

## Prior work

| Source | What it does | Taken | Rejected |
|---|---|---|---|
| KSP | A vessel is a tree of parts; decouplers and broken joints split the tree into vessels; joints break above `breakingForce`/`breakingTorque`; debris persists up to a count limit | The attachment tree, a split as a cut of the tree, each group a vessel, a per-part load limit | Physics-joint flex (D032: rigid bodies); a debris count limit (D083: a rule) |
| Orbiter (docking and undocking) | Separated vessels each continue from the composite's state | Rigid-body separation: each piece at its place in the composite, moving with it | — |

## The foundation (D100–D104)

### Data
- **Materials** (`sim::materials`, `data/materials.ron`): density, modulus, yield and ultimate strength against temperature, conductivity, specific heat, emissivity, absorptivity, service and destruction temperatures, ablation data, the S–N exponent, cost, era; finishes (paints, coatings, solar cells). A material gives a cell skin (`Material::skin`), a wall its areal mass, skin and allowable line load (`catalog::Wall`). Each entry names its sources; `verify` lists the fields still from memory (DATA checks them).
- **Fluids** (`sim::fluids`, `data/fluids.ron`): liquids' saturated density (linear in T) and vapour pressure (ln p linear in 1/T) with the saturation temperature as its inverse, specific and latent heat, freezing, boiling and critical points; ideal gases (M, γ, cₚ).

### The catalog (`sim::catalog`, `data/parts/<family>/<id>.ron`)
- A **generator** is Rust code: parameters → primitives in the part's frame (+Z up the stack, origin at the bottom centre; an engine's at its gimbal point), mounting points (name, position, outward normal, roll reference, size, kind: stack, engine, radial; the wall's line load there), a dry mass derived item by item, tank compartments and ports, functions, electrical draw, interior specific heat and limit. Generators: `Tank` (domes inside or beyond the skirts, a common bulkhead; walls from hoop stress, buckling with SP-8007's knockdown, the domes' crown, the minimum gauge), `Shell`, `ThrustStructure`, `Engine` (performance published, D104; a radiatively cooled extension is the nozzle of D077), `Fin`, `GridFin` (lattice), `Leg` (stowed, its mechanism and foot), `Revolution` (a (z, r) profile), `Component` (bottles, small tanks, batteries, computers, sensors, valves, seats, thruster blocks, engine pods, life support, chute packs, docking ports, other equipment).
- An **entry**: a generator with its parameters (`Tank(radius: …)`, newtype variants unwrapped, `Some` implicit), the published outputs the generator allows (thrust, Isp, capacity, the dry mass) each with its source, the reason for any unmodelled mass, ratings, failure modes, era, cost, `provisional`. Building an entry reports each published value against the derived one; beyond the tolerance (5 %) it is a contradiction, and a published dry mass leaves the gap as the part's unmodelled mass with its reason (D101).

### Craft files of parts (`sim::craft::assembly`)
A `data/craft/<craft>/craft.ron` without `geometry.ron`: parts (entry, `set` parameters by name, `mount: (on, at, via, roll_deg)` — only through compatible mounting points, a parent listed earlier; or `at: (on, pos, axis, roll_deg)`; the first part is the root), patterns (`List(n)`: `#` in mount names; `Radial(count, start_deg)`), components (`in` a part, at a position in its frame), joints (`Integral`, `Bolted` by default, `Separation(system, impulse, strength)`), plumbing lines (`part:port` to `part:port`, `#*` for every copy), electrical buses, data links, loading (each tank's fill, temperature, ullage pressure and pressurant), engine groups, the stage sequence and saved functions as lists of `ShipCommand`, and the whole-craft data the cells and contact take (aero, thermal, impact, contact, antenna). The single-part craft file still loads (the test craft, bit for bit).

### The build (`sim::craft::build`, `layers`, `place`)
Every part generated and placed by its mount (normals opposed, roll references aligned, turned by the roll; quarter turns snap exactly so boxes stay axis-aligned), components placed in their parts and checked (centre inside, volumes fitting; inside a tank they displace its liquid), tanks loaded, the plumbing traced (each engine's oxidizer and fuel tanks, each thruster block's bottles or tanks, through valves), joints with their capacity, buses, links, groups, and every command of the sequence and functions checked against what the craft has. The result is a `CraftDef` for the existing pipeline (union surface, cells by area, per-part shells plus components as solids, the tanks for the volume nodes, the aero bake and thermal network) with its `ShipDef`, and a `BuildReport`. Errors are collected, each naming the part, mount, port or command.

### Ship definition, state, commands (`sim::craft::{ship, state, command}`)
- `ShipDef`: the static items, each naming its part id; `subset` keeps a piece's items in order with feeds renumbered.
- `ShipState` (saved with the vessel): per tank the liquid's mass and temperature and the ullage's moles and temperature; bottles; engines (status, ignitions left, throttle, gimbal, selected); thruster blocks (enabled); valves; batteries; mechanisms (angle, target); joints (intact); parts (integrity, thermal and vibration damage, history); component power switches; the cabin; fates (fields only); the clamps; the sequence's position. At load: tanks at their loading temperature, bottles at their pressure, the first engines the sequence ignites selected. `partition` splits it by part ids exactly as `subset`.
- `ShipCommand`: ignite, shut down, throttle, select (engines by group or selector), open and close valves, deploy, retract, angle (mechanisms), separate (a joint), release clamps, arm chutes, RCS blocks on or off, component power. `ShipState::apply` gives each its plain effect (instant; transients are RUNTIME's) and returns what the vessel must do (a separation, the clamps, the chutes).

### A vessel of parts (`sim::vessel::ship`)
- **Mass**: the dry parts plus each tank's own liquid (a solid cylinder filling from its base) and each bottle's gas, recomputed every tick (`MassModel::with_slugs`); `propellant` is the consumables' total, so coasts, predictions and saves see the same mass (a coast changes it only by its continuous laws; the tanks keep their shares).
- **Engines** (`craft::engine::ship_thrust`): each running engine at its own mount with its own Isp pair and throttle range (the pilot's throttle times its own), drawing oxidizer and fuel by its mixture ratio from the first of its feed tanks that holds any; a tank that cannot supply the tick shares out what it holds. Thrust and mass flow summed; the gimbal acts on the thrust-weighted mount and direction with the smallest range (all deflecting together: exact for the force and the torque), so an engine out leaves a moment about the centre of mass.
- **RCS**: the enabled, fed thruster blocks' authority from their nozzles' geometry about the current centre of mass; their use draws the bottles' gas (cold gas) or tanks (bipropellant).
- **Commands and the sequence**: `Vessel::command`, `Vessel::next_stage`; the readouts (Δv, TWR, Isp) and the maneuver hold's axis follow the engines its throttle runs (`landing::Thrust::of_vessel`, `Vessel::thrust_axis`: those selected, else all main engines). A separation is a pending `Breakup` with the separated part, which the game applies like any breakup: the pieces' ship states partitioned by part, momentum exact.
- Planned burns of a craft of parts are refused (`PlanError::PartsUnsupported`) until the systems run on rails.

### Separation (`sim::vessel::split`, `sim::vessel::bubble`)
A `Separate` command (or the clamps' release) makes a pending breakup with the separated part; the game applies it like any breakup. At the split each fired separation joint gives its impulse along its normal (`JointDef::normal`, from the parent's mount towards the child): the child's piece pushed on, the parent's back, momentum exact (Falcon 9's pushers: 18 kN·s, ~0.5 m/s at MECO, falcon9.md §1.3). The pieces of one split list each other (`Vessel::clear_of`): while any hull point of one is inside the other (the MVac in the interstage it leaves) the bubble gives them no contact, and from the first tick they are clear they touch as any craft (a stage separation once wrecked the booster at 9 m/s through that contact).

### Configurations: mechanisms that move the shape (`sim::craft::configure`)
The craft is rigid (D103), so a moved mechanism is another shape: a configuration is a craft of its own, named `base[#parts]@deployed` (the part ids whose mechanisms are deployed), built once per process like any piece. `CraftDef::deploy` turns each deployed part about its mechanism's hinge by the deployed angle (a leg's foot pad becomes a contact point of kind foot, vertical once deployed; the ship definition's mechanisms and aero surfaces move with it). In flight, a command that deploys or retracts a mechanism (`Vessel::deployed_parts` differs from the craft id's) makes a pending breakup whose plan is one piece in the new configuration; the game applies it as any breakup (`game::debris`, the craft built off the frame), and `Vessel::split` with one piece swaps the craft keeping the centre of mass, the velocity and the angular momentum (ω from the new inertia), the ship state and the temperatures by part. Mechanisms move at once (the systems' rates are RUNTIME's).
A vessel created from a configuration's id (`small-rocket@7-10`) starts with those mechanisms deployed, so its first tick does not fold them back (09d44b8). `CraftParams::bottom_z` is the lowest contact point, which is a foot when the legs are out, so a craft on legs spawns standing rather than in the air (e57e934).

### The pad and its clamps
The pad's launch mount is a part (`Pad` generator, `Function::Pad`, `PartInfo::ground`): posts under a stack mount that takes the first stage's base, so the vehicle stands on it as on the real mount, its engines over the flame trench; a `HoldDown` joint holds it. While a hold-down joint is intact (`Vessel::clamped`) the vessel stays landed, welded to the ground, whatever runs; `ShipCommand::ReleaseClamps` separates every hold-down joint, and the mount's piece is left as ground (no vessel). Limits: while clamped nothing is integrated, so engines running on the pad spend no propellant and load nothing; RUNTIME may make the clamp a load-carrying anchor flown in live ticks.

### The Falcon 9 Crew (`data/craft/falcon9-crew`, `data/parts`)
Built from DATA's data sheet (`docs/vehicles/falcon9.md` §1–3; every number tagged there and in the entries): 33 parts (Crew Dragon's capsule, nosecone on its hinge, PICA-X heat shield, trunk and four fins; the second stage's Dragon ring, tanks, aft skirt and MVac with its niobium extension; the interstage and four grid fins; the first stage's tanks, octaweb, nine Merlins (E1, E5, E9 with three relights) and four legs; the pad's mount) and 70 components (Dragon's four MMH/NTO spheres, helium, four SuperDraco pods, 16 Dracos, seats, ECLSS, batteries, avionics, docking system, parachutes, crew and cargo, trunk solar array; both stages' helium COPVs in the LOX, transfer tubes, nitrogen bottles and thruster pods, avionics, prevalves, the estimated items DATA lists). Body axes: +Z up, the origin at the capsule's shoulder (falcon9.md's z 61.35), +X at falcon9.md's 45° (legs, grid fins and clamps on the axes). Totals: liftoff 560.2 t without the mount (falcon9.md: 560.2), first stage 25.6 t dry and 411.1 t of propellant and gas, second stage 4.0 t and 105.9 t, Dragon with trunk, crew and cargo 13.57 t; 65.65 m from the centre nozzle to the nosecone. The published stage and spacecraft masses carry these unmodelled gaps (reported with their reasons): first stage tanks +2.5 t, second stage +1.1 t, capsule +2.2 t, trunk +2.3 t; the legs' derived 173 kg against their published 500 kg is a reported contradiction. The sequence: Ignition, Liftoff (the clamps), MECO, Stage separation (the booster's nitrogen thrusters enabled), Second stage ignition, SECO, Dragon separation (the Dracos enabled); functions: the booster's three- and one-engine sets and shutdown, grid fins, legs (pinned), parachutes, nosecone, launch abort (thrust termination: every Merlin and the MVac shut down, then Dragon separates on its SuperDracos; marked as an abort, so offered only while it would save the crew, D120). Status: **playable** — the game's default craft: it builds, stands clamped on LC-39A and flies its sequence in the game (the demo: ignition, liftoff, ascent, MECO, separation, the upper stage, the booster) with its systems live (`docs/technical/ship-systems.md`); its gaps are the fidelity review's (`docs/archive/research/fidelity-review-2026-10-29.md`).

### Tests
Generators against hand calculations (`catalog::tests`); configurations (`craft::configure`: ids, a leg's foot after 120°); the Falcon 9 (`craft::falcon9_tests`: totals against falcon9.md, reported gaps, legs deploying to an 17.7 m stance 2.5 m below the engines, the same bits twice; `vessel::ship_tests`: clamped on the pad, lift-off, stage separation with momentum exact and functions partitioned, saves); placement, patterns, components, layers, every validation error by name, mass properties of a draining two-compartment craft against a hand sum, four engines with one off (thrust, moment, draws by mixture ratio, throttle range, empty tanks, debug mode), the partition of definition and state, determinism (`craft::build_tests`); a craft of parts lifting off its pad on its sequence's ignition, a separation conserving mass, centre of mass and momentum with the state partitioned, a save round trip and 60 fps frames equal to one jump (`vessel::ship_tests`).

### Not done (the foundation's limits)
Built since (RUNTIME, SYSTEMS2; [ship-commands.md](ship-commands.md) and the subsystem docs it lists): engines start and tail off (a closed throttle shuts them down, D114), every joint's load is checked against its capacity each live tick (`sim::systems::structure`), mechanisms move at their actuators' rates, lattice fins have their own normal force (`sim::aero::lattice`). What the systems still lump or miss is the fidelity review's table ([fidelity-review-2026-10-29.md](../archive/research/fidelity-review-2026-10-29.md) §1). Left at the part layer:
- A separation's impulse is instantaneous (the pushers' stroke is not flown).
- A mechanism's new shape at its deployed angle is a configuration of its own: a new design (built off the frame, ~0.3–0.5 s for the Falcon 9 in a release build), applied as a one-piece breakup.
- Engine heat goes to the nozzles by the design's static shares (an engine out still heats its nozzle).
- Parts' load limits are not set (`max_load` infinite): only the joints break under load.
- Lattice fins' axial drag is still the bake's Newtonian plate (~1.5–2× their measured transonic peak, E).
- Boxes place only at quarter turns (fins and grid fins on the axes); legs are round struts.
- Crew Dragon's capsule and trunk are one piece (no separation joint between them).

## Design

### 1. The part layer (`sim::craft::parts`)

A craft is a set of **parts** in an attachment tree. A `Part` owns primitives of the craft's geometry (each primitive belongs to exactly one part) and carries placeholders: dry mass, a tank (capacity and the cylinder it fills), an engine (`EngineFile`), RCS thrusters (`RcsFile`), the parachutes (a flag: the craft's `ChuteSpec`), the interior's specific heat and node limit, power (produced in full sunlight at 1 AU, used, stored), a **control** flag (a crew cabin or probe core), crew seats and a **load limit**. A part keeps its `id` (its index in the craft it was designed in), so a piece of a craft names its parts in the original.

- **Mass**: each part's dry mass is a thin shell over its *own* primitives' union surface (weighted by skin areal mass), so its mass properties do not depend on what it is attached to; the craft sums them (parallel-axis theorem). `MassModel` is the dry craft; the propellant's mass properties are each tank's slug where it sits and each bottle's gas (`MassModel::with_slugs`, [ship-model.md](ship-model.md) §5.1).
- **Aggregates**: dry mass, propellant, crew (`parts::aggregate`), the engine of all engine parts firing together (`combine_engines`: summed thrust, the Isp of the summed flow, the thrust-weighted mount, the summed nozzle heat), the RCS couples summed. A piece without an engine has a zero-thrust stand-in that never runs.
- **Design** (`CraftDesign`): every cell and interior node is tagged with its part (`cell_part`, `node_part`); each part's interior heat capacity is spread over the volume its primitives fill (a part too thin for the grid's samples, a fin or a panel, puts it in the nodes under its cells); each node's limit is its part's; each engine part's own bell is its nozzle, taking its engine's share of the wall heat (D077).
- **Where a craft comes from** (`sim::craft::registry`): a `CraftId` resolves to a definition: registered at run time (`register`, for craft made in the game later), `stress-<n>`, or a `data/craft` directory; a **piece** is `base#ids` (ranges of part ids, e.g. `stress-300#12-16,32`). `craft_of` builds each id once per process, so a vessel's design is rebuilt after a load from its id alone (scale baseline problem 3).
- **Cells by area** (scale baseline problem 1): every craft gets at least its area over 0.3 m² in cells (the test craft's 512 on 136 m² are 0.26 m² each); a craft of parts at least 4 per part. **Thin tubes** (problem 2) are split across their circumference into half-tubes, first, so no cell's normals cancel (the test craft's legs included: its cells changed, the playtest save is to be regenerated).

### 2. Splits (`sim::vessel::split`)

A set of parts leaves as a new vessel, a full one with its own trajectory, aerodynamics and heating.

- **Plans**: `Vessel::plan_split(parts)` (separate exactly those parts), `plan_detach(parts)` (cut them from their parents: each leaves with what hangs from it, a decoupler) and `plan_breakup()` (the pending breakup) give a `SplitPlan`: the connected groups after the cuts (`parts::components`), each destroyed part a wreck of its own. The vessel keeps the group with its lowest control part, else the heaviest; with no intact group left it keeps the first wreck and is destroyed.
- **Building the pieces** (`SplitPlan::crafts`: each piece's craft through the registry) is the expensive step (§ Measured); a caller may run it off its frame thread. `Vessel::split(world, plan, crafts, ids)` then applies the split at the vessel's time.
- **Motion**: every piece keeps the body axes, the attitude and the angular velocity ω; its centre of mass is where it was in the craft, moving at v + ω × r (r from the craft's centre of mass). Linear and angular momentum are conserved, since the pieces' mass properties sum to the craft's.
- **State carried**: propellant by tank (each piece's tanks at the craft's fill), skin and node temperatures (each new cell or node takes the nearest one of its own part), the ship clock, the systems (panel, battery and its charge, loads in proportion to the parts' power placeholders; boil-off by tank capacity), debug mode, the limbo placeholders. The parachutes go with their part (a piece without them has none to open), the crew with the seats (the vessel's piece first). The vessel keeps its flight plan and attitude control; a new piece has neither. Every field of the vessel has its own rule, named one by one (`Vessel::piece`: an exhaustive destructuring, no copy of the whole, so a new field fails to compile until it has one; test `vessel/split_tests.rs`), and the ship state item by item with its parts (`craft::partition`).
- **Phases**: live stays live; a coast restarts as coasts at the split epoch (the stored trajectory stays the truth: the split epoch is deterministic, so chunked equals single pass); a landed or crashed vessel is woken into live ticks.

### 3. Breakage (`sim::vessel::breakage`)

Judged at the end of every live tick, with placeholder limits; **debug mode (D064) disables all of it**.

- **Temperature**: a part with a skin cell over its material's limit or a node over its part's limit is **destroyed** (D065, "the part is destroyed cleanly"): it leaves as a wreck, and what was attached through it comes apart into its connected groups. *Decided here:* destroyed and detached as a wreck (its mass and momentum stay in the world), not removed, so the debris rule decides when it goes.
- **Impact** (D066, per contact point): the part of the point that hit too fast is destroyed; the rest of the craft keeps its engine and controls.
- **Load**: a part whose cells carry more than its `max_load` is **torn off** its parent, intact, with what hangs from it (a root part, the craft's or a piece's, has no parent and stays). Its load is the flow's Newtonian pressure on its exposed cells, q·Cp,max·e·A·cos²θ summed as a vector (the most the flow can push on them; exposure e from the aero bake), plus the ground's normal force at its contact points (`contact::normal_loads`, the contact's own spring-dampers as the substeps apply them).
- **One part** (a craft of a single part) keeps the whole-craft rule: over a limit or hit too fast, the vessel is destroyed and flies on as a wreck (D065, D066 as before).
- The vessel then **stops at that tick** (`Vessel::breakup`; `advance` does nothing) until it is split, so the result does not depend on when the caller applies it. Breakups are applied **earliest first** (ties in fleet order), so the fleet's order and the vessels' ids do not depend on the frames either.

### 4. Debris (`sim::vessel::debris`, D083)

`deletable(controllable, dead, nearest)` is the rule: delete a vessel iff it is **not controllable** (intact, with a control part and power), **dead** (destroyed, or no power: no part that produces or stores it, or the battery flat with no panel), and the nearest controllable craft is **10 km or more** away (or there is none). `debris(world, t, fleet, active)` applies it to a fleet (separations through the frame tree, rule 3; never the active vessel). Deleting a vessel changes nothing for the others (D025), so the frame that deletes it does not matter.

### 5. The game (`game::debris`, after `state::advance`)

A broken-up vessel holds the clock (it stops, like any vessel behind); its pieces' craft are built on the async compute pool; the split is applied and the pieces join the fleet, with a toast. Every frame the debris rule's vessels are deleted (through `tracking::delete_vessel`), with a toast.

## Tests (the specification)

`crates/sim/tests/parts.rs`, and unit tests next to the code:

- Conservation: a 40-part stress craft tumbling in a coast and burning live loses a booster: mass, centre of mass (1 µm), momentum (1e-13) and angular momentum (1e-9) conserved; fills at the craft's; coasts restarting at the split epoch; each part's hottest cell as hot as before (the booster's bell after a burn).
- Determinism: an entry that overheats parts of a 12-part craft, flown in 60 fps frames and in one jump, gives the same fleet (9 vessels) bit for bit; a coast split reached in 1× frames and flown on in 1,000,000× frames equals one jump each.
- Triggers: debug mode breaks nothing; 400 m/s at 10 km tears off the panels and a booster tank (with its nose and engine); a 10 m/s drop breaks a leg, not the craft.
- Saves: a split fleet of stress pieces saved and loaded continues bit for bit; a stress craft and a piece rebuilt from their ids give the same design bits.
- Debris: the rule's table; over a fleet, a fin-and-leg piece is kept near its craft and deleted 50 km from another controllable craft; never the active vessel.
- Part layer: multi-tank mass against the slugs summed; two tanks sharing propellant by capacity; each part's interior heat capacity conserved in the nodes; every cell and node tagged; ids round trip.

## Measured

Release builds on the reference Mac (D028), other agents compiling alongside (load averages noted; read every number as ±20 % or worse).

**Splitting a 300-part stress craft** (`examples/bench_parts`, load 7–16): planning a split takes microseconds; **the split itself, once the pieces' craft exist, 1.5–2.1 ms** (under a frame). The cost is building the pieces' craft and designs (`SplitPlan::crafts`, one thread per piece; built once per id and process):

| Split | Pieces (parts) | Large piece: build + design (cells, nodes) | Small piece: build + design |
|---|---|---|---|
| Upper stack at the first decoupler | 237 + 63 | 204 + 304 ms (3,970, 1,083) | 27 + 56 ms (579 cells) |
| A booster | 244 + 56 | 208 + 466 ms (3,653, 1,025) | 140 + 85 ms (851 cells) |
| A panel | 299 + 1 | 564 + 322 ms (4,499, 1,242) | 0 + 14 ms (4 cells, 1 node) |
| A fin and its leg | 298 + 2 | 249 + 728 ms (4,476, 1,239) | 2 + 19 ms (29 cells, 3 nodes) |

So a 300-part craft splits into two in **0.5–1 s of work** (the remainder's new design), off the frame thread in the game; **a debris piece of 1–2 parts costs 15–20 ms**. Where a large piece goes (`bench_parts stages=stress-300`): union surface 17 ms, cells ~200 ms (the hull-contact search, the edge map, the joint search), the parts' shells 11–25 ms, the aero bake ~240 ms of a ~320 ms design.

**The craft pipeline at 300 parts** (`bench_scale craft parts=300 runs=1`, before at load 7–12 / after at load 4–16):

| | Before (BENCH's baseline) | After |
|---|---|---|
| Union surface | 158 ms | 17 ms (broad phase) |
| Volume grid | 199 ms | 7 ms (broad phase) |
| Build (union, cells, mass) | 370 ms | 229 ms |
| Design | 2,054 ms | 305 ms (mostly AERO's bake, 1,787 → 236 ms) |
| Cells with the default options | 858, 88.6 % of the area in tubes | 4,503, 1.6 % (cells by area) |
| Shell centre of mass vs the parts' volume centroids | 0.67 m | 0.01 m (per-part shells) |


## Not done / limits

- No craft editor: a craft of parts is a file (D102). A craft of catalog parts has buses, joint loads and each tank's own contents (`sim::systems`, `MassModel::with_contents`).
- Overheating is judged in live ticks only, not on the coast thermal lattice (a coast never heats a part past its limit today: sunlight and thin air are below every limit).
- Designs of large pieces cost seconds (the remainder of a 300-part craft is a new design); built crafts are kept for the process (no eviction).
- A breakup applies one impact per tick (the first point); further hard points break on following ticks.
