# Brief: INTERFACE, every piece of information and every control within easy reach (2026-10-31)

The owner, 2026-10-31: "At least one agent should work on cleaning things up for the user, making it easy to access all the information and controls, plus adding a feature for the vessel view: thermal actually shows the grid level thermal mesh, plus clicking on a part isolates it and zooms in while pressing esc or closing that window goes back to the original layer view of the part."

D123 froze visual work until both flights fly. The owner's request is an exception for the interface (recorded in D123): access to information and controls, and the systems screen's two features. The look itself stays frozen: no restyling, no plume, cloud or terrain work.

Follow [agent-rules.md](agent-rules.md). Your name is INTERFACE and your target dir is `target/agents/interface` (a cold start; you need a release build of the game for headless runs).

Two other agents run at the same time:
- **PLANNER-3** ([brief-planner.md](brief-planner.md)) moves the game's planner onto nodes: `game::planner`, `interface/menus/maneuver.rs`, the trajectory predictions, and MCP's `set_plan` in `game::agent`. Leave those files to it, and list what they need in your State.
- **RESEARCH** ([brief-research.md](brief-research.md)) writes docs only.

ONEKIND-4 starts later and moves the game's tests off the test craft (`menus/*` tests, `stage_bar/rows_tests`, `navball`, `saves`). Commit often, so it can work on top of you.

## Read first

- D080, D106, D107, D108, D111, D112, D113, D120, D123, D126 in `docs/decisions.md`.
- `docs/systems/game.md`; `docs/technical/systems-screen.md`, `flight-menus.md`, `staging-bar.md`, `ship-commands.md`, `aero-thermal.md` (the skin cells and the volume grid: `sim::craft::volume`, `sim::craft::design`, `sim::thermal`), `knowledge.md`, `testing.md` and `mcp.md` (how to run the game headless and take shots).
- Code: `game::systems_screen` (`mod.rs`, `view.rs`, `rules.rs` and its `pick`, `ui.rs`, `summary.rs`, `inspect.rs`), `game::interface` (`action_bar`, `help`, `menus`, `part_card`, `stage_bar`, `pause`), `game::settings_ui`, `game::map_view`.

## The work, in order

1. **The map of the interface** → `docs/technical/interface-map.md`. Fly the Falcon 9 headless through MCP (pad, ascent, orbit, separation) and shoot every screen:
   - the flight view, the navball cluster, the staging and action bars, every menu, the part cards;
   - the systems screen at each layer;
   - the map and the tracking station;
   - pause, settings and help.

   List every piece of information and every control, with every way to reach it (key, click path, MCP tool). Mark what is hidden, missing a way in, reached two different ways that disagree, or closed in different ways. Look at the shots with the Read tool.
2. **The cleanup, from the map**, in small commits, each with its table tests (rule 8: rules as pure functions, Bevy systems only wire them):
   - one obvious way into each thing;
   - Esc always backs out one level;
   - a help overlay that lists every key and every way in, generated from the same tables the input uses, so it cannot drift;
   - the craft's information one click from the flight view;
   - anything the map found unreachable.

   Keep the owner's earlier rules: no pop-ups in the systems screen (the selection shows in the right panel), the interface never shows something wrong (D106), and debug mode shows the truth.
3. **The thermal layer shows the thermal mesh itself.** Today it colours each part by its hottest cell or node. Draw what the thermal network is instead:
   - each skin cell coloured on the craft's surface by its own temperature;
   - the interior nodes (the volume grid's cubes, `node_size`) as a grid you can see into: translucent, or a cut-away you can move.

   Colours follow D106: a cell a sensor measures is coloured, and the rest show the grid in a neutral "not measured" shade; debug mode colours everything. Record that default under "Open for the owner". Keep it cheap for a craft of many cells. The stress craft (`stress-<n>`) shows the cost.
4. **Clicking a part isolates it and zooms in.**
   - The other parts hide or ghost, and the camera frames the part.
   - The right panel shows the part, as now.
   - **Esc**, or closing the part's panel (its ×), goes back to the layer view as it was: the same layer, camera pose and selection state. A second Esc closes the screen, as now.
   - The part list (PARTS) isolates the same way.
   - The camera's framing and the Esc stack are pure functions with table tests.
5. **Mission check:** shots of the cleaned-up screens, the thermal mesh during the Falcon 9's ascent (the nozzles and the base heating) and in orbit (a sunlit side against a shaded one), and a part isolated and restored. List the shots in your State for the lead.
6. **Docs:** `systems-screen.md` (the thermal mesh and isolation), `flight-menus.md`, `interface-map.md`, the status lines in `systems/game.md`, `owners.md` rows.

Small commits, each passing fmt, clippy and the game's tests you touched (`cargo test -p game <filter>`), plus `cargo test -p sim` if you touch sim. One game at a time, under the lock in agent-rules.

## State

(Update as you go: the map, commits with hashes, shots for the lead, what is left in order, Open for the owner.)
