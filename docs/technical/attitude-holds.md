# Attitude holds

As built in `sim::vessel::hold` (the rule), `sim::vessel::attitude` (the controller) and `sim::vessel::attitude::limits` (a craft of parts' limits); the game's navball shows and sets them. Decisions: D075 (hold modes), D056 (the automatic reference body), D126 (the maneuver hold follows the next node), rule 1. From the old spec in [playtest-1.md](../archive/plans/playtest-1.md) B. The system: [avionics](../systems/avionics.md) (SAS is flight software); the frames a hold is followed in on rails: [limbo.md](limbo.md).

**Status: playable.** Every mode is flown under tests on craft of parts, live and on rails, and the navball's buttons and MCP `set_controls` set them. Planned burns are flown by the maneuver program ([planner.md](planner.md)), which commands the maneuver hold like a pilot. The made-up craft's open-loop burns, which this module once flew on the tick lattice (`vessel::flown`), go with that craft (ONEKIND).

## The modes

`HoldMode` with a `SpeedReference` (`Orbit`, `Surface`, `Target`) and a reference body (`Controls::reference`). `hold_direction(mode, speed, inputs)` is the one rule: a pure function of the vessel's state relative to the reference body, the target's relative state and the next node's thrust direction, giving a unit inertial direction or none.

| Mode | Holds | None when |
|---|---|---|
| Stability | the attitude where the rotation stopped (rate damping first) | always (it is the fallback) |
| Prograde / Retrograde | the nose along / against the velocity in the speed reference: about the body's centre (inertial axes), over its rotating surface, or relative to the target | the speed is under 0.1 m/s (`MIN_SPEED`, the navball's rule too) |
| Target / Anti-target | the nose towards / away from the target | no target |
| Maneuver | the engines' thrust axis along the maneuver program's direction while it flies, else the next node's (`burn::node_direction`) | no node and no program |

A mode with no direction holds the attitude (stability). Direction holds turn the nose (the thrust axis for the maneuver hold) by the smallest rotation (`quat_arc`), so the roll carries over from where the hold began.

- **The reference body** is a control choice, never physics (rule 1): the body the player names (a control input, applied at a frame like any command), or by default the automatic one, `hold_reference`. That is the deepest body whose Laplace sphere `d·(m/M)^(2/5)` about its primary (`primary`, `laplace_radius`) contains the vessel at that instant: D056's dominance as a pure function of time and state, the same for every anchor. Only a mode that reads the velocity looks it up. A coast held in an automatic frame ends where that body changes ([limbo.md](limbo.md)).
- **The target** is a body or a vessel state the game refreshes each frame (`TargetState`: a position and velocity at an epoch, carried at constant velocity), so the sim needs no vessel ids.

## The controller

SAS is a controller in the flight software's place: it outputs a torque within the authority (`attitude::command`), from the RCS blocks and, while an engine runs, its gimbal.

- **Closing rate:** an error is closed at `ω/2·|e|` near the target (critically damped), at most `√(α·|e|)` far from it, so the rotation can stop braking at half the angular acceleration. The hold frequency is 1 rad/s, lower for a craft of parts whose authority could not hold a 0.05 rad error without saturating (`limits::hold_omega`, `LINEAR_ERROR`).
- **The slew cap** for a craft of parts: `ship::turn_cap(q)`, 10°/s up to a dynamic pressure of 1 kPa, then falling as 1/q, never under 0.5°/s; and the gas its thrusters can afford (`Vessel::gas_turn_cap`: spinning up and stopping cost at most 10 % (E) of the propellant they have left; [planner.md](planner.md)). A running gimballed engine turns the stack without gas, and debug mode lifts the gas cap.
- **Snapping:** a direction hold snaps onto its moving target within 10 µrad and 10 µrad/s (`TRACK_SNAP`), a stability hold within 1 µrad (`HOLD_SNAP`).
- **On rails:** a settled direction hold is followed without control ticks on a 60 s lattice (`FOLLOW_STEP`): the attitude there is the last one turned by the smallest rotation onto the direction (`hold::follow`). A direction that moves more than 0.2 rad in one step is a jump (a new reference body, say), and the hold turns with ticks again. Ticks resume from the target one tick back at the craft's rate (`resume_hold`).
- **The turn lead** (`turn_lead`): the time to turn half a revolution from rest and settle, `3·√(2π/(3α)) + 30 s`, α the smaller pitch or yaw authority over the inertia, rounded up to a tick. `turn_lead_capped` takes the slew cap's half turn, `π/ω_max + 1.5·ω_max/α + 30 s`, where that is longer. The maneuver program wakes this far before ignition.

## In the game and MCP

- The navball's hold buttons (STAB, PRO, RETRO, TGT, ANTI, MNV) set the mode; target modes need a target and the maneuver mode a node (`navball::rules::hold_available`). T toggles SAS. The SAS row states the mode and, for prograde and retrograde, the speed reference and body (`rules::sas_label`: "PROGRADE (ORBIT Earth)"). The navball reads the same reference rule (`navball::nav_state`).
- MCP `set_controls` takes `hold` (`stability`, `prograde`, `retrograde`, `target`, `anti_target`, `maneuver`) and `speed_reference` ([mcp.md](mcp.md)).
- A split's pieces keep the hold (`Controls::for_piece`), and every vessel flies its own controls (`sim::vessel::helm`).

## Tests

`hold::tests` (the direction table), `crates/sim/tests/hold_modes.rs`: the maneuver hold turns the craft before ignition; the coast after a burn is what was flown; holds and burns, and turning attitudes, are the same at 60 fps and in one jump; a coast depends on the attitude only through drag and light; holds point where their rule says on rails; a vessel saved mid-turn continues bit-identically; a long burn inside a rails-warp frame flies its live ticks. The navball's rules in `navball/rules_tests.rs` (`sas_label`).

## Not done

- **The navball** shows the pilot's hold (STAB) while the maneuver program flies the maneuver hold ([planner.md](planner.md)).
- **Holds through the fins alone** (an angle of attack held by the grid fins) are in [landing-guidance.md](landing-guidance.md) (design).
