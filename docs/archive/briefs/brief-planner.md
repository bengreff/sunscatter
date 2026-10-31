# Brief: PLANNER, planned burns for craft of parts (2026-10-30)

D126: planned burns run closed-loop to their target state, with real engine starts, ullage settling and tail-off, as flight-layer programs. Today only the test craft can plan a burn (`Vessel::set_plan` returns `PlanError::PartsUnsupported` for a craft of parts), and its burn is open-loop: steady thrust at a constant mass flow along a direction law (`sim::vessel::burn`). Follow [agent-rules.md](../../plans/agent-rules.md). Your name is PLANNER and your target dir is `target/agents/planner` (a cold start).

Two other agents run at the same time:
- **ONEKIND-3** ([brief-onekind.md](../../plans/brief-onekind.md)) moves the other tests onto craft of parts and rebuilds the stress craft. It deletes the made-up path and the test craft only **after** you land: leave the open-loop burn law in place, and tell the lead in your State when your tests no longer need it.
- **AUDIT** ([brief-audit.md](brief-audit.md)) reads the code for D129 (the same course of events at every warp) and D128 (the software layers), and writes only new docs and test files.

## Read first

- Decisions: D126 (revised tonight: nodes only; an automatic burn settles with the craft's thrusters; the takeover rule), D128 (the three software layers: your program is layer 2), D129 (burns fly live for now; the same course of events at every warp), D075 (planned burns are flown by the maneuver hold), D029, D111, D022. The owner's answers: [interview-2026-10-30-roadmap.md](../interviews/interview-2026-10-30-roadmap.md).
- Docs: `docs/systems/motion.md`, `propulsion.md`, `avionics.md`; `docs/technical/engines.md` (start, tail-off, settling before a start), `plumbing.md` (settling by Bond number), `flight-software.md`, `ship-model.md` §5.10, `limbo.md` (low-thrust burns), `mcp.md` (`set_plan`), `testing.md` (how to fly a mission test). The planner's old spec: `docs/archive/plans/realism-1.md` §6a.
- Code: `sim::vessel::burn` and `Vessel::set_plan` (`vessel/mod.rs`), `sim::vessel::trajectory`, `sim::systems::{flight, propulsion, rcs, estimate, plumbing}`, `sim::craft::program`, `game::planner`, `game::trajectory`, `crates/sim/tests/{planned_burns, hold_modes}.rs`.

## The work, in order

1. **Design, in this brief's State, before code** (keep it short):
   - The burn program as one self-contained layer-2 function (D128): it reads only `Knowledge` and the plan, and issues the commands a player issues (the maneuver hold's direction, RCS translation, ignition, throttle, cutoff). No reach into the truth, no part logic inside it.
   - The loop: from the node (Δv prograde, normal and radial at a time) to the target orbit it gives, steered closed-loop and cut off on it, with the cutoff leading by the predicted tail-off impulse.
   - The sequence: warp ramps down to the burn (D126), the craft turns to the burn's attitude, settles its propellant with its own thrusters while the plumbing says it is unsettled, starts its engines through their real start, then burns and cuts off. A craft that cannot settle gets a warning in the planner, and its start does what the physics says.
   - Takeover (D126): input that agrees with the program, or touches what it does not command, leaves it running. Input that conflicts with it (a different throttle, attitude input against the hold, a cutoff) switches the burn fully to manual. The rest of the plan stays.
   - Live (D129): the burn flies on the live tick at physics warp. When a burn comes due while the game is at rails warp, the craft goes live at the burn's own epoch, whatever the warp was. Whether the player sat at 1× or at 10⁵× before the burn, the result is the same bits.
   - What the planner shows before the burn: the predicted orbit, as a sim function with one owner, through the same estimates the staging bar uses (`systems::estimate`). It is labelled as a prediction; the stored trajectory is what was flown.
   - Low-thrust burns on rails (`limbo.md`) are D029's warp while thrusting, which comes later. If the path is generic, keep it. If it only works on the made-up path, it goes with that path (ONEKIND-3), and you record it here.
2. **Build it** with table tests for each rule (rule 8): settling done, ignition, cutoff with tail-off, takeover, the burn coming due under warp.
3. **Move the planned-burn tests onto craft of parts**, each keeping what it checks:
   - `planned_burns` (11 tests) and `hold_modes`' maneuver holds;
   - the game's planner and `trajectory/predict_tests`;
   - `landing::model` (`craft.mass.at`, `craft.engine`).
   Fly the Falcon 9's second stage (MVac, with its cold-gas thrusters for settling) or `craft::small`'s rocket. Keep the determinism, warp and chunked = single-pass tests green. Record any test that cannot be kept, and why.
4. **MCP `set_plan`** accepts craft of parts. **Mission test** (D122; `docs/technical/testing.md`, `tools/mcp.sh`): fly the Falcon 9 to orbit headless, then raise or circularise the orbit with a planned burn on the second stage, with shots of the burn and the result. If Dragon's Dracos can fly a planned burn on the same mechanism (D125), try one after separation.
5. **Docs:**
   - `docs/technical/planner.md` (new): the planner's spec from realism-1 §6a, as built now;
   - the status lines in `systems/motion.md`, `propulsion.md` and `avionics.md`;
   - `owners.md` rows;
   - `mcp.md`'s `set_plan` row;
   - `testing.md`'s mission-test note on orbit insertion.

Small commits, each with its tests passing (`cargo test -p sim`, the game's tests you touched, clippy). End with the report agent-rules asks for.

## State

### Design (PLANNER, 2026-10-30)

- **Where:** `sim::systems::maneuver`, a layer-2 program (D128). `step(&Inputs, &mut Program) -> Commands` reads only what `flight::sense` gives (attitude, navigation, the engine controllers' readouts, the feed tanks' settled flag where a level sensor reads them), the node, the stage estimate (`systems::estimate::lit`, the staging bar's walk for the engines it will light) and the static definition. It returns what a player issues: SAS on with the maneuver hold and its direction, translation, throttle, ignitions. The vessel applies them at the start of each live tick, in one place (`Vessel::tick_controls`) used by all three tick loops.
- **The plan:** `FlightPlan.nodes` (`Node { t, dv: prograde/normal/radial-out, reference }`). A craft of parts takes nodes only, the made-up craft burns only (the open-loop law stays until ONEKIND-3 deletes it). Nodes do not touch the stored trajectory: the coast is unchanged, and the vessel stops at the program's wake epoch like an order (`order_stop`), whatever the warp, and goes live there (D129).
- **Schedule** (at `set_plan`, and for the next node when a burn ends): ignition = node − the time to give half the Δv (the estimate's thrust, flow and mass) − half the start transient; wake = ignition − the turn lead (`hold::turn_lead`) − a settling lead (E: 20 s).
- **Steps:** *wait* → (wake) *turn* (maneuver hold on the burn direction) → (aligned within 2° E, or ignition due) *settle* (translate along the thrust axis while the sensors say a feed is unsettled; with no level sensor, for the settling lead) → (settled at or after ignition, or ignition + 60 s E) *ignite* (`Ignite` for each engine, throttle 1: the engines' own start; on floating propellant the start does what layer 1 does) → *burn* → *tail-off* (throttle 0) → *done*. Takeover or an engine stopping ends it as *manual* or *short*.
- **The loop:** the target is the orbit the node gives: at wake, the navigation state propagated to the node by `kepler::propagate` (new, universal variables; the one Kepler owner) plus the node's Δv in its axes there. Each tick, v_go = the target's velocity at the node minus the current state's propagated to the node (both two-body about the node's reference; the software's model, never the physics). Steer along v_go; cut off when |v_go| falls to the tail-off's Δv (Σ shutdown impulse / mass) plus half a tick of thrust, or when v_go turns against the burn.
- **Takeover (D126):** the pilot's controls at wake are the baseline. A change that equals what the program commands, or touches what it does not command (chutes, RCS mode, speed reference, target, translation outside the settle), leaves it running. A changed throttle, any rotation input, SAS off, another hold, or translation against the settle switches the burn to manual: the program stops commanding and the node is spent; later nodes stay.
- **Prediction (planner):** `Vessel::node_prediction` (one owner): the target state at the node from the stored coast, ignition and duration from the same estimate, warnings (no thrusters push along the thrust axis: cannot settle; not enough Δv; too soon for the turn). The game draws the coast from that state as a prediction; the stored trajectory is what was flown.
- **Low-thrust burns on rails** (`limbo.md`) are the made-up path's open-loop law: they go with it (ONEKIND-3), and come back with D029.

### State (PLANNER-1, wrapped up at ~300k tokens, 2026-10-30)

**Landed:** bce4379 `kepler::propagate` (universal variables, table tests); c7c2de4 `systems::maneuver` (the program's rules and step, 9 table tests: v_go, alignment, cutoff with tail-off, takeover, schedule, the step sequence, short ends) and `estimate::lit`; 40a91ad this design.

**Landed with the vessel side (0a382b6):** `vessel/maneuver.rs` (`Vessel::set_nodes`, `nodes`, `program`, `node_prediction`, the schedule, the wake at its own epoch beside `order_stop`, `tick_controls` in all three tick loops, the ship log), `flight::feeds_read`, `FlightPlan::next_direction` (the maneuver hold points at the next node on rails too), and `tests/maneuver.rs`: a 40 m/s prograde node on the small rocket flies turn, ignition, burn and cutoff ("on target, 0.27 m/s left"), coasts after, ends 0.93 m/s and 33 m from the program's target at the node, and 60 fps frames give the same bits as one jump (D129). Nodes live on the vessel (`Vessel.nodes`), not in `FlightPlan`: a new `FlightPlan` field breaks every struct literal of it, ONEKIND-3's files included. When the made-up path goes, the nodes become the plan.

**Found:** the small rocket's thruster blocks cannot push along its engines (`rcs::can_push`): the planner warns that it cannot settle, and it ignites after the settling lead regardless.

**Left, in order:**
1. Accuracy: 0.93 m/s left against the program's 0.27 m/s at cutoff (the test allows 1.5): match the tail-off's Δv (engine `shutdown_impulse` against what the tail-off gives) and the tick's quantisation (throttle down for the last seconds, or cut at the predicted sub-tick).
2. Table tests at the vessel: the wake at its own epoch under rails warp (a frame of 10⁵× stops at the wake, like an order), takeover mid-burn (a throttle change: manual, the next node kept), a split mid-burn ends the program, save and load mid-burn bit-identical, the planner's warnings.
3. Move `planned_burns`' 11 tests onto `set_nodes` on the small rocket or the Falcon 9 second stage (each keeping what it checks; the trajectory-segment ones become: the coast is untouched by nodes, chunked = single pass of the coast up to the wake); `hold_modes`' maneuver holds (the hold reads `next_direction` with the nodes); the game's planner (`plan_from` sends nodes to a craft of parts: a `GameCommand::SetNodes`), `trajectory/predict_tests` (the predicted path after a node from `node_prediction`), `landing::model`.
4. MCP `set_plan` takes nodes for a craft of parts (`agent.rs`); the mission test (Falcon 9 to orbit, then a node on the second stage), with shots.
5. The game hands the program's last throttle and hold to the pilot's controls when it switches to manual (now the pilot's own controls fly from that tick: a rotation nudge mid-burn closes the throttle if the pilot's is at 0).
6. Docs: `technical/planner.md`, the status lines (motion, propulsion, avionics: *skeleton* for planned burns of parts), `owners.md` rows (`systems::maneuver`, `kepler::propagate`, `Vessel::node_prediction`), `mcp.md`, `testing.md`.

**The made-up path's open-loop law** is still needed by `planned_burns`, `hold_modes`, the game's planner and `limbo` until item 3 lands. Low-thrust burns on rails go with it (design above).

**Open for the owner** (defaults taken, E): aligned within 2° and 0.5°/s before settling; 20 s of settling without a level sensor and as the schedule's lead; ignition 60 s past its epoch regardless of settling; the cutoff at |v_go| ≤ tail-off Δv + half a tick (no throttle-down for the last metres per second); "conflicting input" read as: a throttle change other than the program's, any rotation input, SAS off, another hold, translation against the settle (D126); a split mid-burn ends it.

### State (PLANNER-2, wrapped up at ~270k tokens, 2026-10-30)

**Landed:**
- bb43d53 Left 1, accuracy. A command reaches the engines a tick late (`propulsion::output` thrusts at the level the last tick's step set), so the old cutoff gave a full tick too much. Now `maneuver::last_ticks` counts the committed tick (the readouts' level), and takes whichever end is nearest the target: cut now (the tail-off from the level it starts at), or one or two more ticks at one throttle within the engines' range, then cut. The target is taken afresh each tick of the turn (a natural coast) and latched when settling starts, so the two-body model spans only the settling and the burn. The 40 m/s node on the small rocket: 0.19 m/s from the target (was 0.83), 0.20 m/s from the prediction, apoapsis 84 m from it (was 16 km). On 100 m/s, 0.45 m/s is left by the program's own reading (`planned_burns`: 0.48 m/s from the prediction). That is the steering's lag during the burn: the engines are 4e-6 rad off at ignition.
- 8de24aa `set_nodes`: editing a later node keeps the next node's program as scheduled. Rescheduling at the edit moved it by µs, from the LOX boiling off. A program waiting on a node that went is dropped (it stopped the vessel at a stale wake).
- 835f839 Left 2 and the sim half of Left 3. `planned_burns` (13 tests) and `hold_modes` (8) fly nodes on the small rocket, each keeping what it checks: frames = one jump, chunked = single pass with the coast untouched by nodes, the rocket equation, edits, past edits refused, saved mid-burn and mid-turn, rule 1 (a Moon anchor: 1e-8 m/s, once both start pointing the same way), burnout ends short, debug mode, an earlier target. Also: a rails frame stops at the wake, the pilot's throttle takes over and keeps the next node, the planner's warnings (the second stage's Δv; a node too soon). Not kept: `a_burn_thrusts_along_the_attitude_not_the_law`. It compares the made-up law with the attitude, and a craft of parts thrusts along its engines by the physics.

**Found:**
- **The Falcon 9's second stage empties its nitrogen on one turn** (left: Left 4's blocker). A 100 m/s node 900 s ahead: SAS's slew speeds up to 2.75°/s and spends the bottle's 2,814 mol (~79 kg) in 50 s, braking only part of it. The stage then spins at 0.04 rad/s through the burn direction and cannot settle. The MVac's gimbal then swings it round, and the burn ends 0.97 m/s off. The slew cap (`vessel::ship::turn_cap`: 10°/s in vacuum) knows nothing of the gas left. The test that shows it is in my scratchpad (`planner2/s2_test.rs`, flies in 3 s). Default taken (Open for the owner): a slew cap from the vehicle's state, the rate at which accelerating and braking cost at most a share (E: 10 %) of the RCS propellant left. That share is 2·I·ω·flow/torque from `rcs_authority`, about 0.5°/s for this stack with full tanks. `hold::turn_lead` must use the same cap, so the schedule wakes early enough. The new stage is also unrealistic here: full tanks in LEO, 125 t of inertia. A mission test after insertion carries far less.
- No shipped engine has a shutdown impulse (MVac's is 0). Layer 1's tail-off samples the exponential at each tick's end, giving ~dt/2τ (about 4 %) less impulse than published. A tick-average would match it; it is moot until an engine has one.
- The small rocket reads no tank levels, so its planned Δv is unknown and not warned of. The second stage reads them.

**Left, in order:**
1. **Game planner on nodes** (Left 3's game half): `draft_from` and `plan_from` take and send nodes for a craft of parts (`GameCommand::SetNodes` beside `SetPlan`). The window reads `Vessel::node_prediction` for the burn time, the Δv and the warnings, not `vessel.craft.engine` and `BurnCraft::turn_lead`. It needs a `predict_node(&Node)` that works for a draft node not yet set: split `schedule_node` into a schedule of a node. The line after a node, and its apsides, come from the predicted coast from `node_prediction().after` (`Vessel::coast_from`). `trajectory::predict::path` takes over from the stored coast at the node, as it does at an air entry (`Path.after`), and `needed` is true while a node waits.
2. `trajectory/predict_tests`: its three tests fly the test craft only for the air path (engine off). Move them to `small::rocket()`. `landing::model`: the landing prediction's thrust and mass come from the made-up `craft.engine` and `craft.mass.at`. They should come from the ship's definition and state (`estimate::lit`'s walk). That is its own piece; check with the lead whether it is PLANNER's or LANDING's.
3. The slew cap from the gas left (Found above), then the second stage's settling test (`planner2/s2_test.rs`: turn, settle on the nitrogen, MVac start, cutoff) as a permanent test in `tests/maneuver.rs`. Then a split mid-burn (Dragon's separation) ends the program.
4. MCP `set_plan` takes nodes for a craft of parts (`game/src/agent.rs`). The mission test: Falcon 9 to orbit, then a node on the second stage, with shots; try Dragon's Dracos after separation.
5. Left 5 above: on takeover, hand the program's last throttle and hold to the pilot's controls.
6. Docs (Left 6 above): `technical/planner.md`, the status lines, `mcp.md`, `testing.md`. Done in this wrap-up: the `owners.md` rows.

**The open-loop burn law** is no longer needed by any sim test of planned burns (`planned_burns`, `hold_modes`, `maneuver`). It is still used by: the game's planner (`plan_from`, `PlannedBurn`, `BurnCraft::turn_lead`), MCP `set_plan` (`agent.rs`), the `limbo*` low-thrust burns (ONEKIND-3's files: they go with the path, by the design above), and `game` tests on the test craft (`planner` tests, `fleet_tests`, `state_tests`). Item 1 and 4 free it in the game.

**Open for the owner** (defaults taken, E), beyond PLANNER-1's: the last ticks throttle within the engines' range to end on the target (a player's way; a sub-tick cutoff would be a layer-1 timed shutdown); the slew cap's share of the RCS propellant left (10 %).

### For PLANNER-3 (the lead, 2026-10-31)

- PLANNER-2's question 3: the `landing::model` move is yours (the brief's item 3). It blocks ONEKIND's deletion of the made-up path.
- Questions 1 and 2 stand as your defaults, under "Open for the owner" (the last ticks throttled within the engines' range; the slew cap's 10 % share of the RCS propellant left, E).
- The second-stage reproduction is at `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/planner2/s2_test.rs`. Make it a permanent test with the slew-cap fix.
- Order: the game's planner on nodes first, then `predict_tests` and `landing::model`. This unblocks ONEKIND-4. Then the slew cap, MCP `set_plan` with the mission test, takeover, docs.
- **INTERFACE** runs at the same time in `game::systems_screen` and `game::interface`, except `interface/menus/maneuver.rs`, which is yours.

### State (PLANNER-3, wrapped up at ~260k tokens, 2026-10-31)

**Landed:** 8b5e8e9, one commit: the landing change shares hunks of `trajectory/predict.rs` and `agent.rs` with the planner's, and split it would have left a commit where the game does not build.
- **The game's planner on nodes:** `draft_from(vessel)`, `nodes_from`, `command_for` (`GameCommand::SetNodes` for a craft of parts, `SetPlan` for the made-up craft). The window's numbers come from `draft_info`, which asks `Vessel::predict_node`: a node set or drafted, from a given state for a later one (`node_prediction(i)` calls it; `schedule_of` split out of `schedule_node`; `NodePrediction.mass` after the burn; `first_editable_node`, `nodes_unburnt`). A node's time is the burn's middle ("node in").
- **The line past a node:** `trajectory::predict::past_nodes` takes over from the stored segment the first unburnt node falls in, cut there, and chains each node's predicted coast (`coast_from` the node's `after`, its mass). `needed` is true while a node waits; `vessel_segments` takes over at any stored segment; a coast from a node is coloured as after a burn.
- **MCP `set_plan`** goes through `command_for`: a craft of parts gets nodes (`in_s` is the node), and the result adds `nodes` (`node_in_s`, `ignite_in_s`, `duration_s`, `warnings`) and `available_dv_ms`.
- **`landing::model`** (the lead's question 3): `landing::Thrust` (lumped: vacuum thrust, exit area, flow, lowest setting, axis) from `Thrust::of_vessel` (one owner: a craft of parts' stage estimate of the engines its throttle runs, at vacuum and at sea level), `LandingStart.thrust` and `.mass` (a craft of parts' mass and centre of mass, the mass falling with the flow), `propulsion_of`, `LandingStart::twr`. The small rocket brakes on its own engines (ignites at 1,385 m, stops 10.0 m up). The made-up engine through the lumped law gives the same bits.
- **Tests:** `predict_tests`' three on the small rocket, plus `a_node_ahead_predicts_the_coast_after_it`; the planner's round trip on parts (the node being flown kept at the front of a new draft) and its prediction's warnings (replacing the made-up round trip); `landing/thrust_tests` (2). Game suite 446 passed in a private mirror; sim's landing, `maneuver`, `planned_burns`, `hold_modes` green.

**Draft, not in the tree:** the slew cap from the gas left, `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/planner3/slew-cap.patch` (applies to 8b5e8e9 with `patch -p1`; never compiled): `vessel::ship::{gas_cap, rcs_propellant, TURN_GAS_SHARE}` (10 %), `Vessel::gas_turn_cap`; the attitude limits' `max_rate = turn_cap(q).min(gas)`, except while a gimballed engine runs (the gimbal turns the stack) or in debug mode; `hold::turn_lead_capped` for the schedule. It needs: a compile, PLANNER-2's second-stage test (copied to `planner3/s2_test.rs`) as a permanent test in `tests/maneuver.rs`, and `maneuver`, `planned_burns`, `hold_modes` green (the cap may change the small rocket's timing).

**The open-loop burn law:** nothing in sim or game needs it for a craft of parts any more. ONEKIND-4 may delete it with the made-up craft, together with the made-up branches that only serve it: `planner::{draft_from, plan_from, draft_info, command_for}`'s made-up halves and `GameCommand::SetPlan`, `turn_warning`, `burn_time`, `BurnCraft::turn_lead`; `landing::Thrust::of_engine` and `thrust_tests::the_made_up_engine…` (with `craft.engine`); `LandingStart.mass` then stops being an `Option`. Still on the made-up engine elsewhere (not planned burns): `interface/menus/data.rs` (`burns(sim)` from `plan()`, `landing::twr(&ship.craft, …)` at :425, the Δv at :506-521; INTERFACE's file), `scene/ships.rs:548`, `navball/mod.rs:370`. The game's planner tests on test ships (`an_intercept…`, `opening_the_planner…`) use `spawn_test_ships`.

**Left, in order:**
1. The slew cap (the draft above), then the second stage's settling test permanent; then a split mid-burn (Dragon's separation) ends the program, with its test.
2. The Maneuver menu lists a craft of parts' nodes (`interface/menus/maneuver.rs` is ours; its rows are built in `menus/data.rs::burns`, INTERFACE's: a small hunk from `nodes_unburnt` and `predict_node`).
3. The mission test (D122): Falcon 9 to orbit headless, a node on the second stage through `set_plan`, shots of the turn, the burn and the orbit after; Dracos after separation. It needs 1 (the second stage's nitrogen).
4. Takeover hands the program's last throttle and hold to the pilot's controls (PLANNER-1's Left 5).
5. Docs: `technical/planner.md` (new), the status lines in `systems/motion.md`, `propulsion.md`, `avionics.md` (*skeleton*: the game plans nodes, unflown in a mission test), `testing.md`. Done: the owners rows (they went in with INTERFACE's f7aff55), `mcp.md`'s `set_plan` row.

**Open for the owner** (defaults taken, E): the landing prediction with unmeasured propellant assumes the engines do not run dry and holds the mass (the small rocket reads no tank levels; reading the truth would be the software knowing what it cannot sense); a node's time is the burn's middle, in the planner and MCP; the gas cap's 10 % share stands.


### State (PLANNER-4, 2026-10-31)

**Landed:**
- 178e814 **The turn on the gas the thrusters can afford** (PLANNER-3's draft, compiled and fixed): a craft of parts slews no faster than spinning up and stopping costs 10 % (E) of its thrusters' propellant left (`vessel::ship::gas_cap`, `Vessel::gas_turn_cap`; the schedule's `hold::turn_lead_capped`), unless a gimballed engine runs or in debug mode. The second stage with Dragon and full tanks turns at 0.25°/s on 14 % of its nitrogen (it emptied the bottle before). **The steering is held for a burn's last 2 s** (E, `maneuver::STEER_HOLD`, `steers`): near the cutoff v_go's direction swung 12° in 0.8 s and the MVac's gimbal chasing it left the stage spinning at 0.14 rad/s. PLANNER-2's second-stage test is permanent: 0.06 m/s and 7 m from its target, 0.0002 rad/s at the cutoff. The small rocket's 40 m/s node: 0.08 m/s (was 0.19).
- a2a9d55 **A separation mid-burn** ends the burn, logged as an alarm ("burn N: ended by the separation"); later nodes stay with the kept piece; the others leave with no plan.
- 1eb6715 **The Maneuver menu** lists the burn being flown, then the nodes not yet burnt (numbered as the log numbers burns), with Δv, ignition countdown and duration from `predict_node`, and what the program is doing.
- 431155d **Takeover hands the burn over as it was flying** (`maneuver::handover`, `Helm::hand_over`, `Helm::current`): in what the program commanded and the pilot had not changed, the program's throttle and SAS on stability; the rest of that step reads the handed controls (a closed throttle for one tick would shut the engine down). A separation hands over the same way (`Program::throttle`).
- 2dfd940 The planner warns when a craft has no main engine to burn a node (Dragon after separation).
- Docs (this commit): `technical/planner.md` (new), the status lines (motion, propulsion, avionics: *playable* on main engines), `owners.md` rows, `testing.md`'s planned-burn example, the plan's row.

**The mission test** (D122): from INTERFACE-2's orbit save (192 × 343 km, debug off), MCP `set_plan` with 43.9 m/s prograde at the apoapsis and 20 m/s 46 min later. Both woke out of warp 6, turned, settled, lit the MVac and cut off inside its 3 s start, "on target (0.26 / 0.27 m/s left)": one tick at the MVac's 64 % floor is ~0.6 m/s on this light stage. Burn 1 gave 340 × 353 km (e 0.0009); after both the Orbital menu read 412.1 × 343.2 km against the planner's 413.2 × 343.5. Shots, in `/private/tmp/claude-501/-Users-ben-sunscatter/7c5f130a-69ba-45d3-9965-ca858b31253e/scratchpad/planner4/shots/`: `01-menu-nodes.png` (both nodes listed), `02-turning.png` ("turning to the burn"), `04-settling.png` ("settling the propellant"), `05-burning.png` (the plume; the stage in Earth's shadow), `06-after-burn1.png`, `07-after-burn2.png` (the stack lit, the orbit, "no burns planned", the log). Save after: `planner4/home/saves/planner4-orbit-after.ron`.

**INTERFACE-2's finding 20** (the lead's question): not the engine. In the sim a closed throttle shuts the running MVac down at once (D114): one more tick of propellant (the tick already committed, which `last_ticks` counts), then off, with no tail-off (its shutdown impulse is 0, and so is the program's tail term). The 80 km is `get_vessel`'s apsides lagging: they come from the predicted path, recomputed every 0.25 s of real time as an engine-off coast (`state::update_prediction`). 1 s into burn 2 they read 343.9 km while the state's osculating apoapsis was 360.0 km (the `step` answer's `apoapsis_m`). For the game's owner (INTERFACE/agent): recompute the prediction before an MCP answer at a stop, or give the osculating apsides while powered.

**Found, not fixed:**
- Dragon's Dracos cannot fly a planned burn: the program burns main engines only (now warned). D125's same mechanism needs the program to command a translation on thruster blocks in place of an ignition.
- With the pilot's throttle at 0 at the wake, X (cut) during a burn changes nothing and does not take over (no change from the baseline).
- The navball's hold buttons show the pilot's hold (STAB) while the program flies the maneuver hold.
- During the turn the stage was in Earth's shadow: the shots show only the plume. A lit burn needs a node on the day side.

**Left, in order:**
1. A throttle cut during a burn when the throttle was already closed (above): the program moving the pilot's throttle as it flies (an autothrottle moving the lever) makes X a conflict and the hand-over trivial. A change to PLANNER-1's design: open for the owner.
2. Planned burns on translation thrusters (Dragon's Dracos, D125).
3. The navball's hold buttons during a program; the game's planner window used on screen with nodes (a shot).
4. `get_vessel`'s apsides while powered (finding 20; the game's owner).

**Open for the owner** (defaults taken, E), beyond PLANNER-1..3's: the steering held for a burn's last 2 s; the hand-over gives the program's throttle and SAS on stability in what the pilot had not changed; a separation mid-burn hands over the same way.
