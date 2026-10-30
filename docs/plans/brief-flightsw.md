# Brief: FLIGHTSW, the flight software reads what the sensors know (2026-10-30)

D117's third foundation: F3, the review's recommendation 4 ([fidelity-review-2026-10-29.md](fidelity-review-2026-10-29.md) §3 F3 and §4.4). Follow [agent-rules.md](agent-rules.md). Your name is FLIGHTSW and your target dir is `target/agents/flightsw`. Two other agents run at the same time. **COAST** ([brief-coast.md](brief-coast.md)) is on `vessel/coast.rs`, `vessel/mass.rs`, `systems/plumbing.rs` and the systems lattice. **FIXES2** ([brief-fixes2.md](brief-fixes2.md)) is on the game's staging bar and action bar, `craft/program.rs` and the craft's functions. Stay out of files with their uncommitted changes in them.

## What is wrong

- SAS (`vessel/attitude.rs`), the holds (`vessel/hold.rs::hold_inputs`), the pad's health check (`systems/propulsion.rs::release`), the estimates (`systems/estimate.rs`) and the landing prediction (`landing/`) all read the **true** state. Even with every IMU dead, SAS holds attitude perfectly.
- `Knowledge` (`systems/data.rs`: `knowledge(def, state, debug)`; `Vessel::knowledge`) already gives what working, powered, linked sensors measure, but only the display reads it.
- D115's adaptive functions (engine-out re-trim, launch-commit, propellant utilisation, the navigation filter) and D117's guidance (the booster flying itself home, Dragon's autonomous docking) all need a flight-software layer. Its inputs are `Knowledge` and the computers' health, and its outputs are `ShipCommand`s. Built on the truth, they would have to be rewritten.

## The work, in order

1. **Inventory** every control-side reader of the true state (sim and game), in this brief's State.
2. **The flight-software layer.** It is one owner (rule 5): each vessel's computers (`*.computer` in `data/craft/falcon9-crew/craft.ron`, linked to their sensors and effectors by the data links) run functions whose only inputs are `Knowledge` and the computers' health, and whose outputs are commands. Design it so D117's guidance and D115's adaptive functions plug in as functions of this layer, and so that later a ship-OS program (pillar 4) can be one. Its inputs and outputs are pure, with table tests. Don't build guidance or adaptive functions now.
3. **Move the readers onto it:** SAS and the holds, the pad's release check (it waits on the engine controllers' thrust readings), the estimates the player is shown, and the landing prediction. Debug mode keeps its truth view and changes nothing else.
4. **Acceptance (table tests):**
   - With every IMU lost, SAS will not hold, and says "no attitude reference".
   - With the stage computer unpowered, its running engines are safed by their own controllers. Find the real behaviour, or take the class default and mark it E.
   - The pad's release waits on the engine controllers' `EngineThrust` readings, so a lost controller means a pad abort.
   - Debug mode is unchanged.
   - Determinism and the warp tests are unchanged.
5. **Docs:** the `architecture.md` rows (a new owner for the flight software), `design/ship-model.md` or `features/ship-systems.md` where control is described, the review's §3 F3 marked with the commits, and this brief's State.

Respect D115: only the real vehicle's sensors, and readings to their sensor's significant figures. Noise, drift and outages only where they matter, and not in this brief.

## State

(Update as you land: commits with hashes, the inventory, what is left in order.)
