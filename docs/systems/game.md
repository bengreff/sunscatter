# The game: rendering, interface, camera, MCP

The Bevy app (`crates/game`, D045): it reads the simulation's state each frame, draws it camera-relative in f32, takes the player's input as commands, and answers the agent's tool calls. It never integrates (rule 4): everything drawn is sampled from stored segments at the current clock ([frame-order.md](../technical/frame-order.md)). Game rules are pure functions with table tests, and Bevy systems only wire them up (rule 8).

**Status:** the flight view, the staging bar, part cards, the task menus, the action bar and the systems screen are *playable*; the map view, the tracking station and saves are *playable*. The burn planner is *playable* only for the test craft, which is being deleted; no craft of parts can plan a burn until PLANNER (D126). The one look is *playable*, and visual work is frozen until the first two flights fly end to end (D123). The MCP server and the agent's tools are *playable*. D122's testing tools are built (77c7941): the headless game (`./run headless`), shots, the camera, pause, step and run-until, a new flight, load and save, debug mode, and behind the developer flag the look's benchmark and the profiler. The scripted demo and the body tour are deleted. The first mission test (the Falcon 9 from the pad to orbit and the stage separation) is being flown (MISSION-2).

## Laws

- **One look** (D073, D058): simple, crisp and functional, no graphics settings. Visuals come from physics or the simulation, never for their own sake (D109); a craft's light is the craft's, never the camera's (D110). Light is physical, per body, at a fixed exposure (D052, D055, D087). The 3D view renders at full resolution whenever the frame fits (D072).
- **The interface never shows something wrong** (D106): readouts go through what the craft knows (`game::format`), a dash where nothing measures it.
- **The flight view** (D080, D111): the navball cluster, the staging bar, part cards and flat task menus in fixed docks; the systems screen is a separate screen (D107).
- **Map view per object** by pixel size (D054); orbit lines end after one revolution about the dominant body (D056); apsis markers are extrema of distance on the predicted trajectory (D076); the tracking station is its own screen (D050).
- **One revert**, from a launch save (D114). Saves are not compatible across versions (D063).
- **MCP is the only way an agent reaches the running game** (D044, D069, D122), for players and for development alike. Every agent gets what the player can see and do, including screenshots, camera placement, run-until, a new flight, and load and save; the look's benchmark and the profiler need a developer flag. Game time runs while the agent thinks; it has pause, step and run-until, so tests stay deterministic. The agent sits at a control location (D067) and sees what the player sees (D106). It flies mostly through decisions, plans and ship-OS programs, with light delay; the player's manual controls are its too (D126).
- **One testing framework** (D122): unit tests, with specific deterministic sim cases in CI, and mission tests flown headless through MCP by a development agent on the real flight software, with screenshots ([testing.md](../technical/testing.md)).

## What exists

- **Rendering** (`game::look`, `game::scene`, `game::terrain`, `game::atmosphere`, `game::sky`, `game::lighting`): bodies with terrain LOD and zones, Bevy's atmosphere, stars, rings, cloud decks, craft drawn part by part, the engine plume from each engine's exit state (*playable*, off by default behind D109's toggle), the render scale. [crisp-surfaces.md](../technical/crisp-surfaces.md), [solar-visuals.md](../technical/solar-visuals.md), [exhaust.md](../technical/exhaust.md)
- **The flight interface** (`game::navball`, `game::interface`): the navball's ORBIT/SURFACE/TARGET switch, altitude reference and G row; the staging bar and part cards; the task menus; notifications; the action bar. *Playable.* [flight-menus.md](../technical/flight-menus.md), [staging-bar.md](../technical/staging-bar.md)
- **The systems screen** (`game::systems_screen`): the craft by layer, each layer as a whole, the inspector and edits; the thermal layer drawn as the thermal network itself (each skin cell, the interior nodes behind a cut-away) and a clicked part isolated and framed, Esc back to the layer view (2026-10-31). *Playable.* [systems-screen.md](../technical/systems-screen.md)
- **Ways in** (`game::interface::access`, `help`, `pause::on_escape`): every screen's key, click path and MCP tool in one table; the help generated from it and the key bindings; Esc backs out one level everywhere; the MCP `command` tool reaches a craft's functions and its parts' and components' commands, from the part cards' own list. *Playable.* [interface-map.md](../technical/interface-map.md)
- **Camera, map and trajectories** (`game::camera`, `game::map_view`, `game::trajectory`, `game::planner`, `game::tracking`): camera limits and collision, the per-object map view, orbit lines, apsides, the burn planner, switching vessels. *Playable.* The map view's spec is in the archived [map-view plan](../archive/plans/map-view-lighting-controls.md).
- **State and commands** (`game::state`, `game::commands`, `game::fleet`, `game::saves`): every discrete change is a `GameCommand`, from the keyboard, the interface or the agent; the fleet advanced on the compute pool within the frame's deadline; saves, the launch revert, the loading screen.
- **The agent's tools** (`crates/mcp`, `game::agent`): state, bodies, vessels, trajectories, warp, switching, mission control, controls (with the pilot's rotation and the speed reference), staging, plans, landing prediction, closest approaches. A vessel's readings come through what it knows (D106; debug mode shows the truth). Off by default for the player. *Playable.* [mcp.md](../technical/mcp.md)
- **The session tools and the headless game** (D122, `game::agent::session`, `game::offscreen`): `shoot` (the view or a named screen, at a size and an interface scale), `set_camera`, `pause`, `step`, `run_until` (an epoch or an event: altitude, apsides, the feeding tanks' share, max-Q, a separation, a status), `new_flight`, `save_game`, `load_game`, `set_debug`; `bench`, `profile` and `spawn_vessels` (a fleet of any craft in low orbit, for the performance check) with `SUNSCATTER_DEV=1`. `./run headless [port]` renders offscreen, with the screen locked. *Playable*; the first mission test is being flown.
- **The profiler** (`game::profile`, `SUNSCATTER_PROFILE=1`): CPU per system and thread, GPU per pass group.

## Not built

- The first mission test and its guide for development agents (MISSION-2, [brief-mission.md](../plans/brief-mission.md)); the 11-vessel performance numbers on craft of parts (`spawn_vessels` then `profile`); the old tour's low views (a camera looking at the horizon), the plunge view and the zoom sweep.
- A warp to a point for the player (D126; agents have `run_until`).
- An automation switch with its config menu (D126; [avionics](avionics.md)).
- The visual editor for craft (D102); the career's screens (D027); colonies' (D019).
- Frozen (D123): the plume's rest, the reentry glow, clouds.

## Owners and decisions

- Owners: [owners.md, The game](../owners.md#the-game-state-commands-the-frame) (state, commands and the frame; camera, map and trajectories; rendering; interface).
- Decisions: D028, D044, D045, D050, D052, D054–D056, D058, D063, D067, D069, D072, D073, D076, D080, D087, D089, D106, D107, D109–D111, D114, D122, D123, D126.
- Technical: [frame-order.md](../technical/frame-order.md), [testing.md](../technical/testing.md), [mcp.md](../technical/mcp.md), [flight-menus.md](../technical/flight-menus.md), [staging-bar.md](../technical/staging-bar.md), [systems-screen.md](../technical/systems-screen.md), [crisp-surfaces.md](../technical/crisp-surfaces.md), [exhaust.md](../technical/exhaust.md), [windows.md](../technical/windows.md).
