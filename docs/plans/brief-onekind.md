# Brief: ONEKIND, one kind of craft (2026-10-30)

D121: "The test craft: delete it entirely". Every craft is built from parts (D100), and the made-up path (one placeholder part with a fuel number) goes. Follow [agent-rules.md](agent-rules.md). Your name is ONEKIND and your target dir is `target/agents/onekind`: start warm with `mv target/agents/flightsw target/agents/onekind`. Two other agents run at the same time:
- **MISSION** ([brief-mission.md](brief-mission.md)) replaces the scripted demo and the tour with MCP tools. Leave `crates/game/src/demo*.rs` and the tour to it: they are being deleted. If they stop compiling because of you, tell the lead in your State and wait for MISSION's deletion.
- **DOCS** ([brief-docs.md](brief-docs.md)) is restructuring `docs/`. Edit docs only where CI needs it (citations of a deleted decision). List every other doc change your work needs in your State, and DOCS or the lead applies it.

## ONEKIND-3 (2026-10-30, night)

You continue from this brief's State ("Left, in order"). Your name is ONEKIND-3; your target dir is `target/agents/onekind3`: start warm with `mv target/agents/onekind2 target/agents/onekind3`. MISSION and DOCS have finished. Two other agents run now:
- **PLANNER** ([brief-planner.md](brief-planner.md)) builds closed-loop planned burns for craft of parts and moves the planned-burn tests (`planned_burns`, `hold_modes`' maneuver holds, the game's planner and predictions, `landing::model`). Leave those to it.
- **AUDIT** ([brief-audit.md](brief-audit.md)) reads the code for D128 and D129, and writes only new docs and test files.

The order:
1. Left 1, the tests that are not blocked, and Left 2, the stress craft. Then report to the lead and wait for PLANNER to land before Left 3 and 4.
2. Then Left 3 (delete the made-up path and the test craft) and Left 4 (the decisions and their citations).

The owner's answers tonight ([interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md)):
- **The playtest save:** Dragon alone in orbit. Fly the Falcon 9 to orbit headless through MCP (`docs/technical/testing.md`, `tools/mcp.sh`), separate, then `save_game`. `./run play` loads it. `save_load` builds its own save in code, so a format change never breaks it. Make as many debug saves as you need. A save holds no static data (D063, extended tonight: body data and part behaviour come from the game's files); trajectories and thermal state stay in it.
- **The stress craft:** real catalog generators by judgement. Don't chase a count of arbitrary cubes: "we don't even know what those crafts will look like" (D121, extended tonight). It has to be a plausible many-part craft that exercises the real path.
- **Small item:** replace `GameCommand::SpawnTestShips` with a developer MCP tool that spawns n craft of parts, for the 11-vessel performance check (the hand-off's small items).
- The small rocket's numbers (E) stand unless the owner says otherwise.

## ONEKIND-4 (2026-10-31)

You continue from ONEKIND-3's State. Your name is ONEKIND-4; your target dir is `target/agents/onekind4`: start warm with `mv target/agents/onekind3 target/agents/onekind4`. Running now:
- **PLANNER-3** ([brief-planner.md](brief-planner.md)) moves the game's planner, `predict_tests` and `landing::model` onto nodes, and MCP's `set_plan` (`game::agent`). It says in its State when nothing needs the open-loop burn law any more.
- **INTERFACE** ([brief-interface.md](brief-interface.md)) works in `game::systems_screen` and `game::interface`. Before you edit a game test file there, check `git status` for its uncommitted changes; if it has some, do other work first.

The order:
1. ONEKIND-3's Left 1, what remains: the sim unit tests, the examples that name the test craft, `GameCommand::SpawnTestShips` replaced by a developer MCP tool that spawns n craft of parts, then the game tests.
2. Left 3 and 4 once PLANNER-3's State says the open-loop law is no longer needed: delete the made-up path, the test craft, the placeholder parts (`tests/parts.rs` moves to the stress craft of catalog parts or `craft::small`), D074's fin tests unless a finned craft of parts keeps them; then the decisions and their citations.
3. The playtest save, as ONEKIND-3's section says (Dragon alone in orbit).

Make your own doc changes (owners.md, the parts and vessel technical docs, the systems docs' test-craft mentions); there is no DOCS agent now.

## The work, in order

1. **Inventory** the made-up path in this brief's State: every place that tells the two kinds of craft apart (~270 references). Search for the scalar `Vessel::propellant` and `set_propellant`, `systems::Consumables`, `Aboard::Fill`, D078's ideal couples (`rcs_authority` for the test craft), the test craft's single-part pipeline, `design_of`'s ids, `SUNSCATTER_CRAFT=test-craft`, `data/craft/test-craft`, and the tests that fly the test craft.
2. **Tests first.** Every test that uses the test craft moves to the Falcon 9, or to a small craft of parts built in code (a helper in `craft`, one per shape needed: a tank and an engine, a capsule with chutes …). Each test keeps what it checks; record any that cannot be kept, and why.
3. **The stress generator** (`craft::stress`, for benchmarks and the scale target) builds n parts of real catalog types on the parts path (tanks, engines, structure, panels), so the benchmarks measure the real path at 300, 1,000 and 5,000 parts. Keep `bench_scale` and `bench_coast` running on it.
4. **Delete the made-up path:** one mass law, one attitude model, one thermal fill, one coast mass. Delete the test craft's data, its new-flight entry and its env value. The playtest save (`./run play`) is regenerated from a Falcon 9 flight, or dropped if MISSION's tools replace it: record which.
5. **Decisions:** delete D074. Trim D064 to debug mode only (D123: a dead computer still counts in debug). Update D078 and D104 where they speak of the test craft. Update every citation in the same commit (`tools/check_decision_refs.sh`).

Small commits, each with its tests passing (`cargo test -p sim`, the game's tests, clippy). Determinism, warp and chunked = single-pass tests stay green throughout.

## State

(Update as you land: commits with hashes, the inventory, what is left in order, doc changes needed.)

### 1. Inventory of the made-up path (2026-10-30)

Three kinds of craft exist today: the **test craft** (a craft file `craft.ron` + `geometry.ron`, built as one part by `CraftDef::whole`), **placeholder-part craft** (`ship: None` with several `Part`s: `craft::stress` and its pieces) and **craft of catalog parts** (`ship: Some(ShipDef)`: the Falcon 9, `build_tests::CRAFT`). Everything below tells the first two from the third.

- **Data and entry points:** `data/craft/test-craft/{craft.ron, geometry.ron}`; `craft::TEST_CRAFT`, `craft::test_craft()`, `load_craft`, `parse_craft`, `load_definition`'s geometry branch, `Craft::build`; `craft::file`'s single-part format (`CraftFile`'s `engine`, `propellant`, `attitude_control.rcs`, `chute`; `GeometryFile`'s `tank`; `validate_craft`, `validate_geometry`; `build.rs` still synthesises a `CraftFile` spec for a craft of parts, `ship_aggregate`); `registry::definition`'s `TEST_CRAFT` branch, `cell_options`' file case, `craft_of`'s `TEST_CRAFT` branch; `design::design_of`'s `TEST_CRAFT` branch. Game: `state.rs` (`CRAFTS` entry "Test craft (debug)", `startup_craft` defaulting to it in demo mode, `craft_or_test` falling back to it), `saves.rs:400`, `main.rs` (`SUNSCATTER_CRAFT`), `menus/hottest.rs:35` (names by id). The demo flies the test craft (MISSION's files).
- **The placeholder part pipeline:** `parts::Part`'s placeholder properties (`tank: PartTank`, `engine: EngineFile`, `rcs: RcsFile`, `chute`, `power: PartPower`, `control`, `crew`, `max_load`, `interior_specific_heat`, `node_max_k`), `CraftDef::whole`, `whole_power`, `parts::aggregate`, `parts::rcs`, `CraftDef::tanks`' `None` branch, `Craft::from_def`'s one-part (`MassModel::new`, one tank) and placeholder-parts branches, `stress::StressLayout::def` (`ship: None`), `split`'s made-up pieces (`fill × capacity`), `breakage.rs:87` (interiors only for made-up craft).
- **The scalar propellant (one mass law):** `Vessel::propellant` (field), `create.rs` (`initial_propellant`), `mass.rs` (`propellant()`, `mass_props()`, `aboard()` and `set_propellant`'s `None` branches; 17 callers of `set_propellant`, all tests or the game's debug paths), `mod.rs` (`engine_running`, `thrust_accel` fallbacks), `ship.rs` (`init_ship` zeroing it, `actuators`' `None` branch: `engine.tick_output` on the scalar, the RCS fed by it), `live.rs:577-584` (the tick's `mdot + rcs·rcs_flow + boil_off`), `coast.rs:303-405` (the segment's mass law: `segment_inertia`, `segment_propellant`, the final write-back), `split.rs:217` and `:326`. `systems::Consumables` (boil-off) and `Env::propellant`, `vessel/systems.rs` (`set_boil_off`), `limbo.rs:103`.
- **One thermal fill:** `design::Aboard::Fill` (and `node_capacity`'s fill), used by `mass.rs`, `live.rs`, `coast.rs`; `CraftDesign::propellant_specific_heat`, `tank_capacity`.
- **D078's ideal couples (one attitude model):** `RcsFile::{torque, flow}`, `CraftParams::{torque, rcs_flow}` (`parts::rcs` for made-up craft, `ShipDef::rcs_authority` for parts), `actuators`' `None` branch, `flown.rs:50` and the coast's hold (`CraftParams::torque`), `game` (`navball`, `menus/rcs.rs`, `rcs_data.rs`).
- **Planned burns:** `Vessel::set_plan` refuses a craft of parts (`PlanError::PartsUnsupported`); the burn segments' mass law (`trajectory.mass_at`), `landing::model` (`craft.mass.at(propellant)`, `craft.engine`), the game's planner and predictions run only on the made-up path. "Planned burns for craft of parts" is queued in HANDOFF; until it lands the planned-burn tests have no craft of parts to fly.
- **Tests and benchmarks naming the test craft** (66 files): sim unit tests in `aero/{tests,lift_tests}`, `chute/tests`, `craft/{broad,build,cells_tests,design,engine,mass,mod,parts,registry,volume}`, `forces/surface`, `landing/tests`, `proximity/{contact_tests,hull_tests}`, `thermal/{tests,heating_tests}`, `vessel/{bubble_tests,tests}`; sim integration tests `aerothermal, contact, craft_flight, frame_live_stop, hold_modes, landed_rest, limbo, limbo_physics, limbo_regime, parts, planned_burns, proper_time, save_load, scenario_launch, terrain, vessel_controls, vessel_dynamics`; examples `bench_{aero,bubble,fleet,integrator,limbo,scale,thermal}`, `long_run`; game `debris, fleet_tests, menus/{data,hottest,landing,rcs_data,rcs}, stage_bar/rows_tests, landing, navball, saves, staging, state, state_tests, trajectory/{apsides_tests,line_tests,lines_tests,predict_tests}`, `demo_views` (MISSION's). Plus tests on placeholder-part craft (`stress_tests`, `split_tests`, `mass_tests`, `bubble_tests`).

### Landed

- 3c57cde the inventory (above).
- ce026b1 `craft::small` (D121): the build's fixture rocket and its test entries moved out of `build_tests`; `small-rocket` (ids `small-<name>` resolve in the registry, so saves and pieces reload) is the fixture plus Apollo parachutes (`t-chutes`, D079's numbers) and a radiatively cooled test nozzle (niobium C-103, D077's 2.2e-3); `Vessel::set_fill` (every tank to a fraction). `tests/craft_flight.rs` on it.
- ce17b16 a real fix found on the way: the landed-pose check (`Checked`) is saved, so a save no longer wakes a landed craft of parts whose LOX boiled off while it stood (`proper_time::save_and_load_preserve_the_clock`).
- e38f245 `tests/{landed_rest, proper_time, terrain}.rs` on the small rocket. A fall onto the plateau now breaks the craft up there (parts' loads) instead of crashing it whole; the test checks either.
- fbd63c4 `tests/vessel_dynamics.rs` on the small rocket (masses set through `set_fill`, a breakup counts as a crash).
- ccf8430 the small rocket carries an IMU and a GPS receiver on its computer's bus and data link: without them its flight software (F3) cannot hold an attitude.
- 0107fa2 `tests/vessel_controls.rs` on the small rocket: the probe burns from before t0, and only the prograde hold travels with the delay.
- (ONEKIND-2) 09d44b8 a vessel of a configuration (`small-rocket@7-10`) starts with those mechanisms deployed (`init_ship`), so its first tick does not reconfigure it back; the small rocket's legs are 2.5 m (its feet 3.13 m out and below the bells); `craft::small::legs_out()`.
- 8a29ad3 `tests/frame_live_stop.rs`, `tests/scenario_launch.rs` (debug mode, engines relit for each burn) and `parts`' roughness check on the small rocket.
- e57e934 the small rocket gets four thruster blocks (torque about every axis: 1.9, 1.9 and 1.1 kN·m; the fixture's two gave none about Y), feet springs for its mass (3e5 N/m, 1.8e4 N·s/m, 0.35 m stroke: ~11 cm of sink on Earth) and a nozzle heat share sized by D077's rule for its own nozzle (1.7e-4: ~1400 K, not 2650 K); `CraftParams::bottom_z` is the lowest contact point (a deployed foot's tilted pad rim is lower than its base, so a craft on legs spawned in the air). Its bells cool over 90 s on the coast lattice (`craft_flight`), not 30 s.
- 3d79bff `tests/contact.rs` (13 tests, on `legs_out`) and `tests/aerothermal.rs` (13 of 14; the planned-burn one waits) on the small rocket.

- (ONEKIND-3) 1721504 `tests/{limbo, limbo_physics, limbo_regime, save_load}.rs` and `thermal/tests.rs` on craft of parts. The small rocket gets body-mounted solar cells on its bus (2 m², 28 %: a long coast keeps its computer); `craft::small` gets a nose cone entry (`t-nose`), a flat plate (`small-plate`, one 10 m fin: radiation pressure's textbook shape) and `rocket_ref()` (a `&'static Craft` for tests that keep borrows of its tables).
- (ONEKIND-3) be83b64 **the stress craft of catalog parts** (Left 2): `stress-<n>` is `stress::file(n)`, exactly n parts of `craft::small`'s test parts by `stress::parts_plan` (the capsule with the small rocket's avionics, cells and thrusters; a core stack of tanks, a thrust structure, four engines and four legs; rings of radial boosters, ring k of 4k, each a nose cone, tanks, a thrust structure and four engines on a separation joint to the core's top tank or the nearest booster of the ring inside; each stack's lowest tank feeds its engines; the sequence Ignition, Boosters, Separation). 300 parts: 2 core tanks, 40 boosters, 164 engines; 1,000: 3, 112, 452; 5,000: 10, 312, 1,252. `bench_scale` (its live cases start the engines through Ignition; the pad case stands, engines off), `bench_aero`, `bench_thermal`, `bench_integrator` and the game's `fleet_tests` build it (`stress::craft(n)`). Benchmark numbers below.

### How to move a test (for the next agent)

- `&sim::craft::small::rocket()` replaces `test_craft()` (it is an `Arc<Craft>`). It is about 13.9 t loaded, 1 crew, four 100 kN engines (Isp 320/280 s, throttle 0.4–1), cold-gas thrusters, four legs and Apollo chutes.
- Engines start only through the sequence: `assert_eq!(v.next_stage(&w).unwrap().0, "Ignition")`. A throttle below 0.4 shuts them down.
- Propellant: `set_fill(&w, f)` sets every tank's liquid. For a target mass, fill to 1 and to 0 and interpolate (`vessel_dynamics::above_the_cape`). `propellant()` includes the ullage and bottle gas, and LOX boils off on coasts and on the pad, so compare within ~1 kg.
- A hard impact breaks a craft of parts up (`v.breakup().is_some()`, the advance stops there) rather than `Phase::Crashed`; the impact speed is in `breakup().destroyed` (`vessel_dynamics::wrecked`).
- Contact height: take it from a landed vessel (`terrain::standing`), not from `CraftParams::contact_height(16_000.0)` (the made-up mass law).
- Standing on feet: `small::legs_out()` (the stowed rocket stands on its bells and has no feet). Feet 3.13 m out on X and Y, tip angle 29° empty and ~32° full with two feet downhill.
- Destroyed: `contact::still()` (landed, crashed or a breakup pending) and `contact::impact()` (the impact from `phase` or `breakup().destroyed`); `aerothermal::wrecked()`.
- A `&'static Craft` (tests holding its cells, bake or network): `craft::small::rocket_ref()`. Its skin is aluminium: take emissivities from the cells (`c.emissivity`), not 0.8.
- Tank contents for the thermal nodes: `Aboard::Tanks(plumbing::held(def, &ShipState::initial(def)))`, not `Aboard::Fill`.
- Long coasts: a full rocket boils ~9.5 t of LOX in a year (uninsulated, in sunlight), so it gets lighter and decays faster than the 20 t test craft did: limbo's year flies at 350 km, not 300.
- The shared tree may not compile while another agent works: test in a private mirror (`git archive HEAD` of `crates docs tools` plus your own files, `data/` symlinked, its own target dir).
- An exact mass that does not boil off on a coast: fill only the RP-1 tank (`set_tank` with the index of `tank:fuel` in `ship_def().tanks`), the LOX at 0.

### Tests not kept, and why

- `contact::a_steep_slope_tips_it_over_and_it_comes_to_rest_lying_down`: it still falls over, never frozen mid-fall, but may slide on down the 35° slope lying on its round hull. Its tip angle (29–32°) is above the hull's sliding angle (atan 0.5 = 26.6°), so no slope both tips it and holds it lying. The finned test craft could not roll.
- `contact::twelve_metres_per_second_on_the_hull_destroys_the_craft`: the whole wreck's bounce and rest are gone. A craft of parts breaks up at the impact, and its pieces are the game's split.
- `aerothermal::an_entry_…`: the destroyed cell is the one most over its limit, not the hottest cell (the niobium nozzle runs hotter, within its own limit).
- `frame_live_stop`: the Moon fall ends at rest or lying within its reach of the surface. A round hull on its side has no support polygon to rest on.

- `craft_flight::rcs_burns_propellant_at_its_flow_per_axis` (D078's couples drawing the main propellant: the made-up attitude model itself) and `powered_ticks_burn_propellant_at_the_engine_rate` (the scalar mass law): the parts path's versions are `vessel::ship_tests` (each tank drained by the mixture ratio) and `ship_systems_tests::the_first_stage_draws_its_engines_flow_and_nothing_else`.
- `vessel_dynamics::a_drogue_fired_far_above_its_design_q_tears_and_the_mains_still_land_a_light_craft` is now `…_tears_and_so_do_the_mains_behind_it`. With its drogue torn, the slender rocket reaches the mains' height near 190 m/s and they tear too. The finned test craft's broadside drag used to slow it enough for the mains alone. A mains-alone landing would need a draggier small craft.

### Left, in order

1. **Move the remaining tests** (ONEKIND-3 moved limbo, limbo_physics, limbo_regime's two automatic-reference tests, save_load and thermal/tests).
   - `tests/parts.rs` onto `stress-<n>` of catalog parts: it flies the placeholder layout, registered as `placeholder-<n>` in the test, because its 11 tests were written on that layout's part ids, kinds and numbers (split pieces `#12-16,32,36`, overheated and overloaded parts, a leg's hard landing).
   - sim unit tests: `chute/tests`, `craft/{broad,cells_tests,design,engine,mass,mod,parts,registry,volume}` (`cells_tests` also uses `stress::layout`), `forces/surface`, `landing/tests`, `proximity/{contact_tests,hull_tests}`, `thermal/heating_tests` (its nose test needs the capsule's flat top's nose radius), `vessel/{bubble_tests,tests}`. `aero/tests::the_test_craft_is_stable_nose_first_at_every_mach_number` and `aero/lift_tests`' two fin tests are the finned test craft's passive stability (D074): they go with D074 (Left 4) unless a finned craft of parts is wanted; `aero/tests::the_bell_shadows_the_base…` needs the small rocket's base geometry.
   - examples still naming the test craft: `bench_{aero,bubble,fleet,integrator,limbo,thermal}`, `long_run`.
   - game: `debris`, `fleet_tests` (its small craft), `menus/{data,hottest,landing,rcs_data,rcs}`, `stage_bar/rows_tests`, `landing`, `navball`, `saves`, `staging`, `state`, `state_tests`, `trajectory/{apsides,line,lines}_tests`; `GameCommand::SpawnTestShips` → a developer MCP tool spawning n craft of parts (owner, tonight).
   - **Left on the test craft for PLANNER or Left 3:** `limbo`'s two low-thrust planned burns, `limbo_regime`'s three planned-burn tests, `limbo_physics`' `a_low_thrust_spiral…` (planned) and its two made-up boil-off tests (`boil_off_ends…`, `a_flown_burn_takes_no_boil_off`: delete with the path, D119); `save_load::the_playtest_save_loads…` (goes when the playtest save is regenerated, Left 3); `planned_burns`, `hold_modes`' maneuver holds, the game's planner, `trajectory/predict_tests`, `landing::model` (PLANNER).
2. **Delete the made-up path** (the inventory above), with the placeholder stress layout (`stress::layout`, `StressLayout`, `StressCraft`, `stress_craft`, `volume_com` if unused) once `tests/parts.rs` and `cells_tests` are off it. Also in `build.rs`, the craft-of-parts build falls back to `test_craft()`'s skin and chute: it needs a default skin and "no chutes". Then: the test craft's data, its new-flight entry (`state.rs`), `SUNSCATTER_CRAFT=test-craft`, and the playtest save (Dragon alone in orbit from a headless Falcon 9 flight, owner tonight).
3. **Decisions:** delete D074; trim D064 to debug mode (D123); update D060, D078, D079 (the test craft's chutes), D100 ("the test craft stays as the debug craft") and D104; update every citation in the same commit.

### ONEKIND-4's State (2026-10-31, wrapped up at ~350k tokens)

**Landed** (pushed with INTERFACE's push; the whole sim suite passes on them):
- 716e2fd **the sim unit tests on the small rocket**: `chute`, `craft/{broad,cells_tests,engine,mass,parts,registry,volume,mesh}`, `forces/surface`, `proximity/{contact,hull}_tests`, `thermal/heating_tests`, `aero/tests`, `vessel/{tests,bubble_tests}` (the cells on `legs_out()`; the placeholder layout's users on `stress::def(n)`, the catalog stress craft's definition without cells). New: `Craft::loaded_props()` (mass properties as loaded, from the plumbing's contents). Each keeps what it checks; the numbers come from the rocket's geometry (the commit message lists them). Found on the way: the proximity rest test pushed A alone, so B was an inverted column on it (fall time √(3.8 m / 0.01 m/s²) ≈ 20 s); it now pushes both along their line of centres. The bubble tests' leaders light `engine#1` at 40 % (2.9 m/s²: they stay within the bubble's 10 km).
- 4ee9212 **the examples on the small rocket** (`bench_{fleet,bubble,aero,integrator,thermal,scale,limbo}`, `long_run`; all run in release, `long_run`'s save and reload give the same bits). `bench_thermal` fills the nodes from the tanks as loaded (`Aboard::Tanks`). The four 0.5 N ion spirals in `bench_limbo` and `long_run` are held prograde instead (the open-loop law on rails goes with the made-up craft; D029 brings low thrust back).

**Draft, out of the tree:** the game tests on the small rocket, as a patch: `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/onekind4/onekind4-game-tests.patch` (17 files; `git apply` it at HEAD). `SimState::new()` is the small rocket on its legs; `spawn_test_ships` spawns small rockets; `state_tests`, `fleet_tests` (a node through `set_nodes` replaces the open-loop burn), `debris`, `navball`, `saves`, `landing`, `menus/{crew,power,rcs_data,data,landing}`, `stage_bar/rows_tests`, `staging`, `trajectory/{apsides,line,lines}_tests`. In a mirror of HEAD it passed 446 of 447 game tests. The one left is in PLANNER-3's open `agent.rs` (line ~809): `stage` on the spawned probe is now a small rocket's Ignition, ordered after the light delay, not "not a craft of parts". PLANNER-3 has since committed `agent.rs` and `commands.rs` (8b5e8e9): apply the patch (check it still applies), fix that assertion, run the game suite, commit. It was taken out of the tree so PLANNER-3's own runs of that test do not fail meanwhile.

**Left, in order** (replaces "Left, in order" above):
1. Apply the patch, fix `agent.rs`'s assertion, commit. Then replace `GameCommand::SpawnTestShips` with a developer MCP tool (`agent/session.rs`'s `DEVELOPER` list beside `bench` and `profile`; `commands.rs`; the pause menu's "Spawn 10 test ships" button in `interface/pause.rs`) once those files are committed by their owners.
2. `tests/parts.rs` onto `stress-<n>` of catalog parts. Its 11 tests use `set_propellant` and the placeholder's part names and numbers: use `set_fill`, `stress::def`, and the catalog stress craft's part ids (print them first). Then delete the placeholder layout (`stress::layout`, `StressLayout`, `StressCraft`, `stress_craft`, its `stress_tests`; `volume_com` is still read by `bench_scale` and `stress_tests`).
3. Left 3, once PLANNER-3's State frees the open-loop law. These tests test the made-up machinery itself and stay on the test craft until then: `aero/tests::the_test_craft_is_stable…` and `lift_tests`' two fin tests (`finned_bake()`, D074); `craft/parts::the_test_craft_is_one_part…`; `craft/mod.rs`'s file-loading tests; `registry`'s and `design_of`'s test-craft branches; `volume::propellant_fills_from_the_tank_base` (`test_craft_grid`, `Aboard::Fill`); `mass`'s `MassModel::new` tests; in the game, `staging::stack`'s tests and `rows_tests`' test-craft stack, `hottest`'s names, `state.rs`'s new-flight entry and `craft_or_test`'s fallback (and their `state_tests`). `landing/tests` is PLANNER-3's (`landing::model`).
4. Left 4, the decisions, as above.
5. The playtest save after Left 3: the save format changes when `Vessel::propellant` goes.

**Found, not chased:** in `bench_limbo` the small rocket held prograde at 900 km for 30 days falls from 13.7 t to 7.1 t. That is 6.6 t of its 12.2 t of propellant in a month, against ONEKIND-3's ~9.5 t a year at 350 km. Worth a look at the coast's tank law (D119).

**Doc changes needed** (`docs/owners.md` has others' uncommitted changes): `owners.md`'s craft rows: `Craft::loaded_props`, `stress::def`; `docs/technical/limbo.md` and `scale-performance.md`: the benchmarks' limbo fleets no longer carry ion spirals. ONEKIND-3's list above still stands.

**Open for the owner** (defaults taken): the benchmarks' ion spirals are held prograde until D029; the bubble's one-part destruction test is now a breakup at the parts' impact limit (D066 per point), with nothing breaking in debug mode (D064).

### ONEKIND-5's State (2026-10-31, wrapped up at ~250k tokens)

**Landed** (each passed fmt, clippy and the sim suite or the game suite in a private mirror of HEAD plus my files; pushed):
- 051f52b **the game tests on the small rocket** (ONEKIND-4's patch, 18 files): `SimState::new()` is the small rocket on its legs, spawned test ships are small rockets. Two assertions fixed in `agent.rs`: `stage` on a spawned probe is its Ignition after the light delay; `set_plan` on a craft of parts sends `SetNodes` (D126). Game suite 462 passed.
- b055104 **the developer MCP tool `spawn_vessels`** (`SUNSCATTER_DEV=1`; `agent::session`'s `DEVELOPER` list): `count` (1–50, default 10) vessels of any craft id (default `small-rocket`) coasting in low orbit, every other one uncrewed; answers with their ids. `SimState::spawn_in_orbit` is the one owner (`spawn_test_ships` is its `#[cfg(test)]` helper); `spawn_args` a pure rule with a table test. `GameCommand::SpawnTestShips` and the pause menu's "Spawn 10 test ships" are gone. `mcp.md`, `systems/game.md`, `windows.md`, `scale-performance.md`'s verification (headless: `spawn_vessels`, then `profile` at warp levels 0, 6, 9).
- 84e9b41 **`tests/parts.rs` on `stress-12` and `stress-40` of catalog parts**, and **the placeholder layout deleted** (`stress::layout`, `StressLayout`, `StressCraft`, `stress_craft`, the builder and their tests; `cell_options`, `id`, `volume_com` stay for `bench_scale`). Each of the 11 tests keeps what it checks (the commit message lists how). The load tests: catalog parts tear at their joints (D103, `systems::structure`), not at `max_load` (INFINITY for catalog parts); at 10 km and 400 m/s nothing snaps (the worst ratio is 0.48), so they fly at 5 km and 600 m/s (two booster nose-to-tank joints at 1.9 and 1.2) and the root test at 3 km and 800 m/s. `stress_tests`' pipeline test runs on `stress-12` and `stress-40` (its general checks; the placeholder properties went).
- 9f9e4a1 docs: `limbo.md` and `scale-performance.md`: the benchmarks' four low-thrust craft are held prograde, `long_run` flies small rockets. `owners.md` (in 84e9b41): `stress::def`, `stress::craft`, `Craft::loaded_props`.

**Found, not chased:** with liquid aboard, a split of a tumbling craft of parts conserves angular momentum to ~2e-7 of m·1 m²/s (dry tanks: 1e-9). The liquids, slug drifts, positions, velocities and spin all agree to 1e-11, and `MassModel::with_slugs` is additive (dry + solid-cylinder slugs + point gases). So the gap is somewhere in the pieces' mass properties with liquid aboard. `parts::a_split_in_a_coast…` bounds it at 1e-6 with a comment; rerun it with `set_fill(&w, 0.0)` to see 1e-9.

**The test craft and the made-up path are not deleted yet.** Left, in order (replaces ONEKIND-4's Left 3–5):
1. **Delete the made-up path in one series** (PLANNER-3's State: nothing needs the open-loop law for a craft of parts). Suggested order, each commit green:
   a. The tests that test only the made-up machinery: `limbo`'s two low-thrust planned burns, `limbo_regime`'s three planned-burn tests, `limbo_physics`' `a_low_thrust_spiral…`, `boil_off_ends…`, `a_flown_burn_takes_no_boil_off` (D119), `aero/tests::the_test_craft_is_stable…` and `lift_tests`' two fin tests (D074, `finned_bake()`), `craft/parts::the_test_craft_is_one_part…`, `craft/mod.rs`'s file-loading tests, `volume::propellant_fills_from_the_tank_base`, `mass`'s `MassModel::new` tests, `landing/thrust_tests::the_made_up_engine…`, `save_load::the_playtest_save_loads…`; in the game `staging::stack`'s and `rows_tests`' test-craft stacks, `hottest`'s names, `state_tests`' new-flight entry and fallback, `steady_tests`.
   b. Planned burns of the made-up craft: `Vessel::set_plan`, `FlightPlan`, the burn segments (`vessel/burn.rs`, `trajectory.rs`'s burn branch, `mass_at`), `hold.rs`'s open-loop law, `BurnCraft::turn_lead`; in the game `GameCommand::SetPlan`, `planner::{draft_from, plan_from, draft_info, command_for}`'s made-up halves, `turn_warning`, `burn_time`; `landing::Thrust::of_engine` (then `LandingStart.mass` is not an `Option`). MCP `set_plan`'s description drops "For the made-up craft".
   c. The scalar mass law and attitude model: `Vessel::propellant` (the field), `set_propellant`, `create.rs`'s `initial_propellant`, `mass.rs`' `None` branches, `Aboard::Fill`, `systems::Consumables`, `ship.rs`'s `actuators` `None` branch, `live.rs:577-584`, `coast.rs`'s segment mass law, `split.rs`'s made-up branch (`cut.fill`), `breakage.rs`'s one-part rule and its "interiors only for made-up craft", `CraftParams::{torque, rcs_flow}`, `craft.engine` (game users: `navball/mod.rs:370`, `scene/ships.rs:548`, `menus/data.rs:425,506-521` (INTERFACE-2 holds `menus/data.rs` now), `menus/rcs.rs`, `rcs_data.rs`).
   d. The craft file's single-part format and the test craft: `craft::{TEST_CRAFT, test_craft, load_craft, parse_craft}`, `load_definition`'s geometry branch, `CraftDef::whole`, `whole_power`, `parts::aggregate`, `parts::rcs`, the placeholder properties of `parts::Part` (`tank`, `engine`, `rcs`, `chute`, `power`, `max_load` …), `registry`'s and `design_of`'s test-craft branches, `build.rs`'s fallback to `test_craft()`'s skin and chute (needs a default skin and "no chutes"), `data/craft/test-craft`, `state.rs`'s "Test craft (debug)" entry and `craft_or_test`, `main.rs`'s `SUNSCATTER_CRAFT=test-craft`, `saves.rs:400`, `pad.rs`, `action_bar.rs`.
2. **Decisions** with the last of 1, citations in the same commit (`tools/check_decision_refs.sh`): delete D074; trim D064 to debug mode (D123); update D060, D078 (no ideal couples left), D079 (the test craft's chutes: the small rocket's `t-chutes` carry D079's numbers), D100 ("the test craft stays as the debug craft") and D104 ("the test craft keeps D078's couples").
3. **The playtest save** (owner: Dragon alone in orbit): after 1c, as the save format changes when `Vessel::propellant` goes. Fly the Falcon 9 to orbit headless through MCP (`docs/technical/testing.md`, `tools/mcp.sh`), separate Dragon, `save_game`; `./run play` loads it. HANDOFF's gotcha "the playtest save still carries the test craft" goes then.

**Doc changes still needed** (the lead's files): HANDOFF.md:60 ("the 11-vessel performance check needs a spawn tool": it exists, `spawn_vessels`); `flight-ui-2026-10-27.md`'s MISSION row ("a spawn tool after ONEKIND": landed b055104). ONEKIND-3's list above (parts and vessel technical docs: `craft::small`, `set_fill`, the saved pose check, configurations deployed at start, `bottom_z`) is still open.

**Open for the owner** (defaults taken): the load tests fly faster and lower than before (5 km, 600 m/s) because catalog joints are much stronger than the placeholder's load limits; the pause menu no longer spawns ships (a developer tool now).

### Benchmarks of the stress craft of catalog parts (ONEKIND-3, release, one run)

`bench_scale craft parts=300,1000 runs=1` (release, on a machine shared with two building agents: load 10–20; read the ratios, not the last digit). `bench_coast`: 32.0 µs a step, 19 steps an orbit (a bare segment: no craft).

| parts | primitives | triangles | cells | nodes (links) | contacts (legs stowed) | size (m) | build | bake | design | design / craft / save KB |
|---|---|---|---|---|---|---|---|---|---|---|
| 300 | 468 | 318,340 | 7,097 | 12,220 (30,305) | 183 hull | 12.8 × 20.4 | 392 ms | 493 ms | 577 ms | 13,483 / 30,164 / 341 |
| 1,000 | 1,456 | 1,324,036 | 36,503 | 64,423 (162,774) | 282 hull | 17.3 × 34.2 | 3,196 ms | 7,057 ms | 8,522 ms | 72,441 / 122,598 / 1,596 |

| parts | live tick: entry | ascent M 1.2 | vacuum burn | pad | thermal step 20 ms | coast step | lattice solve 600 s (first, tenth) | tumbling day, per lattice point | 10⁶ s holding attitude |
|---|---|---|---|---|---|---|---|---|---|
| 300 | 17.2 ms | 13.2 ms | 10.4 ms | 13.2 ms | 15.4 ms | 0.98 ms | 12.6 ms, 7.3 ms | 11.1 ms | 7.0 s |
| 1,000 | 103 ms | 84 ms | 67 ms | 82 ms | 87 ms | 3.2 ms | 37.5 ms, 28.7 ms | 26.6 ms | 37.3 s |

**5,000 parts did not finish.** Its build, bake, design and live ticks ran (about 30 minutes of the row); its coast measurement then ran over 40 minutes before I stopped it. A 2 s sample: the time is in the coast's tank law, `systems::coast_law::TankLaw::temperature` (libm `atan` and `log`, `fluids::Liquid::vapour_pressure`) and `plumbing::bottles_of`/`geometry`, over the craft's ~6,500 tank compartments (312 boosters of 9–10 two-compartment tanks) at every sample. That is the parts path's scale cost to fix first (the tank law evaluated per tank per sample; D119's law could be summed per stack or cached between lattice points). Rerun one row with `bench_scale craft parts=5000 runs=1`, or `profile=lattice,5000` under `sample`.

### Doc changes needed (for DOCS or the lead)

- `docs/owners.md`: a row for `sim::craft::small` (the small craft of parts for tests and benchmarks; ids `small-<name>` in the registry).
- The parts and vessel technical docs: craft built in code (`craft::small`), and `Vessel::set_fill`. A landed vessel's pose check is saved with it (ce17b16).
- My row in `flight-ui-2026-10-27.md`: there is none, so I left that plan alone.
- (ONEKIND-2) `docs/owners.md`'s `craft::small` row: also `legs_out()` (the rocket on its feet).
- (ONEKIND-3) `docs/owners.md`: `craft::stress` is now the stress craft of catalog parts (`stress::file`, `parts_plan`, `craft(n)`); `craft::small` also `small-plate` and `rocket_ref()`. `docs/technical/scale-performance.md` (or the parts doc): the new stress craft and the numbers below replace the placeholder layout's. The parts technical doc: a vessel of a configuration starts with those mechanisms deployed; `CraftParams::bottom_z` is the lowest contact point.

### Open for the owner (defaults taken)

- **Planned burns between ONEKIND and PLANNER.** The only craft that can plan a burn is the test craft. Default: its planned-burn tests are deleted with it, and PLANNER writes closed-loop ones on craft of parts (D126 replaces the open-loop burn law anyway). Until then, the game offers no planned burns. The other choice is to keep the test craft until PLANNER lands.
- A hard impact breaks a craft of parts up at its parts' loads (D103) instead of destroying it whole. Tests accept either.
- The small rocket's test engine has a radiatively cooled niobium nozzle, so the nozzle-cooling test keeps its meaning. The real Merlin 1D's is regenerative (0). ONEKIND-2 sized its heat share by D077's rule (near 1400 K on its own nozzle): 1.7e-4, not the test craft's 2.2e-3, which ran it at 2650 K, past C-103's melting point.
- (ONEKIND-3) The small rocket's body-mounted cells (E): 2 m² at 28 %, 6 kg. The stress craft's shape is my judgement (a core with rings of radial boosters, every part a catalog generator's): at 5,000 parts it is 312 boosters round a core, 55 m across. Default: kept for benchmarks; the owner may prefer a station-like shape for the scale target.
- (ONEKIND-2) The small rocket's other test numbers (E): four 400 N cold-gas blocks, feet springs 3e5 N/m and 1.8e4 N·s/m with a 0.35 m stroke, 2.5 m legs. They are test parts, not a design the game offers.
