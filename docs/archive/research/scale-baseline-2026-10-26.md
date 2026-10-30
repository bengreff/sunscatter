# Scale baseline, 2026-10-26 (D081)

> Status: **measurement** (the BENCH workstream of [overnight-2026-10-26.md](../plans/overnight-2026-10-26.md)); it feeds PERF and PARTS. Craft: `sim::craft::stress` (synthetic, benchmarks and tests only; craft creation is the owner's to guide). Bench: `crates/sim/examples/bench_scale.rs`. **Reference commit 1f39836** (code and data): AERO's boundary layer, heating, base drag and fin interference, and its `find_fins` fix; LIMBO's perturbed coasts, their stop at the live boundary, the systems lattice and the averaged sunlight of held and tumbling coasts; SOL's Solar System of that commit. Earlier runs at 869fa4b and 526c411 are named where the code changed the result, and are the other samples in the ranges where it did not.

D081 asks for 50 craft of ~300 parts at every warp up to 1,000,000×, live physics for 1–3 craft at low warp, and 10 or more craft in limbo. The table says where each cost curve breaks against that; the details, the hot spots and the pipeline problems follow.

## Where each curve breaks

Frame: 16.7 ms (60 fps, D028). A live craft runs 0.83 ticks per frame at 1× and 3.3 at 4× (the physics-warp limit, D029). At 10⁶× a frame spans 16,667 s: 28 points of the 600 s coast thermal lattice.

| Cost | 300 parts | 1,000 parts | Against D081 |
|---|---|---|---|
| Live tick, ascent (M 1.2, the dearest in flight) | 2.3 ms (2.2–2.6) | 7.8 ms (7.0–7.8) | **3 live 300-part craft fit at 1× (5.7 ms a frame), not at 4× (23 ms in turn; 7.6 ms if each runs on its own thread).** One 1,000-part craft at 4× takes 26 ms: breaks; at 1× 6.5 ms. |
| Live tick on the pad (contact substeps) | 3.0 ms (2.6–3.3) | 10.0 ms (8.7–10.0) | As above, worse: three on pads at 4× is 30 ms. Most of it is aerodynamics at zero wind (hot spot 3). |
| Live tick, entry / vacuum burn | 1.3 / 0.33 ms | 4.9 / 1.2 ms | Entry a little cheaper than the ascent; vacuum fits everywhere. |
| Design (aero bake, volume grid, network, drag) | 2.0 s | 13.5 s | **Breakups break**: every new piece needs a design. PARTS' "a 300-part craft splits in under a frame budget" is ~130× away (two ~150-part designs ≈ 2 × 1.1 s). Small debris: 0.13 s (1 part) to 0.42 s (50 parts) each. Fine once per design, off the frame thread. Before AERO's 5749414 (`find_fins` against its own primitive only, from this baseline's first version): 4.5–5.7 s and 39–44 s. |
| Craft build (union surface, cells, mass) | 0.37 s (0.37–0.50) | 2.5 s (2.5–3.0) | Also per split; ~22× a frame at 300 parts. |
| Limbo at 10⁶×: 10 tumbling craft, half of 300 parts | — | — | **5–6× the frame in turn, 3× on 4 threads**: 84 ms a frame (51 ms on 4 threads) from launch, 106 ms (46 ms) after 30 days. At 526c411, before LIMBO's averaged sunlight let tumbling lattices skip, 430–527 ms (155–186 ms). |
| 50 craft at 10⁶× (10 limbo, 40 quiet) | — | — | **22–28× the frame in turn, 6–8× on 4 threads**: 463 ms a full-warp frame from launch (138 ms on 4 threads), 377 ms after 30 days (106 ms). A settled quiet 300-part craft alone costs 11 ms a frame. Where it goes (settled): the perturbed coast integration 55 %, the thermal lattice 18 % (hot spots 1 and 2). |
| Coast thermal lattice, one 600 s solve | 1.9–2.6 ms | 6.3–9.2 ms | 18–24 sweeps of all cells and nodes; a tumbling day now averages 1.3 ms a point at 300 parts (skips; 2.6–3.7 ms when every point solved). |
| Work budget | — | — | **Never bites**: 0 of 240 frames compute-limited at 10⁶× while frames take 0.1–0.5 s (the 2 of 240 in one 50-craft run are a vessel holding the clock at the live boundary). A coast thermal solve counts 30 units (`THERMAL_SOLVE_UNITS`, ≈ 200 µs, the test craft's); a 300-part solve costs 1.9–2.6 ms. The game shows 10⁶× at 2–10 fps instead of holding its clock back. |
| A vessel going live inside a rails frame | — | — | **Fixed for the air by LIMBO's 026080b**: at 869fa4b a 300-part craft on a lunar transfer, turned back by the Moon, entered Earth's atmosphere and crashed on Earth inside one 10⁶× frame (16–18 s for that frame). Now its coast stops at the live boundary, the clock holds and the warp falls to 4× (188 of 240 frames; worst frame 1.5 s). Still open: a coast ending on a surface (contact) or an engine start flies on live inside the call. |
| Coast integration step | 6.1 µs | 6.1 µs | Independent of the part count (3.5 µs at 869fa4b, before perturbed coasts and the larger Solar System; LIMBO measures 14 µs for a held LEO coast with drag). At 10⁶× one LEO vessel integrates ~4 ms a frame. |
| Fleet at 1× and 1,000× | — | — | Fits: 50 craft 0.16 ms a frame at 1× (no live craft in the fleets; first frames up to 10 ms while every coast restarts on its pointing), 1.7 ms at 1,000× with 66 ms spikes (the vessels' lattice points fall on the same frames); settled 0.43 ms (19 ms worst). |
| Memory | design 5.7 MB, craft 18.5 MB, vessel 51 KB | 18.7 MB, 64 MB, 170 KB | 50 vessels sharing one design: 1.8 MB. **Breaks for debris** if each piece keeps its own craft and design: 50 distinct 300-part designs ≈ 1.2 GB (the f64 surface and f32 render mesh are 18.5 MB of each). |

## Machine and method

- **Machine:** Mac14,9 (Apple M2 Pro, 6 performance + 4 efficiency cores, 16 GB; the D028 reference), macOS 15.5, rustc 1.98.1, release builds.
- **Load:** other agents compiled, tested and benchmarked all night: load averages 5–10 on 10 cores during the reference runs (one run at 30–43 was thrown away). Each timing is the median of 3 runs; separate runs of the same code differed by up to ±20 %. Ranges in the summary span the runs (two to five) of code that did not change the result. Read every number as ±20 %.
- **Rerun:** `cargo run -p sim --release --example bench_scale` (both sections, ~25 min), or `… craft parts=300 runs=1`, `… fleet fleets=10`.
- **Profiles:** no `samply` or `cargo instruments` on the machine; macOS `sample` on `bench_scale profile=<bake|design|build|live|contact|lattice|fleet|settled>,<parts>` (each repeats one case for 30 s), built with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` and symbolicated with `atos -i` on a `dsymutil` dSYM (inlined frames to source lines). Percentages are self samples (15–30 s at 1 kHz) unless called inclusive.

What each number includes:

- *Union surface*: `mesh::union_surface`. *Build*: `Craft::build` (union surface, `cells::build_cells`, `MassModel::new`). *Aero bake*: `aero::bake` (642 directions). *Volume grid*: `VolumeGrid::new`. *Design*: `CraftDesign::new` (bake, grid, thermal network, limbo surface table, mean drag area; the network and the surface table are under 1 % each of it). *Mean drag area*: `design::mean_drag_area` (642 force evaluations).
- *Cells (tubes)*: cells whose triangles' normals nearly cancel (|Σ n·a| < ½ Σ a); *default 512*: the same surface with `CellOptions::default()`, as `parse_craft` builds every craft today.
- *Memory*: heap arrays by length × element size (design: exposure, cell geometry, network, volume nodes and tank samples, cells, surface table; the volume grid's private index estimated); *craft*: the f64 surface, the f32 render mesh, cells; *vessel*: `Vessel` plus thermal state, trajectory samples and contact points (the design is shared); *save*: the vessel as RON.
- *Live ticks* (debug mode: infinite propellant, no destruction; SAS on): `Vessel::advance` over 4 s (300 and 1,000 parts) or 20 s: **entry** at 100 km, 7.6 km/s inertial (7.1 km/s through the air, M 25), 1.5° down; **ascent** at 12 km, 354 m/s through the turning air, climbing (M 1.2: wave drag, slender body, fins); **vacuum** at 400 km, full thrust; **pad** at the game's pad, engine at minimum throttle (held down: contact substeps every tick). Pieces per call: `aero_forces` at M 12 and 1.2 (q 20 kPa), `thermal::cell_heat`, one 20 ms `ThermalNetwork::step`.
- *Coast*: µs per integration step (a fresh LEO coast); one 600 s lattice solve from 290 K in sunlight, and the 10th in a row (with sweeps); a tumbling day in LEO per lattice point; 10⁶ s in LEO holding attitude from 290 K (the settling dominates). Both vessel cases first advance one second (a new coast restarts on the pointing of its controls) and integrate ahead, so they time attitude, sunlight and thermal only.
- *Fleets*: the game's frame loop as `crates/game/src/state.rs` `advance`: every vessel advanced with the game's 1,200-unit budget, the clock held at the slowest, the active vessel's and one other's look-ahead of 6,000 s with 1,500 and 300 steps, rails warp down to 4× while a vessel is live. 600 frames of 1/60 s at 1× and 1,000×, 240 at 10⁶×; in turn, and on 4 threads (Bevy's compute pool on this machine: io 3, async 3, compute 4; threads spawned per frame). **1:** a 300-part craft in LEO holding attitude. **10:** ten limbo stand-ins, tumbling with SAS off (LIMBO adds the continuous forces), alternately 300-part and test craft, two on lunar transfers (perigee 200 km, apogee 400,000 km). **50:** those ten plus 40 quiet craft holding attitude with SAS on (the game's passive controls): 20 test craft and 15 300-part craft in LEO (300–500 km), 4 on lunar transfers, a test craft on the pad. Vessel 0 (300 parts, LEO) is active. **Settled:** the same fleet with 20,000 km orbits for the lunar transfers, every vessel first advanced 30 days (2 s, 16 s and 72 s of work for 1, 10 and 50). One run per row; three full runs at earlier commits agreed within 15 %.

## The stress craft

`sim::craft::stress::layout(n)` lays out exactly n parts from the existing primitives (see the module): a core stack (engine, tanks with decouplers, avionics rings, pod, nose), four fins and legs, rings of radial boosters (ring k: 4k boosters), small parts in quarter-turn-symmetric sets (thin-disc panels, RCS blocks, batteries, antennas). Each part names its primitives, its parent and placeholder properties (dry mass and centre, propellant capacity, heat capacity and limit, thrust and Isp, power produced and used). The craft file aggregates them for today's one-part pipeline: one tank (the core's section), one engine of the summed thrust, one nozzle (the core bell). `stress_craft(n)` builds with `cell_options(n)`: 16 cells per part, at least 512.

| Parts | Primitives | Triangles | Cells (tubes, their area) | Default 512: cells (tubes, area) | Nodes | Node links | Contacts: feet + hull | Fins found | Height × width | Shell CoM vs parts' |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 1 | 4,914 | 512 (0) | 512 (0) | 27 | 54 | 0 + 114 | 0 | 3.0 × 3.0 m | 0.00 m |
| 50 | 59 | 38,017 | 800 (64, 2.2 %) | 512 (52, 3.7 %) | 177 | 358 | 4 + 44 | 12 | 11.4 × 9.1 m | 0.46 m |
| 300 | 317 | 198,192 | 4,800 (264, 1.5 %) | 858 (232, **88.6 %**) | 1,242 | 2,881 | 4 + 62 | 84 | 29.3 × 12.6 m | 0.67 m |
| 1,000 | 1,029 | 680,577 | 16,000 (702, 1.0 %) | 2,688 (760, **92.4 %**) | 5,127 | 12,908 | 4 + 89 | 272 | 61.4 × 16.1 m | 0.96 m |

The test craft for comparison (`aero-thermal.md`): 17 primitives, 512 cells, 110 nodes.

## Per part count (1f39836)

Build, bake and memory (median of 3):

| Parts | Union surface | Build | Aero bake | Volume grid | Design | Mean drag area | Design | Craft | Vessel | Save |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 0.1 ms | 7.7 ms | 118 ms | 0.0 ms | 125 ms | 2.7 ms | 734 KB | 459 KB | 9.6 KB | 17 KB |
| 50 | 7.5 ms | 42 ms | 395 ms | 10 ms | 416 ms | 4.6 ms | 1.1 MB | 3.6 MB | 10.9 KB | 15 KB |
| 300 | 157 ms | 369 ms | 1.79 s | 199 ms | 2.04 s | 23 ms | 5.7 MB | 18.5 MB | 51 KB | 46 KB |
| 1,000 | 1.63 s | 2.47 s | 9.4 s | 2.25 s | 13.5 s | 66 ms | 18.7 MB | 63.6 MB | 170 KB | 136 KB |

Live ticks (20 ms, µs per tick) and their pieces (µs per call):

| Parts | Entry | Ascent M 1.2 | Vacuum burn | Pad (contact) | `aero_forces` M 12 | M 1.2 | `cell_heat` | Thermal step 20 ms |
|---|---|---|---|---|---|---|---|---|
| 1 | 159 | 245 | 28 | 919 | 24 | 43 | 4.1 | 23 |
| 50 | 238 | 384 | 46 | 694 | 41 | 79 | 6.0 | 42 |
| 300 | 1,343 | 2,281 | 327 | 2,966 | 225 | 419 | 29 | 287 |
| 1,000 | 4,909 | 7,773 | 1,184 | 9,973 | 795 | 1,434 | 99 | 1,062 |

A free-flight tick evaluates `aero_forces` 5 times (`AeroTick::drag_and_lift` and the 4 RK4 stages of the rotation), a contact tick 11 times (once per substep, and the drag): most of the ascent's 2.3 ms at 300 parts. The 1-part cylinder's 114 hull contact points make its pad tick dearer than the 50-part craft's (48 points).

Coast:

| Parts | Integration step | Lattice solve 600 s, first (sweeps) | 10th (sweeps) | Tumbling day, per lattice point | 10⁶ s holding attitude |
|---|---|---|---|---|---|
| 1 | 6.1 µs | 196 µs (20) | 132 µs (16) | 41 µs | 37 ms |
| 50 | 6.1 µs | 415 µs (22) | 255 µs (17) | 324 µs | 89 ms |
| 300 | 6.1 µs | 2.6 ms (24) | 1.9 ms (18) | 1.3 ms | 691 ms |
| 1,000 | 6.3 µs | 9.2 ms (24) | 6.3 ms (18) | 4.8 ms | 3.2 s |

## Fleets: the game's frame loop (1f39836)

Frame times are advance plus look-ahead; "at full warp" leaves out the frames that began with a vessel live (run at 4×).

| Fleet | Warp | Frame in turn: mean at full warp, over all (worst) | Look-ahead | Compute-limited | At 4× (a vessel live) | Warp reached | Slowest single advance | 4 threads: mean at full warp, over all (worst) | Vessels' memory |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1× | 0.00 ms (0.6) | 0.00 ms | 0/600 | 0 | 1× | 0.0 ms | 0.05 ms (0.8) | 61 KB |
| 1 | 1,000× | 0.06 ms (2.7) | 0.00 ms | 0/600 | 0 | 1,000× | 2.7 ms | 0.11 ms (2.6) | 61 KB |
| 1 | 10⁶× | 16.8 ms (94) | 0.74 ms | 0/240 | 0 | 10⁶× | 94 ms | 16.0 ms (97) | 61 KB |
| 1 settled | 1,000× | 0.01 ms (2.0) | 0.00 ms | 0/600 | 0 | 1,000× | 2.0 ms | 0.14 ms (7.1) | 60 KB |
| 1 settled | 10⁶× | 11.1 ms (25) | 0.59 ms | 0/240 | 0 | 10⁶× | 24 ms | 11.0 ms (24) | 60 KB |
| 10 | 1× | 0.08 ms (9.4) | 0.04 ms | 0/600 | 0 | 1× | 0.2 ms | 0.11 ms (9.5) | 384 KB |
| 10 | 1,000× | 0.47 ms (15) | 0.09 ms | 0/600 | 0 | 1,000× | 2.8 ms | 0.29 ms (9.2) | 384 KB |
| 10 | 10⁶× | 84 ms (365) | 4.7 ms | 0/240 | 0 | 10⁶× | 85 ms (#6: 300 parts, tumbling) | 51 ms (133) | 384 KB |
| 10 settled | 1,000× | 0.13 ms (1.3) | 0.07 ms | 0/600 | 0 | 1,000× | 0.6 ms | 0.40 ms (10) | 375 KB |
| 10 settled | 10⁶× | 106 ms (333) | 7.2 ms | 0/240 | 0 | 10⁶× | 135 ms (#6) | 46 ms (75) | 375 KB |
| 50 | 1× | 0.16 ms (9.7) | 0.07 ms | 0/600 | 0 | 1× | 0.7 ms | 0.22 ms (9.7) | 1.8 MB |
| 50 | 1,000× | 1.68 ms (66) | 0.19 ms | 0/600 | 0 | 1,000× | 5.1 ms | 0.87 ms (25) | 1.8 MB |
| 50 | 10⁶× | 463 ms, 104 ms (1,497) | 0.84 ms | 2/240 | 188 | 213,680× | 106 ms (#0) | 138 ms, 34 ms (560) | 1.8 MB |
| 50 settled | 1,000× | 0.43 ms (19) | 0.12 ms | 0/600 | 0 | 1,000× | 6.0 ms | 0.70 ms (12) | 1.8 MB |
| 50 settled | 10⁶× | 377 ms (1,334) | 3.9 ms | 0/240 | 0 | 10⁶× | 207 ms (#42: 300 parts, quiet) | 106 ms (165) | 1.8 MB |

"Warp reached" is the simulated time over the frames' 1/60 s each, not wall time: the clock fell behind only while a vessel was live, the frames simply took longer. In the 50-craft run vessel #47 (300 parts, lunar transfer) is turned back by the Moon and reaches Earth's air after 52 frames; from then on the warp is 4× (the game's `RailsBlock::OtherLive`).

## Hot spots (profiles)

1. **The thermal network solve**, `sim::thermal::ThermalNetwork::step_to` and `cell_temperature` (`crates/sim/src/thermal/mod.rs`): 84 % of a coast lattice point at 300 parts (50 % the per-cell Newton on the quartic, 34 % the Gauss–Seidel sweep), 24 % of a live tick, 64–82 % of the fleet frames at 10⁶× before LIMBO's averaged sunlight (869fa4b, 526c411) and still 18 % of the settled 50-craft frame. Cost = (cells + nodes) × sweeps: 6,042 unknowns × 18–24 sweeps at 600 s ≈ 20 ns an update. Ideas: for limbo craft a reduced network (a few nodes per part: D081 allows extrapolated models, the lattice rule stays); fewer sweeps (an ordering or acceleration, warm starts from the last step's rate); one linearised radiation update per sweep instead of Newton to convergence; budget units from (cells + nodes) × sweeps.
2. **The perturbed coast integration at rails warp**: the `Dopri5` steps of every coast with `sim::vessel::limbo`'s forces (drag at the pointing's attitude, light with eclipses), `sim::vessel::pointing`'s attitude at every stage (torque-free tumbling through `rigid::elliptic`, orbital-frame holds through trig), and the ephemeris (`ephem::chebyshev` and body rotations): 55 % inclusive of the settled 50-craft frame at 10⁶× (limbo forces 29 %, pointing 20 %, snapshots 23 %, overlapping); ~4 ms a frame per LEO vessel at 10⁶×, part-count independent, so it is the floor for 50 craft (≈ 200 ms in turn, 50 ms on 4 threads). Ideas: larger steps where the perturbation is smooth (error control on the perturbing acceleration's own scale), the pointing evaluated once per step rather than per stage, cached snapshots shared by the vessels of a frame.
3. **Skin friction and the boundary layer per cell**, `sim::aero::friction::{friction, layers}` and `sim::aero::boundary` (`crates/sim/src/aero/`, D085): ~57 % of a live entry tick and ~59 % of a pad tick at 300 parts, evaluated 5× a free tick and 11× a contact tick, on the pad at zero wind too (`AeroTick::new` in `crates/sim/src/vessel/aerothermal.rs` skips only an exactly zero speed). AERO's 5749414 provides `aero_forces_layer` to compute the layer once per tick; `vessel::aerothermal` does not use it yet (LIMBO's file). Also: no aerodynamics below a dynamic-pressure floor.
4. **The bake's passes over every triangle**, `sim::aero::bake::bake` (`crates/sim/src/aero/bake.rs`), for each of 642 directions: rasterising every front-facing triangle into the 64 × 64 z-buffer and projecting every vertex (~45 % of the design at 300 parts), `sim::aero::wave::area_along` (17 %), and AERO's `sim::aero::relief::ratios` (21 %). At 680,000 triangles nearly all are below a pixel. Ideas: bake from the cells or a decimated surface (16,000 items, not 680,000); the area distribution from per-cell sums. (`find_fins`, O(cells × triangles), was 62–65 % of the bake until AERO's 5749414.)
5. **Geometry scans against every primitive**: `sim::craft::mesh::union_surface` and `contains` (`crates/sim/src/craft/mesh.rs`) are 70 % of the craft build at 1,000 parts (every triangle against every primitive), and the same scan makes `VolumeGrid::new` (`crates/sim/src/craft/volume.rs`) 2.2 s at 1,000 parts (64 samples per cube); `cells::build_cells` (`crates/sim/src/craft/cells.rs`) searches all groups for the worst at every split (O(target²), 10 %), `link_neighbours` 7.5 %. Ideas: a bounding-box broad phase per primitive; a heap for the split queue.

Also seen: `sim::contact::forces` samples the terrain per contact point per substep (62–89 hull points × 10 substeps; `terrain::noise` is 6–7 % of a pad tick); `vessel::aerothermal::live_step` and `coast_step` allocate their buffers every call.

## Pipeline problems found (not fixed here: other workstreams' files)

1. **Cells collapse on large craft.** `CellOptions::default()` (target 512, at least one cell per face) gives a 300-part craft one cell per face: whole tank sides become tube cells whose normals cancel (89 % of the area; 92 % at 1,000 parts). Their pressure, shadowing, sunlight and radiation use a meaningless normal. `parse_craft` uses the default for every craft. PARTS: cells per part by area (the stress craft use `stress::cell_options`, 16 per part).
2. **Thin tubes are split into rings.** `cells` splits a group along its longest extent, so a strut (legs, antennas, booster struts) becomes rings whose normals cancel, on the test craft too (its legs: coherence 0.015). A split across the circumference when the normals spread widely would fix it (`crates/sim/src/craft/cells.rs`, `Tris::split`).
3. **A stress vessel cannot be reloaded.** `design_of` (`crates/sim/src/craft/design.rs`) rebuilds a design from `data/craft/<id>` and panics for any other id, so saves of stress vessels do not load: PERF's "save/load at scale equal bit for bit" needs a hook (designs registered by id, or a resolver for `stress-N`).
4. **The coast budget does not scale with the craft.** `THERMAL_SOLVE_UNITS` (`crates/sim/src/vessel/coast.rs`) is a constant 30; frames of 0.1–0.5 s are never compute-limited. Units proportional to (cells + nodes) × sweeps, or a time budget, would let the game hold its clock back as designed.
5. **Live ticks inside a rails frame.** Fixed for the air by LIMBO's 026080b (`Vessel::advance` returns at the live boundary; at 869fa4b one frame took 16–18 s). Still open: a coast that ends on a surface (an impact on the Moon, a landing) or an engine start flies the rest of the frame live inside the call (`advance_powered` to the target: up to 833,000 ticks at 10⁶×).
6. **The stress craft's limits** (the one-part pipeline): one tank region (the boosters' propellant sits in the core), one engine and one nozzle (the boosters' bells take no engine heat), the shell mass model's centre of mass 0.5–1 m from the parts' (the skin carries 30 % of each part's mass over its area; hidden faces are removed). The part layer replaces all three.
