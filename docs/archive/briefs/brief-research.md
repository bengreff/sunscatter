# Brief: RESEARCH, the two flights' hard problems (2026-10-31)

Phase 4 flies Crew Dragon to a station and back, and lands the Falcon 9's booster (D117, D123, D125, D126), after the sim-wide wave. This brief prepares the designs those flights need, so their agents start from researched designs instead of a blank page. Follow [agent-rules.md](../../plans/agent-rules.md). Your name is RESEARCH. You build nothing and run no game, so you have no target dir.

Two agents edit code at the same time: **PLANNER-2** ([brief-planner.md](brief-planner.md)) and **ONEKIND-3** ([brief-onekind.md](../../plans/brief-onekind.md)). You write only docs:
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

**Done (RESEARCH, 2026-10-30).** All three items are written; each doc's status is *design*, and each ends with its own "Open for the owner". The docs landed in **26f4639**; this State in the commit after it.

1. [landing-guidance.md](../../technical/landing-guidance.md).
   - **Pick:** one layer-2 program, `ship_os::landing`. Its predictor is `sim::landing::model` taking a burn plan, the one owner of predicted descents. A Gauss–Newton corrector with a fixed number of steps per wake steers the boostback (velocity-to-be-gained, least-norm step), the entry burn (ignition and cutoff on the predicted q and heating) and the aero descent (an angle-of-attack command held by SAS through the fins).
   - **Landing burn:** a lossless-convexification SOCP (2007 thrust bounds, 2013 pointing). Drag is carried as a known acceleration from the last plan, and the back-pressure enters the mass flow exactly. Minimum error and minimum fuel are one exact-penalty solve; t_f is searched across wakes. It is solved every 0.2 s by a custom **primal–dual interior point** (ECOS's algorithm) with a fixed-pattern, pivot-free block-tridiagonal LDLᵀ and a cap of 30 iterations.
   - **Why the interior point:** the relaxation is tight only at the optimum, and a stage that cannot hover must not get a plan under its minimum throttle. It also certifies infeasibility for the fallback. PIPG/xPIPG (sub-ms) is the upgrade path. Estimate 5–15 ms per solve on one M2 Pro core.
   - Tests include the Açıkmeşe–Ploen Mars case (81 s, 399.5 kg) and Crew-13's RTLS.
2. [entry-ablation.md](../../technical/entry-ablation.md).
   - **Ablation:** a FIAT/CMA-class charring column per ablator cell (3-component Arrhenius integrated in closed form, a surface balance with film blowing, a B′c law derived from the atmosphere's oxygen), with TACOT's open parameters as PICA-X (E). Tests: Ablation Test-Cases 2.1 and 2.3 (15.7 / 19.5 / 25.7 mm at 60 s) and Stardust.
   - **Dragon's entry:** lift from the offset centre of mass through the existing aero model (Trevino 2008: L/D 0.18 at 12°, under 5 g), steered by an FNPEG-type bank predictor–corrector on the same predictor.
   - **Chutes:** layer-2 deploy from navigation plus barometric altitude, and an optional layer-1 baroswitch in the chute part, after Apollo's ELS (TN D-7437 checked: 24,000 ft, reefing 10 s, mains at 11,000 ft, 6 s and 10 s).
3. [water-landing.md](../../technical/water-landing.md). Per-cell hydrostatics; the slam by von Kármán's momentum method on the wetted footprint with the added mass implicit; drag in water. Checked by hand against TN D-3980: 20.4 g against the rigid theory's ~22 g, and ~490 kPa against 414 kPa measured. Dragon: 6.6 g at 7 m/s flat.
4. `falcon9.md`: 16 rows tagged (RESEARCH) in §3.2, §4.4, §4.5 and §5.5, and sources [S30]–[S36]. They cover:
   - Crew-13's RTLS timeline; the RTLS reserve (~57 t, D); relights (no spare); grid-fin authority (order of magnitude); the legs' energy (~5.3 m/s);
   - the trunk after the deorbit burn and Pacific splashdowns (2025); Crew-9's timeline; L/D; 3-of-4 mains;
   - LiOH as the CO₂ method, the removal law rate = η·Q·c (it replaces the fixed 4.0 kg/day cap: four crew settle at ~0.2 kPa), the LiOH stock (~60 kg for 4 crew × 10 days), the free-flight endurance (10 days E; Polaris Dawn ~5 days flown);
   - the "rapidly pressurized to 2,400 psi" second report, and the helium bottles' sizing: ~2 m³ at 41.4 MPa, so §1.5's 2 × 0.1 m³ is too small (reported, not changed).

**Not found:** Crew Dragon's helium bottle count, volume and pressure; its heat-shield thickness, centre-of-mass offset and entry algorithm; its reefing; the Falcon 9's grid-fin deflection range and rate, and the legs' stroke and design touchdown speed; TACOT's k(T) and c_p(T) tables (in the workshop spreadsheet, not fetched); the ICES-2020-333 text (the repository refused downloads); Orion's measured water-impact peaks.

**Left for the lead:** link the three docs from `docs/systems/avionics.md`, `aero-thermal.md` and `motion.md`, and add owners rows when they are built (this brief allowed only the files above).

**Open for the owner** (each doc lists its own; the main ones):
- the predictor as a ship-OS call (recommended) or inside the program;
- the predictor's aero: cells or the bake's table;
- the solver: interior point now (recommended), PIPG later;
- TACOT for PICA-X;
- Dragon's shield thickness and its centre-of-mass offset;
- a layer-1 baroswitch in the chute part;
- terrain below sea level counts as ocean;
- von Kármán (recommended) or Wagner;
- the CO₂ removal law in place of the cap.
