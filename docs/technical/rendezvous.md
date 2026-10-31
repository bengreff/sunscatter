# Rendezvous tools

As built in `sim::approach` (closest approaches), `game::rendezvous` (the active vessel against its target), `game::planner::intercept` (a first-guess intercept) and MCP `closest_approaches`. Decisions: D008 (patched conics only as a first guess), rule 3 (relative states through the frame tree), rule 4 (nothing integrated: the stored paths are read). From the old spec in [realism-1.md](../archive/plans/realism-1.md) §6c. The system: [motion](../systems/motion.md); planned burns: [planner.md](planner.md); craft near each other: [proximity.md](proximity.md).

**Status: playable** for closest approaches (map markers, the Orbital menu's Target section, MCP). The intercept helper is a *skeleton*: a two-body first guess with no correction on the N-body plan.

## Closest approaches

`closest_approaches(world, a, b, t0, t1, samples)`: each path is a function of time giving `(anchor, r, v)`, a vessel's stored trajectory (`Trajectory::eval`) or a body through the ephemeris. The relative state is formed through the frame tree at each time, the distance sampled on a uniform grid of at least 3 points, and every local minimum refined by golden-section search on the dense output (60 iterations: f64 resolution of any sampling interval). A minimum at an end of the span counts only while the distance is still falling there. The result is nearest first: the epoch, the distance and the relative speed.

## In the game

- **`game::rendezvous`:** the active vessel against the navball's target, a vessel (both stored trajectories) or a body (its ephemeris: the pass over the Moon after a transfer burn), over the span both have computed, 400 samples, refreshed every 0.5 s of real time.
- **The map** marks the nearest approach on each line (the target's own marker only for a vessel; a body is drawn itself).
- **Orbital ▸ Target** ([flight-menus.md](flight-menus.md)): distance, relative speed, the closest approach (PASS over a body), and the relative inclination.
- **The intercept helper** (the planner's "+ intercept the target (first guess)"): two-body Lambert arcs (`sim::kepler::lambert`) about the vessel's dominant body, 24 departures over the next orbit and 24 transfer times of 0.1 to 1.5 orbits, the cheapest kept as a draft node in prograde, normal and radial axes. Positions come from both stored trajectories; the node is then predicted and drawn by the N-body sim, so the planner shows how close it really gets. "No intercept found in the computed lines" when none solves.

## MCP

`closest_approaches` (two vessel ids): the five nearest over the span both stored trajectories cover, 400 samples: seconds from now, distance and relative speed ([mcp.md](mcp.md)).

## Tests

`approach::tests`: two coplanar circles meet where the phases line up (against the analytic); the golden section finds a parabola's minimum. The planner's intercept test (`an_intercept_between_two_test_ships_is_found_and_affordable`) runs on test ships.

## Not done

- **Correction on the N-body plan:** the spec's differential correction of the Lambert guess (a miss under 100 m in the N-body plan) is not built; the guess is drawn as it is.
- **The intercept helper on a craft of parts** has not been used on screen since the planner moved to nodes.
- No tool matches velocity at the approach, and there is no docking (D082: later). Craft within 10 km fly live together and collide ([proximity.md](proximity.md)).
