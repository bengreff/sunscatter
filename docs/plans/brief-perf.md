# Brief: PERF, D121's scale target (2026-10-31)

D121: "a ~5,000-part craft flown live at 1× and 60 fps on the M2 Pro, and D081's fleet of 50 craft of ~300 parts at every warp up to 10⁶×". D127: vessels advance on parallel workers at one fixed live rate, bit-identical across machines. PERF runs after ADVANCE ([brief-advance.md](brief-advance.md)), which gives every subsystem one advance law; you make those laws fast without changing a bit of what they compute, unless a step says so. Follow [agent-rules.md](agent-rules.md). Your name is PERF and your target dir is `target/agents/perf`: start warm from the newest sim target the lead names. You are the only sim-wide agent running.

## Read first

- D081, D119, D121 (its live costs and its game-side note: "I don't want to steer you towards chasing some target of 5000 cubes arbitrarily rendered"), D127, D129 in `docs/decisions.md`.
- `docs/technical/scale-performance.md` (the stress craft's numbers, ONEKIND-3), `docs/archive/research/scale-baseline-2026-10-26.md` (where a tick's time goes), `docs/technical/aero-thermal.md`, `warp-invariance.md`, `limbo.md`.
- Code: `sim::craft::stress` (`stress-<n>`), the bench examples (`bench_scale`, `bench_coast`, `bench_aero`, `bench_thermal`, `bench_integrator`), `sim::aero`, `sim::thermal`, `sim::craft::{design, volume, cells, broad}`, `sim::systems::coast_law`, `sim::vessel::{live, coast}`, `game::state::advance`, `game::fleet`.

## The work, in order

REVIEW (`docs/technical/review-2026-10-31.md`): `knowledge()` is rebuilt several times per tick, and sensor classes are matched as strings. Build it once per tick, with typed classes.


STATE-2's profile of the 1,000-part live tick (2026-10-31): the thermal network 62 %, the volume grid 7 %, aero 4 %, the ship systems about 3 %. So step 3 (thermal at its own rate, `ThermalNetwork::step_to`) is the live tick's lever; take it right after step 1.


Measure before and after each step on the stress craft at 300, 1,000 and 5,000 parts, and record the numbers in `scale-performance.md`. Every step keeps the warp, chunked = single-pass, determinism and `warp_course` tests green, bit for bit, unless it says otherwise and the change is tested.

1. **The coast at 5,000 parts.** It did not finish in 40 minutes: `coast_law::TankLaw::temperature` runs for each of ~6,500 tank compartments at every sample. Evaluate it once per span and only where it is read.
2. **Aero once per tick** (D121): the cells evaluated once, their forces held through the integrator's stages and the contact substeps. The boundary layer was 57–59 % of a live tick.
3. **Thermal at its own rate** (D121): the network steps at the rate its fastest time constant sets, with the ticks' heat accumulated between steps. On a coast, each node relaxes analytically toward its orbit-averaged equilibrium, with breaks at eclipse entry and exit (D119), unless ADVANCE has done it.
4. **A spatial index for the geometry scans** (the n² in `broad`, the bake, the volume grid and cells), and the bake's directions × triangles.
5. **Parallel vessels** (D127): each vessel, and each coast, on its own worker, the results gathered in a fixed order; craft in contact in the bubble on one worker. A CI case flies the same scenario on macOS and Windows and compares state hashes.
6. **The game's side, by judgement:** what a many-part craft costs to draw and list (instancing, the part lists, the map), measured with the `profile` and `bench` MCP tools in a headless game. Don't chase a count of cubes.

Report where the 5,000-part live tick stands against 16.7 ms, and what stands in the way, honestly. End with the report agent-rules asks for.

## State

(Update as you land: numbers before and after, commits with hashes, what is left in order, Open for the owner.)
