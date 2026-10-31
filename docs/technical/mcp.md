# Connecting your AI agent (MCP)

Sunscatter runs an MCP server for the player's own AI agent (D044, D069). It is the only way an agent reaches the running game, for players and for development alike: D122's mission tests use the same tools. It is **off by default** for the player.

1. In the game: Esc → Settings → Interface → tick **AI agent server**. The port defaults to 7878; a token is generated the first time.
2. The tab shows the command to connect Claude Code; run it in a terminal:

   ```sh
   claude mcp add --transport http sunscatter http://127.0.0.1:7878/mcp --header "Authorization: Bearer <token>"
   ```

3. Start Claude Code and ask it about the game ("what is my apoapsis?", "warp to 1000x").

The server listens on 127.0.0.1 only, refuses requests from non-local web origins, and requires the token. "New token" in the settings invalidates the old one.

## What the agent can do

The agent is at a **control location** like you (D067): mission control when the tracking station is open or the active vessel is uncrewed, the crew of the active vessel otherwise. It sees other vessels as their light arrives there, and cannot read a vessel with no signal. It sees what the player sees (D106): a vessel's readings come only from its working sensors, at their precision, and are `null` where nothing measures them. Debug mode (`set_debug`) shows the truth. Game time runs while the agent thinks, as it does for the player.

### Flying

| Tool | Does |
|---|---|
| `get_state` | Clock (TDB), the origin's clock (TT, D088) and the active ship clock's offset from it, warp, location, active vessel, every vessel's status and signal, the navigation target |
| `list_bodies` | Bodies: name, GM, radius, what they orbit |
| `get_vessel` | A vessel as seen from the location, as far as its sensors measure: position, velocity, altitude above the ellipsoid, orbit (the next apsides along the predicted path; while the active vessel flies live, `prediction_age_s`, how old that path is: it is recomputed a few times a second of real time, so while time runs it lags, and after a `step` or `run_until` it is from their stop; and `now_two_body`, the apsides from the centre if only the body's gravity acted from now, the Orbital menu's ORBIT NOW), attitude (heading, pitch, roll and the body axes in east-north-up), signal; for a craft of parts, `stages` (the next stage, tank levels, engines, the feeding tanks' share, and `sequence`: the staging bar's rows top to bottom, each stage's `stage` number, KSP number, name, place, commands and figures, Δv, burnout, propellant and the active stage's thrust, TWR, mass and Isp, in vacuum or with `air: "current"`, D120) |
| `get_trajectory` | A vessel's predicted path from now, sampled (needs a navigation solution) |
| `set_warp` | Time warp level; resumes stopped time |
| `switch_vessel` | Fly a vessel: it becomes active, and the control location moves aboard it if it is crewed (mission control if not) |
| `go_to_mission_control` | Open the tracking station (mission control) |
| `set_controls` | Throttle, SAS, the hold and the speed reference of any vessel (`id`, default the active one; D112), after the light delay from anywhere but aboard; `rotate`, the pilot's rotation keys held, and `rcs` (off, on, translate, rotate) with `rcs_fine`, the pilot's RCS mode (R and the RCS menu), aboard the active vessel |
| `stage` | A craft of parts' next stage (`id`, default the active one), sent like any command |
| `command` | A craft of parts' commands as the player gives them, sent like any command: `function` runs one of its functions by name (the action bar's: an abort, the legs, the grid fins); `item` (a part's or component's id) with `do` runs one of its commands by its label, as its part card and the systems screen show them (IGNITE, SHUT DOWN, DEPLOY, OPEN, SEPARATE, RCS … on, POWER … off, ARM PARACHUTES). `item` alone lists its commands now; neither lists the functions and every item with commands. The functions are those the action bar and the systems screen offer: the ones the piece's parts still take, an abort only while it would save the crew (D120); the rest are refused with why |
| `edit_stages` | The staging bar's edits, one a call, by the stages' `stage` numbers from `get_vessel`: `move` a stage still to run, `move_command` one of its commands to another stage, `delete` one, or run one already run `again` (D114); the bar's own rules (`stage_bar::rows::check`), sent like any command |
| `set_target` | The navigation target from the Orbital menu's choices (`navball::target_choices`): a `vessel` (not the active one), a `body` with a surface, or `none` |
| `notifications` | The notifications' messages as the player reads them, numbered: those `after` the last one read, with whether each is an error and the vessel it is about |
| `set_plan` | Replace a vessel's planned burns (prograde/normal/radial Δv, time); sent with light delay. A craft of parts gets nodes (D126: `in_s` is the burn's middle, flown closed-loop by the maneuver program); the result gives each node's predicted ignition, duration and warnings, and the Δv available |
| `get_landing_prediction` | Where, when and how fast the active vessel meets the ground (needs a navigation solution) |
| `closest_approaches` | Closest approaches between two vessels (nearest first) |

### The session (`game::agent::session`)

| Tool | Does |
|---|---|
| `shoot` | A screenshot (PNG) once the view has settled, the clock held meanwhile: the flight view or a named screen (`systems` on a `layer` with a `part` selected: a part isolated or a component, a battery or a bottle, selected, `tracking`, `pause`, `settings` on a tab, `saves`, `help`), with `menus` open (the RCS menu while RCS is on), at a `width`/`height` (headless), at an interface scale (`ui_scale`: 0.2 shrinks it to specks). Returns the file's path (default `shots/` under the game's home) |
| `set_camera` | The camera's focus (the active vessel, another vessel or a body), distance, pitch and yaw about the focus's up, or the angle from a body's subsolar point |
| `pause` | Stop time (the player's `,`) |
| `step` | Run `seconds` of game time and stop exactly there, at a warp level; answers when the clock has stopped |
| `run_until` | Run until the first of: an epoch, `seconds`, the vessel's altitude above or below a value, its apoapsis or periapsis above one, its feeding tanks' share below one, max-Q, a separation, a status. Answers with what stopped it (`reached`) and the vessel's state. A condition on something no working sensor measures is refused |
| `new_flight` | A new craft on the pad (`craft`: a directory under `data/craft`) |
| `save_game`, `load_game` | Save or load by name (loading resets warp to 1×; stopped time stays stopped, so a test replays from a save exactly) |
| `set_debug` | Debug mode (D064) for every vessel; a dead computer is still dead (D123) |
| `bench` | Developer flag only: the look's benchmark from the current view (each part's cost); `only` measures the whole look and the parts it names |
| `profile` | Developer flag only: frame statistics over a real-time window (fps, worst and 95th-percentile frame, the warp achieved), with the profiler's report when started with `SUNSCATTER_PROFILE=1` |
| `spawn_vessels` | Developer flag only: `count` vessels (1–50, default 10) of `craft` (any craft id, default `small-rocket`) coasting in low Earth orbit, each on its own orbit, every other one uncrewed; answers with their ids. The fleet of the performance check (with `profile`). It replaced the pause menu's "Spawn 10 test ships" |

`step` and `run_until` stop time at the end (unless `keep_running`), and every command sent while time is stopped applies at that clock, so a mission test is deterministic. Once stopped they answer when the active vessel's predicted path is from the stopped clock (at most 5 real seconds), so `get_vessel`'s apsides after a stop are the stopped state's, the same on every replay (`prediction_age_s` reads one live tick under thrust). A shot or the benchmark holds the clock while it is in flight (overlapping ones until the last ends).

A call naming an argument its tool does not list is refused with the tool's own argument names (`set_camera {distance: 40}` answers that it takes `distance_m`), rather than ignored.

## For developers

- `crates/mcp`: the transport (JSON-RPC 2.0 over HTTP POST, plain JSON responses; `initialize`, `ping`, `tools/list`, `tools/call`). It knows nothing about the game.
- `crates/mcp` serves each request on its own thread; a call waits up to 15 minutes for the game (the long tools bound their own wait).
- `game::agent`: the flying tools, answered once per frame in the input stage; actions become `GameCommand`s (`game::commands`), exactly as the keyboard's. `game::agent::session`: the session tools, answered after the simulation each frame, the long ones over several frames (a shot once the view has settled, a run once the clock stops).
- `SUNSCATTER_AGENT=<port>:<token>` turns the server on at start. `SUNSCATTER_DEV=1` is the developer flag (`bench`, `profile`, `spawn_vessels`). `SUNSCATTER_OFFSCREEN=1` renders into an image with no window (`game::offscreen`; shots work with the screen locked, and the game keeps running when the display goes, e.g. the lid closed).
- **The headless game for a mission test** (D122): `./run headless [port]` sets all three (port 7777 by default, token `dev`) and a fresh home dir for shots and saves (`SUNSCATTER_HOME` to keep one). Headless games neither load nor save settings, and a shader that fails to compile exits with code 3. One game at a time on the machine (the lock in [agent-rules.md](../plans/agent-rules.md)). How to fly a mission test is in [testing.md](testing.md).
- Test a running game by hand:

  ```sh
  curl -s http://127.0.0.1:7878/mcp -H "Authorization: Bearer <token>" -H 'Content-Type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_state","arguments":{}}}'
  ```
