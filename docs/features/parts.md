# Feature: parts, splits and debris

**Status:** skeleton (2026-10-26, overnight PARTS workstream): the data model, aggregation, splitting, breakage triggers and the debris rule are built and tested in `sim`; every property of a part is a placeholder; the game resolves breakups and deletes debris, but no craft the game flies has more than one part, and a piece is still drawn with the test craft's mesh. Decisions: D032 (rigid bodies whose parts can break), D033 (systems on the parts), D065 (heat; a part over its limit is destroyed), D066 (contact judged per point), D064 (debug mode), D081 (scale), D083 (debris). **Craft creation (a part catalog, an editor, design rules) is the owner's to guide and is not here** (interview 2026-10-26): the stress craft (`sim::craft::stress`, benchmarks and tests only) is the only craft of many parts.

## Prior work

| Source | What it does | Taken | Rejected |
|---|---|---|---|
| KSP | A vessel is a tree of parts; decouplers and broken joints split the tree into vessels; joints break above `breakingForce`/`breakingTorque`; debris persists up to a count limit | The attachment tree, a split as a cut of the tree, each group a vessel, a per-part load limit | Physics-joint flex (D032: rigid bodies); a debris count limit (D083: a rule) |
| Orbiter (docking and undocking) | Separated vessels each continue from the composite's state | Rigid-body separation: each piece at its place in the composite, moving with it | — |

## Design

### 1. The part layer (`sim::craft::parts`)

A craft is a set of **parts** in an attachment tree. A `Part` owns primitives of the craft's geometry (each primitive belongs to exactly one part) and carries placeholders: dry mass, a tank (capacity and the cylinder it fills), an engine (`EngineFile`), RCS thrusters (`RcsFile`), the parachutes (a flag: the craft's `ChuteSpec`), the interior's specific heat and node limit, power (produced in full sunlight at 1 AU, used, stored), a **control** flag (a crew cabin or probe core), crew seats and a **load limit**. A part keeps its `id` (its index in the craft it was designed in), so a piece of a craft names its parts in the original.

- **A craft file is one part** (`CraftDef::whole`): the test craft builds, flies and saves bit for bit as before (its `CraftParams` still compare equal to the playtest save's).
- **Mass**: each part's dry mass is a thin shell over its *own* primitives' union surface (weighted by skin areal mass), so its mass properties do not depend on what it is attached to; the craft sums them (parallel-axis theorem). Several tanks **feed together**: every tank is at the same fill fraction, and the propellant's first and second moments are polynomials in that fraction (`mass::TankMoments`), so `MassModel` stays a `Copy` value of one scalar (the vessel's propellant). One part with one tank keeps the old formula.
- **Aggregates**: dry mass, propellant, crew (`parts::aggregate`), the engine of all engine parts firing together (`combine_engines`: summed thrust, the Isp of the summed flow, the thrust-weighted mount, the summed nozzle heat), the RCS couples summed. A piece without an engine has a zero-thrust stand-in that never runs.
- **Design** (`CraftDesign`): every cell and interior node is tagged with its part (`cell_part`, `node_part`); each part's interior heat capacity is spread over the volume its primitives fill (a part too thin for the grid's samples, a fin or a panel, puts it in the nodes under its cells); each node's limit is its part's; each engine part's own bell is its nozzle, taking its engine's share of the wall heat (D077).
- **Where a craft comes from** (`sim::craft::registry`): a `CraftId` resolves to a definition: registered at run time (`register`, for craft made in the game later), `stress-<n>`, or a `data/craft` directory; a **piece** is `base#ids` (ranges of part ids, e.g. `stress-300#12-16,32`). `craft_of` builds each id once per process, so a vessel's design is rebuilt after a load from its id alone (scale baseline problem 3).
- **Cells by area** (scale baseline problem 1): every craft gets at least its area over 0.3 m² in cells (the test craft's 512 on 136 m² are 0.26 m² each); a craft of parts at least 4 per part. **Thin tubes** (problem 2) are split across their circumference into half-tubes, first, so no cell's normals cancel (the test craft's legs included: its cells changed, the playtest save is to be regenerated).

### 2. Splits (`sim::vessel::split`)

A set of parts leaves as a new vessel, a full one with its own trajectory, aerodynamics and heating.

- **Plans**: `Vessel::plan_split(parts)` (separate exactly those parts), `plan_detach(parts)` (cut them from their parents: each leaves with what hangs from it, a decoupler) and `plan_breakup()` (the pending breakup) give a `SplitPlan`: the connected groups after the cuts (`parts::components`), each destroyed part a wreck of its own. The vessel keeps the group with its lowest control part, else the heaviest; with no intact group left it keeps the first wreck and is destroyed.
- **Building the pieces** (`SplitPlan::crafts`: each piece's craft through the registry) is the expensive step (§ Measured); a caller may run it off its frame thread. `Vessel::split(world, plan, crafts, ids)` then applies the split at the vessel's time.
- **Motion**: every piece keeps the body axes, the attitude and the angular velocity ω; its centre of mass is where it was in the craft, moving at v + ω × r (r from the craft's centre of mass). Linear and angular momentum are conserved, since the pieces' mass properties sum to the craft's.
- **State carried**: propellant by tank (each piece's tanks at the craft's fill), skin and node temperatures (each new cell or node takes the nearest one of its own part), the ship clock, the systems (panel, battery and its charge, loads in proportion to the parts' power placeholders; boil-off by tank capacity), debug mode, the limbo placeholders. The parachutes go with their part (a piece without them has none to open), the crew with the seats (the vessel's piece first). The vessel keeps its flight plan and attitude control; a new piece has neither.
- **Phases**: live stays live; a coast restarts as coasts at the split epoch (the stored trajectory stays the truth: the split epoch is deterministic, so chunked equals single pass); a landed or crashed vessel is woken into live ticks.

### 3. Breakage (`sim::vessel::breakage`)

Judged at the end of every live tick, with placeholder limits; **debug mode (D064) disables all of it**.

- **Temperature**: a part with a skin cell over its material's limit or a node over its part's limit is **destroyed** (D065, "the part is destroyed cleanly"): it leaves as a wreck, and what was attached through it comes apart into its connected groups. *Decided here:* destroyed and detached as a wreck (its mass and momentum stay in the world), not removed, so the debris rule decides when it goes.
- **Impact** (D066, per contact point): the part of the point that hit too fast is destroyed; the rest of the craft keeps its engine and controls.
- **Load**: a part whose cells carry more than its `max_load` is **torn off** its parent, intact, with what hangs from it (a root part, the craft's or a piece's, has no parent and stays). Its load is the flow's Newtonian pressure on its exposed cells, q·Cp,max·e·A·cos²θ summed as a vector (the most the flow can push on them; exposure e from the aero bake), plus the ground's normal force at its contact points (`contact::normal_load`, the contact's own spring-dampers).
- **One part** (the test craft) keeps the whole-craft rule: over a limit or hit too fast, the vessel is destroyed and flies on as a wreck (D065, D066 as before).
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

- No part catalog, editor or design rules (the owner's); placeholders everywhere: power is not a path (D033), the load is a pressure ceiling, not a structural solver (joint forces from the rigid body's acceleration and torques are not computed), thrusters are on one part.
- Overheating is judged in live ticks only, not on the coast thermal lattice (a coast never heats a part past its limit today: sunlight and thin air are below every limit).
- A piece is drawn with the test craft's mesh in the game (the piece's `Craft::mesh` exists; the scene does not use it yet); the game flies no craft of parts, so no breakup happens in play today.
- Designs of large pieces cost seconds (the remainder of a 300-part craft is a new design); built crafts are kept for the process (no eviction).
- Tanks feed together (one fill fraction for all); propellant does not flow between pieces after a split.
- A breakup applies one impact per tick (the first point); further hard points break on following ticks.
