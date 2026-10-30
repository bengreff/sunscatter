# Brief: MISSION, one testing framework (2026-10-30)

D122: unit tests, plus mission tests flown headless through MCP by a development agent on the real flight software, with screenshots. The scripted demo and the body tour are deleted. Follow [agent-rules.md](agent-rules.md). Your name is MISSION and your target dir is `target/agents/mission`: start warm with `mv target/agents/fixes2 target/agents/mission` (a release build of the game is in it). Two other agents run at the same time:
- **ONEKIND** ([brief-onekind.md](brief-onekind.md)) is deleting the test craft across `sim` and `game`, except the demo and the tour, which are yours.
- **DOCS** ([brief-docs.md](brief-docs.md)) is restructuring `docs/`. Edit docs only for the commands you change (`CLAUDE.md`'s Commands section, the MCP's doc). List the rest in your State.

## The work, in order

1. **Inventory** what the demo, the tour and the `SUNSCATTER_*` variables do today (screenshots of the look, the look's benchmark `bench_*.md`, performance with 11 vessels, the pad-abort and stage checks, the plunge view, the body tour), and what the MCP server (`crates/mcp`, `game::agent`) offers. Record it in this brief's State.
2. **MCP tools for testing**, each a thin wire to an existing owner (rule 8):
   - `shoot`: a screenshot of the view, or of a named screen, at a size, with the interface scale;
   - camera placement for a shot;
   - `bench`: the look's benchmark;
   - `profile`: the profiler's numbers;
   - time controls: warp, run until an epoch or an event;
   - `new_flight` with a craft;
   - `load` and `save`.

   The game runs headless (offscreen) with the MCP server and needs no window. One way to start it, documented in `CLAUDE.md`.
3. **The agent's tools read `Knowledge`** (FLIGHTSW's Left 3, [brief-flightsw.md](../archive/briefs/brief-flightsw.md)): the player's AI agent sees what the player sees. Debug mode shows the truth.
4. **Fly a mission yourself through MCP** as the first mission test: the Falcon 9 from the pad to orbit, then the stage separation, with screenshots at the key moments. Write the procedure down as a short guide for development agents (`docs/process.md`'s testing section, or a file DOCS places), so the next agent can run a mission test from it.
5. **Delete** the scripted demo (`game::demo*`), the tour (`game::tour`), and the `SUNSCATTER_*` variables they used. A few stay (the home dir, offscreen, the profiler). Do it once your tools cover what they did. Update `CLAUDE.md`'s Commands and the hooks or scripts that call them.
6. Unit tests stay as they are. Keep the CI jobs green.

## State

(Update as you land: commits with hashes, the inventory, what is left in order, doc changes needed.)

### 1. Inventory (2026-10-30)

**The scripted demo** (`SUNSCATTER_DEMO=<dir>`; `game::demo`, `demo_views`, `demo_parts`, `demo_checks`):
- *The test craft's script* (`demo.rs`): pad and pad-top shots, the look's benchmark at the pad (`bench_pad.md`); debug-mode ascent on a magic attitude hold (shots at 10 km and 20 km), coast, circularise, orbit shot with Orbital open and `orbit_save.ron` (the playtest save); bench in orbit; the plunge; Earth from low orbit; map view; a planned TLI line; Earth's night side; Moon disc, Moon close (low Sun), Moon's night side; **performance with 11 vessels** (10 test ships spawned) at 1×, 1000×, 1e6×: fps, worst frame, effective warp, compute-limited frames; deorbit with Landing open (impact marker), fall at rails warp, parachute, landed, sunset, tracking station with 3 ships, pause menu, settings ▸ Interface.
- *A craft of parts' script* (`demo_parts.rs`, `SUNSCATTER_CRAFT=falcon9-crew`): 4 pad views, the systems screen by layer (physical; plumbing with `s1.tank`; structural with `s1.interstage`; electrical with `dragon.capsule`), ignition and liftoff (Space twice), SAS stability to T+10 s, pitch kick 2.5 s at 4 % of authority east, SAS surface prograde; max-Q shot and the bench there (`bench_max_q.md`); MECO at 8 % of the feeding tanks or T+156 s; separation (fleet grows), the upper stage's ignition 3 s later, its shot; the switch to the booster and its shot; interface shots (`parts_ui_*`: menus open, RCS on, a part card). **Pad abort** (`STOP_AFTER=PadAbort`, D114): ignition at 90 %, liftoff armed, the stage editor locked, throttle closed, the fresh vessel and its toast.
- *The plunge* (`demo_views.rs`, `SUNSCATTER_DEMO_ONLY=Plunge`): the pad looking up (the owner's 120 fps view), then a test-craft vessel 185 km up falling at 2.3 km/s (`SUNSCATTER_PLUNGE_OVER=<lat>,<lon>[,<km>]`): fps, a shot, a profile window and `bench_plunge.md`.
- *Checks* (`demo_checks.rs`, demo mode only): exit 3 on a shader that fails to compile; the interface drawn over the main camera; D054's map view at the pad, map and tracking steps; D080's impact marker only with Landing open; fps/altitude/chunks logged every 2 s.
- Capture: freezes the clock, waits ≥ 1.2 s and until `Terrain::busy()` clears (≤ 12 s), then a `Screenshot` of the window or the offscreen image (egui included).
- Settings: neither loaded nor saved in the demo; exhaust off unless `SUNSCATTER_EXHAUST=1`.

**The body tour** (`tour.rs`, `SUNSCATTER_TOUR=<bodies>` with `SUNSCATTER_DEMO`): per body, disc from 3 radii at 30° phase, limb from 1.5 % of the radius (or above the air), ground from 2 km (less in thick air) at a 10° Sun, without the interface, clock frozen; `SUNSCATTER_TOUR_VIEWS` adds eclipse ingress and shadow transit (searched up to 120 days ahead); `SUNSCATTER_TOUR_DATE`. The camera pose is a pure function (`tour::pose`) applied after the rig (`tour::aim`).

**The variables:** demo only: `SUNSCATTER_DEMO`, `_DEMO_OFFSCREEN`, `_DEMO_STOP_AFTER`, `_DEMO_ONLY`, `_DEMO_NO_BENCH`, `_DEMO_NO_CAPTURE`, `_DEMO_ZOOM` (zoom sweep 50 m → 20,000 km), `_DEMO_SIZE`, `_DEMO_UI_SCALE`, `_DEMO_FAR`, `_PLUNGE_OVER`, `_EXHAUST`, `_BENCH_ONLY`, `_TOUR`, `_TOUR_VIEWS`, `_TOUR_DATE`. Kept: `SUNSCATTER_HOME`, `SUNSCATTER_PROFILE`, `SUNSCATTER_AGENT=<port>:<token>` (turns the MCP server on), `SUNSCATTER_RENDER_SCALE`, `SUNSCATTER_DUMP_MSL` (a shader test's), `SUNSCATTER_CRAFT` (ONEKIND's: the test-craft value goes), `SUNSCATTER_TIER` (only in `run`'s comment: stale). `tools/gpu_trace.py` launches the plunge demo.

**The MCP server** (`crates/mcp`, `game::agent`, D044, D069): Streamable HTTP on 127.0.0.1 with a bearer token, JSON replies, a call waits ≤ 10 s for the game's frame; the HTTP thread serves one request at a time. Tools: `get_state`, `list_bodies`, `get_vessel`, `get_trajectory`, `set_warp`, `switch_vessel`, `go_to_mission_control`, `set_plan`, `get_landing_prediction`, `closest_approaches`, `set_controls` (throttle, SAS, hold), `stage`. They read the true state (FLIGHTSW's inventory), light-delayed by `comms`. Missing for a mission test: shots, camera, bench, profile, run-until, new flight, load/save, the pilot's rotation input and speed reference, tank and engine readings.
