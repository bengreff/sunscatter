# Brief: RESEARCH, the two flights' hard problems (2026-10-31)

Phase 4 flies Crew Dragon to a station and back, and lands the Falcon 9's booster (D117, D123, D125, D126), after the sim-wide wave. This brief prepares the designs those flights need, so their agents start from researched designs instead of a blank page. Follow [agent-rules.md](agent-rules.md). Your name is RESEARCH. You build nothing and run no game, so you have no target dir.

Two agents edit code at the same time: **PLANNER-2** ([brief-planner.md](brief-planner.md)) and **ONEKIND-3** ([brief-onekind.md](brief-onekind.md)). You write only docs:
- new files under `docs/technical/`;
- your additions to `docs/vehicles/falcon9.md`, as new rows or sections, never rewording existing ones;
- this brief's State.

## Read first

- D117, D123, D125, D126, D128, D129, D116, D043, D101, D105 in `docs/decisions.md`; `docs/process.md`'s research strategy (KSP mods, Principia, …) and how numbers are tagged (`falcon9.md`'s "How to read this sheet": P, E, D, U, each sourced).
- `docs/vehicles/falcon9.md` §2, §4.4, §4.5, §5.5; `docs/technical/reentry.md`, `aero-thermal.md`, `engines.md`, `software-layers.md` (layer 2 is where guidance lives), `warp-invariance.md` (live, deterministic).
- Code only to see what exists, through grep and `sed -n`: `sim::systems::fins`, `sim::chute`, `sim::thermal`, `sim::landing`, `sim::systems::maneuver`.
- Web research: load `WebSearch` and `WebFetch` through ToolSearch. Prefer primary sources (papers, NASA and FAA documents, SpaceX's user guide) and open-source implementations. Record every source.

## The work, in order

1. **The booster's landing guidance** → `docs/technical/landing-guidance.md` (design). D123: lossless convexification (Açıkmeşe & Ploen, 2007; Blackmore, 2016), re-solved several times a second by a solver with a fixed number of iterations, deterministic, written as layer-2 code that can become WASM (D043).
   - The problem: variables, constraints (thrust bounds with the minimum throttle, the glide slope, pointing), the discretisation, the minimum-error then minimum-fuel steps.
   - The solver: a small second-order-cone solver in pure Rust with no dependency, a fixed number of iterations and no data-dependent branching that would break bit-identity (D127). Compare the options (an interior point with fixed iterations, ADMM, ECOS's algorithm, a custom solver), with open implementations to learn from (MechJeb's landing autopilot, kOS scripts, lcvx, G-FOLD write-ups).
   - The predictor–corrector for the boostback and entry burns to a landing zone first (D117), on the real propagator, plus the grid fins' part in it.
   - Pick one design, with the reason, and the tests that would prove it. Estimate the cost per solve on the M2 Pro.
2. **Dragon's entry, ablation and splashdown** → `docs/technical/entry-ablation.md` and `docs/technical/water-landing.md` (design).
   - 1-D ablation per cell from the material's parameters (D125: PICA-X is one material): the model (pyrolysis, char, surface recession, blowing), the parameters PICA-class materials have in the literature, and the heat shield's thickness and layout.
   - Dragon's entry: lift from its offset centre of mass, the bank-angle guidance it flies (layer 2), and the entry corridor.
   - The chutes' real logic as the automatic option (D126): the triggers, the backups, the disreef stages.
   - The calm sea (D123): slamming pressure (Wagner or von Kármán), added mass and buoyancy for a capsule, checked against Apollo, Orion or Dragon water-impact data.
3. **Data gaps** in `falcon9.md`, as tagged rows:
   - Dragon's CO₂ removal against four crew. The data has 4.0 kg/day against 4.16 kg produced. Dragon's free-flight endurance with four crew is an open item for the owner.
   - Dragon's helium bottles and their pressures (queued in the hand-off).
   - The booster for a landing-zone return: the boostback's propellant reserve, the relight's limits (TEA-TEB), grid-fin size and authority, and the legs.

Keep it to what the flights' agents need. Each design doc ends with "Open for the owner" and its status word (*design*). Wrap up at about 300k tokens of context, commit through a private index, and push. End with the report agent-rules asks for (at most 15 lines).

## State

(Update as you go: docs written with hashes, the picks and their reasons, what is left, Open for the owner.)
