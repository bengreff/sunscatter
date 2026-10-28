# Brief: VESSEL (2026-10-27)

The VESSEL row of [flight-ui-2026-10-27.md](flight-ui-2026-10-27.md): D112's controls. Rules: [agent-rules.md](agent-rules.md). Your name: `vessel` (target dir `target/agents/vessel`).

Read first: D112 and D034 (a probe acts on the controls that have reached it) in `docs/decisions.md`; the owner's answers 27 and 43 in [interview-2026-10-27-flight-ui.md](interview-2026-10-27-flight-ui.md); `docs/architecture.md` (search Controls, fleet, bubble, remote, agent/MCP); `docs/design/motion-model.md` (determinism, chunked = single pass).

## Today

- One set of controls for the active vessel: `sim::save` `controls` (`crates/sim/src/save.rs:58`), `SimState::controls` (`crates/game/src/state.rs:93`). In the step (`state.rs` ~391–421) the active vessel gets `active_controls` (delayed through `sim.remote` for an uncrewed vessel), every other vessel a shared `passive`. Switching resets the controls (`state.rs` ~329).

## Wanted (owner: "Each vessel keeps its own controls (throttle, SAS, holds) … plus ability for a script to control the inactive vessel")

1. **Each vessel owns its latched controls** (throttle, SAS and its hold mode, RCS mode, anything else latched), kept with the vessel in the save. Switching neither resets them nor carries them to the other vessel. The pilot's momentary inputs (rotate/translate keys) go only to the active vessel. Separation: both halves start from the controls the craft had at that moment (the booster keeps its SAS hold and a throttle of 0 unless its sequence says otherwise; decide what is logical and write it down).
2. **D034 per vessel**: each uncrewed vessel acts on the controls that have reached it after its own delay, so `remote` belongs to each vessel, not the session.
3. **Scripts fly any vessel**: the MCP/agent hook (`crates/game/src/agent.rs`, `crates/mcp`) takes an optional vessel id on its control and command tools (default: the active one) and changes that vessel's controls; a test that a script holds an inactive vessel's attitude or burns its engine while another is active, in and out of the physics bubble.
4. Keep one accessor for "the active vessel's controls" (`SimState::controls()` / `controls_mut()` or similar) so the interface code (navball, menus, NAV's `GameCommand::SetRcs`) needs at most a rename.
5. Check `[` / `]` step through vessels nearest first, both ways (NAV's key table, `commands/keys.rs`); fix it if not.
6. Tests: save round trip with several vessels' controls; switching keeps both sets; determinism and chunked = single pass with an inactive vessel under its own controls; the invariance tests still pass.

Out of scope (write it under "left" if not done already): D112's "joined by data" vessel definition (one vessel across a radio link); the interface for picking which vessel a menu shows.

## Watch out

- **NAV is running** in `crates/game/src/{navball,interface,commands,systems_screen}` and may still touch `state.rs` (`SetRcs`). Do the sim side first (`sim::vessel`, `sim::save`, the step's controls closure), then the game side once `git status` shows `state.rs` clean.
- Saves are not compatible across versions (D063), but the playtest save (`./run play`) embeds the format: if it breaks, regenerate it from the demo's Orbit step (`orbit_save.ron`, take the demo lock) or write down that the lead must.
- Done when: playable, the plan's VESSEL row and `docs/architecture.md` updated, a demo check optional (switch to the booster after separation; it keeps its own SAS).

## State (first agent stopped at its context budget, 2026-10-27)

**Landed** (f048de0): the sim side. `sim::vessel::Helm` (`crates/sim/src/vessel/helm.rs`): each vessel's controls as set, what has reached it, and the settings in flight (only changes are queued, so the controls are the same at every instant as queuing every frame; tested). `Vessel::controls()`/`controls_mut()`/`received_controls()`/`controls_in_flight()`/`acting_controls(now, delay)`; `sim::vessel::fleet_controls(fleet, now, delay_of)` gives a step's controls by fleet index. `Controls::idle()` (a new vessel: SAS on, throttle 0), `for_piece()`, `released()`. A split's pieces other than the one keeping the id (for the Falcon 9: the booster; Dragon's capsule is part 0) start from `for_piece()`: hold and SAS kept, throttle 0, no rotation (decided: logical, and the craft's sequence still runs its own engines); the kept piece keeps everything. The save has no session `controls` any more (version 13; the playtest save converted by hand). Tests: `crates/sim/tests/vessel_controls.rs` (an inactive uncrewed probe holds prograde and burns while the active vessel coasts, in and out of the bubble, 0.1 s = 0.5 s frames bit for bit), the save round trip of several vessels' controls and settings in flight (`save_load.rs`), the split's controls (`ship_tests.rs`).

**Landed by the second agent** (the game side): 4b2a144 `SimState` has no session controls or `RemoteControls`; `SimState::controls()`/`controls_mut()` are the active vessel's; `SimState::step(target, delay_of, deadline)` gives every vessel its helm's controls (`fleet_controls`, rotation zeroed above physics warp), then the bubble and the fleet; switching keeps both sets (the vessel left gets `released()`). f5007c6 `left_lit_notice` says it burns on at its own throttle. 14f5288 `GameCommand::SetControls { vessel, throttle, sas, hold }` (`commands::apply_controls`, routed by `commands::controls_route`); MCP `set_controls` takes `id`, answers `arrives_in_s`; a `stage` tool; `state_tests::a_script_flies_an_inactive_probe_through_the_step` (0.1 s = 0.5 s frames). 8a9b66e `[`/`]` nearest first (`tracking::Cycle`, table test). Docs: `docs/architecture.md` rows (helm, commands, fleet, tracking), `docs/mcp.md`, the plan's row.

**Left:** a demo check on screen (switch to the booster after separation: it keeps its SAS hold; switch back: Dragon's throttle as left); D112's "joined by data" vessel (one vessel across a radio link); a menu or navball showing another vessel's controls; the playtest save is unchanged by this round (format 13).
