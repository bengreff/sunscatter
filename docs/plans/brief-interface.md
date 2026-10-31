# Brief: INTERFACE, every piece of information and every control within easy reach (2026-10-31)

The owner, 2026-10-31: "At least one agent should work on cleaning things up for the user, making it easy to access all the information and controls, plus adding a feature for the vessel view: thermal actually shows the grid level thermal mesh, plus clicking on a part isolates it and zooms in while pressing esc or closing that window goes back to the original layer view of the part."

D123 froze visual work until both flights fly. The owner's request is an exception for the interface (recorded in D123): access to information and controls, and the systems screen's two features. The look itself stays frozen: no restyling, no plume, cloud or terrain work.

Follow [agent-rules.md](agent-rules.md). Your name is INTERFACE and your target dir is `target/agents/interface` (a cold start; you need a release build of the game for headless runs).

Two other agents run at the same time:
- **PLANNER-3** ([brief-planner.md](../archive/briefs/brief-planner.md)) moves the game's planner onto nodes: `game::planner`, `interface/menus/maneuver.rs`, the trajectory predictions, and MCP's `set_plan` in `game::agent`. Leave those files to it, and list what they need in your State.
- **RESEARCH** ([brief-research.md](../archive/briefs/brief-research.md)) writes docs only.

ONEKIND-4 starts later and moves the game's tests off the test craft (`menus/*` tests, `stage_bar/rows_tests`, `navball`, `saves`). Commit often, so it can work on top of you.

## Read first

- D080, D106, D107, D108, D111, D112, D113, D120, D123, D126 in `docs/decisions.md`.
- `docs/systems/game.md`; `docs/technical/systems-screen.md`, `flight-menus.md`, `staging-bar.md`, `ship-commands.md`, `aero-thermal.md` (the skin cells and the volume grid: `sim::craft::volume`, `sim::craft::design`, `sim::thermal`), `knowledge.md`, `testing.md` and `mcp.md` (how to run the game headless and take shots).
- Code: `game::systems_screen` (`mod.rs`, `view.rs`, `rules.rs` and its `pick`, `ui.rs`, `summary.rs`, `inspect.rs`), `game::interface` (`action_bar`, `help`, `menus`, `part_card`, `stage_bar`, `pause`), `game::settings_ui`, `game::map_view`.

## The work, in order

1. **The map of the interface** → `docs/technical/interface-map.md`. Fly the Falcon 9 headless through MCP (pad, ascent, orbit, separation) and shoot every screen:
   - the flight view, the navball cluster, the staging and action bars, every menu, the part cards;
   - the systems screen at each layer;
   - the map and the tracking station;
   - pause, settings and help.

   List every piece of information and every control, with every way to reach it (key, click path, MCP tool). Mark what is hidden, missing a way in, reached two different ways that disagree, or closed in different ways. Look at the shots with the Read tool.
2. **The cleanup, from the map**, in small commits, each with its table tests (rule 8: rules as pure functions, Bevy systems only wire them):
   - one obvious way into each thing;
   - Esc always backs out one level;
   - a help overlay that lists every key and every way in, generated from the same tables the input uses, so it cannot drift;
   - the craft's information one click from the flight view;
   - anything the map found unreachable.

   Keep the owner's earlier rules: no pop-ups in the systems screen (the selection shows in the right panel), the interface never shows something wrong (D106), and debug mode shows the truth.
3. **The thermal layer shows the thermal mesh itself.** Today it colours each part by its hottest cell or node. Draw what the thermal network is instead:
   - each skin cell coloured on the craft's surface by its own temperature;
   - the interior nodes (the volume grid's cubes, `node_size`) as a grid you can see into: translucent, or a cut-away you can move.

   Colours follow D106: a cell a sensor measures is coloured, and the rest show the grid in a neutral "not measured" shade; debug mode colours everything. Record that default under "Open for the owner". Keep it cheap for a craft of many cells. The stress craft (`stress-<n>`) shows the cost.
4. **Clicking a part isolates it and zooms in.**
   - The other parts hide or ghost, and the camera frames the part.
   - The right panel shows the part, as now.
   - **Esc**, or closing the part's panel (its ×), goes back to the layer view as it was: the same layer, camera pose and selection state. A second Esc closes the screen, as now.
   - The part list (PARTS) isolates the same way.
   - The camera's framing and the Esc stack are pure functions with table tests.
5. **Mission check:** shots of the cleaned-up screens, the thermal mesh during the Falcon 9's ascent (the nozzles and the base heating) and in orbit (a sunlit side against a shaded one), and a part isolated and restored. List the shots in your State for the lead.
6. **Docs:** `systems-screen.md` (the thermal mesh and isolation), `flight-menus.md`, `interface-map.md`, the status lines in `systems/game.md`, `owners.md` rows.

Small commits, each passing fmt, clippy and the game's tests you touched (`cargo test -p game <filter>`), plus `cargo test -p sim` if you touch sim. One game at a time, under the lock in agent-rules.

## State

**INTERFACE-10 (2026-10-31), wrapped up.** Did INTERFACE-9's Left 1–3: finding 58, the menus' `by` naming its item (finding 59), and the walk of every screen at HEAD (pad, ascent, after separation, the booster, orbit, the map, Dragon alone, every layer, tracking, pause, settings, help), each page read through `read` beside its shot; findings 60–67 found, 60–66 fixed at their owners with table tests. LAYERS-2's unfinished sim work sat in the shared tree, so every commit passed fmt, clippy (game) and the touched modules' tests (139) in a private mirror of HEAD plus my files; the pre-push check ran the whole suites.

**Landed:**
- 3d3ada5: finding 58, `step`'s and `run_until`'s state through the craft's knowledge: `session::Sample::read` (the menus' rules; `agent::known_altitude`, `menus::landing::dynamic_pressure`, the feeding share as a sum of tank readings, `share_of`); the stops compare the readings.
- 390355a: finding 59, `read::command_by`: every control's `by` names its item and label (`command {item: "dragon.draco#1", do: "RCS dragon.draco#1 off"}`), a menu's ship command by the item that offers it now.
- 7bc08b0: finding 62, the run's apsides above the ellipsoid under them (`session::osculating_apsides` through `sim::kepler`), as the menus' Ap and Pe; they read 4.7–5.4 km low at Kennedy's latitude.
- be21ddc: finding 60, the Landing menu's parachute q through `dynamic_pressure`; no advice when unmeasured (`Opening::Unmeasured`).
- ed7d29e: finding 61, `stage_bar::draw::scroll_room`: the pad shows every stage (Dragon separation was scrolled out of view).
- 8aead09: finding 63, `tracking::left_lit_notice` links back to the vessel left burning.
- 542556c: finding 65, `agent::vessel_or_active`: `get_vessel`, `get_trajectory`, `set_plan` default to the active vessel.
- fc34d4d: finding 66, `get_vessel`'s attitude to the Landing menu's decimals (`format::ATTITUDE_DECIMALS`), its axes to the attitude's precision.
- be09730: finding 64, Dragon's Parachutes and Nosecone pinned (the craft file and the playtest save): Dragon alone had an empty action bar.
- the docs commit after them: owners.md rows, this State, the plan's row, a note in brief-layers.md.

**Live** (headless, debug off, HEAD be09730): the pad's bar shows all seven stages and the action bar Parachutes and Nosecone (`/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface10/shots/v_pad.png`); `step` on the pad reads `altitude_m` 30.0, `apoapsis_m` 30.0; at T+62 s `apoapsis_m` 15869 against the navball's 15.6 km (was 10949; the rest is the predicted path's drag); `get_vessel` without `id`, attitude 9.3 / 90.0 / 0.0; the engines-lit notice with "Vessel 1 ›" and `vessel: 1` (`/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface10/shots/v_notice_link_crop.png`). Before the fixes: `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface10/shots/a_pad.png` (the hidden stage), `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface10/shots/b_t60.png` (the parachute q, Ap 15.6 km), `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface10/shots/c_booster.png` (the notice without a link), `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface10/shots/e_dragon_rcs.png` (Dragon's empty bar; its RCS authority BACK only, INTERFACE-7's open item).

**Found, not mine:** since LAYERS-2's f2806ca added Dragon's cabin and barometric sensors as components, **every older `falcon9-crew` save crashes the game at its first power step**, the playtest save (`./run play`) included: `sim::systems::power` index out of bounds (the save's 39 power entries against 41 components). Noted in brief-layers.md.

**Left, in order:**
1. After the save crash is fixed (a save whose ship state does not fit its craft refused at load, or SAVE_VERSION bumped, and the playtest save flown again, testing.md): walk the ascent, separation and orbit again from fresh saves (this walk used INTERFACE-8's, which now crash).
2. Finding 67: `Reading::shown`'s binary noise in the agent's numbers (`speed_ms` 2000.8000000000002): round through the step's inverse (`sim::systems::data`, LAYERS-2's this round).
3. Small: the RCS menu's USE options could each carry their exact `set_controls` arguments (`by` per option: "translate only" is `rcs: "translate"`); `get_landing_prediction`'s latitude and longitude print 15 digits (quantised to the position's precision along the ground, D106-right but noisy).

**Open for the owner (INTERFACE-10):**
- `step` and `run_until` stop on what the craft reads (its readings, not the truth), so an answer always agrees with what stopped it; the dynamic pressure reads to the Landing menu's 10 Pa in every mode, as the orbit's angles keep the menu's decimals in debug mode.
- The run's apsides are osculating (two-body from now), measured above the ellipsoid under each apsis like the menus' Ap and Pe; the menus' come from the predicted path, so under drag or J2 they still differ by that physics (270 m at T+62 s, ~10 km at a 640 km apoapsis).
- The agent's heights (`altitude_m`, the run's apsides) are above the reference ellipsoid; the navball's above the datum. The same on Earth (sea level 0), off by the lowest ground on the Moon and Mars; one rule would read the navball's datum (`navball::altitude::body_datum`, kept per body).
- Without a velocity reading the parachute's advice says "q not measured (no velocity reading): no advice" rather than guessing safe or too fast.
- Parachutes and Nosecone are pinned on the Falcon 9's action bar (eight entries on the pad, two on Dragon alone).

**INTERFACE-9 (2026-10-31), wrapped up.** Closed INTERFACE-8's Left 1–5, the parity table's gaps and overs (interface-map.md's parity section now reads "yes" on every row), found and fixed finding 57, and wrote the `owners.md` rows (INTERFACE-8's and mine, in the docs commit after this State's first). Not done: the walk of every screen (only the new tools were checked live). LAYERS' sim move left the shared tree uncompilable mid-round, so every commit passed fmt, clippy (game) and the touched modules' tests in a private mirror of HEAD plus my files; the pre-push check ran the whole suites.

**Landed:**
- 242881a: the navball cluster's rows as a model: `navball::rules::{readouts, controls}` (`Readouts`, `ControlRows`, `Button`, `hold_short`), gathered by `navball::control_rows`; `draw.rs` only lays them out. Table test of the controls.
- df0a483: MCP `read` (`agent/read.rs`, one model two views): `{menu}` (`MenuId::sections` of the dock's snapshot; while time is stopped it waits up to 60 frames for the stopped clock's snapshot: `FlightData::clock`, `read::fresh`), `{card}` (`part_card::Gathered`, now the cards' own inputs), `{layer}` and `{item}` (`systems_screen::summary::with_inputs` and `view::facts_of`, now the screen's own builders; `rules::Temps::of`), `{navball}`. Unmeasured lines carry `measured_by`; each menu control says which tool works it (`by`). Refused in the tracking station and for the RCS menu while RCS is off. Tests in `read_tests.rs`.
- 1c88f1e: `set_controls {translate}`: `agent::HeldKeys` (rotation and translation, replacing `HeldRotation`; `Effect::Hold`), added in `keys::read_controls`; a warning when RCS is off.
- 2b7a6e3: `edit_program` (`agent/program.rs`): a list read as the editor shows it (the piece's view) with the catalogue; edits (add, remove, swap, rename, add_command by label, remove_command, swap_commands, pin) by the editors' rules (`systems_screen::edit`), anything the editor would not allow refused; the list sent whole as "save to the vessel".
- dde5b7f: the overs (D106): `get_vessel`'s orbit, `get_trajectory`, `get_landing_prediction`, `closest_approaches` through the craft's knowledge by the menus' rules (`agent::known_time`, `known_digits`, `known_ground`; `format::DEGREE_DECIMALS`, `ECCENTRICITY_DECIMALS`, read by the Orbital menu too); `closest_approaches` needs a signal to both vessels.
- 4bb3520: `list_saves` and `revert_to_launch`: `saves::listing`, `saves::revert` (one rule, the pause menu's Revert reads it).
- f0fb5cb: finding 57, a body's distance (its double-click menu) through the position's reading, `hud::body_distance`, and `list_bodies`' `distance_m`.
- the docs commit after them: mcp.md, interface-map.md (the parity table, findings 57–58), the plan's row, this State.

**Live** (headless, debug off, the Falcon 9 on the pad, HEAD 4bb3520): `read {navball}` ("SAS ON", "CLAMPED", G "1.00 / 7.5 g ▲", Ap "—"), `{menu: orbital}`, `{card: dragon.capsule}` (NTO "—/1587 kg", `measured_by` "a level sensor in the tank: none fitted"), `{layer: structural}` (560 t, centre of mass 29.2 m), `{item: s2.battery#1}`; `edit_program` defined "Nose open" (DEPLOY dragon.nose), pinned: it shows on the action bar; a stage removed on the pad came back with `revert_to_launch`; `translate` with RCS off warned. Shots: `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface9/shots/a_pinned_bar.png` (the agent's function on the action bar), `a_pinned.png`.

**Left, in order:**
1. Finding 58: `step` and `run_until` answer with the vessel's `state` as raw truth: `session::Sample::json` through `Known`.
2. The walk of every screen at HEAD (pad, ascent, separation, orbit, Dragon alone), reading each page through `read` beside its shot.
3. Small: a menu control's `by` for a ship command says `command {item, do}` without naming the item; `read {menu}` could give it.

**Open for the owner (INTERFACE-9):**
- The agent reads every page as data through `read`, from the model the page draws; a menu read while time is stopped waits for the stopped clock's snapshot (at most 60 frames), so a replay reads the same.
- `edit_program` applies a call's edits to the vessel's list and sends it whole at once (the player edits a draft, then saves); the player's open draft is left alone.
- `closest_approaches` still takes any two vessels (the player can switch to any and target any) but needs a signal to both and reads through the first's knowledge; it lists up to 5 where the Orbital menu shows the first.
- The orbit's angles and eccentricity reach the agent to the Orbital menu's decimals (2 and 5); an impact's latitude and longitude to the position's precision along the ground.
- Translation held by the agent with RCS off is kept, with a warning (the player's keys do nothing then either).

**INTERFACE-8 (2026-10-31), wrapped up at its context cap.** Did INTERFACE-7's Left 1–3 and the first half of item 2 (the parity table); the gaps it lists are not closed yet, and the walk (item 3) is not done. Every commit passed fmt, clippy (game; sim too for 09e51e1) and the touched modules' tests (sim data and estimate 12, the card 14, agent, state and trajectory 71); the pre-push check the whole suites. STATE-4 runs beside (it landed 2ddecb8, SAVE_VERSION 16, and has `owners.md` and `brief-state.md` open).

**Landed:**
- 09e51e1: a sum's precision by one rule, `impl Add` and `Sum` for `sim::systems::Reading` (the precisions add, the estimate's worst case; noted in brief-state.md), read by the card's group totals (`part_card::inside::total`) and the estimate's fuel bars. The second stage's two batteries read "2800/4000 Wh (70 %)" (1 %), not to half a per cent.
- f7f3d2e: finding 56, the card's unmeasured store by its rule ("—/793 kg", the sensor on hover): `Value::Unknown { text, sensor }`, `content::known`, `Live::held`.
- c103d5f: finding 55, the predicted path losing its apsides mid-insertion: it was a path from before the stop (0.7 s old under 40 m/s² is ~100 km of periapsis), its age hanging on real-time scheduling. `step` and `run_until` answer once the path is the active vessel's from the stopped clock (`state::Prediction::{from, current}`, `session::answer_now`, ≤ 5 real s; table tests). Live: `prediction_age_s` 0.02 s after each `step` under thrust (was up to 0.8 s); a step costs ~0.7 s of real time.
- the docs commit after them: interface-map findings 55–56 and **the parity table** ("Parity between the player and the agent": every piece of information and control, its way in, MCP's, and the gaps and overs), mcp.md, the plan's row, this State.

**Shots** (`/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface8/shots/`): `a_pad.png` ("LOX 286/286 t", finding 51), `a_pad_cards.png` (the batteries' sums; Dragon's NTO and MMH still a bare dash, before f7f3d2e), `g_sep.png` ("Vessel 1 separated: a new vessel · Vessel 2 ›", finding 54), `g_booster_switch.png` ("predicting the impact…", "no parachutes aboard", findings 52–53). Saves (format 16) in the scratchpad's `home/saves`: `x-t60`, `x-s2` (second stage burning), `x-orbit` (196 × 644 km), `x-orbit2` (186 × 207 km, 1 s rounds near burnout: `insert3.py`).

**Left, in order** (the designs are in interface-map.md's parity section):
1. A `read` tool for what the player reads on a page, from the models the screens draw: `{menu}` (`MenuId::sections(&FlightData)`), `{card}` (`part_card::content::card`), `{layer}` and `{layer, item}` (`summary::summary`, `inspect::details`; build their inputs by one helper shared with `systems_screen/ui.rs` and `view.rs`), `{navball}` (move the readouts' and controls' texts out of `navball/draw.rs` into `navball::rules` rows first). Table tests; mcp.md.
2. `set_controls {translate}` (the agent's held keys for rotation and translation: `agent::HeldRotation`, `commands::keys::read_controls`; body axes +X right, +Y down, +Z the nose).
3. An `edit_program` tool over `systems_screen::edit::{allowed, apply, catalogue}` (define stages and functions, rename, their commands, pin), sent as `SetStages` / `SetFunctions`.
4. The overs (D106): `get_landing_prediction`'s and `get_vessel`'s orbit numbers and `get_trajectory`'s points through `known_value` (`Reading::shown`); `closest_approaches` limited to what the Orbital menu shows (the active vessel and its target, with signal).
5. Small: the saves' list, revert to launch (`saves::launch_of`), a body's distance.
6. The brief's item 3: walk the screens again.
7. `owners.md` rows (STATE-4 holds the file): `Reading`'s `Add` and `Sum` (a sum's precision, `sim::systems::data`); `state::Prediction::current` and `session::answer_now` (a run's answer after its stop); `part_card::content::known` and `Live::held` (a card's value unmeasured).

**Open for the owner (INTERFACE-8):**
- A sum's precision is its terms' added (the worst case, as the estimate's bounds), so a group of four batteries each to ±25 Wh reads to ±100 Wh: "10.0/10.0 kWh (100 %)" rather than a share to a tenth (this replaces INTERFACE-7's "half a per cent" note).
- `step` and `run_until` wait (up to 5 real seconds) for the predicted path from their stop before answering, so an agent's reading after a stop is deterministic; `pause` does not wait.
- The settings (orbits, controls, interface, visuals) are the player's preferences, not the flight, so MCP does not reach them; the agent reads and flies what the player reads and flies.

**INTERFACE-7 (2026-10-31).** Did INTERFACE-6's Left 1–4: findings 47 and 48, the centre of mass, and the walk of every screen at HEAD (findings 49–53 in [interface-map.md](../technical/interface-map.md), found and fixed). Every commit passed fmt, clippy (game; sim too for 5cdf7df) and the game's touched modules' tests (137); 5cdf7df the estimate tests, with the whole sim suite left to the pre-push check (the machine at load 19). STATE-3 committed its open files mid-round; STATE-4 now runs beside.

**Landed:**
- 01f9556: finding 47, a battery's charge by one rule, `format::charge` (`amount`'s in Wh, kWh from 10 kWh, with its share, `format::share`, the tank inspector's share made one owner): the Power menu, the Electrical summary, the card's battery and batteries, the inspector ("1400/2000 Wh (70 %)").
- 6766271, e540dcc: finding 48, a bottle's contents through its pressure transducer (`systems_screen::inspect::bottle_contents`, the gas law at the bottle's temperature), read through `format::amount` by the RCS menu's feeds (a tank feed by its level sensor; `Feed::kg` is a reading) and the inspector's bottle.
- 5cdf7df: the centre of mass, `sim::systems::estimate::centre` (noted in brief-state.md): the parts aboard less the pad's mount, over the lowest contact point aboard, its precision the farthest the tanks' known ranges move it (a search over nine levels per tank, each slug computed once); `bound` reads a per-tank `range`; the Structural summary reads it through `format::metres` ("29.2 m above the base" on the pad, no dash; the mount had made it 28.9 m from its own foot). Test file `estimate_centre_tests.rs`.
- 685c4ea: finding 49, pressures and tank temperatures by one rule each: `format::pressure` (bar; two decimals at most under 10 bar, one under 100, none above), `format::bar` (an exact limit), `format::kelvin`, for the card, the inspector and the Plumbing summary.
- 6d65d31: finding 50, MCP `shoot {part}` selects a component too (`inspect::named`), so the battery and bottle inspectors have an agent's way in.
- 782abdb: findings 52, 53, the Landing menu just after a switch: "predicting the impact…" until the flight's first prediction (`LandingPrediction::of`, `FlightData::predicting`), and "no parachutes aboard" on a craft without canopies (the booster offered "safe to deploy").
- 3271e56: finding 51, a full tank no longer reads more than full ("286/285.8 t" → "286/286 t"): `amount` and `charge` give the capacity the held's digits.
- 4c72550: finding 54, a vessel's name by one rule, `tracking::vessel_name` ("Vessel 1 separated: a new vessel", not "Vessel #1"; "Vessel 3 deleted as debris"; the systems screen's bar).
- the docs commit after them: interface-map 47–53, systems-screen.md, flight-menus.md, mcp.md, owners.md, the plan's row, this State.

**The walk** (headless, debug off, testing.md's steps; shots in `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface7/shots/`): the pad (`a_*.png`: every layer, the battery and bottle inspectors `a_sys_battery.png`, `a_sys_bottle.png`, the cards, the Power and RCS menus, help, pause, settings' four tabs, saves, tracking), T+60 s (`b_*.png`: 8.7 km, 34 kPa; the screen's mass "401 t" equal to the bar's), after the stage separation (`g_sep_*.png`, the booster `g_booster*.png`: findings 52 and 53 seen there), orbit after SECO (`c_*.png`, 194 × 821 km: the last insertion rounds overshot near burnout) and Dragon alone (`h_dragon*.png`). For the lead: `a_menus_power.png` and `c_c_menus2.png` (the batteries by one rule), `c_rcs.png` (the feeds), `c_battery.png`, `c_bottle.png` (the inspectors through MCP's component selection), `c_struct.png` and `c_h_struct.png` (the centre of mass, pad and Dragon), `c_plumb.png` (the pressures; the "286/285.8 t" before finding 51), `g_booster.png` (findings 52 and 53 before), `h_dragon.png` (Dragon's RCS authority, below). The old saves no longer load (STATE-3's format change); this round's are `w-t60`, `w-s2`, `w-orbit` (format 14; STATE-4's 223af2a moved saves to 15, so they may no longer load) in the scratchpad's `home/saves`.

**Left, in order:**
1. A walk at HEAD of this round's last three fixes on screen (tested, not shot): "286/286 t" on the pad's bar, "predicting the impact…" and "no parachutes aboard" on the booster just after the switch, "Vessel 1 separated" in the notifications.
2. The part card's group totals (`part_card::inside::total`) take the largest precision of their items: four batteries each to ±25 Wh read as ±25 Wh ("9.8/10.0 kWh (98.4 %)"); the estimate's bounds add them (worst case). One rule for a sum's precision, at its owner.
3. The card's unmeasured tank group shows a bare dash (the sensor on hover) where the screen and the RCS menu give "—/793 kg"; small.
4. For other owners: Dragon alone translates only BACK on RCS with all five blocks on (`h_dragon.png`: RIGHT, LEFT, UP, DOWN, FORWARD "none"; the thruster model or the craft's Draco data); mid-insertion at 160 × 216 km the predicted path had no apsides and an impact in 58 min (`get_vessel`: `apsides: []`, `now_two_body` right), so `insert.py` could not see its periapsis (`game::trajectory::apsides` or the prediction's drag); the booster is deleted as debris "dead" while it has batteries (STATE-4's `powered()`).

**Open for the owner (INTERFACE-7):**
- A bottle's contents read through its pressure transducer with the bottle's temperature taken as known (D106's simplified error): "52/52 kg"; a dash with its full load where no transducer reads it.
- The centre of mass reads as a height above the lowest point aboard, to the precision the tank sensors allow ("29.2 m" on the pad with Dragon's tanks unmeasured), the pad's mount left out of its mass and of its base.
- A pressure's digits go by its size (two decimals under 10 bar, one under 100, none above) and its reading's precision; a tank's or battery's capacity is shown to the held's digits ("150/287 t", not "150/287.4 t"), so a full tank never reads more than full.
- A battery group's sum on the card is known to half a per cent, so its share shows a decimal ("4000/4000 Wh (100.0 %)") where a single battery's shows none.
- Right after a switch or a load the Landing menu says "predicting the impact…" until that flight's first prediction, rather than "no impact predicted"; a craft without canopies reads "no parachutes aboard" in PARACHUTE.
- A vessel is "Vessel 3" wherever the player reads it (the logs keep "#3"); a deleted piece reads "Vessel 3 deleted as debris: …".

**INTERFACE-6 (2026-10-31), wrapped up at its context cap.** Did INTERFACE-5's Left 1–3 and the hunt for quantities shown by two rules (findings 41–48 in [interface-map.md](../technical/interface-map.md)); the walk of every screen (Left 4) is not done. Every commit passed fmt, clippy (game; sim too for 7a88c87) and the game's touched modules' tests; 7a88c87 also the whole sim suite. STATE-3 runs beside: it has sim files open (`estimate.rs` among them) and `stage_bar/icons.rs`, `staging/parts.rs`.

**Landed:**
- c18b1a5: ONEKIND-7's (d2) second half: `scene/ships.rs`' plain model, the test craft's colours and the flame of no engine; the RCS menu's `authority` is the craft's own (`Authority`, "no thruster blocks" without any); `FlightData::parts` gone (every page links into the systems screen); the systems screen's one-part message; stale comments.
- 6e805fd: finding 42, ignitions left one rule (`systems_screen::inspect::ignitions_left`) for the screen and the card's two places: "unlimited (hypergolic)" on the pad too.
- fadf710: finding 43, `map_view::label_at`: an apsis label in the first corner clear of every icon.
- 0126067: brief-state.md notes the sim addition.
- 7a88c87: finding 41, `sim::systems::estimate::aboard` (the mass now to the sensors' precision and the dry mass, the pad's mount left out; a burn's start mass is it, tested equal), read by the systems screen's summaries.
- 0a280e2: finding 44, a tank's contents through one `format::amount` (moved from the staging bar's rows) in the inspector and the Plumbing summary too.
- 3d68e5a: finding 45, LOCAL TWR to the mass's precision (`menus::data::local_twr`, `stage_bar::aboard_of`), both TWRs through `format::ratio`; MCP's surface TWR the same.
- 3ca95de: finding 46, the menus' unused lumped Δv/TWR/mass and `hud::{vehicle_mass, delta_v, twr}` deleted.
- 81ec890: the lead's check after the typed handles: a table test holds the systems screen's LOST group and the unpowered list to the owners' ids (they read right).
- the docs commit after them: interface-map 41–48, systems-screen.md, flight-menus.md, owners.md, the plan's row, this State.

**Left, in order:**
1. Findings 47 (a battery's charge, four renderings: `format::amount`'s rule in Wh with the share for the Power menu, the card's battery and group, the inspector) and 48 (a bottle's and an RCS feed's contents to the precision the pressure gives, through `format::amount`; `RcsData`'s `Feed { kg, known }` becomes a reading).
2. The centre of mass (Structural) is a dash while a tank is unmeasured and counts the pad's mount on the pad: give it a precision from the same bounds (a sim function beside `estimate::aboard`, the mount left out).
3. The walk of every screen at HEAD (pad, ascent, orbit, after separation, the systems screen, the map, tracking, pause, settings, help), shots in the scratchpad: the systems screen's mass rows at T+60 s ("391 t"), Landing ▸ BRAKING's LOCAL TWR during a descent, the map's Pe label just past periapsis, the card's "ignitions left" on the pad (Dracos "unlimited (hypergolic)", Merlins "N (the pad lights it)").

**Open for the owner (INTERFACE-6):**
- The craft's mass now reads to the precision the tank sensors give (each of Dragon's unmeasured tanks anywhere from empty to full: "391 t", not a dash), as the staging bar did; the pad's launch mount is not counted in the craft's mass or dry mass.
- A hypergolic engine's ignitions read "unlimited (hypergolic)" on the pad too; a limited engine's count stands, with "(the pad lights it)" while clamped (the pad's supply lights every engine's start then, `ShipState::apply`).

**INTERFACE-5 (2026-10-31), wrapped up at its context cap.** Did INTERFACE-4's Left 1–4 in the brief's order of value: walked every screen again at HEAD (findings 32–43 in [interface-map.md](../technical/interface-map.md)), fixed what disagreed, then the first half of ONEKIND-7's (d2). Every commit passed fmt, clippy (sim and game) and the game's whole suite (485; 480 after 5d87517 deleted the test craft's tests); the g-load commit also the whole sim suite.

**Landed:**
- ff0409a: MCP, a shot sent while another is in flight waits its turn, the calls behind it too (`session::starting`), the clock held between them. Finding 38 (INTERFACE-4's Left 4).
- 8dad37a: **the g-load had three readings.** The navball's G under thrust used the coast's drag area (averaged over every direction): 0.42 g at max-Q for the trajectory's 2.03 g. The systems screen's g-limit read the last live tick's acceleration through coasts (4.4 g / 7.5 g in orbit against 0.00 g) and said "not accelerating" on the pad (1.00 g). One owner, `navball::proper_accel` / `felt`; `sim::systems::g_limit(def, state, a)` takes the acceleration (a sim API change, committed before STATE-2's step 2d at the lead's request); `Vessel::thrust_accel` deleted. Finding 32.
- 3f7b720: a load starts the menus afresh (`SimState::flight`): the landing prediction of one flight (`LandingPrediction::of`), the snapshot refreshed at once, the hold restarted (`menus::view::Held`). Finding 33.
- a07162a: the Electrical summary names by id (`power::sections_by_id`; INTERFACE-4's Left 2); the pad's buses before their first reading, "— (not read yet)", not "no buses". Findings 34, 35.
- ee125a5: the data layer counts the craft's parts, not the pad's (3 of 32). Finding 36.
- ab5609f: MCP `shoot` puts the view's size back. Finding 37.
- 1da4e29: the pause menu's "Saves" without the quicksave key; the Saves window names both quick keys (`saves::quick_keys`). Finding 39.
- 5d87517, ONEKIND-7's (d2) first half, finding 40: the test craft's stage stack, `StageInputs`, `activate_next`, `Activation`, `Refusal`, `grounded`, `stack_of` and `rows::plain` with their tests gone; `GameCommand::NextStage` gone (Space and the bar send `Stage` of the active vessel); Landing ▸ PARACHUTE lists the craft's own canopies (`staging::canopies`, `FlightData::canopies`). (STATE-2's 3fc9a0a carried this round's owners.md rows, written for it.)
- the docs commit after them: interface-map findings 32–43, this State, the plan's row.

**Shots for the lead** (`/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface5/shots/`): `f_g_compare.png` (after: the navball and the systems screen at T+63 s, 2.02 g and 2.0 g / 4.9 g; in orbit 0.00 g and "none: not accelerating"; the Landing menu after a load with only "no impact predicted"); before: `b_asc2_nav.png` with `b_asc2_struct_crop.png` (0.42 g against 2.0 g), `c_orbit.png` (the ascent's impact over "no impact predicted", 0.00 g), `c_sys_structural.png` (4.4 g in orbit). The pad after: `f_pad_power_c.png`, `f_pad_elec_c.png`, `f_pad_data_c.png`, `f_pad_struct_c.png`. The walk: `a_*.png` (pad), `b_*.png` (T+60 s), `c_*.png` (orbit), `g_sep.png`, `g_map.png`.

**Left, in order:**
1. ONEKIND-7's (d2) second half: the `None` cases of `menus::{power, crew, rcs_data}::gather` and `rcs.rs`' "ideal torque" note with its test; `FlightData::parts` (always true for a vessel now; `menus/mod.rs:507`'s test says "the test craft's do not"); `scene/ships.rs`' plain model and colours; `systems_screen/ui.rs`' "one part" message; the comments (`action_bar.rs:8`, `comms.rs:53`, `tracking.rs:597`, `state.rs:~519`).
2. Finding 41: the mass now with its precision as one rule (the sim's estimate's owner), read by the systems screen and the bar. For STATE or the estimate's owner.
3. Finding 34's model side, for STATE: a power step of no length at a systems lattice's first point, so the buses' readout exists from the start (the interface's dashes then last a frame).
4. Small: finding 42 (hypergolic engines' ignitions on the pad, one rule for the card's two places), the Landing ▸ PARACHUTE canopies on screen during a descent (tested, not shot).

**Open for the owner (INTERFACE-5):**
- The G row and the g-limit read the felt acceleration of the last live tick while the vessel flies live; on a coast the trajectory's own drag, on the ground the ground's push. The g-limit then holds only while the craft accelerates (D111's rule), so in orbit it reads "none: not accelerating" rather than the last burn's.
- After a load or a switch of vessel the menus drop what they held of the last flight at once (the anti-flicker hold applies within a flight).
- Before the first power step the Power menu lists the craft's buses with "— (not read yet)" rather than a voltage.
- The Saves button carries no key; the window says which keys quicksave and quickload.

**INTERFACE-4 (2026-10-31), wrapped up at its context cap.** Did INTERFACE-3's Left, the red cone, gpu_trace.py; found and fixed three more on the way. Every commit passed fmt, clippy and the game's suite (478) in a private mirror of HEAD plus my files.

**Landed:**
- a252a77: `tools/gpu_trace.py` reads the one `*.trace` in its directory (it looked for the plunge demo's) and its notes record the headless game (`SUNSCATTER_DEV=1`, `--time-limit`) with the `bench` tool's rows. HANDOFF's and brief-fixes3's "gpu_trace.py launches the deleted plunge demo" lines can go (the lead's files).
- 0172400: **the red cone was the sky, not a part.** From outside the atmosphere every geometry pixel went through `render_sky.wgsl`'s `ground_from_space`; for a craft above the air seen on a ray rising more than ~15°, its table branch read the transmittance back down through the planet: the craft dark red above a straight line across the view (it moved with the camera; shadows off changed nothing). Geometry the ray meets before the air now takes no aerial perspective (`sky::haze::air_before`, the shader's rule mirrored for a table test). Finding 27.
- e2fbf20: a shot taken while the benchmark ran froze the clock for good (`get_state`: warp 100×; `profile`: warp achieved 0): each saved and restored the hold. Held now while any shot or benchmark is in flight (`session::clock_held`). Finding 28.
- 25a149d: notifications after a load. A save's vessel ids repeat, so the per-id memory of what was notified replayed a loaded orbit's 16 events at once, and after loading an earlier save or reverting to launch hid every new event dated before the old timeline's last (no Ignition/Liftoff/MECO after a revert from orbit). `SimState::restores`, `events::to_notify` (step table test). Seen live: 0 replayed after a load. Finding 29.
- da12434: `get_vessel` under thrust (PLANNER-4's item): `orbit.prediction_age_s` while the active vessel flies live, and `orbit.now_two_body` (apsides from the centre of the state now). Live mid-burn: Ap 441.9 km with age 0.14 s (≈8 m/s at 60 m/s²), then 456.5 km with time stopped; the two-body values steady. Finding 30.
- b6d715b: MCP refuses an argument its tool's schema does not list, naming the tool's own (`mcp::protocol::unknown_arguments`): `set_camera {distance, pitch}` had answered as if it moved. Every call in the docs fits. Note: `step {id}` is now refused (step never listed `id`; `run_until` does). Finding 31.
- 7a2605d: docs: interface-map findings 27–31, mcp.md, owners.md (INTERFACE-3's rows and mine).

**INTERFACE-3's four commits seen at HEAD** (shots in `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface4/shots/`): `k_landing.png` (Landing ▸ Thermal: S2.TANK 332/422 K, DRAGON.CAPSULE 397/2000 K, NOT MEASURED 10 parts), `k_card_crop.png` and `k_card_top.png` (the capsule's card: skin 397/2000 K, interior 291/1650 K; the pressure's value below its label), `k_ts.png` (the tracking list's and Settings' bars at the panels' edges), `l_burn_c.png` (a planned 200 m/s burn: SAS ON, MNV lit, THR 100 %, Maneuver "burning").

**Shots for the lead:** the red craft before, `h_y0_zoom.png` (end-on: the bell and the tank's end seen through the dark red above the line) and `e_grid.png` (the stack at 20, 30 and 60 m, the line through it); after, `j_grid.png` (pitch −5°, −15°, −40° and end-on: no red) and `j_ground.png` (the ground's haze unchanged).

**Left, in order:**
1. **ONEKIND-7's (d2)** (brief-onekind.md, its State's Left 1), the game's branches for a craft without a ship definition. What I found: dead are `staging::{stack, StageInputs, canopy_part, activation, activate_next, Activation, grounded}`, `Refusal::Grounded` and their tests (keep `next`, `arms_chutes`, `no_stage_left`, `Refusal::NoStageLeft`, `parts`); `stack_of` is `parts::stack` (agent.rs `stages_json` already returns early without a ship); `commands.rs`' `GameCommand::NextStage` arm (always converted to `Stage(id)` now: make the conversion unconditional); `FlightData::stages` only feeds Landing ▸ PARACHUTE's test-craft rows (`landing.rs` `part_tone`, its test at ~353; `data.rs:328`, its test at ~614). Consider giving Landing ▸ PARACHUTE the craft's own canopies' states instead (`part_card::content::chutes`). `stage_bar/mod.rs:170`'s `None => rows::plain(…)` and `rows::plain` go with `rows_tests`' test-craft test: **STATE holds `stage_bar/rows_tests.rs`, `icons.rs`, `part_card/content.rs`, `systems_screen/{inspect, summary}.rs`** uncommitted (check `git status`). The rest of ONEKIND-7's list (rcs.rs' note, the `None` cases of power/crew/rcs_data, ships.rs' plain model, ui.rs' "one part", the comments) I did not start.
2. Walk the screens again (the brief's aim): pad, ascent, orbit, after separation, the systems screen, map, tracking. Seen so far this round and fine: the tracking station (Earth is a speck at the Moon's-orbit zoom), Settings, the capsule's card. Not looked at: the systems screen's layers at HEAD after ONEKIND's deletions.
3. Small, from INTERFACE-3: the systems screen's electrical summary names buses in capitals (S2.BUS) where the other layers use ids.
4. Concurrent shots still share one `Restore` of the screens (a second shot started during the first saves the first's open screens and puts them back after): only the clock's hold was fixed (e2fbf20).

**Open for the owner (INTERFACE-4):**
- Notifications stay the session's log across a load (the load's own message after the old flight's); only the event memory restarts. The other choice is to empty the list on a load.
- `get_vessel` gives both the predicted apsides (with their age) and the two-body ones now, rather than computing a fresh prediction on the call.
- MCP refuses unknown argument names (strict), so a key a tool reads but does not list (`step`'s `id`) is refused too.

**INTERFACE-3 (2026-10-31), wrapped up at its context cap.** Continued INTERFACE-2's "Left" (below), then walked the screens again in orbit (findings 21–26 in [interface-map.md](../technical/interface-map.md)) and took two items from PLANNER-4.

**Landed:**
- 12d7faa: Settings, Saves and the help start below the PAUSED banner (it lay over Settings' title; `layout::page_place_in`, `help_place_in`); every scroll area shows a thin bar while its list is cut (`theme::scroll`: the Orbital's TARGET picker under Landing gave no sign it was there). Findings 21, 22.
- a3a4336: MCP `set_target` (the picker's own choices, `navball::target_choices`; `get_state` reads it), `edit_stages` (the bar's rules, now pure: `stage_bar::rows::{editable, runs_again, check}`) with the bar's rows and figures in `get_vessel`'s `stages.sequence` (`stage_bar::{estimates_of, rows_of}`, `air`), `notifications` (numbered, `Toasts::since`). `command` lists and runs only the functions the piece offers (`program::view`, `offered`; it listed the booster's grid fins in orbit). `shoot {screen: saves}`; the RCS menu with RCS off is refused. Findings 10 (closed), 23, 24.
- 8936e94: lists that shrank to their rows put their bar mid-panel (notifications, Settings' bodies, tracking list, stage editor): full width now; a menu heading too wide wraps. Finding 26.
- 50a08d5: the part card's temperature was the skin's mean against the interior's rating (capsule: 344 K / 1650 K against the screen's 397 K / 2000 K skin): it shows the systems screen's skin and interior rows, one owner (`systems_screen::rules::part_temperatures`, `vessel_temperatures`, a single pass); `rules::nearest_limits` (the measured parts ranked, the rest counted). Finding 25.
- 4d4153e: Landing ▸ Thermal by that rule (`menus::hottest`): only measured parts ranked, NOT MEASURED n parts; the INTERIOR row (truth) gone. Finding 13. (`hottest.rs` still returns nothing for a craft without parts, ONEKIND's test craft.)
- b246f9d: the navball draws the controls flown (`maneuver::Program::flown`, `Vessel::flown_controls`): during a programmed burn its buttons showed the pilot's STAB and 0 % (PLANNER-4's item 1).

**Live MCP on the Falcon 9 in orbit** (orbit save, a3a4336): `command {}` (all items, the functions now only Parachutes and Nosecone on Dragon alone; `Grid fins` refused "acts on nothing aboard"); `get_vessel` gives the bar's rows (0 Dragon separation next, 1 SECO active, 2 and 3 done); `stage` then `set_target {vessel: 11}` (the second stage, shown in Orbital as TARGET · Vessel 11, 13 m, 0.4 m/s); `notifications {after: 9}` gave "Dragon separation" and the new vessel.

**Shots for the lead** (scratchpad `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface3/shots/`):
- INTERFACE-2's fixes at HEAD: `h_help.png` (16), `h_tracking.png` (14, TIME STOPPED), `h_cards.png` (15, the capsule's long label), `h_mvac_struct.png` (11, the MVac's joint dashes).
- 21: `h_settings_orbits.png` (PAUSED over the title) → `f_settings.png`, `f_saves.png`.
- 22: `h_menus_ol.png` (Orbital cut at TRUE ANOMALY, no sign) → `f_menus_ol.png` (bars).
- 13 before: `h_menus_ol.png` (Landing ▸ Thermal all dashes) against `f_sys_thermal.png` (the summary's measured s2.tank and capsule); 25 before: `h_cards.png` against `z_capsule.png`.
- MCP's target in the Orbital menu: `z_target.png`.
- Not yet shot: 8936e94, 50a08d5, 4d4153e and b246f9d on screen (tested only).

**Left, in order:**
1. PLANNER-4's item 2: `get_vessel`'s apsides lag while thrusting (they come from the live prediction, refreshed every 0.25 s of real time, so a `step` at speed reads seconds old: 343.9 km against 360.0 mid-burn). Proposed: under thrust, give the prediction's age and the two-body apsides of the state now (from centre, as the Orbital menu's ORBIT NOW), or compute the prediction on the call. `agent.rs` is ONEKIND-7's until its step (b) lands.
2. Shots of the four commits above: the Landing menu in orbit (measured S2.TANK and DRAGON.CAPSULE, NOT MEASURED 10), the capsule's card (skin, interior), the bars at the panels' edges, the navball during a planned burn (MNV lit, the throttle at the program's).
3. Small: the systems screen's electrical summary names buses in capitals (S2.BUS, from the Power menu's rows) where every other layer uses the ids (s2.tank). One shot of a single menu (`menus: ["orbital"]`) showed no menu at all and the next identical one did; not reproduced.
4. `owners.md` rows (held by another agent): `navball::target_choices`; `stage_bar::rows::{editable, runs_again, check}` and `stage_bar::{estimates_of, rows_of}`; `toasts::Toasts::since`; `systems_screen::rules::{nearest_by_part, nearer, nearest_limits, part_temperatures, vessel_temperatures}` (and `menus::hottest` on them); `maneuver::Program::flown`, `Vessel::flown_controls`; `layout::{page_place_in, help_place_in, shown_rect}`, `theme::scroll`; MCP `set_target`, `edit_stages`, `notifications`.

**Open for the owner (INTERFACE-3):**
- A scroll bar shows (thin) whenever a list is cut, not only on hover: an access cue under D123's interface exception, the look otherwise unchanged.
- The part card shows a part's skin and interior against their own limits (as the systems screen), no longer "(rated … K)"; the rating stays in the systems screen's RATINGS.
- Landing ▸ Thermal ranks only measured parts and says how many are not measured; its INTERIOR row went (each part's row includes its interior).
- `edit_stages` names stages by the program's index (`stage` in `get_vessel`), not the bar's countdown number, which changes as stages run.
- `command` refuses a function that acts on nothing aboard the piece, as the action bar hides it.

**INTERFACE-2 (2026-10-31), wrapped up at its context cap.** Continued INTERFACE-1's "Left" (below).

**Landed:**
- 0d43d75: a joint the craft gives no capacity (the MVac's mount, the heat shield's, a leg) reads dashes for its allowable, axial capacity and load, not 0 (`sim::systems::structure::rated`, one owner for the runtime and the screen; the summary counts them under "no capacity"); the condition's peak row says "peak interior"; the unused `SELECTED` tint removed. Finding 11, Left 3 and 5.
- ffc6b0a: Settings and Saves open centred under the menu bar and stop above the navball cluster (`layout::page_place`), the pause menu stepping aside meanwhile (`pause::menu_shown`); the help's window is solid (`theme::SOLID`), so the navball's numbers no longer show through it; Saves' empty-list hint takes its key from the key table, and its list scrolls. Findings 12, 16, 17.
- a77f74c: the MCP `command` tool (a function by name; an item's command by its label; `command {}` and `command {item}` list them), from the part card's list (`part_card::content::part_commands`, now the one list) and the systems screen's (`inspect::commands`); `set_controls` takes `rcs` and `rcs_fine`. Finding 10.
- 25f0bc4: the tracking station says TIME STOPPED, not the requested warp (`hud::time_words`, one owner with the time panel); a card's value that does not fit beside its label goes below it (`part_card::place::value_below`). Findings 14, 15.
- the docs commit after them: `interface-map.md` (findings 10–12 fixed, new findings 13–20), `mcp.md`, `owners.md` rows, this State, the plan's row.

**The flight (debug off, testing.md exactly):** max-Q at T+66 s, 34.6 kPa at 12.1 km; MECO at T+138 s, 65.3 km, pitch 32°: the worked example's numbers to the digit (the earlier fall-back was debug mode's undrained propellant). Insertion by a script of the doc's step 9 (`insert.py` in the scratchpad: the pitch that brings the vertical speed to zero at 200 km, 1 s pulses about up × heading): throttle cut at Pe 188 km, then 192 × 343 km at 28.7° after SECO (finding 20). A replay from the second-stage save matched round for round. Saves `ascent-t60`, `s2-ignition`, `orbit` in the scratchpad's `home/saves` (the build of 769b85d).

**Shots for the lead** (scratchpad `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface2/shots/`):
- the thermal mesh in orbit: `orbit_truth_dragon.trunk.png` (the trunk isolated, debug: its sunlit side green against the shaded side's teal), `orbit_thermal_truth.png` (the craft, debug, no cut), `orbit_thermal_sensors.png` (what the sensors give: the measured tanks coloured, the rest the neutral grid, the nodes behind the half cut);
- the MVac's bell burning: `s2burn_mvac_truth.png` (T+~330 s, debug: the radiatively cooled extension 1918 K of 2620 K, the regeneratively cooled throat blue; the right panel still shows finding 11's zeros, fixed since); `meco_thermal.png` (sensors, at MECO);
- before the fixes: `orbit_settings_orbits.png` (12), `orbit_help.png` (16), `orbit_cards.png` (15), `orbit_tracking.png` (14), `orbit_menus2.png` (13, Landing ▸ Thermal all dashes).

**Not yet seen on screen:** the fixes since 0d43d75 are tested (table tests, the game's whole suite, clippy) but not shot. The next agent should shoot them at HEAD: Settings and Saves in their place, the help over the navball, the tracking station paused, the capsule's card in orbit, the MVac's joint dashes, and a live `command {}` and `command {function: "Launch abort"}` refusal or run on the Falcon 9.

**Left, in order:**
1. The shots above.
2. Finding 13: Landing ▸ Thermal by the systems screen's rule (the measured parts nearest their limits, the rest counted; `menus::hottest` picks by the truth today). It needs `menus/data.rs` (PLANNER-4's hunk) and `hottest.rs` without its test-craft branch (ONEKIND-5).
3. MCP's last gaps: the target (Orbital → TARGET), the staging bar's edits, reading the notifications.
4. For other owners: finding 19 (the Crew Dragon has no cabin sensors), finding 20 (thrust after `throttle: 0` on the MVac, its 64 % floor), finding 18's note (a part's peak interior not kept through a warp-6 coast).

**Open for the owner (INTERFACE-2):**
- The help's window is opaque; every other panel stays translucent (the look is frozen, D123; this is the one change, for reading).
- Settings and Saves take the centre and the pause menu steps aside while one is open (Esc or × brings it back), rather than sitting beside it.
- `command` takes the labels the player sees (any case) and lists only what an item can do now (IGNITE when it is off, SHUT DOWN when it runs).

**INTERFACE-1 (2026-10-31), wrapped up at its context cap.**

**Landed:**
- def10d2: MCP `shoot` opens the help overlay (`screen: help`) and part cards (`cards`).
- 1183eb1: the staging bar's part cards stack in columns (`place::stack`), so none is pushed over another.
- 723ceec: the cleanup, from the map. The keys' labels come from the bindings (`keys::label`, Cmd on a Mac, Ctrl+Alt elsewhere). The help is generated from the bindings, the held keys, `MenuId`, Esc's levels and the ways-in table (`interface::access::WAYS`), and has a `?` on the menu bar. Esc backs out one level everywhere (`pause::on_escape`, `ESC_ORDER`: it now closes the tracking station and a body's menu instead of pausing over them). One Saves button. PAUSED sits under the menu bar. Settings no longer lists the menus. The tracking station's F7 label is gone.
- 85bb56e: the systems screen. The thermal layer draws the thermal network: each skin cell by its own temperature with the cells' grid, and the interior nodes as cubes behind a longitudinal cut-away (a slider, half by default). The meshes come from the sim's own surface, cells and volume grid, and read one texel per cell and node. A click isolates a part: the rest are ghosted and the camera eases to frame it. Esc or × puts the layer view back as it was. The framing fits the view both ways (it cut the nose before). `shoot` takes `cut`.
- the docs commit after them: `interface-map.md` (new), `systems-screen.md`, `flight-menus.md`, `systems/game.md`, `owners.md` rows, this State.

**The map:** [interface-map.md](../technical/interface-map.md): every screen, piece of information and control with its key, click path and MCP tool, and 12 findings (9 fixed).

**Shots for the lead** (scratchpad `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/interface/`):
- before: `shots/pad_sys_thermal.png` (whole parts by their hottest), `shots/pad_sys_selected_before.png` (the MVac hidden inside the interstage), `shots/pad_cards.png` (overlapping cards), `shots/pad_pause.png` (PAUSED over the bar).
- after: `shots2/c_t_craft_cut0.png` and `shots2/c_t_craft_cut50.png` (the thermal mesh: skin cells, then the nodes behind the cut), `shots3/c_pad_merlin_isolated.png` and `shots3/pad_restored.png` (isolated, then the layer view put back), `shots3/c_pad_mvac_thermal.png` (the MVac's nodes: it has no skin cells inside the interstage), `shots3/c_asc2_thermal_truth.png` and `shots3/c_asc2_merlin_truth.png` (ascent, T+109 s, debug), `shots3/pad_help.png`, `shots3/c_pause.png`, `shots3/pad_tracking.png`, `shots3/pad_cards.png`.

**Found while flying (not the interface):**
- The worked example's ascent (`testing.md`) no longer reaches orbit. After the same pitch kick (86.2°), the surface-prograde hold turned the stack flat: pitch 9° at 19.9 km at T+109 s, and it fell back to 1.8 km diving by T+232 s. `run_until {max_q}` never reported max-Q (it stopped on its 90 s bound twice).
- With debug mode on, the propellant never drains, so the stack keeps its full mass and the gravity turn collapses: fly with debug off, and turn it on only for paused shots.
- The Merlins' bells read 342 K / 1330 K after 112 s of burning (regeneratively cooled, so plausible); the MVac's radiatively cooled extension was not reached.
- On the pad, the sunlit and shaded sides do not differ after 30 minutes (the air holds the skin).

**Left, in order:**
1. The thermal mesh in orbit (a sunlit side against a shaded one) and the MVac's bell burning: needs a flight that reaches orbit (see above), or a save of one.
2. An MCP `command` tool in `game::agent` (a function by name, a part's command, RCS's mode): the map's finding 10. `agent.rs` is PLANNER-3's until its `set_plan` lands.
3. Finding 11: a joint's allowable line load and axial capacity read 0 where the craft gives none (a dash, D106).
4. Finding 12: the Settings window opens over the action and staging bars.
5. The rules' `SELECTED` tint is no longer used (a selection is an isolated part); remove it with its tests.
6. ONEKIND-4: the systems screen's tests here use `test_rocket`, not the test craft.

**Open for the owner:**
- The thermal layer's cut-away is longitudinal (the plane holds the craft's long axis and faces the camera) and cuts half the craft by default, so the interior nodes show at once. A part with no skin cells of its own (an engine inside its interstage) shows its nodes uncut.
- Colours under D106: a part a temperature sensor measures is coloured, the rest a neutral grey grid; debug mode colours everything. Each cell and node is coloured against its own limit (the existing ramp), so 290 K and 340 K look alike on a 1330 K bell.
- Selecting a part always isolates it (the view, PARTS, a summary row, a card's "more ›", a menu's link). A click on nothing changes nothing; a click on a ghost isolates that part, keeping the first layer view to go back to. Closing the screen puts the layer view back.
- The menu bar gained a `?` (the help), the only new thing on the flight view (D080).
- Esc closes the tracking station rather than pausing over it.
