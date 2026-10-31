# The map of the interface

Every piece of information and every control the player has, with every way to reach it: a key, a click path, an MCP tool. Made by INTERFACE (2026-10-31, [brief-interface.md](../plans/brief-interface.md)) from the code and from shots of the Falcon 9 flown headless through MCP (the pad set; the ascent and orbit set in the brief's State); walked again in orbit by INTERFACE-2 (findings 13–20), INTERFACE-3 (findings 21–26), INTERFACE-4 (findings 27–31) and INTERFACE-5 (findings 32–43: the pad, ascent, orbit, after separation, every layer, the map, tracking, pause, help). Decisions D080, D106, D107, D111, D112, D120, D123 (the interface exception), D126. Kept current with the interface: a change to a way in changes its row here. The help overlay (`?`) is generated from the same tables as the input (`interface::access`, `commands::keys`), so the keys in it cannot drift; this page adds the findings.

**Status:** the map is complete for the flight view, its menus and cards, the systems screen, the tracking station, pause, settings, saves and help. The findings marked *fixed* are done; the rest are listed under "Open" below.

## The screens and windows

| What | Key | Click path | MCP | Closes by |
|---|---|---|---|---|
| Flight view | — (the base) | — | `shoot` (default), `set_camera` | — |
| Task menus: Orbital, Landing/Reentry, Maneuver, Crew/ECLSS, Power, Comms | 1–6 | menu bar (top centre) | `shoot {menus}` | its key, the bar, its × |
| RCS menu | R (turns RCS on) | navball's RCS button | — (the mode: `set_controls {rcs, rcs_fine}`) | R, the button, its × (turns RCS off) |
| Part card | — | a part on the ship; a part icon on the staging bar | `shoot {cards}` | its ×, a second click on its part |
| Systems screen | Tab | action bar SYSTEMS; a menu's "The systems screen: LAYER" link; a part card's "more ›"; a notification's link | `shoot {screen: systems, layer, part}` | Tab, Esc, its close button |
| The systems screen's isolated part | — | a part in the view, in PARTS, or a row about one | `shoot {screen: systems, part}` | Esc or its panel's × (back to the layer view as it was) |
| Tracking station | M | pause → Tracking station | `go_to_mission_control`, `shoot {screen: tracking}` | M, Esc, Back to flight |
| Pause menu | Esc | — | `shoot {screen: pause}` | Esc, Resume |
| Settings (Orbits, Controls, Interface, Visuals) | — | pause → Settings (centred under the menu bar; the pause menu steps aside) | `shoot {screen: settings, settings_tab}` | Esc, its × (back to the pause menu) |
| Saves | Cmd+S / Cmd+L quick (Ctrl+Alt elsewhere) | pause → Saves (placed as Settings) | `save_game`, `load_game`, `shoot {screen: saves}` | Esc, its × (back to the pause menu) |
| New flight, revert to launch, quit | — | pause → New flight… / Revert to launch… / Quit | `new_flight` | Esc (back to the menu), Cancel |
| Help: the keys and every way in | `?` | menu bar `?`; pause → Help | `shoot {screen: help}` | `?`, Esc, its × |
| Burn planner | — | Maneuver (3) → Open the burn planner; Orbital → the planner button | `set_plan` | Esc, its × |
| Body menu (distance, Focus) | — | double-click a body | — | Esc, its × |
| Notifications | — (shown when a message comes) | ⏷ / ⏶ fold it | `notifications {after}` (numbered, as the player reads them) | ⏷ folds it |
| Performance overlay | — | pause → Performance overlay | `profile` (developer flag) | the same button |
| Debug mode | — | pause → Debug mode | `set_debug` | the same button |

## The flight view's information

| Information | Where | Reached |
|---|---|---|
| The origin's clock (TT, the site), warp, time stopped, frame rate | time panel, top left | always shown |
| Speed and its frame (ORBIT / SURFACE / TARGET, AUTO / LOCK) | navball cluster | always; the switch by click, a click on the ball cycles it |
| Altitude (and its datum), Ap and Pe with their times, the g-load and its limit | navball cluster | always; the datum by click on ALT; the limit on hover |
| SAS, hold modes, RCS, throttle, flight state (CLAMPED, FLIGHT, POWERED…) | navball cluster | always; T, R, clicks, Z / X / Shift / Ctrl |
| Stages: propellant per stage, burnout, Δv; the active stage's thrust, TWR, mass, Isp | staging bar, right edge | always; CURRENT AIR / VACUUM switch under it; MCP `get_vessel`'s `stages.sequence` (`air`) |
| The craft's functions (abort only while it saves the crew, D120) | action bar, top right | always; click |
| A part's live essentials and commands | part card | click the part or its icon |
| Everything about a part, by layer | systems screen, right column | the card's "more ›", or Tab and a click |
| The craft as a whole by layer (mass, joints, temperatures, tanks, power, data, health) | systems screen, left column (SYSTEM-WIDE) | Tab or SYSTEMS: one click from flight |
| Orbit elements, target, closest approach, ship clock | Orbital (1) | key or bar |
| Impact, braking, entry, the parts nearest their limits, parachute, attitude | Landing (2) | key or bar |
| Planned burns | Maneuver (3) | key or bar |
| Crew, cabin air, life support, injuries | Crew (4) | key or bar |
| Buses, batteries, what has no power | Power (5) | key or bar |
| The link home, a probe's delay | Comms (6) | key or bar |
| RCS authority, thruster blocks, feeds | RCS menu | R or the navball's RCS button |
| Messages (saves, events, commands' replies) with links to a vessel or the systems screen | notifications, bottom left | when one comes |

## The controls

| Control | Key | Click | MCP |
|---|---|---|---|
| Throttle | Shift / Ctrl, Z / X | navball throttle bar | `set_controls {throttle}` |
| Rotation, translation | W S, A D, Q E; I K, J L, H N | — | `set_controls {rotate}` (rotation only) |
| SAS, hold modes | T | navball SAS and the holds (click only, D111) | `set_controls {sas, hold, speed_reference}` |
| RCS on / off, mode, fine, blocks | R | navball RCS; the RCS menu | `set_controls {rcs, rcs_fine}`; a block: `command {item, do: "RCS … on"}` |
| Next stage | Space | — | `stage` |
| A stage's edits (move, delete, a command's move, run again) | — | staging bar drags, ×, AGAIN | `edit_stages` (the bar's rules; the rows from `get_vessel`'s `stages.sequence`) |
| Functions (abort, grid fins, legs…) | — (no craft-specific keys, D111) | action bar; the systems screen's run buttons | `command {function}` |
| A part's commands (ignite, valve, deploy, separate…) | — | part card; systems screen right column | `command {item, do}` (`command {}` lists them) |
| Parachutes | P | the card's ARM PARACHUTES | `command {item, do: "ARM PARACHUTES"}` |
| Warp | `.` `,` `/` | the time panel's arrows | `set_warp`, `step`, `run_until`, `pause` |
| Vessels | `[` `]` | a notification's link; tracking station Switch to | `switch_vessel` |
| Target | — | Orbital → TARGET pick list | `set_target` (the same choices); `get_state` reads it |
| Camera | drag, scroll, F, `` ` `` | double-click a body → Focus | `set_camera` |
| Stage sequence, functions, pins | — | systems screen editors | — |
| Saves | Cmd+S, Cmd+L | pause → Saves | `save_game`, `load_game` |

## Findings

Hidden, missing a way in, reached two ways that disagree, closed differently. *Fixed* means done by INTERFACE (the brief's State has the commits).

1. **Esc did not back out one level everywhere.** It opened the pause menu over the tracking station and over a body menu instead of closing them, and it could not undo a selection in the systems screen. *Fixed:* one Esc stack (`pause::on_escape`), the innermost first: help, settings, saves, the body menu, the planner, the systems screen's isolated part, the systems screen, the tracking station, then the pause menu and its pages.
2. **Two ways that disagreed.** The tracking station's button said "Back to flight (F7)", but the key is M (D111 removed the F-keys); the module docs still said F7 (tracking), F3 and F4 (settings, the overlay). *Fixed:* the labels come from the key table.
3. **The help was a hand-kept copy** of the key table, with no ways in by click and nothing on the systems screen, the Esc order or the agent. *Fixed:* generated from `commands::keys::BINDINGS`, the held keys, `MenuId` and the ways-in table, with a `?` button on the menu bar so it has a way in by click.
4. **The systems screen's framing cut the craft's nose** off a wide, short view (it framed by the craft's radius without the view's aspect). *Fixed:* `rules::frame` fits the bounds to the view's field of view both ways.
5. **A selected part could not be seen** when another part hid it (the MVac inside the interstage, a tank inside a fairing). *Fixed:* a click isolates it (the rest ghosted, the camera framing it).
6. **Part cards stacked by the staging bar overlapped** when several were open: egui moved a card that ran past the bottom back up over the one above. *Fixed:* `place::stack` puts them in columns by their drawn heights.
7. **The pause menu's PAUSED label** lay over the menu bar's MANEUVER entry. *Fixed:* it sits under the bar.
8. **Settings → Interface's menu checkboxes** were a third way to open the task menus (with the bar and the keys). *Fixed:* removed; menus are opened from the bar or their keys and remembered as before.
9. **Save and Load** were two pause-menu buttons opening the same window. *Fixed:* one "Saves" button.
10. **MCP could not reach** a craft's functions (the abort, grid fins, legs), a part's commands, RCS's mode and blocks. *Fixed* (INTERFACE-2): the `command` tool runs a function by name or an item's command by its label, from the same list as the part card (`part_card::content::part_commands`) and the systems screen (`inspect::commands`), and lists them; `set_controls` takes `rcs` and `rcs_fine`. The rest *fixed* (INTERFACE-3): `set_target` (the picker's choices, `navball::target_choices`), `edit_stages` (the bar's rules, `stage_bar::rows::check`) with the bar's rows and figures in `get_vessel`, and `notifications`.
11. **A joint's allowable line load and axial capacity read 0** for a bolted joint with no figures (the MVac's), and its load read a measured 0 %. *Fixed* (INTERFACE-2): `sim::systems::structure::rated` says whether the craft gives a joint a capacity; without one the screen shows dashes and the summary counts it under "no capacity".
12. **The Settings window** opened over the action bar and the staging bar, its × on the SYSTEMS button, and was cut at the bottom. *Fixed* (INTERFACE-2): Settings and Saves open centred under the menu bar and stop above the navball cluster (`layout::page_place`), scrolling within that; the pause menu steps aside while one is open (`pause::menu_shown`), and Esc or × brings it back.
13. **Landing ▸ Thermal disagreed with the systems screen.** In orbit it showed every part as "—" (S2.TANK too) while the systems screen showed the second stage's tank measured at 373 K / 422 K: the menu shows values only when every part's temperature is measured, and picks its parts by the truth (`menus::hottest`), so which parts it lists leaks what no sensor measures. Two owners of "the parts nearest their limits". *Fixed* (INTERFACE-3): one rule, `systems_screen::rules::nearest_limits`, for the summary and the menu (`menus::hottest`): each part by its skin cell or interior node nearest its own limit, only where a sensor measures it (every part in debug mode), the rest counted ("NOT MEASURED n parts"). The menu's separate INTERIOR row (the craft's hottest node, from the truth) went: each part's row includes its interior.
14. **The tracking station said "warp 1000x" while time was stopped** (the time panel: TIME STOPPED); it showed the requested level, not the one in effect. *Fixed* (INTERFACE-2): both say it through `hud::time_words`.
15. **A part card's long label runs into its value** ("pressure (highe165.0 bar" on the Dragon capsule's card in orbit). *Open.*
16. **The help could not be read over the navball cluster:** the panel's translucent fill let the cluster's bright numbers show through the help's text. *Fixed* (INTERFACE-2): the help's window is solid (`theme::SOLID`).
17. **The Saves window** said "Cmd+S" whatever the platform and had no scroll for a long list. *Fixed* (INTERFACE-2): the key's label comes from the key table; the list scrolls.
18. **"peak temperature" read below the temperature now** (the trunk's skin at 353 K, its peak 295 K): the row is the interior's peak, against its rating. *Fixed* (INTERFACE-2): labelled "peak interior". (In orbit the interior's current reading was also above its peak, 298 K against 295 K: the part history may not be kept through a warp-6 coast; for the sim.)
19. **The Crew Dragon's cabin** (Crew menu: pressure, O₂, CO₂, temperature) reads all dashes: the craft has no cabin sensors in its data. Craft data, not the interface (D106 is right to dash it).
20. **After `throttle: 0` and SECO** the Falcon 9's apoapsis rose about 80 km in the next 2 s (266 → 343 km): the MVac's 64 % throttle floor may hold thrust until the shutdown lands. For the sim (propulsion).

21. **PAUSED lay over the Settings window's title** (and the help's, opened from the pause menu): both were placed just under the menu bar. *Fixed* (INTERFACE-3): Settings, Saves and the help start below the menu bar and the PAUSED banner, whichever reaches lowest (`layout::page_place_in`, `help_place_in`, from the areas drawn last frame).
22. **A menu cut to its share of the dock gave no sign it went on.** With Orbital and Landing open, Orbital ended at TRUE ANOMALY: its TARGET picker and SHIP CLOCK were below, reachable only by scrolling over it, and egui's floating scroll bars show only while hovered. *Fixed* (INTERFACE-3): every scroll area shows a thin bar whenever its list is cut (`theme::scroll`, egui's `thin` style).
23. **MCP `command {}` listed functions the action bar and the systems screen do not offer:** in orbit, the booster's three engines, one engine, engines off, grid fins and legs, after the booster had gone. *Fixed* (INTERFACE-3): `command` lists and runs only the functions in the piece's view (`program::view`) that it offers (`program::offered`: an abort only while it saves the crew, D120), and refuses the rest with why ("acts on nothing aboard", "would save no one here").
24. **`shoot` could not show the Saves window**, and `menus: ["rcs"]` showed nothing without a word (the RCS menu follows RCS on). *Fixed* (INTERFACE-3): `screen: saves`; the RCS menu with RCS off is refused, saying to turn RCS on.

25. **A part's temperature read two ways that disagreed.** In orbit the capsule's card said "temperature 344 K (rated 1650 K)", the systems screen "skin 397 K / 2000 K, interior 291 K / 1650 K": the card showed the mean of the part's skin cells against the interior's rating. *Fixed* (INTERFACE-3): the card shows the systems screen's two rows, the skin cell and the interior node nearest their own limits, from one owner (`systems_screen::rules::vessel_temperatures`, `part_temperatures`); the summary's "nearest their limits" list is one rule too (`rules::nearest_limits`, the measured parts ranked, the rest counted).
26. **Lists whose scroll bar fell mid-panel** once bars showed (finding 22): the notifications, Settings' bodies (a 220 px list of 30 bodies, itself hidden before), the tracking station's list and the systems screen's stage editor shrank to their rows' width. *Fixed* (INTERFACE-3): they take their panel's width, so the bar sits at its edge. A menu's section heading wider than the menu (the Orbital's "ORBIT NOW · two-body · about Earth's centre" beside a bar) wraps to a second line instead of being cut.

27. **Craft in orbit were drawn dark red above a straight line across the view** whenever the camera looked up at them more than ~15° (DOCS-3's "red translucent cone" at the second stage's forward end after Dragon's separation): not a part or a ghost, but the sky's compositing (`render_sky.wgsl`) giving the craft the ground's aerial perspective from space, which for a rising ray read the light back down through the planet. *Fixed* (INTERFACE-4): geometry the ray meets before the air takes none (`sky::haze::air_before`, mirrored for its table test). Before: `scratchpad/interface4/shots/h_y0_zoom.png`, `e_grid.png`; after: `j_grid.png`.
28. **MCP: time stood still while `get_state` said it ran** (warp 100×, warp achieved 0) after a shot taken during the benchmark: each saved the clock's hold and put it back when done, so the shot restored the benchmark's hold after it had ended. *Fixed* (INTERFACE-4): the clock is held while any shot or benchmark is in flight (`session::clock_held`).
29. **Notifications after a load disagreed with the flight.** Loading a save in orbit replayed its whole log at once (16 messages); loading an earlier one, or reverting to launch, showed none of the new timeline's events dated before the old one's last (no Ignition, Liftoff or MECO after a revert from orbit), so "Dragon separation" stayed the newest message with Dragon attached. What had been seen was kept per vessel id, and a save brings its ids back. *Fixed* (INTERFACE-4): a load forgets what was seen (`events::to_notify`, `SimState::restores`). The list itself stays the session's log, the load's own message ("Quickloaded") after the old flight's.
30. **`get_vessel`'s apsides lagged under thrust** (PLANNER-4: 343.9 km against 360.0 mid-burn): they come from the live prediction, recomputed a few times a second of real time. *Fixed* (INTERFACE-4): `prediction_age_s` says how old the predicted path is while the vessel flies live, and `now_two_body` gives the apsides from the centre of the state now (the Orbital menu's ORBIT NOW).
31. **MCP ignored a misspelt argument without a word** (`set_camera {distance: 40, pitch: -15}` answered as if the camera had moved). *Fixed* (INTERFACE-4): the protocol refuses an argument its tool's schema does not list, naming the tool's own (`mcp::protocol::unknown_arguments`).

32. **The g-load read three ways.** At max-Q the navball's G said 0.42 g while the trajectory felt 2.03 g (its change of velocity less gravity) and the systems screen said 2.0 g: under thrust the navball recomputed the proper acceleration with the coast's drag area, averaged over every direction (a rocket's broadside included). In orbit the systems screen's g-limit said 4.4 g / 7.5 g, the last burn tick's, kept through the coast, against the navball's 0.00 g; on the pad "none: not accelerating" against 1.00 g. *Fixed* (INTERFACE-5): one owner, `navball::proper_accel` and `felt` (the last live tick's felt acceleration while the vessel flies live, the coast's drag, the ground's push), which `sim::systems::g_limit` now takes. Before: `scratchpad/interface5/shots/b_asc2_nav.png`, `b_asc2_struct_crop.png`, `c_sys_structural.png`; after: `f_g_compare.png`.
33. **A load showed the last flight's impact.** After loading an orbit save the Landing menu listed the ascent save's impact (IMPACT IN 32s, its point) above "no impact predicted": the prediction was kept by vessel id, and a save brings its ids back; the menus' snapshot waited for its next tenth of a second and their anti-flicker hold kept the vanished rows 1.5 s more. *Fixed* (INTERFACE-5): `SimState::flight` (the active vessel and the load count) keys the prediction (`LandingPrediction::of`), the snapshot (`menus::data::refresh_due`) and the hold (`menus::view::Held`). Before: `c_orbit.png`; after: `f_g_compare.png` (bottom).
34. **"no buses: nothing draws power" on the pad** (the Power menu and the Electrical summary) for a craft with three buses: the buses' readout is the last power step's, and the pad's first systems lattice point steps nothing (the next is a minute on). *Fixed* in the interface (INTERFACE-5): the craft's buses are listed, "— (not read yet)" (`f_pad_power_c.png`). For the sim: a power step of no length at a lattice's first point would give the readout at once.
35. **The Electrical summary named things in capitals** (S2.BUS, DRAGON.BATTERY#1, the Power menu's style) where every other layer uses ids. *Fixed* (INTERFACE-5): `power::sections_by_id` (`f_pad_elec_c.png`).
36. **The data layer counted the pad's clamp**: "part temperatures 3 of 33" where the thermal layer counts 3 measured and 29 not, and health 32. *Fixed* (INTERFACE-5, `f_pad_data_c.png`).
37. **MCP: a shot's `width`/`height` stayed** for every later shot, where the tool says everything is put back. *Fixed* (INTERFACE-5).
38. **MCP: two overlapping shots shared the screens**: each could show the other's, and the second put the first's screens back after the first had restored the player's. *Fixed* (INTERFACE-5): a shot waits for the one in flight, the calls behind it too (`session::starting`).
39. **The pause menu's "Saves  Cmd+S"** read like "Tracking station  M", whose key opens it, but Cmd+S quicksaves without the window. *Fixed* (INTERFACE-5): "Saves"; the window names both quick keys (`saves::quick_keys`).
40. **The test craft's stage stack was still there** (ONEKIND-7's d2): Space's activation (`staging::stack`, `activate_next`), the staging bar's plain rows, and Landing ▸ PARACHUTE's canopies, which only the test craft's "stage 0" filled. *Fixed* (INTERFACE-5): Space and the bar send `Stage` of the active vessel; PARACHUTE lists the craft's own canopies (`staging::canopies`: drogues and mains, armed, "opens below", reefed, torn).
41. *Open:* the systems screen says "mass —" (physical, structural) while the staging bar gives the stage's mass (391 t → 151 t at T+60 s): the screen dashes the whole when any tank is unmeasured (Dragon's four), the bar gives it to the precision the sensors allow (each unmeasured tank empty to full, `sim::systems::estimate`). The mass now with its precision is the sim's to give (an estimate's owner), then both read it.
42. *Open, small:* every engine's card says "ignitions left: the pad's supply" while clamped, Dragon's hypergolic Dracos included, for which "unlimited" is truer; the words are written twice (`part_card::content::engine`, `part_card::inside::engines`).
43. *Open, cosmetic (the look is frozen, D123):* on the map (the camera at 20,000 km) the periapsis label lies over the craft's marker.

Seen and fine in this walk: the pad's cards placed by the bar (`a_cards.png`), the help (`a_help.png`), the pause menu, the tracking station, the menus at the pad and in orbit, the systems screen's seven layers on the pad and at T+60 s, Dragon alone after separation (`g_sep.png`), the map (`g_map.png`).

## Not mapped

- The planner's own controls (PLANNER-3 is moving it onto nodes).
