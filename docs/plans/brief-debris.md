# Brief: DEBRIS, controllable pieces and ballistic lumps (2026-10-31)

D121's separations: "Every time a craft is separated, we do a quick check: is this thing controlable (radio/crew/onboard computer, power). If not, simplify it to a lump until it gets deleted. If it is controlable, it becomes a new craft, fully simulated as inactive." It is the wave's last agent, after PERF. Follow [agent-rules.md](agent-rules.md). Your name is DEBRIS and your target dir is `target/agents/debris`: start warm from the newest sim target the lead names. You are the only sim-wide agent running.

## Read first

- D083, D103, D121 ("Separations"), D127, D128, D129 in `docs/decisions.md`.
- `docs/systems/structure.md`, `docs/technical/joint-loads.md`, `aero-thermal.md`, `limbo.md`, `software-layers.md` (what "controllable" means in layer terms), `warp-invariance.md` (D4: debris deletion at the frame's clock).
- Code: `sim::vessel::{split, breakage, debris}`, `sim::craft::{partition, design}`, `game::debris`.

## The work, in order

1. **The controllability check** at every separation: a piece is controllable if it has a radio or crew or an onboard computer, and power. It is a pure function of the piece's definition and state, with a table test (the Falcon 9's booster, its second stage, Dragon, the trunk, the fairing-less interstage, a broken-off leg).
2. **A controllable piece** becomes a full craft, simulated while inactive. Its design comes from the stage sequence when the craft is built (the pieces the sequence can make); an unplanned controllable piece gets a design composed from its parts' bakes.
3. **A ballistic lump:** its mass, its inertia, a drag area and heating from its bounding shape, and one heat node. It is deleted when it burns up or hits the ground, at its own epoch, not at the frame's clock (D129, AUDIT's D4). Lumps in orbit stay. It is cheap: count the cost of a hundred lumps against one craft.
4. **Designs at build:** the pieces the sequence can make get their designs (bake, volume grid, network) when the craft is built, so a separation costs nothing at the moment it happens. Measure the separation's cost before and after.
5. The warp, chunked = single-pass and `warp_course` tests stay green; a separation gives the same pieces at every warp (AUDIT's breakup test).

Docs: `systems/structure.md`, the technical docs touched, `owners.md`. End with the report agent-rules asks for.

## State

(Update as you land: commits with hashes, what is left in order, Open for the owner.)
