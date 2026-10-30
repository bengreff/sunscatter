# The game: rendering, interface, camera, MCP

The Bevy app (`crates/game`, D045): it reads the simulation's state each frame, draws it camera-relative in f32, takes the player's input as commands, and answers the agent's tool calls. It never integrates (rule 4): everything drawn is sampled from stored segments at the current clock ([frame-order.md](../technical/frame-order.md)). Game rules are pure functions with table tests, and Bevy systems only wire them up (rule 8).

**Status:** the flight view, the staging bar, part cards, the task menus, the action bar and the systems screen are *playable*; the map view, the tracking station, saves and the burn planner are *playable*. The one look is *playable*, and visual work is frozen until the first two flights fly end to end (D123). The MCP server and the player's agent tools are *playable*; D122's testing tools are being built (MISSION), and the scripted demo and the body tour they replace are being deleted.

## Laws

- **One look** (D073, D058): simple, crisp and functional, no graphics settings. Visuals come from physics or the simulation, never for their own sake (D109); a craft's light is the craft's, never the camera's (D110). Light is physical, per body, at a fixed exposure (D052, D055, D087). The 3D view renders at full resolution whenever the frame fits (D072).
- **The interface never shows something wrong** (D106): readouts go through what the craft knows (`game::format`), a dash where nothing measures it.
- **The flight view** (D080, D111): the navball cluster, the staging bar, part cards and flat task menus in fixed docks; the systems screen is a separate screen (D107).
- **Map view per object** by pixel size (D054); orbit lines end after one revolution about the dominant body (D056); apsis markers are extrema of distance on the predicted trajectory (D076); the tracking station is its own screen (D050).
- **One revert**, from a launch save (D114). Saves are not compatible across versions (D063).
- **MCP is the only way an agent reaches the running game** (D044, D069, D122), for players and for development alike. Every agent gets what the player can see and do; the look's benchmark and the profiler need a developer flag. Game time runs while the agent thinks; it has pause, step and run-until. The agent sits at a control location (D067) and flies through decisions, plans and ship-OS programs, with light delay, not stick and throttle.

## What exists

- **Rendering** (`game::look`, `game::scene`, `game::terrain`, `game::atmosphere`, `game::sky`, `game::lighting`): bodies with terrain LOD and zones, Bevy's atmosphere, stars, rings, cloud decks, craft drawn part by part, the engine plume from each engine's exit state (*playable*, off by default behind D109's toggle), the render scale. [crisp-surfaces.md](../technical/crisp-surfaces.md), [solar-visuals.md](../technical/solar-visuals.md), [exhaust.md](../technical/exhaust.md)
- **The flight interface** (`game::navball`, `game::interface`): the navball's ORBIT/SURFACE/TARGET switch, altitude reference and G row; the staging bar and part cards; the task menus; notifications; the action bar. *Playable.* [flight-menus.md](../technical/flight-menus.md), [staging-bar.md](../technical/staging-bar.md)
- **The systems screen** (`game::systems_screen`): the craft by layer, each layer as a whole, the inspector and edits. *Playable.* [systems-screen.md](../technical/systems-screen.md)
- **Camera, map and trajectories** (`game::camera`, `game::map_view`, `game::trajectory`, `game::planner`, `game::tracking`): camera limits and collision, the per-object map view, orbit lines, apsides, the burn planner, switching vessels. *Playable.* The map view's spec is in the archived [map-view plan](../archive/plans/map-view-lighting-controls.md).
- **State and commands** (`game::state`, `game::commands`, `game::fleet`, `game::saves`): every discrete change is a `GameCommand`, from the keyboard, the interface or the agent; the fleet advanced on the compute pool within the frame's deadline; saves, the launch revert, the loading screen.
- **The agent's tools** (`crates/mcp`, `game::agent`): state, bodies, vessels, trajectories, warp, switching, mission control, controls, staging, plans, landing prediction, closest approaches. Off by default for the player. *Playable.* [mcp.md](../technical/mcp.md)
- **The profiler** (`game::profile`, `SUNSCATTER_PROFILE=1`): CPU per system and thread, GPU per pass group.

## Not built

- D122's testing tools (screenshots, camera placement, the benchmark, the profiler, time controls, a new flight, load and save) and the first mission test flown through them (MISSION, [brief-mission.md](../plans/brief-mission.md)).
- The visual editor for craft (D102); the career's screens (D027); colonies' (D019).
- Frozen (D123): the plume's rest, the reentry glow, clouds.

## Owners and decisions

- Owners: [owners.md, The game](../owners.md#the-game-state-commands-the-frame) (state, commands and the frame; camera, map and trajectories; rendering; interface).
- Decisions: D028, D044, D045, D050, D052, D054–D056, D058, D063, D067, D069, D072, D073, D076, D080, D087, D089, D106, D107, D109–D111, D114, D122, D123.
- Technical: [frame-order.md](../technical/frame-order.md), [testing.md](../technical/testing.md), [mcp.md](../technical/mcp.md), [flight-menus.md](../technical/flight-menus.md), [staging-bar.md](../technical/staging-bar.md), [systems-screen.md](../technical/systems-screen.md), [crisp-surfaces.md](../technical/crisp-surfaces.md), [exhaust.md](../technical/exhaust.md), [windows.md](../technical/windows.md).
