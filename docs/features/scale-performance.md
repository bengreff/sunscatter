# Feature: scale performance (D081)

**Status:** *playable* in the sim and the game's frame loop, measured headless (the game window and the offscreen demo were not run by this workstream: see [Verify](#verify-in-the-offscreen-demo)). Built 2026-10-26 (overnight run, PERF workstream; the coast integration, §6, COAST; the coast thermal lattice, §7, THERMAL), from BENCH's [scale baseline](../plans/scale-baseline-2026-10-26.md). Decisions: D081 (50 craft of ~300 parts to 1,000,000×; live physics for 1–3, limbo for 10+), D028 (60 fps on the M2 Pro), D022, D025, D026, D029, D062; rules 2 and 4.

The owner (2026-10-26): "maximally stable with dozens of crafts flying each with hundreds of parts", "up to 1 million timewarp".

## What was built

| Commit | What | Before → after (release, M2 Pro, other agents loading the machine: read ±20 %, and see the load notes) |
|---|---|---|
| 5a48b8c | **The thermal network solve** (hot spot 1): block Gauss–Seidel over trees of strongly coupled unknowns (`sim::thermal::blocks`), the radiation linearised once per sweep, unknowns renumbered into solve order | 600 s coast solve: test craft 258 → 42–60 µs (22–24 → 9–10 sweeps), 300-part stress craft 2.6–2.9 → 0.74–0.90 ms; error against the converged step 1–9.5 → 1–2.6 mK; 20 ms live step unchanged (4 sweeps) |
| bc16f5b | **Budgets in units of real cost** (pipeline problem 4) and `Vessel::advance_frame`, which stops where a vessel goes live inside a frame (pipeline problem 5) | a coast thermal solve counted 30 units whatever the craft (0.1–0.8 s frames were never compute-limited); now `solve_units` ≈ its microseconds; a coast ending on a surface flew up to 833,000 live ticks in one 1,000,000× frame, now it stops at the hand-over and the warp is capped |
| 9acb062 | **`VolumeGrid::propellant`** per tank (a regression from the part layer, found profiling the fleet) | `node_capacity` on the 300-part craft 2,654 → 59 µs, at every coast thermal solve and every live tick; it was 77 % of a 300-part craft's 1,000,000× frame |
| bd26aa1 | **The fleet's frame** (`crates/game/src/fleet.rs`): every vessel advanced in chunks of work on the compute pool, the one furthest behind first, within the frame's time for the simulation; 7 compute threads instead of 4; the warp achieved shown when compute limits it | below |
| 866feee | **The long run** (`examples/long_run.rs`), the live-stop tests (`tests/frame_live_stop.rs`), the frame's span ramped (`fleet::frame_span`), this note | below |
| f6ec8bc | **The perturbed coast step, bit for bit** (COAST): starlight, the air search and the event searches no longer evaluate what they discard | perturbed LEO step 14.0 → 9.4 µs, bare 6.2 → 5.6 µs; five coasts' samples hash the same |
| fd22069 | **Coasts integrate with DOP853** (COAST): 8th order, 10 cm and 10 µm/s per step, steps of at most 3 h | LEO: 3× fewer force evaluations, 53 → 10 m from the converged orbit after a day; a craft's integration per 1,000,000× frame 3.4–3.7 → 0.7–0.8 ms |
| 93d50b4 | **The coast thermal solve's heat inputs in one pass** (THERMAL, §7): every sample's exposure first, then each cell's samples summed in one pass; exposure without a division per cell; the nodes' capacities kept between solves. The same bits | a 300-part craft's lattice per 1,000,000× frame 18.3 → 12.5 ms; the 50-craft fleet 244,000× → 309,000× per wall second |
| ff54d20, e2b9213, this | **Gravity fields** (GRAV, D090; costs in [solar-system.md](solar-system.md#cost)): each body's measured field to the degree that can matter at the vessel, steps capped to resolve it, the budgets charging its evaluations (`forces::field_units`, 12 per coast step) | a step per simulated day: LEO 2.3 → 8.0 ms, GEO 0.21 → 0.31 ms, a 100 km lunar orbit 1.9 → 100 ms, Mars 1.4 → 19 ms, heliocentric unchanged; `bench_fleet` (11 craft in LEO): look-ahead 485 → 1,250 ms per 1,000,000 s, the frame loop at 1,000,000× still compute-limited at ~296,000× (its lattices bound it), 8.5 → 11.3 ms a frame |

### 1. The thermal solve

Point Gauss–Seidel crawled on the strut and leg **chains of thin rings**: each ring is coupled ~50 times more strongly to its two neighbours than to its capacity, radiation and node (10 % of the test craft's cells at g/(g + diag) > 0.93; 300-part stress craft: 1 %). A 600 s step from 290 K in sunlight took 97 point sweeps (23 over-relaxed at ω 1.6).

`ThermalNetwork::new` joins the **strong links** (g/√(D_u·D_v) ≥ 0.05, D = C/600 s + Σg; a node's capacity is a step input, so its D is its conductances only) into a spanning forest (Kruskal, strongest first, ties by index). Each tree is a **block**: per sweep, one pass from the leaves eliminates it onto its root (the Schur complement of a tree has no fill), one pass back solves it; its other links take their neighbours' latest values (Gauss–Seidel). Each unknown's radiation is linearised at its current temperature: one Newton step per sweep, from above (the tangent of the convex εσA·T⁴ lies below it). This is block Gauss–Seidel on an M-matrix with a regular splitting: it converges at any dt to the same fixed point, conserves energy to the tolerance, keeps every temperature positive. The unknowns are renumbered into solve order once, so a sweep reads its arrays in sequence.

Tried and dropped: over-relaxation on the blocks (block SOR at ω 1.4 needs 40–50 sweeps and diverged at 3,600 s steps; ω 1 converges in 9–10); a strength threshold of 0.1 or 0.2 (more lagged links, more per-update cost); no threshold (the same sweeps, slower updates).

`examples/bench_thermal` (new) times and checks the solve: first and 10th 600 s steps from 290 K, a settled craft under a turning Sun, 20 ms live steps under entry heating, each with its sweeps, its energy residual and its error against the same step converged to 1e-14; then whole coast lattice points of a vessel in LEO (tumbling, held prograde).

### 2. Budgets that bite

A budget unit is about **a microsecond of work** on the reference Mac (`sim::vessel` constants): an integration step `STEP_UNITS` = 10, a systems lattice step 5, a thermal lattice point passed 1, a thermal solve `solve_units(design, samples)` = 10 + (cells + nodes)/8 + cells × samples/300 (the solve's ~10 sweeps at ~12 ns per unknown, the heat inputs ~3 ns per cell per sample). `Vessel::advance` spends up to its budget on the integration and as much on the lattices, and returns when either runs out; the result is the same as without a budget (rule 4).

Measured (calibration in `fleet_frames`, below): median **1.0–1.2 µs per unit** over the 50-craft fleet; the 300-part craft were 4–8 µs per unit before the `VolumeGrid` fix (the 2.7 ms per solve it cost was not in the estimate).

Found on the way: with budgets that bite, a call could run out of integration steps right after reflying a low-thrust burn at its ignition, end there, and refly it again on the next call from the attitude already snapped onto the law; `Trajectory::refly` is not idempotent (projecting a held attitude again moves the law's roll by a rounding), so the month of low thrust at 1,000,000× drifted 0.16 mm from one jump (the limbo invariance test caught it). `advance` now also stops when the integration budget is spent, so a call never reflies with nothing left; `refly` is now idempotent too (it returns early when the attitude is already the burn pointing's at ignition; tested by reflying again at the ignition for every hold mode and both kinds of law, and by a month of low thrust in frames of one to a few coast steps' budget at 1×, 1,000× and 1,000,000×, which also found a budget stop between the systems step and the thermal point of one epoch: no longer possible).

### 3. Going live inside a frame

`Vessel::advance_frame` (the game's call) stops where a vessel goes live with more than `LIVE_STOP` (1 s) of the frame left: at the live boundary (as before, LIMBO's 026080b), close to a surface (a landing, an impact on the Moon), an engine starting, a landed craft waking. The game holds its clock there and caps the warp next frame (`RailsBlock::OtherLive`, D081), so a rails-warp frame never flies live ticks. `Vessel::advance` keeps flying through (tests and tools that jump far ahead).

### 4. The fleet's frame (game)

`fleet::advance_fleet` replaces one task per vessel per frame:
- Each vessel advances in **chunks** of `CHUNK_UNITS` (2,000 units ≈ 2 ms) by `advance_frame`; a scheduler (`fleet::Queue`, a pure rule with table tests) hands the next chunk to the **vessel furthest behind** (the clock waits for it), lowest index on ties, on as many tasks as the compute pool has threads (Bevy's persistent pool: no thread is spawned per frame). A vessel that reaches the target, goes live, stops for good or makes no progress is done for the frame.
- Until the frame's **time for the simulation** (`SIM_FRAME_TIME`, 8 ms) is used: a chunk under way finishes, nothing new starts. The clock is held at the slowest vessel; the HUD then says "limited: compute, N achieved" with the warp actually achieved (simulated over real seconds, smoothed over half a second), and shows the simulation's milliseconds beside the frame rate.
- A frame asks for at most twice what the clock moved last frame (1.25× while compute limits it; `fleet::frame_span`): vessels that finish first are drawn at their own time, ahead of the clock, and without the cap a compute-limited 1,000,000× frame let them lead it by the whole span (4.6 h, three low orbits); now by at most the clock's progress in the frame where compute first limits it, a quarter of it after. A warp change from 1× to 1,000,000× ramps up over 20 frames (a third of a second).
- The motion does not depend on the threads or the deadline (vessels are independent, D025; chunked = single pass, rule 4; results kept per vessel index, rule 2): `fleet_tests::deadlines_and_threads_change_the_clock_not_the_motion` flies four vessels for two days in 1,000,000× frames cut to 300 µs and ends at the same bits as one call each.
- The app's pools (`fleet::task_pools`): IO 1 thread, async compute 2, compute the rest (7 on the 10-core M2 Pro; Bevy's default gave compute 4).

Spikes at 1,000× (many vessels' lattice points in one frame) are cut by the deadline: the work left over waits for the next frame and the clock lags a few frames by seconds of simulated time. Nothing is computed ahead of the clock beyond what the look-ahead already did.

### 5. The long run

`examples/long_run` flies 50 craft (10 limbo: four 0.5 N ion spirals, three held prograde in thin air at 450–500 km, three tumblers at 20,000 km under radiation pressure; 40 quiet: 15 stress craft of 300 parts, 20 test craft, four on eccentric orbits, one on the pad; stress craft from the craft registry, `stress-300`, so they reload) for simulated years in 1,000,000× frames on 7 threads, as the game does, then saves the fleet to RON, reads it back, and flies both on.

### 6. The coast integration (COAST)

**Where a step's time went** (macOS `sample` of `examples/bench_integrator` `BENCH_PROFILE=<case>`, release with line tables). A perturbed LEO step (`leo_perturbed`: the test craft held prograde at 250 km in thin air, sunlight with eclipses, the full Solar System) cost 14.0 µs for Dopri5's 6 force evaluations: `forces::radiation::starlight` 29 %, `limbo::each_air` 14 %, the live-boundary search 10 %, the ephemeris snapshot at each stage (eager Chebyshev tables) ~10 %, `ForceContext::eval` 22 % (the anchor's kinematics with derivatives, Earth's pole by trigonometry for J2, the walks to each source), the automatic hold reference once per step 4 %. A bare step (6.2 µs): the snapshot 24 %, the anchor's kinematics 13 %, the pole 5 %, the atmosphere-entry search 7.5 % (an allocating snapshot per probe), the per-step source growth 4 %. Integrator overhead, sample storage, the surface table and the drag table were each 1–3 %.

**Bit-for-bit fixes** (f6ec8bc): `starlight` allocated its moon systems' radius table on every call and evaluated the positions of the dwarf planets and of far moon systems' barycentres (lazy Chebyshev tables) at every stage. Now a body or system that keeps farther from the star than the vessel plus its size (`Source::faint_min_distance`, `Cluster::star_distance`) is skipped, because every point between the vessel and the star's disc is within that distance of the star, so it hides nothing. `each_air` evaluated the velocity of every body with air (Venus, Mars) before its distance test, and now evaluates only the position. The entry, contact and live-boundary searches use the segment's snapshot cache, and the live boundary tests the step's end first. Result: perturbed 14.0 → 9.4 µs, bare 6.2 → 5.6 µs. The samples of five coasts (bare and perturbed LEO over a day, GEO 10 days, a lunar transfer, a heliocentric year) hash the same as before.

**The integrator** (fd22069). `bench_integrator` `BENCH_ACCURACY=1` integrates each case against a tight DOP853 reference with the same forces (1 µm per step, steps ≤ 10–600 s). It reports the error at the end and the worst error of the quintic Hermite dense output at step midpoints:

| Case | Dopri5, 1 cm per step (before) | DOP853, 1 cm per step | **DOP853, 10 cm, 10 µm/s, 3 h (adopted)** |
|---|---|---|---|
| LEO, a day | 52.6 m, 8,058 evaluations | 0.43 m, 3,588 | **9.9 m, 2,640** |
| LEO held in thin air, a day | 52.7 m, 8,058 | 1.5 m, 3,588 | **9.9 m, 2,640** |
| GEO, 10 days | 25.7 m, 7,104 | 0.04 m (dense 0.29), 2,976 | **0.38 m (dense 2.0), 2,184** |
| Lunar transfer, 6 days | 501 m, 1,518 | 61 m, 936 | **64 m, 1,044** |
| Heliocentric, a year | 56 m, 8,790 | 114 m, 17,580 (6 h steps) | **14 m, 35,088** |

The answer to "is Dopri5's tolerance tighter than needed": no. Its 1 cm per step left 53 m a day in LEO, because the error is per step and adds up along track. DOP853 at 10× that tolerance is 5× more accurate at a third of the evaluations. 100× drifted 121 m a day, so the adopted tolerance is the loosest that beats the old model everywhere.

On heliocentric arcs the steps sit at the cap. Their error comes from kinks in the ephemeris acceleration at Chebyshev segment boundaries, and it grows with the step: 14 m a year at 3 h, 114 m at 6 h, 446 m at 12 h, 838 m at 24 h. DOP853's large stage coefficients amplify a kink more than Dopri5's do (56 m at 6 h), so the cap is 3 h, not 6. The 4× more evaluations cost 35 ms per simulated year.

Burns flown by ticks keep Dopri5 (one 20 ms step per tick), as does the live translation. A DOP853 step costs 11.0 µs bare and 18.4 µs perturbed (12 evaluations), and covers ~6.5 minutes in LEO (220 steps a day, 1,343 before). `STEP_UNITS` went from 10 to 15.

Invariance:
- Chunked = single pass, bit for bit (a unit test).
- Every anchor-invariance test passes with its bound unchanged.
- One bound was raised: `bubble_tests::relative_position_of_members_10_km_apart`, low orbit, from 1e-6 to 1e-4 m. Its coasting reference pair is inside one 216 s start-up step at 60 s, and the two craft's dense-output errors (~3 cm) nearly cancel to 3.3e-5 m in their separation.
- `COAST_GOLDEN` changed by design.

**Per craft per 1,000,000× frame** (16,667 s, release, the same build otherwise):

| Craft | Integration before → after | Rest of the frame (attitude, thermal and systems lattices) |
|---|---|---|
| Test craft, 300 km (`bench_integrator` `BENCH_FLEET=1`) | 3.72 → 0.70 ms | 2.5 / 2.0 ms |
| 300-part stress craft, 300 km | 3.44 → 0.80 ms | 17.4 / 17.8 ms |
| Limbo, thin air at 250 km, held prograde (`bench_limbo`) | 3.57 → 0.75 ms | 2.0 ms |
| Limbo, 0.5 N ion spiral | 2.51 → 0.51 ms | 1.0 ms |
| Limbo, tumbler at 20,000 km | 0.53 → 0.10 ms | 0.45 ms |

**The 50-craft fleet** (`fleet_frames`, 7 threads, machine quiet). The "before" build is the HEAD of 11:30 UTC with f6ec8bc and fd22069 reverted:

| | Before | After |
|---|---|---|
| 1,000,000×: frame mean (worst) | 6.67 ms (15.0) | 6.59 ms (11.8) |
| compute-limited frames | 107/240 | 119/240 |
| warp reached per 1/60 s frame | 48,000× | 72,500× |
| warp reached per wall second | 120,000× | 183,000× |
| 1,000×: frame mean, warp reached per wall second | 0.48 ms, 34,300× | 0.26 ms, 62,200× |

The fleet does not reach 1,000,000×, and integration no longer bounds it. The 20 craft of 300 parts spend ~18 ms per 1,000,000× frame each in their coast thermal lattice, against 0.8 ms integrating. (§7 brought that to 6–15 ms.) The fleet's heaviest vessels run at 1.0–3.3 µs per budget unit, against ~0.6 µs for the rest, so their solves cost more than `solve_units` estimates. One vessel is advanced by one thread at a time, so a vessel of 18–40 ms per frame holds the clock to 8 ms of its work.

Tried and dropped, and not tried:
- Restructuring `starlight`'s loops (members on demand, skipping a non-shading system at once): the same bits and no measurable gain. Its remaining self time (16 % of a perturbed DOP853 step) is the occlusion tests and `eclipse_factor`'s trigonometry on the night side.
- Not tried: interpolating the bodies within a step (one Chebyshev evaluation with derivatives at the step's end, Hermite at the 10 inner stages). It is estimated at 15–20 % of a step and changes bits at ~1e-9 m.
- Not tried: Encke's method. Its reference orbit per anchor needs a Kepler solve per stage and an Encke dense output.
- Steps are now bounded by the dense output as much as by the integrator. At ~390 s LEO steps the quintic Hermite between samples is good to ~1 m at mid-step (25 µm at the old 65 s steps). That is below the propagation error (10 m a day, 53 before), and it nearly cancels between close craft. Longer steps would need DOP853's 7th-order continuous extension (3 more evaluations a step) or a stored mid-step sample.
- A dip into live air shorter than one step is still found only at step ends, and steps are now ~6× longer.


### 7. The coast thermal lattice (THERMAL)

**Why a 300-part craft's lattice cost 18 ms per 1,000,000× frame** (`bench_integrator` `BENCH_FLEET=1`: settled 30 days at 300 km, then 60 frames; 18.3 ms, integration 0.7 ms). Its lattice solved at almost every 600 s point (1,693 of 1,728 in 12 days), with 3–4 sweeps each. The skip never engaged because of the thin air:
- A craft with a known attitude (limbo) takes the air as 12 samples of its osculating orbit (~3.8 W/m² at 300 km). The osculating elements carry J2's short-period terms, so the samples' mean flux and anisotropy move by 1–4 % between lattice points, and the skip asks for 1 % (`SUN_SAME`).
- A coast without a pointing (SAS on, the fleet's quiet craft) takes one air sample at the point: ±11 % around an orbit of e = 0.001, its direction turning in body axes. At 470 km, 64 % of the points solve.
- A fast tumbler takes the flux at the point over an icosahedron, with the same swing.

Each solve (`sample` profile) was 40 % network sweeps, 46 % heat inputs (the sunlight and 12 air samples, each a pass over the 4,503 cells with the exposure interpolated per cell: three divisions per cell) and 8 % the nodes' capacities (the tanks' volume grid).

**What was built** (93d50b4, the same bits):
- `thermal::coast_heat` computes every sample's exposure first, then sums each cell's samples in one pass, in the order and with the operations of `cell_heat_flow` and `free_molecular_heat`.
- `AeroBake::exposure_at` looks up each weighted byte `w·e/255` in a table of 256 values made per direction, instead of dividing per cell.
- `VesselThermal` keeps the nodes' capacities of the last coast solve for its design and propellant. The cache is not saved and compares equal, like `DesignSlot`.

Tests: `coast_heat` equals the per-sample calls bit for bit (one sun; orbit samples with one in shadow; 12 air samples), and `exposure_at` equals the division per cell over 40 directions. Twelve days of four coasts (stress and test craft, still and held prograde, 250–300 km), solved at every lattice point, end at the same bits as before, and the sim's tests pass unchanged (goldens included). The accuracy is therefore unchanged: 1–2 mK against the converged solve at 600 s steps (`bench_thermal`).

| Release, M2 Pro, machine quiet (load ~5); single craft: median of 3 | Before | After |
|---|---|---|
| 300-part craft, thermal lattice per 1,000,000× frame (`BENCH_FLEET=1`) | 18.3 ms | **12.5 ms** |
| 300-part craft, a lattice point over a day at 400 km, settled (`bench_thermal`): tumbling / held prograde | 826 / 735 µs | **489 / 533 µs** |
| Test craft, the same | 84 / 117 µs | 81 / 115 µs |
| **50-craft fleet at 1,000,000×** (`fleet_frames`, 7 threads), warp reached per wall second | 242,000–247,000× (4 runs) | **306,000–311,000×** (3 runs) |
| Fleet's work per 1,000,000× frame, summed over vessels | 342 ms | 268 ms |

Per vessel per 1,000,000× frame (calibration chunks of `fleet_frames`, integration included):
- 300-part tumblers at 300–330 km: 21–24 → 13–14 ms.
- 300-part craft under SAS at 300–320 km: 19–20.5 → 14–15 ms.
- 300-part craft under SAS at 450–495 km: 7–11 → 6–10 ms.
- Test craft: 1.7–3.1 ms (unchanged).

Budget units now read 0.45–0.59 µs per unit on the 300-part craft (0.55–0.76 before) and 0.41–0.57 on the test craft. `solve_units` had underestimated the heat inputs (the divisions), and the craft now agree. A lattice point that evaluates its sunlight and air and is then skipped still counts 1 unit for 4–10 µs (measured: one sun and one air sample, and 13 samples). That is negligible beside the solves (~0.4 ms each).

**Tried and measured, not adopted** (against the same coasts solved at every lattice point, settled days 10–12):
- *Weighing the air's change against the sunlight* (1 % of the larger flux), so the noisy air no longer blocks the skip. A 300-part craft's lattice fell 18.3 → 5.8 ms per frame (3.2×), but the skip's lag showed:

  | Craft | Worst error, before | Worst error, after |
  |---|---|---|
  | Still stress craft | 7.6 mK | 112 mK |
  | Still test craft | 8 mK | 182 mK |
  | Held stress craft | 1.3 K | 2.1 K |
  | Test craft held prograde at 250 km | 44 mK | **4.4 K** |

  At 250 km its cold cells facing the air are warmed mostly by the air (~1.6 K per W/m² at 150 K).
- *The same with `STEP_CHANGE_K` 0.1* (from 0.5): the still stress craft erred 15 mK with 1.5× fewer solves (1,102 of 1,728 in 12 days), but the test craft held at 250 km still erred 2.1 K. The air must be compared on its own scale.
- *An air floor of 10 W/m²* (1 % of the larger of the air and 10 W/m²): only 1.35× fewer solves (1,693 → 1,259), at 28 mK on the still craft and 2.0 K worst (mean 224 mK, 90 before) on the held one.
- *`COAST_CONVERGED` 3e-5* (from 1e-5): 6 % (12.5 → 11.8 ms), with the error per 600 s step 1–1.7 → 2.6–4.7 mK. The bits change for little.
- The existing skip already errs up to 1.3 K against solving every point (the held stress craft). Part of that is the every-point reference following the air samples' noise on cold cells.
- The sweeps run at ~10 ns per unknown, probably bound by the latency along the trees' chains (two divisions per unknown per sweep, each waiting on the last). Cutting that (reciprocals, reordering) changes bits.

## Measured

Release builds on the M2 Pro (Mac14,9), 7 compute threads. **Load:** other agents compiled and benchmarked all night (load averages 25–145 on 10 cores); the numbers below are from the quietest runs (load 23–60) and say so; BENCH's baseline ran at 5–10. Wall times under load read high (up to 5× at load 100); ratios measured in one run (sweeps, units per frame, µs per unit) do not depend on it.

**The 50-craft frame** (`cargo test -p game --release fleet_frames -- --ignored --nocapture`; BENCH's fleet, settled 30 days; each frame as `state::advance`: the fleet within 8 ms, then the look-ahead of the active vessel and one other):

| Warp | Before (BENCH, settled, 4 threads, every vessel to the target) | After (load 23–54) | 
|---|---|---|
| 1× | 0.22 ms mean (9.7 worst; fresh fleet) | **0.29 ms** mean (10.9 worst), none over 16.7 ms |
| 1,000× | 0.70 ms mean (12 worst); fresh fleet 0.87 (25) | **0.91 ms** mean (10.8 worst), none over 16.7 ms; 992× reached (1 frame of 600 held back) |
| 1,000,000× | **106 ms** mean (165 worst): ~9 fps, never compute-limited, 1,000,000× per frame = ~157,000× of real time | **8.7 ms** mean (16.3 worst), none over 16.7 ms: 60 fps, compute-limited 166/240, 55,800× per 1/60 s frame = 107,000× of real time (at load ~25; the warp shown honestly) |

At 1,000,000× the frame now stays under the 60 fps budget and the warp falls to what the cores can carry, instead of the frame rate falling to ~9 fps at the requested warp. The throughput (simulated seconds per real second) is the same order as before at a far higher machine load; with the machine to itself expect more. What bounds it: the fleet's work over 7 threads × 8 ms a frame (a settled 300-part craft in LEO is ~5 ms of work per 1,000,000× frame after these changes, a test craft 1–2 ms: ~130 ms for the fleet, against 56 ms of thread time), and since one vessel is advanced by one thread at a time, 8 ms of any single vessel's work per frame. The coast integration is the larger part of every vessel's cost now (hot spot 2, SOL's: 9.2 → 5.3 µs a LEO step during the night).

**Units against time** (the same run, µs per budget unit per vessel in 1,000,000× chunks): min 0.52, median 0.76, max 0.99 (before the `VolumeGrid` fix: max 7.6 on the 300-part craft).

**One vessel** (`examples/bench_thermal`, quiet runs): a coast lattice point over a day in LEO, the 300-part stress craft: 1.5–2.0 ms before, 0.8–1.1 ms with the new solve (the `VolumeGrid` cost came in between with the part layer and is gone again); the test craft 190–400 µs → 90–190 µs.

**Game-side costs** with 50 vessels in LEO, all tracked and in map view (`game_side_costs`): line ends of all 50 1.4 ms (7.3 at load ~30), map orbit sizes 0.2 ms, the map-view rule over 94 objects 0.014 ms, `Dominance::new` 0.01 ms. `Dominance::of` is called per path point with a hint (cheap); the map rule's O(n²) is 9,000 pair tests at 94 objects: nothing to fix at 50 craft.

**The long run** (`cargo run -p sim --release --example long_run`, years=2, 7 threads, load 23–60): 731 days in 3,787 frames at 1,000,000× (none capped: no vessel went live), 203 s of wall time. Every month: no NaN or infinity in any vessel; the fleet's state 3.19–3.20 MB of RON throughout (bounded: trajectories keep at most 2 samples behind the frame, systems and burns lists do not grow); the ion spirals climbed from 700–850 km to ~11,300 km in their year of thrust (596 kg of propellant used of 2,000), the thin-air craft at 430–455 km. The save at the end: 3.4 MB of RON, written and read in 0.2 s, equal to the fleet, and the saved and reloaded fleets flown 10 more days at 1,000,000× end at the same bits. Stress craft reload through PARTS's craft registry (`stress-300`).

**Going live** (`tests/frame_live_stop.rs`): the test craft falling onto the Moon from 30 km stops at the hand-over within one 1,000,000× frame (under 1 s of work, instead of flying to the ground in that frame), is then flown to the ground in 4× frames, and ends at the same bits as one jump; an engine starting in a long frame stops there, in a physics-warp frame it flies.

## Choices and what was not built

- **No reduced-order thermal model.** The task allowed one for coasting craft (aggregate cells per part or region, handed over to the full network on going live). After the block solver and the `VolumeGrid` fix, a coast thermal solve of a 300-part craft costs ~1 ms and runs only while the temperatures move (`STEP_CHANGE_K`), and the thermal lattice is a minority of the 1,000,000× frame (the integration dominates, SOL's work). A per-part aggregate would cut the solve ~10× more but merges the sunlit and shaded sides of a part (tens of kelvin on a held craft) and needs a hand-over rule with its own error: not worth it while it is not the bottleneck. If it becomes one: aggregate by part and facing direction, keep the nodes, carry each cell's offset from its aggregate's mean through the coast, and test the hand-over against the full network.
- **The simulation is not decoupled from the frame.** The fleet gets 8 ms of the 7 threads per frame (and a vessel at most 8 ms of one), and the threads idle while the rest of the frame draws. Running the fleet continuously on worker threads (the render reading the last completed state) would give the simulation every core all the time; it needs the scene and HUD to read a snapshot of the vessels.
- **Line ends are recomputed every frame** (`trajectory::vessel_line_end`): 1.4 ms for all 50 vessels tracked in map view (7 ms under heavy load). Acceptable at 50 craft; a per-segment prefix sum of the swept angle would make each O(log n).
- **Drawing 50 craft** costs 50 draw calls with one material each (per-ship light, D055) and one shared mesh: not measured on the GPU here.

## Verify in the offscreen demo

The lead runs (one game at a time; the window is not needed):

1. `SUNSCATTER_DEMO=<dir> SUNSCATTER_DEMO_OFFSCREEN=1 cargo run -p game --release` and read the `demo perf:` lines (11 vessels at 1×, 1,000×, 1,000,000×): every average should be ≥ 60 fps with the worst frame under ~20 ms; at 1,000,000× "compute-limited N/M" may now be non-zero (the clock is held back instead of the frame stretched), and the effective warp is what the machine achieved.
2. A screenshot of the time panel at 1,000,000× with Show FPS on: "fps, sim X ms" beside the rate, and "limited: compute, Nkx achieved" when the warp is held back.
3. Optional, the scale case: spawn more vessels (F2 adds 10 test craft per press) and warp to 1,000,000×: the frame rate should stay near 60 with the achieved warp shown.
4. Watch for vessels drawn ahead of the clock at 1,000,000× while compute-limited (an icon a little ahead on its orbit, by at most a quarter of a frame's progress): expected, see the frame's span above.

## What remains

- **Decouple the simulation from the frame:** advance the fleet continuously on worker threads (the render reading the last completed state), so every core works all the time and one heavy vessel no longer bounds the warp to 8 ms of its own work per frame.
- **The coast thermal lattice of 300-part craft** still bounds the fleet at 1,000,000×: 6–15 ms per craft per frame after §7, against 0.7 ms of integration. Their solves run at nearly every lattice point because the thin air's input is noisy (§7). Next steps, in order:
  - Make the air's input as smooth as the sunlight's, so the existing skip engages (up to ~3× on craft in thin air, as when the air was ignored in §7). Average it over the orbit for coasts without a pointing too (the attitude is constant there), and take the samples' semi-major axis from the energy with J2's potential, so they no longer move with the short-period terms.
  - Then measure the skip itself against solving every point. It errs up to 1.3 K on a held 300-part craft, so its error control (`STEP_CHANGE_K`, a long backward-Euler step's lag) may need tightening as it engages more often.
  - A warm start from the last solve's rate (it must be saved), or a direct factorisation, for the sweeps.
  - Splitting one vessel's lattice across threads: a 300-part craft at 13–15 ms per frame exceeds a vessel's 8 ms per frame.
- **The coast integration** (COAST, §6) is 0.5–0.8 ms per craft per 1,000,000× frame, 4–5× less than before. Next steps:
  - Interpolate the bodies within a step (estimated 15–20 %).
  - Compute the pole once per evaluation instead of in both J2 and the air (~5 %).
  - A DOP853 continuous extension, to allow longer steps without the Hermite dense output's error.
  - Find live-air dips within a step.
- **Live ticks of 300-part craft** (hot spot 3, AERO's): ~1 ms per tick climbing at M 1.2 after AERO's boundary-layer reuse; three 300-part craft live at 4× are ~10 ms a frame.
- **Craft design and build per split** (hot spots 4–5): PARTS and AERO cut them during the night; a 300-part split is still far over a frame, and should run off the frame thread.
- Line ends per frame (1.4 ms for 50 tracked), a reduced-order thermal model, GPU cost of 50 craft: not needed at this scale (see Choices).
- The thermal solve's next step if needed: a coarse-grid correction per node (aggregation) for the long-wavelength modes the trees leave (9–10 sweeps now), or a warm start from the last step's rate (needs the rate in the save).
