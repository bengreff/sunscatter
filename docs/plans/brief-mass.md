# Brief: MASS, one mass truth (2026-10-29)

D117's second foundation: F2, the review's recommendation 2 ([fidelity-review-2026-10-29.md](fidelity-review-2026-10-29.md) §3 F2 and §4.2). Follow [agent-rules.md](agent-rules.md). Your name is MASS and your target dir is `target/agents/mass`: start warm with `mv target/agents/foundation target/agents/mass`. ROUNDTRIP works at the same time on the stage sequence's split and merge (`brief-foundation.md`, Left 1: `vessel/sequence*`, `systems/command.rs`, the game's stage bar and editor). QUICKFIX may still be finishing `docs/features/parts.md`. Stay out of their files.

## What is wrong

- **The mass is held twice.** `Vessel::propellant` is a scalar (`systems/mod.rs`), which is also the coast segment's starting mass. `ShipState::scale_to` (`craft/state.rs`) scales the tanks back to it uniformly, and the ship state holds each tank's contents separately.
- **The thermal nodes' capacities come from the scalar** (`vessel/aerothermal.rs::engine_and_nodes`).
- **The mass model ignores where the liquid is.** It treats each tank's liquid as a solid cylinder filling from the tank's base, rigid with the tank (`craft/mass.rs::with_contents`). The plumbing's slug drift (`systems/plumbing.rs`, settling as a 1-D slug along the tank axis) does not move it, and in roll the liquid turns with the tank.
- Slosh, boil-off per tank and the booster's flip all need **one per-tank liquid state** that the mass model, the thermal network and the coast law all read.

## The work, in order

1. **Inventory** every reader and writer of the propellant scalar and of the tanks' contents (sim and game), in this brief's State.
2. **The ship state is the only propellant truth for a craft of parts.** Derive anything that needs a total from the tanks. Remove `scale_to`'s uniform scaling. Keep the coast segment's mass law exact and warp-invariant. Keep the blocks (the made-up craft) working if they rely on the scalar, and record how.
3. **The liquid's centroid comes from the slug's position** in the mass model and the inertia. A slosh pendulum (Abramson, NASA SP-106) is a later step: leave room for it, don't build it.
4. **The thermal nodes' capacities come from the tanks' liquids.**
5. **Tests** (in the owning modules):
   - During a 30 s booster flip on cold gas, the centre of mass moves by the hand-computed amount (±1 cm). Write the hand calculation in the test's doc comment.
   - After a day's coast with boil-off, only the venting tank loses mass.
   - The vessel's propellant equals the ship's consumables bit for bit, both in 1× frames and in one jump.
   - The chunked = single-pass and warp tests are unchanged.
6. **Docs:** the `architecture.md` rows, `design/ship-model.md`'s mass section, the review's §3 F2 marked with the commits, and this brief's State.

Not in scope: F3 (flight software reading `Knowledge`) and slosh dynamics.

## State

(Update as you land: commits with hashes, the inventory, what is left in order.)
