# Testing

Where each kind of test lives. D122 sets the direction: **one testing framework**. Unit tests (with specific deterministic sim cases in CI), plus mission tests flown headless through MCP by a development agent on the real flight software, with screenshots. Rules: tests come with the code (rule 6), and game rules are pure functions with table tests (rule 8).

| Kind | Where | Examples |
|---|---|---|
| Unit and property | next to the code (`#[cfg(test)]`) | Kepler round trips, terrain winding, the look's parts |
| Golden (bit-exact, both OSes) | `sim` | ephemeris generation, LEO coast hash, terrain sample hash |
| Accuracy vs DE440 | `sim` | Earth, Mars and Moon residuals |
| Invariance | `sim` | anchor choice; chunked = single pass; save/load |
| Scenarios | `crates/sim/tests/` | pad → orbit → parachute; landing on a mountain versus the sea; contact (slopes that hold or tip, bounces, slides, touchdowns survived or not, lift-off, SAS within its authority, rest across save/load, warp and frame rate) |
| Game rules | `game`, pure functions | `relations`, persistence, saves, tracking ids, map-view table (D054), impact marker (D080), zoom, fps, the bench's filter |
| Mission tests (D122) | the headless game and MCP, flown by a development agent ([below](#mission-tests)) | the Falcon 9 from the pad to orbit and the stage separation, with shots, the look's benchmark and the profiler at the key moments |

Game rules (visibility, hover, camera limits, lighting) are written as pure functions with table tests, and Bevy systems stay thin wrappers around them.

CI runs a fast `sim` job (no Bevy: fmt, decision references, clippy, tests) and a `game` job, on macOS and Windows. The git pre-push hook (`tools/git-hooks/pre-push`) runs the same checks on the commit being pushed.

## Mission tests

A mission test flies a real mission in the real game, and an agent makes the decisions a player would. Run one for a large integration of new features (a new craft, flight software, staging, aerodynamics or the look), and look at what came out. The tools are the player's agent's tools ([mcp.md](mcp.md)): the test sees what the vessel's sensors measure, and debug mode (`set_debug`) shows the truth.

### Start the game

1. Take the machine's game lock: `mkdir target/demo.lock`. If it fails, another agent's game is running, so do other work and try later ([agent-rules.md](../plans/agent-rules.md)).
2. Build first, in the background (a release build of `game` takes 1–2 minutes warm): `CARGO_TARGET_DIR=target/agents/<name> cargo build -p game --release -j 4`.
3. Start it in the background, with saves and shots in your scratchpad:
   ```
   CARGO_TARGET_DIR=target/agents/<name> SUNSCATTER_HOME=<scratch>/home SUNSCATTER_PROFILE=1 ./run headless 7781 > <scratch>/game.log 2>&1
   ```
   `./run headless [port]` turns on the MCP server (token `dev`), renders into an image with no window, sets the developer flag (`bench`, `profile`) and uses a fresh home dir unless `SUNSCATTER_HOME` is given. `SUNSCATTER_PROFILE=1` adds the profiler's tables to `profile`. The game starts on the pad with the Falcon 9, time running.
4. Wait until it answers (a few seconds): `MCP_PORT=7781 tools/mcp.sh get_state`.
5. When done: `pkill -f "agents/<name>/release/game"`, then `rmdir target/demo.lock`.

`tools/mcp.sh <tool> '<json>'` posts one `tools/call` and prints the answer. Set `MCP_PORT` to your port.

### Keep every call short

The stream watchdog stops an agent whose call shows no output for 600 s. Keep each call under about 5 minutes of real time:
- give `run_until` a `seconds` bound as well as its event, and a `timeout_s` of 240 or less (the server lets a call wait up to 15 minutes, so the bound must come from you);
- at warp 2, game time ran about 3× faster than real time while the Falcon 9 burned (46 game seconds took 16 s). Physics warp is capped while engines run;
- run builds, pushes and the game itself in the background;
- stop time (`pause`) before you think. `step` and `run_until` stop it again at their end.

### The tools a test uses

- **Time:** `pause`; `step {seconds}` (it stops at that epoch exactly); `run_until` with an epoch or an event: `max_q`, `feeding_below` (the share left in the tanks feeding the running engines), `separation` (the fleet grows), `altitude_above_m`, `apoapsis_above_m`, `periapsis_above_m`, `status`. Its answer gives `reached` (which stop) and the vessel's state then. From a stop, the same calls give the same flight to the metre: a replay from a save is a determinism check.
- **Flying:** `set_controls` (throttle, SAS, the hold, the speed reference, and `rotate`: the pilot's rotation keys held, body axes x pitch, y yaw, z roll); `stage` (the next stage of the craft's sequence: ignition, liftoff, MECO, separation …); `switch_vessel`.
- **Reading:** `get_vessel` (position, velocity, altitude, orbit and apsides, attitude with the body axes in east-north-up, and `stages`: the next stage, tank levels, engine states, the feeding share); `get_landing_prediction` (vertical and horizontal speed over the ground, impact); `get_state` (the clock, the fleet, your control location).
- **Looking:** `set_camera` (`focus` "ship", a vessel or a body; `distance_m`, `pitch_deg`, `yaw_deg`; `sun_angle_deg` about a body); `shoot` (`path` in your scratchpad; `menus` open, e.g. `["orbital"]`; `screen` `systems` with `layer` and `part`, `tracking`, `pause` or `settings` with `settings_tab`; `width`/`height`; `ui_scale` 0.2 for a shot of the scene). It waits for the terrain and holds the clock meanwhile. Look at each shot with the Read tool.
- **Sessions:** `save_game`/`load_game` (pause first, and time stays stopped through the load); `new_flight {craft}`; `set_debug`.
- **Developer:** `bench` (the look's frame time, and without each part; `only` names parts, the whole look is always measured; about 4 s per configuration); `profile {seconds}` (fps, worst and 95th-percentile frame, the warp achieved, and with `SUNSCATTER_PROFILE=1` the CPU per system).

### Worked example: the Falcon 9 to orbit (2026-10-30)

Each step is one call unless it says otherwise. The numbers are what this flight gave; the real Falcon 9's are in brackets.

1. `pause`. `shoot` on the pad: `set_camera {focus: "ship", distance_m: 150, pitch_deg: 10, yaw_deg: 30}` frames the whole rocket.
2. `set_controls {throttle: 1}`, `stage` (Ignition), `step {seconds: 3}`. `get_vessel`'s `stages.engines` go from `Starting` to `Running`, and the HUD says "clamped · engines lit".
3. `stage` (Liftoff: the clamps let go at full thrust), `step {seconds: 10}`. It climbs straight up on SAS stability: pitch 89.98°.
4. **The pitch kick.** Rotating about local north tips the nose east. The rotation input is in body axes, so take north's components in them from `get_vessel`'s `attitude.axes_enu`: `rotate = 0.04 × [x[1], y[1], z[1]]` (here `[-0.0065, 0.0395, 0]`), with `sas: false`. `step {seconds: 2.5}`, then `rotate: [0,0,0]`, `step 3`. The nose is then at 86.6° and still turning at about 0.8°/s.
5. `set_controls {sas: true, hold: "stability"}`, `step 4`: the turn stops at 86.1°, and the flight path (from `get_landing_prediction`'s surface speeds) closes on it (86.3°). Then `hold: "prograde", speed_reference: "surface"`: the gravity turn. Flight path 63° at 9.6 km, T+60 s. `save_game {name: "ascent-t20"}` here.
6. `run_until {max_q: true, warp: 2, seconds: 90}`: max-Q at T+66 s, 34.6 kPa at 12.1 km [about T+72 s, ~33 kPa]. A shot with `menus: ["orbital"]`.
7. `run_until {feeding_below: 0.08, warp: 2, seconds: 120}` (8 % kept for the booster's return): T+138 s, 65.3 km, 2.17 km/s over the ground, flight path 32° [MECO at T+150–160 s, 65–70 km, ~2.1 km/s]. `stage` (MECO), `step 2`.
8. `stage` (Stage separation), `run_until {separation: true, seconds: 10}`: `fleet` 2. `step 3`, `stage` (Second stage ignition), `step 6`: MVac `Running`, TWR 0.84. A shot shows the upper stage burning and the booster falling away. `switch_vessel {id: 2}` and a shot of the booster, then back.
9. **Insertion.** Orbit prograde climbs too steeply, to an apoapsis near 190 km while still far below orbital speed. Steer the pitch instead: each round, read `get_vessel`, choose the pitch whose thrust's vertical share cancels gravity less the centrifugal term and brings the vertical speed toward zero at the target height, and turn with a 1 s `rotate` pulse about up × heading (`[-sin h, cos h, 0]` in east-north-up, so it works at any pitch), then SAS stability. A pulse of 0.2 turns the upper stage about 3°, 0.3 about 6°. `step 1` to `step 4` per round. The throttle comes off when the periapsis passes the target.
10. `set_controls {throttle: 0}`, `stage` (SECO): `coasting`, 189 × 600 km at 28.7° (the 4 s rounds overshot near burnout, when the stage accelerates fastest). A shot with Orbital open, then `profile` and `bench` there.

The booster, uncontrolled after separation (its return guidance is not built, D123), broke up on its way down; its pieces were then deleted as debris (D083).

### Worked example: planned burns on the second stage (2026-10-31)

From an orbit save (step 10 above), `set_plan {id, burns: [{in_s, prograde}]}` with `in_s` the node: the burn's middle, here the apoapsis's `in_s` from `get_vessel`. Its answer gives each node's ignition, duration and warnings, and the Δv available. `run_until {seconds, warp: 6}` to a few minutes before the node: the vessel wakes at the program's epoch whatever the warp ("burn 1: turning to the burn" in the log; the Maneuver menu says what the program is doing). Then `step` through the settling, the ignition and the cutoff ("burn 1: on target (0.26 m/s left)"). Read the orbit from a `step` or `run_until` answer (`apoapsis_m`, osculating, from the state) rather than `get_vessel`'s apsides, which come from the predicted path and lag the state while an engine runs. Flown numbers and shots: [planner.md](planner.md).

### Worked example: the playtest save, Dragon alone in orbit (2026-10-31)

**Fly it with `tools/playtest_flight.py <port> all`** (LAYERS-3; in the background, about 10 minutes, then `check <save>` in a fresh home): the whole procedure below through MCP, the insertion steered by itself to level flight at 200 km and cut off when the periapsis passes 190 km. Flown at SAVE_VERSION 20 (INTERFACE-12, after 50ee2f3's bump): Dragon alone at 199 × 645 km; checked by `check`: 150 s at 1×, then an hour at warp 6. Steps 1 to 10 above with debug off (the first flight: orbit 194 × 489 km at 28.7°), then `stage` (Dragon separation): Dragon stays vessel 1 (the capsule is the craft's root) and the second stage leaves. The booster has meanwhile broken up on its uncontrolled entry and its pieces have gone as debris. The second stage has a flight computer on charged batteries, so the debris rule keeps it (D083): deorbit it with `set_plan {id, burns: [{in_s, prograde: -100}]}` at its apoapsis (`get_vessel`'s `orbit.apsides` give each apsis's `in_s`; 100 m/s there puts the periapsis under the ground), then `run_until {seconds: 2700, warp: 6}`, `step` through the burn and `run_until` on: it re-enters, breaks up and goes. `pause`, `save_game`, and copy the file over `docs/plans/playtest-2026-10-01-orbit.ron` (`./run play` copies it to its quicksave). Check it in a fresh game: the file as `saves/quicksave.ron` in a new home, `load_game {name: "quicksave"}`, then `step` on. The save is about 690 KB, nearly all the coast's samples; the craft is named by its id (`falcon9-crew#0-7`). Dragon is the active vessel (its id is whatever the flight issued: 11 after a flight restarted with `new_flight`).

### What to check

- Each call's `reached` is the stop you asked for, not `timeout`, `paused` or `vessel_gone`.
- The engines' states, the stage list's `next` and the fleet's size after each `stage`.
- The numbers against the real vehicle's (the vehicle's dossier in `docs/vehicles/`), such as max-Q, MECO and orbit, with the sources.
- Every shot, by eye. The vehicle is whole and lit, the plume shows, the terrain is loaded, and the interface reads the same as `get_vessel`. Nothing is clipped or missing.
- The log (`grep -v render_scale <scratch>/game.log | tail`): breakups, warnings and panics. A shader that fails to compile ends a headless game with exit 3.
- Determinism, where it matters: pause, load a save and fly the same calls again. The numbers must be identical.
- Record the flight in your brief: what you flew, the shots' paths, the numbers, and what broke and how it was fixed.
