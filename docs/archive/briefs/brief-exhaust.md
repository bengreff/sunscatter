# Brief: EXHAUST (2026-10-27)

Fourth in the owner's order. Rules: [agent-rules.md](../../plans/agent-rules.md). Your name: `exhaust` (target dir `target/agents/exhaust`).

Read first: D109 and D073 (and the "one look" line that D109 revises) in `docs/decisions.md`; [research-2026-10-27-visuals.md](../research/research-2026-10-27-visuals.md) "What exists today" and all of §1 (prior work, physics, the exhaust seed §1.4, the fast render model §1.5, cost budget §1.6, risks), §4 and §5's questions 1–2; the owner's answer 38 in [interview-2026-10-27-flight-ui.md](../interviews/interview-2026-10-27-flight-ui.md) (the engine testbench is the owner's CRUCIBLE in `~/inquiry-project`, not ours: use placeholders from published engines now). The flame today: `crates/game/src/scene/parts.rs`, `scene/ships.rs`; engines in `sim::craft` and `data/`.

## Wanted

1. **The exhaust seed in `sim`**: each engine's state at the nozzle exit (exit pressure, velocity, temperature, γ, mean molecular mass, mass flow, exit radius, and what the plume is made of for its colour: soot, water, alumina…) as a pure function of the engine's data and its throttle, with published placeholders for the engines in the craft files (Merlin 1D, Merlin Vacuum, Draco, SuperDraco, the test craft's), each figure cited. Choose where the per-propellant data lives (a table beside `materials.ron`/`fluids.ron`, or the engine entries) and write the choice under "Open for the owner". Shaped so CRUCIBLE's output can replace the placeholders later without touching the render side.
2. **The render model** (game): the plume from the seed and the local air (ambient pressure → under/over-expansion, plume width growing with altitude, shock diamonds at low altitude, the vacuum plume's wide faint fan), cheap per §1.5–1.6; all of its rules pure functions with table tests (rule 8). Measured in the look's benchmark (`bench_*.md`) against §1.6's budget.
3. **The toggle** (D109): exhaust on/off in the settings, off = today's cone. One owner for the setting (`persist.rs` / `settings_ui.rs`).
4. Docs: a feature spec `docs/technical/exhaust.md` (honest status), `docs/owners.md` rows, a row in the plan.

## Watch out

- Check the other agent's files in `git status` before editing (`settings_ui.rs` was NAV's).
- See it: the demo (pad, ascent, after separation for the Merlin Vacuum; `SUNSCATTER_CRAFT=falcon9-crew`) under the demo lock, with the toggle on and off. Visuals come from physics, never for their own sake (D109): if the model cannot show something, leave it out and say so.

## State (second EXHAUST agent, stopped at its context budget, 2026-10-28)

Landed: the seed (cfe86da); the architecture row for the seed (31c2b7d); the render model and the toggle (80e471e: `game::scene::exhaust` rules and table tests, `exhaust.wgsl`, `persist::VisualSettings`, Settings ▸ Visuals, `look::Parts::exhaust`); the parts demo's benchmark at max-Q and `SUNSCATTER_DEMO_FAR` (221d2f1); docs (spec *playable*, architecture rows, plan row). Seen in the demo on and off (pad, max-Q, upper stage; shots best in a tall window `SUNSCATTER_DEMO_SIZE=1000x2000`): Merlin plumes with diamonds at max-Q, the MVac's vacuum plume drawn but near-invisible in daylight. Benchmark: the exhaust row is inside the noise at max-Q.

Left, in order:
1. Check `data/propellants.ron`'s γ, molar mass and chamber temperatures (written from memory as Sutton & Biblarz Table 5-5) against a source that can be opened (NASA CEA runs, e.g. the CEA web/`cea` results at each row's MR and 1,000 psia, or published papers), cite what was checked in each row's `source`, and flag what could not be.
2. The render model's placeholders (spec, "Light" and "Temperature"): the afterburning temperature and weights, soot's optical constants and mass fraction, the core-length correlation; find sources or mark them for CRUCIBLE.
3. Measure a close-up of nine plumes (the §1.6 worst case) with the benchmark; the bound is a cylinder (a cone would be tighter for vacuum fans).
4. Owner's eye: the sea-level plume at liftoff is mostly hidden by the ground; plumes of nine engines do not merge.
