# Ship model: parts, components, layers, systems, failures, information

> Status: **design** (2026-10-26, lead, from the owner's interview [interview-2026-10-26-parts.md](../archive/interviews/interview-2026-10-26-parts.md)). Decisions D100–D108. The first vehicle is the Falcon 9 Block 5 with Crew Dragon (data: [falcon9.md](../vehicles/falcon9.md)); the build plan is [overnight-2026-10-27-falcon9.md](../archive/plans/overnight-2026-10-27-falcon9.md). It builds on the part skeleton of [parts.md](parts.md) (attachment tree, splits, debris, breakage) and the cell model of [aero-thermal.md](aero-thermal.md).

The target is a ship model beyond KSP + RSS + Realism Overhaul + RP-1: every ship is engineered from real parts, its internal systems behave as the real ones do, abstracted to behaviour at runtime but "not too abstracted", and anything that would make a real part fail, and that we simulate with data behind it, makes ours fail.

## 1. Prior work

| Source | Mechanism | Taken | Rejected |
|---|---|---|---|
| KSP | Attach nodes, stack and radial symmetry, a stage list, action groups | Mounting points (D102), symmetry patterns, stages and saved functions (D107) | Physics joints that wobble (D103); "touching is attached" |
| Procedural Parts, ROTanks (RO) | Tanks sized freely, mass per volume by tank type | Parameters → derived mass (D101) | Mass per volume as a flat constant: we derive it from material, pressure and loads |
| RealFuels (RO) | Ullage (propellant stability from acceleration; an unstable start fails), limited ignitions, mixture ratios, residuals, boil-off, pressure-fed limits | All of these as behaviour (§5.2, §5.3) | Its stability as a percentage gauge: we track settling from the acceleration history |
| TestFlight (RO) | Per-engine reliability curves, rated burn time beyond which the failure rate climbs, failure types (shutdown, thrust loss, gimbal lock, ignition failure, explosion), "data units" raising reliability | Rated burn time and rate curves, the failure types and their different outcomes (§6) | Reliability growing from flight data in the part itself: the part's true rates are fixed by its design and quality; the organisation's knowledge is what grows (D105) |
| Kerbalism | Components with MTBF, quality levels (a better part costs more), malfunctions, life support, radiation | Ratings and quality (owner's "a more expensive valve is more reliable"), life support (§5.8) | Malfunctions on a timer unrelated to conditions |
| FAR | Structural failure from aerodynamic loads | Failure from the loads the joints carry (§5.6) | — |
| Children of a Dead Earth | Materials database with real properties; parts derived from materials and physics | Materials first (§3.1), derived properties | — |
| Real engineering | Equivalent axial line load for shells (N = P/2πR + M/πR²), Miner's rule for fatigue, Arrhenius/Larson–Miller time at temperature, Kingery–Bulmash blast scaling, NASA-HDBK-7005 and GEVS vibration qualification, NASA-STD-3001 crew limits, Abramson (NASA SP-106) slosh | The runtime models (§5, §6) | Finite elements, CFD and flow inside engines at runtime (D020 makes maps offline) |

Agents building a part of this look further and record what they took in the feature notes.

## 2. Concepts

- **Part**: a physical object with a shape, a place and mounting points: a tank, an engine, the octaweb, a grid fin, a leg, the interstage, the capsule's pressure vessel, the heat shield, the trunk. It owns skin cells and interior volume nodes (D065), so aero, heating and contact act on it.
- **Component**: equipment inside a part's volume, with mass, volume, power, heat and functions but no outer shape: a helium bottle (COPV), a battery, a flight computer, a sensor, a valve cluster, a seat, an ECLSS unit, a cold-gas thruster block mounted in a part's wall. Its heat goes into the node that holds it.
- **Layer**: a network across parts and components, not placed as a part: **structure** (joints from mounting points), **plumbing** (tanks, bottles, lines, valves, regulators, vents, engine inlets), **electrical** (buses, sources, loads), **data** (sensors, computers, actuators, telemetry), **thermal** (the existing cells and nodes; coolant loops later), **mechanisms** (hinges, actuators, springs, dampers).
- **Generator**: Rust code that turns parameters into a part or component: its primitives, mounting points, mass properties, tanks and ports, functions, ratings and failure modes. Everything comes from a generator (D101).
- **Catalog entry**: a generator, its parameters, the published numbers that override derived ones (each with a source), ratings, era, cost and lead time. Real hardware ships as entries; a player's part is an entry of their own (D101).
- **Craft file**: parts placed by mounting points (or coordinates), components placed in parts, layer connections, the stage sequence and saved functions (D102).
- **Ship state**: everything that changes in flight, saved with the vessel: tanks, bottles, engines, valves, batteries, mechanisms, joints, part integrity and damage, crew and cabin, failure fates, history.

## 3. Data

### 3.1 Materials (`data/materials.ron`, `sim::materials`)

Each material: density; Young's modulus; yield and ultimate strength as tables against temperature; thermal conductivity, specific heat, emissivity and solar absorptivity; maximum service temperature; destruction temperature (melting, decomposition, or charring through for ablators); ablation data where it ablates (PICA-X: effective heat of ablation or a B′ table, char and virgin densities); fatigue exponent; cost per kg; era. Sourced (MMPDS or the manufacturers' data sheets, NASA TPSX for thermal protection). The first set covers the Falcon 9 and Dragon: Al-Li 2195/2198, Al 2219, Al 7075, Ti-6Al-4V, Inconel 718, a copper alloy for chambers, niobium C-103, stainless steel, carbon-fibre composite, aluminium honeycomb, Kevlar, PICA-X, cork or silicone ablator (backshell), glass.

### 3.2 Fluids (`data/fluids.ron`, `sim::fluids`)

Liquids: density and vapour pressure against temperature, specific heat, boiling point; LOX, RP-1, MMH, NTO, LH2 and LCH4 later, water. Gases: molar mass, γ, specific heat; helium, nitrogen, oxygen, CO₂, water vapour. Sourced (NIST REFPROP/Webbook values, CPIA for RP-1).

### 3.3 Catalog (`data/parts/<family>/<entry>.ron`, `sim::catalog`)

```ron
(
    id: "merlin-1d",  name: "Merlin 1D (Block 5)",  generator: Engine,
    params: ( cycle: GasGenerator, propellants: (oxidizer: "LOX", fuel: "RP-1"), mixture_ratio: 2.36,
              exit_diameter: 0.92, length: 2.9, gimbal_deg: 5.0, ignition: TeaTeb(cartridges: 3), … ),
    published: ( thrust_sl: (854.0e3, "SpaceX Falcon User's Guide 2021"), isp_sl: (282.0, "…"), … ),
    ratings: ( …see §6.1… ),
    era: 2018, cost: Some((1.0e6, "estimate, …")),
)
```

The numbers above illustrate the format; [falcon9.md](../vehicles/falcon9.md) holds the sourced values. A generator states which of its outputs a `published` value may override (thrust, Isp, battery capacity: what is measured on the real thing); everything else is derived. A part that contradicts its own data (a published dry mass a generator cannot reach from its materials) is reported by the build, not silently patched: the gap shows as the part's "unmodelled mass" with its reason (D101).

### 3.4 Craft files (`data/craft/<craft>/craft.ron`, `sim::craft`)

```ron
(
    name: "Falcon 9 Block 5 · Crew Dragon",
    parts: [
        (id: "s1.octaweb", entry: "f9-octaweb"),
        (id: "s1.e1", entry: "merlin-1d", mount: (at: "e1", on: "s1.octaweb", via: "top")),
        // patterns place copies on numbered mounts
        (id: "s1.leg", entry: "f9-leg", pattern: Radial(count: 4, start_deg: 45.0), mount: (on: "s1.octaweb", at: "leg_#")),
        …
    ],
    components: [ (id: "s1.copv1", entry: "f9-copv", in: "s1.lox", pos: (…)), … ],
    plumbing: [ (from: "s1.lox.outlet", to: "s1.e1.ox_in", line: (diameter: 0.2, length: 4.0)), … ],
    electrical: [ … ], data: [ … ],
    joints: [ (between: ("s1.interstage", "s2.lox"), kind: Separation(pushers: …)), … ],   // defaults from the mounts
    sequence: [ (name: "Ignition", actions: [ … ]), … ],
    functions: [ (name: "Landing engines", actions: [ … ]), … ],
)
```

- **Mounting points** (D102) are defined by the generator: name, position, outward normal, a roll reference, a size and a kind (stack, radial, engine, surface, component). A part attaches only through a mounting point to a compatible one; nothing is attached because it touches. Coordinates remain valid (D036) for components and free placement.
- **Patterns** (radial, linear, explicit list) place copies with ids `name#k`.
- **Joints** default from the mount pair's kinds (an integral weld inside a stage, a bolted flange between parts, the declared separation systems); a joint has a capacity (§5.6).
- **Shapes from a modelling tool** (D102): a part's shape may be a glTF (`.glb`) mesh exported from Blender, with mounting points as named empties and part data in custom properties; the physics uses the same mesh (cells, contact, heat). Procedural primitives and surfaces of revolution (the capsule) cover the Falcon 9 tonight; the importer follows.

## 4. Build pipeline (`sim::craft`)

Catalog + craft file → generate every part and component → place them by their mounts → the existing pipeline: union surface, cells by area, mass properties, interior volume nodes, the aero bake, the thermal network (`CraftDesign`), with every cell and node tagged by part (exists) → the layer graphs (plumbing, electrical, data, mechanisms, joints) → validation (unconnected ports, an engine without a feed, an over-full part volume, a part a generator could not reach) → a `CraftDef` with its `ShipState` at the start (tanks loaded at their loading temperatures). Built once per craft id and process (the registry, exists); pieces after a split are `base#ids` (exists).

At a split the ship state goes item by item with its owner (`craft::partition`, F1 of the [fidelity review](../archive/research/fidelity-review-2026-10-29.md)): an item of a part with its part, a joint with its two parts (one the split cuts kept with its child's piece until a docking joins them again), the cabin and its crew with their part, and each field of the vessel as a whole by a rule named for it (the clamps with the piece holding the hold-down joint, the pilot's orders with the vessel's piece, the event log copied to each). The vessel's own fields go the same way (`Vessel::piece`). Both are exhaustive destructurings with no copy of the whole, so a new field does not compile until it has its rule. The program (the stages, the functions and the sequence's position) is the vessel's as a whole: every piece keeps a copy of all of it, and reads it through its view (`craft::program`: the stages and commands that name something the piece has; Space skips the other pieces' stages, AGAIN and the sequence's edits refuse them, the estimates, the staging bar, the action bar and the systems screen's editors show the view and edit by the program's indices). The inverse, `ShipState::merge` and `Runtime::merge` (a docking), gives the state back bit for bit, the program included.

## 5. Runtime

### 5.0 Tick order (live, 20 ms; contact substeps as now)

Commands (player, functions, sequence; later scripts, with light delay for uncrewed craft) → mechanisms → plumbing (pressures, flows, settling) → propulsion (each engine's state and output) → forces and moments (each engine at its mount and gimbal, each RCS thruster, aero on cells and on special surfaces like grid fins, contact, chutes) → the rigid body (exists) → structure loads per joint → heat (exists; engines, components and propellant as sources and sinks) → damage and failure checks → outcomes (splits, explosions, a part's function lost) → power → sensors and knowledge.

### 5.1 Mass properties

The dry parts are fixed per design. The ship state is the only truth of a craft of parts' propellant (F2 of the [fidelity review](../archive/research/fidelity-review-2026-10-29.md), `sim::vessel::mass`): its total, its mass and mass properties are derived from each tank's and bottle's own contents, recomputed each live tick (tens of tanks: microseconds). Each liquid is a solid cylinder where the plumbing has its slug (§5.2's settling: its height from its density, its drift from the outlet; `plumbing::slug`), so a booster's flip throws its LOX to the far dome and moves its centre of mass by metres; each ullage's gas fills the length the liquid leaves; a bottle's gas is a point. The thermal nodes take each liquid's heat capacity where its slug sits (§5.7). The slosh pendulum (§5.2) will move a share of a slug across the axis; until then the liquids turn with their tanks in roll (their spin-up through the boundary layers needs the liquid's angular momentum as state). On a coast the tanks and bottles follow one law of time (D119, §5.10), which both the segment's perturbations and the ship state on the systems lattice read: the segment's mass is the ship's at every lattice point, to the bit.

### 5.2 Plumbing (lumped network)

- **Tank**: volume; liquid mass and temperature (density from the fluid table); ullage volume = tank volume − liquid volume; ullage gas (pressurant and vapour) moles and temperature; pressure from the ideal gas law. Heat from the wall's nodes warms the liquid (densified LOX warming on the pad); at the saturation temperature at the ullage pressure it boils off through the vent (a law of time while coasting, §5.10).
- **Bottle (COPV)**: pressurant moles, temperature (cold helium submerged in LOX on Falcon 9), pressure; polytropic expansion as it empties.
- **Regulator/pressurization**: keeps each tank's ullage at its setpoint while the bottle can (Falcon 9 heats its helium through the engines before it enters the tanks).
- **Feed**: an engine's demand pulls liquid through lines with losses ∝ ṁ²; inlet pressure = ullage pressure + ρ·a·h (the head from the proper acceleration along the tank's outlet axis) − line loss; **NPSP** = inlet pressure − vapour pressure at the liquid's temperature.
- **Settling (ullage)**: a tank's liquid is *settled* at its outlet when the proper acceleration has pointed towards the outlet above a Bond-number threshold for at least its settling time (∝ √(2L/a)); otherwise it may be floating, and an engine drawing from it ingests gas (§6). Cold-gas or RCS thrust settles it, as on the real stages.
- **Slosh**: a pendulum per large tank (mass fraction, frequency and damping from the tank's shape and fill, Abramson) adding its force to the rigid body: the "limited give" D103 allows, because it is the fluid that moves, not the structure.
- **Valves**: open, closed or stuck; each has a rated cycle count and ratings (§6).

### 5.3 Propulsion

- **Engines** (published data first, D104): a state machine per engine instance: *off* → *starting* (needs an ignition left, power, settled propellant and NPSP above the engine's requirement; the igniter is consumed: TEA-TEB cartridges limit restarts; thrust and flows ramp along the start transient) → *running* (thrust, Isp and mixture ratio as functions of throttle within its range, inlet conditions and ambient pressure; flows drawn from the tanks it is plumbed to) → *shutdown* (tail-off impulse) → *off*, or *failed* in one of its failure modes. Gimbal per engine with its range and actuator rate. Heat per D077, per engine. The engine designer (D020) later produces the same maps.
- **Engine groups**: a stage's engines are commanded as a group or one by one (Falcon 9's booster uses 9, 3 or 1); an engine that fails is shut down by its controller when the failure is benign (engine-out capability), or fails uncontained (§6).
- **RCS**: thrusters are components with a position and direction; the torque and force they give come from the geometry, not declared couples (replacing D078's couples for craft of parts; the test craft keeps its couples). Allocation: SAS and the pilot ask for a torque; the thrusters whose torque points along it fire, with pulse widths as fractions of the tick. Cold gas from bottles (Falcon 9's nitrogen), bipropellant from the plumbing (Dragon's Dracos).

### 5.4 Aero surfaces and mechanisms

- **Grid fins** are lattice fins, not panels: their cells are left out of the Newtonian bake and replaced by a lattice-fin model (normal force against Mach and deflection with the transonic choking dip; sourced), on a hydraulic actuator with its range and rate, stowed or deployed.
- **Mechanisms** (D103): hinges and actuators move parts kinematically (legs deploying on helium pneumatics, grid fins unfolding, Dragon's nosecone opening); springs and dampers bounce (the legs' struts, with single-use crush cores absorbing the landing). Cables and chute risers bend. The structure itself never does.
- **Hold-down clamps** belong to the pad: they hold the vessel until released, and release only on command (the sequence releases them once every engine is healthy, as on the real pad).

### 5.5 Power

Power flow in watts over buses (D104): sources (batteries with charge, temperature and internal resistance; solar cells on skin cells, lit by sunlight and later the planet's albedo, with their temperature; ground power through the umbilical on the pad), loads (every component's draw in its state), priorities and shedding. A component without power stops (its function is lost until power returns); bus voltage is derived for display.

### 5.6 Structure

Rigid until it breaks (D103). Each live tick every joint's load is computed exactly for the rigid body: the subtree beyond the joint must be accelerated by it, so the force is Σᵢ mᵢ·aᵢ − F_ext over that subtree (aᵢ the proper acceleration at each part's centre of mass: a + α×rᵢ + ω×(ω×rᵢ); F_ext the thrust, aero, contact and chute forces on the subtree's parts), and the moment likewise about the joint. Capacity: axial (tension; compression with shell buckling, NASA SP-8007 knockdown, raised by internal pressure), shear and bending, from the joint's kind, its parts' materials and their temperatures. The check uses the equivalent axial line load N = P/(2πR) + M/(πR²) against the allowable at temperature. A joint over its ultimate load snaps (§6); repeated loads are not tracked for fatigue unless data supports it.

### 5.7 Heat

The existing network (D065), now per part and material: each part's cells take its material's properties; components put their dissipation into their nodes; tanks' liquids are heat sinks in contact with the wall's nodes; each engine heats its own nozzle (D077). Thermal protection ablates (PICA-X) when its data is in: recession and mass loss from the heat load.

### 5.8 Crew and life support

Dragon's cabin as a lumped volume: pressure, O₂, N₂, CO₂ and water vapour partial pressures, temperature (its node); tanks of O₂ and N₂; CO₂ removal; consumption per crew member (NASA BVAD); crew limits on acceleration by axis and duration and on landing loads (NASA-STD-3001). Radiation dose later. On the overnight list this is cut first if time runs short (owner's default). One cabin per ship state for now (a merge refuses two); a docking needs a list of cabins, pressurised volumes joined by hatches, each with its own air and crew (docking's first step, [brief-foundation.md](../archive/briefs/brief-foundation.md)).

### 5.9 Vibration

Not simulated as structural dynamics (D103). Each part receives a vibration *environment* (grms and its spectrum's shape class) from its sources: running engines (at their mounts, attenuated across each joint by a transmissibility), aerodynamic buffeting and acoustic loads (from q and Mach, peaking transonic), and pyroshock at separations (by distance). Each part's qualification level (NASA-HDBK-7005, GEVS: typically maximum expected flight level plus 3–6 dB for a multiple of the flight duration) bounds it; damage accumulates by Miner's rule, D += (G/G_qual)^b·dt/t_qual, and a part fails (§6) only when D ≥ 1.

### 5.10 Coasts, limbo and planned burns

On rails the systems step on the 60 s lattice (`sim::systems`, D081): power, batteries, life support, the liquids floating, hazard accumulation for components that run. The tanks and bottles do not step: through a coast they follow a law of time (D119, `sim::systems::coast_law`), coarse-grained and analytic. It runs in spans of at least an hour, each starting at one of the segment's samples (so chunked = single pass), with its conditions held: each tank's heat from the coast's orbit-averaged sunlight at the attitude's turn (as the coast temperatures take it) and its liquid's height. A span starts from equilibrium (the gas at its liquid's temperature, the regulator's top-up, relief). Then each tank, sealed, warms or cools in closed form (m·c·dT/dt = S − K·T⁴), vents its pressurant from the temperature where it reaches its relief pressure, and at saturation boils off L·dm/dt = −(a + b·m) (exponential in time). The coast segment's mass is the dry mass plus that law's consumables, and the ship state at each lattice point is the law's there: one truth, bit for bit. Against the lattice it replaced, in the same sunlight: the boil-off within 10⁻⁴, the warming within 0.01 K. The coarse grain is the eclipse swing (averaged over the orbit), the conditions held for a span, the regulator at span starts, and the vapour the lattice vented with the helium (left to the boil-off at saturation: the heat taken in agrees within 1 %). The tanks are brought to the law at the vessel's time before a command, a split, a restart or going live; code that writes a coasting vessel's tanks restarts its coast (`set_tank`), or the law overwrites the write at the next lattice point. Planned burns for craft of parts use the engines' steady state (thrust and flows summed over the burning engines, drawn from their tanks as continuous laws); transients and failures during a rails burn are either modelled exactly on the lattice or the burn is refused with its reason.

## 6. Failures (D105)

### 6.1 Ratings, shown to the player

Every part and component has ratings that make comparison easy: a **reliability score** (1–10, a log scale of its rated life), **rated life** (operating hours, cycles or starts at rated conditions to a 50 % chance of a defect), **rated temperature** (service) and **destruction temperature**, **time at temperature** (how long it holds above its rated temperature), **rated load** and **ultimate load**, **burst pressure**, **qualification vibration level**, **rated burn time** (engines). Quality is a parameter: a better valve costs more and rates higher.

### 6.2 Triggers

- **Physical, deterministic:** a joint over its ultimate load snaps; a tank over its burst pressure bursts; a part at its destruction temperature is destroyed at once; between its rated and destruction temperatures it accumulates **thermal damage** (time at temperature against an Arrhenius/Larson–Miller allowance) and fails when the damage reaches one ("4000 K for complete failure; below it, integrated heat"); an engine drawing unsettled propellant ingests gas; an engine below its NPSP cavitates; a hard start (an ignition with an off-nominal inlet state) damages the engine; landing overloads break legs (exists per contact point).
- **Accumulated damage:** vibration (§5.9), thermal (above), cycles (valves, starts).
- **Statistical, only where appropriate:** for what we do not simulate inside a part (turbomachinery, valve internals, electronics): a hazard rate λ(conditions) (per hour of operation, per start or per cycle; rising beyond the rated life or burn time, and with damage), from the part's own data where it exists, else its class's historical rate, always the **low end** of the rate's confidence range: if the simulator is not sure, nothing fails.

### 6.3 Determinism: fates

Each statistical mode of each part draws once a **fate** E = −ln(u), u from a counter-based hash of (the save's seed, the vessel's launch epoch, the part's id, the mode). The mode accumulates its hazard H += λ(conditions)·dt on the tick and lattice; it fails when H ≥ E. So the same ascent from the same launch time gives the same outcome (no re-roll on reload), a different ascent accumulates a different H and may differ, and warp changes nothing (rule 4). Fates and hazards are saved; a split keeps them with the parts.

### 6.4 Modes and outcomes

Each failure mode of each kind has its outcome: *snap* (the joint breaks: a split, exists), *melt* or *burn-through* (the part is destroyed: its function and contents lost, its mass stays as a wreck), *burst* (the tank's contents are released: an explosion if it holds energetic propellants or high-pressure gas), *uncontained engine failure* (an explosion at the engine), *benign engine shutdown* (the controller stops it: thrust lost, engine-out), *ignition failure*, *gimbal stuck*, *valve stuck open or closed*, *leak*, *battery cell loss*, *computer or sensor lost* (redundancy decides). Debug mode (D064) turns every failure off and gives perfect information.

### 6.5 Explosions and cascades

An explosion's energy comes from what was released: stored gas (pressure–volume energy) and propellant mixing (yields by propellant pair, from the published test programs). Its blast reaches neighbouring parts by distance (Kingery–Bulmash scaled distance Z = R/W^⅓) and damages or destroys them by overpressure against their ratings; fragments later. One failure can cascade (CRS-7's strut, AMOS-6's COPV). No fire or smoke.

### 6.6 History

Each part keeps its history across flights: run time, starts, cycles, peak and integrated heat, peak loads, damage, landings. It moves its hazards (a part beyond its rated life fails sooner); inspection and refurbishment come with the career. The organisation's knowledge of a design's true reliability is an estimate from its tests and flights (the career; not tonight).

## 7. Information (D106)

The interface never shows something wrong: a quantity the ship cannot measure shows a dash, and every value is shown to the precision its sensors give.

- **Sensors** are components: IMU (attitude, rates, acceleration), GPS receiver (position and velocity where it has signal), star tracker, radar altimeter, tank level sensors, pressure transducers, thermocouples, strain gauges, the engine controllers' own readouts, ground tracking through the comm network (range and range rate, with light delay). Each gives quantities with an accuracy and a rate, and needs power and a data path to a computer.
- **Knowledge**: `sim` derives what the vessel knows now (each quantity's value and precision, or nothing) from its working sensors; the flight view, the systems screen, the MCP server and later scripts read only that. Uncrewed craft's knowledge reaches mission control through the comm network with its delay and occlusion (exists for telemetry).
- **Precision** sets the digits shown (no velocity to 50 decimals in orbit). The errors themselves are simplified: values are shown at their precision, not drawn with noise.
- **Debug mode** gives perfect information instantly.
- **The flight software reads only this** (F3, `sim::systems::flight`): SAS and the holds, the pad's launch commit, the engine controllers' safing, the estimates and the landing prediction take what the sensors know and the computers' health, never the true state; D115's adaptive functions and D117's guidance plug in as its functions.

## 8. Controls and the systems screen (D107, D108)

- **The systems screen** is a separate screen (like the tracking station), not a flight menu. Its main view is the craft alone, switchable by layer: **physical** (parts by material), **structural** (joints coloured by load against capacity), **thermal** (cell and node temperatures), **plumbing** (fluids, pressures, flows, valves), **electrical** (buses, sources, loads, charge), **data** (sensors, computers, links), **health** (ratings, damage, history). Selecting a part or component shows everything about it and its commands (open a valve, arm or fire an engine, deploy a leg, switch a load). Here the player defines **stages** (the sequence: ordered lists of actions) and **functions** (named action lists), and pins functions to the mini-panel.
- **In flight**: the essentials stay as they are (D080); **Space** activates the next stage; a **mini systems panel** holds the pinned functions with their keys; throttle and attitude as now, the attitude through gimbals, RCS and (when deployed) grid fins.
- **Commands** are one `ShipCommand` type in `sim`, applied at a tick, shared by the screens, the MCP server and later scripts (D035, D043), with light delay for uncrewed craft (D067).

## 9. Rules this model keeps

- Rule 1: nothing here uses a reference body (the air, the pad and gravity come through the existing owners).
- Rule 2: every random draw from the counter-based hash, no `HashMap` iteration order, no order-dependent reductions; trig through `sim::math`.
- Rule 4: everything on the tick or lattice from saved state; splits at deterministic epochs; chunked equals single pass and every warp gives the same bits (tested for a craft of parts: a burn with an engine failure, a coast with boil-off, a separation).
- Rule 5 and 8: one owner per model (`sim::materials`, `sim::fluids`, `sim::catalog`, `sim::systems::{plumbing, propulsion, power, structure, vibration, failure, data, crew}`), pure functions with table tests; the game only draws and sends commands.

## 10. Budgets (reference Mac)

A Falcon 9 Crew is about 60–100 parts and 200–300 components. Live tick: the systems step under 0.5 ms per vessel (aero and heat stay as measured: ~1 ms at 300 parts); the build of a craft under 1 s, pieces after a split off the frame thread (exists). Limbo: the 60 s lattice for 50 craft stays within D081's budgets.

## 11. Tests (the specification)

Each model with table tests against its sources: tank pressures against the ideal gas and a published pressurization profile; NPSP and settling rules; an engine's start, steady map and shutdown against its data; RCS torques from geometry; joint loads against a hand calculation for a stack under thrust and a side load; the line-load check against a published shell allowable; thermal damage against its allowance curve; Miner's rule; fates (same launch time → same failure, different history → may differ, warp-invariant); blast overpressure against Kingery–Bulmash; power shedding; the knowledge rule (a missing sensor gives nothing; precision gives digits; debug gives everything). Scenarios: the Falcon 9 on the pad, clamps released only when every engine runs; an ascent to MECO; stage separation (pushers, momentum exact, the booster a controllable vessel); MVac ignition after settling and its failure without it; an engine-out ascent; a booster's landing burn on one engine; the same flight at 1× and in one jump bit for bit.

## 12. Open

- The career's knowledge of reliability (D105), refurbishment and costs in use.
- Radiation (dose, single-event upsets), planet albedo and infrared on vessels (also missing for heat and power: `sim::light`).
- Docking ports (IDSS) and the ISS; water for splashdown (D037).
- The glTF importer and the visual editor (D102).
- Coolant loops and radiators (Dragon's trunk), heat pipes.
