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

The agent is at a **control location** like you (D067): mission control when the tracking station is open or the active vessel is uncrewed, the crew of the active vessel otherwise. It sees other vessels as their light arrives there, and cannot read a vessel with no signal. It sees what the player sees (D106): a vessel's readings come only from its working sensors, at their precision, and are `null` where nothing measures them; every number follows its menu's rule (times, to the clock's millisecond, and the orbit's angles once the position and velocity are known, distances through the position's reading, speeds through the velocity's, the eccentricity and the angles to the menu's decimals). Debug mode (`set_debug`) shows the truth. Game time runs while the agent thinks, as it does for the player.

### Flying

| Tool | Does |
|---|---|
| `get_state` | Clock (TDB), the origin's clock (TT, D088) and the active ship clock's offset from it, warp, location, active vessel, every vessel's status and signal, the navigation target |
| `list_bodies` | Bodies: name, GM, radius, what they orbit |
| `get_vessel` | A vessel (`id`, default the active one, as for every tool that takes a vessel) as seen from the location, as far as its sensors measure: position, velocity, altitude above the ellipsoid, orbit (the next apsides along the predicted path; while the active vessel flies live, `prediction_age_s`, how old that path is: it is recomputed a few times a second of real time, so while time runs it lags, and after a `step` or `run_until` it is from their stop; and `now_two_body`, the apsides from the centre if only the body's gravity acted from now, the Orbital menu's ORBIT NOW), attitude (heading, pitch, roll to the Landing menu's tenth of a degree, and the body axes in east-north-up to the attitude's precision), signal; for a craft of parts, `stages` (the next stage, tank levels, engines, the feeding tanks' share, and `sequence`: the staging bar's rows top to bottom, each stage's `stage` number, KSP number, name, place, commands and figures, Δv, burnout, propellant and the active stage's thrust, TWR, mass and Isp, in vacuum or with `air: "current"`, D120) |
| `get_trajectory` | A vessel's predicted path from now, sampled, each point to the position's precision (needs a navigation solution) |
| `set_warp` | Time warp level; resumes stopped time |
| `switch_vessel` | Fly a vessel: it becomes active, and the control location moves aboard it if it is crewed (mission control if not) |
| `go_to_mission_control` | Open the tracking station (mission control) |
| `set_controls` | Throttle, SAS, the hold and the speed reference of any vessel (`id`, default the active one; D112), after the light delay from anywhere but aboard; `rotate`, the pilot's rotation keys held, and `rcs` (off, on, translate, rotate) with `rcs_fine`, the pilot's RCS mode (R and the RCS menu), aboard the active vessel; `translate`, the pilot's translation keys held the same way (I K, J L, H N: body axes +X right, +Y down, +Z the nose; they move the craft with RCS on, and the reply warns when it is off). The held keys are `agent::HeldKeys`, added to the keyboard's in `keys::read_controls` |
| `read` | What the player reads on a page, as data, from the model the page draws (`agent::read`, one model, two views): a task `menu` (its sections, rows, notes, choices, buttons and switches, each control with the tool that works it, `by`: a command to the ship as `command {item: "<id>", do: "<label>"}`, the item that offers it now, as on the card and the inspector), a part's `card` (an unmeasured line with the sensor that would measure it), a systems-screen `layer`'s summary and an `item`'s inspector with its commands, or the `navball` (speed, altitude, G, Ap and Pe, SAS, RCS, the hold buttons, the throttle, the flight state), or `programs: true` (the PROGRAMS tab: each program's switch with each position's `set_switch` call, what it does and is doing now, its press with its `press` call or why there is none, and its `settings`: each one's `key`, label, value, unit, text and `configure_program` call, a load list with the components it could take). A choice's options each carry their exact call (`set_controls {rcs: "translate", rcs_fine: false}`, `set_target {body: "Moon"}`, `set_switch {program: "chutes", switch: "auto"}`). While time is stopped a menu is read from the stopped clock's snapshot. Not in the tracking station; the RCS menu while RCS is on |
| `edit_program` | The systems screen's stage and function editors (`agent::program`): `list` (`stages` or `functions`) read as the editor shows it (the piece's view: each entry's index, its commands' indices and labels, run or pinned, an abort and whether it is offered) with the `catalogue` of commands the craft takes; `edits` (add, remove, swap, rename, add_command by label, remove_command, swap_commands, pin) applied in order by the editors' rules (`systems_screen::edit`), anything the editor would not allow refused, then the list sent whole as "save to the vessel" |
| `set_switch` | A ship-computer program's switch (D126; the systems screen's PROGRAMS tab, the Maneuver menu's PROGRAM, Landing ▸ PARACHUTE's DEPLOY) on a vessel (`id`, default the active one): `program` (`maneuver`, the planned burns; `chutes`, the parachutes' deploy; an installed program's name) to `auto` (by itself), `press` (when pressed for, one run a press) or `off`; by the tab's own rule (`systems_screen::programs::set_switch`), sent like any command (`arrives_in_s`); a program not aboard is refused, naming those that are |
| `configure_program` | A program's settings (D126's config menu; the PROGRAMS tab's fields) on a vessel (`id`): `settings` maps each key `read {programs}` lists to its value: the parachutes' `arm_height_m` and `main_height_m` (m above the ground), a load manager's `loads` (component ids that draw power), `off_below` and `on_at` (shares of the charge, 0 to 1) and `period_s`, the planned burns' `align_deg`, `align_rate_deg_s`, `settle_s`, `settle_wait_s` and `steer_hold_s`; by the tab's rule (`programs::configure`: checked as the computer will check them, all or none, refused with why), sent like any command; answers with what they were |
| `press` | The press for a program (the tab's press; P for the parachutes): a planned burn switched to `press` flies when it wakes, the parachutes arm or, armed, the mains open once the drogues let them, an installed program runs once at its next epoch; refused with why where the tab greys it ("no burn planned", "nothing left to open", "it runs by itself") |
| `stage` | A craft of parts' next stage (`id`, default the active one), sent like any command |
| `command` | A craft of parts' commands as the player gives them, sent like any command: `function` runs one of its functions by name (the action bar's: an abort, the legs, the grid fins); `item` (a part's or component's id) with `do` runs one of its commands by its label, as its part card and the systems screen show them (IGNITE, SHUT DOWN, DEPLOY, OPEN, SEPARATE, RCS … on, POWER … off, ARM PARACHUTES). `item` alone lists its commands now; neither lists the functions and every item with commands. The functions are those the action bar and the systems screen offer: the ones the piece's parts still take, an abort only while it would save the crew (D120); the rest are refused with why |
| `edit_stages` | The staging bar's edits, one a call, by the stages' `stage` numbers from `get_vessel`: `move` a stage still to run, `move_command` one of its commands to another stage, `delete` one, or run one already run `again` (D114); the bar's own rules (`stage_bar::rows::check`), sent like any command |
| `set_target` | The navigation target from the Orbital menu's choices (`navball::target_choices`): a `vessel` (not the active one), a `body` with a surface, or `none` |
| `notifications` | The notifications' messages as the player reads them, numbered: those `after` the last one read, with whether each is an error and the vessel it is about |
| `set_plan` | Replace a vessel's planned burns (prograde/normal/radial Δv, time); sent with light delay. A craft of parts gets nodes (D126: `in_s` is the burn's middle, flown closed-loop by the maneuver program); the result gives each node's predicted ignition, duration and warnings, and the Δv available |
| `get_landing_prediction` | Where, when and how fast the active vessel meets the ground (needs a navigation solution), the braking burn and the surface motion, each as the Landing menu shows it |
| `closest_approaches` | Vessel `a`'s closest approaches to `b` (nearest first), as the Orbital menu shows the active vessel's to its target: both need a signal, each number as `a` measures it |

### The session (`game::agent::session`)

| Tool | Does |
|---|---|
| `shoot` | A screenshot (PNG) once the view has settled, the clock held meanwhile: the flight view or a named screen (`systems` on a `layer` with a `part` selected: a part isolated or a component, a battery or a bottle, selected, its editors at a `tab`: `stages`, `functions` or `programs`, `tracking`, `pause`, `settings` on a tab, `saves`, `help`), with `menus` open (the RCS menu while RCS is on), at a `width`/`height` (headless), at an interface scale (`ui_scale`: 0.2 shrinks it to specks). Returns the file's path (default `shots/` under the game's home) |
| `set_camera` | The camera's focus (the active vessel, another vessel or a body), distance, pitch and yaw about the focus's up, or the angle from a body's subsolar point |
| `pause` | Stop time (the player's `,`) |
| `step` | Run `seconds` of game time and stop exactly there, at a warp level; answers when the clock has stopped, with the vessel's state as `run_until`'s |
| `run_until` | Run until the first of: an epoch, `seconds`, the vessel's altitude above or below a value, its apoapsis or periapsis above one, its feeding tanks' share below one, max-Q, a separation, a status. Answers with what stopped it (`reached`) and the vessel's state as its sensors read it (`session::Sample::read`, the menus' rules as `get_vessel`'s: the altitude through an altimeter or the position, the osculating apsides (their heights above the ellipsoid under them, as the menus' Ap and Pe; `session::osculating_apsides`) through the position once the orbit is known, the orbital speed through the velocity, the dynamic pressure as the Landing menu shows it, the feeding share to its tanks' readings; null where nothing measures it). The stops compare these readings, so an answer agrees with what stopped it. A condition on something no working sensor measures is refused |
| `set_notify` | A craft's notifications (D128; `id`, default the active one; `on`): on (the default), a risk to it (its crew's air or life support, a part destroyed, no flight computer, an essential load shed) drops time warp at the risk's epoch with a toast; off, nothing on it stops warp (its events are still logged). A craft's warp-stop program decides instead while it runs. Answers with the setting and what it was |
| `new_flight` | A new craft on the pad (`craft`: a directory under `data/craft`) |
| `save_game`, `load_game` | Save or load by name (loading resets warp to 1×; stopped time stays stopped, so a test replays from a save exactly) |
| `list_saves` | The Saves window's list (newest first, the time saved) and whether the active vessel has a launch save (`saves::listing`) |
| `revert_to_launch` | The pause menu's Revert to launch (D114, `saves::revert`, one rule for both): the launch save, or a new craft of its kind on the pad without one; stopped time stays stopped |
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
