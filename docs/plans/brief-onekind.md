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

### How to move a test (for the next agent)

- `&sim::craft::small::rocket()` replaces `test_craft()` (it is an `Arc<Craft>`). It is about 13.9 t loaded, 1 crew, four 100 kN engines (Isp 320/280 s, throttle 0.4–1), cold-gas thrusters, four legs and Apollo chutes.
- Engines start only through the sequence: `assert_eq!(v.next_stage(&w).unwrap().0, "Ignition")`. A throttle below 0.4 shuts them down.
- Propellant: `set_fill(&w, f)` sets every tank's liquid. For a target mass, fill to 1 and to 0 and interpolate (`vessel_dynamics::above_the_cape`). `propellant()` includes the ullage and bottle gas, and LOX boils off on coasts and on the pad, so compare within ~1 kg.
- A hard impact breaks a craft of parts up (`v.breakup().is_some()`, the advance stops there) rather than `Phase::Crashed`; the impact speed is in `breakup().destroyed` (`vessel_dynamics::wrecked`).
- Contact height: take it from a landed vessel (`terrain::standing`), not from `CraftParams::contact_height(16_000.0)` (the made-up mass law).
- Standing on feet: `small::legs_out()` (the stowed rocket stands on its bells and has no feet). Feet 3.13 m out on X and Y, tip angle 29° empty and ~32° full with two feet downhill.
- Destroyed: `contact::still()` (landed, crashed or a breakup pending) and `contact::impact()` (the impact from `phase` or `breakup().destroyed`); `aerothermal::wrecked()`.
- An exact mass that does not boil off on a coast: fill only the RP-1 tank (`set_tank` with the index of `tank:fuel` in `ship_def().tanks`), the LOX at 0.

### Tests not kept, and why

- `contact::a_steep_slope_tips_it_over_and_it_comes_to_rest_lying_down`: it still falls over, never frozen mid-fall, but may slide on down the 35° slope lying on its round hull. Its tip angle (29–32°) is above the hull's sliding angle (atan 0.5 = 26.6°), so no slope both tips it and holds it lying. The finned test craft could not roll.
- `contact::twelve_metres_per_second_on_the_hull_destroys_the_craft`: the whole wreck's bounce and rest are gone. A craft of parts breaks up at the impact, and its pieces are the game's split.
- `aerothermal::an_entry_…`: the destroyed cell is the one most over its limit, not the hottest cell (the niobium nozzle runs hotter, within its own limit).
- `frame_live_stop`: the Moon fall ends at rest or lying within its reach of the surface. A round hull on its side has no support polygon to rest on.

- `craft_flight::rcs_burns_propellant_at_its_flow_per_axis` (D078's couples drawing the main propellant: the made-up attitude model itself) and `powered_ticks_burn_propellant_at_the_engine_rate` (the scalar mass law): the parts path's versions are `vessel::ship_tests` (each tank drained by the mixture ratio) and `ship_systems_tests::the_first_stage_draws_its_engines_flow_and_nothing_else`.
- `vessel_dynamics::a_drogue_fired_far_above_its_design_q_tears_and_the_mains_still_land_a_light_craft` is now `…_tears_and_so_do_the_mains_behind_it`. With its drogue torn, the slender rocket reaches the mains' height near 190 m/s and they tear too. The finned test craft's broadside drag used to slow it enough for the mains alone. A mains-alone landing would need a draggier small craft.

### Left, in order

1. **Move the remaining tests.** Nothing blocks them now: DOCS has committed, and the demo and tour are deleted.
   - sim integration tests: `limbo` (3 tests; its two ion-burn tests are planned burns and wait), `limbo_physics` (drag decay: set the ISS-like mass with RP-1 only; the plate; the anchor; thin air. `boil_off_ends_a_low_thrust_burn…` and `a_flown_burn_takes_no_boil_off` are the made-up boil-off (`set_boil_off`) and planned burns: delete with the path, the parts' boil-off is the tanks' own, D119), `limbo_regime`, `save_load` (it reads `docs/plans/playtest-2026-10-01-orbit.ron`, a test-craft save).
   - sim unit tests: `aero/{tests,lift_tests}`, `chute/tests`, `craft/{broad,cells_tests,design,engine,mass,mod,parts,registry,volume}`, `forces/surface`, `landing/tests`, `proximity/{contact_tests,hull_tests}`, `thermal/{tests,heating_tests}`, `vessel/{bubble_tests,tests}`.
   - examples: `bench_{aero,bubble,fleet,integrator,limbo,thermal}`, `long_run`.
   - game: `debris`, `fleet_tests`, `menus/{data,hottest,landing,rcs_data,rcs}`, `stage_bar/rows_tests`, `landing`, `navball`, `saves`, `staging`, `state`, `state_tests`, `trajectory/*_tests`; `GameCommand::SpawnTestShips` (MISSION's brief).
   - **Blocked on planned burns for craft of parts (PLANNER, D126):** `planned_burns` (11 tests), `hold_modes`' maneuver-hold tests, the game's planner and `trajectory/predict_tests`, and `landing::model` (`craft.mass.at`, `craft.engine`). Default below.
2. **The stress generator on the parts path** (not started by ONEKIND-2: no new numbers yet): an `AssemblyFile` built in code from catalog generators (tank, engine, thrust structure, revolution, solar), resolved as `stress-<n>` like `small-<name>`, its entries in `craft::small`'s catalog or one of its own. Keep `bench_scale` and `bench_coast`, and record their numbers at 300, 1,000 and 5,000 parts here. `stress_tests`, `split_tests`, `mass_tests`, `bubble_tests` and `tests/parts.rs` use the stress craft.
3. **Delete the made-up path** (the inventory above). Also in `build.rs`, the craft-of-parts build falls back to `test_craft()`'s skin and chute: it needs a default skin and "no chutes". Then: the test craft's data, its new-flight entry (`state.rs`), `SUNSCATTER_CRAFT=test-craft`, and the playtest save (regenerated from a Falcon 9 flight, or dropped if MISSION's tools replace it).
4. **Decisions:** delete D074; trim D064 to debug mode (D123); update D060, D078, D079 (the test craft's chutes), D100 ("the test craft stays as the debug craft") and D104; update every citation in the same commit.

### Doc changes needed (for DOCS or the lead)

- `docs/owners.md`: a row for `sim::craft::small` (the small craft of parts for tests and benchmarks; ids `small-<name>` in the registry).
- The parts and vessel technical docs: craft built in code (`craft::small`), and `Vessel::set_fill`. A landed vessel's pose check is saved with it (ce17b16).
- My row in `flight-ui-2026-10-27.md`: there is none, so I left that plan alone.
- (ONEKIND-2) `docs/owners.md`'s `craft::small` row: also `legs_out()` (the rocket on its feet). The parts technical doc: a vessel of a configuration starts with those mechanisms deployed; `CraftParams::bottom_z` is the lowest contact point.

### Open for the owner (defaults taken)

- **Planned burns between ONEKIND and PLANNER.** The only craft that can plan a burn is the test craft. Default: its planned-burn tests are deleted with it, and PLANNER writes closed-loop ones on craft of parts (D126 replaces the open-loop burn law anyway). Until then, the game offers no planned burns. The other choice is to keep the test craft until PLANNER lands.
- A hard impact breaks a craft of parts up at its parts' loads (D103) instead of destroying it whole. Tests accept either.
- The small rocket's test engine has a radiatively cooled niobium nozzle, so the nozzle-cooling test keeps its meaning. The real Merlin 1D's is regenerative (0). ONEKIND-2 sized its heat share by D077's rule (near 1400 K on its own nozzle): 1.7e-4, not the test craft's 2.2e-3, which ran it at 2650 K, past C-103's melting point.
- (ONEKIND-2) The small rocket's other test numbers (E): four 400 N cold-gas blocks, feet springs 3e5 N/m and 1.8e4 N·s/m with a 0.35 m stroke, 2.5 m legs. They are test parts, not a design the game offers.
