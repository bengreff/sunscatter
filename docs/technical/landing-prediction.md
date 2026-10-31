# The landing prediction

As built in `sim::landing` (`mod.rs`: the starts, the impact, the braking solution, the air path; `model.rs`: the dynamics and the integration loop) and `game::landing` (the background task and the impact marker). Decisions: D089 (the orbit line through air), D074 (a stable craft weathervanes), D064 (debug mode), D080 (the marker only with its menu). From the old specs in [playtest-1.md](../archive/plans/playtest-1.md) C and [realism-1.md](../archive/plans/realism-1.md) §6b. The system: [motion](../systems/motion.md). Its designed successor, the predictor of descents with a burn plan that the booster's landing program steers by: [landing-guidance.md](landing-guidance.md) (design).

**Status: playable.** Under tests on the made-up craft and on the small rocket of parts; the game's Landing/Reentry menu, the impact marker and MCP `get_landing_prediction` read it. No mission test has landed a craft of parts with it yet.

## What it integrates

`predict_impact(world, craft, start, limits)` integrates the vessel's own force model, not a coast approximation, from `LandingStart::of_vessel` (the vessel's time and state):
- N-body gravity with J2 and the tidal cutoff (`ForceContext`, as a coast);
- drag and lift from the craft's aerodynamic bake (`AeroTick`) at the actual Mach and Knudsen numbers, against the air turning with the body, evaluated at every stage of an adaptive Dormand–Prince integration (`sim::math` only). The live tick freezes the coefficients over each 20 ms tick; the difference is well under a metre over a descent;
- the parachutes, if deployed or armed by the command (`ChuteState::predicted_cd_area`);
- the current throttle held, thrust at ambient pressure along the assumed attitude, and the mass falling with the propellant burned.

**The engines** are lumped (`Thrust::of_vessel`): a craft of parts' stage estimate of the engines its throttle runs (those selected, else all main engines; `estimate::lit`, the staging bar's walk), at full throttle in vacuum and at sea level, along their mean axis, with the highest minimum throttle. Its mass and centre of mass are the ship's at the start; the centre is held. The propellant burnable is the estimate's to burnout, or infinite when no level sensor measures it (the prediction then holds the mass). In debug mode the propellant never runs out. The made-up craft's fallback (`Thrust::of_engine`, `LandingStart.mass: None`) goes with it (ONEKIND).

**The assumed attitude** (`AssumedAttitude`), chosen by the game from the controls (`landing::assumed_attitude`, a table test): SAS off, nose into the relative wind (`SurfacePrograde`: a stable craft left to itself, D074); a surface retrograde or prograde hold, that direction; any other hold, the current attitude held inertially (exact for stability, a fair guess for the rest). Straight up at rest.

**What the software knows:** a prediction needs the flight software's navigation solution, and for an inertial attitude its attitude knowledge (`flight::can_predict`, F3). Landed and crashed vessels have none.

## Where it ends

The lowest contact point at the assumed attitude meets the physical surface (`surface_height`, by 60-step bisection on the step's interpolant, as a coast's contact), within its own horizon (`Limits::two_orbits`: two revolutions of the osculating orbit about the reference body if bound, else a day, at most a day; at most 40,000 steps), independent of the drawn orbit line. Steps are at most 2 s while thrusting or within 20 km of a surface; surfaces farther than their radius plus 200 km are not checked. The tolerance is 1 cm and 0.1 mm/s.

The impact (`Impact`) is body-fixed: the ground point under the centre of mass, its latitude and longitude, and the surface-relative speed split along the local vertical. The game draws it where that ground is *now*, carried by the body's rotation (`Impact::ground_at`). The reference body for the surface quantities is the one whose ground is nearest (`nearest_surface`).

## The braking solution

`braking_solution(world, craft, descent, margin)`: the latest ignition of a full-thrust burn against the surface-relative velocity (thrust at ambient pressure, throttle limits, falling mass, gravity, the body's rotation, drag) that stops the descent with the lowest contact point `margin` above the surface. Shooting from the predicted path, bisecting the ignition time to 1 ms (`IGNITION_PRECISION`: a quarter of a metre at 250 m/s); a crash counts as minus the contact speed. `None` without an impact, or when even igniting at the start does not stop in time ("cannot stop"). The game asks for it within 30 minutes of impact, with a 10 m margin (`BRAKING_WITHIN`, `BRAKING_MARGIN`).

## The air path (D089)

`air_path` is the same model for the orbit line: where a vessel flies live in air, or its coast enters air that needs live flight, the line follows the vessel's own force model until the air is clear for coasting by the vessel's own hand-over rule (`limbo::may_coast`), then the coast. Its result is a finished segment the line samples (rule 4).

## In the game and MCP

- `game::landing::LandingPrediction`: a background task every 0.25 s of real time, swapped in when complete.
- The Landing/Reentry menu ([flight-menus.md](flight-menus.md)): radar altitude, vertical speed, impact time, vertical and horizontal speed and place; BRAKING with the local TWR (thrust at ambient pressure against local gravity, `LandingStart::twr`), BURN IN and IGNITE AT, or "cannot stop".
- The impact marker, drawn only while that menu is open (`landing::impact_marker_shown`). The prediction also feeds the apsis list's impact (one impact source).
- MCP `get_landing_prediction` returns the same ([mcp.md](mcp.md)).

## Tests

`landing/tests.rs`: a vertical drop on the Moon is free fall (against the analytic); the impact on the rotating Earth matches a brute-force tick integration of the same model; under the parachute the impact speed is the terminal velocity; the braking solution flown with ticks stops at the margin; a craft that cannot stop has none; a craft already touching the ground; determinism and anchor independence; a deorbit from low orbit lands within half an orbit and a low lunar orbit does not; the TWR at ambient pressure. `landing/thrust_tests.rs`: a craft of parts predicts with its own engines and mass. The game's `landing::tests` (the assumed attitude's table).

## Not done

- The prediction holds the throttle; it takes no burn plan. A plan (boostback, entry and landing burns) is [landing-guidance.md](landing-guidance.md)'s predictor (design).
- The braking solution burns along surface retrograde at full thrust; it does not throttle or divert.
