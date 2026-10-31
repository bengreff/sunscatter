# The map of the interface

Every piece of information and every control the player has, with every way to reach it: a key, a click path, an MCP tool. Made by INTERFACE (2026-10-31, [brief-interface.md](../plans/brief-interface.md)) from the code and from shots of the Falcon 9 flown headless through MCP (the pad set; the ascent and orbit set in the brief's State). Decisions D080, D106, D107, D111, D112, D120, D123 (the interface exception), D126. Kept current with the interface: a change to a way in changes its row here. The help overlay (`?`) is generated from the same tables as the input (`interface::access`, `commands::keys`), so the keys in it cannot drift; this page adds the findings.

**Status:** the map is complete for the flight view, its menus and cards, the systems screen, the tracking station, pause, settings, saves and help. The findings marked *fixed* are done; the rest are listed under "Open" below.

## The screens and windows

| What | Key | Click path | MCP | Closes by |
|---|---|---|---|---|
| Flight view | — (the base) | — | `shoot` (default), `set_camera` | — |
| Task menus: Orbital, Landing/Reentry, Maneuver, Crew/ECLSS, Power, Comms | 1–6 | menu bar (top centre) | `shoot {menus}` | its key, the bar, its × |
| RCS menu | R (turns RCS on) | navball's RCS button | — (`set_controls` does not turn RCS on) | R, the button, its × (turns RCS off) |
| Part card | — | a part on the ship; a part icon on the staging bar | `shoot {cards}` | its ×, a second click on its part |
| Systems screen | Tab | action bar SYSTEMS; a menu's "The systems screen: LAYER" link; a part card's "more ›"; a notification's link | `shoot {screen: systems, layer, part}` | Tab, Esc, its close button |
| The systems screen's isolated part | — | a part in the view, in PARTS, or a row about one | `shoot {screen: systems, part}` | Esc or its panel's × (back to the layer view as it was) |
| Tracking station | M | pause → Tracking station | `go_to_mission_control`, `shoot {screen: tracking}` | M, Esc, Back to flight |
| Pause menu | Esc | — | `shoot {screen: pause}` | Esc, Resume |
| Settings (Orbits, Controls, Interface, Visuals) | — | pause → Settings | `shoot {screen: settings, settings_tab}` | Esc, its × |
| Saves | Cmd+S / Cmd+L quick (Ctrl+Alt elsewhere) | pause → Saves | `save_game`, `load_game` | Esc, its × |
| New flight, revert to launch, quit | — | pause → New flight… / Revert to launch… / Quit | `new_flight` | Esc (back to the menu), Cancel |
| Help: the keys and every way in | `?` | menu bar `?`; pause → Help | `shoot {screen: help}` | `?`, Esc, its × |
| Burn planner | — | Maneuver (3) → Open the burn planner; Orbital → the planner button | `set_plan` | Esc, its × |
| Body menu (distance, Focus) | — | double-click a body | — | Esc, its × |
| Notifications | — (shown when a message comes) | ⏷ / ⏶ fold it | — (its messages are not read by MCP) | ⏷ folds it |
| Performance overlay | — | pause → Performance overlay | `profile` (developer flag) | the same button |
| Debug mode | — | pause → Debug mode | `set_debug` | the same button |

## The flight view's information

| Information | Where | Reached |
|---|---|---|
| The origin's clock (TT, the site), warp, time stopped, frame rate | time panel, top left | always shown |
| Speed and its frame (ORBIT / SURFACE / TARGET, AUTO / LOCK) | navball cluster | always; the switch by click, a click on the ball cycles it |
| Altitude (and its datum), Ap and Pe with their times, the g-load and its limit | navball cluster | always; the datum by click on ALT; the limit on hover |
| SAS, hold modes, RCS, throttle, flight state (CLAMPED, FLIGHT, POWERED…) | navball cluster | always; T, R, clicks, Z / X / Shift / Ctrl |
| Stages: propellant per stage, burnout, Δv; the active stage's thrust, TWR, mass, Isp | staging bar, right edge | always; CURRENT AIR / VACUUM switch under it |
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
| RCS on / off, mode, fine, blocks | R | navball RCS; the RCS menu | — |
| Next stage | Space | — | `stage` |
| A stage's edits (move, delete, a command's move) | — | staging bar drags, × | — |
| Functions (abort, grid fins, legs…) | — (no craft-specific keys, D111) | action bar; the systems screen's run buttons | — |
| A part's commands (ignite, valve, deploy, separate…) | — | part card; systems screen right column | — |
| Parachutes | P | the card's ARM PARACHUTES | — |
| Warp | `.` `,` `/` | the time panel's arrows | `set_warp`, `step`, `run_until`, `pause` |
| Vessels | `[` `]` | a notification's link; tracking station Switch to | `switch_vessel` |
| Target | — | Orbital → TARGET pick list | — |
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
10. **MCP cannot reach** a craft's functions (the abort, grid fins, legs), a part's commands, RCS's mode and blocks, the target, the staging bar's edits or the notifications. An agent flies the Falcon 9 through `stage` and `set_controls` only. *Open:* a `command` tool (a function by name, a part's command) in `game::agent`, after PLANNER-3's `set_plan` lands there.
11. **A joint's allowable line load and axial capacity read 0** for a bolted joint with no figures (the MVac's): a zero where the value is unknown (D106 wants a dash). *Open* (craft data or `inspect`).
12. **The Settings window** opens over the action bar and the staging bar and is cut at the bottom on the Orbits tab (it scrolls).

## Not mapped

- The planner's own controls (PLANNER-3 is moving it onto nodes).
- The test craft's screens (being deleted, D121).
