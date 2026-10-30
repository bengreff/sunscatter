# Brief: MASS, one mass truth (2026-10-29)

D117's second foundation: F2, the review's recommendation 2 ([fidelity-review-2026-10-29.md](../research/fidelity-review-2026-10-29.md) §3 F2 and §4.2). Follow [agent-rules.md](../../plans/agent-rules.md). Your name is MASS and your target dir is `target/agents/mass`: start warm with `mv target/agents/foundation target/agents/mass`. ROUNDTRIP works at the same time on the stage sequence's split and merge (`brief-foundation.md`, Left 1: `vessel/sequence*`, `systems/command.rs`, the game's stage bar and editor). QUICKFIX may still be finishing `docs/technical/parts.md`. Stay out of their files.

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
6. **Docs:** the `architecture.md` rows, `technical/ship-model.md`'s mass section, the review's §3 F2 marked with the commits, and this brief's State.

Not in scope: F3 (flight software reading `Knowledge`) and slosh dynamics.

## State

### Inventory (before F2)

- **The scalar `Vessel::propellant`.** Written by: the creation (`initial_propellant`), `init_ship` (= the ship's consumables), `set_propellant` (clamped, then `scale_to` over every tank and bottle), the live tick (a made-up craft burns it; a craft of parts = its consumables), the pad's held ticks and the landed lattice (= consumables), the coast's end (= the segment's mass − dry, then = consumables for a craft of parts: `contents_after_coast`), a split's pieces (a craft of parts: its piece's consumables; else the craft's fill), five tests. Read by: `mass()`, `propellant()` (the game's staging bar, planner Δv, data menus' TWR and Δv, MCP's TWR, landing predictions), a made-up craft's mass properties, engine and RCS gating, `live_height` (the uniform-fill model's centre of mass, for every craft), the coast's start mass, the coast's inertia (the uniform-fill model at the segment's mass, for every craft), the coast's and the live tick's thermal capacities, the placeholder systems' `Env`, a split's fill.
- **The tanks' contents** (`ShipState::tanks`, `bottles`, the ullages). Written by the systems' tick and lattice (`systems::ship`: the engines' draw, the plumbing's pressurisation, boil-off and venting), the commands, and `scale_to`. Read by `consumables`, `contents` → `MassModel::with_contents` (each liquid a cylinder from its tank's base, rigid with the tank; each ullage's gas at the top of the region at capacity), the plumbing, the estimates, the RCS's authority, the structure's loads.
- **The thermal nodes' capacities:** `CraftDesign::node_capacity(propellant)`, one fill for every tank and bottle, at the craft file's one specific heat (the Falcon 9's 1,700 J/(kg·K), its helium too).

### Decisions (most consistent with D081, D104, rules 4 and 5)

- A craft of parts' propellant is its ship state's: `Vessel::propellant()` sums the tanks and bottles; the scalar stays 0 for it (a made-up craft's only truth, its segments' mass law). `set_propellant` is the made-up craft's (a debug assertion on a craft of parts); `set_tank` sets one tank. `scale_to` is gone.
- A craft of parts' coast segment integrates its perturbations (drag, radiation pressure) with the mass it started with; the ship state vents on the systems lattice and is the mass at every lattice point (the coast's inertia, the thermal capacities, the systems). Exact and warp-invariant as before; the segment's perturbing accelerations err by the vented share (a day of saturated LOX: 1.45 t of 123 t).
- The liquid's slug is the plumbing's (`plumbing::slug`: its height from its density, `drift` from the outlet); the ullage's gas fills the length it leaves. The mass model takes slugs (`MassModel::with_slugs`, `Slug`); a slosh pendulum later moves a share of one across the axis.
- The thermal nodes take each tank's liquid where its slug sits, at the fluid table's specific heat, and each bottle's gas at its cᵥ (`Aboard::Tanks`, `plumbing::held`, `VolumeGrid::band`); a bottle inside another tank (the Falcon 9's helium in its LOX) goes to its nearest node. The made-up craft keeps its one fill (`Aboard::Fill`, bit for bit as before).

### Landed

- **751d0da** F2's code: `vessel::mass` (the vessel's mass, propellant, mass properties, `aboard`, `set_propellant` for the made-up craft, `set_tank`), `plumbing::{slug, ullage_centre, contents, held}`, `MassModel::with_slugs` (`with_contents`, `ShipState::contents`, `scale_to` and `sync_ship_to_propellant` gone), `design::{Aboard, Held}`, `VolumeGrid::band`; the coast's lattice reads the ship state; tests in `vessel/mass_tests.rs` (the flip: 7.473859 m against the hand 7.4739 m; the day's boil-off; the nodes), `craft/volume.rs` (`a_band_goes_where_it_sits`), the build test's hand sum on slugs. The chunked = single-pass and warp tests unchanged and passing.
- Docs (this commit): `architecture.md` rows, `ship-model.md` §5.1, the review's F2 and recommendation 2.

### Left, in order

1. Nothing of the brief. Later, with slosh: the pendulum (Abramson, SP-106) moving a share of the slug across the axis, and the liquids' spin in roll (both below).
2. On screen (lead): the booster's flip after separation draws the craft shifting against its centre of mass as the LOX is thrown up (`SUNSCATTER_DEMO_STOP_AFTER` past separation); nothing else should look different.

### Open for the owner

- ~~A coast keeps its starting mass for its perturbations while the ship state vents (above).~~ **Resolved by D119** (COAST, dbe3f3b + 861157e): the tanks follow one law of time through a coast (`systems::coast_law`), which the segment's mass and the ship state both read, bit for bit at every lattice point ([brief-coast.md](brief-coast.md)).
- In roll the liquids still turn with their tanks: a liquid spins up only through its boundary layers (Ekman), so its own moment about the tank's axis should leave the roll inertia. It needs the liquid's angular momentum as state: with the slosh pendulum.
- The ullages' gas (kilograms) is left out of the nodes' heat capacities.
- Seen in the boil-off test: Dragon's NTO and MMH tanks vent 11 g and 0.5 g a day in orbit at their relief (held warm by their heaters). Their loading pressure against their MEOP may want a look (plumbing data, not F2).
