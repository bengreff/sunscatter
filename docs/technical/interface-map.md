# The map of the interface

Every piece of information and every control the player has, with every way to reach it: a key, a click path, an MCP tool. Made by INTERFACE (2026-10-31, [brief-interface.md](../plans/brief-interface.md)) from the code and from shots of the Falcon 9 flown headless through MCP (the pad set; the ascent and orbit set in the brief's State); walked again in orbit by INTERFACE-2 (findings 13–20), INTERFACE-3 (findings 21–26), INTERFACE-4 (findings 27–31) and INTERFACE-5 (findings 32–43: the pad, ascent, orbit, after separation, every layer, the map, tracking, pause, help) INTERFACE-6 (findings 44–48: the quantities shown by two rules) INTERFACE-7 (findings 47–49 fixed; the walk at HEAD) and INTERFACE-8 (findings 55–56; the parity of the player and the agent, below). Decisions D080, D106, D107, D111, D112, D120, D123 (the interface exception), D126. Kept current with the interface: a change to a way in changes its row here. The help overlay (`?`) is generated from the same tables as the input (`interface::access`, `commands::keys`), so the keys in it cannot drift; this page adds the findings.

**Status:** the map is complete for the flight view, its menus and cards, the systems screen, the tracking station, pause, settings, saves and help. The findings marked *fixed* are done; the rest are listed under "Open" below.

## The screens and windows

| What | Key | Click path | MCP | Closes by |
|---|---|---|---|---|
| Flight view | — (the base) | — | `shoot` (default), `set_camera` | — |
| Task menus: Orbital, Landing/Reentry, Maneuver, Crew/ECLSS, Power, Comms | 1–6 | menu bar (top centre) | `shoot {menus}` | its key, the bar, its × |
| RCS menu | R (turns RCS on) | navball's RCS button | — (the mode: `set_controls {rcs, rcs_fine}`) | R, the button, its × (turns RCS off) |
| Part card | — | a part on the ship; a part icon on the staging bar | `shoot {cards}` | its ×, a second click on its part |
| Systems screen | Tab | action bar SYSTEMS; a menu's "The systems screen: LAYER" link; a part card's "more ›"; a notification's link | `shoot {screen: systems, layer, part}` | Tab, Esc, its close button |
| The systems screen's isolated part | — | a part in the view, in PARTS, or a row about one | `shoot {screen: systems, part}` (a component's id selects the component) | Esc or its panel's × (back to the layer view as it was) |
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
15. **A part card's long label runs into its value** ("pressure (highe165.0 bar" on the Dragon capsule's card in orbit). *Fixed* (INTERFACE-2, 25f0bc4): a value that does not fit beside its label goes below it (`part_card::place::value_below`).
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
41. **The systems screen said "mass —"** (Physical, Structural) while the staging bar gave the stage's mass (391 t → 151 t at T+60 s): the screen dashed the whole when any tank was unmeasured (Dragon's four), the bar gave it to the precision the sensors allow; on the pad the screen also counted the launch mount the bar leaves out. *Fixed* (INTERFACE-6): one owner, `sim::systems::estimate::aboard` (the walk's own sum and bounds: a burn's start mass is it, tested equal), the dry mass with it; the screen reads it through `format::mass`. The centre of mass was still a dash while a tank was unmeasured and counted the mount on the pad (28.9 m above the mount's foot); *fixed* (INTERFACE-7): `sim::systems::estimate::centre`, the same parts aboard over the lowest contact point aboard, its precision the farthest the tanks' known ranges could move it (`format::metres`).
42. **Ignitions left read three ways:** the card said "the pad's supply" of every engine while clamped, Dragon's hypergolic Dracos included; the systems screen never named the pad. *Fixed* (INTERFACE-6): one rule, `systems_screen::inspect::ignitions_left`, for the screen and the card's engine and engine groups: "unlimited (hypergolic)" whatever the pad, else the count with "(the pad lights it)" while clamped.
43. **On the map the periapsis label lay over the craft** just past periapsis (the camera at 20,000 km). *Fixed* (INTERFACE-6): `map_view::label_at` puts an apsis label in the first corner clear of every drawn icon.
44. **A tank's contents read three ways:** the staging bar and the part card to the level sensor's precision ("150/287.4 t"), the systems screen's tank and its Plumbing summary the truth to ten kilograms ("150.37 t / 287.41 t"). *Fixed* (INTERFACE-6): one rule, `format::amount`.
45. **LOCAL TWR (Landing ▸ BRAKING, MCP `get_landing_prediction`) was a dash whenever a tank was unmeasured**, which on the Falcon 9 is always, where the staging bar gives its TWR to the sensors' precision (finding 41's pattern). *Fixed* (INTERFACE-6): `menus::data::local_twr`, the braking model's value with the mass's precision; both TWRs through `format::ratio`.
46. **A second rule for Δv, TWR and mass sat unused** in the menus' snapshot (`CraftData`: a lumped rocket equation on the vessel less its mount, its full load rebuilt every tenth of a second), shown nowhere. *Fixed* (INTERFACE-6): deleted with `hud::{vehicle_mass, delta_v, twr}`.
47. **A battery's charge read four ways:** the Power menu "38 / 50 Wh (76 %)" and the systems screen "38 / 50 Wh" from the truth, the part card's battery and battery group "76 %" from the reading. *Fixed* (INTERFACE-7): one rule, `format::charge`: `format::amount`'s in Wh (kWh from 10 kWh), the charge rounded to the sensor's 1 % in that unit, with its share of full (`format::share`, the tank inspector's share made one owner), a dash where nothing measures it: "1400/2000 Wh (70 %)" on the Power menu, the card's batteries and the inspector alike.
48. **A bottle's and an RCS feed's contents read the truth to 0.1 kg** when measured (the RCS menu's PROPELLANT "35.2 / 35.6 kg", the systems screen's bottle), where the sensor gives a pressure to its precision. *Fixed* (INTERFACE-7): `systems_screen::inspect::bottle_contents`, the transducer's reading as a mass by the gas law at the bottle's temperature (D106's simplified error), read through `format::amount` by the RCS menu's feeds (a tank feed by its level sensor) and the systems screen's bottle; a dash with the full load where nothing measures it.

49. **Pressures and tank temperatures read by several rules:** a bottle's pressure, through a 0.41 bar transducer, read "300 bar" on the part card and the Plumbing summary (no decimals at most) and "300.4 bar" in the inspector (two at most), "300.00 bar" in debug mode; the limits had 0, 1 or 2 decimals by place (a tank's burst "5.1 bar" in the summary, "5.10 bar" in the inspector); a tank's liquid temperature was the reading to a tenth on the card and the truth to the kelvin in the inspector. *Fixed* (INTERFACE-7): `format::pressure` (bar, at most two decimals under 10 bar, one under 100, none above, to the reading's precision), `format::bar` (an exact limit by the same rule) and `format::kelvin` (whole kelvin at most, to the precision), read by the card's tanks and bottles (own and grouped), the inspector and the Plumbing summary.

50. **MCP could not show a component's inspector** (a battery, a bottle, a computer): `shoot {part}` took only a part's id, so findings 47 and 48's rows had no agent's way in. *Fixed* (INTERFACE-7): `systems_screen::inspect::named`, a part by its id else a component, selected as a click selects it.

51. **A full tank read more than full:** the booster's LOX "286/285.8 t" on the pad (the staging bar, the Plumbing summary): the held rounded to the level sensor's tonne, the capacity to its tenth. *Fixed* (INTERFACE-7): `format::amount` and `format::charge` give the capacity the held's digits ("286/286 t"); unmeasured, the capacity keeps its own.
52. **"no impact predicted" of a falling booster** just after switching to it (Landing menu): a switch or a load starts the prediction afresh, and until the first one is made the menu said there was none. *Fixed* (INTERFACE-7): `landing::LandingPrediction::of` is `None` only until the flight's first prediction (nothing to predict from is a prediction of no impact), the snapshot carries `predicting`, and the summary says "predicting the impact…".
53. **Landing ▸ PARACHUTE on the booster** read "q / DESIGN q 0.1 / 12.0 kPa, safe to deploy (P or Space)": the booster has no canopies (the section read the craft-level chute spec). *Fixed* (INTERFACE-7): a craft without canopies reads "no parachutes aboard".

54. **A vessel's name read two ways:** "Vessel #1 separated: a new vessel" beside its link "Vessel 11", the systems screen's bar "Vessel #1", a deleted piece "Debris #3", where the tracking list and the links say "Vessel 1": `VesselId`'s `Display` (the logs' "#1") was used for the player's text. *Fixed* (INTERFACE-7): one name, `tracking::vessel_name`, for every place.

55. **MCP: mid-insertion `get_vessel` read no apsides and an impact in 58 min** while the two-body orbit was 160 × 216 km (INTERFACE-7's insertion could not see its periapsis). The predicted path was from a moment before the stop: it is recomputed a few times a second of real time, and under the second stage's 40 m/s² near burnout a path 0.7 s old is ~30 m/s slow, its periapsis ~100 km lower, so it reentered. Its age hung on real-time scheduling, so a replay from a save read differently too. *Fixed* (INTERFACE-8): `step` and `run_until`, once they stop the clock, answer when the path is the active vessel's from the stopped clock (`state::Prediction::current`, `session::answer_now`, at most 5 real seconds); after a `step` under thrust `prediction_age_s` reads 0.02 s (one live tick), not up to 0.8 s.
56. **The part card's unmeasured store read a bare dash** (Dragon's NTO and MMH) where the systems screen and the RCS menu give `format::amount`'s "—/793 kg". *Fixed* (INTERFACE-8): the card's `Value::Unknown` carries its rule's text with the sensor on hover (`content::known`, `Live::held`) for tanks and batteries, own and grouped. With it, a group's sum takes `Reading`'s one rule for a sum (the precisions add, as the estimate's bounds: `sim::systems::data`), so the second stage's two batteries read "4000/4000 Wh (100 %)", not to half a per cent.
57. **A body's distance in its double-click menu was the truth** whatever the active vessel knew of its position (D106; a blind craft read a distance to the metre). *Fixed* (INTERFACE-9): `hud::body_distance` through `Known::distance`, the menu and `list_bodies`' `distance_m` by one rule.
58. **MCP: `step` and `run_until` answered with the vessel's `state` as raw truth** (on the pad: `altitude_m` 30.078471440821886, `apoapsis_m` −4891.77…), where every other number reads through the craft's knowledge. *Fixed* (INTERFACE-10): `session::Sample::read`, the truth read through `Known` by the menus' rules (`agent::known_altitude`, one rule with `get_vessel`'s altitude; `menus::landing::dynamic_pressure`, one rule with the Landing menu's DYN. PRESSURE; the feeding share as a sum of its tanks' readings, so `get_vessel`'s `stages.feeding_share` too). The stops compare the readings, so an answer agrees with its stop.
59. **MCP: a control's `by` for a ship command did not name its item** (the menus' and the inspector's said `command {item, do}`, the card's `command {item: s1.tank, do}`). *Fixed* (INTERFACE-10): `read::command_by`, one form for the card, the inspector and the menus: `command {item: "<id>", do: "<label>"}`, a menu's command by the item that offers it now (the `command` tool's own list).
60. **The Landing menu's parachute q read the truth** ("q / DESIGN q 34.8 / 12.0 kPa", and "safe to deploy" or "too fast" from it) where DYN. PRESSURE above it reads through the velocity (D106), at another precision. *Fixed* (INTERFACE-10): both through `menus::landing::dynamic_pressure`; without a velocity reading the q is a dash and the advice "q not measured (no velocity reading): no advice" (`Opening::Unmeasured`).
61. **The staging bar hid its top stage on the pad** (Dragon separation scrolled out of view, the column's top empty): it kept 240 px for an active stage that does not exist before ignition. *Fixed* (INTERFACE-10): `stage_bar::draw::scroll_room`, the active stage's room only while one runs (in flight unchanged).
62. **MCP: `step` and `run_until` measured the apsides from the equatorial radius** (`apoapsis_m` 10949 m where the navball's Ap read 15.6 km at T+62 s; 4.7–5.4 km low at Kennedy's latitude), the only place the player's Ap and Pe are not above the datum. *Fixed* (INTERFACE-10): the osculating apsides as points (`session::osculating_apsides`, through `sim::kepler`), their heights above the ellipsoid under them as the trajectory's apsides are; in orbit the run's periapsis now reads the Orbital menu's (the apoapsis still differs by J2, osculating against the predicted path).
63. **"Vessel 1 left with its engines lit…" had no link back** to the vessel left burning (on screen and in `notifications`, `vessel: null`). *Fixed* (INTERFACE-10): `tracking::left_lit_notice` gives the message and its link ("Vessel 1 ›").
65. **MCP: `get_vessel` (and `get_trajectory`, `set_plan`) required `id`** where `stage`, `command` and `set_controls` default to the active vessel. *Fixed* (INTERFACE-10): `agent::vessel_or_active`, one rule for every tool that takes a vessel.

Seen by INTERFACE-8 at HEAD (shots in `scratchpad/interface8/shots/`): "LOX 286/286 t" on the pad's bar (`a_pad.png`, finding 51), the batteries' sums on the cards (`a_pad_cards.png`), "Vessel 1 separated: a new vessel · Vessel 2 ›" (`g_sep.png`, finding 54), and the booster just after the switch: "predicting the impact…" and "PARACHUTE · no parachutes aboard" (`g_booster_switch.png`, findings 52–53).

Seen by INTERFACE-7 at HEAD (shots in `scratchpad/interface7/shots/`): the pad (`a_*.png`: every layer, the battery and bottle inspectors, the cards, the Power and RCS menus, help, pause, settings' four tabs, saves, tracking), T+60 s (`b_*.png`: the screen's mass "401 t" equal to the bar's, the Landing menu's G-LOAD equal to the navball's), after the stage separation (`g_sep_*.png`, the booster `g_booster*.png`), orbit after SECO (`c_*.png`: the Orbital menu's apsides equal to the navball's, Power, Crew, Comms, six layers, tracking) and Dragon alone (`h_dragon*.png`: 14 t, no abort offered, D120). Not for the interface: Dragon's RCS translates only BACK with all five blocks on (`h_dragon.png`: RIGHT/LEFT/UP/DOWN/FORWARD "none"), for the thruster model or the craft's data; the booster deleted as debris "dead, uncontrollable" while it has batteries (STATE-4's `powered()`).

Earlier walks found fine: the pad's cards placed by the bar, the help, the pause menu, the tracking station, the menus at the pad and in orbit, the systems screen's seven layers, Dragon alone after separation, the map.

## Parity between the player and the agent

D044 and D128's layer 3: the agent reaches the ship computer, calls its functions, takes shots and uses the controls itself; D106: it sees no more than the player. Each row is what the player has, its way in, and MCP's (INTERFACE-8, from this map and `game::agent`). **Yes**: the same, from the same rule. **Gap**: the player has it and MCP does not (a `shoot` of it is an image, not the data). **Over**: MCP shows more than the player sees.

| The player has | Player's way | MCP | Parity |
|---|---|---|---|
| The clock (TT, the site), warp, time stopped | time panel | `get_state` | yes |
| Frame rate | time panel | `profile` (developer flag) | the player's diagnostic |
| Speed in the navball's frame (ORBIT / SURFACE / TARGET) | navball | `read {navball}` (`speed`, `caption`) | yes (INTERFACE-9) |
| Altitude above the datum or the terrain | navball (ALT SEA / TER) | `read {navball}` (the switch's datum); `get_vessel` above the ellipsoid | yes |
| Ap and Pe with their times | navball, map | `get_vessel` `orbit.apsides`, `read {navball}` | yes |
| G-load and its limit | navball's G | `read {navball}` (its level against the limit) | yes |
| SAS, the hold, RCS, throttle, flight state (CLAMPED, POWERED…) | navball | `set_controls` sets them; `read {navball}` reads them back (`controls`, `state`) | yes |
| Stages: propellant, burnout, Δv, the active stage's figures | staging bar | `get_vessel` `stages.sequence` | yes |
| The functions offered | action bar | `command {}` | yes |
| A part's card | click a part or its icon | `read {card}` (`part_card::Gathered`, the cards' own inputs); `command {item, do}` | yes |
| The systems screen's layer summaries (mass, joints, temperatures, tanks, power, data, health) | Tab, a link | `read {layer}` (`summary::with_inputs`, `view::facts_of`, the screen's builders) | yes |
| An item's inspector (a part, a battery, a bottle) | the screen's click | `read {item}` (with its commands) | yes |
| Orbital: elements, target, closest approach, ship clock | 1 | `read {menu: orbital}`; `get_vessel`, `closest_approaches`, `get_state` | yes |
| Landing: impact, braking, reentry, the parts nearest their limits, parachute, attitude | 2 | `read {menu: landing}`; `get_landing_prediction` | yes |
| Maneuver: the planned burns | 3 | `read {menu: maneuver}`; `set_plan` | yes |
| Crew: crew, cabin air, life support, injuries | 4 | `read {menu: crew}` | yes |
| Power: buses, batteries, what has no power | 5 | `read {menu: power}` | yes |
| Comms: the link home, delay | 6 | `get_state`, `get_vessel` `signal` | yes |
| RCS: authority, blocks, feeds | R | `read {menu: rcs}` (while RCS is on); `command` | yes |
| Notifications | bottom left | `notifications` | yes |
| The vessels, their status | tracking station (M) | `get_state` | yes |
| A body's distance | double-click a body | `list_bodies` (`distance_m`, `hud::body_distance`, one rule) | yes |
| The saves | pause → Saves | `list_saves` (`saves::listing`), `save_game`, `load_game` | yes |
| The keys and every way in | help (`?`) | `tools/list` | the agent's own |
| Throttle, SAS, the hold, speed reference; rotation | keys, navball | `set_controls` | yes |
| Translation (I K, J L, H N) | keys | `set_controls {translate}` (`agent::HeldKeys`) | yes |
| RCS on, mode, fine; a block | R, the RCS menu | `set_controls {rcs, rcs_fine}`; `command` | yes |
| Next stage; the bar's edits | Space; drags, ×, AGAIN | `stage`; `edit_stages` | yes |
| Define stages and functions, rename, their commands, pin, save to the vessel | systems screen editors | `edit_program` (`systems_screen::edit`'s rules) | yes |
| Functions; a part's commands; parachutes | action bar, card, screen; P | `command` | yes |
| Warp, stop time | `.` `,` `/`, the time panel | `set_warp`, `pause`, `step`, `run_until` | yes |
| Switch vessel; mission control | `[` `]`, a link, tracking | `switch_vessel`, `go_to_mission_control` | yes |
| Target | Orbital → TARGET | `set_target` | yes |
| Camera | drag, scroll, F, a body's Focus | `set_camera` | yes |
| Save, load; new flight | Cmd+S / Cmd+L, Saves; pause | `save_game`, `load_game`; `new_flight` | yes |
| Revert to launch | pause → Revert | `revert_to_launch` (`saves::revert`, one rule) | yes |
| Debug mode; performance overlay | pause | `set_debug`; `profile` | yes |
| Settings (orbits, controls, interface, visuals) | pause → Settings | — | the player's preferences, not the flight (Open for the owner) |

**Over (D106), closed by INTERFACE-9:** `closest_approaches` read any two vessels' stored trajectories with no signal or sensor check; `get_landing_prediction` gave raw floats, the surface speeds ungated; `get_vessel`'s orbit numbers and `get_trajectory`'s points were not rounded to the position's precision. Each number now reads as its menu shows it (`agent::known_time`, `known_digits`, `known_ground` beside `known_value`): times and the impact's place with the orbit, distances through the position's reading, speeds through the velocity's, the braking burn with the propellant, the elements to the Orbital menu's decimals (`format::DEGREE_DECIMALS`, `ECCENTRICITY_DECIMALS`); `closest_approaches` needs a signal to both vessels and reads through `a`'s knowledge. Debug mode shows the truth.

A body's distance (its double-click menu) read the truth whatever the craft knew (finding 57): now through the position's reading, the menu and `list_bodies` by one rule (`hud::body_distance`). The settings stay the player's preferences (INTERFACE-8's Open for the owner).

## Not mapped

- The planner's own controls (PLANNER-3 is moving it onto nodes).
