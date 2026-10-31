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

**INTERFACE-2 (2026-10-31), wrapped up at its context cap.** Continued INTERFACE-1's "Left" (below).

**Landed:**
- 0d43d75: a joint the craft gives no capacity (the MVac's mount, the heat shield's, a leg) reads dashes for its allowable, axial capacity and load, not 0 (`sim::systems::structure::rated`, one owner for the runtime and the screen; the summary counts them under "no capacity"); the condition's peak row says "peak interior"; the unused `SELECTED` tint removed. Finding 11, Left 3 and 5.
- ffc6b0a: Settings and Saves open centred under the menu bar and stop above the navball cluster (`layout::page_place`), the pause menu stepping aside meanwhile (`pause::menu_shown`); the help's window is solid (`theme::SOLID`), so the navball's numbers no longer show through it; Saves' empty-list hint takes its key from the key table, and its list scrolls. Findings 12, 16, 17.
- a77f74c: the MCP `command` tool (a function by name; an item's command by its label; `command {}` and `command {item}` list them), from the part card's list (`part_card::content::part_commands`, now the one list) and the systems screen's (`inspect::commands`); `set_controls` takes `rcs` and `rcs_fine`. Finding 10.
- 25f0bc4: the tracking station says TIME STOPPED, not the requested warp (`hud::time_words`, one owner with the time panel); a card's value that does not fit beside its label goes below it (`part_card::place::value_below`). Findings 14, 15.
- the docs commit after them: `interface-map.md` (findings 10–12 fixed, new findings 13–20), `mcp.md`, `owners.md` rows, this State, the plan's row.

**The flight (debug off, testing.md exactly):** max-Q at T+66 s, 34.6 kPa at 12.1 km; MECO at T+138 s, 65.3 km, pitch 32°: the worked example's numbers to the digit (the earlier fall-back was debug mode's undrained propellant). Insertion by a script of the doc's step 9 (`insert.py` in the scratchpad: the pitch that brings the vertical speed to zero at 200 km, 1 s pulses about up × heading): throttle cut at Pe 188 km, then 192 × 343 km at 28.7° after SECO (finding 20). A replay from the second-stage save matched round for round. Saves `ascent-t60`, `s2-ignition`, `orbit` in the scratchpad's `home/saves` (the build of 769b85d).

**Shots for the lead** (scratchpad `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface2/shots/`):
- the thermal mesh in orbit: `orbit_truth_dragon.trunk.png` (the trunk isolated, debug: its sunlit side green against the shaded side's teal), `orbit_thermal_truth.png` (the craft, debug, no cut), `orbit_thermal_sensors.png` (what the sensors give: the measured tanks coloured, the rest the neutral grid, the nodes behind the half cut);
- the MVac's bell burning: `s2burn_mvac_truth.png` (T+~330 s, debug: the radiatively cooled extension 1918 K of 2620 K, the regeneratively cooled throat blue; the right panel still shows finding 11's zeros, fixed since); `meco_thermal.png` (sensors, at MECO);
- before the fixes: `orbit_settings_orbits.png` (12), `orbit_help.png` (16), `orbit_cards.png` (15), `orbit_tracking.png` (14), `orbit_menus2.png` (13, Landing ▸ Thermal all dashes).

**Not yet seen on screen:** the fixes since 0d43d75 are tested (table tests, the game's whole suite, clippy) but not shot. The next agent should shoot them at HEAD: Settings and Saves in their place, the help over the navball, the tracking station paused, the capsule's card in orbit, the MVac's joint dashes, and a live `command {}` and `command {function: "Launch abort"}` refusal or run on the Falcon 9.

**Left, in order:**
1. The shots above.
2. Finding 13: Landing ▸ Thermal by the systems screen's rule (the measured parts nearest their limits, the rest counted; `menus::hottest` picks by the truth today). It needs `menus/data.rs` (PLANNER-4's hunk) and `hottest.rs` without its test-craft branch (ONEKIND-5).
3. MCP's last gaps: the target (Orbital → TARGET), the staging bar's edits, reading the notifications.
4. For other owners: finding 19 (the Crew Dragon has no cabin sensors), finding 20 (thrust after `throttle: 0` on the MVac, its 64 % floor), finding 18's note (a part's peak interior not kept through a warp-6 coast).

**Open for the owner (INTERFACE-2):**
- The help's window is opaque; every other panel stays translucent (the look is frozen, D123; this is the one change, for reading).
- Settings and Saves take the centre and the pause menu steps aside while one is open (Esc or × brings it back), rather than sitting beside it.
- `command` takes the labels the player sees (any case) and lists only what an item can do now (IGNITE when it is off, SHUT DOWN when it runs).

**INTERFACE-1 (2026-10-31), wrapped up at its context cap.**

**Landed:**
- def10d2: MCP `shoot` opens the help overlay (`screen: help`) and part cards (`cards`).
- 1183eb1: the staging bar's part cards stack in columns (`place::stack`), so none is pushed over another.
- 723ceec: the cleanup, from the map. The keys' labels come from the bindings (`keys::label`, Cmd on a Mac, Ctrl+Alt elsewhere). The help is generated from the bindings, the held keys, `MenuId`, Esc's levels and the ways-in table (`interface::access::WAYS`), and has a `?` on the menu bar. Esc backs out one level everywhere (`pause::on_escape`, `ESC_ORDER`: it now closes the tracking station and a body's menu instead of pausing over them). One Saves button. PAUSED sits under the menu bar. Settings no longer lists the menus. The tracking station's F7 label is gone.
- 85bb56e: the systems screen. The thermal layer draws the thermal network: each skin cell by its own temperature with the cells' grid, and the interior nodes as cubes behind a longitudinal cut-away (a slider, half by default). The meshes come from the sim's own surface, cells and volume grid, and read one texel per cell and node. A click isolates a part: the rest are ghosted and the camera eases to frame it. Esc or × puts the layer view back as it was. The framing fits the view both ways (it cut the nose before). `shoot` takes `cut`.
- the docs commit after them: `interface-map.md` (new), `systems-screen.md`, `flight-menus.md`, `systems/game.md`, `owners.md` rows, this State.

**The map:** [interface-map.md](../technical/interface-map.md): every screen, piece of information and control with its key, click path and MCP tool, and 12 findings (9 fixed).

**Shots for the lead** (scratchpad `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface/`):
- before: `shots/pad_sys_thermal.png` (whole parts by their hottest), `shots/pad_sys_selected_before.png` (the MVac hidden inside the interstage), `shots/pad_cards.png` (overlapping cards), `shots/pad_pause.png` (PAUSED over the bar).
- after: `shots2/c_t_craft_cut0.png` and `shots2/c_t_craft_cut50.png` (the thermal mesh: skin cells, then the nodes behind the cut), `shots3/c_pad_merlin_isolated.png` and `shots3/pad_restored.png` (isolated, then the layer view put back), `shots3/c_pad_mvac_thermal.png` (the MVac's nodes: it has no skin cells inside the interstage), `shots3/c_asc2_thermal_truth.png` and `shots3/c_asc2_merlin_truth.png` (ascent, T+109 s, debug), `shots3/pad_help.png`, `shots3/c_pause.png`, `shots3/pad_tracking.png`, `shots3/pad_cards.png`.

**Found while flying (not the interface):**
- The worked example's ascent (`testing.md`) no longer reaches orbit. After the same pitch kick (86.2°), the surface-prograde hold turned the stack flat: pitch 9° at 19.9 km at T+109 s, and it fell back to 1.8 km diving by T+232 s. `run_until {max_q}` never reported max-Q (it stopped on its 90 s bound twice).
- With debug mode on, the propellant never drains, so the stack keeps its full mass and the gravity turn collapses: fly with debug off, and turn it on only for paused shots.
- The Merlins' bells read 342 K / 1330 K after 112 s of burning (regeneratively cooled, so plausible); the MVac's radiatively cooled extension was not reached.
- On the pad, the sunlit and shaded sides do not differ after 30 minutes (the air holds the skin).

**Left, in order:**
1. The thermal mesh in orbit (a sunlit side against a shaded one) and the MVac's bell burning: needs a flight that reaches orbit (see above), or a save of one.
2. An MCP `command` tool in `game::agent` (a function by name, a part's command, RCS's mode): the map's finding 10. `agent.rs` is PLANNER-3's until its `set_plan` lands.
3. Finding 11: a joint's allowable line load and axial capacity read 0 where the craft gives none (a dash, D106).
4. Finding 12: the Settings window opens over the action and staging bars.
5. The rules' `SELECTED` tint is no longer used (a selection is an isolated part); remove it with its tests.
6. ONEKIND-4: the systems screen's tests here use `test_rocket`, not the test craft.

**Open for the owner:**
- The thermal layer's cut-away is longitudinal (the plane holds the craft's long axis and faces the camera) and cuts half the craft by default, so the interior nodes show at once. A part with no skin cells of its own (an engine inside its interstage) shows its nodes uncut.
- Colours under D106: a part a temperature sensor measures is coloured, the rest a neutral grey grid; debug mode colours everything. Each cell and node is coloured against its own limit (the existing ramp), so 290 K and 340 K look alike on a 1330 K bell.
- Selecting a part always isolates it (the view, PARTS, a summary row, a card's "more ›", a menu's link). A click on nothing changes nothing; a click on a ghost isolates that part, keeping the first layer view to go back to. Closing the screen puts the layer view back.
- The menu bar gained a `?` (the help), the only new thing on the flight view (D080).
- Esc closes the tracking station rather than pausing over it.
