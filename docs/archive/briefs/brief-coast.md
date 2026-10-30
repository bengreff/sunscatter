# Brief: COAST, one law for a coast's slow changes (2026-10-30)

D119 (owner, 2026-10-30): "On coasts, just do a coarse grained model for things like that. You should still be simulating boil off on coast. Solve analytically when you can." Follow [agent-rules.md](../../plans/agent-rules.md). Your name is COAST and your target dir is `target/agents/coast`. Read MASS's brief first ([brief-mass.md](brief-mass.md): its decisions and Open for the owner), which this replaces in part. Two other agents run at the same time. **FLIGHTSW** ([brief-flightsw.md](brief-flightsw.md)) is on SAS, the holds, `systems/data.rs`, `systems/estimate.rs`, `landing/` and the pad's release. **FIXES2** ([brief-fixes2.md](brief-fixes2.md)) is on the game's bars, `craft/program.rs` and the craft's functions, plus Dragon's tank data. Stay out of files with their uncommitted changes in them.

## What is wrong

- Since F2 (751d0da), a craft of parts' coast segment integrates its perturbations (drag, radiation pressure) with the mass it **started** with, while the ship state vents on the 60 s systems lattice. After a day of saturated LOX they differ by ~1.2 %: two truths again.
- Boil-off and venting are stepped on the lattice even where a closed form exists.

## The work, in order

1. **Inventory** what changes slowly through a coast and how each is computed today (the plumbing's boil-off, venting and pressurisation, battery charge, thermal, ECLSS), in this brief's State.
2. **Analytic where possible.** A tank's heat leak, its pressure rising to relief, then venting at the heat leak over the latent heat, has a closed form over a coast while the conditions hold. Evaluate it at any time, and step it only where no closed form exists. Where the conditions change (eclipse, a heater, a bus going down), break the law at that event, deterministically. Other slow quantities: do it where it is cheap and exact. Otherwise keep the lattice and record why.
3. **One law for the trajectory and the ship state.** The coast segment's mass at time t (its perturbations, its inertia) and the ship state's tanks at t come from the same law. Coarse-grained is fine for the perturbations: for example, the law sampled at the lattice points, or a polynomial fit, stated with its error bound.
4. **Tests:**
   - Over a day's coast, the segment's mass at t equals the ship's consumables at t (bit for bit, or to a stated bound if coarse-grained).
   - The analytic boil-off agrees with the stepped one within a stated tolerance, with the hand derivation in the doc comment.
   - Boil-off still runs through a coast.
   - Chunked = single pass, the same in one jump and in 60 fps frames, and the warp tests unchanged.
5. **Docs:** `technical/motion-model.md` or `ship-model.md` where coasts are described, the `architecture.md` rows, MASS's Open item marked as resolved by D119, and this brief's State.

## State

(Update as you land: commits with hashes, the inventory, what is left in order.)

### Inventory (before COAST)

What changes slowly through a coast of a craft of parts, all on the 60 s systems lattice (`vessel::coast::coast_until` → `vessel::ship_tick::ship_lattice` → `systems::ship::lattice`), while its segment kept the mass it started with (`Limbo::mass`: `mass0`):

| What | How (before) | Mass? |
|---|---|---|
| Each tank's heat (`plumbing::heat_in`) | the sunlight its wetted wall absorbs at the attitude and eclipse of the lattice point, less εσT⁴ of the wetted wall (in air: the film's 9 W/(m²·K) to the air); none inside a component (its heaters) | no |
| The liquid's temperature (`plumbing::slow`) | Euler step of Q·dt/(m·c), clamped to [freezing, critical] | no |
| The ullage's gas and the bottles' (`slow`) | relax to the liquid's temperature, τ 300 s and 60 s | no |
| Relief (`plumbing::relieve`) | above T_sat at the MEOP the excess heat boils off; above the MEOP the pressurant vents with its share of vapour | **yes**: the only mass a coast loses |
| Pressurisation (`plumbing::fast`, `pressurise`) | the regulator tops the ullage up to its loading pressure from the bottles; the ullage's adiabatic re-expansion | moves gas between bottle and ullage |
| Settling (`plumbing::settle`) | in free fall the liquid disperses to the tank's middle (capillary time √(ρR³/σ), ~10 min for the S2 LOX) | moves the centre of mass |
| Power (`power::step_ship`) | solar cells at the instant's sunlight and attitude, batteries, shedding | no |
| Crew (`crew::step`) | the cabin's air from the life support's supply rates | no (not in the consumables) |
| Thermal nodes (`aerothermal::coast_step`, 600 s, orbit-averaged inputs) | reads each tank's contents (`Aboard::Tanks`) | no |
| A made-up craft's boil-off (`systems::Consumables`) | a constant flow: already one continuous law in its segments and live ticks | yes (already one law) |

### Decisions (most consistent with D119, D081, rules 4 and 5)

- **The tanks and bottles of a craft of parts follow one law of time through a coast** (`sim::systems::coast_law::CoastLaw`), carried by the coast segment's limbo model (`Limbo::law`). The segment's mass (its drag and radiation pressure) is the dry mass plus the law's consumables; the ship state at each systems lattice point is the law's there (`systems::ship::coast_lattice`). Both take the one sum (`craft::state::consumables_sum`): the same bits.
- **Spans of at least an hour** (`coast_law::SPAN`), each started at one of the segment's stored samples (`Segment::after_step`), so it is the same however the coast is chunked; a coast continuing into its next segment takes the law on (`Trajectory::next_segment`); a burn's segment and a coast after a predicted burn carry none (the burn is flown live; the coast after it restarts from the ship).
- **The conditions a span holds:** each tank's heat (`plumbing::HeatLaw`) from the coast's orbit-averaged sunlight at the attitude's turn, as the coast thermal lattice takes it (`coast_heat::law_conditions`; the sunlight from every direction where the segment does not know the attitude, as its drag takes the orientation average), and the liquid's height at the span's start.
- **A span starts from equilibrium** (`plumbing::equilibrate`): the ullage's gas and the bottles in a liquid at its temperature (their τ 300 s and 60 s done), the regulator's top-up to the loading pressure, relief.
- **Closed forms** (the hand derivations in `coast_law.rs`'s module doc): sealed warming m·c·dT/dt = S − K·T⁴ (atanh + atan, inverted by bracketed Newton; T⁻³ without sunlight; exponential in air); the pressurant vented from the relief temperature (bisection); boil-off at saturation L·dm/dt = −(a + b·m), exponential, at a constant rate while the liquid fills the tank's length.
- **The vapour the lattice vented with the helium** is left to the boil-off at saturation (its heat warms the liquid instead: the same energy, within 1 % in the test; only the timing moves).
- **The tanks are brought to the law at the vessel's time** (`Vessel::law_to_now`) before a command or an order acts, a split, a restart, or going live. Code that writes a coasting vessel's tanks directly must restart its coast (`set_tank` does; the tests that poke tanks call `start_coast`).
- **Kept on the lattice, and why:** the liquids' dispersal (no mass; its geometry follows the law's liquid), power (solar cells at the instant's sunlight and attitude, batteries and shedding thresholds: events no orbit average keeps, no mass), the crew's air (no mass in the consumables), the thermal nodes (their own 600 s lattice of orbit averages, reading the tanks).

### Landed

- **dbe3f3b** + **861157e** (rustfmt): `sim::systems::coast_law` (with `coast_law_tests.rs`), `plumbing::{HeatLaw, heat_law, equilibrate, float}` (`plumbing_coast.rs`; `heat_in`'s terms unchanged, `relieve` on one tank), `craft::state::{consumables_of, consumables_sum, molar_mass}`, `math::{expm1, atan}`, `Limbo::{law, law_heat, law_from, next_span, without_law, ship_def}`, `coast_heat::law_conditions`, `systems::ship::coast_lattice`, `Vessel::law_to_now`. Tests: the heat law is the lattice's; boil-off at saturation by the law 1,377.793 kg a day, by hand the same, by the 60 s lattice 1,377.811 kg; warming within 0.01 K sealed and the heat taken within 1 % venting; the warming's inverse; over a day's coast the segment's mass is the ship's at all 1,440 lattice points, bit for bit, and one jump gives the same vessel; the day's boil-off on rails 1,451.9 kg (the lattice's 1,450); chunked = single pass and the warp tests unchanged and passing.
- Docs (this commit): `ship-model.md` §5.1, §5.2, §5.10; `architecture.md` (a `coast_law` row, the limbo, lattice and vessel rows); `technical/limbo.md`, `technical/ship-systems.md`; MASS's Open item marked resolved by D119; the plan's row.

### Left, in order

1. The coast's cost with the law for a craft of parts, measured (`examples/bench_coast` runs the test craft: the law adds a sum over the tanks per force evaluation and, while a tank vents its pressurant, a Newton inversion and a vapour-table lookup, which allocates; the spans cost an orbit average an hour).
2. Later, with the plumbing: a coast law for the vapour vented with the pressurant (phase 2's energy balance) if a stage's timing ever needs it.
3. On screen (lead): nothing should look different; the staging bar's propellant through a long coast steps at the lattice points as before.

### Open for the owner

- **The coarse grain.** Through a coast the tanks take the orbit-averaged sunlight (the eclipse swing smoothed, as the coast's temperatures already are) and their conditions are re-read hourly; the regulator tops the ullages up at those hours only. Measured: a day's boil-off in a 400 km orbit 1,452 kg against the 60 s lattice's 1,450.
- **Writes to a coasting vessel's tanks** go through a restart (`set_tank`); a direct write is overwritten by the law at the next lattice point. A rule for future code, written in `ship-model.md` §5.10.
- **Pre-existing, seen here:** in air the plumbing's tank heat uses the pad's film coefficient (9 W/(m²·K)) wherever the air model has air, so a coast between the live boundary and Earth's 150 km top (about 140–150 km) heats its tanks by convection to the thermosphere's ~700 K. Free-molecular heating (½ρv³, as the coast thermal lattice takes it) would replace it there. Not changed.
