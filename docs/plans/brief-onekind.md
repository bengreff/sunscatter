# Brief: ONEKIND, one kind of craft (2026-10-30)

D121: "The test craft: delete it entirely". Every craft is built from parts (D100), and the made-up path (one placeholder part with a fuel number) goes. Follow [agent-rules.md](agent-rules.md). Your name is ONEKIND and your target dir is `target/agents/onekind`: start warm with `mv target/agents/flightsw target/agents/onekind`. Two other agents run at the same time:
- **MISSION** ([brief-mission.md](brief-mission.md)) replaces the scripted demo and the tour with MCP tools. Leave `crates/game/src/demo*.rs` and the tour to it: they are being deleted. If they stop compiling because of you, tell the lead in your State and wait for MISSION's deletion.
- **DOCS** ([brief-docs.md](brief-docs.md)) is restructuring `docs/`. Edit docs only where CI needs it (citations of a deleted decision). List every other doc change your work needs in your State, and DOCS or the lead applies it.

## The work, in order

1. **Inventory** the made-up path in this brief's State: every place that tells the two kinds of craft apart (~270 references). Search for the scalar `Vessel::propellant` and `set_propellant`, `systems::Consumables`, `Aboard::Fill`, D078's ideal couples (`rcs_authority` for the test craft), the test craft's single-part pipeline, `design_of`'s ids, `SUNSCATTER_CRAFT=test-craft`, `data/craft/test-craft`, and the tests that fly the test craft.
2. **Tests first.** Every test that uses the test craft moves to the Falcon 9, or to a small craft of parts built in code (a helper in `craft`, one per shape needed: a tank and an engine, a capsule with chutes …). Each test keeps what it checks; record any that cannot be kept, and why.
3. **The stress generator** (`craft::stress`, for benchmarks and the scale target) builds n parts of real catalog types on the parts path (tanks, engines, structure, panels), so the benchmarks measure the real path at 300, 1,000 and 5,000 parts. Keep `bench_scale` and `bench_coast` running on it.
4. **Delete the made-up path:** one mass law, one attitude model, one thermal fill, one coast mass. Delete the test craft's data, its new-flight entry and its env value. The playtest save (`./run play`) is regenerated from a Falcon 9 flight, or dropped if MISSION's tools replace it: record which.
5. **Decisions:** delete D074. Trim D064 to debug mode only (D123: a dead computer still counts in debug). Update D078 and D104 where they speak of the test craft. Update every citation in the same commit (`tools/check_decision_refs.sh`).

Small commits, each with its tests passing (`cargo test -p sim`, the game's tests, clippy). Determinism, warp and chunked = single-pass tests stay green throughout.

## State

(Update as you land: commits with hashes, the inventory, what is left in order, doc changes needed.)
