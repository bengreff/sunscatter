# The burn planner and planned burns

As built for craft of parts in `sim::systems::maneuver` (the program, layer 2 of D128), `sim::vessel::maneuver` (the vessel side), `game::planner` (the window) and `game::trajectory::predict` (the line past a node). Decisions: D126 (planned burns closed-loop, nodes only, settling on the craft's own thrusters, the takeover rule), D128, D129 (burns fly live; the same course of events at every warp), D075, D011. Built by PLANNER-1 to PLANNER-4 (2026-10-30/31), from the old spec in [realism-1.md](../archive/plans/realism-1.md) §6a. The system: [motion](../systems/motion.md).

**Status: playable** on main engines. The sim flies nodes on the small rocket and the Falcon 9's second stage under tests; the game plans, draws and lists them, MCP `set_plan` sets them, and a mission test flew two on the Falcon 9's second stage in the headless game (below). The planner window itself has not been used on screen since it moved to nodes. The made-up craft's open-loop burns (`sim::vessel::burn`) go with that craft (ONEKIND).

## A node

A node is a Δv along prograde, normal and radial-out at an epoch, relative to a named body (`maneuver::Node`). The body is a choice of axes, never physics (rule 1). The node's time is the burn's middle, in the planner and in MCP. Nodes live on the vessel (`Vessel::nodes`, saved), in order; a node being turned to, settled for or burnt cannot change (`first_editable_node`). Nodes leave the stored trajectory as it is: the coast is unchanged, and the vessel stops at the program's wake epoch like an order, whatever the warp (D129).

## The program

`maneuver::step(&Inputs, &mut Program) -> Commands`, one tick at a time. It reads only what the flight software sees (`flight::sense`: the attitude, the navigation solution, the engine controllers' readouts, the feeds' settled flags where a level sensor reads them), the node, the stage estimate of its engines (`estimate::lit`, the staging bar's walk) and the static definition. It issues what a player issues: SAS on with the maneuver hold along a direction, the throttle, a translation, ignitions. The vessel lays them over the pilot's controls at the start of each live tick (`Vessel::tick_controls`, used by every tick loop).

- **The schedule** (`maneuver::schedule`, at `set_nodes` and when a burn ends): ignition = the node − the time to give half the Δv (the estimate's thrust, flow and mass) − half the engines' start; the wake = ignition − the turn lead (`hold::turn_lead_capped`, at the slew the gas affords) − the settling lead (`SETTLE_LEAD`, 20 s, E).
- **The target** is the orbit the node gives: the navigation state carried to the node along its conic (`kepler::propagate`, the software's model), plus the node's Δv in its axes there. It is taken afresh each tick of the turn and latched when settling begins, so the two-body model spans only the settling and the burn.
- **Steps:** *wait* → (the wake) *turn* (the maneuver hold on the burn direction) → (aligned within 2° and turning under 0.5°/s, E, or ignition due) *settle* (translation along the thrust axis while a sensor says a feed is unsettled; with no sensor, for the settling lead) → (settled at or after ignition, or 60 s after it regardless, E) *ignite* (`Ignite` for each engine, throttle 1: the engines' own start, which on floating propellant does what layer 1 does) → *burn* → *tail-off* (throttle 0) → *done*.
- **Steering:** along v_go, the target's velocity at the node minus the current state's carried there (both two-body about the node's body). In the burn's last 2 s (`STEER_HOLD`, E) the direction is held: near the cutoff errors across v_go swing its direction faster than any turn could follow (a gimballed stage chasing it ended the burn spinning).
- **Cutoff** (`last_ticks`): a command reaches the engines a tick late, so the committed tick counts. The program takes whichever end is nearest the target: cut now (the tail-off from the level it starts at), or one or two more ticks at one throttle within the engines' range, then cut. It also cuts when v_go turns against the burn (`overshot`).
- **Ends:** *done* on target (the log gives the m/s left); *short* when an engine stops on its own or never starts, or with no navigation solution; *manual* on the pilot's conflicting input; a split while it flies ends the burn ("ended by the separation", an alarm), as its engines' indices were the whole craft's. After any end, the next node's program is scheduled.

## Taking over (D126)

The pilot's controls at the wake are the baseline (`maneuver::conflicts`). Input that equals what the program commands, or touches what it does not command (chutes, the RCS mode, the speed reference, the target, translation outside the settling), leaves it running. A changed throttle, any rotation input, SAS off, another hold, or translation against the settling switches the burn to manual: the node is spent, later nodes stay.

The burn is handed over as it was flying (`maneuver::handover`): in what the program commanded and the pilot had not changed, the pilot's controls take the program's throttle, and SAS on with the stability hold (it holds the burn's attitude, or where a rotation input leaves it). What the pilot changed stays theirs. The helm takes the handed controls as set and received at once (`Helm::hand_over`), and the rest of that step reads them in place of the controls passed from before (`Helm::current`): a closed throttle for even a tick would shut the engine down. A separation mid-burn hands over the same way, from the program's last throttle and direction.

## The slew a turn can afford

A craft of parts slews no faster than its thrusters' propellant affords (`vessel::ship::gas_cap`, `Vessel::gas_turn_cap`): spinning up and stopping about an axis cost `2·I·ω·flow/τ` of the propellant the enabled blocks draw on, at most 10 % (E, `TURN_GAS_SHARE`) of what they have left. The Falcon 9's second stage with Dragon and full tanks turns at 0.25°/s (the turn cap allows 10°/s in vacuum); it emptied its nitrogen on one turn before this. A running gimballed engine turns the stack without gas, and debug mode lifts the cap.

## What the planner shows

`Vessel::predict_node` (one owner): for a node set or drafted, the state just after it (the stored coast's state at the node, or a predicted path's past an earlier node, plus the Δv at once), the ignition and duration from the stage estimate, the mass after, the Δv the engines have (if the tanks are measured), and warnings: no main engine to burn it, no thrusters push along the engines (the propellant cannot be settled), not enough Δv, too soon for the turn and settling. It is labelled as a prediction; the stored trajectory is what was flown.

- **The window** (`game::planner`): a draft of nodes edited by handles or numbers, sent as one command (`GameCommand::SetNodes`). Its numbers come from `draft_info`.
- **The line** past a node (`trajectory::predict::past_nodes`): the stored segment is cut at the first unburnt node, then each node's predicted coast is chained, coloured as after a burn.
- **The navball** draws the controls the vessel flies while the program flies a burn (`Vessel::flown_controls`, b246f9d: SAS, the MANEUVER hold, the program's throttle), not the pilot's.
- **The Maneuver menu** (key 3): the burn being flown, then the nodes not yet burnt, numbered as the ship's log numbers burns, with Δv, the ignition countdown, the duration, and what the program is doing ("turning to the burn", "settling the propellant", "burning").
- **MCP `set_plan`** ([mcp.md](mcp.md)): `in_s` is the node; the result gives each node's ignition, duration and warnings, and the Δv available.

## Tests

`crates/sim/tests/maneuver.rs` (the small rocket's 40 m/s node: 0.08 m/s from its target, the same bits in 60 fps frames as in one jump; the second stage's 100 m/s node: the turn at the gas cap on 14 % of its nitrogen, settling, the MVac's start, 0.06 m/s from the target, not spinning; a separation mid-burn; a rotation nudge mid-burn handed over), `planned_burns.rs` (13: frames = one jump, chunked = single pass with the coast untouched, the rocket equation, edits, saved mid-burn and mid-turn, rule 1 with a Moon anchor, burnout, debug mode), `hold_modes.rs`; the rules' tables in `systems/maneuver_tests.rs` (v_go, alignment, last ticks, overshoot, steering, takeover, hand-over, schedule, the steps); the game's planner and `trajectory/predict_tests`.

## The mission test (PLANNER-4, 2026-10-31)

From INTERFACE-2's orbit save (the Falcon 9 to orbit with debug off; second stage and Dragon, 192 × 343 km, 4.9 t of propellant left), headless through MCP: `set_plan` with two prograde nodes, 43.9 m/s at the apoapsis (circularise) and 20 m/s 46 min later. The answer gave 786 m/s available, ignitions 2.0 and 1.7 s before the nodes, burns of 1.0 and 0.46 s, no warnings. Each node woke at its own epoch out of warp 6 ("burn N: turning to the burn"), turned, settled on the nitrogen, lit the MVac and cut off inside its 3 s start: "burn 1: on target (0.26 m/s left)", "burn 2: on target (0.27 m/s left)". On this light stage one tick at the MVac's 64 % floor is about 0.6 m/s, so ending within 0.3 m/s is the tick's resolution. Burn 1 made the orbit 340 × 353 km (osculating, e 0.0009); after both, the Orbital menu read 412.1 × 343.2 km against the planner's 413.2 × 343.5 km. Shots: the Maneuver menu with both nodes, its "turning to the burn" and "settling the propellant", the plume mid-burn (the stage in Earth's shadow), the orbit after (the PLANNER-4 State in [brief-planner.md](../plans/brief-planner.md) lists them).

Found on the way: `get_vessel`'s apsides come from the predicted path, which the game recomputes every 0.25 s of real time as an engine-off coast. While an engine runs, they lag the state: 1 s into burn 2 they read 343.9 km while the state's osculating apoapsis was 360.0 km, which the `step` answer's `apoapsis_m` gave. That explains INTERFACE-2's "apoapsis rose 80 km in the 2 s after `throttle: 0`". In the sim a closed throttle shuts the MVac down at once (D114): one more tick of propellant, the tick already committed, then off, with no tail-off (its shutdown impulse is 0, and so is the program's tail term).

## Not done

- **Dragon's Dracos** cannot fly a planned burn: the program burns a craft's main engines, and Dragon has none (its Dracos are thruster blocks). The planner warns "no main engine to burn it". A burn on translation thrusters is the same mechanism with the thrusters as the engine (D125); it needs the program to command a translation in place of an ignition.
- **A throttle cut with the throttle already closed:** the pilot's controls at the wake are the baseline, so with the throttle at 0 then, X (cut) during the burn changes nothing and does not take over. The program moving the pilot's throttle as it flies (an autothrottle moving the lever) would fix it; open for the owner.
- **Low-thrust burns on rails** (`limbo.md`) are D029's warp while thrusting; they come later.
- No shipped engine has a shutdown impulse (the MVac's is 0), so the cutoff's tail-off term is untested against a real one.
