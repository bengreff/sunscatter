# Brief: PLANNER, planned burns for craft of parts (2026-10-30)

D126: planned burns run closed-loop to their target state, with real engine starts, ullage settling and tail-off, as flight-layer programs. Today only the test craft can plan a burn (`Vessel::set_plan` returns `PlanError::PartsUnsupported` for a craft of parts), and its burn is open-loop: steady thrust at a constant mass flow along a direction law (`sim::vessel::burn`). Follow [agent-rules.md](agent-rules.md). Your name is PLANNER and your target dir is `target/agents/planner` (a cold start).

Two other agents run at the same time:
- **ONEKIND-3** ([brief-onekind.md](brief-onekind.md)) moves the other tests onto craft of parts and rebuilds the stress craft. It deletes the made-up path and the test craft only **after** you land: leave the open-loop burn law in place, and tell the lead in your State when your tests no longer need it.
- **AUDIT** ([brief-audit.md](brief-audit.md)) reads the code for D129 (the same course of events at every warp) and D128 (the software layers), and writes only new docs and test files.

## Read first

- Decisions: D126 (revised tonight: nodes only; an automatic burn settles with the craft's thrusters; the takeover rule), D128 (the three software layers: your program is layer 2), D129 (burns fly live for now; the same course of events at every warp), D075 (planned burns are flown by the maneuver hold), D029, D111, D022. The owner's answers: [interview-2026-10-30-roadmap.md](../archive/interviews/interview-2026-10-30-roadmap.md).
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

(Update as you land: commits with hashes, tests moved and any not kept, what is left in order, Open for the owner.)
