# Scale baseline, 2026-10-26 (D081)

> Status: **measurement** (the BENCH workstream of [overnight-2026-10-26.md](overnight-2026-10-26.md)); it feeds PERF and PARTS. Craft: `sim::craft::stress` (synthetic, benchmarks and tests only; craft creation is the owner's to guide). Bench: `crates/sim/examples/bench_scale.rs`. Measured at commit 869fa4b: AERO's boundary layer and whole-body heating are in, LIMBO's surface table is in the design; LIMBO's perturbed coasts were not committed yet and are not measured.

D081 asks for 50 craft of ~300 parts at every warp up to 1,000,000×, live physics for 1–3 craft at low warp, and 10 or more craft in limbo. The table below says where each cost curve breaks against that; the details, the hot spots and the pipeline problems follow.

## Where each curve breaks

Frame: 16.7 ms (60 fps, D028). A live craft runs 0.83 ticks per frame at 1× and 3.3 at 4× (the physics-warp limit, D029). At 10⁶× a frame spans 16,667 s: 28 points of the 600 s coast thermal lattice.

| Cost | 300 parts | 1,000 parts | Against D081 |
|---|---|---|---|
| Live tick, ascent (M 1.2, the dearest in flight) | 2.2 ms | 7.0 ms | **3 live 300-part craft fit at 1× (5.5 ms a frame), not at 4× (22 ms in turn; 7.3 ms if each runs on its own thread).** One 1,000-part craft at 4× takes 23 ms: breaks; at 1× 5.8 ms. |
| Live tick on the pad (contact substeps) | 2.6–3.3 ms | 8.8–9.5 ms | As above, worse: three on pads at 4× is 26–33 ms. Most of it is aerodynamics at zero wind (hot spot 4). |
| Live tick, entry / vacuum burn | 1.2–1.5 / 0.32–0.46 ms | 4.0–4.3 / 1.2 ms | Entry as the ascent, a little cheaper; vacuum fits everywhere. |
| Design (aero bake, volume grid, network, drag) | 4.5–5.7 s | 39–44 s | **Breakups break**: every new piece needs a design. PARTS' "a 300-part craft splits in under a frame budget" is ~200× away (two ~150-part designs ≈ 2 × 1.7 s; the bake grows as n^1.4–1.9). Small debris too: 0.1 s (1 part) to 0.4 s (50 parts) each. Fine once per design at load, off the frame thread. |
| Craft build (union surface, cells, mass) | 0.38–0.50 s | 2.5 s | Also per split; ~25× a frame at 300 parts. |
| Coast lattice, tumbling (a solve at every point) | 2.6–3.7 ms a point | 9.1–9.2 ms a point | **Limbo breaks at 10⁶×**: 73–104 ms a frame per 300-part craft, 0.7–1.0 s for ten in turn (44–62× the frame), 11–16× on 4 threads. Measured: 10 limbo craft (5 × 300 parts, 5 test craft) 430–527 ms a frame in turn, 155 ms on 4 threads. |
| Coast lattice, holding attitude (skips) | 680–752 ms per 10⁶ s | 3.3–4.5 s per 10⁶ s | 11–12.5 ms a frame per 300-part craft at 10⁶× (measured 12.5 ms): **50 craft ≈ 0.6 s a frame.** Measured: 50 craft 772–926 ms a frame in turn, 255–267 ms on 4 threads (46–55× / 15–16× the frame). |
| Work budget | — | — | **Never bites**: 0 of 240 frames compute-limited at 10⁶× while frames take 0.4–0.8 s. A coast thermal solve counts 30 units (`THERMAL_SOLVE_UNITS`, ≈ 200 µs, the test craft's); a 300-part solve costs 1.9–2.7 ms. The game would show 10⁶× at 1–3 fps instead of holding its clock back. |
| A vessel going live inside a rails frame | — | — | **Breaks**: a 300-part craft on a lunar transfer, turned back by the Moon, entered Earth's atmosphere and crashed on Earth within one 10⁶× frame: 16–18 s for that frame (its whole entry in unbudgeted live ticks). The game caps rails warp only from the next frame (`RailsBlock::OtherLive`). |
| Coast integration step | 3.5 µs | 3.5 µs | Independent of the part count (a point mass); no break. |
| Fleet at 1× and 1,000× | — | — | 50 craft: 0.15–0.18 ms a frame at 1× (no live craft in the fleets), 1.7–4.2 ms at 1,000× with 70–250 ms spikes (the vessels' 600 s lattice points fall on the same frames); 0.6–0.9 ms (24 ms worst) on 4 threads. |
| Memory | design 5.7 MB, craft 18.5 MB, vessel 51 KB | 18.7 MB, 64 MB, 170 KB | 50 vessels sharing one design: 1.8 MB. **Breaks for debris** if each piece keeps its own craft and design: 50 distinct 300-part designs ≈ 1.2 GB (the f64 surface and f32 render mesh are 18.5 MB of it). |

## Machine and method

- **Machine:** Mac14,9 (Apple M2 Pro, 6 performance + 4 efficiency cores, 16 GB; the D028 reference), macOS 15.5, rustc 1.98.1, release builds.
- **Load:** other agents compiled and tested all night: load averages 8–10 on 10 cores during every run. Each timing is the median of 3 runs; separate runs of the same cases differed by up to ±20 % (the 300-part bake 4.3–5.6 s, its pad tick 2.6–3.3 ms, the 50-craft frame at 10⁶× 772–926 ms). The tables are one run; the summary gives the range over two or three. Read every number as ±20 %.
- **Rerun:** `cargo run -p sim --release --example bench_scale` (both sections, ~25 min), or `… craft parts=300 runs=1`, `… fleet fleets=10`.
- **Profiles:** no `samply` or `cargo instruments` on the machine; macOS `sample` on `bench_scale profile=<bake|design|build|live|contact|lattice|fleet>,<parts>` (each repeats one measurement for 30 s), built with `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only` and symbolicated with `atos -i` on a `dsymutil` dSYM (inlined frames to source lines). Percentages are self samples (15–25 s at 1 kHz).

What each number includes:

- *Union surface*: `mesh::union_surface`. *Build*: `Craft::build` (union surface, `cells::build_cells`, `MassModel::new`). *Aero bake*: `aero::bake` (642 directions). *Volume grid*: `VolumeGrid::new`. *Design*: `CraftDesign::new` (bake, grid, thermal network, limbo surface table, mean drag area); the network and the surface table are under 1 % each of it in the profile. *Mean drag area*: `design::mean_drag_area` (642 force evaluations).
- *Cells (tubes)*: cells whose triangles' normals nearly cancel (|Σ n·a| < ½ Σ a); *default 512*: the same surface with `CellOptions::default()`, as `parse_craft` builds every craft today.
- *Memory*: heap arrays by length × element size (design: exposure, cell geometry, network, volume nodes and tank samples, cells, surface table; the volume grid's private index estimated); *craft*: the f64 surface, the f32 render mesh, cells; *vessel*: `Vessel` plus thermal state, trajectory samples and contact points (the design is shared); *save*: the vessel as RON.
- *Live ticks* (debug mode: infinite propellant, no destruction; SAS on): `Vessel::advance` over 4 s (300 and 1,000 parts) or 20 s: **entry** at 100 km, 7.6 km/s, 1.5° down; **ascent** at 12 km, 354 m/s through the turning air, climbing (M 1.2: wave drag, slender body, fins; an earlier run that left out the air's rotation flew M 0.55 and cost within 7 % of it); **vacuum** at 400 km, full thrust; **pad** at the game's pad, engine at minimum throttle (held down, contact substeps every tick). Pieces per call: `aero_forces` at M 12 and 1.2 (q 20 kPa), `thermal::cell_heat`, one 20 ms `ThermalNetwork::step`.
- *Coast*: µs per integration step (LEO); one 600 s lattice solve from 290 K in sunlight, and the 10th in a row (with sweeps); a tumbling day in LEO per lattice point (coast integrated ahead, so attitude, sunlight and thermal only); 10⁶ s in LEO holding attitude from 290 K (the settling dominates).
- *Fleets*: the game's frame loop as `crates/game/src/state.rs` `advance` (every vessel advanced with the game's 1,200-unit budget, the clock held at the slowest, the active vessel's and one other's look-ahead of 6,000 s with 1,500 and 300 steps, rails warp down to 4× while a vessel is live), 600 frames of 1/60 s at 1× and 1,000×, 240 at 10⁶×; in turn, and on 4 threads (Bevy's compute pool on this machine: io 3, async 3, compute 4; threads spawned per frame). **1 craft:** a 300-part craft in LEO holding attitude. **10:** ten limbo stand-ins: tumbling with SAS off (no skipped lattice points; LIMBO adds the continuous forces), alternately 300-part and test craft, two on lunar transfers (perigee 200 km, apogee 400,000 km). **50:** those ten plus 40 quiet craft holding attitude with SAS on (the game's passive controls): 20 test craft and 15 300-part craft in LEO (300–500 km), 4 on lunar transfers, a test craft on the pad. Vessel 0 (300 parts, LEO) is active. One run per row (two full runs agreed within 15 %).

## The stress craft

`sim::craft::stress::layout(n)` lays out exactly n parts from the existing primitives (see the module): a core stack (engine, tanks with decouplers, avionics rings, pod, nose), four fins and legs, rings of radial boosters (ring k: 4k boosters), small parts in quarter-turn-symmetric sets (thin-disc panels, RCS blocks, batteries, antennas). Each part names its primitives, its parent and placeholder properties (dry mass and centre, propellant capacity, heat capacity and limit, thrust and Isp, power produced and used). The craft file aggregates them for today's one-part pipeline: one tank (the core's section), one engine of the summed thrust, one nozzle (the core bell). `stress_craft(n)` builds with `cell_options(n)`: 16 cells per part, at least 512.

| Parts | Primitives | Triangles | Cells (tubes, their area) | Default 512: cells (tubes, area) | Nodes | Node links | Contacts: feet + hull | Fins found | Height × width | Shell CoM vs parts' |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 1 | 4,914 | 512 (0) | 512 (0) | 27 | 54 | 0 + 114 | 0 | 3.0 × 3.0 m | 0.00 m |
| 50 | 59 | 38,017 | 800 (64, 2.2 %) | 512 (52, 3.7 %) | 177 | 358 | 4 + 44 | 12 | 11.4 × 9.1 m | 0.46 m |
| 300 | 317 | 198,192 | 4,800 (264, 1.5 %) | 858 (232, **88.6 %**) | 1,242 | 2,881 | 4 + 62 | 84 | 29.3 × 12.6 m | 0.67 m |
| 1,000 | 1,029 | 680,577 | 16,000 (702, 1.0 %) | 2,688 (760, **92.4 %**) | 5,127 | 12,908 | 4 + 89 | 272 | 61.4 × 16.1 m | 0.96 m |

The test craft for comparison (`aero-thermal.md`): 17 primitives, 512 cells, 110 nodes, bake 193 ms.

## Per part count

Build, bake and memory (median of 3):

| Parts | Union surface | Build | Aero bake | Volume grid | Design | Mean drag area | Design | Craft | Vessel | Save |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | 0.1 ms | 7.8 ms | 109 ms | 0.0 ms | 116 ms | 2.8 ms | 734 KB | 459 KB | 9.4 KB | 16 KB |
| 50 | 7.4 ms | 42 ms | 400 ms | 10 ms | 423 ms | 4.9 ms | 1.1 MB | 3.6 MB | 10.8 KB | 14 KB |
| 300 | 160 ms | 379 ms | 5.6 s | 215 ms | 4.6 s | 25 ms | 5.7 MB | 18.5 MB | 51 KB | 45 KB |
| 1,000 | 1.67 s | 2.5 s | 37.6 s | 2.2 s | 39.3 s | 69 ms | 18.7 MB | 63.6 MB | 170 KB | 136 KB |

(The bake alone above the whole design at 300 parts is the load's noise: in the profile the design is ~95 % bake and ~3 % volume grid; the thermal network and the surface table are under 1 % each.)

Live ticks (20 ms, µs per tick) and their pieces (µs per call):

| Parts | Entry | Ascent M 1.2 | Vacuum burn | Pad (contact) | `aero_forces` M 12 | M 1.2 | `cell_heat` | Thermal step 20 ms |
|---|---|---|---|---|---|---|---|---|
| 1 | 134 | 241 | 28 | 870 | 20 | 40 | 4.1 | 22 |
| 50 | 203 | 359 | 46 | 607 | 35 | 70 | 5.9 | 43 |
| 300 | 1,219 | 2,212 | 359 | 2,604 | 191 | 386 | 35 | 292 |
| 1,000 | 4,317 | 7,007 | 1,182 | 9,539 | 688 | 1,284 | 96 | 1,054 |

A free-flight tick evaluates `aero_forces` 5 times (`AeroTick::drag_and_lift` and the 4 RK4 stages of the rotation), a contact tick 11 times (once per substep, and the drag): most of the ascent's 2.2 ms at 300 parts. The 1-part cylinder's 114 hull contact points make its pad tick dearer than the 50-part craft's (48 points).

Coast:

| Parts | Integration step | Lattice solve 600 s, first (sweeps) | 10th (sweeps) | Tumbling, per lattice point | 10⁶ s holding attitude |
|---|---|---|---|---|---|
| 1 | 3.5 µs | 227 µs (20) | 129 µs (16) | 201 µs | 19 ms |
| 50 | 3.5 µs | 385 µs (22) | 264 µs (17) | 479 µs | 70 ms |
| 300 | 3.5 µs | 2.8 ms (24) | 1.8 ms (18) | 2.6 ms | 680 ms |
| 1,000 | 3.5 µs | 8.8 ms (24) | 6.4 ms (18) | 9.1 ms | 4.5 s |

## Fleets: the game's frame loop

| Fleet | Warp | Frame in turn: mean (worst) | Look-ahead | Compute-limited | At 4× (a vessel live) | Warp reached | Slowest single advance | 4 threads: mean (worst) | Vessels' memory |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1× | 0.00 ms (0.0) | 0.00 ms | 0/600 | 0 | 1× | 0.0 ms | 0.06 ms (0.1) | 61 KB |
| 1 | 1,000× | 0.05 ms (2.5) | 0.00 ms | 0/600 | 0 | 1,000× | 2.5 ms | 0.11 ms (2.6) | 61 KB |
| 1 | 10⁶× | 12.5 ms (48.8) | 0.36 ms | 0/240 | 0 | 10⁶× | 48 ms | 12.4 ms (56.6) | 61 KB |
| 10 | 1× | 0.13 ms (0.2) | 0.00 ms | 0/600 | 0 | 1× | 0.1 ms | 0.12 ms (0.2) | 382 KB |
| 10 | 1,000× | 0.53 ms (14.9) | 0.01 ms | 0/600 | 0 | 1,000× | 3.1 ms | 0.44 ms (11.8) | 382 KB |
| 10 | 10⁶× | 430 ms (1,076) | 0.68 ms | 0/240 | 0 | 10⁶× | 261 ms (#0, 300 parts, tumbling) | 155 ms (194) | 382 KB |
| 50 | 1× | 0.15 ms (0.2) | 0.00 ms | 0/600 | 0 | 1× | 0.1 ms | 0.14 ms (0.3) | 1.8 MB |
| 50 | 1,000× | 2.2 ms (252) | 0.05 ms | 0/600 | 0 | 1,000× | 53 ms (#0) | 0.62 ms (24) | 1.8 MB |
| 50 | 10⁶× | 772 ms (16,549) | 0.69 ms | 0/240 | 0 | 10⁶× | 15.9 s (#47, crashed on Earth) | 267 ms (15,810) | 1.8 MB |

"Warp reached" is the simulated time over the frames' 1/60 s each, not wall time: the clock never fell behind, the frames simply took longer. #47 is a 300-part craft on a lunar transfer: turned back by the Moon, it entered Earth's atmosphere and crashed within one frame (a second run: 17.6 s). "At 4×" counts frames that began with a vessel live: none, because #47 went live and came to rest inside the same frame.

## Hot spots (profiles)

1. **The thermal network solve**, `sim::thermal::ThermalNetwork::step_to` and `cell_temperature` (`crates/sim/src/thermal/mod.rs`). 84 % of a coast lattice point at 300 parts (50 % the per-cell Newton on the quartic, 34 % the Gauss–Seidel sweep itself), **64 % of the 50-craft frame at 10⁶×**, 24 % of a live entry tick. Cost = (cells + nodes) × sweeps: 6,042 unknowns × 18–24 sweeps at 600 s ≈ 20 ns an update. Ideas: for limbo craft a reduced network (a few nodes per part: D081 allows extrapolated models, the lattice rule stays); fewer sweeps (a better ordering or acceleration, warm starts from the last step's rate); one linearised radiation update per sweep instead of Newton to convergence; budget units from (cells + nodes) × sweeps.
2. **`sim::aero::bake::find_fins`** (`crates/sim/src/aero/bake.rs`): 62–65 % of the bake. Every cell ray-casts against every surface triangle (O(cells × triangles): 950 million tests at 300 parts, 11 billion at 1,000), including the cross product and length of every triangle. Ideas: test only the cell's own primitive's triangles (the opposite face of a fin is its own part), or a grid or BVH; with parts, bake per part.
3. **The bake's per-direction loop**, `sim::aero::bake::bake`: rasterising every front-facing triangle into the 64 × 64 z-buffer (`Raster::fill`, 16 %), projecting every vertex (7 %) and `sim::aero::wave::area_along` over every triangle (7–10 %), each × 642 directions. At 680,000 triangles nearly all are below a pixel. Ideas: bake from the cells or a decimated surface (16,000 items, not 680,000); the area distribution from per-cell sums.
4. **Skin friction and the boundary layer per cell**, `sim::aero::friction::{friction, layers}` and `sim::aero::boundary` (`crates/sim/src/aero/`, D085): ~55 % of a live tick at 300 parts (logarithms, square roots, exponentials per wetted cell per evaluation), evaluated 5× a free tick and 11× a contact tick; on the pad at zero wind too, since `AeroTick::new` (`crates/sim/src/vessel/aerothermal.rs`) skips only an exactly zero speed. Ideas: the friction and layer once per tick, reused at the RK4 stages and substeps (the attitude moves little within 20 ms); no aerodynamics below a dynamic-pressure floor.
5. **`sim::craft::mesh::union_surface` and `contains`** (`crates/sim/src/craft/mesh.rs`): 70 % of the craft build at 1,000 parts; every triangle against every primitive (O(triangles × primitives)). The same scan makes `VolumeGrid::new` (`crates/sim/src/craft/volume.rs`) 2.2 s at 1,000 parts (64 samples per cube against every primitive). Also `cells::build_cells` (`crates/sim/src/craft/cells.rs`, 10 %: a linear search for the worst group at every split, O(target²)) and `link_neighbours` (7.5 %). Ideas: a bounding-box broad phase per primitive; a heap for the split queue.

Also seen: `sim::contact::forces` samples the terrain per contact point per substep (62–89 hull points × 10 substeps; `terrain::noise` is 7 % of a pad tick); `vessel::aerothermal::live_step` and `coast_step` allocate their buffers every call.

## Pipeline problems found (not fixed here: other workstreams' files)

1. **Cells collapse on large craft.** `CellOptions::default()` (target 512, at least one cell per face) gives a 300-part craft one cell per face: whole tank sides become tube cells whose normals cancel (89 % of the area; 92 % at 1,000 parts). Their pressure, shadowing, sunlight and radiation use a meaningless normal. `parse_craft` uses the default for every craft. PARTS: cells per part by area (the stress craft use `stress::cell_options`, 16 per part).
2. **Thin tubes are split into rings.** `cells` splits a group along its longest extent, so a strut (legs, antennas, booster struts) becomes rings whose normals cancel, on the test craft too (its legs: coherence 0.015). A split across the circumference when the normals spread widely would fix it (`crates/sim/src/craft/cells.rs`, `Tris::split`).
3. **A stress vessel cannot be reloaded.** `design_of` (`crates/sim/src/craft/design.rs`) rebuilds a design from `data/craft/<id>` and panics for any other id, so saves of stress vessels do not load: PERF's "save/load at scale equal bit for bit" needs a hook (designs registered by id, or a resolver for `stress-N`).
4. **The coast budget does not scale with the craft.** `THERMAL_SOLVE_UNITS` (`crates/sim/src/vessel/coast.rs`) is a constant 30; frames of 0.4–0.8 s are never compute-limited. Units proportional to (cells + nodes) × sweeps, or a time budget, would let the game hold its clock back as designed.
5. **Live ticks are not budgeted at rails warp.** A vessel whose coast reaches an atmosphere or a surface inside a rails frame flies the rest of that frame live (`Vessel::advance` → `advance_powered` until the target: up to 833,000 ticks at 10⁶×). Seen: one 16 s frame. Stopping the advance where the vessel goes live (the game then caps the warp) would bound it (LIMBO's and PERF's: the warp cap of D081).
6. **The stress craft's limits** (the one-part pipeline): one tank region (the boosters' propellant sits in the core), one engine and one nozzle (the boosters' bells take no engine heat), the shell mass model's centre of mass 0.5–1 m from the parts' (the skin carries 30 % of each part's mass over its area; hidden faces are removed). The part layer replaces all three.
