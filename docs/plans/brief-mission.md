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

### Landed (2026-10-30)

- **b239026** MCP: each request on its own thread; a call waits up to 15 min (the long tools bound their own wait).
- **77c7941** The headless game and the session tools; the demo and the tour deleted:
  - `./run headless [port]` (default 7777, token `dev`): `SUNSCATTER_OFFSCREEN=1` (`game::offscreen`), `SUNSCATTER_AGENT=<port>:dev`, `SUNSCATTER_DEV=1`, a fresh home dir unless `SUNSCATTER_HOME` is given. Headless games neither load nor save settings; a shader error exits 3.
  - `game::agent::session`: `shoot`, `set_camera`, `pause`, `step`, `run_until`, `new_flight`, `save_game`, `load_game`, `set_debug`; `bench` and `profile` only with `SUNSCATTER_DEV=1` (D044). `state::advance` ends a frame at `StopAt`, so a step stops at its epoch exactly.
  - `set_controls` takes `speed_reference` and `rotate` (the pilot's keys, held aboard the active vessel: `agent::HeldRotation`, added in `keys::read_controls`).
  - Knowledge (item 3, done): `get_vessel` shows position, velocity, altitude (now above the ellipsoid), orbit and attitude (heading/pitch/roll and the body axes in east-north-up) only where a working sensor measures them, at its precision. It adds `stages` (the next stage, tank levels, engines, the feeding share). `get_landing_prediction` and `get_trajectory` need navigation.
  - Checked on screen: headless start, `get_state`, `pause`, `get_vessel` on the pad (Falcon 9: GNSS and IMU known, Dragon's tanks unmeasured), `set_camera`, `shoot` with Orbital open (settled, 1600×900; the pad shot looked right).

### Left, in order

1. **Fly the mission (item 4)** with `./run headless 7781` (take `target/demo.lock`) and a curl helper, e.g. `mcp.sh <tool> '<json>'` posting `tools/call` with `Authorization: Bearer dev`. The procedure to try:
   - `pause`; `set_controls {throttle: 1}`; `stage` (Ignition); `step {seconds: 0.5}`; `stage` (Liftoff); shots.
   - `step {seconds: 10}`, SAS stability.
   - The pitch kick: `get_vessel`'s `attitude.axes_enu` gives the body axes. Rotating about local north tips the nose east, so `rotate = [n·x, n·y, n·z] × 0.04` with `n = (0,1,0)` in ENU, with `sas: false`. Then `step {seconds: 2.5}`, then `rotate [0,0,0]`, `sas: true, hold: prograde, speed_reference: surface`.
   - `run_until {max_q: true, warp: 2}` and a shot; `run_until {feeding_below: 0.08, seconds: 146}` (MECO) and `stage`; `step 2`; `stage` (separation); `run_until {separation: true}`; `step 3`; `stage` (second-stage ignition); shots of the upper stage, then `switch_vessel` to the booster and a shot.
   - Check each `reached`. If the run never reaches orbit, say how far it got.

   Untested yet: `step`/`run_until` (the clamp at `StopAt`), `bench`, `profile`, `new_flight`, saves, the named screens. Then write the procedure as a short guide (`docs/process.md`'s testing section, or where DOCS puts it).
2. **Perf with 11 vessels:** no tool spawns vessels. `GameCommand::SpawnTestShips` spawns test craft, which ONEKIND is deleting. Add a debug `spawn_ships` tool once ONEKIND settles what it spawns, then measure with `profile` at warps 0, 6 and 9.
3. **What the demo had that no tool does yet:**
   - the body tour's limb and ground views: the camera looks at the focus, so `set_camera` cannot look at the horizon from low down. Moving `tour::pose` into a camera override would do it;
   - the eclipse and transit searches;
   - the plunge view (a vessel placed 185 km up, falling);
   - the zoom sweep;
   - the playtest save from the orbit step (ONEKIND regenerates or drops it);
   - the map-view (D054) and impact-marker (D080) checks, which ran only in the demo: check that their rules' table tests cover them.
4. Doc changes for DOCS (not mine to edit):
   - `docs/process.md` and HANDOFF's Gotchas still describe the demo (`SUNSCATTER_DEMO_*`, `demo.lock` wording, the plume crop);
   - `agent-rules.md` line 16 names `SUNSCATTER_DEMO_OFFSCREEN`, `_STOP_AFTER`, `_ONLY`, `_NO_BENCH`: now `./run headless` and the tools;
   - `docs/technical/mcp.md` (if that is where `mcp.md` went) needs the session tools, the developer flag and Knowledge;
   - the owners table needs rows for `game::offscreen` and `game::agent::session`, and the demo's and tour's rows removed;
   - `docs/technical/windows.md` ran the demo on backhouse.

### Open for the owner

- **The developer flag** is a new variable, `SUNSCATTER_DEV=1`. The profiler's tables still need `SUNSCATTER_PROFILE=1` as well, because the profiler is set up at startup.
- **The pitch kick is stick input:** the agent holds the pilot's rotation keys between a pause and a step. D044 says agents fly by decisions. The decision-level way would be a commanded-attitude hold (a sim `HoldMode`, the pointing laws), which is not built. Should agents lose `rotate` once that hold exists?
- **`step` and `run_until` stop time at the end** (unless `keep_running`); time otherwise runs while the agent thinks (D044).
