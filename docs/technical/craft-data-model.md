# Craft data model

Status: **design** (nothing is built on it yet). Spec, 2026-11-10, consolidating design session 1 (2026-11-04). Decisions: D130-D136. It turns the session's settled rulings into something an engineer can build from. It does not design anew: anything the session did not settle is marked *Proposal* and listed again in section 16, undecided.

Sources and how they are cited:
- "ruling n": the settled list in [part-schema.md](../design/foundation/part-schema.md) (38 rulings).
- "answer n": the owner's scope interview, [interview-2026-11-02-scope.md](../archive/interviews/interview-2026-11-02-scope.md).
- "GL n": the game-layer interview, [interview-2026-11-05-game-layer.md](../archive/interviews/interview-2026-11-05-game-layer.md).
- "scope 3.1": sections of [scope-2026-11.md](../plans/scope-2026-11.md) (itself a proposal except where an answer settles it).
- "G n": a gap in the [Electron worked example](../design/foundation/electron-worked-example.md), section 5.
- "module map n": sections of [module-map.md](../design/foundation/module-map.md). Its line counts are `wc -l`, measured 2026-11-04.
- The catalog: [type-catalog.md](../design/foundation/research/type-catalog.md) (90 part types, 181 component types), brought into line with rulings 1-21 by the Director.

Today's code is described in [parts.md](parts.md), [ship-model.md](ship-model.md), [joint-loads.md](joint-loads.md), [plumbing.md](plumbing.md), [power.md](power.md), [engines.md](engines.md) and [failure-modes.md](failure-modes.md). Those stay true of the code until the rebuild replaces it (D136).

## 1. The three levels: Type > Design > Unit

Every part and every component exists at three levels (ruling 1, D130).

| Level | What it is | Made by | Fixes | Example: Electron's stage-1 tank | Example: Rutherford LOX e-pump |
|---|---|---|---|---|---|
| **Type** | a class: data that composes a few Rust building blocks (ruling 3) | the game's data, or a mod (answer 56) | the shape builder, parameter ranges as functions of field levels, slots, surfaces rule, failure modes | `tank.cylindrical` | `pump.electric_centrifugal` |
| **Design** | the output of a design program (ruling 1, 36), or bought (ruling 2, 32) | the player (or a supplier) | geometry parameters, rated performance with a statistical spread, the recipe | "Electron S1 tank": 1.2 m, about 8.6 m barrel, bulkhead at about 5.9 m (E) | "RLOXP-1": impeller and motor stack fixed, so a fixed pump map |
| **Unit** | one built object on a craft (ruling 1) | a factory, from the recipe (answer 71) | seeded per-unit variation, wear, state | serial 0042: burst-strength factor, hidden void sites, fill levels | bearing life constant, rotor imbalance, speed, run seconds |

Rules:
1. **Type.** Science nodes gate it; its parameter ranges move with field levels (ruling 1; answer 70). Range limits flatten toward real physical limits and move only through a node that changes the physics (answer 79). The edges of the ranges are the in-universe ranges (scope 3.1, owner 2026-11-03). Going outside them is a mod, with documented changes saved by themselves (answer 56).
2. **New physics or new hand-built geometry is code; a new type is data.** "Hundreds is fine eventually" (ruling 3).
3. **Design.**
   - One design program produces one part, and the components inside it swap freely if they fit (answer 75).
   - The truth of a part is hidden until its design program runs. Then its performance is known to within statistical error (answer 71). Some properties are known on unlocking a node, such as a material's strength (answer 71).
   - A design program gives one unit free and unlocks the recipe; building from the recipe is far cheaper than designing (answer 71).
   - A fix after a failure may need a delta design program (GL12).
   - Designs can be proprietary: a world-known node does not give the player a rival's design (GL16).
4. **Unit.**
   - Strength, flaws and life constants are drawn per unit at manufacture, seeded (answer 74).
   - Hidden flaws are revealed only by inspection (answers 71, 74).
   - A unit carries its wear and its history across flights (answer 74; D105's history rule).
5. **Components have the same three levels** (D130).
   - Simple components (lines, valves, sensors, battery packs, heaters) are sized freely once their node is known (ruling 2). *Proposal:* such a component's "design" is just its parameter set, made with no design program and at no program cost.
   - Complex components (turbopumps, flight computers, radios) are designed or bought (ruling 2).

## 2. Part, component, cell, group

- **Part** (ruling 7; scope 3.1, owner's definition 2026-11-03): the smallest thing you place, with one shape, one rigid body and one exterior. Some are tiny (a radial RCS thruster), some huge (a big tank).
- **Craft:** parts joined at mount surfaces (ruling 9). It is a recursive assembly tree, with the system networks crossing the tree (answer 13).
- **Component** (rulings 7, 14, 18, 37): anything inside a part's volume.
  - It has a real 3D placement (ruling 14).
  - Its momentum is component state that torques the part (ruling 7).
  - Components are one level deep (ruling 37).
  - Some are jettisonable (ruling 18).
- **Cell:** the solver mesh (thermal, aero, structural), finer than a part and not a game object (scope 3.1). Today's `craft::cells` and `mesh` are kept (module map 2.1).
- **Group** (ruling 34): a named set of parts.
  - It is used for staging, scripts, the editor and published mass checks.
  - It can be saved as a reusable sub-design (Dragon on another booster).
  - It has no physics. The word "assembly" for this is retired (scope 3.1).
  - A group can carry a mass calibration target (ruling 38, section 9).
- **Size and function** (rulings 10, 17).
  - One pressure-vessel structure is one part and may hold several fluid volumes.
  - A part class has one function ("cylindrical liquid tank for ground-launched rockets"). Its size has a sanity cap for the class (perhaps 20-30 m diameter) and optional predefined sizes (1.2, 5, 9, 10 m); structure limits the rest.
  - A designable part may still break in flight (ruling 10). Being designable is separate from surviving flight (scope 3.1).
  - Orbital construction unlocks far bigger classes (an orbital spherical tank up to about 200 m).
- **Never-rigid objects** (ruling 8): a deformable-part model family.
  - It gives a reduced, quasi-static shape from loads (canopy inflation, sail billow, boom bending), which feeds aero and the load paths.
  - It is in the schema now and is built when first needed (Dragon's chutes).

## 3. The part test, and the boundary cases

**Test** (ruling 7): a thing is a part if it has its own exterior and mount surface, or if it can separate or move as its own body. Anything inside a part's volume is a component, even if it spins or moves (ruling 7), pressure vessels included (ruling 18).

| Object | Verdict | Basis |
|---|---|---|
| common-bulkhead tank (LOX + RP-1) | one part, two fluid volumes | ruling 17 |
| COPV inside a tank or bay | component (`vessel.copv`) | ruling 18; catalog |
| Curie's tanks inside the Kick Stage | components of the Kick Stage structure | ruling 18 (the worked example made them parts, P15) |
| Electron's stage-2 batteries (two of three dropped mid-burn) | jettisonable components; a dropped one leaves as a lump-of-mass debris object | ruling 18 (the worked example made them parts, P9) |
| avionics boxes (flight computer, PDU, transponder) in a bay | components of the bay | ruling 21; catalog |
| reaction wheel in a bus; turbopump or e-pump in an engine | component; its momentum torques the host | ruling 7 |
| engine | part; its pumps, main valves, gimbal actuators and igniter are components | ruling 16; answer 9; scope 3.1 |
| thruster pod with a shared exterior (Kick Stage RCS, SuperDraco pod) | one part, thrusters its components | ruling 7; catalog; G16 |
| standalone radial thruster | part | ruling 7; scope 3.1 (owner's example) |
| thrusters on a bracket | a group of parts | catalog |
| docking port, legs, panels, deployable boom, robotic arm | parts | ruling 7; catalog |
| gimbal, hinge, slider | not a part: an articulated mount-surface kind; its actuator is a component | ruling 19; catalog |
| separation and capture devices (pyro bolts, clamp bands, latches) | components on the mating surface | rulings 9, 22; catalog |
| launch pad, tower, hold-downs | ground infrastructure with its own procedural geometry, not craft parts | ruling 24; module map 4 |
| deployed parachute, sail, inflatable | a part of the deformable family | ruling 8 |
| Draco thrusters set inside the capsule wall | components of the capsule, by the test | ruling 7 (this spec's reading; module map 4 left it to session 1) |
| TPS tiles, linings, acoustic foam | **open**. *Proposal:* a material and thickness field over the host part's cells, a production parameter | G20; part-schema findings |
| heat shield | **open**: its own part or the capsule's | module map 4 |

## 4. Parameters

Every parameter has a name, a unit, a class and an in-universe range given as a function of field levels. This field list is from the TYPE table, which is a *proposal* (part-schema "What a part TYPE declares"). The classes themselves are settled (ruling 4, D131).

| Class | Fixed when | Changing it means | Examples | Basis |
|---|---|---|---|---|
| **Geometry** | by the design | a redesign: a new design program, with the similarity discount | radius, length, wall thickness, ply count, dome ratio, bulkhead station, impeller diameter, engine pattern | rulings 4, 20, 36; answer 75 |
| **Production** | per build | a unit built differently, with no design program | material grade within its family, which component fills a slot, component placement within allowed regions, routes and waypoints, insulation | rulings 4, 14, 20; answer 75 |
| **Operational** | before launch or in flight | state, not design | fills, setpoints, valve and switch states | ruling 4 |

- **Wall thickness and anything else that alters geometry is geometry** (ruling 20). This corrects the first sketch. It also corrects the worked example's section 3.1, which listed ply count as a production choice (it was written before ruling 20).
- **A material swap** is free only within a family, and only if the design still fits together and behaves the same (ruling 20). *Proposal* for the check an engineer can code: the swap passes if every geometry-dependent check (wall sizing at MEOP, mount interface, slot fit) still passes with the design's fixed geometry.
- **Calibration overrides** (answer 10) are not free numbers: each is the one stated free parameter of section 9 (ruling 21).

## 5. Mounting surfaces, mount kinds and ports

### 5.1 Surfaces
- Every part type defines mounting surfaces, each with a mount kind. Compatible surfaces attach directly, a bolted joint implied, with no extra object (ruling 9).
- A component can change a surface's kind: a side mount added to a tank, or a separation component on a tank's outside that creates a surface which severs when fired (ruling 9).
- A separate part goes in between only for a major mechanism, a large adapter, or where it makes the joint structurally sounder (ruling 9). "At the most fundamental level, the actual decoupling is always done by a component." The owner: "This needs refinement."
- *Proposal* (TYPE table): surfaces are generated from the parameters (position, size, kind, structural rating, which ports each can carry).

### 5.2 Mount kinds (rulings 12, 19, 25)

| Kind | Sized by | Notes |
|---|---|---|
| ring | diameter, bolt-load class | ruling 12 |
| partial ring | diameter, arc | fairing halves (ruling 25) |
| seam | length | a line joint along an edge, with its own release components (ruling 25) |
| radial hard point | rated force | ruling 12 |
| truss node | | ruling 12 |
| docking standard | the standard | ruling 12 |
| payload-adapter standard | the standard (for example a bolt circle) | ruling 12 |
| hinge, gimbal, slider (articulated) | range, stiffness | the actuator is a component on one side, attached to a hard point on the other; D103's mechanisms become these (ruling 19) |

- **Compatible** means the same kind and the same size within tolerance. Anything else needs an adapter part or a surface-changing component (ruling 12).
- **A load class is for compatibility only.** The real rating comes from the computed load path (section 7.5): a pressure-stabilised tank's ring strength follows its ullage pressure (ruling 21).
- *Proposal:* the load class numbers L1 (up to 20 kN/m ring line load), L2 (up to 60 kN/m), L3 (up to 150 kN/m), and point mounts with a rated force (worked example conventions). The size tolerance is not set.

### 5.3 Ports
- **Ports live on mounting surfaces** (ruling 13). Joining two surfaces joins their matching ports. Fluid, power and data cross a part boundary only through ports (ruling 13; answer 16).
- An **umbilical** is a port on a surface that a component severs (ruling 13).
- *Proposal* (TYPE table): port kinds are fluid coupling, power/data connector, umbilical and mechanical. "Matching" means the same kind and medium at the same place on the surface. Today's `Port` has fluid kinds only and a position, not a surface (module map 4).

### 5.4 Separation and live cuts
- **Separation** is release components plus push components on the mount surface (ruling 22).
  - Release: pyro bolts, frangible joints, clamp bands, pneumatic latches.
  - Push: springs, or pneumatic pushers fed from a gas bottle on the fluid network.
- Firing is physical. The stored spring or gas energy sets the separation velocity and tip-off, and a failed release leaves a real hang-up (ruling 22).
- **A port cut while live** behaves physically unless a sealing component closes it (ruling 23).
  - Quick-disconnects and self-sealing couplings close.
  - Anything else vents its line's contents (thrust and mass loss), opens the circuit and loses the data link.
  - Choosing quick-disconnects is the player's job.
- *Proposal* (G23): a separation component declares the ports it severs.

### 5.5 Ground
- The launch pad and tower are built with their own procedural geometry. Every other ground service is abstract, as colonies use the component model without geometry (ruling 24; answer 58).
- **Open** (G7): what a craft's umbilical port joins on the ground side. *Proposal:* a world-side ground-interface pseudo-part, with GSE outside the node system (money and mass only).

## 6. Components inside a part

- **Slots.** *Proposal* (TYPE table): a part type lists its component slots: which component types fit, where (volume, wall, dome, face), how many, and the fit rules.
- **Placement** (ruling 14).
  - Each component has a position and an envelope in the part's internal volume, used for routing, CG, inertia and thermal cells.
  - An auto-placer fills the slots. The player can drag components within the allowed regions; that is a production change, not a redesign.
- **Depth** (ruling 37). Components are one level deep. A complex component's internals are parameters and maps in its design: an e-pump's motor, impeller and bearings swap as one unit.
- **Parametric or designed** (ruling 2). Components can also be bought from suppliers (ruling 32).
- **Jettisonable components** (ruling 18) leave as a lump-of-mass debris object.
- **What a component type declares.** *Proposal* (part-schema, after the TYPE table): the same as a part type, minus the shape builder and the mount faces:
  - parameters;
  - the slots it fits;
  - its network memberships (power, fluid, data, thermal, load);
  - its behaviour model;
  - its failure modes;
  - its derived mass;
  - its recipe;
  - whether it is parametric or needs a design.
- **Routing** (answers 15, 16).
  - Wiring and plumbing are auto-routed inside a part's volume, around the components that block them.
  - Route length and bends set mass, pressure drop, resistance and heat leak. The player can pin waypoints.
  - Routes cross parts only through ports.
  - The routing data model (how a route and its waypoints are stored) is **open** for session 3.

## 7. The system networks

One network per domain, for any topology (D133; answer 28). Each runs at its own rate (answer 27).

| Domain | Nodes | Links | Crosses a part boundary | Basis | Today's code (module map 2.1) |
|---|---|---|---|---|---|
| **Fluids** | volumes: each tank's liquid and ullage, manifolds | lines (friction from routed length and bends), valves, orifices, pumps | fluid ports | ruling 26 | `plumbing`, `plumbing_coast`: ADAPT |
| **Power** | buses (voltage nodes) | wires (resistance from routed length), switches, fuses, converters (efficiency maps); batteries and arrays as sources | connector ports | ruling 27 | `power`: ADAPT |
| **Data** | computers, sensors, actuators | buses (1553, Ethernet, CAN, RS-422) with bandwidth and latency, carrying streams and messages | connector ports | ruling 28 | `data`, `links`: KEEP, re-pointed |
| **Thermal** | each part's cells and its components | conduction inside a part; contact conductance at a mount surface; radiation by view factors | conduction: at the joined mount surface; radiation: between any exterior surfaces | ruling 29 | `thermal`: ADAPT (becomes the template network) |
| **Loads** | parts | mount joints, as edges with stiffness | at the joined mount surface | ruling 30 | `structure`, `breakage`: ADAPT, REPLACE |

Reading of ruling 13 for the two last rows: fluid, power and data cross only at ports. Heat and load cross at the joined surface itself, and radiation needs no joint (rulings 29, 30).

### 7.1 Fluids (ruling 26)
- Real properties come from CoolProp.
- Tanks carry boil-off, pressurisation and a few stratification layers. Slosh is an equivalent pendulum.
- There is no water hammer or line acoustics (fidelity by consequence, answer 6), except as a check flag.

### 7.2 Power (ruling 27)
- Batteries are equivalent circuits: open-circuit voltage against charge, internal resistance, temperature.
- Arrays use I-V curves against light and temperature.
- Sag and brownouts are real. E-pump motors draw through this network.

### 7.3 Data (ruling 28)
- A computer is abstracted to processing power, storage, power draw and bit flips (no emulators).
- Computer limits are simulated as resources (answer 38). Scripts are sandboxed WASM (answer 37).

### 7.4 Thermal (ruling 29)
- Contact conductance comes from the joint type and preload.
- View factors are recomputed only when the craft's shape changes.
- The environment loads the exterior cells: Sun, planetary infrared, albedo, plume, aero heating.

### 7.5 Loads (ruling 30; answer 45)
- The craft moves as one rigid body.
- Each step, inertial, thrust, aero, pressure and contact forces are spread through the graph as a sparse static system.
- The solve gives axial, bending and shear loads at every joint and a margin per part section.
- Flex and POGO come later, only where consequential.

### 7.6 Coupling (ruling 31)
- The domains solve in the fixed order **data > power > fluids > engine tables > thermal > loads**, each at its own rate, using the others' latest values.
- Tightly coupled loops (pump + motor + power bus) are solved together as one small system.
- This fits the warp spike's canonical grid. The scheduler and the warp rules are design session 2's (scope 3.3) and are not specified here.

### 7.7 The engine boundary (ruling 16; answer 9; answer 30)

**Simulated, on the networks.** Pumps, turbines and motors run on per-design performance maps (head, flow, speed, efficiency, NPSH), coupled by a real power balance. Valves, gimbals and actuators, the igniter and sensors are simulated too.

**The regen jacket** is simulated on the fluid and thermal networks, and a starved jacket burns through (ruling 16). This differs from G8's proposal, which put the jacket inside the table.

**Tabulated: the chamber and nozzle interior.**
- In: each propellant's inlet pressure, temperature and flow; the exact chamber and nozzle geometry (also CRUCIBLE's input); the ambient pressure.
- Out:
  - chamber pressure;
  - thrust vector;
  - Isp;
  - the mixture actually burned;
  - heat flux per wall cell (into the jacket on the networks);
  - plume state;
  - stability margin;
  - start and shutdown transients.

**Where tables come from.** Tables for real engines come from CEA plus published data, labelled. They are replaced by CRUCIBLE tables as those come (ruling 16; answer 31).

**Engine families** have their own interfaces (answer 30). The chemical liquid family is the first.

*Proposal* (G8): a chamber table keyed by the chamber design and a nozzle table keyed by expansion ratio and contour, so Rutherford SL and Vac share one chamber table.

## 8. Stored and derived, and what a save holds

Settled:
- A save holds no static data: types and part behaviour come from the game's files. A ship's trajectory and its thermal state stay in the save (D063).
- The stored trajectory is the truth (CLAUDE.md rule 4).
- Saves record their version (GL5). A change to what a save holds bumps `SAVE_VERSION` ([agent-rules.md](../plans/agent-rules.md)).

The rest of the table follows from rulings 1 and 4 and is a *proposal*. Derived means recomputed on load or build and never written.

| Item | Lives in | Stored or derived |
|---|---|---|
| part and component types, materials and families, engine tables, stand-ins | game data files | static (not in the save) |
| built-in designs (ruling 32) | game data files | static |
| the company's own and bought designs: geometry, rated performance and spread, recipe, known properties | **open**: the save, or a company library outside it | stored |
| craft file: which designs, which surfaces are joined, production choices per part, groups, scripts | the save for a craft in flight; a readable text file for the editor (answer 43) | stored |
| route waypoints the player pinned (answer 15) | with the production choices | stored |
| unit identity: serial, seed, inspection record, history | the save | stored |
| unit variation (strength factor, flaw sites, life constants) | the save, or re-derived from the seed: **open** | either |
| wear and damage accumulators, failure state, the failure's starting cell | the save | stored |
| operational parameters: fills, setpoints, valve and switch states | the save | stored |
| network state: each volume's mass, temperature, pressure and layers; battery charge and temperature; cell temperatures; component momentum (wheels, pump speed) | the save | stored |
| trajectory segments | the save | stored (D063) |
| mass, CG, inertia, volumes, MEOP | | derived |
| cells, mesh, surfaces, ports, network topology, routes from waypoints, joint stiffness, view factors | | derived |
| calibration factors and the calibration log (section 9) | | derived from the targets on each build |
| rated performance of a unit's design | the design | derived from the design, not copied into the unit |

## 9. Data trust and the calibration log

1. **Order: derived > published > estimate** (answer 11; D134, reversing D115's data rule).
   - Mass and the other properties come from geometry, materials and components.
   - A published figure calibrates the derived one, and a mismatch is logged as a model error, never silently overridden (scope 3.1, from answer 11).
2. **One stated free parameter** absorbs a mismatch when published values conflict or a mass budget will not close. The log records which published values it gives up (ruling 21).
   - Example (G12, numbers from the worked example): stage 1 at 9 x 8.16 kg/s for 155 s burns 11.4 t (derived), leaving 1.6 t of the 13 t lift-off mass (published) for stage-1 dry mass and everything above it.
   - The free parameter there is the throttle schedule (ruling 21).
3. **Groups** (ruling 38). A stage's or group's published mass calibrates the group's total, spread over its parts in proportion to their derived masses.
4. **Stand-ins** (ruling 38). Undisclosed technology gets a labelled stand-in (E) matched to the published performance: Curie's propellant is matched to its published Isp.
5. **Calibration order** (ruling 5; answers 7, 49):
   - Electron first, with Falcon 1 as the cross-check;
   - then Falcon 9 and Dragon on the same types;
   - then a probe.
   - The generality test: Dragon on another booster with an adapter, the engine count changed, a tank stretched, with no code changes.
6. **Labels.** Every number in data carries its provenance. The worked example's tags are **P** published, **D** derived (with the method), **E** estimate, **U** unknown. *Proposal:* the data format carries these tags per value, with a source for P.
7. **The calibration record.** *Proposal:* the target (quantity, value, source, label), the derived value, the free parameter and its value, the residual, and the published values given up.
   - The log is written on each build and checked by tests. Today's `checks.dry_mass` in the craft file become calibration targets (module map 4).

## 10. Failure and breakup

1. **Always a cause** (answer 46). A failure starts in a modelled mode, never "because the Merlin has this operational history".
   - Randomness only seeds variation: per-unit draws at manufacture (answer 74).
   - Stochastic processes such as micrometeoroids and single-event upsets are seeded by entity and epoch, each with a modelled cause (GL4; scope 3.3 rule 5).
2. **The part is the unit of failure.** A broken part becomes lumps of mass, possibly with an explosion as its stored energy is released (ruling 11).
3. **Each failure mode declares its breakup** (ruling 35):
   - burst: 3-8 shell fragments plus the released contents;
   - overload split: 2 pieces at the failed ring;
   - melt: progressive mass loss, then a split.
   Fragment masses and velocities come from the stored energy. Each component goes with the fragment it sat in. Small fragments become short-lived debris.
4. **A component failure damages its host** (a COPV burst, CRS-7). A jettisoned component is a lump-of-mass debris object (ruling 18).
5. **The starting cell.** *Proposal* (scope 3.1): every failure records the cell where it started. Ruling 35's "at the failed ring" needs it, and the module map's structure step assumes it (module map 6, step 6).
   - That record is ground truth. The player's investigation sees only what the craft's sensors recorded (GL12; answer 53).
6. *Proposal* (TYPE table): mode kinds are burst, buckle, melt and fatigue, with outcomes as in ruling 35.
7. **Today's code** (module map 2.1, `systems/failure`, ADAPT): the counter-based draws are kept. Hazard must accumulate on coast and landed steps too, and every statistical mode needs a modelled cause.

## 11. What the model carries for science and design programs

The data fields the model needs. The science system itself is outside this spec.

- **Nodes on a type** (rulings 15, 20).
  - A part class has a node per size regime and per alternative design (common dome, separate tanks, linerless).
  - Each material and each component type that is new in kind has its own node.
  - Trivial components (lines, hoses, wiring, fasteners, baffles, foam) have none; their ranges follow field levels.
- **The flight termination system** is a component under computing and information plus electromagnetism, with no new field (ruling 38).
- **Size regimes** come from real manufacturing thresholds: tooling, transport (road about 4-5 m, barge about 10 m), test facilities, launch-site handling (ruling 33).
  - The owner leans per-class, and two size classes often share one part class's logic with separate nodes (GL14).
  - Director's working default: a shared size-ladder rung, plus a class-specific size node only where the enabling technology really differs (GL17).
- **Design program inputs** (ruling 36; GL20):
  - the class's complexity (a tank much less than an engine, much less than a capsule);
  - novelty, from the design's nodes;
  - similarity to designs already owned;
  - with a base calibrated on NAFCOM and PCEC.
  - It takes months to years, limited by engineering staff. Tests are inside the design program; there are no test stands (GL11).
- **Per part type:** learning curves (answer 72). Per design: publish or keep (answer 73). Recipes are short chains (answer 57) and are information, sent at the speed of light (answer 80).
- **Start of game** (ruling 32).
  - Sandbox has everything.
  - The career's world knows present-day nodes, which the player licenses or researches.
  - The company starts with no designs. It can buy certain components, run a design program on a built-in part design, or make its own.

## 12. Rust building blocks and data types

Ruling 3: few code models, many data types. The test of the split: Dragon on another booster, a stretched tank or a changed engine count needs no code change (answer 7).

| Code (Rust, in `sim`) | Data (files) |
|---|---|
| shape builders (cylinder, dome, cone, body of revolution, plate, box, truss) and hand-built geometry with procedural tweaks for families that need it (ruling 3) | part types and component types: parameters, ranges, slots, surfaces rule, failure modes, recipe template, program model, size limits |
| mount kinds and the compatibility rule (rulings 12, 19, 25) | materials and their families, with level-driven property ranges |
| one network solver per domain and its element kinds (rulings 26-30) | engine tables (CEA now, CRUCIBLE later; ruling 16) |
| the coupling scheduler (ruling 31; session 2) | built-in designs (ruling 32) |
| component behaviour models: pump and motor maps, battery equivalent circuit, array I-V, valve, regulator, sensor, abstracted computer, reaction wheel, heater, actuator | craft files: designs, joined surfaces, production choices, groups (answer 43) |
| engine families and their interfaces (answer 30) | vehicle scripts on the program API, replacing today's hard-coded procedures (module map 2.1: `ship_os` chutes, commit, ullage, power_guard) |
| the deformable family (ruling 8) | calibration targets, provenance tags and stand-ins (section 9) |
| failure-mode kinds and the breakup rule (ruling 35) | |
| release and push kinds (ruling 22); live-cut behaviour (ruling 23) | |
| the auto-placer and the router (ruling 14; answer 15) | |
| the calibration solve (ruling 21, 38) | |

*Proposal:* an outline of the new types. The names are placeholders; the architecture is the Director's call (answer 68).
```text
PartType      id, family, version, nodes, shape builder, ParamSpec[], surface rule,
              SlotSpec[], internal volume, domains, FailureMode[], recipe template,
              program model, size limits
ComponentType id, family, version, nodes, ParamSpec[], slots it fits, network
              memberships, behaviour model, FailureMode[], recipe, parametric|designed
ParamSpec     name, unit, class (Geometry|Production|Operational), range(field levels)
Design        type id, geometry values, rated performance + spread, recipe,
              default production choices, provenance
Unit          design id, serial, seed, drawn variation, wear, history, state
Surface       frame on the part, kind + size, load class, Port[]
Port          kind (fluid|power|data|umbilical|mechanical), medium, place on surface
CraftFile     PartRef[] (design + production choices), Joint[] (surface pairs), Group[]
Network<D>    nodes and links per domain, built from the components and joined ports
Calibration   target, derived value, free parameter, residual, values given up
```

## 13. Mapping onto the module map, and fixtures first

D136: the craft core is rebuilt inside `sim` as new modules beside the old ones (ruling 6; module map 1). The frozen interface reads the model through a thin read adapter (module map 6, step 8).

| Module today (module map 2.1) | Lines | Verdict | What the new model changes |
|---|---|---|---|
| `catalog` (generators, derived mass, ports) | 3,032 | ADAPT | `Generator` splits into part types and component types; vehicle-named entries become generic types plus parameters; published calibrates derived (D134) |
| `craft` cells, mesh, broad, volume, mass, place, design, handle, registry | 3,009 | KEEP | the new build still emits today's cells, mesh, volume grid and mass properties, so aero, thermal, contact and proximity run unchanged (module map 6, step 2) |
| `craft` build, assembly, ship, state, layers, partition, configure, command, program, parts, file, small | 5,246 | ADAPT (rebuilt beside) | `ShipDef`'s per-kind vectors become parts, components and networks; the craft file's `components` move into part designs |
| `craft/engine.rs` | 250 | REPLACE | the engine-family interface (section 7.7) |
| `craft/stress*`, `falcon9_tests` | 585 | REPLACE | the stress craft and calibration tests on the new types |
| `systems` plumbing, plumbing_coast, coast_law, power, crew | 2,577 | ADAPT | one network solver per domain; hard-coded constants become part parameters |
| `systems` structure, g_limit | 1,409 | ADAPT | the load graph and margins per joint and part section (section 7.5) |
| `systems` propulsion, rcs, exhaust, fins, mechanisms, command, ship | 2,215 | ADAPT | engines read family tables; mechanisms become articulated surfaces |
| `systems/failure` | 1,302 | ADAPT | section 10 |
| `systems/ship_os` chutes, commit, ullage, power_guard | 698 | REPLACE | vehicle scripts shipped with each vehicle's data |
| `vessel` ship, ship_tick, ship_loads, systems, mass, split, aerothermal, coast_heat | 4,092 | ADAPT | follows the part/component model; split by joints, parts failing whole |
| `vessel/breakage.rs` | 129 | REPLACE | structure margins from the load graph |

**Craft-file blocks** (module map 4):
- `parts` stay.
- `components` move into part designs.
- `joints` become mount-surface joints with load paths.
- `plumbing`, `electrical` and `data` become port graphs.
- `groups` stay.
- `sequence` and `functions` become scripts.
- `checks.dry_mass` become calibration targets.
- The craft-level `aero`, `thermal`, `impact`, `contact` and `antenna` blocks move into part types. *Proposal* (part-schema findings): craft-level physics is removed.

**Today's 51 catalog entries** (module map 4) become generic types plus parameters. The calls module map 4 left to session 1 now read:
- the five Draco, SuperDraco and cold-gas entries follow section 3: a pod with its own exterior is a part, and thrusters inside a wall or a pod are components;
- the docking adapter is a part (ruling 7);
- `lc39a-hold-down` is ground infrastructure (ruling 24, as module map 4 already had it);
- the heat shield stays open, and so does the maximum part size (section 16, items 2 and 14).

### 13.1 The 21 fixture files (measured 2026-11-10 by grep and `wc -l` over `crates/sim/tests/`)

21 of the 29 files build today's craft: 20 use `craft::small` and 5 load `falcon9-crew` (4 do both). Together they hold 6,141 lines and 139 `#[test]` functions.

| Tier (*proposal*) | What the test needs from a craft | Files (lines, tests) |
|---|---|---|
| A: a body on a path | mass, inertia and a shape only | limbo (157, 3), limbo_physics (373, 5), limbo_regime (168, 3), proper_time (191, 6), frame_live_stop (115, 2), terrain (265, 8), landed_rest (31, 1), contact (514, 13) |
| B: parts and cells | the build, cells, mesh, thermal | parts (545, 11), aerothermal (482, 13), vessel_dynamics (524, 16) |
| C: systems and programs | engines, valves, power, chutes, the program API | craft_flight (166, 6), vessel_controls (114, 2), hold_modes (299, 8), ship_programs (155, 4), scenario_launch (152, 1), maneuver (289, 4), planned_burns (478, 13) |
| D: the determinism net | everything, run twice | save_load (293, 10), warp_course (675, 8) |
| E: the full vehicle | Falcon 9 + Dragon on the new types (module map 6, step 7) | falcon9_flight (155, 2), plus the `falcon9-crew` cases in maneuver, planned_burns, save_load, warp_course |

**Rules** (module map 6, steps 2-3; D136):
1. The new `small::rocket` and an Electron are built on the new types first. The old fixture stays until the new one passes the same assertions.
2. *Proposal:*
   - Relative assertions (chunked equals single-pass, warp invariance, save round trip bit-exact, frame invariance) carry over unchanged.
   - Absolute numbers tied to the old craft (a Δv, a burn time, a temperature) are re-derived for the new one, and the change is named in the commit.
3. The golden ephemeris, gravity and warp suites stay green before anything old is deleted. Deleting the 51 vehicle-named entries, `ShipDef`'s per-kind vectors and `craft/engine.rs` is the last step (module map 6, step 7).

## 14. The Electron worked example: its 23 gaps

The worked example has 33 part instances of 19 entries. Ruling 17 keeps each common-bulkhead tank one part, and ruling 18 moves eight instances (three COPVs, two Curie tanks, three batteries) of five entries into components, which leaves 25 part instances of 14 entries (derived here by counting its section 1).

| Gap | Status | Closed by | What stays open |
|---|---|---|---|
| G1 common bulkhead | closed | ruling 17 | |
| G2 parts inside parts | closed | ruling 18: pressure vessels inside are components; no internal mount surfaces (differs from G2's proposal) | |
| G3 dropped or not | closed | ruling 18: jettisonable components (the lighter of G3's two options) | |
| G4 articulated joints | partly | ruling 19 | whether ports across an articulated surface need flex-rated components |
| G5 non-ring surfaces | partly | ruling 25 (seam, partial ring) | concentric surfaces at one station (radius bands); a seam's load per metre |
| G6 release and push | closed | ruling 22 | |
| G7 ports to the ground | partly | rulings 13, 24 | what the umbilical joins on the ground side; GSE outside the node system |
| G8 engine boundary | closed | ruling 16 (regen jacket on the networks, not inside the table as G8 proposed) | chamber/nozzle table split (proposal) |
| G9 load class not fixed | closed | ruling 21 | |
| G10 complex component internals | closed | ruling 37 | |
| G11 material-swap loophole | closed | ruling 20 | |
| G12 published numbers do not close | closed | ruling 21 | |
| G13 nodes for trivial components | closed | ruling 20 | |
| G14 world-known vs company-known | closed | ruling 32; GL16 | |
| G15 undisclosed technology | closed | ruling 38 | |
| G16 thruster pods | closed | ruling 7; catalog | |
| G17 avionics boxes | closed | ruling 21 | |
| G18 group calibration | partly | ruling 38 (mass) | CG targets for groups |
| G19 missing types | partly | added to the catalog (Director): `pump.electric_centrifugal`, `power.motor_inverter`, `diffuser.pressurant`, `conduit.feed_tunnel` | the kick stage as an equipment bay with a propulsion node; the engine pattern as one type's geometry parameter |
| G20 linings and coatings | open | | TPS and foam as a field over cells (proposal) |
| G21 range safety | closed | ruling 38: a component under existing fields (differs from G21's proposal of outside the node system) | a licensing rule per launch site |
| G22 size regimes at the edges | partly | ruling 33; GL14, GL17 (working default) | what "ground-launched" means; where the regime boundaries fall |
| G23 live cuts and severing order | closed | ruling 23 | which component declares the ports it severs (proposal) |

Count: 16 closed, 6 partly, 1 open (of 23).

## 15. Conflicts with canon

Only D115's data rule is reversed, because answer 11 says so (D134). The older entries below disagree with D130-D136 or with the rulings; each is listed, not resolved ([decisions.md](../decisions.md), open question 6).

1. **D100.** It names Falcon 9 + Crew Dragon as the first vehicle. Ruling 5 and answer 7 make Electron the calibration vehicle first, with F9 + Dragon right after. D100's "layers" are this spec's networks.
2. **D101.**
   - "Truly published numbers overrule procedural numbers", against D134's derived > published > estimate.
   - Its "generator" and "catalog entry" are this spec's type and design.
   - "Their own generators later" for players, against ruling 3 (a new type is data; new physics is code).
3. **D102.** The craft file "places components in parts" and lists layer connections. Here the design owns its components and the craft file holds only production choices within the design's slots (rulings 1, 4, 14; answer 75; where those choices live is this spec's reading); networks come from joined ports. This one is soft. D102's glTF shapes can coexist with shape builders as hand-built geometry (ruling 3).
4. **D103.**
   - Joint loads "computed exactly for the rigid body (the subtree beyond it)", against ruling 30's sparse stiffness graph. The two agree on a tree and differ where load paths are redundant.
   - "Nothing wobbles", against ruling 8's deformable family and ruling 30's "flex and POGO later". D132 refines D103 only for mechanisms.
5. **D104.**
   - Engines from published data with "no flow inside the engine at runtime: what happens there fails statistically", against ruling 16 and answer 9 (simulated pumps, valves and regen jacket; failures with a cause).
   - RCS thrusters as placed components, against ruling 7 (pods and standalone thrusters are parts).
   - Power "in watts over buses (voltage derived)", against ruling 27's nodal voltage solve.
6. **D105.** Statistical modes "from the part's data or else its class's historical rate", against answer 46 ("always a cause"). Fates drawn "from the save's seed, the launch time and the part", against answer 74 (drawn per unit at manufacture).
7. **D027.**
   - "Each catalog entry (D101) names its maker", and "developing and manufacturing your own parts in the career comes later".
   - Against ruling 32: the company starts with no designs, and design programs on built-in designs or its own from the start.
8. **D063.** A save holds no static data, but a player's designs are not game files. Where they are saved is open (section 8).

## 16. Open for the next session

Undecided. Each line says where it comes from.

1. TPS tiles, linings and acoustic foam: a field over a part's cells, or components (G20; part-schema findings).
2. The heat shield: its own part or the capsule's (module map 4).
3. Concentric surfaces at one station, and a seam's load per metre (G5).
4. Flex-rated ports across articulated surfaces (G4).
5. Mount-kind size tolerances and the load class numbers (L1-L3 are a proposal).
6. What "ground-launched" means, and where the size-regime boundaries fall (G22; GL14, GL17).
7. The ground side of an umbilical; GSE outside the node system (G7; part-schema findings).
8. Removing craft-level physics blocks from the craft file (part-schema findings; consistent with D125).
9. Unit variation stored in the save or re-derived from the seed (section 8).
10. The player's designs: in the save, or in a company library (section 8; D063).
11. The routing data model (session 3; answer 15).
12. How a design's statistical spread is represented (answer 71; ruling 1).
13. CG calibration targets for groups (G18).
14. The maximum part size (scope open 6). This spec reads it as answered by rulings 10 (class cap, structure limits the rest) and 35 (breakup by mode); to be confirmed.
15. How a modded game instance is marked (answer 56).
16. The TYPE and component-type field tables, which are proposals (part-schema).
17. The failure's starting-cell record (scope 3.1; implied by ruling 35).
18. A parametric component's "design" as its parameter set (section 1).
19. The material-swap check (section 4) and port matching (section 5.3).
20. Draco thrusters inside the capsule wall as capsule components (section 3, this spec's reading).
21. The packed parachute before deployment: a component, or a deformable part from the start (rulings 7, 8).
22. Domain gaps with no field or solver yet: micrometeoroid and hypervelocity impact, and granular mechanics for wheels and excavation (part-schema findings).
23. The data inconsistencies found: Dragon's helium bottles (0.1 m³ at 31 MPa cannot supply a 5 s abort at 16.5 MPa), the trunk radiator with no numbers, no CG or inertia data for validation (part-schema findings).
24. The canon conflicts of section 15, each one which way.
