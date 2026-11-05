# Design session 1: the part schema (working draft)

Status: in session with the owner, 2026-11-04. Settled items are marked **settled** with the answer number from the [interview](../../archive/interviews/interview-2026-11-02-scope.md) or this session. Everything else is a Director proposal. Inputs: [module map](module-map.md), [type catalog](research/type-catalog.md), [Electron / Falcon 1](research/electron-falcon1.md), [F9 / Dragon breakdown](research/f9-dragon-breakdown.md).

## Settled so far
1. **Type > Design > Unit** (session 1, Q1).
   - **Type**: a part class. Gated by science nodes; its parameter ranges move with field levels. Written as data that composes a small set of Rust building blocks.
   - **Design**: the output of a design program. Fixed geometry parameters, rated performance with statistical spread, a recipe. Components can be swapped freely when compatible, and materials can probably change, without a new design (owner note).
   - **Unit**: one built object on a craft. Its seeded per-unit variations (strength, flaws, life constants), wear and state.
2. **Components: simple ones parametric, complex ones designed or bought** (Q2). Lines, valves, sensors, battery packs and heaters are sized freely once the node is known. Turbopumps, flight computers, radios and similar are designs: a design program, or bought from a supplier. The owner expects many more components per part than the first sketch showed.
3. **Few code models, many data types; lean toward MORE part types** (Q3). New physics or new hand-built geometry is code: "a fusion engine needs to have completely different behavior than a chemical engine, and it would need to have hand built geometry that leaves room for procedural tweaking." A new type is data. "Hundreds is fine eventually."
4. **Parameter classes: geometry / production / operational** (Q4). Geometry is fixed by the design (changing it is a redesign, with the similarity discount). Production can change between builds (material grade within a family, components, routes, insulation; NOT wall thickness, see settled 20). Operational is set in flight or before launch (fill, setpoints, valve states).
5. **Calibration vehicle: Electron** (Director, from [research](research/electron-falcon1.md)). It has a current user's guide, ~99 flights, and the electric-pump Rutherford, which is a battery > motor > pump > fluid coupling across three graphs. Falcon 1 is the cross-check. Biggest gaps: stage dry masses, the Rutherford's internal split, battery sizing, Curie's propellant, and the CFRP wall: all to be derived and labelled.
6. **Reset size: re-architect the core inside `sim`** (Director, from the [module map](module-map.md); architecture is the Director's call, answer 68).
   - Keep about 63.7k lines (53%) as they are.
   - Adapt about 33k lines.
   - Rebuild about 20k lines as new modules beside the old ones: catalog, craft build/ship/state, systems, and the ship side of vessel.
   - The frozen interface (20.6k lines) reads the model through a thin adapter.
   - Rebuild the 21 integration-test fixtures on the new types BEFORE deleting anything old, so the determinism checks are never lost.

7. **Part vs component test: own exterior or own separation** (Q5). A part has its own exterior shape and mount surface, or can separate or move as its own body (thruster pods, docking ports, legs, panels). Anything inside a part's volume is a component, even if it spins or moves: a reaction wheel in the bus, a turbopump in the engine. Its momentum is component state that torques the part.
8. **Never-rigid objects get a deformable-part model family** (Q6): a reduced, quasi-static shape from loads (canopy inflation, sail billow, boom bending) feeding aero and load paths. It goes into the schema now and gets built when the first one is needed (Dragon's chutes).
9. **Attachment is by mounting surfaces; decoupling is always a component** (Q7, owner's words condensed):
   - Every part type defines mounting surfaces with a mount type. Two parts with compatible surfaces attach directly (a bolted joint is implied); no extra object is needed.
   - A component can change a surface's mount type: a side mount added to a tank, or a separation component on a tank's outside that creates a surface which, when fired, severs and imparts its mechanical effects (springs, pushers).
   - A separate part sits in between only when there's a major mechanism or a large adapter, or when it makes the joint structurally sounder. A decoupler or adapter part can do the separation (through its component) and a geometric job at the same time.
   - "At the most fundamental level, the actual decoupling is always done by a component."
   - Owner: "This needs refinement."
10. **Size and function** (Q8):
    - One pressurised volume is one part.
    - **A part class is one function**, enforced by its class: for example "cylindrical liquid tank for ground-launched rockets". Its size is limited by a sanity cap for the class (perhaps 20-30 m diameter even with high-level materials) and otherwise only by structure. A 20 m tank may be designable and still break in real flight.
    - A class can offer predefined sizes (1.2, 5, 9, 10 m).
    - Orbital construction unlocks other, far bigger classes (an orbital spherical tank up to ~200 m).
11. **A broken part becomes lumps of mass** (Q8): a set number of segments, each a lump of mass, possibly with an explosion (stored energy released).
12. **Mount types: a small standard set plus size** (Q9): rings of a diameter and bolt-load class, radial hard points, truss nodes, docking standards, payload adapter standards. Compatible means the same kind and the same size within tolerance; anything else needs an adapter part or a surface-changing component.
13. **Ports live on mounting surfaces** (Q10). Joining two surfaces joins the matching ports, and a system graph crosses a part boundary only there. An umbilical is a port on a surface that a component severs.
14. **Components have real 3D placement, auto-placed by default** (Q11). Each has a position and envelope in the part's internal volume (for routing, CG, inertia, thermal cells). An auto-placer fills the slots; the player can drag components within the allowed regions (production class, no redesign).
15. **Science mapping: class node + material nodes + component nodes** (Q12), and refined: "You would have nodes for different general sizes of tanks, and nodes for alternate part designs. This applies to all parts." So each part class has a node per size regime (small, medium, large tankage) and per alternative design (common dome, separate tanks, linerless), and each material and component type has its own node.
16. **Engines** (Q13-16):
    - The chamber/nozzle interior is a table. In: each propellant's inlet pressure, temperature and flow, the exact chamber and nozzle geometry (also the input CRUCIBLE needs), and ambient pressure. Out: chamber pressure, thrust vector, Isp, mixture actually burned, heat flux per wall cell, plume state, stability margin, start/shutdown transients.
    - Pumps, turbines and motors run on performance maps per design (head/flow/speed, efficiency, NPSH), coupled by a real power balance.
    - The regen jacket is simulated on the fluid and thermal graphs. A starved jacket burns through.
    - Tables for real engines come from CEA plus published data, labelled, and are replaced by CRUCIBLE tables as inquiry produces them.
17. **One pressure-vessel structure is one part, and it may hold several fluid volumes** (Q17). A common bulkhead is one part (it replaces rule 10's "one pressurised volume").
18. **Anything inside a part's volume is a component, including pressure vessels; some components are jettisonable** (Q18). A COPV burst is a component failure that damages its host (CRS-7). A jettisoned component (Electron's spent stage-2 batteries) leaves as a lump-of-mass debris object. No internal mount surfaces are needed.
19. **Moving joints are articulated mount surfaces** (Q19): kind hinge, gimbal or slider, with range and stiffness. The actuator is a component on one side, attached to a hard point on the other. D103's mechanisms become these.
20. **Nodes and free changes** (Q20):
    - Trivial components (lines, hoses, wiring, fasteners, baffles, foam) have no node; their ranges follow field levels. Nodes exist only for component types that are new in kind.
    - A material swap is free only within a family, and only if the design still fits together and behaves the same: "Does switching to CFRP make mounting components necessarily different? Does it change wall thicknesses?"
    - **Wall thickness and similar geometry-altering parameters are geometry: they need a redesign.** (This corrects the first sketch, where thickness was a production parameter.)
21. **Director calls from the worked example** (architecture, reported): mount load classes are for compatibility only, and the real rating comes from the computed load path (a pressure-stabilised tank's ring strength depends on its ullage pressure). When published values conflict or a mass budget won't close, one stated free parameter absorbs the mismatch, and the mismatch is logged (Electron: the throttle schedule). Avionics boxes are components in the avionics bay part, so the type catalog is corrected.
22. **Separation = release components + push components on the mount surface** (Q21). Release: pyro bolts, frangible joints, clamp bands, pneumatic latches. Push: springs, or pneumatic pushers fed from a gas bottle on the fluid graph. Firing is physical: stored spring or gas energy sets the separation velocity and tip-off, and a failed release leaves a real hang-up.
23. **A port cut while live behaves physically unless it has a sealing component** (Q22). Quick-disconnect or self-sealing components close. Anything else vents its line contents (thrust and mass loss), opens the circuit and loses the data link. Choosing QDs is the player's job.
24. **Ground: abstract ground services, except the tower and launch pad** (Q23). The pad and tower are built with their own procedural geometry. All other locations are abstracted (consistent with colonies using the ship component model without geometry).
25. **New mount kinds: seam (a line joint along an edge, with its own release components) and partial ring (an arc)** (Q24), for fairing halves, panels and trusses.

## System graphs (agenda item 3, settled with the owner 2026-11-04)
26. **Fluids: a lumped compressible network with two-phase tanks** (Q25). Nodes are volumes (tank liquid and ullage, manifolds); links are lines (friction from the routed length and bends), valves, orifices and pumps; real properties from CoolProp. Tanks carry boil-off, pressurisation and a few stratification layers; slosh is an equivalent pendulum. No water hammer or line acoustics (fidelity by consequence), except as a check flag.
27. **Power: a DC nodal circuit solve** (Q26). Buses are voltage nodes; wires get resistance from their routed length; switches, fuses and converters run on efficiency maps; batteries are equivalent circuits (OCV vs charge, internal resistance, temperature); arrays use I-V curves vs light and temperature. Sag and brownouts are real, and e-pump motors draw through it.
28. **Data: links with bandwidth and latency, carrying streams and messages** (Q27): computers, sensors and actuators are nodes; buses (1553, Ethernet, CAN, RS-422) are links. **A computer is abstracted** to processing power, storage, power draw and bit flips: "No need for full on emulators for each and every computer on the rocket, after all everything has to run on a laptop."
29. **Thermal: cells + contact conductance + view factors** (Q28). Each part's cells and its components are thermal nodes. Heat crosses a mount surface by contact conductance (from the joint type and preload). Radiation between surfaces uses view factors recomputed only when the craft's shape changes. The environment (Sun, planetary IR, albedo, plume, aero heating) loads the exterior cells.

## What a part TYPE declares (proposal)
| Field | What | Example (`tank.cylindrical`) |
|---|---|---|
| id, family, version | identity; the modding boundary sits here (answer 56) | `tank.cylindrical`, tankage |
| nodes | class node per size regime and alternative design, plus a node per material and per component type (settled 15) | medium ground-launch tankage; linerless design; CFRP |
| shape builder | the Rust geometry block(s) and how parameters drive them | cylinder + 2 domes |
| parameters | name, unit, class (geometry / production / operational), in-universe range as a function of field levels | radius 0.3-6 m and wall thickness (geometry, settled 20); material grade within a family (production); fill (operational) |
| mounting surfaces | generated from the parameters: position, size, mount type, structural rating, which ports each can carry; components can add or change surfaces (settled 9) | top ring, bottom ring; a side mount added by a component |
| ports | interface kinds per face: fluid coupling, power/data connector, umbilical, mechanical | LOX outlet at the bottom dome |
| component slots | which component types fit, where (volume, wall, dome, face), how many, the fit rules | pressurant inlet, outlets, sensors, baffles, heaters, insulation |
| internal volume | routing space and keep-outs for the router | the annulus between the wall and the baffles |
| domain models | which solvers it takes part in, at what fidelity | structure (pressure vessel), thermal cells, aero surface, fluid node(s) |
| failure modes | cause-based: burst, buckle, melt, fatigue; how a failure propagates (section 3.1 of the scope doc: by stored energy) | burst: whole part; overload: split at the weakest ring (later) |
| derived quantities | what's computed from the parameters (mass, volume, MEOP, inertia) and what can be calibrated | dry mass derived; published mass calibrates it |
| recipe template | materials + components + labour as a function of the parameters (short chains, answer 57) | structural metal sheet, insulation, pipe |
| design program model | how cost and time scale (complexity, novelty, similarity discount) | |
| size limits | the class sanity cap + predefined sizes; structure limits the rest (settled 10) | ground-launch tank: up to ~20-30 m diameter |
| broken state | how many segments (lumps of mass) it breaks into, and the stored energy released (settled 11) | burst: 3-6 segments + pressure release |

A **component type** declares the same, minus the shape builder and mount faces: parameters, the slots it fits, its graph memberships (power, fluid, data, thermal, load), its behaviour model, failure modes, derived mass, recipe, and whether it is parametric or needs a design (settled item 2).

## Findings from the research to settle
- **The part/component test.** The type catalog proposes "needs its own rigid-body state at its own mount face". The owner's definition is "the smallest thing you can place". The Director's proposal is in Q-next 1.
- **Thruster pods become parts** (F9/Dragon breakdown): Draco clusters, SuperDraco pods and cold-gas pods are catalogued as components today; the owner's own example of a tiny part is a radial RCS thruster.
- **Never-rigid objects** (type catalog): deployed parachutes, sails, inflatables and long booms have no path in "rigid + load paths".
- **TPS tiles** (type catalog): too small and numerous to be parts. Proposal: a material and thickness field over the heat-shield part's cells, not components.
- **GSE** (umbilicals, fill and drain): it maps to no science field. Proposal (Director): money and mass only, outside the node system.
- **Craft-level aero/thermal/contact/antenna blocks** in today's craft file give a whole craft physics properties, which contradicts 3.1. Proposal (Director): removed; all physics comes from parts.
- **Data inconsistencies found:** Dragon's helium bottles (0.1 m³ at 31 MPa can't supply a 5 s abort at 16.5 MPa); the trunk radiator has no numbers; there's no CG/MOI data for validation.
- **Domain gaps** (type catalog): hypervelocity impact / MMOD and granular mechanics (rover wheels, excavation) sit in no field or solver yet.
