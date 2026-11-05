# Module map for the foundation reset (2026-11-04)

Read-only survey of the code against [scope-2026-11.md](../../plans/scope-2026-11.md) (3.1 data model, 3.2 domains, 3.3 warp, 3.4 adopt, 3.5 targets, 5.2 science, 7 decision changes). Lines are `wc -l` of `.rs` files, inline `*_tests.rs` included. Purpose: let the owner pick the reset size.

## 1. Summary

1. 119,235 lines of Rust (plus 1,895 WGSL): **KEEP 63,740 (53%)**, **ADAPT 32,974 (28%)**, **REPLACE 1,951 (2%)**, **FROZEN 20,570 (17%)**.
2. KEEP is everything the owner kept and more: ephemeris, N-body, frames, gravity, kepler, integrators, aero cells, coast segments, comms, MCP, the tools, and rendering.
3. Vehicle names sit almost only in tests (~700 lines) and data (51 catalog entries, 1 craft file); production code has about 10 code lines that name a vehicle.
4. The specificity is structural: `ShipDef`/`ShipState` keep one vector per function kind; systems are per-kind solvers with one vehicle's constants; vehicle procedures (chute arming, clamp release, ullage, load shed) are Rust.
5. The part/component split already exists in shape (placed generators vs `Generator::Component` inside a host), but entries are vehicle-named instances, the craft file (not the part design) owns components, and graphs are craft-level lists, not ports on mount faces.
6. Warp never enters `sim` as a number; the leaks are step-length and frame-timing ones (power's `charge/dt`, the thermal skip, the 60 s lattice, D1-D6), each with a named fix, 3 with ready `#[ignore]`d tests.
7. Risk 1: the FROZEN UI (20.6k lines) and the agent read `ShipDef`/`ShipState`, so changing the model breaks them unless a thin read adapter sits in between.
8. Risk 2: 21 of 29 integration test files build on today's craft fixtures; rebuild the fixtures before deleting anything, or the determinism net goes. Risk 3: the scheduler change (lattice, 20 ms tick, D127) touches vessel and systems at once.
9. **Recommendation: re-architect the core, inside `sim`.** That means about 20k lines (catalog, craft build/ship/state, systems, the ship side of vessel) rebuilt as new modules beside the old, on top of the kept 53%.
10. Why not the other two: refactoring in place repeats the patch-by-patch pattern of scope section 1; a new crate buys nothing, because `sim` is already engine-free and the new core needs the kept cells, mesh and design types. Section 6 gives the order, with the model-independent warp fixes able to start now.

## 2. Module tables

Verdicts: **KEEP** sound and general; **ADAPT** sound core, reshape to the new data model (what changes is said); **REPLACE** built around one vehicle or the old model (what replaces it is said); **FROZEN** interface work, frozen per scope 7. "Unit" = inline `#[test]` count; "tests/" = files in `crates/sim/tests/` that exercise it.

### 2.1 `crates/sim` (59,647 lines in src, 535 unit tests; 7,902 lines in tests/, 174 tests)

| Module | Lines | What it does | Decisions | Coverage | Verdict |
|---|---|---|---|---|---|
| ephem | 1,206 | Chebyshev/rails tables, "where is node X at t", binary format | D009, D023, D026 | 6 unit; ephemeris_generation, sol_ephemeris, nearly every tests/ file | KEEP (owner-kept) |
| gen | 627 | N-body generation (Yoshida 8), rails fitting | D026 | ephemeris_generation, sol_ephemeris (golden) | KEEP |
| sol | 561 | Solar System inputs, moon systems | D027, D084 | sol_ephemeris | KEEP |
| gravity | 1,008 | Spherical harmonics (Pines), zonal J2-J6 | D051, D090 | 9 unit; gravity_fields, gravity_sources | KEEP |
| kepler | 547 | The one Kepler solver, Lambert, universal-variable propagation | D008 | 10 unit; most tests/ | KEEP |
| integrate | 849 | Yoshida 8 and DOP853 | none | 9 unit; indirect via limbo, ephemeris | KEEP |
| frame, time, math | 618 | Frame-typed vectors, TDB epoch, libm-backed math | D026 | 11 unit; everywhere | KEEP |
| relativity | 127 | Proper time rate | D012, D088 | proper_time, limbo_physics | KEEP |
| body | 1,235 | Body data, atmosphere tables | D037, D047, D059, D062, D090 | 16 unit; aero_boundary, terrain, solar_terrain | KEEP |
| terrain | 968 | Heightmap + procedural detail and craters | D047, D059 | 18 unit; terrain, contact | KEEP |
| world | 659 | Ephemeris plus each body's physical data | D023 | everywhere | KEEP |
| forces | 899 | Translational forces in anchor frames, radiation pressure, surface tables | D023, D090 | 5 unit; gravity_sources, limbo_physics, vessel_dynamics | KEEP |
| light | 265 | Star flux and eclipses | D055 | aerothermal, limbo_physics | KEEP |
| comms | 783 | Links, light time, relay network | D067, D068 | 17 unit only | KEEP |
| approach | 155 | Closest approach between stored paths | none | 2 unit | KEEP |
| rigid | 658 | Euler's equations, exact torque-free rotation | none | 11 unit | KEEP |
| aero | 4,861 | Cell aerodynamics: bake, boundary layer, slender lift, wave drag, shock layer | D061, D070, D073, D085, D109 | 56 unit; aero_boundary, aero_planets, limbo_physics | KEEP (calibrate by SU2 tables later, scope 3.4) |
| thermal | 1,837 | Cell + volume-node network, block Gauss-Seidel solve, heating | D065, D077, D085 | 25 unit; aerothermal, aero_boundary | ADAPT: becomes the template "lumped network" of 3.2; add an embedded error estimate for state-chosen steps and event roots (overheat, thermostat) inside steps |
| fluids, materials | 772 | Propellant/gas and material tables | D101, D104 | 8 unit | KEEP (fluids may sit on CoolProp after the spike) |
| chute | 379 | Drogue + reefed main, filling, overload tear | D064, D079, D128 | 6 unit; vessel_dynamics | KEEP (generic physics; becomes a part type's logic) |
| contact | 704 | Rigid-body ground contact | D066 | 9 unit; contact, landed_rest | KEEP (Rapier spike may replace the solver) |
| proximity | 1,437 | 10 km bubble rule, collision hull, craft-to-craft contact | D066, D082, D083 | 11 unit | KEEP (hull/contact may move to Rapier) |
| landing | 1,266 | Landing prediction and braking start | D064, D089, D090, D125, D128 | 10 unit | KEEP |
| save | 265 | Exact RON round trip of the whole state | D063, D088, D105, D112, D114 | 2 unit; save_load | KEEP (format re-snapshots whatever the new state is) |
| catalog | 3,032 | Generators (tank, shell, thrust structure, engine, fin, grid fin, leg, revolution, component, pad), derived mass, ports, functions | D100-D105, D125 | 10 unit; parts, warp_course | ADAPT: split `Generator` into part types and component types; entries become generic types + parameters instead of vehicle-named instances; reverse override precedence (published calibrates derived, gap logged as model error, D115 reversed); ports move onto mount faces and gain power/data kinds; engine performance from tables (see section 4) |
| craft: cells, mesh, broad, volume, mass, place, design, handle, registry | 3,009 | Solver mesh (cells, volume nodes), triangle surface, poses, mass properties, typed ids, design cache | D065, D102, D121 | ~25 unit; parts, aerothermal | KEEP (cells are solver mesh, as 3.1 says) |
| craft: build, assembly, ship, state, layers, partition, configure, command, program, parts, file, small | 5,246 | Craft file to `ShipDef` (fixed vectors per function kind: engines, bottles, batteries, valves...), `ShipState`, split/merge rules, stage program | D100-D107, D111, D114, D117, D120, D121 | ~40 unit; nearly all tests/ via `small::rocket` | ADAPT: `ShipDef`'s per-kind vectors become part instances holding component instances with ports; plumbing/electrical/data move from craft-level member lists to graphs joined through mount-face ports; craft-level `aero`, `thermal`, `contact`, `impact`, `antenna` blocks move into part types; `groups` stay but carry no physics |
| craft/engine.rs | 250 | Old abstract engine (thrust/Isp as numbers) | D064 | via vessel tests | REPLACE by the engine-family interface (geometry + CRUCIBLE/CEA table, components simulated) |
| craft/stress*, falcon9_tests | 585 | 5,000-part benchmark craft; Falcon 9 build checks | D081, D121 | benches | REPLACE: the station-like "behemoth" stress craft (session Q5) and calibration tests on the new types |
| systems: data, links, estimate, maneuver | 2,408 | Knowledge from sensors; data links; stage Δv estimate; maneuver program | D106, D111, D126, D128 | ~45 unit; maneuver, planned_burns | KEEP (D106 owner-kept; inputs re-pointed to new components) |
| systems: plumbing, plumbing_coast, coast_law, power, crew | 2,577 | Tank/bottle/line network, coast laws, buses and shedding, cabin and life support | D104, D116, D119 | ~30 unit; warp_course, craft_flight | ADAPT: one network solver per domain (3.2) for any topology; closed-form laws with events (warp clauses 2-3); hard-coded constants become part parameters (`TAU_BOTTLE`, `BATTERY_C`, `BATTERY_TAU`, `CABIN_K`, `H_AIR`, `CABIN_SET_K`...) |
| systems: structure, g_limit | 1,409 | Joint loads against capacity each tick; g limit | D103, D111 | ~20 unit | ADAPT: load paths and margins per joint and part (D103 extended); failure records the starting cell |
| systems: propulsion, rcs, exhaust, fins, mechanisms, command, ship, mod | 2,215 | Engine model from published data, RCS mix, exhaust seed, grid fins, hinges, command apply, the live systems tick, the 60 s lattice | D064, D081, D104, D109, D126, D128 | ~25 unit | ADAPT: engines read family tables; the tick and lattice become the canonical-grid scheduler (3.3) |
| systems/failure | 1,302 | Fates (counter-hash per part/mode), damage sums, bursts, blasts | D064, D105 | ~15 unit | ADAPT: keep counter-based draws (already 3.3 rule 5); every failure needs a modelled cause (D105 narrowed); hazard accumulated on coast and landed steps, failure epoch as the root of H(t)=E |
| systems/ship_os: program, sequence, sas, mod | 858 | Program API (layer 2), stage sequence, SAS gate | D043, D107, D111, D114, D128, D129 | ship_programs, hold_modes | KEEP |
| systems/ship_os: chutes, commit, ullage, power_guard | 698 | Hard-coded vehicle procedures: chute arming, pad clamp release, ullage settle, load shed guard | D114, D126, D128 | ~15 unit | REPLACE by vehicle scripts on the program API, shipped with each vehicle's data (scope 2: "configurations plus scripts"); defaults stay frozen until then (scope 7) |
| systems/ship_os/warp_stop + vessel/warp_stop | 393 | What stops warp: built-in risk line, `PRESSURE_FLOOR`, `SUPPLY_MARGIN` | D128 | warp_course | ADAPT: thresholds become per-craft data; stays input-only (never physics) |
| vessel: segment, trajectory, limbo, pointing, coast, clock, id, anchor, create, debris, burn, maneuver, programs, live, attitude | 5,621 | Coast segments ("prediction is the truth"), perturbed coasts, attitude, live 20 ms ticks, programs at epochs | D012, D024, D066, D075, D081, D083, D126, D129 | ~40 unit; most tests/ | KEEP (live tick rate becomes multi-rate inside, D127 replaced) |
| vessel: ship, ship_tick, ship_loads, systems, mass, split, aerothermal, coast_heat | 4,092 | Vessel side of the ship model: mass from tank slugs, systems in phases, loads for structure, splitting, aero/heat wiring | D032, D065, D083, D100, D104, D119 | ~35 unit; parts, aerothermal, warp_course | ADAPT: follows the new part/component model; split by joints with parts failing whole; aerothermal skip replaced (section 5) |
| vessel: helm, hold, bubble, mod (advance) | 2,108 | Delayed controls, holds, bubble membership, the advance loop | D034, D075, D082, D112 | hold_modes, warp_course | ADAPT: warp divergences D1-D3 and the unstepped landed thermal (section 5) |
| vessel/breakage.rs | 129 | Placeholder break limits | D065, D066, D083 | via parts | REPLACE by structure margins (load paths) |
| tests/ (29 files) | 7,902 | Integration and mission tests | D122, D129 | n/a | KEEP; 20 files build `small::rocket`, 5 load `falcon9-crew` (falcon9_flight, maneuver, planned_burns, save_load, warp_course), 21 in all: fixtures must be re-pointed first |

**sim totals:** KEEP 33,007; ADAPT 24,978; REPLACE 1,662.

### 2.2 `crates/game` (45,833 lines of Rust, 530 tests; plus 1,895 lines of WGSL)

| Module(s) | Lines | What it does | Decisions | Tests | Verdict |
|---|---|---|---|---|---|
| interface (8,948), systems_screen (5,659), navball (2,114), tracking (631), planner (724), map + map_view (865), hud (491), staging (446), settings_ui (224), rendezvous (186), landing (282) | 20,570 | HUD, task menus, part cards, staging bar, systems screen, navball, map, planner | D050, D054, D080, D107, D111, D120 | ~230 | FROZEN until the part view (scope 4, answer 44). Most of it reads `ShipDef`/`ShipState`, so it will need re-pointing or stubbing when the model changes |
| agent (4,898) | 4,898 | MCP tools answered from the game, mission sessions, program tools | D044, D069, D122 | 51 | ADAPT: tools address parts/components by the new ids; parity with the UI kept (owner-kept MCP) |
| state (914 with tests), commands (1,174), fleet (626 with tests), debris (252) | 2,966 | Clock and warp, delayed commands, per-frame fleet advance, debris deletion | D022, D034, D083, D112, D129 | 11 | ADAPT: warp divergences D2, D4, D6 (section 5) |
| test_rocket (132) | 132 | Painted test craft | D100 | 0 | ADAPT: rebuilt on the new types |
| pad (134) | 134 | Pad-abort reset | D114 | 2 | REPLACE by a generic "reset to launch save" command |
| trajectory, terrain, scene, lighting, sky, atmosphere, rings, clouds, ship_light, look, render_scale, camera, body_visual, relations, format, comms, saves, persist, profile, bench, offscreen, shader_tests, main | 17,133 | Rendering, camera, orbit lines, saves, profiler, headless game | D046, D052, D055, D072, D073, D087, D109, D110, D122 | ~200 | KEEP (scene's ship drawing reads the craft mesh, unchanged in kind) |

**game totals:** FROZEN 20,570; KEEP 17,133; ADAPT 7,996; REPLACE 134.

### 2.3 Other crates

| Crate | Lines | What it does | Verdict |
|---|---|---|---|
| mcp | 459 | Minimal MCP server (HTTP, JSON-RPC), knows nothing of the game; 8 tests | KEEP (owner-kept) |
| ephem-tool | 1,365 | Ephemeris and gravity-field generation from Horizons and published models | KEEP |
| asset-tool | 4,029 | Offline terrain, colour, water, star-catalogue baking | KEEP |

## 3. Vehicle-specific code inventory

Grep over `crates/` for dragon, falcon, merlin, mvac, draco, superdraco, f9, trunk, octaweb, nosecone, raptor, starship, electron, rutherford, iss. Counts are matching lines, split by whether they sit in test code (`#[cfg(test)]` blocks and `*_tests.rs`) or production code (comments included).

| Module | Total | Production | Tests | Main terms |
|---|---|---|---|---|
| sim/systems | 341 | 51 | 290 | falcon, dragon, merlin, mvac, draco, trunk, octaweb |
| sim/vessel | 128 | 1 | 127 | falcon, merlin, mvac, dragon |
| sim/craft | 123 | 9 | 114 | dragon, falcon, merlin, draco, trunk |
| sim/catalog | 18 | 10 | 8 | merlin, trunk, nosecone, octaweb |
| sim/fluids, materials | 3 | 0 | 3 | falcon, dragon |
| game/interface | 91 | 4 | 87 | falcon, dragon, mvac, draco |
| game/systems_screen | 31 | 0 | 31 | dragon, falcon |
| game/scene | 26 | 1 | 25 | merlin, mvac |
| game/agent + agent.rs | 31 | 1 | 30 | falcon, dragon, draco |
| game/state, state_tests | 22 | 3 | 19 | falcon, f9 |
| game/pad, navball, saves, lighting | 10 | 2 | 8 | falcon, octaweb |

False positives left out: `electron` in sim/aero, comms, thermal is electron density (plasma); `ISS` in asset-tool is Cassini's Imaging Science Subsystem; `f9` in terrain is a hex literal.

**What this shows.** Production code names vehicles in comments (citing `falcon9.md`) and in about 10 code lines; the names live in tests (about 700 lines) and in data (51 catalog entries, one craft file). The specificity is structural, not lexical:
- `ShipDef`/`ShipState` hold one vector per function kind (`engines`, `bottles`, `batteries`, `valves`, `mechanisms`, `surfaces`, `seats`, `solars`, `life_support`), so a new kind of component means new code in craft, systems, vessel, the systems screen and the agent.
- Vehicle procedures are Rust functions in the ship computer: `ship_os/chutes.rs` (Dragon's chute arming), `commit.rs` (pad clamp release), `ullage.rs`, `power_guard.rs`, plus `game/pad.rs`.
- Function kinds that exist because Falcon 9 has them: `Generator::ThrustStructure` (octaweb), `GridFin`, `Pad`; `systems/fins.rs` (grid fins in flight); `mechanisms.rs`'s doc names F9 legs, grid fins and Dragon's nosecone.
- Tuning constants that are one vehicle's values in code: `crew.rs` (`CABIN_SET_K`, `TOTAL_SET`, `CREW_HEAT`), `power.rs` (`BATTERY_C`, `BATTERY_TAU`, `CABIN_K`), `plumbing.rs` (`TAU_BOTTLE`, `TAU_ULLAGE`, `LINE_LOSS`, `HEATED_K`), `ship_os/warp_stop.rs` (`PRESSURE_FLOOR`, `SUPPLY_MARGIN`).
- Tests that assert on Falcon 9 + Dragon ids (`"s1.merlin.9"`, `"dragon.bus"`): `systems/power.rs` (38 lines), `craft/program.rs` (26), `systems/failure/tests.rs` (21), `systems/ship_os/tests.rs` (18), `craft/command.rs` (10), and 5 tests/ files.

## 4. Today's catalog against the new part/component split

Today: 51 entries in `data/parts/{dragon,falcon9,engines,equipment}/*.ron`, each a vehicle-named instance of one `catalog::Generator`; one craft file, `data/craft/falcon9-crew/craft.ron` (272 lines), places 23 part slots by mounts (`parts:`) and 60-odd component slots by `in: <part>, pos:` (`components:`).

**The shape of the split already exists.** `Generator`'s variants other than `Component` (Tank, Shell, ThrustStructure, Engine, Fin, GridFin, Leg, Revolution, Pad) are placed objects with one shape: parts. `Generator::Component` with its `Equipment` (Bottle, Tank, Battery, Computer, Sensor, Valve, Solar, LifeSupport, Thrusters, Engines, Chute, Docking, Seat, Other) sits inside a host part with no outer shape: components. Three things differ from the new model:
1. **Instances vs types.** `dragon-flight-computer.ron` and `f9-battery.ron` are vehicle-named catalog files; in the new model they are parameter sets of generic component types (`computer.flight` with channels 3, 300 W, 100 kg E; `battery.cell_pack` with 2,500 Wh, 28 V, 120 Wh/kg) chosen inside a part's design.
2. **Who owns components.** Today the craft file puts components into parts. In the new model the part's design owns them (scope 5.2: one design program yields one part with its components, which swap within it). The capsule's 4 batteries, triplex computer, IMUs, tanks and COPVs move from `craft.ron` into the capsule part design.
3. **Graphs.** Plumbing, electrical buses (`members: ["dragon.battery#*", ...]`) and data links are craft-level lists today. They become component-to-component graphs that cross part boundaries only through ports on mount faces. Today's `Port` has fluid kinds only and a position, not a mount face.

| Today's entry (count) | Generator | New model | Note |
|---|---|---|---|
| f9-s1-tank, f9-s2-tank (2) | Tank | part, `tank.cylindrical` (common bulkhead option) | components: pressurant bottles, downcomer/transfer tube (lines), prevalves, level and pressure sensors, now in craft.ron |
| f9-interstage, f9-dragon-ring, crew-dragon-trunk (3) | Shell | part, `shell.cylindrical` / adapter | trunk's solar cells become its component |
| f9-octaweb, f9-s2-aft-skirt (2) | ThrustStructure | part, `structure.thrust` | mounts for engines and legs stay |
| merlin-1d, merlin-1d-vac (2) | Engine | part, chemical engine family | scalar `igniter`, `gimbal_rate_dps`, `npsp_*`, `start_time` become components (turbopump, main valves, gimbal actuators, igniter); performance from a CRUCIBLE/CEA table; mass today from `thrust_to_weight` (an estimate) becomes derived |
| f9-grid-fin, f9-landing-leg, crew-dragon-trunk-fin (3) | GridFin, Leg, Fin | parts | actuators (hydraulic, pneumatic) become components |
| crew-dragon-capsule, heat-shield, nosecone (3) | Revolution | parts | capsule hosts most components; heat shield as a separate part is a session-1 call |
| lc39a-hold-down (1) | Pad | not a craft part: ground infrastructure of a site | |
| batteries, flight computers, COPVs, sensors, prevalve, seat, ECLSS, chutes, trunk solar, small MMH/NTO tanks (26) | Component | components of their host part | e.g. `dragon-flight-computer.ron`, `f9-battery.ron`, `imu.ron`: already generic-ish except the vehicle-named ones |
| f9-lox-downcomer, f9-s2-transfer-tube (2) | Component `Other` | `line.tube` components, auto-routed (3.1 routing) | |
| mass-item (1, used 4 times: windows, avionics, crew and cargo) | Component `Other` | dropped: unmodelled mass becomes the logged calibration gap, crew and cargo become payload | |
| draco-cluster, draco-forward, superdraco-pod, f9-cold-gas-pod, f9-s2-cold-gas (5) | Component `Thrusters`/`Engines` | **session-1 call** | owner's example makes a radial RCS thruster a part; Dracos and SuperDracos sit inside the capsule wall; scope 5.2's rule of thumb (bought or swapped between runs = component) |
| dragon-docking-adapter (1) | Component `Docking` | likely a part (own shape, a port on its face) | session-1 call |

Craft-file blocks: `parts` stay; `components` move into part designs; `joints` become mount-face joints with load paths; `plumbing`/`electrical`/`data` become port graphs; `groups` stay (editor, staging, scripts, no physics); `sequence`/`functions` become scripts; `checks.dry_mass` become calibration targets; craft-level `aero` (cd0, roughness), `thermal`, `impact`, `contact`, `antenna` move into part types (craft-level physics contradicts 3.1).

## 5. Where warp or frame time leaks into physics

Sources: `director/logs/night-2026-11-03/warp-research.md` section 6 and `docs/technical/warp-invariance.md` section 4 (divergences D1-D6), checked against today's tree. Good news first: `sim` never reads the warp setting (one hit for "warp" in sim production code, the warp-stop program's name). Warp enters only through the game's frame span (`game/src/state.rs`: `frame_span(dt * warp, ...)`), so every leak is about **where a step ends** or **when an input is applied**, never a warp-scaled coefficient.

| Where | Leak | Kind | Fix (scope 3.3 rule) | Module verdict |
|---|---|---|---|---|
| `sim/src/systems/power.rs` (~line 119, `can`: `charge / dt`) | Shedding judged on what batteries give over the step, so the outcome depends on step length | step length into physics | closed-form battery law, shed and empty epochs as events (rules 2, 4) | ADAPT |
| `sim/src/systems/ship.rs` (~line 64, `since`) | Slow plumbing covers "time since it last stepped" while power and crew step by the tick | mixed step lengths | one law over any interval (rule 2) | ADAPT |
| `sim/src/systems/mod.rs:48` (`STEP = 60 s`) | Lattice is where events land, not a cap on a state-chosen step | events late to grid | events located by root-finding inside steps (rule 4) | ADAPT |
| `sim/src/vessel/aerothermal.rs` (`STEP_CHANGE_K` 0.5 K, `SUN_SAME`, `MAX_SKIP` 1 day) | Heuristic skip of the thermal solve; errs up to 1.3 K | heuristic quiescence | error-controlled steps plus a periodic-steady-state certificate with recorded bound (rule 3) | ADAPT |
| `sim/src/thermal/mod.rs` (`step`, `step_to`) | No error estimate, no events inside steps | events late to grid | keep block Gauss-Seidel; add embedded estimate and event functions | ADAPT |
| `sim/src/vessel/mod.rs` (`advance_grounded`) | A landed vessel's thermal network is never stepped | model gap (same at every warp) | step landed vessels on the same grid | ADAPT |
| `sim/src/systems/failure/mod.rs`, `fates.rs` | Hazard accumulates only in live ticks | phase-dependent | accumulate on coast and landed steps; epoch as root of H(t)=E | ADAPT |
| `sim/src/vessel/hold.rs` (`TargetState`) + `game/src/navball/mod.rs` (`feed_hold`) | D1: TARGET hold fed a per-frame snapshot carried at constant velocity (102° apart at 10⁵x) | frame time into physics | target state as a pure function of epoch from the target's own segment | ADAPT |
| `sim/src/vessel/helm.rs` (`acting`) + `game/src/commands.rs` (`InFlight` due list) | D2: delayed settings act at the first frame after arrival (109° apart) | frame time into input timing | apply at `sent + delay` as an order epoch | ADAPT |
| `sim/src/vessel/bubble.rs` (~line 324, "craft past the epoch ... judged later") | D3: a neighbour moved ahead by the fleet never joins the bubble (0.15 m apart) | frame order into model choice | bubble rule evaluated before any craft passes the lattice epoch (conservative lookahead) | ADAPT |
| `game/src/debris.rs` (~line 153, `debris(world, clock, ...)`) | D4: deletion at the frame's clock | frame time into physics (rare) | deletion as an event on the debris rule | ADAPT |
| agent `run_until` (game/agent) | D5: condition stops checked at frame ends | input timing (test tool) | conditions as event functions | ADAPT |
| `game/src/state.rs` (rotation input only at physics warp) | D6: the one place warp decides something | warp into input | act as at 1x, or refuse warp while held | ADAPT |
| `sim/src/vessel/live.rs` (fixed 20 ms tick, D127) | Not a leak, but the one rate for every subsystem | design | multi-rate on the canonical grid (D127 replaced) | KEEP core |

`crates/sim/tests/warp_course.rs` (675 lines, 36 functions) already pins D1-D3 as `#[ignore]`d tests: the fixes have ready acceptance tests.

## 6. Suggested order of work

Steps on one line run in parallel; nothing waits on a date.

0. **Before code:** land stash@{0}; design session 1 writes `technical/craft-data-model.md` (part and component types, ports on mount faces, graphs, groups, calibration log) and settles the open calls in section 4 (thruster pods, docking adapter, heat shield, maximum part size).
1. **Model-independent warp fixes, now, in parallel with step 0:** D1, D2, D3 (un-ignore their `warp_course` tests), D4, D6, `advance_grounded`'s missing thermal step, fates on coast and landed steps. About 5k lines touched, all ADAPT, no data-model dependency.
2. **New core types beside the old ones** in `sim` (new modules, not a new crate): part type, component type, port, graph, group, calibration record. The build still emits today's cells, mesh, volume grid and mass properties (KEEP), so aero, thermal, contact and proximity run unchanged.
3. **Fixtures first:** rebuild `craft::small::rocket` and an Electron on the new types; re-point the 20 tests/ files that use `small` and the 5 that load `falcon9-crew`. The golden ephemeris, gravity and warp suites must stay green before anything old is deleted.
4. **One network solver per domain**, each with exact or exponential laws and events at roots, under the canonical-grid scheduler (3.3; replaces the 60 s lattice and D127's one rate): thermal first (it is already a network), then power, fluids (plumbing, bottles), data. These run in parallel once step 2's interfaces exist. Each replaces its per-kind `systems/` module and moves its hard-coded constants into part parameters.
5. **Ship software:** `ship_os/chutes`, `commit`, `ullage`, `power_guard` and `game/pad.rs` become scripts on the program API shipped with each vehicle; warp-stop thresholds become data.
6. **Structure:** load paths and margins per joint and part (D103 extended), replacing `vessel/breakage.rs`; failures record their starting cell.
7. **Calibration and composability:** Falcon 9 + Dragon rebuilt from the same types; Dragon on another booster, engine count changed, tank stretched, no code changes. Then delete the 51 vehicle-named entries, `ShipDef`'s per-kind vectors, `craft/engine.rs`, the old stress craft, and build the "behemoth" stress craft for the 3.5 benchmarks.
8. **Interfaces last:** agent tools re-pointed to the new ids (MCP parity); FROZEN UI reads the new model only through a thin read adapter until the part-view session.
