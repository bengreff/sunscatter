# Overnight plan, 2026-10-26: scale foundations, the Solar System, aero gaps

From the owner's answers ([record](interview-2026-10-26.md)); decisions D081–D085. The lead coordinates agents and verifies; each workstream ends with tests, its feature notes, and an independent review. Status words: *design / skeleton / playable / polished*.

## Order

| Phase | Workstreams (one agent each, separate files) |
|---|---|
| 1 | **SOL** (all night, background) · **BENCH** · **LIMBO** |
| 2 | **PARTS** (needs BENCH's generator) · **PERF** (needs BENCH's numbers) · **AERO** |
| 3 | **PROXIMITY** (needs PARTS and LIMBO merged) |
| End | Full offscreen demo, a review per workstream, the Windows run on backhouse, handoff |

## SOL: the Solar System (D084, D014; `docs/design/motion-model.md` §1)

- Bodies: the 8 planets; the dwarf planets (Ceres, Pluto, Eris, Haumea, Makemake, Gonggong, Quaoar, Sedna, Orcus); every moon over 100 km radius (Galileans; Mimas, Enceladus, Tethys, Dione, Rhea, Titan, Hyperion, Iapetus, Phoebe; Miranda, Ariel, Umbriel, Titania, Oberon; Triton, Proteus, Nereid; Charon; and the dwarf planets' large moons). Moons over 5 km after.
- Motion through the pipeline in the design: initial conditions from JPL data, one integration, rails plus drift where the residual is within D042's tolerance, Chebyshev tables where not. The data budget (D046) decides what is affordable for 1,000 years. Planets keep DE440. Golden tests against the reference ephemerides over their overlap.
- Physical data per body (`body.ron`), sourced: GM, radii, IAU rotation, J2 where measured, atmospheres for Venus, Mars and Titan (tables like Earth's US76). Visual data: base colour and albedo; maps only where cheap (zones from public mosaics, D073), else plain.
- Accept: every body appears in the tracking station and the map, on its real orbit (checked against Horizons at a few dates); the ephemeris file within budget; golden tests pass on both OSes.

## BENCH: the stress craft and the baseline (D081)

- `sim::craft::stress`: builds synthetic craft of N parts (stacks and clusters of tanks, engines, fins, legs from the existing primitives), each part with placeholder properties (mass, propellant, thermal, power use or production). Benchmarks and tests only; no catalog, no editor.
- Measure, per part count (1, 50, 300, 1,000): bake time (aero, thermal network), cells and nodes, memory, live tick cost, coast step cost, thermal-lattice step cost. Per fleet (1, 10, 50 craft): the game's frame loop at 1×, 1,000×, 1,000,000×, with 10 craft in limbo.
- Accept: `bench_scale` example + a numbers table in `docs/plans/scale-baseline-2026-10-26.md`; where each cost curve breaks against the D081 target, named.

## LIMBO: perturbed coasts and system lattices (D081, D025, D029; rule 4)

- Coasts under continuous forces, stored as trajectory segments so any warp samples them: low thrust (an engine at part throttle for days to months, propellant draining exactly, attitude from the hold), upper-atmosphere drag above the live boundary (air to 1,000 km from the US76 table, attitude-dependent area), radiation pressure on the craft's cells (the aero bake gives the projected areas; eclipses from `sim::light`).
- Where the boundary between live flight and coast lies: by dynamic pressure or heating, not a fixed altitude, so a decaying orbit hands over to live physics smoothly.
- Systems on fixed lattices independent of warp: thermal (exists), plus placeholder power (solar from sunlight with eclipses, storage, loads) and consumables (boil-off), in one framework a real system can plug into later.
- Accept: invariance tests: a month of low thrust, a year of drag decay and a year of systems give the same bits at 1×-chunking, 1,000× and 1,000,000× (chunked = single pass); drag decay against a published decay rate; SRP acceleration against the analytic flat-plate value.

## PARTS: the part layer, splitting and debris (D032, D083)

- A vessel is one rigid body made of parts; parts carry placeholder properties and own their cells and thermal nodes. Aggregates (mass, CoM, inertia, cells, network) are rebuilt when parts change.
- Split: a set of parts leaves as a new vessel. Momentum, angular momentum, attitude and temperatures are conserved, and both continue in the right phase. Breakage triggers with placeholder limits: a part over its temperature limit, or over a load limit.
- Debris rule (D083): delete a vessel only if it has no control (no crew, probe core or power: placeholder flags) and is dead, and it is 10 km or more from every controllable craft. One pure function with a table test.
- Accept: conservation tests; splitting at 1× and at a chunk boundary agree; debris rule table; a 300-part stress craft splits in under a frame budget.

## PERF: stability and speed at scale (D081)

- From BENCH's numbers: per-vessel work in parallel (vessels are independent, D025; results merged in a fixed order, never an order-dependent reduction, rule 2); budgets that hold 60 fps at 1× with 50 craft and keep 1,000,000× real for limbo craft; rendering many craft (instancing, LOD); the map and trajectory costs listed in `open-issues.md` (`Dominance::of`, line ends, map O(n²)).
- The warp cap (D081): rails warp refused while any craft is live, with the reason; tested.
- Stability: a long run (50 craft, simulated years at 1,000,000×): no NaN, bounded memory, save/load at scale equal bit for bit.

## AERO: D070 gaps and boundary-layer turbulence (D085)

- Laminar–turbulent transition on every cell (Re_θ or Re_x with a Mach correction), turbulent friction and turbulent heating augmentation over the whole body.
- The D070 gaps in `aero-thermal.md`: body–fin interference (Pitts–Nielsen–Kaattari), crossflow Cd vs Mach (Jorgensen), base drag vs Mach, wall-temperature effect on friction, the supersonic area rule if affordable. Verify the constants still cited from memory (Wilmoth, Tauber–Sutton Earth).
- Speed: aero and heating cost per cell budgeted for 1–2 large craft or many small pieces.
- Accept: each gap with a test against published data; cost per cell measured.

## PROXIMITY: the 10 km bubble and collisions (D082)

- Craft within 10 km of a live controllable craft fly live with it, in a shared local frame (rule 3: relative positions through the frame tree, mm precision).
- Craft-to-craft rigid-body contact, extending `sim::contact`.
- Accept: two craft drifting together collide with momentum conserved; relative position precision at 10 km; entering and leaving the bubble changes nothing in the trajectories (invariance).

## Working rules (the owner allowed full use of the machine overnight)

- Each agent builds in its own target directory (`CARGO_TARGET_DIR=target/agents/<name>`) so agents do not wait on each other's cargo locks. All cores.
- No branches or worktrees. Commit only your own paths with `git commit -m … -- <paths>`, then push. Keep the crate compiling: write new modules completely before wiring them in. Errors in a file you do not own are someone else's work in progress: keep working, recheck in a few minutes.
- Agents never run the game or the demo; the lead runs offscreen demos, one at a time.
- `cargo test -p sim` after every change to `sim`; every commit passes fmt (`cargo fmt -p <crate>`), clippy and tests.

## Status at the end of the night (2026-10-26, lead)

| Workstream | Status | Feature notes | Headline |
|---|---|---|---|
| SOL | playable | `docs/features/solar-system.md` | 44 bodies (planets, 9 dwarf planets, 26 moons over 100 km), sourced physical data, moon systems integrated in their barycenters and fitted to JPL (worst 50-year error: Io 24 km, Oberon 54 km, Mimas 390 km, Triton 500 km, Proteus 320 km unfitted); atmospheres for Venus, Mars, Titan; 19.4 MB ephemeris. 1,000 years not affordable as tables (~385 MB): options written up, not decided. |
| BENCH | done | `docs/plans/scale-baseline-2026-10-26.md` | `sim::craft::stress` (1–1,000 parts, placeholders), `bench_scale`; the baseline and its hot spots fed PERF, PARTS, AERO. |
| LIMBO (+LIMBO2) | playable (sim) | `docs/features/limbo.md` | Perturbed coasts: thin-air drag at the held attitude, radiation pressure, low thrust, boil-off; live boundary by q / heating / density; `sim::systems` lattice; every result the same bits at 1×, 1,000×, 1,000,000× (tested); drag decay 127.5 m/day at 400 km vs King-Hele 127.8; SRP within 0.2 %; Edelbaum within 0.3 %. Review fixes: reference body decided in the sim, regime epochs frame-independent, refly idempotent. |
| AERO | playable | `docs/design/aero-thermal.md` | Boundary-layer transition and turbulent friction/heating over the whole body (STS-1 transition onset matched), crossflow vs Mach (Jorgensen), body–fin interference, base drag vs Mach, gas per atmosphere (Mars Pathfinder peak heating 120.5 vs 120 W/cm²), real-gas turbulent heating, roughness transition; bake 5.6 s → 0.2 s at 300 parts, live tick 2.2 → 1.0 ms. |
| PARTS | skeleton | `docs/features/parts.md` | Part layer with placeholder properties, registry, split (momentum exact), breakage, the D083 debris rule in the game; craft build fixes (cells by area, thin tubes, broad phases). Nothing breaks up in play yet: no game craft has parts. |
| PERF | playable | `docs/features/scale-performance.md` | Block Gauss–Seidel thermal solve (sweeps 22–24 → 9–10), cost-based budgets, `game::fleet` on the compute pool: 50 craft at 10⁶× now ~8.7 ms a frame, the clock honestly held to ~107,000× real time; 2 simulated years with 50 craft stable, save/load bit for bit. |
| PROXIMITY | playable (sim), wired, not seen | `docs/features/proximity.md` | Bubbles of craft within 10 km of a live controllable leader, each with its own frame and lattice; craft-to-craft contact with momentum exact; 1.26 ms a tick for two 300-part craft touching; warp shows "limited: near Vessel N". |
| COAST | playable | `docs/features/scale-performance.md` §6 | Coasts integrate with DOP853 (steps ≤ 3 h): LEO-day error 53 → 9.9 m against a tight reference at a third of the evaluations; integration per 10⁶× frame ~3.5 → ~0.75 ms per craft; 50 craft at 10⁶×: ~120,000× → 183,000× of real time. The 300-part craft's coast thermal lattice (~18 ms per craft per frame) now bounds the clock. |

Two independent reviews ran (LIMBO+SOL; PARTS+PROXIMITY+PERF+AERO); every verified finding was fixed or is listed in `open-issues.md`.
