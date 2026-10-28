# Brief: EXHAUST (2026-10-27)

Fourth in the owner's order. Rules: [agent-rules.md](agent-rules.md). Your name: `exhaust` (target dir `target/agents/exhaust`).

Read first: D109 and D073 (and the "one look" line that D109 revises) in `docs/decisions.md`; [research-2026-10-27-visuals.md](research-2026-10-27-visuals.md) "What exists today" and all of §1 (prior work, physics, the exhaust seed §1.4, the fast render model §1.5, cost budget §1.6, risks), §4 and §5's questions 1–2; the owner's answer 38 in [interview-2026-10-27-flight-ui.md](interview-2026-10-27-flight-ui.md) (the engine testbench is the owner's CRUCIBLE in `~/inquiry-project`, not ours: use placeholders from published engines now). The flame today: `crates/game/src/scene/parts.rs`, `scene/ships.rs`; engines in `sim::craft` and `data/`.

## Wanted

1. **The exhaust seed in `sim`**: each engine's state at the nozzle exit (exit pressure, velocity, temperature, γ, mean molecular mass, mass flow, exit radius, and what the plume is made of for its colour: soot, water, alumina…) as a pure function of the engine's data and its throttle, with published placeholders for the engines in the craft files (Merlin 1D, Merlin Vacuum, Draco, SuperDraco, the test craft's), each figure cited. Choose where the per-propellant data lives (a table beside `materials.ron`/`fluids.ron`, or the engine entries) and write the choice under "Open for the owner". Shaped so CRUCIBLE's output can replace the placeholders later without touching the render side.
2. **The render model** (game): the plume from the seed and the local air (ambient pressure → under/over-expansion, plume width growing with altitude, shock diamonds at low altitude, the vacuum plume's wide faint fan), cheap per §1.5–1.6; all of its rules pure functions with table tests (rule 8). Measured in the look's benchmark (`bench_*.md`) against §1.6's budget.
3. **The toggle** (D109): exhaust on/off in the settings, off = today's cone. One owner for the setting (`persist.rs` / `settings_ui.rs`).
4. Docs: a feature spec `docs/features/exhaust.md` (honest status), `docs/architecture.md` rows, a row in the plan.

## Watch out

- Check the other agent's files in `git status` before editing (`settings_ui.rs` was NAV's).
- See it: the demo (pad, ascent, after separation for the Merlin Vacuum; `SUNSCATTER_CRAFT=falcon9-crew`) under the demo lock, with the toggle on and off. Visuals come from physics, never for their own sake (D109): if the model cannot show something, leave it out and say so.

## State (EXHAUST, stopped at its context budget, 2026-10-27)

Landed (see the commit "The exhaust seed"): wanted 1, the seed in `sim::systems::exhaust` (`exit_state`, `NozzleSpec::of` / `of_engine`, `jet`, `propellants()`), the table `data/propellants.ron`, `EngineData::throat_area`, five table tests; the spec [exhaust.md](../features/exhaust.md) with the render model's and the toggle's design.

Left, in order:
1. The `docs/architecture.md` row (SYSTEMS had the file open; add it after it commits): "The exhaust seed (D109): each engine's exit state from its thrust, Isp, nozzle and the propellant pair (`exit_state`, `NozzleSpec`), the jet in the local air (`jet`: pressure ratio, expanded Mach and radius, lip turning, shock-cell length); the pairs' table `data/propellants.ron` | `sim::systems::exhaust` | what flames are drawn from; never read by the trajectory".
2. The toggle (wanted 3): `VisualSettings { exhaust: bool }` (default false) as `visuals` in `persist::SettingsFile` (the test that the file never says "graphics" must still pass), a checkbox in `settings_ui` (ask the agent holding it first).
3. The render model (wanted 2) in a new `game::scene::exhaust` (rules, table tests) wired in `scene::ships::update_parts`: per flame, `NozzleSpec::of(&def.engines[e].data)`, the level from its thrust (`NozzleSpec::level`), the seed, `jet` at the vessel's ambient pressure (already computed there); the cone stays when the toggle is off. The test craft: `NozzleSpec::of_engine` with the LOX/RP-1 row (it has no propellants: an assumption, say so). Then a `look::Parts` row (`exhaust`) and the demo (pad, ascent, after separation; `SUNSCATTER_CRAFT=falcon9-crew`) with the toggle on and off.
4. The plan's row and the spec's status words once each lands.

Open for the owner (written in the plan): the propellant data lives in a pair table beside `fluids.ron`, not in the engine entries (γ, molar mass and composition belong to the propellants and their mixture; an engine's own figures, CRUCIBLE's, would come as a per-engine exit state later). Exit pressure is derived (thrust-consistent), not a new published field.
